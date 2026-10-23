//! Session core — headless state machine.
//!
//! `SessionCore` is the heart of the study table. It owns the variation
//! tree, the seat table, and AI search defaults. Commands go in; events
//! and snapshots come out. No terminal code, no I/O.

use engine::{Board, Color, Move, Status};
use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::actor::{AiConfig, Occupant, OccupantKind};
use crate::command::{Command, CommandError, Event, SeatSel};
use crate::history::{ForwardOutcome, History, PlayOutcome};

#[derive(Debug, Clone, PartialEq)]
pub struct SessionInit {
    /// Black seat occupant
    pub black: Occupant,
    /// White seat occupant
    pub white: Occupant,
    /// Default AI config, applied when releasing a seat
    pub default_config: AiConfig,
    /// Number of MCTS simulations (override) for black
    pub black_sims: Option<u32>,
    /// Number of MCTS simulations (override) for white
    pub white_sims: Option<u32>,
    /// Optional RNG seed for reproducible sessions
    pub seed: Option<u64>,
    /// opening board, the root of the variation tree
    pub opening: Board,
}

impl SessionInit {
    /// start the hot-seat sesssion on an empty board
    #[must_use]
    pub fn new_hotseat() -> Self {
        Self {
            black: Occupant::Human,
            white: Occupant::Human,
            default_config: AiConfig::default(),
            black_sims: None,
            white_sims: None,
            seed: None,
            opening: Board::new(),
        }
    }

    #[must_use]
    pub fn new_hotseat_from_opening(opening: &Board) -> Self {
        Self {
            black: Occupant::Human,
            white: Occupant::Human,
            default_config: AiConfig::default(),
            black_sims: None,
            white_sims: None,
            seed: None,
            opening: opening.clone(),
        }
    }
}

/// Read-only snapshot of a session, suitable for rendering or JSON export
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// Cursor ply in the variation tree.
    pub ply: u32,

    pub branch_options: Vec<(usize, Move)>,

    pub black: OccupantKind,

    pub white: OccupantKind,

    pub status: Status,

    pub last_move: Option<Move>,
}

pub struct SessionCore {
    history: History,
    black: Occupant,
    white: Occupant,
    default_config: AiConfig,
    black_sims: Option<u32>,
    white_sims: Option<u32>,
    #[allow(dead_code)]
    rng: StdRng,
    init: SessionInit,
}

impl SessionCore {
    /// Create a new session from the given initial state.
    pub fn new(init: SessionInit) -> Self {
        let seed = init.seed.unwrap_or_else(rand::random::<u64>);
        let rng = StdRng::seed_from_u64(seed);
        Self {
            history: History::from_opening(&init.opening),
            black: init.black.clone(),
            white: init.white.clone(),
            default_config: init.default_config.clone(),
            black_sims: init.black_sims,
            white_sims: init.white_sims,
            rng,
            init,
        }
    }

