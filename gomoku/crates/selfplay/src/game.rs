//! game

use crate::opening::{GeneratedOpening, Opening, OpeningSource};
use baseline::{BaselineEvaluator, Device};
use engine::{Color, Move, PlayError, Status};
use player::{AiPlayer, DecisionAction, DecisionError, PlayerOptions, ResignReason};
use rand::{SeedableRng, rngs::StdRng};
use thiserror::Error;

/// Search information retained for one played move.
#[derive(Debug, Clone, PartialEq)]
pub struct PlySearch {
    /// Side whose persepective determines `root_value`'s sign.
    pub side_to_move: Color,
    /// Raw visit counts
    pub visits: Vec<(Move, u32)>,
    /// All root priors after optional Dirichlet noise, in descending order.
    pub prior: Vec<(Move, f32)>,
    /// The searched root's value estimate from the `side_to_move`'s perspective.
    pub root_value: f32,
}

/// One played move and the search data available when it was chosen.
#[derive(Debug, Clone, PartialEq)]
pub struct PlyRecord {
    /// The legal move appended to the game
    pub mv: Move,
    /// Search metadata, or `None` when a certified TSS move bypassed search.
    pub search: Option<PlySearch>,
}

/// Complete in-memory description of on self-play game.
#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    /// Seed of the single RNG stream used for opening an move selection.
    pub seed: u64,
    /// Base options for black, preserved verbatim in the record
    pub black_options: PlayerOptions,
    /// Base options for white, preserved verbatim in the record
    pub white_options: PlayerOptions,
    /// Full Swap2 description and its provenenance
    pub opening: Opening,
    /// Played moves in order, with their available search metadata
    pub plies: Vec<PlyRecord>,
    /// Terminal winner or draw
    pub outcome: Outcome,
}

/// How a game was won
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WinReason {
    /// The winner completed a line af at least (!) five stones.
    FiveOrMore,
    /// The loser voluntarily conceded after an enabled resignation check.
    Resignation(ResignReason),
}

/// Terminal result of a game
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Outcome {
    /// One color won, with the reason recorded separately.
    Won {
        /// The winning color
        winner: Color,
        /// Whether the win came from the board or a concession
        reason: WinReason,
    },
    /// the board filled without q winner.
    Draw,
}

impl Game {
    /// Iterate over the ordered move list, excluding opening stones.
    pub fn moves(&self) -> impl ExactSizeIterator<Item = Move> + '_ {
        self.plies.iter().map(|ply| ply.mv)
    }
}

/// A failure while constructing or playing a game
#[derive(Debug, Error)]
pub enum GameError {
    /// The opning description could not be executed by the engine.
    #[error("Swap2 opening failed: {0}")]
    Opening(#[from] PlayError),

    /// A player was asked to decide on a terminal board
    #[error("player decision failed: {0}")]
    Decision(#[from] DecisionError),

    /// A selected move was rejected by the engine.
    #[error("Move {mv:?} at ply {ply} was rejected: {source}")]
    Play {
        /// zero-based index among post-opening moves.
        ply: usize,
        /// Rejected move
        mv: Move,
        /// Engine legality error
        source: PlayError,
    },
}

/// Play one game using an injectable opening source.
///
/// Tournament-list sources can implement [`OpeningSource`] later without
/// changing the game loop. The source and both players share one seeded
/// [`StdRng`], making the seed the complete identity of the game.
///
/// # Errors
/// Returns an error if the opening, a player decision, or a selected move is
/// rejected by the engine.
pub fn play_game_with_opening(
    seed: u64,
    black_options: PlayerOptions,
    white_options: PlayerOptions,
    opening_source: &mut impl OpeningSource,
) -> Result<Game, GameError> {
    let mut rng = StdRng::seed_from_u64(seed);
    let opening = opening_source.opening(&mut rng);
    let mut board = opening.board()?;
    let mut black = AiPlayer::new(Box::new(BaselineEvaluator::new(Device::flex())));
    let mut white = AiPlayer::new(Box::new(BaselineEvaluator::new(Device::flex())));
    let mut plies = Vec::new();

    let outcome = loop {
        match board.status() {
            Status::Won(winner) => {
                break Outcome::Won {
                    winner,
                    reason: WinReason::FiveOrMore,
                };
            }
            Status::Draw => break Outcome::Draw,
            Status::Ongoing => {}
        }

        let side = board.to_move();

        let (seat, options) = match side {
            Color::Black => (&mut black, &black_options),
            Color::White => (&mut white, &white_options),
        };

        let decision = seat.decide(&board, options, &mut rng)?;
        match decision.action {
            DecisionAction::Play(mv) => {
                // Chapter-1 slot conversion, inline: keep only positive-count
                // visit entries and carry the prior through untruncated.
                // Chapter 3 replaces this with `PlySearch::from` and the
                // recorded top-k prior.
                let search = decision.search.map(|stats| PlySearch {
                    side_to_move: stats.side_to_move,
                    visits: stats.visits.into_iter().filter(|(_, n)| *n > 0).collect(),
                    prior: stats.prior,
                    root_value: stats.value,
                });
                board.play(mv).map_err(|source| GameError::Play {
                    ply: plies.len(),
                    mv,
                    source,
                })?;

                plies.push(PlyRecord { mv, search })
            }
            DecisionAction::Resign(reason) => {
                break Outcome::Won {
                    winner: side.other(),
                    reason: WinReason::Resignation(reason),
                };
            }
        }
    };

    Ok(Game {
        seed,
        black_options,
        white_options,
        opening,
        plies,
        outcome,
    })
}

/// Play one game using a freshly generated seeded Swap2 opening.
///
/// Each color owns a persistent [`AiPlayer`] with its own
/// [`BaselineEvaluator`] on [`Device::flex`]. There is no I/O, threading, or
/// GPU work. The supplied base options are preserved in the returned game;
/// `PlayerOptions::default()` disables resignation for ordinary corpus runs.
///
/// # Errors
/// Returns an error if the generated opening, a player decision, or a selected
/// move is rejected by the engine.
pub fn play_game(
    seed: u64,
    black_options: PlayerOptions,
    white_options: PlayerOptions,
) -> Result<Game, GameError> {
    play_game_with_opening(seed, black_options, white_options, &mut GeneratedOpening)
}
