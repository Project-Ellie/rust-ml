//! Durable JSONL metrics reporter.
//!
//! [`JsonlReporter`] writes every [`TrainEvent`] as one JSON object per
//! line, producing `<out>/metrics.jsonl` alongside the checkpoint. The
//! stream is buffered and flushed on eval and finish events so the
//! file is durable where it matters most, while ordinary step events
//! are cheap.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::report::{Reporter, TrainEvent};

/// A reporter that appends JSON objects, one per line, to a writer.
///
/// All events are serialized. Writes are buffered; the buffer is
/// flushed on [`TrainEvent::Eval`] and [`TrainEvent::Finished`] so
/// holdout results and the final state reach disk promptly.
///
/// Write errors are captured in [`Self::last_error`] and never panic.
/// Once an error has occurred, subsequent events are silently skipped
/// so training is not interrupted by display or disk problems.
#[derive(Debug)]
pub struct JsonlReporter<W: Write> {
    writer: BufWriter<W>,
    last_error: Option<io::Error>,
}

impl JsonlReporter<File> {
    /// Create a JSONL reporter that writes to `path`, creating or
    /// truncating the file.
    ///
    /// Parent directories are **not** created; the caller must ensure
    /// the output directory exists before training starts.
    pub fn create(path: impl AsRef<Path>) -> io::Result<Self> {
        File::create(path).map(Self::new)
    }
}

impl<W: Write> JsonlReporter<W> {
    /// Create a reporter from any writer. Useful in tests.
    pub fn new(writer: W) -> Self {
        Self {
            writer: BufWriter::new(writer),
            last_error: None,
        }
    }

    /// Return the most recent write error, if any.
    ///
    /// Training continues despite errors; callers can surface them
    /// after the loop finishes.
    pub fn last_error(&self) -> Option<&io::Error> {
        self.last_error.as_ref()
    }

    fn write_event(&mut self, event: &TrainEvent) {
        if self.last_error.is_some() {
            return;
        }
        let bytes = match serde_json::to_vec(event) {
            Ok(bytes) => bytes,
            Err(e) => {
                self.last_error = Some(io::Error::other(e));
                return;
            }
        };
        if let Err(e) = self.writer.write_all(&bytes) {
            self.last_error = Some(e);
            return;
        }
        if let Err(e) = self.writer.write_all(b"\n") {
            self.last_error = Some(e);
        }
    }

    fn flush(&mut self) {
        if self.last_error.is_some() {
            return;
        }
        if let Err(e) = self.writer.flush() {
            self.last_error = Some(e);
        }
    }
}

impl<W: Write> Reporter for JsonlReporter<W> {
    fn on_event(&mut self, event: &TrainEvent) {
        self.write_event(event);
        match event {
            // Step events are the hot path: keep them buffered.
            TrainEvent::Step(_) => {}
            // Eval and finish events are rare and worth making durable.
            TrainEvent::Eval(_) | TrainEvent::Finished(_) => self.flush(),
            TrainEvent::Started(_) => {
                // Started is emitted once before the loop begins;
                // flush it so the file header is visible immediately.
                self.flush();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{EvalReport, FinishInfo, RunInfo, StepMetrics, StopReason, TrainConfig};
    use crate::train::EvalMetrics;

    fn run_info(steps: usize) -> RunInfo {
        RunInfo {
            model_config: crate::network::ModelConfig::tiny(),
            train_config: TrainConfig {
                steps,
                ..TrainConfig::tiny()
            },
            device_name: "test".to_string(),
            train_samples: 8,
            holdout_samples: 2,
        }
    }

    fn step(step: usize) -> StepMetrics {
        StepMetrics {
            step,
            loss: 0.5,
            policy_loss: 0.25,
            value_loss: 0.25,
            lr: 1e-3,
        }
    }

    fn eval(step: usize) -> EvalReport {
        EvalReport {
            metrics: EvalMetrics {
                step,
                loss: 0.4,
                policy_top1_accuracy: 0.5,
                value_sign_accuracy: 0.6,
            },
        }
    }

    #[test]
    fn serializes_step_and_eval_events() {
        let mut reporter = JsonlReporter::new(Vec::new());

        reporter.on_event(&TrainEvent::Started(run_info(10)));
        reporter.on_event(&TrainEvent::Step(step(1)));
        reporter.on_event(&TrainEvent::Step(step(2)));
        reporter.on_event(&TrainEvent::Eval(eval(2)));
        reporter.on_event(&TrainEvent::Finished(FinishInfo {
            stop_reason: StopReason::Completed,
        }));

        assert!(reporter.last_error().is_none());
        let inner = reporter.writer.into_inner().unwrap();
        let text = String::from_utf8(inner).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 5, "expected 5 JSONL records, got {lines:?}");

        let values: Vec<serde_json::Value> = lines
            .iter()
            .map(|line| serde_json::from_str(line).expect("valid JSON"))
            .collect();

        assert!(matches!(values[0]["Started"], serde_json::Value::Object(_)));
        assert_eq!(values[1]["Step"]["step"], 1);
        assert_eq!(values[2]["Step"]["step"], 2);
        assert_eq!(values[3]["Eval"]["metrics"]["step"], 2);
        assert_eq!(values[4]["Finished"]["stop_reason"], "Completed");
    }

    #[test]
    fn flushes_on_eval_and_finished() {
        /// A writer that counts `flush` calls.
        #[derive(Debug)]
        struct CountingWriter {
            buf: Vec<u8>,
            flushes: usize,
        }

        impl Write for CountingWriter {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.buf.extend_from_slice(buf);
                Ok(buf.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                self.flushes += 1;
                Ok(())
            }
        }

        let writer = CountingWriter {
            buf: Vec::new(),
            flushes: 0,
        };
        let mut reporter = JsonlReporter::new(writer);

        reporter.on_event(&TrainEvent::Started(run_info(10)));
        reporter.on_event(&TrainEvent::Step(step(1)));
        reporter.on_event(&TrainEvent::Step(step(2)));
        reporter.on_event(&TrainEvent::Eval(eval(2)));
        reporter.on_event(&TrainEvent::Finished(FinishInfo {
            stop_reason: StopReason::Completed,
        }));

        let writer = reporter.writer.into_inner().unwrap();
        // Started, Eval, and Finished each flush; the two Steps do not.
        assert_eq!(
            writer.flushes, 3,
            "expected flushes on Started, Eval, Finished"
        );
    }
}
