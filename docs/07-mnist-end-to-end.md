# Chapter 7 — MNIST end to end

This chapter connects everything. First it walks through this repository's
training run. Then it opens the official advanced MNIST example and shows
what "production-grade" looks like in Burn.

## Our pipeline, piece by piece

| File | Role | Chapter |
|------|------|---------|
| `src/data.rs` | `MnistBatcher`: raw items → normalized tensors | 5 |
| `src/model.rs` | `ModelConfig` + `Model<B>`: small CNN | 3 |
| `src/training.rs` | `TrainStep`/`InferenceStep`, `TrainingConfig`, `train()` | 4, 6 |
| `src/inference.rs` | record loading, single prediction, ASCII rendering | 8 |
| `examples/05_mnist_train.rs` | backend selection + `main` | 6 |
| `examples/06_mnist_infer.rs` | prediction + manual accuracy evaluation | 8 |

The architecture (details in `src/model.rs`):

```text
[B, 1, 28, 28] → Conv2d(1→8, 3x3) → Dropout → Conv2d(8→16, 3x3) → Dropout → ReLU
              → AdaptiveAvgPool2d(8x8) → flatten [B, 1024]
              → Linear(1024→128) → Dropout → ReLU → Linear(128→10) → logits
```

Points worth your attention:

- **No padding, no max pooling.** `AdaptiveAvgPool2d` reduces the spatial
  dimensions to a fixed size regardless of input size. Fewer moving parts,
  same teaching value.
- **Dropout after convolutions** is unusual in modern CNNs but fine at this
  scale; the official Burn book example does the same.
- The loss is `CrossEntropyLoss` on **logits + class indices**. Never apply
  softmax before this loss — it does log-softmax internally.

## Run it

```bash
cargo run --release --example 05_mnist_train     # 5 epochs
cargo run --release --example 06_mnist_infer     # predict + evaluate
```

Always `--release` for training. Debug builds of tensor code are one to two
orders of magnitude slower.

## The official advanced example

Burn's repository contains a second, more serious MNIST at
[`examples/mnist`](https://github.com/tracel-ai/burn/tree/v0.21.0/examples/mnist).
Read it after you are comfortable with ours. What it adds, and why:

1. **A proper validation split.** `PartialDataset` carves 55,000/5,000 out
   of the training set. (Our course example validates on the test set —
   acceptable for a tutorial, wrong for research.)
2. **Data augmentation.** Translate/shear/scale/rotation via
   `burn::vision::Transform2D`, applied in a `Mapper` dataset. Sixteen
   augmented variants are combined with `ComposedDataset` and
   `SamplerDataset`.
3. **A stronger optimizer setup.** AdamW with weight decay and *cautious*
   weight decay, plus a composed LR schedule: linear warmup, then cosine
   annealing.
4. **Early stopping** on validation loss (chapter 6).
5. **BatchNorm + MaxPool conv blocks** and a bigger head (GELU, dropout
   0.25).
6. **Runtime backend selection** with `Dispatch`, so one binary runs on
   flex, wgpu, or CUDA via features.
7. **Multi-split evaluation** with `EvaluatorBuilder`: it evaluates the
   trained model on plain *and* transformed test sets to measure
   robustness.

None of these change the framework concepts. They are the same `Dataset` /
`Batcher` / `Module` / `SupervisedTraining` pieces, composed more
ambitiously. That composability is the point of Burn's design.

## Typical numbers

On Flex (CPU, Apple Silicon), one epoch of the course model takes on the
order of a minute in release mode. Validation accuracy after 5 epochs lands
in the high 90s. Record your own baseline on first run — machine, backend,
seed, and config all move the result, and Burn publishes no official target
number for this example.

## Try this

1. Port the train/validation split from the official example into
   `src/training.rs` (`PartialDataset::new(Arc::new(MnistDataset::train()),
   0, 55_000)`). What changes?
2. Add a third conv block. Watch parameter count and epoch time.
3. Read the official
   [`training.rs`](https://github.com/tracel-ai/burn/blob/v0.21.0/examples/mnist/src/training.rs)
   top to bottom. Identify every construct you already know from chapters
   5–6. The remainder is dataset combinators and scheduling.

Next: [Chapter 8 — Inference and model export](08-inference-and-export.md)
