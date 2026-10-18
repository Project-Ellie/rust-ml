# Chapter 07 — Positions from files: puzzles

## Abstract

This chapter enrolls the first external position source into the study
table: **puzzle packs**. A puzzle is not a game mode; it is a *position
with a claimed solution line*, and the study table is where you study
it. You will write the pack parser (`PuzzlePack::from_json`) with full
validation, the `SessionInit::puzzle` constructor with its `PuzzleStudy`
context, the four study commands (`hint`, `reveal`, `next`, `prev`),
and the startup flag `gomoku play --puzzle <pack.json> [--index N]`.
The felt capstone: **load a pack, ask for a hint, try a wrong move,
take it back, reveal the line, move to the next puzzle.**

## Glossary

| Term | Definition |
|------|------------|
| **Pack** | A JSON file of puzzle positions: `{v, count, items: [{i, a, d, b, w, l}]}`. The corpus format of `data/puzzles/files/`. |
| **Solution line** | The pack's claimed forced-win line for one puzzle: the `l` field, a list of `[x, y, side]` triples. A claim, not a proof — milestone 4 re-verifies every line with the threat-space search (TSS) prover before use. |
| **Attacker** | The side with the claimed forced win (the `a` field); always the side to move when the puzzle loads. In VCF (victory by continuous four) puzzles, the side that wins by an unbroken forcing sequence. |
| **Hint** | The command that shows the first move of the loaded puzzle's claimed line. |
| **Reveal** | The command that shows the full claimed line. |
| **Clamp** | `next` / `prev` stop at the pack ends with a typed error instead of wrapping around. |
| **`PuzzleStudy`** | The session context carrying the pack path, the current index, and the whole parsed pack, so `next` / `prev` need no further I/O. |
| **Fixture discipline** | Tests load only the committed `crates/cli/tests/fixtures/mini-pack.json`, never the git-ignored corpora under `data/`. |

## Context

Chapter 06 finished the visualization layer: the table shows the
search's visits, prior, and value at any cursor position. What the
table cannot yet do is start from a position somebody else gives you.
The spec amendment "positions from files" (owner decision, 2026-10-17)
adds two such sources after the overlays chapter; this chapter builds
the first, and chapter 08 builds the second.

The material already exists. `data/puzzles/` holds three VCF puzzle
collections in one JSON format (see its README): one labelled set with
claimed solution lines (`vcf-material-763`, 763 positions) and two
unlabelled sets (kaibao 1052, RenjuPortal 4024) that milestone 4 will
label with the TSS oracle. The files are git-ignored — the puzzle
content is third-party — and regenerate with `data/puzzles/fetch.sh`.

The old CLI had a puzzle viewer of its own; chapter 01 deleted it with
the rest of the legacy UI. One piece of that viewer survives as a
convention: it rendered these packs correctly with file coordinates
`[x, y]` = column, row, 0-based.

Design constraints from the amendment:

1. **Puzzles are not a game mode.** Loading a puzzle seeds the
   variation tree root; everything else — seats, branching, overlays —
   is the table you already have.
2. **The core/shell doctrine is unchanged.** Parsing lives in the
   library behind `SessionInit`; file I/O lives in `main.rs` and
   `sessiondemo`; events remain the only output of the core.
3. **Missing data files are a first-class error.** A missing pack
   exits with a message pointing at `data/puzzles/fetch.sh`, because
   the corpora are git-ignored and "not found" most likely means "not
   fetched yet".
4. **Tests never touch `data/`.** A committed mini fixture under
   `crates/cli/tests/fixtures/` carries every test.

## Intention

1. Create `gomoku/crates/cli/src/puzzle.rs`:
   * `Puzzle`, `PuzzlePack`, and a thiserror `PuzzleError`;
   * `PuzzlePack::from_json` with full validation (stone counts,
     duplicates, overlap, attacker, line head side) under the legacy
     coordinate convention.
2. Extend `gomoku/crates/cli/src/command.rs`:
   * commands `Hint`, `Reveal`, `NextPuzzle`, `PrevPuzzle`;
   * events `Hint`, `SolutionRevealed`, `PuzzleLoaded`;
   * errors `NoPuzzleLoaded`, `NoSolutionLine`, `AtLastPuzzle`,
     `AtFirstPuzzle`;
   * parser arms for `hint`, `reveal`, `next`, `prev`.
3. Extend `gomoku/crates/cli/src/session.rs`:
   * the `PuzzleStudy` context and the `SessionInit::puzzle`
     constructor;
   * `execute_hint`, `execute_reveal`, and `execute_puzzle_nav` with
     clamp-at-ends navigation;
   * `new` resets to the *initially loaded* puzzle.
