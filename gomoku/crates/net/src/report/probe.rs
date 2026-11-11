//! Probe boards for holdout evaluation.
//!
//! A probe is a fixed position chosen from the holdout set at run start.
//! At every holdout evaluation the network is run once per probe and the
//! resulting policy/value are carried in [`EvalReport`](crate::report::EvalReport).

use engine::Move;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use train::Sample;
use train::label::{TacticalClass, classify};

/// Tactical class of a probe position.
///
/// Mirrors [`train::label::TacticalClass`] but lives in `net` so the
/// reporter seam needs no new `train` re-exports and can derive serde.
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProbeClass {
    /// The side to move has an immediate winning move.
    Win,
    /// The opponent has an immediate winning move that must be blocked.
    Block,
    /// No immediate tactic is present.
    Quiet,
}

impl From<TacticalClass> for ProbeClass {
    fn from(class: TacticalClass) -> Self {
        match class {
            TacticalClass::Win => ProbeClass::Win,
            TacticalClass::Block => ProbeClass::Block,
            TacticalClass::Quiet => ProbeClass::Quiet,
        }
    }
}

/// A fixed probe position chosen from the holdout set.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProbePosition {
    /// The source sample (stones, side to move, policy target).
    pub sample: Sample,
    /// Tactical class of the position.
    pub class: ProbeClass,
    /// The expected move: argmax of the sample's policy target.
    ///
    /// For [`ProbeClass::Win`] and [`ProbeClass::Block`] probes this is a
    /// known-correct tactical move. For [`ProbeClass::Quiet`] it is the
    /// (deterministic) argmax of a uniform target and is only used for
    /// the `correct` flag.
    pub expected: Move,
}

/// The result of evaluating one probe position at a holdout step.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProbeResult {
    /// Step at which the probe was evaluated.
    pub step: usize,
    /// Tactical class of the probe.
    pub class: ProbeClass,
    /// Softmaxed policy distribution over the 225 board cells.
    ///
    /// Index `row * 15 + col` gives the probability mass for that cell.
    pub policy: Vec<f32>,
    /// The network's top-1 move.
    pub top1: Move,
    /// The network's value output.
    pub value: f32,
    /// Whether the top-1 move matches the expected move.
    pub correct: bool,
}

/// Select up to three probe positions from `samples`, one per tactical
/// class, in a reproducible seeded order.
///
/// Returns probes in class order Win, Block, Quiet. Missing classes are
/// omitted. Boards that fail to reconstruct are skipped.
pub fn select_probes(samples: &[Sample], seed: u64) -> Vec<ProbePosition> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut indices: Vec<usize> = (0..samples.len()).collect();
    indices.shuffle(&mut rng);

    let mut win = None;
    let mut block = None;
    let mut quiet = None;

    for &idx in &indices {
        if win.is_some() && block.is_some() && quiet.is_some() {
            break;
        }
        let sample = &samples[idx];
        let board = match sample.board() {
            Ok(b) => b,
            Err(_) => continue,
        };
        let class = classify(&board);
        if class == TacticalClass::Win && win.is_some() {
            continue;
        }
        if class == TacticalClass::Block && block.is_some() {
            continue;
        }
        if class == TacticalClass::Quiet && quiet.is_some() {
            continue;
        }

        let expected = policy_argmax(&sample.policy);
        let probe = ProbePosition {
            sample: sample.clone(),
            class: class.into(),
            expected,
        };

        match class {
            TacticalClass::Win => win = Some(probe),
            TacticalClass::Block => block = Some(probe),
            TacticalClass::Quiet => quiet = Some(probe),
        }
    }

    let mut out = Vec::with_capacity(3);
    if let Some(p) = win {
        out.push(p);
    }
    if let Some(p) = block {
        out.push(p);
    }
    if let Some(p) = quiet {
        out.push(p);
    }
    out
}

fn policy_argmax(policy: &[(Move, f32)]) -> Move {
    policy
        .iter()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(m, _)| *m)
        .expect("policy must contain at least one move")
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Move;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use train::collect::{Quotas, collect};

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    fn tiny_dataset(seed: u64, quotas: Quotas) -> Vec<Sample> {
        collect(quotas, &mut StdRng::seed_from_u64(seed))
    }

    #[test]
    fn select_probes_finds_one_per_class() {
        let samples = tiny_dataset(
            42,
            Quotas {
                win: 4,
                block: 4,
                quiet: 8,
            },
        );
        let probes = select_probes(&samples, 123);

        assert_eq!(probes.len(), 3, "expected one probe per class");
        assert_eq!(probes[0].class, ProbeClass::Win);
        assert_eq!(probes[1].class, ProbeClass::Block);
        assert_eq!(probes[2].class, ProbeClass::Quiet);
    }

    #[test]
    fn select_probes_is_reproducible() {
        let samples = tiny_dataset(
            7,
            Quotas {
                win: 3,
                block: 3,
                quiet: 6,
            },
        );
        let a = select_probes(&samples, 99);
        let b = select_probes(&samples, 99);
        assert_eq!(a, b);
    }

    #[test]
    fn select_probes_respects_seed() {
        let samples = tiny_dataset(
            7,
            Quotas {
                win: 10,
                block: 10,
                quiet: 20,
            },
        );
        let a = select_probes(&samples, 1);
        let b = select_probes(&samples, 2);
        assert_ne!(a, b, "different seeds should pick different probes");
    }

    #[test]
    fn select_probes_returns_empty_for_empty_samples() {
        let probes = select_probes(&[], 0);
        assert!(probes.is_empty());
    }

    #[test]
    fn select_probes_omits_missing_classes() {
        // Only quiet samples.
        let samples = tiny_dataset(
            11,
            Quotas {
                win: 0,
                block: 0,
                quiet: 5,
            },
        );
        let probes = select_probes(&samples, 0);
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].class, ProbeClass::Quiet);
    }

    #[test]
    fn probe_expected_move_is_policy_argmax() {
        let samples = tiny_dataset(
            42,
            Quotas {
                win: 4,
                block: 4,
                quiet: 8,
            },
        );
        let probes = select_probes(&samples, 123);
        for probe in &probes {
            let expected = probe
                .sample
                .policy
                .iter()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(m, _)| *m)
                .unwrap();
            assert_eq!(probe.expected, expected);
        }
    }

    #[test]
    fn probe_expected_move_is_legal() {
        let samples = tiny_dataset(
            42,
            Quotas {
                win: 4,
                block: 4,
                quiet: 8,
            },
        );
        let probes = select_probes(&samples, 123);
        for probe in &probes {
            let board = probe.sample.board().expect("probe board must rebuild");
            let legal: Vec<Move> = board.empty_moves().collect();
            assert!(
                legal.contains(&probe.expected),
                "expected move {:?} must be legal",
                probe.expected
            );
        }
    }

    #[test]
    fn select_probes_handles_hand_built_boards() {
        // A hand-built win-in-1 position: White to move completes the
        // five-in-a-row at (7, 11). Counts must respect alternating play,
        // so Black (the side that just moved) has one more stone than White.
        let sample = Sample {
            black: vec![mv(0, 0), mv(0, 1), mv(0, 2), mv(1, 0), mv(1, 1)],
            white: vec![mv(7, 7), mv(7, 8), mv(7, 9), mv(7, 10)],
            to_move: engine::Color::White,
            policy: vec![(mv(7, 11), 1.0)],
            value: 1.0,
        };
        let probes = select_probes(std::slice::from_ref(&sample), 0);
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].class, ProbeClass::Win);
        assert_eq!(probes[0].expected, mv(7, 11));
    }
}
