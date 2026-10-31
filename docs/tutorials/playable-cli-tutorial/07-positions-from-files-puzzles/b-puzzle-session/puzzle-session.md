# Sub-chapter 07b — The puzzle enrolls

## Abstract

This sub-chapter loads a parsed puzzle pack into the study table. It
adds `PuzzleStudy` (pack path, current index, and the whole parsed
pack), the `SessionInit::puzzle` constructor, the `puzzle` field on
`SessionInit` and `SessionCore`, a `puzzle()` accessor, and reset logic
that returns the session to the initially loaded puzzle. The only
ripple outside `session.rs` is the `puzzle: None` field in every
existing `SessionInit` struct literal — in `main.rs`, `tests/actor.rs`,
and `tests/study.rs`.

The sub-chapter's lesson is that a puzzle is a position, not a mode.
Once the puzzle board becomes the variation-tree root, every tool from
chapters 01–06 still works by construction.

## Glossary

| Term | Definition |
|------|------------|
| **Puzzle pack** | A parsed `PuzzlePack` from `src/puzzle.rs`; a list of validated positions. |
| **PuzzleStudy** | The session context that carries `pack_path`, `index`, and the whole `pack`. |
| **`SessionInit::puzzle`** | The constructor that seeds a hotseat session from one puzzle of a pack. |
| **Attacker** | The side to move in the loaded puzzle position. |
| **Initial puzzle** | The puzzle index the session started with; `new` returns to it. |
| **`#[allow(unused_imports)]`** | The attribute on the `puzzle` import in `session.rs` at this slice, because `Puzzle` is not used until sub-chapter 07c. |

## Context

Sub-chapter 07a added `Puzzle`, `PuzzlePack`, and `PuzzleError` in
`src/puzzle.rs`, plus the `mini-pack.json` fixture. The pack parser
validates coordinates, stone counts, duplicates, and the solution-line
head. This sub-chapter wires that parsed pack into `SessionInit` and
`SessionCore` so the table can root a study session on a puzzle
position.

No command parsing changes here. `Hint`, `Reveal`, `NextPuzzle`, and
`PrevPuzzle` arrive in sub-chapter 07c. No startup flag changes here
either; `--puzzle` arrives in sub-chapter 07d.

## Intention

The learner enables this user-facing behavior:

1. `SessionInit::puzzle(pack_path, pack, index)` builds a hotseat
   session whose opening board is the puzzle position and whose side
   to move is the attacker.
2. The session stores the whole parsed pack, so later navigation can
   switch puzzles without reading the file again.
3. `SessionCore::puzzle()` returns the active `PuzzleStudy` context,
   or `None` for non-puzzle sessions.
4. `Command::New` resets to the *initially loaded* puzzle, not to an
   empty board.
5. Because the puzzle is just an opening board, seats, branching, the
   variation tree, and the chapter-06 overlays all work without any
   puzzle-aware code below the session layer.

## Specification as tests

The tests below are ordered along the execution path of loading a
puzzle and checking that the table accepts it as a root position.

### Constructor validation

* `puzzle_init_rejects_out_of_range_index` — `SessionInit::puzzle`
  returns `PuzzleError::IndexOutOfRange` when the requested index is
  not in the pack.

### Loading the position

* `puzzle_session_roots_at_puzzle_position_with_attacker_to_move` —
  after `SessionCore::new`, the board equals the puzzle board, the side
  to move is the attacker, and the puzzle context reports the correct
  index and pack length.

### Study flow on a puzzle root

* `puzzle_position_supports_full_study_flow` — the puzzle position
  accepts `play`, `back`, branching, `forward`, `BranchPoint`, and
  `var`, exactly like a normal opening board.

### Reset behavior

* `new_resets_to_the_initially_loaded_puzzle` — after a move dirties
  the tree, `Command::New` returns the session to the initially loaded
  puzzle root. Navigation to other puzzles is not implemented yet;
  `NextPuzzle` arrives in sub-chapter 07c.

## Walkthrough

### 1. The constructor validates the index

`SessionInit::puzzle` in `src/session.rs` calls `pack.get(index)`
before it builds the session. `PuzzlePack::get` returns
`PuzzleError::IndexOutOfRange` when the index is out of bounds. This
is the first error a caller can see, so it is the first test.

### 2. The constructor builds a hotseat opening

When the index is valid, `SessionInit::puzzle` clones the puzzle board
and builds a `SessionInit` with:

* `black` and `white` set to `Occupant::Human`;
* `default_config` set to `AiConfig::default()`;
* `opening` set to the puzzle board;
* `swap2` set to `false`;
* `puzzle` set to `Some(PuzzleStudy { pack_path, index, pack })`.

The puzzle position *is* the opening, so Swap2 is skipped.

### 3. `SessionCore::new` roots the tree on the puzzle board

`SessionCore::new` builds `History::from_opening(&init.opening)`. It
also copies `init.puzzle` into the core's own `puzzle` field. The core
can now answer `core.puzzle()` for the active context.

