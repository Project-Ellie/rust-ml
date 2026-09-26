//! Evaluation interface: the seam between search and the network.
//!
//! The search crate knows only the [`Evaluator`] trait. Production
//! implementations (direct net wrapper, channel client) live in
//! downstream crates; this crate provides small deterministic stubs
//! for unit tests.

use engine::{Move, Planes};

/// Evaluation request, contains the board as Planes and a vector of legal moves
#[derive(Debug, Clone, PartialEq)]
pub struct EvalRequest {
    /// the board in its 17x17 representation
    pub planes: Planes,

    /// Vector with all legal moves
    pub legal: Vec<Move>,
}

/// The result: Policy and Value
#[derive(Debug, Clone, PartialEq)]
pub struct EvalResult {
    /// the policy vector: 15x15
    pub policy: [f32; 225],

    /// the value of the position `[-1.0 .. 1.0]` from current player's point of view
    pub value: f32,
}

/// the contract with any evaluator implementation
pub trait Evaluator {
    /// return policy and value for current board position
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult;
}

/// Same value for every position
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UniformEvaluator {
    /// value fixed at creation time
    pub value: f32,
}

impl UniformEvaluator {
    /// new evaluator with fixed value
    #[must_use]
    pub fn new(value: f32) -> Self {
        Self { value }
    }
}

impl Default for UniformEvaluator {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl Evaluator for UniformEvaluator {
    fn evaluate(&mut self, _request: EvalRequest) -> EvalResult {
        EvalResult {
            policy: [0.0; 225],
            value: self.value,
        }
    }
}

/// stable softmax over all logits on legal moves
#[must_use]
pub fn masked_softmax(logits: &[f32; 225], legal: &[Move]) -> Vec<(Move, f32)> {
    let max_logit = legal
        .iter()
        .map(|mv| logits[mv.index()])
        .fold(f32::NEG_INFINITY, f32::max);

    let mut sum = 0.0f64;

    let exp_values: Vec<(Move, f64)> = legal
        .iter()
        .map(|&mv| {
            let v = f64::from((logits[mv.index()] - max_logit).exp());
            sum += v;
            (mv, v)
        })
        .collect();

    if sum == 0.0 {
        let uniform = 1.0 / legal.len() as f32;
        return legal.iter().map(|&mv| (mv, uniform)).collect();
    }

    let inv_sum = 1.0 / sum;
    exp_values
        .into_iter()
        .map(|(mv, prob)| (mv, (prob * inv_sum) as f32))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{EvalRequest, UniformEvaluator, masked_softmax};
    use engine::Board;

    fn all_legal_moves(board: &Board) -> Vec<Move> {
        board.empty_moves().collect()
    }

    #[test]
    fn masked_softmax_sums_to_one() {
        let logits = [0.0f32; 225];
        let legal = all_legal_moves(&Board::new());
        let dist = masked_softmax(&logits, &legal);
        let sum: f32 = dist.iter().map(|&(_, prob)| prob).sum();
        assert!((sum - 1.0).abs() < 1e-5, "sum = {sum}");
        assert_eq!(dist.len(), legal.len());
    }

    #[test]
    fn illegal_moves_get_zero_probability() {
        let mut logits = [0.0f32; 225];
        let mut board = Board::new();
        board.play(Move::new(0, 0).unwrap()).unwrap();
        board.play(Move::new(0, 1).unwrap()).unwrap();
        logits[0] = 1000.0;
        logits[1] = 1000.0;
        let legal = all_legal_moves(&board);
        let dist = masked_softmax(&logits, &legal);
        for (mv, p) in &dist {
            assert!(
                !(mv == &Move::new(0, 0).unwrap() || mv == &Move::new(0, 1).unwrap()),
                "illegal move appeared in distribution"
            );
            assert!(*p > 0.0 && *p <= 1.0);
        }
        let sum: f32 = dist.iter().map(|&(_, prob)| prob).sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }

    #[test]
    fn mask_before_normalize_guard() {
        let mut logits = [f32::NEG_INFINITY; 225];
        logits[Move::new(7, 7).unwrap().index()] = 1.0;
        logits[Move::new(0, 0).unwrap().index()] = 1e9;

        let legal = vec![Move::new(7, 7).unwrap()];
        let dist = masked_softmax(&logits, &legal);
        assert_eq!(dist.len(), 1);
        assert!((dist[0].1 - 1.0).abs() < 1e-5);
    }

    #[test]
    fn uniform_evaluator_returns_constant_value() {
        let mut ev = UniformEvaluator::new(0.37);
        let req = EvalRequest {
            planes: engine::encode(&Board::new()),
            legal: all_legal_moves(&Board::new()),
        };
        let res = ev.evaluate(req);
        assert_eq!(res.value, 0.37);
        assert_eq!(res.policy, [0.0f32; 225]);
    }
}
