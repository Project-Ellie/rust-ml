# Chapter 01 — The variation tree

## Abstract

This chapter turns the `cli` crate into a library-plus-binary and
implements the persistent variation tree that underlies the study
table. It is delivered in two slices:

* **Slice 0** refactors the crate: add a `[lib]` target, **delete the
  legacy hotseat/puzzle UI** (`ui.rs`, `input.rs`, `board.rs`,
  `puzzle.rs`, and the `dyn GameBoard` abstraction — owner decision:
  the study table will be the single CLI), keep only the pure rendering
  pieces under [`src/tui/`](../../../gomoku/crates/cli/src/tui/),
  add stub modules for `history`, `command`, `session`, and `actor`,
  and install a `clap` subcommand skeleton (`play` / `selfplay`) whose
  handlers report that the study table lands in chapter 05.
* **Slice 1** builds [`History`](../../../gomoku/crates/cli/src/history.rs):
  a persistent variation tree that silently branches on new moves and
  replays `root → cursor` to produce a board.

The capstone is `historydemo`: a small binary that walks a scripted
branch and prints the board at each step. After this chapter, the
study table has its memory — every position reached stays reachable.

## Glossary

| Term | Definition |
|------|------------|
| **Core/shell split** | The `cli` crate is divided into a headless library (`history`, `command`, `session`, `actor`) and a thin binary front-end. Commands go in; events and snapshots come out. |
| **History / variation tree** | A persistent tree of game positions with a cursor. Each node stores the move that led to it; the root stores the opening board. Siblings are preserved. |
| **Cursor** | The currently displayed node in the history tree. All `History` mutations act at the cursor. |
| **NodeId** | A `u32` index into [`History::nodes`](../../../gomoku/crates/cli/src/history.rs). |
| **PlayOutcome** | Return type that distinguishes a newly appended child (`Moved`) from a walk along an existing edge (`FollowedExisting`). |
| **ForwardOutcome** | Return type that distinguishes forward motion (`Moved`), a branch point that needs a choice (`NeedsChoice`), and a leaf (`AtLeaf`). |
| **Replay-to-cursor** | Rebuilding the board by replaying moves from the root to the cursor. The maximum cost is 225 `Board::play` calls. |
| **Arena idiom** | One `Vec<Node>` with integer indices as handles; already used by [`mcts::Tree`](../../../gomoku/crates/mcts/src/tree.rs). |

## Context

The playable CLI primer ([`00-playable-cli-primer.md`](00-playable-cli-primer.md))
establishes the study-table model: two seats, a persistent variation
tree, and a core/shell split. This chapter builds the first core
module.

Before this chapter, the `cli` crate is a binary-only hotseat client.
Slice 0 of this chapter deletes that legacy UI (the study table will
be the single CLI — the owner accepted having no runnable `gomoku
play` UI until chapter 05), adds the library target, and installs the
`clap` skeleton that later chapters wire up (Task 0 of the
implementation plan,
[`docs/plans/2026-10-17-playable-cli.md`](../../plans/2026-10-17-playable-cli.md)).
Slice 1 implements the tree itself
([`docs/specs/2026-10-17-playable-cli-design.md`](../../specs/2026-10-17-playable-cli-design.md)
§7, Task 1 of the plan).

The design authority is the spec. When this chapter and the spec
disagree, the spec wins.

## Intention

The one thing you get: `historydemo` prints a scripted branch walk as
ASCII boards, and **now you can branch off and navigate back**.

The design value behind that feeling is *silent branching*. When you
play a new move at an old cursor position, the tree adds a sibling
without asking for confirmation, because branching destroys nothing.
The design spec §7 records this as an owner requirement: "otherwise a
new child is appended — **silently, no confirmation**." That silence
is what makes the tree a study instrument rather than an undo buffer
that treats every new line as a destructive action.

## Mental mapping

### Arena + indices: reuse the `mcts::Tree` idiom

The `mcts` crate already stores its search tree as one `Vec<Node>`
with `NodeId` handles. `History` uses the same shape: a single tree of nodes,
a root index, and a cursor index. You are not inventing a data
structure; you are applying an in-repo idiom to game history.

### Root holds the board; children hold only moves

Only the root stores a full [`Board`](../../../gomoku/crates/engine/src/board.rs).
Every other node stores just the [`Move`](../../../gomoku/crates/engine/src/lib.rs)
that led from its parent. This makes branching cheap: adding a child
means pushing one `Node { parent, mv, children: [] }` and one index.

### Replay-to-cursor is the correctness argument

`History::board()` rebuilds the position by replaying the path from
the root to the cursor. The cost is bounded by the maximum game
length: 225 plies on a 15×15 board. The old CLI replayed
from a log for undo, so this is not a new cost model; it is a new
correctness model. The board at the cursor is *exactly* the board the
engine would have after those moves, with no separate state to drift
out of sync.

### `FollowedExisting` vs `Moved`

`History::play` has two successful outcomes:

* `FollowedExisting(id)` — the move already existed as a child; the
cursor simply moved there. Replaying the same line is idempotent.
* `Moved(id)` — the move created a new branch. This is silent
branching in action.

