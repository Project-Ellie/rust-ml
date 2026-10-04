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
    /// RMG seed used to generate the dataset
    pub seed: u64,
    /// Per-class collection quotas
    pub quotas: Quotas,
    /// Number of samples per tactical class
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
/// A failed run may leave partial shards behind. Because of the overwrite
/// protection, a retry will then be refused until the output directory is
/// cleaned.
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

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    counts.insert("win".to_string(), 0);
    counts.insert("block".to_string(), 0);
    counts.insert("quiet".to_string(), 0);

    let mut shards: Vec<String> = Vec::new();
    let mut shard_index = 0;
    let mut shard_count = 0;
    let mut writer: Option<BufWriter<File>> = None;

    for sample in samples {
        match sample.value {
            1.0 => *counts.get_mut("win").expect("win key inserted above") += 1,
            -1.0 => *counts.get_mut("block").expect("win key inserted above") += 1,
            0.0 => *counts.get_mut("quiet").expect("win key inserted above") += 1,
            _ => {}
        }

        if shard_count == SHARD_SIZE {
            if let Some(mut w) = writer.take() {
                w.flush()?;
            }
            shard_index += 1;
            shard_count = 0;
        }

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
                "Encoded Sample exceeds u32 length limit.",
            ));
        }

        let w = writer.as_mut().expect("Writer opened above.");
        w.write_all(&(len as u32).to_le_bytes())?;
        w.write_all(&bytes)?;
        shard_count += 1;
    }

    if let Some(mut w) = writer.take() {
        w.flush()?
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
    use crate::label::{TacticalClass, classify};
    use engine::Move;
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
        std::env::temp_dir().join(format!("{prefix}-{nanos}-{pid}"))
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
        assert_eq!(samples.len(), 10, "Expected 10 samples.");

        let out_dir = unique_temp_dir("train-shard-test");
        let _ = fs::remove_dir_all(&out_dir);
        fs::create_dir_all(&out_dir).unwrap();

        let manifest = write_dataset(&samples, &out_dir, 42, quotas).unwrap();
        let shard_path = out_dir.join("shard-000.bin");
        assert!(shard_path.exists(), "Expected shard 000");
        assert!(
            !out_dir.join("shard-001.bin").exists(),
            "Expected only one shard."
        );

        assert!(
            out_dir.join("manifest.json").exists(),
            "Expected manifest json"
        );

        let decoded = read_shard(&shard_path);
        assert_eq!(
            decoded, samples,
            "Samples should perfectly reproduce from reading"
        );

        let mut expected = BTreeMap::new();
        for sample in samples {
            let class = classify(&sample.board().unwrap());
            let key = match class {
                TacticalClass::Win => "win",
                TacticalClass::Block => "block",
                TacticalClass::Quiet => "quiet",
            };
            *expected.entry(key.to_string()).or_insert(0) += 1;
        }
        assert_eq!(manifest.counts, expected, "Manifest counts must match");
        assert_eq!(manifest.seed, 42);
        assert_eq!(manifest.quotas, quotas);
        assert_eq!(manifest.shards, vec!["shard-000.bin"]);

        assert_eq!(
            manifest.workspace_version,
            env!("CARGO_PKG_VERSION"),
            "Versions must match."
        );

        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn refuses_to_overwrite_existing_shard() {
        let out_dir = unique_temp_dir("train-shards-overwrite");
        let _ = fs::remove_dir_all(&out_dir);
        fs::create_dir_all(&out_dir).unwrap();
        File::create(out_dir.join("shard-000.bin")).unwrap();

        let quotas = Quotas {
            win: 1,
            block: 0,
            quiet: 0,
        };
        let samples = collect(quotas, &mut StdRng::seed_from_u64(1));

        let result = write_dataset(&samples, &out_dir, 42, quotas);
        assert!(result.is_err(), "Must fail when shard file exists.");
        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn refuses_to_overwrite_existing_manifest() {
        let out_dir = unique_temp_dir("train-manifest-overwrite");
        let _ = fs::remove_dir_all(&out_dir);
        fs::create_dir_all(&out_dir).unwrap();
        File::create(out_dir.join("manifest.json")).unwrap();

        let quotas = Quotas {
            win: 1,
            block: 0,
            quiet: 0,
        };
        let samples = collect(quotas, &mut StdRng::seed_from_u64(42));
        let result = write_dataset(&samples, &out_dir, 42, quotas);

        assert!(result.is_err(), "Must fail when manifest file exists.");

        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn shard_rollover_after_shard_size_samples() {
        let out_dir = unique_temp_dir("train-shard-rollover");
        let _ = fs::remove_dir_all(&out_dir);
        fs::create_dir_all(&out_dir).unwrap();

        let mv = Move::new(7, 7).unwrap();
        let sample = Sample::from_position(&[mv], 1, Vec::new(), 0.0);
        let samples = vec![sample; SHARD_SIZE + 1];

        let quotas = Quotas {
            win: 0,
            block: 0,
            quiet: SHARD_SIZE + 1,
        };
        let manifest = write_dataset(&samples, &out_dir, 42, quotas).unwrap();

        assert_eq!(
            manifest.shards,
            vec!["shard-000.bin", "shard-001.bin"],
            "expected two shards"
        );

        let first = read_shard(&out_dir.join("shard-000.bin"));
        assert_eq!(first.len(), SHARD_SIZE, "Must have SHARD_SIZE samples");

        let second = read_shard(&out_dir.join("shard-001.bin"));
        assert_eq!(second.len(), 1, "Expected only 1 overflow");

        assert_eq!(first, samples[..SHARD_SIZE]);
        assert_eq!(second, samples[SHARD_SIZE..]);

        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn empty_dataset_writes_no_shard_and_manifests_zero_counts() {
        let out_dir = unique_temp_dir("train-shard-empty");
        let _ = fs::remove_dir_all(&out_dir);
        fs::create_dir_all(&out_dir).unwrap();

        let quotas = Quotas {
            win: 0,
            block: 0,
            quiet: 0,
        };
        let manifest = write_dataset(&[], &out_dir, 3, quotas).unwrap();

        assert!(!out_dir.join("shard-000.bin").exists(), "Expected no shard");
        assert!(
            out_dir.join("manifest.json").exists(),
            "Manifest should exist"
        );

        assert!(manifest.shards.is_empty(), "Expected no shards.");
        assert_eq!(manifest.counts.get("win"), Some(&0));
        assert_eq!(manifest.counts.get("block"), Some(&0));
        assert_eq!(manifest.counts.get("quiet"), Some(&0));

        assert_eq!(manifest.seed, 3);
        assert_eq!(manifest.quotas, quotas);

        let _ = fs::remove_dir_all(&out_dir);
    }
}
