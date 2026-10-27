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

`AltScreen` is the RAII guard. Its doc comment states the contract
and the Drop order: cursor visible and alternate screen left first
(both are escape sequences on stdout), raw mode off last.

src/tui/screen.rs, lines 45–61:

```rust
/// The full-screen terminal screen, and the RAII guard for it.
///
/// Raw mode and the alternate screen are global, sticky terminal
/// state. If the program returns early, errors out, or panics while
/// they are active, the user's terminal stays broken: no echo, no
/// line buffering, no scrollback. The value's `Drop` restores the
/// terminal, and `Drop` runs on every exit path, panic included. The
/// value carries no data; its job is to stay alive until the shell is
/// done.
///
/// The Drop order is fixed and deliberate: show the cursor and leave
/// the alternate screen first (both are escape sequences on stdout),
/// then disable raw mode (a terminal attribute). Writing escape
/// sequences after raw mode is off would still work, but restoring
/// the visible state before the input state reads better in a
/// terminal log and keeps every stdout write inside the raw-mode
/// section of the program's life.
```

`enter` uses `execute!`, which flushes: the mode switch must
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

## src/tui/input.rs (new, 672 lines, 19 tests)

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

`InputMode` is two variants: `Normal`, and `CommandLine(String)` with
the buffer as payload. The doc comment records why the editor exists:
raw mode delivers one keypress at a time, so appending, Backspace,
and echo are this module's job.

src/tui/input.rs, lines 33–46:

```rust
/// The input mode of the shell.
///
/// Two modes exist. Normal mode translates single keys directly. The
/// command line is a small editor over a `String` buffer: raw mode
/// gives the program one keypress at a time, so appending, Backspace,
/// and echo are this module's job, not the terminal driver's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    /// Single keys act immediately.
    Normal,
    /// A `:` command line is open; the payload is the buffer typed so
    /// far.
    CommandLine(String),
}
```

`InputState` (lines 48–59) is two public fields with doc comments:
the mode and the board cursor.

src/tui/input.rs, lines 48–59:

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
}
```

`InputState::new()` starts in normal mode with the cursor on the
center point h8 — the natural first target on an empty board. There
is no color field: the strict alternation fixes the next stone's
color, so the input state has nothing to select.

src/tui/input.rs, lines 62–74:

```rust
    /// Fresh input state: normal mode, the cursor on the center point
    /// h8.
    ///
    /// The center is the natural first target on an empty board and the
    /// most common first stone of a Swap2 ceremony, so the cursor
    /// starts there instead of in a corner.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode: InputMode::Normal,
            cursor: Move::new(7, 7).expect("the center point is on the board"),
        }
    }
```

`InputOutcome` has five variants. The design paper lists four
(`Command`, `Help`, `Quit`, `None`); `Message(String)` is the fifth,
for parse errors and wrong-phase keys. The translation layer never
executes; it reports, and the loop acts.

src/tui/input.rs, lines 83–101:

```rust
/// What one key press means for the shell.
///
/// The translation layer never executes anything. It reports what the
/// key means, and the loop in [`crate::tui::study`] acts on the report.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// The key changed only input state (cursor moved, buffer edited),
    /// or the key is not mapped. Nothing reaches the session.
    None,
}
```

### The command-line editor

`handle_key` dispatches on the mode.

src/tui/input.rs, lines 103–115:

```rust
/// Translate one key press into an outcome.
///
/// The function is pure: it mutates only `state` and touches no I/O.
/// `core` is read-only; it supplies the phase (the Swap2 snapshot) and
/// the seat table (for Enter during play). Two small machines share the
/// work: the command-line editor while a command line is open, and the
/// normal-mode mapping otherwise.
pub fn handle_key(key: KeyEvent, state: &mut InputState, core: &SessionCore) -> InputOutcome {
    match &state.mode {
        InputMode::Normal => handle_normal_key(key, state, core),
        InputMode::CommandLine(_) => handle_command_line_key(key, state),
    }
}
```

`handle_command_line_key` takes the buffer out of the state with
`mem::replace`, so the mode can change (Esc, Enter) without two live
borrows of `state.mode`. Every path either puts the buffer back or
leaves the state in normal mode.

src/tui/input.rs, lines 123–164:

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

`handle_normal_key` checks the global keys first —
cursor motion, `:`, `?`, `q`:

src/tui/input.rs, lines 166–199:

```rust
/// One key in normal mode: the translation table.
///
/// The table is in two parts. The global keys (cursor motion, `:`,
/// `?`, `q`) mean the same thing in every phase and are checked first.
/// The remaining keys are phase-dependent and dispatch on the session
/// snapshot. Modifiers are not inspected: `?` arrives as `Char('?')`
/// with the Shift modifier on most layouts, and matching the code alone
/// keeps the mapping independent of the layout.
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
        _ => {}
    }
