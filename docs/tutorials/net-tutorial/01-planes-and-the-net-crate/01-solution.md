# Chapter 01 — Deep-dive solution: the `net` crate + planes

This is the opt-in reference for Chapter 01. `Cargo.toml`, `lib.rs`, and
`planes.rs` are quoted verbatim from the verified reference crate, and
byte-identical copies were confirmed with `diff -u` against the reference
worktree before this file was written.

> **Note:** the reference `lib.rs` below declares modules that land in
> later chapters. For chapter 1, only `pub mod planes;` is required.

## `Cargo.toml`

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

## `src/planes.rs`

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
    pub fn to_array(&self) -> [[[u8; EXT]; EXT]; 4] {
        let mut out = [[[0u8; EXT]; EXT]; 4];
        for p in 0..EXT {
            for q in 0..EXT {
                out[0][p][q] = self.me[p * EXT + q];
                out[1][p][q] = self.you[p * EXT + q];
                out[2][p][q] = self.my_last[p * EXT + q];
                out[3][p][q] = self.opp_last[p * EXT + q];
            }
        }
        out
    }
}

/// Build the four input planes for the side to move on `board`.
///
/// The `board` must carry a correct move history (`Board::moves()`),
/// because the last-move planes are derived from it.
pub fn assemble(board: &Board) -> InputPlanes {
    let Planes { me, you } = encode_planes(board);
    let (my_last, opp_last) = last_move_planes(board);
    InputPlanes {
        me,
        you,
        my_last,
        opp_last,
    }
}

fn last_move_planes(board: &Board) -> ([u8; EXT * EXT], [u8; EXT * EXT]) {
    let mut my_last = [0u8; EXT * EXT];
    let mut opp_last = [0u8; EXT * EXT];

    let history = board.moves();
    if history.is_empty() {
        return (my_last, opp_last);
    }

    // History is in play order: Black, White, Black, White, ...
    // The player to move is the one who has *not* played the last move.
    let last_played = history[history.len() - 1];
    set_one_hot(&mut opp_last, last_played);

    if history.len() >= 2 {
        let my_prev = history[history.len() - 2];
        set_one_hot(&mut my_last, my_prev);
    }

    (my_last, opp_last)
}

fn set_one_hot(plane: &mut [u8; EXT * EXT], mv: Move) {
    let dst = (mv.row() as usize + 1) * EXT + (mv.col() as usize + 1);
    plane[dst] = 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Move;

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    #[test]
    fn empty_board_has_no_last_moves() {
        let board = Board::new();
        let planes = assemble(&board);
        assert!(planes.my_last.iter().all(|&x| x == 0));
        assert!(planes.opp_last.iter().all(|&x| x == 0));
    }

    #[test]
    fn after_one_move_opp_last_is_set() {
        let mut board = Board::new();
        board.play(mv(7, 7)).unwrap();
        let planes = assemble(&board);

        // Black played (7,7); White is to move, so Black is "you" (opponent).
        assert_eq!(planes.opp_last[(7 + 1) * EXT + (7 + 1)], 1);
        assert_eq!(planes.my_last.iter().sum::<u8>(), 0);
        assert_eq!(planes.opp_last.iter().sum::<u8>(), 1);
    }

    #[test]
    fn after_two_moves_both_last_planes_are_set() {
        let mut board = Board::new();
        board.play(mv(7, 7)).unwrap();
        board.play(mv(8, 8)).unwrap();
        let planes = assemble(&board);

        // Black to move. Black's last move was (7,7) -> my_last.
        // White's last move was (8,8) -> opp_last.
        assert_eq!(planes.my_last[(7 + 1) * EXT + (7 + 1)], 1);
        assert_eq!(planes.opp_last[(8 + 1) * EXT + (8 + 1)], 1);
        assert_eq!(planes.my_last.iter().sum::<u8>(), 1);
        assert_eq!(planes.opp_last.iter().sum::<u8>(), 1);
    }

    #[test]
    fn last_move_planes_flip_with_side_to_move() {
        let mut black_to_move = Board::new();
        black_to_move.play(mv(3, 3)).unwrap();
        black_to_move.play(mv(4, 4)).unwrap();
        // Black to move, last Black move was (3,3), last White move was (4,4).

        let mut white_to_move = Board::new();
        white_to_move.play(mv(3, 3)).unwrap();
        // White to move, last Black move was (3,3) -> opp_last.

        let black_planes = assemble(&black_to_move);
        let white_planes = assemble(&white_to_move);

        assert_eq!(black_planes.my_last[(3 + 1) * EXT + (3 + 1)], 1);
        assert_eq!(black_planes.opp_last[(4 + 1) * EXT + (4 + 1)], 1);
        assert_eq!(white_planes.opp_last[(3 + 1) * EXT + (3 + 1)], 1);
        assert_eq!(white_planes.my_last.iter().sum::<u8>(), 0);
    }

    #[test]
    fn to_array_shape_is_four_planes() {
        let board = Board::new();
        let planes = assemble(&board);
        let arr = planes.to_array();
        assert_eq!(arr.len(), 4);
        assert_eq!(arr[0].len(), EXT);
        assert_eq!(arr[0][0].len(), EXT);
    }

    #[test]
    fn stones_match_engine_encode() {
        let mut board = Board::new();
        board.play(mv(0, 0)).unwrap();
        board.play(mv(7, 7)).unwrap();
        let planes = assemble(&board);
        let encoded = encode_planes(&board);

        assert_eq!(planes.me, encoded.me);
        assert_eq!(planes.you, encoded.you);
    }
}
```
