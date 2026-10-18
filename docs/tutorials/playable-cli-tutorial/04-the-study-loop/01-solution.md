> **Opt-in solution — read only after you have tried the chapter, or if you have been stuck for more than twenty minutes.**
>
> This file quotes the reference files for chapter 04, verified green at
> commit `d9e8fec` against main's `cli` crate with chapters 01–03 in
> place. Files unchanged from chapter 03 are not quoted; they are listed
> at the end.

## `gomoku/crates/cli/src/actor.rs`

```rust
//! Seat occupants and AI configuration.
//!
//! The study table has two seats (Black and White). Each seat is occupied
//! by either a human or an AI. For Task 2 both seats are human; the AI
//! plumbing lands in Task 3, but the configuration shapes are defined now
//! so the `SessionCore` API stays stable.

use engine::{Board, Move};
use serde::{Deserialize, Serialize};

/// Record of one AI search: visit distribution, root value, and prior policy.
///
/// `value` is the estimated value of the root position **from the
/// side-to-move's perspective at search time**. The sign is derived from
/// the MCTS backup convention in `mcts::backup`: the leaf value (from the
/// leaf player's perspective) is propagated up the selection path with the
/// sign flipped at every edge. Each edge therefore accumulates a value `W`
/// from the perspective of the player who chose at its parent node. For a
/// root edge, that parent is the root, so `W` is already from the root
/// player's (i.e. side-to-move's) point of view. The root value estimate is
/// the visit-weighted average of those root-edge values:
/// `sum(W) / sum(N)`. A positive value favours the side to move.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchInfo {
    /// Visit counts for each legal move at the root, sorted by count descending.
    pub visits: Vec<(Move, u32)>,
    /// Root-edge prior probabilities after softmax (and optional Dirichlet
    /// noise), sorted by probability descending.
    pub prior: Vec<(Move, f32)>,
    /// Estimated root value from the side-to-move's perspective.
    pub value: f32,
    /// AI configuration fingerprint used for the search.
    pub config: AiConfig,
}

impl SearchInfo {
    /// Build a `SearchInfo` from an MCTS search outcome.
    ///
    /// `board` is the position that was searched; it is retained in the
    /// signature so callers document the side-to-move perspective that
    /// determines the sign of [`Self::value`].
    #[must_use]
    pub fn from_search(outcome: &mcts::SearchOutcome, _board: &Board, config: &AiConfig) -> Self {
        let edges = outcome.tree.node(outcome.root).edges();

        let mut visits: Vec<(Move, u32)> = edges.iter().map(|e| (e.mv, e.n)).collect();
        visits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.index().cmp(&b.0.index())));

        let mut prior: Vec<(Move, f32)> = edges.iter().map(|e| (e.mv, e.prior)).collect();
        prior.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.index().cmp(&b.0.index()))
        });

        let total_visits: u32 = edges.iter().map(|e| e.n).sum();
        let total_value: f32 = edges.iter().map(|e| e.w).sum();
        let value = if total_visits > 0 {
            total_value / total_visits as f32
        } else {
            0.0
        };

        Self {
            visits,
            prior,
            value,
            config: config.clone(),
        }
    }
}

/// Who is responsible for producing a seat's moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OccupantKind {
    /// A human player typing commands.
    Human,
    /// An AI player using MCTS search.
    Ai,
}

/// A concrete seat occupant, including AI configuration when applicable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Occupant {
    /// A human player.
    Human,
    /// An AI player with its search configuration.
    ///
    /// `Ai` arrives in Task 3; the variant is defined now for
    /// forward-compatibility.
    Ai(AiConfig),
}

impl Occupant {
    /// The kind of occupant, without configuration details.
    #[must_use]
    pub fn kind(&self) -> OccupantKind {
        match self {
            Occupant::Human => OccupantKind::Human,
            Occupant::Ai(_) => OccupantKind::Ai,
        }
    }

    /// Returns `true` if this seat is occupied by a human.
    #[must_use]
    pub fn is_human(&self) -> bool {
        matches!(self, Occupant::Human)
    }

    /// Returns `true` if this seat is occupied by an AI.
    #[must_use]
    pub fn is_ai(&self) -> bool {
        matches!(self, Occupant::Ai(_))
    }
}

/// Search configuration for an AI seat.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AiConfig {
    /// Which evaluator drives MCTS.
    pub evaluator: EvaluatorKind,
    /// Number of MCTS simulations per search.
    pub simulations: u32,
    /// PUCT exploration constant.
    pub c_puct: f32,
    /// Temperature applied when selecting a move from the visit distribution.
    pub temperature: f32,
    /// Dirichlet noise mix fraction (0.0 disables noise).
    pub noise_epsilon: f32,
    /// Whether `temperature` and `noise_epsilon` were set explicitly by the
    /// user (e.g. via CLI flags). When `false` and both values are at their
    /// defaults (0.0), an AI-vs-AI line from an empty root may use the
    /// opening schedule instead.
    #[serde(default)]
    pub explicit_selection: bool,
}

impl Default for AiConfig {
    /// Default AlphaZero-style search settings.
    ///
    /// - 400 simulations
    /// - c_puct 1.5
    /// - deterministic move selection (temperature 0.0)
    /// - no Dirichlet noise
    /// - selection parameters are not explicit, so the opening schedule may
    ///   apply in AI-vs-AI from an empty root
    fn default() -> Self {
        Self {
            evaluator: EvaluatorKind::Tactics,
            simulations: 400,
            c_puct: 1.5,
            temperature: 0.0,
            noise_epsilon: 0.0,
            explicit_selection: false,
        }
    }
}

/// Evaluator backend available to an AI seat.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum EvaluatorKind {
    /// The hand-woven tactics evaluator (Burn-free baseline).
    Tactics,
}
```

