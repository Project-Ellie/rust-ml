//! Manual supervised training loop.
//!
//! This module owns the outer training loop (docs/09, docs/12): Burn
//! owns the network, optimizer, and batch math; the application owns
//! data loading, D4 augmentation, the loop, evaluation cadence, and
//! checkpointing.

use burn::data::dataloader::DataLoaderBuilder;
use burn::data::dataset::InMemDataset;
use burn::optim::AdamWConfig;
use burn::optim::lr_scheduler::composed::ComposedLrSchedulerConfig;
use burn::optim::lr_scheduler::cosine::CosineAnnealingLrSchedulerConfig;
use burn::optim::lr_scheduler::linear::LinearLrSchedulerConfig;
use burn::optim::lr_scheduler::module_lr_scheduler::ModuleLrScheduler;
use burn::optim::{GradientsParams, ModuleOptimizer};
use burn::tensor::Device;
use train::Sample;

use crate::batcher::{NetBatch, NetBatcher};
use crate::loss::{self, LossInput};
use crate::network::{Model, ModelConfig};
use crate::report::{
    EvalReport, FinishInfo, NullReporter, Reporter, RunInfo, StopReason, TrainEvent,
};

/// Training-loop hyperparameters.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TrainConfig {
    /// Number of optimizer steps.
    pub steps: usize,
    /// Training batch size.
    pub batch_size: usize,
    /// Evaluate on holdout every N steps (0 disables).
    pub eval_interval: usize,
    /// Global seed for backend RNG and shuffle.
    pub seed: u64,
    /// Linear warmup steps.
    pub lr_warmup_steps: usize,
    /// Peak learning rate.
    pub lr_max: f64,
    /// Final learning rate.
    pub lr_min: f64,
    /// Cosine decay steps after warmup.
    pub lr_cosine_steps: usize,
    /// AdamW weight decay.
    pub weight_decay: f32,
    /// Background dataloader workers for batch encoding and D4
    /// augmentation.
    ///
    /// 0 batches on the caller thread: seeded runs stay
    /// bit-reproducible. >0 overlaps CPU batching with device compute
    /// (measured 2026-11-11, M5 Max, metal device, batch 256: 0.333
    /// -> 0.256 s/step) but the augmentation order becomes
    /// scheduling-dependent, i.e. not bit-reproducible.
    #[serde(default)]
    pub dataloader_workers: usize,
}

impl TrainConfig {
    /// Tiny config for fast unit tests.
    pub fn tiny() -> Self {
        Self {
            steps: 50,
            batch_size: 16,
            eval_interval: 0,
            seed: 0,
            lr_warmup_steps: 5,
            lr_max: 1e-3,
            lr_min: 1e-5,
            lr_cosine_steps: 45,
            weight_decay: 1e-4,
            dataloader_workers: 0,
        }
    }
}

/// Metrics recorded at each training step.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StepMetrics {
    /// Step index (1-based).
    pub step: usize,
    /// Average combined loss on the training batch.
    pub loss: f32,
    /// Policy cross-entropy term, before combination.
    #[serde(default)]
    pub policy_loss: f32,
    /// Value mean-squared-error term, before combination.
    #[serde(default)]
    pub value_loss: f32,
    /// Learning rate used for this step.
    pub lr: f64,
}

/// Metrics recorded during a holdout evaluation.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EvalMetrics {
    /// Step index at which evaluation ran.
    pub step: usize,
    /// Average combined loss on the holdout set.
    pub loss: f32,
    /// Policy top-1 accuracy.
    pub policy_top1_accuracy: f32,
    /// Value sign accuracy.
    pub value_sign_accuracy: f32,
}

/// Run the manual training loop.
///
/// Returns the trained model, the per-step training metrics, and any
/// holdout evaluation metrics. Equivalent to
/// [`train_with_reporter`] with a [`NullReporter`].
///
/// `model_config` describes the architecture that produced `model`. It is
/// recorded in the [`RunInfo`] emitted at the start of the run.
pub fn train(
    model: Model,
    config: &TrainConfig,
    model_config: &ModelConfig,
    train_samples: Vec<Sample>,
    holdout_samples: Vec<Sample>,
    device: &Device,
) -> (Model, Vec<StepMetrics>, Vec<EvalMetrics>) {
    let run_info = RunInfo {
        model_config: model_config.clone(),
        train_config: *config,
        device_name: format!("{device:?}"),
        train_samples: train_samples.len(),
        holdout_samples: holdout_samples.len(),
    };
    train_with_reporter(
        model,
        config,
        train_samples,
        holdout_samples,
        device,
        run_info,
        &mut NullReporter,
    )
}

