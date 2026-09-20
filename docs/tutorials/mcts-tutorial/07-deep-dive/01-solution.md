# Chapter 07 solution — `src/search.rs`

This file contains the simulation loop and the search entry point.
It is extracted verbatim from the reference implementation, except
that the two `TacticsEvaluator` sentinel tests are omitted here; they
appear in chapter 09's solution because they depend on the evaluator
built in that chapter.

## Implementation

```rust
//! Top-level search: run `simulations` select/expand/backup steps.
//!
//! Each simulation produces exactly one new evaluation (unless it
//! lands on a terminal node). The tree is discarded after the search;
//! tree reuse is a future optimization supported by the arena layout.

use crate::eval::Evaluator;
use crate::expand::expand;
use crate::select::select;
use crate::tree::{NodeId, NodeState, Tree};
use engine::{Board, Status};

/// Configuration for one search.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SearchConfig {
    /// Number of simulations to run.
    pub simulations: u32,
    /// PUCT exploration constant.
    pub c_puct: f32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            simulations: 400,
            c_puct: 1.5,
        }
    }
}

/// Output of one search: the grown tree and its root id.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchOutcome {
    /// The search tree.
    pub tree: Tree,
    /// The root node id (always 0 for a fresh tree).
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

    // Pre-expand the root so that every simulation descends through
    // at least one edge and contributes one visit to the root. Without
    // this, the first simulation would expand the root with an empty
    // path and no root edge would receive a visit.
    let _root_value = expand(&mut tree, root, root_board, evaluator);

    for _ in 0..config.simulations {
        let selection = select(&tree, root_board, config.c_puct);

        let leaf_value = match tree.node(selection.leaf).state() {
            NodeState::Terminal => terminal_value(&selection.board),
            NodeState::Unexpanded => expand(&mut tree, selection.leaf, &selection.board, evaluator),
            // If select somehow stopped at an already-expanded node
            // (should not happen unless the node has no edges), treat
            // it as a draw to keep the loop moving.
            NodeState::Expanded => 0.0,
        };

        crate::backup::backup(&mut tree, &selection.path, leaf_value);
    }

    SearchOutcome {
        root: tree.root(),
        tree,
    }
}

/// Exact value of a terminal board from the side-to-move perspective.
fn terminal_value(board: &Board) -> f32 {
    match board.status() {
        Status::Won(_) => -1.0,
        // Draw and the defensive Ongoing guard both mean no decisive
        // advantage from the side-to-move perspective.
        Status::Draw | Status::Ongoing => 0.0,
    }
}
```

## Tests for this chapter

The only test that belongs in chapter 07 is the visit-count invariant.
Add this inside `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::UniformEvaluator;

    #[test]
    fn uniform_search_visits_sum_to_simulations() {
        let board = Board::new();
        let mut ev = UniformEvaluator::new(0.0);
        let outcome = search(
            &board,
            &mut ev,
            &SearchConfig {
                simulations: 50,
                c_puct: 1.5,
            },
        );
        let root_edges = outcome.tree.node(outcome.root).edges();
        let total: u32 = root_edges.iter().map(|e| e.n).sum();
        assert_eq!(total, 50);
    }
}
```

## What is deferred to chapter 09

The reference `search.rs` also contains these two tests, which use
`crate::mock::TacticsEvaluator`:

* `immediate_win_for_side_to_move_gets_most_visits`
* `opponent_immediate_win_forces_block`

They are the tactical sentinels for the sign convention. Do not add
them until you have built `TacticsEvaluator` in chapter 09; they will
not compile before then.
