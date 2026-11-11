//! Crossterm terminal driver for the board reporter.
//!
//! This module is gated behind the `tui` cargo feature. It wraps the
//! pure frame composer [`BoardReporter`](crate::report::board::BoardReporter)
//! with raw-mode terminal I/O on **stderr**, leaving stdout clean for the
//! final report. The alternate screen is entered once when the reporter is
//! constructed and restored automatically on drop, even if the program
//! panics.
//!
//! The driver is intentionally thin: the composer decides what a frame
//! looks like, and this module only decides when to redraw and how to map
//! keys to actions. All timing decisions use the injected [`Clock`] seam so
//! they can be unit-tested without a terminal.
//!
//! # Cargo feature
//!
//! The module only exists when the `tui` feature is enabled. Building with
//! `--no-default-features` removes the crossterm dependency entirely.
//!
//! # Monochrome
//!
//! The reporter starts in monochrome mode if either `NO_COLOR` or
//! `NETTRAIN_MONO` is set in the environment. This degrades the progress
//! bar to block glyphs and avoids relying on terminal color support.

use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::style::Print;
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};

use crate::report::board::{BoardFlags, BoardReporter};
use crate::report::line::Clock;
use crate::report::probe::ProbePosition;
use crate::report::{Reporter, TrainEvent};

/// Minimum time between two ordinary step-driven redraws.
const REDRAW_INTERVAL: Duration = Duration::from_millis(100);

/// Progress milestones, in percent, that trigger a one-frame flash.
const FLASH_MILESTONE: usize = 5;

/// A full-screen terminal reporter driven by crossterm.
///
/// Owns the pure [`BoardReporter`] composer, a redraw throttle, and the
/// terminal state guard. Construction enters raw mode and the alternate
/// screen; dropping the reporter restores them.
pub struct TuiReporter<C: Clock> {
    board: BoardReporter<C>,
    last_draw: Option<Instant>,
    pending_flash: bool,
    last_milestone: usize,
    /// When true, terminal I/O is skipped. Used only by tests that need
    /// to exercise the throttle/flash logic headlessly.
    headless: bool,
}

impl<C: Clock> TuiReporter<C> {
    /// Create a new terminal reporter and enter TUI mode on stderr.
    ///
    /// Fails if the terminal does not support raw mode or the alternate
    /// screen. On success the caller can pass the reporter to the training
    /// loop; on failure the caller should fall back to a line reporter.
    pub fn new(clock: C) -> io::Result<Self> {
        let flags = initial_flags();
        let board = BoardReporter::new(clock).with_flags(flags);
        enter_terminal_mode()?;
        Ok(Self {
            board,
            last_draw: None,
            pending_flash: false,
            last_milestone: 0,
            headless: false,
        })
    }

    /// Provide the fixed probe positions used for this run.
    pub fn set_probe_positions(&mut self, positions: Vec<ProbePosition>) {
        self.board.set_probe_positions(positions);
    }

    /// Handle one training event: update state, redraw if due, and poll
    /// for non-blocking key input.
    fn handle_event(&mut self, event: &TrainEvent) {
        self.board.on_event(event);
        self.update_flash(event);

        let now = self.board.clock_now();
        let (should_draw, flash) = match event {
            // Always redraw on eval and finish so important state is never
            // left on the throttle floor.
            TrainEvent::Eval(_) | TrainEvent::Finished(_) => (true, self.pending_flash),
            TrainEvent::Started(_) => (true, false),
            TrainEvent::Step(_) => {
                if self.pending_flash {
                    (true, true)
                } else {
                    let due = self
                        .last_draw
                        .is_none_or(|last| now.saturating_duration_since(last) >= REDRAW_INTERVAL);
                    (due, false)
                }
            }
        };

        if should_draw {
            if let Err(e) = self.draw(flash) {
                // A terminal write failure is not fatal to training.
                // Swallow the error and keep going: the next event will
                // attempt to redraw and poll input again.
                let _ = e;
                self.pending_flash = false;
                return;
            }
            self.last_draw = Some(now);
            self.pending_flash = false;
        }

        if !matches!(event, TrainEvent::Finished(_)) {
            self.poll_keys();
        }
    }

