//! Training the MNIST model with Burn's high-level training API.
//!
//! The pieces and how they connect:
//!
//! - [`TrainStep`] / [`InferenceStep`]: you implement these two traits for your
//!   model. `TrainStep::step` computes the loss for one batch and returns its
//!   gradients via `loss.backward()`; `InferenceStep::step` is the no-grad
//!   counterpart used for validation. This is where Burn's training loop meets
//!   *your* code — everything else (epoch iteration, metric aggregation,
//!   checkpointing, the progress dashboard) is handled by the framework.
//! - [`SupervisedTraining`]: the builder that assembles a supervised training
//!   run: dataloaders, metrics, checkpointer, epoch count.
//! - [`Learner`]: bundles the three things that change during training — the
//!   model, the optimizer, and the learning rate schedule (a plain `f64` is a
//!   constant schedule).
//! - `training.launch(learner)` runs the whole thing and returns the trained
//!   model plus a metrics renderer.

use crate::{
    data::{MnistBatch, MnistBatcher},
    model::{Model, ModelConfig},
};
use burn::{
    data::{dataloader::DataLoaderBuilder, dataset::vision::MnistDataset},
    nn::loss::CrossEntropyLossConfig,
    optim::AdamConfig,
    prelude::*,
    record::CompactRecorder,
    tensor::backend::AutodiffBackend,
    train::{
        metric::{AccuracyMetric, LossMetric},
        ClassificationOutput, InferenceStep, Learner, SupervisedTraining, TrainOutput, TrainStep,
    },
};

impl<B: Backend> Model<B> {
    /// Shared forward+loss computation for both training and validation steps.
    ///
    /// Note that the loss is created from a *config* like every other Burn
    /// component, and that it lives on the same device as the output tensor.
    pub fn forward_classification(
        &self,
        images: Tensor<B, 3>,
        targets: Tensor<B, 1, Int>,
    ) -> ClassificationOutput<B> {
        let output = self.forward(images);
        let loss = CrossEntropyLossConfig::new()
            .init(&output.device())
            .forward(output.clone(), targets.clone());

        ClassificationOutput::new(loss, output, targets)
    }
}

impl<B: AutodiffBackend> TrainStep for Model<B> {
    type Input = MnistBatch<B>;
    type Output = ClassificationOutput<B>;

    fn step(&self, batch: MnistBatch<B>) -> TrainOutput<ClassificationOutput<B>> {
        let item = self.forward_classification(batch.images, batch.targets);

        // `backward()` computes gradients of the loss w.r.t. every model
        // parameter. `TrainOutput` carries both the gradients (for the
        // optimizer) and the item (for the metrics).
        TrainOutput::new(self, item.loss.backward(), item)
    }
}

impl<B: Backend> InferenceStep for Model<B> {
    type Input = MnistBatch<B>;
    type Output = ClassificationOutput<B>;

    fn step(&self, batch: MnistBatch<B>) -> ClassificationOutput<B> {
        self.forward_classification(batch.images, batch.targets)
    }
}

/// Everything that defines one training run.
///
/// The nested `ModelConfig` and `AdamConfig` are themselves `Config`s — Burn
/// configs compose, and the whole tree serializes to a single JSON file.
#[derive(Config, Debug)]
pub struct TrainingConfig {
    pub model: ModelConfig,
    pub optimizer: AdamConfig,
    #[config(default = 10)]
    pub num_epochs: usize,
    #[config(default = 64)]
    pub batch_size: usize,
    #[config(default = 12)]
    pub num_workers: usize,
    #[config(default = 42)]
    pub seed: u64,
    #[config(default = 3.0e-4)]
    pub learning_rate: f64,
}

fn create_artifact_dir(artifact_dir: &str) {
    // Remove existing artifacts before to get an accurate learner summary.
    std::fs::remove_dir_all(artifact_dir).ok();
    std::fs::create_dir_all(artifact_dir).ok();
}

/// Train the model and save weights + config into `artifact_dir`.
///
/// Generic over any backend that supports autodiff (`AutodiffBackend`) — the
/// caller picks `Autodiff<Flex>` for CPU, `Autodiff<Wgpu>` for GPU, etc.
pub fn train<B: AutodiffBackend>(artifact_dir: &str, config: TrainingConfig, device: B::Device) {
    create_artifact_dir(artifact_dir);
    config
        .save(format!("{artifact_dir}/config.json"))
        .expect("Config should be saved successfully");

    // Seed the backend so weight init and shuffling are reproducible.
    B::seed(&device, config.seed);

    let batcher = MnistBatcher::default();

    let dataloader_train = DataLoaderBuilder::new(batcher.clone())
        .batch_size(config.batch_size)
        .shuffle(config.seed)
        .num_workers(config.num_workers)
        .build(MnistDataset::train());

    // We validate on the official test split. For a serious run you would
    // carve a validation set out of the training split instead (see chapter 7
    // of the docs, where the advanced example does exactly that).
    let dataloader_test = DataLoaderBuilder::new(batcher)
        .batch_size(config.batch_size)
        .shuffle(config.seed)
        .num_workers(config.num_workers)
        .build(MnistDataset::test());

    let training = SupervisedTraining::new(artifact_dir, dataloader_train, dataloader_test)
        // Metrics are computed for train and valid splits automatically.
        .metrics((AccuracyMetric::new(), LossMetric::new()))
        // Save the best/last model weights per epoch to disk.
        .with_file_checkpointer(CompactRecorder::new())
        .num_epochs(config.num_epochs)
        // Print a summary table of all epochs at the end.
        .summary();

    let model = config.model.init::<B>(&device);
    let result = training.launch(Learner::new(
        model,
        config.optimizer.init(),
        config.learning_rate,
    ));

    result
        .model
        .save_file(format!("{artifact_dir}/model"), &CompactRecorder::new())
        .expect("Trained model should be saved successfully");
}
