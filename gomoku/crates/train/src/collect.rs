//! Collect samples by classification
//!

use crate::Sample;
use crate::label::{TacticalClass, classify, label};
use crate::playout::{MAX_PLIES_PER_GAME, RandomMode, random_game, sample_plies};
use engine::Status::Ongoing;
use engine::{Board, Move};
use rand::Rng;
use std::collections::HashSet;

/// quotas for the three classes
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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
/// All-zero quotas return an empty vector immediately.
/// Deterministic under `rng` seed.
///
/// # Panics
///
/// Panics if the quotas are still unmet after [`GAME_BUDGET`] games.
/// Random play produces far more quiet positions than tactical ones,
/// so an unreachable quota would otherwise hang forever; the budget
/// turns that into an error with the missing counts attached.
pub fn collect(quotas: Quotas, rng: &mut impl Rng) -> Vec<Sample> {
    let targets = [quotas.win, quotas.block, quotas.quiet];
    let mut counts = [0_usize; 3];
    let mut seen: HashSet<u64> = HashSet::new();
    let mut samples: Vec<Sample> = Vec::with_capacity(targets.iter().sum());
    let mut games = 0_usize;

    while !buckets_full(&counts, &targets) {
        games += 1;
        assert!(
            games <= GAME_BUDGET,
            "quotas unmet after {GAME_BUDGET} games — unsatisfiable? \
             still missing: win {}, block {}, quiet {}",
            targets[0] - counts[0],
            targets[1] - counts[1],
            targets[2] - counts[2],
        );
        let history = random_game(rng, RandomMode::NoFocus);
        let plies = sample_plies(history.len(), MAX_PLIES_PER_GAME, rng);

        for ply in plies {
            let board = board_at_prefix(&history, ply);
            if board.status() != Ongoing {
                continue;
            }
            // Dedup before classifying: a known position need not pay
            // for the tactics scan a second time.
            let key = board.zobrist();
            if seen.contains(&key) {
                continue;
            }
            let class = classify(&board);
            let idx = class_index(class);

            if counts[idx] >= targets[idx] {
                continue;
            }
            seen.insert(key);

            let (policy, value) = label(&board);
            samples.push(Sample::from_position(&history, ply, policy, value));
            counts[idx] += 1;
        }
    }
    samples
}

/// Upper bound on games played before quotas must be met. Chapter-8
/// quotas need hundreds to thousands of games; two orders of
/// magnitude above that means the quota is unsatisfiable, not slow.
const GAME_BUDGET: usize = 100_000;

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

fn board_at_prefix(history: &[Move], ply: usize) -> Board {
    let mut board = Board::new();
    for &mv in &history[..ply] {
        board.play(mv).expect("History prefix must be legal.")
    }
    board
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use std::collections::HashSet;

    fn class_counts(samples: &[Sample]) -> (usize, usize, usize) {
        let mut win = 0;
        let mut block = 0;
        let mut quiet = 0;

        for sample in samples {
            let board = sample.board().expect("Should have board");
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
        assert_eq!(win, quotas.win, "Win count mismatch");
        assert_eq!(block, quotas.block, "Block count mismatch");
        assert_eq!(quiet, quotas.quiet, "Quiet count mismatch");
        assert_eq!(samples.len(), quotas.win + quotas.block + quotas.quiet);

        for sample in &samples {
            let board = sample.board().expect("Sample must rebuild.");
            assert_eq!(
                board.status(),
                Ongoing,
                "Collected Sample must be ongoing position."
            );
        }
    }

    #[test]
    fn terminal_ply_is_never_sampled() {
        // seed 0 provably samples a terminal ply if the Ongoing skip is
        // removed (verified against the pre-fix implementation).
        let quotas = Quotas {
            win: 2,
            block: 2,
            quiet: 3,
        };
        let mut rng = StdRng::seed_from_u64(0);
        let samples = collect(quotas, &mut rng);

        for sample in &samples {
            let board = sample.board().expect("sample must rebuild");
            assert_eq!(
                board.status(),
                Ongoing,
                "Collected sample must be an ongoing position."
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
            .map(|s| s.board().expect("Sample must rebuild.").zobrist())
            .collect();

        assert_eq!(
            keys.len(),
            samples.len(),
            "There can't be duplicate Zobrist keys."
        )
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
