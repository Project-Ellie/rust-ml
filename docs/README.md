# rust-ml wiki — Deep learning in Rust with Burn

This wiki is a curriculum. It teaches the Burn deep learning framework to an
engineer who knows Rust basics and who knows how to design, train, and use
deep neural networks. The end goal is a from-scratch AlphaZero-style Gomoku
agent in Rust.

## How to use this curriculum

1. Read the chapters in order. Each chapter is short.
2. Run the matching example after each chapter. The examples are numbered
   and self-contained: `cargo run --example 01_tensors` and so on.
3. Do the "Try this" exercises. They are small on purpose.
4. Keep the [Burn Book](https://burn.dev/books/burn/) open as the official
   reference. This wiki does not replace it. It explains the same ideas in
   the order that serves our end goal.

## Version policy

All code in this repository uses **Burn 0.21.0**, the latest stable release
(published 2026-05-07). Burn releases a breaking minor version about every
three months. The `0.22.0-pre` series already changes the core tensor API
(it removes the backend type parameter from `Tensor`). Do not mix code from
the `main` branch of the Burn repository into this curriculum. All links in
this wiki point to the `v0.21.0` tag.

The backend is **Flex**, Burn's pure-Rust CPU backend. The Burn project
recommends Flex over the older NdArray backend for new projects. Every
program here is generic over the backend, so you can switch to a GPU backend
later by changing one type alias.

## Chapters

| # | Chapter | Example | Topic |
|---|---------|---------|-------|
| 1 | [The Burn mental model](01-burn-mental-model.md) | — | Ecosystem, crates, backends, CubeCL, why Burn |
| 2 | [Tensors and backends](02-tensors-and-backends.md) | `01_tensors` | `Tensor<B, D>`, TensorData, ownership, devices |
| 3 | [Modules](03-modules.md) | `02_modules` | `#[derive(Module)]`, configs, records |
| 4 | [Autodiff and optimizers](04-autodiff-and-optimizers.md) | `03_autodiff` | backward(), GradientsParams, manual training loop |
| 5 | [The data pipeline](05-data-pipeline.md) | `04_data_pipeline` | Dataset, Batcher, DataLoader |
| 6 | [Training with SupervisedTraining](06-training.md) | `05_mnist_train` | TrainStep, Learner, metrics, checkpoints |
| 7 | [MNIST end to end](07-mnist-end-to-end.md) | `05_mnist_train` | Full walkthrough plus the advanced official example |
| 8 | [Inference and model export](08-inference-and-export.md) | `06_mnist_infer` | Records, burn-store, burn-onnx, deployment |
| 9 | [From MNIST to AlphaZero](09-toward-alphazero.md) | — | Policy/value networks, MCTS, self-play in Burn |
| 10 | [Papers](10-papers.md) | — | Annotated reading list with links |
| 11 | [Pitfalls](11-pitfalls.md) | — | Known traps, including the ones this course hit |
| 12 | [The Gomoku Architecture](12-gomoku-architecture.md) | — | Full design of the AlphaZero-style agent: engine, network, MCTS, parallelism, training, runbook |
| 13 | [The Engine Design](13-engine-design.md) | `gomoku/crates/engine` | Milestone 1 in detail: bitboards, tactics + TSS, Swap2, encoding |
| 14 | [Openings, External Data, and the Supervised Curriculum](14-openings-and-supervised-curriculum.md) | — | Swap2 as value arithmetic; the self-play opening procedure; verified external data; supervised phases (decisions D1–D5) |
| 15 | [The Silver Solution](15-alphazero-silver-solution.md) | — | Essay: the AlphaZero algorithm, why it works, architecture, SGD vs Adam, the loss function, and the math papers behind it |

## Tutorials

Tutorials live in [tutorials/](tutorials/) with the same numbering as the
design chapters. A tutorial turns a design document into guided,
write-it-yourself slices. Current:

- [13-engine-tutorial](tutorials/13-engine-tutorial/README.md) — build the
  Gomoku engine from [chapter 13](13-engine-design.md), slice by slice.
  Chapters that need it grow a companion folder (named after the
  chapter) with derivation papers behind their contracts:
  [03-bitboard-and-board](tutorials/13-engine-tutorial/03-bitboard-and-board/README.md)
  (the stride-16 shift and the `empty_moves()` signature) and
  [04-win-detection](tutorials/13-engine-tutorial/04-win-detection/README.md)
  (why win detection stages its AND and needs no edge masks).
- [cli-tutorial](tutorials/cli-tutorial/README.md) — side quest (after
  slices 2–3): a playable, non-scrolling terminal UI on *both* engine
  boards, coded against a `dyn GameBoard` abstraction owned by the CLI.
  The embryo of chapter 12's `gomoku play`.
- [mcts-tutorial](tutorials/mcts-tutorial/README.md) — milestone 2, the
  `mcts` crate: a baby-steps build tutorial (arena tree, PUCT, backup,
  move selection, mock evaluator, acceptance suite). Start with its
  [primer](tutorials/mcts-tutorial/primer.md), the reading document on
  how Monte Carlo Tree Search works in this project and how the tree
  integrates the engine and the evaluation network.

## Prerequisites

- Rust: you can read generic code, you know ownership and traits. You do not
  need to know const generics or procedural macros. This wiki explains the
  few places where Burn uses them.
- Deep learning: you know what a convolution, a loss function, SGD/Adam,
  backpropagation, and a training/validation split are. The curriculum does
  not teach these. It teaches how Burn expresses them.
- Optional but useful for chapter 9: the AlphaGo Zero paper (see
  [chapter 10](10-papers.md)).

## Repository layout

```text
src/          shared library: data pipeline, model, training, inference
examples/     the runnable curriculum, one file per chapter
docs/         this wiki
artifacts/    training output (created by example 05, git-ignored)
```
