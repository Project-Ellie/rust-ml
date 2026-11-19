//! Extract milestone-4 anchor seeds from verified self-play suffixes.
//!
//! Every stored [`CertifiedSuffix`] is a positive claim: the recorded line is a
//! forced win from the certified position and was actually played. The
//! extraction pass replays the source game through the engine and re-applies
//! [`verify_line`] before emitting a seed; one failure aborts the whole pass.

use engine::{Move, PlayError, Proof, verify_line};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::record::GameRecord;

/// Replay instructions for one milestone-4 anchor position.
///
/// Replay the source game's opening and its first `from_ply` post-opening
/// moves; at that position `line[0]` is the forcing move and the verified
/// value is +1 for the side to move (−1 for its opponent). Storing derivation
/// information rather than planes keeps the seed independent of plane
/// encoding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorSeed {
    /// Zero-based index of the source game in corpus order.
    pub game_index: usize,
    /// Zero-based post-opening ply at which the certified position occurs.
    pub from_ply: u16,
    /// Complete verified forced-win line whose first move is the forcing move.
    pub line: Vec<Move>,
}

/// Why a certified suffix could not be turned into an anchor seed.
#[derive(Debug, Error)]
pub enum AnchorError {
    /// The source game's Swap2 opening was invalid.
    #[error("game {game_index}: opening replay failed: {source}")]
    Opening {
        /// Index of the source game in corpus order.
        game_index: usize,
        /// Underlying engine error.
        #[source]
        source: PlayError,
    },

    /// A source move before the certified position was illegal.
    #[error("game {game_index}: move {mv:?} at ply {ply} was rejected: {source}")]
    Replay {
        /// Index of the source game in corpus order.
        game_index: usize,
        /// Zero-based post-opening ply that was rejected.
        ply: usize,
        /// Rejected move.
        mv: Move,
        /// Underlying engine error.
        #[source]
        source: PlayError,
    },

    /// The certificate does not identify a played position.
    #[error("game {game_index}: certified from_ply {from_ply} is outside {ply_count} plies")]
    SuffixOutOfBounds {
        /// Index of the source game in corpus order.
        game_index: usize,
        /// Invalid start ply stored in the certificate.
        from_ply: u16,
        /// Number of post-opening moves available for replay.
        ply_count: usize,
    },

    /// The stored line is not a prefix of the moves actually played.
    #[error("game {game_index}: certified line from ply {from_ply} was not played")]
    SuffixNotPlayed {
        /// Index of the source game in corpus order.
        game_index: usize,
        /// Start ply of the rejected certificate.
        from_ply: u16,
    },

    /// The engine rejected a stored certificate.
    #[error("game {game_index}: certified line from ply {from_ply} failed verify_line")]
    VerificationFailed {
        /// Index of the source game in corpus order.
        game_index: usize,
        /// Start ply of the rejected certificate.
        from_ply: u16,
    },
}

/// Extract anchor seeds from a slice of game records.
///
/// Every stored certificate is a positive claim and must pass
/// [`verify_line`]; one failure aborts the entire extraction. Records without
/// certificates contribute no seeds and, crucially, no negative labels.
///
/// # Errors
/// Returns the first opening, replay, bounds, or line-verification failure.
pub fn extract_anchor_seeds(records: &[GameRecord]) -> Result<Vec<AnchorSeed>, AnchorError> {
    let mut seeds = Vec::new();

    for (game_index, record) in records.iter().enumerate() {
        let Some(suffix) = &record.certified_suffix else {
            continue;
        };

        let from_ply = usize::from(suffix.from_ply);
        if from_ply >= record.plies.len() {
            return Err(AnchorError::SuffixOutOfBounds {
                game_index,
                from_ply: suffix.from_ply,
                ply_count: record.plies.len(),
            });
        }

        let mut board = record
            .header
            .opening
            .board()
            .map_err(|source| AnchorError::Opening { game_index, source })?;

        for (ply, recorded) in record.plies.iter().take(from_ply).enumerate() {
            board
                .play(recorded.mv)
                .map_err(|source| AnchorError::Replay {
                    game_index,
                    ply,
                    mv: recorded.mv,
                    source,
                })?;
        }

        let proof = Proof {
            winner: board.to_move(),
            line: suffix.line.clone(),
        };
        if !verify_line(&board, &proof) {
            return Err(AnchorError::VerificationFailed {
                game_index,
                from_ply: suffix.from_ply,
            });
        }

        let played_prefix: Vec<_> = record
            .plies
            .iter()
            .skip(from_ply)
            .take(suffix.line.len())
            .map(|recorded| recorded.mv)
            .collect();
        if played_prefix != suffix.line {
            return Err(AnchorError::SuffixNotPlayed {
                game_index,
                from_ply: suffix.from_ply,
            });
        }

        seeds.push(AnchorSeed {
            game_index,
            from_ply: suffix.from_ply,
            line: suffix.line.clone(),
        });
    }

    Ok(seeds)
}
