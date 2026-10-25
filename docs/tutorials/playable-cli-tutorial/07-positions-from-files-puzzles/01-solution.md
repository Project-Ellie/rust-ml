> **Opt-in solution — read only after you have tried the chapter, or if you have been stuck for more than twenty minutes.**
>
> This file quotes the target state your work should produce for
> chapter 07, verbatim. New files and every file with substantive
> edits are quoted whole, including `command.rs` and `session.rs`
> although earlier chapters quoted them too — each chapter's solution
> is self-contained. Target state verified against the tutorial's
> reference build, commit `d6c2af3`.

# Chapter 07 — opt-in solution

## Files not quoted

The following files also change in this chapter, but only
mechanically, so this file does not quote them:

* `gomoku/crates/cli/tests/acceptance.rs` — one `puzzle: None` field in a
  `SessionInit` struct literal.
* `gomoku/crates/cli/tests/actor.rs` — the same field in three literals.
* `gomoku/crates/cli/tests/study.rs` — the same field in one literal, plus
  the `Hint` / `SolutionRevealed` / `PuzzleLoaded` arms in the
  `event_type` helper.

## `gomoku/crates/cli/src/puzzle.rs`

**New in this chapter:** the whole module — `Puzzle`, `PuzzlePack`
(with `from_json` / `get`), `PuzzleError`, and the pack-item parsing
helpers.

