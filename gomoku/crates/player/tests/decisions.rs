use std::cell::{Cell, RefCell};
use std::rc::Rc;

use engine::{Board, Move, Status};
use mcts::{EvalRequest, EvalResult, Evaluator, SearchConfig, UniformEvaluator};
use player::{AiPlayer, DecisionError, GuardConfig, GuardOutcome, OwnTssConfig, PlayerOptions};
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

        assert_eq!(decision.mv, expected_move);
        assert!(board.is_legal(decision.mv));
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
    board.play(first.mv).unwrap();
    let second = player.decide(&board, &options, &mut rng).unwrap();

    assert!(board.is_legal(second.mv));
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
        own: OwnTssConfig::baseline(),
        ..PlayerOptions::default()
    };
    let mut rng = StdRng::seed_from_u64(19);
    let mut untouched_rng = StdRng::seed_from_u64(19);

    let decision = player.decide(&board, &options, &mut rng).unwrap();

    assert_eq!(decision.mv, Move::new(7, 7).unwrap());
    assert!(decision.search.is_none());
    assert_eq!(decision.guard.outcome, GuardOutcome::ProvenWin);
    assert_eq!(rng.random::<u64>(), untouched_rng.random::<u64>());
}

#[test]
fn own_tss_config_defaults_and_presets() {
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

#[test]
fn own_proof_bypasses_search_stats_and_rng() {
    let board = vcf_fixture();
    let certified = Move::new(7, 7).unwrap();
    let mut player = AiPlayer::new(Box::new(NeverEvaluate));
    let options = PlayerOptions {
        simulations: 100,
        temperature: 1.0,
        noise_epsilon: 0.25,
        own: OwnTssConfig::baseline(),
        guard: GuardConfig::baseline(),
        ..PlayerOptions::default()
    };
    let mut rng = StdRng::seed_from_u64(8);
    let mut untouched_rng = StdRng::seed_from_u64(8);

    let decision = player.decide(&board, &options, &mut rng).unwrap();

    assert_eq!(decision.mv, certified);
    assert!(decision.search.is_none());
    assert_eq!(decision.guard.outcome, GuardOutcome::ProvenWin);
    assert_eq!(decision.guard.proposal, certified);
    assert_eq!(decision.guard.checked, 0);
    assert!(decision.guard.refuted.is_empty());
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

    assert!(board.is_legal(decision.mv));
    assert!(calls.get() > 0, "disabled probe must really search");
    let stats = decision
        .search
        .expect("ordinary search has real statistics");
    assert_eq!(stats.visits.iter().map(|(_, n)| n).sum::<u32>(), 9);
    assert_eq!(decision.guard.outcome, GuardOutcome::Disabled);
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

    assert!(board.is_legal(decision.mv));
    assert!(
        calls.get() > 0,
        "an exhausted probe is unknown, so search runs"
    );
    assert!(decision.search.is_some());
    assert_eq!(decision.guard.outcome, GuardOutcome::Disabled);
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
    assert_eq!(proven.mv, certified);
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
    assert!(quiet.is_legal(searched.mv));
    assert_ne!(
        searched.mv, certified,
        "no stale proof move from the earlier decision"
    );
    assert_eq!(searched.guard.outcome, GuardOutcome::Disabled);
}
