> **Opt-in solution — read only after you have tried the chapter, or if
> you have been stuck for more than twenty minutes.**
>
> This file quotes the verified reference implementation for both
> slices of Chapter 01. The files come from the disposable worktree
> `/Users/wgiersche/workspace/Project-Ellie/rust-ml-ref-playable-cli`
> (branch `reference/playable-cli`): slice 0 at commit `664cc40`,
> slice 1 at commit `22f1fa9`.

# Slice 0 (commit `664cc40`) — `refactor(cli): lib/bin split + clap skeleton; legacy UI removed`

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

[dependencies]
crossterm.workspace = true
engine = { path = "../engine" }
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
//! The `gomoku` binary — a clap skeleton over the headless `cli` library.
//!
//! The legacy hotseat/puzzle UI is deleted (owner decision, 2026-10-17:
//! the study table will be the single CLI). The `play` and `selfplay`
//! subcommands parse their arguments, but there is no terminal front-end
//! to dispatch to until the study-table chapter lands.

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "gomoku",
    about = "Terminal Gomoku client",
    subcommand_required = false,
    arg_required_else_help = false
)]
struct CliArgs {
    #[command(subcommand)]
    cmd: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Play at the study table (not yet implemented).
    Play(PlayArgs),
    /// AI-vs-AI self-play (not yet implemented).
    Selfplay(SearchArgs),
}

#[derive(Args, Debug, Clone)]
struct PlayArgs {
    #[command(flatten)]
    search: SearchArgs,

    /// Skip the Swap2 opening and start from an empty board.
    #[arg(long)]
    no_swap2: bool,
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

    match args.cmd {
        None | Some(Commands::Play(_) | Commands::Selfplay(_)) => {
            eprintln!("the study table lands in chapter 05 — nothing runnable yet");
        }
    }
}
```

## The legacy UI is deleted, not moved

The old hotseat/puzzle front-end — `src/ui.rs`, `src/input.rs`,
`src/board.rs`, `src/puzzle.rs`, and the `dyn GameBoard` abstraction
they were built on — is deleted at this commit (owner decision,
2026-10-17: the study table will be the single CLI). There is no
runnable `gomoku play` UI until chapter 05 lands the study table.

Only the pure rendering pieces survive, moved under `src/tui/`:

* `src/tui/opening.rs` (the Swap2 ceremony state machine)
* `src/tui/render.rs` (board rendering, now on `&engine::Board`
  directly — the `GameBoard` indirection is gone)

### `gomoku/crates/cli/src/tui/mod.rs`

```rust
//! Terminal rendering helpers — pure functions, no input handling.
//!
//! The legacy hotseat/puzzle UI (and the `dyn GameBoard` abstraction it
//! needed) is deleted; the study-table front-end will build on these
//! rendering pieces.

pub mod opening;
pub mod render;
```

## Stub core modules

### `gomoku/crates/cli/src/actor.rs`

```rust
//! Seat occupants and AI configuration.
```

### `gomoku/crates/cli/src/command.rs`

```rust
//! Command language and events.
```

### `gomoku/crates/cli/src/history.rs`

```rust
//! Variation tree, stored as a flat node vector (the mcts::Tree idiom).
```

### `gomoku/crates/cli/src/session.rs`

```rust
//! Session core — headless state machine.
```

# Slice 1 (commit `22f1fa9`) — `feat(cli): variation-tree history + historydemo`

## `gomoku/crates/cli/Cargo.toml`

Slice 1 adds the `historydemo` binary, the `thiserror` dependency, and
the `proptest` dev-dependency to the slice-0 manifest:

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

[dependencies]
crossterm.workspace = true
engine = { path = "../engine" }
thiserror.workspace = true
clap = { workspace = true, features = ["derive"] }

[dev-dependencies]
proptest.workspace = true
```

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
    lines.push("    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o".to_string());
    for r in 0..15u8 {
        let mut row = format!("{:2}  ", r + 1);
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
