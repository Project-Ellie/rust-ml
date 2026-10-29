# Sub-chapter 06a — The value strip: solution

This file is opt-in. Read it only if you have been stuck for more than
twenty minutes, or after you have finished the sub-chapter and want to
compare your work.

Verified green from `gomoku/`: `cargo test -p cli`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.

---

The new module is `src/tui/overlay.rs`. It contains the pure formatting
for the value strip and the helper that reads the live seat config.
`gomoku/crates/cli/src/tui/overlay.rs` is quoted in full.

`gomoku/crates/cli/src/tui/overlay.rs`, lines 1–143:

```rust
//! Search-info overlays — pure functions, no I/O.
//!
//! [`value_strip`] turns a cursor node's [`SearchInfo`] into the lines of
//! the side-panel value strip. The helper is deliberately testable
//! without a real MCTS search: callers can construct [`SearchInfo`]
//! directly.

use crate::actor::{AiConfig, SearchInfo};
use crate::fmt::format_move;
use crate::session::SessionCore;

/// Build the value-strip panel lines from a cursor's search info.
///
/// `current_config` is the live config of the seat that searched the
/// cursor position. If it differs from the fingerprint stored in
/// [`SearchInfo::config`], or if the seat is now human, a staleness note
/// is appended. The searching side is read from
/// [`SearchInfo::side_to_move`], so the caller does not pass it.
#[must_use]
pub fn value_strip(info: Option<&SearchInfo>, current_config: Option<&AiConfig>) -> Vec<String> {
    let Some(info) = info else {
        return Vec::new();
    };

    let total: u32 = info.visits.iter().map(|(_, n)| n).sum();
    let value_sign = if info.value >= 0.0 { '+' } else { '-' };
    let side = info.side_to_move;

    let mut lines = Vec::new();
    lines.push(format!(
        "value {}{:.2} for {side:?}",
        value_sign,
        info.value.abs()
    ));
    lines.push(format!("total visits: {total}"));
    lines.push("top moves:".to_string());
    for (i, (mv, n)) in info.visits.iter().take(3).enumerate() {
        let share = if total > 0 {
            *n as f32 / total as f32
        } else {
            0.0
        };
        lines.push(format!(
            "  {}. {} {:>5.1}%",
            i + 1,
            format_move(*mv),
            share * 100.0
        ));
    }

    let stale = match current_config {
        None => true,
        Some(cfg) => cfg != &info.config,
    };
    if stale {
        lines.push(format!("(!searched at sims={})", info.config.simulations));
    }

    lines
}

/// The current effective AI configuration for the seat that searched
/// the cursor position, if that seat is still AI.
///
/// This is a small helper so the renderer does not reach into the
/// occupant enum twice.
#[must_use]
pub fn searching_seat_config<'a>(core: &'a SessionCore, info: &SearchInfo) -> Option<&'a AiConfig> {
    match core.occupant(info.side_to_move) {
        crate::actor::Occupant::Ai(cfg) => Some(cfg),
        crate::actor::Occupant::Human => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Color, Move};

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    fn dummy_config(simulations: u32) -> AiConfig {
        AiConfig {
            simulations,
            ..AiConfig::default()
        }
    }

    fn search_info_with_visits(
        visits: &[(Move, u32)],
        prior: &[(Move, f32)],
        value: f32,
        side_to_move: Color,
    ) -> SearchInfo {
        SearchInfo {
            side_to_move,
            visits: visits.to_vec(),
            prior: prior.to_vec(),
            value,
            config: dummy_config(400),
        }
    }

    #[test]
    fn value_strip_formats_sign_and_color() {
        let info =
            search_info_with_visits(&[(mv(7, 7), 300), (mv(6, 6), 100)], &[], 0.42, Color::White);
        let lines = value_strip(Some(&info), Some(&dummy_config(400)));
        assert_eq!(lines[0], "value +0.42 for White");
        assert!(lines.iter().any(|l| l.contains("total visits: 400")));
        assert!(lines.iter().any(|l| l.contains("75.0%")));
    }

    #[test]
    fn value_strip_negative_value_keeps_sign() {
        let info = search_info_with_visits(&[(mv(7, 7), 100)], &[], -0.17, Color::Black);
        let lines = value_strip(Some(&info), Some(&dummy_config(400)));
        assert_eq!(lines[0], "value -0.17 for Black");
    }

    #[test]
    fn value_strip_appends_staleness_when_config_differs() {
        let info = search_info_with_visits(&[(mv(7, 7), 100)], &[], 0.0, Color::Black);
        let current = dummy_config(800);
        let lines = value_strip(Some(&info), Some(&current));
        assert!(lines.iter().any(|l| l.contains("(!searched at sims=400)")));
    }

    #[test]
    fn value_strip_is_stale_when_seat_is_human() {
        let info = search_info_with_visits(&[(mv(7, 7), 100)], &[], 0.0, Color::Black);
        let lines = value_strip(Some(&info), None);
        assert!(lines.iter().any(|l| l.contains("(!searched at sims=400)")));
    }

    #[test]
    fn value_strip_no_search_info_returns_empty() {
        let lines = value_strip(None, None);
        assert!(lines.is_empty());
    }
}
```

