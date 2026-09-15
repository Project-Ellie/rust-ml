# Chapter 1 — The CLI crate

Goal: a `cli` crate exists in the gomoku workspace, depends on the
engine, and runs. Along the way you open the engine's public surface —
in exactly the ways chapter 13 already plans, plus one additive method.

## Contract

**Workspace** (`gomoku/Cargo.toml`): `members = ["crates/engine",
"crates/cli"]`.

**Engine `lib.rs`** — the re-exports from ch. 13's "Crate layout"
section, as far as slice 3 needs them:

```rust
pub use board::Board;
pub use moveset::Move;
pub use types::{Color, PlayError, Status};   // wherever your slice 3 put them

#[cfg(any(test, feature = "testutil"))]
pub mod reference;
```

The module paths on the right side of each `use` depend on where
*your* slice 3 placed the shared types — adjust.

**Engine `board.rs`** — one new method on the fast `Board`:

```rust
/// `None` = empty cell. Same vocabulary as the reference board.
pub fn stone_at(&self, mv: Move) -> Option<Color>;
```

Why you must add it: slice 3's contract gave the fast board
`stones(color)`, but its natural return type involves `Bitboard`,
which is `pub(crate)` — unusable from another crate. `stone_at` is
the render-facing query, it's two bit tests, and it makes both boards
speak the same API again. Add it with a differential test.

**CLI crate** (`crates/cli/`): package `cli`, binary named `gomoku`
(chapter 12's binary name — this grows into its `play` subcommand).
Depends on `engine` **with the `testutil` feature**, so the naive
oracle is available as a board choice.

## Rust toolbox

**Re-exports as the public API.** `pub use` lets a crate present a
flat surface (`engine::Move`) while its files stay organized
(`moveset::Move`). Users — including your own CLI — only ever write
`engine::Move`, so re-shuffling modules later breaks nobody. This is
why ch. 13 says lib.rs is "re-exports ONLY".

**Features unify.** `engine = { path = "../engine", features =
["testutil"] }` compiles the oracle into your CLI binary. Note what
this means: `cargo build -p engine` alone still skips `reference.rs`;
it only appears when *someone in the build* asks for `testutil`. For
a developer-facing play tool that's fine. When the real `gomoku`
binary from ch. 12 ships, decide whether `play` keeps the naive
option behind a feature flag.

**Stride at the boundary.** `stone_at` converts `Move`'s stride-15
index to the bitboard's stride-16 — slice 3's *the bug* pitfall. Do
the arithmetic in this one method, nowhere else.

## Steps

1. Add `crates/cli` to workspace `members`; `cargo check` the workspace.
2. Add the re-exports to the engine's `lib.rs`; fix what the compiler
   points at (module visibility — `moveset` may need to stay private
   while `Move` is re-exported; that combination is fine).
3. TDD `Board::stone_at`: write the differential test below (RED —
   method doesn't exist), implement it (GREEN).
4. Scaffold the CLI crate and smoke-test the re-exports from `main`.

## Done when

`cargo run -p cli` prints a line proving it can see `engine::Board`,
and `cargo test -p engine` is still fully green (you added a test,
you broke nothing).

Next: [Chapter 2 — The GameBoard trait](02-the-gameboard-trait.md)

---

## Solution

### `gomoku/Cargo.toml`

```toml
[workspace]
resolver = "3"
members = ["crates/engine", "crates/cli"]
```

### Engine `lib.rs`

```rust
//! Gomoku rules engine — milestone 1 of the AlphaZero-style agent.
//! ... (doc header unchanged) ...

mod bitboard;
mod board;
mod encode;
mod moveset;
mod opening;
mod symmetry;
mod tactics;
mod types;     // <- or wherever your slice 3 moved the shared types
mod win;
mod zobrist;

pub use board::Board;
pub use moveset::Move;
pub use types::{Color, PlayError, Status};

#[cfg(any(test, feature = "testutil"))]
pub mod reference;
```

Only the `pub use` lines are new. The modules stay private; the types
they contain are the public surface.

### `Board::stone_at` (engine `board.rs`)

The test first:

```rust
#[cfg(test)]
mod stone_at_tests {
    use super::*;
    use crate::{reference, Color, Move, Status};

    #[test]
    fn stone_at_matches_reference() {
        let mut fast = Board::new();
        let mut naive = reference::Board::new();
        for &(r, c) in &[(7u8, 7u8), (0, 14), (14, 0), (3, 11)] {
            let mv = Move::new(r, c).unwrap();
            assert_eq!(fast.play(mv), naive.play(mv));
        }
        for r in 0..15 {
            for c in 0..15 {
                let mv = Move::new(r, c).unwrap();
                assert_eq!(fast.stone_at(mv), naive.stone_at(mv), "at ({r}, {c})");
            }
        }
        assert_eq!(fast.status(), Status::Ongoing);
    }
}
```

The implementation — two bit tests, stride conversion in exactly one
place:

```rust
impl Board {
    /// `None` = empty cell. Same vocabulary as the reference board.
    pub fn stone_at(&self, mv: Move) -> Option<Color> {
        let idx = mv.row() as usize * 16 + mv.col() as usize; // stride-16!
        if self.black.test(idx) {
            Some(Color::Black)
        } else if self.white.test(idx) {
            Some(Color::White)
        } else {
            None
        }
    }
}
```

(Field names match the slice-3 contract: `black`, `white` bitboards.
Adjust to yours.)

### `crates/cli/Cargo.toml`

```toml
[package]
name = "cli"
description = "Terminal front-end: play Gomoku on either engine board"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "gomoku"
path = "src/main.rs"

[dependencies]
engine = { path = "../engine", features = ["testutil"] }
```

### `crates/cli/src/main.rs` (smoke test — replaced in chapter 2)

```rust
use engine::Board;

fn main() {
    let board = Board::new();
    println!(
        "engine up: {:?} to move, status {:?}",
        board.to_move(),
        board.status()
    );
}
```

Run it:

```text
$ cargo run -p cli
engine up: Black to move, status Ongoing
```
