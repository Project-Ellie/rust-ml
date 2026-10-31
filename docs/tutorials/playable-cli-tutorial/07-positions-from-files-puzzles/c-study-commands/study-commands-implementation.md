# Sub-chapter 07c — The study commands: implementation plan

This document turns the walkthrough in `study-commands.md` into a
construction order. It names the files, enums, functions, and tests. It
states behavioral contracts in prose. It does not contain bodies.

Read `study-commands.md` first. Work one step at a time. Write tests
before code. Open `study-commands-solution.md` only if you are stuck for
more than twenty minutes, or after the sub-chapter is complete.

---

## Step 0 — `command.rs`: the study vocabulary

### 0.1 Extend `Command`

Add four variants after `New`:

```rust
/// Show the first move of the loaded puzzle's claimed line.
Hint,
/// Show the full claimed line of the loaded puzzle.
Reveal,
/// Load the next puzzle in the pack as a fresh tree root.
NextPuzzle,
/// Load the previous puzzle in the pack as a fresh tree root.
PrevPuzzle,
```

### 0.2 Extend `Event`

Add three variants:

```rust
/// The first move of the loaded puzzle's claimed line.
Hint { mv: Move },
/// The full claimed line of the loaded puzzle.
SolutionRevealed { line: Vec<(Move, Color)> },
/// A puzzle was loaded as a fresh variation tree root.
PuzzleLoaded { index: usize, count: usize, attacker: Color },
```

`PuzzleLoaded` fields are 0-based. The `Display` implementation prints
`puzzle {} of {count} · {attacker:?} to move` with the index offset by
1.

### 0.3 Extend `CommandError`

Add four variants:

```rust
#[error("No puzzle loaded — start with 'gomoku play --puzzle <pack.json>'")]
NoPuzzleLoaded,
#[error("Puzzle {} of {count} has no claimed line", index + 1)]
NoSolutionLine { index: usize, count: usize },
#[error("Already at the last puzzle ({count} of {count})")]
AtLastPuzzle { count: usize },
#[error("Already at the first puzzle (1 of {count})")]
AtFirstPuzzle { count: usize },
```

### 0.4 Add parser arms

In `parse`, after the `"new"` arm and before `"help"`, add four match
arms. Each arm calls `ensure_no_more` and returns the matching
`Command` variant.

### 0.5 Add the parser test

Add `parse_puzzle_commands` to the module's test block. It asserts that
`hint`, `REVEAL`, `next`, and `prev` parse correctly and that
`hint x` and `next 2` fail.

### 0.6 Gate

Run `cargo test -p cli --lib command::tests`.

---

## Step 1 — `session.rs`: dispatch and helpers

### 1.1 Clean up the import

Remove the `#[allow(unused_imports)]` above `use crate::puzzle::...`;
the import is now used by the new helpers.

### 1.2 Add dispatch arms

In `SessionCore::execute`, add four arms that route the new `Command`
variants to their executors:

* `Command::Hint => self.execute_hint()`
* `Command::Reveal => self.execute_reveal()`
* `Command::NextPuzzle => self.execute_puzzle_nav(1)`
* `Command::PrevPuzzle => self.execute_puzzle_nav(-1)`

### 1.3 Add `execute_hint`

Signature:

```rust
fn execute_hint(&self) -> Result<Vec<Event>, CommandError>
```

Contract:

* Call `self.puzzle_context()`.
* If `puzzle.solution` is empty, return
  `CommandError::NoSolutionLine { index, count }`.
* Otherwise return `vec![Event::Hint { mv: *mv }]` with the first move.

### 1.4 Add `execute_reveal`

Signature:

```rust
fn execute_reveal(&self) -> Result<Vec<Event>, CommandError>
```

Contract:

* Call `self.puzzle_context()`.
* If `puzzle.solution` is empty, return
  `CommandError::NoSolutionLine { index, count }`.
* Otherwise return `vec![Event::SolutionRevealed {
  line: puzzle.solution.clone() }]`. The clone is cheap because a line
  is short.

### 1.5 Add `execute_puzzle_nav`

Signature:

```rust
fn execute_puzzle_nav(&mut self, delta: i64) -> Result<Vec<Event>, CommandError>
```

Contract:

* Call `self.puzzle_context()`.
* Let `count = study.pack.items.len()`.
* Let `next = study.index as i64 + delta`.
* If `next < 0`, return `CommandError::AtFirstPuzzle { count }`.
* If `next >= count as i64`, return `CommandError::AtLastPuzzle { count }`.
* Let `index = next as usize`.
* Update `self.puzzle.index` to `index`.
* Replace `self.history` with `History::from_opening(&new_board)` where
  `new_board` is the board of the puzzle at `index`.
* Clear `self.search_info`.
* Leave seats, overrides, and AI configuration untouched.
* Return `vec![Event::PuzzleLoaded { index, count, attacker }]`.

### 1.6 Add `puzzle_context`

Signature:

```rust
fn puzzle_context(&self) -> Result<(&PuzzleStudy, &Puzzle), CommandError>
```