```

The rest dispatches on `core.snapshot().opening`. The comment above
the dispatch states the safety argument: one key per action is safe
because the session always knows the phase.

src/tui/input.rs, lines 201–209:

```rust
    // Phase-dependent keys. The session knows the phase; the snapshot
    // reports it. This is what makes one key per action safe: `u`
    // cannot mean two things at once, because the session always knows
    // which phase the session is in.
    match core.snapshot().opening {
        Some(opening) => opening_key(key, state, &opening),
        None => playing_key(key, state, core),
    }
}
```

`opening_key` is the ceremony table. At a placement phase, Enter
translates to `Play` at the cursor and the strict alternation decides
the color, so `b`/`w` are unmapped there: the comment in the arm says
why. At a choice phase, Enter and `u` are not mapped.

src/tui/input.rs, lines 211–253:

```rust
/// The phase-dependent keys during the Swap2 ceremony.
fn opening_key(key: KeyEvent, state: &mut InputState, opening: &OpeningSnapshot) -> InputOutcome {
    match opening.phase {
        OpeningPhase::Placing3 | OpeningPhase::Placing2 => match key.code {
            // Enter places the next stone at the cursor; the
            // ceremony's strict alternation decides its color.
            KeyCode::Enter => InputOutcome::Command(Command::Play { mv: state.cursor }),
            // During the ceremony the session routes `back` to the
            // replay log, so `u` removes the last placement.
            KeyCode::Char('u') => InputOutcome::Command(Command::Back { n: 1 }),
            KeyCode::Char('d') => InputOutcome::Command(Command::Done),
            // `b`/`w` bind to the color choice, which exists only at a
            // choice phase. At a placement phase the keys are unmapped:
            // the alternation fixes the next stone's color, so there
            // is nothing to select.
            // `m` is the decline of the first choice. During placement
            // it applies nowhere, so the key explains itself instead of
            // producing the session's typed error with no visible
            // context.
            KeyCode::Char('m') => InputOutcome::Message(MORE_ONLY_AT_FIRST_CHOICE.to_string()),
            _ => InputOutcome::None,
        },
        // At a choice phase the only stone-related keys are the choice
        // itself. Enter places nothing here, and `u` is not mapped:
        // removing a placement belongs to the placement phases.
        OpeningPhase::FirstChoice | OpeningPhase::FinalChoice => match key.code {
            KeyCode::Char('b') => InputOutcome::Command(Command::ChooseColor {
                color: Color::Black,
            }),
            KeyCode::Char('w') => InputOutcome::Command(Command::ChooseColor {
                color: Color::White,
            }),
            // `more` exists only at the first choice. At the final
            // choice the key gets the same message as at a placement
            // phase, so the rule has one sentence everywhere.
            KeyCode::Char('m') if opening.phase == OpeningPhase::FirstChoice => {
                InputOutcome::Command(Command::More)
            }
            KeyCode::Char('m') => InputOutcome::Message(MORE_ONLY_AT_FIRST_CHOICE.to_string()),
            _ => InputOutcome::None,
        },
    }
}
```

The `m` guard at the choice phases: `more` exists only at the
first choice, so the arm carries a guard; the fallthrough arm returns
the same message as the placement phases.

`playing_key` is the play table: Enter reads the seat table
(`core.occupant(core.board().to_move())`) and becomes `play` at the
cursor or `step`; `u` is `back`; the ceremony keys are unmapped.
The session still validates the result — an occupied square, a
finished game — and the loop shows the error. Translation is a
suggestion; the session decides.

src/tui/input.rs, lines 255–275:

```rust
/// The phase-dependent keys during play, after any ceremony finished.
fn playing_key(key: KeyEvent, state: &InputState, core: &SessionCore) -> InputOutcome {
    match key.code {
        // Enter is the obvious next action: a human seat plays at the
        // cursor, an AI seat searches. The seat table decides; the
        // session validates the result (an occupied square, a finished
        // game) and the loop shows the error.
        KeyCode::Enter => {
            let side = core.board().to_move();
            if core.occupant(side).is_human() {
                InputOutcome::Command(Command::Play { mv: state.cursor })
            } else {
                InputOutcome::Command(Command::Step)
            }
        }
        KeyCode::Char('u') => InputOutcome::Command(Command::Back { n: 1 }),
        // The ceremony keys (`d`, `b`, `w`, `m`) have no meaning during
        // play and are ignored like any other unmapped key.
        _ => InputOutcome::None,
    }
}
```

`move_cursor` clamps with the named idioms:
`saturating_sub` up and left (a `u8` cannot go below zero),
`saturating_add(1).min(14)` down and right. The `expect` documents
that clamped coordinates are always valid; it is not error handling.

src/tui/input.rs, lines 277–296:

```rust
/// Move the board cursor one step, clamped to the board edges.
///
/// Up and left use `saturating_sub`: a `u8` cannot go below zero, and
/// saturating_sub clamps at the edge instead of underflowing. Down and
/// right use `min` with 14, the last valid index on a 15×15 board. The
/// clamped coordinates are always valid, so `Move::new` cannot fail;
/// the `expect` documents that invariant instead of handling an error.
fn move_cursor(cursor: &mut Move, dr: i8, dc: i8) {
    let row = match dr {
        -1 => cursor.row().saturating_sub(1),
        1 => cursor.row().saturating_add(1).min(14),
        _ => cursor.row(),
    };
    let col = match dc {
        -1 => cursor.col().saturating_sub(1),
        1 => cursor.col().saturating_add(1).min(14),
        _ => cursor.col(),
    };
    *cursor = Move::new(row, col).expect("clamped coordinates stay on the board");
}
```

### The tests

Lines 299–672 hold 19 tests. The shared helpers build keys, type
text, and start sessions:

src/tui/input.rs, lines 305–325:

```rust
    /// A key event with no modifiers, as a terminal reports a plain
    /// keypress. `KeyEvent::new` marks the event as a press.
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn char_key(c: char) -> KeyEvent {
        key(KeyCode::Char(c))
    }

    /// Type a string into an open command line, one `Char` key at a time.
    fn type_text(state: &mut InputState, core: &SessionCore, text: &str) {
        for c in text.chars() {
            let outcome = handle_key(char_key(c), state, core);
            assert_eq!(outcome, InputOutcome::None, "typing '{c}' edits only");
        }
    }

    fn playing_core() -> SessionCore {
        SessionCore::new(SessionInit::new_hotseat())
    }
