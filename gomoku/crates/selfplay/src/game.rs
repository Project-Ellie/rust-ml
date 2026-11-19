//! game

use crate::opening::{GeneratedOpening, Opening, OpeningSource};
use crate::record::{CertifiedSuffix, RECORDED_PRIOR_TOP_K};
use crate::schedule::ExplorationSchedule;
use baseline::{BaselineEvaluator, Device};
use engine::{Color, Move, PlayError, Proof, Status};
use player::{AiPlayer, DecisionAction, DecisionError, PlayerOptions, ResignReason, SearchStats};
use rand::{SeedableRng, rngs::StdRng};
use thiserror::Error;

/// Search information retained for one played move.
#[derive(Debug, Clone, PartialEq)]
pub struct PlySearch {
    /// Side whose persepective determines `root_value`'s sign.
    pub side_to_move: Color,
    /// Raw visit counts
    pub visits: Vec<(Move, u32)>,
    /// Top root priors after optional Dirichlet noise, in descending order.
    pub prior: Vec<(Move, f32)>,
    /// The searched root's value estimate from the `side_to_move`'s perspective.
    pub root_value: f32,
}

impl From<SearchStats> for PlySearch {
    fn from(stats: SearchStats) -> Self {
        Self {
            side_to_move: stats.side_to_move,
            visits: stats.visits.into_iter().filter(|(_, n)| *n > 0).collect(),
            prior: stats.prior.into_iter().take(RECORDED_PRIOR_TOP_K).collect(),
            root_value: stats.value,
        }
    }
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
    /// Base options for Black; exploration adjusts selection settings per ply.
    pub black_options: PlayerOptions,
    /// Base options for White; exploration adjusts selection settings per ply.
    pub white_options: PlayerOptions,
    /// Full Swap2 description and its provenenance
    pub opening: Opening,
    /// Played moves in order, with their available search metadata
    pub plies: Vec<PlyRecord>,
    /// Terminal winner or draw
    pub outcome: Outcome,
    /// Latest own-side proof whose line was followed to completion during play.
    pub certified_suffix: Option<CertifiedSuffix>,
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

// The player supplies the exact proof that selected the move. Re-proving here
// could choose a different line and would duplicate the probe's work.

#[derive(Default)]
struct SuffixTracker {
    pending: Option<PendingSuffix>,
    certified: Option<CertifiedSuffix>,
}

struct PendingSuffix {
    suffix: CertifiedSuffix,
    next: usize,
}

impl SuffixTracker {
    fn begin(&mut self, ply: usize, own_proof: Option<Proof>) {
        let Some(proof) = own_proof else {
            return;
        };
        let from_ply = u16::try_from(ply).expect("a game has at most 225 plies");
        self.pending = Some(PendingSuffix {
            suffix: CertifiedSuffix {
                from_ply,
                line: proof.line,
            },
            next: 0,
        });
    }

    fn observe(&mut self, mv: Move) {
        let Some(pending) = self.pending.as_mut() else {
            return;
        };
        if pending.suffix.line.get(pending.next) != Some(&mv) {
            self.pending = None;
            return;
        }
        pending.next += 1;
        if pending.next == pending.suffix.line.len() {
            self.certified = self.pending.take().map(|pending| pending.suffix);
        }
    }

    fn finish(self) -> Option<CertifiedSuffix> {
        self.certified
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
    let mut tracker = SuffixTracker::default();
    let schedule = ExplorationSchedule::default();

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

        let effective_options = schedule.options_for_ply(*options, plies.len());
        let decision = seat.decide(&board, &effective_options, &mut rng)?;
        match decision.action {
            DecisionAction::Play(mv) => {
                tracker.begin(plies.len(), decision.own_proof);
                let search = decision.search.map(PlySearch::from);
                board.play(mv).map_err(|source| GameError::Play {
                    ply: plies.len(),
                    mv,
                    source,
                })?;
                tracker.observe(mv);

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
        certified_suffix: tracker.finish(),
    })
}

/// Play one game using a freshly generated seeded Swap2 opening.
///
/// Each color owns a persistent [`AiPlayer`] with its own
/// [`BaselineEvaluator`] on [`Device::flex`]. There is no I/O, threading, or
/// GPU work. The supplied base options are preserved in the returned game;
/// `PlayerOptions::default()` disables resignation for ordinary corpus runs.
/// The chapter-4 exploration schedule adjusts only selection settings per ply.
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

#[cfg(test)]
mod tests {
    use super::*;
    use baseline::{BaselineEvaluator, Device};
    use engine::Board;
    use player::{AiPlayer, OwnTssConfig, PlayerOptions};
    use rand::{SeedableRng, rngs::StdRng};

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
    fn followed_own_tss_proof_becomes_a_certified_suffix() {
        let mut board = vcf_fixture();
        let mut player = AiPlayer::new(Box::new(BaselineEvaluator::new(Device::flex())));
        let options = PlayerOptions {
            own: OwnTssConfig::baseline(),
            ..PlayerOptions::default()
        };
        let decision = player
            .decide(&board, &options, &mut StdRng::seed_from_u64(8))
            .unwrap();
        let expected_line = decision
            .own_proof
            .as_ref()
            .expect("the VCF fixture has a proof")
            .line
            .clone();

        let mut tracker = SuffixTracker::default();
        tracker.begin(0, decision.own_proof);
        for mv in &expected_line {
            board
                .play(*mv)
                .expect("verified proof moves replay legally");
            tracker.observe(*mv);
        }

        let suffix = tracker.finish().expect("the complete line certifies");
        assert_eq!(suffix.from_ply, 0);
        assert_eq!(suffix.line, expected_line);
    }

    #[test]
    fn departing_from_the_proof_line_leaves_the_suffix_uncertified() {
        let board = vcf_fixture();
        let proof = engine::prove_shortest_forced_win(
            &board,
            board.to_move(),
            OwnTssConfig::baseline().budget,
        )
        .expect("the VCF fixture has a proof");

        let mut tracker = SuffixTracker::default();
        tracker.begin(0, Some(proof));
        tracker.observe(Move::new(14, 14).unwrap());

        assert!(tracker.finish().is_none());
    }
}
