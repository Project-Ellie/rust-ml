# Chapter 01 — The `train` crate + Sample record

## Abstract

This chapter adds the `train` crate to the `gomoku/` workspace, makes
the one sanctioned change to the engine (serde derives on `Move` and
`Color`), and implements the `Sample` record — a labelled training
position stored as stone lists rather than neural-network planes. By the
end you will have a serializable record type, a bincode roundtrip test,
and a clean clippy/fmt gate.

## Glossary

| Term | Definition |
|------|------------|
| **Sample** | One labelled training position: black stones, white stones, side to move, sparse policy target, and value target. |
| **Position** | A board state expressed in engine vocabulary (`Vec<Move>`, `Color`). |
| **Planes** | The 17×17×4 tensor produced by `engine::encode`; derived from a position, not stored. |
| **Policy target** | A probability distribution over legal moves. In this crate it is stored sparsely as `Vec<(Move, f32)>`. |
| **Value target** | A scalar from the side-to-move perspective: `+1.0` for a winning position, `-1.0` for a blocking position, `0.0` for quiet positions in later chapters. |
| **Shard** | A bincode file containing a stream of `Sample` records. Shards are written in Chapter 05. |
| **bincode 2** | A compact binary serialization format used for local shards and manifests; chosen in `docs/12-gomoku-architecture.md`. |
| **Dependency island** | The engine crate's rule: no Burn, no `rand`, no I/O. |

## Context

Milestones 1 and 2 are complete. The `engine` crate knows the rules of
freestyle 15×15 Gomoku and the `mcts` crate turns a policy/value
opinion into a stronger move. Milestone 3 needs a generator that
produces synthetic training positions labelled by the engine's tactics
module. That generator lives in a new crate called `train`.

The data model is already locked in
[`docs/specs/2026-09-27-datagen-tutorial-design.md`](../../specs/2026-09-27-datagen-tutorial-design.md):
positions are stored as stone lists, policies are stored sparsely, and
records are serialized with bincode 2. This chapter builds only the
record type; the generator binary, shard writer, and labelling logic
arrive in later chapters.

## Intention

1. Add `crates/train` to the workspace.
2. Create the crate skeleton: `Cargo.toml`, `src/lib.rs`, and
   `src/sample.rs`.
3. Add `serde::Serialize` and `serde::Deserialize` to `engine::Move`
   and `engine::Color`. This is the only engine change allowed for the
datagen tutorial.
4. Implement the `Sample` record and its two methods:
   `Sample::from_position` and `Sample::board`.
5. Write three tests: a bincode roundtrip, a parity/alternation test,
   and a board-rebuild test.

Observable done-state: `cargo test -p train` passes from `gomoku/`,
`cargo clippy --all-targets -- -D warnings` is green, and `cargo fmt
--all -- --check` makes no changes.

## Mental mapping

### Why a new crate, instead of code inside `mcts` or `engine`?

The `engine` crate is a dependency island: no Burn, no `rand`, no I/O.
The `mcts` crate owns the search algorithm and a deliberately thin
`Evaluator` trait seam. The data generator needs its own concerns:
random playouts, file I/O, class quotas, and deterministic seeds. None
of those belong inside the rules oracle or the search tree. The
boundary is:

* `engine` — rules only.
* `mcts` — search only, depends on `engine`.
* `train` — synthetic data generation only, depends on `engine`. There
  is deliberately no `train → mcts` edge in the workspace layout locked
in [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md).

### Store positions, not planes

A `Sample` stores two `Vec<Move>` lists and a `Color`, not the 17×17×4
planes that a network consumes. This mirrors the replay-buffer decision
in `docs/12-gomoku-architecture.md`: the buffer stores games, and the
sampler encodes + augments on read. Planes are a derived view of a
position; storing them would freeze the encoding and forbid
augmentation. By keeping the position in engine vocabulary, the reader
can apply a random D4 transform every time the sample is loaded at
almost no cost.

The policy target is also sparse. A dense `[f32; 225]` is 900 bytes,
most of which are zeros. The same distribution is fully described by
`(move, probability)` pairs over the legal moves, so that is what the
sample stores. The training code scatters these pairs into a dense
tensor at batch time.

### Excursion — bincode versus rkyv

`docs/12-gomoku-architecture.md` chose bincode 2 for local shard files.
Why not rkyv, which is often faster?

* **Append-friendly streaming.** Shards are written one sample at a time
  while games are being generated. bincode's `encode_to_vec` produces a
  self-contained byte slice per record; rkyv's zero-copy archives are
  usually built over an entire buffer and are less natural for
  append-only files.
* **Schema evolution is not needed here.** Shards are local, ephemeral
  artifacts. We control both writer and reader, and the `Sample` schema
  is simple.
* **serde integration.** The same `Serialize`/`Deserialize` derives used
  for the manifest can be reused for shard records without an extra
  schema definition.

rkyv wins when the same large archive is read many times by immutable
zero-copy code. That is not the shard access pattern in this project.

### Excursion — why serde derives in engine were budgeted from day one

