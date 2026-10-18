> **Opt-in solution — read only after you have tried the chapter, or if
> you have been stuck for more than twenty minutes.**
>
> This file quotes the verified reference implementation for both
> slices of Chapter 01. The files come from the disposable worktree
> `/Users/wgiersche/workspace/Project-Ellie/rust-ml-ref-playable-cli`
> (branch `reference/playable-cli`): slice 0 at commit `c7da12b`,
> slice 1 at commit `ad1aa26`.

# Slice 0 — `refactor(cli): lib/bin split + clap subcommands`

## `gomoku/crates/cli/Cargo.toml`

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

[features]
default = ["naive-engine"]
naive-engine = ["engine/testutil"]

[dependencies]
crossterm.workspace = true
engine = { path = "../engine" }
serde = { workspace = true, features = ["derive"] }
serde_json.workspace = true
thiserror.workspace = true
clap = { workspace = true, features = ["derive"] }
```

## `gomoku/Cargo.toml` (workspace dependency line in context)

```toml
# Gomoku workspace — the AlphaZero-style agent (docs/12-gomoku-architecture.md).
# Independent of the rust-ml root package; Burn version is managed workspace-wide.

[workspace]
resolver = "3"
members = ["crates/engine", "crates/cli", "crates/mcts", "crates/train", "crates/patterns"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "Apache-2.0"

[workspace.dependencies]
burn = { version = "=0.22.0", default-features = false }
crossterm = "0.29"
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
proptest = "1"
criterion = "0.8.2"
clap = { version = "4", features = ["derive"] }
```

## `gomoku/crates/cli/src/lib.rs`

```rust
//! Terminal Gomoku client library.
//!
//! The `cli` crate is split into a headless library (history, command,
//! session, actor) and a thin binary front-end. The `tui` module holds
//! the existing terminal UI pieces moved here unchanged.

pub mod actor;
pub mod command;
pub mod history;
pub mod session;
pub mod tui;
```

## `gomoku/crates/cli/src/main.rs`

```rust
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use cli::tui::board::{EngineKind, board_for};
use cli::tui::puzzle;
use cli::tui::ui;

const PLAY_USAGE: &str = if cfg!(feature = "naive-engine") {
    "usage: gomoku play [--engine naive|fast] [--plain] [--no-swap2] [--puzzle <path> [--index N]]\n\nSwap2 opening is only available with the fast engine; use --no-swap2 to start from an empty board with any engine. Puzzle mode skips the opening entirely."
} else {
    "usage: gomoku play [--engine fast] [--plain] [--no-swap2] [--puzzle <path> [--index N]]\n\nThis build was compiled without the naive-engine feature; only the fast engine is available. Puzzle mode skips the opening entirely."
};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "gomoku",
    about = "Terminal Gomoku client",
    subcommand_required = false,
    arg_required_else_help = false
)]
struct CliArgs {
    /// Legacy `play` flags used when no subcommand is given.
    #[command(flatten)]
    play: PlayArgs,

    #[command(subcommand)]
    cmd: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Play a human game or puzzle (legacy default).
    Play(PlayArgs),
    /// AI-vs-AI self-play (not yet implemented).
    Selfplay(SearchArgs),
}

#[derive(Args, Debug, Clone)]
struct PlayArgs {
    #[command(flatten)]
    search: SearchArgs,

    /// Engine board to use.
    #[arg(long, value_name = "ENGINE")]
    engine: Option<String>,

    /// Use the scrolling plain-text UI.
    #[arg(long)]
    plain: bool,

    /// Skip the Swap2 opening and start from an empty board.
    #[arg(long)]
    no_swap2: bool,

    /// Load a puzzle file instead of playing a game.
    #[arg(long, value_name = "PATH")]
    puzzle: Option<PathBuf>,

    /// Puzzle index to start from.
    #[arg(long, value_name = "N")]
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

    /// Number of MCTS simulations per move.
    #[arg(long, value_name = "N", default_value_t = 400)]
    sims: u32,