## `gomoku/crates/cli/src/session.rs`

```rust
//! Session core — headless state machine.
//!
//! `SessionCore` is the heart of the study table. It owns the variation
//! tree, the seat table, and AI search defaults. Commands go in; events
//! and snapshots come out. No terminal code, no I/O.

use std::collections::HashMap;

use engine::{Board, Color, Move, Status};
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::actor::{AiConfig, EvaluatorKind, Occupant, OccupantKind, SearchInfo};
use crate::command::{Command, CommandError, Event, SeatSel};
use crate::history::NodeId as HistoryNodeId;
use crate::history::{ForwardOutcome, History, PlayOutcome};

/// Initial state used to construct a `SessionCore`.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionInit {
    /// Black seat occupant.
    pub black: Occupant,
    /// White seat occupant.
    pub white: Occupant,
    /// Default AI configuration applied when releasing a seat.
    pub default_config: AiConfig,
    /// Optional per-seat simulation override for Black.
    pub black_sims: Option<u32>,
    /// Optional per-seat simulation override for White.
    pub white_sims: Option<u32>,
    /// Optional RNG seed for reproducible sessions.
    pub seed: Option<u64>,
    /// Opening board used as the root of the variation tree.
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

/// Read-only snapshot of a session, suitable for rendering or JSON export.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// Cursor ply in the variation tree.
    pub ply: u32,
    /// Branch options at the cursor, if any.
    pub branch_options: Vec<(usize, Move)>,
    /// Black seat occupant kind.
    pub black: OccupantKind,
    /// White seat occupant kind.
    pub white: OccupantKind,
    /// Game status at the cursor.
    pub status: Status,
    /// Last move on the cursor path, if any.
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
        _ => Err(format!("unknown status string: {s}")),
    }
}

/// The headless session state machine.
pub struct SessionCore {
    history: History,
    black: Occupant,
    white: Occupant,
    default_config: AiConfig,
    black_sims: Option<u32>,
    white_sims: Option<u32>,
    rng: StdRng,
    init: SessionInit,
    /// Search info stored per history node, populated when an AI search
    /// produced the move leading to that node.
    search_info: HashMap<HistoryNodeId, SearchInfo>,
}

impl SessionCore {
    /// Create a new session from the given initial state.
    #[must_use]
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

    /// Execute a single command, returning a vector of events describing
    /// what changed.
    ///
    /// # Errors
    /// Returns a typed `CommandError` for illegal moves, invalid arguments,
    /// or AI-driven commands that are not yet implemented.
    pub fn execute(&mut self, cmd: Command) -> Result<Vec<Event>, CommandError> {
        match cmd {
            Command::Play(mv) => self.execute_play(mv),
            Command::Step => self.execute_step(),
            Command::Back { n } => {
                let _moved = self.history.back(n);
                Ok(vec![Event::CursorMoved {
                    ply: self.history.ply(),
                }])
            }
            Command::Forward { n } => self.execute_forward(n),
            Command::Var { index } => self.execute_var(index),
            Command::Take { seat } => self.execute_take(seat),
            Command::Release { seat } => self.execute_release(seat),
            Command::SetSims { seat, n } => self.execute_set_sims(seat, n),
            Command::New => {
                self.reset();
                Ok(vec![Event::Reset])
            }
            Command::Help | Command::Quit => Ok(vec![]),
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

    /// The board at the current cursor, replayed from the root.
    #[must_use]
    pub fn board(&self) -> Board {
        self.history.board()
    }

    /// The `SearchInfo` stored on the cursor node, if any.
    ///
    /// A node only has search info when an AI `Step` produced the move
    /// leading to it.
    #[must_use]
    pub fn search_info(&self) -> Option<&SearchInfo> {
        self.search_info.get(&self.history.cursor())
    }

    fn execute_play(&mut self, mv: Move) -> Result<Vec<Event>, CommandError> {
        let board = self.history.board();
        if board.status() != Status::Ongoing {
            return Err(CommandError::GameOver);
        }

        let old_cursor = self.history.cursor();
        let outcome = self.history.play(mv)?;
        let mut events = Vec::new();

        let side = board.to_move();
        let by = self.occupant(side).kind();
        events.push(Event::Moved { mv, by });

        if let PlayOutcome::Moved(_) = outcome {
            // If the node we were at now has multiple children,
            // a sibling branch was created.
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

        if let PlayOutcome::Moved(_) = play_outcome {
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

    fn execute_release(&mut self, seat: SeatSel) -> Result<Vec<Event>, CommandError> {
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
            });
        });
        Ok(events)
    }

    fn execute_set_sims(&mut self, seat: SeatSel, n: u32) -> Result<Vec<Event>, CommandError> {
        self.validate_seat_arg(seat)?;
        if n == 0 {
            return Err(CommandError::ZeroSims);
        }
        let mut events = Vec::new();
        self.apply_to_seats(seat, |this, color| {
            match color {
                Color::Black => this.black_sims = Some(n),
                Color::White => this.white_sims = Some(n),
            }
            // If the seat is currently AI, update its live config too so the
            // change takes effect on the next Step.
            if let Occupant::Ai(cfg) = this.occupant_mut(color) {
                cfg.simulations = n;
            }
            events.push(Event::ConfigChanged {
                seat: color,
                sims: n,
            });
        });
        Ok(events)
    }

    fn execute_forward(&mut self, n: u32) -> Result<Vec<Event>, CommandError> {
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

    fn execute_var(&mut self, index: usize) -> Result<Vec<Event>, CommandError> {
        let cursor = self.history.cursor();
        let children = self.history.children(cursor);
        if children.is_empty() {
            return Err(CommandError::NotABranchPoint);
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

    fn execute_take(&mut self, seat: SeatSel) -> Result<Vec<Event>, CommandError> {
        self.validate_seat_arg(seat)?;
        let mut events = Vec::new();
        self.apply_to_seats(seat, |this, color| {
            let new_occupant = Occupant::Human;
            this.set_occupant(color, new_occupant);
            events.push(Event::SeatChanged {
                seat: color,
                now: OccupantKind::Human,
            });
        });
        Ok(events)
    }

    fn validate_seat_arg(&self, seat: SeatSel) -> Result<(), CommandError> {
        // SeatSel is enum-generated; all variants are valid.
        let _ = seat;
        Ok(())
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
            (0, 0),
            (7, 4),
            (0, 1),
            (7, 5),
            (0, 2),
            (7, 6),
            (0, 3),
            (7, 7),
        ];
        for (i, &(r, c)) in script.iter().enumerate() {
            let events = play(&mut core, r, c);
            let is_last = i == script.len() - 1;
            if is_last {
                assert!(
                    events.iter().any(|e| matches!(e, &Event::GameOver { .. })),
                    "last move should emit GameOver, got {events:?}"
                );
            } else {
                assert!(
                    !events.iter().any(|e| matches!(e, &Event::GameOver { .. })),
                    "game should not be over yet, got {events:?}"
                );
            }
        }

        assert_eq!(core.board().status(), Status::Won(Color::Black));

        let err = core.execute(Command::Play(mv(0, 4))).unwrap_err();
        assert_eq!(err, CommandError::GameOver);
    }

    #[test]
    fn illegal_move_returns_error_and_leaves_state_unchanged() {
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
        play(&mut core, 7, 7);
        play(&mut core, 6, 7);
        play(&mut core, 7, 8);
        play(&mut core, 6, 8);
        play(&mut core, 7, 9);
        play(&mut core, 6, 9);

        // Back three plies and play a new move: creates a branch at ply 3.
        core.execute(Command::Back { n: 3 }).unwrap();
        let events = play(&mut core, 8, 7);
        assert!(
            events.iter().any(|e| matches!(e, &Event::Branched { .. })),
            "expected Branched event, got {events:?}"
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
        play(&mut core, 7, 7);
        play(&mut core, 6, 7);
        play(&mut core, 7, 8);

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
                .any(|e| matches!(e, &Event::BranchPoint { .. })),
            "expected BranchPoint, got {events:?}"
        );

        let events = core.execute(Command::Var { index: 1 }).unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, &Event::CursorMoved { ply: 2 }))
        );
    }

    #[test]
    fn new_emits_reset_and_restores_opening() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        play(&mut core, 7, 7);
        play(&mut core, 6, 7);

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
        )));
    }

    #[test]
    fn step_on_human_seat_returns_typed_error() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        assert_eq!(
            core.execute(Command::Step).unwrap_err(),
            CommandError::StepRequiresAi { seat: Color::Black }
        );
    }

    #[test]
    fn release_sets_seat_to_ai_and_take_reclaims_it() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        let events = core
            .execute(Command::Release {
                seat: SeatSel::White,
            })
            .unwrap();
        assert!(events.iter().any(|e| matches!(
            e,
            Event::SeatChanged {
                seat: Color::White,
                now: OccupantKind::Ai,
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
    fn set_sims_updates_override_and_emits_config_changed() {
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
```

