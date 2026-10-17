> **Opt-in solution — read only after you have tried the chapter, or if you have been stuck for more than twenty minutes.**
>
> This file quotes the verified reference implementation for chapter 02.
> The source files below are byte-for-byte copies from commit `8340f44`
> of the reference worktree (`rust-ml-ref-playable-cli`).
>
> `src/history.rs` from chapter 01 is **unchanged** in this chapter.
> The diff from chapter 01 (`1420081`) to this chapter (`8340f44`)
> touches:
> ```
>  Cargo.toml                              |   5 +
>  src/actor.rs                             |  79 +++++
>  src/bin/historydemo.rs                   |  82 ++---
>  src/bin/sessiondemo.rs                   | 146 ++++++++
>  src/command.rs                           | 542 +++++++++++++++++++++++++++++
>  src/fmt.rs                               |  41 +++
>  src/lib.rs                               |   1 +
>  src/session.rs                           | 579 +++++++++++++++++++++++++++++++
>  tests/session.rs                         | 203 +++++++++++
>  9 files changed, 1616 insertions(+), 62 deletions(-)
> ```

# Chapter 02 — opt-in solution

## `src/command.rs`

````rust
//! Command language and events.
//!
//! The command language is the shared vocabulary between every front-end.
//! Text parsing is a convenience layer; the `Command` enum is the real
//! interface to `SessionCore`. Events are facts about what changed and are
//! the only output a front-end needs to render.

use engine::{Color, Move, PlayError, Status};
use std::fmt;
use thiserror::Error;

use crate::actor::OccupantKind;
use crate::history::HistoryError;

/// An intent issued to `SessionCore::execute`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Play a move at the cursor. Valid when the side-to-move seat is human.
    Play(Move),
    /// Let the AI occupant of the side-to-move seat search and play.
    Step,
    /// Move the cursor toward the root by `n` plies (default 1).
    Back {
        /// Plies to move back.
        n: u32,
    },
    /// Move the cursor toward a leaf by `n` plies (default 1).
    Forward {
        /// Plies to move forward.
        n: u32,
    },
    /// Choose a child variation by index at a branch point.
    Var {
        /// Zero-based child index.
        index: usize,
    },
    /// Make a seat human.
    Take {
        /// Seat or seats to take.
        seat: SeatSel,
    },
    /// Release a seat to the AI (uses the seat's current default config).
    Release {
        /// Seat or seats to release.
        seat: SeatSel,
    },
    /// Override the simulation count for a seat.
    SetSims {
        /// Seat or seats to configure.
        seat: SeatSel,
        /// New simulation count.
        n: u32,
    },
    /// Start a fresh session from the configured opening.
    New,
    /// Print the command summary.
    Help,
    /// Exit the front-end.
    Quit,
}

/// A seat selector used by `Take`, `Release`, and `SetSims`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeatSel {
    /// The Black seat.
    Black,
    /// The White seat.
    White,
    /// Both seats.
    Both,
}

impl fmt::Display for SeatSel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SeatSel::Black => write!(f, "black"),
            SeatSel::White => write!(f, "white"),
            SeatSel::Both => write!(f, "both"),
        }
    }
}

/// A fact emitted by `SessionCore::execute` describing what changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A stone was played at the cursor.
    Moved {
        /// The move played.
        mv: Move,
        /// Who played it.
        by: OccupantKind,
    },
    /// A new sibling branch was created at the cursor's parent.
    Branched {
        /// The available child moves at the branch point.
        options: Vec<Move>,
    },
    /// The cursor moved to a different ply.
    CursorMoved {
        /// The new cursor ply.
        ply: u32,
    },
    /// `Forward` stopped at a branch point and needs a `Var` choice.
    BranchPoint {
        /// Index/move pairs for the available children.
        options: Vec<(usize, Move)>,
    },
    /// A seat's occupant changed.
    SeatChanged {
        /// The seat that changed.
        seat: Color,
        /// The new occupant kind.
        now: OccupantKind,
    },
    /// A seat's search configuration changed.
    ConfigChanged {
        /// The seat that changed.
        seat: Color,
        /// The new simulation count.
        sims: u32,
    },
    /// The game ended at the cursor.
    GameOver {
        /// Final status.
        result: Status,
    },
    /// The session was reset to its initial opening.
    Reset,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::Moved { mv, by } => {
                write!(f, "Moved {} by {by:?}", fmt_move(*mv))
            }
            Event::Branched { options } => {
                let opts = options
                    .iter()
                    .map(|m| fmt_move(*m))
                    .collect::<Vec<_>>()
                    .join(" ");
                write!(f, "Branched: {opts}")
            }
            Event::CursorMoved { ply } => write!(f, "Cursor moved to ply {ply}"),
            Event::BranchPoint { options } => {
                let opts = options
                    .iter()
                    .map(|(i, m)| format!("{i}: {}", fmt_move(*m)))
                    .collect::<Vec<_>>()
                    .join(" ");
                write!(f, "Branch point: {opts}")
            }
            Event::SeatChanged { seat, now } => write!(f, "{seat:?} seat is now {now:?}"),
            Event::ConfigChanged { seat, sims } => {
                write!(f, "{seat:?} simulations set to {sims}")
            }
            Event::GameOver { result } => write!(f, "Game over: {result:?}"),
            Event::Reset => write!(f, "Reset"),
        }
    }
}

