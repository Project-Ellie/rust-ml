use std::cell::{Cell, RefCell};
use std::rc::Rc;

use engine::{Board, Move, Status};
use mcts::{EvalRequest, EvalResult, Evaluator, SearchConfig, UniformEvaluator};
use player::{
    AiPlayer, DecisionAction, DecisionError, GuardConfig, GuardOutcome, OwnTssConfig, PlayerOptions,
};

fn played(action: DecisionAction) -> Move {
    match action {
        DecisionAction::Play(mv) => mv,
        DecisionAction::Resign(_) => panic!("expected play"),
    }
}
use rand::{RngExt, SeedableRng, rngs::StdRng};

fn winning_position() -> Board {
    let mut board = Board::new();
    for (row, col) in [
        (7, 3),
        (0, 0),
        (7, 4),
        (0, 2),
        (7, 5),
        (0, 4),
        (7, 6),
        (0, 6),
    ] {
        board.play(Move::new(row, col).unwrap()).unwrap();
    }
    board
}

struct NeverEvaluate;

impl Evaluator for NeverEvaluate {
    fn evaluate(&mut self, _: EvalRequest) -> EvalResult {
        panic!("terminal boards must not reach the evaluator");
    }
}

/// Non-uniform evaluator giving one favorite move the highest prior.
struct BiasedEvaluator {
    favorite: Move,
}

impl Evaluator for BiasedEvaluator {
    fn evaluate(&mut self, _: EvalRequest) -> EvalResult {
        let mut policy = [0.0f32; 225];
        policy[self.favorite.index()] = 4.0;
        EvalResult { policy, value: 0.0 }
    }
}

#[test]
fn zero_temperature_tied_visits_follow_the_highest_root_prior() {
    // One Black stone at h8; White to move. Zero simulations leave every
    // root edge at 0 visits, so the tie must break by root prior, not by
    // lowest move index (a1). The favorite is deliberately neither the
    // lowest-index legal move nor a tactical win.
    let mut board = Board::new();
    board.play(Move::new(7, 7).unwrap()).unwrap();
    let favorite = Move::new(5, 5).unwrap();
    let mut player = AiPlayer::new(Box::new(BiasedEvaluator { favorite }));
    let options = PlayerOptions {
        simulations: 0,
        temperature: 0.0,
        ..PlayerOptions::default()
    };
    let mut rng = StdRng::seed_from_u64(23);
    let mut untouched_rng = StdRng::seed_from_u64(23);

    let decision = player.decide(&board, &options, &mut rng).unwrap();

    assert_eq!(decision.action, DecisionAction::Play(favorite));
    assert_eq!(rng.random::<u64>(), untouched_rng.random::<u64>());
}

#[test]
fn terminal_board_returns_game_over_without_evaluation_or_rng() {
    let mut board = winning_position();
    board.play(Move::new(7, 7).unwrap()).unwrap();
    assert_ne!(board.status(), Status::Ongoing);
    let mut player = AiPlayer::new(Box::new(NeverEvaluate));
    let mut rng = StdRng::seed_from_u64(7);
    let mut untouched_rng = StdRng::seed_from_u64(7);

    let result = player.decide(&board, &PlayerOptions::default(), &mut rng);

    assert_eq!(result.unwrap_err(), DecisionError::GameOver);
    assert_eq!(rng.random::<u64>(), untouched_rng.random::<u64>());
}

