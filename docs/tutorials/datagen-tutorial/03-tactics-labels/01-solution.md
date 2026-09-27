# Chapter 3 solution — Tactics labeling

This file contains the reference implementation for chapter 3. Open it
only after you have tried the chapter yourself or if you have been
stuck for more than twenty minutes.

## `crates/train/Cargo.toml`

Add the `testutil` engine feature under `[dev-dependencies]`:

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
rand = "0.10.3"

[dev-dependencies]
engine = { path = "../engine", features = ["testutil"] }
```

## `crates/train/src/lib.rs`

Register the new module:

```rust
//! Synthetic training-data generation for the AlphaZero-style Gomoku agent.
//!
//! This crate stores labelled training positions as stone lists, not as
//! neural-network planes. Planes are derived on read in the `net` crate, and
//! a single position can be augmented with a random D4 transform every time it
//! is loaded. At this slice the crate has no Burn dependency and no I/O;
//! randomness lives in the `playout` module.
//!
//! Design documents (locked):
//!
//! * `docs/12-gomoku-architecture.md` in the rust-ml repository — workspace
//!   layout, serialization decision, and the store-games replay-buffer rule.
//! * `docs/specs/2026-09-27-datagen-tutorial-design.md` — synthetic data
//!   generator design for milestone 3.

pub mod label;
pub mod playout;
pub mod sample;

pub use sample::Sample;
```

## `crates/train/src/label.rs`

```rust
//! Tactics-based labelling for synthetic training positions.
//!
//! Every sampled position is classified as win, block, or quiet by the
//! engine's tactics module. The shaped policy target puts 90% of its
//! mass on the tactical moves and 10% on the remaining legal moves;
//! quiet positions receive a uniform target. Value targets are exact
//! `+1.0` / `-1.0` / `0.0` from the side-to-move perspective.

use engine::{Board, Move};

/// Tactical class of a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TacticalClass {
    /// The side to move has at least one immediate win.
    Win,
    /// The opponent has at least one immediate win; forced blocks exist.
    Block,
    /// No immediate wins for either side on this move.
    Quiet,
}

/// Classify `board` into a [`TacticalClass`].
///
/// * [`TacticalClass::Win`] if the side to move has at least one
///   immediate win.
/// * [`TacticalClass::Block`] otherwise, if the opponent has at least
///   one immediate win.
/// * [`TacticalClass::Quiet`] otherwise.
pub fn classify(board: &Board) -> TacticalClass {
    if !engine::immediate_wins(board, board.to_move()).is_empty() {
        TacticalClass::Win
    } else if !engine::forced_blocks(board).is_empty() {
        TacticalClass::Block
    } else {
        TacticalClass::Quiet
    }
}

/// Produce the shaped training target for `board`.
///
/// Returns a sparse policy distribution over all legal moves and the
/// side-to-move value target. Win/block positions put 90% of their
/// mass evenly over the tactical moves and 10% evenly over the rest;
/// quiet positions are uniform over all legal moves.
pub fn label(board: &Board) -> (Vec<(Move, f32)>, f32) {
    let legal: Vec<Move> = board.empty_moves().collect();
    if legal.is_empty() {
        return (Vec::new(), 0.0);
    }

    let wins = engine::immediate_wins(board, board.to_move());
    if !wins.is_empty() {
        return shape(&legal, wins, 1.0);
    }

    let blocks = engine::forced_blocks(board);
    if !blocks.is_empty() {
        return shape(&legal, blocks, -1.0);
    }

    // Quiet: uniform over all legal moves. The f32 sum is within
    // rounding error of 1.0, inside test tolerance, and downstream
    // consumers normalize anyway.
    let uniform = 1.0 / legal.len() as f32;
    let policy: Vec<(Move, f32)> = legal.iter().map(|&mv| (mv, uniform)).collect();

    (policy, 0.0)
}

