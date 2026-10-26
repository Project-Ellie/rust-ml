//! Session core — headless state machine.
//!
//! `SessionCore` is the heart of the study table. It owns the variation
//! tree, the seat table, and AI search defaults. Commands go in; events
//! and snapshots come out. No terminal code, no I/O.

use crate::actor::{AiConfig, EvaluatorKind, Occupant, OccupantKind, SearchInfo};
use crate::command::{Command, CommandError, Event, SeatSel};
use crate::history::NodeId as HistoryNodeId;
use crate::history::{ForwardOutcome, History, PlayOutcome};
use crate::opening::{OpeningSnapshot, OpeningState};
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
    /// Start inside the Swap2 opening ceremony instead of at an empty
    /// tree. Requires `opening` to be an empty board; [`SessionCore::new`]
    /// asserts this.
    pub swap2: bool,
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
            swap2: false,
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
            swap2: false,
        }
    }

    /// Enable or disable the Swap2 opening ceremony.
    ///
    /// A ceremony requires an empty `opening` board: ceremony stone
    /// counts can differ by two mid-round, which no valid game board
    /// can hold. [`SessionCore::new`] asserts the combination, because
    /// mixing a ceremony with a preset position is a programming
    /// error, not user input.
    #[must_use]
    pub fn with_swap2(mut self, swap2: bool) -> Self {
        self.swap2 = swap2;
        self
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
            swap2: false,
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
            swap2: false,
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
    /// the Swap2 ceremony state, `None` when no ceremony is in progress
    pub opening: Option<OpeningSnapshot>,
}

