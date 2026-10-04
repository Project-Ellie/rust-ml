//! Deterministic train/held-out split and dataset statistics.
//!
//! The split is computed from the board's Zobrist key, not from the
//! sample's position in a file or shard. This prevents the collection
//! order (which is driven by per-class quotas) from leaking correlated
//! positions into both sides of the partition.

use crate::label::{TacticalClass, classify};
use crate::sample::Sample;

/// How many samples to be held out for validation
pub const HOLDOUT_PERCENTAGE: u64 = 10;

/// Deterministic partition: sample belongs to held-out iff
/// `zobrist(board) % 100 < HOLDOUT_PERCENTAGE` (~10%). Position-in-file
/// never decides, so quota ordering cannot skew the split. The threshold
/// form nests: raising the percentage only grows the holdout set, it
/// never swaps members between splits.
///
/// If the sample cannot be rebuilt into a legal board, it is treated as
/// held-out. Invalid samples should not appear in a sound dataset;
/// keeping them out of the training split is the conservative default.
pub fn is_holdout(sample: &Sample) -> bool {
    match sample.board() {
        Ok(board) => board.zobrist() % 100 < HOLDOUT_PERCENTAGE,
        Err(_) => true,
    }
}

/// Statistics for a slice of samples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stats {
    /// total number of samples
    pub total: usize,
    /// Per-class counts: `("win", win_count)`, `("block", block_count)`,
    /// `("quiet", quiet_count)`.
    pub per_class: [(&'static str, usize); 3],
    /// Histogram of position lengths.
    ///
    /// Index `i` holds the number of samples that have exactly `i` stones
    /// on the board. The sum of all entries equals [`Stats::total`].
    pub ply_histogram: Vec<usize>,
    /// Number of samples assigned to the held-out split.
    pub holdout: usize,
    /// Number of samples assigned to the training split.
    pub train: usize,
}

/// Compute [`Stats`] over `samples`.
///
/// Rebuilds each board to determine its tactical class and stone count.
///
/// # Panics
///
/// Panics if a sample cannot be rebuilt into a legal board. A sound
/// dataset should not contain such records.
pub fn stats(samples: &[Sample]) -> Stats {
    let mut win = 0;
    let mut block = 0;
    let mut quiet = 0;
    let mut holdout = 0;
    let mut ply_histogram: Vec<usize> = Vec::new();

    for sample in samples {
        let board = sample
            .board()
            .expect("stats expects samples that rebuild into legal boards");
        match classify(&board) {
            TacticalClass::Win => win += 1,
            TacticalClass::Block => block += 1,
            TacticalClass::Quiet => quiet += 1,
        }
        if is_holdout(sample) {
            holdout += 1;
        }

        let stones = sample.black.len() + sample.white.len();
        // >=, not >: indexing slot `stones` requires len >= stones + 1,
        // so a stone count exactly equal to the current length must
        // still trigger the resize.
        if stones >= ply_histogram.len() {
            ply_histogram.resize(stones + 1, 0);
        }
        ply_histogram[stones] += 1;
    }
    Stats {
        total: samples.len(),
        per_class: [("win", win), ("block", block), ("quiet", quiet)],
        ply_histogram,
        holdout,
        train: samples.len() - holdout,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collect::{Quotas, collect};
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn tiny_dataset() -> Vec<Sample> {
        collect(
            Quotas {
                win: 2,
                block: 2,
                quiet: 3,
            },
            &mut StdRng::seed_from_u64(42),
        )
    }

    /// Push one stone into the opposite-color list so the position can no
    /// longer be rebuilt into a legal board.
    fn corrupt_sample(sample: &mut Sample) {
        if let Some(mv) = sample.white.first().copied() {
            sample.black.push(mv);
        } else {
            let mv = sample
                .black
                .first()
                .copied()
                .expect("sample has at least one stone");
            sample.white.push(mv);
        }
    }

    #[test]
    fn split_is_deterministic_and_partition_complete() {
        let samples = tiny_dataset();

        for sample in &samples {
            let a = is_holdout(sample);
            let b = is_holdout(sample);
            assert_eq!(a, b, "is_holdout is deterministic for every sample");
        }

        let holdout_count = samples.iter().filter(|s| is_holdout(s)).count();
        let train_count = samples.iter().filter(|s| !is_holdout(s)).count();
        assert_eq!(holdout_count + train_count, samples.len());
    }

    #[test]
    fn holdout_fraction_is_within_ten_percent_bounds() {
        // ~500 samples keeps the test fast while giving a tight bound.
        let quotas = Quotas {
            win: 50,
            block: 50,
            quiet: 400,
        };
        let samples = collect(quotas, &mut StdRng::seed_from_u64(42));
        let holdout = samples.iter().filter(|s| is_holdout(s)).count();
        let fraction = holdout as f64 / samples.len() as f64;

        assert!(
            (0.05..=0.15).contains(&fraction),
            "holdout fraction {fraction} outside 5–15% for {} samples",
            samples.len()
        );
    }

    #[test]
    fn stats_counts_match_quotas() {
        let quotas = Quotas {
            win: 10,
            block: 10,
            quiet: 30,
        };
        let samples = collect(quotas, &mut StdRng::seed_from_u64(7));
        let s = stats(&samples);

        assert_eq!(s.total, samples.len());
        assert_eq!(s.per_class[0], ("win", quotas.win));
        assert_eq!(s.per_class[1], ("block", quotas.block));
        assert_eq!(s.per_class[2], ("quiet", quotas.quiet));
        assert_eq!(s.holdout + s.train, s.total);
    }

    #[test]
    fn ply_histogram_resizes_when_stone_count_equals_current_length() {
        // Regression for the 25k-sample panic: the resize guard used to
        // be `stones > len`, skipping the resize when a stone count
        // equaled the histogram length and then indexing one past the
        // end. Two samples with consecutive stone counts hit that
        // boundary deterministically.
        let m1 = engine::Move::new(7, 7).unwrap();
        let m2 = engine::Move::new(7, 8).unwrap();

        let one_stone = Sample::from_position(&[m1], 1, Vec::new(), 0.0);
        let two_stones = Sample::from_position(&[m1, m2], 2, Vec::new(), 0.0);

        let s = stats(&[one_stone, two_stones]);
        assert_eq!(s.ply_histogram[1], 1);
        assert_eq!(s.ply_histogram[2], 1);
    }

    #[test]
    fn ply_histogram_sums_to_total_and_indexes_by_stone_count() {
        let samples = tiny_dataset();
        let s = stats(&samples);

        assert_eq!(s.ply_histogram.iter().sum::<usize>(), s.total);

        for sample in &samples {
            let stones = sample.black.len() + sample.white.len();
            assert!(
                stones < s.ply_histogram.len(),
                "histogram must be long enough for stone count {stones}"
            );
            assert!(s.ply_histogram[stones] > 0);
        }
    }

    #[test]
    fn unbuildable_sample_is_treated_as_holdout() {
        let mut samples = tiny_dataset();
        let mut corrupted = samples.pop().expect("dataset is non-empty");
        // Create an overlapping stone so the board cannot be rebuilt.
        corrupt_sample(&mut corrupted);

        assert!(
            is_holdout(&corrupted),
            "an unbuildable sample must be treated as held-out"
        );
    }

    #[test]
    #[should_panic(expected = "stats expects samples that rebuild into legal boards")]
    fn stats_panics_on_unbuildable_sample() {
        let mut samples = tiny_dataset();
        let mut corrupted = samples.pop().expect("dataset is non-empty");
        corrupt_sample(&mut corrupted);

        stats(&[corrupted]);
    }
}
