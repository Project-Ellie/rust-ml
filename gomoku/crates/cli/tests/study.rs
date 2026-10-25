//! Task-4 capstone: the study loop.
//!
//! Take/release seats mid-game, per-seat sims, and the AI-vs-AI opening
//! schedule are all exercised as scripted command transcripts.

use cli::actor::{AiConfig, EvaluatorKind, Occupant, OccupantKind};
use cli::command::{Command, Event, SeatSel};
use cli::session::{SessionCore, SessionInit};
use engine::Move;

fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}

fn play(core: &mut SessionCore, r: u8, c: u8) -> Vec<Event> {
    core.execute(Command::Play(mv(r, c))).unwrap()
}

fn step(core: &mut SessionCore) -> Vec<Event> {
    core.execute(Command::Step).unwrap()
}

fn event_type(ev: &Event) -> &'static str {
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

fn default_ai_config() -> AiConfig {
    AiConfig {
        evaluator: EvaluatorKind::Tactics,
        simulations: 200,
        c_puct: 1.5,
        temperature: 0.0,
        noise_epsilon: 0.0,
        explicit_selection: false,
    }
}

#[test]
fn scripted_study_transcript() {
    // AI-vs-AI for 6 plies from an empty root.
    let mut core = SessionCore::new(SessionInit::ai_vs_ai(default_ai_config(), 7));
    let mut ai_moves: Vec<Move> = Vec::new();
    for _ in 0..6 {
        let events = step(&mut core);
        assert_eq!(
            events.len(),
            1,
            "a single Step should emit exactly one Moved event"
        );
        let moved = events.iter().find_map(|e| match e {
            Event::Moved { mv, .. } => Some(*mv),
            _ => None,
        });
        ai_moves.push(moved.expect("Step must produce a move"));
    }

    // Steal Black's seat.
    let events = core
        .execute(Command::Take {
            seat: SeatSel::Black,
        })
        .unwrap();
    assert_eq!(event_type(&events[0]), "SeatChanged");
    assert_eq!(core.snapshot().black, OccupantKind::Human);

    // Branch from an earlier cursor: go back 2 plies and play something
    // different than the AI's original move there. The alternative is
    // picked from the legal moves at the cursor rather than hardcoded, so
    // the test does not depend on which squares the sampled opening hit.
    core.execute(Command::Back { n: 2 }).unwrap();
    let original_ply5 = ai_moves[4];
    let human_alt = core
        .board()
        .empty_moves()
        .find(|&m| m != original_ply5)
        .expect("there must be a legal alternative to the AI's move");
    let events = play(&mut core, human_alt.row(), human_alt.col());
    assert_eq!(event_type(&events[0]), "Moved");
    assert!(matches!(
        events[0],
        Event::Moved {
            by: OccupantKind::Human,
            ..
        }
    ));
    assert_eq!(event_type(&events[1]), "Branched");
    if let Event::Branched { options } = &events[1] {
        assert_eq!(options.len(), 2);
        assert_eq!(options[0], original_ply5);
        assert_eq!(options[1], human_alt);
    }

    // Go back to the branch point and return to the main line.
    let events = core.execute(Command::Back { n: 1 }).unwrap();
    assert_eq!(event_type(&events[0]), "CursorMoved");

    let events = core.execute(Command::Var { index: 0 }).unwrap();
    assert_eq!(event_type(&events[0]), "CursorMoved");

    // Hand Black back to the AI and let it continue.
    let events = core
        .execute(Command::Release {
            seat: SeatSel::Black,
        })
        .unwrap();
    assert_eq!(event_type(&events[0]), "SeatChanged");
    assert_eq!(core.snapshot().black, OccupantKind::Ai);

    let events = step(&mut core);
    assert_eq!(event_type(&events[0]), "Moved");
    assert!(matches!(
        events[0],
        Event::Moved {
            by: OccupantKind::Ai,
            ..
        }
    ));

    assert_eq!(core.snapshot().black, OccupantKind::Ai);
    assert_eq!(core.snapshot().white, OccupantKind::Ai);
}

