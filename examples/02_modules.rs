//! Chapter 2 — Modules: how Burn defines neural networks.
//!
//! Run: `cargo run --example 02_modules`
//!
//! Companion doc: `docs/03-modules.md`.

use burn::backend::Flex;
use burn::module::Module;
use burn::nn::{Linear, LinearConfig, Relu};
use burn::record::CompactRecorder;
use burn::tensor::{backend::Backend, Tensor};

type B = Flex;

/// A network in Burn is a plain struct, generic over the backend, with
/// `#[derive(Module)]`. The derive macro inspects the fields and generates:
///
/// - collection of all learnable parameters (recursively through submodules)
/// - `load_record` / `save_file` (weight (de)serialization)
/// - `to_device` (move all parameters to another device)
/// - `valid()` (switch to eval mode — dropout off, batch norm frozen)
/// - `fork`, `num_params`, parameter visitors, ...
///
/// Fields whose type does not implement `Module` (like `Relu`, which holds no
/// parameters) can be marked `#[module(skip)]`; here `Relu` implements
/// `Module` so no skip is needed.
#[derive(Module, Debug)]
struct Mlp<B: Backend> {
    hidden: Linear<B>,
    output: Linear<B>,
    activation: Relu,
}

impl<B: Backend> Mlp<B> {
    fn forward(&self, x: Tensor<B, 2>) -> Tensor<B, 2> {
        let x = self.hidden.forward(x);
        let x = self.activation.forward(x);
        self.output.forward(x)
    }
}

fn main() {
    let device = Default::default();

    // Layers are created from *configs*. The config owns the hyperparameters;
    // `init` allocates and initializes the weights on a device. This split is
    // a core Burn idiom: configs are serializable and backend-free, modules
    // are not.
    let model = Mlp {
        hidden: LinearConfig::new(784, 128).init(&device),
        output: LinearConfig::new(128, 10).init(&device),
        activation: Relu::new(),
    };

    // `num_params` comes from the Module derive.
    println!("total parameters: {}", model.num_params());
    // Sanity check by hand: (784*128 + 128) + (128*10 + 10) = 100_480 + 1_290.
    println!("expected        : {}", 784 * 128 + 128 + 128 * 10 + 10);

    // Forward pass with a random batch of 4 "images" (flattened 28x28).
    let input = Tensor::<B, 2>::random(
        [4, 784],
        burn::tensor::Distribution::Normal(0.0, 1.0),
        &device,
    );
    let logits = model.forward(input);
    println!("logits shape     = {:?}", logits.shape()); // [4, 10]

    // --- Records: saving and loading weights ------------------------------------

    // A `Recorder` serializes the module's parameter tree. The record is
    // backend-agnostic, so you can train on GPU and load on CPU.
    //
    // Watch out: `CompactRecorder` is `DefaultFileRecorder<HalfPrecisionSettings>`
    // — it stores floats as f16 to halve file size. That is fine for
    // inference but NOT an exact round-trip. `DefaultRecorder` keeps full
    // precision. We demonstrate the compact one here and measure the error.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mlp");
    model
        .clone()
        .save_file(path.to_str().unwrap(), &CompactRecorder::new())
        .unwrap();

    use burn::record::Recorder;
    let record = CompactRecorder::new()
        .load(path.to_str().unwrap().into(), &device)
        .unwrap();

    let reloaded = Mlp {
        hidden: LinearConfig::new(784, 128).init(&device),
        output: LinearConfig::new(128, 10).init(&device),
        activation: Relu::new(),
    }
    .load_record(record);

    let input = Tensor::<B, 2>::random(
        [4, 784],
        burn::tensor::Distribution::Normal(0.0, 1.0),
        &device,
    );
    let diff: f32 = model
        .forward(input.clone())
        .sub(reloaded.forward(input))
        .abs()
        .max()
        .into_scalar();
    println!("max |original - reloaded| = {diff}"); // ~1e-4, NOT 0: f16 storage

    // With `DefaultRecorder` (full precision) the same check prints exactly 0.
    // Try it: replace both `CompactRecorder` occurrences above with
    // `burn::record::DefaultRecorder`.
}
