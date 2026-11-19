//! Infinite, seeded stream of augmented training samples over a corpus.
//!
//! Each item replays its selected record on demand, so positions are never
//! materialized in advance; the stream yields augmented samples for training.
//! Consumers constructing a split dataset use direct replay plus
//! [`position_is_holdout`](crate::sample::position_is_holdout) before
//! augmentation, or the canonical export path, because applying the Zobrist
//! rule after a D4 transform would hash a different orientation.

use std::io;
use std::path::Path;

use engine::Transform;
use rand::RngExt;
use rand::SeedableRng;
use rand::rngs::StdRng;
use thiserror::Error;
use train::Sample;

use crate::corpus::read_corpus;
use crate::record::GameRecord;
use crate::sample::{SampleError, augment_sample, replay_sample};

/// Failure to construct a corpus sampler.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SamplerError {
    /// No records were supplied, or every record has zero post-opening plies.
    #[error("a corpus sampler needs at least one game with a post-opening ply")]
    EmptyCorpus,
}

/// Seeded, infinite iterator of augmented samples drawn from a corpus.
pub struct CorpusSampler {
    records: Vec<GameRecord>,
    sampleable_games: Vec<usize>,
    rng: StdRng,
}

impl CorpusSampler {
    /// Build a sampler from an in-memory corpus.
    ///
    /// Filters the corpus to games that have at least one post-opening ply
    /// and rejects a corpus with none up front. The error surfaces at
    /// construction, not as a panic on the first draw.
    ///
    /// # Errors
    ///
    /// Returns [`SamplerError::EmptyCorpus`] when no supplied game has a
    /// post-opening ply.
    pub fn new(records: Vec<GameRecord>, seed: u64) -> Result<Self, SamplerError> {
        let sampleable_games: Vec<usize> = records
            .iter()
            .enumerate()
            .filter(|(_, record)| !record.plies.is_empty())
            .map(|(index, _)| index)
            .collect();

        if sampleable_games.is_empty() {
            return Err(SamplerError::EmptyCorpus);
        }

        Ok(Self {
            records,
            sampleable_games,
            rng: StdRng::seed_from_u64(seed),
        })
    }

    /// Build a sampler from a corpus directory on disk.
    ///
    /// I/O and decoding errors propagate; an empty corpus surfaces as
    /// [`io::ErrorKind::InvalidData`].
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] if the directory cannot be read or decoded.
    pub fn from_corpus(dir: &Path, seed: u64) -> io::Result<Self> {
        let records = read_corpus(dir)?;
        Self::new(records, seed).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                SamplerError::EmptyCorpus.to_string(),
            )
        })
    }
}

impl Iterator for CorpusSampler {
    type Item = Result<Sample, SampleError>;

    fn next(&mut self) -> Option<Self::Item> {
        let game_slot = self.rng.random_range(0..self.sampleable_games.len());
        let game_index = self.sampleable_games[game_slot];
        let record = &self.records[game_index];

        let ply = self.rng.random_range(0..record.plies.len());
        let transform = Transform::ALL[self.rng.random_range(0..Transform::ALL.len())];

        Some(replay_sample(record, ply).map(|sample| augment_sample(&sample, transform)))
    }
}
