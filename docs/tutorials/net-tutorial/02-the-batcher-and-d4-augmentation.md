# Chapter 02 — The batcher + D4 augmentation

## Abstract

This chapter turns labelled `train::Sample`s into Burn tensors. By the
end you will have a `NetBatcher` that implements Burn 0.22's `Batcher`
trait, scatters sparse policy targets into a dense 225-vector, and
applies a random D4 transform to each training sample — all while
leaving the holdout split in canonical orientation.

## Glossary

| Term | Definition |
|------|------------|
| **`Batcher`** | Burn 0.22 trait: `Vec<Item>` + device → `Batch`. |
| **`NetBatch`** | A batch of `(input, policy_target, value_target)` tensors. |
| **`TensorData::new(data, shape)`** | Burn helper that builds a tensor from host data. |
| **Scatter (policy)** | Expanding sparse `(move, mass)` pairs into a dense 225-vector. |
| **D4 / dihedral group** | The 8 symmetries of a square: 4 rotations + 4 reflections. |
| **`Transform::ALL`** | Engine array of the 8 transforms in canonical order. |
| **I2, I3, I8** | Inventions: batcher design, policy flattening, augmentation scope. |

## Context

Chapter 1 built the `InputPlanes` representation: four 17×17 `u8`
arrays assembled from a `Board`. This chapter connects that
representation to Burn's data pipeline.

The batcher is the only data bridge between the `train` crate's records
and the network. It receives a `Vec<train::Sample>`, rebuilds each
sample's `Board`, optionally applies a D4 symmetry, assembles the four
planes, and produces three tensors: `[B, 4, 17, 17]` input, `[B, 225]`
policy target, and `[B, 1]` value target.

## Intention

1. Register `pub mod batcher;` in `src/lib.rs`.
2. Create `src/batcher.rs`:
   * `NetBatch` with `input`, `policy_target`, `value_target`.
   * `NetBatcher` with `seed`, `augment` flag, and an internal counter.
   * `impl Batcher<Sample, NetBatch> for NetBatcher`
     with `fn batch(&self, items: Vec<Sample>, device: &Device) -> NetBatch`.
3. Implement `scatter_policy`: `(Move, f32)` pairs → `[f32; 225]` in
   row-major order.
4. Implement D4 augmentation:
   * pick a transform per sample from `Transform::ALL` using a seeded,
     per-batcher counter,
   * transform the board by replaying transformed moves,
   * transform policy move coordinates with the same transform.
5. Holdout mode: `NetBatcher::holdout()` sets `augment = false`.
6. Write tests: batch shapes, policy mass preservation, value target
   match, augmentation legality, holdout canonical, seed determinism.

Observable done-state: `cargo test -p net` passes, `cargo clippy
--all-targets -- -D warnings` is green, and `cargo fmt --all -- --check`
makes no changes.

## Mental mapping

### Why implement `Batcher`?

Burn's training examples usually consume a `Batcher`. The trait is small
— one method — but it gives us a clean seam: the training loop passes
raw `Sample`s to the batcher and receives tensors. Keeping the batcher
as a separate type also makes it easy to test augmentation and
shape semantics in isolation.

The alternative is to inline batch construction in the training loop.
That works for a throwaway script, but it couples data layout to
optimization logic and makes unit testing harder. A standalone batcher
is the conventional Burn home for this concern.

This is **invention I2**, presented as a choice: we choose to implement
Burn's `Batcher` trait with the exact generics Burn 0.22 expects.

> **Excursion — generic signatures in Burn 0.22**
>
> In Burn 0.22 the `Batcher` trait is not generic over a backend type.
> The implementation binds `Item = Sample` and `Output = NetBatch`, and the
> same batcher works for any `Device` (Flex CPU, Wgpu GPU, etc.). For
> training, pass an autodiff device (`device.autodiff()`) so that tensors
> record gradients. Always verify the trait signature against the pinned
> source; blog posts and `main`-branch docs are wrong for this version.

### Why scatter policy targets?

The `Sample` stores a sparse policy: a list of `(move, mass)` pairs.
The network outputs 225 logits. The loss needs a dense target of the
same shape. "Scatter" means placing each mass at the row-major index of
its move: `idx = row * 15 + col`. Moves are guaranteed to be legal, so
no mass ever lands on padding.

This is **invention I3**, presented as a choice. Alternatives exist:
keeping the policy sparse inside the loss (more bookkeeping) or using
Burn's one-hot helper (only works for hard targets). For soft 90/10
labels, a hand-written scatter is the clearest match.

### Why D4 at batch time, train-split-only?

The board is square, so every position has 8 equivalent orientations.
Applying one random transform per sample every epoch effectively
multiplies the dataset size by up to 8 without writing new shards.

Two constraints:

- **Batch time, not generation time.** The datagen tutorial writes
  canonical samples. The batcher augments on read, so the same shard
  can produce a different transformed input on every epoch. This is the
  "augmentation is an on-read concern" decision from the datagen
  tutorial.
- **Train split only.** Holdout evaluation must be deterministic and
  comparable across runs. If the holdout set were augmented, two
  training runs would report metrics on different transformed views of
  the same positions. The holdout batcher therefore uses
  `augment = false`.

This is **invention I8**, presented as a choice.

### Deterministic transform selection

The batcher uses an `AtomicU64` counter that advances by the batch size
on every call. For the *i*-th item in a batch, the transform index is
`(seed + counter + i) % Transform::ALL.len()`. Using a per-batcher
counter (rather than a global one) keeps unit tests independent: two
fresh `NetBatcher::new(7, true)` instances produce identical batches,
which is exactly what the determinism test checks.

