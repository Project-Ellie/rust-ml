# rust-ml

A curriculum for deep learning in Rust with the
[Burn](https://github.com/tracel-ai/burn) framework — from first tensors to a
trained MNIST classifier, and onward toward an AlphaZero-style Gomoku agent.

This repository is the companion to [DeepGomoku](https://github.com/Project-Ellie/DeepGomoku)
(TensorFlow) and the future home of its Rust successor.

## What is inside

- **`docs/` — the wiki.** A navigable curriculum in 11 chapters. Start at
  [docs/README.md](docs/README.md).
- **`examples/` — the runnable curriculum.** One example per chapter,
  verified against Burn 0.21.0 on the Flex (pure-Rust CPU) backend.
- **`src/` — the shared library.** Data pipeline, CNN model, training, and
  inference for MNIST, written in idiomatic Burn and documented.

## Quick start

```bash
# Chapter examples (no training required)
cargo run --example 01_tensors
cargo run --example 02_modules
cargo run --example 03_autodiff
cargo run --example 04_data_pipeline   # downloads MNIST on first run

# Train the MNIST classifier (always --release)
cargo run --release --example 05_mnist_train        # 5 epochs
cargo run --release --example 05_mnist_train -- 1   # 1 epoch, smoke test

# Inference: ASCII-art digits, top-3 predictions, test accuracy
cargo run --release --example 06_mnist_infer
```

Measured baseline (Apple Silicon, Flex backend, seed 42): **92.5% test
accuracy after 1 epoch** (~1 minute of training). Five epochs land in the
high 90s.

## Version policy

- Burn **0.21.0**, the latest stable release. The 0.22 prerelease changes
  the core tensor API; this repository migrates when 0.22 goes stable.
- Backend: **Flex** (pure Rust, CPU). All model code is backend-generic;
  GPU is one type alias away (`wgpu` feature).
- See [docs/01-burn-mental-model.md](docs/01-burn-mental-model.md) for the
  ecosystem map and [docs/11-pitfalls.md](docs/11-pitfalls.md) for the traps
  we already hit so you do not have to.

## Roadmap

1. ~~MNIST curriculum with Burn~~ (this repository, done)
2. AlphaZero-style Gomoku agent in Rust — design brief in
   [docs/09-toward-alphazero.md](docs/09-toward-alphazero.md), reading list
   in [docs/10-papers.md](docs/10-papers.md)

## License

Apache-2.0, same as DeepGomoku.
