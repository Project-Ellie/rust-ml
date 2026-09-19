//! Front-ends. `run_plain` scrolls (debugging fallback); `run_ui` owns the
//! alternate screen. Both share `dispatch`.

use std::io::{self, Stdout, Write};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::Print,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::board::{EngineKind, FastBoard, GameBoard, board_for};
use crate::input::{Command, parse};
use crate::opening::{Holders, OpeningPhase, OpeningState, Party};
use crate::render::{
    overlay_verdict, render_lines, render_lines_with_overlay, render_opening_lines,
    render_opening_styled, render_styled, render_styled_with_overlay, status_line,
};
use engine::{Move, Status};

const HELP_NORMAL: &str =
    "enter <row> <col> to play · 'u' undo · 't' TSS demo · 'new' restart · 'q' quit";
const HELP_OPENING: &str = "<row> <col> to place · 'd' done · 'b' take Black · 'w' take White · 'a' add 2 stones · 'u' undo · 'q' quit";

/// Top-level application state.
pub(crate) enum AppState {
    /// The Swap2 opening is in progress.
    Opening(OpeningState),
    /// Normal alternating play, optionally with hotseat holders from Swap2.
    Normal(Box<dyn GameBoard>, Option<Holders>),
}

impl AppState {
    fn new(kind: EngineKind, swap2: bool) -> Self {
        if swap2 {
            AppState::Opening(OpeningState::new())
        } else {
            AppState::Normal(board_for(kind), None)
        }
    }
}

fn help_for(state: &AppState) -> &'static str {
    match state {
        AppState::Opening(_) => HELP_OPENING,
        AppState::Normal(_, _) => HELP_NORMAL,
    }
}

/// One player action applied to the current state. Returns the message to
/// display next, or `None` to quit.
pub(crate) fn dispatch(
    state: &mut AppState,
    kind: EngineKind,
    swap2: bool,
    cmd: Command,
    overlay: &mut Option<engine::Proof>,
) -> Option<String> {
    // Global commands that work in every phase.
    match &cmd {
        Command::Quit => return None,
        Command::Help => return Some(help_for(state).to_string()),
        Command::New => {
            *state = AppState::new(kind, swap2);
            *overlay = None;
            return Some(start_message(state));
        }
        _ => {}
    }

    // Every command except TSS clears an active proof overlay.
    if !matches!(cmd, Command::Tss) {
        *overlay = None;
    }

    match state {
        AppState::Opening(_) => {
            let mut extracted = std::mem::replace(state, AppState::Opening(OpeningState::new()));
            let AppState::Opening(opening) = &mut extracted else {
                unreachable!()
            };
            match dispatch_opening(opening, cmd)? {
                OpeningResult::Message(msg) => {
                    *state = extracted;
                    Some(msg)
                }
                OpeningResult::Finish(board, holders, msg) => {
                    *state = AppState::Normal(board, Some(holders));
                    Some(msg)
                }
            }
        }
        AppState::Normal(board, holders) => {
            dispatch_normal(&mut **board, holders.as_ref(), cmd, overlay)
        }
    }
}

enum OpeningResult {
    Message(String),
    Finish(Box<dyn GameBoard>, Holders, String),
}

fn start_message(state: &AppState) -> String {
    match state {
        AppState::Opening(_) => {
            "Swap2 opening — Player A places two Black and one White stone".to_string()
        }
        AppState::Normal(_, _) => "new game — Black to move".to_string(),
    }
}

