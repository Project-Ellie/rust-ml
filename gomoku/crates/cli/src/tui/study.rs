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

/// The study-table loop: draw, translate one key, apply the outcome,
/// repeat until quit.
///
/// Everything here is screen-agnostic and terminal-agnostic. `screen`
/// receives finished frames; `input` produces finished outcomes. The
/// same loop serves the alternate screen now and the line-mode
/// fallback in sub-chapter e.
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

/// Assemble the frame for one loop iteration.
///
/// The view carries what the renderer cannot know: the board cursor
/// and the color override live in the input state, and the message
/// line lives in the loop. One bottom line exists: while the command
/// line is open it shows the buffer; in normal mode it shows the
/// message, when there is one.
fn build_frame(
    core: &SessionCore,
    state: &InputState,
    events: &[Event],
    message: Option<&str>,
    styled: bool,
) -> Vec<String> {
    let view = StudyView::new(core, events)
        .cursor(state.cursor)
        .color_override(state.next_color_override)
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
  b, w           Swap2 placement: select the next stone's color;
                 Swap2 choice: choose that color
  m              Swap2 first choice: place two more stones (more)
  :              open the command line (Esc cancels, Enter executes)
  ?              show this help
  q              quit
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionInit;
    use crossterm::event::{KeyCode, KeyModifiers};
    use engine::{Color, Move};
    use std::collections::VecDeque;

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
        assert_eq!(
            core.board().stone_at(Move::new(7, 7).unwrap()),
            Some(Color::Black)
        );
        assert_eq!(
            core.board().stone_at(Move::new(7, 8).unwrap()),
            Some(Color::White)
        );
        // j8 is Black — proof that the override armed exactly one stone.
        assert_eq!(
            core.board().stone_at(Move::new(7, 9).unwrap()),
            Some(Color::Black)
        );

        // Frame 0 is the initial frame; the key at index i is processed
        // before frame i+1 is drawn. Frame 2 follows the `w` key.
        assert!(
            screen.frames[2]
                .iter()
                .any(|line| line.contains("Next: White (override)")),
            "the armed override shows in the panel: {:?}",
            screen.frames[2]
        );
        // Frame 4 follows the placement of i8, which disarmed the override.
        assert!(
            screen.frames[4]
                .iter()
                .any(|line| line.contains("Next: Black (default)")),
            "a landed placement disarms the override: {:?}",
            screen.frames[4]
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
}