```

The ceremony cores are built through the public session API, so the
tests exercise the real phases, not mocks of them.

src/tui/input.rs, lines 327–365:

```rust
    /// A session at the first placement phase.
    fn placing_core() -> SessionCore {
        SessionCore::new(SessionInit::new_hotseat().with_swap2(true))
    }

    /// A session at the first choice: three alternating placements,
    /// `done`.
    fn first_choice_core() -> SessionCore {
        let mut core = placing_core();
        for (r, c) in [(7, 7), (8, 8), (9, 9)] {
            core.execute(Command::Play {
                mv: Move::new(r, c).unwrap(),
            })
            .unwrap();
        }
        core.execute(Command::Done).unwrap();
        core
    }

    /// A session at the second placement phase: the first choice
    /// declined with `more`.
    fn placing2_core() -> SessionCore {
        let mut core = first_choice_core();
        core.execute(Command::More).unwrap();
        core
    }

    /// A session at the final choice: both rounds placed and confirmed.
    fn final_choice_core() -> SessionCore {
        let mut core = placing2_core();
        for (r, c) in [(5, 5), (6, 6)] {
            core.execute(Command::Play {
                mv: Move::new(r, c).unwrap(),
            })
            .unwrap();
        }
        core.execute(Command::Done).unwrap();
        core
    }
