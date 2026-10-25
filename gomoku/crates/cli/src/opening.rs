//! The session-side Swap2 machine: a replay log with the engine's
//! typestate as its validated projection.
//!
//! The engine's `Swap2<State>` typestate makes invalid transitions
//! impossible at compile time, but every transition consumes `self`.
//! A typestate value therefore cannot undo, cannot report its stones,
//! and cannot be stored across commands of different phases. This
//! module supplies the missing properties at runtime: [`OpeningState`]
//! keeps a replay log of `(Move, Color)` placements as the source of
//! truth and rebuilds the typestate from the log whenever the engine
//! must judge the position. The log is the truth; the typestate is a
//! validated projection of the truth.
//!
//! The rule split: the Swap2 rule fixes only the final stone counts of
//! each placement round (two Black and one White, then one more Black
//! and one more White after `more`). The placement order is free. The
//! default order alternates, black first, within each round; a color
//! token on `play` overrides the next stone's color.

use engine::{Board, Color, Move, Swap2};
use engine::{FinalChoice, FirstChoice, Placing2, Placing3};
use serde::{Deserialize, Serialize};

use crate::command::{CommandError, Event, OpeningPhase};

/// The session-side record of a Swap2 ceremony in progress.
///
/// Two fields carry the whole state: the runtime phase (which mirrors
/// the engine's typestate marker) and the replay log of placements.
/// The phase is not derivable from the log alone: three placements can
/// mean "round complete, `done` accepted" (FirstChoice) or "third
/// stone placed, `done` not yet given" (Placing3).
#[derive(Debug, Clone, PartialEq)]
pub struct OpeningState {
    phase: OpeningPhase,
    log: Vec<(Move, Color)>,
}

/// The read model of a ceremony in progress, exposed through
/// `Snapshot`.
///
/// A front-end draws the ceremony from this data alone. It needs no
/// Swap2 rules: the phase says which commands apply, the placements
/// say which stones are on the board, and the default color says what
/// the next `play` without a token will place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpeningSnapshot {
    /// The current phase.
    pub phase: OpeningPhase,
    /// The placements so far, in order of placement.
    pub placements: Vec<(Move, Color)>,
    /// The color the next `play` uses without an override token;
    /// `None` at a choice phase, where no placement is next.
    pub default_next_color: Option<Color>,
}

