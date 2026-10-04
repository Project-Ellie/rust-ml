# Chapter 06 — Deep-dive solution: checkpointing + run journal

This is the opt-in reference for Chapter 06. It matches the verified reference worktree exactly.

No `lib.rs` change is required at this slice; `checkpoint` is registered in the final crate-polish step of Chapter 08.

## `src/checkpoint.rs`

```rust
//! Checkpointing and run journal.
//!
//! Checkpoints are written as:
//!
//! * `model.mpk` — full-precision model record via [`DefaultRecorder`] (I6).
//! * `run.json` — run journal with seed, config, and final metrics.
//!
//! Full precision matters: `CompactRecorder` is f16 and not bit-identical
//! on resume (docs/11).

use std::path::{Path, PathBuf};

use burn::prelude::Module;
use burn::record::{DefaultRecorder, Recorder};
use burn::tensor::backend::Backend;
use serde::{Deserialize, Serialize};

use crate::network::{Model, ModelConfig};
use crate::train::{EvalMetrics, StepMetrics, TrainConfig};

/// Errors that can occur when saving or loading checkpoints.
#[derive(Debug, thiserror::Error)]
pub enum CheckpointError {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Burn record operation failed.
    #[error("Burn record error: {0}")]
    Record(#[from] burn::record::RecorderError),
    /// JSON serialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// A run journal: everything needed to reproduce or resume a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunJournal {
    /// Random seed used for the run.
    pub seed: u64,
    /// Network configuration.
    pub model_config: ModelConfig,
    /// Training-loop configuration.
    pub train_config: TrainConfig,
    /// Final training metrics, if available.
    pub final_step_metrics: Option<StepMetrics>,
    /// Final holdout metrics, if available.
    pub final_eval_metrics: Option<EvalMetrics>,
}

impl RunJournal {
    /// Create a journal from the pieces produced by a training run.
    pub fn new(
        seed: u64,
        model_config: ModelConfig,
        train_config: TrainConfig,
        final_step_metrics: Option<StepMetrics>,
        final_eval_metrics: Option<EvalMetrics>,
    ) -> Self {
        Self {
            seed,
            model_config,
            train_config,
            final_step_metrics,
            final_eval_metrics,
        }
    }
}

/// Save a model record to `dir/model.mpk` using full precision.
pub fn save_model<B: Backend>(model: &Model<B>, dir: &Path) -> Result<PathBuf, CheckpointError> {
    let path = dir.join("model.mpk");
    model
        .clone()
        .save_file(path.clone(), &DefaultRecorder::new())?;
    Ok(path)
}

/// Load a model record from `dir/model.mpk`.
pub fn load_model<B: Backend>(
    dir: &Path,
    config: &ModelConfig,
    device: &B::Device,
) -> Result<Model<B>, CheckpointError> {
    let path = dir.join("model.mpk");
    let record = DefaultRecorder::new().load(path, device)?;
    Ok(config.init::<B>(device).load_record(record))
}

/// Save a run journal to `dir/run.json`.
pub fn save_journal(journal: &RunJournal, dir: &Path) -> Result<PathBuf, CheckpointError> {
    let path = dir.join("run.json");
    let json = serde_json::to_string_pretty(journal)?;
    std::fs::write(&path, json)?;
    Ok(path)
}

/// Load a run journal from `dir/run.json`.
pub fn load_journal(dir: &Path) -> Result<RunJournal, CheckpointError> {
    let path = dir.join("run.json");
    let json = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&json)?)
}

/// Save both model and journal into `dir`.
pub fn save_checkpoint<B: Backend>(
    model: &Model<B>,
    journal: &RunJournal,
    dir: &Path,
) -> Result<(PathBuf, PathBuf), CheckpointError> {
    std::fs::create_dir_all(dir)?;
    let model_path = save_model::<B>(model, dir)?;
    let journal_path = save_journal(journal, dir)?;
    Ok((model_path, journal_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use burn::backend::Flex;
    use burn::backend::flex::FlexDevice;

    type B = Flex;

    fn temp_dir() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let pid = std::process::id();
        std::env::temp_dir().join(format!("net-checkpoint-{pid}-{nanos}.tmp"))
    }

    #[test]
    fn save_load_roundtrip_is_bit_identical() {
        let device = FlexDevice;
        let dir = temp_dir();
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let config = ModelConfig::tiny();
        let model: Model<B> = config.init(&device);
        let input = burn::tensor::Tensor::<B, 4>::zeros([1, 4, engine::EXT, engine::EXT], &device);
        let output_before = model.forward(input.clone());

        save_model(&model, &dir).unwrap();
        let loaded: Model<B> = load_model(&dir, &config, &device).unwrap();
        let output_after = loaded.forward(input);

        let before: Vec<f32> = output_before.policy.into_data().to_vec::<f32>().unwrap();
        let after: Vec<f32> = output_after.policy.into_data().to_vec::<f32>().unwrap();
        assert_eq!(
            before, after,
            "loaded model must produce bit-identical outputs"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn journal_roundtrips_through_json() {
        let dir = temp_dir();
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let journal = RunJournal::new(
            42,
            ModelConfig::tiny(),
            TrainConfig::tiny(),
            Some(StepMetrics {
                step: 10,
                loss: 1.23,
                lr: 1e-4,
            }),
            Some(EvalMetrics {
                step: 10,
                loss: 1.45,
                policy_top1_accuracy: 0.8,
                value_sign_accuracy: 0.85,
            }),
        );

        save_journal(&journal, &dir).unwrap();
        let loaded = load_journal(&dir).unwrap();
        assert_eq!(journal, loaded);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn checkpoint_saves_both_model_and_journal() {
        let device = FlexDevice;
        let dir = temp_dir();
        let _ = std::fs::remove_dir_all(&dir);

        let config = ModelConfig::tiny();
        let model: Model<B> = config.init(&device);
        let journal = RunJournal::new(7, config, TrainConfig::tiny(), None, None);

        save_checkpoint(&model, &journal, &dir).unwrap();

        assert!(dir.join("model.mpk").exists());
        assert!(dir.join("run.json").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}

```
