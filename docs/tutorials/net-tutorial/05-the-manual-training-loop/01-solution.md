# Chapter 05 solution — The manual training loop

This file shows the reference implementation for chapter 05. It is
identical to the verified `train.rs` in the reference worktree, plus the
one-line registration change in `lib.rs`.

## `gomoku/crates/net/src/lib.rs`

Add `pub mod train;` with the other module declarations:

```rust
//! Neural network training for the Gomoku AlphaZero-style agent.
//!
//! This crate implements the policy/value network and the manual
//! supervised-training loop for phase 0. It consumes labelled samples
//! from the `train` crate and derives input planes from engine boards.

#![deny(missing_docs)]

pub mod batcher;
pub mod checkpoint;
pub mod loss;
pub mod network;
pub mod planes;
pub mod train;
```

## `gomoku/crates/net/src/train.rs`

```rust
//! Manual supervised training loop.
//!
//! This module owns the outer training loop (docs/09, docs/12): Burn
//! owns the network, optimizer, and batch math; the application owns
//! data loading, D4 augmentation, the loop, evaluation cadence, and
//! checkpointing.

use burn::data::dataloader::DataLoaderBuilder;
use burn::data::dataset::InMemDataset;
use burn::optim::AdamWConfig;
use burn::optim::adaptor::OptimizerAdaptor;
use burn::optim::lr_scheduler::LrScheduler;
use burn::optim::lr_scheduler::composed::ComposedLrSchedulerConfig;
use burn::optim::lr_scheduler::cosine::CosineAnnealingLrSchedulerConfig;
use burn::optim::lr_scheduler::linear::LinearLrSchedulerConfig;
use burn::optim::{AdamW, GradientsParams, Optimizer};
use burn::prelude::ElementConversion;
use burn::tensor::backend::AutodiffBackend;
use train::Sample;

use crate::batcher::{NetBatch, NetBatcher};
use crate::loss::{self, LossInput};
use crate::network::Model;

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
/// holdout evaluation metrics.
pub fn train<B: AutodiffBackend>(
    mut model: Model<B>,
    config: &TrainConfig,
    train_samples: Vec<Sample>,
    holdout_samples: Vec<Sample>,
    device: &B::Device,
) -> (Model<B>, Vec<StepMetrics>, Vec<EvalMetrics>) {
    B::seed(device, config.seed);

    let mut optimizer = AdamWConfig::new()
        .with_weight_decay(config.weight_decay)
        .init::<B, Model<B>>();
    let mut scheduler = build_scheduler(config);

    let train_dataset = InMemDataset::new(train_samples);
    let train_loader = DataLoaderBuilder::new(NetBatcher::new(config.seed, true))
        .batch_size(config.batch_size)
        .shuffle(config.seed)
        .num_workers(0)
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
        let batch = loop {
            match train_iter.next() {
                Some(batch) => break batch,
                None => {
                    // Start a new epoch with the same shuffled order.
                    train_iter = train_loader.iter();
                }
            }
        };
        let lr = scheduler.step();
        let loss = training_step(&mut model, &batch, &mut optimizer, lr, device);
        let loss_f: f32 = loss.into_scalar().elem();
        step_metrics.push(StepMetrics {
            step,
            loss: loss_f,
            lr,
        });

        if config.eval_interval > 0 && step % config.eval_interval == 0 {
            let eval = evaluate(&model, &holdout_loader, device);
            eval_metrics.push(EvalMetrics { step, ..eval });
        }
    }

    (model, step_metrics, eval_metrics)
}

fn build_scheduler(
    config: &TrainConfig,
) -> burn::optim::lr_scheduler::composed::ComposedLrScheduler {
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

fn training_step<B: AutodiffBackend>(
    model: &mut Model<B>,
    batch: &NetBatch<B>,
    optimizer: &mut OptimizerAdaptor<AdamW, Model<B>, B>,
    lr: f64,
    _device: &B::Device,
) -> burn::tensor::Tensor<B, 1> {
    let output = model.forward(batch.input.clone());
    let loss = loss::compute_loss(LossInput {
        policy_logits: output.policy,
        policy_target: batch.policy_target.clone(),
        value_pred: output.value,
        value_target: batch.value_target.clone(),
    });

    let grads = GradientsParams::from_grads(loss.backward(), model);
    *model = optimizer.step(lr, model.clone(), grads);

    loss.detach()
}

fn evaluate<B: AutodiffBackend>(
    model: &Model<B>,
    loader: &std::sync::Arc<dyn burn::data::dataloader::DataLoader<B, NetBatch<B>>>,
    _device: &B::Device,
) -> EvalMetrics {
    let mut total_loss = 0.0f32;
    let mut total_policy_correct = 0u64;
    let mut total_value_correct = 0u64;
    let mut total_samples = 0u64;

    for batch in loader.iter() {
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
        let loss_f: f32 = loss.into_scalar().elem();
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
    use burn::backend::flex::FlexDevice;
    use burn::backend::{Autodiff, Flex};
    use burn::data::dataloader::batcher::Batcher;
    use engine::Move;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use train::Sample;
    use train::collect::{Quotas, collect};

    type B = Autodiff<Flex>;

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
        let device = FlexDevice;
        let model: Model<B> = crate::network::ModelConfig::tiny().init(&device);

        // One fixed batch with a strong, learnable signal.
        let sample = sample_with_policy(vec![(mv(0, 0), 1.0)], 1.0);
        let batcher = NetBatcher::holdout();
        let batch: NetBatch<B> = batcher.batch(vec![sample.clone(); 16], &device);

        let mut optimizer = AdamWConfig::new()
            .with_weight_decay(1e-4)
            .init::<B, Model<B>>();
        let mut model = model;

        let initial_loss = {
            let out = model.forward(batch.input.clone());
            let loss = loss::compute_loss(LossInput {
                policy_logits: out.policy,
                policy_target: batch.policy_target.clone(),
                value_pred: out.value,
                value_target: batch.value_target.clone(),
            });
            loss.into_scalar().elem::<f32>()
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
            final_loss = loss.clone().into_scalar().elem::<f32>();
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
        let device = FlexDevice;
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

        // Warm up the backend so any lazy compilation / kernel selection
        // happens before the reproducibility comparison.
        let warm_model: Model<B> = crate::network::ModelConfig::tiny().init(&device);
        let _ = train(
            warm_model,
            &config,
            train_samples.clone(),
            holdout_samples.clone(),
            &device,
        );

        // Start from the same model state for both runs. This isolates
        // training-loop determinism from model-init backend quirks.
        let base_model: Model<B> = crate::network::ModelConfig::tiny().init(&device);
        let model_a = base_model.clone();
        let model_b = base_model;

        let (_, metrics_a, _) = train(
            model_a,
            &config,
            train_samples.clone(),
            holdout_samples.clone(),
            &device,
        );
        let (_, metrics_b, _) = train(model_b, &config, train_samples, holdout_samples, &device);

        let losses_a: Vec<f32> = metrics_a.iter().map(|m| m.loss).collect();
        let losses_b: Vec<f32> = metrics_b.iter().map(|m| m.loss).collect();
        assert_eq!(
            losses_a, losses_b,
            "two seeded runs must produce identical loss trajectory"
        );
    }
}
```
