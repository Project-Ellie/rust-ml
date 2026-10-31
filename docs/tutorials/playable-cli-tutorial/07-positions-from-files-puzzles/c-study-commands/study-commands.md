# Sub-chapter 07c — The study commands

## Abstract

This sub-chapter adds the four study commands to the playable CLI
command language: `hint`, `reveal`, `next`, and `prev`. A puzzle is now
loaded (sub-chapter 07b) and a pack is parsed (sub-chapter 07a); here
you let the user ask for the first move of the claimed line, show the
whole claimed line, and move between puzzles in the pack. Navigation
clamps at the ends instead of wrapping. Unlabelled puzzles produce a
typed `NoSolutionLine` error instead of a crash. The demo binary gains
`--puzzle` / `--index` flags, `#`-comment skipping, and a status line so
the capstone script can run end-to-end.

## Glossary

| Term | Definition |
|------|------------|
| **Study command** | One of `hint`, `reveal`, `next`, or `prev`. |
| **Hint** | The command that shows the first move of the loaded puzzle's claimed line. |
| **Reveal** | The command that shows the full claimed line. |
| **Clamp** | `next` / `prev` stop at the pack ends with a typed error instead of wrapping around. |
| **`Command::Hint`** | The parsed command for a hint request. |
| **`Command::Reveal`** | The parsed command for a reveal request. |
| **`Command::NextPuzzle`** | The parsed command to load the next puzzle. |
| **`Command::PrevPuzzle`** | The parsed command to load the previous puzzle. |
| **`Event::Hint`** | Emitted when `hint` succeeds; carries the first move. |
| **`Event::SolutionRevealed`** | Emitted when `reveal` succeeds; carries the full line. |
| **`Event::PuzzleLoaded`** | Emitted when `next` / `prev` load a new puzzle; fields are 0-based, display is 1-based. |
| **`NoPuzzleLoaded`** | `CommandError` when a study command runs without a pack. |
| **`NoSolutionLine`** | `CommandError` when the loaded puzzle has no claimed line. |
| **`AtFirstPuzzle`** | `CommandError` for `prev` at puzzle 0. |
| **`AtLastPuzzle`** | `CommandError` for `next` at the last puzzle. |
| **`puzzle_context`** | The `SessionCore` helper that returns the current `PuzzleStudy` and `Puzzle`. |
| **`execute_puzzle_nav`** | The `SessionCore` helper that clamps and loads a new puzzle root. |

## Context

Read [the playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md)
first. Sub-chapter 07a built the pack parser (`PuzzlePack::from_json`)
and sub-chapter 07b built the puzzle session constructor
(`SessionInit::puzzle`) and the `PuzzleStudy` context. This sub-chapter
adds the command vocabulary and the session dispatch that turn a loaded
puzzle into an interactive study flow.

The starting point is sub-chapter 07b. `SessionCore::execute` already
routes `Command` variants to executor methods. You add four new
variants, four new executors, and the events they emit.

## Intention

The learner enables this user-facing behavior:

1. Type `hint` to see the first move of the loaded puzzle's claimed
   line.
2. Type `reveal` to see the full claimed line.
3. Type `next` / `prev` to load the next or previous puzzle as a fresh
   variation tree root.
4. Get a typed error when navigation would move past the pack ends, so
   a front-end can grey out the button.
5. Get a typed error when a puzzle has no claimed line, so unlabelled
   packs are data, not crashes.
6. Run the demo binary with `--puzzle crates/cli/tests/fixtures/mini-pack.json`
   and the `puzzle_demo.txt` script to reproduce the capstone frames.

## Specification as tests

The tests below are ordered along the execution path of a study command
from typed word through parser, command, executor, event, and the demo
output.

### Parser

* `parse_puzzle_commands` — the words `hint`, `REVEAL`, `next`, and
  `prev` parse to `Command::Hint`, `Command::Reveal`,
  `Command::NextPuzzle`, and `Command::PrevPuzzle`; extra tokens are
  rejected.

### Session dispatch

* `hint_emits_first_solution_move` — `Command::Hint` on puzzle 0 emits
  `Event::Hint { mv: h8 }`.
* `hint_errors_when_puzzle_has_no_line` — `Command::Hint` on a pack with
  an empty `l` field returns `CommandError::NoSolutionLine`.
* `reveal_emits_full_claimed_line` — `Command::Reveal` on puzzle 2 emits
  `Event::SolutionRevealed` with the three-move line.
* `next_and_prev_load_a_fresh_root_and_report_position` — `next` after a
  dirty tree emits `Event::PuzzleLoaded { index: 1, count: 3, attacker:
  White }` and resets the board; `prev` returns to puzzle 0.
* `next_prev_clamp_at_pack_ends` — `prev` at puzzle 0 and `next` at the
  last puzzle return `AtFirstPuzzle` / `AtLastPuzzle`; the current
  puzzle stays loaded.
* `puzzle_commands_error_without_a_pack` — `hint`, `reveal`, `next`, and
  `prev` on a hotseat session return `NoPuzzleLoaded`.
