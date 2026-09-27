# Chapter 05 — Deep-dive solution: streaming shard writer + manifest

This is the opt-in reference for Chapter 05. It matches the verified
reference worktree exactly.

## `Cargo.toml`

Inherit the workspace package version and add `serde_json` as a
workspace dependency:

```toml
[package]
name = "train"
description = "Synthetic training-data generator for the Gomoku AlphaZero-style agent."
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
engine = { path = "../engine" }
serde = { workspace = true }
thiserror = { workspace = true }
bincode = { version = "2", features = ["serde"] }
serde_json = { workspace = true }
rand = "0.10.3"

[dev-dependencies]
engine = { path = "../engine", features = ["testutil"] }
```

## `src/collect.rs`

Add `PartialEq` and serde derives to `Quotas`:

```rust
/// Per-class collection targets.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Quotas {
    /// How many unique win positions to collect.
    pub win: usize,
    /// How many unique block positions to collect.
    pub block: usize,
    /// How many unique quiet positions to collect.
    pub quiet: usize,
}
```

## `src/lib.rs`

Register the module and update the crate doc comment:

```rust
//! Synthetic training-data generation for the AlphaZero-style Gomoku agent.
//!
//! This crate stores labelled training positions as stone lists, not as
//! neural-network planes. Planes are derived on read in the `net` crate, and
//! a single position can be augmented with a random D4 transform every time it
//! is loaded. At this slice the crate has no Burn dependency; randomness lives
//! in the `playout` module and file I/O lives in the `shard` module.
//!
//! Design documents (locked):
//!
//! * `docs/12-gomoku-architecture.md` in the rust-ml repository — workspace
//!   layout, serialization decision, and the store-games replay-buffer rule.
//! * `docs/specs/2026-09-27-datagen-tutorial-design.md` — synthetic data
//!   generator design for milestone 3.

pub mod collect;
pub mod label;
pub mod playout;
pub mod sample;
pub mod shard;

pub use sample::Sample;
```

## `src/shard.rs`

