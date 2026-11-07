//! Checked, owned history records
//!
use super::{History, PlayOutcome};
use engine::{Board, Color, Move};
use serde::{Deserialize, Serialize};

pub(crate) const MAX_NODES: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RootRecord {
    pub black: Vec<[u8; 2]>,
    pub white: Vec<[u8; 2]>,
    pub to_move: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeRecord {
    pub parent: Option<u32>,
    pub square: Option<[u8; 2]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryRecord {
    pub root: RootRecord,
    pub nodes: Vec<NodeRecord>,
    pub cursor: u32,
}

fn checked_square([row, col]: [u8; 2]) -> Result<Move, String> {
    Move::new(row, col).ok_or_else(|| format!("square out of bounds: {}", col))
}

fn checked_stones (squares: &[[u8; 2]], occupied: &mut [bool; 225]) -> Result<Vec<Move>, String> {
    squares.iter().map(|&square| {
        let mv = checked_square(square)?;
        if std::mem::replace(&mut occupied[mv.index()], true) {
            return Err("duplicate or overlapping root cell".into())
        }
        Ok(mv)
    }).collect()
}

impl RootRecord {
    pub(crate) fn from_board(board: &Board) -> Self {
        let mut black = Vec::new();
        let mut white = Vec::new();

        for row in 0..15 {
            for col in 0..15 {
                let mv = Move::new(row, col).expect("bounded coordinates");
                match board.stone_at(mv) {
                    Some(Color::Black) => black.push([row, col]),
                    Some(Color::White) => white.push([row, col]),
                    None => {}
                }
            }
        }
        Self { black, white, to_move: board.to_move() }
    }

    pub (crate) fn to_board(&self) -> Result<Board, String> {
        if self.black.len() + self.white.len() > 225 {
            return Err("too many stones".into());
        }
        let mut occupied = [false; 225];
        let black = checked_stones(&self.black, &mut occupied)?;
        let white = checked_stones(&self.white, &mut occupied)?;
        let board = Board::from_position(&black, &white, self.to_move)
            .map_err(|e| format!("invalid root: {e}"))?;
        if board.has_five_any(Color::Black) && board.has_five_any(Color::White) {
            return Err("both colors win at root".into());
        }
        Ok(board)
    }
}

impl HistoryRecord {
    pub(crate) fn capture(history: &History) -> Self {
        Self {
            root: RootRecord::from_board(&history.root_board),
            nodes: history.nodes.iter().map(|node| NodeRecord {
                parent:  node.parent,
                square: node.mv.map(|mv| [mv.row(), mv.col()]),
            }).collect(),
            cursor: history.cursor,
        }

    }

    pub(crate) fn restore(&self) -> Result<History, String> {
        if self.nodes.is_empty() || self.nodes.len() > MAX_NODES {
            return Err("history must contain 1..=10000 nodes".into());
        }
        if self.cursor as usize >= self.nodes.len() {
            return Err("cursor outside history".into());
        }
        if self.nodes[0].parent.is_some() || self.nodes[0].square.is_some() {
            return Err("root node must have null parent and square".into());
        }
        let mut history = History::from_opening(&self.root.to_board()?);
        for (index, node) in self.nodes.iter().enumerate().skip(1) {
            let parent = node.parent.ok_or("non-root node needs parent")?;
            if parent as usize >= index {
                return Err(format!("node {index}: parent must precede child"));
            }
            let mv = checked_square(node.square.ok_or("non-root node needs square")?)?;
            // Child module access is intentional; never navigate untrusted IDs via var().
            history.cursor = parent;
            let outcome = history.play(mv)
                .map_err(|e| format!("node {index}: {e}"))?;
            if outcome != PlayOutcome::Moved(index as u32) {
                return Err(format!("node {index}: duplicate sibling move"));
            }
        }
        history.cursor = self.cursor;
        Ok(history)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branches_ids_sibling_order_and_cursor_round_trip() {
        let mut history = History::new();
        history.play(Move::new(7, 7).unwrap()).unwrap();
        history.play(Move::new(7, 8).unwrap()).unwrap();
        history.back(1);
        history.play(Move::new(8, 8).unwrap()).unwrap();
        history.back(1);

        let record = HistoryRecord::capture(&history);
        let restored = record.restore().unwrap();

        assert_eq!(restored, history);
        assert_eq!(restored.cursor, 1);
        assert_eq!(restored.children(1), vec![
            Move::new(7, 8).unwrap(), Move::new(8, 8).unwrap()
        ]);
        let mut bad_cursor = record.clone();
        bad_cursor.cursor = bad_cursor.nodes.len() as u32;
        assert!(bad_cursor.restore().is_err());
        let mut duplicate = record;
        duplicate.nodes[3].square = duplicate.nodes[2].square;
        assert!(duplicate.restore().is_err());
    }

    #[test]
    fn illegal_hidden_branch_is_checked_even_when_cursor_is_root() {
        let mut history = History::new();
        history.play(Move::new(7, 7).unwrap()).unwrap();
        history.play(Move::new(7, 8).unwrap()).unwrap();
        history.back(2);
        let mut record = HistoryRecord::capture(&history);
        record.nodes[2].square = Some([7, 7]);
        assert_eq!(record.cursor, 0);
        assert!(record.restore().is_err());
        record.nodes[2].square = Some([15, 0]);
        assert!(record.restore().is_err());
        record.nodes[2].square = Some([0, 255]);
        assert!(record.restore().is_err());
    }
}

