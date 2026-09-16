//! Front-ends. `run_plain` scrolls (debugging fallback); `run_ui` owns the
//! alternate screen. Both share `dispatch`.

use std::io::{self, Stdout, Write};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::Print,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::board::{EngineKind, GameBoard, board_for};
use crate::input::{Command, parse};
use crate::render::{render_lines, render_styled, status_line};

const HELP: &str =
    "enter <row> <col> to play (e.g. '7 7') · 'u' undoes · 'new' restarts · 'q' quits";

/// One player action applied to the board. Returns the message to
/// display next, or `None` to quit.
pub fn dispatch(board: &mut Box<dyn GameBoard>, kind: EngineKind, input: &str) -> Option<String> {
    match parse(input) {
        Ok(Command::Play(mv)) => Some(match board.play(mv) {
            Ok(()) => String::new(),
            Err(e) => e.to_string(),
        }),
        Ok(Command::Undo) => {
            if board.moves().is_empty() {
                Some("nothing to undo".to_string())
            } else {
                board.undo();
                Some(format!(
                    "undid the last move — {:?} to move",
                    board.to_move()
                ))
            }
        }
        Ok(Command::New) => {
            *board = board_for(kind);
            Some("new game — Black to move".to_string())
        }
        Ok(Command::Help) => Some(HELP.to_string()),
        Ok(Command::Quit) => None,
        Err(e) => Some(e),
    }
}

/// The scrolling front-end: render, prompt, read, repeat.
pub fn run_plain(mut board: Box<dyn GameBoard>, kind: EngineKind) -> io::Result<()> {
    let mut message = HELP.to_string();
    loop {
        for line in render_lines(&*board) {
            println!("{line}");
        }
        println!("{}", status_line(&*board));
        if !message.is_empty() {
            println!("{message}");
        }
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break; // EOF (Ctrl-D): leave quietly
        }
        match dispatch(&mut board, kind, &input) {
            Some(next) => message = next,
            None => break,
        }
    }
    Ok(())
}

// -------------------------------------------------- alternate-screen UI

const STATUS_ROW: u16 = 20;
const MESSAGE_ROW: u16 = 21;
const PROMPT_ROW: u16 = 22;

/// Enters the alternate screen; leaving is this value's `Drop` — so every
/// exit path (early return, `?`, panic unwind) restores the terminal.
struct AlternateScreen;

impl AlternateScreen {
    fn enter() -> io::Result<AlternateScreen> {
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(AlternateScreen)
    }
}

impl Drop for AlternateScreen {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
    }
}

/// Draw one frame at fixed coordinates. Cooked mode's ONLCR turns each
/// "\n" into CR+LF, so plain `Print` walks down the screen.
fn draw(stdout: &mut Stdout, board: &dyn GameBoard, message: &str) -> io::Result<()> {
    queue!(stdout, MoveTo(0, 0), Hide, Clear(ClearType::All))?;
    for line in render_styled(board) {
        queue!(stdout, Print(line), Print("\n"))?;
    }
    queue!(stdout, MoveTo(0, STATUS_ROW), Print(status_line(board)))?;
    queue!(stdout, MoveTo(0, MESSAGE_ROW), Print(message))?;
    queue!(stdout, MoveTo(0, PROMPT_ROW), Print("> "), Show)?;
    stdout.flush()
}

/// The alternate-screen front-end: same loop, zero scrolling.
pub fn run_ui(mut board: Box<dyn GameBoard>, kind: EngineKind) -> io::Result<()> {
    let _screen = AlternateScreen::enter()?;
    let mut stdout = io::stdout();
    let mut message = HELP.to_string();
    loop {
        draw(&mut stdout, &*board, &message)?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break; // EOF
        }
        match dispatch(&mut board, kind, &input) {
            Some(next) => message = next,
            None => break,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Move;

    #[test]
    fn undo_with_empty_history_is_refused_not_panicked() {
        let mut board = board_for(EngineKind::Fast);
        let msg = dispatch(&mut board, EngineKind::Fast, "u").unwrap();
        assert_eq!(msg, "nothing to undo");
    }

    #[test]
    fn undo_arms_through_dispatch() {
        let mut board = board_for(EngineKind::Naive);
        dispatch(&mut board, EngineKind::Naive, "7 7").unwrap();
        assert_eq!(board.moves().len(), 1);

        let msg = dispatch(&mut board, EngineKind::Naive, "u").unwrap();
        assert_eq!(msg, "undid the last move — Black to move");
        assert_eq!(board.moves().len(), 0);
        assert_eq!(board.stone_at(Move::new(7, 7).unwrap()), None);
    }
}