/// Run the manual training loop, publishing events to `reporter`.
///
/// Emits [`TrainEvent::Started`] once, [`TrainEvent::Step`] after every
/// optimizer step, [`TrainEvent::Eval`] after every holdout evaluation,
/// and [`TrainEvent::Finished`] once at the end. The loop checks
/// [`Reporter::stop_requested`] between steps and stops early if the
/// reporter requests it.
pub fn train_with_reporter(
    mut model: Model,
    config: &TrainConfig,
    train_samples: Vec<Sample>,
    holdout_samples: Vec<Sample>,
    device: &Device,
    run_info: RunInfo,
    reporter: &mut dyn Reporter,
) -> (Model, Vec<StepMetrics>, Vec<EvalMetrics>) {
    device.seed(config.seed);

    reporter.on_event(&TrainEvent::Started(run_info));

    let mut optimizer = AdamWConfig::new()
        .with_weight_decay(config.weight_decay)
        .init();
    let mut scheduler = build_scheduler(config);

    let train_dataset = InMemDataset::new(train_samples);
    let train_loader = DataLoaderBuilder::new(NetBatcher::new(config.seed, true))
        .batch_size(config.batch_size)
        .shuffle(config.seed)
        // Background batch workers: encoding + D4 augmentation run on
        // CPU while the GPU computes. Measured 2026-11-11 (M5 Max,
        // metal device, batch 256): 0.333 -> 0.256 s/step vs 0
        // workers; on flex 1.49 -> 1.27 s/step. See
        // TrainConfig::dataloader_workers for the reproducibility
        // trade-off.
        .num_workers(config.dataloader_workers)
        .set_device(device.clone())
        .build(train_dataset);

    let holdout_dataset = InMemDataset::new(holdout_samples);
    let holdout_loader = DataLoaderBuilder::new(NetBatcher::holdout())
        .batch_size(config.batch_size)
        .num_workers(0)
        .set_device(device.clone())
        .build(holdout_dataset);

    let mut step_metrics = Vec::new();
    let mut eval_metrics = Vec::new();

    let mut train_iter = train_loader.iter();

    for step in 1..=config.steps {
        if reporter.stop_requested() {
            reporter.on_event(&TrainEvent::Finished(FinishInfo {
                stop_reason: StopReason::StopRequested,
            }));
            return (model, step_metrics, eval_metrics);
        }

        let batch = loop {
            match train_iter.next() {
                Some(batch) => break batch.expect("train loader batch"),
                None => {
                    // Start a new epoch with the same shuffled order.
                    train_iter = train_loader.iter();
                }
            }
        };
        let lr = scheduler.step().base();
        let (loss, split) = training_step(&mut model, &batch, &mut optimizer, lr, device);
        let loss_f: f32 = loss.into_scalar();
        let metrics = StepMetrics {
            step,
            loss: loss_f,
            policy_loss: split.policy,
            value_loss: split.value,
            lr,
        };
        reporter.on_event(&TrainEvent::Step(metrics));
        step_metrics.push(metrics);

        if config.eval_interval > 0 && step % config.eval_interval == 0 {
            let eval = evaluate(&model, &holdout_loader, device);
            let eval_metrics_entry = EvalMetrics { step, ..eval };
            reporter.on_event(&TrainEvent::Eval(EvalReport {
                metrics: eval_metrics_entry,
            }));
            eval_metrics.push(eval_metrics_entry);
        }
    }

    reporter.on_event(&TrainEvent::Finished(FinishInfo {
        stop_reason: StopReason::Completed,
    }));

    (model, step_metrics, eval_metrics)
}

fn build_scheduler(config: &TrainConfig) -> ModuleLrScheduler {
    // Warmup: linear ramp from a tiny positive value to 1 over warmup_steps.
    // The composed scheduler multiplies this by the cosine decay, so the
    // effective LR goes from near 0 up to lr_max, then cosine-decays to lr_min.
    let warmup = LinearLrSchedulerConfig::new(1e-10, 1.0, config.lr_warmup_steps);
    // Decay: cosine from lr_max down to lr_min over cosine_steps.
    let decay = CosineAnnealingLrSchedulerConfig::new(config.lr_max, config.lr_cosine_steps)
        .with_min_lr(config.lr_min);

    ComposedLrSchedulerConfig::new()
        .linear(warmup)
        .cosine(decay)
        .init()
        .expect("composed scheduler valid")
}

