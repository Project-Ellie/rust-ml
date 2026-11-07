use engine::{Board, Color, Move, SearchBudget, Status};
use mcts::{EvalRequest, EvalResult, Evaluator, UniformEvaluator};
use player::{AiPlayer, DecisionAction, GuardConfig, GuardOutcome, OwnTssConfig, PlayerOptions};

fn played(action: DecisionAction) -> Move {
    match action {
        DecisionAction::Play(mv) => mv,
        DecisionAction::Resign(_) => panic!("expected play"),
    }
}
use rand::{RngExt, SeedableRng, rngs::StdRng};

fn mv(row: u8, col: u8) -> Move {
    Move::new(row, col).unwrap()
}

fn position(black: &[(u8, u8)], white: &[(u8, u8)]) -> Board {
    Board::from_position(
        &black.iter().map(|&(r, c)| mv(r, c)).collect::<Vec<_>>(),
        &white.iter().map(|&(r, c)| mv(r, c)).collect::<Vec<_>>(),
        Color::Black,
    )
    .unwrap()
}

// White's four has one winning endpoint; Black must block (7, 7).
fn single_threat() -> Board {
    position(
        &[(7, 2), (2, 0), (2, 2), (2, 4)],
        &[(7, 3), (7, 4), (7, 5), (7, 6)],
    )
}

struct Prefer(Move);
impl Evaluator for Prefer {
    fn evaluate(&mut self, _: EvalRequest) -> EvalResult {
        let mut policy = [0.0; 225];
        policy[self.0.index()] = 10.0;
        EvalResult { policy, value: 0.0 }
    }
}

/// Two-peak bias: `top` takes the highest prior (the zero-temperature
/// proposal), `next` the second-highest (the guard's first ranked
/// alternative after the proposal).
struct PreferPair {
    top: Move,
    next: Move,
}
impl Evaluator for PreferPair {
    fn evaluate(&mut self, _: EvalRequest) -> EvalResult {
        let mut policy = [0.0; 225];
        policy[self.next.index()] = 5.0;
        policy[self.top.index()] = 10.0;
        EvalResult { policy, value: 0.0 }
    }
}

// Guard fixtures may contain own immediate wins; the own probe stays
// disabled (via `PlayerOptions::default`) so these tests isolate the guard.
fn options(guard: GuardConfig) -> PlayerOptions {
    PlayerOptions {
        simulations: 0,
        guard,
        ..PlayerOptions::default()
    }
}

#[test]
fn baseline_guard_is_opt_in_with_fixed_experiment_work_bounds() {
    assert_eq!(PlayerOptions::default().guard, GuardConfig::disabled());
    let baseline = GuardConfig::baseline();
    assert!(baseline.enabled);
    assert_eq!(baseline.max_candidates, 5);
    assert_eq!(
        baseline.budget,
        SearchBudget {
            max_nodes: 1_000,
            max_depth: 10
        }
    );
}

#[test]
fn real_proof_refutes_proposal_and_guard_finds_unvisited_block_without_mutating_board_or_stats() {
    let board = single_threat();
    let original = board.clone();
    let block = mv(7, 7);
    let mut disabled = AiPlayer::new(Box::new(PreferPair {
        top: mv(0, 0),
        next: block,
    }));
    let mut enabled = AiPlayer::new(Box::new(PreferPair {
        top: mv(0, 0),
        next: block,
    }));
    let initial = disabled
        .decide(
            &board,
            &options(GuardConfig::disabled()),
            &mut StdRng::seed_from_u64(1),
        )
        .unwrap();
    assert_eq!(initial.action, DecisionAction::Play(mv(0, 0)));
    let mut hypothetical = board.clone();
    hypothetical.play(played(initial.action)).unwrap();
    let opponent = hypothetical.to_move();
    let proof =
        engine::prove_forced_win(&hypothetical, opponent, GuardConfig::baseline().budget).unwrap();
    assert_eq!(proof.winner, Color::White);
    assert!(!proof.line.is_empty());
    assert!(hypothetical.is_legal(proof.line[0]));
    assert!(engine::verify_line(&hypothetical, &proof));

    let guarded = enabled
        .decide(
            &board,
            &options(GuardConfig::baseline()),
            &mut StdRng::seed_from_u64(1),
        )
        .unwrap();
    assert_eq!(guarded.action, DecisionAction::Play(block));
    let guard = guarded.guard.as_ref().expect("play has diagnostics");
    assert_eq!(guard.proposal, played(initial.action));
    assert_eq!(guard.refuted, vec![played(initial.action)]);
    assert_eq!(guard.checked, 2);
    assert_eq!(guard.outcome, GuardOutcome::Unrefuted);
    assert_eq!(guarded.search, initial.search);
    assert_eq!(board, original);
    assert_eq!(board.zobrist(), original.zobrist());
    assert_eq!(
        initial.guard.as_ref().unwrap().outcome,
        GuardOutcome::Disabled
    );
}