The side to move on the new board is the attacker because
`Board::from_position` was called with the attacker's color in the
pack parser. The test asserts this with `core.board().to_move()`.

### 4. The table tools work by construction

`puzzle_position_supports_full_study_flow` plays a move on the puzzle
root, takes it back, plays a different move to create a branch, moves
forward to the branch point, selects a variation, and asserts the
expected events. No puzzle-specific code is needed: the puzzle is just
an opening board with a `PuzzleStudy` label attached.

### 5. `new` resets to the initial puzzle

`SessionCore::reset` rebuilds the history from `self.init.opening` and
restores `self.puzzle` from `self.init.puzzle`. Because `init.puzzle`
stores the *initially loaded* puzzle, `Command::New` always returns to
that puzzle, even after sub-chapter 07c adds navigation to other
puzzles.

The test `new_resets_to_the_initially_loaded_puzzle` plays one move to
dirty the tree, runs `Command::New`, and asserts that the board and
puzzle index match the original puzzle.

### 6. The unused-import attribute is intentional

`src/session.rs` imports `Puzzle`, `PuzzleError`, and `PuzzlePack` with
`#[allow(unused_imports)]` at this slice. `Puzzle` is not referenced
until sub-chapter 07c adds the command handlers. The attribute keeps
the compiler quiet while the slice stays focused on enrollment.

## Low-level design

### [`gomoku/crates/cli/src/session.rs`](../../../../../gomoku/crates/cli/src/session.rs)

* Add the `puzzle` import from `crate::puzzle` with
  `#[allow(unused_imports)]`.
* Add the `puzzle: Option<PuzzleStudy>` field to `SessionInit`.
* Add the `PuzzleStudy` struct with `pack_path`, `index`, and `pack`.
* Add `SessionInit::puzzle(pack_path, pack, index)`.
* Set `puzzle: None` in `new_hotseat`, `new_hotseat_from_opening`,
  `versus_ai`, and `ai_vs_ai`.
* Add the `puzzle: Option<PuzzleStudy>` field to `SessionCore`.
* Copy `init.puzzle` in `SessionCore::new`.
* Add `SessionCore::puzzle()`.
* Restore `self.puzzle` from `self.init.puzzle` in `reset()`.

### [`gomoku/crates/cli/src/main.rs`](../../../../../gomoku/crates/cli/src/main.rs)

* Add `puzzle: None` to the `SessionInit` struct literal in
  `play_init`.
* Add `puzzle: None` to the `SessionInit` struct literal in
  `selfplay_init`.

### [`gomoku/crates/cli/tests/actor.rs`](../../../../../gomoku/crates/cli/tests/actor.rs)

* Add `puzzle: None` to every explicit `SessionInit` struct literal.

### [`gomoku/crates/cli/tests/study.rs`](../../../../../gomoku/crates/cli/tests/study.rs)

* Add `puzzle: None` to the explicit `SessionInit` struct literal.

### [`gomoku/crates/cli/tests/session.rs`](../../../../../gomoku/crates/cli/tests/session.rs)

* Import `cli::puzzle`.
* Add the `mini_pack()` helper that parses the fixture.
* Add the `puzzle_session(index)` helper that builds a puzzle session.
* Add the four puzzle session tests.

## TDD checklist

Work one test at a time, red to green.

From `gomoku/crates/cli/tests/session.rs`:

1. `puzzle_init_rejects_out_of_range_index`
2. `puzzle_session_roots_at_puzzle_position_with_attacker_to_move`
3. `puzzle_position_supports_full_study_flow`
4. `new_resets_to_the_initially_loaded_puzzle`

## Verification commands

From `gomoku/`:

```text
cargo test -p cli --test session
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Done when

* `SessionInit::puzzle` rejects an out-of-range index with
  `PuzzleError::IndexOutOfRange`.
* A puzzle session roots the variation tree on the puzzle board with
  the attacker to move.
* `SessionCore::puzzle()` returns the active `PuzzleStudy` context.
* `Command::New` resets to the initially loaded puzzle.
* All four tests above pass.
* The three project gates pass.

## References

* [Sub-chapter 07a — The pack parser](../a-pack-parser/pack-parser.md) —
  the `PuzzlePack` parser this sub-chapter wires into the session.
* [Sub-chapter 07c — Study commands](../c-study-commands/study-commands.md) —
  `hint`, `reveal`, `next`, and `prev`, which use the context added
  here.
* [Sub-chapter 07d — The puzzle flag](../d-puzzle-flag/puzzle-flag.md) —
  the `--puzzle` startup flag that calls `SessionInit::puzzle`.
* [Chapter 07 overview](../../07-positions-from-files-puzzles.md) —
  the chapter spec, including the "A puzzle is a position, not a mode"
  design note.
* [`gomoku/crates/cli/src/session.rs`](../../../../../gomoku/crates/cli/src/session.rs) —
  session initialization and reset.
* [`gomoku/crates/cli/tests/session.rs`](../../../../../gomoku/crates/cli/tests/session.rs) —
  the puzzle session integration tests.
