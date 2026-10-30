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
use engine::{Color, Move, Status};

/// The message for the `m` key at a phase where it does not apply.
///
/// The text matches [`crate::command::CommandError::NotAtFirstChoice`]
/// on purpose: the user sees one sentence for one rule, no matter
/// whether the input layer or the session reports it.
const MORE_ONLY_AT_FIRST_CHOICE: &str = "'more' applies only at the first Swap2 choice";

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

impl InputState {
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
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

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
    /// Toggle the value strip in the side panel.
    ToggleValueStrip,
    /// The key changed only input state (cursor moved, buffer edited),
    /// or the key is not mapped. Nothing reaches the session.
    None,
}

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

/// One key while the command line is open: the editing machine.
///
/// The buffer comes out of the state with `mem::replace`, so the mode
/// can change (Esc, Enter) without two live borrows of `state.mode`.
/// Every path below either puts the buffer back or leaves the state in
/// normal mode.
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
        KeyCode::Char('v') => return InputOutcome::ToggleValueStrip,
        _ => {}
    }

    // Phase-dependent keys. The session knows the phase; the snapshot
    // reports it. This is what makes one key per action safe: `u`
    // cannot mean two things at once, because the session always knows
    // which phase the session is in.
    match core.snapshot().opening {
        Some(opening) => opening_key(key, state, &opening),
        None => playing_key(key, state, core),
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::SeatSel;
    use crate::session::SessionInit;
    use crossterm::event::KeyModifiers;

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

    #[test]
    fn arrows_and_hjkl_move_the_cursor() {
        let core = playing_core();
        let mut state = InputState::new();
        assert_eq!(state.cursor, Move::new(7, 7).unwrap());

        handle_key(key(KeyCode::Up), &mut state, &core);
        assert_eq!(state.cursor, Move::new(6, 7).unwrap());
        handle_key(key(KeyCode::Left), &mut state, &core);
        assert_eq!(state.cursor, Move::new(6, 6).unwrap());
        handle_key(char_key('j'), &mut state, &core);
        assert_eq!(state.cursor, Move::new(7, 6).unwrap());
        handle_key(char_key('l'), &mut state, &core);
        assert_eq!(state.cursor, Move::new(7, 7).unwrap());
    }

    #[test]
    fn the_cursor_clamps_at_the_board_edges() {
        let core = playing_core();
        let mut state = InputState {
            cursor: Move::new(0, 0).unwrap(),
            ..InputState::new()
        };
        // The top-left corner: up and left cannot underflow.
        handle_key(char_key('k'), &mut state, &core);
        handle_key(key(KeyCode::Left), &mut state, &core);
        assert_eq!(state.cursor, Move::new(0, 0).unwrap());

        state.cursor = Move::new(14, 14).unwrap();
        // The bottom-right corner: down and right cannot pass index 14.
        handle_key(char_key('j'), &mut state, &core);
        handle_key(key(KeyCode::Right), &mut state, &core);
        assert_eq!(state.cursor, Move::new(14, 14).unwrap());
    }

    #[test]
    fn the_global_keys_work_in_every_phase() {
        let mut state = InputState::new();
        for core in [
            playing_core(),
            placing_core(),
            first_choice_core(),
            final_choice_core(),
        ] {
            assert_eq!(
                handle_key(char_key('?'), &mut state, &core),
                InputOutcome::Help
            );
            assert_eq!(
                handle_key(char_key('q'), &mut state, &core),
                InputOutcome::Quit
            );
            let outcome = handle_key(char_key(':'), &mut state, &core);
            assert_eq!(outcome, InputOutcome::None);
            assert_eq!(state.mode, InputMode::CommandLine(String::new()));
            // Close the line again for the next phase's core.
            handle_key(key(KeyCode::Esc), &mut state, &core);
            assert_eq!(state.mode, InputMode::Normal);
        }
    }

    #[test]
    fn during_play_enter_plays_at_the_cursor_for_a_human_seat() {
        let core = playing_core();
        let mut state = InputState::new();
        state.cursor = Move::new(3, 4).unwrap();
        assert_eq!(
            handle_key(key(KeyCode::Enter), &mut state, &core),
            InputOutcome::Command(Command::Play {
                mv: Move::new(3, 4).unwrap()
            })
        );
    }

    #[test]
    fn during_play_enter_steps_for_an_ai_seat() {
        let mut core = playing_core();
        core.execute(Command::Release {
            seat: SeatSel::Black,
        })
        .unwrap();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(key(KeyCode::Enter), &mut state, &core),
            InputOutcome::Command(Command::Step)
        );
    }

    #[test]
    fn during_play_u_is_back_and_the_ceremony_keys_are_unmapped() {
        let core = playing_core();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(char_key('u'), &mut state, &core),
            InputOutcome::Command(Command::Back { n: 1 })
        );
        for c in ['d', 'b', 'w', 'm'] {
            assert_eq!(
                handle_key(char_key(c), &mut state, &core),
                InputOutcome::None,
                "'{c}' is unmapped during play"
            );
        }
    }

    #[test]
    fn at_a_placement_phase_enter_places_the_next_stone() {
        let core = placing_core();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(key(KeyCode::Enter), &mut state, &core),
            InputOutcome::Command(Command::Play {
                mv: Move::new(7, 7).unwrap()
            })
        );
    }

    #[test]
    fn at_a_placement_phase_b_and_w_are_unmapped() {
        // The alternation fixes the next stone's color, so `b`/`w`
        // select nothing at a placement phase. The binding exists only
        // at a choice phase, where the same keys choose the color.
        for core in [placing_core(), placing2_core()] {
            let mut state = InputState::new();
            for c in ['b', 'w'] {
                assert_eq!(
                    handle_key(char_key(c), &mut state, &core),
                    InputOutcome::None,
                    "'{c}' is unmapped at a placement phase"
                );
            }
        }
    }

    #[test]
    fn at_a_placement_phase_u_undoes_and_d_is_done() {
        for core in [placing_core(), placing2_core()] {
            let mut state = InputState::new();
            assert_eq!(
                handle_key(char_key('u'), &mut state, &core),
                InputOutcome::Command(Command::Back { n: 1 })
            );
            assert_eq!(
                handle_key(char_key('d'), &mut state, &core),
                InputOutcome::Command(Command::Done)
            );
        }
    }

    #[test]
    fn at_a_placement_phase_m_is_a_message_not_a_command() {
        let core = placing_core();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(char_key('m'), &mut state, &core),
            InputOutcome::Message(MORE_ONLY_AT_FIRST_CHOICE.to_string())
        );
    }

    #[test]
    fn at_the_first_choice_b_w_choose_and_m_is_more() {
        let core = first_choice_core();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(char_key('b'), &mut state, &core),
            InputOutcome::Command(Command::ChooseColor {
                color: Color::Black
            })
        );
        assert_eq!(
            handle_key(char_key('w'), &mut state, &core),
            InputOutcome::Command(Command::ChooseColor {
                color: Color::White
            })
        );
        assert_eq!(
            handle_key(char_key('m'), &mut state, &core),
            InputOutcome::Command(Command::More)
        );
    }

    #[test]
    fn at_the_final_choice_b_w_choose_and_m_is_a_message() {
        let core = final_choice_core();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(char_key('b'), &mut state, &core),
            InputOutcome::Command(Command::ChooseColor {
                color: Color::Black
            })
        );
        assert_eq!(
            handle_key(char_key('m'), &mut state, &core),
            InputOutcome::Message(MORE_ONLY_AT_FIRST_CHOICE.to_string())
        );
    }

    #[test]
    fn at_a_choice_phase_enter_and_u_are_unmapped() {
        let core = first_choice_core();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(key(KeyCode::Enter), &mut state, &core),
            InputOutcome::None
        );
        assert_eq!(
            handle_key(char_key('u'), &mut state, &core),
            InputOutcome::None
        );
    }

    #[test]
    fn the_command_line_appends_and_backspace_edits() {
        let core = playing_core();
        let mut state = InputState::new();
        handle_key(char_key(':'), &mut state, &core);

        type_text(&mut state, &core, "pla");
        assert_eq!(state.mode, InputMode::CommandLine("pla".to_string()));

        handle_key(key(KeyCode::Backspace), &mut state, &core);
        assert_eq!(state.mode, InputMode::CommandLine("pl".to_string()));

        // Backspace on an empty buffer is a no-op, not an underflow.
        handle_key(key(KeyCode::Backspace), &mut state, &core);
        handle_key(key(KeyCode::Backspace), &mut state, &core);
        handle_key(key(KeyCode::Backspace), &mut state, &core);
        assert_eq!(state.mode, InputMode::CommandLine(String::new()));
    }

    #[test]
    fn esc_cancels_the_command_line() {
        let core = playing_core();
        let mut state = InputState::new();
        handle_key(char_key(':'), &mut state, &core);
        type_text(&mut state, &core, "play h8");
        assert_eq!(
            handle_key(key(KeyCode::Esc), &mut state, &core),
            InputOutcome::None
        );
        assert_eq!(state.mode, InputMode::Normal);
    }

    #[test]
    fn enter_on_an_empty_command_line_returns_to_normal() {
        let core = playing_core();
        let mut state = InputState::new();
        handle_key(char_key(':'), &mut state, &core);
        assert_eq!(
            handle_key(key(KeyCode::Enter), &mut state, &core),
            InputOutcome::None
        );
        assert_eq!(state.mode, InputMode::Normal);
    }

    #[test]
    fn enter_parses_the_buffer_with_the_shared_grammar() {
        let core = playing_core();
        let mut state = InputState::new();
        handle_key(char_key(':'), &mut state, &core);
        type_text(&mut state, &core, "play h8");
        assert_eq!(
            handle_key(key(KeyCode::Enter), &mut state, &core),
            InputOutcome::Command(Command::Play {
                mv: Move::new(7, 7).unwrap()
            })
        );
        assert_eq!(state.mode, InputMode::Normal);
    }

    #[test]
    fn a_parse_error_is_a_message_and_closes_the_line() {
        let core = playing_core();
        let mut state = InputState::new();
        handle_key(char_key(':'), &mut state, &core);
        type_text(&mut state, &core, "play");
        let outcome = handle_key(key(KeyCode::Enter), &mut state, &core);
        let InputOutcome::Message(text) = outcome else {
            panic!("a parse error is a message, got {outcome:?}");
        };
        assert!(text.contains("Parse error"), "the parser's text: {text}");
        assert_eq!(state.mode, InputMode::Normal);
    }

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