#[test]
fn two_opponent_winning_threats_require_our_immediate_winning_alternative() {
    let board = position(
        &[(7, 2), (7, 3), (7, 4), (7, 5)],
        &[(9, 3), (9, 4), (9, 5), (9, 6)],
    );
    let winning = mv(7, 6);
    let mut after_bad = board.clone();
    after_bad.play(mv(0, 0)).unwrap();
    assert_eq!(engine::immediate_wins(&after_bad, Color::White).len(), 2);
    let decision = AiPlayer::new(Box::new(PreferPair {
        top: mv(0, 0),
        next: winning,
    }))
    .decide(
        &board,
        &options(GuardConfig::baseline()),
        &mut StdRng::seed_from_u64(1),
    )
    .unwrap();
    let guard = decision.guard.as_ref().expect("play has diagnostics");
    assert_eq!(guard.proposal, mv(0, 0));
    assert_eq!(guard.refuted, vec![mv(0, 0)]);
    assert_eq!(decision.action, DecisionAction::Play(winning));
    assert_eq!(guard.outcome, GuardOutcome::Terminal);
    assert_eq!(guard.checked, 2);
}

#[test]
fn blocking_one_of_two_distinct_opponent_lines_is_refuted_in_favor_of_own_immediate_win() {
    // White's horizontal b1-e1 needs a1 (f1 is Black); its vertical
    // k2-k5 needs k1 (k6 is Black). Blocking one does not cover the other.
    let board = position(
        &[
            (0, 5),
            (5, 10),
            (13, 5),
            (13, 7),
            (9, 1),
            (9, 2),
            (9, 3),
            (9, 4),
        ],
        &[
            (0, 1),
            (0, 2),
            (0, 3),
            (0, 4),
            (1, 10),
            (2, 10),
            (3, 10),
            (4, 10),
        ],
    );
    let original = board.clone();
    let proposal = mv(0, 0); // a1: the evaluator's top prior; visits are all zero.
    let remaining_win = mv(0, 10); // k1: the distinct vertical line.
    let winning = mv(9, 5); // f10: Black completes b10-f10.
    let guard = GuardConfig {
        enabled: true,
        max_candidates: 2,
        budget: SearchBudget {
            max_nodes: 1_000,
            max_depth: 10,
        },
    };
    assert_eq!(
        engine::immediate_wins(&board, Color::White)
            .iter()
            .collect::<Vec<_>>(),
        vec![proposal, remaining_win]
    );
    let mut after_block = board.clone();
    after_block.play(proposal).unwrap();
    assert_eq!(
        engine::immediate_wins(&after_block, Color::White)
            .iter()
            .collect::<Vec<_>>(),
        vec![remaining_win]
    );
    let proof =
        engine::prove_forced_win(&after_block, after_block.to_move(), guard.budget).unwrap();
    assert_eq!(proof.winner, Color::White);
    assert_eq!(proof.line, vec![remaining_win]);
    assert!(engine::verify_line(&after_block, &proof));

    let mut plain_rng = StdRng::seed_from_u64(1);
    let mut guarded_rng = StdRng::seed_from_u64(1);
    let plain = AiPlayer::new(Box::new(PreferPair {
        top: proposal,
        next: winning,
    }))
    .decide(&board, &options(GuardConfig::disabled()), &mut plain_rng)
    .unwrap();
    let guarded = AiPlayer::new(Box::new(PreferPair {
        top: proposal,
        next: winning,
    }))
    .decide(&board, &options(guard), &mut guarded_rng)
    .unwrap();
    assert_eq!(plain.action, DecisionAction::Play(proposal));
    assert_eq!(
        plain.guard.as_ref().unwrap().outcome,
        GuardOutcome::Disabled
    );
    let stats = plain.search.as_ref().unwrap();
    assert!(stats.visits.iter().all(|&(_, visits)| visits == 0));
    assert_eq!(stats.prior[0].0, proposal);
    let guard = guarded.guard.as_ref().expect("play has diagnostics");
    assert_eq!(guard.proposal, proposal);
    assert_eq!(guard.refuted, vec![proposal]);
    assert_eq!(guard.checked, 2);
    assert_eq!(guard.outcome, GuardOutcome::Terminal);
    assert_eq!(guarded.action, DecisionAction::Play(winning));
    assert_eq!(guarded.search, plain.search);
    assert_eq!(plain_rng.random::<u64>(), guarded_rng.random::<u64>());
    let mut after_win = board.clone();
    after_win.play(played(guarded.action)).unwrap();
    assert_eq!(after_win.status(), Status::Won(Color::Black));
    assert_eq!(board, original);
    assert_eq!(board.zobrist(), original.zobrist());
}

