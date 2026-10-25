//! The shared test kit for all tests in the gomoku workspace.
//!
//! One copy of the small helpers every test suite uses: move builders,
//! board builders, session drivers, and AI config builders. Test tools
//! only — nothing here belongs in a production target.

use cli::actor::{AiConfig, EvaluatorKind};
use cli::command::{Command, Event};
use cli::history::History;
use cli::session::SessionCore;
use engine::{Board, Color, Move};

/// Build a `Move` from zero-based row/column, panicking if out of range.
pub fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}

/// An empty 15×15 board with Black to move.
pub fn empty_board() -> Board {
    Board::new()
}

/// A fixed opening position: Black at (7,7) and (6,8), White at (6,7),
/// White to move.
pub fn opening_board() -> Board {
    Board::from_position(&[mv(7, 7), mv(6, 8)], &[mv(6, 7)], Color::White).unwrap()
}

/// Replay the moves on the history's current cursor path onto a fresh
/// board, so tests can cross-check `history.board()` against an
/// independently rebuilt position.
pub fn replay_cursor_path(history: &History) -> Board {
    let mut board = empty_board();
    for &m in history.board().moves() {
        board.play(m).unwrap();
    }
    board
}

/// Play one move on the session and return the emitted events.
pub fn play(core: &mut SessionCore, r: u8, c: u8) -> Vec<Event> {
    core.execute(Command::Play(mv(r, c))).unwrap()
}

/// Play a list of moves given as a flat array of row/col pairs.
pub fn play_some(core: &mut SessionCore, moves: &[u8]) {
    let (coords, _) = moves.as_chunks::<2>();
    for [r, c] in coords.iter() {
        play(core, *r, *c);
    }
}

/// Step the AI seat once and return the emitted events.
pub fn step(core: &mut SessionCore) -> Vec<Event> {
    core.execute(Command::Step).unwrap()
}

/// Classify an event by variant name, for transcript-style assertions.
pub fn event_type(ev: &Event) -> &'static str {
    match ev {
        Event::Moved { .. } => "Moved",
        Event::Branched { .. } => "Branched",
        Event::CursorMoved { .. } => "CursorMoved",
        Event::BranchPoint { .. } => "BranchPoint",
        Event::SeatChanged { .. } => "SeatChanged",
        Event::ConfigChanged { .. } => "ConfigChanged",
        Event::GameOver { .. } => "GameOver",
        Event::Reset => "Reset",
    }
}

/// The actor tests' baseline AI config: tactics evaluator, 200
/// simulations, deterministic selection (temperature 0, no noise).
pub fn ai_config_tactics() -> AiConfig {
    AiConfig {
        evaluator: EvaluatorKind::Tactics,
        simulations: 200,
        c_puct: 1.5,
        temperature: 0.0,
        noise_epsilon: 0.0,
        explicit_selection: false,
    }
}

/// The study tests' default AI config: tactics evaluator, 200
/// simulations, deterministic selection, scheduled (non-explicit)
/// opening.
pub fn default_ai_config() -> AiConfig {
    AiConfig {
        evaluator: EvaluatorKind::Tactics,
        simulations: 200,
        c_puct: 1.5,
        temperature: 0.0,
        noise_epsilon: 0.0,
        explicit_selection: false,
    }
}