* `new_resets_to_the_initially_loaded_puzzle` — after `next` and a move,
  `Command::New` returns to the initially loaded puzzle (this is a
  strengthening of the test from 07b).

## Walkthrough

### 1. The user types a word

`parse` in `command.rs` already lowercases the input and splits it into
tokens. Add four new match arms:

* `hint` → `Command::Hint`
* `reveal` → `Command::Reveal`
* `next` → `Command::NextPuzzle`
* `prev` → `Command::PrevPuzzle`

Each arm calls `ensure_no_more` so that `hint x`, `next 2`, and similar
trailers produce a parse error. The commands are case-insensitive
because `parse` lowercases the trimmed input before matching.

### 2. The parser returns a `Command`

The new variants travel through the same path as every other command:
text → `Command` → `SessionCore::execute` → events. There is no special
input-mode handling for study commands; the line interpreter calls
`command::parse` directly.

### 3. `SessionCore::execute` dispatches the variants

Add four new arms to the `match cmd` block in `execute`:

* `Command::Hint` → `self.execute_hint()`
* `Command::Reveal` → `self.execute_reveal()`
* `Command::NextPuzzle` → `self.execute_puzzle_nav(1)`
* `Command::PrevPuzzle` → `self.execute_puzzle_nav(-1)`

`execute_hint` and `execute_reveal` are `&self` methods: they only read
state and emit events. `execute_puzzle_nav` is `&mut self` because it
replaces the variation tree root.

### 4. `puzzle_context` checks for a loaded pack

All three executors start with `self.puzzle_context()`. That helper
returns `NoPuzzleLoaded` when `self.puzzle` is `None`. This single check
makes every study command fail the same way when the session started
without a pack.

### 5. `execute_hint` returns the first claimed move

`execute_hint` reads through `puzzle_context`, takes the first element
of `puzzle.solution`, and returns `Event::Hint { mv: *mv }`. If the
solution is empty it returns `NoSolutionLine { index, count }`. The
board is not changed.

### 6. `execute_reveal` returns the full claimed line

`execute_reveal` checks whether the solution is empty and, if not,
clones it into `Event::SolutionRevealed { line: puzzle.solution.clone()
}`. The clone is cheap because a line is at most a few moves. The board
is not changed.

### 7. `execute_puzzle_nav` clamps at the ends and rebuilds the tree

`execute_puzzle_nav(delta)` computes `next = index + delta`. If `next`
is below 0 it returns `AtFirstPuzzle { count }`; if `next` is at or
above `count` it returns `AtLastPuzzle { count }`. On success it:

* updates `self.puzzle.index` to the new index;
* rebuilds the history from the new puzzle's board with
  `History::from_opening`;
* clears `search_info` so that stale overlays from the previous puzzle
  do not leak;
* leaves seats and AI configuration untouched;
* emits `Event::PuzzleLoaded { index, count, attacker }`.

The event fields stay 0-based for machine consumers; the `Display`
implementation adds 1 to the index so a human sees
`puzzle 2 of 3 · White to move`.

### 8. `new` resets to the initially loaded puzzle

`reset` already restores `self.puzzle` from `self.init.puzzle`. Because
`SessionInit::puzzle` stores the initial index, `Command::New` returns
to that puzzle. The test `new_resets_to_the_initially_loaded_puzzle`
is strengthened to navigate with `NextPuzzle` before playing a move and
then `new`; this proves that reset uses `init.puzzle`, not the current
puzzle.

### 9. The demo binary loads packs and skips comments

`src/bin/sessiondemo.rs` gains:

* `parse_flag_value` to read `--puzzle <path>` and `--index <N>`.
* `build_init` that reads the file, parses it with
  `PuzzlePack::from_json`, builds the puzzle init, and exits 1 on an
  unreadable or invalid pack.
* `#`-comment skipping in `process_line` so the demo script can carry
  explanatory comments.
* A puzzle status line in `print_state`:
  `puzzle <i+1> of <count>  attacker <Color>`.
* Three new help lines for `hint`, `reveal`, and `next / prev`.

### 10. `testkit` classifies the new events

`crates/testkit/src/lib.rs` adds three arms to `event_type` so that
integration tests can assert the variant names of `Hint`,
`SolutionRevealed`, and `PuzzleLoaded`. This is plumbing, not pedagogy,
but it keeps transcript-style assertions consistent across the
workspace.

## Low-level design

### [`gomoku/crates/cli/src/command.rs`](../../../../../gomoku/crates/cli/src/command.rs)

* Add `Hint`, `Reveal`, `NextPuzzle`, and `PrevPuzzle` to `Command`.
* Add `Hint`, `SolutionRevealed`, and `PuzzleLoaded` to `Event`, with
  0-based fields and a 1-based `Display` for `PuzzleLoaded`.
* Add `NoPuzzleLoaded`, `NoSolutionLine`, `AtLastPuzzle`, and
  `AtFirstPuzzle` to `CommandError`.
* Add parser arms for `hint`, `reveal`, `next`, and `prev`, each
  rejecting extra tokens.