```rust
//! Puzzle pack loading — positions with a claimed solution line.
//!
//! Puzzles are not a game mode; a puzzle is a position plus the pack's
//! claimed forced-win line, loaded into the study table for analysis.
//! The pack format is the corpus format (`data/puzzles/README.md` in the
//! main repo): JSON `{v, count, items}` with items `{i, a, d, b, w, l}`
//! where `a` is the attacker (side to move), `b`/`w` the stone lists,
//! and `l` the claimed line as `[x, y, side]` triples.
//!
//! Coordinate convention (kept from the legacy puzzle viewer, which
//! rendered these packs correctly): file coordinates are 0-based
//! `[x, y]` with `x` the column and `y` the row — `Move::new(y, x)`.
//! Line side markers are `1` = Black, `2` = White.

use engine::{Board, Color, Move, PositionError};
use serde::Deserialize;

/// One puzzle position, parsed and validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puzzle {
    /// Zero-based index within the pack.
    pub index: usize,
    /// Side to move (the attacker in VCF terminology).
    pub attacker: Color,
    /// The position, ready to seed a variation tree root.
    pub board: Board,
    /// The pack's claimed forced-win line with the side for each move.
    /// Empty for unlabelled packs (positions only).
    pub solution: Vec<(Move, Color)>,
}

/// A parsed puzzle pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PuzzlePack {
    /// All puzzles in file order.
    pub items: Vec<Puzzle>,
}

impl PuzzlePack {
    /// Parse and validate a pack from its JSON text.
    ///
    /// Every item is validated: coordinates on the board, no duplicate or
    /// overlapping stones, stone counts consistent with the attacker, and
    /// the claimed line (if any) starting with the attacker.
    ///
    /// # Errors
    /// Returns a typed `PuzzleError` describing the first violation.
    pub fn from_json(text: &str) -> Result<Self, PuzzleError> {
        let file: PackFile = serde_json::from_str(text)?;

        if file.count != file.items.len() {
            return Err(PuzzleError::CountMismatch {
                declared: file.count,
                actual: file.items.len(),
            });
        }

        let items = file
            .items
            .into_iter()
            .enumerate()
            .map(|(pos, item)| parse_item(pos, item))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { items })
    }

    /// The puzzle at `index`.
    ///
    /// # Errors
    /// Returns `PuzzleError::IndexOutOfRange` if `index` is not in the pack.
    pub fn get(&self, index: usize) -> Result<&Puzzle, PuzzleError> {
        self.items.get(index).ok_or(PuzzleError::IndexOutOfRange {
            index,
            count: self.items.len(),
        })
    }
}

/// Everything that can go wrong when parsing a puzzle pack.
#[derive(Debug, thiserror::Error)]
pub enum PuzzleError {
    /// The text is not valid JSON for the pack format.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The top-level `count` field does not match `items.len()`.
    #[error("count mismatch: declared {declared}, actual {actual}")]
    CountMismatch {
        /// Value of the `count` field.
        declared: usize,
        /// Length of the `items` array.
        actual: usize,
    },

    /// The `a` (attacker) field is not "black" or "white".
    #[error("puzzle {item}: invalid attacker '{value}'")]
    InvalidAttacker {
        /// Puzzle index within the file (the item's `i` field).
        item: u32,
        /// Raw value from the file.
        value: String,
    },

    /// A coordinate is outside the 15×15 board.
    #[error("puzzle {item}: coordinate ({x}, {y}) is off the board")]
    InvalidCoord {
        /// Puzzle index within the file (the item's `i` field).
        item: u32,
        /// File x coordinate (column, 0-based).
        x: u8,
        /// File y coordinate (row, 0-based).
        y: u8,
    },

    /// The same cell appears twice in one color's stone list.
    #[error("puzzle {item}: duplicate stone at ({x}, {y})")]
    DuplicateStone {
        /// Puzzle index within the file (the item's `i` field).
        item: u32,
        /// File x coordinate (column) of the duplicated cell.
        x: u8,
        /// File y coordinate (row) of the duplicated cell.
        y: u8,
    },

    /// A cell appears in both Black's and White's stone lists.
    #[error("puzzle {item}: black and white stones overlap")]
    OverlappingColors {
        /// Puzzle index within the file (the item's `i` field).
        item: u32,
    },

    /// Stone counts violate the Gomoku turn invariant for the attacker.
    #[error("puzzle {item}: invalid counts (black={black}, white={white}, attacker={attacker:?})")]
    InvalidCounts {
        /// Puzzle index within the file (the item's `i` field).
        item: u32,
        /// Number of black stones.
        black: usize,
        /// Number of white stones.
        white: usize,
        /// Side to move according to the file.
        attacker: Color,
    },

    /// A line side marker is not 1 (Black) or 2 (White).
    #[error("puzzle {item}: invalid line side marker {value}")]
    InvalidSide {
        /// Puzzle index within the file (the item's `i` field).
        item: u32,
        /// Raw side value from the file.
        value: u8,
    },

    /// The first move of the claimed line is not by the attacker.
    #[error("puzzle {item}: line starts with side {found}, expected {expected:?}")]
    InvalidSolutionSide {
        /// Puzzle index within the file (the item's `i` field).
        item: u32,
        /// Expected side (the attacker).
        expected: Color,
        /// Side value found in the file.
        found: u8,
    },

    /// A requested puzzle index is not in the pack.
    #[error("puzzle index {index} out of range (pack holds {count})")]
    IndexOutOfRange {
        /// Requested zero-based index.
        index: usize,
        /// Number of puzzles in the pack.
        count: usize,
    },
}

// ------------------------------------------------------------------ pack file schema

#[derive(Debug, Deserialize)]
struct PackFile {
    /// Format version, currently always 1.
    #[allow(dead_code)]
    v: u32,
    count: usize,
    items: Vec<PackItem>,
}

#[derive(Debug, Deserialize)]
struct PackItem {
    /// The file's own puzzle number; used in error messages.
    i: u32,
    a: String,
    /// Claimed line depth, if the format provides one.
    #[allow(dead_code)]
    d: Option<u32>,
    #[serde(rename = "b")]
    black: Vec<[u8; 2]>,
    #[serde(rename = "w")]
    white: Vec<[u8; 2]>,
    /// Claimed forced-win line; absent in unlabelled packs.
    #[serde(rename = "l", default)]
    line: Vec<[u8; 3]>,
}

fn parse_item(pos: usize, item: PackItem) -> Result<Puzzle, PuzzleError> {
    let attacker = match item.a.as_str() {
        "black" => Color::Black,
        "white" => Color::White,
        other => {
            return Err(PuzzleError::InvalidAttacker {
                item: item.i,
                value: other.to_string(),
            });
        }
    };

    let black = parse_stones(item.i, &item.black)?;
    let white = parse_stones(item.i, &item.white)?;

    // No duplicates within a color and no overlap between colors. The
    // bitboard builder would silently collapse duplicates, so check here.
    let mut seen = std::collections::HashSet::new();
    for mv in &black {
        if !seen.insert(mv.index()) {
            return Err(PuzzleError::DuplicateStone {
                item: item.i,
                x: mv.col(),
                y: mv.row(),
            });
        }
    }
    for mv in &white {
        if seen.contains(&mv.index()) {
            return Err(PuzzleError::OverlappingColors { item: item.i });
        }
        if !seen.insert(mv.index()) {
            return Err(PuzzleError::DuplicateStone {
                item: item.i,
                x: mv.col(),
                y: mv.row(),
            });
        }
    }

    let board = Board::from_position(&black, &white, attacker).map_err(|e| match e {
        PositionError::OverlappingColors => PuzzleError::OverlappingColors { item: item.i },
        PositionError::InvalidCounts => PuzzleError::InvalidCounts {
            item: item.i,
            black: black.len(),
            white: white.len(),
            attacker,
        },
    })?;

    let mut solution = Vec::with_capacity(item.line.len());
    for (ply, &[x, y, side]) in item.line.iter().enumerate() {
        let color = match side {
            1 => Color::Black,
            2 => Color::White,
            other => {
                return Err(PuzzleError::InvalidSide {
                    item: item.i,
                    value: other,
                });
            }
        };
        if ply == 0 && color != attacker {
            return Err(PuzzleError::InvalidSolutionSide {
                item: item.i,
                expected: attacker,
                found: side,
            });
        }
        let mv = Move::new(y, x).ok_or(PuzzleError::InvalidCoord { item: item.i, x, y })?;
        solution.push((mv, color));
    }

    Ok(Puzzle {
        index: pos,
        attacker,
        board,
        solution,
    })
}

fn parse_stones(item: u32, coords: &[[u8; 2]]) -> Result<Vec<Move>, PuzzleError> {
    coords
        .iter()
        .map(|&[x, y]| Move::new(y, x).ok_or(PuzzleError::InvalidCoord { item, x, y }))
        .collect()
}
```

## `gomoku/crates/cli/src/command.rs`

**New in this chapter:** the puzzle vocabulary — commands `Hint`,
`Reveal`, `NextPuzzle`, `PrevPuzzle`; events `SolutionRevealed`,
`PuzzleLoaded`; errors `NoPuzzleLoaded`, `NoSolutionLine`,
`AtLastPuzzle`, `AtFirstPuzzle`; plus their parser arms. The rest is
unchanged from chapter 03.

