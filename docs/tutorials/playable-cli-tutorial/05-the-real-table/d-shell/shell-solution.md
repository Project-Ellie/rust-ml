# Sub-chapter 05d — The alternate-screen shell: solution

> **Opt-in solution — read only after you have tried the sub-chapter, or
> if you have been stuck for more than twenty minutes.**
>
> This file quotes the code that sub-chapter d adds or changes, as
> annotated snippets with line numbers. Line numbers refer to the files
> after the sub-chapter is applied. Deletions are described in prose;
> raw diffs are not shown. Verified green: `cargo test -p cli`,
> `cargo clippy --all-targets -- -D warnings`, and
> `cargo fmt --all -- --check` all pass from `gomoku/`. The two
> scripted demo transcripts (sub-chapter a's checklist) are
> byte-identical to before.

Reading order: the screen first (`screen.rs`), then the input
translation (`input.rs`), then the renderer extension (`render.rs`),
then the loop (`study.rs`).

## src/tui/screen.rs (new, 122 lines)

The module doc (lines 1–13) states the doctrine: the trait is
deliberately small; a screen knows how to draw lines and nothing
about sessions, commands, or keys. That smallness is what lets a test
replace the terminal with a `Vec`.

src/tui/screen.rs, lines 1–23:

```rust
//! The screen abstraction: where a finished frame goes.
//!
//! One trait, one method. The loop in [`crate::tui::study`] builds a
//! frame as a `Vec<String>` and hands it to a [`Screen`]; the screen
//! decides how the lines reach the user. [`AltScreen`] is the
//! full-screen implementation: crossterm, the alternate screen, and an
//! RAII guard that restores the terminal on every exit path. The
//! line-mode fallback of sub-chapter e will be a second implementation.
//!
//! The trait is deliberately small. A screen does not know about
//! sessions, commands, or keys; it knows how to draw lines. That is
//! what lets a test replace the terminal with a `Vec` of recorded
//! frames.

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::Print,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};

/// A destination for one rendered frame.
```

The trait has one required method and one defaulted one. `styled()`
lives here because sub-chapter b established that the styling choice
belongs to the screen; the default is unstyled, so a recording test
screen gets plain text without opting out.

src/tui/screen.rs, lines 24–43:

```rust
pub trait Screen {
    /// Draw one frame: the lines of the study table, top to bottom.
    ///
    /// The frame is complete; the screen repaints everything and does
    /// no diffing. A 15×15 board frame is about twenty short lines, and
    /// terminals are fast, so a full repaint per input event costs
    /// nothing measurable.
    fn draw(&mut self, frame: &[String]) -> io::Result<()>;

    /// Whether frames for this screen carry ANSI styling.
    ///
    /// The renderer takes styling as a runtime parameter, and the
    /// screen makes the choice: the alternate screen wants colors and
    /// attributes; pipes and CI logs want plain text. The default is
    /// unstyled, so a test screen that records frames gets plain text
    /// without opting out.
    fn styled(&self) -> bool {
        false
    }
}
```

`AltScreen` is the RAII guard. The struct doc comment (lines 46–61)
states the contract and the Drop order: cursor visible and alternate
screen left first (both are escape sequences on stdout), raw mode off
last. `enter` uses `execute!`, which flushes: the mode switch must
take effect before the first frame is queued.

src/tui/screen.rs, lines 62–94:

```rust
pub struct AltScreen;

impl AltScreen {
    /// Put the terminal into TUI mode: raw mode on, alternate screen
    /// active, cursor hidden.
    ///
    /// The cursor is hidden because the shell draws its own cursor as
    /// angle brackets around a board cell; a second, terminal-drawn
    /// cursor would only distract.
    ///
    /// # Errors
    /// Returns an error when the terminal does not accept the mode
    /// switches (for example when stdout is not a terminal).
    pub fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        // `execute!` flushes: the mode switch must take effect before
        // the first frame is queued, not whenever a later flush
        // happens to run.
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(Self)
    }
}

impl Drop for AltScreen {
    fn drop(&mut self) {
        // Errors are ignored with `let _`. If restoration fails there
        // is nothing sensible left to do, and panicking inside Drop
        // during an unwind would abort the process — the opposite of
        // what this guard exists for.
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
```

The draw is the excursion's batching idiom: one clear, every line at
its absolute position, one flush.

src/tui/screen.rs, lines 96–122:

```rust
impl Screen for AltScreen {
    /// Paint the frame: one clear, then every line at its absolute
    /// position, all queued and flushed once.
    ///
    /// Painting a frame is dozens of cursor moves and prints. `queue!`
    /// batches them into one buffer and the single `flush` at the end
    /// sends them together; flushing per line is what would make the
    /// picture flicker. Absolute positioning (`MoveTo(0, y)` per line)
    /// makes the paint independent of where a previous frame left the
    /// cursor.
    fn draw(&mut self, frame: &[String]) -> io::Result<()> {
        let mut out = io::stdout();
        queue!(out, Clear(ClearType::All))?;
        for (y, line) in frame.iter().enumerate() {
            // The frame is about twenty lines, so the y coordinate fits
            // a u16 (the terminal's coordinate type) with room to
            // spare.
            queue!(out, MoveTo(0, y as u16), Print(line))?;
        }
        out.flush()
    }

    /// The full-screen shell owns the terminal, so styling is safe.
    fn styled(&self) -> bool {
        true
    }
}
```

The guard's Drop order is documented in comments, not tested: the
test needs a terminal, and this sub-chapter's tests run headless by
design.

## src/tui/input.rs (new, 706 lines, 19 tests)

The module doc (lines 1–18) states the two rules that shape the file:
the module reads crossterm's key vocabulary but performs no I/O, and
the phase-dependent keys are safe because the session knows the
phase. One constant sets the wrong-phase message for `m`, matching
the session's own error sentence word for word.

src/tui/input.rs, lines 1–24:

```rust
//! Input translation for the full-screen shell: keys in, outcomes out.
//!
//! This module is the pure half of the shell. It reads crossterm's key
//! vocabulary ([`KeyEvent`]) but performs no I/O: a key event plus the
//! input state plus the session go in, and an [`InputOutcome`] comes
//! out. Every key mapping is unit-tested without a terminal.
//!
//! The model is modal, in the style of vim. In normal mode single keys
//! act: arrows and `hjkl` move the board cursor, Enter plays or steps,
//! `?` shows help, `q` quits. `:` opens a command line that accepts the
//! full command language of [`crate::command`]. Keys are only
//! translations to `Command` values; the session core sees no
//! difference between a key and a typed line.
//!
//! Several keys are phase-dependent. The session knows the phase (a
//! Swap2 ceremony in progress, or not, and which ceremony phase), so
//! one key can safely carry one meaning per phase: `u` removes a
//! placement during the ceremony and goes back one ply afterwards.

use crate::command::{self, Command, OpeningPhase};
use crate::opening::OpeningSnapshot;
use crate::session::SessionCore;
use crossterm::event::{KeyCode, KeyEvent};
use engine::{Color, Move};
```

src/tui/input.rs, lines 26–31:

```rust
/// The message for the `m` key at a phase where it does not apply.
///
/// The text matches [`crate::command::CommandError::NotAtFirstChoice`]
/// on purpose: the user sees one sentence for one rule, no matter
/// whether the input layer or the session reports it.
const MORE_ONLY_AT_FIRST_CHOICE: &str = "'more' applies only at the first Swap2 choice";
```

### The state and the outcome

`InputMode` (lines 40–49) is two variants: `Normal`, and
`CommandLine(String)` with the buffer as payload. The doc comment
records why the editor exists: raw mode delivers one keypress at a
time, so appending, Backspace, and echo are this module's job.

`InputState` (lines 48–64) is three public fields with doc comments.
`InputState::new()` (lines 67–81) starts in normal mode with the
cursor on the center point h8 — the natural first target on an empty
board. The `next_color_override` field documents its own lifecycle:
set by `b`/`w`, cleared by the loop when a placement lands.

src/tui/input.rs, lines 48–64:

```rust
/// The shell's input state.
///
/// This is display and editing state, not session state. The session
/// core owns the game; this struct owns what the user's hands are
/// doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputState {
    /// The current mode.
    pub mode: InputMode,
    /// The board cursor: the square the next Enter acts on.
    pub cursor: Move,
    /// The selected color of the next Swap2 stone, set with `b`/`w` at
    /// a placement phase. `None` means the ceremony's default order
    /// applies. The shell clears it when a placement lands, so the
    /// override arms exactly one stone.
    pub next_color_override: Option<Color>,
}
```

`InputOutcome` (lines 94–112) has five variants. The design paper
lists four (`Command`, `Help`, `Quit`, `None`); `Message(String)` is
the fifth, for parse errors and wrong-phase keys. The translation
layer never executes; it reports, and the loop acts.

### The command-line editor

`handle_key` (lines 116–122) dispatches on the mode.
`handle_command_line_key` takes the buffer out of the state with
`mem::replace`, so the mode can change (Esc, Enter) without two live
borrows of `state.mode`. Every path either puts the buffer back or
leaves the state in normal mode.

src/tui/input.rs, lines 129–170:

```rust
fn handle_command_line_key(key: KeyEvent, state: &mut InputState) -> InputOutcome {
    let InputMode::CommandLine(mut buffer) = std::mem::replace(&mut state.mode, InputMode::Normal)
    else {
        unreachable!("handle_key dispatches here only in command-line mode")
    };
    match key.code {
        // Any character appends, including characters that are commands
        // in normal mode. The command line is text, not keys.
        KeyCode::Char(c) => {
            buffer.push(c);
            state.mode = InputMode::CommandLine(buffer);
            InputOutcome::None
        }
        KeyCode::Backspace => {
            buffer.pop();
            state.mode = InputMode::CommandLine(buffer);
            InputOutcome::None
        }
        // Esc cancels: the mode is already Normal (the replace above),
        // and the buffer is dropped with this frame.
        KeyCode::Esc => InputOutcome::None,
        KeyCode::Enter => {
            let text = buffer.trim().to_string();
            // An empty command line closes quietly, the way vim's `:`
            // followed by Enter does nothing.
            if text.is_empty() {
                return InputOutcome::None;
            }
            // One grammar exists, and it lives in `command::parse`. The
            // command line reuses it; a second parser here would drift.
            match command::parse(&text) {
                Ok(cmd) => InputOutcome::Command(cmd),
                Err(err) => InputOutcome::Message(err.to_string()),
            }
        }
        // Arrows and every other key do not edit; the buffer stays open.
        _ => {
            state.mode = InputMode::CommandLine(buffer);
            InputOutcome::None
        }
    }
}
```

Note what the Enter arm returns on a parse error: `Message` with the
parser's own text, and the mode is already normal. The buffer is
dropped; vim drops a failed ex command line the same way.

### The normal-mode table

`handle_normal_key` (lines 180–215) checks the global keys first —
cursor motion, `:`, `?`, `q` — and dispatches the rest on
`core.snapshot().opening`. The comment above the dispatch states the
safety argument: one key per action is safe because the session
always knows the phase.

src/tui/input.rs, lines 207–214:

```rust
    // Phase-dependent keys. The session knows the phase; the snapshot
    // reports it. This is what makes one key per action safe: `u`
    // cannot mean two things at once, because the session always knows
    // which phase the session is in.
    match core.snapshot().opening {
        Some(opening) => opening_key(key, state, &opening),
        None => playing_key(key, state, core),
    }
```

`opening_key` (lines 218–270) is the ceremony table. At a placement
phase, Enter translates to `Play` with the armed override; the
translator does not clear the override, because it cannot know
whether the placement will succeed. `b`/`w` arm the override and
return `None`: the panel shows the selection, so no message is
needed. At a choice phase, Enter and `u` are not mapped.

src/tui/input.rs, lines 220–237 (excerpt):

```rust
        OpeningPhase::Placing3 | OpeningPhase::Placing2 => match key.code {
            // Enter places the next stone at the cursor. The override
            // colors this one stone when set; the loop clears it when
            // the placement lands.
            KeyCode::Enter => InputOutcome::Command(Command::Play {
                mv: state.cursor,
                color: state.next_color_override,
            }),
            // During the ceremony the session routes `back` to the
            // replay log, so `u` removes the last placement.
            KeyCode::Char('u') => InputOutcome::Command(Command::Back { n: 1 }),
            KeyCode::Char('d') => InputOutcome::Command(Command::Done),
            // `b`/`w` select the next stone's color. The panel shows
            // the selection, so the outcome needs no message.
            KeyCode::Char('b') => {
                state.next_color_override = Some(Color::Black);
                InputOutcome::None
            }
```

The `m` guard at the choice phases (lines 259–265): `more` exists only
at the first choice, so the arm carries a guard; the fallthrough arm
returns the same message as the placement phases.

`playing_key` (lines 272–301) is the play table: Enter reads the seat
table (`core.occupant(core.board().to_move())`) and becomes `play` at
the cursor or `step`; `u` is `back`; the ceremony keys are unmapped.
The session still validates the result — an occupied square, a
finished game — and the loop shows the error. Translation is a
suggestion; the session decides.

`move_cursor` (lines 303–315) clamps with the named idioms:
`saturating_sub` up and left (a `u8` cannot go below zero),
`saturating_add(1).min(14)` down and right. The `expect` documents
that clamped coordinates are always valid; it is not error handling.

### The tests

Lines 318–706 hold 19 tests. The ceremony cores are built through the
public session API (`placing_core`, `first_choice_core`,
`placing2_core`, `final_choice_core`), so the tests exercise the real
phases, not mocks of them. One test per mapping group, plus the modal
transitions and the editing edge cases. An example of the style:

```rust
#[test]
fn at_a_placement_phase_m_is_a_message_not_a_command() {
    let core = placing_core();
    let mut state = InputState::new();
    assert_eq!(
        handle_key(char_key('m'), &mut state, &core),
        InputOutcome::Message(MORE_ONLY_AT_FIRST_CHOICE.to_string())
    );
}
```

The test `the_typed_quit_and_help_are_commands_for_the_loop_to_intercept`
pins the interception contract: typed `quit` and `help` translate to
`Command::Quit` and `Command::Help`, and the loop — not the
translator — turns them into shell behavior, because the session core
treats both as no-ops.

## src/tui/render.rs (extended)

### The view

`StudyView` gains two fields, each with a chainable setter.
`message` (lines 97–107) is the bottom line in normal mode; the doc
comment records that the command line owns the bottom line while it
is open. `color_override` (lines 109–119) carries the armed override
from the input state; the selection lives in the shell, not in the
session, so the renderer must be told.

src/tui/render.rs, lines 109–119:

```rust
    /// Report `color` as the selected color of the next Swap2 stone.
    ///
    /// The selection lives in the shell's input state, not in the
    /// session, so the renderer must be told. At a placement phase the
    /// panel shows the override when one is set, else the ceremony's
    /// default color. Outside a placement phase the value is ignored.
    #[must_use]
    pub fn color_override(mut self, color: Option<Color>) -> Self {
        self.color_override = color;
        self
    }
```

### The board

`render_study` passes the opening snapshot into both halves (lines
144–153). The comment at the call site repeats the sub-chapter-c
invariant: the tree board is empty during the ceremony by design, so
the stones to draw come from the opening snapshot.

The cell lookup (lines 258–269) falls back from the board to the
placement list:

```rust
    // Ceremony placements are not in the tree board, so the board
    // lookup falls back to the placement list. The list holds at most
    // five entries; a linear scan is fast enough for a per-frame draw.
    let stone = board.stone_at(mv).or_else(|| {
        opening.and_then(|opening| {
            opening
                .placements
                .iter()
                .find(|(m, _)| *m == mv)
                .map(|(_, color)| *color)
        })
    });
```

Everything downstream — glyph, color, markers, cursor precedence —
is unchanged, because a ceremony stone is a stone. There is no
last-move bracket during the ceremony: the marker belongs to the
tree, and the tree is empty.

### The panel

`render_side_panel` (lines 305–330) splits its middle block by phase.
The existing ply/status/path/branch code moves into `position_lines`
(lines 343–369) unchanged. The ceremony block is new:

src/tui/render.rs, lines 371–423:

```rust
/// The ceremony block of the side panel: the phase, the placements,
/// the color of the next stone at a placement phase, and the keys that
/// apply now.
///
/// The panel teaches the phase-dependent keys at the point where the
/// user needs them: a placement phase shows the placement keys, a
/// choice phase shows the choice keys. The guidance strings match the
/// key table in the help screen word for word.
fn ceremony_lines(
    lines: &mut Vec<String>,
    opening: &OpeningSnapshot,
    color_override: Option<Color>,
) {
    lines.push(format!("Swap2: {:?}", opening.phase));
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
    lines.push(format!("Stones: {stones}"));
    match opening.phase {
        OpeningPhase::Placing3 | OpeningPhase::Placing2 => {
            // The next stone's color is the override when the user
            // selected one with `b`/`w`, else the ceremony's default.
            // The snapshot guarantees a default at a placement phase.
            let (color, source) = match color_override {
                Some(color) => (color, "override"),
                None => (
                    opening
                        .default_next_color
                        .expect("a placement phase always has a default color"),
                    "default",
                ),
            };
            lines.push(format!("Next: {color:?} ({source})"));
            lines.push("Enter place · u undo · d done".to_string());
        }
        OpeningPhase::FirstChoice => {
            // A choice phase has no next stone, so there is no `Next:`
            // line; showing a default here would report a stone that
            // does not exist.
            lines.push("b black · w white · m more".to_string());
        }
        OpeningPhase::FinalChoice => {
            lines.push("b black · w white".to_string());
        }
    }
}
```

The bottom-line rule in `render_study` (lines 170–178): the command
line while it is open, else the message when there is one. One bottom
line exists; the mode decides its content.

The four new tests (lines 653–746): ceremony placements render as
stones with byte offsets (the same technique as the last-move test),
the panel shows phase/stones/next color/guidance and drops the
game-state lines, a choice phase shows no `Next:` line, and the
message/command-line precedence on the bottom line. The pre-existing
styled/unstyled equivalence test passes unchanged: ceremony stones
render through the same `study_cell` path.

## src/tui/study.rs (new, 561 lines, 8 tests)

src/tui/study.rs, lines 1–38:

```rust
//! The study-table shell: one loop over a [`Screen`] and an
//! [`InputSource`].
//!
//! The loop is the rhythm from the TUI excursion: draw the frame, wait
//! for one key, update the state, repeat. It knows nothing about
//! terminals. The screen decides where the frame goes and whether it
//! is styled; the input source decides where keys come from and runs
//! the state machine of [`crate::tui::input`]. Tests drive the same
//! loop with a recording screen and a scripted input source; no test
//! touches a terminal.
//!
//! The loop keeps three pieces of shell state next to the session
//! core: the [`InputState`] (mode, board cursor, color override), the
//! capped list of recent events for the panel, and the message line
//! (the last event summary or the last error text).

use crate::command::{Command, Event};
use crate::repl;
use crate::session::{SessionCore, SessionInit};
use crate::tui::input::{InputMode, InputOutcome, InputState, handle_key};
use crate::tui::render::{StudyView, render_study};
use crate::tui::screen::{AltScreen, Screen};
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use std::io::{self, IsTerminal};

/// The maximum number of events the panel keeps.
///
/// The panel shows only the newest five. The cap keeps a longer margin
/// so the list does not churn at the boundary, and it bounds memory for
/// sessions that run for hours. The oldest events are dropped first.
const EVENT_CAP: usize = 20;

/// Where the shell's input comes from.
///
/// The abstraction exists so the loop can run headless: the full-screen
/// shell reads real keypresses, the tests read a script. Both feed the
/// same [`handle_key`] state machine, so the translation rules are
/// identical on every path.
```

### The input source

The trait has two methods. `wait_any_key` exists for the help screen:
the dismissing key must not also act on the table, and reading it
through the source keeps the discard on the same input path as every
other key.

src/tui/study.rs, lines 39–58:

```rust
pub trait InputSource {
    /// Wait for the next key and translate it into an outcome.
    ///
    /// `state` and `core` are the translator's inputs: the state
    /// changes (cursor motion, buffer edits) happen here, inside the
    /// source, because the source is what reads keys.
    fn next_outcome(
        &mut self,
        state: &mut InputState,
        core: &SessionCore,
    ) -> io::Result<InputOutcome>;

    /// Wait for one keypress and discard it.
    ///
    /// The help screen uses this: any key returns to the table, and
    /// the dismissing key must not also act on the table. Reading it
    /// through the source keeps the discard on the same input path as
    /// every other key.
    fn wait_any_key(&mut self) -> io::Result<()>;
}
```

`RawInput` (lines 61–78) is a unit struct; the work is in
`read_key_press` (lines 85–97), which filters for
`KeyEventKind::Press`. Some platforms report release and repeat
events; without the filter every key would act twice there. Non-key
events are skipped: the loop repaints the whole frame on every key,
so a resize needs no special handling beyond the next paint.

### The loop

`study_loop` (lines 130–174) holds three pieces of shell state: the
input state, the capped event list, and the message line. One
iteration is draw, translate, apply. Two match arms intercept
`Command::Help` and `Command::Quit`: the typed command language
contains both words, and the session core treats them as no-ops, so
the loop turns them into the same behavior as the `?` and `q` keys.

src/tui/study.rs, lines 130–174:

```rust
pub fn study_loop<S: Screen, I: InputSource>(
    core: &mut SessionCore,
    screen: &mut S,
    input: &mut I,
    hint: Option<&str>,
) -> io::Result<()> {
    let mut state = InputState::new();
    let mut events: Vec<Event> = Vec::new();
    // The message line shows the last event summary or the last error
    // text; the caller's hint, when given, is the first message.
    let mut message: Option<String> = hint.map(str::to_string);
    loop {
        let frame = build_frame(core, &state, &events, message.as_deref(), screen.styled());
        screen.draw(&frame)?;
        match input.next_outcome(&mut state, core)? {
            InputOutcome::None => {}
            InputOutcome::Message(text) => message = Some(text),
            InputOutcome::Help => show_help(screen, input)?,
            InputOutcome::Quit => return Ok(()),
            // `help` and `quit` are part of the typed command language,
            // so they can arrive as commands from the command line. The
            // session core treats both as no-ops (they are shell
            // concerns), so the loop intercepts them and they do what
            // the same words do as normal-mode keys.
            InputOutcome::Command(Command::Help) => show_help(screen, input)?,
            InputOutcome::Command(Command::Quit) => return Ok(()),
            InputOutcome::Command(cmd) => match core.execute(cmd) {
                Ok(new_events) => {
                    // The color override arms one stone. A landed
                    // placement disarms it, so the ceremony's default
                    // order resumes; a failed placement leaves it
                    // armed, so the user does not have to re-select.
                    if new_events
                        .iter()
                        .any(|event| matches!(event, Event::StonePlaced { .. }))
                    {
                        state.next_color_override = None;
                    }
                    record_events(&mut events, &mut message, &new_events);
                }
                Err(err) => message = Some(err.to_string()),
            },
        }
    }
}
```

The override disarm is event-driven: the loop reacts to the
`StonePlaced` fact, not to the command's identity. A placement typed
on the command line (`play h8 white`) disarms an armed override too —
the armed stone landed, wherever the command came from.

`build_frame` (lines 183–207) assembles the view. The command line
owns the bottom line while it is open; in normal mode the message
shows, when there is one. `record_events` (lines 210–219) caps the
list at 20, dropping from the front because index 0 is the oldest
event, and sets the message to the batch's last event.

### The help screen

`help_frame` (lines 233–246) reuses `repl::HELP` for the typed
language and appends the normal-mode key table (`NORMAL_MODE_KEYS`,
lines 249–262). Reusing the constant is the point: the help screen
and the line-mode `help` command cannot drift apart. The key table
lists the phase-dependent keys with their phases, because that is the
one thing a user cannot guess.

### The entry point

`run` (lines 105–127) checks `stdout().is_terminal()` first. On a
pipe it returns an `Unsupported` error with a sentence that names
sub-chapter e; writing escape sequences into a pipe would produce
garbage, and a clear error beats garbage. On a terminal it enters the
guard and runs the loop. The `SessionCore` is built here, inside the
guard's lifetime, so a construction panic still restores the
terminal.

src/tui/study.rs, lines 105–118:

```rust
pub fn run(init: SessionInit, hint: Option<&str>) -> io::Result<()> {
    if !io::stdout().is_terminal() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "the study table needs a terminal; the line-mode fallback is sub-chapter e",
        ));
    }
    // The guard restores the terminal on every exit path, panic
    // included. The binding must stay alive until the loop returns.
    let mut screen = AltScreen::enter()?;
    let mut input = RawInput;
    let mut core = SessionCore::new(init);
    study_loop(&mut core, &mut screen, &mut input, hint)
}
```

### The loop tests

The tests (lines 264–561) define the two fakes. `TestScreen` records
every frame in a `Vec`; its `styled()` keeps the default `false`, so
assertions read plain text. `ScriptedInput` plays back a list of key
events and treats exhaustion as an error, not a silent quit: a
shorter-than-intended script must fail the test, not pass it.

src/tui/study.rs, lines 303–310 (excerpt):

```rust
    /// An input source that plays back a scripted list of keys.
    ///
    /// A script must end the session itself (with `q` or a typed
    /// `quit`). Running out of keys is an error, not a silent quit:
    /// a shorter-than-intended script must fail the test, not pass it.
    struct ScriptedInput {
        keys: VecDeque<KeyEvent>,
    }
```

One test as an example of the style — the ceremony path, which also
pins the one-shot override:

```rust
#[test]
fn the_loop_runs_the_ceremony_and_disarms_the_override() {
    let core = SessionCore::new(SessionInit::new_hotseat().with_swap2(true));
    // Enter places h8 Black; `w` arms White; Right+Enter places i8
    // White; Right+Enter places j8 with the default again; `d` ends
    // the round; `b` takes Black; `q` quits.
    let keys = vec![
        key(KeyCode::Enter),
        char_key('w'),
        key(KeyCode::Right),
        key(KeyCode::Enter),
        key(KeyCode::Right),
        key(KeyCode::Enter),
        char_key('d'),
        char_key('b'),
        char_key('q'),
    ];
    let (core, screen) = run_scripted(core, keys).expect("a scripted ceremony runs clean");

    // The ceremony finished: the tree root holds the opening board
    // and White moves first.
    assert!(core.snapshot().opening.is_none());
    assert_eq!(core.board().to_move(), Color::White);
    // j8 is Black — proof that the override armed exactly one stone.
    assert_eq!(core.board().stone_at(Move::new(7, 9).unwrap()), Some(Color::Black));
}
```

The assertions on recorded frames use the frame index as a timeline:
frame 0 is the initial frame, and the key at index *i* is processed
before frame *i*+1 is drawn, so `frames[2]` is the frame after the
`w` key.

## Deviations from the design paper's wording

Three, all small:

1. **`InputOutcome` has five variants, not four.** The paper lists
   `Command`, `Help`, `Quit`, `None`. `Message(String)` is the fifth:
   the paper's own error-message outcome for the command-line parse
   failure, and the channel for the wrong-phase `m` key. The
   alternative — executing the command and letting the session's
   typed error appear — was rejected for `m` during placement: an
   error with no visible change behind it reads as a malfunction, and
   the message names the phase where the key applies.
2. **`Screen` has a defaulted second method.** `styled()` puts the
   styling choice on the screen, which is where sub-chapter b's
   module documentation already assigned it. The alternative — a
   `styled: bool` parameter on `study_loop` — would let a caller
   combine a screen and a styling setting that do not match.
3. **`u` and Enter are unmapped at the choice phases.** The paper's
   key table assigns keys per phase; at a choice phase only the
   choice keys act. `u` during a choice would remove the stone that
   completed the round, taking the `done` back with it — legal in the
   session, but not in the assigned key set, so the key is unmapped
   and documented as such.

## Verification

From `gomoku/`, after the sub-chapter:

* `cargo test -p cli` — 132 tests pass (101 before): 78 lib tests (47
  before; +19 input tests, +8 loop tests, +4 render tests), and the
  unchanged suites (7 actor, 9 history, 20 opening, 13 session, 4
  study, 1 doctest).
* `cargo clippy --all-targets -- -D warnings` — clean.
* `cargo fmt --all -- --check` — clean.
* Both demo transcripts are byte-identical to before. The shell adds
  no output path to sessiondemo; the repl code is untouched.

A note on size: `input.rs` is 706 lines and `study.rs` is 561, past
the chapter's 250-line budget. Counted without tests, input.rs is
317 lines and study.rs is 263 — still past, driven by the chapter's
own commenting standard and by the per-phase match arms, which are
table entries more than logic. Each file holds one concern (the input
state machine; the loop), which is the budget's stated purpose. A
split for size alone would separate the key table from its tests, and
that trade reads worse than the overage.