impl OpeningState {
    /// A fresh ceremony: phase Placing3, empty log.
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: OpeningPhase::Placing3,
            log: Vec::new(),
        }
    }

    /// The read model for the session snapshot.
    #[must_use]
    pub fn snapshot(&self) -> OpeningSnapshot {
        OpeningSnapshot {
            phase: self.phase,
            placements: self.log.clone(),
            default_next_color: self.default_next_color(),
        }
    }

    /// The color the next `play` uses when no color token overrides it.
    ///
    /// The default alternates within each round, black first. The
    /// count restarts per round: the fourth stone overall is the first
    /// stone of the second round, so it defaults to black again. At a
    /// choice phase there is no next placement, so the answer is
    /// `None`.
    #[must_use]
    pub fn default_next_color(&self) -> Option<Color> {
        let round_start = match self.phase {
            OpeningPhase::Placing3 => 0,
            OpeningPhase::Placing2 => 3,
            OpeningPhase::FirstChoice | OpeningPhase::FinalChoice => return None,
        };
        let index_in_round = self.log.len() - round_start;
        Some(if index_in_round.is_multiple_of(2) {
            Color::Black
        } else {
            Color::White
        })
    }

    /// Place the next stone of the ceremony.
    ///
    /// The engine remains the rules authority: the typestate is
    /// rebuilt from the log, and its `place` rejects an occupied
    /// square. The log grows only after the engine accepts the
    /// placement, so the log can never hold a stone the engine
    /// refused.
    pub fn place(&mut self, mv: Move, color: Option<Color>) -> Result<Vec<Event>, CommandError> {
        // A choice phase has no next stone; the `None` default makes
        // `play` there a typed error without touching the engine.
        let color = match color.or_else(|| self.default_next_color()) {
            Some(color) => color,
            None => return Err(CommandError::NotAtPlacementPhase),
        };
        match self.phase {
            OpeningPhase::Placing3 => rebuild_placing3(&self.log).place(mv, color)?,
            OpeningPhase::Placing2 => rebuild_placing2(&self.log).place(mv, color)?,
            OpeningPhase::FirstChoice | OpeningPhase::FinalChoice => {
                return Err(CommandError::NotAtPlacementPhase);
            }
        }
        self.log.push((mv, color));
        Ok(vec![Event::StonePlaced { mv, color }])
    }

    /// Take back the last placement(s) — the meaning of `back` during
    /// the ceremony.
    ///
    /// No engine rebuild is needed here. The log only grows through
    /// validated placements, so every prefix of the log was once the
    /// live state and is valid by construction; removing entries cannot
    /// produce an invalid state. The phase after the removal is a
    /// function of the log length, and it is never a choice phase:
    /// removing the stone that completed a round takes the `done` back
    /// with it, and reaching a choice again takes a fresh `done`.
    pub fn undo(&mut self, n: u32) -> Result<Vec<Event>, CommandError> {
        if self.log.is_empty() {
            return Err(CommandError::OpeningLogEmpty);
        }
        let mut events = Vec::new();
        // `back N` removes up to N placements; the count clamps at the
        // log length the way tree navigation clamps at the root.
        let removals = (n as usize).min(self.log.len());
        for _ in 0..removals {
            let (mv, _) = self.log.pop().expect("the length was checked above");
            events.push(Event::StoneRemoved { mv });
        }
        // Four or five stones can only be a second round in progress;
        // three or fewer belong to the first round.
        let phase = if self.log.len() >= 4 {
            OpeningPhase::Placing2
        } else {
            OpeningPhase::Placing3
        };
        if phase != self.phase {
            self.phase = phase;
            events.push(Event::OpeningPhaseChanged { phase });
        }
        Ok(events)
    }

    /// End the current placement round.
    ///
    /// The engine validates the counts: the rebuild replays the log
    /// into the typestate, and `finish` accepts only two Black and one
    /// White in the first round or three Black and two White in the
    /// second. On rejection the phase and the log stay untouched, so
    /// the user can place or remove stones and try again.
    pub fn done(&mut self) -> Result<Vec<Event>, CommandError> {
        let next = match self.phase {
            OpeningPhase::Placing3 => {
                rebuild_placing3(&self.log).finish()?;
                OpeningPhase::FirstChoice
            }
            OpeningPhase::Placing2 => {
                rebuild_placing2(&self.log).finish()?;
                OpeningPhase::FinalChoice
            }
            OpeningPhase::FirstChoice | OpeningPhase::FinalChoice => {
                return Err(CommandError::NotAtPlacementPhase);
            }
        };
        self.phase = next;
        Ok(vec![Event::OpeningPhaseChanged { phase: next }])
    }

    /// Decline the first choice and start the two-stone round.
    ///
    /// The engine transition `place_two_more` cannot fail, but the
    /// rebuild still routes through it: every protocol transition in
    /// this module goes through the typestate, so the Swap2 rules live
    /// in exactly one place — the engine.
    pub fn more(&mut self) -> Result<Vec<Event>, CommandError> {
        match self.phase {
            OpeningPhase::FirstChoice => {
                let _ = rebuild_first_choice(&self.log).place_two_more();
                self.phase = OpeningPhase::Placing2;
                Ok(vec![Event::OpeningPhaseChanged {
                    phase: OpeningPhase::Placing2,
                }])
            }
            _ => Err(CommandError::NotAtFirstChoice),
        }
    }

    /// Choose a color at a choice phase and report the finished board.
    ///
    /// The board comes from the engine's `take_black` / `take_white`,
    /// which guarantee White to move, because White always has fewer
    /// stones after the ceremony. The method is read-only: the session
    /// decides what the board becomes (the root of a fresh variation
    /// tree) and clears the opening state.
    pub fn choose(&self, color: Color) -> Result<Board, CommandError> {
        match self.phase {
            OpeningPhase::FirstChoice => {
                let state = rebuild_first_choice(&self.log);
                Ok(match color {
                    Color::Black => state.take_black(),
                    Color::White => state.take_white(),
                })
            }
            OpeningPhase::FinalChoice => {
                let state = rebuild_final_choice(&self.log);
                Ok(match color {
                    Color::Black => state.take_black(),
                    Color::White => state.take_white(),
                })
            }
            OpeningPhase::Placing3 | OpeningPhase::Placing2 => Err(CommandError::NotAtChoicePhase),
        }
    }
}

