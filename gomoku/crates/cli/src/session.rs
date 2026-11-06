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
use crate::puzzle::{Puzzle, PuzzleError, PuzzlePack};
use engine::{Board, Color, Move, Status};
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::path::PathBuf;

/// Initial state for a session.
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
    /// opening variant Swap2
    pub swap2: bool,
    /// puzzle study context when the session was started from a pack
    pub puzzle: Option<PuzzleStudy>,
}

/// Puzzle study context carried by a session started from a pack.
///
/// The whole pack stays in memory so `next` / `prev` can load adjacent
/// puzzles without further I/O.
#[derive(Clone, Debug, PartialEq)]
pub struct PuzzleStudy {
    /// Path the pack was loaded from
    pub pack_path: PathBuf,
    /// Zero-based index
    pub index: usize,
    /// the parsed pack
    pub pack: PuzzlePack,
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
            puzzle: None,
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
            puzzle: None,
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
            swap2: false,
            puzzle: None,
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
            puzzle: None,
        }
    }

    /// Enable or disable the Swap2 opening ceremony.
    #[must_use]
    pub fn with_swap2(mut self, swap2: bool) -> Self {
        self.swap2 = swap2;
        self
    }

    /// Hotseat study session seeded from puzzle `index` of `pack`.
    ///
    /// The puzzle position becomes the root of the variation tree with
    /// the attacker to move; both seats start human. Seats and AI
    /// configuration can be adjusted afterwards with struct-update
    /// syntax, exactly as for the other constructors.
    ///
    /// # Errors
    /// Returns `PuzzleError::IndexOutOfRange` if `index` is not in the pack.
    pub fn puzzle(pack_path: PathBuf, pack: PuzzlePack, index: usize) -> Result<Self, PuzzleError> {
        let opening = pack.get(index)?.board.clone();
        Ok(Self {
            black: Occupant::Human,
            white: Occupant::Human,
            default_config: Default::default(),
            black_sims: None,
            white_sims: None,
            seed: None,
            opening,
            swap2: false,
            puzzle: Some(PuzzleStudy {
                pack_path,
                index,
                pack,
            }),
        })
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

/// Headless session state machine.
pub struct SessionCore {
    history: History,
    black: Occupant,
    white: Occupant,
    // Evaluators are runtime state, not clonable/serializable seat config.
    black_player: Option<player::AiPlayer>,
    white_player: Option<player::AiPlayer>,
    default_config: AiConfig,
    black_sims: Option<u32>,
    white_sims: Option<u32>,
    rng: StdRng,
    init: SessionInit,
    search_info: HashMap<HistoryNodeId, SearchInfo>,
    opening: Option<OpeningState>,
    puzzle: Option<PuzzleStudy>,
}

impl SessionCore {
    /// Create a new session from the given initial state.
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
            black_player: Self::make_player(&init.black),
            white_player: Self::make_player(&init.white),
            default_config: init.default_config.clone(),
            black_sims: init.black_sims,
            white_sims: init.white_sims,
            rng,
            opening: if init.swap2 {
                Some(OpeningState::new())
            } else {
                None
            },
            puzzle: init.puzzle.clone(),
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

    /// The puzzle study context, if this session was started from a pack.
    #[must_use]
    pub fn puzzle(&self) -> Option<&PuzzleStudy> {
        self.puzzle.as_ref()
    }

    /// The board at the cursor.
    #[must_use]
    pub fn board(&self) -> Board {
        self.history.board()
    }

    /// The stored search info at the cursor, if any.
    #[must_use]
    pub fn search_info(&self) -> Option<&SearchInfo> {
        self.search_info.get(&self.history.cursor())
    }

    /// Who plays Black: a human at the keys or the AI.
    #[must_use]
    pub fn black(&self) -> &Occupant {
        &self.black
    }

    /// Who plays White.
    #[must_use]
    pub fn white(&self) -> &Occupant {
        &self.white
    }

    /// The AI configuration every AI seat plays with.
    #[must_use]
    pub fn default_config(&self) -> &AiConfig {
        &self.default_config
    }

    /// Execute one command and return the events it produced.
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
            Command::Help | Command::Quit => Ok(vec![]),
            Command::Done => self.execute_done(),
            Command::ChooseColor { color } => self.execute_choose_color(color),
            Command::More => self.execute_more(),
            Command::Hint => self.execute_hint(),
            Command::Reveal => self.execute_reveal(),
            Command::NextPuzzle => self.execute_puzzle_nav(1),
            Command::PrevPuzzle => self.execute_puzzle_nav(-1),
        }
    }

    fn execute_done(&mut self) -> Result<Vec<Event>, CommandError> {
        match &mut self.opening {
            Some(opening) => opening.done(),
            None => Err(CommandError::NotInOpening),
        }
    }

    fn execute_more(&mut self) -> Result<Vec<Event>, CommandError> {
        match &mut self.opening {
            Some(opening) => opening.more(),
            None => Err(CommandError::NotInOpening),
        }
    }

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

        let options = player::PlayerOptions {
            simulations: effective_config.simulations,
            c_puct: effective_config.c_puct,
            temperature: effective_config.temperature,
            noise_epsilon: effective_config.noise_epsilon,
            noise_alpha: 0.3,
            own: match effective_config.evaluator {
                EvaluatorKind::Baseline => player::OwnTssConfig::baseline(),
                EvaluatorKind::Tactics => player::OwnTssConfig::disabled(),
            },
            guard: match effective_config.evaluator {
                EvaluatorKind::Baseline => player::GuardConfig::baseline(),
                EvaluatorKind::Tactics => player::GuardConfig::disabled(),
            },
        };
        let player = match side {
            Color::Black => &mut self.black_player,
            Color::White => &mut self.white_player,
        }
        .as_mut()
        .expect("AI seat must own a persistent player");
        let decision = player
            .decide(&board, &options, &mut self.rng)
            .map_err(|player::DecisionError::GameOver| CommandError::GameOver)?;
        let mv = decision.mv;
        let Some(stats) = decision.search else {
            // A certified own forced win bypasses search and its statistics.
            return self.execute_play(mv);
        };
        let info = SearchInfo::from_stats(stats, &effective_config);

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

    /// Reset the session to its initial opening.
    pub fn execute_new(&mut self) -> Result<Vec<Event>, CommandError> {
        self.reset();
        Ok(vec![Event::Reset])
    }

    /// Set the simulation count for a seat or seats.
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

    /// Release a seat or seats to the AI.
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

    /// Play a move at the cursor, or place a stone during the Swap2 ceremony.
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

    /// Go back `n` plies, or undo Swap2 placements during the ceremony.
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

    /// Go forward `n` plies along the current line.
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

    /// Choose child variation `index` at the cursor.
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

    /// Take a seat or seats for the human player.
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

    fn execute_hint(&self) -> Result<Vec<Event>, CommandError> {
        let (study, puzzle) = self.puzzle_context()?;
        let (mv, _) = puzzle
            .solution
            .first()
            .ok_or(CommandError::NoSolutionLine {
                index: study.index,
                count: study.pack.items.len(),
            })?;
        Ok(vec![Event::Hint { mv: *mv }])
    }

    fn execute_reveal(&self) -> Result<Vec<Event>, CommandError> {
        let (study, puzzle) = self.puzzle_context()?;
        if puzzle.solution.is_empty() {
            return Err(CommandError::NoSolutionLine {
                index: study.index,
                count: study.pack.items.len(),
            });
        }
        Ok(vec![Event::SolutionRevealed {
            line: puzzle.solution.clone(),
        }])
    }

    /// Pack navigation for `next` (`delta` 1) and `prev` (`delta` -1).
    ///
    /// Navigation **clamps** at the pack ends rather than wrapping: the
    /// command fails with `AtFirstPuzzle` / `AtLastPuzzle` and the
    /// current puzzle stays loaded. The chosen puzzle replaces the
    /// variation tree root wholesale — every puzzle is a fresh study
    /// position; seats and AI configuration are untouched.
    fn execute_puzzle_nav(&mut self, delta: i64) -> Result<Vec<Event>, CommandError> {
        let (study, _) = self.puzzle_context()?;
        let count = study.pack.items.len();
        let next = study.index as i64 + delta;
        if next < 0 {
            return Err(CommandError::AtFirstPuzzle { count });
        }
        if next >= count as i64 {
            return Err(CommandError::AtLastPuzzle { count });
        }
        let index = next as usize;
        let board = study.pack.items[index].board.clone();
        let attacker = study.pack.items[index].attacker;

        if let Some(study) = self.puzzle.as_mut() {
            study.index = index;
        }
        self.history = History::from_opening(&board);
        self.search_info.clear();

        Ok(vec![Event::PuzzleLoaded {
            index,
            count,
            attacker,
        }])
    }

    fn puzzle_context(&self) -> Result<(&PuzzleStudy, &Puzzle), CommandError> {
        let study = self.puzzle.as_ref().ok_or(CommandError::NoPuzzleLoaded)?;
        // The index is valid by construction: navigation clamps and the
        // pack never shrinks.
        let puzzle = &study.pack.items[study.index];
        Ok((study, puzzle))
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

    /// Who occupies the seat of `color`: a human or the AI.
    #[must_use]
    pub fn occupant(&self, color: Color) -> &Occupant {
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
        let player = Self::make_player(&occupant);
        match color {
            Color::Black => {
                self.black = occupant;
                self.black_player = player;
            }
            Color::White => {
                self.white = occupant;
                self.white_player = player;
            }
        }
    }

    fn reset(&mut self) {
        self.history = History::from_opening(&self.init.opening);
        self.set_occupant(Color::Black, self.init.black.clone());
        self.set_occupant(Color::White, self.init.white.clone());
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
        self.puzzle = self.init.puzzle.clone();
        // Keep the same RNG; reset does not reseed.
    }

    fn make_player(occupant: &Occupant) -> Option<player::AiPlayer> {
        match occupant {
            Occupant::Ai(config) => Some(player::AiPlayer::new(Self::make_evaluator(
                &config.evaluator,
            ))),
            Occupant::Human => None,
        }
    }

    fn make_evaluator(kind: &EvaluatorKind) -> Box<dyn mcts::Evaluator> {
        match kind {
            // Currently, there are two.
            EvaluatorKind::Tactics => Box::new(mcts::TacticsEvaluator::new()),
            #[cfg(feature = "baseline")]
            EvaluatorKind::Baseline => {
                Box::new(baseline::BaselineEvaluator::new(baseline::Device::flex()))
            }
            #[cfg(not(feature = "baseline"))]
            EvaluatorKind::Baseline => unreachable!("Baseline feature is disabled."),
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

    /// Store `info` as the cursor position's search, for tests.
    ///
    /// Real searches store their own info; this lets a test render a
    /// deterministic overlay or value strip without running MCTS.
    #[cfg(test)]
    pub fn set_search_info(&mut self, info: SearchInfo) {
        self.search_info.insert(self.history.cursor(), info);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::PlayError;
    use std::cell::Cell;
    use std::rc::Rc;
    use testkit::mv;

    // NOTE: `play`/`play_some` stay local. They take `&mut SessionCore`,
    // and in this crate's own unit-test build `testkit`'s `SessionCore`
    // (from the `cli` rlib it links) is a different type to rustc than
    // this crate's `SessionCore`, so the testkit versions cannot be
    // called from here. Integration tests use `testkit::{play, play_some}`.
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

    struct CountingEvaluator {
        calls: Rc<Cell<u32>>,
        drops: Rc<Cell<u32>>,
        inner: mcts::TacticsEvaluator,
    }

    impl mcts::Evaluator for CountingEvaluator {
        fn evaluate(&mut self, request: mcts::EvalRequest) -> mcts::EvalResult {
            self.calls.set(self.calls.get() + 1);
            mcts::Evaluator::evaluate(&mut self.inner, request)
        }
    }

    impl Drop for CountingEvaluator {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    #[test]
    fn persistent_player_reuses_evaluator_across_steps_and_set_sims() {
        let config = AiConfig {
            simulations: 1,
            ..AiConfig::default()
        };
        let mut core = SessionCore::new(SessionInit::versus_ai(Color::White, config));
        let calls = Rc::new(Cell::new(0));
        let drops = Rc::new(Cell::new(0));
        core.black_player = Some(player::AiPlayer::new(Box::new(CountingEvaluator {
            calls: calls.clone(),
            drops: drops.clone(),
            inner: mcts::TacticsEvaluator::new(),
        })));

        core.execute(Command::Step).unwrap();
        assert_eq!(calls.get(), 2, "root plus one simulation");
        core.execute(Command::Back { n: 1 }).unwrap();
        core.execute(Command::SetSims {
            seat: SeatSel::Black,
            n: 3,
        })
        .unwrap();
        assert_eq!(drops.get(), 0, "SetSims must retain the evaluator");
        core.execute(Command::Step).unwrap();
        assert_eq!(calls.get(), 6, "same evaluator: root plus three more sims");
        let info = core.search_info().unwrap();
        assert_eq!(info.config.simulations, 3);
        assert_eq!(info.visits.iter().map(|(_, n)| n).sum::<u32>(), 3);

        core.execute(Command::Take {
            seat: SeatSel::Black,
        })
        .unwrap();
        assert_eq!(drops.get(), 1, "taking a seat drops its evaluator");
        assert!(core.black_player.is_none());
    }

    #[test]
    fn persistent_players_follow_init_release_take_and_reset() {
        let config = AiConfig {
            simulations: 1,
            ..AiConfig::default()
        };
        let init = SessionInit::versus_ai(Color::White, config);
        let mut core = SessionCore::new(init.clone());
        assert!(core.black_player.is_some());
        assert!(core.white_player.is_none());

        let calls = Rc::new(Cell::new(0));
        let drops = Rc::new(Cell::new(0));
        core.black_player = Some(player::AiPlayer::new(Box::new(CountingEvaluator {
            calls: calls.clone(),
            drops: drops.clone(),
            inner: mcts::TacticsEvaluator::new(),
        })));
        core.execute(Command::Release {
            seat: SeatSel::White,
        })
        .unwrap();
        assert_eq!(drops.get(), 0, "releasing White must retain Black's player");
        assert!(core.white_player.is_some());
        core.white_player = Some(player::AiPlayer::new(Box::new(CountingEvaluator {
            calls,
            drops: drops.clone(),
            inner: mcts::TacticsEvaluator::new(),
        })));
        core.execute(Command::SetSims {
            seat: SeatSel::Both,
            n: 3,
        })
        .unwrap();
        assert_eq!(drops.get(), 0);

        assert_eq!(core.execute(Command::New).unwrap(), vec![Event::Reset]);
        assert_eq!(drops.get(), 2, "reset discards both old evaluators");
        assert!(core.black_player.is_some());
        assert!(core.white_player.is_none());
        assert_eq!(core.black(), &init.black);
        assert_eq!(core.white(), &init.white);
        core.execute(Command::Step).unwrap();
        assert_eq!(core.search_info().unwrap().config.simulations, 1);

        core.execute(Command::Take {
            seat: SeatSel::Both,
        })
        .unwrap();
        assert!(core.black_player.is_none());
        assert!(core.white_player.is_none());
        core.execute(Command::Release {
            seat: SeatSel::Both,
        })
        .unwrap();
        assert!(core.black_player.is_some());
        assert!(core.white_player.is_some());
        core.execute(Command::Step).unwrap();
        assert_eq!(core.search_info().unwrap().config.simulations, 1);
    }

    struct NeverEvaluate;

    impl mcts::Evaluator for NeverEvaluate {
        fn evaluate(&mut self, _: mcts::EvalRequest) -> mcts::EvalResult {
            panic!("a certified own win must bypass evaluation");
        }
    }

    // Play the slice-A win-in-3 VCF script into a replayed session position:
    // Black to move forces a win starting with (7, 7).
    fn play_vcf_script(core: &mut SessionCore) {
        play_some(
            core,
            &[7, 4, 7, 3, 7, 5, 0, 0, 7, 6, 0, 2, 6, 6, 0, 4, 8, 8, 0, 6],
        );
    }

    #[test]
    fn persistent_player_proof_plays_without_search_info_or_rng_draws() {
        use rand::RngExt;

        let config = AiConfig {
            evaluator: EvaluatorKind::Baseline,
            temperature: 1.0,
            noise_epsilon: 0.25,
            explicit_selection: true,
            ..AiConfig::default()
        };
        let mut core = SessionCore::new(SessionInit {
            seed: Some(42),
            ..SessionInit::new_hotseat()
        });
        play_vcf_script(&mut core);
        core.black = Occupant::Ai(config);
        core.black_player = Some(player::AiPlayer::new(Box::new(NeverEvaluate)));
        let mut untouched_rng = StdRng::seed_from_u64(42);

        assert_eq!(
            core.execute(Command::Step).unwrap(),
            vec![Event::Moved {
                mv: mv(7, 7),
                by: OccupantKind::Ai
            }]
        );
        // The proof path stores no record at the fresh cursor; historical
        // records elsewhere in the tree are unaffected either way.
        assert!(core.search_info().is_none());
        assert_eq!(core.rng.random::<u64>(), untouched_rng.random::<u64>());
    }

    #[test]
    fn baseline_enables_own_tss_probe_but_tactics_searches() {
        // Same injected evaluator under both kinds on the VCF fixture: the
        // Baseline policy certifies before search; the Tactics policy really
        // searches and stores the record.
        for (kind, expect_proof) in [
            (EvaluatorKind::Baseline, true),
            (EvaluatorKind::Tactics, false),
        ] {
            let config = AiConfig {
                evaluator: kind,
                simulations: 5,
                explicit_selection: true,
                ..AiConfig::default()
            };
            let mut core = SessionCore::new(SessionInit {
                seed: Some(42),
                ..SessionInit::new_hotseat()
            });
            play_vcf_script(&mut core);
            let calls = Rc::new(Cell::new(0));
            core.black = Occupant::Ai(config);
            core.black_player = Some(player::AiPlayer::new(Box::new(CountingEvaluator {
                calls: calls.clone(),
                drops: Rc::new(Cell::new(0)),
                inner: mcts::TacticsEvaluator::new(),
            })));

            let events = core.execute(Command::Step).unwrap();

            if expect_proof {
                assert_eq!(calls.get(), 0, "baseline own probe precedes search");
                assert!(events.contains(&Event::Moved {
                    mv: mv(7, 7),
                    by: OccupantKind::Ai
                }));
                assert!(core.search_info().is_none());
            } else {
                assert!(calls.get() > 0, "tactics policy really searches");
                let info = core.search_info().expect("searched step stores info");
                assert_eq!(info.config.simulations, 5);
                assert_eq!(info.visits.iter().map(|(_, n)| n).sum::<u32>(), 5);
            }
        }
    }

    #[test]
    fn baseline_enables_defensive_guard_but_tactics_preserves_proposal() {
        struct PreferBlock;
        impl mcts::Evaluator for PreferBlock {
            fn evaluate(&mut self, _: mcts::EvalRequest) -> mcts::EvalResult {
                let mut policy = [0.0; 225];
                policy[mv(7, 7).index()] = 10.0;
                mcts::EvalResult { policy, value: 0.0 }
            }
        }
        let board = Board::from_position(
            &[mv(7, 2), mv(2, 0), mv(2, 2), mv(2, 4)],
            &[mv(7, 3), mv(7, 4), mv(7, 5), mv(7, 6)],
            Color::Black,
        )
        .unwrap();
        for (kind, expected) in [
            (EvaluatorKind::Baseline, mv(7, 7)),
            (EvaluatorKind::Tactics, mv(0, 0)),
        ] {
            // Initialize a cheap tactics seat, then inject a fixed evaluator.
            // This tests the CLI policy mapping without running Burn in debug.
            let config = AiConfig {
                evaluator: EvaluatorKind::Tactics,
                simulations: 0,
                explicit_selection: true,
                ..AiConfig::default()
            };
            let mut core = SessionCore::new(SessionInit {
                opening: board.clone(),
                seed: Some(42),
                ..SessionInit::versus_ai(Color::White, config.clone())
            });
            core.black = Occupant::Ai(AiConfig {
                evaluator: kind,
                ..config
            });
            core.black_player = Some(player::AiPlayer::new(Box::new(PreferBlock)));
            let events = core.execute(Command::Step).unwrap();
            assert!(events.contains(&Event::Moved {
                mv: expected,
                by: OccupantKind::Ai
            }));
            assert_eq!(core.board().stone_at(expected), Some(Color::Black));
            let info = core.search_info().expect("guard keeps real search info");
            assert!(info.visits.iter().all(|(_, n)| *n == 0));
            assert_eq!(info.prior[0].0, mv(7, 7));
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

    #[test]
    fn seat_accessors_read_their_properties() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        assert_eq!(core.black(), &Occupant::Human);
        assert_eq!(core.white(), &Occupant::Human);
        assert_eq!(core.occupant(Color::White), core.white());
        assert_eq!(core.occupant(Color::Black), core.black());
        assert_eq!(core.default_config(), &AiConfig::default());
    }
}
