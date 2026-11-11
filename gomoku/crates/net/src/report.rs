//! Reporter seam for the training loop.
//!
//! The loop in [`crate::train`] publishes [`TrainEvent`]s to a
//! [`Reporter`] trait object. Consumers observe progress, render it,
//! request early stops, or write durable streams. The seam itself
//! carries no Burn types so it is backend-agnostic and cheap to copy.

use crate::network::ModelConfig;
use crate::train::{EvalMetrics, StepMetrics, TrainConfig};

pub mod board;
pub mod jsonl;
pub mod line;
pub mod probe;

#[cfg(feature = "tui")]
pub mod tui;

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
    /// Run start timestamp formatted as `YYYY-MM-DDTHH:MM:SS`.
    pub started_at: String,
}

/// Format a [`std::time::SystemTime`] as `YYYY-MM-DDTHH:MM:SS` in UTC.
///
/// This is a small, dependency-free formatter for the run-id header.
/// It handles leap years correctly through 2099 and saturates dates
/// outside that range rather than panicking.
pub fn format_system_time(t: std::time::SystemTime) -> String {
    let dur = t
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO);
    let secs = dur.as_secs();

    // Days since 1970-01-01.
    let days = secs / 86_400;
    let mut year = 1970u64;
    let mut remaining_days = days;
    loop {
        let year_days = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < year_days {
            break;
        }
        remaining_days -= year_days;
        year += 1;
        if year > 2099 {
            // Saturate to avoid overflow in pathological inputs.
            year = 2099;
            remaining_days = if is_leap_year(year) { 365 } else { 364 };
            break;
        }
    }

    let month_lengths = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    let mut day_in_month = remaining_days;
    for (i, &len) in month_lengths.iter().enumerate() {
        if day_in_month < len {
            month = (i as u64) + 1;
            day_in_month += 1; // 1-based
            break;
        }
        day_in_month -= len;
        if i == 11 {
            month = 12;
            day_in_month = len;
        }
    }

    let hh = (secs % 86_400) / 3600;
    let mm = (secs % 3600) / 60;
    let ss = secs % 60;

    format!("{year:04}-{month:02}-{day_in_month:02}T{hh:02}:{mm:02}:{ss:02}")
}

fn is_leap_year(year: u64) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

/// Holdout evaluation report.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EvalReport {
    /// Holdout evaluation metrics.
    pub metrics: EvalMetrics,
    /// Per-probe forward-pass results.
    ///
    /// Empty when the run has no probe boards.
    pub probes: Vec<probe::ProbeResult>,
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

/// A reporter that fans events out to a collection of inner reporters.
///
/// This is the composition root used by `nettrain` to wire the durable
/// JSONL reporter and the display reporter together. `stop_requested`
/// returns `true` if any inner reporter requests a stop.
pub struct CompositeReporter<'a>(pub Vec<&'a mut dyn Reporter>);

impl std::fmt::Debug for CompositeReporter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositeReporter")
            .field("reporters", &self.0.len())
            .finish()
    }
}

impl<'a> CompositeReporter<'a> {
    /// Create an empty composite reporter.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Add a reporter to the fan-out.
    pub fn push(&mut self, reporter: &'a mut dyn Reporter) {
        self.0.push(reporter);
    }
}

impl Default for CompositeReporter<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter for CompositeReporter<'_> {
    fn on_event(&mut self, event: &TrainEvent) {
        for reporter in &mut self.0 {
            reporter.on_event(event);
        }
    }

    fn stop_requested(&self) -> bool {
        self.0.iter().any(|reporter| reporter.stop_requested())
    }
}