## `gomoku/crates/cli/src/bin/sessiondemo.rs`

```rust
//! Chapter-02 capstone: a headless REPL over the session core.
//!
//! Reads commands from stdin, executes them through `SessionCore`, and
//! prints the resulting events plus a compact ASCII board. Demonstrates
//! the full command language in human-vs-human (hotseat) form.
//!
//! Run interactively:
//!
//! ```text
//! cargo run -p cli --bin sessiondemo
//! ```
//!
//! Run a scripted game non-interactively:
//!
//! ```text
//! cargo run -p cli --bin sessiondemo -- --script script.txt
//! ```

use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Write, stdin, stdout};

use cli::command::Command;
use cli::fmt::render_board;
use cli::session::{SessionCore, SessionInit};

const HELP: &str = r#"Commands (case-insensitive):
  SQUARE           play a move, e.g. "h8"
  play SQUARE      same as above
  step             let the AI play
  back [N]         go back N plies (default 1)
  forward [N]      go forward N plies (default 1)
  var N            choose child variation N
  take black|white|both      make seat(s) human
  release black|white|both   release seat(s) to AI
  sims black|white N         set per-seat simulations
  info             show AI search info at the cursor
  new              reset to opening
  h, help, ?       show this help
  q, quit, exit    quit
"#;