#[test]
fn ordinary_decisions_match_manual_search_stats_and_seeded_selection() {
    let mut board = Board::new();
    board.play(Move::new(7, 7).unwrap()).unwrap();
    for (simulations, temperature, noise_epsilon) in [
        (17, 0.0, 0.0),
        (19, 1.0, 0.0),
        (23, 0.7, 0.25),
        (0, 1.0, 0.25),
    ] {
        let options = PlayerOptions {
            simulations,
            c_puct: 1.7,
            temperature,
            noise_epsilon,
            noise_alpha: 0.3,
            ..PlayerOptions::default()
        };
        let mut manual_rng = StdRng::seed_from_u64(42);
        let mut player_rng = StdRng::seed_from_u64(42);
        let mut evaluator = UniformEvaluator::new(0.6);
        let mcts::SearchOutcome { mut tree, root } = mcts::search(
            &board,
            &mut evaluator,
            &SearchConfig {
                simulations,
                c_puct: options.c_puct,
            },
        );
        if noise_epsilon > 0.0 {
            mcts::add_dirichlet_noise(
                &mut tree,
                root,
                noise_epsilon,
                options.noise_alpha,
                &mut manual_rng,
            );
        }
        let visits = mcts::visit_distribution(&tree, root);
        let expected_move = mcts::select_move(&visits, temperature, &mut manual_rng).unwrap();
        let mut expected_visits = visits;
        expected_visits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.index().cmp(&b.0.index())));
        let edges = tree.node(root).edges();
        let mut expected_prior: Vec<_> = edges.iter().map(|e| (e.mv, e.prior)).collect();
        expected_prior.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap()
                .then_with(|| a.0.index().cmp(&b.0.index()))
        });
        let expected_value = if simulations == 0 {
            0.0
        } else {
            edges.iter().map(|e| e.w).sum::<f32>() / simulations as f32
        };
        let mut player = AiPlayer::new(Box::new(UniformEvaluator::new(0.6)));

        let decision = player.decide(&board, &options, &mut player_rng).unwrap();

        assert_eq!(decision.action, DecisionAction::Play(expected_move));
        assert!(decision.resignation.is_none());
        assert!(board.is_legal(played(decision.action)));
        let stats = decision
            .search
            .expect("ordinary search has real statistics");
        assert_eq!(stats.side_to_move, board.to_move());
        assert_eq!(stats.visits, expected_visits);
        assert_eq!(
            stats.visits.iter().map(|(_, n)| n).sum::<u32>(),
            simulations
        );
        assert_eq!(stats.prior, expected_prior);
        assert_eq!(stats.value, expected_value);
        assert_eq!(player_rng.random::<u64>(), manual_rng.random::<u64>());
    }
}

#[test]
fn unknown_resignation_preserves_exact_decision_and_rng_with_noise_sampling_and_guard() {
    let mut board = Board::new();
    board.play(Move::new(7, 7).unwrap()).unwrap();
    let original = board.clone();
    for guard in [GuardConfig::disabled(), GuardConfig::baseline()] {
        for simulations in [0, 3] {
            let disabled = PlayerOptions {
                simulations,
                temperature: 0.7,
                noise_epsilon: 0.25,
                guard,
                ..PlayerOptions::default()
            };
            let enabled = PlayerOptions {
                resign: true,
                ..disabled
            };
            let mut a = AiPlayer::new(Box::new(UniformEvaluator::new(0.3)));
            let mut b = AiPlayer::new(Box::new(UniformEvaluator::new(0.3)));
            let mut rng_a = StdRng::seed_from_u64(19);
            let mut rng_b = StdRng::seed_from_u64(19);
            assert_eq!(rng_a.random::<u64>(), rng_b.random::<u64>());
            let ordinary = a.decide(&board, &disabled, &mut rng_a).unwrap();
            let fallback = b.decide(&board, &enabled, &mut rng_b).unwrap();
            assert_eq!(fallback, ordinary);
            assert!(ordinary.resignation.is_none());
            assert!(fallback.resignation.is_none());
            assert!(matches!(fallback.action, DecisionAction::Play(_)));
            assert_eq!(rng_a.random::<u64>(), rng_b.random::<u64>());
            assert_eq!(board, original);
        }
    }
}

struct StatefulEvaluator {
    calls: u32,
    observed: Rc<RefCell<Vec<u32>>>,
}

impl Evaluator for StatefulEvaluator {
    fn evaluate(&mut self, _: EvalRequest) -> EvalResult {
        self.calls += 1;
        self.observed.borrow_mut().push(self.calls);
        EvalResult {
            policy: [0.0; 225],
            value: 0.1,
        }
    }
}