4. Extend `gomoku/crates/cli/src/main.rs` with
   `--puzzle <PACK.json> [--index N]` (Swap2 skipped; exit 2 plus the
   fetch.sh hint on a missing or invalid file) and
   `gomoku/crates/cli/src/bin/sessiondemo.rs` with the same flags plus
   `#`-comments in scripts.
5. Extend `gomoku/crates/cli/src/tui/render.rs` with the side-panel
   puzzle line and `gomoku/crates/cli/src/tui/study.rs` with the new
   help entries.
6. Create `gomoku/crates/cli/tests/fixtures/mini-pack.json`,
   `gomoku/crates/cli/tests/puzzle.rs`, the puzzle session tests in
   `gomoku/crates/cli/tests/session.rs`, and
   `gomoku/crates/cli/puzzle_demo.txt`.

Observable done-state: the scripted `sessiondemo --puzzle` run in the
capstone reproduces; the TDD checklist passes; the gates are clean.

## Mental mapping

### A puzzle is a position, not a mode

The legacy puzzle viewer was a separate flow with its own board and its
own input handling. The study table deletes that idea. Loading a puzzle
produces a hotseat session whose variation tree root is the puzzle
position, attacker to move. Take/release, AI seats, branching, the
overlays from chapter 06 — all of it works by construction, because
there is nothing puzzle-specific below the command layer. The puzzle
adds exactly three things: the claimed solution line (`hint` /
`reveal`), the pack (`next` / `prev`), and one context line in the side
panel.

Note what the table does *not* do: it does not grade you. Trying a move
that is not the claimed first move emits an ordinary `Moved` event, not
a buzzer. Your try becomes a branch in the tree; `back` takes it back;
the branch stays visible (`branches: 0:i9` in the capstone). Studying a
puzzle means comparing your try against the claim — and, when the claim
surprises you, releasing a seat and letting the search examine it. A
grader would collapse all of that into right/wrong.

### The pack format, and the coordinate trap

The pack format (from `data/puzzles/README.md`):

```json
{ "v": 1, "count": 763, "items": [
  { "i": 0, "a": "black", "d": 6, "b": [[x,y],...], "w": [[x,y],...],
    "l": [[x,y,side],...] } ] }
```

`v` is the format version (currently 1), `count` must equal
`items.len()`, `i` is the file's own puzzle number (used in error
messages), `a` the attacker, `d` the claimed line depth (optional,
unused by the table), `b`/`w` the stone lists, and `l` the claimed
line.

Two conventions, both kept from the legacy viewer:

* **Coordinates are 0-based `[x, y]` with `x` the column and `y` the
  row**, mapping to `Move::new(y, x)`. Swapping `x` and `y` mirrors
  every position across the main diagonal: stones are still on the
  board, fives are still fives, and every puzzle is subtly wrong. The
  test `file_coordinates_are_x_column_y_row` pins the convention.
* **Line side markers are `1` = Black, `2` = White**, and the first
  move of a claimed line must be the attacker's.

Validation is strict about what the bitboard builder would forgive:
duplicate stones within a color and overlap between colors are checked
explicitly, because `Board::from_position` would silently collapse
them. The board itself is built with
`Board::from_position(&black, &white, attacker)`, which enforces the
Gomoku turn invariant (stone counts consistent with the attacker).

### Unlabelled packs are not errors

The `l` field is optional: two of the three corpora are positions only
— their labels are milestone-4 generator output, where the TSS prover
produces the lines and `verify_line` filters them. An empty line is
therefore data, not a parse failure. `hint` and `reveal` on an
unlabelled puzzle return the typed `NoSolutionLine` error naming the
puzzle, so a front-end can grey out the commands instead of crashing.

### Why navigation clamps instead of wrapping

`next` at the last puzzle has two candidate designs: wrap around to the
first, or fail. Wrapping silently teleports you away from the puzzle
you were studying — and because every load is a fresh tree root, it
discards your variations without a word. Clamping fails loudly with the
typed `AtFirstPuzzle` / `AtLastPuzzle` errors and leaves the current
puzzle loaded; a shell can catch the error and grey out the button.

Each load replaces the variation tree wholesale
(`History::from_opening` of the new position, `search_info` cleared).
Seats and AI configuration are untouched: if White was an AI at 800
simulations on puzzle 4, it still is on puzzle 5. And `new` resets to
the puzzle the session *started* with, not the one you navigated to —
the `SessionInit` is the source of truth for resets.

### The session keeps the whole pack