    /// Paint one frame to the alternate screen.
    fn draw(&mut self, flash: bool) -> io::Result<()> {
        if self.headless {
            return Ok(());
        }
        let frame = self.board.render(flash);
        let mut out = io::stderr();
        queue!(out, Clear(ClearType::All))?;
        for (y, line) in frame.iter().enumerate() {
            queue!(out, MoveTo(0, y as u16), Print(line))?;
        }
        out.flush()
    }

    /// Poll available key events without blocking and apply their actions.
    fn poll_keys(&mut self) {
        if self.headless {
            return;
        }
        while let Ok(true) = event::poll(Duration::ZERO) {
            match event::read() {
                Ok(Event::Key(key)) => self.apply_key(key),
                Err(_) => break,
                _ => {}
            }
        }
    }

    /// Apply a single key event, if it maps to a known action.
    fn apply_key(&mut self, key: KeyEvent) {
        // Ignore key releases and repeats; only presses matter.
        if key.kind != KeyEventKind::Press {
            return;
        }
        match translate_key(&key) {
            Some(Action::Stop) => self.board.request_stop(),
            Some(Action::ToggleLogScale) => self.board.flags_mut().log_scale ^= true,
            Some(Action::ToggleShowProbes) => self.board.flags_mut().show_probes ^= true,
            None => {}
        }
    }

    /// Update the pending-flash flag when progress crosses a milestone.
    fn update_flash(&mut self, event: &TrainEvent) {
        let TrainEvent::Step(metrics) = event else {
            return;
        };
        let total = self.board.total_steps().max(1);
        let step = metrics.step.min(total);
        let pct = step * 100 / total;
        let milestone = pct - (pct % FLASH_MILESTONE);
        if milestone > self.last_milestone && milestone > 0 {
            self.pending_flash = true;
        }
        self.last_milestone = self.last_milestone.max(milestone);
    }
}

impl<C: Clock> Reporter for TuiReporter<C> {
    fn on_event(&mut self, event: &TrainEvent) {
        self.handle_event(event);
    }

    fn stop_requested(&self) -> bool {
        self.board.stop_requested()
    }
}

impl<C: Clock> Drop for TuiReporter<C> {
    fn drop(&mut self) {
        if !self.headless {
            let _ = leave_terminal_mode();
        }
    }
}

/// Compute the initial display flags from the environment.
fn initial_flags() -> BoardFlags {
    BoardFlags {
        monochrome: std::env::var("NO_COLOR").is_ok() || std::env::var("NETTRAIN_MONO").is_ok(),
        ..BoardFlags::default()
    }
}

/// Enter raw mode and the alternate screen on stderr, hiding the cursor.
fn enter_terminal_mode() -> io::Result<()> {
    terminal::enable_raw_mode()?;

    // If any subsequent setup step fails, the process must not be left in
    // raw mode (or with the cursor hidden). This guard restores the
    // terminal on drop; it is disarmed once setup completes successfully.
    struct TerminalGuard;
    impl Drop for TerminalGuard {
        fn drop(&mut self) {
            let _ = leave_terminal_mode();
        }
    }
    let guard = TerminalGuard;

    execute!(io::stderr(), EnterAlternateScreen, Hide)?;
    std::mem::forget(guard);
    Ok(())
}

/// Restore the cursor, leave the alternate screen, and disable raw mode.
fn leave_terminal_mode() -> io::Result<()> {
    leave_terminal_mode_with(
        || execute!(io::stderr(), Show, LeaveAlternateScreen),
        terminal::disable_raw_mode,
    )
}

fn leave_terminal_mode_with(
    restore_screen: impl FnOnce() -> io::Result<()>,
    disable_raw_mode: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    let screen_result = restore_screen();
    let raw_mode_result = disable_raw_mode();
    screen_result.and(raw_mode_result)
}

/// A key action understood by the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// Request an early stop.
    Stop,
    /// Toggle the log/linear sparkline scale.
    ToggleLogScale,
    /// Toggle the probe panel.
    ToggleShowProbes,
}

