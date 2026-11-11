//! Golden-frame tests for the board reporter.
//!
//! These tests feed scripted event sequences into a [`BoardReporter`] and
//! compare the composed frame against committed expected strings. They
//! follow the same scripted-acceptance discipline as the `cli` crate's
//! study-table tests: pure rendering in, exact text out.

use std::cell::Cell;
use std::time::{Duration, Instant};

use engine::Move;
use net::report::board::{BoardFlags, BoardReporter};
use net::report::line::Clock;
use net::report::probe::{ProbeClass, ProbePosition, ProbeResult};
use net::report::{EvalReport, Reporter, RunInfo, TrainEvent};
use net::train::{EvalMetrics, StepMetrics, TrainConfig};

fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}

fn run_info(steps: usize) -> RunInfo {
    RunInfo {
        model_config: net::network::ModelConfig::tiny(),
        train_config: TrainConfig {
            steps,
            ..TrainConfig::tiny()
        },
        device_name: "test-device".to_string(),
        train_samples: 45_080,
        holdout_samples: 4_920,
        started_at: "2026-11-11T20:41:07".to_string(),
    }
}

fn step(s: usize, loss: f32, lr: f64) -> StepMetrics {
    StepMetrics {
        step: s,
        loss,
        policy_loss: loss * 0.5,
        value_loss: loss * 0.5,
        lr,
    }
}

fn make_probes() -> (Vec<ProbePosition>, Vec<ProbeResult>) {
    // Win probe: White to move completes five on row 7.
    let win_sample = train::Sample {
        black: vec![mv(0, 0), mv(1, 0), mv(1, 1), mv(2, 2), mv(3, 3)],
        white: vec![mv(7, 7), mv(7, 8), mv(7, 9), mv(7, 10)],
        to_move: engine::Color::White,
        policy: vec![(mv(7, 11), 1.0)],
        value: 1.0,
    };
    let win_position = ProbePosition {
        sample: win_sample,
        class: ProbeClass::Win,
        expected: mv(7, 11),
    };
    let mut win_policy = vec![0.0f32; 225];
    win_policy[7 * 15 + 11] = 0.92;
    let win_result = ProbeResult {
        step: 80,
        class: ProbeClass::Win,
        policy: win_policy,
        top1: mv(7, 11),
        value: 0.94,
        correct: true,
    };

    // Block probe: Black to move blocks White's open four on row 8.
    // Black has fewer stones because Black is the side to move.
    let block_sample = train::Sample {
        black: vec![mv(0, 0), mv(1, 0), mv(1, 1), mv(2, 2)],
        white: vec![mv(8, 6), mv(8, 7), mv(8, 8), mv(8, 9), mv(3, 3)],
        to_move: engine::Color::Black,
        policy: vec![(mv(8, 10), 1.0)],
        value: -1.0,
    };
    let block_position = ProbePosition {
        sample: block_sample,
        class: ProbeClass::Block,
        expected: mv(8, 10),
    };
    let mut block_policy = vec![0.0f32; 225];
    block_policy[8 * 15 + 11] = 0.85; // wrong side of the four, inside the window
    let block_result = ProbeResult {
        step: 80,
        class: ProbeClass::Block,
        policy: block_policy,
        top1: mv(8, 11),
        value: -0.12,
        correct: false,
    };

    // Quiet probe: empty-ish position, uniform-ish target.
    let quiet_sample = train::Sample {
        black: vec![mv(7, 7)],
        white: vec![mv(8, 8)],
        to_move: engine::Color::Black,
        policy: vec![(mv(7, 8), 1.0)],
        value: 0.0,
    };
    let quiet_position = ProbePosition {
        sample: quiet_sample,
        class: ProbeClass::Quiet,
        expected: mv(7, 8),
    };
    let quiet_policy = vec![1.0 / 225.0; 225];
    let quiet_result = ProbeResult {
        step: 80,
        class: ProbeClass::Quiet,
        policy: quiet_policy,
        top1: mv(7, 8),
        value: 0.03,
        correct: true,
    };

    (
        vec![win_position, block_position, quiet_position],
        vec![win_result, block_result, quiet_result],
    )
}

/// Deterministic clock for integration tests.
#[derive(Debug)]
struct TestClock {
    now: Cell<Instant>,
}

impl TestClock {
    fn new() -> Self {
        Self {
            now: Cell::new(Instant::now()),
        }
    }

    fn advance(&self, duration: Duration) {
        self.now.set(self.now.get() + duration);
    }
}

impl Clock for TestClock {
    fn now(&self) -> Instant {
        self.now.get()
    }
}

impl Clock for &TestClock {
    fn now(&self) -> Instant {
        self.now.get()
    }
}

