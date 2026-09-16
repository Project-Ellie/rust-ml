//! `Board`: absolute colors + `to_move`, play/undo, legality, status.
//! Slice 3. See docs/13-engine-design.md, "Board".
use crate::bitboard::{Bitboard, VALID, idx};
use crate::moveset::Move;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black,
    White,
}

impl Color {
    pub fn other(self) -> Color {
        match self {
            Color::Black => Color::White,
            Color::White => Color::Black,
        }
    }
}

/// Game state. An enum, not bool flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ongoing,
    Won(Color),
    Draw,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PlayError {
    #[error("Cell is already occupied.")]
    Occupied,
    #[error("Game is already over.")]
    GameOver,
}

/// The production board. Two bitboards in ABSOLUTE colors (ch. 13,
/// decision 5: Swap2's non-alternating opening cannot be expressed in
/// a relative me/you store), plus side to move, status, and full
/// move history (encoding, undo, game records).
///
/// `PartialEq` is derived for tests ("undo everything ⇒ equals
/// `Board::new()`"); it compares bitboards, not game meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    black: Bitboard,
    white: Bitboard,
    to_move: Color,
    status: Status,
    moves: Vec<Move>,
}

const DIRS: [i32; 4] = [1, 16, 15, 17];

/// Consecutive set bits starting one step away from `i`, walking in
/// `step` direction. Two termination mechanisms, both free:
///   - the index guard stops walks at the array edge (verticals);
///   - the PADDING INVARIANT stops horizontal/diagonal wraps: every
///     wrap path lands in column 15, whose bits are always zero.
fn count_walk(stones: Bitboard, i: usize, step: i32) -> usize {
    let mut n = 0;
    let mut x = i as i32 + step;
    while (0..240).contains(&x) && stones.test(x as usize) {
        n += 1;
        x += step;
    }
    n
}


impl Board {
    pub fn new() -> Board {
        Board {
            black: Bitboard::EMPTY,
            white: Bitboard::EMPTY,
            to_move: Color::Black,
            status: Status::Ongoing,
            moves: Vec::new(),
        }
    }

    pub fn play(&mut self, mv: Move) -> Result<(), PlayError> {
        if self.status != Status::Ongoing {
            return Err(PlayError::GameOver)
        }
        let i = idx(mv.row(), mv.col());
        if (self.black | self.white).test(i) {
            return Err(PlayError::Occupied);
        }
        match self.to_move {
            Color::Black => self.black = self.black.with_bit(i),
            Color::White => self.white = self.white.with_bit(i),
        }
        self.moves.push(mv);

        if self.wins_from(i, self.to_move) {
            self.status = Status::Won(self.to_move);
        } else if self.moves.len() == 225 {
            self.status = Status::Draw;
        }

        self.to_move = self.to_move.other();

        Ok(())
    }

    fn wins_from(&self, i: usize, color: Color) -> bool {
        let stones = self.stones(color);
        DIRS.iter()
            .any(|&s| 1 + count_walk(stones, i, s) + count_walk(stones, i, -s) >= 5)
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn to_move(&self) -> Color {
        self.to_move
    }

    pub fn moves(&self) -> &[Move] {
        &self.moves
    }

    pub fn is_legal(&self, mv: Move) -> bool {
        self.status == Status::Ongoing &&
            !(self.black | self.white).test(idx(mv.row(), mv.col()))
    }

    pub fn undo(&mut self) {
        let mv = self.moves.pop().expect("undo with empty history");
        let i = idx(mv.row(), mv.col());
        match self.to_move.other() {
            Color::Black => self.black = self.black.without_bit(i),
            Color::White => self.white = self.white.without_bit(i),
        }
        self.to_move = self.to_move.other(); // flip back
        // Undoing the winning move un-wins the game; undoing into a
        // would-be draw likewise reopens it. Ongoing is always right.
        self.status = Status::Ongoing;
    }

    pub fn empty_moves(&self) -> impl Iterator<Item = Move> + '_ {
        // `!occupied` sets ALL padding bits — mask immediately.
        // This is the one place complements are allowed, and the mask
        // is non-negotiable (the padding invariant, ch. 13).
        let empty = !(self.black | self.white) & VALID;

        // The classic set-bit walk, word by word. `bits & (bits - 1)`
        // clears the lowest set bit — commit it to memory, you will
        // meet it in every bitboard codebase.
        empty.0.into_iter().enumerate().flat_map(|(w, mut bits)| {
            std::iter::from_fn(move || {
                if bits == 0 {
                    return None; // word exhausted → next word
                }
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let i = w * 64 + bit; // stride-16 index
                // Back across the boundary: stride-16 → (row, col) →
                // stride-15 Move. The `unwrap` is justified by VALID:
                // padding bits are never set in `empty`, so c < 15
                // always. Drop the mask and this panics — LOUD, never
                // silently wrong.
                Some(Move::new((i / 16) as u8, (i % 16) as u8).unwrap())
            })
        })
    }


