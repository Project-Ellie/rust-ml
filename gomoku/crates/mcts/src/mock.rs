//! Deterministic mock evaluator for milestone-2 tests.
//!
//! The `TacticsEvaluator` shapes policy priors using the engine's
//! tactics module: immediate wins and forced blocks are concentrated
//! on, everything else is near-uniform noise. Values are crude
//! tutorial scaffolding — they are clearly marked as such and are not
//! meant to generalize beyond the tactical test suite.

use crate::eval::{EvalRequest, EvalResult, Evaluator};
use engine::{Board, Color, Move, Planes};
use std::collections::HashSet;

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
    /// Create a new `TacticsEvaluator`
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

fn reconstruct_board(planes: &Planes, legal: &[Move]) -> Board {
    let mut black: Vec<Move> = Vec::new();
    let mut white: Vec<Move> = Vec::new();

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

    let to_move = if white.len() == black.len() {
        Color::Black
    } else {
        Color::White
    };

    let (black, white) = if to_move == Color::White {
        (white, black)
    } else {
        (black, white)
    };

    let board = Board::from_position(&black, &white, to_move)
        .expect("Encoded planes must form a valid position");

    let supplied_legal: HashSet<_> = legal.iter().copied().collect();
    let computed_legal: HashSet<_> = board.empty_moves().collect();
    assert_eq!(
        supplied_legal, computed_legal,
        "Supplied legal moves must match the reconstructed board's legal moves."
    );

    board
}

impl Evaluator for TacticsEvaluator {
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
        let board = reconstruct_board(&req.planes, &req.legal);
        let wins = engine::immediate_wins(&board, board.to_move());
        let blocks = engine::forced_blocks(&board);

        let (highlighted, value) = if !wins.is_empty() {
            (wins, 0.95f32)
        } else if !blocks.is_empty() {
            (blocks, -0.9f32)
        } else {
            let mut policy = [0.0f32; 225];
            let n = req.legal.len();
            if n > 0 {
                let logit = (n as f32).ln();
                for mv in req.legal {
                    policy[mv.index()] = logit;
                }
            }
            return EvalResult { policy, value: 0.0 };
        };

        let mut policy = [f32::NEG_INFINITY; 225];
        let k = highlighted.len();
        let n_other = req.legal.len().saturating_sub(k as usize);

        let highlight_logit = if k == 1 { 2.2f32 } else { 1.6f32 };
        let other_logit = if n_other == 0 {
            f32::NEG_INFINITY
        } else {
            -1.2f32
        };

        for mv in highlighted.iter() {
            policy[mv.index()] = highlight_logit;
        }

        if n_other > 0 {
            for mv in req.legal {
                if !highlighted.contains(mv) {
                    policy[mv.index()] = other_logit;
                }
            }
        }

        EvalResult { policy, value }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::eval::{EvalRequest, Evaluator};
    use engine::{Board, Color, Move};

    #[test]
    fn tactics_evaluator_concentrates_on_immediate_win() {
        let mut board = Board::new();

        let script = &[
            (7, 3),
            (0, 0),
            (7, 4),
            (0, 2),
            (7, 5),
            (0, 4),
            (7, 6),
            (0, 6),
        ];
        for &(r, c) in script {
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
                "Winning move {mv:?} must have positive logit!"
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

    #[test]
    fn tactics_evaluator_is_uniform_and_neutral_on_quiet_board() {
        // Scattered stones, no immediate wins or forced blocks for either side.
        let mut board = Board::new();
        let script = [(7, 7), (0, 0), (8, 9), (14, 14)];
        for &(r, c) in &script {
            board.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert!(engine::immediate_wins(&board, board.to_move()).is_empty());
        assert!(engine::forced_blocks(&board).is_empty());

        let legal: Vec<Move> = board.empty_moves().collect();
        let req = EvalRequest {
            planes: engine::encode(&board),
            legal: legal.clone(),
        };
        let mut ev = TacticsEvaluator;
        let res = ev.evaluate(req);

        assert!(res.value.abs() < 1e-6, "quiet board must be value 0.0");
        let expected = (legal.len() as f32).ln();
        for mv in &legal {
            assert!(
                (res.policy[mv.index()] - expected).abs() < 1e-6,
                "legal move {mv:?} must carry the uniform logit {expected}"
            );
        }
    }
}