```

One test per mapping group, plus the modal
transitions and the editing edge cases. An example of the style:

src/tui/input.rs, lines 516–524:

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

src/tui/input.rs, lines 651–671:

```rust
    #[test]
    fn the_typed_quit_and_help_are_commands_for_the_loop_to_intercept() {
        // `quit` and `help` typed on the command line translate to the
        // same Command values as everything else. The loop intercepts
        // them, because the session core treats both as no-ops.
        let core = playing_core();
        let mut state = InputState::new();
        handle_key(char_key(':'), &mut state, &core);
        type_text(&mut state, &core, "quit");
        assert_eq!(
            handle_key(key(KeyCode::Enter), &mut state, &core),
            InputOutcome::Command(Command::Quit)
        );

        handle_key(char_key(':'), &mut state, &core);
        type_text(&mut state, &core, "help");
        assert_eq!(
            handle_key(key(KeyCode::Enter), &mut state, &core),
            InputOutcome::Command(Command::Help)
        );
    }
```

## src/tui/render.rs (extended)

### The view

`StudyView` gains one field with a chainable setter. `message` is
the bottom line in normal mode; the doc comment records that the
command line owns the bottom line while it is open. The view needs no
color input: the snapshot's `next_color` already states the
alternation's answer.

src/tui/render.rs, lines 95–105:

```rust
    /// Show `text` as the bottom line of the frame.
    ///
    /// One bottom line exists. While the command line is open it shows
    /// the buffer and the message is not drawn; in normal mode the
    /// message carries the last event summary or the last error text.
    /// The line is never styled: it is feedback text, not furniture.
    #[must_use]
    pub fn message(mut self, text: &'a str) -> Self {
        self.message = Some(text);
        self
    }
