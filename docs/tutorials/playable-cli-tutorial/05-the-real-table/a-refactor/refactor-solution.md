# Sub-chapter 05a — Refactor: solution

> **Opt-in solution — read only after you have tried the sub-chapter, or
> if you have been stuck for more than twenty minutes.**
>
> This file quotes the code that sub-chapter a adds or changes, as
> annotated snippets with line numbers. Line numbers refer to the files
> after the sub-chapter is applied. Deletions are described in prose; raw
> diffs are not shown. Verified green: `cargo test -p cli`,
> `cargo clippy --all-targets -- -D warnings`, and
> `cargo fmt --all -- --check` all pass from `gomoku/`, and the two
> scripted demo transcripts are byte-identical before and after.

## src/notation.rs (new, 179 lines)

The module doc (lines 1–10) states why the module exists: the
conversion used to live in three places, and copies of one logic
diverge over time.

src/notation.rs, lines 1–13:

```rust
//! Algebraic notation: the one canonical `h8` ↔ [`Move`] conversion.
//!
//! Before this module existed, the conversion lived in three places: a
//! private parser in `command.rs`, `format_move` in `fmt.rs`, and a
//! `notation()` helper in the historydemo binary. Copies of one logic
//! diverge over time. This module is the single implementation; every
//! caller delegates to it.
//!
//! The notation is column letter (`a`–`o`) followed by row number (`1`–`15`),
//! case-insensitive: `h8` is the center of the 15×15 board.

use engine::Move;
use thiserror::Error;
```

The error type comes first. The `Display` text of each variant
reproduces the strings the old private parser produced, because tests
assert on them and users read them.

src/notation.rs, lines 15–45:

```rust
/// Why a token is not a valid algebraic square.
///
/// The `Display` text of each variant is part of the user interface: the
/// command language flattens these errors into [`crate::command::CommandError::Parse`]
/// and existing tests assert on the exact messages. Change a message only
/// together with those tests.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum NotationError {
    /// The token is too short to hold a column letter and a row number.
    #[error("Expected algebraic square like 'h8', got '{0}'")]
    NotAlgebraic(String),
    /// The first character is not a column letter in `a`–`o`.
    #[error("Expected column letter a-o, got '{letter}' in '{token}'")]
    BadColumn {
        /// The offending first character (already lowercased).
        letter: char,
        /// The token as the user typed it.
        token: String,
    },
    /// The part after the column letter is not a number.
    #[error("Expected row number 1-15, got '{digits}' in '{token}'")]
    BadRow {
        /// The offending row digits (already lowercased).
        digits: String,
        /// The token as the user typed it.
        token: String,
    },
    /// Column and row parsed, but the square is outside the 15×15 board.
    #[error("square '{0}' is off the board")]
    OffBoard(String),
}
```

The parser. The check order is the old order: length, column, row
digits, bounds. The old version returned `(row, col)` and let the
caller construct the `Move`; this version returns the `Move` directly,
because the range checks make construction infallible.

src/notation.rs, lines 47–87:

```rust
/// Parse an algebraic square (`h8`) into a [`Move`].
///
/// The token is trimmed and lowercased first, so `H8`, ` h8 `, and `h8`
/// name the same square. Error messages quote the token as passed in, not
/// the normalized copy, so the user sees their own input.
pub fn parse_algebraic(token: &str) -> Result<Move, NotationError> {
    let text = token.trim().to_ascii_lowercase();
    // A square needs at least one column letter and one row digit.
    // `len` counts bytes; that is safe here because the column check below
    // rejects any non-ASCII first character before we slice bytes.
    if text.len() < 2 {
        return Err(NotationError::NotAlgebraic(token.to_string()));
    }
    let col_letter = text.chars().next().expect("len >= 2 has a first char");
    if !('a'..='o').contains(&col_letter) {
        return Err(NotationError::BadColumn {
            letter: col_letter,
            token: token.to_string(),
        });
    }
    // The column letter is ASCII (one byte), so slicing at byte 1 lands on
    // a char boundary.
    let row_digits = &text[1..];
    let row = row_digits
        .parse::<u8>()
        .map_err(|_| NotationError::BadRow {
            digits: row_digits.to_string(),
            token: token.to_string(),
        })?;
    let col = col_letter as u8 - b'a';
    // Rows are 1-based in notation and 0-based in `Move`. The `col > 14`
    // half of the check is unreachable — the letter check above already
    // bounds the column — but is kept so this function validates its full
    // contract on its own.
    if !(1..=15).contains(&row) || col > 14 {
        return Err(NotationError::OffBoard(token.to_string()));
    }
    // The range checks above are exactly `Move::new`'s preconditions, so
    // construction cannot fail.
    Ok(Move::new(row - 1, col).expect("row and column are in range"))
}
```