#[test]
fn golden_early_run() {
    let clock = TestClock::new();
    let mut reporter = BoardReporter::new(&clock);
    reporter.on_event(&TrainEvent::Started(run_info(10_000)));
    for i in 1..=10 {
        clock.advance(Duration::from_millis(100));
        reporter.on_event(&TrainEvent::Step(step(i, 0.693, 1e-3)));
    }

    let frame = reporter.render(false);
    let expected = "nettrain - run 2026-11-11T20:41:07 - config 8chx1res - device test-device - seed 0\n\
                    data: 45 080 train / 4 920 holdout - batch 16 - eval every off\n\
                    ●○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○  10/10000    0 %   eta —\n\
                    10 steps/s - lr 1.00e-3 (cosine, warmup done @5)\n\
                    loss   0.693  ema 0.693 →   [log]\n\
                    policy 0.347                                                                                                                                                                                                ██████████\n\
                    value  0.347                                                                                                                                                                                                ██████████\n\
                    holdout: waiting for first evaluation\n\
                    [q] stop & checkpoint    [l] log/linear    [p] probes on/off";
    assert_eq!(frame.join("\n"), expected);
}

#[test]
fn golden_mid_run_with_probes() {
    let clock = TestClock::new();
    let mut reporter = BoardReporter::new(&clock);
    let (positions, results) = make_probes();
    reporter.set_probe_positions(positions);

    reporter.on_event(&TrainEvent::Started(run_info(10_000)));
    for i in 1..=80 {
        clock.advance(Duration::from_millis(100));
        let loss = 0.693 - (i as f32 * 0.005);
        reporter.on_event(&TrainEvent::Step(step(i, loss.max(0.1), 1e-3)));
    }

    let prev_eval = EvalReport {
        metrics: EvalMetrics {
            step: 40,
            loss: 0.75,
            policy_top1_accuracy: 0.35,
            value_sign_accuracy: 0.70,
        },
        probes: vec![],
    };
    reporter.on_event(&TrainEvent::Eval(prev_eval));

    let eval = EvalReport {
        metrics: EvalMetrics {
            step: 80,
            loss: 0.65,
            policy_top1_accuracy: 0.42,
            value_sign_accuracy: 0.78,
        },
        probes: results,
    };
    reporter.on_event(&TrainEvent::Eval(eval));

    let frame = reporter.render(false);
    let expected = "nettrain - run 2026-11-11T20:41:07 - config 8chx1res - device test-device - seed 0\n\
                    data: 45 080 train / 4 920 holdout - batch 16 - eval every off\n\
                    ●○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○  80/10000    1 %   eta 16:32\n\
                    10 steps/s - lr 1.00e-3 (cosine, warmup done @5)\n\
                    loss   0.293  ema 0.386 ▼   [log]\n\
                    policy 0.147                                                                                                                          █████████▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▆▆▆▆▆▆▆▆▆▆▆▆▆▅▅▅▅▅▅▅▅▅▅▅▄▄▄▄▄▄▄▄▄▄▄▃▃▃▃▃▃▃▃▃▂▂▂▂▂▂▂▂▁▁▁▁\n\
                    value  0.147                                                                                                                          █████████▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▆▆▆▆▆▆▆▆▆▆▆▆▆▅▅▅▅▅▅▅▅▅▅▅▄▄▄▄▄▄▄▄▄▄▄▃▃▃▃▃▃▃▃▃▂▂▂▂▂▂▂▂▁▁▁▁\n\
                    holdout @80   loss 0.650 (−0.100)   policy top-1 42.0 % (+7.0)   value sign 78.0 % (+8.0)\n\
                    probes @80\n\
                    win-in-1    forced block    quiet\n\
                    · · · · · ·    · · · · · ·    · · · · · ·\n\
                    · · · · · ·    · · · · · ·    · · · · · ·\n\
                    ○ ○ ◆ · · ·    ○ ○ · ◆ · ·    · ● ◆ · · ·\n\
                    · · · · · ·    · · · · · ·    · · ○ · · ·\n\
                    · · · · · ·    · · · · · ·    · · · · · ·\n\
                    · · · · · ·    · · · · · ·    · · · · · ·\n\
                    top1 l8 ✓   v +0.94    top1 l9 ✗   v −0.12    top1 i8 ✓   v +0.03\n\
                    [q] stop & checkpoint    [l] log/linear    [p] probes on/off";
    assert_eq!(frame.join("\n"), expected);
}

#[test]
fn golden_monochrome_mid_run() {
    let clock = TestClock::new();
    let mut reporter = BoardReporter::new(&clock).with_flags(BoardFlags {
        monochrome: true,
        log_scale: true,
        show_probes: false,
    });
    reporter.on_event(&TrainEvent::Started(run_info(10_000)));
    for i in 1..=80 {
        clock.advance(Duration::from_millis(100));
        reporter.on_event(&TrainEvent::Step(step(i, 0.5, 1e-3)));
    }

    let frame = reporter.render(false);
    let expected = "nettrain - run 2026-11-11T20:41:07 - config 8chx1res - device test-device - seed 0\n\
                    data: 45 080 train / 4 920 holdout - batch 16 - eval every off\n\
                    █░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  80/10000    1 %   eta 16:32\n\
                    10 steps/s - lr 1.00e-3 (cosine, warmup done @5)\n\
                    loss   0.500  ema 0.500 →   [log]\n\
                    policy 0.250                                                                                                                          ████████████████████████████████████████████████████████████████████████████████\n\
                    value  0.250                                                                                                                          ████████████████████████████████████████████████████████████████████████████████\n\
                    holdout: waiting for first evaluation\n\
                    [q] stop & checkpoint    [l] log/linear    [p] probes on/off";
    assert_eq!(frame.join("\n"), expected);
}