/// Shape a win/block policy target.
///
/// Precondition: `tactical` must be a subset of `legal` (otherwise
/// `rest = n - k` would underflow).
fn shape(legal: &[Move], tactical: engine::MoveSet, value: f32) -> (Vec<(Move, f32)>, f32) {
    let k = tactical.len() as usize;
    let n = legal.len();
    let rest = n - k;

    if rest == 0 {
        let p = 1.0 / k as f32;
        return (legal.iter().map(|&mv| (mv, p)).collect(), value);
    }

    // Direct 90/10 split. Each tactical move gets 0.9/k, each
    // non-tactical move gets 0.1/rest. The f32 total is within
    // rounding error of 1.0, inside test tolerance, and downstream
    // consumers normalize.
    let tactical_p = 0.9_f32 / k as f32;
    let other_p = 0.1_f32 / rest as f32;

    let policy: Vec<(Move, f32)> = legal
        .iter()
        .map(|&mv| {
            if tactical.contains(mv) {
                (mv, tactical_p)
            } else {
                (mv, other_p)
            }
        })
        .collect();

    (policy, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::reference::board_from_ascii;

    #[test]
    fn open_four_for_side_to_move_is_win() {
        let board = board_from_ascii(
            "
            O . O . O . O . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . X X X X . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            ",
        );

        assert_eq!(classify(&board), TacticalClass::Win);
        let (policy, value) = label(&board);
        assert_eq!(value, 1.0);

        let legal_count = board.empty_moves().count();
        assert_eq!(policy.len(), legal_count);

        let wins = engine::immediate_wins(&board, board.to_move());
        let k = wins.len() as usize;
        let rest = legal_count - k;

        let sum: f32 = policy.iter().map(|(_, p)| p).sum();
        assert!((sum - 1.0).abs() < 1e-5, "policy masses sum to {sum}");

        let argmax = policy
            .iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(m, _)| *m)
            .unwrap();
        assert!(wins.contains(argmax), "argmax {argmax:?} is a winning move");

        for (mv, p) in &policy {
            if wins.contains(*mv) {
                assert!((p - 0.9 / k as f32).abs() < 1e-5);
            } else {
                assert!((p - 0.1 / rest as f32).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn closed_four_for_opponent_is_block() {
        let board = board_from_ascii(
            "
            O . O . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . O X X X X . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            ",
        );

        assert_eq!(classify(&board), TacticalClass::Block);
        let (policy, value) = label(&board);
        assert_eq!(value, -1.0);

        let legal_count = board.empty_moves().count();
        assert_eq!(policy.len(), legal_count);

        let blocks = engine::forced_blocks(&board);
        let k = blocks.len() as usize;
        let rest = legal_count - k;

        let sum: f32 = policy.iter().map(|(_, p)| p).sum();
        assert!((sum - 1.0).abs() < 1e-5, "policy masses sum to {sum}");

        let argmax = policy
            .iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(m, _)| *m)
            .unwrap();
        assert!(
            blocks.contains(argmax),
            "argmax {argmax:?} is a forced block"
        );

        for (mv, p) in &policy {
            if blocks.contains(*mv) {
                assert!((p - 0.9 / k as f32).abs() < 1e-5);
            } else {
                assert!((p - 0.1 / rest as f32).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn scattered_position_is_quiet_and_uniform() {
        let board = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . X . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . O . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . O . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . X . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            ",
        );

        assert_eq!(classify(&board), TacticalClass::Quiet);
        let (policy, value) = label(&board);
        assert_eq!(value, 0.0);

        let legal_count = board.empty_moves().count();
        assert_eq!(policy.len(), legal_count);

        let sum: f32 = policy.iter().map(|(_, p)| p).sum();
        assert!((sum - 1.0).abs() < 1e-5, "policy masses sum to {sum}");

        let expected = 1.0 / legal_count as f32;
        for (_, p) in &policy {
            assert!((p - expected).abs() < 1e-5);
        }
    }

    #[test]
    fn win_takes_precedence_when_both_sides_have_immediate_win() {
        let board = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . X X X X . . . . . . . .
            . . . O O O O . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            ",
        );

        let wins = engine::immediate_wins(&board, board.to_move());
        let opponent_wins = engine::immediate_wins(&board, board.to_move().other());
        assert!(!wins.is_empty(), "side to move has an immediate win");
        assert!(
            !opponent_wins.is_empty(),
            "opponent also has an immediate win"
        );

        assert_eq!(classify(&board), TacticalClass::Win);
        let (policy, value) = label(&board);
        assert_eq!(value, 1.0);

        let k = wins.len() as usize;
        for (mv, p) in &policy {
            if wins.contains(*mv) {
                assert!((p - 0.9 / k as f32).abs() < 1e-5);
            } else {
                assert!(
                    *p < 0.9 / k as f32,
                    "non-tactical mass is below tactical mass"
                );
            }
        }
    }

    #[test]
    fn full_board_terminal_draw_has_empty_policy_and_zero_value() {
        let mut board = Board::new();
        let mut blacks = Vec::new();
        let mut whites = Vec::new();
        for r in 0..15u8 {
            for c in 0..15u8 {
                let stripe = (c % 4) < 2;
                let black = stripe != (r % 2 == 1);
                if black {
                    blacks.push(Move::new(r, c).unwrap());
                } else {
                    whites.push(Move::new(r, c).unwrap());
                }
            }
        }
        for i in 0..112 {
            board.play(blacks[i]).unwrap();
            board.play(whites[i]).unwrap();
        }
        board.play(blacks[112]).unwrap();
        assert_eq!(board.status(), engine::Status::Draw);
        assert!(board.empty_moves().next().is_none());

        let (policy, value) = label(&board);
        assert!(policy.is_empty());
        assert_eq!(value, 0.0);
    }
}
```

## Notes

* The `testutil` dev-dependency is the minimal correct way to use the
  ASCII puzzle parser in `train` tests. It does not affect the
  production build.
* The value targets are `+1.0` / `−1.0` / `0.0` because these are the
  exact targets the value-head squared-error loss can reach. The MCTS
  mock's `+0.95` / `−0.90` are search scaffolding, not training labels.
* The policy masses are assigned directly (`0.9 / k` and `0.1 / rest`)
  rather than derived from a remainder; the `f32` total is within
  rounding error of `1.0` and downstream consumers normalize.
