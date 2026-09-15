//! Naive array-based reference engine — the differential-testing oracle.
//! Slice 2. Compiled only for tests and the `testutil` feature.

use crate::moveset::Move;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black,
    White
}

impl Color {
    pub fn other(self) -> Color {
        match self {
            Color::Black => Color::White,
            Color::White => Color::Black,
        }
    }
}


/// One board cell. Private - the outside world sees `Option<Color`
/// through `stone_at`; the `Cell` representation is our business.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cell {
    Empty,
    Stone(Color),
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
}

/// the naive board 15x15 cells + full move history
pub struct Board {
    cells: [[Cell; 15]; 15],
    to_move: Color,
    status: Status,
    moves: Vec<Move>,
}

/// The four axes as (drow, dcol) step vectors. Each axis is walked in
/// both directions, for four entries cover all eight rays
const DIRECTIONS: [(i32, i32); 4] = [
    (0, 1), (1, 0), (1, 1), (1, -1)
];

impl Board {
    pub fn new() -> Board {
        Board {
            cells: [[Cell::Empty; 15]; 15],
            to_move: Color::Black,
            status: Status::Ongoing,
            moves: Vec::new(),
        }
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn to_move(&self) -> Color {
        self.to_move
    }

    /// `None` = empty cell. Translates our private `Cell` into the
    /// public `Option<Color>` vocabulary.
    pub fn stone_at(&self, mv: Move) -> Option<Color> {
        match self.cells[mv.row() as usize][mv.col() as usize] {
            Cell::Empty => None,
            Cell::Stone(color) => Some(color),
        }
    }

    pub fn play(&mut self, mv: Move) -> Result<(), PlayError> {
        if self.stone_at(mv).is_some() {
            return Err(PlayError::Occupied);
        }
        self.cells[mv.row() as usize][mv.col() as usize] = Cell::Stone(self.to_move);
        self.moves.push(mv);
        if self.wins_from(mv.row() as usize, mv.col() as usize, self.to_move) {
            self.status = Status::Won(self.to_move);
        }
        self.to_move = self.to_move.other();
        Ok(())
    }

    fn wins_from(&self, r: usize, c: usize, color: Color) -> bool {
        DIRECTIONS.iter().any(|&(dr, dc)| {
           1 + self.count_dir(r, c, dr, dc, color)
            + self.count_dir(r, c, -dr, -dc, color) >= 5
        })
    }

    pub fn count_dir(&self, r: usize, c: usize, dr: i32, dc: i32, color: Color) -> usize{
        let mut n = 0;
        let mut nr = r as i32 + dr;
        let mut nc = c as i32 + dc;
        while (0..15).contains(&nr) && (0..15).contains(&nc)
            && self.cells[nr as usize][nc as usize] == Cell::Stone(color) {
            n+=1;
            nr += dr;
            nc += dc;
        }
        n
    }

    pub fn moves(&self) -> &[Move] {
        &self.moves
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::moveset::Move;
    use crate::reference::{Board, Status, Color};

    #[test]
    fn new_board_is_empty_ongoing_black_to_move() {
        let b = Board::new();
        assert_eq!(b.status(), Status::Ongoing);
        assert_eq!(b.to_move(), Color::Black);
        assert_eq!(b.stone_at(Move::new(7, 7).unwrap()), None);
        assert!(b.moves().is_empty());
    }

    #[test]
    fn play_places_stone_and_flips_to_move() {
        let mut b = Board::new();
        let mv = Move::new(7, 7).unwrap();
        b.play(mv).unwrap();
        assert_eq!(b.stone_at(mv), Some(Color::Black));
        assert_eq!(b.to_move(), Color::White);
    }

    #[test]
    fn play_on_occupied_cell_is_rejected() {
        let mut b = Board::new();
        let mv  = Move::new(5, 5).unwrap();
        b.play(mv).unwrap();

        assert_eq!(b.play(mv), Err(PlayError::Occupied));
        assert_eq!(b.stone_at(mv), Some(Color::Black));
        assert_eq!(b.to_move(), Color::White);
        assert_eq!(b.moves().len(), 1);
    }

    #[test]
    fn horizontal_five_wins_for_black() {
        let mut b = Board::new();
        let script = [
            (7, 3), (0, 0), (7, 4), (0, 2),
            (7, 5), (0, 4), (7, 6), (0, 6),
            (7, 7),
        ];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(b.status(), Status::Won(Color::Black));
    }

    #[test]
    fn vertical_five_wins() {
        let mut b = Board::new();
        let script = [
            (2, 5), (0, 0),
            (3, 5), (0, 2),
            (4, 5), (0, 4),
            (5, 5), (0, 6),
            (6, 5),
        ];
        for (r, c) in script { b.play(Move::new(r, c).unwrap()).unwrap(); }
        assert_eq!(b.status(), Status::Won(Color::Black));
    }

    #[test]
    fn diagonal_down_right_five_wins() {
        let mut b = Board::new();
        let script = [
            (2, 2), (0, 0),
            (3, 3), (0, 2),
            (4, 4), (0, 4),
            (5, 5), (0, 6),
            (6, 6),
        ];
        for (r, c) in script { b.play(Move::new(r, c).unwrap()).unwrap(); }
        assert_eq!(b.status(), Status::Won(Color::Black));
    }

    #[test]
    fn diagonal_down_left_five_wins() {
        let mut b = Board::new();
        let script = [
            (2, 8), (0, 0),
            (3, 7), (0, 2),
            (4, 6), (0, 4),
            (5, 5), (0, 6),
            (6, 4),
        ];
        for (r, c) in script { b.play(Move::new(r, c).unwrap()).unwrap(); }
        assert_eq!(b.status(), Status::Won(Color::Black));
    }
}