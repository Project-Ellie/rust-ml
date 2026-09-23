//! Chapter 7 — Inference: load the trained model, predict, evaluate.
//!
//! Run: `cargo run --release --example 06_mnist_infer`
//! (requires `05_mnist_train` to have run first)
//!
//! Companion doc: `docs/08-inference-and-export.md`.

use burn::backend::Flex;
use burn::data::dataloader::DataLoaderBuilder;
use burn::data::dataset::vision::MnistDataset;
use burn::data::dataset::Dataset;
use burn::prelude::*;
use burn::record::{CompactRecorder, Recorder};
use rust_ml::data::MnistBatcher;
use rust_ml::inference::infer;
use rust_ml::training::TrainingConfig;

// Inference runs on the plain backend — no Autodiff wrapper. The type system
// makes it impossible to accidentally build a graph during inference.
type B = Flex;

fn main() {
    let device: burn::backend::flex::FlexDevice = Default::default();
    let artifact_dir = "artifacts/mnist";

    // --- Single predictions with visualization -------------------------------------

    let test = MnistDataset::test();
    for index in [42, 7, 1234] {
        println!("=== test item {index} ===");
        infer::<B>(artifact_dir, device, test.get(index).unwrap());
        println!();
    }

    // --- Accuracy over (part of) the test set, by hand ------------------------------

    // Loading the model manually this time — same steps as `infer`, inlined so
    // you see them once without the helper.
    let config = TrainingConfig::load(format!("{artifact_dir}/config.json")).unwrap();
    let record = CompactRecorder::new()
        .load(format!("{artifact_dir}/model").into(), &device)
        .unwrap();
    let model = config.model.init::<B>(&device).load_record(record);

    let dataloader = DataLoaderBuilder::new(MnistBatcher::default())
        .batch_size(256)
        .num_workers(4)
        .build(MnistDataset::test());

    let mut correct = 0_usize;
    let mut total = 0_usize;
    for (i, batch) in dataloader.iter().enumerate() {
        if i >= 10 {
            break; // 10 * 256 = 2560 samples is plenty for a demo
        }
        let output = model.forward(batch.images);
        // argmax keeps the reduced dimension: [batch, 10] -> [batch, 1].
        let predicted = output.argmax(1).squeeze_dim::<1>(1); // [batch]
        let hits: i32 = predicted.equal(batch.targets).int().sum().into_scalar();
        correct += hits as usize;
        total += 256;
    }
    println!(
        "accuracy on first {total} test samples: {:.2}%",
        100.0 * correct as f64 / total as f64
    );

    // Note: `burn::train::EvaluatorBuilder` automates this loop with metric
    // registration and rendering — the advanced example in docs/07 uses it.
}