    /// PUCT exploration constant.
    #[arg(long, value_name = "F", default_value_t = 1.5)]
    c_puct: f32,

    /// Move-selection temperature.
    #[arg(long, value_name = "F", default_value_t = 0.0)]
    temperature: f32,

    /// Dirichlet noise weight.
    #[arg(long, value_name = "F", default_value_t = 0.0)]
    noise: f32,

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
            // Reject legacy `play` flags placed before the subcommand.
            if args.play.engine.is_some()
                || args.play.plain
                || args.play.no_swap2
                || args.play.puzzle.is_some()
                || args.play.index.is_some()
            {
                eprintln!(
                    "error: --engine, --plain, --no-swap2, --puzzle, and --index are only valid with the 'play' subcommand"
                );
                eprintln!("{PLAY_USAGE}");
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
    let kind = parse_engine(play.engine.as_deref())?;
    let plain = play.plain;
    let swap2 = !play.no_swap2;

    if let Some(puzzle_path) = &play.puzzle {
        let index = play.index.unwrap_or(0);
        run_puzzle_mode(puzzle_path.clone(), index, plain)
    } else {
        validate_search_args(&play.search, kind, false)?;

        #[cfg(feature = "naive-engine")]
        if swap2 && kind == EngineKind::Naive {
            eprintln!("error: the Swap2 opening requires the fast engine");
            eprintln!("       use --no-swap2 to start from an empty board with the naive engine");
            std::process::exit(2);
        }

        if play.search.ai.is_some() {
            eprintln!("note: --ai is parsed but not yet wired (coming in T3+)");
        }

        let board = board_for(kind);
        if plain {
            ui::run_plain(board, kind, swap2)
        } else {
            ui::run_ui(board, kind, swap2)
        }
    }
}

fn run_selfplay(search: &SearchArgs) -> std::io::Result<()> {
    validate_search_args(search, EngineKind::Fast, true)?;
    println!("not yet implemented");
    Ok(())
}

fn run_puzzle_mode(path: PathBuf, index: usize, plain: bool) -> std::io::Result<()> {
    let puzzles = puzzle::load(&path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    if puzzles.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "puzzle file contains no puzzles",
        ));
    }
    if index >= puzzles.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("--index {index} is out of range (0..{})", puzzles.len()),
        ));
    }
    if plain {
        ui::run_plain_puzzle(puzzles, index)
    } else {
        ui::run_ui_puzzle(puzzles, index)
    }
}

fn parse_engine(value: Option<&str>) -> std::io::Result<EngineKind> {
    match value {
        None => Ok(EngineKind::Fast),
        Some("fast") => Ok(EngineKind::Fast),
        #[cfg(feature = "naive-engine")]
        Some("naive") => Ok(EngineKind::Naive),
        #[cfg(not(feature = "naive-engine"))]
        Some("naive") => {
            eprintln!("error: built without the naive-engine feature");
            eprintln!("{PLAY_USAGE}");
            std::process::exit(2);
        }
        Some(other) => {
            eprintln!("error: unknown engine '{other}'");
            eprintln!("{PLAY_USAGE}");
            std::process::exit(2);
        }
    }
}

fn validate_search_args(
    search: &SearchArgs,
    kind: EngineKind,
    is_selfplay: bool,
) -> std::io::Result<()> {
    let _ = search.ai;
    let _ = search.color;

    if kind == EngineKind::Naive && (search.ai.is_some() || is_selfplay) {
        eprintln!("error: --engine naive cannot be used with --ai or selfplay");
        eprintln!("{PLAY_USAGE}");
        std::process::exit(2);
    }

    Ok(())
}
```

## Moved TUI modules (unchanged)

The existing UI files were moved from `src/` to `src/tui/`:

* `src/tui/board.rs`
* `src/tui/input.rs`
* `src/tui/opening.rs`
* `src/tui/puzzle.rs`
* `src/tui/render.rs`
* `src/tui/ui.rs`

### `gomoku/crates/cli/src/tui/mod.rs`

```rust
//! Existing terminal UI modules.
//!
//! Moved under `tui/` during the lib/bin split; their behavior is
//! unchanged.