The formatter. This body is the logic that `fmt::format_move` and
historydemo's `notation()` each implemented on their own.

src/notation.rs, lines 89–97:

```rust
/// Format a [`Move`] as an algebraic square (`h8`).
///
/// This is the inverse of [`parse_algebraic`]: for every valid move,
/// `parse_algebraic(&algebraic(mv)) == Ok(mv)`.
#[must_use]
pub fn algebraic(mv: Move) -> String {
    // Column 0 prints as 'a'; row 0 prints as 1.
    format!("{}{}", (b'a' + mv.col()) as char, mv.row() + 1)
}
```

The unit tests cover the corners, the center, case-insensitivity, the
round-trip, and every error variant with its exact message. The module
opening imports `mv` from the shared test kit (the `testkit` crate,
quoted below) instead of defining a local copy. The first block shows
the module opening and the value tests.

src/notation.rs, lines 99–126:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use testkit::mv;

    #[test]
    fn parses_edge_and_center_squares() {
        // a1 and o15 are the two far corners; h8 is the center. If the
        // corners round-trip, every square in between does too.
        assert_eq!(parse_algebraic("a1"), Ok(mv(0, 0)));
        assert_eq!(parse_algebraic("o15"), Ok(mv(14, 14)));
        assert_eq!(parse_algebraic("h8"), Ok(mv(7, 7)));
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(parse_algebraic("H8"), Ok(mv(7, 7)));
        assert_eq!(parse_algebraic("A1"), Ok(mv(0, 0)));
        assert_eq!(parse_algebraic("O15"), Ok(mv(14, 14)));
    }

    #[test]
    fn round_trips_edge_and_center_squares() {
        for token in ["a1", "o15", "h8"] {
            let parsed = parse_algebraic(token).unwrap();
            assert_eq!(algebraic(parsed), token);
        }
    }
```

Lines 127–179 contain four more tests, one per error variant. Each
asserts the variant and the exact `Display` text. One of them:

src/notation.rs, lines 138–152:

```rust
    #[test]
    fn rejects_a_bad_column_letter() {
        let err = parse_algebraic("p8").unwrap_err();
        assert_eq!(
            err,
            NotationError::BadColumn {
                letter: 'p',
                token: "p8".to_string()
            }
        );
        assert_eq!(
            err.to_string(),
            "Expected column letter a-o, got 'p' in 'p8'"
        );
    }
```

## src/command.rs

The private `parse_algebraic` (33 lines) is deleted. Both call sites
now go through one private adapter that keeps the old error surface:
`NotationError` is flattened into `CommandError::Parse` with its
`Display` text, so the messages the user sees do not change. The old
`Move::new(...).ok_or_else(...)` fallback after parsing is gone; the
notation module returns a `Move` directly, and its range checks make
that construction infallible.

src/command.rs, lines 183–188 (the explicit `play` arm; the bare-square
arm at lines 238–242 has the same shape):

```rust
        "play" => {
            let token = parse_required_token(&mut tokens, "square")?;
            let mv = parse_move(&token)?;
            ensure_no_more(&mut tokens, "play")?;
            Ok(Command::Play(mv))
        }
