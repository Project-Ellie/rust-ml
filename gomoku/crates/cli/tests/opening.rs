//! Sub-chapter 05c integration tests: the Swap2 opening as a session
//! phase, driven through the public command language.
//!
//! Every test is headless: commands in, events and snapshots out. The
//! engine's typestate stays the rules authority; these tests assert the
//! session-level behavior around it — the replay log, the phase
//! routing, the typed errors, and the handover to the variation tree.

use cli::command::{Command, CommandError, Event, OpeningPhase};
use cli::session::{SessionCore, SessionInit, Snapshot};
use engine::{Board, Color, PlayError};

use testkit::{event_types, mv, play, play_color, swap2_core};

/// Place the first round with the default alternation (black, white,
/// black) and end it with `done`. Returns the events of `done`.
fn place_first_round(core: &mut SessionCore) -> Vec<Event> {
    play(core, 7, 7);
    play(core, 8, 8);
    play(core, 9, 9);
    core.execute(Command::Done).unwrap()
}

/// Run the second round: `more`, two placements, `done`.
fn place_second_round(core: &mut SessionCore) {
    core.execute(Command::More).unwrap();
    play(core, 5, 5);
    play(core, 4, 4);
    core.execute(Command::Done).unwrap();
}

/// Count the stones of `color` through the public per-cell API.
/// (`Board::stones` is engine-internal; a test goes cell by cell.)
fn stone_count(board: &Board, color: Color) -> usize {
    (0..15)
        .flat_map(|r| (0..15).map(move |c| mv(r, c)))
        .filter(|&m| board.stone_at(m) == Some(color))
        .count()
}

#[test]
fn take_black_finishes_the_ceremony_and_white_moves_first() {
    let mut core = swap2_core();

    // The default order alternates, black first.
    let events = play(&mut core, 7, 7);
    assert_eq!(
        events,
        vec![Event::StonePlaced {
            mv: mv(7, 7),
            color: Color::Black
        }]
    );
    play(&mut core, 8, 8);
    play(&mut core, 9, 9);

    let done = core.execute(Command::Done).unwrap();
    assert_eq!(
        done,
        vec![Event::OpeningPhaseChanged {
            phase: OpeningPhase::FirstChoice
        }]
    );

    let events = core
        .execute(Command::ChooseColor {
            color: Color::Black,
        })
        .unwrap();
    assert_eq!(
        events,
        vec![Event::OpeningFinished {
            chosen: Color::Black
        }]
    );

    // The ceremony is over: the snapshot carries no opening, and the
    // game board holds the placed stones with White to move.
    assert_eq!(core.snapshot().opening, None);
    let board = core.board();
    assert_eq!(board.to_move(), Color::White);
    assert_eq!(board.stone_at(mv(7, 7)), Some(Color::Black));
    assert_eq!(board.stone_at(mv(8, 8)), Some(Color::White));
    assert_eq!(board.stone_at(mv(9, 9)), Some(Color::Black));

    // Play continues with the normal command set; White moves first,
    // so the next stone on the board is white.
    let events = play(&mut core, 10, 10);
    assert_eq!(event_types(&events), ["Moved"]);
    assert_eq!(core.board().stone_at(mv(10, 10)), Some(Color::White));
}

#[test]
fn take_white_finishes_the_ceremony_with_the_same_stones() {
    let mut core = swap2_core();
    place_first_round(&mut core);

    let events = core
        .execute(Command::ChooseColor {
            color: Color::White,
        })
        .unwrap();
    assert_eq!(
        events,
        vec![Event::OpeningFinished {
            chosen: Color::White
        }]
    );

    // Whoever chooses, the side to move is White: White always has
    // fewer stones after the ceremony.
    let board = core.board();
    assert_eq!(board.to_move(), Color::White);
    assert_eq!(stone_count(&board, Color::Black), 2);
    assert_eq!(stone_count(&board, Color::White), 1);
    // The ceremony stones are the root position, not moves in the tree.
    assert_eq!(core.snapshot().ply, 0);
    assert_eq!(core.snapshot().last_move, None);
}

