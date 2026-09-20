# Chapter 9 — Solution: `TacticsEvaluator` and sign sentinels

This file quotes the verified reference implementation verbatim. Read it
when you are stuck, or after you have typed your own version, to compare.

---

## `src/mock.rs`

```rust
//! Deterministic mock evaluator for milestone-2 tests.
//!
//! The `TacticsEvaluator` shapes policy priors using the engine's
//! tactics module: immediate wins and forced blocks are concentrated
//! on, everything else is near-uniform noise. Values are crude
//! tutorial scaffolding — they are clearly marked as such and are not
//! meant to generalize beyond the tactical test suite.

use crate::eval::{EvalRequest, EvalResult, Evaluator};
use engine::{Board, Move};

/// Deterministic evaluator using engine tactics.
///
/// Prior allocation:
/// * If the side to move has immediate wins: 90% of mass split evenly
///   over the winning moves, 10% split evenly over all other legal moves.
/// * Else if the opponent has immediate wins (forced blocks): 90% of
///   mass split evenly over the blocking moves, 10% split evenly over
///   the rest.
/// * Else: uniform over all legal moves.
///
/// Value assignment (tutorial scaffolding):
/// * Own immediate win: +0.95
/// * Forced block required: -0.90
/// * Otherwise: 0.00
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TacticsEvaluator;

impl TacticsEvaluator {
    /// Create a new tactics evaluator.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Evaluator for TacticsEvaluator {
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
        let board = reconstruct_board(&req.planes, &req.legal);
        let wins = engine::immediate_wins(&board, board.to_move());
        let blocks = engine::forced_blocks(&board);

        let (highlighted, value) = if !wins.is_empty() {
            (wins, 0.95f32)
        } else if !blocks.is_empty() {
            (blocks, -0.90f32)
        } else {
            // Near-uniform; tiny preference for center is unnecessary
            // for the milestone-2 tests.
            let mut policy = [0.0f32; 225];
            let n = req.legal.len();
            if n > 0 {
                let logit = (n as f32).ln();
                for mv in &req.legal {
                    policy[mv.index()] = logit;
                }
            }
            return EvalResult { policy, value: 0.0 };
        };

        let mut policy = [f32::NEG_INFINITY; 225];
        let k = highlighted.len();
        let n_other = req.legal.len().saturating_sub(k as usize);

        let highlight_logit = if k == 1 {
            2.2f32 // single winning/blocking move
        } else {
            1.6f32 // split among several
        };
        let other_logit = if n_other == 0 {
            f32::NEG_INFINITY
        } else {
            -1.2f32
        };

        for mv in highlighted.iter() {
            policy[mv.index()] = highlight_logit;
        }
        if n_other > 0 {
            for mv in &req.legal {
                if !highlighted.contains(*mv) {
                    policy[mv.index()] = other_logit;
                }
            }
        }

        EvalResult { policy, value }
    }
}

/// Reconstruct a `Board` from encoded planes and legal moves.
///
/// The evaluator receives `Planes`, not a `Board`. For the mock we
/// rebuild the board by replaying the move history implied by the
/// planes. This is correct because `encode` is relative to the side
/// to move, but the engine stores absolute colors. We recover the
/// absolute position by walking the cells and assigning colors based
/// on `to_move` inferred from stone counts.
fn reconstruct_board(planes: &engine::Planes, legal: &[Move]) -> Board {
    // Start from an empty board and collect occupied cells.
    let mut black = Vec::new();
    let mut white = Vec::new();

    // We need to know which plane is "me" in the encoded position.
    // The encoded planes are relative to the side to move. Since we
    // don't have the board, we infer to_move from parity: if black
    // and white counts are equal, Black to move; otherwise White to
    // move (because Black moves first).
    for r in 0..15u8 {
        for c in 0..15u8 {
            let dst = (r as usize + 1) * engine::EXT + (c as usize + 1);
            let mv = Move::new(r, c).unwrap();
            if planes.me[dst] == 1 {
                black.push(mv);
            } else if planes.you[dst] == 1 {
                white.push(mv);
            }
        }
    }

    let to_move = if black.len() == white.len() {
        engine::Color::Black
    } else {
        engine::Color::White
    };

    // The above assignment assumes "me" == black. But if to_move is
    // White, then "me" is actually White. Swap.
    let (black, white) = if to_move == engine::Color::White {
        (white, black)
    } else {
        (black, white)
    };

    let board = Board::from_position(&black, &white, to_move)
        .expect("encoded planes must form a valid position");

    // Safety net: ensure the supplied legal moves agree with the board.
    // In test positions this should always hold; if not, panic loudly.
    let computed_legal: std::collections::HashSet<_> = board.empty_moves().collect();
    for mv in legal {
        assert!(
            computed_legal.contains(mv),
            "supplied legal move {mv:?} is not legal in reconstructed board"
        );
    }

    board
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{EvalRequest, Evaluator};
    use engine::{Board, Color, Move};

    #[test]
    fn tactics_evaluator_concentrates_on_immediate_win() {
        let mut board = Board::new();
        // Black open four on row 7.
        let script = [
            (7, 3),
            (0, 0),
            (7, 4),
            (0, 2),
            (7, 5),
            (0, 4),
            (7, 6),
            (0, 6),
        ];
        for &(r, c) in &script {
            board.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(board.to_move(), Color::Black);

        let req = EvalRequest {
            planes: engine::encode(&board),
            legal: board.empty_moves().collect(),
        };
        let mut ev = TacticsEvaluator;
        let res = ev.evaluate(req);
        assert!((res.value - 0.95).abs() < 1e-6);

        let wins = engine::immediate_wins(&board, Color::Black);
        for mv in wins.iter() {
            assert!(
                res.policy[mv.index()] > 0.0,
                "winning move {mv:?} must have positive logit"
            );
        }
    }

    #[test]
    fn tactics_evaluator_values_forced_block_negatively() {
        use engine::reference::board_from_ascii;
        let b = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . O O O O . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            X . X . X . X . . . . . . . .
            ",
        );
        assert_eq!(b.to_move(), Color::Black);

        let req = EvalRequest {
            planes: engine::encode(&b),
            legal: b.empty_moves().collect(),
        };
        let mut ev = TacticsEvaluator;
        let res = ev.evaluate(req);
        assert!((res.value + 0.90).abs() < 1e-6);
    }
}

```