Contract:

* Return `Err(CommandError::NoPuzzleLoaded)` if `self.puzzle` is `None`.
* Otherwise return the study and a reference to
  `study.pack.items[study.index]`. The index is valid by construction.

### 1.7 Strengthen the reset test

In `tests/session.rs`, update `new_resets_to_the_initially_loaded_puzzle`
to:

* create a puzzle session at index 0;
* call `Command::NextPuzzle` once;
* play a move;
* call `Command::New`;
* assert `Event::Reset` is emitted;
* assert the puzzle index is 0 and the board matches the initial puzzle.

### 1.8 Gate

Run `cargo test -p cli --test session`.

---

## Step 2 — `tests/session.rs`: the six new tests

### 2.1 Tests to add

* `hint_emits_first_solution_move`
* `hint_errors_when_puzzle_has_no_line`
* `reveal_emits_full_claimed_line`
* `next_and_prev_load_a_fresh_root_and_report_position`
* `next_prev_clamp_at_pack_ends`
* `puzzle_commands_error_without_a_pack`

### 2.2 Gate

Run `cargo test -p cli --test session`.

---

## Step 3 — `sessiondemo.rs`: flags, comments, and status

### 3.1 Add help lines

Add the following lines to `HELP` after the `info` line:

```text
  hint             show the first move of the puzzle's line
  reveal           show the puzzle's full claimed line
  next / prev      load the next/previous puzzle (clamps at the ends)
```

### 3.2 Add `parse_flag_value`

Signature:

```rust
fn parse_flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str>
```

Contract: find `flag` in `args` and return the next argument, if any.

### 3.3 Add `build_init`

Signature:

```rust
fn build_init(args: &[String]) -> SessionInit
```

Contract:

* Read `--seed` as before.
* If `--puzzle` is present, read the file, parse it with
  `PuzzlePack::from_json`, read `--index` (default 0), build the init
  with `SessionInit::puzzle`, and merge the seed.
* If `--puzzle` is absent, build a hotseat init with the seed.
* Exit 1 with a message on unreadable file, invalid JSON, or invalid
  puzzle index.

### 3.4 Skip comments in scripts

In `process_line`, ignore lines whose trimmed text starts with `#`.

### 3.5 Print puzzle status

In `print_state`, after the status line, if `core.puzzle()` is `Some`,
print:

```text
puzzle <i+1> of <count>  attacker <Color>
```

using 1-based display.

### 3.6 Gate

Run the capstone command from `gomoku/`:

```text
cargo run -q -p cli --bin sessiondemo -- --puzzle crates/cli/tests/fixtures/mini-pack.json --script crates/cli/puzzle_demo.txt
```

---

## Step 4 — `testkit`: event classification

### 4.1 Add event type arms

In `crates/testkit/src/lib.rs`, extend `event_type` with three arms:

* `Event::Hint { .. } => "Hint"`
* `Event::SolutionRevealed { .. } => "SolutionRevealed"`
* `Event::PuzzleLoaded { .. } => "PuzzleLoaded"`

### 4.2 Gate

Run `cargo test -p cli`.

---

## Step 5 — Project gates

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

---

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `src/command.rs` | `Command::Hint/Reveal/NextPuzzle/PrevPuzzle` | variants added |
| `src/command.rs` | `Event::Hint/SolutionRevealed/PuzzleLoaded` | variants added, `Display` 1-based for `PuzzleLoaded` |
| `src/command.rs` | `CommandError::NoPuzzleLoaded/NoSolutionLine/AtLastPuzzle/AtFirstPuzzle` | variants added |
| `src/command.rs` | parser arms | `hint`/`reveal`/`next`/`prev`, each rejects extra tokens |
| `src/command.rs` | `parse_puzzle_commands` | passes |
| `src/session.rs` | import cleanup | `#[allow(unused_imports)]` removed |
| `src/session.rs` | `execute` dispatch | four new arms route to helpers |
| `src/session.rs` | `execute_hint` | emits `Hint` or `NoSolutionLine` |
| `src/session.rs` | `execute_reveal` | emits `SolutionRevealed` or `NoSolutionLine` |
| `src/session.rs` | `execute_puzzle_nav` | clamps, rebuilds history, clears search info, preserves seats |
| `src/session.rs` | `puzzle_context` | returns `NoPuzzleLoaded` when absent |
| `tests/session.rs` | six new tests | pass |
| `tests/session.rs` | reset test | strengthened to use `NextPuzzle` |
| `src/bin/sessiondemo.rs` | `HELP` | lists study commands |
| `src/bin/sessiondemo.rs` | `parse_flag_value` | reads `--puzzle` and `--index` |
| `src/bin/sessiondemo.rs` | `build_init` | builds puzzle or hotseat init, exits on bad data |
| `src/bin/sessiondemo.rs` | `process_line` | skips `#` comments |
| `src/bin/sessiondemo.rs` | `print_state` | prints puzzle status line |
| `crates/testkit/src/lib.rs` | `event_type` | classifies new events |
