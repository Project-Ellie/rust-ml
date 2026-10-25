# Sub-chapter 05a — Refactor: notation, the repl core, and the test kit

Status: IMPLEMENTED. This sub-chapter moves code. It does not change
what the code does.

## Abstract

Before the study table gets its screen, you remove three duplications
that the first four chapters accumulated. Algebraic conversion exists
in three places: a private parser in `command.rs`, a formatter in
`fmt.rs`, and a second formatter plus eight hardcoded square
constructors in historydemo. The sessiondemo binary contains the line
interpreter, where no test can reach it. Four test files each define
their own copies of the same helpers. This sub-chapter extracts all
three into one place each: `src/notation.rs`, `src/repl.rs`, and
`tests/common/mod.rs`.

The sub-chapter teaches one skill: refactoring without behavior change.
The proof is not an argument about the code. The proof is the unchanged
test suite plus two demo transcripts that stay byte-identical.

## Glossary

| Term | Definition |
|------|------------|
| **Extraction** | Moving code into its own module without changing what the code does. |
| **Canonical implementation** | The one place where a piece of logic lives. Every other use delegates to it. |
| **Line interpreter** | The function that takes one input line and a session core, and produces output lines plus a quit flag. |
| **`LineOutcome`** | The return type of the line interpreter: the output lines and the quit flag. |
| **Test kit** | `tests/common/mod.rs`: the helpers every integration test file shares. |
| **Golden transcript** | The captured output of a scripted demo run, used as a before/after reference. |

## Context

Read [the design paper](../00-design.md) first. It explains why chapter
05 is split into sub-chapters, and it assigns this one: section
"a — Refactor: notation, the repl core, and the test kit".

Three facts from the current code motivate the work:

1. `command.rs` has a private `parse_algebraic`; `fmt.rs` has
   `format_move`; historydemo has its own `notation()` helper and
   eight hardcoded constructors (`h8()`, `h7()`, ...). The design
   paper records where such duplication leads: the reference build
   grew one more parser whose error text differed from the one in
   `command.rs`. Copies diverge; one implementation cannot.
2. sessiondemo contains `process_line`, `print_state`, and
   `print_search_info`. These functions call `println!` directly. A
   function that prints can be observed only through a terminal. The
   line-mode fallback in sub-chapter e needs the same logic, so the
   logic must move into the library and return its text.
3. `tests/actor.rs`, `tests/session.rs`, `tests/study.rs`, and
   `tests/history.rs` each define `mv`; three of them define `play`;
   two define an AI-config builder under two different names
   (`ai_config_tactics`, `default_ai_config`) with the same body. The
   unit-test modules in `src/` each define their own `mv` too.

The design paper's budget applies: no source file in this chapter
exceeds approximately 250 lines. If a file grows past the budget, it
contains more than one concern; split it.

## Intention

1. Create `src/notation.rs` with the canonical conversion between
   algebraic text (`h8`) and `Move`, plus its unit tests.
2. Make `command.rs` delegate to it and delete the private copy.
3. Make `fmt::format_move` delegate to `notation::algebraic`, so one
   implementation formats every move.
4. Point historydemo at the notation module and delete its local
   helpers.
5. Create `src/repl.rs`: move `process_line`, `print_state`, and
   `print_search_info` out of sessiondemo, and change the printing
   helpers to return lines. sessiondemo keeps `main`, argument
   handling, the input loop, and the printing of the returned lines.
6. Create `tests/common/mod.rs` with the shared helpers and make all
   four test files use it.
7. Create `src/testkit.rs` with the unit-test `mv` and import it in
   the unit-test modules instead of defining a local copy. Tests live
   in two worlds that cannot see each other: unit tests
   (`#[cfg(test)] mod tests` inside `src/`) share helpers through
   `src/testkit.rs`; integration tests (the binaries in `tests/`)
   share helpers through `tests/common/mod.rs`.
8. Prove that nothing changed: the full test suite passes, and the two
   scripted demo runs produce byte-identical output.

## Mental mapping

### Refactoring means: the behavior is the contract

A refactoring changes the shape of the code and nothing else. For this
codebase, behavior has three observable surfaces:

* the test suite, which asserts on event sequences and on error text;
* the error messages themselves, which users read and tests match
  verbatim;
* the demo transcripts, which capture exactly what a user sees.

Preserve all three. The sections below describe where each surface is
at risk and how the extraction protects it.

### Why the helpers return lines

`print_state` and `print_search_info` used to call `println!`. That
made them untestable and single-use: their text could go only to
stdout, only in this binary.

