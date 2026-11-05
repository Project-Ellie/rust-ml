//! Hand-written policy/value evaluator for the Gomoku baseline player.
//!
//! This crate implements the [`mcts::Evaluator`] seam with a
//! deterministic policy that combines the gravity kernel with
//! immediate wins and forced blocks from the threat network. The
//! value stays flat at `0.0` in this sub-chapter.

#![deny(missing_docs)]

pub mod adapter;
pub mod gravity;
pub mod readout;

pub use burn::tensor::Device;

use engine::Color;
use gravity::GravityKernel;
use mcts::{EvalRequest, EvalResult, Evaluator};
use patterns::net::ThreatNet;

/// The baseline evaluator.
///
/// v2 combines the gravity kernel with immediate wins and forced
/// blocks from the threat network. The value stays flat at `0.0`.
pub struct BaselineEvaluator {
    device: Device,
    kernel: GravityKernel,
    net: ThreatNet,
}

impl BaselineEvaluator {
    /// Create a new [`BaselineEvaluator`] on `device`.
    #[must_use]
    pub fn new(device: Device) -> Self {
        let kernel = GravityKernel::new(&device);
        let net = ThreatNet::new(&device);
        Self {
            device,
            kernel,
            net,
        }
    }
}

impl Evaluator for BaselineEvaluator {
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
        let _ = &self.device;
        let board = adapter::reconstruct_board(&req.planes, &req.legal);
        let maps = patterns::net::analyze_both(&self.net, &board, &self.device);
        let (own, opp) = match board.to_move() {
            Color::Black => (&maps[0], &maps[1]),
            Color::White => (&maps[1], &maps[0]),
        };
        let gravity = self.kernel.compute(&req.planes);
        let policy = readout::policy_logits(&gravity, own, opp, &req.legal);
        EvalResult {
            policy,
            value: readout::value(own, opp),
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
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);
        for mv in board.empty_moves() {
            assert_eq!(res.policy[mv.index()], 0.0f32);
        }
    }

    #[test]
    fn v0_occupied_cells_are_masked() {
        let mut board = Board::new();
        board.play(Move::new(7, 7).unwrap()).unwrap();
        board.play(Move::new(0, 0).unwrap()).unwrap();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);
        assert_eq!(
            res.policy[Move::new(7, 7).unwrap().index()],
            f32::NEG_INFINITY
        );
        assert_eq!(
            res.policy[Move::new(0, 0).unwrap().index()],
            f32::NEG_INFINITY
        );
    }

    #[test]
    fn v0_value_is_zero() {
        let board = Board::new();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);
        assert_eq!(res.value, 0.0f32);
    }

    #[test]
    fn v0_is_deterministic() {
        let board = Board::new();
        let req = request_from_board(&board);
        let mut ev1 = BaselineEvaluator::new(Device::flex());
        let mut ev2 = BaselineEvaluator::new(Device::flex());
        let res1 = ev1.evaluate(req.clone());
        let res2 = ev2.evaluate(req);
        assert_eq!(res1, res2);
    }

    #[test]
    fn v1_highest_logit_hugs_stones() {
        let mut board = Board::new();
        board.play(Move::new(7, 7).unwrap()).unwrap();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);

        let max_logit = board
            .empty_moves()
            .map(|mv| res.policy[mv.index()])
            .fold(f32::NEG_INFINITY, f32::max);
        assert_eq!(max_logit, 2.0f32 / 32.0f32);

        // Cells far from the stone (distance > 2) remain at zero.
        assert_eq!(res.policy[Move::new(0, 0).unwrap().index()], 0.0f32);
    }

    #[test]
    fn v1_empty_board_is_uniform() {
        let board = Board::new();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);
        for mv in board.empty_moves() {
            assert_eq!(res.policy[mv.index()], 0.0f32);
        }
    }

    #[test]
    fn v1_corner_stone_not_boosted() {
        let mut board = Board::new();
        board.play(Move::new(0, 0).unwrap()).unwrap();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);

        let max_logit = board
            .empty_moves()
            .map(|mv| res.policy[mv.index()])
            .fold(f32::NEG_INFINITY, f32::max);
        assert_eq!(max_logit, 2.0f32 / 32.0f32);

        let corner_sum: f32 = board.empty_moves().map(|mv| res.policy[mv.index()]).sum();
        assert!(corner_sum < 1.0f32);
    }

    #[test]
    fn v1_value_still_zero() {
        let board = Board::new();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);
        assert_eq!(res.value, 0.0f32);
    }

    #[test]
    fn t1_win_now_is_argmax() {
        let mut board = Board::new();
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
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);

        let wins = engine::immediate_wins(&board, board.to_move());
        let argmax = board
            .empty_moves()
            .max_by(|a, b| {
                res.policy[a.index()]
                    .partial_cmp(&res.policy[b.index()])
                    .unwrap()
            })
            .unwrap();
        assert!(
            wins.contains(argmax),
            "argmax {argmax:?} must be a winning move"
        );
    }

    #[test]
    fn t2_forced_block_is_argmax() {
        use engine::reference::board_from_ascii;
        let board = board_from_ascii(
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
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req.clone());

        let argmax = board
            .empty_moves()
            .max_by(|a, b| {
                res.policy[a.index()]
                    .partial_cmp(&res.policy[b.index()])
                    .unwrap()
            })
            .unwrap();
        let blocks = engine::forced_blocks(&board);
        assert!(blocks.contains(argmax), "argmax {argmax:?} must be a block");
    }

    #[test]
    fn t2_exact_logit_on_crafted_board() {
        use engine::reference::board_from_ascii;
        let board = board_from_ascii(
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
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req.clone());

        let gravity = gravity::gravity_map(&req.planes);
        let blocks = engine::forced_blocks(&board);
        let mv = blocks.iter().next().unwrap();
        let expected = readout::T2_FORCED_BLOCK + gravity[mv.row() as usize][mv.col() as usize];
        assert_eq!(res.policy[mv.index()], expected);
    }
}
