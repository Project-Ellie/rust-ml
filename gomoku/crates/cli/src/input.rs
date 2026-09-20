//! Command parsing — pure functions, no I/O.

use engine::Move;

#[derive(Debug)]
pub enum Command {
    Play(Move),
    Undo,
    Redo,
    New,
    Quit,
    Help,
    /// Swap2: take Black.
    TakeBlack,
    /// Swap2: take White.
    TakeWhite,
    /// Swap2: place two more stones instead of choosing a color.
    AddStones,
    /// Confirm a full placement phase and move to the choice prompt.
    Done,
    /// Run the threat-space search demo overlay.
    Tss,
    /// Puzzle mode: step forward through the claimed solution line.
    SolutionStep,
    /// Puzzle mode: next sample.
    NextPuzzle,
    /// Puzzle mode: previous sample.
    PrevPuzzle,
}

// Kept mode-agnostic on purpose: mode-specific keys live in the
// ui.rs HELP_* strings shown by the 'h' command.
const USAGE: &str = "usage: <row> <col> (0-14) · 'h' help · 'q' quit";

/// Parse one line of user input. Never panics; typos become `Err`
/// messages the UI can display.
pub fn parse(input: &str) -> Result<Command, String> {
    let text = input.trim().to_lowercase();
    match text.as_str() {
        "q" | "quit" | "exit" => return Ok(Command::Quit),
        "u" | "undo" => return Ok(Command::Undo),
        "r" | "redo" => return Ok(Command::Redo),
        "s" | "step" => return Ok(Command::SolutionStep),
        "new" => return Ok(Command::New),
        "h" | "help" | "?" => return Ok(Command::Help),
        "b" => return Ok(Command::TakeBlack),
        "w" => return Ok(Command::TakeWhite),
        "a" => return Ok(Command::AddStones),
        "d" => return Ok(Command::Done),
        "t" => return Ok(Command::Tss),
        "n" => return Ok(Command::NextPuzzle),
        "p" => return Ok(Command::PrevPuzzle),
        _ => {}
    }

    let parts: Vec<&str> = text.split([',', ' ']).filter(|s| !s.is_empty()).collect();
    if parts.len() != 2 {
        return Err(USAGE.to_string());
    }
    let row: u8 = parts[0].parse().map_err(|_| USAGE.to_string())?;
    let col: u8 = parts[1].parse().map_err(|_| USAGE.to_string())?;
    let mv = Move::new(row, col)
        .ok_or_else(|| format!("({row}, {col}) is off the board — coordinates are 0-14"))?;
    Ok(Command::Play(mv))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn played(input: &str) -> (u8, u8) {
        match parse(input) {
            Ok(Command::Play(mv)) => (mv.row(), mv.col()),
            other => panic!("{input:?} should be a move, got {other:?}"),
        }
    }

    #[test]
    fn space_separated_coordinates() {
        assert_eq!(played("7 7"), (7, 7));
    }

    #[test]
    fn comma_separated_coordinates() {
        assert_eq!(played("7,7"), (7, 7));
        assert_eq!(played(" 7 , 7 "), (7, 7));
    }

    #[test]
    fn keywords() {
        assert!(matches!(parse("q"), Ok(Command::Quit)));
        assert!(matches!(parse("quit"), Ok(Command::Quit)));
        assert!(matches!(parse("u"), Ok(Command::Undo)));
        assert!(matches!(parse("undo"), Ok(Command::Undo)));
        assert!(matches!(parse("new"), Ok(Command::New)));
        assert!(matches!(parse("h"), Ok(Command::Help)));
        assert!(matches!(parse("?"), Ok(Command::Help)));
        assert!(matches!(parse("b"), Ok(Command::TakeBlack)));
        assert!(matches!(parse("w"), Ok(Command::TakeWhite)));
        assert!(matches!(parse("a"), Ok(Command::AddStones)));
        assert!(matches!(parse("d"), Ok(Command::Done)));
        assert!(matches!(parse("t"), Ok(Command::Tss)));
        assert!(matches!(parse("n"), Ok(Command::NextPuzzle)));
        assert!(matches!(parse("p"), Ok(Command::PrevPuzzle)));
        assert!(matches!(parse("r"), Ok(Command::Redo)));
    }

    #[test]
    fn off_board_is_an_error() {
        let err = parse("7 15").unwrap_err();
        assert!(err.contains("off the board"), "{err}");
    }

    #[test]
    fn garbage_gets_usage() {
        for bad in ["banana", "", "7", "1 2 3", "7 x"] {
            let err = parse(bad).unwrap_err();
            assert!(err.contains("usage"), "{bad:?} -> {err}");
        }
    }
}
