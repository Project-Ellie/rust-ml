//! Session core — headless state machine.
//!
//! `SessionCore` is the heart of the study table. It owns the variation
//! tree, the seat table, and AI search defaults. Commands go in; events
//! and snapshots come out. No terminal code, no I/O.

use crate::actor::{AiConfig, EvaluatorKind, Occupant, OccupantKind, SearchInfo};
use crate::command::{Command, CommandError, Event, SeatSel};
use crate::history::NodeId as HistoryNodeId;
use crate::history::{ForwardOutcome, History, PlayOutcome};
use engine::{Board, Color, Move, Status};
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;

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
    /// Start a hotseat session on an empty board.
    #[must_use]
    pub fn new_hotseat() -> Self {
        Self::hotseat()
    }

    /// Start a hotseat session from a custom opening position.
    #[must_use]
    pub fn new_hotseat_from_opening(opening: &Board) -> Self {
        Self::hotseat_from_opening(opening)
    }

    /// Human vs human on an empty board.
    #[must_use]
    pub fn hotseat() -> Self {
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

    /// Human vs human from a custom opening position.
    #[must_use]
    pub fn hotseat_from_opening(opening: &Board) -> Self {
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

    /// One human seat and one AI seat on an empty board.
    ///
    /// `human_color` is the human seat; the opposite seat is AI using
    /// `ai_config`.
    #[must_use]
    pub fn versus_ai(human_color: Color, ai_config: AiConfig) -> Self {
        let (black, white) = match human_color {
            Color::Black => (Occupant::Human, Occupant::Ai(ai_config.clone())),
            Color::White => (Occupant::Ai(ai_config.clone()), Occupant::Human),
        };
        Self {
            black,
            white,
            default_config: ai_config,
            black_sims: None,
            white_sims: None,
            seed: None,
            opening: Board::new(),
        }
    }

    /// AI vs AI on an empty board.
    ///
    /// `config` configures both seats. `seed` makes the session reproducible.
    #[must_use]
    pub fn ai_vs_ai(config: AiConfig, seed: u64) -> Self {
        Self {
            black: Occupant::Ai(config.clone()),
            white: Occupant::Ai(config.clone()),
            default_config: config,
            black_sims: None,
            white_sims: None,
            seed: Some(seed),
            opening: Board::new(),
        }
    }
}

/// Read-only snapshot of a session, suitable for rendering or JSON export
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// Cursor ply in the variation tree.
    pub ply: u32,
    /// the branches from this position
    pub branch_options: Vec<(usize, Move)>,
    /// ai or human
    pub black: OccupantKind,
    /// ai or human
    pub white: OccupantKind,
    /// game status
    pub status: Status,
    /// most recent move
    pub last_move: Option<Move>,
}

impl Serialize for Snapshot {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Snapshot", 6)?;
        state.serialize_field("ply", &self.ply)?;
        state.serialize_field("branch_options", &self.branch_options)?;
        state.serialize_field("black", &self.black)?;
        state.serialize_field("white", &self.white)?;
        state.serialize_field("status", &status_as_str(self.status))?;
        state.serialize_field("last_move", &self.last_move)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Snapshot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct SnapshotHelper {
            ply: u32,
            branch_options: Vec<(usize, Move)>,
            black: OccupantKind,
            white: OccupantKind,
            status: String,
            last_move: Option<Move>,
        }
        let helper = SnapshotHelper::deserialize(deserializer)?;
        Ok(Snapshot {
            ply: helper.ply,
            branch_options: helper.branch_options,
            black: helper.black,
            white: helper.white,
            status: status_from_str(&helper.status).map_err(serde::de::Error::custom)?,
            last_move: helper.last_move,
        })
    }
}

fn status_as_str(status: Status) -> &'static str {
    match status {
        Status::Ongoing => "ongoing",
        Status::Won(Color::Black) => "won_black",
        Status::Won(Color::White) => "won_white",
        Status::Draw => "draw",
    }
}