```

src/command.rs, lines 246–252 (the adapter):

```rust
/// Parse a square token through the canonical notation module.
///
/// The notation error is flattened into [`CommandError::Parse`] with its
/// `Display` text, so the messages the user sees are unchanged.
fn parse_move(token: &str) -> Result<Move, CommandError> {
    crate::notation::parse_algebraic(token).map_err(|err| CommandError::Parse(err.to_string()))
}
```

## src/fmt.rs

`format_move` keeps its name and its call sites. The implementation
delegates, so one function formats every move.

src/fmt.rs, lines 37–45:

```rust
/// Format a move as algebraic notation (`h8`).
///
/// The canonical implementation is [`crate::notation::algebraic`]. This
/// helper stays so the call sites that predate the notation module keep
/// working unchanged.
#[must_use]
pub fn format_move(mv: Move) -> String {
    crate::notation::algebraic(mv)
}
```

## src/lib.rs

Two module lines added, in alphabetical order with the other modules.
The test kit needs no registration here: the kit is a separate crate,
not a module of `cli`.

src/lib.rs, lines 1–9:

```rust
pub mod actor;
pub mod command;
pub mod history;
pub mod notation;
pub mod repl;
pub mod session;
pub mod tui;

pub mod fmt;
```

## crates/testkit (new)

The shared test kit is one workspace crate. Before this sub-chapter,
the four integration test files each defined their own copies of the
helpers, and the unit-test modules in `src/` each defined another
copy of `mv`. The crate ends the duplication for both worlds at once.

The manifest makes the kit a normal package with path dependencies on
`engine` and `cli`:

crates/testkit/Cargo.toml, lines 1–10:

```toml
[package]
name = "testkit"
description = "The shared test kit for unit and integration tests"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
engine = { path = "../engine" }
cli = { path = "../cli" }
```

cli in turn dev-depends on the kit:

```toml
[dev-dependencies]
testkit = { path = "../testkit" }
```

Three rules shape the crate.

1. **`#[cfg(test)]` never crosses a crate boundary.** The attribute
   gates code to the defining crate's own test build. A `#[cfg(test)]`
   helper inside the kit crate would vanish for every consumer (the
   compiler reports E0432, unresolved import). Every helper in the kit
   is therefore a plain `pub fn`.
2. **The dev-dependency edge is the shipping boundary.** Because cli
   dev-depends on `testkit`, the kit is available to unit tests,
   integration tests, examples, and benches — and never to normal
   builds. No attribute provides this property; the edge does. Cargo
   also permits the resulting cycle: `testkit` depends on `cli` (the
   helpers take cli types) while cli dev-depends on `testkit`.
3. **Helpers over cli types do not unify inside cli's own unit
   tests.** In the unit-test build, rustc sees two versions of crate
   cli: the rlib the kit links against, and the crate under test. A
   kit helper that takes `&mut SessionCore` therefore does not
   type-check inside a `src/` unit test (E0308); such helpers serve
   the integration tests only. Helpers over engine types (`mv`) work
   everywhere. The `src/session.rs` unit tests keep small local
   `play`/`play_some` wrappers, with a `NOTE` comment that records
   this reason — the one accepted exception to the no-local-copies
   rule.

The kit also ends the config-builder duplication. There is one
builder, `fast_tactics_config`, with one comment that records the
deliberate divergence from `AiConfig::default()`: 200 simulations
instead of 400, to keep the test suite fast. Before the kit, two
names and two copies existed; the reason for the number lived
nowhere.

The module doc states rules 1 and 2 where a reader of the crate will
look for them:

crates/testkit/src/lib.rs, lines 1–13:

```rust
//! The shared test kit for unit and integration tests.
//!
//! `cli` dev-depends on this crate. The dev-dependency makes the kit
//! available to the unit tests inside `src/` and to the integration-test
//! binaries under `tests/`, while normal builds never compile it.
//! `#[cfg(test)]` must not appear in this crate: the attribute never
//! crosses crate boundaries, so a `#[cfg(test)]` module inside `cli`
//! stays invisible to the integration tests.

