//! Isolated diagnostic for the captured g4 position; no live process interaction.
use cli::{
    notation::{algebraic, parse_algebraic},
    puzzle::PuzzlePack,
};
use engine::{Board, Color, Move, Proof, SearchBudget, Status, immediate_wins, verify_line};
use mcts::{EvalRequest, EvalResult, Evaluator};
use player::{AiPlayer, DecisionAction, GuardConfig, OwnTssConfig, PlayerOptions};
use rand::{SeedableRng, rngs::StdRng};
use serde_json::{Value, json};
use std::time::Instant;

fn mv(s: &str) -> Move {
    parse_algebraic(s).unwrap()
}
fn names(moves: impl IntoIterator<Item = Move>) -> Vec<String> {
    moves.into_iter().map(algebraic).collect()
}
fn hypothesis() -> Proof {
    Proof {
        winner: Color::White,
        line: ["g4", "f3", "h4"].map(mv).to_vec(),
    }
}
fn details(b: &Board) {
    println!(
        "to_move={:?} status={:?} Black_wins={:?} White_wins={:?}",
        b.to_move(),
        b.status(),
        names(immediate_wins(b, Color::Black).iter()),
        names(immediate_wins(b, Color::White).iter())
    );
}
struct Prefer(Move);
impl Evaluator for Prefer {
    fn evaluate(&mut self, _: EvalRequest) -> EvalResult {
        let mut policy = [0.0; 225];
        policy[self.0.index()] = 10.0;
        EvalResult { policy, value: 0.0 }
    }
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let text = std::fs::read_to_string(&args[1]).unwrap();
    let pack = PuzzlePack::from_json(&text).unwrap();
    let root = &pack.get(0).unwrap().board;
    let counts = [Color::Black, Color::White].map(|side| {
        (0..15)
            .flat_map(|r| (0..15).map(move |c| Move::new(r, c).unwrap()))
            .filter(|&m| root.stone_at(m) == Some(side))
            .count()
    });
    assert_eq!(counts, [21, 21]);
    assert_eq!(root.to_move(), Color::Black);
    assert!(pack.items[0].solution.is_empty());
    println!(
        "PuzzlePack validated: 21 Black / 21 White; Black to move; empty claimed line; imported position, NOT recovered history"
    );
    let mode = args[2].as_str();
    let mut b = root.clone();
    if mode == "validate" {
        details(&b);
        return;
    }
    if mode == "exhaustive" {
        exhaustive(root, args[3].parse().unwrap(), args[4].parse().unwrap());
        return;
    }
    if mode == "own" {
        let budget = OwnTssConfig::baseline().budget;
        let proof = engine::prove_forced_win(root, Color::Black, budget);
        println!(
            "{}",
            json!({"own_probe": proof_record(root, proof.as_ref()), "max_nodes": budget.max_nodes, "max_depth": budget.max_depth})
        );
        return;
    }
    if mode == "real" {
        real_decision(root, args[3].parse().unwrap());
        return;
    }
    let candidate = mv(&args[3]);
    if mode == "guard" {
        let mut ai = AiPlayer::new(Box::new(Prefer(candidate)));
        let options = PlayerOptions {
            simulations: 0,
            temperature: 0.0,
            noise_epsilon: 0.0,
            own: OwnTssConfig::disabled(),
            guard: GuardConfig::baseline(),
            ..PlayerOptions::default()
        };
        let start = Instant::now();
        let result = ai
            .decide(root, &options, &mut StdRng::seed_from_u64(3477))
            .unwrap();
        let selected = match result.action {
            DecisionAction::Play(mv) => mv,
            DecisionAction::Resign(_) => panic!("resignation is disabled in this diagnostic"),
        };
        let guard = result.guard.as_ref().expect("resignation disabled");
        assert_eq!(guard.proposal, candidate);
        println!(
            "CONTROLLED guard, NOT original 10000-sim/RNG reproduction: proposal={} chosen={} checked={} refuted={:?} outcome={:?} config={:?} elapsed_ms={:.3}",
            algebraic(guard.proposal),
            algebraic(selected),
            guard.checked,
            names(guard.refuted.iter().copied()),
            guard.outcome,
            options.guard,
            start.elapsed().as_secs_f64() * 1000.0
        );
        return;
    }
    b.play(candidate).unwrap();
    println!("after Black {}", algebraic(candidate));
    details(&b);
    println!(
        "specific White g4 Black f3 White h4 verified={}",
        verify_line(&b, &hypothesis())
    );
    if mode == "line" {
        for m in hypothesis().line {
            if !b.is_legal(m) {
                println!("{} illegal", algebraic(m));
                break;
            }
            println!("play {:?} {}", b.to_move(), algebraic(m));
            b.play(m).unwrap();
            details(&b);
        }
        return;
    }
    let budget = SearchBudget {
        max_nodes: args[4].parse().unwrap(),
        max_depth: args[5].parse().unwrap(),
    };
    let start = Instant::now();
    let proof = engine::prove_forced_win(&b, b.to_move(), budget);
    let prover_ms = start.elapsed().as_secs_f64() * 1000.0;
    match proof {
        Some(proof) => println!(
            "budget={budget:?} winner={:?} line={:?} verified={} prover_ms={prover_ms:.3} total_ms={:.3}",
            proof.winner,
            names(proof.line.iter().copied()),
            verify_line(&b, &proof),
            start.elapsed().as_secs_f64() * 1000.0
        ),
        None => println!("budget={budget:?} NoProof=UNKNOWN prover_ms={prover_ms:.3}"),
    }
}