    pub(crate) fn stones(&self, color: Color) -> Bitboard {
        match color {
            Color::Black => self.black,
            Color::White => self.white,
        }
    }
}






#[cfg(test)]
mod tests {
    use std::iter::Once;
    use super::*;
    use crate::moveset::Move;

    #[test]
    fn new_board_has_225_legal_moves_black_to_move() {
        let b = Board::new();
        assert_eq!(b.status(), Status::Ongoing);
        assert_eq!(b.to_move(), Color::Black);
        assert_eq!(b.empty_moves().count(), 225);
        assert!(b.moves().is_empty());
    }

    #[test]
    fn play_places_a_stone_and_flips() {
        let mut b = Board::new();
        let mv = Move::new(7, 7).unwrap();
        b.play(mv).unwrap();
        assert_eq!(b.to_move(), Color::White);
        assert!(!b.is_legal(mv));
        assert_eq!(b.empty_moves().count(), 224);
    }

    #[test]
    fn empty_moves_and_legality_track_the_stones() {
        let mut b = Board::new();
        let mv = Move::new(7, 7).unwrap();
        assert!(b.is_legal(mv));

        b.play(mv).unwrap();
        assert!(!b.is_legal(mv));
        assert_eq!(b.empty_moves().count(), 224);
        assert!(b.empty_moves().all(|m| m != mv));

        for r in 0..15u8 {
            for c in 0..15u8 {
                let _ = b.play(Move::new(r, c).unwrap());
            }
        }
        assert_eq!(b.empty_moves().count(), 225 - b.moves.len());
        assert_eq!(b.status(), Status::Won(Color::White))
    }

    #[test]
    fn horizontal_five_wins() {
        let mut b = crate::reference::Board::new();
        let script = [
            (7, 3),
            (0, 0),
            (7, 4),
            (0, 2),
            (7, 5),
            (0, 4),
            (7, 6),
            (0, 6),
            (7, 7),
        ];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(b.status(), Status::Won(Color::Black));

        assert_eq!(
            b.play(Move::new(13, 13).unwrap()),
            Err(PlayError::GameOver)
        )
    }

    #[test]
    fn undo_everything_returns_a_pristine_board() {
        let mut b = Board::new();
        let script = [(5, 6), (5, 7), (4, 6), (3, 6)];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        for _ in 0..script.len() {
            b.undo();
        }
        assert_eq!(b, Board::new());
    }

    #[test]
    fn undo_the_winning_move_unwins_the_game() {
        let mut b = Board::new();
        let script = [
            (7, 3), (0, 0), (7, 4), (0, 2), (7, 5), (0, 4), (7, 6), (0, 6), (7, 7),
        ];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(b.status(), Status::Won(Color::Black));

        b.undo();
        assert_eq!(b.status(), Status::Ongoing);
        assert_eq!(b.to_move(), Color::Black);
        assert!(b.is_legal(Move::new(7, 7).unwrap()));
        assert_eq!(b.moves.len(), 8);
    }

}