use cli::actor::{AiConfig, EvaluatorKind};
use cli::command::{Command, Event};
use cli::session::{SessionCore, SessionInit};
use engine::Move;
```

One note before the helper quotes: they show the kit as it stands at
the end of the chapter on the reference branch. Sub-chapter c turns
`Command::Play` into a one-field struct variant and adds
`swap2_core`; after this sub-chapter, `play` builds
the tuple variant `Command::Play(mv(r, c))`, and the Swap2
helper does not exist yet.

The move builder and the first session driver. The `unwrap` in `play`
is intentional: a test that plays an illegal move should fail loudly,
and the panic points at the test script.

crates/testkit/src/lib.rs, lines 15–31:

```rust
/// Build a move from zero-based row and column.
///
/// Test scripts are full of coordinates; the `unwrap` is intentional in
/// tests: a test that constructs an invalid move should fail loudly.
pub fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}

/// Play one stone and return the events the session emitted.
///
/// Tests that use this helper expect the move to be legal; an illegal move
/// panics through the `unwrap`, which points at the test script. During a
/// Swap2 ceremony the stone's color comes from the ceremony's strict
/// alternation.
pub fn play(core: &mut SessionCore, r: u8, c: u8) -> Vec<Event> {
    core.execute(Command::Play { mv: mv(r, c) }).unwrap()
}
```

The Swap2 helper (sub-chapter c adds it). `swap2_core` builds a
hotseat session that starts inside the opening ceremony.

crates/testkit/src/lib.rs, lines 33–36:

```rust
/// A hotseat session that starts inside the Swap2 opening ceremony.
pub fn swap2_core() -> SessionCore {
    SessionCore::new(SessionInit::new_hotseat().with_swap2(true))
}
```

The AI driver and the event-name helpers. The `unwrap` in `step` is
intentional too: a test that steps a human seat should fail loudly.
Many assertions care only about which things happened, not about the
payload; comparing variant names keeps those assertions short.
`event_types` is the plural convenience:
`assert_eq!(event_types(&events), ["Moved", "GameOver"])`.

crates/testkit/src/lib.rs, lines 38–73:

```rust
/// Let the AI play the side to move and return the events.
///
/// The seat must be AI-occupied; otherwise `Step` returns an error and the
/// `unwrap` fails the test, which is the intent.
pub fn step(core: &mut SessionCore) -> Vec<Event> {
    core.execute(Command::Step).unwrap()
}

/// The name of an event's variant.
///
/// Many assertions care only about which things happened, not about the
/// payload. Comparing variant names keeps those assertions short.
pub fn event_type(ev: &Event) -> &'static str {
    match ev {
        Event::Moved { .. } => "Moved",
        Event::Branched { .. } => "Branched",
        Event::CursorMoved { .. } => "CursorMoved",
        Event::BranchPoint { .. } => "BranchPoint",
        Event::SeatChanged { .. } => "SeatChanged",
        Event::ConfigChanged { .. } => "ConfigChanged",
        Event::GameOver { .. } => "GameOver",
        Event::Reset => "Reset",
        Event::StonePlaced { .. } => "StonePlaced",
        Event::StoneRemoved { .. } => "StoneRemoved",
        Event::OpeningPhaseChanged { .. } => "OpeningPhaseChanged",
        Event::OpeningFinished { .. } => "OpeningFinished",
    }
}

