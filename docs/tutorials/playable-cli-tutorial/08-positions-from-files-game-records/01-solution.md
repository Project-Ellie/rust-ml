> **Opt-in solution — read only after you have tried the chapter, or if you have been stuck for more than twenty minutes.**
>
> This file quotes the reference files for chapter 08, verified green at
> commit `3bf0544` of the reference worktree (parent `d6c2af3`, the
> chapter-07 state). New files and every file with substantive edits are
> quoted whole. Files already quoted in chapter 07's solution
> (`session.rs`, `main.rs`, `sessiondemo.rs`, `render.rs`, `lib.rs`,
> `tests/session.rs`) are quoted again at their chapter-08 state — each
> chapter's solution is self-contained. The quoted text was verified
> byte-for-byte against the reference worktree with
> `git show 3bf0544:<path>`.

# Chapter 08 — opt-in solution

## Files not quoted

The following files changed at `3bf0544` but only mechanically — a
`game: None` field added to `SessionInit` struct literals — so they are
not quoted above:

* `gomoku/crates/cli/tests/acceptance.rs` — one `game: None` line.
* `gomoku/crates/cli/tests/actor.rs` — three `game: None` lines.
* `gomoku/crates/cli/tests/study.rs` — one `game: None` line.

## `gomoku/crates/cli/src/game.rs`

```rust
//! Game record loading — Piskvork `.psq` files.
//!
//! A game record is not a game mode; it is a finished (or unfinished)
//! game loaded into the study table for review. The `.psq` format is the
//! Piskvork manager's record format (see `data/gomocup-games/` in the
//! main repo): a header line (`Piskvorky 15x15, …`) followed by one move
//! per line as `x,y,time-ms` — 1-based, `x` the column, `y` the row,
//! Black first, alternating. After the last move Piskvork appends a
//! metadata tail (two player-name lines, a result marker, a tournament
//! line).
//!
//! Format decisions, verified against all 11,400 records of the 2026
//! Gomocup Freestyle15 corpus:
//!
//! * The header must start with `Piskvorky ` and name a square board;
//!   only 15×15 is accepted. The remaining header fields (time control)
//!   may vary.
//! * Move lines are consumed greedily from line 2; the first line that
//!   is not `int,int,int` ends the game and the rest of the file is
//!   ignored metadata. Every real record carries such a tail, so
//!   rejecting trailing lines is not an option — but a line that starts
//!   like a move (`<digits>,<digits>`) yet is not a full move line is a
//!   truncated record and is rejected, not silently dropped. No real
//!   metadata line starts that way.
//! * CRLF line endings are tolerated (the whole corpus is CRLF).
//! * Every move must replay legally through `engine::Board`: on-board
//!   1-based coordinates, no duplicates, and no moves after the game has
//!   ended. The final position may be `Won`, `Draw`, or `Ongoing` (a
//!   record that simply ends early). Six corpus records (of 11,400) end
//!   with a duplicate of an earlier move — Piskvork's way of recording a
//!   loss by illegal move; these are rejected as `IllegalMove` since the
//!   recorded sequence does not replay.

use engine::{Board, Move, PlayError};

/// A parsed game record: the moves of the main line in play order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRecord {
    /// All moves of the game, Black first, alternating.
    pub moves: Vec<Move>,
}

impl GameRecord {
    /// Parse and validate a `.psq` record from its text.
    ///
    /// The move list is validated by replaying it through
    /// `engine::Board`: coordinates must be on the 15×15 board (1-based),
    /// no cell may be played twice, and no move may follow a terminal
    /// position.
    ///
    /// # Errors
    /// Returns a typed `GameError` describing the first violation.
    pub fn from_psq(text: &str) -> Result<Self, GameError> {
        let mut lines = text.lines().enumerate();

        let (_, header) = lines.next().ok_or(GameError::MissingHeader)?;
        parse_header(header.trim_end_matches('\r'))?;

        let mut board = Board::new();
        let mut moves = Vec::new();
        for (index, raw) in lines {
            let line_no = index + 1;
            let line = raw.trim_end_matches('\r');
            let Some((x, y)) = parse_move_line(line) else {
                if starts_like_a_move(line) {
                    return Err(GameError::MalformedMoveLine {
                        line: line_no,
                        text: line.to_string(),
                    });
                }
                // First non-move line: the Piskvork metadata tail begins.
                break;
            };
            let mv = if (1..=15).contains(&x) && (1..=15).contains(&y) {
                // 1-based x=column, y=row maps to 0-based Move::new(row, col).
                Move::new((y - 1) as u8, (x - 1) as u8).expect("1..=15 maps to 0..=14")
            } else {
                return Err(GameError::OffBoard {
                    line: line_no,
                    x,
                    y,
                });
            };
            board.play(mv).map_err(|source| GameError::IllegalMove {
                line: line_no,
                source,
            })?;
            moves.push(mv);
        }

        Ok(Self { moves })
    }
}

/// Everything that can go wrong when parsing a `.psq` game record.
#[derive(Debug, thiserror::Error)]
pub enum GameError {
    /// The file is empty; even the header is missing.
    #[error("missing header line")]
    MissingHeader,

    /// The first line is not a `Piskvorky <W>x<H>, …` header.
    #[error("invalid header: expected 'Piskvorky <W>x<H>, …', found '{0}'")]
    InvalidHeader(String),

    /// The header names a board other than 15×15.
    #[error("unsupported board size {width}x{height}: only 15x15 is supported")]
    UnsupportedBoardSize {
        /// Board width from the header.
        width: u32,
        /// Board height from the header.
        height: u32,
    },

    /// A line starts like a move (`<digits>,<digits>`) but is not a full
    /// `x,y,time` line — a truncated record.
    #[error("line {line}: malformed move line '{text}'")]
    MalformedMoveLine {
        /// 1-based line number in the file.
        line: usize,
        /// Raw text of the offending line.
        text: String,
    },

    /// A coordinate is outside the 15×15 board (1-based: 1..=15).
    #[error("line {line}: coordinate ({x}, {y}) is off the board (1-based, 15x15)")]
    OffBoard {
        /// 1-based line number in the file.
        line: usize,
        /// File x coordinate (column, 1-based).
        x: u32,
        /// File y coordinate (row, 1-based).
        y: u32,
    },

    /// A move does not replay legally: duplicate cell, or a move after
    /// the game had already ended.
    #[error("line {line}: illegal move in sequence: {source}")]
    IllegalMove {
        /// 1-based line number in the file.
        line: usize,
        /// The engine's replay error.
        source: PlayError,
    },
}

/// Validate the header line: `Piskvorky <W>x<H>, …` with W == H == 15.
fn parse_header(header: &str) -> Result<(), GameError> {
    let size = header
        .strip_prefix("Piskvorky ")
        .and_then(|rest| rest.split(',').next())
        .and_then(|token| token.split_once('x'));
    let Some((w, h)) = size else {
        return Err(GameError::InvalidHeader(header.to_string()));
    };
    let (Ok(width), Ok(height)) = (w.parse::<u32>(), h.parse::<u32>()) else {
        return Err(GameError::InvalidHeader(header.to_string()));
    };
    if width != 15 || height != 15 {
        return Err(GameError::UnsupportedBoardSize { width, height });
    }
    Ok(())
}

/// Parse a full move line `x,y,time`, returning the 1-based coordinates.
/// The time field is validated as a number but otherwise ignored.
fn parse_move_line(line: &str) -> Option<(u32, u32)> {
    let mut fields = line.split(',');
    let x = fields.next()?.parse::<u32>().ok()?;
    let y = fields.next()?.parse::<u32>().ok()?;
    let _time = fields.next()?.parse::<u64>().ok()?;
    if fields.next().is_some() {
        return None;
    }
    Some((x, y))
}

/// Does this line start like a move (`<digits>,<digits>`) without being
/// one? Such a line is a truncated record, not metadata.
fn starts_like_a_move(line: &str) -> bool {
    let mut fields = line.split(',');
    let Some(first) = fields.next() else {
        return false;
    };
    let Some(second) = fields.next() else {
        return false;
    };
    !first.is_empty()
        && first.bytes().all(|b| b.is_ascii_digit())
        && !second.is_empty()
        && second.bytes().all(|b| b.is_ascii_digit())
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
use std::path::PathBuf;

use engine::{Board, Color, Move, Status};
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::actor::{AiConfig, EvaluatorKind, Occupant, OccupantKind, SearchInfo};
use crate::command::{Command, CommandError, Event, SeatSel};
use crate::game::GameRecord;
use crate::history::NodeId as HistoryNodeId;
use crate::history::{ForwardOutcome, History, PlayOutcome};
use crate::puzzle::{Puzzle, PuzzleError, PuzzlePack};

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
    /// Puzzle study context when the session was started from a pack.
    pub puzzle: Option<PuzzleStudy>,
    /// Game study context when the session was started from a record.
    pub game: Option<GameStudy>,
}

/// Game study context carried by a session started from a `.psq` record.
///
/// The record's moves are replayed as the variation tree's main line
/// when the session is built; the record itself stays in memory for
/// display (source, total plies).
#[derive(Clone, Debug, PartialEq)]
pub struct GameStudy {
    /// Path the record was loaded from (for display and error messages).
    pub source: PathBuf,
    /// The parsed record.
    pub record: GameRecord,
}

/// Puzzle study context carried by a session started from a pack.
///
/// The whole pack stays in memory so `next` / `prev` can load adjacent
/// puzzles without further I/O.
#[derive(Clone, Debug, PartialEq)]
pub struct PuzzleStudy {
    /// Path the pack was loaded from (for display and error messages).
    pub pack_path: PathBuf,
    /// Zero-based index of the currently loaded puzzle.
    pub index: usize,
    /// The parsed pack.
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
            puzzle: None,
            game: None,
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
            puzzle: None,
            game: None,
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
            puzzle: None,
            game: None,
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
            puzzle: None,
            game: None,
        }
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
            default_config: AiConfig::default(),
            black_sims: None,
            white_sims: None,
            seed: None,
            opening,
            puzzle: Some(PuzzleStudy {
                pack_path,
                index,
                pack,
            }),
            game: None,
        })
    }

    /// Hotseat study session with `record` replayed as the main line.
    ///
    /// The game is replayed from an empty root onto the variation tree's
    /// main line with the cursor at the END position; the engine
    /// adjudicates the final status. Both seats start human. Seats and
    /// AI configuration can be adjusted afterwards with struct-update
    /// syntax, exactly as for the other constructors.
    ///
    /// Infallible: `GameRecord::from_psq` already validated that the
    /// moves replay legally.
    #[must_use]
    pub fn game(source: PathBuf, record: GameRecord) -> Self {
        Self {
            black: Occupant::Human,
            white: Occupant::Human,
            default_config: AiConfig::default(),
            black_sims: None,
            white_sims: None,
            seed: None,
            opening: Board::new(),
            puzzle: None,
            game: Some(GameStudy { source, record }),
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
    /// Puzzle study context; `Some` only for sessions started from a pack.
    puzzle: Option<PuzzleStudy>,
    /// Game study context; `Some` only for sessions started from a record.
    game: Option<GameStudy>,
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
            history: Self::build_history(&init),
            black: init.black.clone(),
            white: init.white.clone(),
            default_config: init.default_config.clone(),
            black_sims: init.black_sims,
            white_sims: init.white_sims,
            rng,
            puzzle: init.puzzle.clone(),
            game: init.game.clone(),
            init,
            search_info: HashMap::new(),
        }
    }

    /// Build the variation tree for a session: the opening root plus,
    /// for game sessions, the record replayed as the main line with the
    /// cursor at the end position.
    fn build_history(init: &SessionInit) -> History {
        let mut history = History::from_opening(&init.opening);
        if let Some(game) = &init.game {
            for &mv in &game.record.moves {
                // GameRecord::from_psq validated the replay; this cannot fail.
                history.play(mv).expect("game record replay must be legal");
            }
        }
        history
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
            Command::Hint => self.execute_hint(),
            Command::Reveal => self.execute_reveal(),
            Command::NextPuzzle => self.execute_puzzle_nav(1),
            Command::PrevPuzzle => self.execute_puzzle_nav(-1),
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

    /// The puzzle study context, if this session was started from a pack.
    #[must_use]
    pub fn puzzle(&self) -> Option<&PuzzleStudy> {
        self.puzzle.as_ref()
    }

    /// The game study context, if this session was started from a record.
    #[must_use]
    pub fn game(&self) -> Option<&GameStudy> {
        self.game.as_ref()
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

    /// The default AI configuration used when releasing a seat.
    #[must_use]
    pub fn default_config(&self) -> &AiConfig {
        &self.default_config
    }

    /// The occupant of the given seat.
    #[must_use]
    pub fn occupant(&self, color: Color) -> &Occupant {
        match color {
            Color::Black => &self.black,
            Color::White => &self.white,
        }
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
        self.history = Self::build_history(&self.init);
        self.black = self.init.black.clone();
        self.white = self.init.white.clone();
        self.default_config = self.init.default_config.clone();
        self.black_sims = self.init.black_sims;
        self.white_sims = self.init.white_sims;
        self.puzzle = self.init.puzzle.clone();
        self.game = self.init.game.clone();
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

## `gomoku/crates/cli/src/main.rs`

```rust
use clap::{Args, Parser, Subcommand, ValueEnum};
use engine::{Board, Color};
use std::path::{Path, PathBuf};