fn status_from_str(s: &str) -> Result<Status, String> {
    match s {
        "ongoing" => Ok(Status::Ongoing),
        "won_black" => Ok(Status::Won(Color::Black)),
        "won_white" => Ok(Status::Won(Color::White)),
        "draw" => Ok(Status::Draw),
        _ => Err(format!("unknown status string {s}")),
    }
}

pub struct SessionCore {
    history: History,
    black: Occupant,
    white: Occupant,
    default_config: AiConfig,
    black_sims: Option<u32>,
    white_sims: Option<u32>,
    rng: StdRng,
    init: SessionInit,
    search_info: HashMap<HistoryNodeId, SearchInfo>,
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
            search_info: HashMap::new(),
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

    #[must_use]
    pub fn board(&self) -> Board {
        self.history.board()
    }

    #[must_use]
    pub fn search_info(&self) -> Option<&SearchInfo> {
        self.search_info.get(&self.history.cursor())
    }

    pub fn execute(&mut self, cmd: Command) -> Result<Vec<Event>, CommandError> {
        match cmd {
            Command::Play(mv) => self.execute_play(mv),
            Command::Back { n } => self.execute_back(n),
            Command::Forward { n } => self.execute_forward(n),
            Command::Var { index } => self.execute_var(index),
            Command::New => self.execute_new(),
            Command::Take { seat } => self.execute_take(seat),
            Command::Step => self.execute_step(),
            Command::Release { seat } => self.execute_release(seat),
            Command::SetSims { seat, n } => self.execute_set_sims(seat, n),

            Command::Help | Command::Quit => Ok(vec![]),
        }
    }

    fn execute_step(&mut self) -> Result<Vec<Event>, CommandError> {
        let board = self.history.board();
        if board.status() != Status::Ongoing {
            return Err(CommandError::GameOver);
        }
        let side = board.to_move();
        let config = match self.occupant(side) {
            Occupant::Ai(cfg) => cfg.clone(),
            Occupant::Human => {
                return Err(CommandError::StepRequiresAi { seat: side });
            }
        };

        let mut effective_config = config.clone();
        if self.opening_schedule_active(side) {
            effective_config.temperature = 1.0;
            effective_config.noise_epsilon = 0.25;
        }

        let mut evaluator = Self::make_evaluator(&effective_config.evaluator);
        let search_cfg = mcts::SearchConfig {
            simulations: effective_config.simulations,
            c_puct: effective_config.c_puct,
        };

        let mut outcome = mcts::search(&board, &mut *evaluator, &search_cfg);

        if effective_config.noise_epsilon > 0.0 {
            mcts::add_dirichlet_noise(
                &mut outcome.tree,
                outcome.root,
                effective_config.noise_epsilon,
                0.3,
                &mut self.rng,
            );
        }

        let info = SearchInfo::from_search(&outcome, &board, &effective_config);
        let visits = mcts::visit_distribution(&outcome.tree, outcome.root);
        let mv = mcts::select_move(&visits, effective_config.temperature, &mut self.rng)
            .expect("AI must select a move from a non-empty distribution");

        let old_cursor = self.history.cursor();
        let play_outcome = self.history.play(mv)?;
        let new_cursor = self.history.cursor();
        self.search_info.insert(new_cursor, info);

        let mut events = Vec::new();
        events.push(Event::Moved {
            mv,
            by: OccupantKind::Ai,
        });

        if matches!(play_outcome, PlayOutcome::Moved(_)) {
            let siblings = self.history.children(old_cursor);
            if siblings.len() > 1 {
                events.push(Event::Branched { options: siblings });
            }
        }

        let new_board = self.history.board();
        if new_board.status() != Status::Ongoing {
            events.push(Event::GameOver {
                result: new_board.status(),
            });
        }

        Ok(events)
    }

    pub fn execute_new(&mut self) -> Result<Vec<Event>, CommandError> {
        self.reset();
        Ok(vec![Event::Reset])
    }