fn dispatch_opening(opening: &mut OpeningState, cmd: Command) -> Option<OpeningResult> {
    use engine::Color;

    Some(match cmd {
        Command::Play(mv) => {
            if !opening.can_place() {
                return Some(OpeningResult::Message(
                    "all stones for this phase are placed — type 'd' to continue or 'u' to undo"
                        .to_string(),
                ));
            }
            match opening.place(mv) {
                Ok(()) => OpeningResult::Message(opening_progress_message(opening, mv)),
                Err(e) => OpeningResult::Message(e),
            }
        }
        Command::Undo => {
            if opening.is_at_start() {
                OpeningResult::Message("nothing to undo".to_string())
            } else {
                opening.undo();
                OpeningResult::Message(opening_undo_message(opening))
            }
        }
        Command::Done => match opening.finish() {
            Ok(()) => OpeningResult::Message(opening_prompt_message(opening)),
            Err(e) => OpeningResult::Message(e),
        },
        Command::TakeBlack => return finish_opening(opening, Color::Black),
        Command::TakeWhite => return finish_opening(opening, Color::White),
        Command::AddStones => {
            if opening.phase() == OpeningPhase::FirstChoice {
                *opening = opening.clone().add_two_more().expect("first choice phase");
                OpeningResult::Message(format!(
                    "{} adds two more stones — place one Black, then one White",
                    Party::B.label()
                ))
            } else {
                OpeningResult::Message(
                    "'a' is only available when choosing after the first three stones".to_string(),
                )
            }
        }
        Command::Tss => {
            OpeningResult::Message("TSS demo is only available during normal play".to_string())
        }
        Command::New | Command::Help | Command::Quit => unreachable!("handled globally"),
    })
}

fn finish_opening(opening: &OpeningState, choice: engine::Color) -> Option<OpeningResult> {
    let outcome = match opening.clone().choose(choice) {
        Some(o) => o,
        None => {
            return Some(OpeningResult::Message(
                "choose a color at the Swap2 prompt: 'b' or 'w'".to_string(),
            ));
        }
    };
    let holders = Holders::new(outcome.black_holder, outcome.white_holder);
    Some(OpeningResult::Finish(
        Box::new(FastBoard::new(outcome.board)),
        holders,
        format!(
            "Swap2 done — Black: {} · White: {} · White moves first",
            holders.black().label(),
            holders.white().label()
        ),
    ))
}

