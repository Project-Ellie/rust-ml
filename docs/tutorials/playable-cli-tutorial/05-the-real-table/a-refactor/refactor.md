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
their own copies of the same helpers, and the unit-test modules in
`src/` each define one more copy. This sub-chapter extracts all
three into one place each: `src/notation.rs`, `src/repl.rs`, and one
shared test-kit crate, `crates/testkit`.

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
| **Test kit** | The `testkit` crate (`crates/testkit`): the helpers every unit test and integration test share. |
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
   unit-test modules in `src/` each define their own `mv` too. A
   correction to one copy does not reach the others.

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
6. Create the `crates/testkit` workspace crate: a manifest with path
   dependencies on `engine` and `cli`, a `src/lib.rs` with the shared
   helpers as plain `pub fn`, and a dev-dependency from cli on the
   kit. Unit tests and integration tests both import the kit with
   `use testkit::{...}`.
7. Prove that nothing changed: the full test suite passes, and the two
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

### The test kit is a crate

Cargo compiles tests in two worlds. Unit tests (`#[cfg(test)] mod
tests` inside `src/`) build together with the crate under test.
Integration tests (the binaries under `tests/`) build against the
crate's public API. A module inside `cli` cannot serve both worlds:
`#[cfg(test)]` gates code to the defining crate's own test build, and
the attribute never crosses a crate boundary. A `#[cfg(test)]` helper
inside the kit would vanish for every consumer — the compiler reports
E0432, unresolved import. So the kit carries no `#[cfg(test)]` at all;
every helper is a plain `pub fn`.

The kit is therefore its own workspace crate, `crates/testkit`, and
cli dev-depends on it. The dev-dependency edge is the shipping
boundary: dev-dependencies are available to unit tests, integration
tests, examples, and benches, and never to normal builds. No attribute
provides this property; the edge does. Cargo also permits the
resulting cycle: `testkit` depends on `cli` normally, because the
helpers take cli types, while cli dev-depends on `testkit`.

One limitation follows. A kit helper whose signature mentions cli
types compiles against the cli rlib the kit links. In cli's own
unit-test build, rustc sees two versions of crate cli — the rlib and
the crate under test — and the types do not unify (E0308). Those
helpers (`play`, `step`, `event_type`, `event_types`,
`fast_tactics_config`) are callable from the integration tests only.
Helpers over engine types (`mv`) work everywhere. Consequence: the `src/session.rs` unit tests keep small
local `play`/`play_some` wrappers, with a `NOTE` comment that records
the reason. This is the one accepted exception to the
no-local-copies rule.

The crate needs no `#![allow(dead_code)]`. The public items of a
library crate are not dead code, whether or not one consumer calls
them.

The kit also ends the config-builder duplication. There is one
builder, `fast_tactics_config`, with one comment that records the
deliberate divergence from `AiConfig::default()`: 200 simulations
instead of 400, to keep the test suite fast. Before the kit, two
names and two copies existed; the reason for the number lived
nowhere.

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

### `crates/testkit` (new)

A new workspace member. The manifest carries path dependencies on
`engine` and `cli`; the workspace `members` list gains
`crates/testkit`; cli's manifest gains the dev-dependency edge:

```toml
[dev-dependencies]
testkit = { path = "../testkit" }
```

`src/lib.rs` holds the shared helpers, all plain `pub fn`:

```rust
pub fn mv(r: u8, c: u8) -> Move;
pub fn play(core: &mut SessionCore, r: u8, c: u8) -> Vec<Event>;
pub fn step(core: &mut SessionCore) -> Vec<Event>;
pub fn event_type(ev: &Event) -> &'static str;
pub fn event_types(events: &[Event]) -> Vec<&'static str>;
pub fn fast_tactics_config() -> AiConfig;
```

No `#[cfg(test)]` anywhere in the crate: the attribute gates code to
the defining crate's own test build and never crosses a crate
boundary. The dev-dependency edge — not an attribute — keeps the kit
out of normal builds.

One accepted exception to the no-local-copies rule: helpers whose
signatures mention cli types (`play`, `step`) cannot be called from
cli's own unit tests, because the unit-test build sees two versions
of crate cli and the types do not unify. The `src/session.rs` unit
tests keep small local `play`/`play_some` wrappers with a `NOTE`
comment that records this reason. Helpers over engine types (`mv`)
work everywhere. `tests/history.rs` drives `History` directly, not
`SessionCore`; it imports only `mv`.

### Changes in existing files

* `src/command.rs`: the private `parse_algebraic` is deleted. Both
  call sites use a new private `parse_move` that delegates to
  `notation::parse_algebraic` and maps the error into
  `CommandError::Parse`.
* `src/fmt.rs`: `format_move` delegates to `notation::algebraic`.
* `src/lib.rs`: `pub mod notation;` and `pub mod repl;`. The kit
  needs no registration: it is a crate, not a module.
* `src/bin/historydemo.rs`: one `sq()` helper parses demo squares
  through `notation::parse_algebraic`; formatting uses
  `notation::algebraic`. The local `notation()` and the eight
  constructors are deleted.
* `src/bin/sessiondemo.rs`: keeps `main`, `parse_seed`, the two input
  loops, and a `print_lines` helper. Everything else moved to `repl`.
* `tests/actor.rs`, `tests/session.rs`, `tests/study.rs`,
  `tests/history.rs`: import what they need with
  `use testkit::{...}`; the local copies are deleted.

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
4. **Create the testkit crate.** Create `crates/testkit/` with its
   manifest (path dependencies on `engine` and `cli`) and a
   `src/lib.rs` with the six shared helpers as plain `pub fn` — no
   `#[cfg(test)]` anywhere in the crate. Register the crate in the
   workspace `members` list and add `testkit = { path =
   "../testkit" }` to cli's `[dev-dependencies]`. Build the
   workspace.
5. **Convert the integration tests.** In each of the four test files,
   delete the local helper copies and import the kit with
   `use testkit::{...}`. Run the gates.
6. **Convert the unit tests.** In each `src/` test module with a
   local `mv`, delete the copy and import `use testkit::mv;`. Keep
   the small local `play`/`play_some` wrappers in `src/session.rs`:
   helpers whose signatures mention cli types cannot be called from
   cli's own unit-test build (the two-versions limitation in the
   low-level design). Run the gates.
7. **Prove byte-identical behavior.** Capture the transcripts again
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

* `src/notation.rs`, `src/repl.rs`, and `crates/testkit` exist with
  the contracts above.
* `command.rs`, `fmt.rs`, `lib.rs`, both demo binaries, and all four
  test files delegate to the new modules and the kit crate.
* `cargo test -p cli` is green, and no local helper copies remain —
  except the documented `src/session.rs` wrappers.
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
