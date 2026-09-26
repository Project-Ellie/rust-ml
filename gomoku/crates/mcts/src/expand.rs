//! Leaf expansion: terminal check first, then one network evaluation.
//!
//! Primer §§4.2–4.3: before any network work, ask the engine whether
//! the leaf is already won or drawn. Terminals get exact values and
//! no edges; ongoing positions are encoded, evaluated, masked-softmaxed,
//! and converted into edges.

use crate::eval::{EvalRequest, Evaluator, masked_softmax};
use crate::tree::{Edge, NodeId, NodeState, Tree};
use engine::{Board, Status};

/// Expand `leaf` in `tree` using `board` (the position at that leaf)
/// and the evaluator.
///
/// Returns the leaf's value from the perspective of the side to move
/// at that leaf:
///
/// * `Status::Won(_) -> -1.0` (the player to move has already lost)
/// * `Status::Draw -> 0.0`
/// * `Status::Ongoing -> evaluator.value`
///
/// The node state is updated to `Terminal` for game-over positions or
/// `Expanded` for ongoing positions.
///
/// # Panics
/// Panics only on internal invariants (e.g. evaluator returning an
/// illegal distribution). The evaluator is never called for terminals.
pub fn expand(tree: &mut Tree, leaf: NodeId, board: &Board, evaluator: &mut dyn Evaluator) -> f32 {
    match board.status() {
        Status::Won(_) => {
            *tree.node_mut(leaf).state_mut() = NodeState::Terminal;
            -1.0
        }
        Status::Draw => {
            *tree.node_mut(leaf).state_mut() = NodeState::Terminal;
            0.0
        }
        Status::Ongoing => {
            if tree.node(leaf).state() == NodeState::Expanded {
                return 0.0;
            }

            let legal: Vec<_> = board.empty_moves().collect();
            let planes = engine::encode(board);
            let result = evaluator.evaluate(EvalRequest {
                planes,
                legal: legal.clone(),
            });
            let dist = masked_softmax(&result.policy, &legal);

            let mut sum = 0.0f32;
            for (mv, prob) in dist {
                let edge_index = tree.node_mut(leaf).edges_mut().len();
                tree.node_mut(leaf).edges_mut().push(Edge {
                    mv,
                    prior: prob,
                    n: 0,
                    w: 0.0,
                    child: None,
                });
                let _child_id = tree.add_child(leaf, edge_index);
                sum += prob;
            }
            debug_assert!((sum - 1.0).abs() < 1e-4, "Priors must sum to 1, got {sum}");

            *tree.node_mut(leaf).state_mut() = NodeState::Expanded;
            result.value.clamp(-1.0, 1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{EvalRequest, EvalResult, Evaluator};
    use crate::tree::{Edge, NodeState, Tree};
    use engine::{Board, Color, Move, Status};

    struct PanicEvaluator;
    impl Evaluator for PanicEvaluator {
        fn evaluate(&mut self, _req: EvalRequest) -> EvalResult {
            panic!("Evaluator must not be called on terminal leaf");
        }
    }

    struct CountingEvaluator<'a> {
        inner: &'a mut dyn Evaluator,
        calls: std::cell::Cell<usize>,
    }

    impl Evaluator for CountingEvaluator<'_> {
        fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
            self.calls.set(self.calls.get() + 1);
            self.inner.evaluate(req)
        }
    }

    #[test]
    fn terminal_won_leaf_has_no_edges_and_returns_minus_one() {
        let mut tree = Tree::new();
        let mut board = Board::new();
        // Black wins on row 7.
        let script = [
            (7, 3),
            (0, 0),
            (7, 4),
            (0, 2),
            (7, 5),
            (0, 4),
            (7, 6),
            (0, 6),
            (7, 7),
        ];
        for &(r, c) in &script {
            board.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(board.status(), Status::Won(Color::Black));

        let mut panic = PanicEvaluator;
        let v = expand(&mut tree, 0, &board, &mut panic);
        assert!((v + 1.0).abs() < 1e-6);
        assert_eq!(tree.node(0).state(), NodeState::Terminal);
        assert_eq!(tree.node(0).edges().len(), 0);
    }

    #[test]
    fn terminal_draw_leaf_returns_zero() {
        let mut tree = Tree::new();
        let mut board = Board::new();
        // Fill the board with the no-five stripe pattern.
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
        assert_eq!(board.status(), Status::Draw);

        let mut panic = PanicEvaluator;
        let v = expand(&mut tree, 0, &board, &mut panic);
        assert!(v.abs() < 1e-6);
        assert_eq!(tree.node(0).state(), NodeState::Terminal);
        assert_eq!(tree.node(0).edges().len(), 0);
    }

    #[test]
    fn ongoing_leaf_gets_edges_summing_to_one() {
        let mut tree = Tree::new();
        let board = Board::new();
        let mut uniform = crate::eval::UniformEvaluator::new(0.1);
        let mut counting = CountingEvaluator {
            inner: &mut uniform,
            calls: std::cell::Cell::new(0),
        };
        let v = expand(&mut tree, 0, &board, &mut counting);
        assert!((v - 0.1).abs() < 1e-6);
        assert_eq!(counting.calls.get(), 1);
        assert_eq!(tree.node(0).state(), NodeState::Expanded);
        let edges = tree.node(0).edges();
        assert_eq!(edges.len(), 225);
        let sum: f32 = edges.iter().map(|e: &Edge| e.prior).sum();
        assert!((sum - 1.0).abs() < 1e-5);
        assert!(
            edges
                .iter()
                .all(|e| e.n == 0 && e.w == 0.0 && e.child.is_some())
        );
    }

    #[test]
    fn reexpansion_is_safe_and_does_not_reevaluate() {
        let mut tree = Tree::new();
        let board = Board::new();
        let mut uniform = crate::eval::UniformEvaluator::new(0.1);
        let mut counting = CountingEvaluator {
            inner: &mut uniform,
            calls: std::cell::Cell::new(0),
        };

        let first = expand(&mut tree, 0, &board, &mut counting);
        let second = expand(&mut tree, 0, &board, &mut counting);

        assert_eq!(counting.calls.get(), 1, "evaluator must run only once");
        assert!(second.abs() < 1e-6, "re-expansion returns the safe default");
        assert!(
            (first - 0.1).abs() < 1e-6,
            "first expansion returns the value"
        );
        assert_eq!(tree.node(0).state(), NodeState::Expanded);
        assert_eq!(
            tree.node(0).edges().len(),
            225,
            "re-expansion must not duplicate edges"
        );
    }
}
