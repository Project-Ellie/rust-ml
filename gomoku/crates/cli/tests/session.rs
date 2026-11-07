//! Chapter-02 integration tests: drive `SessionCore` through the public
//! crate API only (no `#[cfg(test)]` internals) and assert the event
//! stream a front-end would observe.

use cli::actor::OccupantKind;
use cli::command::{Command, CommandError, Event, SeatSel};
use cli::puzzle;
use cli::session::{SessionCore, SessionInit, Snapshot};
use engine::{Board, Color, PlayError, Status};
use testkit::{mv, play, play_some};

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
fn resignation_snapshot_serde_preserves_old_wire_and_both_optional_fields() {
    use cli::history::Resignation;
    use cli::session::Snapshot;
    let core = SessionCore::new(SessionInit::hotseat());
    let plain = core.snapshot();
    let old_wire = serde_json::to_value(&plain).unwrap();
    assert_eq!(old_wire.as_object().unwrap().len(), 6);
    assert!(old_wire.get("resignation").is_none());
    assert_eq!(serde_json::from_value::<Snapshot>(old_wire).unwrap(), plain);
    let opening = SessionCore::new(SessionInit::hotseat().with_swap2(true))
        .snapshot()
        .opening;
    for opening in [None, opening] {
        let annotated = Snapshot {
            resignation: Some(Resignation {
                by: Color::Black,
                candidates: 5,
            }),
            opening,
            ..plain.clone()
        };
        let value = serde_json::to_value(&annotated).unwrap();
        assert_eq!(
            value.as_object().unwrap().len(),
            7 + usize::from(annotated.opening.is_some())
        );
        assert_eq!(
            value["resignation"],
            serde_json::json!({"by": "Black", "candidates": 5})
        );
        assert_eq!(
            serde_json::from_value::<Snapshot>(value).unwrap(),
            annotated
        );
    }
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
            resignation: None,
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
        resignation: None,
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
        resignation: None,
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

fn mini_pack() -> puzzle::PuzzlePack {
    puzzle::PuzzlePack::from_json(include_str!("fixtures/mini-pack.json")).unwrap()
}

fn puzzle_session(index: usize) -> SessionCore {
    let init = SessionInit::puzzle(
        std::path::PathBuf::from("mini-pack.json"),
        mini_pack(),
        index,
    )
    .unwrap();
    SessionCore::new(init)
}

#[test]
fn puzzle_session_roots_at_puzzle_position_with_attacker_to_move() {
    let core = puzzle_session(1);
    assert_eq!(core.board(), mini_pack().items[1].board);
    assert_eq!(core.board().to_move(), Color::White);
    let study = core.puzzle().expect("puzzle context");
    assert_eq!(study.index, 1);
    assert_eq!(study.pack.items.len(), 3);
}

#[test]
fn puzzle_init_rejects_out_of_range_index() {
    let err = SessionInit::puzzle(std::path::PathBuf::from("mini-pack.json"), mini_pack(), 3)
        .unwrap_err();
    assert!(matches!(
        err,
        puzzle::PuzzleError::IndexOutOfRange { index: 3, count: 3 }
    ));
}

#[test]
fn puzzle_position_supports_full_study_flow() {
    let mut core = puzzle_session(2);
    play(&mut core, 7, 10);
    core.execute(Command::Back { n: 1 }).unwrap();
    let events = play(&mut core, 10, 10);
    assert!(
        events.iter().any(|e| matches!(e, &Event::Branched { .. })),
        "expected Branched, got {events:?}"
    );
    core.execute(Command::Back { n: 1 }).unwrap();
    let events = core.execute(Command::Forward { n: 1 }).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, &Event::BranchPoint { .. })),
        "Expected BranchPoint, got {events:?}"
    );
    let events = core.execute(Command::Var { index: 0 }).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, &Event::CursorMoved { .. })),
        "expected CursorMoved, got {events:?}"
    );
}

#[test]
fn new_resets_to_the_initially_loaded_puzzle() {
    let mut core = puzzle_session(0);
    core.execute(Command::NextPuzzle).unwrap();
    play(&mut core, 9, 9);
    let events = core.execute(Command::New).unwrap();
    assert!(events.iter().any(|e| matches! {e, &Event::Reset}));
    assert_eq!(core.puzzle().unwrap().index, 0);
    assert_eq!(core.board(), mini_pack().items[0].board);
}

#[test]
fn hint_emits_first_solution_move() {
    let mut core = puzzle_session(0);
    let events = core.execute(Command::Hint).unwrap();
    assert_eq!(events, vec![Event::Hint { mv: mv(7, 7) }]);
}

#[test]
fn hint_errors_when_puzzle_has_no_line() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [], "l": [] }
    ] }"#;
    let pack = puzzle::PuzzlePack::from_json(text).unwrap();
    let init = SessionInit::puzzle(std::path::PathBuf::from("pack.json"), pack, 0).unwrap();
    let mut core = SessionCore::new(init);
    assert_eq!(
        core.execute(Command::Hint).unwrap_err(),
        CommandError::NoSolutionLine { index: 0, count: 1 }
    );
}

#[test]
fn reveal_emits_full_claimed_line() {
    let mut core = puzzle_session(2);
    let events = core.execute(Command::Reveal).unwrap();
    assert_eq!(
        events,
        vec![Event::SolutionRevealed {
            line: vec![
                (mv(7, 10), Color::Black),
                (mv(7, 6), Color::White),
                (mv(7, 11), Color::Black)
            ]
        }]
    );
}

#[test]
fn next_and_prev_load_a_fresh_root_and_report_position() {
    let mut core = puzzle_session(0);
    play(&mut core, 9, 9);
    let events = core.execute(Command::NextPuzzle).unwrap();
    assert_eq!(
        events,
        vec![Event::PuzzleLoaded {
            index: 1,
            count: 3,
            attacker: Color::White,
        }]
    );
    assert_eq!(core.snapshot().ply, 0);
    assert_eq!(core.board(), mini_pack().items[1].board);

    let events = core.execute(Command::PrevPuzzle).unwrap();
    assert_eq!(
        events,
        vec![Event::PuzzleLoaded {
            index: 0,
            count: 3,
            attacker: Color::Black,
        }]
    );
    assert_eq!(core.board(), mini_pack().items[0].board);
}

#[test]
fn next_prev_clamp_at_pack_ends() {
    let mut core = puzzle_session(0);
    assert_eq!(
        core.execute(Command::PrevPuzzle).unwrap_err(),
        CommandError::AtFirstPuzzle { count: 3 }
    );
    core.execute(Command::NextPuzzle).unwrap();
    core.execute(Command::NextPuzzle).unwrap();
    assert_eq!(
        core.execute(Command::NextPuzzle).unwrap_err(),
        CommandError::AtLastPuzzle { count: 3 }
    );
    assert_eq!(core.puzzle().unwrap().index, 2);
}

#[test]
fn puzzle_commands_error_without_a_pack() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    for command in [
        Command::Hint,
        Command::Reveal,
        Command::NextPuzzle,
        Command::PrevPuzzle,
    ] {
        assert_eq!(
            core.execute(command).unwrap_err(),
            CommandError::NoPuzzleLoaded
        );
    }
}