Tests can assert which outcome occurs to prove tree shape, and the
front-end can use the distinction to report "new variation" when it
matters.

## Low-level design

### Slice 0 — crate split and clap skeleton

Files you touch in this slice:

* [`gomoku/crates/cli/Cargo.toml`](../../../gomoku/crates/cli/Cargo.toml)
  — add `[lib]`, keep `[[bin]]`, add `clap`; the `naive-engine`
  feature is gone with the legacy UI.
* [`gomoku/Cargo.toml`](../../../gomoku/Cargo.toml) — add `clap` to
  workspace dependencies.
* [`gomoku/crates/cli/src/lib.rs`](../../../gomoku/crates/cli/src/lib.rs)
  — new library root exposing `actor`, `command`, `history`, `session`,
  and `tui`.
* [`gomoku/crates/cli/src/main.rs`](../../../gomoku/crates/cli/src/main.rs)
  — clap skeleton with `play` and `selfplay` subcommands; both handlers
  print "the study table lands in chapter 05 — nothing runnable yet".
* [`gomoku/crates/cli/src/tui/mod.rs`](../../../gomoku/crates/cli/src/tui/mod.rs)
  — `tui/` keeps only the pure rendering pieces (`opening.rs`,
  `render.rs`); `render.rs` now works on `&engine::Board` directly.
  The study shell (`study.rs`) and overlays land in chapters 05–06.
* `actor.rs`, `command.rs`, `history.rs`, `session.rs` — stub modules
  with module docs.

Then delete the legacy UI: `src/ui.rs`, `src/input.rs`,
`src/board.rs`, `src/puzzle.rs`, and the `dyn GameBoard` trait they
implement. The
naive engine stays in the `engine` crate as the test oracle; *playing*
on it was curriculum scaffolding, and that scaffolding is what you are
deleting.

Why split the crate now? The core/shell split is the load-bearing
structure of the whole tutorial. A headless library lets every
subsequent behavior be tested by feeding commands to `SessionCore` and
asserting events, with no terminal to mock. It also means a future REST
server or GUI could drive the same core. The `clap` skeleton is added
now so that later chapters only wire subcommands, not redesign the
front-end.

Why delete the old UI instead of preserving it behind flags? Because
two UIs means two of everything: two input paths, two rendering stacks,
two test matrices. The design spec records the owner decision
(2026-10-17): there is exactly one UI, and it is the study table.
`--plain` is subsumed by the non-tty line-mode fallback (chapter 05),
naive-engine play is deleted, and the old puzzle mode is replaced by
puzzle *positions* enrolling into the study table (chapter 07). The
cost is real — nothing runnable until chapter 05 — and the owner
accepted it explicitly.

### Slice 1 — the variation tree

Files you create or modify in this slice:

* [`gomoku/crates/cli/src/history.rs`](../../../gomoku/crates/cli/src/history.rs)
  — the variation tree.
* [`gomoku/crates/cli/src/bin/historydemo.rs`](../../../gomoku/crates/cli/src/bin/historydemo.rs)
  — the chapter capstone binary.
* [`gomoku/crates/cli/tests/history.rs`](../../../gomoku/crates/cli/tests/history.rs)
  — unit and property tests.

#### Exact `History` API

Implement exactly this public contract:

```rust
pub type NodeId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayOutcome {
    Moved(NodeId),
    FollowedExisting(NodeId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForwardOutcome {
    Moved(u32),
    NeedsChoice(Vec<Move>),
    AtLeaf,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HistoryError {
    IllegalMove(PlayError),
    VarOutOfBounds { index: usize, child_count: usize },
}

impl History {
    #[must_use]
    pub fn new() -> Self;

    #[must_use]
    pub fn from_opening(board: &Board) -> Self;

    #[must_use]
    pub fn cursor(&self) -> NodeId;

    #[must_use]
    pub fn board(&self) -> Board;

    pub fn play(&mut self, mv: Move) -> Result<PlayOutcome, HistoryError>;

    pub fn back(&mut self, n: u32) -> u32;

    pub fn forward(&mut self, n: u32) -> ForwardOutcome;

    pub fn var(&mut self, index: usize) -> Result<Move, HistoryError>;

    #[must_use]
    pub fn children(&self, at: NodeId) -> Vec<Move>;

    #[must_use]
    pub fn ply(&self) -> u32;
}
```

Behavior:

* `play` validates the move against the cursor board. If an identical
  child exists, the cursor follows it and returns
  `FollowedExisting(id)`; otherwise a new child is appended and
  `Moved(id)` is returned.
* `back(n)` walks toward the root and returns the number of plies
  actually moved, clamped at the root.
* `forward(n)` follows single-child lines. If it reaches a leaf it
  returns `AtLeaf`; if it reaches a branch point it returns
  `NeedsChoice` with the child moves and stops there.
* `var(index)` chooses the child at `index` into `children(cursor)`.
* `board()` recomputes the cursor position by replaying from the root.
* `children(at)` lists the child moves at any node, in creation order.
* `ply()` returns the cursor depth in plies from the root.

