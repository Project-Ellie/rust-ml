//! The study-table shell: one loop over a [`Screen`] and an
//! [`InputSource`].
//!
//! The loop is the rhythm from the TUI excursion: draw the frame, wait
//! for one input, update the state, repeat. It knows nothing about
//! terminals. The screen decides where the frame goes and whether it
//! is styled; the input source decides where input comes from. Two
//! sources exist. [`RawInput`] reads raw-mode keypresses and runs the
//! state machine of [`crate::tui::input`]. [`LineInput`] reads
//! complete lines, the fallback for pipes, CI, and redirects. Tests
//! drive the same loop with a recording screen and scripted input; no
//! test touches a terminal.
//!
//! Both [`InputSource`] implementations live here, next to the trait,
//! and not in [`crate::tui::input`]: that module is the pure half of
//! the shell (its own documentation states that it performs no I/O),
//! and an input source is the I/O edge — [`RawInput`] reads terminal
//! events, [`LineInput`] reads a byte stream.
//!
//! The loop keeps three pieces of shell state next to the session
//! core: the [`InputState`] (mode and board cursor), the capped list
//! of recent events for the panel, and the message line (the last
//! event summary or the last error text).

use crate::command::{self, Command, Event};
use crate::repl;
use crate::session::{SessionCore, SessionInit};
use crate::tui::input::{InputMode, InputOutcome, InputState, handle_key};
use crate::tui::render::{StudyView, render_study};
use crate::tui::screen::{AltScreen, LineScreen, Screen};
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use engine::{Move, Status};
use std::io::{self, BufRead, IsTerminal};

/// The maximum number of events the panel keeps.
///
/// The panel shows only the newest five. The cap keeps a longer margin
/// so the list does not churn at the boundary, and it bounds memory for
/// sessions that run for hours. The oldest events are dropped first.
const EVENT_CAP: usize = 20;

/// Where the shell's input comes from.
///
/// The abstraction exists so the loop can run terminal-agnostic: the
/// full-screen shell reads real keypresses ([`RawInput`]), the
/// line-mode fallback reads complete lines ([`LineInput`]), and the
/// tests read a script. All three produce [`InputOutcome`] values, so
/// the loop below them sees no difference.
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

    /// Whether this source has a normal mode: single keys that act, a
    /// board cursor, and a `:` command line.
    ///
    /// The loop asks this question in two places. A `true` answer
    /// puts the board cursor on the frame and the normal-mode key
    /// table on the help screen. A `false` answer (the line-mode
    /// source) omits both: a cursor that no input can move and a key
    /// table for keys that do not exist would both be false
    /// statements about the shell.
    fn has_normal_mode(&self) -> bool {
        true
    }
}

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

/// The line-mode input source: one complete line per call.
///
/// `LineInput` is the canonical-mode counterpart of [`RawInput`]. The
/// terminal driver in canonical mode delivers a line at a time, and
/// so do pipes, CI logs, and redirected scripts. Each line maps to an
/// outcome through the shared grammar of [`command::parse`]; the
/// mapping has no special cases beyond the empty line, because the
/// grammar already knows `h`/`help`/`?` as `Help` and
/// `q`/`quit`/`exit` as `Quit`, and the loop intercepts both
/// commands. What line mode does not have are the cursor-only keys —
/// arrows and the `d`/`b`/`w`/`m` ceremony keys — because there is no
/// cursor to move and no normal mode to press them in. The typed
/// commands (`play h8`, `done`, `black`, `white`, `more`, `back`)
/// cover the ceremony.
///
/// The source is generic over the reader so tests can script it from
/// memory; production reads stdin.
pub struct LineInput<R> {
    reader: R,
}

impl<R: BufRead> LineInput<R> {
    /// A source over any line reader.
    ///
    /// Production passes a locked stdin (`StdinLock` implements
    /// [`BufRead`]; `Stdin` itself does not). Tests pass an in-memory
    /// reader such as `Cursor<&str>`.
    #[must_use]
    pub fn new(reader: R) -> Self {
        Self { reader }
    }
}

