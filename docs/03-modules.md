# Chapter 3 — Modules: how Burn defines neural networks

Run the example first: `cargo run --example 02_modules`

## A model is a struct

```rust
#[derive(Module, Debug)]
struct Mlp<B: Backend> {
    hidden: Linear<B>,
    output: Linear<B>,
    activation: Relu,
}
```

`#[derive(Module)]` inspects the fields and generates the `Module` trait
implementation. You get, for free:

- recursive collection of all learnable parameters
- `into_record()` / `load_record()` — weight serialization
- `save_file()` / `load_file()` — same thing, straight to disk
- `to_device()` — move all parameters to another device
- `valid()` — the eval-mode twin of the model (dropout off, batch norm uses
  running statistics); it returns the model on the **inner** backend, which
  also strips autodiff (chapter 4)
- `num_params()`, `fork()`, parameter visitors

There is no global registry and no base class. A module is a value. Modules
compose by nesting: a field that is itself a `Module` is traversed
recursively. That is how the MNIST `Model` contains `Conv2d`, `Linear`, and
`Dropout` fields.

## The config pattern

Every Burn component is built in two steps:

```rust
let linear = LinearConfig::new(784, 128).init(&device);
```

- The **config** owns hyperparameters. It is a plain serializable struct.
  Derive `Config` for your own: `#[derive(Config, Debug)]`, with
  `#[config(default = ...)]` for optional fields.
- **`init`** allocates and initializes parameters on a device.

This split is deliberate. The config knows nothing about backends; the module
knows nothing about JSON. Training saves the config tree next to the weights,
and inference rebuilds the exact architecture from it:

```rust
let config = TrainingConfig::load(format!("{dir}/config.json"))?;
let model = config.model.init::<B>(&device).load_record(record);
```

Keep this discipline in your own projects: everything needed to rebuild the
architecture goes into the config. Nothing else.

## What counts as a parameter

The derive macro treats each field by its type:

| Field type | Treatment |
|------------|-----------|
| `Linear<B>`, `Conv2d<B>`, ... | submodule, traversed recursively |
| `Param<Tensor<B, D>>` | a raw learnable tensor; gets a parameter id |
| bare `Tensor<B, D>` | **a constant.** Not trained, not saved |
| non-`Module` type | compile error unless marked `#[module(skip)]` |

If you ever add a weight matrix by hand (you will, for the AlphaZero value
head experiments), wrap it in `Param`:

```rust
use burn::module::Param;
use burn::tensor::Tensor;

w: Param<Tensor<B, 2>>,
```

If you need a plain Rust field (say, a `usize` capacity), mark it
`#[module(skip)]` or the derive will reject it.

## Records: saving and loading weights

A **recorder** serializes the parameter tree. The recorder choice matters:

| Recorder | Precision | Notes |
|----------|-----------|-------|
| `CompactRecorder` | **f16** | small files, lossy round-trip |
| `DefaultRecorder` | full | exact round-trip |
| `NamedMpkFileRecorder<FullPrecisionSettings>` | full | human-inspectable names |

`example 02` demonstrates the f16 effect: reload a model saved with
`CompactRecorder` and the outputs differ by ~1e-4. For inference that is
usually acceptable. For exact resume of training, use a full-precision
recorder.

Records are backend-agnostic: train on GPU, load on CPU. Records are **not**
version-agnostic: Burn 0.21 changed the `TensorData` shape type, and binary
records from older versions need conversion. Pin the version, keep the
recorder choice in your config, and test loading in CI.

## Reading

- Book: [Module](https://burn.dev/books/burn/building-blocks/module.html),
  [Config](https://burn.dev/books/burn/building-blocks/config.html),
  [Record](https://burn.dev/books/burn/building-blocks/record.html)

## Try this

1. Add a second hidden layer to the `Mlp` in `02_modules.rs`. Check that
   `num_params()` grows as expected.
2. Replace `CompactRecorder` with `DefaultRecorder` and confirm the
   round-trip error becomes exactly `0`.
3. Add a `#[module(skip)]`-marked `name: &'static str` field to the `Mlp`
   struct and give each instance a name.

Next: [Chapter 4 — Autodiff and optimizers](04-autodiff-and-optimizers.md)