```rust
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
    /// Show the first move of the loaded puzzle's claimed line.
    Hint,
    /// Show the full claimed line of the loaded puzzle.
    Reveal,
    /// Load the next puzzle in the pack as a fresh tree root.
    ///
    /// Navigation clamps: at the last puzzle this is an error, not a
    /// wrap-around.
    NextPuzzle,
    /// Load the previous puzzle in the pack as a fresh tree root.
    ///
    /// Navigation clamps: at the first puzzle this is an error, not a
    /// wrap-around.
    PrevPuzzle,
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
    /// The first move of the loaded puzzle's claimed line.
    Hint {
        /// First move of the claimed line.
        mv: Move,
    },
    /// The full claimed line of the loaded puzzle.
    SolutionRevealed {
        /// Claimed line: move and side for each ply.
        line: Vec<(Move, Color)>,
    },
    /// A puzzle was loaded as a fresh variation tree root.
    PuzzleLoaded {
        /// Zero-based index within the pack.
        index: usize,
        /// Number of puzzles in the pack.
        count: usize,
        /// Side to move (the attacker).
        attacker: Color,
    },
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
            Event::Hint { mv } => write!(f, "hint: the line starts at {}", fmt_move(*mv)),
            Event::SolutionRevealed { line } => {
                let text = line
                    .iter()
                    .map(|(mv, side)| format!("{} {side:?}", fmt_move(*mv)))
                    .collect::<Vec<_>>()
                    .join(" → ");
                write!(f, "solution: {text}")
            }
            Event::PuzzleLoaded {
                index,
                count,
                attacker,
            } => {
                // Display is 1-based; the event fields stay 0-based for
                // machine consumers.
                write!(f, "puzzle {} of {count} · {attacker:?} to move", index + 1)
            }
        }
    }
}

fn fmt_move(mv: Move) -> String {
    crate::fmt::format_move(mv)
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
    /// `Step` was issued but the side-to-move seat is occupied by a human.
    #[error("Step requires an AI seat to move; {seat:?} is human")]
    StepRequiresAi {
        /// The seat that was asked to step but is human.
        seat: Color,
    },
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
    /// A puzzle command was issued but no puzzle pack is loaded.
    #[error("No puzzle loaded — start with 'gomoku play --puzzle <pack.json>'")]
    NoPuzzleLoaded,
    /// The loaded puzzle has no claimed solution line.
    #[error("Puzzle {} of {count} has no claimed line", index + 1)]
    NoSolutionLine {
        /// Zero-based index of the loaded puzzle.
        index: usize,
        /// Number of puzzles in the pack.
        count: usize,
    },
    /// `next` at the last puzzle — navigation clamps.
    #[error("Already at the last puzzle ({count} of {count})")]
    AtLastPuzzle {
        /// Number of puzzles in the pack.
        count: usize,
    },
    /// `prev` at the first puzzle — navigation clamps.
    #[error("Already at the first puzzle (1 of {count})")]
    AtFirstPuzzle {
        /// Number of puzzles in the pack.
        count: usize,
    },
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
/// * `h8` or `play h8` → `Play`
/// * `step`
/// * `back [n]` (default 1)
/// * `forward [n]` (default 1)
/// * `var N`
/// * `take black|white|both`
/// * `release black|white|both`
/// * `sims black|white N`
/// * `new`
/// * `hint`
/// * `reveal`
/// * `next` / `prev` (pack navigation, clamps at the ends)
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
        "hint" => {
            ensure_no_more(&mut tokens, "hint")?;
            Ok(Command::Hint)
        }
        "reveal" => {
            ensure_no_more(&mut tokens, "reveal")?;
            Ok(Command::Reveal)
        }
        "next" => {
            ensure_no_more(&mut tokens, "next")?;
            Ok(Command::NextPuzzle)
        }
        "prev" => {
            ensure_no_more(&mut tokens, "prev")?;
            Ok(Command::PrevPuzzle)
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
            let token = parse_required_token(&mut tokens, "square")?;
            let (row, col) = parse_algebraic(&token).map_err(CommandError::Parse)?;
            ensure_no_more(&mut tokens, "play")?;
            Ok(Command::Play(Move::new(row, col).ok_or_else(|| {
                CommandError::Parse(format!("move out of bounds: {}", token))
            })?))
        }
        _ => {
            // Try parsing a single algebraic token like `h8`.
            let (row, col) = parse_algebraic(first).map_err(CommandError::Parse)?;
            ensure_no_more(&mut tokens, "play")?;
            Ok(Command::Play(Move::new(row, col).ok_or_else(|| {
                CommandError::Parse(format!("move out of bounds: {}", first))
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

fn parse_algebraic(token: &str) -> Result<(u8, u8), String> {
    let text = token.trim().to_ascii_lowercase();
    if text.len() < 2 {
        return Err(format!(
            "expected algebraic square like 'h8', got '{}'",
            token
        ));
    }
    let col_letter = text.chars().next().unwrap();
    if !('a'..='o').contains(&col_letter) {
        return Err(format!(
            "expected column letter a-o, got '{}' in '{}'",
            col_letter, token
        ));
    }
    let row_digits = &text[1..];
    let row = row_digits.parse::<u8>().map_err(|_| {
        format!(
            "expected row number 1-15, got '{}' in '{}'",
            row_digits, token
        )
    })?;
    let col = col_letter as u8 - b'a';
    if !(1..=15).contains(&row) || col > 14 {
        return Err(format!("square '{}' is off the board", token));
    }
    Ok((row - 1, col))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    #[test]
    fn parse_play_bare_numbers() {
        assert_eq!(parse("h8"), Ok(Command::Play(mv(7, 7))));
        assert_eq!(parse("o1"), Ok(Command::Play(mv(0, 14))));
        assert_eq!(parse("a15"), Ok(Command::Play(mv(14, 0))));
    }

    #[test]
    fn parse_play_explicit() {
        assert_eq!(parse("play h8"), Ok(Command::Play(mv(7, 7))));
        assert_eq!(parse("PLAY O1"), Ok(Command::Play(mv(0, 14))));
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
    fn parse_puzzle_commands() {
        assert_eq!(parse("hint"), Ok(Command::Hint));
        assert_eq!(parse("REVEAL"), Ok(Command::Reveal));
        assert_eq!(parse("next"), Ok(Command::NextPuzzle));
        assert_eq!(parse("prev"), Ok(Command::PrevPuzzle));
        assert!(parse("hint x").is_err());
        assert!(parse("next 2").is_err());
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
        assert!(parse("a").is_err());
        assert!(parse("h8h").is_err());
        assert!(parse("play a").is_err());
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
        assert!(parse("h16").is_err());
        assert!(parse("p8").is_err());
    }
}
```

