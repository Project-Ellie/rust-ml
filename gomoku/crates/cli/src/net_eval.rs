//! A trained `net` model behind the `mcts::Evaluator` seam.
//!
//! [`NetEvaluator`] loads a checkpoint written by `nettrain` — a run
//! directory holding `model.mpk` and `run.json` — and evaluates
//! positions for MCTS. The run journal supplies the `ModelConfig`, so
//! a single directory path is enough. This is the minimal precursor of
//! the client model-integration tutorial (the journal-aware model
//! picker), deliberately small.
//!
//! v1 limitation: the `mcts::EvalRequest` seam carries only the two
//! engine planes (me/you). The network's last-move planes (net
//! tutorial chapter 1) are zero-filled, so the model plays without
//! seeing the most recent move.

use std::path::Path;

use burn::tensor::{Device, Tensor, TensorData};
use engine::EXT;
use mcts::{EvalRequest, EvalResult};
use net::checkpoint::{self, CheckpointError};
use net::network::Model;

/// A trained policy/value network driving MCTS priors and values.
pub struct NetEvaluator {
    model: Model,
    device: Device,
}

impl NetEvaluator {
    /// Load the model from a `nettrain` run directory.
    ///
    /// The device is plain Flex CPU: inference only. A non-autodiff
    /// device also selects the batch-norm inference path (Burn 0.22
    /// keys training behavior on `device.is_autodiff()`), and a single
    /// evaluation has batch size 1, which the training path cannot
    /// normalize.
    ///
    /// # Errors
    /// Reports journal/record I/O and JSON errors via
    /// [`CheckpointError`].
    pub fn load(dir: &Path) -> Result<Self, CheckpointError> {
        let journal = checkpoint::load_journal(dir)?;
        let device = Device::flex();
        let model = checkpoint::load_model(dir, &journal.model_config, &device)?;
        Ok(Self::from_model(model))
    }

    /// Wrap an already-loaded model (every AI seat gets its own copy).
    pub fn from_model(model: Model) -> Self {
        Self {
            model,
            device: Device::flex(),
        }
    }

    /// Unwrap into the model (the session stores the model and clones
    /// it into per-seat evaluators).
    pub fn into_model(self) -> Model {
        self.model
    }
}

impl mcts::Evaluator for NetEvaluator {
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
        // NCHW: plane 0 = me, plane 1 = you (engine::encode semantics,
        // identical to net::planes::assemble); planes 2/3 (last moves)
        // stay zero — the EvalRequest seam does not carry history.
        let mut data = vec![0.0f32; 4 * EXT * EXT];
        for (i, (&me, &you)) in req.planes.me.iter().zip(req.planes.you.iter()).enumerate() {
            data[i] = f32::from(me);
            data[EXT * EXT + i] = f32::from(you);
        }
        let input = Tensor::<4>::from_data(
            TensorData::new(data, [1, 4, EXT, EXT]).convert::<f32>(),
            &self.device,
        );
        let output = self.model.forward(input);

        let logits: Vec<f32> = output
            .policy
            .into_data()
            .try_to_vec()
            .expect("policy logits convert to f32");
        let mut policy = [0.0f32; 225];
        policy.copy_from_slice(&logits);
        let value: f32 = output.value.into_scalar();
        EvalResult { policy, value }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Board;
    use mcts::Evaluator as _;
    use net::checkpoint::{RunJournal, save_checkpoint};
    use net::network::ModelConfig;
    use net::report::StopReason;
    use net::train::TrainConfig;
    use std::path::PathBuf;

    /// Write a tiny trained-model run directory (untrained weights).
    pub(crate) fn write_tiny_run(dir: &Path) {
        let device = Device::flex();
        let config = ModelConfig::tiny();
        let model: Model = config.init(&device);
        let journal = RunJournal::new(
            0,
            config,
            TrainConfig::tiny(),
            None,
            None,
            StopReason::Completed,
        );
        save_checkpoint(&model, &journal, dir).unwrap();
    }

    pub(crate) fn scratch_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("cli-net-{tag}-{}-{nanos}", std::process::id()))
    }

    #[test]
    fn load_and_evaluate_empty_board() {
        let dir = scratch_dir("eval");
        write_tiny_run(&dir);

        let mut evaluator = NetEvaluator::load(&dir).unwrap();
        let board = Board::new();
        let result = evaluator.evaluate(EvalRequest {
            planes: engine::encode(&board),
            // The evaluator does not read `legal`; mcts masks.
            legal: Vec::new(),
        });

        assert!(
            result.value.abs() <= 1.0,
            "tanh value, got {}",
            result.value
        );
        assert!(result.policy.iter().all(|v| v.is_finite()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_missing_dir_fails() {
        let dir = scratch_dir("missing");
        assert!(NetEvaluator::load(&dir).is_err());
    }
}