`SessionInit::puzzle` takes the *parsed* pack, not a path. The core
never touches the filesystem (core/shell doctrine): `main.rs` and
`sessiondemo` read the file once at startup, and `next` / `prev` need
no further I/O. The pack path is kept in `PuzzleStudy` only for display
and error messages.

### Startup: one more initial seat assignment

`gomoku play --puzzle pack.json --index 4` is the `play` flow with a
different opening board. Swap2 is skipped — the puzzle position *is*
the opening — and the `--ai` / `--color` / sims flags compose exactly as
they do for a Swap2 opening. A missing or invalid pack exits with code
2 (usage/data error), names the path, and points at
`data/puzzles/fetch.sh`.

## Low-level design

### Files changed vs chapter 06

The reference worktree built the acceptance tests (chapter 09) before
this chapter, so the puzzle commit `d6c2af3` sits on `7d7ff13`, not on
chapter 06's `c41d339`. The in-between commits are test-only; the diff
below touches `tests/acceptance.rs` only with a mechanical
`puzzle: None` field addition.

```text
$ git diff 7d7ff13..d6c2af3 --stat -- gomoku/crates/cli
 gomoku/crates/cli/Cargo.toml                    |   1 +
 gomoku/crates/cli/puzzle_demo.txt               |  13 ++
 gomoku/crates/cli/src/bin/sessiondemo.rs        |  62 ++++-
 gomoku/crates/cli/src/command.rs                | 103 +++++++++
 gomoku/crates/cli/src/lib.rs                    |   3 +-
 gomoku/crates/cli/src/main.rs                   |  62 ++++-
 gomoku/crates/cli/src/puzzle.rs                 | 290 ++++++++++++++++++++++++
 gomoku/crates/cli/src/session.rs                | 130 +++++++++++
 gomoku/crates/cli/src/tui/render.rs             |   9 +
 gomoku/crates/cli/src/tui/study.rs              |   3 +
 gomoku/crates/cli/tests/acceptance.rs           |   1 +
 gomoku/crates/cli/tests/actor.rs                |   3 +
 gomoku/crates/cli/tests/fixtures/mini-pack.json |  36 +++
 gomoku/crates/cli/tests/puzzle.rs               | 215 ++++++++++++++++++
 gomoku/crates/cli/tests/session.rs              | 180 +++++++++++++++
 gomoku/crates/cli/tests/study.rs                |   4 +
 16 files changed, 1106 insertions(+), 9 deletions(-)
```

### `src/puzzle.rs`

New module. Public surface:

```rust
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

pub struct PuzzlePack {
    /// All puzzles in file order.
    pub items: Vec<Puzzle>,
}

impl PuzzlePack {
    pub fn from_json(text: &str) -> Result<Self, PuzzleError>;
    pub fn get(&self, index: usize) -> Result<&Puzzle, PuzzleError>;
}
```

`PuzzleError` variants: `Json`, `CountMismatch`, `InvalidAttacker`,
`InvalidCoord`, `DuplicateStone`, `OverlappingColors`, `InvalidCounts`,
`InvalidSide`, `InvalidSolutionSide`, `IndexOutOfRange`.