    pub fn execute_set_sims(
        &mut self,
        seat: SeatSel,
        sims: u32,
    ) -> Result<Vec<Event>, CommandError> {
        self.validate_seat_arg(seat)?;
        if sims == 0 {
            return Err(CommandError::ZeroSims);
        }
        let mut events = Vec::new();
        self.apply_to_seats(seat, |this, color| {
            match color {
                Color::Black => this.black_sims = Some(sims),
                Color::White => this.white_sims = Some(sims),
            }
            if let Occupant::Ai(config) = this.occupant_mut(color) {
                config.simulations = sims;
            }
            events.push(Event::ConfigChanged { seat: color, sims })
        });

        Ok(events)
    }

    pub fn execute_release(&mut self, seat: SeatSel) -> Result<Vec<Event>, CommandError> {
        self.validate_seat_arg(seat)?;
        let mut events = Vec::new();
        self.apply_to_seats(seat, |this, color| {
            let sims = match color {
                Color::Black => this.black_sims,
                Color::White => this.white_sims,
            };
            let mut config = this.default_config.clone();
            if let Some(s) = sims {
                config.simulations = s;
            }
            this.set_occupant(color, Occupant::Ai(config));
            events.push(Event::SeatChanged {
                seat: color,
                now: OccupantKind::Ai,
            })
        });
        Ok(events)
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

    fn occupant(&self, color: Color) -> &Occupant {
        match color {
            Color::Black => &self.black,
            Color::White => &self.white,
        }
    }

    fn occupant_mut(&mut self, color: Color) -> &mut Occupant {
        match color {
            Color::Black => &mut self.black,
            Color::White => &mut self.white,
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
        self.search_info.clear();
        // Keep the same RNG; reset does not reseed.
    }

    fn make_evaluator(kind: &EvaluatorKind) -> Box<dyn mcts::Evaluator> {
        match kind {
            // Currently, there's only one. Will have two more, soon.
            EvaluatorKind::Tactics => Box::new(mcts::TacticsEvaluator::new()),
        }
    }

    /// [experiment] Opening schedule for AI-vs-AI from an empty root.
    ///
    /// When the user has not explicitly configured temperature or
    /// Dirichlet noise, the first 10 plies of each line use temperature
    /// 1.0 with ε = 0.25 / α = 0.3 Dirichlet noise to introduce opening
    /// variety. After ply 10 (and for any explicitly configured session)
    /// the configured values are used verbatim.
    ///
    /// The schedule is keyed to the cursor ply along the current path, so
    /// branching back into the first 10 plies re-enters the schedule.
    fn opening_schedule_active(&self, _side: Color) -> bool {
        self.init.opening == Board::new()
            && self.black.is_ai()
            && self.white.is_ai()
            && !self.default_config.explicit_selection
            && self.default_config.temperature == 0.0
            && self.default_config.noise_epsilon == 0.0
            && self.history.ply() < 10
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
    fn release_seats_ai_and_take_reclaims_it() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());

        let events = core
            .execute(Command::Release {
                seat: SeatSel::White,
            })
            .unwrap();
        assert!(events.iter().any(|e| matches!(
            e,
            Event::SeatChanged {
                now: OccupantKind::Ai,
                ..
            }
        )));
        assert_eq!(core.snapshot().white, OccupantKind::Ai);

        let events = core
            .execute(Command::Take {
                seat: SeatSel::White,
            })
            .unwrap();
        assert!(events.iter().any(|e| matches!(
            e,
            Event::SeatChanged {
                seat: Color::White,
                now: OccupantKind::Human,
            }
        )));
        assert_eq!(core.snapshot().white, OccupantKind::Human);
    }

    #[test]
    fn set_sims_updates_and_emits_config_changed() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());

        let events = core
            .execute(Command::SetSims {
                seat: SeatSel::Black,
                n: 123,
            })
            .unwrap();
        assert_eq!(events.len(), 1);
        assert!(events.iter().any(|e| matches!(
            e,
            Event::ConfigChanged {
                seat: Color::Black,
                sims: 123,
            }
        )));
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