/// Translate a crossterm key event into an action.
///
/// Kept as a pure function at the edge of the module so the mapping is
/// unit-testable without a terminal.
fn translate_key(key: &KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => Some(Action::Stop),
        KeyCode::Char('l') | KeyCode::Char('L') => Some(Action::ToggleLogScale),
        KeyCode::Char('p') | KeyCode::Char('P') => Some(Action::ToggleShowProbes),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::line::TestClock;
    use crate::report::{EvalMetrics, EvalReport, RunInfo, StepMetrics, TrainConfig};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::time::Duration;

    fn key_char(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

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

    fn step(step: usize, loss: f32) -> StepMetrics {
        StepMetrics {
            step,
            loss,
            policy_loss: loss * 0.5,
            value_loss: loss * 0.5,
            lr: 1e-3,
        }
    }

    #[test]
    fn terminal_restoration_attempts_raw_mode_after_screen_error() {
        let raw_mode_attempted = std::cell::Cell::new(false);

        let error = leave_terminal_mode_with(
            || Err(io::Error::new(io::ErrorKind::BrokenPipe, "screen")),
            || {
                raw_mode_attempted.set(true);
                Err(io::Error::other("raw mode"))
            },
        )
        .unwrap_err();

        assert!(raw_mode_attempted.get());
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(error.to_string(), "screen");
    }

    #[test]
    fn key_mapping_translates_controls() {
        assert_eq!(translate_key(&key_char('q')), Some(Action::Stop));
        assert_eq!(translate_key(&key_char('Q')), Some(Action::Stop));
        assert_eq!(translate_key(&key_char('l')), Some(Action::ToggleLogScale));
        assert_eq!(translate_key(&key_char('L')), Some(Action::ToggleLogScale));
        assert_eq!(
            translate_key(&key_char('p')),
            Some(Action::ToggleShowProbes)
        );
        assert_eq!(
            translate_key(&key_char('P')),
            Some(Action::ToggleShowProbes)
        );
    }

    #[test]
    fn key_mapping_ignores_unmapped_keys() {
        assert_eq!(translate_key(&key_char('x')), None);
        assert_eq!(
            translate_key(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            None
        );
        assert_eq!(
            translate_key(&KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            None
        );
    }

    #[test]
    fn throttle_collapses_fast_steps() {
        let clock = TestClock::new();
        // Headless: bypass terminal mode and writes while exercising the
        // throttle/flash decisions. Use many total steps so the early
        // steps do not cross a 5% flash milestone.
        let mut reporter = TuiReporter {
            board: BoardReporter::new(&clock),
            last_draw: None,
            pending_flash: false,
            last_milestone: 0,
            headless: true,
        };

        reporter.handle_event(&TrainEvent::Started(run_info(1000)));
        assert!(reporter.last_draw.is_some(), "Started redraws immediately");

        let first_draw = reporter.last_draw;
        // Nine steps at 10ms each keep the elapsed time strictly below
        // the 100ms throttle window.
        for i in 1..=9 {
            clock.advance(Duration::from_millis(10));
            reporter.handle_event(&TrainEvent::Step(step(i, 0.5)));
        }
        assert_eq!(
            reporter.last_draw, first_draw,
            "steps faster than 100ms should collapse to one redraw"
        );

        clock.advance(Duration::from_millis(100));
        reporter.handle_event(&TrainEvent::Step(step(10, 0.5)));
        assert!(
            reporter.last_draw.unwrap() > first_draw.unwrap(),
            "step after the throttle window should redraw"
        );
    }

    #[test]
    fn eval_always_redraws() {
        let clock = TestClock::new();
        let mut reporter = TuiReporter {
            board: BoardReporter::new(&clock),
            last_draw: None,
            pending_flash: false,
            last_milestone: 0,
            headless: true,
        };
        reporter.handle_event(&TrainEvent::Started(run_info(100)));
        let last = reporter.last_draw.unwrap();

        clock.advance(Duration::from_millis(10));
        reporter.handle_event(&TrainEvent::Step(step(1, 0.5)));
        assert_eq!(
            reporter.last_draw.unwrap(),
            last,
            "step inside window does not redraw"
        );

        let eval_report = EvalReport {
            metrics: EvalMetrics {
                step: 1,
                loss: 0.4,
                policy_top1_accuracy: 0.5,
                value_sign_accuracy: 0.6,
                win_block_policy_top1_accuracy: Some(0.5),
                win_block_policy_samples: 1,
            },
            probes: Vec::new(),
        };
        reporter.handle_event(&TrainEvent::Eval(eval_report));
        assert!(
            reporter.last_draw.unwrap() > last,
            "eval must redraw even inside the throttle window"
        );
    }

    #[test]
    fn finished_always_redraws() {
        let clock = TestClock::new();
        let mut reporter = TuiReporter {
            board: BoardReporter::new(&clock),
            last_draw: None,
            pending_flash: false,
            last_milestone: 0,
            headless: true,
        };
        reporter.handle_event(&TrainEvent::Started(run_info(100)));
        reporter.handle_event(&TrainEvent::Step(step(1, 0.5)));
        let last = reporter.last_draw.unwrap();

        clock.advance(Duration::from_millis(10));
        reporter.handle_event(&TrainEvent::Finished(crate::report::FinishInfo {
            stop_reason: crate::report::StopReason::Completed,
        }));
        assert!(
            reporter.last_draw.unwrap() > last,
            "Finished must redraw even inside the throttle window"
        );
    }

    #[test]
    fn flash_fires_once_per_milestone() {
        let clock = TestClock::new();
        let mut reporter = TuiReporter {
            board: BoardReporter::new(&clock),
            last_draw: None,
            pending_flash: false,
            last_milestone: 0,
            headless: true,
        };
        reporter.handle_event(&TrainEvent::Started(run_info(100)));

        // Steps 1-4 are below the first 5% milestone.
        for i in 1..=4 {
            reporter.handle_event(&TrainEvent::Step(step(i, 0.5)));
        }
        assert!(!reporter.pending_flash, "below 5% there is no flash");
        assert_eq!(reporter.last_milestone, 0);

        // Step 5 crosses 5% and the immediate redraw consumes the flash.
        reporter.handle_event(&TrainEvent::Step(step(5, 0.5)));
        assert_eq!(reporter.last_milestone, 5, "milestone should advance");

        // Staying in the same milestone does not flash again.
        reporter.handle_event(&TrainEvent::Step(step(6, 0.5)));
        assert!(!reporter.pending_flash, "same milestone should not reflash");

        // Crossing 10% flashes once.
        reporter.handle_event(&TrainEvent::Step(step(10, 0.5)));
        assert_eq!(reporter.last_milestone, 10);
    }

    #[test]
    fn stop_key_requests_stop() {
        let clock = TestClock::new();
        let mut reporter = TuiReporter {
            board: BoardReporter::new(&clock),
            last_draw: None,
            pending_flash: false,
            last_milestone: 0,
            headless: true,
        };
        assert!(!reporter.stop_requested());
        reporter.apply_key(key_char('q'));
        assert!(reporter.stop_requested());
    }

    #[test]
    fn toggle_keys_flip_flags() {
        let clock = TestClock::new();
        let mut reporter = TuiReporter {
            board: BoardReporter::new(&clock),
            last_draw: None,
            pending_flash: false,
            last_milestone: 0,
            headless: true,
        };
        assert!(reporter.board.flags().log_scale);
        reporter.apply_key(key_char('l'));
        assert!(!reporter.board.flags().log_scale);

        assert!(reporter.board.flags().show_probes);
        reporter.apply_key(key_char('p'));
        assert!(!reporter.board.flags().show_probes);
    }

    #[test]
    fn ignored_keys_do_nothing() {
        let clock = TestClock::new();
        let mut reporter = TuiReporter {
            board: BoardReporter::new(&clock),
            last_draw: None,
            pending_flash: false,
            last_milestone: 0,
            headless: true,
        };
        let before = *reporter.board.flags();
        reporter.apply_key(key_char('x'));
        assert_eq!(reporter.board.flags(), &before);
    }
}
