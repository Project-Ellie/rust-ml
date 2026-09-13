# Chapter 5 — The data pipeline

Run the example first: `cargo run --example 04_data_pipeline`
(first run downloads MNIST, ~10 MB)

## Three roles, three types

Burn splits data handling into three independent pieces:

```text
Dataset<I>            indexable collection of raw items  (host data)
   |  DataLoader: shuffles, groups into Vec<I>, drives worker threads
   v
Batcher<B, I, O>      Vec<I>  ->  O (tensors on a device)
   |
   v
Batch<O>              one training step's input
```

1. **Dataset** — a trait with `get(index)` and `len()`. It yields *raw
   items*: plain Rust structs, no tensors. `MnistDataset` downloads MNIST
   and yields `MnistItem { image: [[f32; 28]; 28], label: u8 }`.
2. **Batcher** — converts a `Vec` of items into one batch of tensors on a
   device. **All per-sample preprocessing lives here**: normalization,
   encoding, augmentation. The batcher is the only place where your data
   meets the backend.
3. **DataLoader** — built with `DataLoaderBuilder`: batch size, shuffle
   seed, worker threads. The workers clone the batcher and prepare batches
   in parallel. The loader is an iterator of batches; the training API
   consumes it.

## The MNIST batcher, annotated

Our `src/data.rs` does four things per item:

```rust
TensorData::from(item.image).convert::<B::FloatElem>() // host array -> TensorData
Tensor::<B, 2>::from_data(data, device)                // upload to device
tensor.reshape([1, 28, 28])                            // add batch dim
((tensor / 255) - 0.1307) / 0.3081                     // MNIST standardization
```

Then `Tensor::cat(images, 0)` stacks the batch. Targets are
`Tensor<B, 1, Int>` — class indices, not one-hot vectors. Burn's
`CrossEntropyLoss` takes logits + class indices directly, like PyTorch.

The constants `0.1307` / `0.3081` are the MNIST dataset mean and standard
deviation (the same values the PyTorch MNIST example uses).

## Dataset combinators

`burn-dataset` ships wrappers that transform datasets lazily. You will meet
them in the advanced MNIST example (chapter 7):

- `PartialDataset::new(dataset, start, end)` — a slice, for train/valid splits
- `MapperDataset::new(dataset, mapper)` — lazy per-item transform
- `ComposedDataset::new(vec![...])` — concatenate datasets
- `SamplerDataset::with_replacement(dataset, n)` — resample, for class
  balancing or augmentation multipliers
- `InMemDataset` — wrap a `Vec` as a dataset (your replay buffer later)

These compose. The advanced example builds 16 augmented variants of MNIST
with them.

## Design rules

- Keep datasets raw. Preprocess in the batcher or in a `Mapper`. Raw data is
  reusable; preprocessed data locks you to one model.
- Make the batcher stateless and cheap to clone. Workers clone it.
- Create tensors on the `device` argument passed to `batch()`, never on a
  captured device. The training API decides the device.
- The batch type is yours to define. Ours is `MnistBatch<B> { images,
  targets }`. For the AlphaZero project it will be something like
  `PolicyValueBatch<B> { boards, policy_targets, value_targets }`.

## Reading

- Book: [Dataset](https://burn.dev/books/burn/building-blocks/dataset.html),
  [Basic workflow: Data](https://burn.dev/books/burn/basic-workflow/data.html)

## Try this

1. Modify `04_data_pipeline.rs` to print the mean and standard deviation of
   a normalized batch. They should be near 0 and 1.
2. Write a batcher variant that horizontally flips each image with 50%
   probability (hint: `Tensor::flip([2])`). Does MNIST accuracy care? Think
   about why flipping is wrong for digits but right for Gomoku boards.
3. Build an `InMemDataset` from the first 100 test items and run a
   dataloader over it.

Next: [Chapter 6 — Training with SupervisedTraining](06-training.md)
