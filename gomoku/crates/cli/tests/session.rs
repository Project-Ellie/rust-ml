//! Chapter-02 integration tests: drive `SessionCore` through the public
//! crate API only (no `#[cfg(test)]` internals) and assert the event
//! stream a front-end would observe.

use cli::actor::OccupantKind;
use cli::command::{Command, CommandError, Event, SeatSel};
use cli::session::{SessionCore, SessionInit, Snapshot};
use engine::{Board, Color, PlayError, Status};

use testkit::{mv, play};

/// Play a list of moves given as a flat array of row/col pairs.
fn play_some(core: &mut SessionCore, moves: &[u8]) {
    let (coords, _) = moves.as_chunks::<2>();
    for [r, c] in coords.iter() {
        play(core, *r, *c);
    }
}

#[test]
fn hotseat_game_to_black_win_emits_game_over_and_blocks_further_play() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());

    // Five in a row for Black on row 7.
    let script = [
        (7, 3),
        (0, 1),
        (7, 4),
        (0, 2),
        (7, 5),
        (0, 3),
        (7, 6),
        (0, 4),
    ];

    for &(r, c) in script.iter() {
        let events = play(&mut core, r, c);
        assert!(
            !events.iter().any(|e| matches!(e, &Event::GameOver { .. })),
            "Game should not be over yet."
        );
    }

    let events = play(&mut core, 7, 7);
    assert!(
        events.iter().any(|e| matches!(e, Event::GameOver { .. })),
        "Game should be over by now."
    );
    assert!(
        events.iter().any(|e| matches!(e, Event::Moved { .. })),
        "Game should report the last move."
    );

    // The table is closed: further play is rejected.
    let err = core.execute(Command::Play { mv: mv(0, 0) }).unwrap_err();
    assert_eq!(err, CommandError::GameOver);
}

#[test]
fn illegal_move_returns_error_and_leaves_state_unchanged() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    play(&mut core, 7, 7);
    let snapshot_before = core.snapshot();
    let err = core.execute(Command::Play { mv: mv(7, 7) }).unwrap_err();
    assert_eq!(err, CommandError::IllegalMove(PlayError::Occupied));
    assert_eq!(core.snapshot(), snapshot_before);
}

#[test]
fn branch_mid_game_emits_branched_event_with_correct_options() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    play_some(&mut core, &[7, 7, 6, 7, 7, 8, 6, 8, 7, 9, 6, 9]);
    core.execute(Command::Back { n: 3 }).unwrap();
    let events = play(&mut core, 8, 7);
    assert!(
        events.iter().any(|e| matches!(e, &Event::Branched { .. })),
        "Expected Branched event, got {events:?}"
    );

    let branched = events.iter().find_map(|e| match e {
        Event::Branched { options } => Some(options.clone()),
        _ => None,
    });

    assert_eq!(branched, Some(vec![mv(6, 8), mv(8, 7)]));
}

#[test]
fn back_forward_and_var_emit_navigation_events() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    play_some(&mut core, &[7, 7, 6, 7, 7, 8]);
    let events = core.execute(Command::Back { n: 2 }).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, &Event::CursorMoved { ply: 1 }))
    );
    let events = core.execute(Command::Forward { n: 1 }).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, &Event::CursorMoved { ply: 2 }))
    );

    core.execute(Command::Back { n: 1 }).unwrap();
    play(&mut core, 8, 7);
    core.execute(Command::Back { n: 1 }).unwrap();

    let events = core.execute(Command::Forward { n: 1 }).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::BranchPoint { .. })),
        "Expected BranchPoint. Got {events:?}"
    );

    let events = core.execute(Command::Var { index: 1 }).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::CursorMoved { ply: 2 }))
    );
}

#[test]
fn new_emits_reset_and_returns_to_opening() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    play_some(&mut core, &[7, 7, 6, 7]);
    let events = core.execute(Command::New).unwrap();
    assert!(events.iter().any(|e| matches!(e, &Event::Reset)));
    assert_eq!(core.board(), Board::new());
    assert_eq!(core.snapshot().ply, 0);
}

#[test]
fn step_on_human_seat_returns_typed_error() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    let err = core.execute(Command::Step).unwrap_err();
    assert_eq!(err, CommandError::StepRequiresAi { seat: Color::Black });
}