#[test]
fn more_places_two_more_stones_then_player_a_chooses() {
    let mut core = swap2_core();
    place_first_round(&mut core);

    let events = core.execute(Command::More).unwrap();
    assert_eq!(
        events,
        vec![Event::OpeningPhaseChanged {
            phase: OpeningPhase::Placing2
        }]
    );

    // The second round alternates as well, starting with black.
    let events = play(&mut core, 5, 5);
    assert_eq!(
        events,
        vec![Event::StonePlaced {
            mv: mv(5, 5),
            color: Color::Black
        }]
    );
    let events = play(&mut core, 4, 4);
    assert_eq!(
        events,
        vec![Event::StonePlaced {
            mv: mv(4, 4),
            color: Color::White
        }]
    );

    let events = core.execute(Command::Done).unwrap();
    assert_eq!(
        events,
        vec![Event::OpeningPhaseChanged {
            phase: OpeningPhase::FinalChoice
        }]
    );

    let events = core
        .execute(Command::ChooseColor {
            color: Color::White,
        })
        .unwrap();
    assert_eq!(
        events,
        vec![Event::OpeningFinished {
            chosen: Color::White
        }]
    );

    let board = core.board();
    assert_eq!(stone_count(&board, Color::Black), 3);
    assert_eq!(stone_count(&board, Color::White), 2);
    assert_eq!(board.to_move(), Color::White);
}

#[test]
fn back_removes_placements_and_recomputes_the_phase() {
    let mut core = swap2_core();
    play(&mut core, 7, 7);
    play(&mut core, 8, 8);

    // Plain undo inside a round: one StoneRemoved, no phase change.
    let events = core.execute(Command::Back { n: 1 }).unwrap();
    assert_eq!(events, vec![Event::StoneRemoved { mv: mv(8, 8) }]);
    let opening = core.snapshot().opening.unwrap();
    assert_eq!(opening.placements.len(), 1);
    assert_eq!(opening.phase, OpeningPhase::Placing3);

    // Undo across a finished round: removing the stone that completed
    // the round takes the `done` back with it.
    play(&mut core, 8, 8);
    play(&mut core, 9, 9);
    core.execute(Command::Done).unwrap();
    let events = core.execute(Command::Back { n: 1 }).unwrap();
    assert_eq!(
        events,
        vec![
            Event::StoneRemoved { mv: mv(9, 9) },
            Event::OpeningPhaseChanged {
                phase: OpeningPhase::Placing3
            },
        ]
    );

    // `back N` removes up to N placements.
    let events = core.execute(Command::Back { n: 2 }).unwrap();
    assert_eq!(event_types(&events), ["StoneRemoved", "StoneRemoved"]);
    assert!(core.snapshot().opening.unwrap().placements.is_empty());
}

#[test]
fn back_at_the_final_choice_returns_to_the_second_round() {
    let mut core = swap2_core();
    place_first_round(&mut core);
    place_second_round(&mut core);

    let events = core.execute(Command::Back { n: 1 }).unwrap();
    assert_eq!(
        events,
        vec![
            Event::StoneRemoved { mv: mv(4, 4) },
            Event::OpeningPhaseChanged {
                phase: OpeningPhase::Placing2
            },
        ]
    );

    // The taken-back stone can be placed elsewhere; the ceremony
    // continues normally to a finished opening.
    play(&mut core, 3, 3);
    core.execute(Command::Done).unwrap();
    let events = core
        .execute(Command::ChooseColor {
            color: Color::Black,
        })
        .unwrap();
    assert_eq!(
        events,
        vec![Event::OpeningFinished {
            chosen: Color::Black
        }]
    );
    assert_eq!(core.board().stone_at(mv(3, 3)), Some(Color::White));
}

#[test]
fn back_on_an_empty_log_is_a_typed_error() {
    let mut core = swap2_core();
    let err = core.execute(Command::Back { n: 1 }).unwrap_err();
    assert_eq!(err, CommandError::OpeningLogEmpty);
}

#[test]
fn done_with_wrong_counts_is_rejected_and_the_round_continues() {
    let mut core = swap2_core();
    play(&mut core, 7, 7);
    play(&mut core, 8, 8);

    // Two stones do not make a round: 2 black + 1 white are required.
    let err = core.execute(Command::Done).unwrap_err();
    assert_eq!(err, CommandError::IllegalMove(PlayError::BadOpeningCounts));

    // The session stays in the phase: the third stone still places,
    // and `done` then succeeds.
    play(&mut core, 9, 9);
    let events = core.execute(Command::Done).unwrap();
    assert_eq!(
        events,
        vec![Event::OpeningPhaseChanged {
            phase: OpeningPhase::FirstChoice
        }]
    );
}