## `gomoku/crates/cli/src/session.rs`

**New in this chapter:** `PuzzleStudy`, the `SessionInit::puzzle`
constructor, `SessionCore::puzzle()`, and the `execute_hint` /
`execute_reveal` / `execute_puzzle_nav` handlers. The chapter-04 core
is unchanged.

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
        })
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
            puzzle: init.puzzle.clone(),
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
        self.history = History::from_opening(&self.init.opening);
        self.black = self.init.black.clone();
        self.white = self.init.white.clone();
        self.default_config = self.init.default_config.clone();
        self.black_sims = self.init.black_sims;
        self.white_sims = self.init.white_sims;
        self.puzzle = self.init.puzzle.clone();
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

**New in this chapter:** `PlayArgs` gains `--puzzle` / `--index`, and
`build_puzzle_init` constructs the puzzle `SessionInit`. The chapter-05
front-end is otherwise unchanged; only the changed items are quoted.

```rust
use clap::{Args, Parser, Subcommand, ValueEnum};
use engine::{Board, Color};
use std::path::{Path, PathBuf};

use cli::actor::{AiConfig, EvaluatorKind, Occupant};
use cli::puzzle::PuzzlePack;
use cli::session::{PuzzleStudy, SessionInit};
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

**New in this chapter:** `parse_flag_value`, `--puzzle` / `--index`
handling in `build_init`, and `#`-comment lines skipped in scripts.
The rest is unchanged from chapter 04.

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

**New in this chapter:** one side-panel line under `Path:` when a pack
is loaded (`Puzzle: <i+1> of <count> · <attacker> to move`). The rest
is unchanged from chapter 06.

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

## `gomoku/crates/cli/src/tui/study.rs`

**New in this chapter:** the puzzle keys in the help text and input
handling. The rest is unchanged from chapter 06.

