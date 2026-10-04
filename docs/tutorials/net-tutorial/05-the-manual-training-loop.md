# Chapter 05 — The manual training loop

## Abstract

This chapter adds the outer supervised-training loop to the `net`
crate. Burn owns the network, the optimizer, and the batch math; the
application owns data loading, the loop, evaluation cadence, and
checkpointing (locked in
[`docs/09-toward-alphazero.md`](../../09-toward-alphazero.md)). The
loop uses AdamW with decoupled weight decay, a warmup-plus-cosine
learning-rate schedule, a seeded shuffling data loader that cycles
through epochs, and periodic holdout evaluation. By the end you will
have `train.rs` with two end-to-end tests: a single-batch overfit that
proves gradients flow, and a seeded reproducibility test that proves the
loop is deterministic.

## Glossary

| Term | Definition |
|------|------------|
| **Manual training loop** | Application-owned code that iterates batches, calls `loss.backward()`, and applies `optimizer.step()`, rather than using Burn's `SupervisedTraining` helper. |
| **AdamW** | Adam optimizer with decoupled weight decay. The weight-decay term is added directly to the gradient, not folded into the adaptive moment estimate. |
| **Warmup-plus-cosine schedule** | Learning rate starts near zero, linearly ramps to a peak over `lr_warmup_steps`, then cosine-decays to a minimum over `lr_cosine_steps`. |
| **ComposedLrScheduler** | Burn 0.21 scheduler that multiplies (or sums) the outputs of child schedulers. Here it multiplies a warmup ramp by a cosine decay. |
| **`AutodiffBackend`** | The Burn backend trait that records operations for automatic differentiation; required for `backward()`. |
| **`GradientsParams`** | Burn type that wraps raw gradients and maps them to a module's parameters for `optimizer.step()`. |
| **Holdout evaluation** | Running the model on a separate dataset at fixed intervals to report loss and metrics without updating weights. |

## Context

Chapter 3 built the network and chapter 4 added the loss and metrics.
This chapter wires them into a training procedure. The design split is
locked in
[`docs/09-toward-alphazero.md`](../../09-toward-alphazero.md): Burn owns
the network, optimizer, and batch math; the application owns data
loading, D4 augmentation, the outer loop, evaluation cadence, and
checkpointing. That split matters because the project's data arrives
continuously from self-play, not as a fixed supervised dataset, so the
`SupervisedTraining` convenience helper is not the right shape.