fn parse_seed(args: &[String]) -> Option<u64> {
    args.iter()
        .position(|a| a == "--seed")
        .and_then(|i| args.get(i + 1).and_then(|s| s.parse::<u64>().ok()))
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let script_path = args
        .iter()
        .position(|a| a == "--script")
        .and_then(|i| args.get(i + 1));
    let seed = parse_seed(&args);

    let init = SessionInit {
        seed,
        ..SessionInit::new_hotseat()
    };
    let mut core = SessionCore::new(init);

    print_state(&core, "Session started. Type 'h' for help.");

    match script_path {
        Some(path) => {
            let file = File::open(path).unwrap_or_else(|e| {
                eprintln!("Cannot open script '{}': {}", path, e);
                std::process::exit(1);
            });
            let reader = BufReader::new(file);
            for line in reader.lines() {
                let line = line.unwrap_or_else(|e| {
                    eprintln!("Error reading script: {}", e);
                    std::process::exit(1);
                });
                process_line(&line, &mut core, false);
            }
        }
        None => {
            let stdin = stdin();
            print!("session> ");
            stdout().flush().unwrap();
            for line in stdin.lock().lines() {
                let line = line.expect("stdin read failed");
                let should_quit = process_line(&line, &mut core, true);
                if should_quit {
                    break;
                }
                print!("session> ");
                stdout().flush().unwrap();
            }
        }
    }
}

/// Execute one line and print feedback. Returns true if the session
/// should quit (only meaningful in interactive mode).
fn process_line(line: &str, core: &mut SessionCore, interactive: bool) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }

    if trimmed.eq_ignore_ascii_case("info") {
        print_search_info(core);
        return false;
    }

    match cli::command::parse(trimmed) {
        Ok(Command::Help) => {
            println!("{HELP}");
            return false;
        }
        Ok(Command::Quit) => {
            if interactive {
                println!("Goodbye.");
            }
            return true;
        }
        Ok(cmd) => match core.execute(cmd) {
            Ok(events) => {
                if events.is_empty() {
                    println!("(no events)");
                } else {
                    for ev in &events {
                        println!("  {ev}");
                    }
                }
                print_state(core, "");
            }
            Err(e) => {
                println!("Error: {e}");
            }
        },
        Err(e) => {
            println!("Parse error: {e}");
        }
    }
    false
}

fn print_search_info(core: &SessionCore) {
    match core.search_info() {
        None => println!("No search info at the cursor."),
        Some(info) => {
            let total_visits: u32 = info.visits.iter().map(|(_, n)| n).sum();
            println!("value {:.3}  total visits {total_visits}", info.value);
            if info.visits.is_empty() {
                println!("(no legal moves)");
                return;
            }
            let prior_by_move: std::collections::HashMap<_, _> =
                info.prior.iter().map(|(mv, p)| (*mv, *p)).collect();
            println!("top moves (visits, prior%):");
            for (i, (mv, visits)) in info.visits.iter().take(5).enumerate() {
                let prior = prior_by_move.get(mv).copied().unwrap_or(0.0);
                println!(
                    "  {}. ({}, {:.1}%) {}",
                    i + 1,
                    visits,
                    prior * 100.0,
                    cli::fmt::format_move(*mv)
                );
            }
        }
    }
}