#[test]
fn white_player_guard_probes_actual_black_opponent() {
    let board = Board::from_position(
        &[mv(7, 3), mv(7, 4), mv(7, 5), mv(7, 6), mv(14, 14)],
        &[mv(7, 2), mv(2, 0), mv(2, 2), mv(2, 4)],
        Color::White,
    )
    .unwrap();
    let block = mv(7, 7);
    let original = board.clone();
    let decision = AiPlayer::new(Box::new(PreferPair {
        top: mv(0, 0),
        next: block,
    }))
    .decide(
        &board,
        &options(GuardConfig::baseline()),
        &mut StdRng::seed_from_u64(1),
    )
    .unwrap();
    let guard = decision.guard.as_ref().expect("play has diagnostics");
    assert_eq!(guard.refuted, vec![mv(0, 0)]);
    assert_eq!(decision.action, DecisionAction::Play(block));
    assert_eq!(guard.outcome, GuardOutcome::Unrefuted);
    assert_eq!(board, original);
}

#[test]
fn zero_depth_probe_is_unknown_even_with_opponent_immediate_win() {
    let board = single_threat();
    let config = GuardConfig {
        budget: SearchBudget {
            max_nodes: 1_000,
            max_depth: 0,
        },
        ..GuardConfig::baseline()
    };
    let decision = AiPlayer::new(Box::new(Prefer(mv(0, 0))))
        .decide(&board, &options(config), &mut StdRng::seed_from_u64(1))
        .unwrap();
    assert_eq!(decision.action, DecisionAction::Play(mv(0, 0)));
    let guard = decision.guard.as_ref().expect("play has diagnostics");
    assert_eq!(guard.outcome, GuardOutcome::Unrefuted);
    assert_eq!(guard.checked, 1);
    assert!(guard.refuted.is_empty());
}

#[test]
fn zero_node_probe_is_unknown_even_with_opponent_immediate_win() {
    let board = single_threat();
    let config = GuardConfig {
        budget: SearchBudget {
            max_nodes: 0,
            max_depth: 10,
        },
        ..GuardConfig::baseline()
    };
    let decision = AiPlayer::new(Box::new(UniformEvaluator::new(0.0)))
        .decide(&board, &options(config), &mut StdRng::seed_from_u64(1))
        .unwrap();
    assert_eq!(decision.action, DecisionAction::Play(mv(0, 0)));
    let guard = decision.guard.as_ref().expect("play has diagnostics");
    assert_eq!(guard.outcome, GuardOutcome::Unrefuted);
    assert_eq!(guard.checked, 1);
    assert!(guard.refuted.is_empty());
}

#[test]
fn cap_falls_back_to_unchecked_block_not_refuted_original() {
    let board = single_threat();
    let block = mv(7, 7);
    let guard = GuardConfig {
        max_candidates: 1,
        ..GuardConfig::baseline()
    };
    let decision = AiPlayer::new(Box::new(PreferPair {
        top: mv(0, 0),
        next: block,
    }))
    .decide(&board, &options(guard), &mut StdRng::seed_from_u64(1))
    .unwrap();
    assert_eq!(decision.action, DecisionAction::Play(block));
    let guard = decision.guard.as_ref().expect("play has diagnostics");
    assert_eq!(guard.outcome, GuardOutcome::BudgetExhausted);
    assert_eq!(guard.checked, 1);
    assert_eq!(guard.refuted, vec![mv(0, 0)]);
}

#[test]
fn zero_candidate_cap_keeps_original_unchecked() {
    let board = single_threat();
    let guard = GuardConfig {
        max_candidates: 0,
        ..GuardConfig::baseline()
    };
    let decision = AiPlayer::new(Box::new(Prefer(mv(0, 0))))
        .decide(&board, &options(guard), &mut StdRng::seed_from_u64(1))
        .unwrap();
    assert_eq!(decision.action, DecisionAction::Play(mv(0, 0)));
    let guard = decision.guard.as_ref().expect("play has diagnostics");
    assert_eq!(guard.checked, 0);
    assert!(guard.refuted.is_empty());
    assert_eq!(guard.outcome, GuardOutcome::BudgetExhausted);
}