```

### The board

`render_study` passes the opening snapshot into both halves. The
comment at the call site repeats the sub-chapter-c invariant: the
tree board is empty during the ceremony by design, so the stones to
draw come from the opening snapshot. The bottom-line rule closes the
function: the command line while it is open, else the message when
there is one. One bottom line exists; the mode decides its content.

src/tui/render.rs, lines 126–158:

```rust
pub fn render_study(view: &StudyView) -> Vec<String> {
    let core = view.core;
    let board = core.board();
    let snapshot = core.snapshot();
    // During a Swap2 ceremony the tree board is empty by design (the
    // placements are not game moves), so the stones to draw come from
    // the opening snapshot.
    let opening = snapshot.opening.as_ref();
    let board_lines = study_board_lines(
        &board,
        opening,
        snapshot.last_move,
        view.cursor,
        view.styled,
    );
    let panel_lines = render_side_panel(core, &board, &snapshot, view.recent_events, view.styled);
    let rows = board_lines.len().max(panel_lines.len());
    let mut out = Vec::with_capacity(rows + 1);
    for i in 0..rows {
        let left = board_lines.get(i).map(String::as_str).unwrap_or("");
        let right = panel_lines.get(i).map(String::as_str).unwrap_or("");
        out.push(format!("{left}  {right}"));
    }
    // One bottom line: the command line while it is open (the user's
    // text always at the bottom of the screen), else the message when
    // the loop has one to show.
    if let Some(buffer) = view.command_line {
        out.push(format!(":{buffer}"));
    } else if let Some(text) = view.message {
        out.push(text.to_string());
    }
    out
}
```

The cell lookup falls back from the board to the
placement list:

src/tui/render.rs, lines 237–248:

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

`render_side_panel` splits its middle block by phase.

src/tui/render.rs, lines 284–317:

```rust
fn render_side_panel(
    core: &SessionCore,
    board: &Board,
    snapshot: &Snapshot,
    recent_events: &[Event],
    styled: bool,
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
    lines.push(String::new());
    lines.push("Last events:".to_string());
    if recent_events.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        // The five most recent events, oldest first: the latest event is
        // the bottom line of the list.
        for ev in recent_events.iter().rev().take(5).rev() {
            lines.push(format!("  {ev}"));
        }
    }
    lines.push(String::new());
    lines.push(panel_line("? help · q quit", styled));
    lines
}
```

The existing ply/status/path/branch code moves into `position_lines`
unchanged.

src/tui/render.rs, lines 319–347:

```rust
/// The game-state block of the side panel: ply, side to move or game
/// result, path tail, and branch options.
fn position_lines(lines: &mut Vec<String>, board: &Board, snapshot: &Snapshot) {
    lines.push(format!("Ply: {}", snapshot.ply));
    // One line covers both states a game can be in: waiting for a side,
    // or finished with a result.
    match snapshot.status {
        Status::Ongoing => lines.push(format!("To move: {:?}", board.to_move())),
        Status::Won(color) => lines.push(format!("Game over: {color:?} wins")),
        Status::Draw => lines.push("Game over: draw".to_string()),
    }
    lines.push(format!("Path: {}", path_tail_string(board)));
    // One continuation is the main line, not a choice. The panel lists
    // options only when the cursor is at a real branch point.
    if snapshot.branch_options.len() > 1 {
        let opts: Vec<String> = snapshot
            .branch_options
            .iter()
            // The numbers are the 0-based index the `var` command takes,
            // so what the user reads is what the user types.
            .map(|(i, m)| format!("[{i}] {}", algebraic(*m)))
            .collect();
        lines.push(format!(
            "{} variations: {}",
            snapshot.branch_options.len(),
            opts.join(" ")
        ));
    }
}
```

The ceremony block is new:

src/tui/render.rs, lines 349–392:

```rust
/// The ceremony block of the side panel: the phase, the placements,
/// the color of the next stone at a placement phase, and the keys that
/// apply now.
///
/// The panel teaches the phase-dependent keys at the point where the
/// user needs them: a placement phase shows the placement keys, a
/// choice phase shows the choice keys. The guidance strings match the
/// key table in the help screen word for word.
fn ceremony_lines(lines: &mut Vec<String>, opening: &OpeningSnapshot) {
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
            // The strict alternation fixes the next stone's color, so
            // the panel states it without a source: there is nothing
            // to select. The snapshot guarantees a color at a
            // placement phase.
            let color = opening
                .next_color
                .expect("a placement phase always has a next color");
            lines.push(format!("Next: {color:?}"));
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

The bottom-line rule of `render_study` applies to the whole frame:
the command line while it is open, else the message when there is
one. The four new tests (lines 611–699): ceremony placements render as
stones with byte offsets (the same technique as the last-move test),
the panel shows phase/stones/next color/guidance and drops the
game-state lines, a choice phase shows no `Next:` line, and the
message/command-line precedence on the bottom line. The pre-existing
styled/unstyled equivalence test passes unchanged: ceremony stones
render through the same `study_cell` path.

## src/tui/study.rs (new, 542 lines, 8 tests)

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
//! core: the [`InputState`] (mode and board cursor), the capped list
//! of recent events for the panel, and the message line (the last
//! event summary or the last error text).

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

`RawInput` is a unit struct; the work is in `read_key_press`, which
filters for `KeyEventKind::Press`.

src/tui/study.rs, lines 60–75:

```rust
/// The raw-mode input source: one terminal keypress per call.
pub struct RawInput;

impl InputSource for RawInput {
    fn next_outcome(
        &mut self,
        state: &mut InputState,
        core: &SessionCore,
    ) -> io::Result<InputOutcome> {
        Ok(handle_key(read_key_press()?, state, core))
    }

    fn wait_any_key(&mut self) -> io::Result<()> {
        read_key_press().map(|_| ())
    }
}
```

The filter and its reasoning:

src/tui/study.rs, lines 77–93:

```rust
/// Read one key press from the terminal.
///
/// crossterm reports press, release, and repeat events on some
/// platforms (Windows); Unix terminals report presses only. Filtering
/// on `KeyEventKind::Press` keeps every key acting exactly once on
/// every platform. Non-key events (resize, mouse) are skipped: the
/// loop repaints the full frame on every key anyway, so a resize needs
/// no special handling beyond the next paint.
fn read_key_press() -> io::Result<KeyEvent> {
    loop {
        if let CrosstermEvent::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            return Ok(key);
        }
    }
}
```

Some platforms report release and repeat events; without the
filter every key would act twice there. Non-key events are skipped:
the loop repaints the whole frame on every key, so a resize needs no
special handling beyond the next paint.

### The loop

`study_loop` (lines 130–164) holds three pieces of shell state: the
input state, the capped event list, and the message line. One
iteration is draw, translate, apply. Two match arms intercept
`Command::Help` and `Command::Quit`: the typed command language
contains both words, and the session core treats them as no-ops, so
the loop turns them into the same behavior as the `?` and `q` keys.

src/tui/study.rs, lines 130–164:

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
                    record_events(&mut events, &mut message, &new_events);
                }
                Err(err) => message = Some(err.to_string()),
            },
        }
    }
}
```

`build_frame` (lines 166–191) assembles the view. The command line
owns the bottom line while it is open; in normal mode the message
shows, when there is one.

src/tui/study.rs, lines 166–191:

```rust
/// Assemble the frame for one loop iteration.
///
/// The view carries what the renderer cannot know: the board cursor
/// lives in the input state, and the message line lives in the loop.
/// One bottom line exists: while the command line is open it shows
/// the buffer; in normal mode it shows the message, when there is
/// one.
fn build_frame(
    core: &SessionCore,
    state: &InputState,
    events: &[Event],
    message: Option<&str>,
    styled: bool,
) -> Vec<String> {
    let view = StudyView::new(core, events)
        .cursor(state.cursor)
        .styled(styled);
    let view = match &state.mode {
        InputMode::CommandLine(buffer) => view.command_line(buffer),
        InputMode::Normal => match message {
            Some(text) => view.message(text),
            None => view,
        },
    };
    render_study(&view)
}
```

`record_events` (lines 193–207) caps the list at 20, dropping from
the front because index 0 is the oldest event, and sets the message
to the batch's last event.

src/tui/study.rs, lines 193–207:

```rust
/// Append one batch of events to the capped list and update the
/// message line.
///
/// The message is the summary of the batch's last event: the newest
/// fact about the session. The list drops from the front when it
/// exceeds the cap, because index 0 is the oldest event.
fn record_events(events: &mut Vec<Event>, message: &mut Option<String>, new_events: &[Event]) {
    if let Some(last) = new_events.last() {
        *message = Some(last.to_string());
    }
    events.extend_from_slice(new_events);
    let excess = events.len().saturating_sub(EVENT_CAP);
    // `drain` is lazy; `drop` on the iterator performs the removal.
    drop(events.drain(..excess));
}
```

### The help screen

`show_help` draws the frame and waits for one key through the
source; `help_frame` reuses `repl::HELP` for the typed language and
appends the normal-mode key table (`NORMAL_MODE_KEYS`). Reusing the
constant is the point: the help screen and the line-mode `help`
command cannot drift apart. The key table lists the phase-dependent
keys with their phases, because that is the one thing a user cannot
guess; `b`/`w` appear only as the choice keys, because the strict
alternation leaves nothing to select at a placement phase.

src/tui/study.rs, lines 209–231:

```rust
/// Draw the full-screen help and wait for one key to dismiss it.
fn show_help<S: Screen, I: InputSource>(screen: &mut S, input: &mut I) -> io::Result<()> {
    screen.draw(&help_frame())?;
    input.wait_any_key()
}