#[test]
fn an_occupied_square_is_rejected_during_the_ceremony() {
    let mut core = swap2_core();
    play(&mut core, 7, 7);

    let err = core
        .execute(Command::Play {
            mv: mv(7, 7),
            color: None,
        })
        .unwrap_err();
    assert_eq!(err, CommandError::IllegalMove(PlayError::Occupied));
    // The failed placement did not enter the log.
    assert_eq!(core.snapshot().opening.unwrap().placements.len(), 1);
}

#[test]
fn the_color_token_overrides_the_default_alternation() {
    let mut core = swap2_core();
    let events = play_color(&mut core, 7, 7, Color::White);
    assert_eq!(
        events,
        vec![Event::StonePlaced {
            mv: mv(7, 7),
            color: Color::White
        }]
    );

    // The default for the next stone follows the placement count, not
    // the placed colors: one stone placed, so white is the default —
    // even though the first stone was white. The Swap2 rule fixes the
    // final counts; the order is free.
    let opening = core.snapshot().opening.unwrap();
    assert_eq!(opening.default_next_color, Some(Color::White));
}

#[test]
fn the_default_order_alternates_black_white_black() {
    let mut core = swap2_core();
    let mut colors = Vec::new();
    for (r, c) in [(7, 7), (8, 8), (9, 9)] {
        play(&mut core, r, c);
        let opening = core.snapshot().opening.unwrap();
        colors.push(opening.placements.last().unwrap().1);
    }
    assert_eq!(colors, vec![Color::Black, Color::White, Color::Black]);
}

#[test]
fn the_color_token_is_rejected_outside_the_ceremony() {
    // A session without Swap2.
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    let err = core
        .execute(Command::Play {
            mv: mv(7, 7),
            color: Some(Color::White),
        })
        .unwrap_err();
    assert_eq!(err, CommandError::ColorOverrideOutsideOpening);

    // And after a finished ceremony.
    let mut core = swap2_core();
    place_first_round(&mut core);
    core.execute(Command::ChooseColor {
        color: Color::Black,
    })
    .unwrap();
    let err = core
        .execute(Command::Play {
            mv: mv(10, 10),
            color: Some(Color::Black),
        })
        .unwrap_err();
    assert_eq!(err, CommandError::ColorOverrideOutsideOpening);
}

#[test]
fn ceremony_commands_outside_the_ceremony_are_typed_errors() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    for cmd in [
        Command::Done,
        Command::More,
        Command::ChooseColor {
            color: Color::Black,
        },
        Command::ChooseColor {
            color: Color::White,
        },
    ] {
        assert_eq!(core.execute(cmd), Err(CommandError::NotInOpening));
    }

    // The same holds after a ceremony finished.
    let mut core = swap2_core();
    place_first_round(&mut core);
    core.execute(Command::ChooseColor {
        color: Color::Black,
    })
    .unwrap();
    assert_eq!(core.execute(Command::Done), Err(CommandError::NotInOpening));
}

#[test]
fn commands_in_the_wrong_phase_are_typed_errors() {
    let mut core = swap2_core();

    // Placing3: choices and `more` do not apply.
    assert_eq!(
        core.execute(Command::ChooseColor {
            color: Color::Black
        })
        .unwrap_err(),
        CommandError::NotAtChoicePhase
    );
    assert_eq!(
        core.execute(Command::More).unwrap_err(),
        CommandError::NotAtFirstChoice
    );

    place_first_round(&mut core);

    // FirstChoice: placements and `done` do not apply.
    assert_eq!(
        core.execute(Command::Play {
            mv: mv(1, 1),
            color: None
        })
        .unwrap_err(),
        CommandError::NotAtPlacementPhase
    );
    assert_eq!(
        core.execute(Command::Done).unwrap_err(),
        CommandError::NotAtPlacementPhase
    );

    place_second_round(&mut core);

    // FinalChoice: `more` no longer exists.
    assert_eq!(
        core.execute(Command::More).unwrap_err(),
        CommandError::NotAtFirstChoice
    );
    assert_eq!(
        core.execute(Command::Done).unwrap_err(),
        CommandError::NotAtPlacementPhase
    );
}

