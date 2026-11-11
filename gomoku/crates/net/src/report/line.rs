//! Line reporter: timestamped, grep-friendly progress lines.
//!
//! [`LineReporter`] prints one line per throttled optimizer step and one
//! line per holdout evaluation to any [`std::io::Write`] target. Step
//! lines are throttled to at most one per second; eval and finish lines
//! always print. The clock is injected so throttle and ETA logic are
//! unit-testable without sleeping.

use std::io::Write;
use std::time::{Duration, Instant};

#[cfg(test)]
use std::cell::Cell;

use crate::report::{EvalReport, FinishInfo, Reporter, StepMetrics, TrainEvent};

/// Source of [`Instant`] values for reporters.
///
/// A clock seam lets tests advance time deterministically without
/// wall-clock sleeps.
pub trait Clock {
    /// Return the current instant.
    fn now(&self) -> Instant;
}

/// Wall-clock time source.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealClock;

impl Clock for RealClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// A reporter that prints timestamped lines to a writer.
///
/// Step lines are throttled to at most one per second. Eval and finish
/// lines are always printed. Throughput and ETA use the total elapsed
/// time since [`TrainEvent::Started`]; ETA is suppressed for the first
/// 50 steps because early steps are inflated by backend warm-up.
#[derive(Debug)]
pub struct LineReporter<W: Write, C: Clock> {
    writer: W,
    clock: C,
    /// Minimum interval between two printed step lines.
    throttle: Duration,
    /// Instant of the [`TrainEvent::Started`] event, if received.
    start: Option<Instant>,
    /// Instant of the most recent printed step line.
    last_step_line: Option<Instant>,
    /// Total number of steps configured for the run, if known.
    total_steps: Option<usize>,
    /// Exponential moving average of step loss.
    ema: Option<f32>,
    /// Step index of the most recent step event.
    steps_done: usize,
}

impl<W: Write, C: Clock> LineReporter<W, C> {
    /// EMA smoothing factor for the displayed step loss.
    ///
    /// A small alpha keeps the line readable through per-batch noise
    /// while still tracking trend changes within a few dozen steps.
    const EMA_ALPHA: f32 = 0.05;

    /// Number of steps before ETA is considered reliable.
    ///
    /// Early steps overstate throughput because of lazy compilation and
    /// kernel autotuning, so we show a placeholder until enough history
    /// exists.
    const ETA_MIN_STEPS: usize = 50;

    /// Create a new line reporter that writes to `writer` and reads
    /// time from `clock`.
    pub fn new(writer: W, clock: C) -> Self {
        Self {
            writer,
            clock,
            throttle: Duration::from_secs(1),
            start: None,
            last_step_line: None,
            total_steps: None,
            ema: None,
            steps_done: 0,
        }
    }

    /// Change the throttle interval. Used by tests.
    #[cfg(test)]
    fn with_throttle(mut self, throttle: Duration) -> Self {
        self.throttle = throttle;
        self
    }
}

impl<W: Write, C: Clock> Reporter for LineReporter<W, C> {
    fn on_event(&mut self, event: &TrainEvent) {
        match event {
            TrainEvent::Started(info) => {
                self.start = Some(self.clock.now());
                self.total_steps = Some(info.train_config.steps);
            }
            TrainEvent::Step(metrics) => {
                self.steps_done = metrics.step;
                let prev = self.ema.unwrap_or(metrics.loss);
                self.ema =
                    Some(Self::EMA_ALPHA.mul_add(metrics.loss, (1.0 - Self::EMA_ALPHA) * prev));

                let now = self.clock.now();
                let due = self.last_step_line.is_none_or(|last| {
                    now.checked_duration_since(last).unwrap_or(Duration::ZERO) >= self.throttle
                });
                if due {
                    let line = self.format_step_line(metrics, now);
                    if writeln!(self.writer, "{line}").is_ok() {
                        self.last_step_line = Some(now);
                    }
                }
            }
            TrainEvent::Eval(report) => {
                let now = self.clock.now();
                let line = self.format_eval_line(report, now);
                let _ = writeln!(self.writer, "{line}");
            }
            TrainEvent::Finished(info) => {
                let now = self.clock.now();
                let line = self.format_finished_line(info, now);
                let _ = writeln!(self.writer, "{line}");
            }
        }
    }
}

