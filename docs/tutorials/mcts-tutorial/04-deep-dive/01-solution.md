# Chapter 04 deep dive — The evaluation seam

This is the opt-in reference solution for chapter 04. It is a verbatim copy of `src/eval.rs` from the verified reference crate (`/tmp/mcts-reference/src/eval.rs`), included so you can compare your implementation line by line after you have tried the step yourself.

The code compiles and the tests pass against the reference tree implementation.

```rust
//! Evaluation interface: the seam between search and the network.
//!
//! The search crate knows only the [`Evaluator`] trait. Production
//! implementations (direct net wrapper, channel client) live in
//! downstream crates; this crate provides small deterministic stubs
//! for unit tests.

use engine::{Move, Planes};

/// Request sent to an evaluator: encoded planes plus the legal moves.
#[derive(Debug, Clone, PartialEq)]
pub struct EvalRequest {
    /// Encoded 17×17 planes (relative to the side to move).
    pub planes: Planes,
    /// Legal moves in the position. The evaluator may use this to mask
    /// and normalize its policy output.
    pub legal: Vec<Move>,
}

/// Result returned by an evaluator: policy logits over every cell and
/// a scalar value.
#[derive(Debug, Clone, PartialEq)]
pub struct EvalResult {
    /// Raw policy logits over all 225 cells (15×15 inner board). The
    /// search code is responsible for masking illegal cells and
    /// normalizing.
    pub policy: [f32; 225],
    /// Estimated outcome in `[-1, 1]` from the side-to-move's
    /// perspective.
    pub value: f32,
}

/// Trait implemented by every policy/value provider the search uses.
pub trait Evaluator {
    /// Evaluate one position.
    ///
    /// The returned value must be from the perspective of the player
    /// whose turn it is in `req.planes`.
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult;
}

/// Mask illegal moves, then compute a numerically-stable softmax over
/// the legal moves only.
///
/// This is the bug-farm gate from primer §4.3 and §9 #2: normalize
/// *after* masking, never before. The returned vector contains each
/// legal move paired with its normalized probability; probabilities
/// sum to 1 (within floating-point tolerance).
#[must_use]
pub fn masked_softmax(logits: &[f32; 225], legal: &[Move]) -> Vec<(Move, f32)> {
    // Find the maximum logit among legal moves for numerical stability.
    let max_logit = legal
        .iter()
        .map(|mv| logits[mv.index()])
        .fold(f32::NEG_INFINITY, f32::max);

    let mut sum = 0.0f64; // f64 accumulator guards against 225 tiny exponentials
    let exp_values: Vec<(Move, f64)> = legal
        .iter()
        .map(|&mv| {
            let v = f64::from((logits[mv.index()] - max_logit).exp());
            sum += v;
            (mv, v)
        })
        .collect();

    if sum == 0.0 {
        // Degenerate case: all legal logits were -∞. Fall back to
        // uniform so the search does not divide by zero.
        let uniform = 1.0 / legal.len() as f32;
        return legal.iter().map(|&mv| (mv, uniform)).collect();
    }

    let inv_sum = 1.0 / sum;
    exp_values
        .into_iter()
        .map(|(mv, prob)| (mv, (prob * inv_sum) as f32))
        .collect()
}

/// A test evaluator: uniform policy logits and a constant value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UniformEvaluator {
    /// The constant value returned for every position.
    pub value: f32,
}

impl UniformEvaluator {
    /// Create a uniform evaluator with the given constant value.
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
    fn evaluate(&mut self, _req: EvalRequest) -> EvalResult {
        EvalResult {
            // Uniform logits: every cell gets the same weight before
            // masking, so legality alone determines the policy.
            policy: [0.0f32; 225],
            value: self.value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Board, Move};

    fn all_legal_moves(board: &Board) -> Vec<Move> {
        board.empty_moves().collect()
    }

    #[test]
    fn masked_softmax_sums_to_one() {
        let logits = [0.0f32; 225];
        let legal = all_legal_moves(&Board::new());
        let dist = masked_softmax(&logits, &legal);
        let sum: f32 = dist.iter().map(|(_, p)| p).sum();
        assert!((sum - 1.0).abs() < 1e-5, "sum = {sum}");
        assert_eq!(dist.len(), legal.len());
    }

    #[test]
    fn illegal_moves_get_zero_probability() {
        let mut logits = [0.0f32; 225];
        // Make (0,0) and (0,1) illegal by playing them.
        let mut board = Board::new();
        board.play(Move::new(0, 0).unwrap()).unwrap();
        board.play(Move::new(0, 1).unwrap()).unwrap();

        // Give the illegal cells huge logits.
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
        let sum: f32 = dist.iter().map(|(_, p)| p).sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }

    #[test]
    fn mask_before_normalize_guard() {
        // One legal move with a modest logit; one illegal move with a
        // gigantic logit. The legal move must still receive all mass.
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
```