pub mod board;
pub mod input;
pub mod opening;
pub mod puzzle;
pub mod render;
pub mod ui;
```

## Stub core modules

`actor.rs`:

```rust
//! Seat occupants and AI configuration.
```

`command.rs`:

```rust
//! Command language and events.
```

`history.rs`:

```rust
//! Variation tree, stored as a flat node vector (the mcts::Tree idiom).
```

`session.rs`:

```rust
//! Session core — headless state machine.
```

# Slice 1 — `feat(cli): variation-tree history + historydemo`

## `gomoku/crates/cli/src/history.rs`

```rust
//! Variation tree, stored as a flat node vector (the mcts::Tree idiom).
//!
//! A persistent game history: every position reached stays reachable.
//! Playing a move at an old position adds a child branch without deleting
//! siblings. Navigation is explicit: `back`/`forward` walk the cursor,
//! `var` chooses at branch points.
//!
//! Implementation follows the in-repo arena idiom used by `mcts::Tree`:
//! one `Vec<Node>` per history, children stored as `NodeId` indices. The
//! root node stores the full opening board; every other node stores only
//! the move that led to it. Replaying `board()` recomputes the cursor
//! position from the root each call (≤225 plies).

use engine::{Board, Move, PlayError};
use thiserror::Error;

/// Node identifier: an index into [`History::nodes`].
pub type NodeId = u32;

/// One node in the variation tree.
#[derive(Debug, Clone, PartialEq)]
struct Node {
    /// Parent node, `None` only at the root.
    parent: Option<NodeId>,
    /// Move from parent to this node, `None` only at the root.
    mv: Option<Move>,
    /// Child nodes in creation order.
    children: Vec<NodeId>,
}

/// Persistent variation tree with a cursor.
#[derive(Debug, Clone, PartialEq)]
pub struct History {
    nodes: Vec<Node>,
    root: NodeId,
    cursor: NodeId,
    root_board: Board,
}

/// Result of [`History::play`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayOutcome {
    /// A new child node was appended.
    Moved(NodeId),
    /// The move matched an existing child; the cursor moved there.
    FollowedExisting(NodeId),
}

/// Result of [`History::forward`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForwardOutcome {
    /// The cursor moved this many plies.
    Moved(u32),
    /// The cursor stopped at a branch point; choose one of these moves.
    NeedsChoice(Vec<Move>),
    /// The cursor stopped at a leaf.
    AtLeaf,
}

/// Errors returned by history mutation commands.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HistoryError {
    /// The move is illegal in the cursor position.
    #[error("Illegal move: {0}")]
    IllegalMove(#[from] PlayError),
    /// The requested child index does not exist at the cursor.
    #[error("Variation index {index} out of bounds (children: {child_count})")]
    VarOutOfBounds {
        /// Index that was requested.
        index: usize,
        /// Number of children at the cursor.
        child_count: usize,
    },
}

impl History {
    /// Create a history rooted at an empty board with Black to move.
    #[must_use]
    pub fn new() -> Self {
        Self::from_opening(&Board::new())
    }

    /// Create a history rooted at an arbitrary opening position.
    ///
    /// The opening board is cloned and stored at the root; subsequent play
    /// continues from that position with the correct side to move.
    #[must_use]
    pub fn from_opening(board: &Board) -> Self {
        Self {
            nodes: vec![Node {
                parent: None,
                mv: None,
                children: Vec::new(),
            }],
            root: 0,
            cursor: 0,
            root_board: board.clone(),
        }
    }

    /// The current cursor node id.
    #[must_use]
    pub fn cursor(&self) -> NodeId {
        self.cursor
    }

    /// The board at the cursor, recomputed by replaying the root→cursor path.
    #[must_use]
    pub fn board(&self) -> Board {
        let mut board = self.root_board.clone();
        for &mv in &self.path_to_cursor() {
            // Moves are validated on insertion, so replay is safe by construction.
            board.play(mv).expect("replayed cursor path must be legal");
        }
        board
    }

