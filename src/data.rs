//! Data pipeline for MNIST.
//!
//! Burn splits data handling into three roles:
//!
//! - [`MnistDataset`] (from `burn-dataset`): downloads MNIST and yields one
//!   [`MnistItem`] per image. A *dataset* is an indexable collection of raw items.
//! - [`MnistBatcher`]: converts a `Vec` of raw items into one batch of *tensors*
//!   on a target device. This is where per-sample preprocessing happens
//!   (normalization, augmentation, ...).
//! - `DataLoader` (built with `DataLoaderBuilder`): shuffles the dataset, groups
//!   items into `Vec`s, calls the batcher (in parallel worker threads) and yields
//!   batches during training.

use burn::{
    data::{dataloader::batcher::Batcher, dataset::vision::MnistItem},
    prelude::*,
};

/// Stateless converter from raw MNIST items to tensor batches.
///
/// The batcher is deliberately stateless so the dataloader can clone it into
/// worker threads. Any configuration (e.g. augmentation flags) would be stored
/// here as plain fields.
#[derive(Clone, Default)]
pub struct MnistBatcher {}

/// One batch of MNIST data as tensors on backend `B`.
///
/// - `images`: `[batch_size, 28, 28]` floats, normalized (see below)
/// - `targets`: `[batch_size]` integer class labels (0-9)
#[derive(Clone, Debug)]
pub struct MnistBatch<B: Backend> {
    pub images: Tensor<B, 3>,
    pub targets: Tensor<B, 1, Int>,
}

impl<B: Backend> Batcher<B, MnistItem, MnistBatch<B>> for MnistBatcher {
    fn batch(&self, items: Vec<MnistItem>, device: &B::Device) -> MnistBatch<B> {
        let images = items
            .iter()
            // `MnistItem.image` is `[[f32; 28]; 28]`; `TensorData` is the
            // backend-independent container that moves host data into tensors.
            .map(|item| TensorData::from(item.image).convert::<B::FloatElem>())
            .map(|data| Tensor::<B, 2>::from_data(data, device))
            .map(|tensor| tensor.reshape([1, 28, 28]))
            // Normalize: scale to [0,1], then standardize with the MNIST
            // dataset statistics (same values as the PyTorch MNIST example).
            .map(|tensor| ((tensor / 255) - 0.1307) / 0.3081)
            .collect();

        let targets = items
            .iter()
            .map(|item| {
                // `.elem::<B::IntElem>()` converts the Rust integer into the
                // integer element type the backend uses (i32 for Flex,
                // i64 for NdArray).
                Tensor::<B, 1, Int>::from_data([(item.label as i64).elem::<B::IntElem>()], device)
            })
            .collect();

        // Stack the per-sample tensors along a new leading batch dimension.
        let images = Tensor::cat(images, 0);
        let targets = Tensor::cat(targets, 0);

        MnistBatch { images, targets }
    }
}