The serde schema (`PackFile` / `PackItem`) is private; unknown fields
(the corpora's extra `u` / `f`) are ignored, and `l` defaults to an
empty vec.

### `src/command.rs`

Additions to the vocabulary:

```rust
pub enum Command {
    // ...
    /// Show the first move of the loaded puzzle's claimed line.
    Hint,
    /// Show the full claimed line of the loaded puzzle.
    Reveal,
    /// Load the next puzzle in the pack as a fresh tree root.
    NextPuzzle,
    /// Load the previous puzzle in the pack as a fresh tree root.
    PrevPuzzle,
    // ...
}

pub enum Event {
    // ...
    Hint { mv: Move },
    SolutionRevealed { line: Vec<(Move, Color)> },
    PuzzleLoaded { index: usize, count: usize, attacker: Color },
    // ...
}

pub enum CommandError {
    // ...
    NoPuzzleLoaded,
    NoSolutionLine { index: usize, count: usize },
    AtLastPuzzle { count: usize },
    AtFirstPuzzle { count: usize },
    // ...
}
```

`Event::PuzzleLoaded` keeps its fields 0-based for machine consumers;
its `Display` adds the 1 (`puzzle 2 of 3 · White to move`). The parser
gains four arms: `hint`, `reveal`, `next`, `prev`, each rejecting extra
tokens.

### `src/session.rs`

```rust
pub struct PuzzleStudy {
    /// Path the pack was loaded from (for display and error messages).
    pub pack_path: PathBuf,
    /// Zero-based index of the currently loaded puzzle.
    pub index: usize,
    /// The parsed pack.
    pub pack: PuzzlePack,
}

impl SessionInit {
    pub fn puzzle(pack_path: PathBuf, pack: PuzzlePack, index: usize)
        -> Result<Self, PuzzleError>;
}

impl SessionCore {
    pub fn puzzle(&self) -> Option<&PuzzleStudy>;
}
```

`execute_hint` and `execute_reveal` read through a `puzzle_context()`
helper that returns `NoPuzzleLoaded` when the session has no pack.
`execute_puzzle_nav(delta)` with `delta` of 1 or -1 clamps at the ends,
rebuilds the history from the new puzzle's board, clears `search_info`,
and emits `PuzzleLoaded`. `reset()` restores `init.puzzle`, so `new`
returns to the initially loaded puzzle.

### `src/main.rs`

`PlayArgs` gains:

```rust
/// Study puzzle N of a puzzle pack (skips Swap2).
#[arg(long, value_name = "PACK.json")]
puzzle: Option<PathBuf>,

/// Zero-based index of the puzzle to load (default 0).
#[arg(long, value_name = "N", requires = "puzzle")]
index: Option<usize>,
```

`build_puzzle_init` reads and parses the pack (exit 2 with the fetch.sh
hint on failure), builds the normal play init with the puzzle board as
opening, and attaches the `PuzzleStudy`. The `selfplay` subcommand
rejects `--puzzle` with exit 2.

### `src/bin/sessiondemo.rs`

A `parse_flag_value` helper, `--puzzle` / `--index` handling in
`build_init`, `#`-comment lines skipped in scripts, and a
`puzzle <i+1> of <count>  attacker <Color>` line in `print_state`.

### `src/tui/render.rs` and `src/tui/study.rs`

The side panel gains one line under `Path:` when a pack is loaded:
`Puzzle: <i+1> of <count> · <attacker> to move`. The study-table help
text gains the three command summaries.

### The fixture

`tests/fixtures/mini-pack.json` holds three hand-verified puzzles: (1)
Black to move, a vertical four on column h with the one-move line `h8`;
(2) White to move, a horizontal four on row 5 with the one-move line
`i5`; (3) a three-ply line. Every stone list satisfies the turn
invariant for its attacker, and the extra `u` / `f` fields prove the
schema ignores unknown keys. Tests load it with `include_str!`, never
the corpora.

## Solution (opt-in)

The complete reference files for this chapter are quoted verbatim in
[07-positions-from-files-puzzles/01-solution.md](07-positions-from-files-puzzles/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare your work.

## TDD checklist

Run from `gomoku/` with `cargo test -p cli`.

### `tests/puzzle.rs`

Exact `#[test]` function names from commit `d6c2af3`:

1. `parses_valid_pack`
2. `file_coordinates_are_x_column_y_row`
3. `board_is_replayable_position_with_attacker_to_move`
4. `malformed_json_is_rejected`
5. `count_mismatch_is_rejected`
6. `off_board_coordinate_is_rejected`
7. `off_board_line_coordinate_is_rejected`
8. `duplicate_stone_is_rejected`
9. `overlapping_colors_are_rejected`
10. `counts_disagreeing_with_attacker_are_rejected`
11. `unknown_attacker_is_rejected`
12. `line_starting_with_the_wrong_side_is_rejected`
13. `unknown_line_side_marker_is_rejected`
14. `missing_line_defaults_to_empty_solution`
15. `index_out_of_range_is_rejected`

### `tests/session.rs`

Ten new session tests at `d6c2af3`:

1. `puzzle_session_roots_at_puzzle_position_with_attacker_to_move`
2. `puzzle_init_rejects_out_of_range_index`
3. `hint_emits_first_solution_move`
4. `hint_errors_when_puzzle_has_no_line`
5. `reveal_emits_full_claimed_line`
6. `next_and_prev_load_a_fresh_root_and_report_position`
7. `next_prev_clamp_at_pack_ends`
8. `puzzle_commands_error_without_a_pack`
9. `puzzle_position_supports_full_study_flow`
10. `new_resets_to_the_initially_loaded_puzzle`

### `src/command.rs`

One new parser test at `d6c2af3`:

1. `parse_puzzle_commands`

All earlier tests continue to pass and are part of the chapter-07 gate;
the earlier `SessionInit` struct literals gain a `puzzle: None` field
(`tests/acceptance.rs`, `tests/actor.rs`, `tests/study.rs`).

### Red-green-refactor rhythm

1. **Red:** Create `src/puzzle.rs`, extend `src/command.rs` and
   `src/session.rs`, add the fixture, `tests/puzzle.rs`, and the new
   session tests. Run `cargo test -p cli`. Expect failures.
2. **Green:** Implement the contracts above until `cargo test -p cli`
   passes.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any issues.

## Capstone: hint, wrong try, take-back, reveal, next

The script (`crates/cli/puzzle_demo.txt`; `#` lines are comments):

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

Run it in the reference worktree:

```text
cd gomoku
cargo run -q -p cli --bin sessiondemo -- --puzzle crates/cli/tests/fixtures/mini-pack.json --script crates/cli/puzzle_demo.txt
```

Frames from the measured output (commit `d6c2af3`).

The session opens on puzzle 1 of 3: Black to move, the vertical four on
column h, and the puzzle line under the status row:

```text
Session started. Type 'h' for help.
ply 0  status Ongoing  black Human  white Human
puzzle 1 of 3  attacker Black
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  O  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 2  O  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  O  .  .  .  .  .  .  O  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  X  .  .  .  .  .  .  .  
 5  .  .  .  .  .  .  .  X  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  X  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  X  .  .  .  .  .  .  .  
 8  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
```

`hint` shows the first move of the claimed line; the board is
unchanged:

```text
  hint: the line starts at h8
ply 0  status Ongoing  black Human  white Human
puzzle 1 of 3  attacker Black
```

The wrong try `i9` is legal, so the table plays it — an ordinary move,
not a grader's buzzer:

```text
  Moved i9 by Human
ply 1  status Ongoing  black Human  white Human
puzzle 1 of 3  attacker Black
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  O  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 2  O  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  O  .  .  .  .  .  .  O  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  X  .  .  .  .  .  .  .  
 5  .  .  .  .  .  .  .  X  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  X  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  X  .  .  .  .  .  .  .  
 8  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 9  .  .  .  .  .  .  .  .  x  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
```

`back` takes the try back. The branch stays in the tree:

```text
  Cursor moved to ply 0
ply 0  status Ongoing  black Human  white Human
puzzle 1 of 3  attacker Black
branches: 0:i9
```

Giving up: `reveal` shows the full claimed line (one move here — the
four on column h completes at `h8`):

```text
  solution: h8 Black
ply 0  status Ongoing  black Human  white Human
puzzle 1 of 3  attacker Black
branches: 0:i9
```

`next` loads puzzle 2 as a fresh root — White to move, a horizontal
four on row 5; the `i9` branch is gone with puzzle 1:

```text
  puzzle 2 of 3 · White to move
ply 0  status Ongoing  black Human  white Human
puzzle 2 of 3  attacker White
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  X  X  X  X  .  .  .  .  .  .  .  .  .  .  .  
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 5  .  .  .  .  O  O  O  O  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 8  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  X  
```

And the new puzzle's hint:

```text
  hint: the line starts at i5
ply 0  status Ongoing  black Human  white Human
puzzle 2 of 3  attacker White
```

That is the whole workflow: load a pack, ask for a hint, try a wrong
move, take it back, reveal the line, move on. From here every table
feature applies — release a seat and the AI will examine the claimed
line with the chapter-06 overlays.

## Done when

* `cargo test -p cli` passes from `gomoku/` (including all earlier
  tests, the 15 parser tests, and the 10 new session tests).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* The `sessiondemo --puzzle` scripted demo prints the frames above.
* Commit message for the code slice:
  `feat(cli): puzzles enroll into the study table`.

Next: [Chapter 08 — Positions from files: game records](08-positions-from-files-game-records.md)

## References

* [`00-playable-cli-primer.md`](00-playable-cli-primer.md) — the study
  table, core/shell, and the positions-from-files amendment summary.
* [`README.md`](README.md) — chapter map and tutorial conventions.
* [`docs/plans/2026-10-17-playable-cli.md`](../../plans/2026-10-17-playable-cli.md)
  — amendment task T8 (this chapter).
* [`docs/specs/2026-10-17-playable-cli-design.md`](../../specs/2026-10-17-playable-cli-design.md)
  — the "positions from files" amendment (puzzle packs).
* [`data/puzzles/README.md`](../../../data/puzzles/README.md) — the
  pack format, the three corpora, and the milestone-4 re-verification
  requirement.
* [`01-the-variation-tree.md`](01-the-variation-tree.md) — the
  variation tree every puzzle loads into.
* [`06-seeing-the-mind.md`](06-seeing-the-mind.md) — the overlays that
  work on puzzle positions by construction.
