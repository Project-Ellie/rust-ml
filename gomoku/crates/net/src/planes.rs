//! Assemble the four 17×17 input planes from an engine [`Board`].
//!
//! Plane 0: stones of the player to move ("me").
//! Plane 1: stones of the opponent ("you"), including the border ring.
//! Plane 2: the most recent move played by "me", if any.
//! Plane 3: the most recent move played by "you", if any.
//!
//! This module owns the "planes live in `net`" invention (I1): the
//! engine emits only two planes, and `net` adds the history planes.

use engine::{Board, EXT, Move, Planes, encode as encode_planes};

/// Four 17x17 input planes as congiguous `u8` arrays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputPlanes {
    /// stones of the player to move
    pub me: [u8; EXT * EXT],
    /// Opponent stones + the border ring
    pub you: [u8; EXT * EXT],
    /// the last move playeed by `to_move`
    pub my_last: [u8; EXT * EXT],
    /// the last move played by the opponent
    pub opp_last: [u8; EXT * EXT],
}

impl InputPlanes {
    /// returns the four planes as shape [4, EXT, EXT] array
    pub fn to_array(&self) -> [[[u8; EXT]; EXT]; 4] {
        let mut out = [[[0u8; EXT]; EXT]; 4];
        for r in 0..EXT {
            for c in 0..EXT {
                out[0][r][c] = self.me[r * EXT + c];
                out[1][r][c] = self.you[r * EXT + c];
                out[2][r][c] = self.my_last[r * EXT + c];
                out[3][r][c] = self.opp_last[r * EXT + c];
            }
        }
        out
    }
}

/// Build the four input planes for the side to move on `board`.
///
/// The `board` must carry a correct move history (`Board::moves()`),
/// because the last-move planes are derived from it.
pub fn assemble(board: &Board) -> InputPlanes {
    let Planes { me, you } = encode_planes(board);
    let (my_last, opp_last) = last_move_planes(board);
    InputPlanes {
        me,
        you,
        my_last,
        opp_last,
    }
}

fn last_move_planes(board: &Board) -> ([u8; EXT * EXT], [u8; EXT * EXT]) {
    let mut my_last = [0u8; EXT * EXT];
    let mut opp_last = [0u8; EXT * EXT];

    let history = board.moves();
    if history.is_empty() {
        return (my_last, opp_last);
    }

    let last_played = history[history.len() - 1];
    set_one_hot(&mut opp_last, last_played);

    if history.len() >= 2 {
        let my_prev = history[history.len() - 2];
        set_one_hot(&mut my_last, my_prev);
    }
    (my_last, opp_last)
}

fn set_one_hot(plane: &mut [u8; EXT * EXT], mv: Move) {
    let dst = (mv.row() as usize + 1) * EXT + mv.col() as usize + 1;
    plane[dst] = 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Move;

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    #[test]
    fn empty_board_has_no_last_moves() {
        let board = Board::new();
        let planes = assemble(&board);
        assert!(planes.my_last.iter().all(|&x| x == 0));
        assert!(planes.opp_last.iter().all(|&x| x == 0));
    }

    #[test]
    fn after_one_move_opp_last_is_set() {
        let mut board = Board::new();
        board.play(mv(7, 7)).unwrap();
        let planes = assemble(&board);

        assert_eq!(planes.opp_last[(7 + 1) * EXT + (7 + 1)], 1);
        assert_eq!(planes.my_last.iter().sum::<u8>(), 0);
        assert_eq!(planes.opp_last.iter().sum::<u8>(), 1);
    }

    #[test]
    fn after_two_moves_both_last_planes_are_set() {
        let mut board = Board::new();
        board.play(mv(7, 7)).unwrap();
        board.play(mv(8, 8)).unwrap();
        let planes = assemble(&board);

        // Black to move. Black's last move was (7,7) -> my_last.
        // White's last move was (8,8) -> opp_last.
        assert_eq!(planes.my_last[(7 + 1) * EXT + (7 + 1)], 1);
        assert_eq!(planes.opp_last[(8 + 1) * EXT + (8 + 1)], 1);
        assert_eq!(planes.my_last.iter().sum::<u8>(), 1);
        assert_eq!(planes.opp_last.iter().sum::<u8>(), 1);
    }

    #[test]
    fn last_move_planes_flip_with_side_to_move() {
        let mut black_to_move = Board::new();
        black_to_move.play(mv(3, 3)).unwrap();
        black_to_move.play(mv(4, 4)).unwrap();

        let mut white_to_move = Board::new();
        white_to_move.play(mv(3, 3)).unwrap();

        let black_planes = assemble(&black_to_move);
        let white_planes = assemble(&white_to_move);

        let (dst33, dst44) = ((3 + 1) * EXT + 3 + 1, (4 + 1) * EXT + 4 + 1);

        assert_eq!(black_planes.my_last[dst33], 1);
        assert_eq!(black_planes.opp_last[dst44], 1);
        assert_eq!(white_planes.opp_last[dst33], 1);
        assert_eq!(white_planes.my_last.iter().sum::<u8>(), 0);
    }

    #[test]
    fn to_array_shape_is_four_planes() {
        let arr = assemble(&Board::new()).to_array();
        assert_eq!(arr.len(), 4);
        assert_eq!(arr[0].len(), EXT);
        assert_eq!(arr[0][0].len(), EXT);
    }

    #[test]
    fn stones_match_engine_encode() {
        let mut board = Board::new();
        board.play(mv(0, 0)).unwrap();
        board.play(mv(7, 7)).unwrap();
        let planes = assemble(&board);
        let encoded = encode_planes(&board);

        assert_eq!(planes.me, encoded.me);
        assert_eq!(planes.you, encoded.you);
    }
}