/// The variant names of a whole event batch, in order.
///
/// Use this to assert an exact expected sequence:
/// `assert_eq!(event_types(&events), ["Moved", "GameOver"])`.
pub fn event_types(events: &[Event]) -> Vec<&'static str> {
    events.iter().map(event_type).collect()
}
```

The one AI-config builder. The comment records the deliberate
divergence: 200 simulations instead of the default 400.

crates/testkit/src/lib.rs, lines 75–91:

```rust
/// The AI configuration shared by the integration tests.
///
/// Deliberate divergence from [`AiConfig::default`]: the tests run 200
/// simulations instead of the default 400, to keep the test suite fast.
/// Every other field matches the default. One builder with this comment
/// replaces the per-file copies (`ai_config_tactics`, `default_ai_config`)
/// that existed before this kit.
pub fn fast_tactics_config() -> AiConfig {
    AiConfig {
        evaluator: EvaluatorKind::Tactics,
        simulations: 200,
        c_puct: 1.5,
        temperature: 0.0,
        noise_epsilon: 0.0,
        explicit_selection: false,
    }
}
```

`command.rs`, `notation.rs`, and `session.rs` delete their local `mv`
and import the kit (`use testkit::mv;`). Sub-chapters b–f add unit
tests to `render.rs`, `opening.rs`, and `repl.rs`; those modules
import the kit from the start. The `src/session.rs` unit tests keep
their local `play`/`play_some` wrappers, under rule 3 above.

## src/repl.rs (new, 175 lines)

The module doc (lines 1–9) states the extraction and its one deliberate
change: nothing here calls `println!`. The `HELP` text (lines 18–36)
moved unchanged from sessiondemo and is quoted above with the header.

src/repl.rs, lines 1–36:

```rust
//! The line interpreter: one input line in, output lines out.
//!
//! This module is the headless core of every line-based front-end. It was
//! extracted from the sessiondemo binary so that the same code can serve
//! the demo, the line-mode fallback screen, and tests. The extraction
//! changes one detail: nothing here calls `println!`. The helpers return
//! their text as lines and the caller decides where the text goes. A
//! function that returns lines can be unit-tested and reused; a function
//! that prints can only be observed through a terminal.

use std::collections::HashMap;

use crate::command::{self, Command};
use crate::fmt::render_board;
use crate::notation::algebraic;
use crate::session::SessionCore;

/// The help text for the line interpreter, shown for `h`, `help`, and `?`.
///
/// The raw string ends with a newline, which [`process_line`] accounts for
/// when it turns the text into lines.
pub const HELP: &str = r#"Commands (case-insensitive):
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
  new              reset to opening
  h, help, ?       show this help
  q, quit, exit    quit