    /// Play a move at the cursor.
    ///
    /// * If the move matches an existing child, the cursor moves there and
    ///   [`PlayOutcome::FollowedExisting`] is returned.
    /// * Otherwise a new child is appended and [`PlayOutcome::Moved`] is returned.
    ///
    /// # Errors
    /// Returns [`HistoryError::IllegalMove`] if the move is illegal at the
    /// cursor (occupied cell or game already over). User input must never
    /// cause a panic.
    pub fn play(&mut self, mv: Move) -> Result<PlayOutcome, HistoryError> {
        // Validate the move before touching the tree.
        let mut probe = self.root_board.clone();
        for &m in &self.path_to_cursor() {
            probe.play(m).expect("cursor path is legal");
        }
        probe.play(mv)?;

        // Follow an identical child if one exists.
        for &child_id in &self.nodes[self.cursor as usize].children {
            if self.nodes[child_id as usize].mv == Some(mv) {
                self.cursor = child_id;
                return Ok(PlayOutcome::FollowedExisting(child_id));
            }
        }

        // Append a new child node.
        let new_id = self.nodes.len() as NodeId;
        self.nodes.push(Node {
            parent: Some(self.cursor),
            mv: Some(mv),
            children: Vec::new(),
        });
        self.nodes[self.cursor as usize].children.push(new_id);
        self.cursor = new_id;
        Ok(PlayOutcome::Moved(new_id))
    }

    /// Move the cursor toward the root by up to `n` plies.
    ///
    /// Returns the number of plies actually moved (clamped at the root).
    pub fn back(&mut self, n: u32) -> u32 {
        let mut moved = 0;
        for _ in 0..n {
            match self.nodes[self.cursor as usize].parent {
                Some(parent) => {
                    self.cursor = parent;
                    moved += 1;
                }
                None => break,
            }
        }
        moved
    }

    /// Move the cursor forward by up to `n` plies.
    ///
    /// * At a leaf, returns [`ForwardOutcome::AtLeaf`].
    /// * At a branch point (multiple children), returns
    ///   [`ForwardOutcome::NeedsChoice`] with the child moves.
    /// * Along a single-child line, follows as far as possible and returns
    ///   [`ForwardOutcome::Moved`] with the plies actually moved.
    pub fn forward(&mut self, n: u32) -> ForwardOutcome {
        let mut moved = 0;
        for _ in 0..n {
            let children = &self.nodes[self.cursor as usize].children;
            match children.len() {
                0 => return ForwardOutcome::AtLeaf,
                1 => {
                    self.cursor = children[0];
                    moved += 1;
                }
                _ => {
                    return ForwardOutcome::NeedsChoice(
                        children
                            .iter()
                            .map(|&id| self.nodes[id as usize].mv.expect("child has a move"))
                            .collect(),
                    );
                }
            }
        }
        ForwardOutcome::Moved(moved)
    }

    /// Choose a child variation by index at the cursor.
    ///
    /// # Errors
    /// Returns [`HistoryError::VarOutOfBounds`] if `index` is not a valid
    /// child index.
    pub fn var(&mut self, index: usize) -> Result<Move, HistoryError> {
        let children = &self.nodes[self.cursor as usize].children;
        if index >= children.len() {
            return Err(HistoryError::VarOutOfBounds {
                index,
                child_count: children.len(),
            });
        }
        let child_id = children[index];
        self.cursor = child_id;
        Ok(self.nodes[child_id as usize].mv.expect("child has a move"))
    }

    /// The child moves of `at`, in creation order.
    #[must_use]
    pub fn children(&self, at: NodeId) -> Vec<Move> {
        self.nodes[at as usize]
            .children
            .iter()
            .map(|&id| self.nodes[id as usize].mv.expect("child has a move"))
            .collect()
    }

    /// The cursor depth in plies from the root.
    #[must_use]
    pub fn ply(&self) -> u32 {
        let mut depth = 0;
        let mut id = self.cursor;
        while let Some(parent) = self.nodes[id as usize].parent {
            depth += 1;
            id = parent;
        }
        depth
    }

