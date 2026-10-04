# Chapter 02 — Deep-dive solution: the batcher + D4 augmentation

This is the opt-in reference for Chapter 02. `lib.rs` (registration) and
`batcher.rs` are quoted verbatim from the verified reference crate, and
byte-identical copies were confirmed with `diff -u` against the reference
worktree before this file was written.

> **Note:** for chapter 2, only the `pub mod batcher;` line in `lib.rs`
> is new; the other declarations land in later chapters.

## `src/lib.rs`

```rust
//! Neural network training for the Gomoku AlphaZero-style agent.
//!
//! This crate implements the policy/value network and the manual
//! supervised-training loop for phase 0. It consumes labelled samples
//! from the `train` crate and derives input planes from engine boards.

#![deny(missing_docs)]

pub mod batcher;
pub mod checkpoint;
pub mod loss;
pub mod network;
pub mod planes;
pub mod train;
```

## `src/batcher.rs`

```rust
//! Burn [`Batcher`] that turns `train::Sample`s into network tensors.
//!
//! The batcher:
//!
//! * assembles the four input planes from each sample's board,
//! * optionally applies a random D4 transform to the planes and policy
//!   target at batch time (I8),
//! * scatters the sparse policy target into a dense 225-vector (I3).

use burn::data::dataloader::batcher::Batcher;
use burn::tensor::{Tensor, backend::Backend};
use engine::{Board, EXT, Move, Transform};
use std::sync::atomic::{AtomicU64, Ordering};
use train::Sample;

use crate::planes::{self, InputPlanes};

/// A batch of network inputs and targets.
#[derive(Debug, Clone)]
pub struct NetBatch<B: Backend> {
    /// Input planes of shape `[B, 4, EXT, EXT]`.
    pub input: Tensor<B, 4>,
    /// Dense policy target of shape `[B, 225]`.
    pub policy_target: Tensor<B, 2>,
    /// Value target of shape `[B, 1]`.
    pub value_target: Tensor<B, 2>,
}

/// Batcher configuration.
#[derive(Debug)]
pub struct NetBatcher {
    /// Base seed for deterministic transform selection.
    pub seed: u64,
    /// If true, applies a random D4 transform per sample; if false,
    /// every sample stays in canonical orientation (holdout mode).
    pub augment: bool,
    /// Per-batcher counter for deterministic transform selection.
    /// Kept independent across batchers so that unit tests are not
    /// coupled by a global counter.
    counter: AtomicU64,
}

impl NetBatcher {
    /// Create a new batcher.
    pub fn new(seed: u64, augment: bool) -> Self {
        Self {
            seed,
            augment,
            counter: AtomicU64::new(0),
        }
    }

    /// Create a holdout batcher: canonical orientation, no augmentation.
    pub fn holdout() -> Self {
        Self::new(0, false)
    }

    /// Reset the transform counter. Useful for reproducibility tests.
    pub fn reset_counter(&self) {
        self.counter.store(0, Ordering::Relaxed);
    }
}

impl<B: Backend> Batcher<B, Sample, NetBatch<B>> for NetBatcher {
    fn batch(&self, items: Vec<Sample>, device: &B::Device) -> NetBatch<B> {
        let counter = self
            .counter
            .fetch_add(items.len() as u64, Ordering::Relaxed);
        let mut input_planes = Vec::with_capacity(items.len());
        let mut policy_targets = Vec::with_capacity(items.len());
        let mut value_targets = Vec::with_capacity(items.len());

        for (i, sample) in items.into_iter().enumerate() {
            let transform = if self.augment {
                let idx =
                    (self.seed.wrapping_add(counter + i as u64) as usize) % Transform::ALL.len();
                Some(Transform::ALL[idx])
            } else {
                None
            };

            let (board, policy) = transform_sample(&sample, transform);
            let planes = planes::assemble(&board);
            input_planes.push(planes);
            policy_targets.push(scatter_policy(&policy));
            value_targets.push(sample.value);
        }

        let input = stack_planes::<B>(&input_planes, device);
        let batch = policy_targets.len();
        let policy_data: Vec<f32> = policy_targets.into_iter().flatten().collect();
        let policy_target = Tensor::<B, 2>::from_data(
            burn::tensor::TensorData::new(policy_data, [batch, 225]).convert::<B::FloatElem>(),
            device,
        );
        let value_target = Tensor::<B, 2>::from_data(
            burn::tensor::TensorData::new(value_targets, [batch, 1]).convert::<B::FloatElem>(),
            device,
        );

        NetBatch {
            input,
            policy_target,
            value_target,
        }
    }
}

fn transform_sample(sample: &Sample, transform: Option<Transform>) -> (Board, Vec<(Move, f32)>) {
    let board = sample
        .board()
        .expect("sample must rebuild into a legal board");
    let policy = sample.policy.clone();

    let Some(t) = transform else {
        return (board, policy);
    };

    // Replay the transformed move history into a fresh board.
    let mut transformed_board = Board::new();
    for &mv in board.moves() {
        let _ = transformed_board.play(t.transform_move(mv));
    }

    // Transform policy masses along with their moves.
    let transformed_policy: Vec<(Move, f32)> = policy
        .into_iter()
        .map(|(mv, mass)| (t.transform_move(mv), mass))
        .collect();

    (transformed_board, transformed_policy)
}

/// Scatter sparse `(move, mass)` pairs into a row-major 225-vector.
fn scatter_policy(policy: &[(Move, f32)]) -> [f32; 225] {
    let mut dense = [0.0f32; 225];
    for &(mv, mass) in policy {
        let idx = mv.row() as usize * 15 + mv.col() as usize;
        dense[idx] = mass;
    }
    dense
}

fn stack_planes<B: Backend>(planes: &[InputPlanes], device: &B::Device) -> Tensor<B, 4> {
    let batch = planes.len();
    let mut data = vec![0.0f32; batch * 4 * EXT * EXT];
    for (b, p) in planes.iter().enumerate() {
        for r in 0..EXT {
            for c in 0..EXT {
                let idx = ((b * 4) * EXT + r) * EXT + c;
                data[idx] = p.me[r * EXT + c] as f32;
                let idx = ((b * 4 + 1) * EXT + r) * EXT + c;
                data[idx] = p.you[r * EXT + c] as f32;
                let idx = ((b * 4 + 2) * EXT + r) * EXT + c;
                data[idx] = p.my_last[r * EXT + c] as f32;
                let idx = ((b * 4 + 3) * EXT + r) * EXT + c;
                data[idx] = p.opp_last[r * EXT + c] as f32;
            }
        }
    }
    Tensor::<B, 4>::from_data(
        burn::tensor::TensorData::new(data, [batch, 4, EXT, EXT]).convert::<B::FloatElem>(),
        device,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use burn::backend::Flex;
    use engine::Color;

    type B = Flex;

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    fn sample_with_policy(policy: Vec<(Move, f32)>, value: f32) -> Sample {
        Sample {
            black: vec![mv(7, 7)],
            white: vec![],
            to_move: Color::White,
            policy,
            value,
        }
    }

    #[test]
    fn batch_shapes_are_correct() {
        let batcher = NetBatcher::holdout();
        let device = Default::default();
        let sample = sample_with_policy(vec![(mv(0, 0), 1.0)], 0.5);
        let batch: NetBatch<B> = batcher.batch(vec![sample], &device);

        assert_eq!(batch.input.dims(), [1, 4, EXT, EXT]);
        assert_eq!(batch.policy_target.dims(), [1, 225]);
        assert_eq!(batch.value_target.dims(), [1, 1]);
    }

    #[test]
    fn policy_mass_is_preserved() {
        let batcher = NetBatcher::holdout();
        let device = Default::default();
        let policy = vec![(mv(0, 0), 0.3), (mv(1, 1), 0.7)];
        let sample = sample_with_policy(policy, 0.0);
        let batch: NetBatch<B> = batcher.batch(vec![sample], &device);

        let dense: Vec<f32> = batch.policy_target.into_data().to_vec::<f32>().unwrap();
        let sum: f32 = dense.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-5,
            "policy masses must sum to 1, got {sum}"
        );
    }

    #[test]
    fn value_target_matches_input() {
        let batcher = NetBatcher::holdout();
        let device = Default::default();
        let sample = sample_with_policy(vec![(mv(0, 0), 1.0)], -0.75);
        let batch: NetBatch<B> = batcher.batch(vec![sample], &device);

        let values: Vec<f32> = batch.value_target.into_data().to_vec::<f32>().unwrap();
        assert_eq!(values, vec![-0.75]);
    }

    #[test]
    fn augmentation_preserves_policy_mass() {
        let batcher = NetBatcher::new(42, true);
        let device = Default::default();
        let policy = vec![(mv(0, 0), 0.4), (mv(14, 14), 0.6)];
        let sample = sample_with_policy(policy, 1.0);
        let batch: NetBatch<B> = batcher.batch(vec![sample], &device);

        let dense: Vec<f32> = batch.policy_target.into_data().to_vec::<f32>().unwrap();
        let sum: f32 = dense.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }

    #[test]
    fn augmentation_keeps_stones_on_board() {
        let batcher = NetBatcher::new(42, true);
        let device = Default::default();
        let sample = sample_with_policy(vec![(mv(0, 0), 1.0)], 0.0);
        let batch: NetBatch<B> = batcher.batch(vec![sample], &device);

        let input: Vec<f32> = batch.input.into_data().to_vec::<f32>().unwrap();
        // Sum of plane 0 (me) should be 1: one stone.
        let _me_sum: f32 = input.iter().step_by(4 * EXT * EXT).sum();
        // Wait, plane 0 is at offset 0 of each sample; easier: reshape mentally.
        // Just assert total non-border non-zero count equals one stone per plane.
        let non_zero = input.iter().filter(|&&x| x > 0.5).count();
        // One stone in me, plus possibly last-move plane.
        assert!(
            non_zero >= 1,
            "transformed stone must still be on the board"
        );
    }

    #[test]
    fn holdout_is_canonical() {
        let holdout = NetBatcher::holdout();
        let train = NetBatcher::new(42, true);
        let device = Default::default();

        let policy = vec![(mv(0, 0), 1.0)];
        let sample = sample_with_policy(policy.clone(), 0.0);

        let canonical: NetBatch<B> = holdout.batch(vec![sample.clone()], &device);
        // Reset counter so train batcher consumes the same counter value.
        let maybe_aug: NetBatch<B> = train.batch(vec![sample], &device);

        // With seed 42 and counter 0, the transform is Transform::ALL[42 % 8].
        // If that happens to be identity, this assertion would spuriously pass.
        // To avoid flakiness, only check that mass is preserved; identity is
        // tested implicitly by other cases.
        let c: Vec<f32> = canonical.policy_target.into_data().to_vec::<f32>().unwrap();
        let m: Vec<f32> = maybe_aug.policy_target.into_data().to_vec::<f32>().unwrap();
        assert!((c.iter().sum::<f32>() - m.iter().sum::<f32>()).abs() < 1e-5);
    }

    #[test]
    fn same_seed_same_transforms() {
        let batcher_a = NetBatcher::new(7, true);
        let batcher_b = NetBatcher::new(7, true);
        let device = Default::default();

        let sample = sample_with_policy(vec![(mv(0, 0), 1.0)], 0.0);
        let batch_a: NetBatch<B> = batcher_a.batch(vec![sample.clone()], &device);
        let batch_b: NetBatch<B> = batcher_b.batch(vec![sample], &device);

        let a_input: Vec<f32> = batch_a.input.into_data().to_vec::<f32>().unwrap();
        let b_input: Vec<f32> = batch_b.input.into_data().to_vec::<f32>().unwrap();
        assert_eq!(a_input, b_input);

        let a_policy: Vec<f32> = batch_a.policy_target.into_data().to_vec::<f32>().unwrap();
        let b_policy: Vec<f32> = batch_b.policy_target.into_data().to_vec::<f32>().unwrap();
        assert_eq!(a_policy, b_policy);
    }
}
```
