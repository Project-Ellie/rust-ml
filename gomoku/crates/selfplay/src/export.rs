//! Integration seam from self-play game records to a `nettrain --data` dataset.
//!
//! `nettrain --data` reads length-delimited sample shards and ignores manifest
//! details, so this module is the integration seam and no network or training
//! changes are needed. Exported positions stay canonical, so `nettrain`
//! computes the Zobrist holdout split before its existing seeded batch-time D4
//! transform.

use std::fs::File;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::record::GameRecord;
use crate::sample::{SampleError, replay_sample};

/// Sample-export manifest layout version.
///
/// Independent of [`GAME_RECORD_FORMAT_VERSION`](crate::record::GAME_RECORD_FORMAT_VERSION):
/// both are 1 today by coincidence, not coupling, and evolve independently.
pub const SAMPLE_EXPORT_FORMAT_VERSION: u16 = 1;

/// Provenance summary of one exported sample dataset, serialized as
/// `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportManifest {
    /// Sample-export layout version.
    pub format_version: u16,
    /// Number of source games exported.
    pub game_count: usize,
    /// Number of replayed samples exported — one per post-opening ply.
    pub sample_count: usize,
    /// Shard file names, in sample order.
    pub shards: Vec<String>,
    /// Workspace package version at export time, shared with the `train` reader.
    pub workspace_version: String,
}

/// Failure while exporting a self-play corpus to a sample dataset.
#[derive(Debug, Error)]
pub enum ExportError {
    /// No source records were supplied.
    #[error("cannot export an empty corpus")]
    EmptyCorpus,

    /// Shard capacity must be positive.
    #[error("shard capacity must be greater than zero")]
    ZeroShardCapacity,

    /// One source position failed replay or target derivation.
    #[error("game {game_index} ply {ply}: {source}")]
    Sample {
        /// Zero-based index of the source game in the corpus.
        game_index: usize,
        /// Zero-based post-opening ply that failed replay.
        ply: usize,
        /// The replay error.
        #[source]
        source: SampleError,
    },

    /// Filesystem, framing, serialization, or manifest I/O failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Export a self-play corpus into canonical training samples.
///
/// Shards use exactly the same bincode framing and [`train::Sample`]
/// serialization as [`train::dataset::read_dataset`] reads. The manifest
/// carries game-export provenance rather than synthetic tactical quotas. Any
/// non-empty output directory is refused before files are changed.
///
/// # Errors
///
/// Returns an error if the corpus is empty, the shard capacity is zero, any
/// position fails to replay, the output directory already exists and is
/// non-empty, or any shard or manifest write fails. Replay validation finishes
/// before the output directory is modified.
pub fn export_corpus(
    records: &[GameRecord],
    out_dir: &Path,
    shard_capacity: usize,
) -> Result<ExportManifest, ExportError> {
    // Phase 1: validate the inputs.
    if records.is_empty() {
        return Err(ExportError::EmptyCorpus);
    }
    if shard_capacity == 0 {
        return Err(ExportError::ZeroShardCapacity);
    }

    // Phase 2: replay every position before touching the disk.
    let mut samples = Vec::new();
    for (game_index, record) in records.iter().enumerate() {
        for ply in 0..record.plies.len() {
            let sample = replay_sample(record, ply).map_err(|source| ExportError::Sample {
                game_index,
                ply,
                source,
            })?;
            samples.push(sample);
        }
    }

    // Phase 3: refuse a non-empty output directory.
    match std::fs::read_dir(out_dir) {
        Ok(mut entries) => {
            if entries.next().transpose()?.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "refusing to export samples into a non-empty directory",
                )
                .into());
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    // Phase 4: write the shards.
    std::fs::create_dir_all(out_dir)?;
    let mut shards = Vec::new();
    for (index, chunk) in samples.chunks(shard_capacity).enumerate() {
        let name = format!("shard-{index:03}.bin");
        train::storage::write_shard(chunk, &out_dir.join(&name))?;
        shards.push(name);
    }

    // Phase 5: write the manifest, last.
    let manifest = ExportManifest {
        format_version: SAMPLE_EXPORT_FORMAT_VERSION,
        game_count: records.len(),
        sample_count: samples.len(),
        shards,
        workspace_version: env!("CARGO_PKG_VERSION").to_owned(),
    };
    let file = File::options()
        .write(true)
        .create_new(true)
        .open(out_dir.join("manifest.json"))?;
    serde_json::to_writer_pretty(file, &manifest)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    Ok(manifest)
}