impl Serialize for Snapshot {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        // The `opening` field is written only while a ceremony is in
        // progress, so the wire format of an ordinary session is
        // byte-identical to before the Swap2 sub-chapter.
        let field_count = if self.opening.is_some() { 7 } else { 6 };
        let mut state = serializer.serialize_struct("Snapshot", field_count)?;
        state.serialize_field("ply", &self.ply)?;
        state.serialize_field("branch_options", &self.branch_options)?;
        state.serialize_field("black", &self.black)?;
        state.serialize_field("white", &self.white)?;
        state.serialize_field("status", &status_as_str(self.status))?;
        state.serialize_field("last_move", &self.last_move)?;
        if let Some(opening) = &self.opening {
            state.serialize_field("opening", opening)?;
        }
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
            // `Option` fields are implicitly optional on the wire: a
            // snapshot written before this field existed still parses.
            opening: Option<OpeningSnapshot>,
        }
        let helper = SnapshotHelper::deserialize(deserializer)?;
        Ok(Snapshot {
            ply: helper.ply,
            branch_options: helper.branch_options,
            black: helper.black,
            white: helper.white,
            status: status_from_str(&helper.status).map_err(serde::de::Error::custom)?,
            last_move: helper.last_move,
            opening: helper.opening,
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
    /// The Swap2 ceremony state; `None` outside the ceremony. While
    /// this is `Some`, the variation tree still holds the (empty)
    /// initial board: ceremony placements are not game moves and enter
    /// the tree only as the finished opening board.
    opening: Option<OpeningState>,
}

impl SessionCore {
    /// Create a new session from the given initial state.
    ///
    /// # Panics
    /// Panics when `init.swap2` is set on a non-empty opening board.
    /// That combination is a programming error, not user input, so it
    /// is an assert rather than a typed error.
    pub fn new(init: SessionInit) -> Self {
        // A Swap2 ceremony is not a game position: its stone counts can
        // differ by two mid-round, which no valid `Board` can hold. A
        // ceremony on top of a preset opening would mix two sources of
        // truth, so the combination is rejected at construction.
        assert!(
            !init.swap2 || init.opening == Board::new(),
            "swap2 requires an empty opening board"
        );
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
            opening: if init.swap2 {
                Some(OpeningState::new())
            } else {
                None
            },
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
            opening: self.opening.as_ref().map(OpeningState::snapshot),
        }
    }

    /// The board at the cursor.
    ///
    /// During a Swap2 ceremony this is the (empty) initial board:
    /// ceremony placements are not game moves, and their mid-round
    /// counts can be illegal in a game position, which no valid `Board`
    /// can hold. The stones in play are in `snapshot().opening`.
    #[must_use]
    pub fn board(&self) -> Board {
        self.history.board()
    }

    #[must_use]
    pub fn search_info(&self) -> Option<&SearchInfo> {
        self.search_info.get(&self.history.cursor())
    }

    /// The occupant of the Black seat.
    #[must_use]
    pub fn black(&self) -> &Occupant {
        &self.black
    }

    /// The occupant of the White seat.
    #[must_use]
    pub fn white(&self) -> &Occupant {
        &self.white
    }

    /// The default AI configuration, applied when a seat is released to
    /// the AI.
    #[must_use]
    pub fn default_config(&self) -> &AiConfig {
        &self.default_config
    }

    /// The occupant of the seat for `color`.
    #[must_use]
    pub fn occupant(&self, color: Color) -> &Occupant {
        match color {
            Color::Black => &self.black,
            Color::White => &self.white,
        }
    }

    pub fn execute(&mut self, cmd: Command) -> Result<Vec<Event>, CommandError> {
        match cmd {
            Command::Play { mv } => self.execute_play(mv),
            Command::Back { n } => self.execute_back(n),
            Command::Forward { n } => self.execute_forward(n),
            Command::Var { index } => self.execute_var(index),
            Command::New => self.execute_new(),
            Command::Take { seat } => self.execute_take(seat),
            Command::Step => self.execute_step(),
            Command::Release { seat } => self.execute_release(seat),
            Command::SetSims { seat, n } => self.execute_set_sims(seat, n),
            Command::Done => self.execute_done(),
            Command::ChooseColor { color } => self.execute_choose_color(color),
            Command::More => self.execute_more(),

            Command::Help | Command::Quit => Ok(vec![]),
        }
    }

    /// Route `done` into the ceremony, or reject it outside one.
    fn execute_done(&mut self) -> Result<Vec<Event>, CommandError> {
        match &mut self.opening {
            Some(opening) => opening.done(),
            None => Err(CommandError::NotInOpening),
        }
    }

    /// Route `more` into the ceremony, or reject it outside one.
    fn execute_more(&mut self) -> Result<Vec<Event>, CommandError> {
        match &mut self.opening {
            Some(opening) => opening.more(),
            None => Err(CommandError::NotInOpening),
        }
    }

    /// Choosing a color ends the ceremony: the engine hands back the
    /// finished opening board, which becomes the root of a fresh
    /// variation tree, and the normal command set takes over.
    fn execute_choose_color(&mut self, color: Color) -> Result<Vec<Event>, CommandError> {
        let Some(opening) = &self.opening else {
            return Err(CommandError::NotInOpening);
        };
        let board = opening.choose(color)?;
        self.history = History::from_opening(&board);
        self.opening = None;
        Ok(vec![Event::OpeningFinished { chosen: color }])
    }

    fn execute_step(&mut self) -> Result<Vec<Event>, CommandError> {
        // The AI searches game positions. Ceremony stones are not a
        // game position, and a move searched now would be discarded
        // when the ceremony finishes and resets the tree. Refuse
        // instead.
        if self.opening.is_some() {
            return Err(CommandError::OpeningInProgress);
        }
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
        // Phase routing: during the ceremony `play` places a stone in
        // the alternation's color; afterwards `play` is a tree move.
        if let Some(opening) = &mut self.opening {
            return opening.place(mv);
        }
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
        // `back` is phase-dependent: it removes placements during the
        // ceremony and navigates the variation tree afterwards.
        if let Some(opening) = &mut self.opening {
            return opening.undo(n);
        }
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
        // `new` returns the session to its initial phase. A Swap2
        // session started in the ceremony, so it resets into the
        // ceremony.
        self.opening = if self.init.swap2 {
            Some(OpeningState::new())
        } else {
            None
        };
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
    use testkit::mv;

    fn play(core: &mut SessionCore, r: u8, c: u8) -> Vec<Event> {
        core.execute(Command::Play { mv: mv(r, c) }).unwrap()
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
        let err = core.execute(Command::Play { mv: mv(7, 7) }).unwrap_err();
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
    fn seat_accessors_report_the_occupants() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        assert_eq!(core.black(), &Occupant::Human);
        assert_eq!(core.white(), &Occupant::Human);
        assert_eq!(core.occupant(Color::Black), core.black());
        assert_eq!(core.occupant(Color::White), core.white());
        assert_eq!(core.default_config(), &AiConfig::default());
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
