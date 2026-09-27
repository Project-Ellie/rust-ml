# Chapter 02 — Deep-dive solution: seeded random playout driver

This is the opt-in reference for Chapter 02. `playout.rs` is quoted
verbatim from the verified reference crate, including its tests; the
`lib.rs` registration line is included at the end.

## `src/playout.rs`

```rust
//! Seeded random playout driver.
//!
//! This module plays uniformly random legal games on the engine board.
//! It mirrors the engine differential harness (random legal games) but
//! lives in `train` because the engine crate is a dependency island:
//! no `rand`, no I/O, no Burn.

use engine::{Board, Move, Status};
use rand::{Rng, RngExt};

/// Maximum number of plies sampled from a single game.
///
/// The cap prevents one unusually tactical game from dominating a
/// class bucket in the quota collector (chapter 4).
pub const MAX_PLIES_PER_GAME: usize = 3;

/// Play a uniformly random legal game to terminal; return the move history.
pub fn random_game(rng: &mut impl Rng) -> Vec<Move> {
    let mut board = Board::new();
    while board.status() == Status::Ongoing {
        let moves: Vec<Move> = board.empty_moves().collect();
        let i = rng.random_range(0..moves.len());
        board
            .play(moves[i])
            .expect("empty_moves returns only legal moves");
    }
    board.moves().to_vec()
}

/// Choose `n` distinct ply indices in `1..=game_len` (uniform without
/// replacement), so one game contributes at most `n` positions.
pub fn sample_plies(game_len: usize, n: usize, rng: &mut impl Rng) -> Vec<usize> {
    if game_len == 0 || n == 0 {
        return Vec::new();
    }

    // Partial Fisher–Yates shuffle: only shuffle the first `k` positions.
    let mut indices: Vec<usize> = (1..=game_len).collect();
    let k = n.min(game_len);
    for i in 0..k {
        let j = rng.random_range(i..game_len);
        indices.swap(i, j);
    }
    indices.truncate(k);
    indices
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngExt;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn random_game_terminates_within_225_plies() {
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..100 {
            let history = random_game(&mut rng);
            assert!(
                history.len() <= 225,
                "game length {} exceeds 225",
                history.len()
            );

            let mut board = Board::new();
            for &mv in &history {
                assert_eq!(board.status(), Status::Ongoing);
                board.play(mv).expect("every move must be legal");
            }
            assert_ne!(board.status(), Status::Ongoing);
        }
    }

    #[test]
    fn same_seed_gives_same_history() {
        let a = random_game(&mut StdRng::seed_from_u64(12345));
        let b = random_game(&mut StdRng::seed_from_u64(12345));
        assert_eq!(a, b);
    }

    #[test]
    fn sample_plies_are_distinct_and_in_range() {
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..200 {
            let game_len = rng.random_range(1..=225);
            let n = rng.random_range(0..=10);
            let plies = sample_plies(game_len, n, &mut rng);
            assert!(plies.len() <= n.min(game_len));
            assert!(plies.iter().all(|&p| p >= 1 && p <= game_len));
            let mut sorted = plies.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), plies.len(), "duplicates found");
        }
    }
}
```

## `src/lib.rs` registration

```rust
pub mod playout;
pub mod sample;
```