fn print_state(core: &SessionCore, header: &str) {
    if !header.is_empty() {
        println!("{header}");
    }
    let snap = core.snapshot();
    println!(
        "ply {}  status {:?}  black {:?}  white {:?}",
        snap.ply, snap.status, snap.black, snap.white
    );
    if !snap.branch_options.is_empty() {
        let opts: Vec<String> = snap
            .branch_options
            .iter()
            .map(|(i, m)| format!("{i}:{}", cli::fmt::format_move(*m)))
            .collect();
        println!("branches: {}", opts.join("  "));
    }
    println!("{}", render_board(&core.board(), snap.last_move));
    println!();
}
```

## `gomoku/crates/cli/tests/study.rs`

```rust
//! Task-4 capstone: the study loop.
//!
//! Take/release seats mid-game, per-seat sims, and the AI-vs-AI opening
//! schedule are all exercised as scripted command transcripts.

use cli::actor::{AiConfig, EvaluatorKind, Occupant, OccupantKind};
use cli::command::{Command, Event, SeatSel};
use cli::session::{SessionCore, SessionInit};
use engine::Move;

fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}

fn play(core: &mut SessionCore, r: u8, c: u8) -> Vec<Event> {
    core.execute(Command::Play(mv(r, c))).unwrap()
}

fn step(core: &mut SessionCore) -> Vec<Event> {
    core.execute(Command::Step).unwrap()
}

fn event_type(ev: &Event) -> &'static str {
    match ev {
        Event::Moved { .. } => "Moved",
        Event::Branched { .. } => "Branched",
        Event::CursorMoved { .. } => "CursorMoved",
        Event::BranchPoint { .. } => "BranchPoint",
        Event::SeatChanged { .. } => "SeatChanged",
        Event::ConfigChanged { .. } => "ConfigChanged",
        Event::GameOver { .. } => "GameOver",
        Event::Reset => "Reset",
    }
}

fn default_ai_config() -> AiConfig {
    AiConfig {
        evaluator: EvaluatorKind::Tactics,
        simulations: 200,
        c_puct: 1.5,
        temperature: 0.0,
        noise_epsilon: 0.0,
        explicit_selection: false,
    }
}

#[test]
fn scripted_study_transcript() {
    // AI-vs-AI for 6 plies from an empty root.
    let mut core = SessionCore::new(SessionInit::ai_vs_ai(default_ai_config(), 7));
    let mut ai_moves: Vec<Move> = Vec::new();
    for _ in 0..6 {
        let events = step(&mut core);
        assert_eq!(
            events.len(),
            1,
            "a single Step should emit exactly one Moved event"
        );
        let moved = events.iter().find_map(|e| match e {
            Event::Moved { mv, .. } => Some(*mv),
            _ => None,
        });
        ai_moves.push(moved.expect("Step must produce a move"));
    }

    // Steal Black's seat.
    let events = core
        .execute(Command::Take {
            seat: SeatSel::Black,
        })
        .unwrap();
    assert_eq!(event_type(&events[0]), "SeatChanged");
    assert_eq!(core.snapshot().black, OccupantKind::Human);

    // Branch from an earlier cursor: go back 2 plies and play something
    // different than the AI's original move there.
    core.execute(Command::Back { n: 2 }).unwrap();
    let original_ply5 = ai_moves[4];
    let human_alt = if original_ply5 != mv(0, 0) {
        mv(0, 0)
    } else {
        mv(0, 1)
    };
    let events = play(&mut core, human_alt.row(), human_alt.col());
    assert_eq!(event_type(&events[0]), "Moved");
    assert!(matches!(
        events[0],
        Event::Moved {
            by: OccupantKind::Human,
            ..
        }
    ));
    assert_eq!(event_type(&events[1]), "Branched");
    if let Event::Branched { options } = &events[1] {
        assert_eq!(options.len(), 2);
        assert_eq!(options[0], original_ply5);
        assert_eq!(options[1], human_alt);
    }

    // Go back to the branch point and return to the main line.
    let events = core.execute(Command::Back { n: 1 }).unwrap();
    assert_eq!(event_type(&events[0]), "CursorMoved");

    let events = core.execute(Command::Var { index: 0 }).unwrap();
    assert_eq!(event_type(&events[0]), "CursorMoved");

    // Hand Black back to the AI and let it continue.
    let events = core
        .execute(Command::Release {
            seat: SeatSel::Black,
        })
        .unwrap();
    assert_eq!(event_type(&events[0]), "SeatChanged");
    assert_eq!(core.snapshot().black, OccupantKind::Ai);

    let events = step(&mut core);
    assert_eq!(event_type(&events[0]), "Moved");
    assert!(matches!(
        events[0],
        Event::Moved {
            by: OccupantKind::Ai,
            ..
        }
    ));

    assert_eq!(core.snapshot().black, OccupantKind::Ai);
    assert_eq!(core.snapshot().white, OccupantKind::Ai);
}

