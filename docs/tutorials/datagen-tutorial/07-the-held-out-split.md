# Chapter 07 — Held-out split + stats report

## Abstract

This chapter adds the final piece of the datagen pipeline before the
acceptance run: a deterministic train/held-out partition and a summary
statistics report. The split is computed from each sample's Zobrist key,
not from its position in a file or shard, so the quota collector's
ordering cannot leak correlated positions into both sides. The stats
report gives per-class counts, a ply histogram, and the train/held-out
sizes. By the end you will have `train/src/split.rs`, its registration
in `lib.rs`, and tests that prove determinism, partition completeness,
and a ~10% held-out fraction.

## Glossary

| Term | Definition |
|------|------------|
| **Held-out set** | The subset of the dataset reserved for the network tutorial's top-1 accuracy benchmark; never used for gradient updates. |
| **Training set** | The complement of the held-out set; the samples the network is trained on. |
| **Zobrist key** | A 64-bit incremental hash maintained by [`engine::Board`](../../../gomoku/crates/engine/src/board.rs); identical positions have identical keys and distinct positions are extremely unlikely to collide. |
| **Key-hash split** | A partition based on a deterministic function of the board content (here `zobrist % 10`), not on the sample's index in a file. |
| **Data leakage** | Any situation where information from the training set influences the held-out evaluation, producing an over-optimistic accuracy estimate. |
| **Ply histogram** | A vector whose index `i` counts how many samples have exactly `i` stones on the board. |
| **Per-class quota** | One of the win / block / quiet targets passed to [`collect`](04-the-quota-collector.md). |

## Context

Chapters 1–6 built the full datagen pipeline: the [`Sample`](01-the-train-crate.md)
record, seeded random playouts, tactics labels, quota collection,
streaming shard writing, and the dataset reader with a soundness gate.
This chapter adds the split that separates the milestone-3 synthetic
dataset into a training set and a held-out evaluation set, plus a
statistics report that the acceptance chapter and the network tutorial
will use.

The split rule is locked in
[`docs/specs/2026-09-27-datagen-tutorial-design.md`](../../specs/2026-09-27-datagen-tutorial-design.md):
the partition must be deterministic and must not depend on position in
file, because quota ordering could otherwise bias which classes land on
which side. The network tutorial will later read the held-out set when
it reports top-1 accuracy on synthetic threats.

## Intention

1. Create `gomoku/crates/train/src/split.rs` and register
   `pub mod split;` in `gomoku/crates/train/src/lib.rs`.
2. Implement `is_holdout(sample: &Sample) -> bool`:
   * Rebuild the board via [`Sample::board`](01-the-train-crate.md).
   * Return `true` iff `board.zobrist() % 10 == 0`.
   * If the board cannot be rebuilt, treat the sample as held-out.
3. Define `Stats`:
   ```rust
   pub struct Stats {
       pub total: usize,
       pub per_class: [(&'static str, usize); 3],
       pub ply_histogram: Vec<usize>,
       pub holdout: usize,
       pub train: usize,
   }
   ```
4. Implement `stats(samples: &[Sample]) -> Stats`:
   * Rebuild each board to classify it as win / block / quiet via
     [`crate::label::classify`](03-tactics-labels.md).
   * Count held-out vs. training samples using `is_holdout`.
   * Build `ply_histogram` such that index `i` holds the number of
     samples with exactly `i` stones on the board.
5. Write tests for: determinism and partition completeness; a ~10%
   held-out fraction (within 5–15%) over a seeded dataset of a few
   hundred samples; stats counts that match the per-class quotas;
   ply histogram semantics; and the unbuildable-sample policy.

Observable done-state: `cargo test -p train` passes, `cargo clippy
--all-targets -- -D warnings` is green, and `cargo fmt --all --
--check` makes no changes.

## Mental mapping

### Why split by Zobrist key, not by row?

The simplest split is "every 10th sample in the file is held-out." That
is easy to implement and easy to get wrong, because the order of samples
in a shard is driven by the quota collector: it fills the win bucket,
then the block bucket, then the quiet bucket, and within each bucket it
follows the random game stream. A row-based split would therefore place
whole runs of the same class on one side and runs of another class on
the other. Worse, consecutive samples often come from the same game, and
positions from the same game are correlated — they share the same
opening, the same tactical context, and sometimes the same forcing
sequence. Putting some of those positions in training and some in
held-out leaks information across the boundary and makes the held-out
accuracy look better than it really is.

