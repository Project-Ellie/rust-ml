# Chapter 06 — Deep-dive solution: reading back + soundness

This is the opt-in reference for Chapter 06. It matches the verified
reference worktree exactly.

## `src/lib.rs`

Register the module and update the crate doc comment:

```rust
//! Synthetic training-data generation for the AlphaZero-style Gomoku agent.
//!
//! This crate stores labelled training positions as stone lists, not as
//! neural-network planes. Planes are derived on read in the `net` crate, and
//! a single position can be augmented with a random D4 transform every time it
//! is loaded. At this slice the crate has no Burn dependency; randomness lives
//! in the `playout` module, file I/O lives in the `shard` module, and the
//! dataset reader lives in the `dataset` module.
//!
//! Design documents (locked):
//!
//! * `docs/12-gomoku-architecture.md` in the rust-ml repository — workspace
//!   layout, serialization decision, and the store-games replay-buffer rule.
//! * `docs/specs/2026-09-27-datagen-tutorial-design.md` — synthetic data
//!   generator design for milestone 3.

pub mod collect;
pub mod dataset;
pub mod label;
pub mod playout;
pub mod sample;
pub mod shard;

pub use sample::Sample;
```

## `src/dataset.rs`

```rust
//! Dataset reader and soundness gate.
//!
//! [`read_dataset`] iterates every shard in a dataset directory and returns
//! the reconstructed [`Sample`] vector. [`check_soundness`] validates that a
//! sample is rules-true: the board rebuilds, policy masses sum to 1, and the
//! policy argmax is a tactical move for win/block positions.

use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::Path;

use crate::label::{TacticalClass, classify};
use crate::sample::Sample;

/// Number of bytes in the length-delimited frame prefix.
///
/// This mirrors the framing written by [`crate::shard::write_dataset`].
const LENGTH_PREFIX_BYTES: usize = 4;

/// Maximum allowed length for a single length-delimited shard record.
///
/// A [`Sample`] bincode payload is tiny; this cap is a guard against a
/// corrupted length prefix forcing a huge allocation. It is intentionally
/// much larger than any legitimate record.
const MAX_RECORD_LEN: usize = 1 << 20; // 1 MiB

/// Soundness violations detected by [`check_soundness`].
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SoundnessError {
    /// The stored stone lists do not reconstruct a legal board.
    #[error("stored stones do not reconstruct a legal board")]
    IllegalPosition,
    /// The policy argmax is not one of the required tactical moves.
    #[error("policy argmax is not a tactical move for this class")]
    ArgmaxNotTactical,
    /// The policy masses do not sum to 1 within tolerance.
    #[error("policy masses do not sum to 1")]
    BadPolicyMass,
    /// The value target does not match the position's tactical class.
    #[error("value target does not match tactical class")]
    InconsistentValue,
}

/// Tolerance for the policy-mass sum check.
///
/// Consistent with the f32 rounding tolerances used in [`crate::label`].
const MASS_TOLERANCE: f32 = 1e-5;

/// Iterate all samples in all shards of a dataset directory, in order.
///
/// Discovers files matching `shard-*.bin`, sorts them lexicographically,
/// and decodes the length-delimited bincode records in each shard. The
/// manifest is not required: the shards are the data; the manifest is
/// provenance.
///
/// # Errors
///
/// Returns `io::Error` if a shard cannot be opened, a length prefix is
/// truncated, or a bincode record fails to decode.
pub fn read_dataset(dir: &Path) -> io::Result<Vec<Sample>> {
    let mut shard_paths: Vec<_> = fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("shard-") && n.ends_with(".bin"))
                    .unwrap_or(false)
        })
        .collect();
    shard_paths.sort_unstable();

    let mut samples = Vec::new();
    for path in shard_paths {
        let file = File::open(&path)?;
        let mut reader = BufReader::new(file);
        read_shard(&mut reader, &mut samples)?;
    }

    Ok(samples)
}

fn read_shard(reader: &mut impl Read, samples: &mut Vec<Sample>) -> io::Result<()> {
    let mut len_buf = [0u8; LENGTH_PREFIX_BYTES];
    loop {
        match reader.read_exact(&mut len_buf) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }

        let len = u32::from_le_bytes(len_buf) as usize;
        if len > MAX_RECORD_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("shard record length {len} exceeds maximum allowed {MAX_RECORD_LEN}"),
            ));
        }
        let mut bytes = vec![0u8; len];
        reader.read_exact(&mut bytes)?;

        let (sample, _): (Sample, usize) =
            bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        samples.push(sample);
    }
    Ok(())
}

/// Engine-truth check for a single sample.
///
/// * Rebuilds the board via [`Sample::board`].
/// * Verifies policy masses sum to 1 within [`MASS_TOLERANCE`].
/// * Classifies the position via [`crate::label::classify`].
/// * Verifies the value target matches the tactical class
///   (`+1.0` win, `-1.0` block, `0.0` quiet).
/// * For [`TacticalClass::Win`], the policy argmax must be in
///   [`engine::immediate_wins`](engine::immediate_wins).
/// * For [`TacticalClass::Block`], the policy argmax must be in
///   [`engine::forced_blocks`](engine::forced_blocks).
/// * [`TacticalClass::Quiet`] has no tactical argmax requirement.
///
/// # Errors
///
/// Returns the first detected [`SoundnessError`].
pub fn check_soundness(sample: &Sample) -> Result<(), SoundnessError> {
    let board = sample
        .board()
        .map_err(|_| SoundnessError::IllegalPosition)?;

    // NaN comparisons are false, so finiteness must be checked explicitly.
    if sample.policy.iter().any(|(_, p)| !p.is_finite()) {
        return Err(SoundnessError::BadPolicyMass);
    }

    let mass_sum: f32 = sample.policy.iter().map(|(_, p)| p).sum();
    if (mass_sum - 1.0).abs() > MASS_TOLERANCE {
        return Err(SoundnessError::BadPolicyMass);
    }

    let class = classify(&board);
    if value_class(sample.value) != Some(class) {
        return Err(SoundnessError::InconsistentValue);
    }
    match class {
        TacticalClass::Win => {
            let wins = engine::immediate_wins(&board, board.to_move());
            let argmax = policy_argmax(&sample.policy);
            if !wins.contains(argmax) {
                return Err(SoundnessError::ArgmaxNotTactical);
            }
        }
        TacticalClass::Block => {
            let blocks = engine::forced_blocks(&board);
            let argmax = policy_argmax(&sample.policy);
            if !blocks.contains(argmax) {
                return Err(SoundnessError::ArgmaxNotTactical);
            }
        }
        TacticalClass::Quiet => {}
    }

    Ok(())
}

/// Return the move with the highest policy mass.
///
/// # Panics
///
/// Panics if `policy` is empty. Callers must ensure at least one policy
/// entry before calling this helper.
fn policy_argmax(policy: &[(engine::Move, f32)]) -> engine::Move {
    policy
        .iter()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(m, _)| *m)
        .expect("policy mass check guarantees at least one entry")
}

/// Map the exact value target to its implied tactical class.
///
/// Returns `None` for any value other than `+1.0`, `-1.0`, or `0.0`.
fn value_class(value: f32) -> Option<TacticalClass> {
    match value {
        1.0 => Some(TacticalClass::Win),
        -1.0 => Some(TacticalClass::Block),
        0.0 => Some(TacticalClass::Quiet),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collect::{Quotas, collect};
    use crate::shard::write_dataset;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let pid = std::process::id();
        std::env::temp_dir().join(format!("{prefix}-{pid}-{nanos}"))
    }

    fn tiny_dataset() -> Vec<Sample> {
        let quotas = Quotas {
            win: 2,
            block: 2,
            quiet: 2,
        };
        collect(quotas, &mut StdRng::seed_from_u64(42))
    }

    #[test]
    fn roundtrip_reads_back_written_dataset() {
        let samples = tiny_dataset();
        let out_dir = unique_temp_dir("train-dataset-roundtrip");
        let _ = fs::remove_dir_all(&out_dir);
        fs::create_dir_all(&out_dir).unwrap();

        let quotas = Quotas {
            win: 2,
            block: 2,
            quiet: 2,
        };
        write_dataset(&samples, &out_dir, 42, quotas).unwrap();

        let read_back = read_dataset(&out_dir).unwrap();
        assert_eq!(
            read_back, samples,
            "read samples must match written samples"
        );

        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn collected_samples_pass_soundness() {
        let samples = tiny_dataset();
        for sample in &samples {
            assert_eq!(
                check_soundness(sample),
                Ok(()),
                "collected sample failed soundness: {:?}",
                sample
            );
        }
    }

    #[test]
    fn win_argmax_outside_tactical_moves_fails() {
        let samples = tiny_dataset();
        let win = samples
            .iter()
            .find(|s| s.value == 1.0)
            .expect("tiny dataset contains a win sample")
            .clone();

        let board = win.board().unwrap();
        let wins = engine::immediate_wins(&board, board.to_move());
        let non_tactical = board
            .empty_moves()
            .find(|m| !wins.contains(*m))
            .expect("win position has at least one non-tactical legal move");

        let mut corrupted = win;
        corrupted.policy = vec![(non_tactical, 1.0)];

        assert_eq!(
            check_soundness(&corrupted),
            Err(SoundnessError::ArgmaxNotTactical)
        );
    }

    #[test]
    fn bad_policy_mass_fails() {
        let samples = tiny_dataset();
        let sample = &samples[0];
        let mut corrupted = sample.clone();
        corrupted.policy = corrupted
            .policy
            .iter()
            .map(|(m, p)| (*m, p * 0.5))
            .collect();

        assert_eq!(
            check_soundness(&corrupted),
            Err(SoundnessError::BadPolicyMass)
        );
    }

    #[test]
    fn non_finite_policy_mass_fails() {
        let samples = tiny_dataset();
        let win = samples
            .iter()
            .find(|s| s.value == 1.0)
            .expect("tiny dataset contains a win sample")
            .clone();

        for bad in [f32::NAN, f32::INFINITY] {
            let mut corrupted = win.clone();
            let mv = corrupted.policy[0].0;
            corrupted.policy = vec![(mv, bad)];

            assert_eq!(
                check_soundness(&corrupted),
                Err(SoundnessError::BadPolicyMass),
                "{bad:?} policy mass must be rejected, not panic"
            );
        }
    }

    #[test]
    fn illegal_position_fails() {
        let samples = tiny_dataset();
        let sample = &samples[0];
        let mut corrupted = sample.clone();

        // Create overlapping colors: push a white stone into the black list.
        if let Some(mv) = corrupted.white.first().copied() {
            corrupted.black.push(mv);
        } else {
            // A quiet-only tiny dataset is vanishingly unlikely; force an overlap
            // by reusing one of the black stones in the white list.
            let mv = corrupted.black.first().copied().expect("sample has stones");
            corrupted.white.push(mv);
        }

        assert_eq!(
            check_soundness(&corrupted),
            Err(SoundnessError::IllegalPosition)
        );
    }

    #[test]
    fn record_length_cap_rejects_invalid_shard() {
        let out_dir = unique_temp_dir("train-dataset-cap");
        let _ = fs::remove_dir_all(&out_dir);
        fs::create_dir_all(&out_dir).unwrap();

        let bad_len = (MAX_RECORD_LEN + 1) as u32;
        fs::write(out_dir.join("shard-000.bin"), bad_len.to_le_bytes()).unwrap();

        let err = read_dataset(&out_dir).expect_err("should reject oversized length prefix");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(
            err.to_string().contains("exceeds maximum allowed"),
            "error should name the cap: {err}"
        );

        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn inconsistent_value_fails() {
        let samples = tiny_dataset();
        let mut corrupted = samples
            .iter()
            .find(|s| s.value == 1.0)
            .expect("tiny dataset contains a win sample")
            .clone();
        corrupted.value = 0.0;

        assert_eq!(
            check_soundness(&corrupted),
            Err(SoundnessError::InconsistentValue)
        );
    }

    #[test]
    fn read_empty_dataset_directory_returns_empty_vector() {
        let out_dir = unique_temp_dir("train-dataset-empty");
        let _ = fs::remove_dir_all(&out_dir);
        fs::create_dir_all(&out_dir).unwrap();

        let samples = read_dataset(&out_dir).unwrap();
        assert!(samples.is_empty());

        let _ = fs::remove_dir_all(&out_dir);
    }
}
```
