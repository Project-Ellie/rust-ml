//! Variation tree, stored as a flat node vector (the mcts::Tree idiom).
//!
//! A persistent game history: every position reached stays reachable.
//! Playing a move at an old position adds a child branch without deleting
//! siblings. Navigation is explicit: `back`/`forward` walk the cursor,
//! `var` chooses at branch points.
//!
//! Implementation follows the in-repo arena idiom used by `mcts::Tree`:
//! one `Vec<Node>` per history, children stored as `NodeId` indices. The
//! root node stores the full opening board; every other node stores only
//! the move that led to it. Replaying `board()` recomputes the cursor
//! position from the root each call (≤225 plies).

use engine::{Board, Move, PlayError};
use thiserror::Error;

pub type NodeId = u32;

/// One Node in the varation tree.
#[derive(Debug, Clone, PartialEq)]
struct Node {
    // Parent node: None only at the root
    parent: Option<NodeId>,
    // Move from parent to this node, None only at the root
    mv: Option<Move>,
    //Child nodes in creation order
    children: Vec<NodeId>,
}

/// Persistent varation tree with a cursor
#[derive(Debug, Clone, PartialEq)]
pub struct History {
    nodes: Vec<Node>,
    root: NodeId,
    cursor: NodeId,
    root_board: Board,
}

/// Result of History::play
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayOutcome {
    /// a new child was appended
    Moved(NodeId),
    /// the move matched an existing child
    FollowedExisting(NodeId),
}

/// Result of History::forward
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForwardOutcome {
    /// cursor moved that many plies
    Moved(u32),
    /// The cursor stopped at a branch point
    NeedsChoice(Vec<Move>),
    /// The cursor stopped at a leaf
    AtLeaf,
}

/// Errors returned by history mutation commands
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HistoryError {
    /// The move is illegal in the cursor position
    #[error("Illegal move: {0}")]
    IllegalMove(#[from] PlayError),
    /// The requested child index does not exist at the cursor.
    #[error("Variation index {index} out of bound (children: {child_count}")]
    VarOutOfBounds {
        /// Index that was requested.
        index: usize,
        /// Number of children at the cursor.
        child_count: usize,
    },
}

impl History {
    /// Create a history rooted at an empty board with black to move.
    #[must_use]
    pub fn new() -> Self {
        Self::from_opening(&Board::new())
    }

    /// Create a history rooted at an arbitrary opening position.
    ///
    /// The opening board is cloned and stored at the root; subsequent play
    /// continues from that position with the correct side to move.
    #[must_use]
    pub fn from_opening(board: &Board) -> Self {
        Self {
            nodes: vec![Node {
                parent: None,
                mv: None,
                children: Vec::new(),
            }],
            root: 0,
            cursor: 0,
            root_board: board.clone(),
        }
    }

    /// The current cursor node id.
    #[must_use]
    pub fn cursor(&self) -> NodeId {
        self.cursor
    }

    /// The board at the cursor, recomputed by replaying the root→cursor path.
    #[must_use]
    pub fn board(&self) -> Board {
        let mut board = self.root_board.clone();
        for &mv in &self.path_to_cursor() {
            // Moves are validated on insertion, so replay is safe by construction.
            board.play(mv).expect("Replayed cursor path must be legal.")
        }
        board
    }

    /// Play a move at the cursor.
    ///
    /// * If the move matches an existing child, the cursor moves there and
    ///   [`PlayOutcome::FollowedExisting`] is returned.
    /// * Otherwise a new child is appended and [`PlayOutcome::Moved`] is returned.
    ///
    /// # Errors
    /// Returns [`HistoryError::IllegalMove`] if the move is illegal at the
    /// cursor (occupied cell or game already over). User input must never
    /// cause a panic.
    pub fn play(&mut self, mv: Move) -> Result<PlayOutcome, HistoryError> {
        // Validate the move before touching the tree.
        let mut probe = self.root_board.clone();
        for &m in &self.path_to_cursor() {
            probe.play(m).expect("Replayed cursor path must be legal.");
        }
        probe.play(mv)?;

        // Follow an identical child if one exists.
        for &child_id in &self.nodes[self.cursor as usize].children {
            if self.nodes[child_id as usize].mv == Some(mv) {
                self.cursor = child_id;
                return Ok(PlayOutcome::FollowedExisting(child_id));
            }
        }

        // Append a new child node.
        let new_id = self.nodes.len() as NodeId;
        self.nodes.push(Node {
            parent: Some(self.cursor),
            mv: Some(mv),
            children: Vec::new(),
        });
        self.nodes[self.cursor as usize].children.push(new_id);
        self.cursor = new_id;
        Ok(PlayOutcome::Moved(new_id))
    }

    /// Move the cursor toward the root by up to `n` plies.
    ///
    /// Returns the number of plies actually moved (clamped at the root).
    pub fn back(&mut self, n: u32) -> u32 {
        let mut moved = 0;
        for _ in 0..n {
            match self.nodes[self.cursor as usize].parent {
                Some(parent) => {
                    self.cursor = parent;
                    moved += 1;
                }
                None => break,
            }
        }
        moved
    }

    /// Move the cursor forward by up to `n` plies.
    ///
    /// * At a leaf, returns [`ForwardOutcome::AtLeaf`].
    /// * At a branch point (multiple children), returns
    ///   [`ForwardOutcome::NeedsChoice`] with the child moves.
    /// * Along a single-child line, follows as far as possible and returns
    ///   [`ForwardOutcome::Moved`] with the plies actually moved.
    pub fn forward(&mut self, n: u32) -> ForwardOutcome {
        let mut moved = 0;
        for _ in 0..n {
            let children = &self.nodes[self.cursor as usize].children;
            match children.len() {
                0 => return ForwardOutcome::AtLeaf,
                1 => {
                    self.cursor = children[0];
                    moved += 1;
                }
                _ => {
                    return ForwardOutcome::NeedsChoice(
                        children
                            .iter()
                            .map(|&id| self.nodes[id as usize].mv.expect("child has a move"))
                            .collect(),
                    );
                }
            }
        }
        ForwardOutcome::Moved(moved)
    }

    /// Choose a child variation by index at the cursor.
    ///
    /// # Errors
    /// Returns [`HistoryError::VarOutOfBounds`] if `index` is not a valid
    /// child index.
    pub fn var(&mut self, index: usize) -> Result<Move, HistoryError> {
        let children = &self.nodes[self.cursor as usize].children;
        if index > children.len() {
            return Err(HistoryError::VarOutOfBounds {
                index,
                child_count: children.len(),
            });
        }
        let child_id = children[index];
        self.cursor = child_id;
        Ok(self.nodes[child_id as usize].mv.expect("child has a move"))
    }

    /// The child moves of `at`, in creation order.
    pub fn children(&self, at: NodeId) -> Vec<Move> {
        self.nodes[at as usize]
            .children
            .iter()
            .map(|&id| self.nodes[id as usize].mv.expect("child has a move"))
            .collect()
    }

    /// the cursor depth in plies from the root.
    #[must_use]
    pub fn ply(&self) -> u32 {
        let mut depth = 0;
        let mut id = self.cursor;
        while let Some(parent) = self.nodes[id as usize].parent {
            depth += 1;
            id = parent;
        }
        depth
    }

    /// Collect the moves from the root to the cursor.
    fn path_to_cursor(&self) -> Vec<Move> {
        let mut path = Vec::new();
        let mut id = self.cursor;
        while let Some(parent) = self.nodes[id as usize].parent {
            path.push(self.nodes[id as usize].mv.expect("non-root has a move"));
            id = parent;
        }

        path.reverse();
        path
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}