/// The help frame: the typed command language, then the normal-mode
/// key table, then the dismissal note.
///
/// The command-language part reuses [`repl::HELP`], so the help screen
/// and the line-mode `help` command cannot drift apart. The frame is
/// unstyled text: it must stay readable on every terminal, and the
/// help screen has no stones to highlight.
fn help_frame() -> Vec<String> {
    // `HELP` ends with a newline; `str::lines` drops it, so the blank
    // separator between the two parts is added explicitly.
    let mut lines: Vec<String> = repl::HELP.lines().map(str::to_string).collect();
    lines.push(String::new());
    lines.extend(NORMAL_MODE_KEYS.lines().map(str::to_string));
    lines.push(String::new());
    lines.push("Press any key to return to the table.".to_string());
    lines
}
```

The key table constant:

src/tui/study.rs, lines 233–249:

```rust
/// The normal-mode half of the help text.
///
/// The table lists the phase-dependent keys with their phases, because
/// that is the one thing a user cannot guess: `u` undoes a placement
/// during the ceremony and goes back one ply afterwards.
const NORMAL_MODE_KEYS: &str = r#"Normal-mode keys (single keys, no Enter):
  arrows, hjkl   move the board cursor
  Enter          play at the cursor (human seat), step the AI (AI seat);
                 during Swap2 placement: place the next stone
  u              undo: remove a Swap2 placement / go back one ply
  d              Swap2 placement: end the round (done)
  b, w           Swap2 choice: choose that color
  m              Swap2 first choice: place two more stones (more)
  :              open the command line (Esc cancels, Enter executes)
  ?              show this help
  q              quit