impl<W: Write, C: Clock> LineReporter<W, C> {
    fn elapsed(&self, now: Instant) -> Duration {
        self.start.map_or(Duration::ZERO, |start| {
            now.checked_duration_since(start).unwrap_or(Duration::ZERO)
        })
    }

    fn format_step_line(&self, metrics: &StepMetrics, now: Instant) -> String {
        let elapsed = self.elapsed(now);
        let total = self.total_steps.unwrap_or(metrics.step);
        let pct = if total > 0 {
            metrics.step as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        let elapsed_secs = elapsed.as_secs_f64();
        let steps_per_sec = if elapsed_secs > 0.0 {
            metrics.step as f64 / elapsed_secs
        } else {
            0.0
        };
        let eta = if metrics.step >= Self::ETA_MIN_STEPS && steps_per_sec > 0.0 {
            let remaining = total.saturating_sub(metrics.step);
            Some((remaining as f64 / steps_per_sec) as u64)
        } else {
            None
        };

        format!(
            "[{}] step {}/{} ({:.1}%) loss={:.3} ema={:.3} lr={:.2e} {:.0} steps/s eta={}",
            format_duration_hms(elapsed),
            metrics.step,
            total,
            pct,
            metrics.loss,
            self.ema.unwrap_or(metrics.loss),
            metrics.lr,
            steps_per_sec,
            eta.map_or_else(|| "—".to_string(), format_eta_mm_ss),
        )
    }

    fn format_eval_line(&self, report: &EvalReport, now: Instant) -> String {
        let elapsed = self.elapsed(now);
        let win_block = report.metrics.win_block_policy_top1_accuracy.map_or_else(
            || "win_block=n/a".to_string(),
            |accuracy| {
                format!(
                    "win_block={:.1}% (n={})",
                    accuracy * 100.0,
                    report.metrics.win_block_policy_samples
                )
            },
        );
        format!(
            "[{}] holdout @{} loss={:.3} top1={:.1}% vsign={:.1}% {win_block}",
            format_duration_hms(elapsed),
            report.metrics.step,
            report.metrics.loss,
            report.metrics.policy_top1_accuracy * 100.0,
            report.metrics.value_sign_accuracy * 100.0,
        )
    }

    fn format_finished_line(&self, info: &FinishInfo, now: Instant) -> String {
        let elapsed = self.elapsed(now);
        let reason = match info.stop_reason {
            crate::report::StopReason::Completed => "completed",
            crate::report::StopReason::StopRequested => "stopped",
        };
        format!("[{}] finished: {}", format_duration_hms(elapsed), reason)
    }
}

/// Format a duration as `hh:mm:ss`.
fn format_duration_hms(duration: Duration) -> String {
    let secs = duration.as_secs();
    let hh = secs / 3600;
    let mm = (secs % 3600) / 60;
    let ss = secs % 60;
    format!("{hh:02}:{mm:02}:{ss:02}")
}

/// Format a duration as `mm:ss`, falling back to `hh:mm:ss` for long runs.
fn format_eta_mm_ss(seconds: u64) -> String {
    let hh = seconds / 3600;
    let mm = (seconds % 3600) / 60;
    let ss = seconds % 60;
    if hh > 0 {
        format!("{hh:02}:{mm:02}:{ss:02}")
    } else {
        format!("{mm:02}:{ss:02}")
    }
}

/// Deterministic clock for tests. Time only advances when the test
/// calls [`TestClock::advance`].
#[cfg(test)]
#[derive(Debug)]
pub struct TestClock {
    now: Cell<Instant>,
}

#[cfg(test)]
impl TestClock {
    /// Create a clock starting at the current wall-clock instant.
    pub fn new() -> Self {
        Self {
            now: Cell::new(Instant::now()),
        }
    }

    /// Move the clock backward by `duration`.
    ///
    /// Used to test that reporters saturate elapsed-time arithmetic
    /// instead of panicking when the injected clock regresses.
    pub fn rewind(&self, duration: Duration) {
        self.now.set(
            self.now
                .get()
                .checked_sub(duration)
                .expect("rewind exceeds clock origin"),
        );
    }