impl Default for OpeningState {
    fn default() -> Self {
        Self::new()
    }
}

/// Replay the log into a fresh `Swap2<Placing3>`.
///
/// Every entry was validated by the engine when it was appended, so
/// the replay cannot fail. The `expect` documents that invariant; it
/// is not error handling.
fn rebuild_placing3(log: &[(Move, Color)]) -> Swap2<Placing3> {
    let mut opening = Swap2::<Placing3>::new();
    for &(mv, color) in log {
        opening
            .place(mv, color)
            .expect("the log contains only engine-validated placements");
    }
    opening
}

/// Replay the log into `Swap2<Placing2>`: the first three entries are
/// the finished first round; the rest are second-round placements.
fn rebuild_placing2(log: &[(Move, Color)]) -> Swap2<Placing2> {
    debug_assert!(
        log.len() >= 3,
        "the second round starts after three placements"
    );
    let mut placing2 = rebuild_placing3(&log[..3])
        .finish()
        .expect("the first round was count-validated by `done`")
        .place_two_more();
    for &(mv, color) in &log[3..] {
        placing2
            .place(mv, color)
            .expect("the log contains only engine-validated placements");
    }
    placing2
}

/// Replay the log into `Swap2<FirstChoice>`: exactly three placements,
/// count-validated when `done` accepted them.
fn rebuild_first_choice(log: &[(Move, Color)]) -> Swap2<FirstChoice> {
    debug_assert!(
        log.len() == 3,
        "the first choice follows exactly three placements"
    );
    rebuild_placing3(log)
        .finish()
        .expect("the first round was count-validated by `done`")
}

/// Replay the log into `Swap2<FinalChoice>`: exactly five placements,
/// count-validated when the second `done` accepted them.
fn rebuild_final_choice(log: &[(Move, Color)]) -> Swap2<FinalChoice> {
    debug_assert!(
        log.len() == 5,
        "the final choice follows exactly five placements"
    );
    rebuild_placing2(log)
        .finish()
        .expect("the second round was count-validated by `done`")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    #[test]
    fn the_default_color_alternates_per_round_starting_with_black() {
        let mut opening = OpeningState::new();
        assert_eq!(opening.default_next_color(), Some(Color::Black));
        opening.place(mv(7, 7), None).unwrap();
        assert_eq!(opening.default_next_color(), Some(Color::White));
        opening.place(mv(8, 8), None).unwrap();
        assert_eq!(opening.default_next_color(), Some(Color::Black));
        opening.place(mv(9, 9), None).unwrap();
        opening.done().unwrap();
        // A choice phase has no next stone.
        assert_eq!(opening.default_next_color(), None);
        opening.more().unwrap();
        // The second round starts its alternation over, black first.
        assert_eq!(opening.default_next_color(), Some(Color::Black));
        opening.place(mv(5, 5), None).unwrap();
        assert_eq!(opening.default_next_color(), Some(Color::White));
    }

    #[test]
    fn undo_recomputes_the_phase_from_the_log_length() {
        let mut opening = OpeningState::new();
        opening.place(mv(7, 7), None).unwrap();
        opening.place(mv(8, 8), None).unwrap();
        opening.place(mv(9, 9), None).unwrap();
        opening.done().unwrap();
        opening.more().unwrap();
        opening.place(mv(5, 5), None).unwrap();
        assert_eq!(opening.phase, OpeningPhase::Placing2);

        // Removing the fourth stone crosses the round boundary: three
        // stones on the log means the first round is open again and a
        // fresh `done` is required.
        opening.undo(1).unwrap();
        assert_eq!(opening.phase, OpeningPhase::Placing3);
        opening.done().unwrap();
        assert_eq!(opening.phase, OpeningPhase::FirstChoice);
    }

    #[test]
    fn the_snapshot_mirrors_phase_log_and_default_color() {
        let mut opening = OpeningState::new();
        opening.place(mv(7, 7), None).unwrap();
        opening.place(mv(8, 8), Some(Color::Black)).unwrap();
        let snapshot = opening.snapshot();
        assert_eq!(snapshot.phase, OpeningPhase::Placing3);
        assert_eq!(
            snapshot.placements,
            vec![(mv(7, 7), Color::Black), (mv(8, 8), Color::Black)]
        );
        assert_eq!(snapshot.default_next_color, Some(Color::Black));
    }
}
