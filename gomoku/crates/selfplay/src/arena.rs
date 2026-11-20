//! The arena: candidate vs incumbent at an equal simulation budget.
//!
//! Promotion decisions are made here, not by vibes. A candidate plays the
//! incumbent over an even number of games in color-swapped pairs: pair *p*
//! uses base seed `seed_base + p` for both games — same opening — with the
//! candidate as Black in one and as White in the other. No exploration
//! schedule, no temperature: the arena measures strength, it does not
//! generate data. Arena games are not written as a corpus: the seat
//! evaluator flips per game, which the per-corpus pairing provenance cannot
//! represent honestly.
//!
//! The verdict is a Wilson score interval over the candidate's score
//! (win 1, draw ½, loss 0): the candidate is promoted when the interval's
//! lower bound clears 0.5 — strictly better than the incumbent with 95%
//! confidence, not merely ahead on the night.

use engine::Color;
use mcts::Evaluator;
use player::PlayerOptions;
use thiserror::Error;

use crate::game::{GameError, Outcome, play_game_with_seats};
use crate::opening::OpeningSource;
use crate::schedule::ExplorationSchedule;

/// The arena's no-exploration schedule: deterministic selection from the
/// first post-opening ply.
const ARENA_SCHEDULE: ExplorationSchedule = ExplorationSchedule {
    plies: 0,
    temperature: 0.0,
    noise_epsilon: 0.0,
    noise_alpha: 0.3,
};

/// One arena match's setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArenaConfig {
    /// Number of games; must be even so every pair covers both colors.
    pub games: usize,
    /// MCTS simulations per move for both seats — the arena's equal-budget
    /// creed is not negotiable.
    pub sims: u32,
    /// Base seed of pair 0; pair *p* plays seed `seed_base + p` twice.
    pub seed_base: u64,
}

/// Failure to configure an arena.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArenaConfigError {
    /// The game count must be even (color-swapped pairs) and non-zero.
    #[error("an arena needs a positive even game count, got {0}")]
    OddGameCount(usize),
}

/// The candidate's result in one game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameResult {
    /// The candidate won.
    Win,
    /// The candidate lost.
    Loss,
    /// The board filled without a winner.
    Draw,
}

impl GameResult {
    /// The score contribution: win 1, draw ½, loss 0.
    pub fn score(self) -> f64 {
        match self {
            GameResult::Win => 1.0,
            GameResult::Draw => 0.5,
            GameResult::Loss => 0.0,
        }
    }
}

/// One played arena game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArenaGame {
    /// The seed both pair members share.
    pub seed: u64,
    /// The color the candidate played.
    pub candidate_color: Color,
    /// The candidate's result.
    pub result: GameResult,
}

/// The arena tally and verdict.
#[derive(Debug, Clone, PartialEq)]
pub struct ArenaResult {
    /// Every game, in play order.
    pub games: Vec<ArenaGame>,
    /// The candidate's total score (draws count ½).
    pub score: f64,
    /// The Wilson 95% interval for the candidate's score rate.
    pub interval: (f64, f64),
}

impl ArenaResult {
    /// The number of games played.
    pub fn game_count(&self) -> usize {
        self.games.len()
    }

    /// Wins, draws, and losses from the candidate's perspective.
    pub fn tally(&self) -> (usize, usize, usize) {
        let mut wins = 0;
        let mut draws = 0;
        let mut losses = 0;
        for game in &self.games {
            match game.result {
                GameResult::Win => wins += 1,
                GameResult::Draw => draws += 1,
                GameResult::Loss => losses += 1,
            }
        }
        (wins, draws, losses)
    }

    /// The promotion verdict: the Wilson lower bound clears 0.5.
    pub fn promoted(&self) -> bool {
        self.interval.0 > 0.5
    }
}

/// The Wilson score interval for a score rate at 95% confidence.
///
/// `score` sums per-game scores (win 1, draw ½, loss 0). With few games the
/// interval is honestly wide; the promotion rule keys on the lower bound.
#[must_use]
pub fn wilson_interval(score: f64, games: usize) -> (f64, f64) {
    if games == 0 {
        return (0.0, 1.0);
    }
    const Z: f64 = 1.96;
    let n = games as f64;
    let p = score / n;
    let z2 = Z * Z;
    let center = (p + z2 / (2.0 * n)) / (1.0 + z2 / n);
    let half = Z / (1.0 + z2 / n) * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
    let lower = (center - half).clamp(0.0, 1.0);
    let upper = (center + half).clamp(0.0, 1.0);
    (lower, upper)
}

/// Play one arena: color-swapped pairs, no exploration, equal budgets.
///
/// `candidate_factory` and `incumbent_factory` build a fresh evaluator per
/// game, mirroring the self-play generator's per-game construction. The
/// seat options beyond the simulation budget are the caller's base options
/// with the arena schedule applied per ply (which, at zero exploration
/// plies, leaves them untouched).
///
/// # Errors
/// Returns [`ArenaConfigError::OddGameCount`] for an odd or zero game
/// count, or [`GameError`] when a game fails.
pub fn play_arena<C, I>(
    config: ArenaConfig,
    base_options: PlayerOptions,
    opening_source: &mut impl OpeningSource,
    mut candidate_factory: C,
    mut incumbent_factory: I,
) -> Result<ArenaResult, ArenaError>
where
    C: FnMut() -> Box<dyn Evaluator>,
    I: FnMut() -> Box<dyn Evaluator>,
{
    if config.games == 0 || !config.games.is_multiple_of(2) {
        return Err(ArenaConfigError::OddGameCount(config.games).into());
    }

    let options = PlayerOptions {
        simulations: config.sims,
        ..base_options
    };
    let mut games = Vec::with_capacity(config.games);
    let mut score = 0.0;
    for pair in 0..config.games / 2 {
        let seed = config.seed_base + pair as u64;
        for candidate_color in [Color::Black, Color::White] {
            let (black, white) = match candidate_color {
                Color::Black => (candidate_factory(), incumbent_factory()),
                Color::White => (incumbent_factory(), candidate_factory()),
            };
            let game = play_game_with_seats(
                seed,
                options,
                options,
                ARENA_SCHEDULE,
                opening_source,
                black,
                white,
            )?;
            let result = match game.outcome {
                Outcome::Won { winner, .. } if winner == candidate_color => GameResult::Win,
                Outcome::Won { .. } => GameResult::Loss,
                Outcome::Draw => GameResult::Draw,
            };
            score += result.score();
            games.push(ArenaGame {
                seed,
                candidate_color,
                result,
            });
        }
    }

    let interval = wilson_interval(score, games.len());
    Ok(ArenaResult {
        games,
        score,
        interval,
    })
}

/// A failure to configure or play an arena.
#[derive(Debug, Error)]
pub enum ArenaError {
    /// The game count was odd or zero.
    #[error(transparent)]
    Config(#[from] ArenaConfigError),

    /// A game failed to play out.
    #[error(transparent)]
    Game(#[from] GameError),
}