"#;
```

### The entry point

`run` checks `stdout().is_terminal()` first. On a pipe it returns
an `Unsupported` error with a sentence that names
sub-chapter e; writing escape sequences into a pipe would produce
garbage, and a clear error beats garbage. On a terminal it enters the
guard and runs the loop. The `SessionCore` is built here, inside the
guard's lifetime, so a construction panic still restores the
terminal.

src/tui/study.rs, lines 95–118:

```rust
/// Run the study table on the full screen.
///
/// `hint`, when given, is the first message on the message line — for
/// example a note that the session starts inside the Swap2 ceremony.
///
/// # Errors
/// Returns an error when stdout is not a terminal. The line-mode
/// fallback for pipes and CI is sub-chapter e; until then the shell
/// refuses instead of writing escape sequences into a pipe. Also
/// returns any I/O error from the terminal itself.
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

The tests (lines 252–542) define the two fakes. `TestScreen` records
every frame in a `Vec`; its `styled()` keeps the default `false`, so
assertions read plain text.

src/tui/study.rs, lines 259–289:

```rust
    /// A screen that records every frame it is asked to draw.
    ///
    /// The terminal cannot be asserted on; a `Vec` can. The recorded
    /// frames are unstyled, because the default `styled()` is false.
    struct TestScreen {
        frames: Vec<Vec<String>>,
    }

    impl TestScreen {
        fn new() -> Self {
            Self { frames: Vec::new() }
        }

        /// All recorded frames joined into one text, for content
        /// assertions that do not care about frame boundaries.
        fn all_text(&self) -> String {
            self.frames
                .iter()
                .flat_map(|frame| frame.iter())
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    impl Screen for TestScreen {
        fn draw(&mut self, frame: &[String]) -> io::Result<()> {
            self.frames.push(frame.to_vec());
            Ok(())
        }
    }
```

`ScriptedInput` plays back a list of key events and treats
exhaustion as an error, not a silent quit: a shorter-than-intended
script must fail the test, not pass it.

src/tui/study.rs, lines 291–298 (excerpt):

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

The implementation plays the keys back in order:

src/tui/study.rs, lines 300–324:

```rust
    impl ScriptedInput {
        fn new(keys: Vec<KeyEvent>) -> Self {
            Self { keys: keys.into() }
        }

        fn pop(&mut self) -> io::Result<KeyEvent> {
            self.keys.pop_front().ok_or_else(|| {
                io::Error::new(io::ErrorKind::UnexpectedEof, "the script ran out of keys")
            })
        }
    }

    impl InputSource for ScriptedInput {
        fn next_outcome(
            &mut self,
            state: &mut InputState,
            core: &SessionCore,
        ) -> io::Result<InputOutcome> {
            Ok(handle_key(self.pop()?, state, core))
        }

        fn wait_any_key(&mut self) -> io::Result<()> {
            self.pop().map(|_| ())
        }
    }
```

The key constructors, the typed-line helper, and the scripted runner:

src/tui/study.rs, lines 326–352:

```rust
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn char_key(c: char) -> KeyEvent {
        key(KeyCode::Char(c))
    }

    /// A typed command line as a list of keys: `:`, the text, Enter.
    fn typed_line(text: &str) -> Vec<KeyEvent> {
        let mut keys = vec![char_key(':')];
        keys.extend(text.chars().map(char_key));
        keys.push(key(KeyCode::Enter));
        keys
    }

    /// Run the loop to completion over a scripted session and return
    /// the recorded frames.
    fn run_scripted(
        mut core: SessionCore,
        keys: Vec<KeyEvent>,
    ) -> io::Result<(SessionCore, TestScreen)> {
        let mut screen = TestScreen::new();
        let mut input = ScriptedInput::new(keys);
        study_loop(&mut core, &mut screen, &mut input, None)?;
        Ok((core, screen))
    }
```

One test as an example of the style — the ceremony path, which also
pins the strict alternation across the frames:

src/tui/study.rs, lines 390–436:

```rust
    #[test]
    fn the_loop_runs_the_ceremony_on_alternated_placements() {
        let core = SessionCore::new(SessionInit::new_hotseat().with_swap2(true));
        // Enter places h8 Black; Right+Enter places i8 White;
        // Right+Enter places j8 Black; `d` ends the round; `b` takes
        // Black; `q` quits. The colors come from the strict
        // alternation — no key selects them.
        let keys = vec![
            key(KeyCode::Enter),
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
        assert_eq!(
            core.board().stone_at(Move::new(7, 7).unwrap()),
            Some(Color::Black)
        );
        assert_eq!(
            core.board().stone_at(Move::new(7, 8).unwrap()),
            Some(Color::White)
        );
        assert_eq!(
            core.board().stone_at(Move::new(7, 9).unwrap()),
            Some(Color::Black)
        );

        // Frame 0 is the initial frame; the key at index i is processed
        // before frame i+1 is drawn. Frame 1 follows the placement of
        // h8: the panel names the alternation's next color.
        assert!(
            screen.frames[1]
                .iter()
                .any(|line| line.contains("Next: White")),
            "the panel shows the next stone's color: {:?}",
            screen.frames[1]
        );
    }
```

The assertions on recorded frames use the frame index as a timeline:
frame 0 is the initial frame, and the key at index *i* is processed
before frame *i*+1 is drawn, so `frames[1]` is the frame after the
first placement.

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

* `cargo test -p cli` — 130 tests pass (99 before): 78 lib tests (47
  before; +19 input tests, +8 loop tests, +4 render tests), and the
  unchanged suites (7 actor, 9 history, 18 opening, 13 session, 4
  study, 1 doctest).
* `cargo clippy --all-targets -- -D warnings` — clean.
* `cargo fmt --all -- --check` — clean.
* Both demo transcripts are byte-identical to before. The shell adds
  no output path to sessiondemo; the repl code is untouched.

A note on size: `input.rs` is 672 lines and `study.rs` is 542, past
the chapter's 250-line budget. Counted without tests, input.rs is
297 lines and study.rs is 251 — still past, driven by the chapter's
own commenting standard and by the per-phase match arms, which are
table entries more than logic. Each file holds one concern (the input
state machine; the loop), which is the budget's stated purpose. A
split for size alone would separate the key table from its tests, and
that trade reads worse than the overage.