    /// A serializable read-model snapshot of the current session.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let cursor = self.history.cursor();
        let children = self.history.children(cursor);
        let branch_options = children.into_iter().enumerate().collect();
        Snapshot {
            ply: self.history.ply(),
            branch_options,
            black: self.black.kind(),
            white: self.white.kind(),
            status: self.history.board().status(),
            last_move: self.history.board().moves().last().copied(),
        }
    }

    pub fn board(&self) -> Board {
        self.history.board()
    }

    pub fn execute(&mut self, cmd: Command) -> Result<Vec<Event>, CommandError> {
        match cmd {
            Command::Play(mv) => self.execute_play(mv),
            Command::Back { n } => self.execute_back(n),
            Command::Forward { n } => self.execute_forward(n),
            Command::Var { index } => self.execute_var(index),
            Command::New => self.execute_new(),
            Command::Take { seat } => self.execute_take(seat),
            Command::Step => Err(CommandError::AiSeatsNotImplemented(
                "Step requires Ai Seat".to_string(),
            )),
            Command::Release { seat } => {
                self.validate_seat_arg(seat)?;
                Err(CommandError::AiSeatsNotImplemented(
                    "Release requires AI seats (Task 3)".to_string(),
                ))
            }
            Command::SetSims { seat, n } => {
                self.validate_seat_arg(seat)?;
                if n == 0 {
                    return Err(CommandError::ZeroSims);
                }
                Err(CommandError::AiSeatsNotImplemented(
                    "SetSims requires AI seats (Task 3)".to_string(),
                ))
            }

            _ => todo!(),
        }
    }

    fn validate_seat_arg(&self, seat: SeatSel) -> Result<(), CommandError> {
        let _ = seat;
        Ok(())
    }

    pub fn execute_play(&mut self, mv: Move) -> Result<Vec<Event>, CommandError> {
        let board = self.history.board();
        if board.status() != Status::Ongoing {
            return Err(CommandError::GameOver);
        }
        let original_cursor = self.history.cursor();
        let outcome = self.history.play(mv)?;
        let mut events = Vec::new();

        let side = board.to_move();
        let by = self.occupant(side).kind();
        events.push(Event::Moved { mv, by });

        if let PlayOutcome::Moved(_) = outcome {
            let siblings = self.history.children(original_cursor);
            if siblings.len() > 1 {
                events.push(Event::Branched { options: siblings });
            }
        }

        let new_board = self.history.board();
        if new_board.status() != Status::Ongoing {
            events.push(Event::GameOver {
                result: new_board.status(),
            })
        }
        Ok(events)
    }

    pub fn execute_back(&mut self, n: u32) -> Result<Vec<Event>, CommandError> {
        let _moved = self.history.back(n);
        Ok(vec![Event::CursorMoved {
            ply: self.history.ply(),
        }])
    }

    pub fn execute_forward(&mut self, n: u32) -> Result<Vec<Event>, CommandError> {
        match self.history.forward(n) {
            ForwardOutcome::Moved(_) => Ok(vec![Event::CursorMoved {
                ply: self.history.ply(),
            }]),

            ForwardOutcome::NeedsChoice(options) => {
                let indexed: Vec<_> = options.into_iter().enumerate().collect();
                Ok(vec![Event::BranchPoint { options: indexed }])
            }

            ForwardOutcome::AtLeaf => Err(CommandError::ForwardAtLeaf),
        }
    }

    pub fn execute_var(&mut self, index: usize) -> Result<Vec<Event>, CommandError> {
        let cursor = self.history.cursor();
        let children = self.history.children(cursor);
        if children.is_empty() {
            return Err(CommandError::NotAtBranchPoint);
        }
        if index >= children.len() {
            return Err(CommandError::VarOutOfBounds {
                index,
                count: children.len(),
            });
        }
        self.history.var(index)?;
        Ok(vec![Event::CursorMoved {
            ply: self.history.ply(),
        }])
    }

    pub fn execute_take(&mut self, seat: SeatSel) -> Result<Vec<Event>, CommandError> {
        let mut events = Vec::new();
        self.apply_to_seats(seat, |this, color| {
            let new_occupant = Occupant::Human;
            this.set_occupant(color, new_occupant);
            events.push(Event::SeatChanged {
                seat: color,
                now: OccupantKind::Human,
            })
        });
        Ok(events)
    }

    fn apply_to_seats<F>(&mut self, seat: SeatSel, mut f: F)
    where
        F: FnMut(&mut Self, Color),
    {
        match seat {
            SeatSel::Black => f(self, Color::Black),
            SeatSel::White => f(self, Color::White),
            SeatSel::Both => {
                f(self, Color::Black);
                f(self, Color::White);
            }
        }
    }

    pub fn execute_new(&mut self) -> Result<Vec<Event>, CommandError> {
        self.reset();
        Ok(vec![Event::Reset])
    }

    fn occupant(&self, color: Color) -> &Occupant {
        match color {
            Color::Black => &self.black,
            Color::White => &self.white,
        }
    }

    fn set_occupant(&mut self, color: Color, occupant: Occupant) {
        match color {
            Color::Black => self.black = occupant,
            Color::White => self.white = occupant,
        }
    }

    fn reset(&mut self) {
        self.history = History::from_opening(&self.init.opening);
        self.black = self.init.black.clone();
        self.white = self.init.white.clone();
        self.default_config = self.init.default_config.clone();
        self.black_sims = self.init.black_sims;
        self.white_sims = self.init.white_sims;
        // Keep the same RNG; reset does not reseed.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::PlayError;

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    fn play(core: &mut SessionCore, r: u8, c: u8) -> Vec<Event> {
        core.execute(Command::Play(mv(r, c))).unwrap()
    }

    // Play a list of moves that come as array of u8, interpreted in pairs
    fn play_some(core: &mut SessionCore, moves: &[u8]) {
        let (coords, _) = moves.as_chunks::<2>();
        for [r, c] in coords.iter() {
            play(core, *r, *c);
        }
    }

    #[test]
    fn new_session_has_empty_root_and_human_seats() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        assert_eq!(core.board(), Board::new());
        assert_eq!(core.snapshot().ply, 0);
        assert_eq!(core.snapshot().black, OccupantKind::Human);
        assert_eq!(core.snapshot().white, OccupantKind::Human);
        assert_eq!(core.snapshot().status, Status::Ongoing);
    }

    #[test]
    fn hotseat_game_to_black_win_emits_game_over() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());

        // Five in a row for Black on row 7.
        let script = [
            (7, 3),
            (0, 1),
            (7, 4),
            (0, 2),
            (7, 5),
            (0, 3),
            (7, 6),
            (0, 4),
        ];

        for &(r, c) in script.iter() {
            let events = play(&mut core, r, c);
            assert!(
                !events.iter().any(|e| matches!(e, &Event::GameOver { .. })),
                "Game should not be over yet."
            );
        }

        let events = play(&mut core, 7, 7);
        assert!(
            events.iter().any(|e| matches!(e, Event::GameOver { .. })),
            "Game should be over by now."
        );
        assert!(
            events.iter().any(|e| matches!(e, Event::Moved { .. })),
            "Game should report the last move."
        );
    }

    #[test]
    fn illegal_move_returns_error_and_lets_board_unchanged() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        play(&mut core, 7, 7);
        let snapshot_before = core.snapshot();
        let err = core.execute(Command::Play(mv(7, 7))).unwrap_err();
        assert_eq!(err, CommandError::IllegalMove(PlayError::Occupied));
        assert_eq!(core.snapshot(), snapshot_before);
    }

    #[test]
    fn branch_mid_game_emits_branched_event() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        play_some(&mut core, &[7, 7, 6, 7, 7, 8, 6, 8, 7, 9, 6, 9]);
        core.execute(Command::Back { n: 3 }).unwrap();
        let events = play(&mut core, 8, 7);
        assert!(
            events.iter().any(|e| matches!(e, &Event::Branched { .. })),
            "Expected Branched event, got {events:?}"
        );

        let branched = events.iter().find_map(|e| match e {
            Event::Branched { options } => Some(options.clone()),
            _ => None,
        });

        assert_eq!(branched, Some(vec![mv(6, 8), mv(8, 7)]));
    }

    #[test]
    fn back_forward_var_navigation_emits_cursor_moved() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        play_some(&mut core, &[7, 7, 6, 7, 7, 8]);
        let events = core.execute(Command::Back { n: 2 }).unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, &Event::CursorMoved { ply: 1 }))
        );
        let events = core.execute(Command::Forward { n: 1 }).unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, &Event::CursorMoved { ply: 2 }))
        );

        core.execute(Command::Back { n: 1 }).unwrap();
        play(&mut core, 8, 7);
        core.execute(Command::Back { n: 1 }).unwrap();

        let events = core.execute(Command::Forward { n: 1 }).unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::BranchPoint { .. })),
            "Expected BranchPoint. Got {events:?}"
        );

        let events = core.execute(Command::Var { index: 1 }).unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::CursorMoved { ply: 2 }))
        );
    }

    #[test]
    fn new_emits_reset_and_restores_opening() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        play_some(&mut core, &[7, 7, 6, 7]);
        let events = core.execute(Command::New).unwrap();
        assert!(events.iter().any(|e| matches!(e, &Event::Reset)));
        assert_eq!(core.board(), Board::new());
        assert_eq!(core.snapshot().ply, 0);
    }

    #[test]
    fn take_is_no_op_when_seats_are_human() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        let events = core
            .execute(Command::Take {
                seat: SeatSel::Both,
            })
            .unwrap();
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| matches!(
            e,
            &Event::SeatChanged {
                now: OccupantKind::Human,
                ..
            }
        )))
    }

    #[test]
    fn step_release_set_sims_return_not_yet_implemented() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());

        assert!(matches!(
            core.execute(Command::Step),
            Err(CommandError::AiSeatsNotImplemented(_))
        ));

        assert!(matches!(
            core.execute(Command::Release {
                seat: SeatSel::White
            }),
            Err(CommandError::AiSeatsNotImplemented(_))
        ));

        assert!(matches!(
            core.execute(Command::SetSims {
                seat: SeatSel::Black,
                n: 100
            }),
            Err(CommandError::AiSeatsNotImplemented(_))
        ));
    }

    #[test]
    fn zero_sims_is_rejected() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        let err = core
            .execute(Command::SetSims {
                seat: SeatSel::Black,
                n: 0,
            })
            .unwrap_err();
        assert_eq!(err, CommandError::ZeroSims);
    }

    #[test]
    fn seeded_determinism_smoke() {
        let init = SessionInit {
            seed: Some(12345),
            ..SessionInit::new_hotseat()
        };
        let mut a = SessionCore::new(init.clone());
        let mut b = SessionCore::new(init);
        let script = [(7, 7), (6, 7), (7, 8), (6, 8)];
        for &(r, c) in &script {
            let ev_a = play(&mut a, r, c);
            let ev_b = play(&mut b, r, c);
            assert_eq!(ev_a, ev_b);
        }
        assert_eq!(a.snapshot(), b.snapshot());
    }
}
