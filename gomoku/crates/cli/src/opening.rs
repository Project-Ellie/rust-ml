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
//! and one more White after `more`). The placement order is a
//! session-level convention on top of the rule: strict alternation,
//! black first, over the whole ceremony (B, W, B, then W, B). The
//! convention loses no reachable position — every count-valid stone
//! set has an alternation order that produces it — so the session
//! fixes the order and `play` takes no color token. The engine stays
//! permissive: its typestate accepts explicit `(Move, Color)` pairs
//! with count validation, because imported game records need not
//! follow the convention.

use engine::{Board, Color, Move, Swap2};
use engine::{FinalChoice, FirstChoice, Placing2, Placing3};
use serde::{Deserialize, Serialize};

use crate::command::{CommandError, Event, OpeningPhase};

/// The read model of a ceremony in progress, exposed through
/// `Snapshot`.
///
/// A front-end draws the ceremony from this data alone. It needs no
/// Swap2 rules: the phase says which commands apply, the placements
/// say which stones are on the board, and the next color says what
/// the next `play` will place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpeningSnapshot {
    pub phase: OpeningPhase,
    pub placements: Vec<(Move, Color)>,
    pub next_color: Option<Color>,
}

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

impl Default for OpeningState {
    fn default() -> Self {
        Self::new()
    }
}

impl OpeningState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: OpeningPhase::Placing3,
            log: Vec::new(),
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> OpeningSnapshot {
        OpeningSnapshot {
            phase: self.phase,
            placements: self.log.clone(),
            next_color: self.next_color(),
        }
    }

    /// Place the next stone of the ceremony, in the alternation's
    /// color.
    ///
    /// The engine remains the rules authority: the typestate is
    /// rebuilt from the log, and its `place` rejects an occupied
    /// square. The log grows only after the engine accepts the
    /// placement, so the log can never hold a stone the engine
    /// refused.
    pub fn place(&mut self, mv: Move) -> Result<Vec<Event>, CommandError> {
        // A choice phase has no next stone; the `None` answer makes
        // `play` there a typed error without touching the engine.
        let Some(color) = self.next_color() else {
            return Err(CommandError::NotAtPlacementPhase);
        };
        match self.phase {
            OpeningPhase::Placing3 => {
                if self.log.len() == 3 {
                    return Err(CommandError::OpeningRoundFull);
                }
                rebuild_placing3(&self.log).place(mv, color)?
            }
            OpeningPhase::Placing2 => {
                if self.log.len() == 5 {
                    return Err(CommandError::OpeningRoundFull);
                }
                rebuild_placing2(&self.log).place(mv, color)?
            }
            OpeningPhase::FirstChoice | OpeningPhase::FinalChoice => {
                return Err(CommandError::NotAtPlacementPhase);
            }
        }
        self.log.push((mv, color));
        Ok(vec![Event::StonePlaced { mv, color }])
    }

    /// The color of the next stone: strict alternation, black first,
    /// over the whole ceremony.
    ///
    /// The alternation runs on the placement count alone: an even
    /// count places Black next, an odd count White, so the sequence
    /// is B, W, B in the first round and W, B in the second. Every
    /// count-valid stone set has an alternation order that reaches
    /// it, so fixing the order costs nothing. At a choice phase
    /// there is no next placement, so the answer is `None`.
    #[must_use]
    pub fn next_color(&self) -> Option<Color> {
        match self.phase {
            OpeningPhase::FirstChoice | OpeningPhase::FinalChoice => None,
            OpeningPhase::Placing3 | OpeningPhase::Placing2 => {
                Some(if self.log.len().is_multiple_of(2) {
                    Color::Black
                } else {
                    Color::White
                })
            }
        }
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
        let n_removals = (n as usize).min(self.log.len());
        for _ in 0..n_removals {
            let (mv, _) = self.log.pop().expect("length was checked.");
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
            .expect("The log contains only engine-validated placements.")
    }
    opening
}

/// Replay the log into `Swap2<Placing2>`: the first three entries are
/// the finished first round; the rest are second-round placements.
fn rebuild_placing2(log: &[(Move, Color)]) -> Swap2<Placing2> {
    debug_assert!(
        log.len() >= 3,
        "The second round starts after three placements"
    );
    let mut placing2 = rebuild_placing3(&log[..3])
        .finish()
        .expect("The first found was count-validated by `done`")
        .place_two_more();
    for &(mv, color) in &log[3..] {
        placing2
            .place(mv, color)
            .expect("The log contains only engine-validated placements.")
    }
    placing2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Event;
    use testkit::mv;

    #[test]
    fn the_color_alternates_starting_with_black() {
        let mut opening = OpeningState::new();
        assert_eq!(opening.next_color(), Some(Color::Black));
        opening.place(mv(7, 7)).unwrap();
        assert_eq!(opening.next_color(), Some(Color::White));
        opening.place(mv(8, 8)).unwrap();
        assert_eq!(opening.next_color(), Some(Color::Black));
        opening.place(mv(9, 9)).unwrap();
        opening.done().unwrap();

        assert_eq!(opening.next_color(), None);

        opening.more().unwrap();
        assert_eq!(opening.next_color(), Some(Color::White));
        opening.place(mv(10, 10)).unwrap();
        assert_eq!(opening.next_color(), Some(Color::Black));
        let res = opening.place(mv(11, 11)).unwrap();

        assert_eq!(
            res,
            vec![Event::StonePlaced {
                mv: mv(11, 11),
                color: Color::Black
            }]
        );
    }

    #[test]
    fn undo_4th_stone_goes_back_to_placing3() {
        let mut opening = OpeningState::new();
        opening.place(mv(6, 6)).unwrap();
        opening.place(mv(7, 7)).unwrap();
        opening.place(mv(8, 8)).unwrap();
        opening.done().unwrap();
        opening.more().unwrap();
        assert_eq!(opening.phase, OpeningPhase::Placing2);
        opening.place(mv(9, 9)).unwrap();
        assert_eq!(opening.phase, OpeningPhase::Placing2);

        opening.undo(1).unwrap();
        assert_eq!(opening.phase, OpeningPhase::Placing3);
        assert_eq!(opening.next_color(), Some(Color::White));
        opening.done().unwrap();
        assert_eq!(opening.phase, OpeningPhase::FirstChoice);
    }

    #[test]
    fn the_snapshot_mirrors_the_log() {
        let mut opening = OpeningState::new();
        opening.place(mv(6, 6)).unwrap();
        opening.place(mv(7, 7)).unwrap();
        let snapshot = opening.snapshot();
        assert_eq!(snapshot.phase, OpeningPhase::Placing3);
        assert_eq!(
            snapshot.placements,
            vec![(mv(6, 6), Color::Black), (mv(7, 7), Color::White)]
        );
        assert_eq!(snapshot.next_color, Some(Color::Black));
    }
}