fn training_step(
    model: &mut Model,
    batch: &NetBatch,
    optimizer: &mut ModuleOptimizer,
    lr: f64,
    _device: &Device,
) -> (burn::tensor::Tensor<1>, loss::LossOutput) {
    let output = model.forward(batch.input.clone());
    let (loss, split) = loss::compute_loss_split(LossInput {
        policy_logits: output.policy,
        policy_target: batch.policy_target.clone(),
        value_pred: output.value,
        value_target: batch.value_target.clone(),
    });

    let grads = GradientsParams::from_grads(loss.backward(), model);
    *model = optimizer.step(lr, model.clone(), grads);

    (loss.detach(), split)
}

fn evaluate(
    model: &Model,
    loader: &std::sync::Arc<dyn burn::data::dataloader::DataLoader<NetBatch>>,
    _device: &Device,
) -> EvalMetrics {
    let mut total_loss = 0.0f32;
    let mut total_policy_correct = 0u64;
    let mut total_value_correct = 0u64;
    let mut total_samples = 0u64;

    for batch in loader.iter() {
        let batch = batch.expect("holdout loader batch");
        let output = model.forward(batch.input.clone());
        let loss = loss::compute_loss(LossInput {
            policy_logits: output.policy.clone(),
            policy_target: batch.policy_target.clone(),
            value_pred: output.value.clone(),
            value_target: batch.value_target.clone(),
        });
        let metrics = loss::compute_metrics(
            &output.policy,
            &batch.policy_target,
            &output.value,
            &batch.value_target,
        );
        let batch_size = batch.input.dims()[0];
        let loss_f: f32 = loss.into_scalar();
        total_loss += loss_f * batch_size as f32;
        total_policy_correct += (metrics.policy_top1_accuracy * batch_size as f32) as u64;
        total_value_correct += (metrics.value_sign_accuracy * batch_size as f32) as u64;
        total_samples += batch_size as u64;
    }

    EvalMetrics {
        step: 0,
        loss: if total_samples > 0 {
            total_loss / total_samples as f32
        } else {
            0.0
        },
        policy_top1_accuracy: if total_samples > 0 {
            total_policy_correct as f32 / total_samples as f32
        } else {
            0.0
        },
        value_sign_accuracy: if total_samples > 0 {
            total_value_correct as f32 / total_samples as f32
        } else {
            0.0
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use burn::data::dataloader::batcher::Batcher;
    use burn::tensor::Device;
    use engine::Move;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use train::Sample;
    use train::collect::{Quotas, collect};

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    fn tiny_dataset(seed: u64, quotas: Quotas) -> Vec<Sample> {
        collect(quotas, &mut StdRng::seed_from_u64(seed))
    }

    fn sample_with_policy(policy: Vec<(Move, f32)>, value: f32) -> Sample {
        Sample {
            black: vec![mv(7, 7)],
            white: vec![],
            to_move: engine::Color::White,
            policy,
            value,
        }
    }

    #[test]
    fn single_batch_overfit_loss_decreases() {
        let device = Device::flex().autodiff();
        let model: Model = crate::network::ModelConfig::tiny().init(&device);

        // One fixed batch with a strong, learnable signal.
        let sample = sample_with_policy(vec![(mv(0, 0), 1.0)], 1.0);
        let batcher = NetBatcher::holdout();
        let batch: NetBatch = batcher.batch(vec![sample.clone(); 16], &device);

        let mut optimizer = AdamWConfig::new().with_weight_decay(1e-4).init();
        let mut model = model;

        let initial_loss = {
            let out = model.forward(batch.input.clone());
            let loss = loss::compute_loss(LossInput {
                policy_logits: out.policy,
                policy_target: batch.policy_target.clone(),
                value_pred: out.value,
                value_target: batch.value_target.clone(),
            });
            loss.into_scalar::<f32>()
        };

        let mut final_loss = initial_loss;
        for _ in 0..50 {
            let out = model.forward(batch.input.clone());
            let loss = loss::compute_loss(LossInput {
                policy_logits: out.policy,
                policy_target: batch.policy_target.clone(),
                value_pred: out.value,
                value_target: batch.value_target.clone(),
            });
            final_loss = loss.clone().into_scalar::<f32>();
            let grads = GradientsParams::from_grads(loss.backward(), &model);
            model = optimizer.step(1e-3, model, grads);
        }

        assert!(
            final_loss < initial_loss,
            "loss must decrease: initial {initial_loss} final {final_loss}"
        );
    }

    #[test]
    fn seeded_training_is_reproducible() {
        let device = Device::flex().autodiff();
        let samples = tiny_dataset(
            42,
            Quotas {
                win: 4,
                block: 4,
                quiet: 8,
            },
        );
        let train_samples = samples.clone();
        let holdout_samples = Vec::new();

        let config = TrainConfig {
            steps: 20,
            batch_size: 4,
            eval_interval: 0,
            seed: 7,
            lr_warmup_steps: 5,
            lr_max: 1e-3,
            lr_min: 1e-5,
            lr_cosine_steps: 15,
            weight_decay: 1e-4,
            dataloader_workers: 0,
        };

        let model_config = crate::network::ModelConfig::tiny();

        // Warm up the backend so any lazy compilation / kernel selection
        // happens before the reproducibility comparison.
        let warm_model: Model = model_config.init(&device);
        let _ = train(
            warm_model,
            &config,
            &model_config,
            train_samples.clone(),
            holdout_samples.clone(),
            &device,
        );

        // Start from the same model state for both runs. This isolates
        // training-loop determinism from model-init backend quirks.
        let base_model: Model = model_config.init(&device);
        let model_a = base_model.clone();
        let model_b = base_model;

        let (_, metrics_a, _) = train(
            model_a,
            &config,
            &model_config,
            train_samples.clone(),
            holdout_samples.clone(),
            &device,
        );
        let (_, metrics_b, _) = train(
            model_b,
            &config,
            &model_config,
            train_samples,
            holdout_samples,
            &device,
        );

        let losses_a: Vec<f32> = metrics_a.iter().map(|m| m.loss).collect();
        let losses_b: Vec<f32> = metrics_b.iter().map(|m| m.loss).collect();
        assert_eq!(
            losses_a, losses_b,
            "two seeded runs must produce identical loss trajectory"
        );
    }

    #[test]
    fn reporter_emits_ordered_events() {
        let device = Device::flex().autodiff();
        let samples = tiny_dataset(
            42,
            Quotas {
                win: 2,
                block: 2,
                quiet: 4,
            },
        );
        let train_samples = samples.clone();
        let holdout_samples = samples;

        let config = TrainConfig {
            steps: 10,
            batch_size: 4,
            eval_interval: 3,
            seed: 1,
            lr_warmup_steps: 2,
            lr_max: 1e-3,
            lr_min: 1e-5,
            lr_cosine_steps: 8,
            weight_decay: 1e-4,
        };

        let model_config = crate::network::ModelConfig::tiny();
        let model: Model = model_config.init(&device);
        let mut reporter = crate::report::RecordingReporter::new();
        let run_info = RunInfo {
            model_config,
            train_config: config,
            device_name: format!("{device:?}"),
            train_samples: train_samples.len(),
            holdout_samples: holdout_samples.len(),
        };

        let _ = train_with_reporter(
            model,
            &config,
            train_samples,
            holdout_samples,
            &device,
            run_info,
            &mut reporter,
        );

        let events = reporter.events();
        assert!(matches!(events.first(), Some(TrainEvent::Started(_))));
        assert!(
            matches!(events.last(), Some(TrainEvent::Finished(f)) if f.stop_reason == StopReason::Completed)
        );

        let step_count = events
            .iter()
            .filter(|e| matches!(e, TrainEvent::Step(_)))
            .count();
        let eval_count = events
            .iter()
            .filter(|e| matches!(e, TrainEvent::Eval(_)))
            .count();
        assert_eq!(step_count, config.steps);
        assert_eq!(eval_count, config.steps / config.eval_interval);

        // Started, all steps, evals interleaved at their steps, Finished.
        let mut expected_index = 0;
        assert!(matches!(&events[expected_index], TrainEvent::Started(_)));
        expected_index += 1;
        for step in 1..=config.steps {
            assert!(matches!(&events[expected_index], TrainEvent::Step(m) if m.step == step));
            expected_index += 1;
            if step % config.eval_interval == 0 {
                assert!(
                    matches!(&events[expected_index], TrainEvent::Eval(e) if e.metrics.step == step)
                );
                expected_index += 1;
            }
        }
        assert!(
            matches!(&events[expected_index], TrainEvent::Finished(f) if f.stop_reason == StopReason::Completed)
        );
    }

    #[test]
    fn seeded_training_with_reporter_is_reproducible() {
        let device = Device::flex().autodiff();
        let samples = tiny_dataset(
            42,
            Quotas {
                win: 4,
                block: 4,
                quiet: 8,
            },
        );
        let train_samples = samples.clone();
        let holdout_samples = Vec::new();

        let config = TrainConfig {
            steps: 20,
            batch_size: 4,
            eval_interval: 0,
            seed: 7,
            lr_warmup_steps: 5,
            lr_max: 1e-3,
            lr_min: 1e-5,
            lr_cosine_steps: 15,
            weight_decay: 1e-4,
        };

        let model_config = crate::network::ModelConfig::tiny();

        // Warm up the backend so any lazy compilation / kernel selection
        // happens before the reproducibility comparison.
        let warm_model: Model = model_config.init(&device);
        let _ = train(
            warm_model,
            &config,
            &model_config,
            train_samples.clone(),
            holdout_samples.clone(),
            &device,
        );

        let base_model: Model = model_config.init(&device);
        let run_info = RunInfo {
            model_config,
            train_config: config,
            device_name: format!("{device:?}"),
            train_samples: train_samples.len(),
            holdout_samples: holdout_samples.len(),
        };

        let mut reporter_a = crate::report::RecordingReporter::new();
        let mut reporter_b = crate::report::RecordingReporter::new();

        let (_, metrics_a, _) = train_with_reporter(
            base_model.clone(),
            &config,
            train_samples.clone(),
            holdout_samples.clone(),
            &device,
            run_info.clone(),
            &mut reporter_a,
        );
        let (_, metrics_b, _) = train_with_reporter(
            base_model,
            &config,
            train_samples,
            holdout_samples,
            &device,
            run_info,
            &mut reporter_b,
        );

        assert_eq!(
            metrics_a.iter().map(|m| m.loss).collect::<Vec<_>>(),
            metrics_b.iter().map(|m| m.loss).collect::<Vec<_>>(),
            "loss trajectories must match with reporter attached"
        );
        assert_eq!(
            metrics_a
                .iter()
                .map(|m| (m.policy_loss, m.value_loss))
                .collect::<Vec<_>>(),
            metrics_b
                .iter()
                .map(|m| (m.policy_loss, m.value_loss))
                .collect::<Vec<_>>(),
            "loss decomposition must match with reporter attached"
        );
        assert_eq!(reporter_a.events(), reporter_b.events());
    }

    #[test]
    fn reporter_does_not_change_loss_trajectory() {
        let device = Device::flex().autodiff();
        let samples = tiny_dataset(
            42,
            Quotas {
                win: 4,
                block: 4,
                quiet: 8,
            },
        );
        let train_samples = samples.clone();
        let holdout_samples = Vec::new();

        let config = TrainConfig {
            steps: 20,
            batch_size: 4,
            eval_interval: 0,
            seed: 7,
            lr_warmup_steps: 5,
            lr_max: 1e-3,
            lr_min: 1e-5,
            lr_cosine_steps: 15,
            weight_decay: 1e-4,
        };

        let model_config = crate::network::ModelConfig::tiny();

        // Warm up the backend so any lazy compilation / kernel selection
        // happens before the reproducibility comparison.
        let warm_model: Model = model_config.init(&device);
        let _ = train(
            warm_model,
            &config,
            &model_config,
            train_samples.clone(),
            holdout_samples.clone(),
            &device,
        );

        // Start from the same model state for both runs. This isolates
        // training-loop determinism from model-init backend quirks.
        let base_model: Model = model_config.init(&device);
        let run_info = RunInfo {
            model_config: model_config.clone(),
            train_config: config,
            device_name: format!("{device:?}"),
            train_samples: train_samples.len(),
            holdout_samples: holdout_samples.len(),
        };

        let (_, metrics_null, _) = train(
            base_model.clone(),
            &config,
            &model_config,
            train_samples.clone(),
            holdout_samples.clone(),
            &device,
        );

        let mut reporter = crate::report::RecordingReporter::new();
        let (_, metrics_recorded, _) = train_with_reporter(
            base_model,
            &config,
            train_samples,
            holdout_samples,
            &device,
            run_info,
            &mut reporter,
        );

        assert_eq!(
            metrics_null.len(),
            metrics_recorded.len(),
            "reporter run must produce the same number of steps"
        );
        assert_eq!(
            metrics_null.iter().map(|m| m.loss).collect::<Vec<_>>(),
            metrics_recorded.iter().map(|m| m.loss).collect::<Vec<_>>(),
            "loss trajectory must match with null reporter vs recording reporter"
        );
        assert_eq!(
            metrics_null
                .iter()
                .map(|m| m.policy_loss)
                .collect::<Vec<_>>(),
            metrics_recorded
                .iter()
                .map(|m| m.policy_loss)
                .collect::<Vec<_>>(),
            "policy loss must match with null reporter vs recording reporter"
        );
        assert_eq!(
            metrics_null
                .iter()
                .map(|m| m.value_loss)
                .collect::<Vec<_>>(),
            metrics_recorded
                .iter()
                .map(|m| m.value_loss)
                .collect::<Vec<_>>(),
            "value loss must match with null reporter vs recording reporter"
        );

        let events = reporter.events();
        let started_count = events
            .iter()
            .filter(|e| matches!(e, TrainEvent::Started(_)))
            .count();
        let step_count = events
            .iter()
            .filter(|e| matches!(e, TrainEvent::Step(_)))
            .count();
        let finished_count = events
            .iter()
            .filter(|e| matches!(e, TrainEvent::Finished(_)))
            .count();
        assert_eq!(
            started_count, 1,
            "recording must contain exactly one Started event"
        );
        assert_eq!(
            step_count, config.steps,
            "recording must contain one Step event per step"
        );
        assert_eq!(
            finished_count, 1,
            "recording must contain exactly one Finished event"
        );
    }

    #[test]
    fn stop_requested_stops_early() {
        struct StopAfterK {
            events: Vec<crate::report::TrainEvent>,
            threshold: usize,
        }
        impl crate::report::Reporter for StopAfterK {
            fn on_event(&mut self, event: &crate::report::TrainEvent) {
                self.events.push(event.clone());
            }
            fn stop_requested(&self) -> bool {
                self.events.len() >= self.threshold
            }
        }

        let device = Device::flex().autodiff();
        let samples = tiny_dataset(
            42,
            Quotas {
                win: 2,
                block: 2,
                quiet: 4,
            },
        );
        let train_samples = samples.clone();
        let holdout_samples = Vec::new();

        let config = TrainConfig {
            steps: 20,
            batch_size: 4,
            eval_interval: 0,
            seed: 1,
            lr_warmup_steps: 2,
            lr_max: 1e-3,
            lr_min: 1e-5,
            lr_cosine_steps: 18,
            weight_decay: 1e-4,
        };

        let model_config = crate::network::ModelConfig::tiny();
        let model: Model = model_config.init(&device);
        let mut reporter = StopAfterK {
            events: Vec::new(),
            threshold: 3, // Started + 2 Step -> stop before step 3
        };
        let run_info = RunInfo {
            model_config,
            train_config: config,
            device_name: format!("{device:?}"),
            train_samples: train_samples.len(),
            holdout_samples: holdout_samples.len(),
        };

        let (_, metrics, _) = train_with_reporter(
            model,
            &config,
            train_samples,
            holdout_samples,
            &device,
            run_info,
            &mut reporter,
        );

        assert_eq!(metrics.len(), 2, "stop must interrupt after two steps");
        assert!(matches!(
            reporter.events.last(),
            Some(TrainEvent::Finished(f)) if f.stop_reason == StopReason::StopRequested
        ));
    }

    #[test]
    fn step_metrics_split_sums_to_total() {
        let device = Device::flex().autodiff();
        let samples = tiny_dataset(
            42,
            Quotas {
                win: 4,
                block: 4,
                quiet: 8,
            },
        );
        let train_samples = samples.clone();
        let holdout_samples = Vec::new();

        let config = TrainConfig {
            steps: 5,
            batch_size: 4,
            eval_interval: 0,
            seed: 3,
            lr_warmup_steps: 2,
            lr_max: 1e-3,
            lr_min: 1e-5,
            lr_cosine_steps: 3,
            weight_decay: 1e-4,
        };

        let model_config = crate::network::ModelConfig::tiny();
        let model: Model = model_config.init(&device);
        let (_, metrics, _) = train(
            model,
            &config,
            &model_config,
            train_samples,
            holdout_samples,
            &device,
        );

        for m in &metrics {
            assert!(
                (m.loss - (m.policy_loss + m.value_loss)).abs() < 1e-4,
                "step {} combined loss {} must equal policy {} + value {}",
                m.step,
                m.loss,
                m.policy_loss,
                m.value_loss
            );
        }
    }
}
