# Chapter 1 — The Burn mental model

## What Burn is

Burn is a deep learning framework written in Rust, by the
[Tracel AI](https://github.com/tracel-ai) organization. Its design goals:

- **One codebase for training and inference.** The same model code trains on
  a GPU cluster and runs on an embedded device. There is no separate
  "deployment format" step as in TensorFlow or PyTorch.
- **Backend flexibility.** Model code is generic over a `Backend` trait. You
  select the compute engine with a type alias, not with code changes.
- **Performance as a core feature.** Kernel fusion, asynchronous execution,
  and automatic kernel selection are built in, mostly through CubeCL (see
  below).

## The crate map

`burn` is a facade crate. It re-exports a workspace of smaller crates. You
only ever depend on `burn`, but it helps to know where things live:

| Crate | Contents |
|-------|----------|
| `burn-core` | `Module`, configs, records, the training types |
| `burn-tensor` | `Tensor`, `TensorData`, the `Backend` trait |
| `burn-autodiff` | the `Autodiff<B>` backend decorator |
| `burn-nn` | layers (Linear, Conv2d, ...), activations, losses |
| `burn-optim` | optimizers (Sgd, Adam, AdamW, ...), LR schedulers |
| `burn-dataset` | `Dataset` trait, MNIST and other sources, transforms |
| `burn-train` | `SupervisedTraining`, `Learner`, metrics, evaluators |
| `burn-store` | weight files: Burnpack, SafeTensors, PyTorch state dicts |
| `burn-onnx` | ONNX import (separate repository, build-time codegen) |

Backend crates: `burn-flex` (pure Rust, CPU, `no_std`-capable), `burn-ndarray`
(legacy CPU), `burn-wgpu` / `burn-cuda` / `burn-rocm` (GPU, all on CubeCL),
`burn-cpu` (CubeCL CPU), `burn-candle` and `burn-tch` (bridges to external
libraries, both deprecated — do not use them for new code).

You select crates and backends through cargo features on `burn`. This
repository uses:

```toml
burn = { version = "0.21", features = ["flex", "train", "vision", "metrics"] }
```

## CubeCL

CubeCL is Tracel's compute language and compiler for Rust. It JIT-compiles
tensor kernels for wgpu (Vulkan/Metal/DX12/WebGPU), CUDA, ROCm, and CPU. Burn
backends such as `burn-wgpu` sit on top of it. You do not need to know CubeCL
for this curriculum. Know that it exists: it is the reason Burn's GPU support
covers many platforms without per-platform kernel code.

## The four ideas that organize everything

1. **Tensors are typed by backend, rank, and kind.**
   `Tensor<B, 4>` is a rank-4 float tensor on backend `B`. The rank is a const
   generic: the compiler catches rank errors. Shapes are runtime values.
2. **Models are plain structs with `#[derive(Module)]`.** The derive macro
   generates parameter traversal, serialization, and device movement. There
   is no global layer registry.
3. **Configuration and state are separate.** Every component has a `Config`
   struct (serializable hyperparameters) and an `init` method (allocates
   parameters on a device). You save configs to JSON and weights to records.
4. **Autodiff is a backend, not a mode.** `Autodiff<B>` wraps any backend and
   records a gradient tape. Code that trains is generic over
   `AutodiffBackend`; code that runs inference is generic over `Backend`.
   The type system replaces PyTorch's `torch.no_grad()`.

If you keep these four ideas in mind, the rest of the framework is
mechanical.

## How Burn compares to the alternatives

- **Candle** (Hugging Face): minimalist, inference-focused, great for running
  existing HF models. Less training infrastructure than Burn.
- **tch-rs**: bindings to libtorch. Closest to PyTorch semantics, but it
  drags in the C++ library and its deployment problems.
- **dfdx**: elegant typed autodiff experiment, unmaintained since 2023.

For our goal — train a policy/value network from scratch, iterate on the
training loop, and eventually deploy a self-contained Rust binary — Burn is
the right choice. The cost is a fast-moving API: pin versions, keep the
lockfile, and upgrade deliberately.

## Version reality check (as of 2026-09)

- Latest stable: **0.21.0** (2026-05-07). This curriculum pins it.
- Latest prerelease: 0.22.0-pre.3 (2026-08-25). It removes the backend type
  parameter from `Tensor` and `Module` ([issue
  #4879](https://github.com/tracel-ai/burn/issues/4879)). When 0.22 goes
  stable, this curriculum needs a migration pass.
- Release cadence: a breaking minor about every three months.

## Try this

- Open `Cargo.toml` and read the feature list. Then compare with the full
  feature list of the `burn` crate on
  [docs.rs](https://docs.rs/crate/burn/0.21.0/features).
- Skim the [Burn Book table of
  contents](https://burn.dev/books/burn/). Mark the chapters that this wiki
  covers in chapters 2–8.

Next: [Chapter 2 — Tensors and backends](02-tensors-and-backends.md)
