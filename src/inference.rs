//! Inference: load a trained model and use it.
//!
//! The flow mirrors any other framework:
//!
//! 1. Load the saved `TrainingConfig` (the JSON written during training) to
//!    rebuild the architecture.
//! 2. Load the weight *record* with a `Recorder` — here `CompactRecorder`,
//!    Burn's default binary format. Recorders are the serialization
//!    abstraction; alternatives cover `safetensors`, ONNX import, and
//!    `no_std`-compatible formats.
//! 3. `config.model.init::<B>(&device).load_record(record)` gives you the
//!    trained model on a *plain* backend — no autodiff, no training
//!    machinery. Note that `B` here is `Flex`, not `Autodiff<Flex>`.

use crate::{data::MnistBatcher, training::TrainingConfig};
use burn::{
    data::{dataloader::batcher::Batcher, dataset::vision::MnistItem},
    prelude::*,
    record::{CompactRecorder, Recorder},
    tensor::activation::softmax,
};

/// Run the trained model on a single MNIST item and print the result.
pub fn infer<B: Backend>(artifact_dir: &str, device: B::Device, item: MnistItem) {
    let config = TrainingConfig::load(format!("{artifact_dir}/config.json"))
        .expect("Config should exist for the model; run 05_mnist_train first");
    let record = CompactRecorder::new()
        .load(format!("{artifact_dir}/model").into(), &device)
        .expect("Trained model should exist; run 05_mnist_train first");

    let model = config.model.init::<B>(&device).load_record(record);

    let label = item.label;
    print_digit(&item);

    let batcher = MnistBatcher::default();
    let batch = batcher.batch(vec![item], &device);
    let output = model.forward(batch.images);

    // Raw logits -> probabilities -> top-3 classes.
    let probs = softmax(output.clone(), 1).squeeze::<1>();
    let (values, indices) = probs.topk_with_indices(3, 0);
    let values: Vec<f32> = values.into_data().to_vec().unwrap();
    let indices: Vec<i64> = indices.into_data().convert::<i64>().to_vec().unwrap();

    println!("True label: {label}");
    for (p, i) in values.iter().zip(indices.iter()) {
        println!("  class {i}: {:.2}%", p * 100.0);
    }

    let predicted = output.argmax(1).flatten::<1>(0, 1).into_scalar();
    println!("Predicted {predicted} — expected {label}");
}

/// Render a 28x28 MNIST image as ASCII art.
pub fn print_digit(item: &MnistItem) {
    for row in item.image.iter() {
        let line: String = row
            .iter()
            .map(|&v| match v as u8 {
                0..=31 => ' ',
                32..=95 => '.',
                96..=159 => '+',
                160..=223 => '#',
                _ => '@',
            })
            .collect();
        println!("{line}");
    }
}
