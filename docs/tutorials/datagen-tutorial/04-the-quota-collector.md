# Chapter 04 — The quota collector

## Abstract

This chapter adds the quota collector to the `train` crate: a single
function that runs seeded random games, samples a small number of
plies from each game, classifies the resulting positions, and keeps
samples until per-class quotas are met. Positions are deduplicated by
their Zobrist key, so a given board state enters the dataset at most
once. By the end you will have a deterministic collector with four
unit tests: exact per-class counts for tiny quotas, uniqueness of
Zobrist keys, bit-identical reproduction under the same seed, and a
seeded regression test proving terminal plies are never sampled.

## Glossary

| Term | Definition |
|------|------------|
| **Quota** | A per-class collection target: how many unique win, block, and quiet positions the collector must produce. |
| **Class bucket** | The subset of collected samples that share the same [`TacticalClass`](03-tactics-labels.md) (win, block, or quiet). |
| **Zobrist key** | A 64-bit hash maintained incrementally by [`engine::Board`](../../../gomoku/crates/engine/src/board.rs); two identical positions have identical keys, and distinct positions are extremely unlikely to collide. |
| **Rejection sampling** | Drawing a candidate (here, a sampled ply), evaluating it, and discarding it if it fails a criterion (duplicate key or full bucket). The collector uses rejection sampling to enforce quotas and deduplication. |
| **Distribution by design** | The synthetic dataset is not bound to the natural distribution that random play produces; quotas deliberately shape the class mix (ch. 14 §5.2). |
| **Symmetric duplicate** | The same board state reached through a different move order or after a D4 transform; the collector only rejects identical Zobrist keys, not transformed equivalents. |
| **Determinism** | Running the collector twice with the same seeded random-number generator produces exactly the same sequence of samples. |

## Context

Chapter 1 added the [`Sample`](01-the-train-crate.md) record, chapter
2 added the [`random_game` / `sample_plies`](02-random-playouts.md)
playout driver, and chapter 3 added the
[`classify` / `label`](03-tactics-labels.md) tactics labeller. Those
three pieces are now composed into a complete collector: random games
produce positions, the labeller turns each position into a supervised
example, and a small bookkeeping loop enforces per-class quotas while
deduplicating by Zobrist key.

The collector is the last purely in-memory piece of the datagen
tutorial. Chapter 5 will stream its output to bincode shards and write
a manifest; chapter 8 will size the quotas so the generated dataset is
large enough to pre-train the network.

## Intention

1. Create `gomoku/crates/train/src/collect.rs` and register
   `pub mod collect;` in `gomoku/crates/train/src/lib.rs`.
2. Define `Quotas { win: usize, block: usize, quiet: usize }`.
3. Implement `collect(quotas: Quotas, rng: &mut impl Rng) -> Vec<Sample>`:
   run random games, sample up to [`MAX_PLIES_PER_GAME`](02-random-playouts.md)
   plies per game, classify each position, and keep it only if its
   class bucket is not yet full and its Zobrist key has not been seen.
4. Make the function deterministic under a seeded RNG: no hidden
   state, no thread-local randomness, no I/O.
5. Write four tests: tiny quotas produce exact win/block/quiet counts;
   all collected samples have unique Zobrist keys; two runs with the
   same seed produce identical sample vectors; and a deterministic
   regression test that fails if terminal plies are collected.

Observable done-state: `cargo test -p train` passes, `cargo clippy
--all-targets -- -D warnings` is green, and `cargo fmt --all --
--check` makes no changes.

## Mental mapping

### Putting the pipeline together

The collector is the first place where the crate's modules stop being
isolated utilities and start cooperating:

* `playout::random_game` supplies a complete, legal game history.
* `playout::sample_plies` picks which plies in that history are worth
  turning into samples.
* `label::classify` decides which bucket the position belongs to.
* `label::label` produces the sparse policy target and the value target.
* `sample::Sample::from_position` stores the position in engine
  vocabulary.

The loop continues until all three buckets are full. Because
`MAX_PLIES_PER_GAME` caps how many plies a single game can contribute,
no one unusually tactical game can dominate a bucket.

