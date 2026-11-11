//! Reporter seam for the training loop.
//!
//! The loop in [`crate::train`] publishes [`TrainEvent`]s to a
//! [`Reporter`] trait object. Consumers observe progress, render it,
//! request early stops, or write durable streams. The seam itself
//! carries no Burn types so it is backend-agnostic and cheap to copy.

use crate::network::ModelConfig;
use crate::train::{EvalMetrics, StepMetrics, TrainConfig};

/// One thing that happened during training.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TrainEvent {
    /// Emitted once before the first step.
    Started(RunInfo),
    /// Emitted after every optimizer step.
    Step(StepMetrics),
    /// Emitted after every holdout evaluation.
    Eval(EvalReport),
    /// Emitted once after the last step or a stop request.
    Finished(FinishInfo),
}

/// Static information about a training run, emitted at the start.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RunInfo {
    /// Network configuration used for the run.
    pub model_config: ModelConfig,
    /// Training-loop configuration.
    pub train_config: TrainConfig,
    /// Compute device name.
    pub device_name: String,
    /// Number of training samples.
    pub train_samples: usize,
    /// Number of holdout samples.
    pub holdout_samples: usize,
}

/// Holdout evaluation report.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EvalReport {
    /// Holdout evaluation metrics.
    pub metrics: EvalMetrics,
}

/// Information about how the training loop finished.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FinishInfo {
    /// Why the loop stopped.
    pub stop_reason: StopReason,
}

/// Reason the training loop stopped.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StopReason {
    /// Ran all configured steps.
    #[default]
    Completed,
    /// Stopped early because the reporter requested it.
    StopRequested,
}

/// A consumer of training events.
pub trait Reporter {
    /// Receive one event.
    fn on_event(&mut self, event: &TrainEvent);

    /// Poll whether the loop should stop early.
    ///
    /// The training loop checks this between steps. The default is
    /// never to request a stop.
    fn stop_requested(&self) -> bool {
        false
    }
}

/// A reporter that does nothing.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct NullReporter;

impl Reporter for NullReporter {
    fn on_event(&mut self, _event: &TrainEvent) {}
}

/// A reporter that records every event it receives.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct RecordingReporter(pub Vec<TrainEvent>);

impl RecordingReporter {
    /// Create an empty recording reporter.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Borrow the recorded events.
    pub fn events(&self) -> &[TrainEvent] {
        &self.0
    }
}

impl Reporter for RecordingReporter {
    fn on_event(&mut self, event: &TrainEvent) {
        self.0.push(event.clone());
    }
}
