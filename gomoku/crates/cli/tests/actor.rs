//! Headless tests for AI seats and `SearchInfo`.
//!
//! These exercises are the Task-3 capstone: releasing a seat to the AI,
//! stepping it, and inspecting the stored search record.

use cli::actor::{AiConfig, Occupant, OccupantKind};
use cli::command::{Command, CommandError, Event, SeatSel};
use cli::session::{SessionCore, SessionInit};
use engine::{Board, Color, Status};

use testkit::{fast_tactics_config, mv, play};

#[test]
fn tactics_ai_takes_win_in_one() {
    let mut board = Board::new();
    let script = [
        (7, 3),
        (0, 0),
        (7, 4),
        (0, 1),
        (7, 5),
        (0, 2),
        (7, 6),
        (0, 3),
    ];
    for &(r, c) in &script {
        board.play(mv(r, c)).unwrap()
    }
    assert_eq!(board.to_move(), Color::Black);
    let wins = engine::immediate_wins(&board, Color::Black);
    assert!(!wins.is_empty(), "There is an immediate win for black.");

    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Ai(fast_tactics_config()),
        white: Occupant::Human,
        default_config: fast_tactics_config(),
        black_sims: None,
        white_sims: None,
        seed: Some(1),
        opening: board,
        swap2: false,
    });

    let events = core.execute(Command::Step).unwrap();
    assert!(
        events.iter().any(|e| matches! {
            e, Event::Moved { by: OccupantKind::Ai, ..}
        }),
        "Step with an AI seat must emit Move{{by: Ai}}, got {events:?}"
    );

    let info = core.search_info().expect("Search info must be stored");
    assert!(
        info.value > 0.0,
        "Winning side-to-move must hava positive root value, got {}",
        info.value
    );
    let best = info.visits[0].0;
    assert!(
        wins.contains(best),
        "There is no win for a best move. Got {best:?}"
    );
}

#[test]
fn tactics_ai_blocks_opponent_open_four() {
    // White has an open four on row 9; Black to move must block.
    // 300 simulations are used so that the two blocking moves accumulate
    // enough extra visits (via their much higher prior) to beat the
    // temperature-0 tie-breaker.
    let board = engine::reference::board_from_ascii(
        "
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . O O O O . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        X . X . X . X . . . . . . . .
        ",
    );
    assert_eq!(board.to_move(), Color::Black);
    let blocks = engine::forced_blocks(&board);
    assert!(!blocks.is_empty(), "There is an immediate block for black.");

    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Ai(AiConfig {
            simulations: 300,
            ..fast_tactics_config()
        }),
        white: Occupant::Human,
        default_config: fast_tactics_config(),
        black_sims: None,
        white_sims: None,
        seed: Some(1),
        opening: board,
        swap2: false,
    });

    let events = core.execute(Command::Step).unwrap();
    let moved = events
        .iter()
        .find_map(|e| match e {
            Event::Moved { mv, .. } => Some(*mv),
            _ => None,
        })
        .expect("AI should move.");
    assert!(blocks.contains(moved), "AI should block the open four");
}

#[test]
fn same_seed_same_events_and_search_info() {
    let config = AiConfig {
        temperature: 1.0,
        noise_epsilon: 0.25,
        ..fast_tactics_config()
    };
    let init = SessionInit {
        white: Occupant::Ai(config.clone()),
        default_config: config,
        seed: Some(42),
        ..SessionInit::new_hotseat()
    };
    let mut a = SessionCore::new(init.clone());
    let mut b = SessionCore::new(init);

    play(&mut a, 7, 7);
    play(&mut b, 7, 7);

    let ev_a = a.execute(Command::Step).unwrap();
    let ev_b = b.execute(Command::Step).unwrap();
    assert_eq!(ev_a, ev_b);

    assert_eq!(a.search_info().cloned(), b.search_info().cloned());
}

#[test]
fn different_sims_per_seat_yield_different_total_visits() {
    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Human,
        white: Occupant::Human,
        default_config: AiConfig {
            simulations: 10, // should be overwritten on both seats
            ..fast_tactics_config()
        },
        black_sims: Some(400),
        white_sims: Some(50),
        seed: Some(1),
        opening: Board::new(),
        swap2: false,
    });

    core.execute(Command::Release {
        seat: SeatSel::Both,
    })
    .unwrap();

    let events = core.execute(Command::Step).unwrap();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Moved {
                by: OccupantKind::Ai,
                ..
            }
        )),
        "Black AI should move, got {events:?}"
    );
    let black_info = core.search_info().expect("Search info after black step");
    assert_eq!(black_info.config.simulations, 400);
    assert_eq!(
        black_info.visits.iter().map(|(_, n)| n).sum::<u32>(),
        400,
        "Total root visits must match black simulations"
    );

    core.execute(Command::Step).unwrap();
    let white_info = core.search_info().expect("Search info after white step");
    assert_eq!(white_info.config.simulations, 50);
    assert_eq!(
        white_info.visits.iter().map(|(_, n)| n).sum::<u32>(),
        50,
        "Total root visits must match white simulations"
    );
}

#[test]
fn step_on_human_seat_turn_is_typed_error() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    let err = core.execute(Command::Step).unwrap_err();
    assert_eq!(err, CommandError::StepRequiresAi { seat: Color::Black });
}

#[test]
fn step_after_game_over_is_error() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    let script = [
        (7, 3),
        (0, 0),
        (7, 4),
        (0, 1),
        (7, 5),
        (0, 2),
        (7, 6),
        (0, 3),
        (7, 7),
    ];
    for &(r, c) in &script {
        core.execute(Command::Play {
            mv: mv(r, c),
            color: None,
        })
        .unwrap();
    }
    assert_eq!(core.board().status(), Status::Won(Color::Black));
    let err = core.execute(Command::Step).unwrap_err();
    assert_eq!(err, CommandError::GameOver);
}

#[test]
fn search_info_prior_and_visits_are_sorted_descending() {
    let mut core = SessionCore::new(SessionInit {
        white: Occupant::Ai(fast_tactics_config()),
        seed: Some(7),
        ..SessionInit::new_hotseat()
    });

    play(&mut core, 7, 7);
    core.execute(Command::Step).unwrap();

    let info = core.search_info().expect("search info stored.");
    assert!(!info.visits.is_empty());
    assert!(!info.prior.is_empty());

    let visits_sorted = info.visits.windows(2).all(|w| w[0].1 >= w[1].1);
    assert!(visits_sorted, "visits must be sorted descending");
    let prior_sorted = info.prior.windows(2).all(|w| w[0].1 >= w[1].1);
    assert!(prior_sorted, "prior must be sorted descending");
}