fn fmt_move(mv: Move) -> String {
    format!("({}, {})", mv.row(), mv.col())
}

/// Errors returned by the command parser and by `SessionCore::execute`.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CommandError {
    /// The line could not be parsed as a command.
    #[error("Parse error: {0}")]
    Parse(String),
    /// The move is illegal in the cursor position.
    #[error("Illegal move: {0}")]
    IllegalMove(#[from] PlayError),
    /// The game has already ended.
    #[error("Game is already over")]
    GameOver,
    /// The requested AI-driven command is not implemented in Task 2.
    #[error("AI seats are not yet implemented (Task 3): {0}")]
    AiSeatsNotImplemented(String),
    /// `Forward` reached a leaf without finding a branch point.
    #[error("Forward reached a leaf")]
    ForwardAtLeaf,
    /// The cursor is not at a branch point.
    #[error("No branch point at cursor")]
    NotABranchPoint,
    /// The requested variation index does not exist.
    #[error("Variation index {index} out of bounds (options: {count})")]
    VarOutOfBounds {
        /// Requested index.
        index: usize,
        /// Number of children available.
        count: usize,
    },
    /// The requested seat selector is invalid.
    #[error("Invalid seat: {0}")]
    InvalidSeat(String),
    /// The command requires a non-zero simulation count.
    #[error("Simulation count must be greater than zero")]
    ZeroSims,
}

impl From<HistoryError> for CommandError {
    fn from(err: HistoryError) -> Self {
        match err {
            HistoryError::IllegalMove(e) => CommandError::IllegalMove(e),
            HistoryError::VarOutOfBounds { index, child_count } => CommandError::VarOutOfBounds {
                index,
                count: child_count,
            },
        }
    }
}

/// Parse a line of user input into a `Command`.
///
/// Parsing is case-insensitive. Whitespace separates tokens. Documented
/// forms:
///
/// * `7 7` or `play 7 7` → `Play`
/// * `step`
/// * `back [n]` (default 1)
/// * `forward [n]` (default 1)
/// * `var N`
/// * `take black|white|both`
/// * `release black|white|both`
/// * `sims black|white N`
/// * `new`
/// * `h`, `help`, `?`
/// * `q`, `quit`, `exit`
///
/// # Errors
/// Returns `CommandError::Parse` for any line that does not match a
/// documented form.
pub fn parse(input: &str) -> Result<Command, CommandError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CommandError::Parse("empty line".to_string()));
    }

    let lower = trimmed.to_ascii_lowercase();
    let mut tokens = lower.split_whitespace();
    let first = tokens.next().expect("non-empty input has a token");

    match first {
        "step" => {
            ensure_no_more(&mut tokens, "step")?;
            Ok(Command::Step)
        }
        "back" => {
            let n = parse_optional_u32(&mut tokens, 1, "back")?;
            Ok(Command::Back { n })
        }
        "forward" => {
            let n = parse_optional_u32(&mut tokens, 1, "forward")?;
            Ok(Command::Forward { n })
        }
        "var" => {
            let index = parse_required_usize(&mut tokens, "var")?;
            Ok(Command::Var { index })
        }
        "take" => {
            let seat = parse_required_seat(&mut tokens, "take")?;
            ensure_no_more(&mut tokens, "take")?;
            Ok(Command::Take { seat })
        }
        "release" => {
            let seat = parse_required_seat(&mut tokens, "release")?;
            ensure_no_more(&mut tokens, "release")?;
            Ok(Command::Release { seat })
        }
        "sims" => {
            let seat = parse_required_seat(&mut tokens, "sims")?;
            let n = parse_required_u32(&mut tokens, "sims")?;
            Ok(Command::SetSims { seat, n })
        }
        "new" => {
            ensure_no_more(&mut tokens, "new")?;
            Ok(Command::New)
        }
        "help" | "h" | "?" => {
            ensure_no_more(&mut tokens, "help")?;
            Ok(Command::Help)
        }
        "quit" | "q" | "exit" => {
            ensure_no_more(&mut tokens, "quit")?;
            Ok(Command::Quit)
        }
        "play" => {
            let (row, col) = parse_move_pair(&mut tokens, "play")?;
            Ok(Command::Play(Move::new(row, col).ok_or_else(|| {
                CommandError::Parse(format!("move out of bounds: ({row}, {col})"))
            })?))
        }
        _ => {
            // Try parsing `7 7` form: first token is a row number.
            let row = first
                .parse::<u8>()
                .map_err(|_| CommandError::Parse(format!("unknown command: {first}")))?;
            let col = parse_required_token(&mut tokens, "column")?;
            let col = col
                .parse::<u8>()
                .map_err(|_| CommandError::Parse(format!("expected column number, got '{col}'")))?;
            ensure_no_more(&mut tokens, "play")?;
            Ok(Command::Play(Move::new(row, col).ok_or_else(|| {
                CommandError::Parse(format!("move out of bounds: ({row}, {col})"))
            })?))
        }
    }
}

