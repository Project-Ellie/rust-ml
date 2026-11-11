//! Board reporter: pure frame composer for the full-screen training monitor.
//!
//! This module implements the rendering core described in
//! `docs/design/2026-11-11-nettrain-progress-monitor.md` §7. It consumes
//! [`TrainEvent`]s, maintains ring buffers, and composes each frame as a
//! [`Vec<String>`]. It performs no terminal I/O and has no crossterm
//! dependency; the driver in sub-slice 3c prints what this composer returns.
//!
//! The composer is intentionally stateful: the same [`BoardReporter`] folds
//! every event it receives into running buffers, so a frame can be requested
//! at any time without re-playing history.

use std::time::{Duration, Instant};

use engine::{Board, Color, Move};

use crate::report::line::Clock;
use crate::report::probe::{ProbeClass, ProbePosition, ProbeResult};
use crate::report::{EvalReport, Reporter, RunInfo, StepMetrics, TrainEvent};

/// Number of cells in the progress bar.
const PROGRESS_BAR_CELLS: usize = 60;

/// Number of recent step durations kept for ETA/throughput.
const STEP_TIME_WINDOW: usize = 100;

/// ETA is suppressed until this many steps of history exist.
const ETA_MIN_STEPS: usize = 50;

/// Number of loss values kept for sparklines.
const LOSS_HISTORY_STEPS: usize = 200;

/// EMA smoothing factor for the displayed combined loss.
const EMA_ALPHA: f32 = 0.05;

/// Trend arrow compares the current EMA against the EMA this many steps ago.
const TREND_LOOKBACK: usize = 50;

/// Sparkline block characters, from low to high.
const SPARKLINE_BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Policy-mass ramp for empty cells on probe boards, low to high.
///
/// The five levels are spaced at absolute mass thresholds so the heatmap is
/// comparable across probes: a uniform cell (~1/225 ≈ 0.004) renders as `·`,
/// while a confident tactical move (mass > 0.5) renders as `█`.
const POLICY_RAMP: [char; 5] = ['·', '░', '▒', '▓', '█'];

/// Absolute policy-mass thresholds for [`POLICY_RAMP`] (lower bounds).
const POLICY_THRESHOLDS: [f32; 4] = [0.02, 0.08, 0.20, 0.50];

/// Display flags for the composer.
///
/// These flags are owned by the terminal driver in 3c; the composer reads
/// them but does not mutate them directly. The only exception is the
/// stop-request flag, which the driver sets in response to the `q` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardFlags {
    /// Use ASCII-friendly bar glyphs (`█`/`░`) instead of stones (`●`/`○`).
    pub monochrome: bool,
    /// Render sparklines on a log10 scale.
    pub log_scale: bool,
    /// Show the probe board panel.
    pub show_probes: bool,
}

impl Default for BoardFlags {
    fn default() -> Self {
        Self {
            monochrome: false,
            log_scale: true,
            show_probes: true,
        }
    }
}

/// Pure state + composer for the full-screen board reporter.
///
/// Implements [`Reporter`] so it can be wired directly into the training
/// loop, and exposes [`BoardReporter::render`] to produce a frame on demand.
pub struct BoardReporter<C: Clock> {
    clock: C,
    flags: BoardFlags,
    run_info: Option<RunInfo>,
    start: Option<Instant>,
    steps_done: usize,
    total_steps: usize,
    latest_lr: f64,
    ema: Option<f32>,
    ema_history: Vec<(usize, f32)>,
    combined_losses: Vec<f32>,
    policy_losses: Vec<f32>,
    value_losses: Vec<f32>,
    step_durations: Vec<Duration>,
    last_step_instant: Option<Instant>,
    eval_history: Vec<EvalReport>,
    latest_probes: Vec<ProbeResult>,
    latest_probe_positions: Vec<ProbePosition>,
    stop_requested: bool,
}

