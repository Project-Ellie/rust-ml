//! Seeded random playout driver.
//!
//! This module plays uniformly random legal games on the engine board.
//! It mirrors the engine differential harness (random legal games) but
//! lives in `train` because the engine crate is a dependency island:
//! no `rand`, no I/O, no Burn.

use engine::utils::dist;
use engine::{Board, Move, Status};
use rand::{Rng, RngExt};

/// Maximum number of plies sampled from a single game.
///
/// The cap prevents one unusually tactical game from dominating a
/// class bucket in the quota collector (chapter 4).
pub const MAX_PLIES_PER_GAME: usize = 3;

/// Focus mode would place the stone preferrably near the center, generating lines earlier.
#[derive(Clone, Copy, Debug)]
pub enum RandomMode {
    NoFocus,
    Focus,
}

/// Play a uniformly random legal game to terminal; return the move history.
///
/// # Panics
///
/// This function does not panic in correct engine usage: `empty_moves`
/// returns only legal cells, and the loop only plays while the status is
/// `Ongoing`. The `.expect("empty_moves returns only legal moves")` is
/// therefore unreachable under the engine's contract.
/// parameter `RandomMode` creates more 'realistically' looking positions
pub fn random_game(rng: &mut impl Rng, mode: RandomMode) -> Vec<Move> {
    let mut board = Board::new();
    while board.status() == Status::Ongoing {
        let moves: Vec<Move> = board.empty_moves().collect();

        let mv = match mode {
            RandomMode::NoFocus => {
                let i = rng.random_range(0..moves.len());
                moves[i]
            }
            RandomMode::Focus => {
                // a wabbeling center
                let rand_r = rng.random_range(3..=11);
                let rand_c = rng.random_range(3..=11);
                let center = Move::new(rand_r, rand_c).unwrap();

                let best = moves
                    .iter()
                    .map(|m| (m, dist(m, &center)))
                    .min_by(|a, b| a.1.total_cmp(&b.1));

                let best = best.expect("There's always a closest stone").0;
                *best
            }
        };
        board.play(mv).expect("failed to play move");
    }
    board.moves().to_vec()
}

/// Choose `n` distinct ply indices in `1..=game_len` (uniform without
/// replacement), so one game contributes at most `n` positions.
///
/// If `n` is larger than `game_len`, the result is saturated: every index
/// in `1..=game_len` is returned, giving `game_len` plies.
pub fn sample_plies(game_len: usize, n: usize, rng: &mut impl Rng) -> Vec<usize> {
    if game_len == 0 {
        return Vec::new();
    }
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
    use engine::utils::board_to_string;
    use rand::rngs::StdRng;
    use rand::{SeedableRng, rng};

    // this is not really a test. But I wanted to see the samples as ascii boards.
    #[test]
    fn focus_puts_stones_near_center() {
        let moves = random_game(&mut rng(), RandomMode::Focus);
        let display = board_to_string(moves);
        println!("{display}");
    }

    #[test]
    fn random_game_terminates_within_225_plies() {
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..100 {
            let moves = random_game(&mut rng, RandomMode::NoFocus);
            assert!(
                moves.len() <= 225,
                "Game length {} exceeds 225",
                moves.len()
            );

            let mut board = Board::new();
            for mv in moves {
                assert_eq!(board.status(), Status::Ongoing);
                board.play(mv).expect("Every move must be legal");
            }
            assert_ne!(board.status(), Status::Ongoing);
        }
    }

    #[test]
    fn same_seed_gives_same_history() {
        let h1 = random_game(&mut StdRng::seed_from_u64(12345), RandomMode::NoFocus);
        let h2 = random_game(&mut StdRng::seed_from_u64(12345), RandomMode::NoFocus);
        assert_eq!(h1, h2);
        let h1 = random_game(&mut StdRng::seed_from_u64(37), RandomMode::Focus);
        let h2 = random_game(&mut StdRng::seed_from_u64(37), RandomMode::Focus);
        assert_eq!(h1, h2);
    }

    #[test]
    fn sample_plies_are_distinct_and_in_range() {
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..200 {
            let game_len = rng.random_range(1..225);
            let n = rng.random_range(0..10);
            let plies = sample_plies(game_len, n, &mut rng);
            assert!(plies.len() <= n.min(game_len));
            assert!(plies.iter().all(|&p| p >= 1 && p <= game_len));
            let mut sorted = plies.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), plies.len(), "duplicated found");
        }
    }
}
