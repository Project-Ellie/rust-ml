//! Arena tree: one `Vec<Node>` per search, children stored as `u32` indices.
//!
//! The design is from primer §6.3: no `Box`, no `Rc`, no per-node
//! allocation. Edges carry the per-move PUCT statistics `(P, N, W)`
//! and an optional child node id.

/// Node identifier: An index into [`Tree:nodes`]
pub type NodeId = u32;

/// One outgoing edge from a node: the move, the fixed prior, the
/// accumulating statistics, and an optional child.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    /// the move this edge represents
    pub mv: engine::Move,

    /// Prior probability `P(s, a)` assigned when parent was expanded.
    pub prior: f32,

    /// Visit count `N(s, a)`
    pub n: u32,

    /// Cumulative value `W(s, a)` from the perspective of the player
    /// who chose at the parent node.
    pub w: f32,

    /// Child node index, created eagerly during expansion
    pub child: Option<NodeId>,
}

impl Edge {
    /// Mean action value `Q(s, a) = W / N`. By convention an
    /// unvisited edge has `Q = 0.0`, matching PUCT: the exploration
    /// term `U` dominates until visits accumulate.
    ///
    /// # Panics
    /// Never; `n == 0` is the defined zero case.
    #[must_use]
    pub fn q(&self) -> f32 {
        if self.n == 0 {
            0.0
        } else {
            self.w / self.n as f32
        }
    }
}

/// Lifecycle state of a node
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    /// Fresh node, never evaluated, no Edges.
    Unexpanded,

    /// Evaluated and outgoing edges created.
    Expanded,

    /// Game Over.
    Terminal,
}

/// One tree node
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    state: NodeState,
    edges: Vec<Edge>,
}

impl Node {
    /// Create a new unexpanded Node with no edges
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: NodeState::Unexpanded,
            edges: Vec::new(),
        }
    }

    /// Current lifecycle state.
    #[must_use]
    pub fn state(&self) -> NodeState {
        self.state
    }

    /// Outgoing edges in creation order
    #[must_use]
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Mutable outg
    #[must_use]
    pub fn edges_mut(&mut self) -> &mut Vec<Edge> {
        &mut self.edges
    }
}
impl Default for Node {
    fn default() -> Self {
        Self::new()
    }
}

/// Arena tree: all nodes live in one contiguous vector
#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    /// Create a tree containing the first unexpanded root node (id 0)
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: vec![Node::new()],
        }
    }

    /// the root node is always 0
    #[must_use]
    pub fn root(&self) -> NodeId {
        0
    }

    /// Immutable view of the node
    #[must_use]
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    /// Mutable view of the node
    #[must_use]
    pub fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id as usize]
    }

    /// Append a new unexpanded child node and attach it to the given
    /// edge of `parent`.
    ///
    /// # Panics
    /// Panics if `edge_index` is out of range for the parent.
    #[must_use]
    pub fn add_child(&mut self, parent: NodeId, edge_index: usize) -> NodeId {
        let child_id = self.nodes.len() as NodeId;
        self.nodes.push(Node::new());
        self.nodes[parent as usize].edges_mut()[edge_index].child = Some(child_id);
        child_id
    }
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Move;

    #[test]
    fn new_tree_has_root_zero() {
        let tree = Tree::new();
        assert_eq!(tree.root(), 0);
        assert_eq!(tree.node(tree.root()).state(), NodeState::Unexpanded);
        assert_eq!(tree.node(tree.root()).edges.len(), 0);
    }

    #[test]
    fn arena_grows_and_indices_stay_stable() {
        let mut tree = Tree::new();
        tree.node_mut(0).edges_mut().push(Edge {
            mv: Move::new(0, 0).unwrap(),
            prior: 0.5,
            n: 0,
            w: 0.0,
            child: None,
        });

        tree.node_mut(0).edges_mut().push(Edge {
            mv: Move::new(0, 1).unwrap(),
            prior: 0.5,
            n: 0,
            w: 0.0,
            child: None,
        });

        let a = tree.add_child(0, 0);
        let b = tree.add_child(0, 1);

        tree.node_mut(a).edges_mut().push(Edge {
            mv: Move::new(1, 0).unwrap(),
            prior: 0.6,
            n: 0,
            w: 0.0,
            child: None,
        });

        let a0 = tree.add_child(a, 0);
        assert_eq!(a, 1);
        assert_eq!(b, 2);
        assert_eq!(a0, 3);

        tree.node_mut(b).edges_mut().push(Edge {
            mv: Move::new(0, 2).unwrap(),
            prior: 0.5,
            n: 0,
            w: 0.0,
            child: None,
        });

        let _ = tree.add_child(b, 0);
        assert_eq!(tree.node(a).edges[0].child, Some(a0));
    }

    #[test]
    fn edges_attach_to_the_right_parent() {
        let mut tree = Tree::new();
        tree.node_mut(0).edges_mut().push(Edge {
            mv: Move::new(0, 0).unwrap(),
            prior: 0.5,
            n: 0,
            w: 0.0,
            child: None,
        });
        let child = tree.add_child(0, 0);
        assert_eq!(tree.node(0).edges()[0].child, Some(child));
        assert_eq!(tree.node(child).edges().len(), 0);
    }

    #[test]
    fn q_is_zero_when_unvisited() {
        let edge = Edge {
            mv: Move::new(7, 7).unwrap(),
            prior: 0.5,
            n: 0,
            w: 42.0,
            child: None,
        };
        assert!(edge.q().abs() < 1e-6);
    }

    #[test]
    fn q_is_mean_after_visits() {
        let edge = Edge {
            mv: Move::new(6, 4).unwrap(),
            prior: 0.5,
            n: 8,
            w: 2.4,
            child: None,
        };
        assert!((edge.q() - 0.3).abs() < 1e-6);
    }
}