fn proof_record(board: &Board, proof: Option<&Proof>) -> Value {
    match proof {
        Some(proof) => {
            let verified = verify_line(board, proof);
            let legal_first = proof.line.first().is_some_and(|&m| board.is_legal(m));
            json!({"winner": format!("{:?}", proof.winner), "line": names(proof.line.iter().copied()), "verified": verified, "legal_first": legal_first, "admitted": proof.winner == board.to_move() && legal_first && verified})
        }
        None => {
            json!({"line": null, "verified": false, "admitted": false, "result": "NoProof=UNKNOWN"})
        }
    }
}

fn exhaustive(root: &Board, start: usize, end: usize) {
    let legal: Vec<_> = root.empty_moves().filter(|&m| root.is_legal(m)).collect();
    assert_eq!(legal.len(), 183);
    let mut records = Vec::new();
    for &candidate in &legal[start..end] {
        let mut after = root.clone();
        after.play(candidate).unwrap();
        let terminal = after.status();
        let mut attempts = Vec::new();
        let mut covered = false;
        for line in [
            vec!["g4", "f3", "h4"],
            vec!["k4", "l3", "l5", "i5", "j3"],
            vec!["f3", "g4", "k4", "l3", "l5", "i5", "j3"],
        ] {
            let proof = Proof {
                winner: Color::White,
                line: line.into_iter().map(mv).collect(),
            };
            let record = proof_record(&after, Some(&proof));
            covered |= record["admitted"] == true;
            attempts
                .push(json!({"source": "known_line_independent_verification", "proof": record}));
        }
        if !covered && terminal == Status::Ongoing {
            for budget in [
                GuardConfig::baseline().budget,
                SearchBudget {
                    max_nodes: 10_000,
                    max_depth: 3,
                },
                SearchBudget {
                    max_nodes: 10_000,
                    max_depth: 5,
                },
                SearchBudget {
                    max_nodes: 10_000,
                    max_depth: 10,
                },
            ] {
                let proof = engine::prove_forced_win(&after, Color::White, budget);
                let record = proof_record(&after, proof.as_ref());
                covered |= record["admitted"] == true;
                attempts.push(json!({"source": "actual_tss", "max_nodes": budget.max_nodes, "max_depth": budget.max_depth, "proof": record}));
                if covered {
                    break;
                }
            }
        }
        records.push(json!({"move": algebraic(candidate), "index": candidate.index(), "terminal_status": format!("{terminal:?}"), "black_terminal_win_or_draw": matches!(terminal, Status::Won(Color::Black) | Status::Draw), "white_win_certified": covered, "classification": if covered { "verified_White_win" } else { "UNKNOWN_not_defense" }, "attempts": attempts}));
    }
    assert_eq!(root.empty_moves().count(), 183);
    println!(
        "{}",
        json!({"start": start, "end": end, "root_status": format!("{:?}", root.status()), "legal_count": legal.len(), "records": records})
    );
}

#[cfg(feature = "baseline")]
fn real_decision(root: &Board, simulations: u32) {
    let options = PlayerOptions {
        simulations,
        c_puct: 1.5,
        temperature: 1.0,
        noise_epsilon: 0.0,
        own: OwnTssConfig::baseline(),
        guard: GuardConfig::baseline(),
        ..PlayerOptions::default()
    };
    println!(
        "REAL_START fresh_seed=3477 NOT_original_RNG_state simulations={simulations} options={options:?}"
    );
    let mut ai = AiPlayer::new(Box::new(baseline::BaselineEvaluator::new(
        baseline::Device::flex(),
    )));
    let start = Instant::now();
    let result = ai
        .decide(root, &options, &mut StdRng::seed_from_u64(3477))
        .unwrap();
    let selected = match result.action {
        DecisionAction::Play(mv) => mv,
        DecisionAction::Resign(_) => panic!("resignation is disabled in this diagnostic"),
    };
    let guard = result.guard.as_ref().expect("resignation disabled");
    let mut ranked = Vec::new();
    if let Some(stats) = &result.search {
        for &(m, visits) in &stats.visits {
            let prior = stats.prior.iter().find(|&&(p, _)| p == m).unwrap().1;
            ranked.push((m, visits, prior));
        }
        ranked.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| b.2.partial_cmp(&a.2).unwrap())
                .then_with(|| a.0.index().cmp(&b.0.index()))
        });
        assert_eq!(ranked.len(), 183);
        assert_eq!(ranked.iter().map(|x| x.1).sum::<u32>(), simulations);
    }
    let rank = |m| ranked.iter().position(|x| x.0 == m).map(|n| n + 1);
    let ranks: Vec<_> = ["e7", "f6", "g4", "h4", "f3"]
        .map(|s| json!({"move": s, "rank": rank(mv(s))}))
        .to_vec();
    let ordering: Vec<_> = ranked.iter().enumerate().map(|(n, &(m, visits, prior))| json!({"rank": n+1, "move": algebraic(m), "index": m.index(), "visits": visits, "prior": prior})).collect();
    println!(
        "{}",
        json!({"simulations": simulations, "seed": 3477, "fresh_seed_not_exact_replay": true, "original_proposal": algebraic(guard.proposal), "selected_move": algebraic(selected), "checked": guard.checked, "refuted": names(guard.refuted.iter().copied()), "outcome": format!("{:?}", guard.outcome), "own_certified_move_bypassed_search": result.search.is_none(), "search_value": result.search.as_ref().map(|s| s.value), "chosen_rank": rank(selected), "proposal_rank": rank(guard.proposal), "requested_ranks": ranks, "ordering": ordering, "elapsed_ms": start.elapsed().as_secs_f64()*1000.0})
    );
}

#[cfg(not(feature = "baseline"))]
fn real_decision(_: &Board, _: u32) {
    panic!("real decision requires baseline feature");
}
