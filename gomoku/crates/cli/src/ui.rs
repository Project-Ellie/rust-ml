//! Front-ends. `run_plain` scrolls (debugging fallback); `run_ui` owns the
//! alternate screen. Both share `dispatch`.

use std::io::{self, Stdout, Write};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::Print,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::board::{EngineKind, FastBoard, GameBoard, PuzzleBoard, board_for};
use crate::input::{Command, parse};
use crate::opening::{Holders, OpeningPhase, OpeningState, Party};
use crate::puzzle::Puzzle;
use crate::render::{
    overlay_verdict, render_lines, render_lines_with_overlay, render_opening_lines,
    render_opening_styled, render_styled, render_styled_with_overlay, status_line,
};
use engine::{Move, Status};

const HELP_NORMAL: &str =
    "enter <row> <col> to play · 'u' undo · 't' TSS demo (toggle) · 'new' restart · 'q' quit";
const HELP_OPENING: &str = "<row> <col> to place · 'd' done · 'b' take Black · 'w' take White · 'a' add 2 stones · 'u' undo · 'q' quit";
const HELP_PUZZLE: &str = "enter <row> <col> to play · 's' solution step · 'u' undo · 'r' redo · 'n' next · 'p' previous · 't' TSS demo · 'new' reset puzzle · 'q' quit";

/// Top-level application state.
pub(crate) enum AppState {
    /// The Swap2 opening is in progress.
    Opening(OpeningState),
    /// Normal alternating play, optionally with hotseat holders from Swap2.
    Normal(Box<dyn GameBoard>, Option<Holders>),
    /// Puzzle examination mode.
    Puzzle(PuzzleState),
}

pub(crate) struct PuzzleState {
    puzzles: Vec<Puzzle>,
    index: usize,
    board: PuzzleBoard,
}

impl AppState {
    fn new(kind: EngineKind, swap2: bool) -> Self {
        if swap2 {
            AppState::Opening(OpeningState::new())
        } else {
            AppState::Normal(board_for(kind), None)
        }
    }

    fn new_puzzle(puzzles: Vec<Puzzle>, index: usize) -> Self {
        let board = PuzzleBoard::new(&puzzles[index]);
        AppState::Puzzle(PuzzleState {
            puzzles,
            index,
            board,
        })
    }
}

fn help_for(state: &AppState) -> &'static str {
    match state {
        AppState::Opening(_) => HELP_OPENING,
        AppState::Normal(_, _) => HELP_NORMAL,
        AppState::Puzzle(_) => HELP_PUZZLE,
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
            *overlay = None;
            if let AppState::Puzzle(puzzle) = state {
                let puzzles = std::mem::take(&mut puzzle.puzzles);
                let index = puzzle.index;
                *state = AppState::new_puzzle(puzzles, index);
            } else {
                *state = AppState::new(kind, swap2);
            }
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
        AppState::Puzzle(puzzle) => dispatch_puzzle(puzzle, cmd, overlay),
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
        // Not the status line: draw() prints that separately, and a
        // duplicate reads as a rendering bug.
        AppState::Puzzle(_) => {
            "puzzle mode — 'n'/'p' browse · 'u'/'r' history · 'h' help".to_string()
        }
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
        Command::Redo | Command::SolutionStep | Command::NextPuzzle | Command::PrevPuzzle => {
            OpeningResult::Message("that command is only available in puzzle mode".to_string())
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
        Command::Redo => {
            if board.can_redo() {
                board.redo();
                Some(format!("redid move — {:?} to move", board.to_move()))
            } else {
                Some("nothing to redo".to_string())
            }
        }
        Command::Tss => run_tss(board, holders, overlay),
        Command::SolutionStep | Command::NextPuzzle | Command::PrevPuzzle => {
            Some("that command is only available in puzzle mode".to_string())
        }
        Command::Done | Command::TakeBlack | Command::TakeWhite | Command::AddStones => {
            Some("that command is only available during the Swap2 opening".to_string())
        }
        Command::New | Command::Help | Command::Quit => unreachable!("handled globally"),
    }
}