fn ensure_no_more<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    cmd: &str,
) -> Result<(), CommandError> {
    if let Some(extra) = tokens.next() {
        return Err(CommandError::Parse(format!(
            "{cmd} takes no arguments, got extra: '{extra}'"
        )));
    }
    Ok(())
}

fn parse_required_token<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    name: &str,
) -> Result<String, CommandError> {
    tokens
        .next()
        .map(|s| s.to_string())
        .ok_or_else(|| CommandError::Parse(format!("missing {name}")))
}

fn parse_optional_u32<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    default: u32,
    cmd: &str,
) -> Result<u32, CommandError> {
    match tokens.next() {
        None => Ok(default),
        Some(t) => {
            let n = t.parse::<u32>().map_err(|_| {
                CommandError::Parse(format!("{cmd} expects a non-negative number, got '{t}'"))
            })?;
            ensure_no_more(tokens, cmd)?;
            Ok(n)
        }
    }
}

fn parse_required_u32<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    cmd: &str,
) -> Result<u32, CommandError> {
    let t = tokens
        .next()
        .ok_or_else(|| CommandError::Parse(format!("{cmd} expects a number")))?;
    let n = t.parse::<u32>().map_err(|_| {
        CommandError::Parse(format!("{cmd} expects a non-negative number, got '{t}'"))
    })?;
    ensure_no_more(tokens, cmd)?;
    Ok(n)
}

fn parse_required_usize<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    cmd: &str,
) -> Result<usize, CommandError> {
    let t = tokens
        .next()
        .ok_or_else(|| CommandError::Parse(format!("{cmd} expects an index")))?;
    let n = t.parse::<usize>().map_err(|_| {
        CommandError::Parse(format!("{cmd} expects a non-negative index, got '{t}'"))
    })?;
    ensure_no_more(tokens, cmd)?;
    Ok(n)
}

fn parse_required_seat<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    cmd: &str,
) -> Result<SeatSel, CommandError> {
    let t = tokens
        .next()
        .ok_or_else(|| CommandError::Parse(format!("{cmd} expects a seat: black|white|both")))?;
    match t {
        "black" => Ok(SeatSel::Black),
        "white" => Ok(SeatSel::White),
        "both" => Ok(SeatSel::Both),
        _ => Err(CommandError::Parse(format!(
            "{cmd} expects a seat: black|white|both, got '{t}'"
        ))),
    }
}

