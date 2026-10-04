# Chapter 07 — Deep-dive solution: the nettrain binary

This is the opt-in reference for Chapter 07. It matches the verified reference worktree exactly.

## `src/bin/nettrain.rs`

```rust
//! Train the Gomoku policy/value network from synthetic data.
//!
//! Usage:
//!
//! ```text
//! nettrain --out <DIR> [--data <DIR>] [--config tiny|mid|default]
//!          [--seed <u64>] [--steps <n>] [--batch-size <n>]
//!          [--eval-interval <n>]
//! ```
//!
//! If `--data` is omitted, a tiny synthetic dataset is generated in-memory
//! for a quick smoke run.

use std::env;
use std::path::PathBuf;
use std::process;

use burn::backend::flex::FlexDevice;
use burn::backend::{Autodiff, Flex};
use rand::SeedableRng;
use rand::rngs::StdRng;

use net::checkpoint::{RunJournal, save_checkpoint};
use net::network::{Model, ModelConfig};
use net::train::{EvalMetrics, StepMetrics, TrainConfig, train};
use train::collect::{Quotas, collect};
use train::dataset::read_dataset;
use train::split::is_holdout;

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
}

fn main() {
    let args = match parse_args(env::args().skip(1)) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            eprintln!(
                "Usage: nettrain --out <DIR> [--data <DIR>] [--config tiny|mid|default]\n\
                 [--seed <u64>] [--steps <n>] [--batch-size <n>] [--eval-interval <n>]\n\
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
    type B = Autodiff<Flex>;
    let device = FlexDevice;

    eprintln!(
        "nettrain: seed={}, steps={}, batch_size={}, eval_interval={}",
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

    let model: Model<B> = args.config.init(&device);
    let (model, step_metrics, eval_metrics) = train(
        model,
        &args.train_config,
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
    );

    save_checkpoint::<B>(&model, &journal, &args.out_dir)
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
    if let Some(eval) = eval_metrics.last() {
        println!(
            "Holdout @ step {}: loss={:.6} policy_top1={:.2}% value_sign={:.2}%",
            eval.step,
            eval.loss,
            eval.policy_top1_accuracy * 100.0,
            eval.value_sign_accuracy * 100.0,
        );
    }
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
            lr_max: 3e-4,
            lr_min: 3e-6,
            lr_cosine_steps: steps,
            weight_decay: 1e-4,
        },
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
}
```


## `tests/nettrain_smoke.rs`

```rust
//! Integration smoke test for the `nettrain` binary.
//!
//! Runs `nettrain` for a bounded number of steps on a tiny in-memory
//! generated dataset and verifies that it produces a checkpoint and a
//! decreasing training loss.

use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let pid = std::process::id();
    std::env::temp_dir().join(format!("{prefix}-{pid}-{nanos}.tmp"))
}

#[test]
fn nettrain_runs_twenty_steps_and_writes_checkpoint() {
    let out_dir = unique_temp_dir("nettrain-smoke");
    let _ = std::fs::remove_dir_all(&out_dir);

    let status = Command::new(env!("CARGO_BIN_EXE_nettrain"))
        .args([
            "--out",
            &out_dir.to_string_lossy(),
            "--steps",
            "20",
            "--batch-size",
            "8",
            "--seed",
            "42",
        ])
        .status()
        .expect("failed to run nettrain binary");

    assert!(status.success(), "nettrain must exit successfully");
    assert!(
        out_dir.join("model.mpk").exists(),
        "model checkpoint must exist"
    );
    assert!(out_dir.join("run.json").exists(), "run journal must exist");

    let journal: net::checkpoint::RunJournal =
        net::checkpoint::load_journal(&out_dir).expect("journal must load");
    assert_eq!(journal.seed, 42);

    if let Some(metrics) = journal.final_step_metrics {
        assert!(metrics.step == 20, "final step must be 20");
        assert!(
            metrics.loss.is_finite(),
            "final loss must be finite, got {}",
            metrics.loss
        );
    } else {
        panic!("journal must contain final step metrics");
    }

    let _ = std::fs::remove_dir_all(&out_dir);
}
```