```rust
//! The study-table screen — the real table (chapter 05).
//!
//! Owns a `SessionCore`, reads commands, executes them, and repaints an
//! event-driven board with a side panel. Falls back to line-mode output
//! when stdout is not a terminal so the session can be scripted.

use std::io::{self, IsTerminal, Stdout, Write};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::Print,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use engine::{Board, Color, Move, Status};

use crate::command::{Command, Event};
use crate::session::{SessionCore, SessionInit};
use crate::tui::opening::OpeningState;
use crate::tui::overlay::PolicyOverlay;
use crate::tui::render::{overlay_verdict, render_study_with_overlays};

const HELP: &str = r#"Commands:
  SQUARE or play SQUARE     play a move
  step                      let the AI play
  back [N]                  go back N plies
  forward [N]               go forward N plies
  var N                     choose child variation N
  take black|white|both     make seat(s) human
  release black|white|both  release seat(s) to AI
  sims black|white N        set per-seat simulations
  p                         cycle policy overlay (off→visits→prior)
  v                         toggle value strip
  t                         TSS forced-win overlay
  hint                      show the first move of the puzzle's line
  reveal                    show the puzzle's full claimed line
  next / prev               load the next/previous puzzle (clamps at the ends)
  new                       reset to opening
  h, help, ?                show this help
  q, quit, exit             leave
"#;

/// Run the study table starting from `init`.
///
/// `hint` is shown on the first frame (e.g. "Enter = step" for self-play).
pub fn run(init: SessionInit, hint: Option<&str>) -> io::Result<()> {
    if io::stdout().is_terminal() {
        run_alt(init, hint)
    } else {
        run_line(init, hint)
    }
}

/// Run the study table after the Swap2 opening ceremony.
///
/// The opening is driven interactively; the resulting board is passed to
/// `build_init`, which constructs the `SessionInit` for the study table.
pub fn run_with_swap2(
    build_init: impl FnOnce(Board) -> SessionInit,
    hint: Option<&str>,
) -> io::Result<()> {
    let board = run_swap2()?;
    run(build_init(board), hint)
}

// ------------------------------------------------------------------ alternate-screen loop

struct AlternateScreen;

impl AlternateScreen {
    fn enter() -> io::Result<AlternateScreen> {
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(AlternateScreen)
    }
}

impl Drop for AlternateScreen {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
    }
}

fn run_alt(init: SessionInit, hint: Option<&str>) -> io::Result<()> {
    let _screen = AlternateScreen::enter()?;
    let mut stdout = io::stdout();
    let mut core = SessionCore::new(init);
    let mut events: Vec<Event> = Vec::new();
    let mut message = hint
        .map(|h| h.to_string())
        .unwrap_or_else(|| "Study table — type 'h' for help".to_string());
    let mut shell = ShellState::default();

    loop {
        draw(&mut stdout, &core, &events, &message, &shell)?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break; // EOF
        }

        let trimmed = input.trim();
        if trimmed.is_empty() {
            if let Some(cmd) = empty_line_command(&core) {
                message = "thinking...".to_string();
                draw(&mut stdout, &core, &events, &message, &shell)?;
                stdout.flush()?;
                update(&mut core, cmd, &mut events, &mut message);
            }
            continue;
        }

        match parse_study_input(trimmed) {
            Parsed::Quit => break,
            Parsed::Help => message = HELP.to_string(),
            Parsed::Policy => {
                shell.policy_overlay = shell.policy_overlay.cycle();
                shell.tss_proof = None;
                if shell.policy_overlay != PolicyOverlay::Off && core.search_info().is_none() {
                    message = "no search here yet".to_string();
                } else {
                    message = format!("policy overlay: {}", shell.policy_overlay.label());
                }
            }
            Parsed::Value => {
                shell.show_value_strip = !shell.show_value_strip;
                message = if shell.show_value_strip {
                    "value strip on"
                } else {
                    "value strip off"
                }
                .to_string();
            }
            Parsed::Tss => message = run_tss(&core, &mut shell),
            Parsed::Command(cmd) => {
                update(&mut core, cmd, &mut events, &mut message);
            }
            Parsed::Error(err) => message = err,
        }
    }

    Ok(())
}

fn draw(
    stdout: &mut Stdout,
    core: &SessionCore,
    events: &[Event],
    message: &str,
    shell: &ShellState,
) -> io::Result<()> {
    queue!(stdout, MoveTo(0, 0), Hide, Clear(ClearType::All))?;
    for line in render_study_with_overlays(
        core,
        events,
        true,
        shell.policy_overlay,
        shell.tss_proof.as_ref(),
        shell.show_value_strip,
    ) {
        queue!(stdout, Print(line), Print("\n"))?;
    }
    queue!(stdout, MoveTo(0, 19), Print(status_line(core)))?;
    queue!(stdout, MoveTo(0, 20), Print(message))?;
    queue!(stdout, MoveTo(0, 21), Print("> "), Show)?;
    stdout.flush()
}

// ------------------------------------------------------------------ line-mode fallback

fn run_line(init: SessionInit, hint: Option<&str>) -> io::Result<()> {
    let mut core = SessionCore::new(init);
    let mut events: Vec<Event> = Vec::new();
    let mut message = hint
        .map(|h| h.to_string())
        .unwrap_or_else(|| "Study table — type 'h' for help".to_string());
    let mut shell = ShellState::default();
    println!("{message}");

    loop {
        for line in render_study_with_overlays(
            &core,
            &events,
            false,
            shell.policy_overlay,
            shell.tss_proof.as_ref(),
            shell.show_value_strip,
        ) {
            println!("{line}");
        }
        println!("{}", status_line(&core));
        if !message.is_empty() {
            println!("{message}");
        }
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break; // EOF
        }

        let trimmed = input.trim();
        if trimmed.is_empty() {
            if let Some(cmd) = empty_line_command(&core) {
                println!("thinking...");
                update(&mut core, cmd, &mut events, &mut message);
            }
            continue;
        }

        match parse_study_input(trimmed) {
            Parsed::Quit => break,
            Parsed::Help => message = HELP.to_string(),
            Parsed::Policy => {
                shell.policy_overlay = shell.policy_overlay.cycle();
                shell.tss_proof = None;
                if shell.policy_overlay != PolicyOverlay::Off && core.search_info().is_none() {
                    message = "no search here yet".to_string();
                } else {
                    message = format!("policy overlay: {}", shell.policy_overlay.label());
                }
            }
            Parsed::Value => {
                shell.show_value_strip = !shell.show_value_strip;
                message = if shell.show_value_strip {
                    "value strip on"
                } else {
                    "value strip off"
                }
                .to_string();
            }
            Parsed::Tss => message = run_tss(&core, &mut shell),
            Parsed::Command(cmd) => update(&mut core, cmd, &mut events, &mut message),
            Parsed::Error(err) => message = err,
        }
    }

    Ok(())
}

// ------------------------------------------------------------------ Swap2 opening ceremony

fn parse_algebraic(token: &str) -> Result<(u8, u8), String> {
    let text = token.trim().to_ascii_lowercase();
    if text.len() < 2 {
        return Err(format!(
            "expected algebraic square like 'h8', got '{}'",
            token
        ));
    }
    let col_letter = text.chars().next().unwrap();
    if !('a'..='o').contains(&col_letter) {
        return Err(format!(
            "expected column letter a-o, got '{}' in '{}'",
            col_letter, token
        ));
    }
    let row_digits = &text[1..];
    let row = row_digits.parse::<u8>().map_err(|_| {
        format!(
            "expected row number 1-15, got '{}' in '{}'",
            row_digits, token
        )
    })?;
    let col = col_letter as u8 - b'a';
    if !(1..=15).contains(&row) || col > 14 {
        return Err(format!("{} is off the board — squares are a1-o15", token));
    }
    Ok((row - 1, col))
}

fn run_swap2() -> io::Result<Board> {
    if io::stdout().is_terminal() {
        run_swap2_alt()
    } else {
        run_swap2_line()
    }
}

fn run_swap2_alt() -> io::Result<Board> {
    let _screen = AlternateScreen::enter()?;
    let mut stdout = io::stdout();
    let mut opening = OpeningState::new();
    let mut message = "Swap2 opening — Player A places two Black and one White stone".to_string();

    loop {
        queue!(stdout, MoveTo(0, 0), Hide, Clear(ClearType::All))?;
        for line in crate::tui::render::render_opening_styled(&opening) {
            queue!(stdout, Print(line), Print("\n"))?;
        }
        queue!(stdout, MoveTo(0, 19), Print(opening_status(&opening)))?;
        queue!(stdout, MoveTo(0, 20), Print(&message))?;
        queue!(stdout, MoveTo(0, 21), Print("> "), Show)?;
        stdout.flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }

        match dispatch_opening(&mut opening, input.trim()) {
            OpeningCmd::Continue(msg) => message = msg,
            OpeningCmd::Finish(board) => return Ok(board),
            OpeningCmd::Quit => return Err(io::Error::other("Swap2 opening aborted")),
        }
    }

    Err(io::Error::other("Swap2 opening aborted"))
}

fn run_swap2_line() -> io::Result<Board> {
    let mut opening = OpeningState::new();
    println!("Swap2 opening — Player A places two Black and one White stone");

    loop {
        for line in crate::tui::render::render_opening_lines(&opening) {
            println!("{line}");
        }
        println!("{}", opening_status(&opening));
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }

        match dispatch_opening(&mut opening, input.trim()) {
            OpeningCmd::Continue(msg) => println!("{msg}"),
            OpeningCmd::Finish(board) => return Ok(board),
            OpeningCmd::Quit => return Err(io::Error::other("Swap2 opening aborted")),
        }
    }

    Err(io::Error::other("Swap2 opening aborted"))
}

enum OpeningCmd {
    Continue(String),
    Finish(Board),
    Quit,
}

fn dispatch_opening(opening: &mut OpeningState, input: &str) -> OpeningCmd {
    let text = input.trim().to_ascii_lowercase();
    match text.as_str() {
        "q" | "quit" | "exit" => OpeningCmd::Quit,
        "u" | "undo" => {
            if opening.is_at_start() {
                OpeningCmd::Continue("nothing to undo".to_string())
            } else {
                opening.undo();
                OpeningCmd::Continue(opening_status(opening))
            }
        }
        "d" | "done" => match opening.finish() {
            Ok(()) => OpeningCmd::Continue(opening_status(opening)),
            Err(e) => OpeningCmd::Continue(e),
        },
        "b" | "black" => match opening.clone().choose(Color::Black) {
            Some(outcome) => OpeningCmd::Finish(outcome.board),
            None => OpeningCmd::Continue("choose a color when prompted: 'b' or 'w'".to_string()),
        },
        "w" | "white" => match opening.clone().choose(Color::White) {
            Some(outcome) => OpeningCmd::Finish(outcome.board),
            None => OpeningCmd::Continue("choose a color when prompted: 'b' or 'w'".to_string()),
        },
        "a" | "add" => {
            let next = opening.clone().add_two_more();
            match next {
                Some(state) => {
                    *opening = state;
                    OpeningCmd::Continue(format!(
                        "{} adds two more stones — place one Black, then one White",
                        opening.active_party().label()
                    ))
                }
                None => OpeningCmd::Continue(
                    "'a' is only available when choosing after the first three stones".to_string(),
                ),
            }
        }
        _ => {
            let parts: Vec<&str> = text.split([',', ' ']).filter(|s| !s.is_empty()).collect();
            if parts.len() != 2 {
                return OpeningCmd::Continue(
                    "enter <square> to place · 'd' done · 'b'/'w' choose · 'a' add two · 'u' undo · 'q' quit".to_string(),
                );
            }
            let token = parts[0];
            let (row, col) = match parse_algebraic(token) {
                Ok(v) => v,
                Err(e) => return OpeningCmd::Continue(e),
            };
            let mv = match Move::new(row, col) {
                Some(m) => m,
                None => {
                    return OpeningCmd::Continue(format!(
                        "{} is off the board — squares are a1-o15",
                        token
                    ));
                }
            };
            if !opening.can_place() {
                return OpeningCmd::Continue(
                    "all stones for this phase are placed — type 'd' to continue or 'u' to undo"
                        .to_string(),
                );
            }
            match opening.place(mv) {
                Ok(placed) => OpeningCmd::Continue(format!(
                    "{} placed {placed:?} at ({}, {})",
                    opening.active_party().label(),
                    mv.row(),
                    mv.col()
                )),
                Err(e) => OpeningCmd::Continue(e),
            }
        }
    }
}

fn opening_status(opening: &OpeningState) -> String {
    use crate::tui::opening::OpeningPhase;
    let party = opening.active_party().label();
    match opening.phase() {
        OpeningPhase::Placing3 => {
            if opening.placements().len() == 3 {
                format!("{party}: 3 stones placed — [d] done · [u] undo")
            } else {
                let color = opening.current_color().unwrap();
                let remaining = opening.remaining(color);
                format!("{party}: place {color:?} ({remaining} remain) — square · [u] undo")
            }
        }
        OpeningPhase::FirstChoice => {
            format!("{party}: [b] take Black · [w] take White · [a] add two stones · [u] undo")
        }
        OpeningPhase::Placing2 => {
            if opening.placements().len() == 5 {
                format!("{party}: 5 stones placed — [d] done · [u] undo")
            } else {
                let color = opening.current_color().unwrap();
                let remaining = opening.remaining(color);
                format!("{party}: place {color:?} ({remaining} remain) — square · [u] undo")
            }
        }
        OpeningPhase::FinalChoice => {
            format!("{party}: [b] take Black · [w] take White · [u] undo")
        }
    }
}

// ------------------------------------------------------------------ shared helpers

#[derive(Debug, Default)]
struct ShellState {
    policy_overlay: PolicyOverlay,
    tss_proof: Option<engine::Proof>,
    show_value_strip: bool,
}

enum Parsed {
    Quit,
    Help,
    Command(Command),
    Policy,
    Value,
    Tss,
    Error(String),
}

fn parse_study_input(input: &str) -> Parsed {
    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("p") {
        return Parsed::Policy;
    }
    if trimmed.eq_ignore_ascii_case("v") {
        return Parsed::Value;
    }
    if trimmed.eq_ignore_ascii_case("t") {
        return Parsed::Tss;
    }
    match trimmed.eq_ignore_ascii_case("h")
        || trimmed.eq_ignore_ascii_case("help")
        || trimmed == "?"
    {
        true => Parsed::Help,
        false => match crate::command::parse(trimmed) {
            Ok(Command::Help) => Parsed::Help,
            Ok(Command::Quit) => Parsed::Quit,
            Ok(cmd) => Parsed::Command(cmd),
            Err(e) => Parsed::Error(e.to_string()),
        },
    }
}

fn run_tss(core: &SessionCore, shell: &mut ShellState) -> String {
    if shell.tss_proof.is_some() {
        shell.tss_proof = None;
        return "TSS overlay hidden".to_string();
    }

    let board = core.board();
    if board.status() != Status::Ongoing {
        return "TSS demo is only available during ongoing games".to_string();
    }

    let budget = engine::SearchBudget {
        max_nodes: 100_000,
        max_depth: 9,
    };
    let side = board.to_move();
    let Some(proof) = engine::prove_forced_win(&board, side, budget) else {
        return "no forced win proven within budget".to_string();
    };

    if !engine::verify_line(&board, &proof) {
        return "internal error: unverifiable proof".to_string();
    }

    let verdict = overlay_verdict(&proof);
    shell.tss_proof = Some(proof);
    shell.policy_overlay = PolicyOverlay::Off;
    verdict
}

fn empty_line_command(core: &SessionCore) -> Option<Command> {
    if core.board().status() != Status::Ongoing {
        return None;
    }
    let side = core.board().to_move();
    if core.occupant(side).is_ai() {
        Some(Command::Step)
    } else {
        None
    }
}

fn update(core: &mut SessionCore, cmd: Command, events: &mut Vec<Event>, message: &mut String) {
    match core.execute(cmd) {
        Ok(new_events) => {
            events.extend(new_events.iter().cloned());
            if events.len() > 20 {
                events.drain(0..events.len() - 20);
            }
            *message = new_events.last().map(|e| e.to_string()).unwrap_or_default();
        }
        Err(e) => *message = e.to_string(),
    }
}

fn status_line(core: &SessionCore) -> String {
    let snap = core.snapshot();
    match snap.status {
        Status::Ongoing => format!(
            "ply {} · {:?} to move · status ongoing",
            snap.ply,
            core.board().to_move()
        ),
        other => format!("ply {} · {:?}", snap.ply, other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionInit;

    #[test]
    fn empty_line_step_only_when_ai_to_move() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        assert!(empty_line_command(&core).is_none());

        let ai_init = SessionInit {
            black: crate::actor::Occupant::Ai(crate::actor::AiConfig::default()),
            ..SessionInit::new_hotseat()
        };
        let core = SessionCore::new(ai_init);
        assert!(matches!(empty_line_command(&core), Some(Command::Step)));
    }
}
```

