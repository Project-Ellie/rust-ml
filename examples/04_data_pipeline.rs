//! Chapter 4 — The data pipeline: datasets, batchers, dataloaders.
//!
//! Run: `cargo run --example 04_data_pipeline`
//!
//! Companion doc: `docs/05-data-pipeline.md`.
//!
//! On first run this downloads MNIST (~10 MB) into a cache directory.

use burn::backend::Flex;
use burn::data::dataloader::batcher::Batcher;
use burn::data::dataloader::DataLoaderBuilder;
use burn::data::dataset::vision::MnistDataset;
use burn::data::dataset::Dataset;
use rust_ml::data::{MnistBatch, MnistBatcher};
use rust_ml::inference::print_digit;

type B = Flex;

fn main() {
    let device = Default::default();

    // --- Dataset: an indexable collection of raw items --------------------------

    let train = MnistDataset::train();
    let test = MnistDataset::test();
    println!("train items: {}", train.len());
    println!("test items:  {}", test.len());

    // A `MnistItem` is plain host data: `image: [[f32; 28]; 28]`, `label: u8`.
    let item = test.get(42).unwrap();
    println!("item 42 has label {}", item.label);
    print_digit(&item);

    // --- Batcher: Vec<Item> -> tensors -------------------------------------------

    let batcher = MnistBatcher::default();
    let items: Vec<_> = (0..4).map(|i| test.get(i).unwrap()).collect();
    let batch: MnistBatch<B> = batcher.batch(items, &device);
    println!("batch.images  shape = {:?}", batch.images.shape()); // [4, 28, 28]
    println!("batch.targets shape = {:?}", batch.targets.shape()); // [4]
    println!("targets              = {}", batch.targets);

    // --- DataLoader: shuffling, batching, worker threads --------------------------

    let dataloader = DataLoaderBuilder::new(batcher)
        .batch_size(64)
        .shuffle(42)
        .num_workers(4)
        .build(MnistDataset::train());

    // The dataloader is an iterator of batches — this is what `SupervisedTraining`
    // consumes internally.
    let mut batches = dataloader.iter();
    let first: MnistBatch<B> = batches.next().unwrap();
    println!("first loader batch images: {:?}", first.images.shape()); // [64, 28, 28]
    println!("num batches per epoch:   {}", dataloader.num_items() / 64);

    // Design notes:
    // - The batcher is the ONLY place per-sample preprocessing happens. Keep
    //   datasets raw; normalize/augment in the batcher (or in a `Mapper`
    //   dataset wrapper — the advanced example in chapter 7 shows that).
    // - `num_workers` parallelizes batching across threads, which is why the
    //   batcher must be `Clone` and cheap to construct.
}
