//! Train the Gomoku policy/value network from synthetic data.
//!
//! Usage:
//!
//! ```text
//! nettrain --out <DIR> [--data <DIR>] [--config tiny|mid|default]
//!          [--device flex|wgpu|metal] [--seed <u64>] [--steps <n>]
//!          [--batch-size <n>] [--eval-interval <n>] [--lr-max <f>]
//! ```
//!
//! If `--data` is omitted, a tiny synthetic dataset is generated in-memory
//! for a quick smoke run.

use std::env;
use std::path::PathBuf;
use std::process;

use burn::tensor::Device;
use rand::SeedableRng;
use rand::rngs::StdRng;

use net::checkpoint::{RunJournal, save_checkpoint};
use net::network::{Model, ModelConfig};
use net::report::StopReason;
use net::train::{EvalMetrics, StepMetrics, TrainConfig, train};
use train::collect::{Quotas, collect};
use train::dataset::read_dataset;
use train::split::is_holdout;

/// The compute device selected on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeviceChoice {
    /// Pure-Rust CPU (verified, deterministic; the default).
    Flex,
    /// Wgpu (Metal on macOS via WGSL); requires the `gpu` cargo
    /// feature. Training parity with Flex is not verified
    /// (docs/WARM-UP); treat runs as experiments.
    Wgpu,
    /// Wgpu with native MSL shaders instead of WGSL; requires the
    /// `gpu` cargo feature. Measured ~2x faster than `Wgpu` on this
    /// network (layerbench example, 2026-11-11). Same parity caveat.
    Metal,
}

/// Parsed command-line arguments.
#[derive(Debug)]
struct Args {
    /// Output directory for the checkpoint and run journal.
    out_dir: PathBuf,
    /// Optional dataset directory. If omitted, a tiny dataset is generated.
    data_dir: Option<PathBuf>,
    /// Network size preset.
    config: ModelConfig,
    /// Training hyperparameters.
    train_config: TrainConfig,
    /// Compute device.
    device: DeviceChoice,
}

fn main() {
    let args = match parse_args(env::args().skip(1)) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            eprintln!(
                "Usage: nettrain --out <DIR> [--data <DIR>] [--config tiny|mid|default]\n\
                 [--device flex|wgpu|metal] [--seed <u64>] [--steps <n>] [--batch-size <n>]\n\
                 [--eval-interval <n>] [--lr-max <f>]\n\
                 Use `--flag value` only; `--flag=value` is not supported."
            );
            process::exit(2);
        }
    };

    if let Err(e) = run(args) {
        eprintln!("nettrain failed: {e}");
        process::exit(1);
    }
}

fn run(args: Args) -> Result<(), String> {
    // Training needs gradient tracking: an autodiff device (docs/11 trap).
    let device = match args.device {
        DeviceChoice::Flex => Device::flex().autodiff(),
        DeviceChoice::Wgpu => wgpu_device()?,
        DeviceChoice::Metal => metal_device()?,
    };

    eprintln!(
        "nettrain: device={}, seed={}, steps={}, batch_size={}, eval_interval={}",
        match args.device {
            DeviceChoice::Flex => "flex",
            DeviceChoice::Wgpu => "wgpu",
            DeviceChoice::Metal => "metal",
        },
        args.train_config.seed,
        args.train_config.steps,
        args.train_config.batch_size,
        args.train_config.eval_interval
    );

    let (train_samples, holdout_samples) = load_or_generate_data(&args)?;
    eprintln!(
        "Training on {} samples, holdout {} samples",
        train_samples.len(),
        holdout_samples.len()
    );

    let model: Model = args.config.init(&device);
    let (model, step_metrics, eval_metrics) = train(
        model,
        &args.train_config,
        &args.config,
        train_samples,
        holdout_samples,
        &device,
    );

    let journal = RunJournal::new(
        args.train_config.seed,
        args.config,
        args.train_config,
        step_metrics.last().copied(),
        eval_metrics.last().copied(),
        StopReason::Completed,
    );

    save_checkpoint(&model, &journal, &args.out_dir)
        .map_err(|e| format!("failed to save checkpoint: {e}"))?;

    print_report(&step_metrics, &eval_metrics, &args.out_dir);

    Ok(())
}