#[test]
fn release_sets_seat_to_ai_and_take_reclaims_it() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    let events = core
        .execute(Command::Release {
            seat: SeatSel::White,
        })
        .unwrap();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::SeatChanged {
            seat: Color::White,
            now: OccupantKind::Ai,
        }
    )));
    assert_eq!(core.snapshot().white, OccupantKind::Ai);

    let events = core
        .execute(Command::Take {
            seat: SeatSel::White,
        })
        .unwrap();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::SeatChanged {
            seat: Color::White,
            now: OccupantKind::Human,
        }
    )));
    assert_eq!(core.snapshot().white, OccupantKind::Human);
}

#[test]
fn set_sims_updates_override_and_emits_config_changed() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    let events = core
        .execute(Command::SetSims {
            seat: SeatSel::Black,
            n: 123,
        })
        .unwrap();
    assert_eq!(events.len(), 1);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::ConfigChanged {
            seat: Color::Black,
            sims: 123,
        }
    )));
}

#[test]
fn take_is_no_op_for_human_seats() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    let events = core
        .execute(Command::Take {
            seat: SeatSel::Both,
        })
        .unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|e| matches!(
        e,
        &Event::SeatChanged {
            now: OccupantKind::Human,
            ..
        }
    )))
}

#[test]
fn seeded_determinism_smoke() {
    let init = SessionInit {
        seed: Some(12345),
        ..SessionInit::new_hotseat()
    };
    let mut a = SessionCore::new(init.clone());
    let mut b = SessionCore::new(init);
    let script = [(7, 7), (6, 7), (7, 8), (6, 8)];
    for &(r, c) in &script {
        let ev_a = play(&mut a, r, c);
        let ev_b = play(&mut b, r, c);
        assert_eq!(ev_a, ev_b);
    }
    assert_eq!(a.snapshot(), b.snapshot());
}

// --- Snapshot serde contract: the future gomoku-gui REST seam ---

fn round_trip(snapshot: &Snapshot) {
    let json = serde_json::to_string(snapshot).expect("snapshot must serialize");
    let back: Snapshot = serde_json::from_str(&json).expect("snapshot must deserialize");
    assert_eq!(&back, snapshot, "round trip must be lossless: {json}");
}

#[test]
fn snapshot_serde_round_trip_covers_all_statuses() {
    // A live mid-game snapshot from a real session.
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    play(&mut core, 7, 7);
    play(&mut core, 6, 7);
    round_trip(&core.snapshot());

    // All status variants, with branch options and a last move present.
    for status in [
        Status::Ongoing,
        Status::Won(Color::Black),
        Status::Won(Color::White),
        Status::Draw,
    ] {
        round_trip(&Snapshot {
            ply: 3,
            branch_options: vec![(0, mv(6, 7)), (1, mv(8, 7))],
            black: OccupantKind::Human,
            white: OccupantKind::Ai,
            status,
            last_move: Some(mv(7, 7)),
            opening: None,
        });
    }

    // Edge case: fresh session shape — no options, no last move.
    round_trip(&Snapshot {
        ply: 0,
        branch_options: vec![],
        black: OccupantKind::Human,
        white: OccupantKind::Human,
        status: Status::Ongoing,
        last_move: None,
        opening: None,
    });
}

#[test]
fn snapshot_json_wire_format_is_stable() {
    let snapshot = Snapshot {
        ply: 2,
        branch_options: vec![(0, mv(6, 7)), (1, mv(8, 7))],
        black: OccupantKind::Human,
        white: OccupantKind::Ai,
        status: Status::Won(Color::Black),
        last_move: Some(mv(7, 7)),
        opening: None,
    };
    let value = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "ply": 2,
            "branch_options": [[0, 97], [1, 127]],
            "black": "Human",
            "white": "Ai",
            "status": "won_black",
            "last_move": 112,
        })
    );
}

#[test]
fn snapshot_deserialization_rejects_bad_input() {
    let good = serde_json::json!({
        "ply": 0,
        "branch_options": [],
        "black": "Human",
        "white": "Human",
        "status": "ongoing",
        "last_move": null,
    });

    // Unknown status string is rejected.
    let mut bad_status = good.clone();
    bad_status["status"] = serde_json::json!("won_green");
    assert!(serde_json::from_value::<Snapshot>(bad_status).is_err());

    // A missing mandatory field is rejected. Note: `last_move` is
    // `Option<Move>`, which serde treats as implicitly optional — the
    // mandatory fields on the wire are the non-Option ones.
    let mut missing = good.clone();
    missing.as_object_mut().unwrap().remove("status");
    assert!(serde_json::from_value::<Snapshot>(missing).is_err());
}