#[test]
fn opening_schedule_changes_ai_vs_ai_play() {
    let seed = 42;
    let scheduled_config = default_ai_config();
    let explicit_config = AiConfig {
        explicit_selection: true,
        ..default_ai_config()
    };

    let mut scheduled = SessionCore::new(SessionInit::ai_vs_ai(scheduled_config, seed));
    let mut explicit = SessionCore::new(SessionInit::ai_vs_ai(explicit_config, seed));

    let mut scheduled_moves = Vec::new();
    let mut explicit_moves = Vec::new();

    for ply in 0..12 {
        let scheduled_events = step(&mut scheduled);
        let explicit_events = step(&mut explicit);

        let scheduled_mv = scheduled_events.iter().find_map(|e| match e {
            Event::Moved { mv, .. } => Some(*mv),
            _ => None,
        });
        let explicit_mv = explicit_events.iter().find_map(|e| match e {
            Event::Moved { mv, .. } => Some(*mv),
            _ => None,
        });

        scheduled_moves.push(scheduled_mv.unwrap());
        explicit_moves.push(explicit_mv.unwrap());

        // Inside the scheduled window the effective config carries the
        // schedule's temperature and noise; outside it matches the default.
        let info = scheduled.search_info().expect("search info stored");
        if ply < 10 {
            assert_eq!(info.config.temperature, 1.0);
            assert_eq!(info.config.noise_epsilon, 0.25);
        } else {
            assert_eq!(info.config.temperature, 0.0);
            assert_eq!(info.config.noise_epsilon, 0.0);
        }

        // Explicit config is always applied verbatim.
        let explicit_info = explicit.search_info().expect("search info stored");
        assert_eq!(explicit_info.config.temperature, 0.0);
        assert_eq!(explicit_info.config.noise_epsilon, 0.0);
    }

    // The scheduled opening should diverge from pure argmax at least once
    // in the first 10 plies.
    assert_ne!(
        &scheduled_moves[..10],
        &explicit_moves[..10],
        "scheduled AI-vs-AI must differ from explicit argmax in the first 10 plies"
    );
}

#[test]
fn per_seat_sims_reach_search() {
    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Human,
        white: Occupant::Human,
        default_config: default_ai_config(),
        black_sims: Some(50),
        white_sims: Some(400),
        seed: Some(1),
        opening: engine::Board::new(),
    });

    core.execute(Command::Release {
        seat: SeatSel::Both,
    })
    .unwrap();

    let events = step(&mut core);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Moved {
            by: OccupantKind::Ai,
            ..
        }
    )));
    let black_info = core.search_info().expect("search info after black step");
    assert_eq!(
        black_info.visits.iter().map(|(_, n)| n).sum::<u32>(),
        50,
        "Black AI must use black_sims"
    );

    step(&mut core);
    let white_info = core.search_info().expect("search info after white step");
    assert_eq!(
        white_info.visits.iter().map(|(_, n)| n).sum::<u32>(),
        400,
        "White AI must use white_sims"
    );
}

#[test]
fn take_and_release_are_idempotent() {
    let mut core = SessionCore::new(SessionInit::hotseat());

    // Taking an already-human seat emits SeatChanged but leaves it human.
    let first = core
        .execute(Command::Take {
            seat: SeatSel::Black,
        })
        .unwrap();
    assert_eq!(event_type(&first[0]), "SeatChanged");
    assert_eq!(core.snapshot().black, OccupantKind::Human);

    let second = core
        .execute(Command::Take {
            seat: SeatSel::Black,
        })
        .unwrap();
    assert_eq!(event_type(&second[0]), "SeatChanged");
    assert_eq!(core.snapshot().black, OccupantKind::Human);

    // Releasing an already-AI seat emits SeatChanged but leaves it AI.
    core.execute(Command::Release {
        seat: SeatSel::White,
    })
    .unwrap();
    assert_eq!(core.snapshot().white, OccupantKind::Ai);

    let again = core
        .execute(Command::Release {
            seat: SeatSel::White,
        })
        .unwrap();
    assert_eq!(event_type(&again[0]), "SeatChanged");
    assert_eq!(core.snapshot().white, OccupantKind::Ai);
}
```

## `gomoku/crates/cli/tests/actor.rs`

```rust
//! Headless tests for AI seats and `SearchInfo`.
//!
//! These exercises are the Task-3 capstone: releasing a seat to the AI,
//! stepping it, and inspecting the stored search record.

