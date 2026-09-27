//! Acceptance tests for the MCTS crate (milestone 2).
//!
//! These are the tutorial's TDD capstones: tactical puzzles that
//! prove the sign convention and search direction are correct, plus
//! a sanity flood of full games against a uniform evaluator.

use engine::reference::board_from_ascii;
use engine::{Board, Color, Move, Status};
use mcts::{
    SearchConfig, TacticsEvaluator, UniformEvaluator, search, select_move, visit_distribution,
};
use rand::SeedableRng;
use rand::rngs::StdRng;

/// Helper: run MCTS on a board and return the most-visited move.
fn best_move(board: &Board, simulations: u32, rng: &mut StdRng) -> Move {
    let mut ev = TacticsEvaluator::new();
    let cfg = SearchConfig {
        simulations,
        c_puct: 1.5,
    };
    let outcome = search(board, &mut ev, &cfg);
    let dist = visit_distribution(&outcome.tree, outcome.root);
    select_move(&dist, 0.0, rng).expect("root has edges")
}

// ------------------------------------------------------------------
// Tactical suite
// ------------------------------------------------------------------

#[test]
fn win_in_one_is_solved_at_50_sims() {
    // Black has an open four on row 7.
    let b = board_from_ascii(
        "
        O . O . O . O . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . X X X X . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        ",
    );
    assert_eq!(b.to_move(), Color::Black);

    let mut rng = StdRng::seed_from_u64(1);
    let mv = best_move(&b, 50, &mut rng);
    let wins = engine::immediate_wins(&b, Color::Black);
    assert!(
        wins.contains(mv),
        "best move {:?} must be an immediate win",
        mv
    );
}

#[test]
fn forced_block_is_taken_at_50_sims() {
    // Build a position where White has a CLOSED four on row 7 and
    // Black to move must block the single winning cell at (7,7).
    // Sequence: Black first blocks the left end at (7,2), then the
    // players trade quiet stones until White has four in a row.
    let mut b = Board::new();
    let script = [
        (7, 2), // Black blocks the left end
        (7, 3),
        (0, 0),
        (7, 4),
        (0, 1),
        (7, 5),
        (0, 2),
        (7, 6),
    ];
    for &(r, c) in &script {
        b.play(Move::new(r, c).unwrap()).unwrap();
    }
    assert_eq!(b.to_move(), Color::Black);
    let blocks = engine::forced_blocks(&b);
    assert_eq!(blocks.len(), 1, "White must have exactly one winning cell");
    assert!(blocks.contains(Move::new(7, 7).unwrap()));

    let mut rng = StdRng::seed_from_u64(2);
    let mv = best_move(&b, 50, &mut rng);
    assert!(
        blocks.contains(mv),
        "best move {:?} must be the forced block",
        mv
    );
}

#[test]
fn win_in_three_first_move_is_found() {
    // Reuse the engine TSS win-in-3 script. Black plays (7,7) to
    // create a closed four; White must block; Black follows with a
    // double threat.
    let mut b = Board::new();
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
        b.play(Move::new(r, c).unwrap()).unwrap();
    }
    assert_eq!(b.to_move(), Color::Black);

    let mut rng = StdRng::seed_from_u64(3);
    let mv = best_move(&b, 200, &mut rng);
    assert_eq!(
        mv,
        Move::new(7, 7).unwrap(),
        "search must find the forcing first move"
    );
}

// ------------------------------------------------------------------
// Sanity suite
// ------------------------------------------------------------------

#[test]
fn thousand_full_games_terminate_with_legal_moves() {
    let mut rng = StdRng::seed_from_u64(42);
    let cfg = SearchConfig {
        simulations: 25,
        c_puct: 1.5,
    };

    for game in 0..1000 {
        let mut board = Board::new();
        let mut moves_played = 0;

        while board.status() == Status::Ongoing && moves_played < 225 {
            let mut ev = UniformEvaluator::new(0.0);
            let outcome = search(&board, &mut ev, &cfg);
            let dist = visit_distribution(&outcome.tree, outcome.root);
            let mv = select_move(&dist, 0.0, &mut rng)
                .expect("root must have legal moves while game is ongoing");

            assert!(
                board.is_legal(mv),
                "game {game}, move {moves_played}: search suggested illegal move {mv:?}"
            );
            board.play(mv).expect("legal move must be accepted");
            moves_played += 1;
        }

        assert!(
            board.status() != Status::Ongoing || moves_played == 225,
            "game {game} did not terminate"
        );
        assert!(
            moves_played <= 225,
            "game {game} exceeded the maximal move count"
        );
    }
}