impl<C: Clock> BoardReporter<C> {
    /// Create a new board reporter that reads time from `clock`.
    pub fn new(clock: C) -> Self {
        Self {
            clock,
            flags: BoardFlags::default(),
            run_info: None,
            start: None,
            steps_done: 0,
            total_steps: 0,
            latest_lr: 0.0,
            ema: None,
            ema_history: Vec::with_capacity(LOSS_HISTORY_STEPS),
            combined_losses: Vec::with_capacity(LOSS_HISTORY_STEPS),
            policy_losses: Vec::with_capacity(LOSS_HISTORY_STEPS),
            value_losses: Vec::with_capacity(LOSS_HISTORY_STEPS),
            step_durations: Vec::with_capacity(STEP_TIME_WINDOW),
            last_step_instant: None,
            eval_history: Vec::with_capacity(2),
            latest_probes: Vec::new(),
            latest_probe_positions: Vec::new(),
            stop_requested: false,
        }
    }

    /// Replace the default display flags.
    #[must_use]
    pub fn with_flags(mut self, flags: BoardFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Request an early stop. The training loop polls [`Reporter::stop_requested`].
    pub fn request_stop(&mut self) {
        self.stop_requested = true;
    }

    /// Provide the fixed probe positions used for this run.
    ///
    /// The positions are chosen once at run start and persist across
    /// evaluations. The composer needs them to reconstruct the board and
    /// crop the 6×6 window around the expected move.
    pub fn set_probe_positions(&mut self, positions: Vec<ProbePosition>) {
        self.latest_probe_positions = positions;
    }

    /// Compose the current frame.
    ///
    /// `flash` asks for the 5 %-milestone variant of the progress bar: the
    /// five newest completed stones are rendered with a distinct glyph. This
    /// is the seam for the animation described in §7.2; the 3c driver decides
    /// when `flash` is true and simply prints the returned lines.
    #[must_use]
    pub fn render(&self, flash: bool) -> Vec<String> {
        let mut frame = Vec::new();
        self.header_lines(&mut frame);
        frame.push(self.progress_line(flash));
        frame.push(self.throughput_line());
        self.loss_panel(&mut frame);
        frame.push(self.holdout_line());
        self.probe_panel(&mut frame);
        frame.push(FOOTER.to_string());
        frame
    }

    /// Borrow the current display flags.
    pub fn flags(&self) -> &BoardFlags {
        &self.flags
    }

    /// Mutably borrow the display flags.
    ///
    /// The 3c driver uses this to apply key toggles (`l`, `p`) without
    /// reaching into the reporter's event state.
    pub fn flags_mut(&mut self) -> &mut BoardFlags {
        &mut self.flags
    }

    /// The most recent holdout evaluation, if any.
    pub fn latest_eval(&self) -> Option<&EvalReport> {
        self.eval_history.last()
    }

    fn header_lines(&self, out: &mut Vec<String>) {
        let info = match &self.run_info {
            Some(i) => i,
            None => {
                out.push("nettrain - starting...".to_string());
                out.push(String::new());
                return;
            }
        };
        let config_label = format!(
            "{}chx{}res",
            info.model_config.channels, info.model_config.num_blocks
        );
        out.push(format!(
            "nettrain - run {} - config {} - device {} - seed {}",
            info.started_at, config_label, info.device_name, info.train_config.seed
        ));
        out.push(format!(
            "data: {} train / {} holdout - batch {} - eval every {}",
            format_thousands(info.train_samples),
            format_thousands(info.holdout_samples),
            info.train_config.batch_size,
            if info.train_config.eval_interval == 0 {
                "off".to_string()
            } else {
                info.train_config.eval_interval.to_string()
            }
        ));
    }

    fn progress_line(&self, flash: bool) -> String {
        let total = self.total_steps.max(1);
        let step = self.steps_done.min(total);
        let completed = (step * PROGRESS_BAR_CELLS)
            .div_ceil(total)
            .min(PROGRESS_BAR_CELLS);
        let pct = step as f64 / total as f64 * 100.0;

        let (full, empty, flash_glyph) = if self.flags.monochrome {
            ('█', '░', '▓')
        } else {
            ('●', '○', '◉')
        };

        let mut bar = String::with_capacity(PROGRESS_BAR_CELLS);
        for i in 0..PROGRESS_BAR_CELLS {
            if i < completed.saturating_sub(5) {
                bar.push(full);
            } else if i < completed {
                bar.push(if flash { flash_glyph } else { full });
            } else {
                bar.push(empty);
            }
        }

        let eta_str = self.format_eta(step, total);
        format!(
            "{}  {}/{}  {:>3.0} %   eta {}",
            bar, step, total, pct, eta_str
        )
    }

    fn format_eta(&self, step: usize, total: usize) -> String {
        if step < ETA_MIN_STEPS {
            return "—".to_string();
        }
        let window: Vec<_> = self
            .step_durations
            .iter()
            .rev()
            .take(STEP_TIME_WINDOW)
            .copied()
            .collect();
        if window.is_empty() {
            return "—".to_string();
        }
        let mean = window.iter().sum::<Duration>() / window.len() as u32;
        if mean.is_zero() {
            return "—".to_string();
        }
        let remaining = total.saturating_sub(step);
        let eta = mean * remaining as u32;
        format_duration_mm_ss(eta)
    }

    fn throughput_line(&self) -> String {
        let steps_per_sec = self.steps_per_second();
        let lr = self.current_lr();
        let phase = self.lr_phase();
        format!("{:.0} steps/s - lr {:.2e} ({})", steps_per_sec, lr, phase)
    }

    fn steps_per_second(&self) -> f64 {
        let window: Vec<_> = self
            .step_durations
            .iter()
            .rev()
            .take(STEP_TIME_WINDOW)
            .copied()
            .collect();
        if window.is_empty() {
            return 0.0;
        }
        let mean = window.iter().sum::<Duration>() / window.len() as u32;
        if mean.is_zero() {
            return 0.0;
        }
        1.0 / mean.as_secs_f64()
    }

    fn current_lr(&self) -> f64 {
        // The latest Step event carried the LR used for that step.
        // Before any step arrives, fall back to the peak learning rate.
        if self.latest_lr > 0.0 {
            self.latest_lr
        } else {
            self.run_info
                .as_ref()
                .map_or(0.0, |info| info.train_config.lr_max)
        }
    }

    fn lr_phase(&self) -> String {
        let info = match &self.run_info {
            Some(i) => i,
            None => return "warmup".to_string(),
        };
        let warmup = info.train_config.lr_warmup_steps;
        if self.steps_done < warmup {
            format!("warmup @{}", warmup)
        } else {
            format!("cosine, warmup done @{}", warmup)
        }
    }

    fn loss_panel(&self, out: &mut Vec<String>) {
        let latest_loss = self.combined_losses.last().copied();
        let ema = self.ema;
        let arrow = self.trend_arrow();
        let scale_label = if self.flags.log_scale {
            "[log]"
        } else {
            "[lin]"
        };

        let loss_line = match (latest_loss, ema) {
            (Some(l), Some(e)) => {
                format!("loss   {:.3}  ema {:.3} {}   {}", l, e, arrow, scale_label)
            }
            (Some(l), None) => format!("loss   {:.3}  ema - →   {}", l, scale_label),
            _ => format!("loss   -      ema - →   {}", scale_label),
        };
        out.push(loss_line);

        let policy_spark = self.sparkline(&self.policy_losses, self.flags.log_scale);
        let value_spark = self.sparkline(&self.value_losses, self.flags.log_scale);

        let latest_policy = self.policy_losses.last().copied().unwrap_or(0.0);
        let latest_value = self.value_losses.last().copied().unwrap_or(0.0);

        out.push(format!("policy {:.3}  {}", latest_policy, policy_spark));
        out.push(format!("value  {:.3}  {}", latest_value, value_spark));
    }

    fn trend_arrow(&self) -> char {
        let current = match self.ema {
            Some(e) => e,
            None => return '→',
        };
        let target_step = self.steps_done.saturating_sub(TREND_LOOKBACK).max(1);
        let past = self
            .ema_history
            .iter()
            .find(|(s, _)| *s >= target_step)
            .map(|(_, e)| *e);
        match past {
            Some(p) => {
                let delta = current - p;
                if delta.abs() < 1e-4 {
                    '→'
                } else if delta < 0.0 {
                    '▼'
                } else {
                    '▲'
                }
            }
            None => '→',
        }
    }

    fn sparkline(&self, values: &[f32], log_scale: bool) -> String {
        let window: Vec<_> = values
            .iter()
            .rev()
            .take(LOSS_HISTORY_STEPS)
            .copied()
            .collect();
        if window.is_empty() {
            return " ".repeat(LOSS_HISTORY_STEPS);
        }
        let normalized: Vec<f32> = if log_scale {
            window
                .iter()
                .map(|&v| if v > 0.0 { v.log10() } else { f32::MIN })
                .collect()
        } else {
            window.to_vec()
        };

        let min = normalized
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(f32::INFINITY, f32::min);
        let max = normalized
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(f32::NEG_INFINITY, f32::max);

        let pad = LOSS_HISTORY_STEPS.saturating_sub(window.len());
        let mut chars: Vec<char> = Vec::with_capacity(LOSS_HISTORY_STEPS);
        chars.extend(std::iter::repeat_n(' ', pad));

        for &v in normalized.iter().rev() {
            let ch = if !v.is_finite() {
                '▁'
            } else if max == min {
                '█'
            } else {
                let t = ((v - min) / (max - min)).clamp(0.0, 1.0);
                let idx = (t * (SPARKLINE_BLOCKS.len() - 1) as f32).round() as usize;
                SPARKLINE_BLOCKS[idx.clamp(0, SPARKLINE_BLOCKS.len() - 1)]
            };
            chars.push(ch);
        }
        chars.iter().collect()
    }

    fn holdout_line(&self) -> String {
        let latest = match self.eval_history.last() {
            Some(e) => e,
            None => return "holdout: waiting for first evaluation".to_string(),
        };
        let prev = if self.eval_history.len() >= 2 {
            Some(&self.eval_history[self.eval_history.len() - 2])
        } else {
            None
        };

        let m = latest.metrics;
        let loss_delta = prev.map(|p| m.loss - p.metrics.loss);
        let top1_delta =
            prev.map(|p| m.policy_top1_accuracy * 100.0 - p.metrics.policy_top1_accuracy * 100.0);
        let vsign_delta =
            prev.map(|p| m.value_sign_accuracy * 100.0 - p.metrics.value_sign_accuracy * 100.0);

        format!(
            "holdout @{}   loss {:.3} {}   policy top-1 {:.1} % {}   value sign {:.1} % {}",
            m.step,
            m.loss,
            format_delta(loss_delta),
            m.policy_top1_accuracy * 100.0,
            format_delta_pct(top1_delta),
            m.value_sign_accuracy * 100.0,
            format_delta_pct(vsign_delta)
        )
    }

    fn probe_panel(&self, out: &mut Vec<String>) {
        let latest_eval = match self.eval_history.last() {
            Some(e) => e,
            None => return,
        };
        if latest_eval.probes.is_empty() {
            out.push("probes: none".to_string());
            return;
        }

        let step = latest_eval.metrics.step;
        if !self.flags.show_probes {
            out.push(format!("probes @{} (panel off; press 'p' to show)", step));
            return;
        }

        out.push(format!("probes @{}", step));

        // Titles and boards are rendered line by line so the three probes
        // sit side by side in the same terminal row.
        let titles: Vec<String> = latest_eval
            .probes
            .iter()
            .map(|p| probe_title(p.class))
            .collect();
        out.push(titles.join("    "));

        let boards: Vec<Vec<String>> = latest_eval
            .probes
            .iter()
            .zip(self.latest_probe_positions.iter())
            .map(|(result, position)| probe_board_lines(result, position))
            .collect();
        let height = boards.iter().map(|b| b.len()).max().unwrap_or(0);
        for row in 0..height {
            let mut line = String::new();
            for (i, board) in boards.iter().enumerate() {
                if i > 0 {
                    line.push_str("    ");
                }
                line.push_str(board.get(row).map(String::as_str).unwrap_or(""));
            }
            out.push(line);
        }

        let captions: Vec<String> = latest_eval
            .probes
            .iter()
            .map(|p| {
                let mark = if p.correct { '✓' } else { '✗' };
                let value = format_value(p.value);
                format!("top1 {} {}   v {}", algebraic(p.top1), mark, value)
            })
            .collect();
        out.push(captions.join("    "));
    }
}

impl<C: Clock> Reporter for BoardReporter<C> {
    fn on_event(&mut self, event: &TrainEvent) {
        match event {
            TrainEvent::Started(info) => {
                self.run_info = Some(info.clone());
                self.start = Some(self.clock.now());
                self.total_steps = info.train_config.steps;
            }
            TrainEvent::Step(metrics) => {
                self.steps_done = metrics.step;
                self.latest_lr = metrics.lr;
                self.update_loss_buffers(metrics);
                self.update_step_duration();
            }
            TrainEvent::Eval(report) => {
                if self.eval_history.len() >= 2 {
                    self.eval_history.remove(0);
                }
                self.eval_history.push(report.clone());
                self.latest_probes = report.probes.clone();
            }
            TrainEvent::Finished(_) => {}
        }
    }

