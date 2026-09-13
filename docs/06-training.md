# Chapter 6 — Training with SupervisedTraining

Run the example: `cargo run --release --example 05_mnist_train -- 2`

## The contract: TrainStep and InferenceStep

The training API needs to know how your model handles one batch. You say it
with two trait implementations (in `src/training.rs`):

```rust
impl<B: AutodiffBackend> TrainStep for Model<B> {
    type Input = MnistBatch<B>;
    type Output = ClassificationOutput<B>;

    fn step(&self, batch: MnistBatch<B>) -> TrainOutput<ClassificationOutput<B>> {
        let item = self.forward_classification(batch.images, batch.targets);
        TrainOutput::new(self, item.loss.backward(), item)
    }
}
```

- `TrainStep::step` computes the loss **and** its gradients. The returned
  `TrainOutput` carries the gradients for the optimizer and the item for the
  metrics.
- `InferenceStep::step` is the validation counterpart: same computation, no
  gradients. It runs on the inner (non-autodiff) backend automatically.
- `ClassificationOutput<B>` bundles `loss`, `output` (logits), and
  `targets`. Burn's built-in classification metrics know how to read it.

Note where your code ends and the framework begins: you write the per-batch
logic. The framework iterates epochs and batches, switches the model between
train and valid modes, applies the optimizer, aggregates metrics, renders
the dashboard, and writes checkpoints.

## Assembling a run

```rust
let training = SupervisedTraining::new(artifact_dir, dataloader_train, dataloader_valid)
    .metrics((AccuracyMetric::new(), LossMetric::new()))
    .with_file_checkpointer(CompactRecorder::new())
    .num_epochs(config.num_epochs)
    .summary();

let result = training.launch(Learner::new(model, config.optimizer.init(), lr));
```

- `SupervisedTraining` is the builder for the whole run. Artifacts (metrics
  logs, checkpoints, the epoch summary) go to `artifact_dir`.
- `.metrics(...)` registers metrics for **both** train and validation.
  `metric_train_numeric` adds numeric-only plots (e.g. the learning rate).
- `.with_file_checkpointer(...)` saves the model per epoch with the given
  recorder.
- `Learner` bundles the three things that evolve during training: model,
  optimizer, LR schedule. `launch` consumes everything and returns the
  trained model plus the metrics renderer.

Old tutorials use `LearnerBuilder`. That API is gone in 0.21. If a blog post
shows `LearnerBuilder::new(...)`, it is stale — use `SupervisedTraining`.

## Metrics

`AccuracyMetric` and `LossMetric` cover classification. Metrics are
stateful: they update per batch and aggregate per epoch, weighted by batch
size. If you write a custom metric later (you will: policy accuracy and
value MSE for AlphaZero), copy the sample-weighting pattern from the book's
[metric chapter](https://burn.dev/books/burn/building-blocks/metric.html) —
naive per-batch averaging silently biases the epoch value when the last
batch is short.

## Early stopping and interruption

The advanced MNIST example registers metric-based early stopping:

```rust
.early_stopping(MetricEarlyStoppingStrategy::new(
    &LossMetric::<B>::new(),
    Aggregate::Mean,
    Direction::Lowest,
    Split::Valid,
    StoppingCondition::NoImprovementSince { n_epochs: 5 },
))
```

For a manual stop (Ctrl-C-friendly long runs), `training.interrupter()`
gives a handle you can trigger from a signal handler or another thread.

## What you should see

With the default config (5 epochs, batch 64, Adam @ 1e-4, Flex backend,
release build), expect validation accuracy in the high 90s. There is no
official Burn accuracy claim for this example — backend, seed, and config
all move the number. Treat your first run as **your** baseline: record the
version, backend, seed, and config, and compare future runs against it.

## Reading

- Book: [Learner](https://burn.dev/books/burn/building-blocks/learner.html),
  [Metric](https://burn.dev/books/burn/building-blocks/metric.html),
  [Basic workflow: Training](https://burn.dev/books/burn/basic-workflow/training.html)

## Try this

1. Train with 1, 2, and 5 epochs. Watch where validation loss flattens.
2. Change the learning rate to `1e-2` and observe the failure mode. Then
   explain it.
3. Register `LearningRateMetric` with `.metric_train_numeric(...)` and find
   where it appears in the artifacts directory.

Next: [Chapter 7 — MNIST end to end](07-mnist-end-to-end.md)