The module is declared in `src/tui/mod.rs`.

`gomoku/crates/cli/src/tui/mod.rs`, lines 1–6:

```rust
pub mod input;
pub mod opening;
pub mod overlay;
pub mod render;
pub mod screen;
pub mod study;
```

In `src/tui/input.rs` the `Status` import is added so the moved
`empty_line_outcome` can guard on the board status.

`gomoku/crates/cli/src/tui/input.rs`, lines 20–24:

```rust
use crate::command::{self, Command, OpeningPhase};
use crate::opening::OpeningSnapshot;
use crate::session::SessionCore;
use crossterm::event::{KeyCode, KeyEvent};
use engine::{Color, Move, Status};
```

`InputOutcome` gains `ToggleValueStrip` between `Message` and `None`.

`gomoku/crates/cli/src/tui/input.rs`, lines 88–103:

```rust
pub enum InputOutcome {
    /// Execute this command on the session core.
    Command(Command),
    /// Show the full-screen help; any key returns to the table.
    Help,
    /// Leave the shell.
    Quit,
    /// Show this text on the message line. Parse errors and keys that
    /// do not apply in the current phase use this variant.
    Message(String),
    /// Toggle the value strip in the side panel.
    ToggleValueStrip,
    /// The key changed only input state (cursor moved, buffer edited),
    /// or the key is not mapped. Nothing reaches the session.
    None,
}
```

The normal-mode global-key table returns the new outcome for `v`.

`gomoku/crates/cli/src/tui/input.rs`, lines 176–202:

```rust
fn handle_normal_key(key: KeyEvent, state: &mut InputState, core: &SessionCore) -> InputOutcome {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            move_cursor(&mut state.cursor, -1, 0);
            return InputOutcome::None;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            move_cursor(&mut state.cursor, 1, 0);
            return InputOutcome::None;
        }
        KeyCode::Left | KeyCode::Char('h') => {
            move_cursor(&mut state.cursor, 0, -1);
            return InputOutcome::None;
        }
        KeyCode::Right | KeyCode::Char('l') => {
            move_cursor(&mut state.cursor, 0, 1);
            return InputOutcome::None;
        }
        KeyCode::Char(':') => {
            state.mode = InputMode::CommandLine(String::new());
            return InputOutcome::None;
        }
        KeyCode::Char('?') => return InputOutcome::Help,
        KeyCode::Char('q') => return InputOutcome::Quit,
        KeyCode::Char('v') => return InputOutcome::ToggleValueStrip,
        _ => {}
    }
```

`line_outcome` and `empty_line_outcome` now live in `input.rs`. The
former recognizes the shell-level words `v` and `value` before falling
back to the shared command grammar.

`gomoku/crates/cli/src/tui/input.rs`, lines 280–323:

```rust
/// Translate one input line into an outcome: the line-mode mapping.
///
/// Everything non-empty goes through [`command::parse`] except the
/// shell-level words `v` and `value`, which toggle the value strip.
/// The overlay is a shell concern, so the command grammar does not
/// gain a variant for it. A parse error becomes an
/// [`InputOutcome::Message`] with the parser's own text, the same as
/// on the `:` command line of the full-screen shell.
pub fn line_outcome(line: &str, core: &SessionCore) -> InputOutcome {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return empty_line_outcome(core);
    }
    if trimmed.eq_ignore_ascii_case("v") || trimmed.eq_ignore_ascii_case("value") {
        return InputOutcome::ToggleValueStrip;
    }
    match command::parse(trimmed) {
        Ok(cmd) => InputOutcome::Command(cmd),
        Err(err) => InputOutcome::Message(err.to_string()),
    }
}

/// The empty line, by the reference build's rule: `step` when the
/// side-to-move seat is AI, nothing otherwise.
///
/// This is the surviving half of Enter's normal-mode meaning. Enter at
/// a human seat plays at the cursor, and a cursor does not exist in
/// line mode, so that half cannot apply. Two guards come first. A
/// finished game has nothing to step (the reference's own guard). And
/// a ceremony in progress refuses `step` — the AI searches game
/// positions, and ceremony stones are not a game position — so the
/// empty line stays quiet there instead of answering every Enter with
/// the `OpeningInProgress` error.
fn empty_line_outcome(core: &SessionCore) -> InputOutcome {
    let board = core.board();
    if core.snapshot().opening.is_some() || board.status() != Status::Ongoing {
        return InputOutcome::None;
    }
    if core.occupant(board.to_move()).is_ai() {
        InputOutcome::Command(Command::Step)
    } else {
        InputOutcome::None
    }
}
```

The two new input tests cover the `v` key and the typed words.

`gomoku/crates/cli/src/tui/input.rs`, lines 721–741:

```rust
    #[test]
    fn v_in_normal_mode_toggles_value_strip() {
        let core = playing_core();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(char_key('v'), &mut state, &core),
            InputOutcome::ToggleValueStrip
        );
    }

    #[test]
    fn v_and_value_in_line_mode_toggle_value_strip() {
        let core = playing_core();
        for word in ["v", "V", "value", "VALUE"] {
            assert_eq!(
                line_outcome(word, &core),
                InputOutcome::ToggleValueStrip,
                "{word} toggles the value strip"
            );
        }
    }
```

The line-mode tests moved here from `study.rs`. They validate the empty
line rule and the shared grammar.

`gomoku/crates/cli/src/tui/input.rs`, lines 743–875:

```rust
    // -------------------------------------------------------- line mode

    fn hotseat_core() -> SessionCore {
        SessionCore::new(SessionInit::new_hotseat())
    }

    #[test]
    fn the_empty_line_steps_an_ai_seat_and_stays_quiet_for_a_human_seat() {
        let mut core = hotseat_core();
        // Both seats human: nothing to step.
        assert_eq!(line_outcome("", &core), InputOutcome::None);
        assert_eq!(line_outcome("   ", &core), InputOutcome::None);

        // Black to move, Black released to the AI: the empty line steps.
        core.execute(Command::Release {
            seat: SeatSel::Black,
        })
        .unwrap();
        assert_eq!(
            line_outcome("", &core),
            InputOutcome::Command(Command::Step)
        );

        // White to move and human: quiet again. (`play` does not check
        // seats; the seat table matters only to `step`.)
        core.execute(Command::Play {
            mv: Move::new(7, 7).unwrap(),
        })
        .unwrap();
        assert_eq!(line_outcome("", &core), InputOutcome::None);
    }

    #[test]
    fn the_empty_line_stays_quiet_during_the_ceremony() {
        // An AI Black seat would qualify for a step during play, but
        // the ceremony refuses step: the AI searches game positions,
        // and ceremony stones are not a game position.
        let mut core = SessionCore::new(SessionInit::new_hotseat().with_swap2(true));
        core.execute(Command::Release {
            seat: SeatSel::Black,
        })
        .unwrap();
        assert_eq!(line_outcome("", &core), InputOutcome::None);
    }

    #[test]
    fn the_empty_line_stays_quiet_after_the_game_ends() {
        let mut core = hotseat_core();
        // Black completes five on row 8; White parks on row 1.
        for (r, c) in [
            (7, 3),
            (0, 1),
            (7, 4),
            (0, 2),
            (7, 5),
            (0, 3),
            (7, 6),
            (0, 4),
            (7, 7),
        ] {
            core.execute(Command::Play {
                mv: Move::new(r, c).unwrap(),
            })
            .unwrap();
        }
        assert_eq!(core.board().status(), Status::Won(Color::Black));
        // White to move, White released to the AI: without the status
        // guard the rule would step; a finished game steps nothing.
        core.execute(Command::Release {
            seat: SeatSel::White,
        })
        .unwrap();
        assert_eq!(line_outcome("", &core), InputOutcome::None);
    }

    #[test]
    fn a_line_maps_through_the_shared_grammar() {
        let core = hotseat_core();
        assert_eq!(
            line_outcome("h8", &core),
            InputOutcome::Command(Command::Play {
                mv: Move::new(7, 7).unwrap()
            })
        );
        // There is no color token on `play`: ceremony placements
        // follow strict alternation, so a trailing token is a parse
        // error, shown as a message like any other.
        let InputOutcome::Message(text) = line_outcome("play i8 white", &core) else {
            panic!("a color token on play is a parse error");
        };
        assert!(text.contains("play takes no arguments"), "the text: {text}");
        // Help and quit need no special cases in the line mapping: the
        // grammar knows them, and the loop intercepts the commands.
        for word in ["h", "help", "?", "HELP"] {
            assert_eq!(
                line_outcome(word, &core),
                InputOutcome::Command(Command::Help),
                "{word} maps to Help"
            );
        }
        for word in ["q", "quit", "exit", "QUIT"] {
            assert_eq!(
                line_outcome(word, &core),
                InputOutcome::Command(Command::Quit),
                "{word} maps to Quit"
            );
        }
        // The ceremony vocabulary parses like everything else.
        assert_eq!(
            line_outcome("done", &core),
            InputOutcome::Command(Command::Done)
        );
        assert_eq!(
            line_outcome("black", &core),
            InputOutcome::Command(Command::ChooseColor {
                color: Color::Black
            })
        );
        assert_eq!(
            line_outcome("more", &core),
            InputOutcome::Command(Command::More)
        );
    }

    #[test]
    fn a_parse_error_is_a_message_with_the_parsers_text() {
        let core = hotseat_core();
        let InputOutcome::Message(text) = line_outcome("bogus", &core) else {
            panic!("a parse error is a message");
        };
        assert!(text.contains("Parse error"), "the parser's text: {text}");
    }
}
```