The optimizer and schedule numbers are locked in
[`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) §10
and §11: AdamW with weight decay `1e-4`, batch size 1024, linear warmup
over ~2,000 steps, cosine decay `3e-4 → 3e-6`. The implementation makes
these configurable so tests can use tiny values.

## Intention

1. Create `gomoku/crates/net/src/train.rs` and register
   `pub mod train;` in `gomoku/crates/net/src/lib.rs`.
2. Define `TrainConfig` with serializable fields: `steps`, `batch_size`,
   `eval_interval`, `seed`, `lr_warmup_steps`, `lr_max`, `lr_min`,
   `lr_cosine_steps`, `weight_decay`. Add a `tiny()` constructor for
   tests.
3. Define `StepMetrics` and `EvalMetrics` records for logging.
4. Implement `train<B: AutodiffBackend>(model, config, train_samples,
   holdout_samples, device)`:
   * seed the backend with `B::seed(device, config.seed)`;
   * build an AdamW optimizer from `AdamWConfig`;
   * build a warmup-plus-cosine scheduler using
     `ComposedLrSchedulerConfig`;
   * build a shuffling training data loader with `NetBatcher` and an
     unshuffled holdout loader with the canonical batcher;
   * iterate `config.steps`, drawing the next batch (cycling the loader
     when an epoch ends), running `training_step`, and recording metrics;
   * run `evaluate` on the holdout set every `eval_interval` steps.
5. Implement `training_step`:
   * forward the batch through the model;
   * compute the combined loss;
   * call `loss.backward()` and wrap gradients with `GradientsParams`;
   * apply `optimizer.step(lr, model.clone(), grads)` and update the
     model.
6. Implement `evaluate`:
   * iterate the holdout loader;
   * accumulate loss and metrics weighted by batch size;
   * return averaged `EvalMetrics`.
7. Implement `build_scheduler`:
   * linear warmup `1e-10 → 1.0` over `lr_warmup_steps`;
   * cosine decay `lr_max → lr_min` over `lr_cosine_steps`;
   * multiply them with `ComposedLrSchedulerConfig`.
8. Write two tests:
   * `single_batch_overfit_loss_decreases` — repeatedly optimize on one
     fixed batch of 16 identical samples for 50 steps and assert the
     final loss is lower than the initial loss.
   * `seeded_training_is_reproducible` — run the full `train` function
     twice from the same cloned starting model and assert the loss
     trajectories are identical.

Observable done-state: `cargo test -p net` passes, `cargo clippy
--all-targets -- -D warnings` is green, and `cargo fmt --all --
--check` makes no changes.

## Mental mapping

### Why the loop is manual

Burn 0.21 provides `SupervisedTraining`, a high-level API that bundles
data loader, learner, and metrics. For a fixed MNIST-style dataset it is
convenient. For the AlphaZero loop it is the wrong shape because:

* Training data arrives continuously from self-play, not as a fixed
  corpus.
* The project wants explicit control over evaluation cadence,
  checkpointing, and run journaling.
* The replay buffer and augmentation logic live outside Burn, in the
  `train` crate and the `NetBatcher`.

A manual loop is therefore not extra work — it is the required
architecture. The code is only a few dozen lines: forward, backward,
step, log.

> **Excursion — stale API names in old tutorials**
>
> Pre-0.21 tutorials and blog posts often use `LearnerBuilder`, the
> `Infer` trait, or `InitRecord`. Those names are gone in Burn 0.21.
> The current names are `SupervisedTraining` (plus `Learner`),
> `InferenceStep`, and `Config::init` / `load_record`. If you search
> the web for Burn training examples, check the version first; this
> project is pinned to 0.21, and 0.22-pre already removes the backend
> type parameter from `Tensor` and `Module`. This is the same trap
> listed in [`docs/11-pitfalls.md`](../../11-pitfalls.md).

> **Excursion — the `1e-10` warmup workaround**
>
> A natural warmup scheduler would start at `0.0` and ramp linearly to
> the peak learning rate. Burn 0.21's `LinearLrSchedulerConfig` rejects
> `initial_lr <= 0.0` with the error "Initial learning rate must be
> greater than 0 and at most 1" (verified against the pinned source in
> `burn-optim-0.21.0/src/lr_scheduler/linear.rs`).
>
> The workaround used in the reference implementation is to start the
> warmup at `1e-10` and end at `1.0`, then multiply it by a cosine
> scheduler that goes from `lr_max` down to `lr_min`. The product is
> `1e-10 * lr_max ≈ 0` at step 0, `1.0 * lr_max` at the end of warmup,
> and follows the cosine decay thereafter. The tiny positive start is
> indistinguishable from zero in practice but satisfies Burn's range
> check.
>
> The composed scheduler is built as:
>
> ```rust
> let warmup = LinearLrSchedulerConfig::new(1e-10, 1.0, config.lr_warmup_steps);
> let decay = CosineAnnealingLrSchedulerConfig::new(config.lr_max, config.lr_cosine_steps)
>     .with_min_lr(config.lr_min);
>
> ComposedLrSchedulerConfig::new()
>     .linear(warmup)
>     .cosine(decay)
>     .init()
>     .expect("composed scheduler valid")
> ```
>
> Burn's `ComposedLrScheduler` defaults to `SchedulerReduction::Prod`,
> so the child learning rates are multiplied. Verify that the warmup
> and cosine step counts sum to `config.steps` for the production
> schedule (2,000 + decay steps = total steps).

> **Excursion — cycling the data loader**
>
> Burn's `DataLoader` iterator returns `None` when an epoch finishes.
> The training loop must restart it, otherwise a short dataset would
> terminate training early. The reference wraps each batch fetch in a
> `loop { match iter.next() { Some(b) => break b, None => iter = loader.iter() } }`.
> This is simple and correct; it is also deterministic because the
> loader uses the same seed on every restart, so the epoch order is
> fixed. A more sophisticated sampler might reshuffle each epoch, but
> for the tutorial's deterministic policy a fixed shuffle is the right
> starting point.

> **Excursion — reproducibility and cloned models**
>
> Chapter 3 noted that `B::seed` does not make two independent
> `Model::init` calls bit-identical. The reproducibility test in this
> chapter therefore initializes one model and `clone()`s it for the
> second run. Both runs start from identical weights, use the same
> seeded data loader, and call `B::seed` with the same seed, so every
> gradient and every optimizer step must match. The test asserts that
> the per-step loss vectors are equal.
>
> One subtlety: some backends perform lazy compilation or kernel
> selection on the first forward pass. To avoid having the warm-up run
> affect the reproducibility comparison, the reference runs one throw-
> away training call before cloning the base model. That primes any
> backend caches so the two measured runs see identical state.

## Low-level design

### Files

```text
gomoku/crates/net/
└── src/
    ├── lib.rs          # add `pub mod train;`
    └── train.rs        # new module
```

### `src/lib.rs`

Add `pub mod train;` with the existing module declarations.

### `src/train.rs`

Module-level documentation should state that the module owns the outer
training loop and cite docs/09 and docs/12.

Required imports:

```rust
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
```

`TrainConfig`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TrainConfig {
    pub steps: usize,
    pub batch_size: usize,
    pub eval_interval: usize,
    pub seed: u64,
    pub lr_warmup_steps: usize,
    pub lr_max: f64,
    pub lr_min: f64,
    pub lr_cosine_steps: usize,
    pub weight_decay: f32,
}
```

with `tiny()` returning `steps: 50, batch_size: 16, eval_interval: 0,
seed: 0, lr_warmup_steps: 5, lr_max: 1e-3, lr_min: 1e-5,
lr_cosine_steps: 45, weight_decay: 1e-4`.

`StepMetrics` and `EvalMetrics`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StepMetrics {
    pub step: usize,
    pub loss: f32,
    pub lr: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EvalMetrics {
    pub step: usize,
    pub loss: f32,
    pub policy_top1_accuracy: f32,
    pub value_sign_accuracy: f32,
}
```

`train` function skeleton:

```rust
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

    // ... loop and return
}
```

Inside the loop:

```rust
let mut step_metrics = Vec::new();
let mut eval_metrics = Vec::new();
let mut train_iter = train_loader.iter();

for step in 1..=config.steps {
    let batch = loop {
        match train_iter.next() {
            Some(batch) => break batch,
            None => {
                train_iter = train_loader.iter();
            }
        }
    };
    let lr = scheduler.step();
    let loss = training_step(&mut model, &batch, &mut optimizer, lr, device);
    let loss_f: f32 = loss.into_scalar().elem();
    step_metrics.push(StepMetrics { step, loss: loss_f, lr });

    if config.eval_interval > 0 && step % config.eval_interval == 0 {
        let eval = evaluate(&model, &holdout_loader, device);
        eval_metrics.push(EvalMetrics { step, ..eval });
    }
}

(model, step_metrics, eval_metrics)
```

`build_scheduler`:

```rust
fn build_scheduler(
    config: &TrainConfig,
) -> burn::optim::lr_scheduler::composed::ComposedLrScheduler {
    let warmup = LinearLrSchedulerConfig::new(1e-10, 1.0, config.lr_warmup_steps);
    let decay = CosineAnnealingLrSchedulerConfig::new(config.lr_max, config.lr_cosine_steps)
        .with_min_lr(config.lr_min);

    ComposedLrSchedulerConfig::new()
        .linear(warmup)
        .cosine(decay)
        .init()
        .expect("composed scheduler valid")
}
```

`training_step`:

```rust
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
```

`evaluate` accumulates size-weighted totals:

```rust
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
        loss: total_loss / total_samples as f32,
        policy_top1_accuracy: total_policy_correct as f32 / total_samples as f32,
        value_sign_accuracy: total_value_correct as f32 / total_samples as f32,
    }
}
```

Guard against division by zero if `holdout_samples` is empty.

### Tests

Use `burn::backend::{Autodiff, Flex}` and `burn::backend::flex::FlexDevice`:

```rust
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
}
```

Helper `mv(r, c)` builds `Move::new(r, c).unwrap()`. Helper
`sample_with_policy(policy, value)` builds a minimal `Sample` with one
black move at the center and an empty white list, so the batcher can
encode it. Helper `tiny_dataset(seed, quotas)` calls `collect` from the
`train` crate.

1. `single_batch_overfit_loss_decreases`:
   * Build a batch of 16 copies of a sample whose policy puts all mass
     on `(0, 0)` and whose value is `+1.0`. Use `NetBatcher::holdout()`
     so augmentation does not interfere.
   * Run 50 AdamW steps at a fixed learning rate (e.g., `1e-3`) with
     weight decay `1e-4`.
   * Assert that the final loss is strictly less than the initial loss.
   This is a vertical test: if any of network, loss, backward, or
   optimizer is broken, the loss will not decrease.

2. `seeded_training_is_reproducible`:
   * Collect a tiny dataset: `Quotas { win: 4, block: 4, quiet: 8 }`
     with seed `42`.
   * Use the full `TrainConfig` with `steps: 20`, `batch_size: 4`,
     `eval_interval: 0`, `seed: 7`, `lr_warmup_steps: 5`,
     `lr_max: 1e-3`, `lr_min: 1e-5`, `lr_cosine_steps: 15`.
   * Run one throw-away training call to warm up the backend.
   * Initialize a base model, clone it into `model_a` and `model_b`, and
     run `train` twice.
   * Compare the `loss` field of every `StepMetrics`; assert equality.

> **Forward note — holdout evaluation in later chapters**
>
> This chapter's loop supports `eval_interval > 0`, but the tests set
> it to 0 to keep them fast and deterministic. Chapter 8's acceptance
> run will exercise holdout evaluation on a real dataset.

## Solution (opt-in)

The complete reference code for this chapter — `train.rs` and the
`lib.rs` registration line — lives in
[05-the-manual-training-loop/01-solution.md](05-the-manual-training-loop/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare.

## TDD checklist

1. **Red:** Create `train.rs` with the configs, the `train`,
   `build_scheduler`, `training_step`, and `evaluate` signatures, and
   the two tests, leaving function bodies as `todo!()`. Register `pub
   mod train;` in `lib.rs`. Run `cargo test -p net` and expect failures
   from the `todo!()` panics.
2. **Green:** Implement the loop, scheduler, step, and evaluation.
   Re-run `cargo test -p net`. All eleven tests (four from chapter 3,
   five from chapter 4, and two from this chapter) should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p net` passes from `gomoku/`.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.

Next: Chapter 06 — Checkpointing + run journal.

## References

* [`docs/09-toward-alphazero.md`](../../09-toward-alphazero.md) — the
  Burn/application split: Burn owns network/optimizer/batch math; the
  application owns data loading, augmentation, the loop, evaluation, and
  checkpointing.
* [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  §10 and §11: AdamW, weight decay `1e-4`, warmup 2k steps, cosine decay
  `3e-4 → 3e-6`, batch size 1024.
* [`docs/11-pitfalls.md`](../../11-pitfalls.md) — the `argmax`
  dimension-keeping trap and the batch-size-weighted metric trap.
* [`docs/tutorials/datagen-tutorial/`](../datagen-tutorial/) — the
  `Sample` record and `collect` function used in the reproducibility
  test.
* Burn 0.21.0 source, `burn-optim-0.21.0/src/lr_scheduler/linear.rs` —
  verifies that `LinearLrSchedulerConfig` rejects `initial_lr <= 0.0`.
