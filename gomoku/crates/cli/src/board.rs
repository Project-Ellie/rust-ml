//! The UI's view of a Gomoku board — object-safe and engine-agnostic.
//!
//! This trait lives here, not in the engine: the CLI is the consumer that
//! needs runtime board selection, while the engine keeps static dispatch
//! for self-play speed (tutorial 13, "no shared trait").

use engine::{Color, Move, PlayError, Status};

/// Everything the UI needs from a board, and nothing more.
///
/// Object-safe by design: no generics, no `Self` returns, no constructors.
/// Build concrete boards behind [`board_for`].
pub trait GameBoard {
    fn play(&mut self, mv: Move) -> Result<(), PlayError>;
    /// Undo the last move. Callers must ensure the history is non-empty:
    /// both implementations panic otherwise.
    fn undo(&mut self);
    fn status(&self) -> Status;
    fn to_move(&self) -> Color;
    fn stone_at(&self, mv: Move) -> Option<Color>;
    fn moves(&self) -> &[Move];
    fn name(&self) -> &'static str;
    /// Access the underlying fast-engine board, if there is one.
    /// Used for engine-specific features such as the TSS demo.
    fn engine_board(&self) -> Option<&engine::Board> {
        None
    }
}

/// The naive oracle, wrapped so UI extras have a home.
pub struct NaiveBoard(engine::reference::Board);

impl GameBoard for NaiveBoard {
    fn play(&mut self, mv: Move) -> Result<(), PlayError> {
        self.0.play(mv)
    }

    fn undo(&mut self) {
        self.0.undo()
    }

    fn status(&self) -> Status {
        self.0.status()
    }

    fn to_move(&self) -> Color {
        self.0.to_move()
    }

    fn stone_at(&self, mv: Move) -> Option<Color> {
        self.0.stone_at(mv)
    }

    fn moves(&self) -> &[Move] {
        self.0.moves()
    }

    fn name(&self) -> &'static str {
        "naive"
    }
}

/// The production bitboard engine.
pub struct FastBoard(engine::Board);

impl FastBoard {
    /// Wrap an engine board for the UI.
    pub fn new(board: engine::Board) -> Self {
        Self(board)
    }
}

impl GameBoard for FastBoard {
    fn play(&mut self, mv: Move) -> Result<(), PlayError> {
        self.0.play(mv)
    }

    fn undo(&mut self) {
        self.0.undo()
    }

    fn status(&self) -> Status {
        self.0.status()
    }

    fn to_move(&self) -> Color {
        self.0.to_move()
    }

    fn stone_at(&self, mv: Move) -> Option<Color> {
        self.0.stone_at(mv)
    }

    fn moves(&self) -> &[Move] {
        self.0.moves()
    }

    fn name(&self) -> &'static str {
        "fast"
    }

    fn engine_board(&self) -> Option<&engine::Board> {
        Some(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    Naive,
    Fast,
}

pub fn board_for(kind: EngineKind) -> Box<dyn GameBoard> {
    match kind {
        EngineKind::Naive => Box::new(NaiveBoard(engine::reference::Board::new())),
        EngineKind::Fast => Box::new(FastBoard(engine::Board::new())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_boards_agree_through_the_trait() {
        let mut boards: Vec<Box<dyn GameBoard>> =
            vec![board_for(EngineKind::Naive), board_for(EngineKind::Fast)];
        let script = [(7u8, 7u8), (7, 8), (8, 7), (8, 8), (6, 7)];
        for (r, c) in script {
            let mv = Move::new(r, c).unwrap();
            for b in &mut boards {
                assert!(b.play(mv).is_ok());
            }
            let [a, b] = &mut boards[..] else {
                unreachable!()
            };
            assert_eq!(a.status(), b.status());
            assert_eq!(a.stone_at(mv), b.stone_at(mv));
        }
    }

    #[test]
    fn play_error_travels_through_the_trait() {
        let mut b = board_for(EngineKind::Fast);
        let mv = Move::new(7, 7).unwrap();
        b.play(mv).unwrap();
        assert_eq!(b.play(mv), Err(PlayError::Occupied));
    }

    #[test]
    fn undo_travels_through_the_trait_on_both_boards() {
        for kind in [EngineKind::Naive, EngineKind::Fast] {
            let mut board = board_for(kind);
            let mv = Move::new(7, 7).unwrap();
            board.play(mv).unwrap();
            assert_eq!(board.moves().len(), 1);

            board.undo();
            assert_eq!(board.moves().len(), 0);
            assert_eq!(board.stone_at(mv), None);
            assert_eq!(board.status(), Status::Ongoing);
            assert_eq!(board.to_move(), Color::Black);
        }
    }

    #[test]
    fn undo_after_a_win_reopens_the_game() {
        let mut board = board_for(EngineKind::Fast);
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
            board.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(board.status(), Status::Won(Color::Black));

        board.undo();
        assert_eq!(board.status(), Status::Ongoing);
        assert_eq!(board.moves().len(), 8);
    }
}
