# Chapter 03 — Deep-dive solution: selection PUCT

This is the opt-in reference for Chapter 03. `select.rs` is quoted
verbatim from the verified reference crate, including its tests.

## `src/select.rs`

````rust
//! PUCT selection: descend the tree while nodes are expanded.
//!
//! Primer §3 defines the selection criterion:
//!
//! ```text
//! Q(s,a) = W(s,a) / N(s,a)            (0 if N == 0)
//! U(s,a) = c_puct * P(s,a) * sqrt(sum_b N(s,b)) / (1 + N(s,a))
//! a*     = argmax_a [ Q(s,a) + U(s,a) ]
//! ```

use crate::tree::{Edge, NodeId, NodeState, Tree};
use engine::Board;

/// Compute the PUCT score of an edge given its parent's total visits.
///
/// `parent_visits` is `Σ_b N(s,b)` — the sum of visit counts over all
/// edges of the parent node. For an unvisited edge (`edge.n == 0`) the
/// exploration term is at its maximum.
#[must_use]
pub fn puct_score(edge: &Edge, parent_visits: u32, c_puct: f32) -> f32 {
    let q = edge.q();
    let total_visits = parent_visits.max(1) as f32;
    let u = c_puct * edge.prior * total_visits.sqrt() / (1.0 + edge.n as f32);
    q + u
}

/// Result of one selection walk.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    /// The leaf node reached: either unexpanded or terminal.
    pub leaf: NodeId,
    /// A clone of `root_board` with the selected path played on it.
    pub board: Board,
    /// The path taken: `(node_id, edge_index)` for each level descended.
    pub path: Vec<(NodeId, usize)>,
}

/// Descend from the root while nodes are [`Expanded`][NodeState::Expanded],
/// choosing at each step the edge with the highest PUCT score.
///
/// The returned `board` is a clone of `root_board` with every move on
/// `path` already played. Selection stops as soon as it reaches a node
/// that is `Unexpanded` or `Terminal`; that node id is `leaf`.
///
/// v1 clones the board once per simulation (primer §4.1). The
/// play/undo micro-optimization is left for later measurement.
///
/// # Panics
/// Panics only on internal invariants: an expanded node with no edges,
/// or a selected move that the engine rejects as illegal.
#[must_use]
pub fn select(tree: &Tree, root_board: &Board, c_puct: f32) -> Selection {
    let mut board = root_board.clone();
    let mut current = tree.root();
    let mut path = Vec::new();

    while tree.node(current).state() == NodeState::Expanded {
        let node = tree.node(current);
        let edges = node.edges();
        debug_assert!(!edges.is_empty(), "expanded node must have edges");

        let parent_visits = edges.iter().map(|e| e.n).sum();
        let best = edges
            .iter()
            .enumerate()
            .map(|(i, e)| (i, puct_score(e, parent_visits, c_puct)))
            .fold(
                (0, f32::NEG_INFINITY),
                |acc, x| if x.1 > acc.1 { x } else { acc },
            );

        let edge_index = best.0;
        let mv = edges[edge_index].mv;
        path.push((current, edge_index));

        // Play the move on the cloned board. This cannot fail: the
        // edge was created from a legal position, and expand() created
        // the child node eagerly so select() can traverse immutably.
        board.play(mv).expect("selected move must be legal");

        current = edges[edge_index]
            .child
            .expect("expanded edge must have a child");
    }

    Selection {
        leaf: current,
        board,
        path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{Edge, NodeState, Tree};
    use engine::{Board, Move};

    #[test]
    fn puct_worked_example_from_primer() {
        // Primer §3 table: c_puct=1.5, parent N=100.
        let total = 100;
        let a = Edge {
            mv: Move::new(0, 0).unwrap(),
            prior: 0.50,
            n: 60,
            w: 30.0,
            child: None,
        };
        let b = Edge {
            mv: Move::new(0, 1).unwrap(),
            prior: 0.30,
            n: 30,
            w: 12.0,
            child: None,
        };
        let c = Edge {
            mv: Move::new(0, 2).unwrap(),
            prior: 0.20,
            n: 10,
            w: -1.0,
            child: None,
        };

        let score_a = puct_score(&a, total, 1.5);
        let score_b = puct_score(&b, total, 1.5);
        let score_c = puct_score(&c, total, 1.5);

        // Worked values: a ≈ 0.62, b ≈ 0.55, c ≈ 0.17.
        assert!((score_a - 0.62).abs() < 0.01, "a = {score_a}");
        assert!((score_b - 0.55).abs() < 0.01, "b = {score_b}");
        assert!((score_c - 0.17).abs() < 0.01, "c = {score_c}");
        assert!(score_a > score_b);
        assert!(score_b > score_c);
    }

    #[test]
    fn unvisited_high_prior_beats_visited_low_prior_when_q_equal() {
        let total = 10;
        let visited = Edge {
            mv: Move::new(0, 0).unwrap(),
            prior: 0.01,
            n: 10,
            w: 5.0, // Q = 0.5
            child: None,
        };
        let unvisited = Edge {
            mv: Move::new(0, 1).unwrap(),
            prior: 0.99,
            n: 0,
            w: 0.0, // Q = 0.0
            child: None,
        };
        assert!(puct_score(&unvisited, total, 1.5) > puct_score(&visited, total, 1.5));
    }

    #[test]
    fn select_descends_puct_winner_and_records_path() {
        // Root has two children. Edge 0 has higher PUCT; select should
        // descend through it.
        let mut tree = Tree::new();
        tree.node_mut(0).edges_mut().push(Edge {
            mv: Move::new(7, 7).unwrap(),
            prior: 0.9,
            n: 0,
            w: 0.0,
            child: None,
        });
        tree.node_mut(0).edges_mut().push(Edge {
            mv: Move::new(7, 8).unwrap(),
            prior: 0.1,
            n: 0,
            w: 0.0,
            child: None,
        });
        let child = tree.add_child(0, 0);
        *tree.node_mut(0).state_mut() = NodeState::Expanded;
        // Mark the child unexpanded so selection stops there.

        let root = Board::new();
        let sel = select(&tree, &root, 1.5);

        assert_eq!(sel.leaf, child);
        assert_eq!(sel.path, vec![(0, 0)]);
        assert_eq!(sel.board.moves().len(), 1);
        assert_eq!(sel.board.moves()[0], Move::new(7, 7).unwrap());
    }

    #[test]
    fn select_stops_at_unexpanded() {
        let tree = Tree::new();
        let root = Board::new();
        let sel = select(&tree, &root, 1.5);
        assert_eq!(sel.leaf, 0);
        assert_eq!(sel.path.len(), 0);
        assert_eq!(sel.board, root);
    }

    #[test]
    fn select_stops_at_terminal() {
        // Terminal nodes have no edges; select should never enter them.
        let mut tree = Tree::new();
        *tree.node_mut(0).state_mut() = NodeState::Terminal;
        let root = Board::new();
        let sel = select(&tree, &root, 1.5);
        assert_eq!(sel.leaf, 0);
        assert_eq!(sel.path.len(), 0);
    }
}
````