fn dispatch_puzzle(
    puzzle: &mut PuzzleState,
    cmd: Command,
    overlay: &mut Option<engine::Proof>,
) -> Option<String> {
    match cmd {
        Command::Play(mv) => Some(match puzzle.board.play(mv) {
            Ok(()) => String::new(),
            Err(e) => e.to_string(),
        }),
        // Step through the claimed solution line. Deliberately NOT
        // verified against the engine (personal illustration only) —
        // verification belongs to verify_line per ch. 14 decision D5.
        Command::SolutionStep => {
            let line = puzzle.puzzles[puzzle.index].solution.as_deref();
            let Some(line) = line else {
                return Some("this puzzle has no claimed solution".to_string());
            };
            let history = puzzle.board.moves();
            if history.len() >= line.len() {
                return Some("end of the claimed line".to_string());
            }
            if history
                .iter()
                .zip(line.iter())
                .any(|(played, claimed)| played != claimed)
            {
                return Some("history diverged from the claimed line — 'new' to reset".to_string());
            }
            let next = line[history.len()];
            let step = history.len() + 1;
            match puzzle.board.play(next) {
                Ok(()) => Some(format!("claimed line {step}/{}", line.len())),
                Err(e) => Some(format!("claimed line broke at step {step}: {e}")),
            }
        }
        Command::Undo => {
            if puzzle.board.moves().is_empty() {
                Some("already at the puzzle root".to_string())
            } else {
                puzzle.board.undo();
                Some(format!(
                    "undid the last move — {:?} to move",
                    puzzle.board.to_move()
                ))
            }
        }
        Command::Redo => {
            if puzzle.board.can_redo() {
                puzzle.board.redo();
                Some(format!("redid move — {:?} to move", puzzle.board.to_move()))
            } else {
                Some("nothing to redo".to_string())
            }
        }
        Command::NextPuzzle => {
            puzzle.index = (puzzle.index + 1) % puzzle.puzzles.len();
            puzzle.board = PuzzleBoard::new(&puzzle.puzzles[puzzle.index]);
            *overlay = None;
            Some(format!(
                "puzzle {}/{}",
                puzzle.index + 1,
                puzzle.puzzles.len()
            ))
        }
        Command::PrevPuzzle => {
            let n = puzzle.puzzles.len();
            puzzle.index = (puzzle.index + n - 1) % n;
            puzzle.board = PuzzleBoard::new(&puzzle.puzzles[puzzle.index]);
            *overlay = None;
            Some(format!(
                "puzzle {}/{}",
                puzzle.index + 1,
                puzzle.puzzles.len()
            ))
        }
        Command::Tss => run_tss(&puzzle.board, None, overlay),
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
    // Toggle: an active overlay is hidden by pressing 't' again.
    if overlay.is_some() {
        *overlay = None;
        return Some("TSS overlay hidden".to_string());
    }

    let Some(fast) = board.engine_board() else {
        return Some("TSS demo requires the fast engine".to_string());
    };

    // Depth 9 keeps overlay numbers single-digit, which keeps the
    // board at the normal width-3 grid (see render.rs overlay cells).
    let budget = engine::SearchBudget {
        max_nodes: 100_000,
        max_depth: 9,
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
        AppState::Puzzle(puzzle) => puzzle_status_line(puzzle),
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

fn puzzle_status_line(puzzle: &PuzzleState) -> String {
    let p = &puzzle.puzzles[puzzle.index];
    let solution_tag = if p.solution.is_some() {
        " · solution"
    } else {
        ""
    };
    let depth_tag = if let Some(d) = p.depth {
        format!(" · depth {d}")
    } else {
        String::new()
    };
    match puzzle.board.status() {
        Status::Ongoing => format!(
            "{} · {}/{} · {:?} to move{solution_tag}{depth_tag}",
            p.name,
            puzzle.index + 1,
            puzzle.puzzles.len(),
            puzzle.board.to_move()
        ),
        _ => status_line(&puzzle.board),
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
        AppState::Normal(board, _) => render_board(&**board, overlay, styled),
        AppState::Puzzle(puzzle) => render_board(&puzzle.board, overlay, styled),
    }
}

fn render_board(
    board: &dyn GameBoard,
    overlay: Option<&engine::Proof>,
    styled: bool,
) -> Vec<String> {
    if let Some(proof) = overlay {
        if styled {
            render_styled_with_overlay(board, proof)
        } else {
            render_lines_with_overlay(board, proof)
        }
    } else if styled {
        render_styled(board)
    } else {
        render_lines(board)
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

// ------------------------------------------------------------------
// Puzzle mode front-ends

/// Scrolling front-end for puzzle mode: same loop as `run_plain`,
/// browsing and playing puzzles without the alternate screen.
pub fn run_plain_puzzle(puzzles: Vec<Puzzle>, index: usize) -> io::Result<()> {
    let mut state = AppState::new_puzzle(puzzles, index);
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
        // `kind` and `swap2` are unused in puzzle mode because `New`
        // resets the current puzzle, not the normal game.
        match dispatch(&mut state, EngineKind::Fast, false, cmd, &mut overlay) {
            Some(next) => message = next,
            None => break,
        }
    }
    Ok(())
}

/// Alternate-screen front-end for puzzle mode.
pub fn run_ui_puzzle(puzzles: Vec<Puzzle>, index: usize) -> io::Result<()> {
    let _screen = AlternateScreen::enter()?;
    let mut stdout = io::stdout();
    let mut state = AppState::new_puzzle(puzzles, index);
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
        // `kind` and `swap2` are unused in puzzle mode because `New`
        // resets the current puzzle, not the normal game.
        match dispatch(&mut state, EngineKind::Fast, false, cmd, &mut overlay) {
            Some(next) => message = next,
            None => break,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Color, Move};

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

    fn dummy_puzzle(index: u32) -> Puzzle {
        Puzzle {
            name: format!("puzzle #{index}"),
            black: vec![Move::new(7, 7).unwrap()],
            white: vec![],
            to_move: Color::White,
            solution: None,
            depth: None,
        }
    }

    fn dummy_puzzle_with_line() -> Puzzle {
        // White to move; a three-move claimed line on empty cells.
        Puzzle {
            solution: Some(vec![
                Move::new(7, 8).unwrap(),
                Move::new(8, 8).unwrap(),
                Move::new(7, 9).unwrap(),
            ]),
            ..dummy_puzzle(0)
        }
    }

    #[test]
    fn solution_step_walks_the_claimed_line() {
        let puzzles = vec![dummy_puzzle_with_line()];
        let mut state = AppState::new_puzzle(puzzles, 0);
        let mut overlay = None;

        for step in 1..=3 {
            let msg = dispatch(
                &mut state,
                EngineKind::Fast,
                false,
                Command::SolutionStep,
                &mut overlay,
            )
            .unwrap();
            assert_eq!(msg, format!("claimed line {step}/3"));
        }
        let AppState::Puzzle(p) = &state else {
            panic!("expected puzzle state");
        };
        assert_eq!(p.board.moves().len(), 3);

        let msg = dispatch(
            &mut state,
            EngineKind::Fast,
            false,
            Command::SolutionStep,
            &mut overlay,
        )
        .unwrap();
        assert_eq!(msg, "end of the claimed line");
    }

    #[test]
    fn solution_step_without_line_is_refused() {
        let puzzles = vec![dummy_puzzle(0)];
        let mut state = AppState::new_puzzle(puzzles, 0);
        let mut overlay = None;
        let msg = dispatch(
            &mut state,
            EngineKind::Fast,
            false,
            Command::SolutionStep,
            &mut overlay,
        )
        .unwrap();
        assert_eq!(msg, "this puzzle has no claimed solution");
    }

    #[test]
    fn solution_step_refuses_diverged_history_and_recovers_via_undo() {
        let puzzles = vec![dummy_puzzle_with_line()];
        let mut state = AppState::new_puzzle(puzzles, 0);
        let mut overlay = None;

        // Play a move that is NOT the claimed first move.
        dispatch(
            &mut state,
            EngineKind::Fast,
            false,
            Command::Play(Move::new(0, 0).unwrap()),
            &mut overlay,
        );
        let msg = dispatch(
            &mut state,
            EngineKind::Fast,
            false,
            Command::SolutionStep,
            &mut overlay,
        )
        .unwrap();
        assert!(msg.contains("diverged"), "{msg}");

        // Undo back to the root: stepping works again.
        dispatch(
            &mut state,
            EngineKind::Fast,
            false,
            Command::Undo,
            &mut overlay,
        );
        let msg = dispatch(
            &mut state,
            EngineKind::Fast,
            false,
            Command::SolutionStep,
            &mut overlay,
        )
        .unwrap();
        assert_eq!(msg, "claimed line 1/3");
    }

    #[test]
    fn solution_step_is_puzzle_only() {
        let mut state = AppState::new(EngineKind::Fast, false);
        let mut overlay = None;
        let msg = dispatch(
            &mut state,
            EngineKind::Fast,
            false,
            Command::SolutionStep,
            &mut overlay,
        )
        .unwrap();
        assert!(msg.contains("only available in puzzle mode"), "{msg}");
    }

    #[test]
    fn puzzle_navigation_wraps_around() {
        let puzzles = vec![dummy_puzzle(0), dummy_puzzle(1), dummy_puzzle(2)];
        let mut state = AppState::new_puzzle(puzzles, 0);
        let mut overlay = None;

        dispatch(
            &mut state,
            EngineKind::Fast,
            false,
            Command::PrevPuzzle,
            &mut overlay,
        )
        .unwrap();
        let AppState::Puzzle(p) = &state else {
            panic!("expected puzzle state");
        };
        assert_eq!(p.index, 2);

        dispatch(
            &mut state,
            EngineKind::Fast,
            false,
            Command::NextPuzzle,
            &mut overlay,
        )
        .unwrap();
        let AppState::Puzzle(p) = &state else {
            panic!("expected puzzle state");
        };
        assert_eq!(p.index, 0);
    }

    #[test]
    fn puzzle_reset_clears_history() {
        let puzzles = vec![dummy_puzzle(0)];
        let mut state = AppState::new_puzzle(puzzles, 0);
        let mut overlay = None;

        dispatch(
            &mut state,
            EngineKind::Fast,
            false,
            Command::Play(Move::new(7, 8).unwrap()),
            &mut overlay,
        )
        .unwrap();
        let AppState::Puzzle(p) = &state else {
            panic!("expected puzzle state");
        };
        assert_eq!(p.board.moves().len(), 1);

        // Undo once so a redo stack exists; reset must clear it too.
        dispatch(
            &mut state,
            EngineKind::Fast,
            false,
            Command::Undo,
            &mut overlay,
        );
        let AppState::Puzzle(p) = &state else {
            panic!("expected puzzle state");
        };
        assert!(p.board.can_redo());

        dispatch(
            &mut state,
            EngineKind::Fast,
            false,
            Command::New,
            &mut overlay,
        );
        let AppState::Puzzle(p) = &state else {
            panic!("expected puzzle state");
        };
        assert!(p.board.moves().is_empty());
        assert!(!p.board.can_redo());
    }
}
