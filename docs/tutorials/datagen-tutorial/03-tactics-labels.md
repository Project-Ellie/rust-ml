# Chapter 03 — Tactics labeling

## Abstract

This chapter turns a raw board position into a supervised training
example. You will add a `label` module to the `train` crate that
classifies every position as **win**, **block**, or **quiet** using the
engine's tactics functions, then shapes a sparse policy target and a
side-to-move value target. By the end you will have three ASCII-puzzle
unit tests and a clean clippy/fmt gate. The lesson also fixes a common
bug: copying the MCTS mock evaluator's scaffolding values (`+0.95` /
`−0.90`) into training labels.

## Glossary

| Term | Definition |
|------|------------|
| **Tactical class** | One of `Win`, `Block`, or `Quiet`, determined by `immediate_wins` and `forced_blocks`. |
| **Win class** | The side to move has at least one immediate win. |
| **Block class** | The side to move has no immediate win, but the opponent does, so forced blocks exist. |
| **Quiet class** | Neither side has an immediate win on this move. |
| **Policy target** | A probability distribution over legal moves, stored sparsely as `Vec<(Move, f32)>`. |
| **Value target** | A scalar from the side-to-move perspective: `+1.0` for win, `−1.0` for block, `0.0` for quiet. |
| **90/10 shaping** | Win/block positions receive 90% of policy mass split over tactical moves and 10% split over the rest. |
| **Signal ladder** | The ordering of training signals by noise, from solver-verified (cleanest) to self-play bootstrap (noisiest); the synthetic tier is "rules-true by construction, distribution by design" (ch. 14 §5.2). |
| **Scaffolding value** | A deliberately inexact constant used only to make search tests pass, not as a training target. |
| **Class imbalance** | The natural tendency of random-play positions to be quiet; addressed by quotas in chapter 4. |

## Context

Chapter 1 defined the [`Sample`](01-the-train-crate.md) record and
chapter 2 added the [`random_game` / `sample_plies`](02-random-playouts.md)
playout driver. Those pieces produce raw board positions; this chapter
labels them. The label is the bridge between the engine's certain
tactical knowledge and the network's supervised loss.

The datagen design in
[`docs/specs/2026-09-27-datagen-tutorial-design.md`](../../specs/2026-09-27-datagen-tutorial-design.md)
calls for three tactical classes and a shaped policy target. The
implementation is intentionally small — roughly forty lines over
[`engine::immediate_wins`](../../../gomoku/crates/engine/src/tactics.rs),
[`engine::forced_blocks`](../../../gomoku/crates/engine/src/tactics.rs),
and [`Board::empty_moves`](../../../gomoku/crates/engine/src/board.rs).

## Intention

1. Create `gomoku/crates/train/src/label.rs` and register
   `pub mod label;` in `gomoku/crates/train/src/lib.rs`.
2. Add `engine = { path = "../engine", features = ["testutil"] }` to
   the `[dev-dependencies]` of `crates/train/Cargo.toml` so tests can
   use [`engine::reference::board_from_ascii`](../../../gomoku/crates/engine/src/reference.rs).
3. Define `TacticalClass { Win, Block, Quiet }` and implement
   `classify(board)`.
4. Implement `label(board)` returning `(Vec<(Move, f32)>, f32)` with
   the 90/10 policy split for win/block and a uniform target for quiet.
5. Value targets must be exact `+1.0`, `−1.0`, or `0.0` — not the MCTS
   mock's scaffolding values.
6. Write three ASCII-puzzle tests: open four for the side to move
   (win), closed four for the opponent (block), and a scattered quiet
   position.

Observable done-state: `cargo test -p train` passes,
`cargo clippy --all-targets -- -D warnings` is green, and
`cargo fmt --all -- --check` makes no changes.

## Mental mapping

### The signal ladder: why synthetic labels are trustworthy

Chapter 14 §5.2 orders training signals by noise. The synthetic
tactical tier sits just below solver-verified labels:

> **Synthetic tactical data** (milestone 3's generator): rules-true by
> construction, distribution by design.

"Rules-true by construction" means the label comes from the engine's
bitboard tactics, not from a model's guess. If `immediate_wins` reports
that playing `(7, 3)` completes five, then playing `(7, 3)` really does
win under the project's freestyle rules (overlines count, draw at 225).
There is no human labeler, no engine style, and no approximation.

"Distribution by design" means the generator does not have to accept
the distribution that random playouts naturally produce. Random play
produces far more quiet positions than wins or blocks, so chapter 4
adds class quotas. The labels themselves are exact; only the mix is
tuned.

### Why training values are ±1.0 / 0.0, not the mock's 0.95 / −0.90

The MCTS tutorial's
[`TacticsEvaluator`](../mcts-tutorial/09-the-tactics-evaluator.md)
uses `+0.95` for a win, `−0.90` for a block, and `0.00` for quiet.
Those numbers are **search scaffolding**: they make the sign of the
value unambiguous in the tree and ensure PUCT's exploration term does
not drown the prior. They are deliberately not exact because the tests
only ask whether the search *finds* the tactical move, not whether it
estimates the position to three decimal places.

Training labels are different. The value head of a network is trained
to minimize a squared-error loss against the target. If the target is
`0.95`, the loss can never reach zero for a true win, and the network
has no reason to believe the position is fully won. A target of `+1.0`
for a winning position, `−1.0` for a blocking position, and `0.0` for a
quiet position is the cleanest target the loss can actually reach.
Copying the mock's scaffolding values into the training pipeline is a
real bug; this chapter prevents it by fixing the contract.

### Class imbalance: why the 90/10 split and the quotas both matter

A uniformly random playout spends most of its plies in quiet positions.
If you collected every sampled position without filtering, the dataset
would be overwhelmingly quiet. The network would learn that Gomoku is a
game of scattered stones and would under-fit the tactical moments where
games are actually decided.

The 90/10 policy split in win/block positions is one counter-weight:
even when a position is tactical, the network still sees the other
legal moves, but it is told loudly which moves matter. Chapter 4 adds
the second counter-weight: per-class quotas that stop collecting quiet
positions once a cap is reached and keep collecting wins and blocks
until their quotas are full. The combination is what makes the
distribution "by design."

> **Excursion — the 90/10 split is `[experiment]` scaffolding.**
>
> The 90% tactical / 10% background split is not derived from AlphaZero
> mathematics; it is a practical default that keeps the tactical moves
> visible while preventing the policy from collapsing to zero entropy.
> Chapter 8 may tune it. The important rule now is that the split is
> explicit, tested, and easy to change in one place.

## Low-level design

### Files

```text
gomoku/crates/train/
├── Cargo.toml          # add engine testutil dev-dependency
└── src/
    ├── lib.rs          # add `pub mod label;`
    └── label.rs        # new module
```

### `Cargo.toml`

Add a `[dev-dependencies]` section:

```toml
[dev-dependencies]
engine = { path = "../engine", features = ["testutil"] }
```

This mirrors the `mcts` crate's dev-dependency setup and exposes the
ASCII puzzle parser only for tests.

### `src/lib.rs`

Add `pub mod label;` next to the existing module declarations.

### `src/label.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TacticalClass { Win, Block, Quiet }

/// Win: side to move has an immediate win. Else Block: opponent has
/// one (forced blocks exist). Else Quiet.
pub fn classify(board: &engine::Board) -> TacticalClass;

/// The shaped training target: 90% mass over the tactical moves
/// (win/block), 10% over the rest; uniform when quiet. Value is
/// +1.0 / -1.0 / 0.0 by class.
pub fn label(board: &engine::Board) -> (Vec<(engine::Move, f32)>, f32);
```

Implementation notes:

* `classify` checks `engine::immediate_wins(board, board.to_move())`
  first, then `engine::forced_blocks(board)`. The order matters: a
  position where both sides have an immediate win is classified as
  `Win` because the side to move gets to play first.
* `label` collects all legal moves with `board.empty_moves()`, then
  delegates to a private `shape` helper for win/block. Quiet positions
  get a uniform target.
* The helper assigns `0.9 / k` to each tactical move and `0.1 / rest`
  to each non-tactical move. Because `f32` cannot represent every
  fraction exactly, the last entry is adjusted so the distribution sums
  to `1.0`; tests check the sum to `±1e-6` and each mass to `±1e-5`.
* Value targets are exact literals: `1.0`, `-1.0`, `0.0`.

### Tests

Inside `#[cfg(test)] mod tests` in `label.rs`:

1. `open_four_for_side_to_move_is_win` — build a board where Black
   (the side to move) has an open four; assert `classify` is `Win`,
   value is `1.0`, policy argmax is in `immediate_wins`, masses sum to
   `1.0 ± 1e-6`, tactical moves carry `0.9 / k`, and the rest carries
   `0.1 / rest`.
2. `closed_four_for_opponent_is_block` — build a board where White is
   to move and Black has a closed four; assert `classify` is `Block`,
   value is `-1.0`, policy argmax is in `forced_blocks`, and the same
   mass checks hold.
3. `scattered_position_is_quiet_and_uniform` — build a quiet board;
   assert `classify` is `Quiet`, value is `0.0`, and every legal move
   carries `1.0 / legal_count`.

All three puzzles use `engine::reference::board_from_ascii`. Remember
that the parser enforces alternating-reachable stone counts: equal
Black/White counts mean Black to move; one more Black stone than White
means White to move.

## Solution (opt-in)

The complete reference code for this chapter — `label.rs`, the
`lib.rs` registration line, and the `Cargo.toml` dev-dependency — lives
in [03-tactics-labels/01-solution.md](03-tactics-labels/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare.

## TDD checklist

1. **Red:** Add the `testutil` dev-dependency, create `label.rs` with
   the `TacticalClass` enum, the two public function signatures, and
   the three tests, leaving the function bodies as `todo!()`. Register
   `pub mod label;` in `lib.rs`. Run `cargo test -p train` and expect
   failures from the `todo!()` panics.
2. **Green:** Implement `classify` and `label`. Re-run
   `cargo test -p train`. All ten tests (three from this chapter plus
   seven from chapters 1–2) should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p train` passes from `gomoku/`.
* `cargo test -p engine --features testutil` still passes (no engine
  changes this slice).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Commit message in the reference worktree: `feat(train): tactics labeling`.

Next: Chapter 04 — Quota collector.

## References

* [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  workspace crate layout and the dependency-island rule that keeps
  `rand` and file I/O out of `engine`.
* [`docs/13-engine-design.md`](../../13-engine-design.md) — the engine
  API contract, including `Board::empty_moves`,
  `engine::immediate_wins`, and `engine::forced_blocks`.
* [`docs/14-openings-and-supervised-curriculum.md`](../../14-openings-and-supervised-curriculum.md) —
  §5.2 the signal ladder and §5.3 phase 0, which explain where the
  synthetic dataset sits in the training curriculum.
* [`docs/specs/2026-09-27-datagen-tutorial-design.md`](../../specs/2026-09-27-datagen-tutorial-design.md)
  — the datagen design: classes, 90/10 shaping, value targets, and
  quotas.
* [`docs/tutorials/datagen-tutorial/01-the-train-crate.md`](01-the-train-crate.md)
  — the previous chapter: `Sample`, serde derives, and bincode.
* [`docs/tutorials/datagen-tutorial/02-random-playouts.md`](02-random-playouts.md)
  — the previous chapter: the playout driver and random ply sampling.
* [`docs/tutorials/mcts-tutorial/09-the-tactics-evaluator.md`](../mcts-tutorial/09-the-tactics-evaluator.md)
  — the mock evaluator whose `+0.95` / `−0.90` values are search
  scaffolding, not training targets.
* [`gomoku/crates/engine/src/tactics.rs`](../../../gomoku/crates/engine/src/tactics.rs)
  — exact engine tactics API.
* [`gomoku/crates/engine/src/moveset.rs`](../../../gomoku/crates/engine/src/moveset.rs)
  — `Move`, `MoveSet`, and `.iter()` / `.contains()`.
* [`gomoku/crates/engine/src/reference.rs`](../../../gomoku/crates/engine/src/reference.rs)
  — the ASCII puzzle parser used in tests.