#[test]
fn evaluator_instance_persists_across_two_decisions() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let mut player = AiPlayer::new(Box::new(StatefulEvaluator {
        calls: 0,
        observed: Rc::clone(&observed),
    }));
    let options = PlayerOptions {
        simulations: 1,
        ..PlayerOptions::default()
    };
    let mut board = Board::new();
    let mut rng = StdRng::seed_from_u64(9);

    let first = player.decide(&board, &options, &mut rng).unwrap();
    assert_eq!(*observed.borrow(), [1, 2]);
    board.play(played(first.action)).unwrap();
    let second = player.decide(&board, &options, &mut rng).unwrap();

    assert!(board.is_legal(played(second.action)));
    assert_eq!(*observed.borrow(), [1, 2, 3, 4]);
}

#[test]
fn white_own_proof_bypasses_search_and_certifies_absolute_color() {
    // White-to-move mirror of the Black VCF fixture: the same attacking
    // shape with colors swapped, plus one inert Black stone at (14, 14) so
    // Black has exactly one more stone — a legal alternating position with
    // White to move. Pins absolute-color correctness of the own probe: the
    // certificate must name White, not just "the side that was searched".
    let mut board = Board::new();
    for (row, col) in [
        (7, 3),
        (7, 4),
        (0, 0),
        (7, 5),
        (0, 2),
        (7, 6),
        (0, 4),
        (6, 6),
        (0, 6),
        (8, 8),
        (14, 14),
    ] {
        board.play(Move::new(row, col).unwrap()).unwrap();
    }
    assert_eq!(board.to_move(), engine::Color::White);
    let proof = engine::prove_forced_win(&board, board.to_move(), OwnTssConfig::baseline().budget)
        .expect("White has the mirrored VCF");
    assert_eq!(proof.winner, engine::Color::White);
    assert_eq!(proof.line[0], Move::new(7, 7).unwrap());

    let mut player = AiPlayer::new(Box::new(NeverEvaluate));
    let options = PlayerOptions {
        simulations: 100,
        resign: true,
        own: OwnTssConfig::baseline(),
        ..PlayerOptions::default()
    };
    let mut rng = StdRng::seed_from_u64(19);
    let mut untouched_rng = StdRng::seed_from_u64(19);

    let decision = player.decide(&board, &options, &mut rng).unwrap();

    assert_eq!(
        decision.action,
        DecisionAction::Play(Move::new(7, 7).unwrap())
    );
    assert!(decision.search.is_none());
    assert!(decision.resignation.is_none());
    assert_eq!(
        decision.guard.as_ref().unwrap().outcome,
        GuardOutcome::ProvenWin
    );
    assert_eq!(rng.random::<u64>(), untouched_rng.random::<u64>());
}

#[test]
fn own_tss_config_defaults_and_presets() {
    assert!(!PlayerOptions::default().resign);
    assert_eq!(PlayerOptions::default().own, OwnTssConfig::disabled());
    assert_eq!(
        OwnTssConfig::baseline(),
        OwnTssConfig {
            enabled: true,
            budget: engine::SearchBudget {
                max_nodes: 10_000,
                max_depth: 30,
            },
        }
    );
    assert_eq!(
        OwnTssConfig::disabled(),
        OwnTssConfig {
            enabled: false,
            ..OwnTssConfig::baseline()
        }
    );
    assert_eq!(OwnTssConfig::default(), OwnTssConfig::disabled());
    assert_eq!(GuardConfig::default(), GuardConfig::disabled());
    assert!(!GuardConfig::default().enabled);
}

/// The win-in-3 VCF fixture from the engine prover tests: Black to move
/// forces a win starting with (7, 7).
fn vcf_fixture() -> Board {
    let mut board = Board::new();
    for (row, col) in [
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
    ] {
        board.play(Move::new(row, col).unwrap()).unwrap();
    }
    board
}

fn nonminimal_vcf_fixture() -> Board {
    let mut board = Board::new();
    for (row, col) in [
        (7, 4),
        (0, 0),
        (7, 5),
        (0, 2),
        (7, 6),
        (0, 4),
        (6, 6),
        (0, 6),
        (8, 8),
        (0, 8),
        (8, 11),
        (7, 11),
        (9, 11),
        (11, 7),
        (10, 11),
        (14, 0),
        (11, 8),
        (14, 2),
        (11, 9),
        (14, 4),
        (11, 10),
        (14, 6),
    ] {
        board.play(Move::new(row, col).unwrap()).unwrap();
    }
    board
}