    /// Advance the clock by `duration`.
    pub fn advance(&self, duration: Duration) {
        self.now.set(self.now.get() + duration);
    }
}

#[cfg(test)]
impl Clock for TestClock {
    fn now(&self) -> Instant {
        self.now.get()
    }
}

#[cfg(test)]
impl Clock for &TestClock {
    fn now(&self) -> Instant {
        self.now.get()
    }
}

#[cfg(test)]
impl Default for TestClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[derive(Debug, Default, Clone)]
struct SharedWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

#[cfg(test)]
impl SharedWriter {
    fn new() -> Self {
        Self::default()
    }

    fn take(&self) -> Vec<u8> {
        self.0.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{RunInfo, StopReason, TrainConfig};

    fn run_info(steps: usize) -> RunInfo {
        RunInfo {
            model_config: crate::network::ModelConfig::tiny(),
            train_config: TrainConfig {
                steps,
                ..TrainConfig::tiny()
            },
            device_name: "test".to_string(),
            train_samples: 1,
            holdout_samples: 1,
            started_at: "2026-11-11T20:41:07".to_string(),
        }
    }

    fn step(step: usize, loss: f32, lr: f64) -> StepMetrics {
        StepMetrics {
            step,
            loss,
            policy_loss: loss * 0.5,
            value_loss: loss * 0.5,
            lr,
        }
    }

    fn eval(step: usize) -> EvalReport {
        EvalReport {
            metrics: crate::train::EvalMetrics {
                step,
                loss: 0.5,
                policy_top1_accuracy: 0.42,
                value_sign_accuracy: 0.78,
                win_block_policy_top1_accuracy: Some(0.5),
                win_block_policy_samples: 2,
            },
            probes: Vec::new(),
        }
    }

    fn finish(reason: StopReason) -> FinishInfo {
        FinishInfo {
            stop_reason: reason,
        }
    }

    #[test]
    fn step_line_contains_all_key_fields() {
        let clock = TestClock::new();
        let out = SharedWriter::new();
        let mut reporter = LineReporter::new(out.clone(), &clock);

        reporter.on_event(&TrainEvent::Started(run_info(100)));
        clock.advance(Duration::from_secs(10));
        reporter.on_event(&TrainEvent::Step(step(10, 0.693, 2.41e-4)));

        let s = String::from_utf8(out.take()).unwrap();
        assert!(s.contains("step 10/100"), "{s}");
        assert!(s.contains("loss=0.693"), "{s}");
        assert!(s.contains("ema=0.693"), "{s}");
        assert!(s.contains("lr=2.41e-4"), "{s}");
        assert!(s.contains("steps/s"), "{s}");
        assert!(s.contains("eta=—"), "{s}");
    }

    #[test]
    fn eta_appears_after_fifty_steps() {
        let clock = TestClock::new();
        let out = SharedWriter::new();
        let mut reporter = LineReporter::new(out.clone(), &clock).with_throttle(Duration::ZERO);

        reporter.on_event(&TrainEvent::Started(run_info(100)));
        clock.advance(Duration::from_secs(1));

        // 49 steps in one simulated second -> throughput high, but ETA
        // should still be suppressed.
        for i in 1..=49 {
            reporter.on_event(&TrainEvent::Step(step(i, 0.5, 1e-3)));
        }
        let before = String::from_utf8(out.take()).unwrap();
        assert!(before.contains("eta=—"), "{before}");

        reporter.on_event(&TrainEvent::Step(step(50, 0.5, 1e-3)));
        let after = String::from_utf8(out.take()).unwrap();
        assert!(after.contains("eta=00:"), "{after}");
    }

    #[test]
    fn step_lines_are_throttled() {
        let clock = TestClock::new();
        let out = SharedWriter::new();
        let mut reporter = LineReporter::new(out.clone(), &clock);

        reporter.on_event(&TrainEvent::Started(run_info(100)));
        for i in 1..=10 {
            reporter.on_event(&TrainEvent::Step(step(i, 0.5, 1e-3)));
        }
        let lines: Vec<_> = String::from_utf8(out.take())
            .unwrap()
            .lines()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(lines.len(), 1, "expected one throttled line, got {lines:?}");
        // The first step is emitted before the throttle window closes.
        assert!(lines[0].contains("step 1/100"), "{}", lines[0]);
    }

    #[test]
    fn eval_line_contains_win_block_policy_metric() {
        let clock = TestClock::new();
        let out = SharedWriter::new();
        let mut reporter = LineReporter::new(out.clone(), &clock);

        reporter.on_event(&TrainEvent::Started(run_info(100)));
        reporter.on_event(&TrainEvent::Eval(eval(1)));

        let line = String::from_utf8(out.take()).unwrap();
        assert!(line.contains("win_block=50.0% (n=2)"), "{line}");
    }

    #[test]
    fn eval_lines_are_not_throttled() {
        let clock = TestClock::new();
        let out = SharedWriter::new();
        let mut reporter = LineReporter::new(out.clone(), &clock);

        reporter.on_event(&TrainEvent::Started(run_info(100)));
        reporter.on_event(&TrainEvent::Eval(eval(1)));
        reporter.on_event(&TrainEvent::Eval(eval(2)));
        reporter.on_event(&TrainEvent::Eval(eval(3)));

        let binding = String::from_utf8(out.take()).unwrap();
        let lines: Vec<_> = binding.lines().collect();
        assert_eq!(lines.len(), 3, "expected three eval lines, got {lines:?}");
        assert!(lines.iter().all(|l| l.contains("holdout @")), "{lines:?}");
    }

    #[test]
    fn finished_prints_stop_reason() {
        let clock = TestClock::new();
        let out = SharedWriter::new();
        let mut reporter = LineReporter::new(out.clone(), &clock);

        reporter.on_event(&TrainEvent::Started(run_info(10)));
        reporter.on_event(&TrainEvent::Finished(finish(StopReason::Completed)));

        let s = String::from_utf8(out.take()).unwrap();
        assert!(s.contains("finished: completed"), "{s}");

        reporter.on_event(&TrainEvent::Finished(finish(StopReason::StopRequested)));
        let s = String::from_utf8(out.take()).unwrap();
        assert!(s.contains("finished: stopped"), "{s}");
    }

    #[test]
    fn ema_smooths_loss() {
        let clock = TestClock::new();
        let out = SharedWriter::new();
        let mut reporter = LineReporter::new(out.clone(), &clock).with_throttle(Duration::ZERO);

        reporter.on_event(&TrainEvent::Started(run_info(10)));
        for i in 1..=3 {
            reporter.on_event(&TrainEvent::Step(step(i, i as f32, 1e-3)));
        }

        let binding = String::from_utf8(out.take()).unwrap();
        let lines: Vec<_> = binding.lines().map(String::from).collect();
        let last = lines.last().unwrap();
        // EMA with alpha 0.05 stays closer to the first value than the
        // latest raw loss, proving smoothing is applied.
        assert!(
            last.contains("ema=1.147") && !last.contains("ema=3.000"),
            "EMA should smooth; got {last}"
        );
    }

    #[test]
    fn backwards_clock_does_not_panic_and_prints_sane_lines() {
        let clock = TestClock::new();
        let out = SharedWriter::new();
        let mut reporter = LineReporter::new(out.clone(), &clock).with_throttle(Duration::ZERO);

        reporter.on_event(&TrainEvent::Started(run_info(100)));
        clock.advance(Duration::from_secs(10));
        reporter.on_event(&TrainEvent::Step(step(10, 0.693, 2.41e-4)));

        // Simulate an injected clock that regresses. Elapsed-time and
        // throttle arithmetic must saturate to zero rather than panic.
        clock.rewind(Duration::from_secs(5));
        reporter.on_event(&TrainEvent::Step(step(11, 0.690, 2.40e-4)));
        reporter.on_event(&TrainEvent::Eval(eval(11)));
        reporter.on_event(&TrainEvent::Finished(finish(StopReason::Completed)));

        let s = String::from_utf8(out.take()).unwrap();
        assert!(s.contains("step 10/100"), "{s}");
        assert!(s.contains("step 11/100"), "{s}");
        assert!(s.contains("holdout @11"), "{s}");
        assert!(s.contains("finished: completed"), "{s}");
    }
}