The extracted versions return `Vec<String>`. Three consequences:

1. **Tests can assert on the text.** A unit test calls the function
   and compares the returned lines. No terminal, no capture harness.
2. **The caller owns the output port.** sessiondemo prints the lines.
   The line-mode fallback screen in sub-chapter e will route the same
   lines through its own display path. The repl code serves both
   because it decided nothing about output.
3. **Byte-identical output stays provable.** Printing each returned
   line with `println!` reproduces the old byte stream exactly,
   including the blank line after the board and the blank line after
   the help text. The two functions that needed care are documented in
   the code comments.

The same argument applies to `process_line`. It returns a
`LineOutcome`, so the input loop in sessiondemo is the only code that
prints, and the `interactive` flag keeps its exact old meaning: it
controls only whether `quit` is acknowledged with `Bye.`.

### One test kit, compiled four times

Cargo builds every file in `tests/` as a separate binary. A file at
`tests/common/mod.rs` is not a test target; it is a module that test
files import with `mod common;`. Each test binary compiles its own copy
of the module.

That has one sharp edge: no test binary uses every helper, so every
binary would warn about dead code, and `clippy -D warnings` turns the
warning into an error. The kit carries a module-level
`#![allow(dead_code)]` for exactly this reason. The allowance is
documented in the module, because an unexplained `allow` reads as an
accident.

The kit also ends the config-builder duplication. There is one builder,
`fast_tactics_config`, with one comment that records the deliberate
divergence from `AiConfig::default()`: 200 simulations instead of 400,
to keep the test suite fast. Before the kit, two names and two copies
existed; the reason for the number lived nowhere.

### Error text is an interface

`NotationError` is a small `thiserror` enum. Each variant holds the
data its message needs, and the `#[error("...")]` attribute keeps the
message next to the variant. The messages reproduce the old strings
verbatim, because tests assert on them and users read them. When an
error message is part of the user interface, treat it as a contract:
change it only together with its tests.

`command.rs` flattens the `NotationError` into `CommandError::Parse`
with its `Display` text. The parser's public behavior does not change;
only the owner of the logic changes.

## Low-level design

### `src/notation.rs` (new)

```rust
pub fn parse_algebraic(token: &str) -> Result<Move, NotationError>;
pub fn algebraic(mv: Move) -> String;
```

`parse_algebraic` trims and lowercases the token, then checks in order:
length, column letter, row digits, board bounds. Error messages quote
the token as passed in. The error type:

| Variant | Condition | Message |
|---------|-----------|---------|
| `NotAlgebraic(String)` | fewer than 2 characters | `Expected algebraic square like 'h8', got '{0}'` |
| `BadColumn { letter, token }` | first char not in `a`–`o` | `Expected column letter a-o, got '{letter}' in '{token}'` |
| `BadRow { digits, token }` | row part is not a number | `Expected row number 1-15, got '{digits}' in '{token}'` |
| `OffBoard(String)` | parsed, but outside 15×15 | `square '{0}' is off the board` |

`algebraic` is the inverse: column 0 prints as `a`, row 0 prints as 1.

### `src/repl.rs` (new)

```rust
pub const HELP: &str;

pub struct LineOutcome {
    pub lines: Vec<String>,
    pub quit: bool,
}

pub fn process_line(line: &str, core: &mut SessionCore, interactive: bool) -> LineOutcome;
pub fn print_state(core: &SessionCore, header: &str) -> Vec<String>;
pub fn print_search_info(core: &SessionCore) -> Vec<String>;
```

The logic is the sessiondemo logic, moved. Three details preserve the
byte stream:

* `HELP` ends with a newline, and the old code printed it with
  `println!`. The returned lines are `HELP.lines()` plus one explicit
  empty line.
* The rendered board is one `String` with embedded newlines. It stays
  one element of the vector.
* `print_state` ends its vector with one empty line, the old trailing
  `println!()`.

The `info` command stays a repl-level special case: it reads search
state and changes nothing, so it does not go through
`command::parse`.

### `src/testkit.rs` (new)

```rust
pub(crate) fn mv(r: u8, c: u8) -> Move;
```

The unit-test counterpart of the integration kit, gated `#[cfg(test)]`
and registered in `src/lib.rs`. It starts with one helper: the move
constructor that the unit-test modules each define locally today.

### `tests/common/mod.rs` (new)