> **Excursion — Zobrist keys paying off a second time**
>
> In engine slice 5 the board learned to maintain an incremental 64-bit
> Zobrist key in O(1) per play/undo. The immediate motivation was
> replay-buffer deduplication and test identity. Here that investment
> pays off again: rejecting duplicate positions is a single
> `board.zobrist()` call and a `HashSet` insertion. Without it, dedup
> would require either comparing 225-cell board states or replaying
> every stored position from its stone lists. The key is not perfect
> (collisions are theoretically possible), but the 64-bit space makes
> them negligible for a local dataset, and the engine already tests
> that the incremental key matches a from-scratch recomputation.

> **Excursion — rejection sampling and "distribution by design"**
>
> Chapter 14 §5.2 calls the synthetic tactical tier "rules-true by
> construction, distribution by design." The rules-true part comes
> from `label::classify`, which uses the engine's bitboard tactics. The
> distribution-by-design part comes from the collector's rejection
> logic.
>
> Random play naturally produces far more quiet positions than wins or
> blocks. If the collector accepted every sampled ply, the quiet
> bucket would fill first and the remaining classes would be
> under-represented. Quotas flip that around: once the quiet bucket is
> full, quiet positions are rejected even though the loop keeps
> running, and the RNG keeps generating games until win and block
> quotas are satisfied. The distribution is therefore not the raw
> output of random play; it is the output of random play *filtered by
> the quotas*. That filtering is deliberate, tested, and tunable.

> **Honest note — symmetric duplicates are not rejected**
>
> The collector deduplicates by Zobrist key, and the key is computed
> from absolute stone coordinates. A D4 rotation or reflection of the
> same position has a different key, so it is *not* rejected here.
> Removing symmetric duplicates is the job of the training sampler in
> the net tutorial, which applies one random D4 transform on read. The
> datagen tutorial stores canonical positions only; augmentation is an
> on-read concern.

## Low-level design

### Files

```text
gomoku/crates/train/
└── src/
    ├── lib.rs          # add `pub mod collect;`
    └── collect.rs      # new module
```

### `src/lib.rs`

Add `pub mod collect;` next to the existing module declarations.

### `src/collect.rs`

```rust
#[derive(Debug, Clone, Copy)]
pub struct Quotas {
    pub win: usize,
    pub block: usize,
    pub quiet: usize,
}

/// Run random games until every class bucket is full. Positions are
/// deduplicated by Zobrist key; each game yields at most
/// [`MAX_PLIES_PER_GAME`] samples. Deterministic under `rng` seed.
pub fn collect(quotas: Quotas, rng: &mut impl rand::Rng) -> Vec<crate::sample::Sample>;
```

> **Forward note — extra `Quotas` derives come in chapter 5.**
>
> The final `Quotas` struct also derives `PartialEq, serde::Serialize,
> serde::Deserialize`; chapter 5 adds those so the manifest can store
> the quotas that produced each dataset. They are not needed for the
> collector itself.

Implementation notes:

* Keep three counters (win, block, quiet) and a `HashSet<u64>` of seen
  Zobrist keys.
* Loop while any counter is below its quota.
* Inside the loop:
  1. `let history = random_game(rng);`
  2. `let plies = sample_plies(history.len(), MAX_PLIES_PER_GAME, rng);`
  3. For each `ply` in `plies`, rebuild the board at that prefix. The
     cleanest way is to replay `history[..ply]` through a fresh
     `Board::new()`, because the collector owns the full history.
  4. Skip the ply if `board.status() != engine::Status::Ongoing`.
     Terminal positions are never evaluated by the network — MCTS
     returns exact values for finished positions without calling the
     network — so they must never enter the dataset.
  5. `let class = classify(&board);`
  6. If the corresponding bucket is already full, `continue`.
  7. `let key = board.zobrist();` — if `!seen.insert(key)`, `continue`.
  8. `let (policy, value) = label(&board);`
  9. `samples.push(Sample::from_position(&history, ply, policy, value));`
  10. Increment the bucket counter.
* Determinism holds because the only source of variation is the RNG,
  and every branch (skip because bucket full, skip because duplicate)
  is a deterministic function of the board and the current counters.