## `gomoku/crates/cli/src/lib.rs`

**New in this chapter:** one line — `pub mod puzzle;`.

```rust
//! Terminal Gomoku client library.
//!
//! The `cli` crate is split into a headless library (history, command,
//! session, actor, puzzle) and a thin binary front-end. The `tui` module holds
//! the existing terminal UI pieces moved here unchanged.

#![deny(missing_docs)]

pub mod actor;
pub mod command;
pub mod fmt;
pub mod history;
pub mod puzzle;
pub mod session;
pub mod tui;
```

## `gomoku/crates/cli/Cargo.toml`

**New in this chapter:** the `serde_json` dependency. The rest is
unchanged from chapter 03.

```toml
[package]
name = "cli"
description = "Terminal frontend: play Gomoku on either engine board"
version.workspace = true
edition.workspace = true
license.workspace = true

[lib]
name = "cli"
path = "src/lib.rs"

[[bin]]
name = "gomoku"
path = "src/main.rs"

[[bin]]
name = "historydemo"
path = "src/bin/historydemo.rs"

[[bin]]
name = "sessiondemo"
path = "src/bin/sessiondemo.rs"

[dependencies]
crossterm.workspace = true
engine = { path = "../engine" }
mcts = { path = "../mcts" }
rand = "0.10.3"
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
thiserror.workspace = true
clap = { workspace = true, features = ["derive"] }

[dev-dependencies]
engine = { path = "../engine", features = ["testutil"] }
proptest.workspace = true
```