#[test]
fn every_legal_move_really_refuted_reports_all_refuted_and_original_fallback() {
    // Two independent winning endpoints: neither can be covered with one move.
    let board = position(
        &[(2, 0), (2, 2), (2, 4), (2, 6)],
        &[(7, 3), (7, 4), (7, 5), (7, 6)],
    );
    let original = board.clone();
    let guard = GuardConfig {
        max_candidates: board.empty_moves().count(),
        budget: SearchBudget {
            max_nodes: 1,
            max_depth: 1,
        },
        ..GuardConfig::baseline()
    };
    let decision = AiPlayer::new(Box::new(UniformEvaluator::new(0.0)))
        .decide(&board, &options(guard), &mut StdRng::seed_from_u64(1))
        .unwrap();
    let diagnostics = decision.guard.as_ref().expect("play has diagnostics");
    assert_eq!(diagnostics.outcome, GuardOutcome::AllRefuted);
    assert_eq!(diagnostics.checked, board.empty_moves().count());
    assert_eq!(diagnostics.refuted.len(), board.empty_moves().count());
    for candidate in board.empty_moves() {
        assert!(diagnostics.refuted.contains(&candidate));
        let mut after = board.clone();
        after.play(candidate).unwrap();
        let proof = engine::prove_forced_win(&after, after.to_move(), guard.budget).unwrap();
        assert!(engine::verify_line(&after, &proof));
    }
    assert_eq!(decision.action, DecisionAction::Play(diagnostics.proposal));
    assert!(board.is_legal(played(decision.action)));
    assert_eq!(board, original);
}

#[test]
fn own_immediate_win_is_terminal_not_opponent_probe_unknown() {
    let board = position(
        &[(0, 1), (0, 2), (0, 3), (0, 4)],
        &[(2, 0), (2, 2), (2, 4), (2, 6)],
    );
    let guard = GuardConfig {
        budget: SearchBudget {
            max_nodes: 0,
            max_depth: 0,
        },
        ..GuardConfig::baseline()
    };
    let decision = AiPlayer::new(Box::new(UniformEvaluator::new(0.0)))
        .decide(&board, &options(guard), &mut StdRng::seed_from_u64(1))
        .unwrap();
    assert_eq!(decision.action, DecisionAction::Play(mv(0, 0)));
    let guard = decision.guard.as_ref().expect("play has diagnostics");
    assert_eq!(guard.outcome, GuardOutcome::Terminal);
    assert_eq!(guard.checked, 1);
    assert!(guard.refuted.is_empty());
    let mut after = board.clone();
    after.play(played(decision.action)).unwrap();
    assert_eq!(after.status(), Status::Won(Color::Black));
}

#[test]
fn enabled_unknown_preserves_seeded_first_candidate_noise_stats_and_rng() {
    let board = Board::new();
    let original = board.clone();
    for simulations in [0, 11] {
        let mut plain_options = options(GuardConfig::disabled());
        plain_options.simulations = simulations;
        plain_options.temperature = 0.7;
        plain_options.noise_epsilon = 0.25;
        let guarded_options = PlayerOptions {
            guard: GuardConfig {
                budget: SearchBudget {
                    max_nodes: 0,
                    max_depth: 10,
                },
                ..GuardConfig::baseline()
            },
            ..plain_options
        };
        let mut plain_rng = StdRng::seed_from_u64(42);
        let mut guarded_rng = StdRng::seed_from_u64(42);
        let plain = AiPlayer::new(Box::new(UniformEvaluator::new(0.3)))
            .decide(&board, &plain_options, &mut plain_rng)
            .unwrap();
        let guarded = AiPlayer::new(Box::new(UniformEvaluator::new(0.3)))
            .decide(&board, &guarded_options, &mut guarded_rng)
            .unwrap();
        assert_eq!(guarded.action, plain.action);
        let guard = guarded.guard.as_ref().expect("play has diagnostics");
        assert_eq!(guard.proposal, played(plain.action));
        assert_eq!(guard.outcome, GuardOutcome::Unrefuted);
        assert_eq!(guarded.search, plain.search);
        assert_eq!(plain_rng.random::<u64>(), guarded_rng.random::<u64>());
        assert_eq!(board, original);
    }
}

struct NeverEvaluate;
impl Evaluator for NeverEvaluate {
    fn evaluate(&mut self, _: EvalRequest) -> EvalResult {
        panic!("a certified own win must bypass search and the guard");
    }
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
fn enabled_own_probe_precedes_guard_and_search() {
    // With both probes enabled, the own probe certifies first: no search,
    // no guard candidate checks, no evaluator call.
    let board = vcf_fixture();
    let certified = mv(7, 7);
    let mut player = AiPlayer::new(Box::new(NeverEvaluate));
    let options = PlayerOptions {
        simulations: 100,
        own: OwnTssConfig::baseline(),
        guard: GuardConfig::baseline(),
        ..PlayerOptions::default()
    };
    let mut rng = StdRng::seed_from_u64(3);

    let decision = player.decide(&board, &options, &mut rng).unwrap();

    assert_eq!(decision.action, DecisionAction::Play(certified));
    assert!(decision.search.is_none());
    let guard = decision.guard.as_ref().expect("play has diagnostics");
    assert_eq!(guard.outcome, GuardOutcome::ProvenWin);
    assert_eq!(guard.checked, 0);
    assert!(guard.refuted.is_empty());
}