## Low-level design

### Files

```text
gomoku/crates/net/
└── src/
    ├── lib.rs      # add `pub mod batcher;`
    └── batcher.rs  # new module
```

### `src/lib.rs`

Add `pub mod batcher;` after the existing module declarations:

```rust
pub mod batcher;
pub mod checkpoint;
pub mod loss;
pub mod network;
pub mod planes;
pub mod train;
```

### `src/batcher.rs`

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
use burn::tensor::Tensor;
use burn::tensor::Device;
use engine::{Board, EXT, Move, Transform};
use std::sync::atomic::{AtomicU64, Ordering};
use train::Sample;

use crate::planes::{self, InputPlanes};

/// A batch of network inputs and targets.
#[derive(Debug, Clone)]
pub struct NetBatch {
    /// Input planes of shape `[B, 4, EXT, EXT]`.
    pub input: Tensor<4>,
    /// Dense policy target of shape `[B, 225]`.
    pub policy_target: Tensor<2>,
    /// Value target of shape `[B, 1]`.
    pub value_target: Tensor<2>,
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
    counter: AtomicU64,
}

impl NetBatcher {
    /// Create a new batcher.
    pub fn new(seed: u64, augment: bool) -> Self;

    /// Create a holdout batcher: canonical orientation, no augmentation.
    pub fn holdout() -> Self;

    /// Reset the transform counter. Useful for reproducibility tests.
    pub fn reset_counter(&self);
}

impl Batcher<Sample, NetBatch> for NetBatcher {
    fn batch(&self, items: Vec<Sample>, device: &Device) -> NetBatch;
}
```

Implementation notes:

- `batch` increments the counter by `items.len()` once, then loops over
  items. For each item it selects a transform (if augmenting),
  transforms board and policy, assembles `InputPlanes`, and collects
  planes, dense policy vectors, and value scalars.
- After the loop it stacks planes into a `Tensor<4>` of shape
  `[batch, 4, EXT, EXT]`, builds the policy target from flattened
  225-vectors, and builds the value target from collected scalars.
- `transform_sample` rebuilds the board by replaying transformed moves
  into a fresh `Board::new()`. This guarantees the transformed position
  is legal and that the last-move planes (derived from history in
  chapter 1) stay consistent.
- `scatter_policy` writes each mass at `row * 15 + col`. The sum of the
  dense vector equals the sum of the input masses, which should be 1.0
  for a valid sample.

### `stack_planes`

Helper inside `batcher.rs`:

```rust
fn stack_planes(planes: &[InputPlanes], device: &Device) -> Tensor<4>;
```

It builds a `Vec<f32>` in NCHW order and calls
`Tensor::<4>::from_data(TensorData::new(data, [batch, 4, EXT, EXT]).convert::<f32>(), device)`.

### Tests

Inside `#[cfg(test)] mod tests` in `batcher.rs`:

1. `batch_shapes_are_correct` — single sample produces the expected
   input/policy/value shapes.
2. `policy_mass_is_preserved` — the dense policy target sums to 1.0.
3. `value_target_matches_input` — the value tensor equals the sample's
   value.
4. `augmentation_preserves_policy_mass` — a transformed sample still has
   policy mass summing to 1.0.
5. `augmentation_keeps_stones_on_board` — after augmentation the input
   still contains the original stone.
6. `holdout_is_canonical` — the holdout batcher produces a canonical
   batch; at minimum policy mass is preserved.
7. `same_seed_same_transforms` — two batchers with the same seed produce
   identical input and policy tensors.

Use `Device::flex()` (or the device the `patterns` crate used) as
the concrete test backend.

## Solution (opt-in)

The complete reference code for this chapter — `lib.rs` registration and
`batcher.rs` — lives in
[02-the-batcher-and-d4-augmentation/01-solution.md](02-the-batcher-and-d4-augmentation/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare.

## TDD checklist

1. **Red:** Add `pub mod batcher;` to `lib.rs`, create `batcher.rs`
   with the struct, `Batcher` implementation, helper functions, and the
   seven tests, leaving function bodies as `todo!()`. Run `cargo test -p
   net`. Expect failures.
2. **Green:** Implement `NetBatcher::batch`, `scatter_policy`,
   `transform_sample`, and `stack_planes`. Re-run `cargo test -p net`.
   All tests should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p net` passes from `gomoku/`.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Commit message in the reference worktree: `feat(net): batcher with D4 augmentation`.

Next: Chapter 03 — The network module.

## References

* [`docs/specs/2026-10-04-net-tutorial-design.md`](../../specs/2026-10-04-net-tutorial-design.md) —
  binding spec: inventions I2, I3, I8.
* [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  network input shape, backend story, D4 at batch time.
* [`docs/09-toward-alphazero.md`](../../09-toward-alphazero.md) —
  the batcher as the data bridge between game logic and network.
* [`docs/tutorials/datagen-tutorial/04-the-quota-collector.md`](../datagen-tutorial/04-the-quota-collector.md) —
  samples carry `(move, mass)` policy targets.
* [`gomoku/crates/engine/src/symmetry.rs`](../../../gomoku/crates/engine/src/symmetry.rs) —
  `Transform` API.
* [`gomoku/crates/train/src/sample.rs`](../../../gomoku/crates/train/src/sample.rs) —
  `Sample::board()` and the policy/value fields.
