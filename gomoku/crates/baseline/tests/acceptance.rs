//! Acceptance measurements for the baseline evaluator.
//!
//! This file currently carries the `[experiment]` quiet-whisper A/B
//! from sub-chapter 07. The measured strength ladder (uniform-random
//! gradient, TacticsEvaluator match, VCF puzzle solve rate) lands in
//! sub-chapter 09 and grows this file.

use baseline::{BaselineEvaluator, Device};
use engine::{Board, Color, Move, Status};
use mcts::{SearchConfig, TacticsEvaluator, search, select_move, visit_distribution};
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::time::Instant;

/// The baseline's move at the given simulation budget.
fn baseline_best_move(
    board: &Board,
    simulations: u32,
    rng: &mut StdRng,
    ev: &mut BaselineEvaluator,
) -> Move {
    let cfg = SearchConfig {
        simulations,
        c_puct: 1.5,
    };
    let outcome = search(board, ev, &cfg);
    let dist = visit_distribution(&outcome.tree, outcome.root);
    select_move(&dist, 0.0, rng).expect("root has legal edges")
}

/// The tactics mock's move at the given simulation budget.
fn tactics_best_move(
    board: &Board,
    simulations: u32,
    rng: &mut StdRng,
    ev: &mut TacticsEvaluator,
) -> Move {
    let cfg = SearchConfig {
        simulations,
        c_puct: 1.5,
    };
    let outcome = search(board, ev, &cfg);
    let dist = visit_distribution(&outcome.tree, outcome.root);
    select_move(&dist, 0.0, rng).expect("root has legal edges")
}

/// Run explicitly in release:
/// `cargo test -p baseline --release --test acceptance quiet_whisper_ab_measured -- --ignored --nocapture`
///
/// Protocol: 20 games vs the `TacticsEvaluator` at 200 simulations with
/// alternating colors. One arm uses the default whisper-on evaluator,
/// the other uses `.without_quiet_value()`. Base seed 20_260_901.
///
/// The test does not assert a win-rate bar: the mock is expected to be
/// too weak to discriminate. The measurement is the result.
#[ignore]
#[test]
fn quiet_whisper_ab_measured() {
    const GAMES: usize = 20;
    const SIMS: u32 = 200;
    const BASE_SEED: u64 = 20_260_901;

    fn play_game_ab(baseline_color: Color, quiet: bool, seed: u64) -> (Status, u32, f64) {
        let mut board = Board::new();
        let mut rng = StdRng::seed_from_u64(seed);
        let mut plies = 0u32;
        let start = Instant::now();
        let mut baseline_ev = if quiet {
            BaselineEvaluator::new(Device::flex())
        } else {
            BaselineEvaluator::new(Device::flex()).without_quiet_value()
        };
        let mut tactics_ev = TacticsEvaluator::new();

        while board.status() == Status::Ongoing && plies < 225 {
            let side = board.to_move();
            let mv = if side == baseline_color {
                baseline_best_move(&board, SIMS, &mut rng, &mut baseline_ev)
            } else {
                tactics_best_move(&board, SIMS, &mut rng, &mut tactics_ev)
            };
            assert!(
                board.is_legal(mv),
                "illegal move {mv:?} at ply {plies} for {side:?}"
            );
            board.play(mv).unwrap();
            plies += 1;
        }
        (board.status(), plies, start.elapsed().as_secs_f64())
    }

    println!("\n=== quiet whisper A/B ===\n");
    println!(
        "{:<12} {:>6} {:>6} {:>6} {:>10} {:>12}",
        "arm", "wins", "losses", "draws", "avg_plies", "avg_s/game"
    );
    for quiet in [true, false] {
        let label = if quiet { "whisper ON" } else { "whisper OFF" };
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut draws = 0usize;
        let mut total_plies = 0u32;
        let mut total_time = 0.0f64;
        for g in 0..GAMES {
            let baseline_color = if g % 2 == 0 {
                Color::Black
            } else {
                Color::White
            };
            let seed = BASE_SEED + g as u64;
            let (status, plies, secs) = play_game_ab(baseline_color, quiet, seed);
            total_plies += plies;
            total_time += secs;
            match status {
                Status::Won(c) if c == baseline_color => wins += 1,
                Status::Won(_) => losses += 1,
                Status::Draw => draws += 1,
                Status::Ongoing => panic!("game {g} did not terminate"),
            }
        }
        println!(
            "{:<12} {:>6} {:>6} {:>6} {:>10.1} {:>12.3}",
            label,
            wins,
            losses,
            draws,
            total_plies as f64 / GAMES as f64,
            total_time / GAMES as f64
        );
    }
}