```rust
pub fn mv(r: u8, c: u8) -> Move;
pub fn play(core: &mut SessionCore, r: u8, c: u8) -> Vec<Event>;
pub fn step(core: &mut SessionCore) -> Vec<Event>;
pub fn event_type(ev: &Event) -> &'static str;
pub fn event_types(events: &[Event]) -> Vec<&'static str>;
pub fn fast_tactics_config() -> AiConfig;
```

Plus the module-level `#![allow(dead_code)]` with its comment.
`tests/history.rs` drives `History` directly, not `SessionCore`; it
imports only `mv`.

### Changes in existing files

* `src/command.rs`: the private `parse_algebraic` is deleted. Both
  call sites use a new private `parse_move` that delegates to
  `notation::parse_algebraic` and maps the error into
  `CommandError::Parse`.
* `src/fmt.rs`: `format_move` delegates to `notation::algebraic`.
* `src/lib.rs`: `pub mod notation;` and `pub mod repl;`, plus the
  `#[cfg(test)] mod testkit;` registration.
* `src/bin/historydemo.rs`: one `sq()` helper parses demo squares
  through `notation::parse_algebraic`; formatting uses
  `notation::algebraic`. The local `notation()` and the eight
  constructors are deleted.
* `src/bin/sessiondemo.rs`: keeps `main`, `parse_seed`, the two input
  loops, and a `print_lines` helper. Everything else moved to `repl`.
* `tests/actor.rs`, `tests/session.rs`, `tests/study.rs`,
  `tests/history.rs`: `mod common;` plus the imports they need; the
  local copies are deleted.

## Solution (opt-in)

The added and changed code, as annotated snippets with line numbers, is
in [refactor-solution.md](refactor-solution.md). Open it only if you
have been stuck for more than twenty minutes, or after you have
finished the sub-chapter and want to compare your work.

## TDD checklist

A refactoring has no red phase: the tests exist already, and they must
stay green throughout. The rhythm here is baseline, move, prove.

Run from `gomoku/`.

1. **Baseline.** Run the three gates on the untouched code. Capture
   the golden transcripts:

   ```text
   cargo run -q -p cli --bin sessiondemo -- --script crates/cli/demo_script.txt > /tmp/sessiondemo_before.txt 2>&1
   cargo run -q -p cli --bin sessiondemo -- --script crates/cli/study_demo.txt --seed 7 > /tmp/studydemo_before.txt 2>&1
   ```

2. **Extract notation.** Create `src/notation.rs` with the error enum,
   both functions, and unit tests: round-trips for `a1`, `o15`, `h8`;
   case-insensitivity (`H8`); one test per error variant that asserts
   the exact message. Wire `command.rs`, `fmt.rs`, historydemo, and
   `lib.rs`. Run the gates.
3. **Extract the repl.** Create `src/repl.rs`, slim sessiondemo down
   to its shell, wire `lib.rs`. Run the gates.
4. **Unify the test kits.** Create `tests/common/mod.rs` and
   `src/testkit.rs`, convert the four integration test files and the
   unit-test modules with a local `mv`. Run the gates.
5. **Prove byte-identical behavior.** Capture the transcripts again
   and diff:

   ```text
   cargo run -q -p cli --bin sessiondemo -- --script crates/cli/demo_script.txt > /tmp/sessiondemo_after.txt 2>&1
   cargo run -q -p cli --bin sessiondemo -- --script crates/cli/study_demo.txt --seed 7 > /tmp/studydemo_after.txt 2>&1
   diff /tmp/sessiondemo_before.txt /tmp/sessiondemo_after.txt
   diff /tmp/studydemo_before.txt /tmp/studydemo_after.txt
   ```

   Both diffs must be empty.

## Verification commands

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Plus the transcript capture and diff from the checklist.

## Done when

* `src/notation.rs`, `src/repl.rs`, `tests/common/mod.rs`, and
  `src/testkit.rs` exist with the contracts above.
* `command.rs`, `fmt.rs`, `lib.rs`, both demo binaries, and all four
  test files delegate to the new modules.
* No source file exceeds approximately 250 lines. (Result here:
  `notation.rs` 179, `repl.rs` 175.)
* All three gates pass.
* Both transcript diffs are empty.

Next: sub-chapter b — the study-table renderer.

## References

* [00-design.md](../00-design.md) — the contract for all six
  sub-chapters, including the size budget and the commenting standard.
* [Chapter 02 — The command language](../02-the-command-language.md) —
  where sessiondemo and the command language came from.
* [README.md](../README.md) — chapter map and tutorial conventions.