impl<R: BufRead> InputSource for LineInput<R> {
    fn next_outcome(
        &mut self,
        state: &mut InputState,
        core: &SessionCore,
    ) -> io::Result<InputOutcome> {
        // `state` exists for key-driven sources. A line of text
        // carries its own meaning: there is no cursor to move and no
        // buffer to edit.
        let _ = state;
        let mut line = String::new();
        // `read_line` reports 0 bytes at EOF. A piped script ends at
        // EOF, so EOF means quit — the reference build's line loop
        // broke on end of input for the same reason.
        if self.reader.read_line(&mut line)? == 0 {
            return Ok(InputOutcome::Quit);
        }
        Ok(line_outcome(&line, core))
    }

    fn wait_any_key(&mut self) -> io::Result<()> {
        // The "any key" of line mode is the Enter key: canonical mode
        // delivers a line at a time, so one read dismisses the help
        // screen. EOF dismisses it too.
        let mut line = String::new();
        self.reader.read_line(&mut line).map(|_| ())
    }

    /// Line mode has no normal mode: no single keys, no board cursor,
    /// no `:` command line.
    fn has_normal_mode(&self) -> bool {
        false
    }
}

/// Translate one input line into an outcome: the line-mode mapping.
///
/// Everything non-empty goes through [`command::parse`]: one grammar
/// exists, and a second parser here would drift from it, the way the
/// reference build's two line parsers drifted apart. A parse error
/// becomes a [`InputOutcome::Message`] with the parser's own text, the
/// same as on the `:` command line of the full-screen shell.
fn line_outcome(line: &str, core: &SessionCore) -> InputOutcome {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return empty_line_outcome(core);
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
/// the `OpeningInProgress` error. The reference predates the
/// in-session ceremony, so the ceremony guard is this sub-chapter's
/// extension of its rule.
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

/// Run the study table.
///
/// The selection rule: a terminal gets the full-screen shell
/// ([`AltScreen`] and [`RawInput`]); anything else gets the line-mode
/// fallback ([`LineScreen`] and [`LineInput`]). Piped output, CI, and
/// redirected sessions keep working: the fallback writes plain text
/// and reads complete lines, so a script drives the same loop as a
/// terminal.
///
/// `hint`, when given, is the first message on the message line — for
/// example a note that the session starts inside the Swap2 ceremony.
///
/// # Errors
/// Returns any I/O error from the terminal or the streams.
pub fn run(init: SessionInit, hint: Option<&str>) -> io::Result<()> {
    let mut core = SessionCore::new(init);
    match select_shell(io::stdout().is_terminal()) {
        Shell::FullScreen => {
            // The guard restores the terminal on every exit path,
            // panic included. The binding must stay alive until the
            // loop returns.
            let mut screen = AltScreen::enter()?;
            let mut input = RawInput;
            study_loop(&mut core, &mut screen, &mut input, hint)
        }
        Shell::Line => {
            let mut screen = LineScreen::stdout();
            // `StdinLock` implements `BufRead`; `Stdin` itself does
            // not. The lock borrows from a static handle, so it lives
            // as long as the loop needs it.
            let mut input = LineInput::new(io::stdin().lock());
            study_loop(&mut core, &mut screen, &mut input, hint)
        }
    }
}

/// Which shell serves the session.
///
/// The enum exists so the selection rule is a pure decision that a
/// test can pin: the terminal check goes in, the shell choice comes
/// out, and only [`run`] performs the side effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shell {
    /// The full-screen shell: alternate screen, raw-mode keys.
    FullScreen,
    /// The line-mode fallback: plain output, complete lines.
    Line,
}

/// The selection rule as a pure decision.
///
/// A terminal gets the full-screen shell; a pipe, a CI log, or a
/// redirect gets the line-mode fallback. One bit in, one choice out.
fn select_shell(stdout_is_terminal: bool) -> Shell {
    if stdout_is_terminal {
        Shell::FullScreen
    } else {
        Shell::Line
    }
}

/// The study-table loop: draw, translate one input, apply the
/// outcome, repeat until quit.
///
/// Everything here is screen-agnostic and terminal-agnostic. `screen`
/// receives finished frames; `input` produces finished outcomes. The
/// same loop serves the alternate screen and the line-mode fallback.
///
/// # Errors
/// Propagates I/O errors from the screen and the input source.
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
        );
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