use cli::actor::{AiConfig, EvaluatorKind, Occupant};
use cli::game::GameRecord;
use cli::puzzle::PuzzlePack;
use cli::session::{GameStudy, PuzzleStudy, SessionInit};
use cli::tui::study;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "gomoku",
    about = "Terminal Gomoku client",
    subcommand_required = false,
    arg_required_else_help = false
)]
struct CliArgs {
    /// `play` flags used when no subcommand is given.
    #[command(flatten)]
    play: PlayArgs,

    #[command(subcommand)]
    cmd: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Play at the study table.
    Play(PlayArgs),
    /// AI-vs-AI self-play.
    Selfplay(SearchArgs),
}

#[derive(Args, Debug, Clone)]
struct PlayArgs {
    #[command(flatten)]
    search: SearchArgs,

    /// Skip the Swap2 opening and start from an empty board.
    #[arg(long)]
    no_swap2: bool,

    /// Study puzzle N of a puzzle pack (skips Swap2).
    #[arg(long, value_name = "PACK.json")]
    puzzle: Option<PathBuf>,

    /// Study a Piskvork game record (skips Swap2).
    #[arg(long, value_name = "FILE.psq", conflicts_with = "puzzle")]
    game: Option<PathBuf>,

    /// Zero-based index of the puzzle to load (default 0).
    #[arg(long, value_name = "N", requires = "puzzle")]
    index: Option<usize>,
}

#[derive(Args, Debug, Clone)]
struct SearchArgs {
    /// AI occupant kind (currently only `tactics`).
    #[arg(long, value_name = "AI")]
    ai: Option<AiKind>,

    /// Human seat color when `--ai` is used.
    #[arg(long, value_name = "COLOR")]
    color: Option<PlayerColor>,

    /// Number of MCTS simulations per move (default 400).
    #[arg(long, value_name = "N", default_value_t = 400)]
    sims: u32,

    /// PUCT exploration constant (default 1.5).
    #[arg(long, value_name = "F", default_value_t = 1.5)]
    c_puct: f32,

    /// Move-selection temperature (default 0.0).
    #[arg(long, value_name = "F")]
    temperature: Option<f32>,

    /// Dirichlet noise weight (default 0.0).
    #[arg(long, value_name = "F")]
    noise: Option<f32>,

    /// Override simulation count for Black.
    #[arg(long, value_name = "N")]
    black_sims: Option<u32>,

    /// Override simulation count for White.
    #[arg(long, value_name = "N")]
    white_sims: Option<u32>,

