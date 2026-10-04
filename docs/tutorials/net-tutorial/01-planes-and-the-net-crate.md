# Chapter 01 — The `net` crate + planes

## Abstract

This chapter creates the `net` crate, wires it into the workspace, and
implements the first module: `planes`. By the end you will have four
17×17 input planes assembled from an engine `Board` and its move
history, laid out contiguously so the batcher can turn them into a Burn
tensor in chapter 2.

## Glossary

| Term | Definition |
|------|------------|
| **`InputPlanes`** | The four 17×17 `u8` planes produced by this module. |
| **`engine::encode`** | The engine function that emits the relative `me`/`you` planes (2 of the 4). |
| **`EXT`** | Engine constant: side length of the encoded board, `17`. |
| **`Board::moves()`** | The move history of a board, in play order. |
| **Last-move plane** | A one-hot 17×17 plane marking the most recent move of one side. |
| **I1** | Invention: plane assembly lives in `net`, not the engine. |

## Context

The datagen tutorial finished the `train` crate. A `Sample` stores a
position as stone lists plus a side to move and carries policy/value
labels. The network, however, needs a dense tensor input.

This chapter is the bridge. It creates the `net` crate, adds Burn as a
dependency, and builds the first input-representation module. The engine
already emits two planes (`me` and `you`); this module adds the two
history planes and packages everything into a tensor-friendly layout.

## Intention

1. Create `gomoku/crates/net/` and add it to the workspace members in
   `gomoku/Cargo.toml`.
2. Write `gomoku/crates/net/Cargo.toml` with Burn pinned to `=0.21.0`,
   depending on `engine`, `train`, `rand`, `serde`, `serde_json`, and
   `thiserror`. Match the feature pattern used by the `patterns` crate
   (`flex` default, `gpu` opt-in).
3. Create `gomoku/crates/net/src/lib.rs` with `#![deny(missing_docs)]`
   and declare `pub mod planes;`.
4. Implement `gomoku/crates/net/src/planes.rs`:
   * `InputPlanes` with four `[u8; EXT * EXT]` arrays.
   * `assemble(board: &Board) -> InputPlanes`.
   * `InputPlanes::to_array()` returning `[[[u8; EXT]; EXT]; 4]`.
5. Write tests: empty-board last moves, one-move and two-move history,
   side-to-move flip, array shape, and agreement with `engine::encode`.

Observable done-state: `cargo test -p net` passes, `cargo clippy
--all-targets -- -D warnings` is green, and `cargo fmt --all -- --check`
makes no changes.

## Mental mapping

### Why four planes?

The engine encodes a position as two binary planes: stones of the player
to move (`me`) and stones of the opponent plus the border ring (`you`).
AlphaGo-style networks typically also receive history planes showing the
most recent moves, because the immediate tactical situation often
depends on what just happened (a new threat, a forcing block, a swap of
initiative).

Adding history planes is a **choice**, not a locked decision. The engine
is a dependency island: it has no Burn dependency and no knowledge of
network inputs. Therefore the history planes must be built in `net` from
`Board::moves()`. This is **invention I1**.

> **Excursion — the dependency-island boundary**
>
> One reasonable alternative is to put all four planes in the engine and
> expose `engine::InputPlanes` directly. That would couple the engine to
> the network architecture and force every engine user (the CLI, the MCTS
> baseline, future self-play workers) to think about neural-network input
> shape. Keeping the engine small — rules, win detection, symmetry,
> encoding of *stones* only — lets those callers stay ignorant of Burn.
> The cost is a small amount of "whose responsibility is this?" work in
> `net`, which is exactly what this chapter does.

### Last-move semantics

`Board::moves()` returns moves in play order: Black, White, Black,
White, ... The player to move is the one who has *not* played the last
move. So:

- After one move (Black played), White is to move; Black's move is the
  opponent's last move.
- After two moves, Black is to move; Black's move is "my" last move and
  White's move is the opponent's last move.
- On an empty board, both last-move planes are all zero.

The one-hot index is `(row + 1) * EXT + (col + 1)` because the 15×15
board is embedded in the center of the 17×17 plane, leaving the one-cell
border ring to the engine's `you` plane.

### Tensor-ready layout

`InputPlanes` stores each plane as a flat `[u8; EXT * EXT]` array. That
is the natural layout for `TensorData::new(data, [batch, 4, EXT, EXT])`
in NCHW order. The `to_array()` helper returns the same data as a
`[4, EXT, EXT]` array, which is convenient for tests and for any caller
that wants to reason about individual planes as 2-D boards.

The actual `Tensor` construction is deliberately left to chapter 2: the
batcher will stack a batch of `InputPlanes` into a single
`Tensor<B, 4>`.

## Low-level design

### Files

```text
gomoku/
├── Cargo.toml          # add "crates/net" to workspace.members
└── crates/net/
    ├── Cargo.toml      # new crate manifest
    └── src/
        ├── lib.rs      # #![deny(missing_docs)] + pub mod planes;
        └── planes.rs   # new module
```

### `gomoku/Cargo.toml`

Add `"crates/net"` to the `members` array:

```toml
[workspace]
members = ["crates/engine", "crates/cli", "crates/mcts", "crates/train", "crates/net"]
```

### `crates/net/Cargo.toml`

