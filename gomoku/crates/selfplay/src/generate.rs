//! Seeded corpus generation with per-run Zobrist-walk deduplication.
//!
//! One invocation of [`generate_games`] accepts a contiguous base-seed
//! range, two seat configurations, and an [`OpeningSource`]. Each slot
//! starts from its base seed; if the resulting game's ordered Zobrist walk
//! collides with a game already accepted in this run, the slot derives the
//! next seed as a wrapping add and tries again. The accepted seed — base or
//! derived — is the seed recorded in the game's header and the corpus
//! manifest.

use std::collections::HashSet;

use baseline::{BaselineEvaluator, Device};
use engine::{Board, Move};
use mcts::Evaluator;
use player::PlayerOptions;
use thiserror::Error;

use crate::game::{Game, GameError, play_game_with_seats};
use crate::opening::{OpeningAnswer, OpeningSource};
use crate::schedule::ExplorationSchedule;

/// Failure during corpus generation.
#[derive(Debug, Error)]
pub enum GenerateError {
    /// The seeded runner failed to produce or complete a candidate game.
    #[error(transparent)]
    Game(#[from] GameError),

    /// A base seed could not produce a unique game within the attempt budget.
    #[error("could not generate a unique game for base seed {base_seed} after {attempts} attempts")]
    DuplicateExhausted {
        /// Seed the slot started from.
        base_seed: u64,
        /// Number of candidates tried before giving up.
        attempts: u64,
    },
}

/// Maximum candidate games tried for one base seed before reporting failure.
const MAX_ATTEMPTS_PER_GAME: u64 = 128;

/// Opening placements in ceremony order: the three initial stones, plus the
/// two added stones when the answer is the add-two variant.
fn opening_moves(game: &Game) -> Vec<Move> {
    let mut moves = Vec::with_capacity(5);
    moves.extend_from_slice(&game.opening.initial);
    if let OpeningAnswer::AddTwo { stones, .. } = &game.opening.answer {
        moves.extend_from_slice(stones);
    }
    moves
}

/// Replay the full game through a fresh board and collect the Zobrist key
/// after every move, opening placements included.
///
/// The walk, not the final position, is the identity: two games that reach
/// the same end position through different move orders are different games.
fn zobrist_walk(game: &Game) -> Result<Vec<u64>, GameError> {
    let mut board = Board::new();
    let mut walk = Vec::with_capacity(game.plies.len() + 5);

    for mv in opening_moves(game) {
        board.play(mv).map_err(GameError::Opening)?;
        walk.push(board.zobrist());
    }

    for (ply, record) in game.plies.iter().enumerate() {
        board.play(record.mv).map_err(|source| GameError::Play {
            ply,
            mv: record.mv,
            source,
        })?;
        walk.push(board.zobrist());
    }

    Ok(walk)
}

/// Generate a corpus using an injected runner for each candidate seed.
///
/// The deterministic entry for fabricated-game tests and for callers with
/// another in-process runner. Seed derivation and collision handling are
/// identical to [`generate_games`]: attempt 0 uses the base seed itself,
/// attempt *n* uses `base_seed.wrapping_add(n)`, and at most
/// [`MAX_ATTEMPTS_PER_GAME`] candidates are tried per slot.
///
/// # Errors
/// Returns [`GenerateError::Game`] when the injected runner fails, or
/// [`GenerateError::DuplicateExhausted`] when a single base seed collides
/// on every attempt within the budget.
pub fn generate_games_with<I, F>(base_seeds: I, mut runner: F) -> Result<Vec<Game>, GenerateError>
where
    I: IntoIterator<Item = u64>,
    F: FnMut(u64) -> Result<Game, GameError>,
{
    let mut seen = HashSet::new();
    let mut accepted = Vec::new();

    for base_seed in base_seeds {
        let mut attempt = 0u64;
        loop {
            if attempt >= MAX_ATTEMPTS_PER_GAME {
                return Err(GenerateError::DuplicateExhausted {
                    base_seed,
                    attempts: attempt,
                });
            }

            let seed = base_seed.wrapping_add(attempt);
            let game = runner(seed)?;
            let walk = zobrist_walk(&game)?;

            if seen.insert(walk) {
                accepted.push(game);
                break;
            }

            attempt += 1;
        }
    }

    Ok(accepted)
}

/// Generate a corpus from a base-seed iterator with the real game runner.
///
/// Both seats run the baseline evaluator under the default exploration
/// schedule; use [`generate_games_with_seats`] to inject evaluator
/// factories or a custom schedule. Each accepted game's actual seed — base
/// or derived — is preserved in the returned [`Game`] and therefore in the
/// record header and corpus manifest. Deduplication covers the full ordered
/// move list, opening placements included, and is scoped to this single run.
///
/// # Errors
/// Returns [`GenerateError::Game`] when the runner fails, or
/// [`GenerateError::DuplicateExhausted`] when a base seed cannot produce a
/// unique game within the attempt budget.
pub fn generate_games<I>(
    base_seeds: I,
    black_options: PlayerOptions,
    white_options: PlayerOptions,
    opening_source: &mut impl OpeningSource,
) -> Result<Vec<Game>, GenerateError>
where
    I: IntoIterator<Item = u64>,
{
    generate_games_with_seats(
        base_seeds,
        black_options,
        white_options,
        ExplorationSchedule::default(),
        opening_source,
        || Box::new(BaselineEvaluator::new(Device::flex())),
        || Box::new(BaselineEvaluator::new(Device::flex())),
    )
}

/// Generate a corpus with explicit seat evaluator factories and schedule.
///
/// The factories build one fresh evaluator per candidate game, mirroring
/// the per-game player construction of
/// [`play_game_with_seats`](crate::game::play_game_with_seats); a factory
/// may simply reload its model from disk. Everything else — seed
/// derivation, Zobrist-walk deduplication, attempt budget — is identical to
/// [`generate_games`].
///
/// # Errors
/// Returns [`GenerateError::Game`] when a candidate game fails, or
/// [`GenerateError::DuplicateExhausted`] when a base seed cannot produce a
/// unique game within the attempt budget.
pub fn generate_games_with_seats<I, B, W>(
    base_seeds: I,
    black_options: PlayerOptions,
    white_options: PlayerOptions,
    schedule: ExplorationSchedule,
    opening_source: &mut impl OpeningSource,
    mut black_factory: B,
    mut white_factory: W,
) -> Result<Vec<Game>, GenerateError>
where
    I: IntoIterator<Item = u64>,
    B: FnMut() -> Box<dyn Evaluator>,
    W: FnMut() -> Box<dyn Evaluator>,
{
    generate_games_with(base_seeds, |seed| {
        play_game_with_seats(
            seed,
            black_options,
            white_options,
            schedule,
            opening_source,
            black_factory(),
            white_factory(),
        )
    })
}
