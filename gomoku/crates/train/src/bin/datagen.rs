//! Synthetic dataset generator for the Gomoku AlphaZero-style agent.
//!
//! This binary is the front end of the milestone-3 data pipeline.
//! It runs seeded random playouts, labels positions with the engine's
//! tactics module, writes length-delimited bincode shards plus a JSON
//! manifest, reads the shards back, checks every sample for soundness,
//! and prints a train/held-out statistics report.
//!
//! Usage:
//!
//! ```text
//! datagen --out <DIR> [--seed <u64>] [--win <n>] [--block <n>] [--quiet <n>]
//!
//! Use `--flag value` only; `--flag=value` is not supported.
//! If a flag appears more than once, the last value wins.
//! ```

use std::env;
use std::path::PathBuf;
use std::process;

use rand::SeedableRng;
use rand::rngs::StdRng;

use train::collect::{Quotas, collect};
use train::dataset::{check_soundness, read_dataset};
use train::shard::{Manifest, write_dataset};
use train::split::stats;

/// Default win-class quota.
///
/// Chosen so the full default run finishes in a few minutes on a
/// modern laptop while still yielding a tactical dataset large enough
/// for the phase-0 network (see chapter 8 for the timing probe).
const DEFAULT_WIN: usize = 20_000;

/// Default block-class quota.
const DEFAULT_BLOCK: usize = 20_000;

/// Default quiet-class quota.
const DEFAULT_QUIET: usize = 10_000;

/// Command-line configuration for the generator.
struct Args {
    /// RNG seed for deterministic generation
    seed: u64,

    /// Output directory for shards and manifests. Created if don't exist.
    out_dir: PathBuf,

    /// Per-class collection quotas
    quotas: Quotas,
}

pub fn main() {
    let args = match parse_args(env::args().skip(1)) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            eprintln!(
                "Usage: datagen --out <DIR> [--seed <u64>] [--win <n>] [--block <n>] [--quiet <n>]\n\
                 Use `--flag value` only; `--flag=value` is not supported.\n\
                 If a flag appears more than once, the last value wins."
            );
            process::exit(2);
        }
    };

    if let Err(e) = run(args) {
        eprintln!("datagen failed: {e}");
        process::exit(1);
    }
}
fn run(args: Args) -> Result<(), String> {
    let mut rng = StdRng::seed_from_u64(args.seed);

    eprintln!(
        "Generating dataset: seed={}, quotas=win:{} block:{} quiet:{}",
        args.seed, args.quotas.win, args.quotas.block, args.quotas.quiet
    );

    let samples = collect(args.quotas, &mut rng);
    eprintln!("Collected {} samples", samples.len());

    let manifest = write_dataset(&samples, &args.out_dir, args.seed, args.quotas)
        .map_err(|e| format!("failed to write dataset: {e}"))?;

    print_manifest_summary(&manifest, &args.out_dir);

    let read_back =
        read_dataset(&args.out_dir).map_err(|e| format!("failed to read back dataset: {e}"))?;

    if read_back.len() != samples.len() {
        return Err(format!(
            "read-back count mismatch: wrote {} samples, read {}",
            samples.len(),
            read_back.len()
        ));
    }

    for (i, sample) in read_back.iter().enumerate() {
        check_soundness(sample)
            .map_err(|e| format!("soundness check failed on sample {i}: {e}"))?;
    }
    eprintln!("Soundness gate passed for all {} samples", read_back.len());

    let report = stats(&read_back);
    println!("{report:?}");

    Ok(())
}
fn parse_args<I>(mut args: I) -> Result<Args, String>
where
    I: Iterator<Item = String>,
{
    let mut seed: Option<u64> = Some(0);
    let mut out_dir: Option<PathBuf> = None;
    let mut win: Option<usize> = Some(DEFAULT_WIN);
    let mut block: Option<usize> = Some(DEFAULT_BLOCK);
    let mut quiet: Option<usize> = Some(DEFAULT_QUIET);

    while let Some(arg) = args.next() {
        match arg.as_str() {
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
            "--out" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--out required a value".to_string())?;
                out_dir = Some(PathBuf::from(value));
            }
            "--win" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--win requires a value".to_string())?;
                win = Some(
                    value
                        .parse::<usize>()
                        .map_err(|e| format!("invalid --win value '{value}': {e}"))?,
                );
            }
            "--block" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--block requires a value".to_string())?;
                block = Some(
                    value
                        .parse::<usize>()
                        .map_err(|e| format!("invalid --block value '{value}': {e}"))?,
                );
            }
            "--quiet" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--quiet requires a value".to_string())?;
                quiet = Some(
                    value
                        .parse::<usize>()
                        .map_err(|e| format!("invalid --quiet value '{value}': {e}"))?,
                );
            }
            other => return Err(format!("unknown flag: {other}")),
        }
    }

    let out_dir = out_dir.ok_or_else(|| "--out is required".to_string())?;

    Ok(Args {
        seed: seed.unwrap_or(0),
        out_dir,
        quotas: Quotas {
            win: win.unwrap_or(DEFAULT_WIN),
            block: block.unwrap_or(DEFAULT_BLOCK),
            quiet: quiet.unwrap_or(DEFAULT_QUIET),
        },
    })
}

fn print_manifest_summary(manifest: &Manifest, out_dir: &std::path::Path) {
    eprintln!(
        "Wrote manifest to {}",
        out_dir.join("manifest.json").display()
    );
    eprintln!("Shards: {}", manifest.shards.len());
    eprintln!("Last Shard: {}", manifest.shards[manifest.shards.len() - 1]);
    eprintln!(
        "Counts: win={}, block={}, quiet={}",
        manifest.counts.get("win").copied().unwrap_or(0),
        manifest.counts.get("block").copied().unwrap_or(0),
        manifest.counts.get("quiet").copied().unwrap_or(0),
    )
}
