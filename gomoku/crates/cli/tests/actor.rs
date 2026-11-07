//! Headless tests for AI seats and `SearchInfo`.
//!
//! These exercises are the Task-3 capstone: releasing a seat to the AI,
//! stepping it, and inspecting the stored search record.

use cli::actor::{AiConfig, EvaluatorKind, Occupant, OccupantKind};
use cli::command::{Command, CommandError, Event, SeatSel};
use cli::session::{SessionCore, SessionInit};
use engine::{Board, Color, Status};
use testkit::{ai_config_tactics, mv, play};

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
        black: Occupant::Ai(ai_config_tactics()),
        white: Occupant::Human,
        default_config: ai_config_tactics(),
        black_sims: None,
        white_sims: None,
        seed: Some(1),
        opening: board,
        swap2: false,
        puzzle: None,
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
            ..ai_config_tactics()
        }),
        white: Occupant::Human,
        default_config: ai_config_tactics(),
        black_sims: None,
        white_sims: None,
        seed: Some(1),
        opening: board,
        swap2: false,
        puzzle: None,
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
        ..ai_config_tactics()
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
fn persistent_players_match_seeded_legacy_search_through_opening_schedule() {
    use cli::actor::SearchInfo;
    use rand::{SeedableRng, rngs::StdRng};

    let config = AiConfig {
        simulations: 3,
        ..ai_config_tactics()
    };
    let mut core = SessionCore::new(SessionInit::ai_vs_ai(config.clone(), 42));
    let mut rng = StdRng::seed_from_u64(42);
    let mut board = Board::new();
    for ply in 0..11 {
        let mut effective = config.clone();
        if ply < 10 {
            effective.temperature = 1.0;
            effective.noise_epsilon = 0.25;
        }
        // Reproduce the pre-player CLI path, including a fresh evaluator.
        let mut evaluator = mcts::TacticsEvaluator::new();
        let mut outcome = mcts::search(
            &board,
            &mut evaluator,
            &mcts::SearchConfig {
                simulations: effective.simulations,
                c_puct: effective.c_puct,
            },
        );
        if effective.noise_epsilon > 0.0 {
            mcts::add_dirichlet_noise(
                &mut outcome.tree,
                outcome.root,
                effective.noise_epsilon,
                0.3,
                &mut rng,
            );
        }
        let expected_info = SearchInfo::from_search(&outcome, &board, &effective);
        let expected_move = mcts::select_move(
            &mcts::visit_distribution(&outcome.tree, outcome.root),
            effective.temperature,
            &mut rng,
        )
        .unwrap();

        assert_eq!(
            core.execute(Command::Step).unwrap(),
            vec![Event::Moved {
                mv: expected_move,
                by: OccupantKind::Ai
            }],
            "same seeded move at ply {ply}"
        );
        assert_eq!(
            core.search_info(),
            Some(&expected_info),
            "stats at ply {ply}"
        );
        board.play(expected_move).unwrap();
        assert_eq!(core.board(), board);
    }
}

#[test]
fn different_sims_per_seat_yield_different_total_visits() {
    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Human,
        white: Occupant::Human,
        default_config: AiConfig {
            simulations: 10, // should be overwritten on both seats
            ..ai_config_tactics()
        },
        black_sims: Some(400),
        white_sims: Some(50),
        seed: Some(1),
        opening: Board::new(),
        swap2: false,
        puzzle: None,
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
        core.execute(Command::Play { mv: mv(r, c) }).unwrap();
    }
    assert_eq!(core.board().status(), Status::Won(Color::Black));
    let err = core.execute(Command::Step).unwrap_err();
    assert_eq!(err, CommandError::GameOver);
}

#[test]
fn baseline_ai_plays_proven_vcf_without_search_info() {
    let mut board = Board::new();
    let script = [
        (7, 4),
        (7, 3),
        (7, 5),
        (0, 0),
        (7, 6),
        (0, 2),
        (6, 6),
        (0, 4),
        (8, 8),
        (0, 6),
    ];
    for &(r, c) in &script {
        board.play(mv(r, c)).unwrap();
    }
    assert_eq!(board.to_move(), Color::Black);

    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Ai(AiConfig {
            evaluator: EvaluatorKind::Baseline,
            simulations: 50,
            c_puct: 1.5,
            temperature: 0.0,
            noise_epsilon: 0.0,
            explicit_selection: true,
            resign: false,
        }),
        white: Occupant::Human,
        default_config: AiConfig::default(),
        black_sims: None,
        white_sims: None,
        seed: Some(1),
        opening: board,
        swap2: false,
        puzzle: None,
    });

    let events = core.execute(Command::Step).unwrap();
    let moved = events
        .iter()
        .find_map(|e| match e {
            Event::Moved { mv, .. } => Some(*mv),
            _ => None,
        })
        .expect("AI should move.");
    assert_eq!(
        moved,
        mv(7, 7),
        "proven VCF must play the forcing first move"
    );
    assert!(
        core.search_info().is_none(),
        "proven win must not store search info at the fresh cursor"
    );
}

#[test]
fn baseline_ai_falls_back_to_search_info_on_quiet_position() {
    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Ai(AiConfig {
            evaluator: EvaluatorKind::Baseline,
            simulations: 50,
            c_puct: 1.5,
            temperature: 0.0,
            noise_epsilon: 0.0,
            explicit_selection: true,
            resign: false,
        }),
        white: Occupant::Human,
        default_config: AiConfig::default(),
        black_sims: None,
        white_sims: None,
        seed: Some(1),
        opening: Board::new(),
        swap2: false,
        puzzle: None,
    });

    let events = core.execute(Command::Step).unwrap();
    assert!(
        events.iter().any(|e| matches! {
            e, Event::Moved { by: OccupantKind::Ai, ..}
        }),
        "Step with an AI seat must emit Move{{by: Ai}}, got {events:?}"
    );

    let info = core
        .search_info()
        .expect("searched position must store info");
    assert_eq!(info.config.simulations, 50);
    assert_eq!(
        info.visits.iter().map(|(_, n)| n).sum::<u32>(),
        50,
        "total root visits must match simulations"
    );
}

#[test]
fn search_info_prior_and_visits_are_sorted_descending() {
    let mut core = SessionCore::new(SessionInit {
        white: Occupant::Ai(ai_config_tactics()),
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
