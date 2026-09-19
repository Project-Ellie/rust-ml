//! Swap2 opening state machine for the CLI.
//!
//! The engine's typestate consumes `self`, so the UI keeps its own
//! replay log of `(Move, Color)` placements and rebuilds the opening
//! state on undo. This module is pure logic and is fully unit-testable.

use engine::{Board, Color, Move};

/// The two hotseat parties. Player A starts the opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Party {
    /// The party that places the first three stones.
    A,
    /// The party that responds with a choice or two more stones.
    B,
}

impl Party {
    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Party::A => "Player A",
            Party::B => "Player B",
        }
    }
}

/// Who holds each absolute color after the opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Holders {
    black: Party,
    white: Party,
}

impl Holders {
    /// Direct constructor for tests and UI reconstruction.
    pub fn new(black: Party, white: Party) -> Self {
        Self { black, white }
    }

    /// Build the holder map from a Swap2 choice.
    ///
    /// * `FirstChoice` — Player B chooses.
    /// * `FinalChoice` — Player A chooses.
    pub fn from_choice(phase: OpeningPhase, choice: Color) -> Option<Self> {
        match (phase, choice) {
            (OpeningPhase::FirstChoice, Color::Black) => Some(Holders {
                black: Party::B,
                white: Party::A,
            }),
            (OpeningPhase::FirstChoice, Color::White) => Some(Holders {
                black: Party::A,
                white: Party::B,
            }),
            (OpeningPhase::FinalChoice, Color::Black) => Some(Holders {
                black: Party::A,
                white: Party::B,
            }),
            (OpeningPhase::FinalChoice, Color::White) => Some(Holders {
                black: Party::B,
                white: Party::A,
            }),
            _ => None,
        }
    }

    /// The party holding Black.
    pub fn black(self) -> Party {
        self.black
    }

    /// The party holding White.
    pub fn white(self) -> Party {
        self.white
    }
}

/// A phase of the Swap2 opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningPhase {
    /// Player A is placing two Black and one White stone.
    Placing3,
    /// Player B chooses: take Black, take White, or place two more.
    FirstChoice,
    /// Player B is placing one Black and one White stone.
    Placing2,
    /// Player A picks a color after the extra stones were placed.
    FinalChoice,
}

/// The result of finishing the opening with a color choice.
#[derive(Debug)]
pub struct OpeningOutcome {
    /// The board to play the rest of the game on.
    pub board: Board,
    /// Who holds Black.
    pub black_holder: Party,
    /// Who holds White.
    pub white_holder: Party,
}

/// Mutable Swap2 opening state kept by the UI.
#[derive(Debug, Clone)]
pub struct OpeningState {
    placements: Vec<(Move, Color)>,
    phase: OpeningPhase,
}

impl OpeningState {
    /// Start a fresh Swap2 opening.
    pub fn new() -> Self {
        Self {
            placements: Vec::with_capacity(5),
            phase: OpeningPhase::Placing3,
        }
    }

    /// Current phase.
    pub fn phase(&self) -> OpeningPhase {
        self.phase
    }

    /// All stones placed so far, in order.
    pub fn placements(&self) -> &[(Move, Color)] {
        &self.placements
    }

    /// The party whose turn it is to act in the current phase.
    pub fn active_party(&self) -> Party {
        match self.phase {
            OpeningPhase::Placing3 | OpeningPhase::FinalChoice => Party::A,
            OpeningPhase::FirstChoice | OpeningPhase::Placing2 => Party::B,
        }
    }

    /// The color that must be placed next, if any.
    pub fn current_color(&self) -> Option<Color> {
        let black = self.black_count();
        match self.phase {
            OpeningPhase::Placing3 => {
                if black < 2 {
                    Some(Color::Black)
                } else {
                    Some(Color::White)
                }
            }
            OpeningPhase::Placing2 => {
                if black < 3 {
                    Some(Color::Black)
                } else {
                    Some(Color::White)
                }
            }
            _ => None,
        }
    }

    /// How many stones of `color` still need to be placed in the active
    /// placement phase. Returns `0` during choice phases.
    pub fn remaining(&self, color: Color) -> u8 {
        let current = self.current_color();
        if current != Some(color) {
            return 0;
        }
        match (self.phase, color) {
            (OpeningPhase::Placing3, Color::Black) => 2 - self.black_count(),
            (OpeningPhase::Placing3, Color::White) => 1 - self.white_count(),
            (OpeningPhase::Placing2, Color::Black) => 3 - self.black_count(),
            (OpeningPhase::Placing2, Color::White) => 2 - self.white_count(),
            _ => 0,
        }
    }

