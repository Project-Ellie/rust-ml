//! Chapter 5/6 — Train the MNIST classifier end to end.
//!
//! Run: `cargo run --release --example 05_mnist_train -- [num_epochs]`
//!
//! Companion docs: `docs/06-training.md` and `docs/07-mnist-end-to-end.md`.
//!
//! Uses the Flex (pure-Rust CPU) backend. In `--release` one epoch takes on the
//! order of a minute; debug builds are much slower — always train in release.

use burn::backend::{wgpu, Autodiff};
use burn::optim::AdamConfig;
use rust_ml::model::ModelConfig;
use rust_ml::training::{train, TrainingConfig};

fn main() {
    // Backend selection is a pair of type aliases: the compute backend, and
    // its autodiff-decorated training counterpart. To train on GPU instead,
    // swap `Flex` for `Wgpu` (plus the `wgpu` feature) — nothing else in
    // the training code changes.
    type B = wgpu::Wgpu;
    type AutodiffB = Autodiff<B>;

    let device = Default::default();
    let artifact_dir = "artifacts/mnist";

    // Optional CLI override so you can smoke-test with a single epoch:
    //   cargo run --release --example 05_mnist_train -- 1
    let num_epochs = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(5);

    let config = TrainingConfig::new(ModelConfig::new(10, 128), AdamConfig::new())
        .with_num_epochs(num_epochs);

    println!("Training for {num_epochs} epoch(s); artifacts -> {artifact_dir}");
    train::<AutodiffB>(artifact_dir, config, device);
    println!("Done. Now run: cargo run --release --example 06_mnist_infer");
}