In `src/tui/render.rs` the overlay helpers are imported.

`gomoku/crates/cli/src/tui/render.rs`, lines 14–21:

```rust
use crate::actor::Occupant;
use crate::command::{Event, OpeningPhase};
use crate::notation::algebraic;
use crate::opening::OpeningSnapshot;
use crate::session::{SessionCore, Snapshot};
use crate::tui::overlay::{searching_seat_config, value_strip};
use crossterm::style::Stylize;
use engine::{Board, Color, Move, Status};
```

`StudyView` stores the flag and the constructor initializes it to
`false`.

`gomoku/crates/cli/src/tui/render.rs`, lines 49–72:

```rust
pub struct StudyView<'a> {
    core: &'a SessionCore,
    recent_events: &'a [Event],
    cursor: Option<Move>,
    command_line: Option<&'a str>,
    message: Option<&'a str>,
    styled: bool,
    show_value_strip: bool,
}

impl<'a> StudyView<'a> {
    /// A plain view of the session: no cursor, no command line, no
    /// styling.
    #[must_use]
    pub fn new(core: &'a SessionCore, recent_events: &'a [Event]) -> Self {
        Self {
            core,
            recent_events,
            cursor: None,
            command_line: None,
            message: None,
            styled: false,
            show_value_strip: false,
        }
```

The chainable setter turns the strip on or off.

`gomoku/crates/cli/src/tui/render.rs`, lines 120–128:

```rust
    /// Show the value strip in the side panel.
    ///
    /// The strip shows the search info at the cursor position, or an
    /// explanatory line when the cursor has no stored search.
    #[must_use]
    pub fn show_value_strip(mut self, show: bool) -> Self {
        self.show_value_strip = show;
        self
    }
```

`render_study` forwards the flag into `render_side_panel`.

`gomoku/crates/cli/src/tui/render.rs`, lines 154–161:

```rust
    let panel_lines = render_side_panel(
        core,
        &board,
        &snapshot,
        view.recent_events,
        view.styled,
        view.show_value_strip,
    );
```

`render_side_panel` accepts the flag and, when it is true, inserts the
strip block before `Last events`.

`gomoku/crates/cli/src/tui/render.rs`, lines 304–334:

```rust
fn render_side_panel(
    core: &SessionCore,
    board: &Board,
    snapshot: &Snapshot,
    recent_events: &[Event],
    styled: bool,
    show_value_strip: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(panel_line("-- Study table --", styled));
    lines.push(format!("Black: {}", seat_label(core.black())));
    lines.push(format!("White: {}", seat_label(core.white())));
    lines.push(String::new());
    // The middle block is phase-dependent: during the ceremony the tree
    // is empty, so ply and side-to-move would describe a game that has
    // not started. The ceremony block takes their place.
    match &snapshot.opening {
        Some(opening) => ceremony_lines(&mut lines, opening),
        None => position_lines(&mut lines, board, snapshot),
    }

    if show_value_strip {
        lines.push(String::new());
        lines.push("Value strip:".to_string());
        if let Some(info) = core.search_info() {
            let current_config = searching_seat_config(core, info);
            lines.extend(value_strip(Some(info), current_config));
        } else {
            lines.push("  (no search here yet)".to_string());
        }
    }
```

The tests module imports `AiConfig` for the new rendering test that
steps an AI-vs-AI seat.

`gomoku/crates/cli/src/tui/render.rs`, lines 469–473:

```rust
mod tests {
    use super::*;
    use crate::actor::AiConfig;
    use crate::command::Command;
    use crate::session::SessionInit;
```

The three rendering tests assert the default state, the no-search
explanation, and the full readout.

`gomoku/crates/cli/src/tui/render.rs`, lines 768–809:

```rust
    #[test]
    fn the_value_strip_is_hidden_by_default() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        let frame = render_study(&StudyView::new(&core, &[]));
        let text = frame.join("\n");
        assert!(
            !text.contains("Value strip:"),
            "the strip is off by default: {text}"
        );
    }

    #[test]
    fn the_value_strip_explains_when_there_is_no_search() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        let frame = render_study(&StudyView::new(&core, &[]).show_value_strip(true));
        let text = frame.join("\n");
        assert!(text.contains("Value strip:"), "the header appears: {text}");
        assert!(
            text.contains("(no search here yet)"),
            "the no-search note appears: {text}"
        );
    }

    #[test]
    fn the_panel_shows_the_value_strip_when_enabled() {
        let mut core = SessionCore::new(SessionInit::ai_vs_ai(AiConfig::default(), 1));
        core.execute(Command::Step).unwrap();
        assert!(core.search_info().is_some(), "a step stores search info");

        let frame = render_study(&StudyView::new(&core, &[]).show_value_strip(true));
        let text = frame.join("\n");
        assert!(text.contains("Value strip:"), "the header appears: {text}");
        assert!(text.contains("value "), "the value line appears: {text}");
        assert!(
            text.contains("total visits:"),
            "the visits line appears: {text}"
        );
        assert!(
            text.contains("top moves:"),
            "the top moves line appears: {text}"
        );
    }
```

In `src/tui/study.rs` the imports drop `command` and `Status` and import
`line_outcome` from `input.rs`.

`gomoku/crates/cli/src/tui/study.rs`, lines 25–33:

```rust
use crate::command::{Command, Event};
use crate::repl;
use crate::session::{SessionCore, SessionInit};
use crate::tui::input::{InputMode, InputOutcome, InputState, handle_key, line_outcome};
use crate::tui::render::{StudyView, render_study};
use crate::tui::screen::{AltScreen, LineScreen, Screen};
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use engine::Move;
use std::io::{self, BufRead, IsTerminal};
```

The `LineInput` doc comment is updated to point at the moved
`line_outcome` and the two new shell-level words.

`gomoku/crates/cli/src/tui/study.rs`, lines 100–114:

```rust
/// The line-mode input source: one complete line per call.
///
/// `LineInput` is the canonical-mode counterpart of [`RawInput`]. The
/// terminal driver in canonical mode delivers a line at a time, and
/// so do pipes, CI logs, and redirected scripts. Each line maps to an
/// outcome through [`crate::tui::input::line_outcome`]; that function
/// reuses the shared grammar of [`command::parse`] and adds the two
/// shell-level words `v` and `value` for toggling the value strip.
/// The grammar already knows `h`/`help`/`?` as `Help` and
/// `q`/`quit`/`exit` as `Quit`, and the loop intercepts both
/// commands. What line mode does not have are the cursor-only keys —
/// arrows and the `d`/`b`/`w`/`m` ceremony keys — because there is no
/// cursor to move and no normal mode to press them in. The typed
/// commands (`play h8`, `done`, `black`, `white`, `more`, `back`)
/// cover the ceremony.
```