* The quiet bucket usually fills first; the loop then keeps running
  until win and block quotas are met. For the tiny test quotas
  `{win: 2, block: 2, quiet: 3}` the collector typically finishes in
  a few dozen games, while larger chapter-8 quotas run into the
  hundreds or thousands.

> **Excursion — why terminal plies must be skipped**
>
> The engine's tactics functions are only meaningful on ongoing
> positions. On a board that is already won, `forced_blocks`
> degenerates: the winner's five survives any legal placement, so
> every empty cell appears to "block" an unstoppable threat. The
> position would be mislabeled as a Block with a meaningless uniform
> policy target. Worse, the network is never asked to evaluate
> terminals — MCTS handles them exactly — so there is no valid
> supervision signal to learn from. Skipping them keeps the dataset
> rules-true.

### Tests

Inside `#[cfg(test)] mod tests` in `collect.rs`:

1. `tiny_quotas_produce_exact_class_counts` — build
   `Quotas { win: 2, block: 2, quiet: 3 }`, seed `StdRng` with `42`,
   call `collect`, then rebuild every sample's board and classify it
   with `crate::label::classify`. Assert the returned vector contains
   exactly 2 win, 2 block, and 3 quiet samples, and that its length is
   the sum of the quotas.
2. `collected_samples_have_unique_zobrist_keys` — collect the same
   tiny quotas, rebuild every board, and collect the Zobrist keys into
   a `HashSet<u64>`. Assert `keys.len() == samples.len()`.
3. `same_seed_gives_identical_samples` — run `collect` twice from
   `StdRng::seed_from_u64(42)` and assert the two `Vec<Sample>` are
   equal. `Sample` implements `PartialEq`, so the comparison includes
   the sparse policy vectors and value targets.
4. `terminal_ply_is_never_sampled` — collect the tiny quotas with
   `StdRng::seed_from_u64(0)` and assert that every rebuilt board has
   `engine::Status::Ongoing`. Seed 0 was verified to sample a
   `Won(Black)` terminal position when the Ongoing skip is removed,
   so this test deterministically guards the skip.

## Solution (opt-in)

The complete reference code for this chapter — `collect.rs` and the
`lib.rs` registration line — lives in
[04-the-quota-collector/01-solution.md](04-the-quota-collector/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare.

## TDD checklist

1. **Red:** Create `collect.rs` with the `Quotas` struct, the
   `collect` function signature, and the four tests, leaving the
   function body as `todo!()`. Register `pub mod collect;` in
   `lib.rs`. Run `cargo test -p train` and expect failures from the
   `todo!()` panics.
2. **Green:** Implement `collect`. Re-run `cargo test -p train`. All
   sixteen tests (four from this chapter plus twelve from chapters
   1–3) should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p train` passes from `gomoku/`.
* `cargo test -p engine --features testutil` still passes (no engine
  changes this slice).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Commit message in the reference worktree: `feat(train): quota collector with Zobrist dedup`.

Next: Chapter 05 — Streaming shard writer + manifest.

## References

* [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  workspace crate layout, dependency-island rule, and store-games
  replay-buffer decision.
* [`docs/13-engine-design.md`](../../13-engine-design.md) — engine
  design, including `Board::zobrist` and `Board::from_position`.
* [`docs/14-openings-and-supervised-curriculum.md`](../../14-openings-and-supervised-curriculum.md) —
  §5.2 the signal ladder and the phrase "rules-true by construction,
  distribution by design."
* [`docs/specs/2026-09-27-datagen-tutorial-design.md`](../../specs/2026-09-27-datagen-tutorial-design.md) —
  the datagen design: quotas, dedup, and determinism.
* [`docs/tutorials/datagen-tutorial/01-the-train-crate.md`](01-the-train-crate.md) —
  the `Sample` record.
* [`docs/tutorials/datagen-tutorial/02-random-playouts.md`](02-random-playouts.md) —
  the playout driver and `MAX_PLIES_PER_GAME`.
* [`docs/tutorials/datagen-tutorial/03-tactics-labels.md`](03-tactics-labels.md) —
  `classify`, `label`, and `TacticalClass`.
* [`gomoku/crates/engine/src/zobrist.rs`](../../../gomoku/crates/engine/src/zobrist.rs) —
  the incremental Zobrist implementation.