use cli::actor::{AiConfig, EvaluatorKind, Occupant, OccupantKind};
use cli::command::{Command, CommandError, Event, SeatSel};
use cli::session::{SessionCore, SessionInit};
use engine::{Board, Color, Move, Status};

fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}

fn play(core: &mut SessionCore, r: u8, c: u8) -> Vec<Event> {
    core.execute(Command::Play(mv(r, c))).unwrap()
}

fn ai_config_tactics() -> AiConfig {
    AiConfig {
        evaluator: EvaluatorKind::Tactics,
        simulations: 200,
        c_puct: 1.5,
        temperature: 0.0,
        noise_epsilon: 0.0,
        explicit_selection: false,
    }
}

#[test]
fn tactics_ai_takes_win_in_one() {
    // Black to move has an open four on row 7; playing (7,2) or (7,7) wins.
    let mut board = Board::new();
    let script = [
        (7, 3),
        (0, 0),
        (7, 4),
        (0, 1),
        (7, 5),
        (0, 2),
        (7, 6),
        (0, 3),
    ];
    for &(r, c) in &script {
        board.play(mv(r, c)).unwrap();
    }
    assert_eq!(board.to_move(), Color::Black);
    let wins = engine::immediate_wins(&board, Color::Black);
    assert!(
        !wins.is_empty(),
        "test setup must give Black an immediate win"
    );

    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Ai(ai_config_tactics()),
        white: Occupant::Human,
        default_config: ai_config_tactics(),
        black_sims: None,
        white_sims: None,
        seed: Some(1),
        opening: board,
    });

    let events = core.execute(Command::Step).unwrap();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Moved {
                by: OccupantKind::Ai,
                ..
            }
        )),
        "Step with an AI seat must emit Moved{{by: Ai}}, got {events:?}"
    );
    assert_eq!(core.board().status(), Status::Won(Color::Black));

    let info = core.search_info().expect("search info must be stored");
    assert!(
        info.value > 0.0,
        "winning side-to-move must have a positive root value, got {}",
        info.value
    );
    let best = info.visits[0].0;
    assert!(
        wins.contains(best),
        "AI should play a winning move, got {best:?}"
    );
}

#[test]
fn tactics_ai_blocks_opponent_open_four() {
    // White has an open four on row 9; Black to move must block.
    // 300 simulations are used so that the two blocking moves accumulate
    // enough extra visits (via their much higher prior) to beat the
    // temperature-0 tie-breaker.
    let board = engine::reference::board_from_ascii(
        "
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . O O O O . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        X . X . X . X . . . . . . . .
        ",
    );
    assert_eq!(board.to_move(), Color::Black);
    let blocks = engine::forced_blocks(&board);
    assert!(
        !blocks.is_empty(),
        "test setup must give White an open four"
    );

    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Ai(AiConfig {
            simulations: 300,
            ..ai_config_tactics()
        }),
        white: Occupant::Human,
        default_config: ai_config_tactics(),
        black_sims: None,
        white_sims: None,
        seed: Some(1),
        opening: board,
    });

    let events = core.execute(Command::Step).unwrap();
    let moved = events
        .iter()
        .find_map(|e| match e {
            Event::Moved { mv, .. } => Some(*mv),
            _ => None,
        })
        .expect("AI should move");
    assert!(
        blocks.contains(moved),
        "AI should block the open four, got {moved:?}"
    );
}

#[test]
fn seeded_determinism_same_seed_same_events_and_search_info() {
    let config = AiConfig {
        temperature: 1.0,
        noise_epsilon: 0.25,
        ..ai_config_tactics()
    };
    let init = SessionInit {
        white: Occupant::Ai(config.clone()),
        default_config: config,
        seed: Some(42),
        ..SessionInit::new_hotseat()
    };
    let mut a = SessionCore::new(init.clone());
    let mut b = SessionCore::new(init);

    // Black plays, White AI steps.
    play(&mut a, 7, 7);
    play(&mut b, 7, 7);
    let ev_a = a.execute(Command::Step).unwrap();
    let ev_b = b.execute(Command::Step).unwrap();
    assert_eq!(ev_a, ev_b);

    let info_a = a.search_info().cloned();
    let info_b = b.search_info().cloned();
    assert_eq!(info_a, info_b);
}

