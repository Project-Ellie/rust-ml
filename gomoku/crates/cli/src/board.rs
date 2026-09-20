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
    /// Redo the last undone move. Default is a no-op for boards that do
    /// not keep a redo stack.
    fn redo(&mut self) {}
    /// True if there are moves available to redo.
    fn can_redo(&self) -> bool {
        false
    }
}

/// The naive oracle, wrapped so UI extras have a home.
#[cfg(feature = "naive-engine")]
pub struct NaiveBoard(engine::reference::Board);

#[cfg(feature = "naive-engine")]
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
    #[cfg(feature = "naive-engine")]
    Naive,
    Fast,
}

pub fn board_for(kind: EngineKind) -> Box<dyn GameBoard> {
    match kind {
        #[cfg(feature = "naive-engine")]
        EngineKind::Naive => Box::new(NaiveBoard(engine::reference::Board::new())),
        EngineKind::Fast => Box::new(FastBoard(engine::Board::new())),
    }
}

// ------------------------------------------------------------------
// Puzzle board

use crate::puzzle::Puzzle;

/// A board that starts from a puzzle root and supports undo/redo over
/// the moves played since that root.
///
/// The underlying engine board is rebuilt from `Board::from_position`
/// plus the current history. This matches the CLI's existing
/// rebuild-by-replay approach and gives a hard undo floor at the puzzle
/// root.
pub struct PuzzleBoard {
    board: engine::Board,
    redo: Vec<Move>,
}

impl PuzzleBoard {
    /// Build a board from a loaded puzzle.
    ///
    /// # Panics
    /// Panics if the puzzle does not form a valid position. The parser
    /// already validates this, so this is an internal-consistency check.
    pub fn new(puzzle: &Puzzle) -> Self {
        let board = engine::Board::from_position(&puzzle.black, &puzzle.white, puzzle.to_move)
            .expect("puzzle parser validated the position");
        Self {
            board,
            redo: Vec::new(),
        }
    }
}

impl GameBoard for PuzzleBoard {
    fn play(&mut self, mv: Move) -> Result<(), PlayError> {
        self.board.play(mv)?;
        self.redo.clear();
        Ok(())
    }

    fn undo(&mut self) {
        if self.board.moves().is_empty() {
            return;
        }
        let mv = self.board.moves().last().copied().expect("non-empty");
        self.board.undo();
        self.redo.push(mv);
    }

    fn status(&self) -> Status {
        self.board.status()
    }

    fn to_move(&self) -> Color {
        self.board.to_move()
    }

    fn stone_at(&self, mv: Move) -> Option<Color> {
        self.board.stone_at(mv)
    }

    fn moves(&self) -> &[Move] {
        self.board.moves()
    }

    fn name(&self) -> &'static str {
        "fast"
    }

    fn engine_board(&self) -> Option<&engine::Board> {
        Some(&self.board)
    }

    fn redo(&mut self) {
        if let Some(mv) = self.redo.pop() {
            let _ = self.board.play(mv);
        }
    }

    fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "naive-engine")]
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
    #[cfg(feature = "naive-engine")]
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

    #[test]
    fn puzzle_board_undo_floors_at_root() {
        let puzzle = Puzzle {
            name: "test".to_string(),
            black: vec![Move::new(7, 7).unwrap()],
            white: vec![],
            to_move: Color::White,
            solution: None,
            depth: None,
        };
        let mut board = PuzzleBoard::new(&puzzle);
        assert!(board.moves().is_empty());

        board.undo();
        assert!(board.moves().is_empty());

        board.play(Move::new(7, 8).unwrap()).unwrap();
        assert_eq!(board.moves().len(), 1);
        board.undo();
        assert!(board.moves().is_empty());
        board.undo();
        assert!(board.moves().is_empty());
    }

    #[test]
    fn puzzle_board_redo_is_cleared_on_new_move() {
        let puzzle = Puzzle {
            name: "test".to_string(),
            black: vec![Move::new(7, 7).unwrap()],
            white: vec![],
            to_move: Color::White,
            solution: None,
            depth: None,
        };
        let mut board = PuzzleBoard::new(&puzzle);
        board.play(Move::new(7, 8).unwrap()).unwrap();
        board.play(Move::new(6, 6).unwrap()).unwrap();
        board.undo();
        assert!(board.can_redo());

        board.play(Move::new(5, 5).unwrap()).unwrap();
        assert!(!board.can_redo());
        assert_eq!(board.moves().len(), 2);
        assert_eq!(
            board.moves().last().copied(),
            Some(Move::new(5, 5).unwrap())
        );
    }

    #[test]
    fn puzzle_board_redo_replays_moves() {
        let puzzle = Puzzle {
            name: "test".to_string(),
            black: vec![],
            white: vec![],
            to_move: Color::Black,
            solution: None,
            depth: None,
        };
        let mut board = PuzzleBoard::new(&puzzle);
        board.play(Move::new(7, 7).unwrap()).unwrap();
        board.undo();
        assert!(board.moves().is_empty());

        board.redo();
        assert_eq!(board.moves().len(), 1);
        assert_eq!(board.stone_at(Move::new(7, 7).unwrap()), Some(Color::Black));
    }
}