## `gomoku/crates/cli/tests/puzzle.rs`

**New in this chapter:** the whole file — the pack parser tests from
the TDD checklist.

```rust
//! Parser tests for puzzle packs (spec amendment "positions from files").
//!
//! The valid-pack tests load the committed mini fixture; malformed inputs
//! are inline JSON strings. Tests never touch the git-ignored corpora
//! under `data/`.

use cli::puzzle::{PuzzleError, PuzzlePack};
use engine::{Color, Move};

const MINI_PACK: &str = include_str!("fixtures/mini-pack.json");

fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}

#[test]
fn parses_valid_pack() {
    let pack = PuzzlePack::from_json(MINI_PACK).expect("mini fixture parses");
    assert_eq!(pack.items.len(), 3);

    let first = &pack.items[0];
    assert_eq!(first.index, 0);
    assert_eq!(first.attacker, Color::Black);
    assert_eq!(first.board.to_move(), Color::Black);
    assert_eq!(first.solution, vec![(mv(7, 7), Color::Black)]);

    let third = &pack.items[2];
    assert_eq!(third.index, 2);
    assert_eq!(third.attacker, Color::Black);
    assert_eq!(
        third.solution,
        vec![
            (mv(7, 10), Color::Black),
            (mv(7, 6), Color::White),
            (mv(7, 11), Color::Black),
        ]
    );
}

#[test]
fn file_coordinates_are_x_column_y_row() {
    // Legacy viewer convention: file [x, y] maps to Move::new(y, x) —
    // x is the column, y is the row.
    let pack = PuzzlePack::from_json(MINI_PACK).unwrap();
    let first = &pack.items[0];
    // File stone [7, 3]: column 7 ("h"), row 3.
    assert_eq!(first.board.stone_at(mv(3, 7)), Some(Color::Black));
    assert_eq!(first.board.stone_at(mv(7, 3)), None);
    // File line head [7, 7] renders as algebraic h8.
    assert_eq!(cli::fmt::format_move(first.solution[0].0), "h8");
}

#[test]
fn board_is_replayable_position_with_attacker_to_move() {
    let pack = PuzzlePack::from_json(MINI_PACK).unwrap();
    let white_puzzle = &pack.items[1];
    assert_eq!(white_puzzle.attacker, Color::White);
    assert_eq!(white_puzzle.board.to_move(), Color::White);
    assert_eq!(white_puzzle.board.moves().len(), 0);
}

#[test]
fn malformed_json_is_rejected() {
    let err = PuzzlePack::from_json("{ not json").unwrap_err();
    assert!(matches!(err, PuzzleError::Json(_)));
}

#[test]
fn count_mismatch_is_rejected() {
    let text = r#"{ "v": 1, "count": 2, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::CountMismatch {
            declared: 2,
            actual: 1
        }
    ));
}

#[test]
fn off_board_coordinate_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [[15, 3]], "w": [], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::InvalidCoord {
            item: 0,
            x: 15,
            y: 3
        }
    ));
}

#[test]
fn off_board_line_coordinate_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [], "l": [[7, 15, 1]] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::InvalidCoord {
            item: 0,
            x: 7,
            y: 15
        }
    ));
}

#[test]
fn duplicate_stone_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [[7, 3], [7, 3]], "w": [], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::DuplicateStone {
            item: 0,
            x: 7,
            y: 3
        }
    ));
}

#[test]
fn overlapping_colors_are_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [[7, 3]], "w": [[7, 3]], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(err, PuzzleError::OverlappingColors { item: 0 }));
}

#[test]
fn counts_disagreeing_with_attacker_are_rejected() {
    // Black to move requires equal counts; here Black has one more.
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [[7, 3], [8, 3]], "w": [[0, 0]], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::InvalidCounts {
            item: 0,
            black: 2,
            white: 1,
            attacker: Color::Black,
        }
    ));
}

#[test]
fn unknown_attacker_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "red", "d": 1, "b": [], "w": [], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(err, PuzzleError::InvalidAttacker { item: 0, .. }));
}

#[test]
fn line_starting_with_the_wrong_side_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [], "l": [[7, 7, 2]] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::InvalidSolutionSide {
            item: 0,
            expected: Color::Black,
            found: 2,
        }
    ));
}

#[test]
fn unknown_line_side_marker_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [], "l": [[7, 7, 1], [8, 7, 9]] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::InvalidSide { item: 0, value: 9 }
    ));
}

#[test]
fn missing_line_defaults_to_empty_solution() {
    // Unlabelled packs (positions only) carry no `l` field.
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [] }
    ] }"#;
    let pack = PuzzlePack::from_json(text).unwrap();
    assert!(pack.items[0].solution.is_empty());
}

#[test]
fn index_out_of_range_is_rejected() {
    let pack = PuzzlePack::from_json(MINI_PACK).unwrap();
    assert!(pack.get(0).is_ok());
    assert!(pack.get(2).is_ok());
    let err = pack.get(3).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::IndexOutOfRange { index: 3, count: 3 }
    ));
}
```