#[test]
fn step_is_rejected_during_the_ceremony() {
    let mut core = swap2_core();
    assert_eq!(
        core.execute(Command::Step).unwrap_err(),
        CommandError::OpeningInProgress
    );
}

#[test]
fn new_restarts_the_ceremony() {
    let mut core = swap2_core();
    play(&mut core, 7, 7);
    play(&mut core, 8, 8);

    let events = core.execute(Command::New).unwrap();
    assert_eq!(events, vec![Event::Reset]);

    // `new` returns to the initial phase: the ceremony.
    let opening = core
        .snapshot()
        .opening
        .expect("a Swap2 session resets into the ceremony");
    assert_eq!(opening.phase, OpeningPhase::Placing3);
    assert!(opening.placements.is_empty());
    assert_eq!(opening.default_next_color, Some(Color::Black));
    assert_eq!(core.board(), Board::new());
}

#[test]
fn new_after_the_ceremony_restarts_the_ceremony() {
    let mut core = swap2_core();
    place_first_round(&mut core);
    core.execute(Command::ChooseColor {
        color: Color::Black,
    })
    .unwrap();
    play(&mut core, 10, 10);

    core.execute(Command::New).unwrap();
    assert!(core.snapshot().opening.is_some());
    assert_eq!(core.board(), Board::new());
}

#[test]
fn the_snapshot_reports_the_ceremony_state() {
    let mut core = swap2_core();
    let opening = core.snapshot().opening.unwrap();
    assert_eq!(opening.phase, OpeningPhase::Placing3);
    assert!(opening.placements.is_empty());
    assert_eq!(opening.default_next_color, Some(Color::Black));

    play(&mut core, 7, 7);
    let opening = core.snapshot().opening.unwrap();
    assert_eq!(opening.placements, vec![(mv(7, 7), Color::Black)]);
    assert_eq!(opening.default_next_color, Some(Color::White));

    play(&mut core, 8, 8);
    play(&mut core, 9, 9);
    core.execute(Command::Done).unwrap();
    let opening = core.snapshot().opening.unwrap();
    assert_eq!(opening.phase, OpeningPhase::FirstChoice);
    // A choice phase has no next placement.
    assert_eq!(opening.default_next_color, None);
}

#[test]
fn the_snapshot_serializes_the_ceremony_state() {
    let mut core = swap2_core();
    play(&mut core, 7, 7);
    play_color(&mut core, 8, 8, Color::Black);
    let snapshot = core.snapshot();
    let json = serde_json::to_string(&snapshot).expect("snapshot must serialize");
    let back: Snapshot = serde_json::from_str(&json).expect("snapshot must deserialize");
    assert_eq!(back, snapshot, "round trip must be lossless: {json}");

    // Without a ceremony the wire format carries no opening field.
    let core = SessionCore::new(SessionInit::new_hotseat());
    let value = serde_json::to_value(core.snapshot()).unwrap();
    assert!(
        value.get("opening").is_none(),
        "ordinary sessions keep the old wire format"
    );
}

#[test]
fn back_after_the_ceremony_navigates_the_tree() {
    let mut core = swap2_core();
    place_first_round(&mut core);
    core.execute(Command::ChooseColor {
        color: Color::Black,
    })
    .unwrap();
    play(&mut core, 10, 10);

    // `back` is tree navigation again.
    let events = core.execute(Command::Back { n: 1 }).unwrap();
    assert_eq!(event_types(&events), ["CursorMoved"]);
    assert_eq!(core.snapshot().ply, 0);
    // The root of the tree is the finished opening board.
    assert_eq!(stone_count(&core.board(), Color::Black), 2);
    assert_eq!(stone_count(&core.board(), Color::White), 1);
}

#[test]
#[should_panic(expected = "swap2 requires an empty opening board")]
fn swap2_with_a_preset_opening_is_a_construction_error() {
    let mut board = Board::new();
    board.play(mv(7, 7)).unwrap();
    let _ = SessionCore::new(SessionInit::new_hotseat_from_opening(&board).with_swap2(true));
}