    /// True if the current phase is a placement phase and the required
    /// count has not yet been reached.
    pub fn can_place(&self) -> bool {
        matches!(self.phase, OpeningPhase::Placing3 | OpeningPhase::Placing2) && !self.phase_full()
    }

    /// True if the current phase has exactly the stones it needs.
    fn phase_full(&self) -> bool {
        match self.phase {
            OpeningPhase::Placing3 => self.placements.len() == 3,
            OpeningPhase::Placing2 => self.placements.len() == 5,
            _ => false,
        }
    }

    /// Place the color required by the current phase on `mv`.
    ///
    /// Automatically advances to the next phase when the required count
    /// is reached.
    ///
    /// # Errors
    /// * `"cell is already occupied"` if the target is taken.
    /// * `"no more stones to place in this phase"` if not in a placement phase.
    pub fn place(&mut self, mv: Move) -> Result<(), String> {
        let color = self
            .current_color()
            .ok_or_else(|| "no more stones to place in this phase".to_string())?;

        if self.placements.iter().any(|(m, _)| *m == mv) {
            return Err("cell is already occupied".to_string());
        }

        self.placements.push((mv, color));
        self.try_advance();
        Ok(())
    }

    /// Confirm a full placement phase and move to the choice prompt.
    ///
    /// This is used after an undo returns the UI to a full placement
    /// phase; the player must explicitly press 'd' to continue.
    pub fn finish(&mut self) -> Result<(), String> {
        match self.phase {
            OpeningPhase::Placing3 if self.placements.len() == 3 => {
                self.phase = OpeningPhase::FirstChoice;
                Ok(())
            }
            OpeningPhase::Placing2 if self.placements.len() == 5 => {
                self.phase = OpeningPhase::FinalChoice;
                Ok(())
            }
            _ => Err("no stones left to place in this phase — type a move or 'u'".to_string()),
        }
    }

    /// Undo the last action.
    ///
    /// During a placement phase this removes the last stone. From a
    /// choice prompt it returns to the preceding full placement phase,
    /// from which the player must press 'd' to continue.
    pub fn undo(&mut self) {
        match self.phase {
            OpeningPhase::FirstChoice => self.phase = OpeningPhase::Placing3,
            OpeningPhase::FinalChoice => self.phase = OpeningPhase::Placing2,
            OpeningPhase::Placing2 => {
                if self.placements.len() > 3 {
                    self.placements.pop();
                } else {
                    // Already at the boundary: go back to the choice.
                    self.phase = OpeningPhase::FirstChoice;
                }
            }
            OpeningPhase::Placing3 => {
                self.placements.pop();
            }
        }
    }

    /// True if there is nothing to undo.
    pub fn is_at_start(&self) -> bool {
        self.placements.is_empty() && self.phase == OpeningPhase::Placing3
    }

    /// Choose a color and finish the opening.
    ///
    /// Returns `None` if called in a phase that does not allow a choice.
    pub fn choose(self, color: Color) -> Option<OpeningOutcome> {
        let holders = Holders::from_choice(self.phase, color)?;
        let board = self.build_board();
        Some(OpeningOutcome {
            board,
            black_holder: holders.black(),
            white_holder: holders.white(),
        })
    }

    /// Switch from the first choice to placing two more stones.
    ///
    /// Returns `None` if not in `FirstChoice`.
    pub fn add_two_more(mut self) -> Option<Self> {
        if self.phase != OpeningPhase::FirstChoice {
            return None;
        }
        self.phase = OpeningPhase::Placing2;
        Some(self)
    }

    fn try_advance(&mut self) {
        match self.phase {
            OpeningPhase::Placing3 if self.placements.len() == 3 => {
                self.phase = OpeningPhase::FirstChoice;
            }
            OpeningPhase::Placing2 if self.placements.len() == 5 => {
                self.phase = OpeningPhase::FinalChoice;
            }
            _ => {}
        }
    }