#[test]
fn own_proof_plays_the_shortest_certified_line() {
    let board = nonminimal_vcf_fixture();
    let budget = engine::SearchBudget {
        max_nodes: 50_000,
        max_depth: 9,
    };
    let ordinary = engine::prove_forced_win(&board, board.to_move(), budget).unwrap();
    let shortest = engine::prove_shortest_forced_win(&board, board.to_move(), budget).unwrap();
    assert!(shortest.line.len() < ordinary.line.len());
    assert_ne!(shortest.line[0], ordinary.line[0]);

    let mut player = AiPlayer::new(Box::new(NeverEvaluate));
    let options = PlayerOptions {
        own: OwnTssConfig {
            enabled: true,
            budget,
        },
        ..PlayerOptions::default()
    };

    let decision = player
        .decide(&board, &options, &mut StdRng::seed_from_u64(8))
        .unwrap();

    assert_eq!(decision.action, DecisionAction::Play(shortest.line[0]));
    assert!(decision.search.is_none());
}

#[test]
fn own_proof_bypasses_search_stats_and_rng() {
    let board = vcf_fixture();
    let certified = Move::new(7, 7).unwrap();
    let mut player = AiPlayer::new(Box::new(NeverEvaluate));
    let options = PlayerOptions {
        simulations: 100,
        temperature: 1.0,
        noise_epsilon: 0.25,
        resign: true,
        own: OwnTssConfig::baseline(),
        guard: GuardConfig::baseline(),
        ..PlayerOptions::default()
    };
    let mut rng = StdRng::seed_from_u64(8);
    let mut untouched_rng = StdRng::seed_from_u64(8);

    let decision = player.decide(&board, &options, &mut rng).unwrap();

    assert_eq!(decision.action, DecisionAction::Play(certified));
    assert!(decision.search.is_none());
    assert!(decision.resignation.is_none());
    let guard = decision.guard.as_ref().expect("play has diagnostics");
    assert_eq!(guard.outcome, GuardOutcome::ProvenWin);
    assert_eq!(guard.proposal, certified);
    assert_eq!(guard.checked, 0);
    assert!(guard.refuted.is_empty());
    assert_eq!(rng.random::<u64>(), untouched_rng.random::<u64>());
}

struct CountingEvaluator {
    calls: Rc<Cell<u32>>,
}

impl Evaluator for CountingEvaluator {
    fn evaluate(&mut self, request: EvalRequest) -> EvalResult {
        self.calls.set(self.calls.get() + 1);
        UniformEvaluator::new(0.0).evaluate(request)
    }
}

#[test]
fn resignation_evidence_adds_no_evaluator_calls_to_the_real_search() {
    let mv = |r, c| Move::new(r, c).unwrap();
    let losing = Board::from_position(
        &[mv(2, 0), mv(2, 2), mv(2, 4), mv(2, 6)],
        &[mv(7, 3), mv(7, 4), mv(7, 5), mv(7, 6)],
        engine::Color::Black,
    )
    .unwrap();
    for (board, should_resign) in [(losing, true), (Board::new(), false)] {
        let options = PlayerOptions {
            simulations: 7,
            resign: true,
            temperature: 0.7,
            noise_epsilon: 0.25,
            guard: GuardConfig::baseline(),
            ..PlayerOptions::default()
        };
        let expected_calls = Rc::new(Cell::new(0));
        let mut evaluator = CountingEvaluator {
            calls: Rc::clone(&expected_calls),
        };
        let _ = mcts::search(
            &board,
            &mut evaluator,
            &SearchConfig {
                simulations: options.simulations,
                c_puct: options.c_puct,
            },
        );
        assert!(expected_calls.get() > 0);
        let actual_calls = Rc::new(Cell::new(0));
        let mut player = AiPlayer::new(Box::new(CountingEvaluator {
            calls: Rc::clone(&actual_calls),
        }));
        let decision = player
            .decide(&board, &options, &mut StdRng::seed_from_u64(31))
            .unwrap();
        assert_eq!(actual_calls.get(), expected_calls.get());
        assert_eq!(decision.resignation.is_some(), should_resign);
        assert_eq!(
            matches!(decision.action, DecisionAction::Resign(_)),
            should_resign
        );
    }
}