fn load_or_generate_data(args: &Args) -> Result<(Vec<train::Sample>, Vec<train::Sample>), String> {
    let samples = if let Some(dir) = &args.data_dir {
        read_dataset(dir).map_err(|e| format!("failed to read dataset: {e}"))?
    } else {
        let quotas = Quotas {
            win: 50,
            block: 50,
            quiet: 100,
        };
        let mut rng = StdRng::seed_from_u64(args.train_config.seed);
        collect(quotas, &mut rng)
    };

    if samples.is_empty() {
        return Err("dataset is empty".to_string());
    }

    let mut train_samples = Vec::new();
    let mut holdout_samples = Vec::new();
    for sample in samples {
        if is_holdout(&sample) {
            holdout_samples.push(sample);
        } else {
            train_samples.push(sample);
        }
    }

    Ok((train_samples, holdout_samples))
}

fn print_report(
    step_metrics: &[StepMetrics],
    eval_metrics: &[EvalMetrics],
    out_dir: &std::path::Path,
) {
    println!("Checkpoint written to {}", out_dir.display());
    if let (Some(first), Some(last)) = (step_metrics.first(), step_metrics.last()) {
        println!(
            "Train loss: initial={:.6} final={:.6} over {} steps",
            first.loss, last.loss, last.step
        );
    }
    for eval in eval_metrics {
        println!(
            "Holdout @ step {}: loss={:.6} policy_top1={:.2}% value_sign={:.2}%",
            eval.step,
            eval.loss,
            eval.policy_top1_accuracy * 100.0,
            eval.value_sign_accuracy * 100.0,
        );
    }
}

/// The wgpu training device; only available with the `gpu` feature.
///
/// GPU training is not parity-verified against Flex (Burn issues #5162
/// and #5626 hit this network's layers) and is not bit-reproducible;
/// treat wgpu runs as experiments, not candidates.
#[cfg(feature = "gpu")]
fn wgpu_device() -> Result<Device, String> {
    Ok(Device::wgpu(Default::default()).autodiff())
}

/// The wgpu device with native MSL shaders; only available with the
/// `gpu` feature.
///
/// Same parity caveat as [`wgpu_device`].
#[cfg(feature = "gpu")]
fn metal_device() -> Result<Device, String> {
    Ok(Device::metal(Default::default()).autodiff())
}

/// Without the `gpu` feature, requesting metal is a run failure with
/// build instructions.
#[cfg(not(feature = "gpu"))]
fn metal_device() -> Result<Device, String> {
    Err("device 'metal' requires the gpu feature: \
         cargo run --release -p net --features gpu --bin nettrain -- ..."
        .to_string())
}

/// Without the `gpu` feature, requesting wgpu is a run failure with
/// build instructions.
#[cfg(not(feature = "gpu"))]
fn wgpu_device() -> Result<Device, String> {
    Err("device 'wgpu' requires the gpu feature: \
         cargo run --release -p net --features gpu --bin nettrain -- ..."
        .to_string())
}