    /// Random seed for reproducible sessions.
    #[arg(long, value_name = "N")]
    seed: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
enum AiKind {
    Tactics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
enum PlayerColor {
    Black,
    White,
}

fn main() {
    let args = CliArgs::parse();

    let (mode, play_args, selfplay_args) = match args.cmd {
        None => (Mode::Play, Some(args.play), None),
        Some(Commands::Play(p)) => (Mode::Play, Some(p), None),
        Some(Commands::Selfplay(s)) => {
            // Reject `play` flags placed before the subcommand.
            if args.play.no_swap2 {
                eprintln!("error: --no-swap2 is only valid with the 'play' subcommand");
                std::process::exit(2);
            }
            if args.play.puzzle.is_some() {
                eprintln!("error: --puzzle is only valid with the 'play' subcommand");
                std::process::exit(2);
            }
            if args.play.game.is_some() {
                eprintln!("error: --game is only valid with the 'play' subcommand");
                std::process::exit(2);
            }
            (Mode::Selfplay, None, Some(s))
        }
    };

    let result = match mode {
        Mode::Play => {
            let play = play_args.expect("play mode has play args");
            run_play(&play)
        }
        Mode::Selfplay => {
            let search = selfplay_args.expect("selfplay mode has search args");
            run_selfplay(&search)
        }
    };

    if let Err(e) = result {
        eprintln!("terminal error: {e}");
        std::process::exit(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Play,
    Selfplay,
}

fn run_play(play: &PlayArgs) -> std::io::Result<()> {
    validate_search_args(&play.search, false)?;

    // A puzzle pack seeds the study table directly; Swap2 is skipped.
    if let Some(pack_path) = &play.puzzle {
        let init = build_puzzle_init(&play.search, pack_path, play.index.unwrap_or(0));
        return study::run(init, None);
    }

    // A game record replays onto the study table's main line; Swap2 is
    // skipped.
    if let Some(game_path) = &play.game {
        let init = build_game_init(&play.search, game_path);
        return study::run(init, None);
    }

    // The study table is the single front-end. Swap2 runs first unless
    // disabled, then the resulting board seeds the session.
    let make_init = |board: Board| build_play_init(&play.search, board);
    if play.no_swap2 {
        study::run(make_init(Board::new()), None)
    } else {
        study::run_with_swap2(make_init, None)
    }
}

/// Load a puzzle pack and build the `SessionInit` for studying it.
///
/// Exits with code 2 (usage/data error) when the pack is missing or
/// invalid; the puzzle corpora are git-ignored, so the message points at
/// the corpus's `data/puzzles/fetch.sh`.
fn build_puzzle_init(search: &SearchArgs, pack_path: &Path, index: usize) -> SessionInit {
    let text = std::fs::read_to_string(pack_path).unwrap_or_else(|e| {
        eprintln!(
            "error: cannot read puzzle pack '{}': {e}",
            pack_path.display()
        );
        eprintln!("       puzzle corpora are git-ignored — fetch them with data/puzzles/fetch.sh");
        std::process::exit(2);
    });
    let pack = PuzzlePack::from_json(&text).unwrap_or_else(|e| {
        eprintln!("error: invalid puzzle pack '{}': {e}", pack_path.display());
        std::process::exit(2);
    });
    let opening = pack
        .get(index)
        .unwrap_or_else(|e| {
            eprintln!("error: {e} in '{}'", pack_path.display());
            std::process::exit(2);
        })
        .board
        .clone();

    // Seats and AI flags compose exactly as for a Swap2 opening.
    let mut init = build_play_init(search, opening);
    init.puzzle = Some(PuzzleStudy {
        pack_path: pack_path.to_path_buf(),
        index,
        pack,
    });
    init
}

/// Load a game record and build the `SessionInit` for studying it.
///
/// Exits with code 2 (usage/data error) when the file is missing or
/// invalid; the game corpora are git-ignored, so the message points at
/// the corpus's `data/gomocup-games/fetch.sh`.
fn build_game_init(search: &SearchArgs, game_path: &Path) -> SessionInit {
    let text = std::fs::read_to_string(game_path).unwrap_or_else(|e| {
        eprintln!(
            "error: cannot read game record '{}': {e}",
            game_path.display()
        );
        eprintln!(
            "       game corpora are git-ignored — fetch them with data/gomocup-games/fetch.sh"
        );
        std::process::exit(2);
    });
    let record = GameRecord::from_psq(&text).unwrap_or_else(|e| {
        eprintln!("error: invalid game record '{}': {e}", game_path.display());
        std::process::exit(2);
    });

    // Seats and AI flags compose exactly as for a Swap2 opening.
    let mut init = build_play_init(search, Board::new());
    init.game = Some(GameStudy {
        source: game_path.to_path_buf(),
        record,
    });
    init
}

fn run_selfplay(search: &SearchArgs) -> std::io::Result<()> {
    validate_search_args(search, true)?;

    let (config, black_sims, white_sims) = ai_config_from(search);
    let init = SessionInit {
        black: Occupant::Ai(config.clone()),
        white: Occupant::Ai(config.clone()),
        default_config: config,
        black_sims,
        white_sims,
        seed: search.seed,
        opening: Board::new(),
        puzzle: None,
        game: None,
    };

    study::run(init, Some("Enter = step"))
}

fn validate_search_args(search: &SearchArgs, is_selfplay: bool) -> std::io::Result<()> {
    if !is_selfplay && search.color.is_some() && search.ai.is_none() {
        eprintln!("error: --color requires --ai");
        std::process::exit(2);
    }

    if is_selfplay && search.color.is_some() {
        eprintln!("error: --color is not valid for selfplay");
        std::process::exit(2);
    }

    Ok(())
}

fn ai_config_from(search: &SearchArgs) -> (AiConfig, Option<u32>, Option<u32>) {
    let temperature = search.temperature.unwrap_or(0.0);
    let noise = search.noise.unwrap_or(0.0);
    let explicit_selection = search.temperature.is_some() || search.noise.is_some();

    let config = AiConfig {
        evaluator: EvaluatorKind::Tactics,
        simulations: search.sims,
        c_puct: search.c_puct,
        temperature,
        noise_epsilon: noise,
        explicit_selection,
    };

    (config, search.black_sims, search.white_sims)
}

fn build_play_init(search: &SearchArgs, opening: Board) -> SessionInit {
    let (config, black_sims, white_sims) = ai_config_from(search);

    if let Some(AiKind::Tactics) = search.ai {
        let human_color = search
            .color
            .map(player_color_to_engine)
            .unwrap_or(Color::Black);
        let (black, white) = match human_color {
            Color::Black => (Occupant::Human, Occupant::Ai(config.clone())),
            Color::White => (Occupant::Ai(config.clone()), Occupant::Human),
        };
        SessionInit {
            black,
            white,
            default_config: config,
            black_sims,
            white_sims,
            seed: search.seed,
            opening,
            puzzle: None,
            game: None,
        }
    } else {
        SessionInit {
            black: Occupant::Human,
            white: Occupant::Human,
            default_config: config,
            black_sims,
            white_sims,
            seed: search.seed,
            opening,
            puzzle: None,
            game: None,
        }
    }
}

fn player_color_to_engine(color: PlayerColor) -> Color {
    match color {
        PlayerColor::Black => Color::Black,
        PlayerColor::White => Color::White,
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
//!
//! Study a puzzle pack (skips the empty-board start):
//!
//! ```text
//! cargo run -p cli --bin sessiondemo -- --puzzle pack.json --script puzzle_demo.txt
//! ```
//!
//! Study a recorded game (replayed as the main line, cursor at the end):
//!
//! ```text
//! cargo run -p cli --bin sessiondemo -- --game game.psq --script game_demo.txt
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
  hint             show the first move of the puzzle's line
  reveal           show the puzzle's full claimed line
  next / prev      load the next/previous puzzle (clamps at the ends)
  new              reset to opening
  h, help, ?       show this help
  q, quit, exit    quit
"#;

fn parse_seed(args: &[String]) -> Option<u64> {
    args.iter()
        .position(|a| a == "--seed")
        .and_then(|i| args.get(i + 1).and_then(|s| s.parse::<u64>().ok()))
}

fn parse_flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).map(String::as_str))
}

fn build_init(args: &[String]) -> SessionInit {
    let seed = parse_seed(args);
    if let Some(path) = parse_flag_value(args, "--game") {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("Cannot read game record '{}': {}", path, e);
            std::process::exit(1);
        });
        let record = cli::game::GameRecord::from_psq(&text).unwrap_or_else(|e| {
            eprintln!("Invalid game record '{}': {}", path, e);
            std::process::exit(1);
        });
        let init = SessionInit::game(std::path::PathBuf::from(path), record);
        return SessionInit { seed, ..init };
    }
    match parse_flag_value(args, "--puzzle") {
        Some(path) => {
            let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Cannot read puzzle pack '{}': {}", path, e);
                std::process::exit(1);
            });
            let pack = cli::puzzle::PuzzlePack::from_json(&text).unwrap_or_else(|e| {
                eprintln!("Invalid puzzle pack '{}': {}", path, e);
                std::process::exit(1);
            });
            let index = parse_flag_value(args, "--index")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);
            let init = SessionInit::puzzle(std::path::PathBuf::from(path), pack, index)
                .unwrap_or_else(|e| {
                    eprintln!("Cannot load puzzle: {}", e);
                    std::process::exit(1);
                });
            SessionInit { seed, ..init }
        }
        None => SessionInit {
            seed,
            ..SessionInit::new_hotseat()
        },
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let script_path = args
        .iter()
        .position(|a| a == "--script")
        .and_then(|i| args.get(i + 1));
    let init = build_init(&args);
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
    if trimmed.is_empty() || trimmed.starts_with('#') {
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
    if let Some(study) = core.puzzle() {
        let puzzle = &study.pack.items[study.index];
        println!(
            "puzzle {} of {}  attacker {:?}",
            study.index + 1,
            study.pack.items.len(),
            puzzle.attacker
        );
    }
    if let Some(study) = core.game() {
        let stem = study
            .source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| study.source.display().to_string());
        println!("game {}  {} plies", stem, study.record.moves.len());
    }
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

## `gomoku/crates/cli/src/tui/render.rs`