The engine's `Cargo.toml` already depended on `serde` before this
chapter. That was not an accident: `Move` and `Color` are project
vocabulary, and moving them across crate boundaries as messages or
records requires a stable, versioned serialization boundary. Adding the
derives does not compromise the dependency-island rule because serde is
purely a type-level annotation library; it brings no I/O, no `rand`, and
no Burn. The only alternative — hand-written wrappers inside `train` —
would duplicate the engine's public types and invite the two copies to
drift apart.

## Low-level design

### Files you will create

```text
gomoku/crates/train/
├── Cargo.toml
└── src/
    ├── lib.rs
    └── sample.rs
```

You will also add one line to `gomoku/Cargo.toml` under `[workspace]`
`members`.

### `Cargo.toml`

```toml
[package]
name = "train"
version = "0.1.0"
edition = "2024"
description = "Synthetic training-data generator for the Gomoku AlphaZero-style agent."

[dependencies]
engine = { path = "../engine" }
serde = { workspace = true }
thiserror = { workspace = true }
bincode = { version = "2", features = ["serde"] }
```

Notes:

* `engine` is a path dependency, like `mcts`.
* `serde` and `thiserror` are workspace dependencies. `thiserror` is not
  used in this chapter; it is included because later chapters follow the
  project's error-typing convention.
* `bincode` is version 2 with the `serde` feature enabled.

### Engine change

In `gomoku/crates/engine/src/board.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Color { ... }
```

In `gomoku/crates/engine/src/moveset.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Move(u8);
```

No other engine types or logic change.

### `src/lib.rs`

Crate-level documentation explaining that this crate stores positions
(not planes), has no Burn dependency yet, and does no I/O outside the
future `shard.rs` module and generator binary. Re-export `Sample`.

### `src/sample.rs`

```rust
/// One labelled training position. Stores stones, never planes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Sample {
    pub black: Vec<engine::Move>,
    pub white: Vec<engine::Move>,
    pub to_move: engine::Color,
    pub policy: Vec<(engine::Move, f32)>,
    pub value: f32,
}

impl Sample {
    /// Build from a played move prefix: `history[..ply]` is on the
    /// board; colors alternate from Black; `to_move` follows from parity.
    pub fn from_position(
        history: &[engine::Move],
        ply: usize,
        policy: Vec<(engine::Move, f32)>,
        value: f32,
    ) -> Self;

    /// Rebuild the board via `engine::Board::from_position`.
    pub fn board(&self) -> Result<engine::Board, engine::PositionError>;
}
```

`from_position` must split `history[..ply]` by alternation: indices
`0, 2, 4, ...` are Black; indices `1, 3, 5, ...` are White. The side to
move is Black when `ply` is even and White when `ply` is odd.

`board` delegates to `engine::Board::from_position`, so it returns
`engine::PositionError` if the stored stone lists overlap or have
inconsistent counts.

### Tests

Inside `#[cfg(test)]` in `sample.rs`:

1. `sample_roundtrips_through_bincode` — create a `Sample` with 3 black
   stones, 2 white stones, a policy over 4 moves, and value `1.0`.
   Encode with
   `bincode::serde::encode_to_vec(&sample, bincode::config::standard())`,
   decode with
   `bincode::serde::decode_from_slice(&bytes, bincode::config::standard())`,
   and assert equality.
2. `from_position_splits_by_alternation` — build a sample at `ply = 4`
   and `ply = 5` from the same history; verify the stone lists and
   `to_move`.
3. `board_roundtrip_rebuilds_position` — build a sample from a 5-move
   history, call `board()`, and assert `empty_moves().count() == 225 -
ply`.

## Solution (opt-in)

The complete reference code for this chapter lives in
[01-the-train-crate/01-solution.md](01-the-train-crate/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare.

## TDD checklist

1. **Red:** Add `train` to the workspace, create the crate skeleton with
   the `Sample` struct and the three tests, but leave
   `from_position` and `board` as `todo!()`. Run `cargo test -p train`.
   Expect failures from the two method tests.
2. **Green:** Implement `from_position` and `board`. Re-run `cargo test
   -p train`. All three tests should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p train` passes from `gomoku/`.
* `cargo test -p engine --features testutil` still passes (the serde
  derives are additive).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Commit message in the reference worktree: `feat(train): crate skeleton + Sample record`.

Next: Chapter 02 — Seeded random playout driver.

## References

* [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  workspace crate layout, serialization decision, and store-games
  replay-buffer rule.
* [`docs/specs/2026-09-27-datagen-tutorial-design.md`](../../specs/2026-09-27-datagen-tutorial-design.md)
  — synthetic data generator design for milestone 3.
* [`docs/tutorials/mcts-tutorial/01-the-crate.md`](../mcts-tutorial/01-the-crate.md)
  — the chapter structure this tutorial mirrors.
* [bincode 2 documentation](https://docs.rs/bincode/2.0.1/bincode/) —
  `encode_to_vec` and `decode_from_slice` API used in the roundtrip
  test.
* [serde documentation](https://serde.rs/) — `Serialize` and
  `Deserialize` derives.