#[test]
fn opening_schedule_changes_ai_vs_ai_play() {
    let seed = 42;
    let scheduled_config = default_ai_config();
    let explicit_config = AiConfig {
        explicit_selection: true,
        ..default_ai_config()
    };

    let mut scheduled = SessionCore::new(SessionInit::ai_vs_ai(scheduled_config, seed));
    let mut explicit = SessionCore::new(SessionInit::ai_vs_ai(explicit_config, seed));

    let mut scheduled_moves = Vec::new();
    let mut explicit_moves = Vec::new();

    for ply in 0..12 {
        let scheduled_events = step(&mut scheduled);
        let explicit_events = step(&mut explicit);

        let scheduled_mv = scheduled_events.iter().find_map(|e| match e {
            Event::Moved { mv, .. } => Some(*mv),
            _ => None,
        });
        let explicit_mv = explicit_events.iter().find_map(|e| match e {
            Event::Moved { mv, .. } => Some(*mv),
            _ => None,
        });

        scheduled_moves.push(scheduled_mv.unwrap());
        explicit_moves.push(explicit_mv.unwrap());

        // Inside the scheduled window the effective config carries the
        // schedule's temperature and noise; outside it matches the default.
        let info = scheduled.search_info().expect("search info stored");
        if ply < 10 {
            assert_eq!(info.config.temperature, 1.0);
            assert_eq!(info.config.noise_epsilon, 0.25);
        } else {
            assert_eq!(info.config.temperature, 0.0);
            assert_eq!(info.config.noise_epsilon, 0.0);
        }

        // Explicit config is always applied verbatim.
        let explicit_info = explicit.search_info().expect("search info stored");
        assert_eq!(explicit_info.config.temperature, 0.0);
        assert_eq!(explicit_info.config.noise_epsilon, 0.0);
    }

    // The scheduled opening should diverge from pure argmax at least once
    // in the first 10 plies.
    assert_ne!(
        &scheduled_moves[..10],
        &explicit_moves[..10],
        "scheduled AI-vs-AI must differ from explicit argmax in the first 10 plies"
    );
}

#[test]
fn per_seat_sims_reach_search() {
    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Human,
        white: Occupant::Human,
        default_config: default_ai_config(),
        black_sims: Some(50),
        white_sims: Some(400),
        seed: Some(1),
        opening: engine::Board::new(),
    });

    core.execute(Command::Release {
        seat: SeatSel::Both,
    })
    .unwrap();

    let events = step(&mut core);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Moved {
            by: OccupantKind::Ai,
            ..
        }
    )));
    let black_info = core.search_info().expect("search info after black step");
    assert_eq!(
        black_info.visits.iter().map(|(_, n)| n).sum::<u32>(),
        50,
        "Black AI must use black_sims"
    );

    step(&mut core);
    let white_info = core.search_info().expect("search info after white step");
    assert_eq!(
        white_info.visits.iter().map(|(_, n)| n).sum::<u32>(),
        400,
        "White AI must use white_sims"
    );
}

#[test]
fn take_and_release_are_idempotent() {
    let mut core = SessionCore::new(SessionInit::hotseat());

    // Taking an already-human seat emits SeatChanged but leaves it human.
    let first = core
        .execute(Command::Take {
            seat: SeatSel::Black,
        })
        .unwrap();
    assert_eq!(event_type(&first[0]), "SeatChanged");
    assert_eq!(core.snapshot().black, OccupantKind::Human);

    let second = core
        .execute(Command::Take {
            seat: SeatSel::Black,
        })
        .unwrap();
    assert_eq!(event_type(&second[0]), "SeatChanged");
    assert_eq!(core.snapshot().black, OccupantKind::Human);

    // Releasing an already-AI seat emits SeatChanged but leaves it AI.
    core.execute(Command::Release {
        seat: SeatSel::White,
    })
    .unwrap();
    assert_eq!(core.snapshot().white, OccupantKind::Ai);

    let again = core
        .execute(Command::Release {
            seat: SeatSel::White,
        })
        .unwrap();
    assert_eq!(event_type(&again[0]), "SeatChanged");
    assert_eq!(core.snapshot().white, OccupantKind::Ai);
}