```rust
//! Board rendering — pure functions, no I/O.

use crate::actor::{Occupant, SearchInfo};
use crate::command::Event;
use crate::session::SessionCore;
use crate::tui::opening::OpeningState;
use crate::tui::overlay::{
    PolicyOverlay, overlay_cell, render_overlay_cell_plain, render_overlay_cell_styled, value_strip,
};
use crossterm::style::Stylize;
use engine::{Board, Color, Move, Proof, Status};

fn glyph(stone: Option<Color>) -> char {
    match stone {
        None => '.',
        Some(Color::Black) => 'x',
        Some(Color::White) => 'o',
    }
}

/// Shared layout: row label, bar, 15 cells, bar, row label — inside a
/// ruler/border frame. `paint(glyph, is_last_move, move)` produces one
/// cell's visible characters; `cell_width` is used for the ruler and frame.
/// The board is passed as its move list plus a stone lookup, so render-only
/// views (such as the Swap2 opening) can share the layout.
fn render_with(
    moves: &[Move],
    stone_at: impl Fn(Move) -> Option<Color>,
    cell_width: usize,
    paint: impl Fn(char, bool, Move) -> String,
) -> Vec<String> {
    let ruler: String = (0..15u8)
        .map(|c| format!("{:^cell_width$}", (b'a' + c) as char))
        .collect();
    let ruler = format!("    {ruler}    ");
    let border = format!("   +{}+   ", "-".repeat(cell_width * 15));
    let last = moves.last().copied();

    let mut lines = Vec::with_capacity(19);
    lines.push(ruler.clone());
    lines.push(border.clone());
    for row in 0..15u8 {
        let mut line = format!("{:>2} |", row + 1);
        for col in 0..15u8 {
            let mv = Move::new(row, col).expect("0..15 is always on board");
            line.push_str(&paint(glyph(stone_at(mv)), Some(mv) == last, mv));
        }
        line.push_str(&format!("| {:<2}", row + 1));
        lines.push(line);
    }
    lines.push(border);
    lines.push(ruler);
    lines
}

/// Plain ASCII: tests and piping into a file.
pub fn render_lines(board: &Board) -> Vec<String> {
    render_with(
        board.moves(),
        |mv| board.stone_at(mv),
        3,
        |g, is_last, _mv| {
            if is_last {
                format!("[{g}]")
            } else {
                format!(" {g} ")
            }
        },
    )
}

/// ANSI-coloured: the alternate-screen front-end.
pub fn render_styled(board: &Board) -> Vec<String> {
    render_with(
        board.moves(),
        |mv| board.stone_at(mv),
        3,
        |g, is_last, _mv| {
            let styled = match g {
                '.' => ".".dark_grey(),
                'x' => "x".cyan().bold(),
                'o' => "o".yellow(),
                _ => unreachable!("glyph is one of . x o"),
            };
            if is_last {
                format!("[{styled}]")
            } else {
                format!(" {styled} ")
            }
        },
    )
}

/// One line of context for below the board.
pub fn status_line(board: &Board) -> String {
    match board.status() {
        Status::Ongoing => format!(
            "engine: fast · move {} · {:?} to move",
            board.moves().len(),
            board.to_move()
        ),
        Status::Won(color) => format!("{color:?} wins! — type 'new' to start over"),
        Status::Draw => "Draw! — type 'new' to start over".to_string(),
    }
}

// -----------------------------------------------------------------------------
// Swap2 opening rendering

/// Plain rendering of the Swap2 opening position.
pub fn render_opening_lines(state: &OpeningState) -> Vec<String> {
    render_opening(state, |glyph, is_last, _mv| {
        if is_last {
            format!("[{glyph}]")
        } else {
            format!(" {glyph} ")
        }
    })
}

/// Styled rendering of the Swap2 opening position.
pub fn render_opening_styled(state: &OpeningState) -> Vec<String> {
    render_opening(state, |glyph, is_last, _mv| {
        let styled = match glyph {
            '.' => ".".dark_grey(),
            'x' => "x".cyan().bold(),
            'o' => "o".yellow(),
            _ => unreachable!("glyph is one of . x o"),
        };
        if is_last {
            format!("[{styled}]")
        } else {
            format!(" {styled} ")
        }
    })
}

fn render_opening(state: &OpeningState, paint: impl Fn(char, bool, Move) -> String) -> Vec<String> {
    let view = TemporaryOpeningBoard::from(state);
    render_with(&view.moves, |mv| view.stone_at(mv), 3, paint)
}

/// A temporary board view of an opening position, used only for rendering.
struct TemporaryOpeningBoard {
    moves: Vec<Move>,
    colors: Vec<Color>,
}

impl TemporaryOpeningBoard {
    fn from(state: &OpeningState) -> Self {
        let mut moves = Vec::with_capacity(state.placements().len());
        let mut colors = Vec::with_capacity(state.placements().len());
        for &(mv, color) in state.placements() {
            moves.push(mv);
            colors.push(color);
        }
        Self { moves, colors }
    }

    fn stone_at(&self, mv: Move) -> Option<Color> {
        self.moves
            .iter()
            .zip(&self.colors)
            .find(|(m, _)| **m == mv)
            .map(|(_, &c)| c)
    }
}

// -----------------------------------------------------------------------------
// TSS proof overlay

/// Plain rendering with a verified TSS line overlaid as numbered cells.
pub fn render_lines_with_overlay(board: &Board, proof: &Proof) -> Vec<String> {
    let overlay = Overlay::new(proof);
    render_with(
        board.moves(),
        |mv| board.stone_at(mv),
        3,
        |glyph, is_last, mv| {
            if let Some((n, side)) = overlay.get(mv) {
                return overlay_cell_plain(n, side);
            }
            plain_cell(glyph, is_last)
        },
    )
}

/// Styled rendering with a verified TSS line overlaid as numbered cells.
pub fn render_styled_with_overlay(board: &Board, proof: &Proof) -> Vec<String> {
    let overlay = Overlay::new(proof);
    render_with(
        board.moves(),
        |mv| board.stone_at(mv),
        3,
        |glyph, is_last, mv| {
            if let Some((n, side)) = overlay.get(mv) {
                return overlay_cell_styled(n, side, proof.winner);
            }
            let styled = match glyph {
                '.' => ".".dark_grey(),
                'x' => "x".cyan().bold(),
                'o' => "o".yellow(),
                _ => unreachable!("glyph is one of . x o"),
            };
            styled_cell(&styled, is_last)
        },
    )
}

fn plain_cell(glyph: char, is_last: bool) -> String {
    if is_last {
        format!("[{glyph}]")
    } else {
        format!(" {glyph} ")
    }
}

fn styled_cell(styled: &dyn std::fmt::Display, is_last: bool) -> String {
    if is_last {
        format!("[{styled}]")
    } else {
        format!(" {styled} ")
    }
}

struct Overlay {
    cells: [Option<(usize, Side)>; 225],
}

impl Overlay {
    fn new(proof: &Proof) -> Self {
        let mut cells = [None; 225];
        for (i, &mv) in proof.line.iter().enumerate() {
            let side = if i % 2 == 0 {
                Side::Attacker
            } else {
                Side::Defender
            };
            cells[mv.index()] = Some((i + 1, side));
        }
        Self { cells }
    }

    fn get(&self, mv: Move) -> Option<(usize, Side)> {
        self.cells[mv.index()]
    }
}

#[derive(Debug, Clone, Copy)]
enum Side {
    Attacker,
    Defender,
}

// Overlay numbers are single-digit because the CLI's TSS budget caps
// max_depth at 9 — that is what lets the overlay keep the normal width-3
// grid. Raise the depth cap and the width must follow.
fn overlay_cell_plain(n: usize, side: Side) -> String {
    debug_assert!(n < 10, "single-digit overlay numbers (max_depth = 9)");
    match side {
        Side::Attacker => format!(" {n} "),
        Side::Defender => format!("[{n}]"),
    }
}

fn overlay_cell_styled(n: usize, side: Side, winner: Color) -> String {
    let s = n.to_string();
    let color = match side {
        Side::Attacker => winner,
        Side::Defender => winner.other(),
    };
    let styled = match color {
        Color::Black => s.cyan().bold(),
        Color::White => s.yellow(),
    };
    debug_assert!(n < 10, "single-digit overlay numbers (max_depth = 9)");
    match side {
        Side::Attacker => format!(" {styled} "),
        Side::Defender => format!("[{styled}]"),
    }
}

/// Verdict text for a verified proof overlay.
pub fn overlay_verdict(proof: &Proof) -> String {
    format!(
        "certified forced win for {:?} in {} ply",
        proof.winner,
        proof.line.len()
    )
}

// ------------------------------------------------------------------
// Study-table rendering

/// Render the full study view: board with star points, last-move marker,
/// and a right-hand side panel.
pub fn render_study(core: &SessionCore, recent_events: &[Event], styled: bool) -> Vec<String> {
    render_study_with_overlays(core, recent_events, styled, PolicyOverlay::Off, None, false)
}

/// Render the study view with optional policy/TSS overlays and value strip.
pub fn render_study_with_overlays(
    core: &SessionCore,
    recent_events: &[Event],
    styled: bool,
    policy_overlay: PolicyOverlay,
    tss_proof: Option<&Proof>,
    show_value_strip: bool,
) -> Vec<String> {
    let board_lines = render_study_board_with_overlays(core, styled, policy_overlay, tss_proof);
    let panel_lines =
        render_side_panel_with_value_strip(core, recent_events, styled, show_value_strip);
    let max_rows = board_lines.len().max(panel_lines.len());
    let mut out = Vec::with_capacity(max_rows);
    for i in 0..max_rows {
        let left = board_lines.get(i).map(String::as_str).unwrap_or("");
        let right = panel_lines.get(i).map(String::as_str).unwrap_or("");
        out.push(format!("{left}  {right}"));
    }
    out
}

fn render_study_board_with_overlays(
    core: &SessionCore,
    styled: bool,
    policy_overlay: PolicyOverlay,
    tss_proof: Option<&Proof>,
) -> Vec<String> {
    let board = core.board();
    let last = core.snapshot().last_move;

    // TSS and policy overlays are mutually exclusive in the shell; if both
    // are somehow active, TSS wins (it is a deterministic proof overlay).
    if let Some(proof) = tss_proof {
        return study_board_lines_with_tss(&board, last, proof, styled);
    }

    if policy_overlay != PolicyOverlay::Off
        && let Some(info) = core.search_info()
    {
        return study_board_lines_with_policy(&board, last, info, policy_overlay, styled);
    }

    study_board_lines(&board, last, styled)
}

const HOSHI: [u8; 3] = [3, 7, 11];

fn study_board_lines(board: &Board, last: Option<Move>, styled: bool) -> Vec<String> {
    let ruler_plain = build_ruler();
    let border_plain = format!("   +{}+   ", "-".repeat(45));
    let mut lines = Vec::with_capacity(19);
    lines.push(style_gutter(&ruler_plain, styled));
    lines.push(style_gutter(&border_plain, styled));
    for row in 0..15u8 {
        let label = style_gutter(&format!("{:>2}", row + 1), styled);
        let rlabel = style_gutter(&format!("{:<2}", row + 1), styled);
        let mut line = format!("{label} |");
        for col in 0..15u8 {
            let mv = Move::new(row, col).expect("0..15 is always on board");
            line.push_str(&study_cell(board, mv, last, styled));
        }
        line.push_str(&format!("| {rlabel}"));
        lines.push(line);
    }
    lines.push(style_gutter(&border_plain, styled));
    lines.push(style_gutter(&ruler_plain, styled));
    lines
}

fn build_ruler() -> String {
    let mut s = String::with_capacity(53);
    s.push_str("    ");
    for c in 0..15u8 {
        s.push_str(&format!("{:^3}", (b'a' + c) as char));
    }
    s.push_str("    ");
    s
}

fn style_gutter(text: &str, styled: bool) -> String {
    if styled {
        text.dark_grey().to_string()
    } else {
        text.to_string()
    }
}

fn study_cell(board: &Board, mv: Move, last: Option<Move>, styled: bool) -> String {
    let is_last = Some(mv) == last;
    match board.stone_at(mv) {
        Some(color) => {
            let s = if styled {
                match color {
                    Color::Black => "x".cyan().bold().to_string(),
                    Color::White => "o".yellow().to_string(),
                }
            } else {
                match color {
                    Color::Black => "x".to_string(),
                    Color::White => "o".to_string(),
                }
            };
            if is_last {
                format!("[{s}]")
            } else {
                format!(" {s} ")
            }
        }
        None => {
            let is_hoshi = HOSHI.contains(&mv.row()) && HOSHI.contains(&mv.col());
            let g = if is_hoshi { "+" } else { "." };
            let s = if styled {
                g.dark_grey().to_string()
            } else {
                g.to_string()
            };
            format!(" {s} ")
        }
    }
}

fn study_board_lines_with_policy(
    board: &Board,
    last: Option<Move>,
    info: &SearchInfo,
    mode: PolicyOverlay,
    styled: bool,
) -> Vec<String> {
    let ruler_plain = build_ruler();
    let border_plain = format!("   +{}+   ", "-".repeat(45));
    let mut lines = Vec::with_capacity(19);
    lines.push(style_gutter(&ruler_plain, styled));
    lines.push(style_gutter(&border_plain, styled));
    for row in 0..15u8 {
        let label = style_gutter(&format!("{:>2}", row + 1), styled);
        let rlabel = style_gutter(&format!("{:<2}", row + 1), styled);
        let mut line = format!("{label} |");
        for col in 0..15u8 {
            let mv = Move::new(row, col).expect("0..15 is always on board");
            line.push_str(&policy_cell(board, mv, last, info, mode, styled));
        }
        line.push_str(&format!("| {rlabel}"));
        lines.push(line);
    }
    lines.push(style_gutter(&border_plain, styled));
    lines.push(style_gutter(&ruler_plain, styled));
    lines
}

fn policy_cell(
    board: &Board,
    mv: Move,
    last: Option<Move>,
    info: &SearchInfo,
    mode: PolicyOverlay,
    styled: bool,
) -> String {
    // Stones are never over-painted by the policy heatmap.
    if let Some(color) = board.stone_at(mv) {
        return stone_cell(color, Some(mv) == last, styled);
    }

    let glyph = if HOSHI.contains(&mv.row()) && HOSHI.contains(&mv.col()) {
        '+'
    } else {
        '.'
    };

    match overlay_cell(info, mv, mode) {
        Some(cell) if styled => render_overlay_cell_styled(glyph, &cell),
        Some(cell) => render_overlay_cell_plain(glyph, &cell),
        None => format!(" {} ", glyph),
    }
}

fn stone_cell(color: Color, is_last: bool, styled: bool) -> String {
    let s = if styled {
        match color {
            Color::Black => "x".cyan().bold().to_string(),
            Color::White => "o".yellow().to_string(),
        }
    } else {
        match color {
            Color::Black => "x".to_string(),
            Color::White => "o".to_string(),
        }
    };
    if is_last {
        format!("[{s}]")
    } else {
        format!(" {s} ")
    }
}

fn study_board_lines_with_tss(
    board: &Board,
    last: Option<Move>,
    proof: &Proof,
    styled: bool,
) -> Vec<String> {
    let overlay = Overlay::new(proof);
    let ruler_plain = build_ruler();
    let border_plain = format!("   +{}+   ", "-".repeat(45));
    let mut lines = Vec::with_capacity(19);
    lines.push(style_gutter(&ruler_plain, styled));
    lines.push(style_gutter(&border_plain, styled));
    for row in 0..15u8 {
        let label = style_gutter(&format!("{:>2}", row + 1), styled);
        let rlabel = style_gutter(&format!("{:<2}", row + 1), styled);
        let mut line = format!("{label} |");
        for col in 0..15u8 {
            let mv = Move::new(row, col).expect("0..15 is always on board");
            line.push_str(&tss_study_cell(board, mv, last, &overlay, proof, styled));
        }
        line.push_str(&format!("| {rlabel}"));
        lines.push(line);
    }
    lines.push(style_gutter(&border_plain, styled));
    lines.push(style_gutter(&ruler_plain, styled));
    lines
}

fn tss_study_cell(
    board: &Board,
    mv: Move,
    last: Option<Move>,
    overlay: &Overlay,
    proof: &Proof,
    styled: bool,
) -> String {
    if let Some(color) = board.stone_at(mv) {
        return stone_cell(color, Some(mv) == last, styled);
    }
    if let Some((n, side)) = overlay.get(mv) {
        if styled {
            return overlay_cell_styled(n, side, proof.winner);
        }
        return overlay_cell_plain(n, side);
    }
    let is_hoshi = HOSHI.contains(&mv.row()) && HOSHI.contains(&mv.col());
    let g = if is_hoshi { "+" } else { "." };
    let s = if styled {
        g.dark_grey().to_string()
    } else {
        g.to_string()
    };
    format!(" {s} ")
}

fn render_side_panel_with_value_strip(
    core: &SessionCore,
    recent_events: &[Event],
    styled: bool,
    show_value_strip: bool,
) -> Vec<String> {
    let snap = core.snapshot();
    let mut lines = Vec::new();
    lines.push(panel_line("-- Study table --", styled));
    lines.push(format!("Black: {}", seat_label(core.black())));
    lines.push(format!("White: {}", seat_label(core.white())));
    lines.push(String::new());
    lines.push(format!("Ply: {}", snap.ply));
    lines.push(format!("Path: {}", path_tail_string(core)));
    if let Some(study) = core.puzzle() {
        let puzzle = &study.pack.items[study.index];
        lines.push(format!(
            "Puzzle: {} of {} · {:?} to move",
            study.index + 1,
            study.pack.items.len(),
            puzzle.attacker
        ));
    }
    if let Some(study) = core.game() {
        let stem = study
            .source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| study.source.display().to_string());
        lines.push(format!(
            "Game: {} · {} plies",
            stem,
            study.record.moves.len()
        ));
    }
    if snap.branch_options.len() > 1 {
        let opts: Vec<String> = snap
            .branch_options
            .iter()
            .map(|(i, m)| format!("[{}] {}", i + 1, crate::fmt::format_move(*m)))
            .collect();
        lines.push(format!(
            "{} variations: {}",
            snap.branch_options.len(),
            opts.join(" ")
        ));
    }

    if show_value_strip {
        lines.push(String::new());
        let side_at_search = core.board().to_move().other();
        let current_config = match core.occupant(side_at_search) {
            Occupant::Ai(cfg) => Some(cfg),
            Occupant::Human => None,
        };
        let strip = value_strip(core.search_info(), current_config, Some(side_at_search));
        if strip.is_empty() {
            lines.push("Value strip:".to_string());
            lines.push("  (no search here yet)".to_string());
        } else {
            lines.push("Value strip:".to_string());
            lines.extend(strip);
        }
    }

    lines.push(String::new());
    lines.push("Last events:".to_string());
    if recent_events.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for ev in recent_events.iter().rev().take(5).rev() {
            lines.push(format!("  {ev}"));
        }
    }
    lines.push(String::new());
    lines.push(panel_line("h help · q quit", styled));
    lines.push(panel_line("p policy overlay (ch.06)", styled));
    lines.push(panel_line("v value strip (ch.06)", styled));
    lines
}

fn panel_line(text: &str, styled: bool) -> String {
    if styled {
        text.dark_grey().to_string()
    } else {
        text.to_string()
    }
}

fn seat_label(occupant: &Occupant) -> String {
    match occupant {
        Occupant::Human => "Human".to_string(),
        Occupant::Ai(cfg) => format!("AI({})", cfg.simulations),
    }
}

fn path_tail_string(core: &SessionCore) -> String {
    let board = core.board();
    let moves = board.moves();
    if moves.is_empty() {
        return "(root)".to_string();
    }
    let tail: Vec<String> = moves
        .iter()
        .rev()
        .take(8)
        .rev()
        .map(|m| crate::fmt::format_move(*m))
        .collect();
    if moves.len() > tail.len() {
        format!("... {}", tail.join(" "))
    } else {
        tail.join(" ")
    }
}

/// Render a policy-overlay board for snapshot tests.
#[cfg(test)]
pub fn render_policy_board_for_test(
    board: &Board,
    info: &SearchInfo,
    mode: PolicyOverlay,
    styled: bool,
) -> Vec<String> {
    study_board_lines_with_policy(board, board.moves().last().copied(), info, mode, styled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::AiConfig;
    use crate::tui::opening::OpeningState;
    use crate::tui::overlay::PolicyOverlay;

    #[test]
    fn empty_board_is_ruled_and_framed() {
        let board = Board::new();
        let lines = render_lines(&board);

        assert_eq!(lines.len(), 19);
        assert!(lines.iter().all(|l| l.chars().count() == 53));

        let ruler = "     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o     ";
        let border = "   +---------------------------------------------+   ";
        assert_eq!(lines[0], ruler);
        assert_eq!(lines[1], border);
        assert_eq!(lines[17], border);
        assert_eq!(lines[18], ruler);

        for (row, line) in lines[2..17].iter().enumerate() {
            assert_eq!(
                line,
                &format!("{:>2} |{}| {:<2}", row + 1, " . ".repeat(15), row + 1)
            );
        }
    }

    #[test]
    fn last_move_wears_brackets() {
        let mut board = Board::new();
        board.play(Move::new(7, 7).unwrap()).unwrap(); // x — last move
        board.play(Move::new(7, 8).unwrap()).unwrap(); // o — now last

        let lines = render_lines(&board);
        let row7 = &lines[2 + 7]; // line 0 ruler, line 1 border
        // 4-char gutter, then 3 bytes per column.
        assert_eq!(&row7[25..28], " x ");
        assert_eq!(&row7[28..31], "[o]");
    }

    #[test]
    fn brackets_follow_an_undo() {
        let mut board = Board::new();
        board.play(Move::new(7, 7).unwrap()).unwrap();
        board.play(Move::new(7, 8).unwrap()).unwrap();
        board.undo();

        let lines = render_lines(&board);
        let row7 = &lines[2 + 7];
        assert_eq!(&row7[25..28], "[x]"); // the bracket walks back
        assert_eq!(&row7[28..31], " . ");
    }

    #[test]
    fn status_line_reports_to_move_and_result() {
        let mut board = Board::new();
        assert_eq!(status_line(&board), "engine: fast · move 0 · Black to move");

        // Black wins with five on row 0; White parks on row 1.
        for (br, bc, wr, wc) in [(0, 0, 1, 0), (0, 1, 1, 1), (0, 2, 1, 2), (0, 3, 1, 3)] {
            board.play(Move::new(br, bc).unwrap()).unwrap();
            board.play(Move::new(wr, wc).unwrap()).unwrap();
        }
        board.play(Move::new(0, 4).unwrap()).unwrap();

        assert_eq!(
            status_line(&board),
            "Black wins! — type 'new' to start over"
        );
    }

    #[test]
    fn opening_render_shows_placed_colors() {
        let mut state = OpeningState::new();
        state.place(Move::new(7, 7).unwrap()).unwrap();
        state.place(Move::new(6, 6).unwrap()).unwrap();
        state.place(Move::new(5, 5).unwrap()).unwrap();

        let lines = render_opening_lines(&state);
        // The last placement is White at (5,5), so that cell wears brackets.
        let row7 = &lines[2 + 7];
        assert_eq!(&row7[25..28], " x ");
        let row6 = &lines[2 + 6];
        assert_eq!(&row6[22..25], " x ");
        let row5 = &lines[2 + 5];
        assert_eq!(&row5[19..22], "[o]");
    }

    #[test]
    fn tss_overlay_numbers_attack_and_defence() {
        let mut board = Board::new();
        // Build a VCF win-in-3 from the engine TSS tests.
        let script = [
            (7, 4),
            (7, 3),
            (7, 5),
            (0, 0),
            (7, 6),
            (0, 2),
            (6, 6),
            (0, 4),
            (8, 8),
            (0, 6),
        ];
        for &(r, c) in &script {
            board.play(Move::new(r, c).unwrap()).unwrap();
        }

        let proof = engine::prove_forced_win(
            &board,
            board.to_move(),
            engine::SearchBudget {
                max_nodes: 200_000,
                max_depth: 16,
            },
        )
        .expect("test position has a known forced win");
        assert!(engine::verify_line(&board, &proof));

        let lines = render_lines_with_overlay(&board, &proof);
        assert_eq!(lines.len(), 19);
        // Width-3 cells like normal play — the overlay causes no layout jump.
        assert!(lines.iter().all(|l| l.chars().count() == 4 + 45 + 4));

        // The first attacking move should be visible as a single-digit number.
        let mv1 = proof.line[0];
        let row = &lines[2 + mv1.row() as usize];
        let start = 4 + mv1.col() as usize * 3;
        let cell = &row[start..start + 3];
        assert!(
            cell.trim().contains('1'),
            "attacking cell should show 1, got {cell:?}"
        );
    }

    #[test]
    fn study_board_shows_star_points_on_empty_cells() {
        let core = SessionCore::new(crate::session::SessionInit::new_hotseat());
        let lines = render_study(&core, &[], false);
        // Row 7 (a hoshi row) should contain '+' star markers.
        let row7 = &lines[2 + 7];
        assert!(
            row7.matches(" + ").count() >= 3,
            "expected hoshi markers on row 7, got: {row7}"
        );
    }

    #[test]
    fn study_board_marks_last_move() {
        let mut core = SessionCore::new(crate::session::SessionInit::new_hotseat());
        core.execute(crate::command::Command::Play(Move::new(7, 7).unwrap()))
            .unwrap();
        let lines = render_study(&core, &[], false);
        let output = lines.join("\n");
        assert!(
            output.contains("[x]"),
            "last Black move should be bracketed"
        );
    }

    #[test]
    fn study_side_panel_shows_seats_and_ply() {
        let mut core = SessionCore::new(crate::session::SessionInit::new_hotseat());
        let lines = render_study(&core, &[], false);
        let output = lines.join("\n");
        assert!(output.contains("Black: Human"), "panel shows Black seat");
        assert!(output.contains("White: Human"), "panel shows White seat");
        assert!(output.contains("Ply: 0"), "panel shows initial ply");

        core.execute(crate::command::Command::Play(Move::new(7, 7).unwrap()))
            .unwrap();
        let lines = render_study(&core, &[], false);
        let output = lines.join("\n");
        assert!(output.contains("Ply: 1"), "panel updates ply");
        assert!(output.contains("h8"), "panel shows path tail");
    }

    #[test]
    fn study_side_panel_shows_branch_options() {
        let mut core = SessionCore::new(crate::session::SessionInit::new_hotseat());
        // Main line: (7,7) (6,7). Back one and branch to (7,8).
        core.execute(crate::command::Command::Play(Move::new(7, 7).unwrap()))
            .unwrap();
        core.execute(crate::command::Command::Play(Move::new(6, 7).unwrap()))
            .unwrap();
        core.execute(crate::command::Command::Back { n: 1 })
            .unwrap();
        core.execute(crate::command::Command::Play(Move::new(7, 8).unwrap()))
            .unwrap();
        // Move back to the branch point (the parent of the two children).
        core.execute(crate::command::Command::Back { n: 1 })
            .unwrap();

        let lines = render_study(&core, &[], false);
        let output = lines.join("\n");
        assert!(
            output.contains("variations:"),
            "panel should list branch options, got: {output}"
        );
        assert!(output.contains("[1] h7"), "panel shows first branch");
        assert!(output.contains("[2] i8"), "panel shows second branch");
    }

    #[test]
    fn policy_overlay_snapshot_with_fake_search_info() {
        // Construct SearchInfo directly — no real MCTS search needed.
        let mut board = Board::new();
        board.play(Move::new(7, 7).unwrap()).unwrap();
        board.play(Move::new(6, 7).unwrap()).unwrap();

        let info = crate::actor::SearchInfo {
            visits: vec![
                (Move::new(7, 8).unwrap(), 320),
                (Move::new(8, 7).unwrap(), 80),
            ],
            prior: vec![
                (Move::new(7, 8).unwrap(), 0.6),
                (Move::new(8, 7).unwrap(), 0.2),
            ],
            value: 0.35,
            config: AiConfig::default(),
        };

        let lines = render_policy_board_for_test(&board, &info, PolicyOverlay::Visits, false);
        let row7 = &lines[2 + 7];

        // Hot cell (7,8) shows raw visit count 320; an unvisited cell stays dim.
        let col8_start = 4 + 8 * 3;
        assert_eq!(&row7[col8_start..col8_start + 3], "320");
        let row8 = &lines[2 + 8];
        assert_eq!(&row8[col8_start..col8_start + 3], " . ");
    }
}
```

