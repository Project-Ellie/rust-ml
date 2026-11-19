//! Replay a durable [`GameRecord`] into a training [`Sample`].
//!
//! Replay lives in `selfplay`, not `train`, because replay owns the
//! [`GameRecord`] and opening vocabulary, and the dependency already points
//! `selfplay → train` for storage, so `train` stays the shared sample and
//! shard-format crate without learning about self-play records.

use engine::{Board, Color, Move, PlayError, Transform};
use thiserror::Error;
use train::Sample;

use crate::game::Outcome;
use crate::record::GameRecord;

/// Failure while replaying a recorded game into a training sample.
#[derive(Debug, Error)]
pub enum SampleError {
    /// The requested post-opening ply does not exist.
    #[error("ply {ply} is outside the record's {ply_count} plies")]
    PlyOutOfBounds {
        /// The requested zero-based ply.
        ply: usize,
        /// The number of post-opening plies in the record.
        ply_count: usize,
    },

    /// A nonempty visit histogram contains no actual visits.
    #[error("visit histogram at ply {ply} has zero total visits")]
    ZeroVisitTotal {
        /// The ply whose histogram is corrupt.
        ply: usize,
    },

    /// The record's Swap2 opening was invalid.
    #[error("opening replay failed: {0}")]
    Opening(#[source] PlayError),

    /// A recorded move was rejected during replay.
    #[error("move {mv:?} at ply {ply} was rejected: {source}")]
    Replay {
        /// The zero-based ply of the rejected move.
        ply: usize,
        /// The rejected move.
        mv: Move,
        /// The engine legality error.
        #[source]
        source: PlayError,
    },
}

/// Replay a recorded game up to `ply` and produce a canonical training sample.
///
/// The returned sample describes the position *immediately before* `ply` is
/// played. It is in canonical orientation, with both targets derived from the
/// record at this same ply:
///
/// * π is the normalized root visit-count distribution at `ply`; when search
///   did not run, the policy is a one-hot on the played move.
/// * z is the recorded game outcome from the perspective of the side to move
///   in the replayed position.
///
/// The recorded `prior` and `root_value` are audit data and are not used here.
pub fn replay_sample(record: &GameRecord, ply: usize) -> Result<Sample, SampleError> {
    let board = replay_board(record, ply)?;

    let recorded = &record.plies[ply];

    // Derive π from the recorded visits.
    let policy = if recorded.visits.is_empty() {
        // A certified TSS move bypasses search, so there is no visit histogram
        // to normalize; its deterministic one-hot policy is deliberately
        // behavior cloning for that ply; the game-outcome value target is
        // unchanged.
        vec![(recorded.mv, 1.0)]
    } else {
        let total: u64 = recorded.visits.iter().map(|(_, n)| u64::from(*n)).sum();
        if total == 0 {
            return Err(SampleError::ZeroVisitTotal { ply });
        }
        let total_f = total as f32;
        recorded
            .visits
            .iter()
            .map(|(mv, n)| (*mv, *n as f32 / total_f))
            .collect()
    };

    // Derive z from the outcome and the side to move.
    let value = value_from_outcome(record.outcome, board.to_move());

    Ok(Sample {
        black: stones(&board, Color::Black),
        white: stones(&board, Color::White),
        to_move: board.to_move(),
        policy,
        value,
    })
}

/// Apply one D4 transform to a canonical sample.
///
/// Rotations and reflections preserve wins, losses, draws, and turn order, so
/// both stone lists and every policy move are transformed together while
/// `to_move` and `value` are carried unchanged. The function operates on stone
/// lists; the 17×17 border is still added later by `net`.
#[must_use]
pub fn augment_sample(sample: &Sample, transform: Transform) -> Sample {
    Sample {
        black: sample
            .black
            .iter()
            .map(|mv| transform.transform_move(*mv))
            .collect(),
        white: sample
            .white
            .iter()
            .map(|mv| transform.transform_move(*mv))
            .collect(),
        to_move: sample.to_move,
        policy: sample
            .policy
            .iter()
            .map(|(mv, p)| (transform.transform_move(*mv), *p))
            .collect(),
        value: sample.value,
    }
}

/// Whether the canonical position at `ply` belongs to the holdout split.
///
/// The rule deliberately evaluates the canonical position before augmentation,
/// matching `nettrain`, because Zobrist keys are orientation-specific and a
/// random D4 orientation must not move a position between training and holdout.
/// Callers that already own a canonical [`Sample`] call
/// [`train::split::is_holdout`] directly.
pub fn position_is_holdout(record: &GameRecord, ply: usize) -> Result<bool, SampleError> {
    let sample = replay_sample(record, ply)?;
    Ok(train::split::is_holdout(&sample))
}

fn value_from_outcome(outcome: Outcome, side_to_move: Color) -> f32 {
    match outcome {
        Outcome::Draw => 0.0,
        Outcome::Won { winner, .. } => {
            if winner == side_to_move {
                1.0
            } else {
                -1.0
            }
        }
    }
}

fn replay_board(record: &GameRecord, ply: usize) -> Result<Board, SampleError> {
    let ply_count = record.plies.len();
    if ply >= ply_count {
        return Err(SampleError::PlyOutOfBounds { ply, ply_count });
    }

    let mut board = record
        .header
        .opening
        .board()
        .map_err(SampleError::Opening)?;

    for (index, recorded) in record.plies.iter().take(ply).enumerate() {
        board
            .play(recorded.mv)
            .map_err(|source| SampleError::Replay {
                ply: index,
                mv: recorded.mv,
                source,
            })?;
    }

    Ok(board)
}

fn stones(board: &Board, color: Color) -> Vec<Move> {
    let mut out = Vec::new();
    for index in 0..225u8 {
        let mv = Move::new(index / 15, index % 15).expect("index is on the board");
        if board.stone_at(mv) == Some(color) {
            out.push(mv);
        }
    }
    out
}