Splitting by `zobrist % 10` removes position-in-file from the decision
entirely. The verdict depends only on the board state, so:

* Quota ordering cannot skew the class balance between train and
  held-out.
* The exact same position always goes to the same side, so accidental
  duplicates do not appear on both sides.
* Positions from the same game are assigned independently according to
  their content, not according to the order in which the collector
  happened to write them.

This is not a cryptographic guarantee — two different positions can in
principle share a key — but the engine's 64-bit Zobrist keys make such
collisions astronomically unlikely, and the split remains deterministic
and reproducible under a fixed seed.

> **Excursion — data leakage in positional splits**
>
> A positional split (first 90% of rows train, last 10% held-out) is the
> most common form of data leakage in sequential datasets. In a Gomoku
> dataset, positions from the same game are not independent: they share
> the same stone configuration history, the same player styles (because
> the random playout generated them), and often the same local tactical
> motifs. If the network sees positions 1–20 of a game during training
> and position 21 during evaluation, it has already learned the
> surrounding context from the training positions. The held-out accuracy
> then measures memorization of nearby positions more than generalization.
>
> A content-based split does not remove all correlation — two positions
> from the same game can still end up on different sides by chance — but
> it removes the systematic bias introduced by the collection order. The
> stronger remedy, used in later milestones, is to split by *game* rather
> than by position; for the synthetic tactics dataset we split by
> position but make the split depend only on the position itself, which
> is the honest compromise for this milestone.

### What the stats report is for

The stats report is the dataset's health check. It answers three
questions before the network tutorial consumes the data:

* **Class balance:** do the per-class counts match the quotas that were
  requested? A mismatch means the collector or labeler is drifting.
* **Position length distribution:** the `ply_histogram` shows whether the
  dataset is front-loaded with empty openings or back-loaded with late
  endgames. Either extreme changes what the network is trained to
  recognize.
* **Held-out size:** is the held-out fraction close to the designed 10%?
  A much smaller held-out set makes the benchmark noisy; a much larger
  one wastes training data.

The network tutorial's acceptance benchmark — >90% top-1 accuracy on
held-out synthetic threats — reads the held-out set produced by this
split. If the split is biased, the benchmark is biased.

### Why treat unbuildable samples as held-out?

A sample whose stone lists cannot be rebuilt into a legal board should
not exist in a dataset that has passed the soundness gate from
[`dataset.rs`](06-reading-back.md). If `is_holdout` encounters one
anyway, the conservative choice is to send it to the held-out set. The
training set is the product we care about; the held-out set is where we
park samples we do not want to learn from. Panicking is also defensible,
but it would make the split fragile when applied to imperfect data. The
stats function, by contrast, panics on an unbuildable sample because a
stats report over corrupted data is meaningless.

## Low-level design

### Files

```text
gomoku/crates/train/
└── src/
    ├── lib.rs          # add `pub mod split;`
    └── split.rs        # new module
```

### `src/lib.rs`

Add `pub mod split;` next to the existing module declarations and update
the crate-level doc comment to note that the train/held-out partition
lives in `split`.

### `src/split.rs`

```rust
/// Deterministic partition: sample belongs to held-out iff
/// `zobrist(board) % 10 == 0` (~10%). Position-in-file never decides,
/// so quota ordering cannot skew the split.
pub fn is_holdout(sample: &crate::sample::Sample) -> bool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stats {
    pub total: usize,
    pub per_class: [(&'static str, usize); 3],
    pub ply_histogram: Vec<usize>,
    pub holdout: usize,
    pub train: usize,
}

pub fn stats(samples: &[crate::sample::Sample]) -> Stats;
```

Implementation notes:

* `is_holdout` calls `sample.board()` and returns `true` for held-out if
  the board's Zobrist key modulo 10 is 0. On a `PositionError`, return
  `true` so the invalid sample is excluded from training.