## `gomoku/crates/cli/src/lib.rs`

```rust
//! Terminal Gomoku client library.
//!
//! The `cli` crate is split into a headless library (history, command,
//! session, actor, puzzle, game) and a thin binary front-end. The `tui` module holds
//! the existing terminal UI pieces moved here unchanged.

#![deny(missing_docs)]

pub mod actor;
pub mod command;
pub mod fmt;
pub mod game;
pub mod history;
pub mod puzzle;
pub mod session;
pub mod tui;
```

## `gomoku/crates/cli/tests/game.rs`

```rust
//! Parser tests for Piskvork `.psq` game records (spec amendment
//! "positions from files", item 2).
//!
//! The valid-record tests load the committed mini fixture; malformed
//! inputs are inline strings. Tests never touch the git-ignored corpora
//! under `data/`.

use cli::game::{GameError, GameRecord};
use engine::{Board, Color, Move, PlayError, Status};

const MINI_GAME: &str = include_str!("fixtures/mini-game.psq");

fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}

fn replay(record: &GameRecord) -> Board {
    let mut board = Board::new();
    for &m in &record.moves {
        board.play(m).expect("parsed record replays legally");
    }
    board
}

#[test]
fn parses_fixture_into_the_main_line() {
    let record = GameRecord::from_psq(MINI_GAME).expect("mini fixture parses");
    assert_eq!(record.moves.len(), 13);
    // File coordinates are 1-based x=column, y=row: "7,8,0" is row 7,
    // column 6 zero-based.
    assert_eq!(record.moves[0], mv(7, 6));
    assert_eq!(record.moves[12], mv(7, 10));
}

#[test]
fn file_coordinates_are_1_based_x_column_y_row() {
    let record = GameRecord::from_psq(MINI_GAME).unwrap();
    let board = replay(&record);
    // First move "7,8": column 7 ("g"), row 8 — algebraic g8.
    assert_eq!(board.stone_at(mv(7, 6)), Some(Color::Black));
    assert_eq!(board.stone_at(mv(6, 7)), None);
    assert_eq!(cli::fmt::format_move(record.moves[0]), "g8");
}

#[test]
fn fixture_replays_to_a_black_win() {
    // Freestyle five on row 8: columns 7–11.
    let record = GameRecord::from_psq(MINI_GAME).unwrap();
    assert_eq!(replay(&record).status(), Status::Won(Color::Black));
}

#[test]
fn trailing_metadata_is_ignored() {
    // The fixture carries the Piskvork tail (player names, result marker,
    // tournament line) after the last move; only the 13 move lines count.
    let record = GameRecord::from_psq(MINI_GAME).unwrap();
    assert_eq!(record.moves.len(), 13);

    // Inline variant: metadata straight after the last move, no trailing
    // newline.
    let text =
        "Piskvorky 15x15, 11:11, 0\n7,8,0\n8,8,0\nPLAYER1.zip\nPLAYER2.zip\n-1\n1,Freestyle15_2";
    let record = GameRecord::from_psq(text).unwrap();
    assert_eq!(record.moves, vec![mv(7, 6), mv(7, 7)]);
}

#[test]
fn crlf_endings_are_tolerated() {
    let text = "Piskvorky 15x15, 11:11, 0\r\n7,8,0\r\n1,1,0\r\n8,8,120\r\n";
    let record = GameRecord::from_psq(text).unwrap();
    assert_eq!(record.moves, vec![mv(7, 6), mv(0, 0), mv(7, 7)]);
}

#[test]
fn header_time_fields_may_vary() {
    let text = "Piskvorky 15x15, 90:00, 1\n7,8,0\n";
    let record = GameRecord::from_psq(text).unwrap();
    assert_eq!(record.moves.len(), 1);
}

#[test]
fn empty_text_is_rejected() {
    let err = GameRecord::from_psq("").unwrap_err();
    assert!(matches!(err, GameError::MissingHeader));
}

#[test]
fn missing_header_is_rejected() {
    let text = "7,8,0\n8,8,0\n";
    let err = GameRecord::from_psq(text).unwrap_err();
    assert!(matches!(err, GameError::InvalidHeader(_)));
}

#[test]
fn non_15x15_board_is_rejected() {
    for header in ["Piskvorky 19x19, 11:11, 0", "Piskvorky 15x20, 11:11, 0"] {
        let text = format!("{header}\n7,8,0\n");
        let err = GameRecord::from_psq(&text).unwrap_err();
        assert!(
            matches!(err, GameError::UnsupportedBoardSize { .. }),
            "header '{header}' must be rejected, got {err:?}"
        );
    }
    let text = "Piskvorky 19x19, 11:11, 0\n7,8,0\n";
    let err = GameRecord::from_psq(text).unwrap_err();
    assert!(matches!(
        err,
        GameError::UnsupportedBoardSize {
            width: 19,
            height: 19
        }
    ));
}

#[test]
fn off_board_coordinate_is_rejected() {
    let text = "Piskvorky 15x15, 11:11, 0\n16,8,0\n";
    let err = GameRecord::from_psq(text).unwrap_err();
    assert!(matches!(
        err,
        GameError::OffBoard {
            line: 2,
            x: 16,
            y: 8
        }
    ));

    // 1-based coordinates: zero is off the board too.
    let text = "Piskvorky 15x15, 11:11, 0\n7,0,0\n";
    let err = GameRecord::from_psq(text).unwrap_err();
    assert!(matches!(
        err,
        GameError::OffBoard {
            line: 2,
            x: 7,
            y: 0
        }
    ));
}

#[test]
fn duplicate_move_is_rejected() {
    let text = "Piskvorky 15x15, 11:11, 0\n7,8,0\n1,1,0\n7,8,0\n";
    let err = GameRecord::from_psq(text).unwrap_err();
    assert!(matches!(
        err,
        GameError::IllegalMove {
            line: 4,
            source: PlayError::Occupied
        }
    ));
}

#[test]
fn malformed_move_line_is_rejected() {
    // A line that starts like a move (<digits>,<digits>) but is missing
    // the time field is a truncated record, not metadata.
    let text = "Piskvorky 15x15, 11:11, 0\n7,8,0\n9,8\n";
    let err = GameRecord::from_psq(text).unwrap_err();
    assert!(matches!(err, GameError::MalformedMoveLine { line: 3, .. }));

    let text = "Piskvorky 15x15, 11:11, 0\n7,8,\n";
    let err = GameRecord::from_psq(text).unwrap_err();
    assert!(matches!(err, GameError::MalformedMoveLine { line: 2, .. }));
}

#[test]
fn truncated_game_parses_as_an_ongoing_position() {
    // A record that simply ends early is a shorter, still legal game.
    let text = "Piskvorky 15x15, 11:11, 0\n7,8,0\n1,1,0\n8,8,120\n15,1,0\n9,8,250\n";
    let record = GameRecord::from_psq(text).unwrap();
    assert_eq!(record.moves.len(), 5);
    assert_eq!(replay(&record).status(), Status::Ongoing);
}

#[test]
fn move_after_the_game_ended_is_rejected() {
    // The fixture game ends on move 13; a 14th move line is part of the
    // move section and must fail the replay.
    let mut text: String = MINI_GAME
        .lines()
        .take(14) // header + 13 moves, without the metadata tail
        .collect::<Vec<_>>()
        .join("\n");
    text.push_str("\n2,2,0\n");
    let err = GameRecord::from_psq(&text).unwrap_err();
    assert!(matches!(
        err,
        GameError::IllegalMove {
            line: 15,
            source: PlayError::GameOver
        }
    ));
}
```

