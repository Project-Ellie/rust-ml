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
use crate::opening::OpeningSnapshot;
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
  v, value         toggle the value strip
  new              reset to opening
  h, help, ?       show this help
  q, quit, exit    quit

Swap2 opening (sessions started with swap2):
  play SQUARE      place the next stone (colors alternate, black first)
  back [N]         remove the last placement(s)
  done             end the current placement round
  black | white    choose that color at a choice phase
  more             decline the first choice and place two more stones
"#;

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
    // During the ceremony the rendered board is empty, because
    // placements are not game moves. One line carries the ceremony
    // state instead; the full-screen renderer will draw the stones
    // from the same snapshot field.
    if let Some(opening) = &snapshot.opening {
        lines.push(opening_line(opening));
    }
    // The board arrives as one `String` with embedded newlines. It is kept
    // as a single element so the caller can print every element the same
    // way.
    lines.push(render_board(&core.board(), snapshot.last_move));
    lines.push(String::new());
    lines
}

/// The one state line of a ceremony in progress: phase, placements in
/// order, and the color the next `play` places.
///
/// At a choice phase there is no next stone, so the `next` part is
/// left out. The line appears only in Swap2 sessions; ordinary
/// transcripts are byte-identical to before.
fn opening_line(opening: &OpeningSnapshot) -> String {
    let stones = if opening.placements.is_empty() {
        "(none)".to_string()
    } else {
        opening
            .placements
            .iter()
            .map(|(mv, color)| format!("{}:{color:?}", algebraic(*mv)))
            .collect::<Vec<_>>()
            .join(" ")
    };
    match opening.next_color {
        Some(color) => format!(
            "swap2 {:?}  stones: {stones}  next: {color:?}",
            opening.phase
        ),
        None => format!("swap2 {:?}  stones: {stones}", opening.phase),
    }
}

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
