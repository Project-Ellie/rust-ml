//! Chapter-02 integration tests: drive `SessionCore` through the public
//! crate API only (no `#[cfg(test)]` internals) and assert the event
//! stream a front-end would observe.

use cli::actor::OccupantKind;
use cli::command::{Command, CommandError, Event, SeatSel};
use cli::session::{SessionCore, SessionInit};
use engine::{Board, Move, PlayError};

fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}

fn play(core: &mut SessionCore, r: u8, c: u8) -> Vec<Event> {
    core.execute(Command::Play(mv(r, c))).unwrap()
}

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
    let err = core.execute(Command::Play(mv(0, 0))).unwrap_err();
    assert_eq!(err, CommandError::GameOver);
}

#[test]
fn illegal_move_returns_error_and_leaves_state_unchanged() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    play(&mut core, 7, 7);
    let snapshot_before = core.snapshot();
    let err = core.execute(Command::Play(mv(7, 7))).unwrap_err();
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
fn step_release_and_set_sims_are_not_yet_implemented() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());

    assert!(matches!(
        core.execute(Command::Step),
        Err(CommandError::AiSeatsNotImplemented(_))
    ));

    assert!(matches!(
        core.execute(Command::Release {
            seat: SeatSel::White
        }),
        Err(CommandError::AiSeatsNotImplemented(_))
    ));

    assert!(matches!(
        core.execute(Command::SetSims {
            seat: SeatSel::Black,
            n: 100
        }),
        Err(CommandError::AiSeatsNotImplemented(_))
    ));
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