* Add the `parse_puzzle_commands` unit test.

### [`gomoku/crates/cli/src/session.rs`](../../../../../gomoku/crates/cli/src/session.rs)

* Remove the `#[allow(unused_imports)]` left by sub-chapter 07b;
  `PuzzlePack` is now used.
* Add dispatch arms for the four new `Command` variants in
  `SessionCore::execute`.
* Add `execute_hint`, `execute_reveal`, and `execute_puzzle_nav`.
* Add `puzzle_context`.

### [`gomoku/crates/cli/src/bin/sessiondemo.rs`](../../../../../gomoku/crates/cli/src/bin/sessiondemo.rs)

* Add `hint`, `reveal`, and `next / prev` to `HELP`.
* Add `parse_flag_value`.
* Add `build_init` with `--puzzle` and `--index` support.
* Skip lines that start with `#` in `process_line`.
* Print the puzzle status line in `print_state` when a pack is loaded.

### [`gomoku/crates/cli/tests/session.rs`](../../../../../gomoku/crates/cli/tests/session.rs)

* Add the six session tests listed in the specification.
* Strengthen `new_resets_to_the_initially_loaded_puzzle` by navigating
  with `NextPuzzle` before playing a move.

### [`gomoku/crates/testkit/src/lib.rs`](../../../../../gomoku/crates/testkit/src/lib.rs)

* Add `Hint`, `SolutionRevealed`, and `PuzzleLoaded` arms to
  `event_type`.

## TDD checklist

Work one test at a time, red to green.

From `gomoku/crates/cli/src/command.rs`:

1. `parse_puzzle_commands`

From `gomoku/crates/cli/tests/session.rs`:

2. `hint_emits_first_solution_move`
3. `hint_errors_when_puzzle_has_no_line`
4. `reveal_emits_full_claimed_line`
5. `next_and_prev_load_a_fresh_root_and_report_position`
6. `next_prev_clamp_at_pack_ends`
7. `puzzle_commands_error_without_a_pack`
8. `new_resets_to_the_initially_loaded_puzzle` (strengthened)

## Verification commands

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Capstone: hint, wrong try, take-back, reveal, next

From `gomoku/`:

```text
cargo run -q -p cli --bin sessiondemo -- --puzzle crates/cli/tests/fixtures/mini-pack.json --script crates/cli/puzzle_demo.txt
```

The session opens on puzzle 1 of 3: Black to move, a vertical four on
column h:

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

`hint` shows the first move of the claimed line:

```text
  hint: the line starts at h8
ply 0  status Ongoing  black Human  white Human
puzzle 1 of 3  attacker Black
```

The wrong try `i9` is legal, so it is played as an ordinary move:

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
```

`back` takes it back. The branch stays in the tree:

```text
  Cursor moved to ply 0
ply 0  status Ongoing  black Human  white Human
puzzle 1 of 3  attacker Black
branches: 0:i9
```

`reveal` shows the full claimed line:

```text
  solution: h8 Black
ply 0  status Ongoing  black Human  white Human
puzzle 1 of 3  attacker Black
branches: 0:i9
```

`next` loads puzzle 2 as a fresh root — White to move, a horizontal
four on row 5:

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
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  X
```

The final `hint` shows the new puzzle's first move:

```text
  hint: the line starts at i5
ply 0  status Ongoing  black Human  white Human
puzzle 2 of 3  attacker White
```

## Done when

* `hint`, `reveal`, `next`, and `prev` parse and execute correctly.
* Navigation clamps at both ends with `AtFirstPuzzle` / `AtLastPuzzle`.
* Unlabelled puzzles return `NoSolutionLine` instead of panicking.
* `new` resets to the initially loaded puzzle, not the current one.
* The `sessiondemo --puzzle` capstone reproduces the frames above.
* All tests in the TDD checklist pass.
* The three project gates pass.

## References

* [Playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md)
  — core/shell doctrine and the positions-from-files amendment.
* [Chapter 07 source spec](../../07-positions-from-files-puzzles.md) —
  pack format, clamp decision, unlabelled packs, and capstone frames.
* [Sub-chapter 07a — The pack parser](../a-pack-parser/pack-parser.md)
* [Sub-chapter 07b — The puzzle session](../b-puzzle-session/puzzle-session.md)
* [Sub-chapter 07d — The puzzle flag](../d-puzzle-flag/puzzle-flag.md)
* [`gomoku/crates/cli/src/command.rs`](../../../../../gomoku/crates/cli/src/command.rs)
* [`gomoku/crates/cli/src/session.rs`](../../../../../gomoku/crates/cli/src/session.rs)
* [`gomoku/crates/cli/src/bin/sessiondemo.rs`](../../../../../gomoku/crates/cli/src/bin/sessiondemo.rs)
* [`gomoku/crates/cli/tests/session.rs`](../../../../../gomoku/crates/cli/tests/session.rs)
* [`gomoku/crates/testkit/src/lib.rs`](../../../../../gomoku/crates/testkit/src/lib.rs)