* `stats` iterates over `samples`, rebuilds each board, and classifies it
  with `crate::label::classify`. It accumulates counts for win / block /
  quiet and increments `holdout` when the rebuilt board's Zobrist key
  modulo 10 is 0 (reusing the already-built board rather than calling
  `is_holdout` again).
* `stats` panics if a sample cannot be rebuilt into a legal board.
* `ply_histogram` is built by resizing the vector as needed: for a sample
  with `black.len() + white.len()` stones, the local variable is called
  `stones`, the vector is grown to at least that length, and
  `histogram[stones]` is incremented.
* `train` is computed as `samples.len() - holdout`.

### Tests

Inside `#[cfg(test)] mod tests` in `split.rs`:

1. `split_is_deterministic_and_partition_complete` — collect the tiny
   dataset from chapter 4, call `is_holdout` twice on every sample, and
   assert the two calls agree. Then count train and held-out samples and
   assert their sum equals `samples.len()`.
2. `holdout_fraction_is_within_ten_percent_bounds` — collect a seeded
   dataset of a few hundred samples (e.g. `Quotas { win: 50, block: 50,
   quiet: 400 }`) and assert that the held-out fraction is between 5%
   and 15%.
3. `stats_counts_match_quotas` — collect a seeded dataset with known
   quotas and assert that `stats.per_class` matches the quotas and that
   `holdout + train == total`.
4. `ply_histogram_sums_to_total_and_indexes_by_stone_count` — compute
   stats for a dataset and assert that the histogram entries sum to the
   total and that every sample's stone count has a non-zero bucket.
5. `unbuildable_sample_is_treated_as_holdout` — corrupt a sample so that
   `Sample::board()` fails and assert `is_holdout` returns `true`.
6. `stats_panics_on_unbuildable_sample` — pass a corrupted sample to
   `stats` and assert it panics with a clear message. Both corruption
   tests share a small `corrupt_sample` helper that pushes one stone into
   the opposite-color list.

## Solution (opt-in)

The complete reference code for this chapter — `split.rs` and the
`lib.rs` registration line — lives in
[07-the-held-out-split/01-solution.md](07-the-held-out-split/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare.

## TDD checklist

1. **Red:** Create `split.rs` with the `Stats` struct, the `is_holdout`
   and `stats` signatures, and the six tests, leaving the function bodies
   as `todo!()`. Register `pub mod split;` in `lib.rs`. Run
   `cargo test -p train` and expect failures from the `todo!()` panics.
2. **Green:** Implement `is_holdout` and `stats`. Re-run
   `cargo test -p train`. All tests should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p train` passes from `gomoku/`.
* `cargo test -p engine --features testutil` still passes (no engine
  changes this slice).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Commit message in the reference worktree: `feat(train): held-out split + stats`.
* Commit message in the main repository: `docs(tutorials): datagen chapter 7`.

Next: Chapter 08 — Acceptance.

## References

* [`docs/specs/2026-09-27-datagen-tutorial-design.md`](../../specs/2026-09-27-datagen-tutorial-design.md)
  — synthetic data generator design, including the split-by-content
  requirement.
* [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  workspace crate layout and the store-games replay-buffer decision.
* [`docs/tutorials/datagen-tutorial/01-the-train-crate.md`](01-the-train-crate.md)
  — the `Sample` record and `Sample::board`.
* [`docs/tutorials/datagen-tutorial/03-tactics-labels.md`](03-tactics-labels.md)
  — `classify`, `label`, and `TacticalClass`.
* [`docs/tutorials/datagen-tutorial/04-the-quota-collector.md`](04-the-quota-collector.md)
  — `collect`, `Quotas`, and Zobrist dedup.
* [`docs/tutorials/datagen-tutorial/06-reading-back.md`](06-reading-back.md)
  — the soundness gate and why invalid samples should not reach the split.
* [`gomoku/crates/engine/src/zobrist.rs`](../../../gomoku/crates/engine/src/zobrist.rs)
  — the incremental Zobrist implementation used by `board.zobrist()`.
* [`gomoku/crates/engine/src/board.rs`](../../../gomoku/crates/engine/src/board.rs)
  — `Board::from_position` and `PositionError`.