fn parse_move_pair<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    cmd: &str,
) -> Result<(u8, u8), CommandError> {
    let row = parse_required_token(tokens, "row")?;
    let row = row
        .parse::<u8>()
        .map_err(|_| CommandError::Parse(format!("{cmd} expects row number, got '{row}'")))?;
    let col = parse_required_token(tokens, "column")?;
    let col = col
        .parse::<u8>()
        .map_err(|_| CommandError::Parse(format!("{cmd} expects column number, got '{col}'")))?;
    ensure_no_more(tokens, cmd)?;
    Ok((row, col))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    #[test]
    fn parse_play_bare_numbers() {
        assert_eq!(parse("7 7"), Ok(Command::Play(mv(7, 7))));
        assert_eq!(parse("0 14"), Ok(Command::Play(mv(0, 14))));
        assert_eq!(parse("14 0"), Ok(Command::Play(mv(14, 0))));
    }

    #[test]
    fn parse_play_explicit() {
        assert_eq!(parse("play 7 7"), Ok(Command::Play(mv(7, 7))));
        assert_eq!(parse("PLAY 0 14"), Ok(Command::Play(mv(0, 14))));
    }

    #[test]
    fn parse_step() {
        assert_eq!(parse("step"), Ok(Command::Step));
        assert_eq!(parse("STEP"), Ok(Command::Step));
    }

    #[test]
    fn parse_back_default_and_explicit() {
        assert_eq!(parse("back"), Ok(Command::Back { n: 1 }));
        assert_eq!(parse("back 3"), Ok(Command::Back { n: 3 }));
        assert_eq!(parse("BACK 0"), Ok(Command::Back { n: 0 }));
    }

    #[test]
    fn parse_forward_default_and_explicit() {
        assert_eq!(parse("forward"), Ok(Command::Forward { n: 1 }));
        assert_eq!(parse("forward 5"), Ok(Command::Forward { n: 5 }));
    }

    #[test]
    fn parse_var() {
        assert_eq!(parse("var 0"), Ok(Command::Var { index: 0 }));
        assert_eq!(parse("VAR 2"), Ok(Command::Var { index: 2 }));
    }

    #[test]
    fn parse_take_release() {
        assert_eq!(
            parse("take black"),
            Ok(Command::Take {
                seat: SeatSel::Black
            })
        );
        assert_eq!(
            parse("take white"),
            Ok(Command::Take {
                seat: SeatSel::White
            })
        );
        assert_eq!(
            parse("take both"),
            Ok(Command::Take {
                seat: SeatSel::Both
            })
        );
        assert_eq!(
            parse("release black"),
            Ok(Command::Release {
                seat: SeatSel::Black
            })
        );
        assert_eq!(
            parse("RELEASE WHITE"),
            Ok(Command::Release {
                seat: SeatSel::White
            })
        );
    }

    #[test]
    fn parse_sims() {
        assert_eq!(
            parse("sims black 800"),
            Ok(Command::SetSims {
                seat: SeatSel::Black,
                n: 800,
            })
        );
        assert_eq!(
            parse("sims white 50"),
            Ok(Command::SetSims {
                seat: SeatSel::White,
                n: 50,
            })
        );
    }

    #[test]
    fn parse_new_help_quit() {
        assert_eq!(parse("new"), Ok(Command::New));
        assert_eq!(parse("h"), Ok(Command::Help));
        assert_eq!(parse("help"), Ok(Command::Help));
        assert_eq!(parse("?"), Ok(Command::Help));
        assert_eq!(parse("q"), Ok(Command::Quit));
        assert_eq!(parse("quit"), Ok(Command::Quit));
        assert_eq!(parse("exit"), Ok(Command::Quit));
    }

    #[test]
    fn parse_errors() {
        assert!(parse("").is_err());
        assert!(parse("7").is_err());
        assert!(parse("7 7 7").is_err());
        assert!(parse("play 7").is_err());
        assert!(parse("back x").is_err());
        assert!(parse("var").is_err());
        assert!(parse("take red").is_err());
        assert!(parse("sims black").is_err());
        assert!(parse("sims black 100 extra").is_err());
        assert!(parse("unknown").is_err());
        assert!(parse("step extra").is_err());
    }

    #[test]
    fn parse_out_of_bounds_move_is_error() {
        assert!(parse("15 7").is_err());
        assert!(parse("7 15").is_err());
    }
}
````

## `src/session.rs`

````rust
//! Session core — headless state machine.
//!
//! `SessionCore` is the heart of the study table. It owns the variation
//! tree, the seat table, and AI search defaults. Commands go in; events
//! and snapshots come out. No terminal code, no I/O.