```toml
[package]
name = "net"
description = "Policy/value network for the Gomoku AlphaZero-style agent."
version.workspace = true
edition.workspace = true
license.workspace = true

[features]
default = ["flex"]
flex = ["burn/flex", "burn/dataset", "burn/train"]
gpu = ["burn/wgpu", "burn/dataset", "burn/train"]

[dependencies]
burn = { workspace = true }
engine = { path = "../engine" }
train = { path = "../train" }
rand = "0.10.3"
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
engine = { path = "../engine", features = ["testutil"] }
```

Key points:

- `burn = { workspace = true }` picks up the pinned `=0.21.0` from the
  workspace root.
- The `flex` feature enables Burn's default CPU backend and the
  `dataset`/`train` features needed later. The `gpu` feature swaps in
  `wgpu` for local GPU training.
- `dev-dependencies` pulls `engine/testutil` so tests can use engine
  test helpers if needed.

### `src/lib.rs`

```rust
//! Neural network training for the Gomoku AlphaZero-style agent.
//!
//! This crate implements the policy/value network and the manual
//! supervised-training loop for phase 0. It consumes labelled samples
//! from the `train` crate and derives input planes from engine boards.

#![deny(missing_docs)]

pub mod planes;
```

Only `pub mod planes;` is needed for this chapter. The remaining modules
land in later chapters.

### `src/planes.rs`

```rust
//! Assemble the four 17×17 input planes from an engine [`Board`].
//!
//! Plane 0: stones of the player to move ("me").
//! Plane 1: stones of the opponent ("you"), including the border ring.
//! Plane 2: the most recent move played by "me", if any.
//! Plane 3: the most recent move played by "you", if any.
//!
//! This module owns the "planes live in `net`" invention (I1): the
//! engine emits only two planes, and `net` adds the history planes.

use engine::{Board, EXT, Move, Planes, encode as encode_planes};

/// Four 17×17 input planes as contiguous `u8` arrays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputPlanes {
    /// Stones of the player to move.
    pub me: [u8; EXT * EXT],
    /// Opponent stones plus the border ring.
    pub you: [u8; EXT * EXT],
    /// The last move played by the player to move.
    pub my_last: [u8; EXT * EXT],
    /// The last move played by the opponent.
    pub opp_last: [u8; EXT * EXT],
}

impl InputPlanes {
    /// Returns the four planes as a shape `[4, EXT, EXT]` array.
    pub fn to_array(&self) -> [[[u8; EXT]; EXT]; 4];
}

/// Build the four input planes for the side to move on `board`.
///
/// The `board` must carry a correct move history (`Board::moves()`),
/// because the last-move planes are derived from it.
pub fn assemble(board: &Board) -> InputPlanes;
```

Implementation notes:

- `me` and `you` come from `engine::encode(board)`.
- `my_last` and `opp_last` are built from `board.moves()` using the
  parity rule described above.
- All four arrays are flat, row-major within each plane.

### Tests

Inside `#[cfg(test)] mod tests` in `planes.rs`:

1. `empty_board_has_no_last_moves` — both last-move planes are all zero.
2. `after_one_move_opp_last_is_set` — after a single Black move, White
   is to move and the opponent's last move is set.
3. `after_two_moves_both_last_planes_are_set` — after B-W, Black is to
   move and both last-move planes are set.
4. `last_move_planes_flip_with_side_to_move` — the same stone
   configuration viewed from different sides to move produces swapped
   `my_last`/`opp_last` planes.
5. `to_array_shape_is_four_planes` — `to_array()` returns `[4, EXT, EXT]`.
6. `stones_match_engine_encode` — the `me` and `you` planes equal the
   engine's `encode` output.

## Solution (opt-in)

The complete reference code for this chapter — `Cargo.toml`, `lib.rs`
registration, and `planes.rs` — lives in
[01-planes-and-the-net-crate/01-solution.md](01-planes-and-the-net-crate/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare.

## TDD checklist

1. **Red:** Create the crate, add the workspace member, write
   `Cargo.toml` and `lib.rs`, and create `planes.rs` with the struct,
   `assemble`, `to_array`, and the six tests, leaving the function
   bodies as `todo!()`. Run `cargo test -p net`. Expect failures.
2. **Green:** Implement `assemble` and `to_array`. Re-run
   `cargo test -p net`. All six tests should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p net` passes from `gomoku/`.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Commit message in the reference worktree: `feat(net): crate skeleton + input planes`.

Next: Chapter 02 — The batcher + D4 augmentation.

## References

* [`docs/specs/2026-10-04-net-tutorial-design.md`](../../specs/2026-10-04-net-tutorial-design.md) —
  binding spec: invention I1, chapter map, hard rules.
* [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  input plane design, dependency-island rule, workspace layout.
* [`docs/13-engine-design.md`](../../13-engine-design.md) —
  engine encoding (`encode`, `EXT`, relative colors).
* [`docs/tutorials/convolutions-tutorial/01-planes-and-a-conv-by-hand.md`](../convolutions-tutorial/01-planes-and-a-conv-by-hand.md) —
  plane-encoding precedent in the `patterns` crate.
* [`gomoku/crates/engine/src/encode.rs`](../../../gomoku/crates/engine/src/encode.rs) —
  exact `encode` output and border-ring semantics.
* [`gomoku/crates/engine/src/symmetry.rs`](../../../gomoku/crates/engine/src/symmetry.rs) —
  `Transform`, used in chapter 2.