## `gomoku/crates/cli/tests/session.rs`

**New in this chapter:** ten puzzle-session tests appended to the
chapter-02/03 suite. The earlier tests are unchanged.

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
```

## `gomoku/crates/cli/tests/fixtures/mini-pack.json`

**New in this chapter:** the whole file — a minimal pack fixture for
the tests.

```json
{
  "v": 1,
  "count": 3,
  "items": [
    {
      "i": 0,
      "a": "black",
      "d": 1,
      "u": 0,
      "f": 0,
      "b": [[7, 3], [7, 4], [7, 5], [7, 6]],
      "w": [[0, 0], [0, 1], [0, 2], [7, 2]],
      "l": [[7, 7, 1]]
    },
    {
      "i": 1,
      "a": "white",
      "d": 1,
      "u": 0,
      "f": 0,
      "b": [[0, 0], [1, 0], [2, 0], [3, 0], [14, 14]],
      "w": [[4, 4], [5, 4], [6, 4], [7, 4]],
      "l": [[8, 4, 2]]
    },
    {
      "i": 2,
      "a": "black",
      "d": 3,
      "u": 0,
      "f": 0,
      "b": [[7, 7], [8, 7], [9, 7]],
      "w": [[5, 5], [6, 5], [7, 5]],
      "l": [[10, 7, 1], [6, 7, 2], [11, 7, 1]]
    }
  ]
}
```

## `gomoku/crates/cli/puzzle_demo.txt`

**New in this chapter:** the whole file — the capstone script.

```text
# puzzle_demo.txt — studying a puzzle pack at the table.
# Run from gomoku/:
#   cargo run -p cli --bin sessiondemo -- --puzzle crates/cli/tests/fixtures/mini-pack.json --script crates/cli/puzzle_demo.txt
# Puzzle 1 of 3: Black to move, vertical four on column h. Where does the line start?
hint
# A wrong try: i9 is legal but is not the claimed move.
i9
back
# Give up and show the claimed line.
reveal
# Move on to the next puzzle in the pack (clamps at the ends).
next
hint
```

