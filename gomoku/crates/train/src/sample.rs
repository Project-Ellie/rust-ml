//! The labelled training record.
//!
//! A [`Sample`] stores a position as two stone lists plus the side to move.
//! Keeping the position in engine vocabulary, rather than as neural-network
//! planes, lets the reader apply augmentation and derive planes on load.

use engine::Color::{Black, White};
use engine::{Board, Color, Move, PositionError};
use std::collections::HashMap;

/// One labelled training position. Stores stones, never planes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Sample {
    /// Black stones on the board, in absolute color.
    pub black: Vec<Move>,
    /// White stones on the board, in absolute color.
    pub white: Vec<Move>,
    /// The side to move in this position.
    pub to_move: Color,
    /// Sparse policy target over legal moves: `(move, probability)`.
    pub policy: Vec<(Move, f32)>,
    /// Value target from the side-to-move perspective.
    pub value: f32,
}
impl Sample {
    /// Builds a sample from a move `history` truncated to `ply` moves.
    ///
    /// Stones are split by alternation (Black moves first) and `to_move`
    /// follows from parity: it is Black exactly when `ply` is even.
    ///
    /// # Panics
    ///
    /// Panics if `ply` exceeds `history.len()`.
    pub fn from_position(
        history: &[Move],
        ply: usize,
        policy: Vec<(Move, f32)>,
        value: f32,
    ) -> Self {
        assert!(
            ply <= history.len(),
            "ply {ply} exceeds history length {}",
            history.len()
        );
        let black = vec![];
        let white = vec![];
        let mut moves: HashMap<Color, Vec<Move>> = HashMap::from([(Black, black), (White, white)]);

        let mut to_move = Color::Black;
        for &mv in history.iter().take(ply) {
            moves.get_mut(&to_move).unwrap().push(mv);
            to_move = to_move.other();
        }

        Self {
            black: moves.get(&Black).unwrap().to_vec(),
            white: moves.get(&White).unwrap().to_vec(),
            to_move,
            policy,
            value,
        }
    }

    /// Rebuilds the engine [`Board`] from the stored stones.
    ///
    /// Returns a [`PositionError`] if the stored lists overlap or the
    /// counts are inconsistent with alternating play.
    pub fn board(&self) -> Result<Board, PositionError> {
        let black = self.black.as_slice();
        let white = self.white.as_slice();
        Board::from_position(black, white, self.to_move)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }
    #[test]
    fn sample_roundtrips_through_bincode() {
        let sample = Sample {
            black: vec![mv(0, 1), mv(1, 0), mv(2, 2)],
            white: vec![mv(1, 1), mv(1, 2)],
            to_move: Black,
            policy: vec![
                (mv(0, 0), 0.0),
                (mv(2, 1), 0.4),
                (mv(1, 3), 0.2),
                (mv(2, 3), 0.5),
            ],
            value: -0.3,
        };
        let config = bincode::config::standard();
        let bytes = bincode::serde::encode_to_vec(&sample, config).unwrap();
        let (decoded, _): (Sample, usize) =
            bincode::serde::decode_from_slice(&bytes, config).unwrap();
        assert_eq!(sample, decoded);
    }

    #[test]
    fn from_position_splits_by_alternation() {
        let history = [mv(1, 0), mv(2, 0), mv(3, 0), mv(4, 0)];

        let sample = Sample::from_position(&history, 4, vec![], 0.6);
        assert_eq!(sample.black, [mv(1, 0), mv(3, 0)]);
        assert_eq!(sample.white, [mv(2, 0), mv(4, 0)]);
        assert_eq!(sample.to_move, Color::Black);

        let sample = Sample::from_position(&history, 3, vec![], 0.6);
        assert_eq!(sample.black, [mv(1, 0), mv(3, 0)]);
        assert_eq!(sample.white, [mv(2, 0)]);
        assert_eq!(sample.to_move, Color::White);
    }

    #[test]
    #[should_panic(expected = "exceeds history length")]
    fn from_position_panics_when_ply_exceeds_history() {
        let history = [mv(0, 0), mv(0, 1), mv(1, 0)];
        Sample::from_position(&history, 4, vec![], 0.6);
    }

    #[test]
    fn board_roundtrip_rebuilds_position() {
        let history = [mv(7, 7), mv(8, 8), mv(6, 8), mv(8, 6), mv(7, 8)];
        let ply = 5;
        let sample = Sample::from_position(&history, ply, vec![], 0.5);
        let board = sample.board().unwrap();
        assert_eq!(board.to_move(), sample.to_move);
        assert_eq!(board.empty_moves().count(), 225 - ply);
    }
}