## `gomoku/crates/cli/tests/session.rs`

```rust
//! Headless integration tests for the session core.
//!
//! These tests drive `SessionCore` through the public command language and
//! assert on the emitted event streams and snapshots.

use cli::actor::OccupantKind;
use cli::command::{Command, CommandError, Event, SeatSel};
use cli::session::{SessionCore, SessionInit};
use engine::{Board, Color, Move, PlayError, Status};

fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}

fn play(core: &mut SessionCore, r: u8, c: u8) -> Vec<Event> {
    core.execute(Command::Play(mv(r, c))).unwrap()
}

fn new_hotseat() -> SessionCore {
    SessionCore::new(SessionInit::new_hotseat())
}

#[test]
fn hotseat_game_to_black_win_emits_game_over_and_blocks_further_play() {
    let mut core = new_hotseat();

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
    let mut core = new_hotseat();
    play(&mut core, 7, 7);

    let snapshot_before = core.snapshot();
    let err = core.execute(Command::Play(mv(7, 7))).unwrap_err();
    assert_eq!(err, CommandError::IllegalMove(PlayError::Occupied));
    assert_eq!(core.snapshot(), snapshot_before);
}

#[test]
fn branch_mid_game_emits_branched_event_with_correct_options() {
    let mut core = new_hotseat();
    play(&mut core, 7, 7);
    play(&mut core, 6, 7);
    play(&mut core, 7, 8);
    play(&mut core, 6, 8);
    play(&mut core, 7, 9);
    play(&mut core, 6, 9);

    // Back three plies and play a new move: creates a branch at ply 3.
    core.execute(Command::Back { n: 3 }).unwrap();
    let events = play(&mut core, 8, 7);

    let branched = events.iter().find_map(|e| match e {
        Event::Branched { options } => Some(options.clone()),
        _ => None,
    });
    assert_eq!(branched, Some(vec![mv(6, 8), mv(8, 7)]));
}

#[test]
fn back_forward_and_var_emit_navigation_events() {
    let mut core = new_hotseat();
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

    // Create a branch point and navigate through it.
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
fn new_emits_reset_and_returns_to_opening() {
    let mut core = new_hotseat();
    play(&mut core, 7, 7);
    play(&mut core, 6, 7);

    let events = core.execute(Command::New).unwrap();
    assert!(events.iter().any(|e| matches!(e, &Event::Reset)));
    assert_eq!(core.board(), Board::new());
    assert_eq!(core.snapshot().ply, 0);
}

#[test]
fn take_is_no_op_for_human_seats() {
    let mut core = new_hotseat();
    let events = core
        .execute(Command::Take {
            seat: SeatSel::Both,
        })
        .unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|e| matches!(
        e,
        Event::SeatChanged {
            now: cli::actor::OccupantKind::Human,
            ..
        }
    )));
}

#[test]
fn step_on_human_seat_returns_typed_error() {
    let mut core = new_hotseat();
    assert_eq!(
        core.execute(Command::Step).unwrap_err(),
        CommandError::StepRequiresAi { seat: Color::Black }
    );
}

#[test]
fn release_sets_seat_to_ai_and_take_reclaims_it() {
    let mut core = new_hotseat();
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
    let mut core = new_hotseat();
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
fn seeded_determinism_smoke() {
    let init = SessionInit {
        seed: Some(12345),
        ..SessionInit::new_hotseat()
    };
    let mut a = SessionCore::new(init.clone());
    let mut b = SessionCore::new(init);

    let script = [(7, 7), (6, 7), (7, 8), (6, 8), (7, 9), (6, 9)];
    for &(r, c) in &script {
        let ev_a = play(&mut a, r, c);
        let ev_b = play(&mut b, r, c);
        assert_eq!(ev_a, ev_b);
    }

    assert_eq!(a.snapshot(), b.snapshot());
}

// ------------------------------------------------------------------ puzzles

fn mini_pack() -> cli::puzzle::PuzzlePack {
    cli::puzzle::PuzzlePack::from_json(include_str!("fixtures/mini-pack.json")).unwrap()
}

fn puzzle_session(index: usize) -> SessionCore {
    let init = SessionInit::puzzle(
        std::path::PathBuf::from("mini-pack.json"),
        mini_pack(),
        index,
    )
    .unwrap();
    SessionCore::new(init)
}

#[test]
fn puzzle_session_roots_at_puzzle_position_with_attacker_to_move() {
    let core = puzzle_session(1);
    assert_eq!(core.board(), mini_pack().items[1].board);
    assert_eq!(core.board().to_move(), Color::White);
    let study = core.puzzle().expect("puzzle context");
    assert_eq!(study.index, 1);
    assert_eq!(study.pack.items.len(), 3);
}

#[test]
fn puzzle_init_rejects_out_of_range_index() {
    let err = SessionInit::puzzle(std::path::PathBuf::from("mini-pack.json"), mini_pack(), 3)
        .unwrap_err();
    assert!(matches!(
        err,
        cli::puzzle::PuzzleError::IndexOutOfRange { index: 3, count: 3 }
    ));
}

#[test]
fn hint_emits_first_solution_move() {
    let mut core = puzzle_session(0);
    let events = core.execute(Command::Hint).unwrap();
    assert_eq!(events, vec![Event::Hint { mv: mv(7, 7) }]);
}

#[test]
fn hint_errors_when_puzzle_has_no_line() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [], "l": [] }
    ] }"#;
    let pack = cli::puzzle::PuzzlePack::from_json(text).unwrap();
    let init = SessionInit::puzzle(std::path::PathBuf::from("pack.json"), pack, 0).unwrap();
    let mut core = SessionCore::new(init);
    assert_eq!(
        core.execute(Command::Hint).unwrap_err(),
        CommandError::NoSolutionLine { index: 0, count: 1 }
    );
}

#[test]
fn reveal_emits_full_claimed_line() {
    let mut core = puzzle_session(2);
    let events = core.execute(Command::Reveal).unwrap();
    assert_eq!(
        events,
        vec![Event::SolutionRevealed {
            line: vec![
                (mv(7, 10), Color::Black),
                (mv(7, 6), Color::White),
                (mv(7, 11), Color::Black),
            ]
        }]
    );
}

#[test]
fn next_and_prev_load_a_fresh_root_and_report_position() {
    let mut core = puzzle_session(0);
    // Dirty the tree first; navigation must replace the root wholesale.
    play(&mut core, 9, 9);

    let events = core.execute(Command::NextPuzzle).unwrap();
    assert_eq!(
        events,
        vec![Event::PuzzleLoaded {
            index: 1,
            count: 3,
            attacker: Color::White,
        }]
    );
    assert_eq!(core.snapshot().ply, 0);
    assert_eq!(core.board(), mini_pack().items[1].board);

    let events = core.execute(Command::PrevPuzzle).unwrap();
    assert_eq!(
        events,
        vec![Event::PuzzleLoaded {
            index: 0,
            count: 3,
            attacker: Color::Black,
        }]
    );
    assert_eq!(core.board(), mini_pack().items[0].board);
}

#[test]
fn next_prev_clamp_at_pack_ends() {
    let mut core = puzzle_session(0);
    assert_eq!(
        core.execute(Command::PrevPuzzle).unwrap_err(),
        CommandError::AtFirstPuzzle { count: 3 }
    );
    core.execute(Command::NextPuzzle).unwrap();
    core.execute(Command::NextPuzzle).unwrap();
    assert_eq!(
        core.execute(Command::NextPuzzle).unwrap_err(),
        CommandError::AtLastPuzzle { count: 3 }
    );
    // Clamping leaves the current puzzle loaded.
    assert_eq!(core.puzzle().unwrap().index, 2);
}

#[test]
fn puzzle_commands_error_without_a_pack() {
    let mut core = new_hotseat();
    assert_eq!(
        core.execute(Command::Hint).unwrap_err(),
        CommandError::NoPuzzleLoaded
    );
    assert_eq!(
        core.execute(Command::Reveal).unwrap_err(),
        CommandError::NoPuzzleLoaded
    );
    assert_eq!(
        core.execute(Command::NextPuzzle).unwrap_err(),
        CommandError::NoPuzzleLoaded
    );
    assert_eq!(
        core.execute(Command::PrevPuzzle).unwrap_err(),
        CommandError::NoPuzzleLoaded
    );
}

#[test]
fn puzzle_position_supports_full_study_flow() {
    let mut core = puzzle_session(2);
    // Play the claimed line head, then branch with a wrong try.
    play(&mut core, 7, 10); // k8 — the claimed first move
    core.execute(Command::Back { n: 1 }).unwrap();
    let events = play(&mut core, 10, 10);
    assert!(
        events.iter().any(|e| matches!(e, &Event::Branched { .. })),
        "expected Branched, got {events:?}"
    );
    core.execute(Command::Back { n: 1 }).unwrap();
    let events = core.execute(Command::Forward { n: 1 }).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, &Event::BranchPoint { .. })),
        "expected BranchPoint, got {events:?}"
    );
    let events = core.execute(Command::Var { index: 0 }).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, &Event::CursorMoved { .. })),
        "expected CursorMoved, got {events:?}"
    );
}

#[test]
fn new_resets_to_the_initially_loaded_puzzle() {
    let mut core = puzzle_session(0);
    core.execute(Command::NextPuzzle).unwrap();
    play(&mut core, 9, 9);
    let events = core.execute(Command::New).unwrap();
    assert!(events.iter().any(|e| matches!(e, &Event::Reset)));
    assert_eq!(core.puzzle().unwrap().index, 0);
    assert_eq!(core.board(), mini_pack().items[0].board);
}

// ------------------------------------------------------------------ game records

fn mini_game() -> cli::game::GameRecord {
    cli::game::GameRecord::from_psq(include_str!("fixtures/mini-game.psq")).unwrap()
}

fn game_session() -> SessionCore {
    let init = SessionInit::game(std::path::PathBuf::from("mini-game.psq"), mini_game());
    SessionCore::new(init)
}

#[test]
fn game_session_replays_the_record_as_the_main_line_with_cursor_at_end() {
    let core = game_session();
    let snap = core.snapshot();
    assert_eq!(snap.ply, 13, "main line length equals the move count");
    assert_eq!(core.board().moves().len(), 13);
    // The engine adjudicates the final position: freestyle five for Black.
    assert_eq!(snap.status, Status::Won(Color::Black));
    assert_eq!(snap.last_move, Some(mv(7, 10)));
    // Hotseat seats by default.
    assert_eq!(snap.black, OccupantKind::Human);
    assert_eq!(snap.white, OccupantKind::Human);

    let study = core.game().expect("game study context");
    assert_eq!(study.record.moves.len(), 13);
    assert_eq!(
        study.source.file_stem().unwrap().to_str().unwrap(),
        "mini-game"
    );
}

#[test]
fn game_session_navigation_works_across_the_loaded_line() {
    let mut core = game_session();

    let events = core.execute(Command::Back { n: 5 }).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, &Event::CursorMoved { ply: 8 }))
    );
    assert_eq!(core.snapshot().status, Status::Ongoing);
    assert_eq!(core.board().to_move(), Color::Black);

    let events = core.execute(Command::Forward { n: 5 }).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, &Event::CursorMoved { ply: 13 }))
    );
    assert_eq!(core.snapshot().status, Status::Won(Color::Black));

    // All the way back: the game was replayed from an empty root.
    core.execute(Command::Back { n: 13 }).unwrap();
    assert_eq!(core.snapshot().ply, 0);
    assert_eq!(core.board(), Board::new());
}

#[test]
fn game_session_branch_and_var_at_a_mid_game_cursor() {
    let mut core = game_session();
    core.execute(Command::Back { n: 1 }).unwrap(); // ply 12, White to move

    // An alternative White try branches off the recorded line.
    let events = play(&mut core, 9, 3);
    assert!(
        events.iter().any(|e| matches!(e, &Event::Branched { .. })),
        "expected Branched, got {events:?}"
    );

    core.execute(Command::Back { n: 1 }).unwrap();
    let events = core.execute(Command::Forward { n: 1 }).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, &Event::BranchPoint { .. })),
        "expected BranchPoint, got {events:?}"
    );

    // Variation 0 is the recorded winning move; variation 1 the new try.
    let events = core.execute(Command::Var { index: 0 }).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, &Event::CursorMoved { ply: 13 }))
    );
    assert_eq!(core.board().moves().last().copied(), Some(mv(7, 10)));
    assert_eq!(core.snapshot().status, Status::Won(Color::Black));

    core.execute(Command::Back { n: 1 }).unwrap();
    core.execute(Command::Var { index: 1 }).unwrap();
    assert_eq!(core.board().moves().last().copied(), Some(mv(9, 3)));
    assert_eq!(core.snapshot().status, Status::Ongoing);
}

#[test]
fn game_session_take_release_and_step_from_a_mid_game_cursor() {
    let mut core = game_session();
    core.execute(Command::Back { n: 5 }).unwrap(); // ply 8, Black to move

    let events = core
        .execute(Command::Release {
            seat: SeatSel::Black,
        })
        .unwrap();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::SeatChanged {
            seat: Color::Black,
            now: OccupantKind::Ai,
        }
    )));

    let events = core.execute(Command::Step).unwrap();
    assert!(events.iter().any(|e| matches!(
        e,
        &Event::Moved {
            by: OccupantKind::Ai,
            ..
        }
    )));
    assert_eq!(core.snapshot().ply, 9);
    // The AI search at this mid-game position is reviewable.
    assert!(core.search_info().is_some());

    let events = core
        .execute(Command::Take {
            seat: SeatSel::Black,
        })
        .unwrap();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::SeatChanged {
            seat: Color::Black,
            now: OccupantKind::Human,
        }
    )));
}

#[test]
fn game_session_search_info_stays_on_its_node_when_navigating() {
    let mut core = game_session();
    core.execute(Command::Back { n: 5 }).unwrap(); // ply 8, Black to move
    core.execute(Command::Release {
        seat: SeatSel::Black,
    })
    .unwrap();
    core.execute(Command::Step).unwrap();
    assert!(core.search_info().is_some());

    // Navigating away leaves the info behind; navigating back finds it.
    core.execute(Command::Back { n: 1 }).unwrap();
    assert!(core.search_info().is_none());
    let events = core.execute(Command::Forward { n: 1 }).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, &Event::BranchPoint { .. })),
        "the AI move branched off the recorded line, got {events:?}"
    );
    core.execute(Command::Var { index: 1 }).unwrap();
    assert!(core.search_info().is_some());
}

#[test]
fn new_resets_to_the_fully_replayed_game() {
    let mut core = game_session();
    core.execute(Command::Back { n: 13 }).unwrap();
    play(&mut core, 2, 2);
    let events = core.execute(Command::New).unwrap();
    assert!(events.iter().any(|e| matches!(e, &Event::Reset)));
    assert_eq!(core.snapshot().ply, 13);
    assert_eq!(core.snapshot().status, Status::Won(Color::Black));
}
```