#[test]
fn different_sims_per_seat_yield_different_total_visits() {
    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Human,
        white: Occupant::Human,
        default_config: AiConfig {
            simulations: 50,
            ..ai_config_tactics()
        },
        black_sims: Some(400),
        white_sims: Some(50),
        seed: Some(1),
        opening: Board::new(),
    });

    core.execute(Command::Release {
        seat: SeatSel::Both,
    })
    .unwrap();

    // Black AI steps first with 400 simulations.
    let events = core.execute(Command::Step).unwrap();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Moved {
                by: OccupantKind::Ai,
                ..
            }
        )),
        "Black AI should move, got {events:?}"
    );
    let black_info = core.search_info().expect("search info after black step");
    assert_eq!(black_info.config.simulations, 400);
    assert_eq!(
        black_info.visits.iter().map(|(_, n)| n).sum::<u32>(),
        400,
        "total root visits must match black simulations"
    );

    // White AI steps with 50 simulations.
    core.execute(Command::Step).unwrap();
    let white_info = core.search_info().expect("search info after white step");
    assert_eq!(white_info.config.simulations, 50);
    assert_eq!(
        white_info.visits.iter().map(|(_, n)| n).sum::<u32>(),
        50,
        "total root visits must match white simulations"
    );
}

#[test]
fn step_on_human_seat_turn_is_typed_error() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    let err = core.execute(Command::Step).unwrap_err();
    assert_eq!(err, CommandError::StepRequiresAi { seat: Color::Black });
}

#[test]
fn step_after_game_over_is_error() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    let script = [
        (7, 3),
        (0, 0),
        (7, 4),
        (0, 1),
        (7, 5),
        (0, 2),
        (7, 6),
        (0, 3),
        (7, 7),
    ];
    for &(r, c) in &script {
        core.execute(Command::Play(mv(r, c))).unwrap();
    }
    assert_eq!(core.board().status(), Status::Won(Color::Black));
    let err = core.execute(Command::Step).unwrap_err();
    assert_eq!(err, CommandError::GameOver);
}

#[test]
fn search_info_prior_and_visits_are_sorted_descending() {
    let mut core = SessionCore::new(SessionInit {
        white: Occupant::Ai(ai_config_tactics()),
        seed: Some(7),
        ..SessionInit::new_hotseat()
    });

    play(&mut core, 7, 7);
    core.execute(Command::Step).unwrap();

    let info = core.search_info().expect("search info stored");
    assert!(!info.visits.is_empty());
    assert!(!info.prior.is_empty());

    let visits_sorted = info.visits.windows(2).all(|w| w[0].1 >= w[1].1);
    assert!(visits_sorted, "visits must be sorted descending");

    let prior_sorted = info.prior.windows(2).all(|w| w[0].1 >= w[1].1);
    assert!(prior_sorted, "prior must be sorted descending");
}
```

## `gomoku/crates/cli/study_demo.txt`

```text
release both
step
step
step
step
step
step
step
step
step
step
step
step
```

## Files unchanged from chapter 03

The following files are identical to their chapter-03 versions at
`251246c` and are not quoted above:

* `gomoku/crates/cli/src/command.rs`
* `gomoku/crates/cli/src/history.rs`
* `gomoku/crates/cli/src/fmt.rs`
* `gomoku/crates/cli/Cargo.toml`
* `gomoku/crates/cli/tests/history.rs`
* `gomoku/crates/cli/tests/session.rs`

## Verification

The quoted files were extracted from commit `d9e8fec` of the reference
worktree. To verify them yourself:

```bash
#!/usr/bin/env bash
set -euo pipefail
REPO=/Users/wgiersche/workspace/Project-Ellie/rust-ml-ref-playable-cli
COMMIT=d9e8fec
SOLUTION=/Users/wgiersche/workspace/Project-Ellie/rust-ml/docs/tutorials/playable-cli-tutorial/04-the-study-loop/01-solution.md

python3 - "$REPO" "$COMMIT" "$SOLUTION" <<'PY'
import re, subprocess, sys
repo, commit, solution_path = sys.argv[1:4]
with open(solution_path, "r") as f:
    text = f.read()
# Each quoted file is a ## `path` header followed by a fenced code block.
# The closing fence is on its own line, so we match that explicitly.
pattern = re.compile(
    r'^## `([^`]+)`\n+```(?:rust|text)\n(.*?)\n```(?:\n|$)',
    re.MULTILINE | re.DOTALL,
)
ok = True
for m in pattern.finditer(text):
    path = m.group(1)
    if not (path.endswith(".rs") or path.endswith(".txt")):
        continue
    body = m.group(2) + "\n"  # restore the newline before the closing fence
    expected = subprocess.check_output(
        ["git", "-C", repo, "show", f"{commit}:{path}"],
        text=True,
    )
    if body != expected:
        print(f"MISMATCH: {path}", file=sys.stderr)
        ok = False
    else:
        print(f"OK: {path}")
sys.exit(0 if ok else 1)
PY
```