fn parse_args<I>(mut args: I) -> Result<Args, String>
where
    I: Iterator<Item = String>,
{
    let mut out_dir: Option<PathBuf> = None;
    let mut data_dir: Option<PathBuf> = None;
    let mut config_name = Some("tiny".to_string());
    let mut seed: Option<u64> = Some(0);
    let mut steps: Option<usize> = Some(20);
    let mut batch_size: Option<usize> = Some(16);
    let mut eval_interval: Option<usize> = Some(0);
    let mut lr_max: Option<f64> = None;
    let mut device = DeviceChoice::Flex;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--out requires a value".to_string())?;
                out_dir = Some(PathBuf::from(value));
            }
            "--data" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--data requires a value".to_string())?;
                data_dir = Some(PathBuf::from(value));
            }
            "--config" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--config requires a value".to_string())?;
                config_name = Some(value);
            }
            "--seed" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--seed requires a value".to_string())?;
                seed = Some(
                    value
                        .parse::<u64>()
                        .map_err(|e| format!("invalid --seed value '{value}': {e}"))?,
                );
            }
            "--steps" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--steps requires a value".to_string())?;
                steps = Some(
                    value
                        .parse::<usize>()
                        .map_err(|e| format!("invalid --steps value '{value}': {e}"))?,
                );
            }
            "--batch-size" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--batch-size requires a value".to_string())?;
                batch_size = Some(
                    value
                        .parse::<usize>()
                        .map_err(|e| format!("invalid --batch-size value '{value}': {e}"))?,
                );
            }
            "--eval-interval" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--eval-interval requires a value".to_string())?;
                eval_interval = Some(
                    value
                        .parse::<usize>()
                        .map_err(|e| format!("invalid --eval-interval value '{value}': {e}"))?,
                );
            }
            "--lr-max" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--lr-max requires a value".to_string())?;
                lr_max = Some(
                    value
                        .parse::<f64>()
                        .map_err(|e| format!("invalid --lr-max value '{value}': {e}"))?,
                );
            }
            "--device" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--device requires a value".to_string())?;
                device = match value.as_str() {
                    "flex" => DeviceChoice::Flex,
                    "wgpu" => DeviceChoice::Wgpu,
                    "metal" => DeviceChoice::Metal,
                    other => {
                        return Err(format!(
                            "unknown device '{other}'; use flex, wgpu, or metal"
                        ));
                    }
                };
            }
            other => return Err(format!("unknown flag: {other}")),
        }
    }

    let out_dir = out_dir.ok_or_else(|| "--out is required".to_string())?;
    let config = match config_name.as_deref().unwrap_or("tiny") {
        "tiny" => ModelConfig::tiny(),
        "mid" => ModelConfig::mid(),
        "default" => ModelConfig::new(),
        other => {
            return Err(format!(
                "unknown config '{other}'; use tiny, mid, or default"
            ));
        }
    };

    let steps = steps.unwrap_or(20);
    let batch_size = batch_size.unwrap_or(16);
    let seed = seed.unwrap_or(0);

    Ok(Args {
        out_dir,
        data_dir,
        config,
        train_config: TrainConfig {
            steps,
            batch_size,
            eval_interval: eval_interval.unwrap_or(0),
            seed,
            lr_warmup_steps: (steps / 10).max(1),
            // Locked production schedule from docs/12: 3e-4 -> 3e-6.
            // --lr-max overrides the peak for experiments.
            lr_max: lr_max.unwrap_or(3e-4),
            lr_min: 3e-6,
            lr_cosine_steps: steps,
            weight_decay: 1e-4,
            // Throughput over bit-reproducibility for real runs;
            // set 0 for a reproducible seeded run (field docs).
            dataloader_workers: 8,
        },
        device,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_requires_out() {
        let result = parse_args(vec!["--seed".to_string(), "42".to_string()].into_iter());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--out is required"));
    }

    #[test]
    fn parse_args_default_to_tiny_config() {
        let args =
            parse_args(vec!["--out".to_string(), "/tmp/net".to_string()].into_iter()).unwrap();
        assert_eq!(args.config, ModelConfig::tiny());
        assert_eq!(args.train_config.steps, 20);
        assert_eq!(args.train_config.batch_size, 16);
    }

    #[test]
    fn parse_args_rejects_unknown_config() {
        let result = parse_args(
            vec![
                "--out".to_string(),
                "/tmp/net".to_string(),
                "--config".to_string(),
                "huge".to_string(),
            ]
            .into_iter(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_args_roundtrips_values() {
        let args = parse_args(
            vec![
                "--out".to_string(),
                "/tmp/out".to_string(),
                "--data".to_string(),
                "/tmp/data".to_string(),
                "--config".to_string(),
                "mid".to_string(),
                "--seed".to_string(),
                "7".to_string(),
                "--steps".to_string(),
                "100".to_string(),
                "--batch-size".to_string(),
                "32".to_string(),
                "--eval-interval".to_string(),
                "10".to_string(),
            ]
            .into_iter(),
        )
        .unwrap();

        assert_eq!(args.out_dir, PathBuf::from("/tmp/out"));
        assert_eq!(args.data_dir, Some(PathBuf::from("/tmp/data")));
        assert_eq!(args.config, ModelConfig::mid());
        assert_eq!(args.train_config.seed, 7);
        assert_eq!(args.train_config.steps, 100);
        assert_eq!(args.train_config.batch_size, 32);
        assert_eq!(args.train_config.eval_interval, 10);
    }

    #[test]
    fn parse_args_device_defaults_to_flex() {
        let args =
            parse_args(vec!["--out".to_string(), "/tmp/net".to_string()].into_iter()).unwrap();
        assert_eq!(args.device, DeviceChoice::Flex);
    }

    #[test]
    fn parse_args_device_roundtrip_and_rejects_unknown() {
        let args = parse_args(
            vec![
                "--out".to_string(),
                "/tmp/net".to_string(),
                "--device".to_string(),
                "wgpu".to_string(),
            ]
            .into_iter(),
        )
        .unwrap();
        assert_eq!(args.device, DeviceChoice::Wgpu);

        let result = parse_args(
            vec![
                "--out".to_string(),
                "/tmp/net".to_string(),
                "--device".to_string(),
                "cuda".to_string(),
            ]
            .into_iter(),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown device"));

        let metal = parse_args(
            vec![
                "--out".to_string(),
                "/tmp/net".to_string(),
                "--device".to_string(),
                "metal".to_string(),
            ]
            .into_iter(),
        );
        assert_eq!(metal.unwrap().device, DeviceChoice::Metal);
    }
}
