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
    use crate::TacticsEvaluator;
    use crate::eval::UniformEvaluator;
    use engine::Color;
    use engine::reference::board_from_ascii;

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

    #[test]
    fn immediate_win_for_side_to_move_gets_most_visits() {
        // Black has an open four on row 7; (7,3) or (7,8) wins.
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
            .expect("Root has edges");
        let wins = engine::immediate_wins(&b, b.to_move());
        assert!(
            wins.contains(best.mv),
            "Best move {:?} must be a winning move",
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
            "Best move {:?} must be a block.",
            best.mv
        );
    }
}