## `gomoku/crates/cli/tests/fixtures/mini-game.psq`

```text
Piskvorky 15x15, 11:11, 0
7,8,0
1,1,0
8,8,120
15,1,0
9,8,250
1,15,0
5,5,410
15,15,0
10,8,530
3,12,60
6,6,700
12,3,0
11,8,900
CARBON17.zip
PISQ7.zip
-1
1,Freestyle15_2
```

## `gomoku/crates/cli/game_demo.txt`

```text
# game_demo.txt — studying a recorded game at the table.
# Run from gomoku/:
#   cargo run -p cli --bin sessiondemo -- --game crates/cli/tests/fixtures/mini-game.psq --script crates/cli/game_demo.txt
# The record opens at the END: 13 plies, Black has won (freestyle five
# on row 8). Step back to the position where the win was still open.
back 5
# Ply 8, Black to move. Peek at the recorded continuation, then return.
forward 2
back 2
# Branch: d6 is a legal alternative to the recorded j8.
d6
back
forward
# Variation 0 is the recorded winning line.
var 0
forward 4
# Back at ply 8: release the Black seat and let the AI analyze the
# mid-game position.
back 5
release black
step
info
take black
quit
```

## Verification

The quoted files were extracted from commit `3bf0544` of the reference
worktree. To verify them yourself:

```bash
#!/usr/bin/env bash
set -euo pipefail
REPO=/Users/wgiersche/workspace/Project-Ellie/rust-ml-ref-playable-cli
COMMIT=3bf0544
SOLUTION=/Users/wgiersche/workspace/Project-Ellie/rust-ml/docs/tutorials/playable-cli-tutorial/08-positions-from-files-game-records/01-solution.md

python3 - "$REPO" "$COMMIT" "$SOLUTION" <<'PY'
import re, subprocess, sys
repo, commit, solution_path = sys.argv[1:4]
with open(solution_path, "r") as f:
    text = f.read()
# Each quoted file is a ## `path` header followed by a fenced code block.
# The closing fence is on its own line, so we match that explicitly.
pattern = re.compile(
    r'^## `([^`]+)`\n+```(?:rust|text|json|toml)\n(.*?)\n```(?:\n|$)',
    re.MULTILINE | re.DOTALL,
)
ok = True
for m in pattern.finditer(text):
    path = m.group(1)
    if not path.endswith((".rs", ".txt", ".json", ".psq", ".toml")):
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