#[test]
fn disabled_own_probe_searches_vcf_position() {
    let board = vcf_fixture();
    let calls = Rc::new(Cell::new(0));
    let mut player = AiPlayer::new(Box::new(CountingEvaluator {
        calls: Rc::clone(&calls),
    }));
    let options = PlayerOptions {
        simulations: 9,
        own: OwnTssConfig::disabled(),
        ..PlayerOptions::default()
    };
    let mut rng = StdRng::seed_from_u64(11);

    let decision = player.decide(&board, &options, &mut rng).unwrap();

    assert!(board.is_legal(played(decision.action)));
    assert!(calls.get() > 0, "disabled probe must really search");
    let stats = decision
        .search
        .expect("ordinary search has real statistics");
    assert_eq!(stats.visits.iter().map(|(_, n)| n).sum::<u32>(), 9);
    assert_eq!(
        decision.guard.as_ref().unwrap().outcome,
        GuardOutcome::Disabled
    );
}

#[test]
fn zero_budget_own_probe_falls_through_to_search() {
    let board = vcf_fixture();
    let calls = Rc::new(Cell::new(0));
    let mut player = AiPlayer::new(Box::new(CountingEvaluator {
        calls: Rc::clone(&calls),
    }));
    let options = PlayerOptions {
        simulations: 5,
        own: OwnTssConfig {
            enabled: true,
            budget: engine::SearchBudget {
                max_nodes: 0,
                max_depth: 30,
            },
        },
        ..PlayerOptions::default()
    };
    let mut rng = StdRng::seed_from_u64(13);

    let decision = player.decide(&board, &options, &mut rng).unwrap();

    assert!(board.is_legal(played(decision.action)));
    assert!(
        calls.get() > 0,
        "an exhausted probe is unknown, so search runs"
    );
    assert!(decision.search.is_some());
    assert_eq!(
        decision.guard.as_ref().unwrap().outcome,
        GuardOutcome::Disabled
    );
}

#[test]
fn proven_decision_does_not_short_circuit_a_later_quiet_root() {
    // Structural invariant (code-review-verifiable): AiPlayer holds no state
    // besides the evaluator, so there is nothing that could cache a proof.
    // This test pins the dynamic half: after a proven decision on the VCF
    // fixture, a separate quiet board is really searched by the same player.
    let vcf = vcf_fixture();
    let certified = Move::new(7, 7).unwrap();
    let calls = Rc::new(Cell::new(0));
    let mut player = AiPlayer::new(Box::new(CountingEvaluator {
        calls: Rc::clone(&calls),
    }));
    let options = PlayerOptions {
        simulations: 5,
        own: OwnTssConfig::baseline(),
        ..PlayerOptions::default()
    };
    let mut rng = StdRng::seed_from_u64(17);

    let proven = player.decide(&vcf, &options, &mut rng).unwrap();
    assert_eq!(proven.action, DecisionAction::Play(certified));
    assert!(proven.search.is_none());
    assert_eq!(calls.get(), 0, "the proof path must not evaluate");

    // A separate, known-quiet board: one Black stone at (7, 7), White to
    // move. No forced win exists within any budget, and the certified move
    // from the previous decision is not even legal here.
    let mut quiet = Board::new();
    quiet.play(certified).unwrap();
    assert!(!quiet.is_legal(certified));

    let searched = player.decide(&quiet, &options, &mut rng).unwrap();

    assert!(calls.get() > 0, "the quiet root must really be searched");
    let stats = searched.search.expect("the quiet root has real statistics");
    assert_eq!(stats.side_to_move, quiet.to_move());
    assert_eq!(stats.visits.iter().map(|(_, n)| n).sum::<u32>(), 5);
    assert!(quiet.is_legal(played(searched.action)));
    assert_ne!(
        searched.action,
        DecisionAction::Play(certified),
        "no stale proof move from the earlier decision"
    );
    assert_eq!(
        searched.guard.as_ref().unwrap().outcome,
        GuardOutcome::Disabled
    );
}
