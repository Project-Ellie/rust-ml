//! The shared test kit for the `cli` integration tests.
//!
//! Every file in `tests/` gets the helpers it needs from here instead of
//! defining its own copies. One helper exists once, with one name and one
//! comment, so a fix or an explanation lands in a single place.
//!
//! Each integration-test binary compiles its own copy of this module, and
//! no binary uses every helper. The module-level `allow` keeps `dead_code`
//! warnings (promoted to errors by clippy `-D warnings`) from firing on
//! the helpers a given binary does not use.
#![allow(dead_code)]

use cli::actor::{AiConfig, EvaluatorKind};
use cli::command::{Command, Event};
use cli::session::{SessionCore, SessionInit};
use engine::{Color, Move};

/// Build a move from zero-based row and column.
///
/// Test scripts are full of coordinates; the `unwrap` is acceptable here
/// because a test that constructs an invalid move should fail loudly.
pub fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}

/// Play one stone and return the events the session emitted.
///
/// Tests that use this helper expect the move to be legal; an illegal move
/// panics through the `unwrap`, which points at the test script.
pub fn play(core: &mut SessionCore, r: u8, c: u8) -> Vec<Event> {
    core.execute(Command::Play {
        mv: mv(r, c),
        color: None,
    })
    .unwrap()
}

/// Place one stone with an explicit color token: the Swap2 override.
///
/// Legal only during the opening; outside it the session returns the
/// typed `ColorOverrideOutsideOpening` error and the `unwrap` fails the
/// test, which is the intent.
pub fn play_color(core: &mut SessionCore, r: u8, c: u8, color: Color) -> Vec<Event> {
    core.execute(Command::Play {
        mv: mv(r, c),
        color: Some(color),
    })
    .unwrap()
}

/// A hotseat session that starts inside the Swap2 opening ceremony.
pub fn swap2_core() -> SessionCore {
    SessionCore::new(SessionInit::new_hotseat().with_swap2(true))
}

/// Let the AI play the side to move and return the events.
///
/// The seat must be AI-occupied; otherwise `Step` returns an error and the
/// `unwrap` fails the test, which is the intent.
pub fn step(core: &mut SessionCore) -> Vec<Event> {
    core.execute(Command::Step).unwrap()
}

/// The name of an event's variant.
///
/// Many assertions care only about which things happened, not about the
/// payload. Comparing variant names keeps those assertions short.
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
        Event::StonePlaced { .. } => "StonePlaced",
        Event::StoneRemoved { .. } => "StoneRemoved",
        Event::OpeningPhaseChanged { .. } => "OpeningPhaseChanged",
        Event::OpeningFinished { .. } => "OpeningFinished",
    }
}

/// The variant names of a whole event batch, in order.
///
/// Use this to assert an exact expected sequence:
/// `assert_eq!(event_types(&events), ["Moved", "GameOver"])`.
pub fn event_types(events: &[Event]) -> Vec<&'static str> {
    events.iter().map(event_type).collect()
}

/// The AI configuration shared by the integration tests.
///
/// Deliberate divergence from [`AiConfig::default`]: the tests run 200
/// simulations instead of the default 400, to keep the test suite fast.
/// Every other field matches the default. One builder with this comment
/// replaces the per-file copies (`ai_config_tactics`, `default_ai_config`)
/// that existed before this kit.
pub fn fast_tactics_config() -> AiConfig {
    AiConfig {
        evaluator: EvaluatorKind::Tactics,
        simulations: 200,
        c_puct: 1.5,
        temperature: 0.0,
        noise_epsilon: 0.0,
        explicit_selection: false,
    }
}
