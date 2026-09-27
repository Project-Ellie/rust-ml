# Chapter 04 solution — Quota collector

This file shows the reference implementation for chapter 04. It is
identical to the verified `collect.rs` in the reference worktree, plus
the one-line registration change in `lib.rs`.

## `gomoku/crates/train/src/lib.rs`

Add `pub mod collect;` with the other module declarations:

```rust
pub mod collect;
pub mod label;
pub mod playout;
pub mod sample;
```

## `gomoku/crates/train/src/collect.rs`

```rust
//! Quota collector for synthetic training samples.
//!
//! Runs random games until per-class quotas are met, deduplicating
//! positions by their Zobrist key.

use std::collections::HashSet;

use engine::Board;
use rand::Rng;

use crate::label::{TacticalClass, classify, label};
use crate::playout::{MAX_PLIES_PER_GAME, random_game, sample_plies};
use crate::sample::Sample;

/// Per-class collection targets.
#[derive(Debug, Clone, Copy)]
pub struct Quotas {
    /// How many unique win positions to collect.
    pub win: usize,
    /// How many unique block positions to collect.
    pub block: usize,
    /// How many unique quiet positions to collect.
    pub quiet: usize,
}

/// Run random games until every class bucket is full. Positions are
/// deduplicated by Zobrist key; each game yields at most
/// [`MAX_PLIES_PER_GAME`] samples. Terminal plies are skipped because
/// the network is never asked to evaluate finished positions.
/// Deterministic under `rng` seed.
pub fn collect(quotas: Quotas, rng: &mut impl Rng) -> Vec<Sample> {
    let targets = [quotas.win, quotas.block, quotas.quiet];
    let mut counts = [0_usize; 3];
    let mut seen = HashSet::new();
    let mut samples = Vec::with_capacity(targets.iter().sum());

    while !buckets_full(&counts, &targets) {
        let history = random_game(rng);
        let plies = sample_plies(history.len(), MAX_PLIES_PER_GAME, rng);

        for ply in plies {
            let board = board_at_prefix(&history, ply);
            // Terminals are never evaluated by the network, so they never enter the dataset.
            if board.status() != engine::Status::Ongoing {
                continue;
            }
            let class = classify(&board);
            let idx = class_index(class);

            if counts[idx] >= targets[idx] {
                continue;
            }

            let key = board.zobrist();
            if !seen.insert(key) {
                continue;
            }

            let (policy, value) = label(&board);
            samples.push(Sample::from_position(&history, ply, policy, value));
            counts[idx] += 1;
        }
    }

    samples
}

fn buckets_full(counts: &[usize; 3], targets: &[usize; 3]) -> bool {
    counts.iter().zip(targets.iter()).all(|(c, t)| c >= t)
}

fn class_index(class: TacticalClass) -> usize {
    match class {
        TacticalClass::Win => 0,
        TacticalClass::Block => 1,
        TacticalClass::Quiet => 2,
    }
}

fn board_at_prefix(history: &[engine::Move], ply: usize) -> Board {
    let mut board = Board::new();
    for &mv in &history[..ply] {
        // `random_game` only ever plays legal moves.
        board.play(mv).expect("history prefix must be legal");
    }
    board
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn class_counts(samples: &[Sample]) -> (usize, usize, usize) {
        let mut win = 0;
        let mut block = 0;
        let mut quiet = 0;
        for sample in samples {
            let board = sample.board().expect("sample must rebuild");
            match classify(&board) {
                TacticalClass::Win => win += 1,
                TacticalClass::Block => block += 1,
                TacticalClass::Quiet => quiet += 1,
            }
        }
        (win, block, quiet)
    }

    #[test]
    fn tiny_quotas_produce_exact_class_counts() {
        let quotas = Quotas {
            win: 2,
            block: 2,
            quiet: 3,
        };
        let mut rng = StdRng::seed_from_u64(42);
        let samples = collect(quotas, &mut rng);

        let (win, block, quiet) = class_counts(&samples);
        assert_eq!(win, quotas.win, "win count mismatch");
        assert_eq!(block, quotas.block, "block count mismatch");
        assert_eq!(quiet, quotas.quiet, "quiet count mismatch");
        assert_eq!(samples.len(), quotas.win + quotas.block + quotas.quiet);

        for sample in &samples {
            let board = sample.board().expect("sample must rebuild");
            assert_eq!(
                board.status(),
                engine::Status::Ongoing,
                "collected sample must be an ongoing position"
            );
        }
    }

    #[test]
    fn collected_samples_have_unique_zobrist_keys() {
        let quotas = Quotas {
            win: 2,
            block: 2,
            quiet: 3,
        };
        let mut rng = StdRng::seed_from_u64(42);
        let samples = collect(quotas, &mut rng);

        let keys: HashSet<u64> = samples
            .iter()
            .map(|s| s.board().expect("sample must rebuild").zobrist())
            .collect();
        assert_eq!(keys.len(), samples.len(), "duplicate Zobrist keys found");
    }

    #[test]
    fn same_seed_gives_identical_samples() {
        let quotas = Quotas {
            win: 2,
            block: 2,
            quiet: 3,
        };
        let a = collect(quotas, &mut StdRng::seed_from_u64(42));
        let b = collect(quotas, &mut StdRng::seed_from_u64(42));
        assert_eq!(a, b);
    }
}
```