---

## Sign sentinels in `src/search.rs`

Append these two tests to the `#[cfg(test)] mod tests` block in
`src/search.rs`:

```rust
    #[test]
    fn immediate_win_for_side_to_move_gets_most_visits() {
        // Black has an open four on row 7; (7,3) or (7,8) wins.
        use engine::reference::board_from_ascii;
        let b = board_from_ascii(
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
        assert_eq!(b.status(), Status::Ongoing);
        assert_eq!(b.to_move(), Color::Black);

        let mut ev = TacticsEvaluator;
        let outcome = search(
            &b,
            &mut ev,
            &SearchConfig {
                simulations: 50,
                c_puct: 1.5,
            },
        );
        let root_edges = outcome.tree.node(outcome.root).edges();
        let best = root_edges
            .iter()
            .max_by_key(|e| e.n)
            .expect("root has edges");
        let wins = engine::immediate_wins(&b, b.to_move());
        assert!(
            wins.contains(best.mv),
            "best move {:?} must be a winning move",
            best.mv
        );
    }

    #[test]
    fn opponent_immediate_win_forces_block() {
        // White has an open four on row 9; Black to move must block.
        use engine::reference::board_from_ascii;
        let b = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . O O O O . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            X . X . X . X . . . . . . . .
            ",
        );
        assert_eq!(b.to_move(), Color::Black);
        assert!(!engine::forced_blocks(&b).is_empty());

        let mut ev = TacticsEvaluator;
        let outcome = search(
            &b,
            &mut ev,
            &SearchConfig {
                simulations: 50,
                c_puct: 1.5,
            },
        );
        let root_edges = outcome.tree.node(outcome.root).edges();
        let best = root_edges.iter().max_by_key(|e| e.n).unwrap();
        let blocks = engine::forced_blocks(&b);
        assert!(
            blocks.contains(best.mv),
            "best move {:?} must be a block",
            best.mv
        );
    }
```

---

## Remember

* The constants `0.95`, `−0.90`, `2.2`, `1.6`, `−1.2` are tutorial
  scaffolding. They are deliberately not tuned.
* `reconstruct_board` exists only because the mock consumes `Planes`.
  The real network in milestone 3 will not need it.
* These two sign sentinels are the cheapest possible proof that the
  backup sign convention is correct. If you ever refactor backup,
  these tests must stay green.
