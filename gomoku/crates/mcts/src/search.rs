//! Top-level search: run `simulations` select/expand/backup steps.
//!
//! Each simulation produces exactly one new evaluation (unless it
//! lands on a terminal node). The tree is discarded after the search;
//! tree reuse is a future optimization supported by the arena layout.

use crate::backup::backup;
use crate::eval::Evaluator;
use crate::expand::expand;
use crate::select::select;
use crate::tree::{NodeId, NodeState, Tree};
use engine::{Board, Status};

/// search configuration with default
/// number of simulations and `c_puct`
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SearchConfig {
    /// Number of simulations per search
    pub simulations: u32,

    /// puct constant
    pub c_puct: f32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            // an experimental tuning knob; AlphaZero used 800, but v1 favors self-play throughput while the network is weak
            simulations: 400,
            // ELF OpenGo's published value — DeepMind never published c_puct for AlphaZero
            c_puct: 1.5,
        }
    }
}

/// The outcome of a single search
#[derive(Debug, Clone, PartialEq)]
pub struct SearchOutcome {
    /// The whole tree after the search
    pub tree: Tree,

    /// The starting node
    pub root: NodeId,
}

/// Run MCTS from `root_board` using `evaluator` and `config`.
///
/// Returns the full tree so callers can extract visit counts, priors,
/// or reuse the subtree later.
pub fn search(
    root_board: &Board,
    evaluator: &mut dyn Evaluator,
    config: &SearchConfig,
) -> SearchOutcome {
    let mut tree = Tree::new();
    let root = tree.root();

    let _ = expand(&mut tree, root, root_board, evaluator);

    for _ in 0..config.simulations {
        let selection = select(&tree, root_board, config.c_puct);
        let leaf_value = match tree.node(selection.leaf).state() {
            NodeState::Terminal => terminal_value(&selection.board),
            NodeState::Unexpanded => expand(&mut tree, selection.leaf, &selection.board, evaluator),
            NodeState::Expanded => 0.0,
        };
        backup(&mut tree, &selection.path, leaf_value);
    }

    SearchOutcome { tree, root }
}

fn terminal_value(board: &Board) -> f32 {
    match board.status() {
        Status::Won(_) => -1.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::UniformEvaluator;

    #[test]
    fn uniform_search_visits_sum_to_simulations() {
        let board = Board::new();
        let mut evaluator = UniformEvaluator::new(0.7);
        let config = SearchConfig {
            simulations: 50,
            c_puct: 1.5,
        };

        let outcome = search(&board, &mut evaluator, &config);

        let root_edges = outcome.tree.node(outcome.root).edges();
        let total: u32 = root_edges.iter().map(|e| e.n).sum();
        assert_eq!(config.simulations, total);
    }
}