    fn stop_requested(&self) -> bool {
        self.stop_requested
    }
}

impl<C: Clock> BoardReporter<C> {
    fn update_loss_buffers(&mut self, metrics: &StepMetrics) {
        self.combined_losses.push(metrics.loss);
        self.policy_losses.push(metrics.policy_loss);
        self.value_losses.push(metrics.value_loss);
        if self.combined_losses.len() > LOSS_HISTORY_STEPS {
            self.combined_losses.remove(0);
        }
        if self.policy_losses.len() > LOSS_HISTORY_STEPS {
            self.policy_losses.remove(0);
        }
        if self.value_losses.len() > LOSS_HISTORY_STEPS {
            self.value_losses.remove(0);
        }

        let prev = self.ema.unwrap_or(metrics.loss);
        let next = EMA_ALPHA * metrics.loss + (1.0 - EMA_ALPHA) * prev;
        self.ema = Some(next);

        self.ema_history.push((metrics.step, next));
        if self.ema_history.len() > LOSS_HISTORY_STEPS {
            self.ema_history.remove(0);
        }
    }

    fn update_step_duration(&mut self) {
        let now = self.clock.now();
        let prev = self.last_step_instant.or(self.start);
        if let Some(p) = prev {
            let dur = now.saturating_duration_since(p);
            self.step_durations.push(dur);
            if self.step_durations.len() > STEP_TIME_WINDOW {
                self.step_durations.remove(0);
            }
        }
        self.last_step_instant = Some(now);
    }
}

const FOOTER: &str = "[q] stop & checkpoint    [l] log/linear    [p] probes on/off";

fn probe_title(class: ProbeClass) -> String {
    match class {
        ProbeClass::Win => "win-in-1".to_string(),
        ProbeClass::Block => "forced block".to_string(),
        ProbeClass::Quiet => "quiet".to_string(),
    }
}

fn probe_board_lines(result: &ProbeResult, position: &ProbePosition) -> Vec<String> {
    let board = match position.sample.board() {
        Ok(b) => b,
        Err(_) => {
            return vec!["(bad board)".to_string()];
        }
    };

    let center = position.expected;
    let r0 = center.row().saturating_sub(2);
    let r1 = (center.row() + 3).min(14);
    let c0 = center.col().saturating_sub(2);
    let c1 = (center.col() + 3).min(14);

    let mut lines = Vec::new();
    for r in r0..=r1 {
        let mut line = String::new();
        for c in c0..=c1 {
            let Some(mv) = Move::new(r, c) else {
                continue;
            };
            let glyph = probe_cell(&board, result, mv);
            line.push(glyph);
            line.push(' ');
        }
        // Trim trailing space so side-by-side boards line up cleanly.
        lines.push(line.trim_end().to_string());
    }
    lines
}

fn probe_cell(board: &Board, result: &ProbeResult, mv: Move) -> char {
    if let Some(color) = board.stone_at(mv) {
        return match color {
            Color::Black => '●',
            Color::White => '○',
        };
    }
    if mv == result.top1 {
        return '◆';
    }
    let mass = result.policy[usize::from(mv.row()) * 15 + usize::from(mv.col())];
    policy_ramp_char(mass)
}

fn policy_ramp_char(mass: f32) -> char {
    for (i, &thresh) in POLICY_THRESHOLDS.iter().enumerate().rev() {
        if mass >= thresh {
            return POLICY_RAMP[i + 1];
        }
    }
    POLICY_RAMP[0]
}

fn format_delta(delta: Option<f32>) -> String {
    match delta {
        None => "     ".to_string(),
        Some(d) => {
            let sign = if d >= 0.0 { '+' } else { '−' };
            format!("({}{:.3})", sign, d.abs())
        }
    }
}

fn format_delta_pct(delta: Option<f32>) -> String {
    match delta {
        None => "      ".to_string(),
        Some(d) => {
            let sign = if d >= 0.0 { '+' } else { '−' };
            format!("({}{:.1})", sign, d.abs())
        }
    }
}

fn format_value(v: f32) -> String {
    let sign = if v >= 0.0 { '+' } else { '−' };
    format!("{}{:.2}", sign, v.abs())
}

fn algebraic(mv: Move) -> String {
    format!("{}{}", (b'a' + mv.col()) as char, mv.row() + 1)
}

fn format_thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

fn format_duration_mm_ss(d: Duration) -> String {
    let secs = d.as_secs();
    let mm = (secs % 3600) / 60;
    let ss = secs % 60;
    format!("{:02}:{:02}", mm, ss)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::line::TestClock;
    use crate::report::{RunInfo, TrainConfig};
    use crate::train::EvalMetrics;
    use engine::Move;
    use std::time::Duration;

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    fn run_info(steps: usize) -> RunInfo {
        RunInfo {
            model_config: crate::network::ModelConfig::tiny(),
            train_config: TrainConfig {
                steps,
                ..TrainConfig::tiny()
            },
            device_name: "test".to_string(),
            train_samples: 1_000,
            holdout_samples: 200,
            started_at: "2026-11-11T20:41:07".to_string(),
        }
    }

    fn step(metrics_step: usize, loss: f32, lr: f64) -> StepMetrics {
        StepMetrics {
            step: metrics_step,
            loss,
            policy_loss: loss * 0.5,
            value_loss: loss * 0.5,
            lr,
        }
    }

    #[test]
    fn early_frame_shows_placeholder_eta_and_no_eval() {
        let clock = TestClock::new();
        let mut reporter = BoardReporter::new(&clock);
        reporter.on_event(&TrainEvent::Started(run_info(100)));
        for i in 1..=10 {
            clock.advance(Duration::from_millis(100));
            reporter.on_event(&TrainEvent::Step(step(i, 0.693, 1e-3)));
        }
        let frame = reporter.render(false);
        let text = frame.join("\n");
        assert!(
            text.contains("nettrain - run 2026-11-11T20:41:07"),
            "{text}"
        );
        assert!(text.contains("data: 1 000 train / 200 holdout"), "{text}");
        assert!(text.contains("eta —"), "{text}");
        assert!(
            text.contains("holdout: waiting for first evaluation"),
            "{text}"
        );
        assert!(text.contains(FOOTER), "{text}");
    }

    #[test]
    fn eta_appears_after_fifty_steps() {
        let clock = TestClock::new();
        let mut reporter = BoardReporter::new(&clock);
        reporter.on_event(&TrainEvent::Started(run_info(100)));
        for i in 1..=50 {
            clock.advance(Duration::from_millis(100));
            reporter.on_event(&TrainEvent::Step(step(i, 0.5, 1e-3)));
        }
        let frame = reporter.render(false);
        let bar_line = frame.iter().find(|l| l.contains("eta")).unwrap();
        assert!(
            !bar_line.contains("eta —"),
            "ETA should appear after 50 steps: {bar_line}"
        );
        assert!(
            bar_line.contains("eta 00:05"),
            "expected 5s ETA: {bar_line}"
        );
    }

    #[test]
    fn monochrome_bar_uses_block_glyphs() {
        let clock = TestClock::new();
        let mut reporter = BoardReporter::new(&clock).with_flags(BoardFlags {
            monochrome: true,
            ..BoardFlags::default()
        });
        reporter.on_event(&TrainEvent::Started(run_info(100)));
        for i in 1..=50 {
            reporter.on_event(&TrainEvent::Step(step(i, 0.5, 1e-3)));
        }
        let frame = reporter.render(false);
        let bar_line = frame.iter().find(|l| l.contains("eta")).unwrap();
        assert!(bar_line.contains('█'), "monochrome bar uses █: {bar_line}");
        assert!(bar_line.contains('░'), "monochrome bar uses ░: {bar_line}");
        assert!(
            !bar_line.contains('●'),
            "monochrome bar avoids ●: {bar_line}"
        );
    }

    #[test]
    fn flash_varies_the_newest_stones() {
        let clock = TestClock::new();
        let mut reporter = BoardReporter::new(&clock);
        reporter.on_event(&TrainEvent::Started(run_info(100)));
        for i in 1..=50 {
            reporter.on_event(&TrainEvent::Step(step(i, 0.5, 1e-3)));
        }
        let plain = reporter.render(false);
        let flash = reporter.render(true);
        let plain_bar = plain.iter().find(|l| l.contains("eta")).unwrap();
        let flash_bar = flash.iter().find(|l| l.contains("eta")).unwrap();
        assert_ne!(plain_bar, flash_bar, "flash must change the bar line");
        let flash_full_count = flash_bar.chars().filter(|&c| c == '●' || c == '◉').count();
        let plain_full_count = plain_bar.chars().filter(|&c| c == '●').count();
        assert_eq!(
            flash_full_count, plain_full_count,
            "same number of completed cells"
        );
        assert!(
            flash_bar.contains('◉'),
            "flash uses distinct glyph: {flash_bar}"
        );
    }

    #[test]
    fn sparkline_slides_after_200_steps() {
        let clock = TestClock::new();
        let mut reporter = BoardReporter::new(&clock);
        reporter.on_event(&TrainEvent::Started(run_info(300)));
        for i in 1..=250 {
            reporter.on_event(&TrainEvent::Step(step(i, i as f32 * 0.01, 1e-3)));
        }
        let frame = reporter.render(false);
        let policy_line = frame.iter().find(|l| l.starts_with("policy")).unwrap();
        let spark_start = policy_line.find("  ").unwrap() + 2;
        let spark = &policy_line[spark_start..];
        assert_eq!(spark.chars().count(), LOSS_HISTORY_STEPS);
        assert!(
            spark.ends_with('█'),
            "recent high loss reaches top: {spark}"
        );
    }

    #[test]
    fn sparkline_left_pads_when_short() {
        let clock = TestClock::new();
        let mut reporter = BoardReporter::new(&clock);
        reporter.on_event(&TrainEvent::Started(run_info(100)));
        reporter.on_event(&TrainEvent::Step(step(1, 1.0, 1e-3)));
        let frame = reporter.render(false);
        let policy_line = frame.iter().find(|l| l.starts_with("policy")).unwrap();
        let spark_start = policy_line.find("  ").unwrap() + 2;
        let spark = &policy_line[spark_start..];
        assert_eq!(spark.chars().count(), LOSS_HISTORY_STEPS);
        assert!(
            spark.ends_with('█'),
            "single value reaches top block: {spark}"
        );
        assert!(
            spark.starts_with(' '),
            "short history is left-padded: {spark}"
        );
    }

    #[test]
    fn trend_arrow_points_down_when_loss_falls() {
        let clock = TestClock::new();
        let mut reporter = BoardReporter::new(&clock);
        reporter.on_event(&TrainEvent::Started(run_info(200)));
        for i in 1..=60 {
            let loss = 1.0 - (i as f32 * 0.015);
            reporter.on_event(&TrainEvent::Step(step(i, loss.max(0.1), 1e-3)));
        }
        let frame = reporter.render(false);
        let loss_line = frame.iter().find(|l| l.starts_with("loss")).unwrap();
        assert!(loss_line.contains('▼'), "falling loss shows ▼: {loss_line}");
    }

    #[test]
    fn holdout_deltas_appear_after_second_eval() {
        let clock = TestClock::new();
        let mut reporter = BoardReporter::new(&clock);
        reporter.on_event(&TrainEvent::Started(run_info(100)));
        reporter.on_event(&TrainEvent::Step(step(1, 0.5, 1e-3)));

        let first = EvalReport {
            metrics: EvalMetrics {
                step: 10,
                loss: 0.7,
                policy_top1_accuracy: 0.40,
                value_sign_accuracy: 0.75,
            },
            probes: Vec::new(),
        };
        let second = EvalReport {
            metrics: EvalMetrics {
                step: 20,
                loss: 0.65,
                policy_top1_accuracy: 0.45,
                value_sign_accuracy: 0.80,
            },
            probes: Vec::new(),
        };
        reporter.on_event(&TrainEvent::Eval(first));
        let after_first = reporter.render(false).join("\n");
        assert!(
            !after_first.contains("(−"),
            "first eval has no deltas: {after_first}"
        );

        reporter.on_event(&TrainEvent::Eval(second));
        let after_second = reporter.render(false).join("\n");
        assert!(
            after_second.contains("(−0.050)"),
            "loss delta: {after_second}"
        );
        assert!(
            after_second.contains("(+5.0)"),
            "top1 delta: {after_second}"
        );
        assert!(
            after_second.contains("(+5.0)"),
            "vsign delta: {after_second}"
        );
    }

    #[test]
    fn probe_board_renders_known_position() {
        // A win-in-1 for White: four White stones on row 7, cols 7-10,
        // plus quiet Black stones so White is the side to move.
        let sample = train::Sample {
            black: vec![mv(0, 0), mv(1, 0), mv(1, 1), mv(2, 2), mv(3, 3)],
            white: vec![mv(7, 7), mv(7, 8), mv(7, 9), mv(7, 10)],
            to_move: engine::Color::White,
            policy: vec![(mv(7, 11), 1.0)],
            value: 1.0,
        };
        let position = ProbePosition {
            sample,
            class: ProbeClass::Win,
            expected: mv(7, 11),
        };
        let result = ProbeResult {
            step: 10,
            class: ProbeClass::Win,
            policy: vec![0.0; 225],
            top1: mv(7, 11),
            value: 0.94,
            correct: true,
        };
        let lines = probe_board_lines(&result, &position);
        assert_eq!(lines.len(), 6, "expected 6 rows: {lines:?}");
        let text = lines.join("\n");
        assert!(
            text.contains('●') || text.contains('○'),
            "stones render: {text}"
        );
        assert!(text.contains('◆'), "top-1 is marked: {text}");
    }

    #[test]
    fn probe_panel_summary_when_hidden() {
        let clock = TestClock::new();
        let mut reporter = BoardReporter::new(&clock).with_flags(BoardFlags {
            show_probes: false,
            ..BoardFlags::default()
        });
        reporter.on_event(&TrainEvent::Started(run_info(100)));

        let sample = train::Sample {
            black: vec![mv(0, 0), mv(1, 0), mv(1, 1), mv(2, 2), mv(3, 3)],
            white: vec![mv(7, 7), mv(7, 8), mv(7, 9), mv(7, 10)],
            to_move: engine::Color::White,
            policy: vec![(mv(7, 11), 1.0)],
            value: 1.0,
        };
        let position = ProbePosition {
            sample,
            class: ProbeClass::Win,
            expected: mv(7, 11),
        };
        let result = ProbeResult {
            step: 10,
            class: ProbeClass::Win,
            policy: vec![0.0; 225],
            top1: mv(7, 11),
            value: 0.94,
            correct: true,
        };
        let eval = EvalReport {
            metrics: EvalMetrics {
                step: 10,
                loss: 0.5,
                policy_top1_accuracy: 0.42,
                value_sign_accuracy: 0.78,
            },
            probes: vec![result],
        };
        reporter.latest_probe_positions = vec![position];
        reporter.on_event(&TrainEvent::Eval(eval));
        let frame = reporter.render(false);
        let text = frame.join("\n");
        assert!(
            text.contains("probes @10 (panel off"),
            "hidden panel renders summary: {text}"
        );
    }

    #[test]
    fn stop_request_is_remembered() {
        let clock = TestClock::new();
        let mut reporter = BoardReporter::new(&clock);
        assert!(!reporter.stop_requested());
        reporter.request_stop();
        assert!(reporter.stop_requested());
    }
}
