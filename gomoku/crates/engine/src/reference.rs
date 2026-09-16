//! Naive array-based reference engine — the differential-testing oracle.
//! Slice 2. Compiled only for tests and the `testutil` feature.

use crate::moveset::Move;
use crate::board::{Color, Status, PlayError};

/// One board cell. Private - the outside world sees `Option<Color`
/// through `stone_at`; the `Cell` representation is our business.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cell {
    Empty,
    Stone(Color),
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
const DIRECTIONS: [(i32, i32); 4] = [(0, 1), (1, 0), (1, 1), (1, -1)];

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
        if self.status != Status::Ongoing {
            return Err(PlayError::GameOver);
        }
        if self.stone_at(mv).is_some() {
            return Err(PlayError::Occupied);
        }
        self.cells[mv.row() as usize][mv.col() as usize] = Cell::Stone(self.to_move);
        self.moves.push(mv);
        if self.wins_from(mv.row() as usize, mv.col() as usize, self.to_move) {
            self.status = Status::Won(self.to_move);
        } else if self.moves.len() == 225 {
            self.status = Status::Draw;
        }
        self.to_move = self.to_move.other();
        Ok(())
    }

    fn wins_from(&self, r: usize, c: usize, color: Color) -> bool {
        DIRECTIONS.iter().any(|&(dr, dc)| {
            1 + self.count_dir(r, c, dr, dc, color) + self.count_dir(r, c, -dr, -dc, color) >= 5
        })
    }

    pub fn count_dir(&self, r: usize, c: usize, dr: i32, dc: i32, color: Color) -> usize {
        let mut n = 0;
        let mut nr = r as i32 + dr;
        let mut nc = c as i32 + dc;
        while (0..15).contains(&nr)
            && (0..15).contains(&nc)
            && self.cells[nr as usize][nc as usize] == Cell::Stone(color)
        {
            n += 1;
            nr += dr;
            nc += dc;
        }
        n
    }

    pub fn moves(&self) -> &[Move] {
        &self.moves
    }

    /// Undoes the last move by REPLAYING the remaining history onto a
    /// fresh board. O(n) where the fast undo is O(1) — and obviously
    /// correct, which is the only virtue an oracle needs.
    pub fn undo(&mut self) {
        self.moves.pop().expect("undo with non-empty history");
        // A prefix of a legal game is always replayable: a game ends
        // only ON its winning/drawing move, and we just removed it.
        let history = self.moves.clone();
        *self = Board::new();
        for mv in history {
            self.play(mv).expect("replaying a valid history");
        }
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
        let mv = Move::new(5, 5).unwrap();
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
    }

    #[test]
    fn horizontal_five_wins_for_white() {
        let mut b = Board::new();
        // White wins, so White makes the last move: the script ends
        // on a White stone. Black's junk sits on rank 12 with gaps.
        let script = [
            (12, 0),
            (4, 4),
            (12, 2),
            (4, 5),
            (12, 4),
            (4, 6),
            (12, 6),
            (4, 7),
            (12, 8),
            (4, 8),
        ];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(b.status(), Status::Won(Color::White));
    }

    #[test]
    fn vertical_five_wins() {
        let mut b = Board::new();
        let script = [
            (2, 5),
            (0, 0),
            (3, 5),
            (0, 2),
            (4, 5),
            (0, 4),
            (5, 5),
            (0, 6),
            (6, 5),
        ];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(b.status(), Status::Won(Color::Black));
    }

    #[test]
    fn diagonal_down_right_five_wins() {
        let mut b = Board::new();
        let script = [
            (2, 2),
            (0, 0),
            (3, 3),
            (0, 2),
            (4, 4),
            (0, 4),
            (5, 5),
            (0, 6),
            (6, 6),
        ];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(b.status(), Status::Won(Color::Black));
    }

    #[test]
    fn diagonal_down_left_five_wins() {
        let mut b = Board::new();
        let script = [
            (2, 8),
            (0, 0),
            (3, 7),
            (0, 2),
            (4, 6),
            (0, 4),
            (5, 5),
            (0, 6),
            (6, 4),
        ];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(b.status(), Status::Won(Color::Black));
    }

    #[test]
    fn overline_six_counts_as_a_win() {
        let mut b = Board::new();
        // Black builds the two arms (7,3)-(7,4) and (7,6)-(7,8),
        // then bridges them with (7,5). White junks on rank 0, gapped.
        let script = [
            (7, 3),
            (0, 0),
            (7, 4),
            (0, 2),
            (7, 6),
            (0, 4),
            (7, 7),
            (0, 6),
            (7, 8),
            (0, 8),
            // Black's runs so far: length 2 and 3. No five yet!
            (7, 5), // the bridge — six in a row, all at once
        ];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        // Freestyle rules: overlines count (ch. 13, decision 1).
        assert_eq!(b.status(), Status::Won(Color::Black));
    }

    #[test]
    fn four_in_a_row_is_still_ongoing() {
        let mut b = Board::new();
        let script = [
            (7, 3),
            (0, 0),
            (7, 4),
            (0, 2),
            (7, 5),
            (0, 4),
            (7, 6),
            (0, 6),
        ];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(b.status(), Status::Ongoing);
        assert_eq!(b.to_move(), Color::Black); // and the game continues
    }

    #[test]
    fn five_in_the_top_left_corner_wins() {
        let mut b = Board::new();
        // Black fills rank 0 starting at column 0: the win walk runs
        // into the left edge and must stop cleanly.
        let script = [
            (0, 0),
            (7, 7),
            (0, 1),
            (7, 9),
            (0, 2),
            (9, 7),
            (0, 3),
            (9, 9),
            (0, 4),
        ];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(b.status(), Status::Won(Color::Black));
    }

    #[test]
    fn five_on_the_bottom_edge_wins_for_white() {
        let mut b = Board::new();
        // White wins along rank 14, ending in the bottom-right corner.
        let script = [
            (0, 0),
            (14, 10),
            (0, 2),
            (14, 11),
            (0, 4),
            (14, 12),
            (0, 6),
            (14, 13),
            (0, 8),
            (14, 14),
        ];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(b.status(), Status::Won(Color::White));
    }

    #[test]
    fn full_board_without_five_is_a_draw() {
        let mut b = Board::new();

        // Split the 225 cells into black/white by the stripe pattern:
        //   row pattern BBWW BBWW …, inverted on odd rows.
        let mut blacks = Vec::new(); // 113 cells
        let mut whites = Vec::new(); // 112 cells
        for r in 0..15u8 {
            for c in 0..15u8 {
                let stripe = (c % 4) < 2; // BBWW repeating
                let black = stripe != (r % 2 == 1); // inverted on odd rows
                if black {
                    blacks.push(Move::new(r, c).unwrap());
                } else {
                    whites.push(Move::new(r, c).unwrap());
                }
            }
        }

        // Alternate strictly: Black, White, Black, White, …
        for i in 0..112 {
            b.play(blacks[i]).unwrap();
            b.play(whites[i]).unwrap();
        }
        assert_eq!(b.status(), Status::Ongoing); // 224 moves: still playing

        b.play(blacks[112]).unwrap(); // move 225 — board full
        assert_eq!(b.status(), Status::Draw);
        assert_eq!(b.moves().len(), 225);

        // A decided game rejects further moves — even though no cell
        // is free, GameOver (checked first) is the honest answer.
        assert_eq!(b.play(blacks[0]), Err(PlayError::GameOver));
    }

    #[test]
    fn play_after_a_won_game_is_rejected() {
        let mut b = Board::new();
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

        // The board is full of empty cells — but the game is over.
        assert_eq!(b.play(Move::new(13, 13).unwrap()), Err(PlayError::GameOver));
    }

    #[test]
    fn moves_returns_history_in_play_order() {
        let mut b = Board::new();
        let script = [(7, 7), (3, 3), (7, 8), (3, 4)];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }

        let expected: Vec<Move> = [(7, 7), (3, 3), (7, 8), (3, 4)]
            .iter()
            .map(|&(r, c)| Move::new(r, c).unwrap())
            .collect();
        assert_eq!(b.moves(), expected.as_slice());
    }

    #[test]
    fn undo_replays_history_without_the_last_move() {
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
        assert_eq!(b.stone_at(Move::new(7, 7).unwrap()), None);
        assert_eq!(b.moves().len(), 8);
    }
}
