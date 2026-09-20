# Chapter 06 deep dive — Backup: the sign convention

This is the opt-in reference solution for chapter 06. It is a verbatim copy of `src/backup.rs` from the verified reference crate (`/tmp/mcts-reference/src/backup.rs`), included so you can compare your implementation line by line after you have tried the step yourself.

The code compiles and the tests pass against the reference tree implementation.

```rust
//! Backup: propagate the leaf value up the selection path.
//!
//! Primer §4.4: the value returned by `expand` is from the
//! perspective of the player **to move at the leaf**. Each edge on
//! the path belongs to the player who chose at that node, so the
//! value flips sign at every level during the ascent.

use crate::tree::{NodeId, Tree};

/// Back up `leaf_value` along `path`, flipping sign at each step.
///
/// `path` is ordered root-to-leaf: each pair is `(node_id,
/// edge_index)` for the edge chosen when descending from that node.
/// We walk it in reverse, adding `±leaf_value` to each edge's `W`
/// and incrementing `N`. The sign alternates so that the edge at the
/// leaf's parent receives `-leaf_value` (what is good for the leaf
/// side-to-move is bad for the player who chose at the parent).
///
/// # Panics
/// Panics only on corrupted paths (out-of-range ids/indices).
pub fn backup(tree: &mut Tree, path: &[(NodeId, usize)], leaf_value: f32) {
    // Start with the value for the player who chose at the leaf's
    // parent. Because the leaf value is from the leaf side-to-move's
    // perspective, the parent player (who just moved to reach the
    // leaf) sees the opposite.
    let mut value = -leaf_value;

    for &(node_id, edge_index) in path.iter().rev() {
        let edge = &mut tree.node_mut(node_id).edges_mut()[edge_index];
        edge.n += 1;
        edge.w += value;
        value = -value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{Edge, NodeState, Tree};
    use engine::Move;

    fn make_edge(mv: (u8, u8), prior: f32, n: u32, w: f32) -> Edge {
        Edge {
            mv: Move::new(mv.0, mv.1).unwrap(),
            prior,
            n,
            w,
            child: None,
        }
    }

    #[test]
    fn backup_sign_convention() {
        // Hand-built path: root -> child -> leaf. Leaf value = +0.8
        // for side to move at leaf.
        //
        // Primer §4.4: value flips sign at every level. The edge at
        // the leaf's parent (child -> leaf) belongs to the player who
        // just moved to reach the leaf, so it receives -0.8. The edge
        // at the root belongs to the player two plies earlier, so it
        // receives +0.8.
        let mut tree = Tree::new();
        *tree.node_mut(0).state_mut() = NodeState::Expanded;
        tree.node_mut(0)
            .edges_mut()
            .push(make_edge((7, 7), 1.0, 0, 0.0));

        let child = tree.add_child(0, 0);
        *tree.node_mut(child).state_mut() = NodeState::Expanded;
        tree.node_mut(child)
            .edges_mut()
            .push(make_edge((7, 8), 1.0, 0, 0.0));

        let path = vec![(0, 0), (child, 0)];
        backup(&mut tree, &path, 0.8);

        assert_eq!(tree.node(0).edges()[0].n, 1);
        assert!((tree.node(0).edges()[0].w - 0.8).abs() < 1e-6);
        assert!((tree.node(0).edges()[0].q() - 0.8).abs() < 1e-6);

        assert_eq!(tree.node(child).edges()[0].n, 1);
        assert!((tree.node(child).edges()[0].w + 0.8).abs() < 1e-6);
        assert!((tree.node(child).edges()[0].q() + 0.8).abs() < 1e-6);
    }

    #[test]
    fn backup_increments_visits_along_path() {
        let mut tree = Tree::new();
        *tree.node_mut(0).state_mut() = NodeState::Expanded;
        tree.node_mut(0)
            .edges_mut()
            .push(make_edge((7, 7), 1.0, 5, 2.0));
        let child = tree.add_child(0, 0);
        *tree.node_mut(child).state_mut() = NodeState::Expanded;
        tree.node_mut(child)
            .edges_mut()
            .push(make_edge((7, 8), 1.0, 0, 0.0));

        backup(&mut tree, &[(0, 0), (child, 0)], 0.5);
        assert_eq!(tree.node(0).edges()[0].n, 6);
        assert!((tree.node(0).edges()[0].w - 2.5).abs() < 1e-6); // 2.0 + 0.5 (flip twice)
    }

    #[test]
    fn backup_on_empty_path_is_noop() {
        let mut tree = Tree::new();
        backup(&mut tree, &[], 0.5);
        assert_eq!(tree.node(0).edges().len(), 0);
    }
}
```