`ShellState` is created before the loop and its `show_value_strip` flag
is passed to `build_frame`.

`gomoku/crates/cli/src/tui/study.rs`, lines 265–282:

```rust
    // The message line shows the last event summary or the last error
    // text; the caller's hint, when given, is the first message.
    let mut message: Option<String> = hint.map(str::to_string);
    let mut shell = ShellState::default();
    loop {
        // The frame shows the board cursor only when the input source
        // has one to move. The line-mode source has none, so it draws
        // no marker instead of a marker no key can move.
        let cursor = input.has_normal_mode().then_some(state.cursor);
        let frame = build_frame(
            core,
            &state,
            cursor,
            &events,
            message.as_deref(),
            screen.styled(),
            shell.show_value_strip,
        );
```

The loop handles `ToggleValueStrip` by flipping the flag and writing a
short message.

`gomoku/crates/cli/src/tui/study.rs`, lines 287–297:

```rust
            InputOutcome::ToggleValueStrip => {
                shell.show_value_strip = !shell.show_value_strip;
                message = Some(
                    if shell.show_value_strip {
                        "value strip on"
                    } else {
                        "value strip off"
                    }
                    .to_string(),
                );
            }
```

`ShellState` itself is a small private struct.

`gomoku/crates/cli/src/tui/study.rs`, lines 317–325:

```rust
/// Display and overlay state owned by the shell loop.
///
/// This is not session state: it belongs to the user's view of the
/// table and is reset when the shell restarts.
#[derive(Debug, Default)]
struct ShellState {
    /// Whether the side panel shows the value strip.
    show_value_strip: bool,
}
```

`build_frame` receives the flag and forwards it to `StudyView`.

`gomoku/crates/cli/src/tui/study.rs`, lines 335–358:

```rust
fn build_frame(
    core: &SessionCore,
    state: &InputState,
    cursor: Option<Move>,
    events: &[Event],
    message: Option<&str>,
    styled: bool,
    show_value_strip: bool,
) -> Vec<String> {
    let view = StudyView::new(core, events)
        .styled(styled)
        .show_value_strip(show_value_strip);
    let view = match cursor {
        Some(mv) => view.cursor(mv),
        None => view,
    };
    let view = match &state.mode {
        InputMode::CommandLine(buffer) => view.command_line(buffer),
        InputMode::Normal => match message {
            Some(text) => view.message(text),
            None => view,
        },
    };
    render_study(&view)
```

The normal-mode key table lists `v`.

`gomoku/crates/cli/src/tui/study.rs`, lines 419–428:

```rust
                 during Swap2 placement: place the next stone
  u              undo: remove a Swap2 placement / go back one ply
  d              Swap2 placement: end the round (done)
  b, w           Swap2 choice: choose that color
  m              Swap2 first choice: place two more stones (more)
  v              toggle value strip
  :              open the command line (Esc cancels, Enter executes)
  ?              show this help
  q              quit
"#;
```

The loop test presses `v` and checks that the next frame contains the
strip header and the no-search explanation.

`gomoku/crates/cli/src/tui/study.rs`, lines 754–773:

```rust
    #[test]
    fn v_toggles_the_value_strip_in_the_frame() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        // `v` toggles the strip on; `q` quits. The cursor has no
        // stored search, so the panel shows the explanatory line.
        let keys = vec![char_key('v'), char_key('q')];
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        // Frame 0 is the initial frame. Frame 1 follows the `v` key.
        let frame1 = &screen.frames[1];
        let text = frame1.join("\n");
        assert!(
            text.contains("Value strip:"),
            "the strip header appears: {text}"
        );
        assert!(
            text.contains("(no search here yet)"),
            "the no-search note appears: {text}"
        );
    }
```

The line interpreter's help text also documents the typed command.

`gomoku/crates/cli/src/repl.rs`, lines 23–37:

```rust
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
```

The byte-exact integration transcript grows by the matching help line.

`gomoku/crates/cli/tests/line_mode.rs`, lines 123–127:

```rust
  info             show AI search info at the cursor
  v, value         toggle the value strip
  new              reset to opening
  h, help, ?       show this help
  q, quit, exit    quit
```
