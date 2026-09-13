# Chapter 8 — Inference and model export

Run the example: `cargo run --release --example 06_mnist_infer`

## The inference pattern

Three steps, always the same (`src/inference.rs`):

```rust
let config = TrainingConfig::load(format!("{dir}/config.json"))?;   // architecture
let record = CompactRecorder::new().load(path.into(), &device)?;    // weights
let model = config.model.init::<B>(&device).load_record(record);    // trained model
```

Then `model.forward(...)` and read the outputs. Example 06 renders the input
digit as ASCII art, prints the top-3 classes with softmax probabilities, and
measures accuracy over test batches by hand.

Points that matter:

- **The backend is plain `B: Backend`**, not `Autodiff<B>`. No tape, no
  gradient memory, and the compiler enforces it.
- Load with the **same recorder family** you saved with. Records are not
  self-describing across recorders.
- Inside a training process, `model.valid()` gives you the eval model
  directly — no record round-trip needed.

## From records to portable artifacts

Burn records are the right format for checkpoints inside one project. For
sharing models or importing foreign weights, the 0.21 ecosystem is:

| Tool | Purpose |
|------|---------|
| `burn-store` | Burnpack (`.bpk`), SafeTensors, PyTorch state-dict import/export, key remapping |
| `burn-onnx` | ONNX **import**: generates native Burn Rust code + weights at build time |

Two warnings:

- `burn-import` and the old `PyTorchFileRecorder` / `SafetensorsFileRecorder`
  are deprecated. Use `burn-store` and `burn-onnx` for anything new.
- `burn-onnx` 0.21 imports ONNX; it does not export Burn models to ONNX. An
  exporter exists on the 0.22 prerelease branch only.

For the AlphaZero project this matters in one place: if you ever want to
compare against a Python-trained reference network, `burn-store` can read
its SafeTensors weights into a Burn module with matching parameter names.

## Deployment notes

One of Burn's headline features: the same model code runs everywhere. The
practical routes:

- **Native binary**: what we do here. Flex or a GPU backend, one
  `cargo build --release`.
- **`no_std` / embedded**: Flex supports `no_std`; pair it with
  `NoStdInferenceRecorder` to embed weights as bytes in the binary.
- **WebAssembly**: the wgpu backend runs in the browser; Flex compiles to
  WASM too. (Caveat: some ops, e.g. `topk` on the WebGPU WASM backend, are
  not implemented there yet — check before you design around them.)

These are paths, not promises. For the Gomoku project the native binary is
all we need.

## Reading

- Book: [Saving & loading](https://burn.dev/books/burn/saving-and-loading.html),
  [Inference](https://burn.dev/books/burn/basic-workflow/inference.html),
  [Models & pre-trained weights](https://burn.dev/books/burn/models-and-pretrained-weights.html)
- [burn-store migration guide](https://github.com/tracel-ai/burn/blob/v0.21.0/crates/burn-store/MIGRATION.md)

## Try this

1. Save the model with `DefaultRecorder` instead and compare file sizes
   (`ls -la artifacts/mnist`). Explain the ratio.
2. Write a `predict` function that takes a `[[f32; 28]; 28]` array (not a
   `MnistItem`) and returns the class. This is the shape of an API boundary.
3. Evaluate on the full 10,000 test images and time it.

Next: [Chapter 9 — From MNIST to AlphaZero](09-toward-alphazero.md)
