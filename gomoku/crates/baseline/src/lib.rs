//! Hand-written policy/value evaluator for the Gomoku baseline player.
//!
//! This crate implements the [`mcts::Evaluator`] seam with a
//! deterministic, Burn-free (v0) policy and a flat value. The board
//! planes are ignored in this sub-chapter.

#![deny(missing_docs)]

use mcts::{EvalRequest, EvalResult, Evaluator};

/// The baseline evaluator.
///
/// v0 returns a uniform policy over legal moves and a value of `0.0`.
/// The board planes are ignored.
pub struct BaselineEvaluator;

impl BaselineEvaluator {
    /// Create a new [`BaselineEvaluator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for BaselineEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluator for BaselineEvaluator {
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
        let mut policy = [f32::NEG_INFINITY; 225];
        for mv in req.legal {
            policy[mv.index()] = 0.0f32;
        }
        EvalResult {
            policy,
            value: 0.0f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Board, Move};

    fn request_from_board(board: &Board) -> EvalRequest {
        EvalRequest {
            planes: engine::encode(board),
            legal: board.empty_moves().collect(),
        }
    }

    #[test]
    fn v0_policy_is_uniform_over_legal() {
        let board = Board::new();
        let request = request_from_board(&board);
        let mut ev = BaselineEvaluator::new();
        let res = ev.evaluate(request);
        for mv in board.empty_moves() {
            assert_eq!(res.policy[mv.index()], 0.0f32);
        }
    }

    #[test]
    fn v0_occupied_cells_are_masked() {
        let mut board = Board::new();
        let m1 = Move::new(7, 7).unwrap();
        let m2 = Move::new(0, 0).unwrap();
        board.play(m1).unwrap();
        board.play(m2).unwrap();
        let request = request_from_board(&board);
        let mut ev = BaselineEvaluator::new();
        let res = ev.evaluate(request);
        assert_eq!(res.policy[m1.index()], f32::NEG_INFINITY);
        assert_eq!(res.policy[m2.index()], f32::NEG_INFINITY);
    }

    #[test]
    fn v0_value_is_zero() {
        let board = Board::new();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new();
        let res = ev.evaluate(req);
        assert_eq!(res.value, 0.0f32);
    }

    #[test]
    fn v0_is_deterministic() {
        let board = Board::new();
        let req = request_from_board(&board);
        let mut ev1 = BaselineEvaluator::new();
        let mut ev2 = BaselineEvaluator::new();
        let res1 = ev1.evaluate(req.clone());
        let res2 = ev2.evaluate(req);
        assert_eq!(res1, res2);
    }
}