use engine::{Board, Color, Move, Status};
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::actor::{AiConfig, Occupant, OccupantKind};
use crate::command::{Command, CommandError, Event, SeatSel};
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

    /// Start a hotseat session from a custom opening position.
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
    #[allow(dead_code)]
    rng: StdRng,
    init: SessionInit,
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
            Command::Step => Err(CommandError::AiSeatsNotImplemented(
                "Step requires an AI seat (Task 3)".to_string(),
            )),
            Command::Back { n } => {
                let _moved = self.history.back(n);
                Ok(vec![Event::CursorMoved {
                    ply: self.history.ply(),
                }])
            }
            Command::Forward { n } => self.execute_forward(n),
            Command::Var { index } => self.execute_var(index),
            Command::Take { seat } => self.execute_take(seat),
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
````

## `src/actor.rs`

````rust
//! Seat occupants and AI configuration.
//!
//! The study table has two seats (Black and White). Each seat is occupied
//! by either a human or an AI. For Task 2 both seats are human; the AI
//! plumbing lands in Task 3, but the configuration shapes are defined now
//! so the `SessionCore` API stays stable.

use serde::{Deserialize, Serialize};

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
}

impl Default for AiConfig {
    /// Default AlphaZero-style search settings.
    ///
    /// - 400 simulations
    /// - c_puct 1.5
    /// - deterministic move selection (temperature 0.0)
    /// - no Dirichlet noise
    fn default() -> Self {
        Self {
            evaluator: EvaluatorKind::Tactics,
            simulations: 400,
            c_puct: 1.5,
            temperature: 0.0,
            noise_epsilon: 0.0,
        }
    }
}

/// Evaluator backend available to an AI seat.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum EvaluatorKind {
    /// The hand-woven tactics evaluator (Burn-free baseline).
    Tactics,
}
````

## `src/fmt.rs`

````rust
//! Small text-formatting helpers shared by the demo binaries.
//!
//! These are intentionally plain `String`-returning functions so they can
//! be used by both the unit tests and the demo binaries without pulling
//! in any terminal dependencies.

use engine::{Board, Color, Move};

/// Render a 15×15 board as compact ASCII.
///
/// Black stones are shown as `X`, White stones as `O`, empty cells as `.`.
/// The optional `last_move` is marked with lowercase `x`/`o`.
#[must_use]
pub fn render_board(board: &Board, last_move: Option<Move>) -> String {
    let mut lines = Vec::new();
    lines.push("    0  1  2  3  4  5  6  7  8  9 10 11 12 13 14".to_string());
    for r in 0..15u8 {
        let mut row = format!("{:2}  ", r);
        for c in 0..15u8 {
            let mv = Move::new(r, c).expect("board coordinates are valid");
            let is_last = last_move == Some(mv);
            let ch = match board.stone_at(mv) {
                Some(Color::Black) if is_last => 'x',
                Some(Color::White) if is_last => 'o',
                Some(Color::Black) => 'X',
                Some(Color::White) => 'O',
                None => '.',
            };
            row.push(ch);
            row.push_str("  ");
        }
        lines.push(row);
    }
    lines.join("\n")
}

/// Format a move as a numeric `(row, col)` pair.
#[must_use]
pub fn format_move(mv: Move) -> String {
    format!("({}, {})", mv.row(), mv.col())
}
````

## `src/bin/sessiondemo.rs`

````rust
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
  ROW COL          play a move, e.g. "7 7"
  play ROW COL     same as above
  step             let the AI play (Task 3)
  back [N]         go back N plies (default 1)
  forward [N]      go forward N plies (default 1)
  var N            choose child variation N
  take black|white|both      make seat(s) human
  release black|white|both   release seat(s) to AI (Task 3)
  sims black|white N         set per-seat simulations (Task 3)
  new              reset to opening
  h, help, ?       show this help
  q, quit, exit    quit
"#;

fn main() {
    let args: Vec<String> = env::args().collect();
    let script_path = args
        .iter()
        .position(|a| a == "--script")
        .and_then(|i| args.get(i + 1));

    let init = SessionInit::new_hotseat();
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
            .map(|(i, m)| format!("{i}:({}, {})", m.row(), m.col()))
            .collect();
        println!("branches: {}", opts.join("  "));
    }
    println!("{}", render_board(&core.board(), snap.last_move));
    println!();
}
````

## `tests/session.rs`

````rust
//! Headless integration tests for the session core.
//!
//! These tests drive `SessionCore` through the public command language and
//! assert on the emitted event streams and snapshots.

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
fn step_release_and_set_sims_are_not_yet_implemented() {
    let mut core = new_hotseat();

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
````
