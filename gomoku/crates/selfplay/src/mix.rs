//! Weighted round-robin mixing of sample streams.
//!
//! The mixer knows nothing about samples, boards, or records. It combines
//! generic iterators so that one stream can mix plain synthetic samples and
//! fallible corpus draws without hiding replay errors. Each source contributes
//! `weight` consecutive items per turn; the seed picks the initial source,
//! making the stream deterministic without introducing a second random
//! scheduling policy. Exhausted finite sources are skipped, the mixer ends
//! after every source is exhausted, and infinite sources make the mixed stream
//! infinite.

use rand::RngExt;
use rand::SeedableRng;
use rand::rngs::StdRng;
use thiserror::Error;

/// Failure to construct a source mixer.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MixError {
    /// No sources were supplied.
    #[error("a source mixer needs at least one source")]
    Empty,

    /// One source had zero weight and could never be drawn.
    #[error("source {index} has zero weight")]
    ZeroWeight {
        /// The zero-based source index.
        index: usize,
    },
}

/// One weighted iterator in a [`SourceMixer`].
///
/// The item type is generic so a mixer can combine plain synthetic
/// [`train::Sample`] iterators and fallible corpus draws
/// (`Result<Sample, SampleError>`) without hiding replay errors. A zero weight
/// is rejected downstream by [`SourceMixer::new`], alongside the other sources;
/// [`WeightedSource::new`] itself does not validate.
pub struct WeightedSource<T> {
    weight: u32,
    iterator: Box<dyn Iterator<Item = T>>,
    exhausted: bool,
}

impl<T> WeightedSource<T> {
    /// Box an iterator with a weight.
    #[must_use]
    pub fn new(weight: u32, samples: impl Iterator<Item = T> + 'static) -> Self {
        Self {
            weight,
            iterator: Box::new(samples),
            exhausted: false,
        }
    }
}

/// Seeded weighted round-robin over generic sample iterators.
pub struct SourceMixer<T> {
    sources: Vec<WeightedSource<T>>,
    cursor: usize,
    emitted_in_turn: u32,
    active: usize,
}

impl<T> SourceMixer<T> {
    /// Build a mixer from weighted sources.
    ///
    /// An empty vector fails with [`MixError::Empty`]; the first zero-weight
    /// source fails with [`MixError::ZeroWeight`]. The seed is used exactly
    /// once to pick the initial source uniformly.
    ///
    /// # Errors
    ///
    /// Returns [`MixError::Empty`] or [`MixError::ZeroWeight`] as described.
    pub fn new(sources: Vec<WeightedSource<T>>, seed: u64) -> Result<Self, MixError> {
        if sources.is_empty() {
            return Err(MixError::Empty);
        }
        if let Some(index) = sources.iter().position(|source| source.weight == 0) {
            return Err(MixError::ZeroWeight { index });
        }

        let active = sources.len();
        let mut rng = StdRng::seed_from_u64(seed);
        let cursor = rng.random_range(0..active);

        Ok(Self {
            sources,
            cursor,
            emitted_in_turn: 0,
            active,
        })
    }

    fn advance(&mut self) {
        self.cursor = (self.cursor + 1) % self.sources.len();
        self.emitted_in_turn = 0;
    }
}

impl<T> Iterator for SourceMixer<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        while self.active > 0 {
            if self.sources[self.cursor].exhausted {
                self.advance();
                continue;
            }

            match self.sources[self.cursor].iterator.next() {
                Some(item) => {
                    self.emitted_in_turn += 1;
                    if self.emitted_in_turn >= self.sources[self.cursor].weight {
                        self.advance();
                    }
                    return Some(item);
                }
                None => {
                    self.sources[self.cursor].exhausted = true;
                    self.active -= 1;
                    self.advance();
                }
            }
        }
        None
    }
}