"#;
```

The return type of the interpreter. `quit` carries what the old
`-> bool` return carried; `lines` carries what used to go straight to
stdout.

src/repl.rs, lines 38–52:

```rust
/// The result of processing one input line.
///
/// The interpreter never writes to stdout itself. It returns the text a
/// front-end should show plus a flag for the input loop.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineOutcome {
    /// The output produced by the line, in display order. One element can
    /// hold several visual rows (the rendered board is one `String` with
    /// embedded newlines); printing each element with `println!`
    /// reproduces the intended byte stream.
    pub lines: Vec<String>,
    /// `true` when the line was a quit command and the input loop should
    /// stop. Script runners may ignore this flag.
    pub quit: bool,
}
```

The interpreter. The structure is the old `process_line`; every
`println!` became a `push`. Two byte-level details are commented where
they happen: the blank line after the help text, and the meaning of
`interactive`.

src/repl.rs, lines 54–105:

```rust
/// Process one input line against the session core.
///
/// Empty lines produce nothing. The special command `info` is handled here
/// and not by [`command::parse`], because it reads search state instead of
/// changing session state. Everything else goes through the shared command
/// language; the events it returns are listed, followed by the new state.
///
/// `interactive` controls exactly one behavior: a quit command is
/// acknowledged with `Bye.` only in interactive sessions. Scripted runs
/// stay silent so their transcripts contain only game content.
pub fn process_line(line: &str, core: &mut SessionCore, interactive: bool) -> LineOutcome {
    let mut outcome = LineOutcome::default();
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return outcome;
    }
    if trimmed.eq_ignore_ascii_case("info") {
        outcome.lines = print_search_info(core);
        return outcome;
    }
    match command::parse(trimmed) {
        Ok(Command::Help) => {
            // `HELP` ends with a newline, and the original code printed it
            // with `println!`, which added a blank line after the text.
            // `str::lines` drops that trailing newline, so the blank line
            // is appended explicitly to keep the output byte-identical.
            outcome.lines.extend(HELP.lines().map(str::to_string));
            outcome.lines.push(String::new());
        }
        Ok(Command::Quit) => {
            if interactive {
                outcome.lines.push("Bye.".to_string());
            }
            outcome.quit = true;
        }
        Ok(cmd) => match core.execute(cmd) {
            Ok(events) => {
                if events.is_empty() {
                    outcome.lines.push("(no events)".to_string());
                } else {
                    for event in &events {
                        outcome.lines.push(format!("  {event}"));
                    }
                }
                outcome.lines.extend(print_state(core, ""));
            }
            Err(e) => outcome.lines.push(format!("Error {e}")),
        },
        Err(e) => outcome.lines.push(format!("Parse error {e}")),
    }
    outcome
}
```

The state view. The final empty line is the old trailing `println!()`.

src/repl.rs, lines 107–137:

```rust
/// The lines that describe the session: optional header, one status line,
/// the branch options if any, and the board.
///
/// This is the companion to every successful command: after the events are
/// listed, the user sees where the session stands. The returned vector ends
/// with one empty line, the visual separator before the next prompt.
pub fn print_state(core: &SessionCore, header: &str) -> Vec<String> {
    let mut lines = Vec::new();
    if !header.is_empty() {
        lines.push(header.to_string());
    }
    let snapshot = core.snapshot();
    lines.push(format!(
        "ply {}  status {:?}  black {:?}  white {:?}",
        snapshot.ply, snapshot.status, snapshot.black, snapshot.white
    ));
    if !snapshot.branch_options.is_empty() {
        let opts: Vec<String> = snapshot
            .branch_options
            .iter()
            .map(|(i, m)| format!("{i}: {}", algebraic(*m)))
            .collect();
        lines.push(format!("branches: {}", opts.join("  ")));
    }
    // The board arrives as one `String` with embedded newlines. It is kept
    // as a single element so the caller can print every element the same
    // way.
    lines.push(render_board(&core.board(), snapshot.last_move));
    lines.push(String::new());
    lines
}
```

The search-info view.

src/repl.rs, lines 139–175:

```rust
/// The lines that describe the AI search stored at the cursor: root value,
/// total visits, and the five most-visited moves with their priors.
///
/// Search info exists only where an AI searched, so on a human-only line
/// the answer is a single explanatory line.
pub fn print_search_info(core: &SessionCore) -> Vec<String> {
    let mut lines = Vec::new();
    match core.search_info() {
        None => lines.push("No search info at the cursor.".to_string()),
        Some(info) => {
            let total_visits: u32 = info.visits.iter().map(|(_, n)| n).sum();
            lines.push(format!(
                "value {:.3}  total visits {total_visits}",
                info.value
            ));
            if info.visits.is_empty() {
                lines.push("(no legal moves)".to_string());
                return lines;
            }
            // The prior list is keyed by move; a map lookup is the direct
            // way to align it with the visit-sorted list.
            let prior_by_move: HashMap<_, _> = info.prior.iter().map(|(mv, p)| (*mv, *p)).collect();
            lines.push("top moves (visits, prior%):".to_string());
            for (i, (mv, visits)) in info.visits.iter().take(5).enumerate() {
                let prior = prior_by_move.get(mv).copied().unwrap_or(0.0);
                lines.push(format!(
                    "  {}. ({}, {:.1}%) {}",
                    i + 1,
                    visits,
                    prior * 100.0,
                    algebraic(*mv)
                ));
            }
        }
    }
    lines
}
```

## src/bin/sessiondemo.rs

Deleted: the `HELP` constant, `process_line`, `print_state`, and
`print_search_info`. Kept: the module doc, `parse_seed`, `main` with
its two input loops, and one new helper that owns the printing.

src/bin/sessiondemo.rs, lines 36–45:

```rust
/// Print one batch of output lines.
///
/// [`cli::repl`] returns text instead of printing it, so this helper is
/// the single place where the binary touches stdout for content. (The
/// prompt below is the one exception: it must not end with a newline.)
fn print_lines(lines: &[String]) {
    for line in lines {
        println!("{line}");
    }
}
```

The two loops. The script loop ignores `quit` on purpose, because the
old code discarded the return value in script mode; the interactive
loop breaks on `quit` exactly as before.

src/bin/sessiondemo.rs, lines 73–100:

```rust
            for line in reader.lines() {
                let line = line.unwrap_or_else(|e| {
                    eprintln!("Cannot read line {}", e);
                    std::process::exit(1);
                });
                let outcome = repl::process_line(&line, &mut core, false);
                // The quit flag is ignored on purpose: a script continues
                // after a `quit` line, exactly as the pre-extraction
                // sessiondemo did when it discarded the return value.
                print_lines(&outcome.lines);
            }
        }

        None => {
            let stdin = stdin();
            print!("session> ");
            stdout().flush().unwrap();
            for line in stdin.lock().lines() {
                let line = line.expect("Cannot read line from stdin");
                let outcome = repl::process_line(&line, &mut core, true);
                print_lines(&outcome.lines);
                if outcome.quit {
                    break;
                }
                print!("session> ");
                stdout().flush().unwrap();
            }
        }