/// Assemble the frame for one loop iteration.
///
/// The view carries what the renderer cannot know: the board cursor
/// lives in the input state, and the message line lives in the loop.
/// `cursor` is `None` when the input source has no cursor (line
/// mode), and the frame then shows no cursor marker. One bottom line
/// exists: while the command line is open it shows the buffer; in
/// normal mode it shows the message, when there is one.
fn build_frame(
    core: &SessionCore,
    state: &InputState,
    cursor: Option<Move>,
    events: &[Event],
    message: Option<&str>,
    styled: bool,
) -> Vec<String> {
    let view = StudyView::new(core, events).styled(styled);
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
}

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

/// Draw the help screen and wait for one input to dismiss it.
fn show_help<S: Screen, I: InputSource>(screen: &mut S, input: &mut I) -> io::Result<()> {
    screen.draw(&help_frame(input.has_normal_mode()))?;
    input.wait_any_key()
}

/// The help frame: the typed command language, then — for a shell
/// with a normal mode — the normal-mode key table, then the dismissal
/// note.
///
/// The command-language part reuses [`repl::HELP`], so the help screen
/// and the line-mode `help` command cannot drift apart. The key table
/// is conditional: in line mode the keys do not exist, and a help
/// text must not advertise input the shell cannot take. The frame is
/// unstyled text: it must stay readable on every terminal, and the
/// help screen has no stones to highlight.
fn help_frame(normal_mode: bool) -> Vec<String> {
    // `HELP` ends with a newline; `str::lines` drops it, so the blank
    // separator between the parts is added explicitly.
    let mut lines: Vec<String> = repl::HELP.lines().map(str::to_string).collect();
    let note = if normal_mode {
        lines.push(String::new());
        lines.extend(NORMAL_MODE_KEYS.lines().map(str::to_string));
        "Press any key to return to the table."
    } else {
        // Canonical mode delivers a line at a time, so the dismissing
        // "key" is Enter.
        "Press Enter to return to the table."
    };
    lines.push(String::new());
    lines.push(note.to_string());
    lines
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::SeatSel;
    use crate::session::SessionInit;
    use crossterm::event::{KeyCode, KeyModifiers};
    use engine::{Color, Move};
    use std::collections::VecDeque;
    use std::io::Cursor;

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

    /// An input source that plays back a scripted list of keys.
    ///
    /// A script must end the session itself (with `q` or a typed
    /// `quit`). Running out of keys is an error, not a silent quit:
    /// a shorter-than-intended script must fail the test, not pass it.
    struct ScriptedInput {
        keys: VecDeque<KeyEvent>,
    }

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

    #[test]
    fn the_loop_plays_moves_and_quits_cleanly() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        // Enter plays h8 (Black); Right + Enter plays i8 (White); q quits.
        let keys = vec![
            key(KeyCode::Enter),
            key(KeyCode::Right),
            key(KeyCode::Enter),
            char_key('q'),
        ];
        let (core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        // One frame per key: each loop iteration draws once and reads once.
        assert_eq!(screen.frames.len(), 4);
        assert!(
            screen.frames.iter().all(|frame| frame.len() >= 19),
            "every frame holds at least the 19 board lines"
        );
        assert_eq!(
            core.board().stone_at(Move::new(7, 7).unwrap()),
            Some(Color::Black)
        );
        assert_eq!(
            core.board().stone_at(Move::new(7, 8).unwrap()),
            Some(Color::White)
        );
        // The message line summarizes the last event.
        assert!(
            screen.frames[3]
                .iter()
                .any(|line| line.contains("Moved i8")),
            "the last frame shows the last event: {:?}",
            screen.frames[3]
        );
    }

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

    #[test]
    fn a_wrong_phase_key_lands_on_the_message_line() {
        let core = SessionCore::new(SessionInit::new_hotseat().with_swap2(true));
        let keys = vec![char_key('m'), char_key('q')];
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        assert_eq!(screen.frames.len(), 2);
        assert!(
            screen.frames[1]
                .iter()
                .any(|line| line.contains("'more' applies only at the first Swap2 choice")),
            "the message line shows the key's explanation: {:?}",
            screen.frames[1]
        );
    }

    #[test]
    fn a_failed_command_lands_on_the_message_line() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        // Two Enters on the same square: the second is illegal.
        let keys = vec![key(KeyCode::Enter), key(KeyCode::Enter), char_key('q')];
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        assert!(
            screen.frames[2]
                .iter()
                .any(|line| line.contains("Illegal Move")),
            "the session's error text shows on the message line: {:?}",
            screen.frames[2]
        );
    }

    #[test]
    fn help_is_a_full_screen_frame_and_any_key_dismisses_it() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        // `?` opens help; `x` dismisses it (any key); `q` quits.
        let keys = vec![char_key('?'), char_key('x'), char_key('q')];
        let (core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        assert_eq!(screen.frames.len(), 3);
        let help = screen.frames[1].join("\n");
        assert!(
            help.contains("Commands (case-insensitive):"),
            "the typed language: {help}"
        );
        assert!(help.contains("Normal-mode keys"), "the key table: {help}");
        assert!(
            help.contains("Press any key to return to the table."),
            "the note: {help}"
        );
        // The dismissing key did not act on the table: no stone, no
        // cursor change visible in the session.
        assert_eq!(core.snapshot().ply, 0);
        assert!(
            !screen.all_text().contains("Moved"),
            "the dismissed help key played no move"
        );
    }

    #[test]
    fn typed_help_and_quit_do_what_the_keys_do() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        // `:help` shows the help screen; `z` dismisses it; `:quit` quits.
        let mut keys = typed_line("help");
        keys.push(char_key('z'));
        keys.extend(typed_line("quit"));
        let expected_frames = keys.len();
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        assert_eq!(screen.frames.len(), expected_frames);
        // The help frame is drawn right after the `:help` line executed:
        // initial frame, `:`, `:h`, ..., `:help` (6 frames), then help.
        let help = screen.frames[6].join("\n");
        assert!(
            help.contains("Normal-mode keys"),
            "typed help opens the help screen"
        );
        // While the command line was open, the buffer was the bottom
        // line. Frame 2 follows the `:` and the `h` keys.
        assert_eq!(screen.frames[2].last().unwrap(), ":h");
    }

    #[test]
    fn a_parse_error_on_the_command_line_lands_on_the_message_line() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        let mut keys = typed_line("play");
        keys.push(char_key('q'));
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        assert!(
            screen.all_text().contains("Parse error"),
            "the parser's error text shows on the message line"
        );
    }

    #[test]
    fn an_exhausted_script_is_an_error_not_a_silent_quit() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        // No quit key: the script runs dry while the loop still runs.
        let keys = vec![key(KeyCode::Enter)];
        let result = run_scripted(core, keys);
        assert!(result.is_err(), "an exhausted script returns an error");
    }

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
        assert!(text.contains("usage: play <square>"), "the text: {text}");
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

    #[test]
    fn a_line_reader_yields_outcomes_until_eof_then_quits() {
        let core = hotseat_core();
        let mut state = InputState::new();
        let mut input = LineInput::new(Cursor::new("h8\nquit\n"));
        assert_eq!(
            input.next_outcome(&mut state, &core).unwrap(),
            InputOutcome::Command(Command::Play {
                mv: Move::new(7, 7).unwrap()
            })
        );
        // The input state is untouched: a line carries its own meaning.
        assert_eq!(state, InputState::new());
        assert_eq!(
            input.next_outcome(&mut state, &core).unwrap(),
            InputOutcome::Command(Command::Quit)
        );
        // The script is exhausted: EOF quits, and keeps quitting.
        assert_eq!(
            input.next_outcome(&mut state, &core).unwrap(),
            InputOutcome::Quit
        );
        assert_eq!(
            input.next_outcome(&mut state, &core).unwrap(),
            InputOutcome::Quit
        );
    }

    #[test]
    fn the_selection_rule_picks_the_shell_from_the_terminal_check() {
        assert_eq!(select_shell(true), Shell::FullScreen);
        assert_eq!(select_shell(false), Shell::Line);
    }

    #[test]
    fn the_normal_mode_predicate_distinguishes_the_sources() {
        assert!(RawInput.has_normal_mode());
        assert!(ScriptedInput::new(vec![]).has_normal_mode());
        assert!(!LineInput::new(Cursor::new("")).has_normal_mode());
    }

    #[test]
    fn the_help_frame_drops_the_key_table_without_a_normal_mode() {
        let keyed = help_frame(true).join("\n");
        assert!(keyed.contains("Normal-mode keys"));
        assert!(keyed.contains("Press any key to return to the table."));

        let plain = help_frame(false).join("\n");
        assert!(plain.contains("Commands (case-insensitive):"));
        assert!(!plain.contains("Normal-mode keys"));
        assert!(plain.contains("Press Enter to return to the table."));
    }
}