```rust
//! Streaming shard writer and manifest.
//!
//! Writes a [`Sample`] stream to length-delimited bincode shard files,
//! plus a JSON manifest recording provenance and per-class counts.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::collect::Quotas;
use crate::sample::Sample;

/// Number of samples per shard.
pub const SHARD_SIZE: usize = 4096;

/// Dataset provenance and inventory.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    /// RNG seed used to generate the dataset.
    pub seed: u64,
    /// Per-class collection quotas.
    pub quotas: Quotas,
    /// Number of samples per tactical class.
    pub counts: BTreeMap<String, usize>,
    /// Shard file names, in order.
    pub shards: Vec<String>,
    /// Version string captured from the workspace package version.
    ///
    /// This crate inherits `version.workspace = true`, so the value is
    /// structurally shared by every crate that does the same — including
    /// `engine`, whose tactics module produced the labels.
    pub workspace_version: String,
}

/// Write `samples` to `out_dir` as length-delimited bincode shards and a JSON manifest.
///
/// Creates `out_dir` if it does not exist. Fails if any shard file or
/// `manifest.json` already exists, to avoid silently overwriting a
/// previous dataset.
///
/// Shards are named `shard-000.bin`, `shard-001.bin`, ... Each shard
/// contains up to [`SHARD_SIZE`] samples. Each sample is encoded with
/// bincode and prefixed with its length as a little-endian `u32`.
///
/// Class counts in the manifest are derived from the sample value
/// target: [`label`](crate::label) produces `+1.0` for win, `-1.0`
/// for block, and `0.0` for quiet positions.
pub fn write_dataset(
    samples: &[Sample],
    out_dir: &Path,
    seed: u64,
    quotas: Quotas,
) -> io::Result<Manifest> {
    fs::create_dir_all(out_dir)?;

    let mut counts = BTreeMap::new();
    counts.insert("win".to_string(), 0);
    counts.insert("block".to_string(), 0);
    counts.insert("quiet".to_string(), 0);

    let mut shards = Vec::new();
    let mut shard_index = 0;
    let mut shard_count = 0;
    let mut writer: Option<BufWriter<File>> = None;

    for sample in samples {
        // The label contract encodes class as the value target.
        match sample.value {
            1.0 => *counts.get_mut("win").expect("win key inserted above") += 1,
            -1.0 => *counts.get_mut("block").expect("block key inserted above") += 1,
            0.0 => *counts.get_mut("quiet").expect("quiet key inserted above") += 1,
            _ => {}
        }

        // Rollover to a new shard when the current one is full.
        if shard_count == SHARD_SIZE {
            if let Some(mut w) = writer.take() {
                w.flush()?;
            }
            shard_index += 1;
            shard_count = 0;
        }

        // Open the shard file on the first sample of the shard.
        if writer.is_none() {
            let name = format!("shard-{shard_index:03}.bin");
            let path = out_dir.join(&name);
            let file = File::options().write(true).create_new(true).open(&path)?;
            shards.push(name);
            writer = Some(BufWriter::new(file));
        }

        let bytes = bincode::serde::encode_to_vec(sample, bincode::config::standard())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let len = bytes.len();
        if len > u32::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "encoded sample exceeds u32 length limit",
            ));
        }

        let w = writer.as_mut().expect("writer opened above");
        w.write_all(&(len as u32).to_le_bytes())?;
        w.write_all(&bytes)?;
        shard_count += 1;
    }

    if let Some(mut w) = writer.take() {
        w.flush()?;
    }

    let manifest = Manifest {
        seed,
        quotas,
        counts,
        shards,
        workspace_version: env!("CARGO_PKG_VERSION").to_string(),
    };

    let manifest_path = out_dir.join("manifest.json");
    let manifest_file = File::options()
        .write(true)
        .create_new(true)
        .open(&manifest_path)?;
    serde_json::to_writer_pretty(manifest_file, &manifest)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collect::{Quotas, collect};
    use crate::label::{self, TacticalClass};
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use std::fs;
    use std::io::Read;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let pid = std::process::id();
        std::env::temp_dir().join(format!("{prefix}-{pid}-{nanos}"))
    }

    fn read_shard(path: &Path) -> Vec<Sample> {
        let mut file = fs::File::open(path).unwrap();
        let mut samples = Vec::new();
        let mut len_buf = [0u8; 4];
        while file.read_exact(&mut len_buf).is_ok() {
            let len = u32::from_le_bytes(len_buf) as usize;
            let mut bytes = vec![0u8; len];
            file.read_exact(&mut bytes).unwrap();
            let (sample, _): (Sample, usize) =
                bincode::serde::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
            samples.push(sample);
        }
        samples
    }

    #[test]
    fn writes_one_shard_and_manifest_for_ten_samples() {
        let quotas = Quotas {
            win: 3,
            block: 3,
            quiet: 4,
        };
        let samples = collect(quotas, &mut StdRng::seed_from_u64(42));
        assert_eq!(samples.len(), 10, "expected 10 samples from quotas");

        let out_dir = unique_temp_dir("train-shard-test");
        let _ = fs::remove_dir_all(&out_dir);
        fs::create_dir_all(&out_dir).unwrap();

        let manifest = write_dataset(&samples, &out_dir, 42, quotas).unwrap();

        let shard_path = out_dir.join("shard-000.bin");
        assert!(shard_path.exists(), "shard-000.bin should exist");
        assert!(
            !out_dir.join("shard-001.bin").exists(),
            "expected one shard"
        );
        assert!(
            out_dir.join("manifest.json").exists(),
            "manifest should exist"
        );

        let decoded = read_shard(&shard_path);
        assert_eq!(
            decoded, samples,
            "decoded samples must match written samples"
        );

        let mut expected = BTreeMap::new();
        for sample in &samples {
            let class = label::classify(&sample.board().unwrap());
            let key = match class {
                TacticalClass::Win => "win",
                TacticalClass::Block => "block",
                TacticalClass::Quiet => "quiet",
            };
            *expected.entry(key.to_string()).or_insert(0) += 1;
        }
        assert_eq!(
            manifest.counts, expected,
            "manifest counts must match sample classes"
        );
        assert_eq!(manifest.seed, 42);
        assert_eq!(manifest.quotas, quotas);
        assert_eq!(manifest.shards, vec!["shard-000.bin"]);
        assert_eq!(
            manifest.workspace_version,
            env!("CARGO_PKG_VERSION"),
            "workspace_version records the workspace package version"
        );

        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn refuses_to_overwrite_existing_shard() {
        let out_dir = unique_temp_dir("train-shard-overwrite");
        let _ = fs::remove_dir_all(&out_dir);
        fs::create_dir_all(&out_dir).unwrap();
        fs::File::create(out_dir.join("shard-000.bin")).unwrap();

        let samples = collect(
            Quotas {
                win: 1,
                block: 0,
                quiet: 0,
            },
            &mut StdRng::seed_from_u64(1),
        );
        let result = write_dataset(
            &samples,
            &out_dir,
            1,
            Quotas {
                win: 1,
                block: 0,
                quiet: 0,
            },
        );
        assert!(result.is_err(), "must fail when shard file already exists");

        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn refuses_to_overwrite_existing_manifest() {
        let out_dir = unique_temp_dir("train-manifest-overwrite");
        let _ = fs::remove_dir_all(&out_dir);
        fs::create_dir_all(&out_dir).unwrap();
        fs::File::create(out_dir.join("manifest.json")).unwrap();

        let samples = collect(
            Quotas {
                win: 1,
                block: 0,
                quiet: 0,
            },
            &mut StdRng::seed_from_u64(1),
        );
        let result = write_dataset(
            &samples,
            &out_dir,
            1,
            Quotas {
                win: 1,
                block: 0,
                quiet: 0,
            },
        );
        assert!(
            result.is_err(),
            "must fail when manifest.json already exists"
        );

        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn empty_dataset_writes_no_shards_and_zero_counts() {
        let out_dir = unique_temp_dir("train-shard-empty");
        let _ = fs::remove_dir_all(&out_dir);
        fs::create_dir_all(&out_dir).unwrap();

        let quotas = Quotas {
            win: 0,
            block: 0,
            quiet: 0,
        };
        let manifest = write_dataset(&[], &out_dir, 7, quotas).unwrap();

        assert!(
            !out_dir.join("shard-000.bin").exists(),
            "no shards for empty dataset"
        );
        assert!(
            out_dir.join("manifest.json").exists(),
            "manifest should exist"
        );
        assert!(manifest.shards.is_empty());
        assert_eq!(manifest.counts.get("win"), Some(&0));
        assert_eq!(manifest.counts.get("block"), Some(&0));
        assert_eq!(manifest.counts.get("quiet"), Some(&0));
        assert_eq!(manifest.seed, 7);
        assert_eq!(manifest.quotas, quotas);

        let _ = fs::remove_dir_all(&out_dir);
    }
}
```
