//! Shared helpers for the differential gates.
//!
//! This module is not a test target: Cargo only compiles `.rs` files
//! directly under `tests/` as integration tests. Each test target that
//! needs these helpers declares `mod common;` and compiles its own
//! copy — hence the crate-level `allow(dead_code)`, since not every
//! target necessarily uses every helper.

#![allow(dead_code)]

use engine::{Board, Status};
use rand::{RngExt, SeedableRng, rngs::StdRng};

/// Minimum number of plies in a generated board.
///
/// Near-empty boards make the differential gates vacuous: with no
/// threats anywhere, network and engine trivially agree.
const MIN_PLIES: u32 = 10;

/// Maximum number of plies in a generated board.
///
/// Kept well below the 225-cell board capacity so boards stay
/// mid-game and accidental terminal positions (a random line
/// completing five) are rare.
const MAX_PLIES: u32 = 60;

/// Build a deterministic mid-game board from a seed.
///
/// The seed drives both the ply count (drawn uniformly from
/// `MIN_PLIES..=MAX_PLIES`) and each uniformly random legal move.
/// Same seed → same board, always.
///
/// The returned board is never terminal: a random line may complete
/// five on the final ply, and the differential gates test mid-game
/// positions (the engine oracles and the network only agree there —
/// see below), so any terminal board is rewound to the last ongoing
/// position.
///
/// Why terminal boards must be excluded: `engine::immediate_wins`
/// answers "does the side have five *after* the move?", which is
/// trivially true for *every* empty cell once the side already has
/// five, while the network computes "creates a *new* five". The two
/// semantics coincide exactly on non-terminal boards and diverge on
/// terminal ones.
pub fn seeded_board(seed: u64) -> Board {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut board = Board::new();

    let plies = rng.random_range(MIN_PLIES..=MAX_PLIES);

    for _ in 0..plies {
        if board.status() != Status::Ongoing {
            break;
        }
        let moves: Vec<_> = board.empty_moves().collect();
        let mv = moves[rng.random_range(0..moves.len())];
        board
            .play(mv)
            .expect("empty_moves returns only legal moves");
    }

    while board.status() != Status::Ongoing {
        board.undo();
    }

    board
}