fn dispatch_normal(
    board: &mut dyn GameBoard,
    holders: Option<&Holders>,
    cmd: Command,
    overlay: &mut Option<engine::Proof>,
) -> Option<String> {
    match cmd {
        Command::Play(mv) => Some(match board.play(mv) {
            Ok(()) => String::new(),
            Err(e) => e.to_string(),
        }),
        Command::Undo => {
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
        Command::Tss => run_tss(board, holders, overlay),
        Command::Done | Command::TakeBlack | Command::TakeWhite | Command::AddStones => {
            Some("that command is only available during the Swap2 opening".to_string())
        }
        Command::New | Command::Help | Command::Quit => unreachable!("handled globally"),
    }
}

fn run_tss(
    board: &dyn GameBoard,
    holders: Option<&Holders>,
    overlay: &mut Option<engine::Proof>,
) -> Option<String> {
    let Some(fast) = board.engine_board() else {
        return Some("TSS demo requires the fast engine".to_string());
    };

    let budget = engine::SearchBudget {
        max_nodes: 100_000,
        max_depth: 16,
    };
    let side = board.to_move();
    let Some(proof) = engine::prove_forced_win(fast, side, budget) else {
        return Some("no forced win proven within budget".to_string());
    };

    if !engine::verify_line(fast, &proof) {
        return Some("internal error: unverifiable proof".to_string());
    }

    let verdict = if let Some(h) = holders {
        let winner_holder = match proof.winner {
            engine::Color::Black => h.black().label(),
            engine::Color::White => h.white().label(),
        };
        format!(
            "certified forced win for {} ({:?}) in {} ply",
            winner_holder,
            proof.winner,
            proof.line.len()
        )
    } else {
        overlay_verdict(&proof)
    };

    *overlay = Some(proof);
    Some(verdict)
}

// ------------------------------------------------------------------ messages

fn opening_progress_message(opening: &OpeningState, mv: Move) -> String {
    let party = opening.active_party().label();
    match opening.phase() {
        OpeningPhase::Placing3 | OpeningPhase::Placing2 if opening.can_place() => {
            let color = opening.current_color().unwrap();
            let remaining = opening.remaining(color);
            format!(
                "{party} placed {color:?} at ({}, {}) — {remaining} more {color:?} to place",
                mv.row(),
                mv.col()
            )
        }
        OpeningPhase::FirstChoice => opening_prompt_message(opening),
        OpeningPhase::FinalChoice => opening_prompt_message(opening),
        _ => opening_prompt_message(opening),
    }
}

fn opening_undo_message(opening: &OpeningState) -> String {
    format!("undid — {}", opening_prompt_message(opening))
}

fn opening_prompt_message(opening: &OpeningState) -> String {
    let party = opening.active_party().label();
    match opening.phase() {
        OpeningPhase::Placing3 => {
            if opening.placements().len() == 3 {
                format!("{party}: 3 stones placed — [d] done · [u] undo")
            } else {
                let color = opening.current_color().unwrap();
                let remaining = opening.remaining(color);
                format!("{party}: place {color:?} ({remaining} remain) — enter row col · [u] undo")
            }
        }
        OpeningPhase::FirstChoice => {
            format!("{party}: [b] take Black · [w] take White · [a] add two stones · [u] undo")
        }
        OpeningPhase::Placing2 => {
            if opening.placements().len() == 5 {
                format!("{party}: 5 stones placed — [d] done · [u] undo")
            } else {
                let color = opening.current_color().unwrap();
                let remaining = opening.remaining(color);
                format!("{party}: place {color:?} ({remaining} remain) — enter row col · [u] undo")
            }
        }
        OpeningPhase::FinalChoice => format!("{party}: [b] take Black · [w] take White · [u] undo"),
    }
}

fn status_for(state: &AppState) -> String {
    match state {
        AppState::Opening(opening) => opening_prompt_message(opening),
        AppState::Normal(board, holders) => status_line_for(&**board, holders.as_ref()),
    }
}

fn status_line_for(board: &dyn GameBoard, holders: Option<&Holders>) -> String {
    match board.status() {
        Status::Ongoing => {
            let base = if let Some(h) = holders {
                format!(
                    "Black: {} · White: {} · move {}",
                    h.black().label(),
                    h.white().label(),
                    board.moves().len()
                )
            } else {
                format!("engine: {} · move {}", board.name(), board.moves().len())
            };
            format!("{base} · {:?} to move", board.to_move())
        }
        _ => status_line(board),
    }
}

// ------------------------------------------------------------------ plain loop

/// The scrolling front-end: render, prompt, read, repeat.
pub fn run_plain(_board: Box<dyn GameBoard>, kind: EngineKind, swap2: bool) -> io::Result<()> {
    let mut state = AppState::new(kind, swap2);
    let mut overlay: Option<engine::Proof> = None;
    let mut message = start_message(&state);
    loop {
        for line in render_state(&state, overlay.as_ref(), false) {
            println!("{line}");
        }
        println!("{}", status_for(&state));
        if !message.is_empty() {
            println!("{message}");
        }
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break; // EOF (Ctrl-D): leave quietly
        }
        let cmd = match parse(&input) {
            Ok(c) => c,
            Err(e) => {
                message = e;
                continue;
            }
        };
        match dispatch(&mut state, kind, swap2, cmd, &mut overlay) {
            Some(next) => message = next,
            None => break,
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ alternate-screen UI

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
fn draw(
    stdout: &mut Stdout,
    state: &AppState,
    overlay: Option<&engine::Proof>,
    message: &str,
) -> io::Result<()> {
    queue!(stdout, MoveTo(0, 0), Hide, Clear(ClearType::All))?;
    for line in render_state(state, overlay, true) {
        queue!(stdout, Print(line), Print("\n"))?;
    }
    queue!(stdout, MoveTo(0, STATUS_ROW), Print(status_for(state)))?;
    queue!(stdout, MoveTo(0, MESSAGE_ROW), Print(message))?;
    queue!(stdout, MoveTo(0, PROMPT_ROW), Print("> "), Show)?;
    stdout.flush()
}

fn render_state(state: &AppState, overlay: Option<&engine::Proof>, styled: bool) -> Vec<String> {
    match state {
        AppState::Opening(opening) => {
            if styled {
                render_opening_styled(opening)
            } else {
                render_opening_lines(opening)
            }
        }
        AppState::Normal(board, _) => {
            if let Some(proof) = overlay {
                if styled {
                    render_styled_with_overlay(&**board, proof)
                } else {
                    render_lines_with_overlay(&**board, proof)
                }
            } else if styled {
                render_styled(&**board)
            } else {
                render_lines(&**board)
            }
        }
    }
}

/// The alternate-screen front-end: same loop, zero scrolling.
pub fn run_ui(_board: Box<dyn GameBoard>, kind: EngineKind, swap2: bool) -> io::Result<()> {
    let _screen = AlternateScreen::enter()?;
    let mut stdout = io::stdout();
    let mut state = AppState::new(kind, swap2);
    let mut overlay: Option<engine::Proof> = None;
    let mut message = start_message(&state);
    loop {
        draw(&mut stdout, &state, overlay.as_ref(), &message)?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break; // EOF
        }
        let cmd = match parse(&input) {
            Ok(c) => c,
            Err(e) => {
                message = e;
                continue;
            }
        };
        match dispatch(&mut state, kind, swap2, cmd, &mut overlay) {
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
        let mut state = AppState::new(EngineKind::Fast, false);
        let mut overlay = None;
        let msg = dispatch(
            &mut state,
            EngineKind::Fast,
            false,
            Command::Undo,
            &mut overlay,
        )
        .unwrap();
        assert_eq!(msg, "nothing to undo");
    }

    #[test]
    fn undo_arms_through_dispatch() {
        let mut state = AppState::new(EngineKind::Naive, false);
        let mut overlay = None;
        dispatch(
            &mut state,
            EngineKind::Naive,
            false,
            Command::Play(Move::new(7, 7).unwrap()),
            &mut overlay,
        )
        .unwrap();

        let msg = dispatch(
            &mut state,
            EngineKind::Naive,
            false,
            Command::Undo,
            &mut overlay,
        )
        .unwrap();
        assert_eq!(msg, "undid the last move — Black to move");
    }

    #[test]
    fn swap2_opening_place_undo_flow() {
        let mut state = AppState::new(EngineKind::Fast, true);
        let mut overlay = None;

        // First three stones.
        for &(r, c) in &[(7u8, 7u8), (6, 6), (5, 5)] {
            dispatch(
                &mut state,
                EngineKind::Fast,
                true,
                Command::Play(Move::new(r, c).unwrap()),
                &mut overlay,
            )
            .unwrap();
        }
        assert!(matches!(state, AppState::Opening(_)));

        // Undo from the first-choice prompt returns to the full placement.
        let msg = dispatch(
            &mut state,
            EngineKind::Fast,
            true,
            Command::Undo,
            &mut overlay,
        )
        .unwrap();
        assert!(msg.contains("3 stones placed"), "{msg}");

        // Finish and take Black as Player B.
        dispatch(
            &mut state,
            EngineKind::Fast,
            true,
            Command::Done,
            &mut overlay,
        )
        .unwrap();
        let msg = dispatch(
            &mut state,
            EngineKind::Fast,
            true,
            Command::TakeBlack,
            &mut overlay,
        )
        .unwrap();
        assert!(msg.contains("Swap2 done"), "{msg}");
        assert!(matches!(state, AppState::Normal(_, Some(_))));
    }

    #[test]
    fn swap2_add_two_more_then_choose() {
        let mut state = AppState::new(EngineKind::Fast, true);
        let mut overlay = None;

        for &(r, c) in &[(7u8, 7u8), (6, 6), (5, 5)] {
            dispatch(
                &mut state,
                EngineKind::Fast,
                true,
                Command::Play(Move::new(r, c).unwrap()),
                &mut overlay,
            )
            .unwrap();
        }

        dispatch(
            &mut state,
            EngineKind::Fast,
            true,
            Command::AddStones,
            &mut overlay,
        )
        .unwrap();
        for &(r, c) in &[(4u8, 4u8), (3, 3)] {
            dispatch(
                &mut state,
                EngineKind::Fast,
                true,
                Command::Play(Move::new(r, c).unwrap()),
                &mut overlay,
            )
            .unwrap();
        }

        let msg = dispatch(
            &mut state,
            EngineKind::Fast,
            true,
            Command::TakeWhite,
            &mut overlay,
        )
        .unwrap();
        assert!(msg.contains("Swap2 done"), "{msg}");
    }
}