    /// Collect the moves from the root to the cursor.
    fn path_to_cursor(&self) -> Vec<Move> {
        let mut path = Vec::new();
        let mut id = self.cursor;
        while let Some(parent) = self.nodes[id as usize].parent {
            path.push(
                self.nodes[id as usize]
                    .mv
                    .expect("non-root node has a move"),
            );
            id = parent;
        }
        path.reverse();
        path
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}
```

## `gomoku/crates/cli/src/bin/historydemo.rs`

````rust
//! Chapter-01 capstone: a scripted walk through the variation tree.
//!
//! Plays a short main line, branches, navigates back, chooses a variation,
//! extends it, and finally returns to the root. Prints the board, ply, and
//! variation path after each step.
//!
//! Run with:
//!
//! ```text
//! cargo run -p cli --bin historydemo
//! ```

use cli::history::{ForwardOutcome, History, PlayOutcome};
use engine::{Board, Color, Move};

fn main() {
    println!("variation-tree demo");
    println!("=================================\n");

    let mut history = History::new();
    print_step(&history, "start: empty board, root cursor");

    // Six-move main line.
    let main_line = [h8(), h7(), i8(), i7(), j8(), j7()];
    for mv in main_line {
        match history.play(mv) {
            Ok(PlayOutcome::Moved(_)) => {}
            other => panic!("unexpected play outcome: {other:?}"),
        }
    }
    print_step(&history, "main line: h8 h7 i8 i7 j8 j7");

    // Rewind three plies.
    let backed = history.back(3);
    assert_eq!(backed, 3);
    print_step(&history, "back 3");

    // Branch to a different move.
    let branch = h9();
    history.play(branch).unwrap();
    print_step(&history, "branch: h9");

    // Return to the branch point.
    history.back(1);
    print_step(&history, "back to branch point");

    // Forward reaches the branch point and asks for a choice.
    match history.forward(1) {
        ForwardOutcome::NeedsChoice(ref choices) => {
            print_step(&history, "forward: NeedsChoice");
            println!(
                "    choices: {}\n",
                choices
                    .iter()
                    .map(|&m| notation(m))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        other => panic!("expected NeedsChoice, got {other:?}"),
    }

    // Choose the second variation (h9).
    let chosen = history.var(1).unwrap();
    print_step(&history, &format!("var 1 -> {}", notation(chosen)));

    // Extend the branch.
    let extension = i9();
    history.play(extension).unwrap();
    print_step(&history, "extend: i9");

    // Back to the root.
    let to_root = history.back(100);
    print_step(&history, &format!("back {to_root} to root"));
}

fn print_step(history: &History, label: &str) {
    println!("{label}");
    println!("ply {}  path: {}", history.ply(), path_string(history));
    println!("{}", render_board(&history.board()));
    println!();
}

fn path_string(history: &History) -> String {
    let board = history.board();
    // The cursor path is the move history on a replayed board. For a root
    // built from an empty board this is exactly the path we navigated.
    if board.moves().is_empty() {
        "(root)".to_string()
    } else {
        board
            .moves()
            .iter()
            .map(|&m| notation(m))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn render_board(board: &Board) -> String {
    let mut lines = Vec::new();
    lines.push("    0  1  2  3  4  5  6  7  8  9 10 11 12 13 14".to_string());
    for r in 0..15u8 {
        let mut row = format!("{:2}  ", r);
        for c in 0..15u8 {
            let ch = match board.stone_at(Move::new(r, c).unwrap()) {
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

fn notation(mv: Move) -> String {
    let col = (b'a' + mv.col()) as char;
    let row = mv.row() + 1;
    format!("{col}{row}")
}

fn h8() -> Move {
    Move::new(7, 7).unwrap()
}
fn h7() -> Move {
    Move::new(6, 7).unwrap()
}
fn i8() -> Move {
    Move::new(7, 8).unwrap()
}
fn i7() -> Move {
    Move::new(6, 8).unwrap()
}
fn j8() -> Move {
    Move::new(7, 9).unwrap()
}
fn j7() -> Move {
    Move::new(6, 9).unwrap()
}
fn h9() -> Move {
    Move::new(8, 7).unwrap()
}
fn i9() -> Move {
    Move::new(8, 8).unwrap()
}
````

## `gomoku/crates/cli/tests/history.rs`

```rust
//! Headless tests for the variation-tree history.

use cli::history::{ForwardOutcome, History, HistoryError, PlayOutcome};
use engine::{Board, Color, Move, PlayError, Status};
use proptest::prelude::*;

fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}

fn empty_board() -> Board {
    Board::new()
}

fn opening_board() -> Board {
    // A small Swap2-like position: Black has two stones, White one, White to move.
    Board::from_position(&[mv(7, 7), mv(6, 8)], &[mv(6, 7)], Color::White).unwrap()
}

/// Naive reference replay of the cursor path: play out `history.board().moves()`
/// from an empty board. For a `History::new()` this must equal `history.board()`.
fn replay_cursor_path(history: &History) -> Board {
    let mut board = empty_board();
    for &m in history.board().moves() {
        board.play(m).unwrap();
    }
    board
}

#[test]
fn new_history_has_empty_root() {
    let history = History::new();
    assert_eq!(history.ply(), 0);
    assert_eq!(history.board(), empty_board());
    assert!(history.children(history.cursor()).is_empty());
}

#[test]
fn from_opening_uses_given_root() {
    let opening = opening_board();
    let history = History::from_opening(&opening);
    assert_eq!(history.board(), opening);
    assert_eq!(history.board().to_move(), Color::White);
    assert_eq!(history.ply(), 0);
}

#[test]
fn play_appends_and_follows_existing() {
    let mut history = History::new();
    let a = mv(7, 7);
    let b = mv(6, 7);

    let out = history.play(a).unwrap();
    assert!(matches!(out, PlayOutcome::Moved(_)));
    assert_eq!(history.ply(), 1);
    assert_eq!(history.board().to_move(), Color::White);

    // Back to root and replay the same move: it follows the existing child.
    history.back(1);
    let out = history.play(a).unwrap();
    assert!(matches!(out, PlayOutcome::FollowedExisting(_)));
    assert_eq!(history.ply(), 1);

    // A different move is a new branch.
    history.back(1);
    let out = history.play(b).unwrap();
    assert!(matches!(out, PlayOutcome::Moved(_)));
    assert_eq!(history.ply(), 1);
}

#[test]
fn navigation_branch_walk() {
    let mut history = History::new();
    let main_line = [mv(7, 7), mv(6, 7), mv(7, 8), mv(6, 8), mv(7, 9), mv(6, 9)];
    for &m in &main_line {
        history.play(m).unwrap();
    }
    assert_eq!(history.ply(), 6);

    // Back three plies: from ply 6 to ply 3 (cursor on the third move).
    let backed = history.back(3);
    assert_eq!(backed, 3);
    assert_eq!(history.ply(), 3);

    // Branch to a new move.
    let branch = mv(8, 7);
    let out = history.play(branch).unwrap();
    assert!(matches!(out, PlayOutcome::Moved(_)));
    assert_eq!(history.ply(), 4);

    // Return to the branch point.
    let backed2 = history.back(1);
    assert_eq!(backed2, 1);
    assert_eq!(history.ply(), 3);

    // Replay the original fourth move: should follow the existing child.
    let out = history.play(main_line[3]).unwrap();
    assert!(matches!(out, PlayOutcome::FollowedExisting(_)));
    assert_eq!(history.ply(), 4);

    // Back to the branch point and forward should ask for a choice.
    history.back(1);
    let forward = history.forward(1);
    let choices = match forward {
        ForwardOutcome::NeedsChoice(opts) => opts,
        other => panic!("expected NeedsChoice, got {other:?}"),
    };
    assert_eq!(choices, vec![main_line[3], branch]);

    // Pick the branch variation.
    let chosen = history.var(1).unwrap();
    assert_eq!(chosen, branch);
    assert_eq!(history.ply(), 4);

    // Extend the branch.
    let extension = mv(8, 8);
    let out = history.play(extension).unwrap();
    assert!(matches!(out, PlayOutcome::Moved(_)));
    assert_eq!(history.ply(), 5);

    // Forward at a leaf stops.
    let leaf = history.forward(1);
    assert!(matches!(leaf, ForwardOutcome::AtLeaf));

    // Back past the root clamps.
    let backed3 = history.back(100);
    assert_eq!(backed3, 5);
    assert_eq!(history.ply(), 0);
}

#[test]
fn illegal_move_returns_error() {
    let mut history = History::new();
    history.play(mv(7, 7)).unwrap();

    let err = history.play(mv(7, 7)).unwrap_err();
    assert_eq!(err, HistoryError::IllegalMove(PlayError::Occupied));
}

#[test]
fn playing_after_game_over_returns_error() {
    let mut history = History::new();
    // Five in a row for Black on row 7.
    let wins = [
        mv(7, 3),
        mv(0, 0),
        mv(7, 4),
        mv(0, 1),
        mv(7, 5),
        mv(0, 2),
        mv(7, 6),
        mv(0, 3),
        mv(7, 7),
    ];
    for &m in &wins {
        history.play(m).unwrap();
    }
    assert_eq!(history.board().status(), Status::Won(Color::Black));

    let err = history.play(mv(0, 4)).unwrap_err();
    assert_eq!(err, HistoryError::IllegalMove(PlayError::GameOver));
}

#[test]
fn var_out_of_bounds_returns_error() {
    let mut history = History::new();
    history.play(mv(7, 7)).unwrap();
    history.play(mv(6, 7)).unwrap();
    history.back(1);

    let err = history.var(5).unwrap_err();
    assert!(matches!(err, HistoryError::VarOutOfBounds { .. }));
}

#[test]
fn from_opening_play_continues_correct_side() {
    let opening = opening_board();
    let mut history = History::from_opening(&opening);
    let next = mv(5, 7);
    history.play(next).unwrap();
    assert_eq!(history.ply(), 1);
    assert_eq!(history.board().to_move(), Color::Black);
    assert!(history.board().stone_at(next).is_some());
}

#[derive(Debug, Clone)]
enum HistCmd {
    Play(usize),
    Back(u32),
    Forward(u32),
    Var(usize),
}

fn hist_cmd() -> impl Strategy<Value = HistCmd> {
    prop_oneof![
        any::<usize>().prop_map(HistCmd::Play),
        any::<u32>().prop_map(HistCmd::Back),
        any::<u32>().prop_map(HistCmd::Forward),
        any::<usize>().prop_map(HistCmd::Var),
    ]
}

proptest! {
    #[test]
    fn random_walks_keep_board_consistent(cmds in prop::collection::vec(hist_cmd(), 1..100)) {
        let mut history = History::new();

        for cmd in cmds {
            match cmd {
                HistCmd::Play(idx) => {
                    let board = history.board();
                    if board.status() != Status::Ongoing {
                        continue;
                    }
                    let legals: Vec<_> = board.empty_moves().collect();
                    if legals.is_empty() {
                        continue;
                    }
                    let mv = legals[idx % legals.len()];
                    let _ = history.play(mv);
                }
                HistCmd::Back(n) => {
                    history.back(n);
                }
                HistCmd::Forward(n) => {
                    if let ForwardOutcome::NeedsChoice(children) = history.forward(n)
                        && !children.is_empty()
                    {
                        let _ = history.var(0);
                    }
                }
                HistCmd::Var(idx) => {
                    let children = history.children(history.cursor());
                    if !children.is_empty() {
                        let _ = history.var(idx % children.len());
                    }
                }
            }

            // Invariant: board() equals a fresh replay of the cursor path.
            let replayed = replay_cursor_path(&history);
            assert_eq!(history.board(), replayed);

            // Invariant: ply equals the number of moves on the cursor.
            assert_eq!(history.ply(), history.board().moves().len() as u32);
        }
    }
}
```