```

## src/bin/historydemo.rs

The local `notation()` helper and the eight hardcoded constructors
(`h8()` … `i9()`) are deleted. Formatting calls `notation::algebraic`
(at lines 50, 66, and 95); construction goes through one helper that
runs the canonical parser, so the demo exercises the same code path a
user types into.

src/bin/historydemo.rs, lines 14–15:

```rust
use cli::notation::{algebraic, parse_algebraic};
use engine::{Board, Color, Move};
```

src/bin/historydemo.rs, lines 24–25:

```rust
    // Six-move main line.
    let main_line = [sq("h8"), sq("h7"), sq("i8"), sq("i7"), sq("j8"), sq("j7")];
```

src/bin/historydemo.rs, lines 120–126:

```rust
/// Parse a demo square through the canonical notation module.
///
/// The demo uses the same parser as the command language, so a typo in a
/// square fails here with the same message a user would see.
fn sq(token: &str) -> Move {
    parse_algebraic(token).expect("demo squares are valid algebraic notation")
}
```

## The four test files

Each test file deletes its local copies and imports the kit. The new
headers:

tests/actor.rs, lines 6–11 (the local `mv`, `play`, and
`ai_config_tactics` are deleted; all uses of `ai_config_tactics()`
became `fast_tactics_config()`):

```rust
use cli::actor::{AiConfig, Occupant, OccupantKind};
use cli::command::{Command, CommandError, Event, SeatSel};
use cli::session::{SessionCore, SessionInit};
use engine::{Board, Color, Status};

use testkit::{fast_tactics_config, mv, play};
```

tests/session.rs, lines 9–11 (`play_some` stays; it is the one helper
no other file uses):

```rust

use testkit::{mv, play};

```

tests/study.rs, lines 10–12 (the local `mv`, `play`, `step`,
`event_type`, and `default_ai_config` are deleted):

```rust

use testkit::{event_type, fast_tactics_config, play, step};

```

tests/history.rs, lines 4–6 (this file drives `History` directly, so it
shares only `mv`):

```rust

use testkit::mv;

```

## Verification

From `gomoku/`, after the refactor:

* `cargo test -p cli` — 63 tests pass (56 before, plus 7 new notation
  unit tests).
* `cargo clippy --all-targets -- -D warnings` — clean.
* `cargo fmt --all -- --check` — clean.
* The two scripted demo transcripts captured before and after the
  change diff empty.