    fn build_board(&self) -> Board {
        let black: Vec<Move> = self
            .placements
            .iter()
            .filter(|(_, c)| *c == Color::Black)
            .map(|(m, _)| *m)
            .collect();
        let white: Vec<Move> = self
            .placements
            .iter()
            .filter(|(_, c)| *c == Color::White)
            .map(|(m, _)| *m)
            .collect();
        // Swap2 invariant: White always has fewer stones, so White moves first.
        Board::from_position(&black, &white, Color::White)
            .expect("Swap2 invariant guarantees a valid position")
    }

    fn black_count(&self) -> u8 {
        self.placements
            .iter()
            .filter(|(_, c)| *c == Color::Black)
            .count() as u8
    }

    fn white_count(&self) -> u8 {
        self.placements
            .iter()
            .filter(|(_, c)| *c == Color::White)
            .count() as u8
    }
}

impl Default for OpeningState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    #[test]
    fn placing3_fixed_order_black_black_white() {
        let mut s = OpeningState::new();
        assert_eq!(s.current_color(), Some(Color::Black));
        s.place(mv(7, 7)).unwrap();
        assert_eq!(s.current_color(), Some(Color::Black));
        s.place(mv(6, 6)).unwrap();
        assert_eq!(s.current_color(), Some(Color::White));
        s.place(mv(5, 5)).unwrap();
        assert_eq!(s.phase(), OpeningPhase::FirstChoice);
    }

    #[test]
    fn first_choice_take_colors() {
        let mut s = OpeningState::new();
        for m in [mv(7, 7), mv(6, 6), mv(5, 5)] {
            s.place(m).unwrap();
        }

        let outcome = s.clone().choose(Color::Black).unwrap();
        assert_eq!(outcome.black_holder, Party::B);
        assert_eq!(outcome.white_holder, Party::A);
        assert_eq!(outcome.board.to_move(), Color::White);

        let outcome = s.choose(Color::White).unwrap();
        assert_eq!(outcome.black_holder, Party::A);
        assert_eq!(outcome.white_holder, Party::B);
    }

    #[test]
    fn add_two_more_then_final_choice() {
        let mut s = OpeningState::new();
        for m in [mv(7, 7), mv(6, 6), mv(5, 5)] {
            s.place(m).unwrap();
        }
        s = s.add_two_more().unwrap();
        assert_eq!(s.phase(), OpeningPhase::Placing2);
        s.place(mv(4, 4)).unwrap();
        s.place(mv(3, 3)).unwrap();
        assert_eq!(s.phase(), OpeningPhase::FinalChoice);

        let outcome = s.choose(Color::Black).unwrap();
        assert_eq!(outcome.black_holder, Party::A);
        assert_eq!(outcome.white_holder, Party::B);
    }

    #[test]
    fn undo_from_choice_returns_to_full_placement() {
        let mut s = OpeningState::new();
        for m in [mv(7, 7), mv(6, 6), mv(5, 5)] {
            s.place(m).unwrap();
        }
        assert_eq!(s.phase(), OpeningPhase::FirstChoice);

        s.undo();
        assert_eq!(s.phase(), OpeningPhase::Placing3);
        assert_eq!(s.placements().len(), 3);

        // From the full placement phase, finish returns to the choice.
        s.finish().unwrap();
        assert_eq!(s.phase(), OpeningPhase::FirstChoice);
    }

    #[test]
    fn undo_during_placing3_removes_last_stone() {
        let mut s = OpeningState::new();
        s.place(mv(7, 7)).unwrap();
        s.place(mv(6, 6)).unwrap();
        s.undo();
        assert_eq!(s.placements().len(), 1);
        assert_eq!(s.current_color(), Some(Color::Black));
    }

    #[test]
    fn undo_from_placing2_boundary_returns_to_first_choice() {
        let mut s = OpeningState::new();
        for m in [mv(7, 7), mv(6, 6), mv(5, 5)] {
            s.place(m).unwrap();
        }
        s = s.add_two_more().unwrap();
        s.undo();
        assert_eq!(s.phase(), OpeningPhase::FirstChoice);
    }

    #[test]
    fn occupied_cell_is_rejected() {
        let mut s = OpeningState::new();
        let m = mv(7, 7);
        s.place(m).unwrap();
        let err = s.place(m).unwrap_err();
        assert!(err.contains("occupied"), "{err}");
    }

    #[test]
    fn finish_requires_full_phase() {
        let mut s = OpeningState::new();
        s.place(mv(7, 7)).unwrap();
        assert!(s.finish().is_err());
    }
}
