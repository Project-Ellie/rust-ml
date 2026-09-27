# Chapter 01 — Deep-dive solution: the `train` crate + Sample record

This is the opt-in reference for Chapter 01. It matches the verified
reference crate exactly.

## `Cargo.toml`

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

## `src/sample.rs`

```rust
//! The labelled training record.
//!
//! A [`Sample`] stores a position as two stone lists plus the side to move.
//! Keeping the position in engine vocabulary, rather than as neural-network
//! planes, lets the reader apply augmentation and derive planes on load.

use engine::{Board, Color, Move, PositionError};

/// One labelled training position. Stores stones, never planes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Sample {
    /// Black stones on the board, in absolute color.
    pub black: Vec<Move>,
    /// White stones on the board, in absolute color.
    pub white: Vec<Move>,
    /// The side to move in this position.
    pub to_move: Color,
    /// Sparse policy target over legal moves: `(move, probability)`.
    pub policy: Vec<(Move, f32)>,
    /// Value target from the side-to-move perspective.
    pub value: f32,
}

impl Sample {
    /// Build from a played move prefix: `history[..ply]` is on the
    /// board; colors alternate from Black; `to_move` follows from parity.
    ///
    /// # Panics
    /// Panics if `ply` is greater than `history.len()`.
    pub fn from_position(
        history: &[Move],
        ply: usize,
        policy: Vec<(Move, f32)>,
        value: f32,
    ) -> Self {
        let prefix = &history[..ply];
        let mut black = Vec::with_capacity(ply.div_ceil(2));
        let mut white = Vec::with_capacity(ply / 2);

        for (i, &mv) in prefix.iter().enumerate() {
            if i % 2 == 0 {
                black.push(mv);
            } else {
                white.push(mv);
            }
        }

        let to_move = if ply.is_multiple_of(2) {
            Color::Black
        } else {
            Color::White
        };

        Sample {
            black,
            white,
            to_move,
            policy,
            value,
        }
    }

    /// Rebuild the board via [`Board::from_position`].
    ///
    /// # Errors
    /// Returns the same [`PositionError`] variants as [`Board::from_position`]
    /// if the stored stone lists are inconsistent.
    pub fn board(&self) -> Result<Board, PositionError> {
        Board::from_position(&self.black, &self.white, self.to_move)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv(row: u8, col: u8) -> Move {
        Move::new(row, col).unwrap()
    }

    #[test]
    fn sample_roundtrips_through_bincode() {
        let sample = Sample {
            black: vec![mv(0, 0), mv(0, 2), mv(0, 4)],
            white: vec![mv(1, 0), mv(1, 2)],
            to_move: Color::Black,
            policy: vec![
                (mv(0, 1), 0.45),
                (mv(0, 3), 0.45),
                (mv(2, 0), 0.05),
                (mv(2, 2), 0.05),
            ],
            value: 1.0,
        };

        let bytes = bincode::serde::encode_to_vec(&sample, bincode::config::standard()).unwrap();
        let (decoded, _): (Sample, usize) =
            bincode::serde::decode_from_slice(&bytes, bincode::config::standard()).unwrap();

        assert_eq!(decoded, sample);
    }

    #[test]
    fn from_position_splits_by_alternation() {
        let history = [
            mv(0, 0), // Black
            mv(1, 1), // White
            mv(0, 1), // Black
            mv(1, 0), // White
            mv(0, 2), // Black
        ];

        let sample = Sample::from_position(&history, 4, Vec::new(), 0.0);
        assert_eq!(sample.black, vec![mv(0, 0), mv(0, 1)]);
        assert_eq!(sample.white, vec![mv(1, 1), mv(1, 0)]);
        assert_eq!(sample.to_move, Color::Black);

        let sample = Sample::from_position(&history, 5, Vec::new(), 0.0);
        assert_eq!(sample.black, vec![mv(0, 0), mv(0, 1), mv(0, 2)]);
        assert_eq!(sample.white, vec![mv(1, 1), mv(1, 0)]);
        assert_eq!(sample.to_move, Color::White);
    }

    #[test]
    fn board_roundtrip_rebuilds_position() {
        let history = [mv(7, 7), mv(6, 6), mv(7, 8), mv(6, 8), mv(7, 9)];
        let ply = 5;
        let sample = Sample::from_position(&history, ply, Vec::new(), 1.0);

        let board = sample.board().unwrap();
        assert_eq!(board.to_move(), sample.to_move);
        assert_eq!(board.empty_moves().count(), 225 - ply);
    }
}
```

## `src/lib.rs`

```rust
//! Synthetic training-data generation for the AlphaZero-style Gomoku agent.
//!
//! This crate stores labelled training positions as stone lists, not as
//! neural-network planes. Planes are derived on read in the `net` crate, and
//! a single position can be augmented with a random D4 transform every time it
//! is loaded. At this slice the crate has no Burn dependency, no I/O, and no
//! randomness yet.
//!
//! Design documents (locked):
//!
//! * `docs/12-gomoku-architecture.md` in the rust-ml repository — workspace
//!   layout, serialization decision, and the store-games replay-buffer rule.
//! * `docs/specs/2026-09-27-datagen-tutorial-design.md` — synthetic data
//!   generator design for milestone 3.

pub mod sample;

pub use sample::Sample;
```

## Engine changes

`gomoku/crates/engine/src/board.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Color { ... }
```

`gomoku/crates/engine/src/moveset.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Move(u8);
```

## Workspace membership

`gomoku/Cargo.toml`:

```toml
members = ["crates/engine", "crates/cli", "crates/mcts", "crates/train"]
```