Coordinates use algebraic notation: a move is printed as a column
letter (a–o) followed by a row number (1–15). The internal
representation remains `Move { row, col }`; only the display mapping
changed.

## Capstone: branch off and navigate back

Run it with:

```text
cd gomoku
cargo run -q -p cli --bin historydemo
```

Expect exactly this output:

```text
variation-tree demo
=================================

start: empty board, root cursor
ply 0  path: (root)
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 5  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 8  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  

main line: h8 h7 i8 i7 j8 j7
ply 6  path: h8 h7 i8 i7 j8 j7
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 5  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  O  O  O  .  .  .  .  .  
 8  .  .  .  .  .  .  .  X  X  X  .  .  .  .  .  
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  

back 3
ply 3  path: h8 h7 i8
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 5  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  O  .  .  .  .  .  .  .  
 8  .  .  .  .  .  .  .  X  X  .  .  .  .  .  .  
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  

branch: h9
ply 4  path: h8 h7 i8 h9
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 5  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  O  .  .  .  .  .  .  .  
 8  .  .  .  .  .  .  .  X  X  .  .  .  .  .  .  
 9  .  .  .  .  .  .  .  O  .  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  

back to branch point
ply 3  path: h8 h7 i8
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 5  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  O  .  .  .  .  .  .  .  
 8  .  .  .  .  .  .  .  X  X  .  .  .  .  .  .  
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  

forward: NeedsChoice
ply 3  path: h8 h7 i8
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 5  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  O  .  .  .  .  .  .  .  
 8  .  .  .  .  .  .  .  X  X  .  .  .  .  .  .  
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  

    choices: i7 h9

var 1 -> h9
ply 4  path: h8 h7 i8 h9
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 5  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  O  .  .  .  .  .  .  .  
 8  .  .  .  .  .  .  .  X  X  .  .  .  .  .  .  
 9  .  .  .  .  .  .  .  O  .  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  

extend: i9
ply 5  path: h8 h7 i8 h9 i9
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 5  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  O  .  .  .  .  .  .  .  
 8  .  .  .  .  .  .  .  X  X  .  .  .  .  .  .  
 9  .  .  .  .  .  .  .  O  X  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  

back 5 to root
ply 0  path: (root)
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 5  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 8  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  


```

## Solution (opt-in)

> **Opt-in solution — read only after you have tried the chapter, or if
> you have been stuck for more than twenty minutes.**
>
> The target state of both slices is quoted in
> [`01-the-variation-tree/01-solution.md`](01-the-variation-tree/01-solution.md).
> Open it only if you need it; using it is not failure.

## TDD checklist

Run these from `gomoku/` with `cargo test -p cli`.

The exact `#[test]` functions in `tests/history.rs` are:

1. `new_history_has_empty_root`
2. `from_opening_uses_given_root`
3. `play_appends_and_follows_existing`
4. `navigation_branch_walk`
5. `illegal_move_returns_error`
6. `playing_after_game_over_returns_error`
7. `var_out_of_bounds_returns_error`
8. `from_opening_play_continues_correct_side`
9. `random_walks_keep_board_consistent`

Supporting helpers (not `#[test]`):

* `empty_board`
* `opening_board`
* `replay_cursor_path`
* `mv`
* `hist_cmd`

Follow the red-green-refactor rhythm:

1. **Red:** Add the crate split and the `History` API. Run
   `cargo test -p cli`; expect failures.
2. **Green:** Implement slice 0 and slice 1 until the tests above
   pass.
3. **Run the capstone:** `cargo run -q -p cli --bin historydemo` must
   print the output shown above.
4. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

All tests that survive the slice-0 deletion continue to pass; the
deleted hotseat and puzzle flows take their tests with them. The
`engine` and `mcts` crates are untouched.

## Done when

* Slice 0 builds and all existing tests pass:
  `cargo test -p cli` from `gomoku/`.
* Slice 1 tests pass: the nine test names above are green.
* `cargo run -q -p cli --bin historydemo` prints the quoted branch
  walk.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes from `gomoku/`.
* Commit slice 0:
  `refactor(cli): lib/bin split + clap skeleton; legacy UI removed`.
* Commit slice 1:
  `feat(cli): variation-tree history + historydemo`.

Next: [Chapter 02 — The command language](02-the-command-language.md)

## References

* [`00-playable-cli-primer.md`](00-playable-cli-primer.md) — the study
  table, core/shell, commands/events.
* [`docs/plans/2026-10-17-playable-cli.md`](../../plans/2026-10-17-playable-cli.md)
  — Task 0 and Task 1.
* [`docs/specs/2026-10-17-playable-cli-design.md`](../../specs/2026-10-17-playable-cli-design.md)
  — §7 variation tree, §4 core/shell split.
* [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  crate topology and dependency rules.
* [`docs/13-engine-design.md`](../../13-engine-design.md) — the fast
  engine board and move semantics.
* [`docs/tutorials/mcts-tutorial/README.md`](../mcts-tutorial/README.md)
  — the `mcts::Tree` arena idiom and `Evaluator` seam.
