# Chapter 08 — Deep-dive solution: the `datagen` binary + crate polish

This is the opt-in reference for Chapter 08. It matches the verified
reference worktree exactly.

## `src/lib.rs`

Add `#![deny(missing_docs)]` immediately after the inner doc comment:

```rust
//! Synthetic training-data generation for the AlphaZero-style Gomoku agent.
//!
//! This crate stores labelled training positions as stone lists, not as
//! neural-network planes. Planes are derived on read in the `net` crate, and
//! a single position can be augmented with a random D4 transform every time it
//! is loaded. At this slice the crate has no Burn dependency; randomness lives
//! in the `playout` module, file I/O lives in the `shard` module, the
//! dataset reader lives in the `dataset` module, and the train/held-out
//! partition lives in the `split` module.
//!
//! Design documents (locked):
//!
//! * `docs/12-gomoku-architecture.md` in the rust-ml repository — workspace
//!   layout, serialization decision, and the store-games replay-buffer rule.
//! * `docs/specs/2026-09-27-datagen-tutorial-design.md` — synthetic data
//!   generator design for milestone 3.

#![deny(missing_docs)]

pub mod collect;
pub mod dataset;
pub mod label;
pub mod playout;
pub mod sample;
pub mod shard;
pub mod split;

pub use sample::Sample;
```

## `src/bin/datagen.rs`

```rust
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
#[derive(Debug, Clone, PartialEq)]
struct Args {
    /// RNG seed for deterministic generation.
    seed: u64,
    /// Output directory for shards and manifest.
    out_dir: PathBuf,
    /// Per-class collection quotas.
    quotas: Quotas,
}

fn main() {
    let args = match parse_args(env::args().skip(1)) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            eprintln!(
                "Usage: datagen --out <DIR> [--seed <u64>] [--win <n>] [--block <n>] [--quiet <n>]"
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
                    .ok_or_else(|| "--out requires a value".to_string())?;
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
    for name in &manifest.shards {
        eprintln!("  - {}", out_dir.join(name).display());
    }
    eprintln!(
        "Counts: win={}, block={}, quiet={}",
        manifest.counts.get("win").copied().unwrap_or(0),
        manifest.counts.get("block").copied().unwrap_or(0),
        manifest.counts.get("quiet").copied().unwrap_or(0),
    );
}
```

## `tests/datagen.rs`

```rust
//! Integration test for the `datagen` binary.
//!
//! Runs the compiled binary with tiny quotas, verifies that it exits
//! successfully, writes a manifest and at least one shard, and that
//! every sample passes the train crate's soundness gate when read back.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use train::dataset::{check_soundness, read_dataset};
use train::shard::Manifest;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let pid = std::process::id();
    std::env::temp_dir().join(format!("{prefix}-{pid}-{nanos}"))
}

#[test]
fn datagen_runs_end_to_end_with_tiny_quotas() {
    let out_dir = unique_temp_dir("train-datagen-integration");
    let _ = fs::remove_dir_all(&out_dir);

    let status = Command::new(env!("CARGO_BIN_EXE_datagen"))
        .args([
            "--seed".as_ref(),
            "7".as_ref(),
            "--win".as_ref(),
            "2".as_ref(),
            "--block".as_ref(),
            "2".as_ref(),
            "--quiet".as_ref(),
            "3".as_ref(),
            "--out".as_ref(),
            out_dir.as_os_str(),
        ])
        .status()
        .expect("datagen binary should be runnable");

    assert!(status.success(), "datagen should exit with status 0");

    let manifest_path = out_dir.join("manifest.json");
    assert!(manifest_path.exists(), "manifest.json should exist");

    let manifest_bytes = fs::read_to_string(&manifest_path).expect("manifest should be readable");
    let manifest: Manifest =
        serde_json::from_str(&manifest_bytes).expect("manifest should be valid JSON");
    assert_eq!(manifest.seed, 7);
    assert_eq!(manifest.quotas.win, 2);
    assert_eq!(manifest.quotas.block, 2);
    assert_eq!(manifest.quotas.quiet, 3);

    let shard_paths: Vec<_> = fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("shard-") && n.ends_with(".bin"))
                    .unwrap_or(false)
        })
        .collect();
    assert!(!shard_paths.is_empty(), "at least one shard should exist");

    let samples = read_dataset(&out_dir).expect("dataset should be readable");
    assert_eq!(
        samples.len(),
        manifest.quotas.win + manifest.quotas.block + manifest.quotas.quiet,
        "read-back sample count should match the requested quotas"
    );

    for (i, sample) in samples.iter().enumerate() {
        assert_eq!(
            check_soundness(sample),
            Ok(()),
            "sample {i} should pass the soundness gate"
        );
    }

    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn datagen_fails_without_required_out_flag() {
    let status = Command::new(env!("CARGO_BIN_EXE_datagen"))
        .args(["--seed", "1", "--win", "1", "--block", "1", "--quiet", "1"])
        .status()
        .expect("datagen binary should be runnable");

    assert!(
        !status.success(),
        "datagen should exit non-zero without --out"
    );
}

#[test]
fn datagen_rejects_unknown_flag() {
    let out_dir = unique_temp_dir("train-datagen-unknown-flag");
    let _ = fs::remove_dir_all(&out_dir);

    let status = Command::new(env!("CARGO_BIN_EXE_datagen"))
        .args([
            "--seed",
            "1",
            "--win",
            "1",
            "--block",
            "1",
            "--quiet",
            "1",
            "--out",
            out_dir.to_str().unwrap(),
            "--nonsense",
        ])
        .status()
        .expect("datagen binary should be runnable");

    assert!(
        !status.success(),
        "datagen should exit non-zero on unknown flag"
    );
    let _ = fs::remove_dir_all(&out_dir);
}
```
