# Chapter 02 — Trusting the framework

## Abstract

This chapter moves from hand-written loops to Burn 0.21's tensor and
convolution machinery. You will learn how Burn stores a 4-D tensor in
NCHW order, how a `Conv2d` layer is configured and how its parameters
are assigned by hand, and what "backend-generic" code looks like in
practice. The chapter's central exercise is a differential test: your
chapter-1 `naive::cross_correlate` and Burn's `Conv2d::forward` must
produce the same numbers exactly — no epsilon — on random
small-integer inputs and weights. Because the inputs are integers in a
range where every `f32` sum is exact, exact equality is a legitimate
learning instrument, not a fragile coincidence. You will also see why
the tutorial materializes padding in the input planes rather than using
Burn's `Explicit` padding, which only zero-pads. By the end you will
have a backend-generic integration test, `tests/spike.rs`, that passes
on `NdArray` by default and on `Wgpu` when the crate's `gpu` feature is
enabled.

## Glossary

| Term | Definition |
|------|------------|
| **Backend** | In Burn, a Rust trait (`Backend`) abstracting over execution targets such as `NdArray` (CPU) and `Wgpu` (GPU). |
| **Backend-generic** | Code written with a type parameter `B: Backend` so the same logic runs on any Burn backend. |
| **Batch dimension** | The leading `N` in an NCHW tensor: the number of independent inputs processed together. |
| **Channel** | One plane of a tensor. The input has two channels; a conv layer can have many input and output channels. |
| **Cross-correlation** | The sliding weighted-sum operation that deep-learning frameworks call "convolution"; the kernel is not flipped. |
| **Differential test** | A test that runs two independent implementations on the same input and requires identical outputs. |
| **Explicit padding** | Burn's `PaddingConfig2d::Explicit(top, left, bottom, right)`, which adds a border of zeros before the convolution. |
| **Materialized padding** | Padding written explicitly into the input array, as in chapter 1, rather than requested from the framework. |
| **NCHW** | Tensor layout `[batch, channels, height, width]` used by Burn for 4-D conv tensors. |
| **Param** | Burn's parameter wrapper (`burn::module::Param`) around a tensor; a `Conv2d` stores its weight and bias as `Param` values. |
| **TensorData** | Burn's container for raw tensor values and shape, used to construct a `Tensor` from a `Vec`. |

## Context

Chapter 1 implemented two things in plain Rust: the padded input planes
(`src/planes.rs`) and a hand-written single-channel cross-correlation
(`src/naive.rs`). Those implementations are intentionally
framework-free so the sliding-multiply-and-add mechanism is visible.
This chapter connects that mechanism to Burn.

The goal is not to start training a network; it is to understand what
Burn's `Conv2d` computes and to prove that your mental model matches
the framework. That proof is done by differential testing: the same
operation is written twice — once in plain Rust and once through Burn —
and the test asserts that the outputs are identical. When the numbers
match exactly on many random inputs, you know *why* they match because
you wrote both sides. This is the same technique the rest of the
tutorial will use to compare threat maps against the engine and
against a naive oracle.

The material here is the bridge from the primer's kernel language to
the actual Burn API. Later chapters will generate kernels
programmatically; this chapter makes sure the machinery that runs them
is trusted.

## Intention

1. Create `gomoku/crates/patterns/tests/spike.rs` and write
   backend-generic test helpers.
2. Verify that `patterns::naive::cross_correlate` matches Burn's
   `Conv2d::forward` exactly on:
   * a hand-computed smoke test (`smoke_ndarray`);
   * random single-channel inputs/weights (`single_channel_exact_ndarray`);
   * random multi-channel inputs/weights (`multi_channel_exact_ndarray`);
   * an explicit-padding case that proves `Explicit` only zero-pads
     (`explicit_padding_zero_pads_ndarray`).
3. Learn the Burn 0.21 weight-assignment idiom: build a `Conv2d` from
   `Conv2dConfig`, then replace its public `weight` and `bias` fields
   with `Param::from_tensor(...)` because Burn 0.21 has no weight
   setter.
4. Observe backend genericity: the same helper code uses `B: Backend`
   and runs on `NdArray` by default and on `Wgpu` when `--features gpu`
   is supplied.

Observable done-state: `cargo test -p patterns` passes from `gomoku/`,
`cargo test -p patterns --features gpu` passes on a machine with a
working GPU adapter, and the formatting/lint gates are green.

## Mental mapping

### Why differential testing, and why exact equality?

The standard way to learn a framework is to read its documentation and
hope the mental model is right. The standard way to *know* is to
compare the framework's output against an independently written
reference. Your `naive::cross_correlate` is that reference: it is a
plain Rust implementation of the same operation, with no shared code
with Burn.

The comparison uses `assert_eq!`, not `assert!((a - b).abs() < 1e-5)`.
That is possible because every input and weight is a small integer. An
`f32` can represent every integer exactly up to `2^24`, and the sums in
a small kernel are far below that threshold. A product of two integers
in `−3..=3` is exact; a sum of a few dozen such products is exact; the
result is therefore deterministic and identical across CPU and GPU
backends. Exact equality turns the test into a precise diagnostic: if
it fails, the shapes, strides, or padding interpretation are wrong.

### Why NCHW, and why does the weight shape look like that?

Burn stores a 4-D conv tensor as `[batch, channels, height, width]`
(NCHW). The weight tensor for a `Conv2d` is
`[channels_out, channels_in / groups, kernel_size_1, kernel_size_2]`
(`burn-nn-0.21.0/src/modules/conv/conv2d.rs:54`). The bias is a 1-D
tensor of length `channels_out`. The output is
`[batch, channels_out, height_out, width_out]`
(`burn-nn-0.21.0/src/modules/conv/conv2d.rs:146`).

This layout matters when you flatten a hand-written kernel into a
`TensorData::new(...)` call. For a single-channel input, the kernel
flat order is `[1, 1, kh, kw]`. For a multi-channel input with
`in_ch = 2` and `out_ch = 3`, the flat order is
`[out_ch, in_ch, kh, kw]`: all kernels for output channel 0 first,
within that all input channels, then output channel 1, and so on. The
`naive_multi_channel` helper in the spike test mirrors that arithmetic:
for each output channel, sum the per-input-channel cross-correlations
and add one bias.

### Why assign weights by re-assembling the `Conv2d` struct?

Burn 0.21 exposes the `Conv2d` fields as public:

```rust
pub struct Conv2d<B: Backend> {
    pub weight: Param<Tensor<B, 4>>,
    pub bias: Option<Param<Tensor<B, 1>>>,
    pub stride: [usize; 2],
    pub kernel_size: [usize; 2],
    pub dilation: [usize; 2],
    pub groups: usize,
    pub padding: PaddingConfig2d,
}
```

(`burn-nn-0.21.0/src/modules/conv/conv2d.rs:53`). There is no
`set_weight` method. The idiom is therefore:

1. Build a `Conv2d` once from `Conv2dConfig` so all the metadata
   fields (`stride`, `kernel_size`, `dilation`, `groups`, `padding`)
   are correct (`burn-nn-0.21.0/src/modules/conv/conv2d.rs:72`).
2. Construct the desired `weight` and `bias` tensors from
   `TensorData::new(...)`
   (`burn-tensor-0.21.0/src/tensor/api/base.rs:1942`;
   `burn-backend-0.21.0/src/data/tensor.rs:59`).
3. Wrap each tensor in `Param::from_tensor(...)`
   (`burn-core-0.21.0/src/module/param/tensor.rs:69`).
4. Return a new `Conv2d` with the hand-written parameters and the
   metadata copied from the configured module.

`Param::from_tensor` marks the tensor as requiring gradients. For the
hand-written threat network this is harmless: the parameters are simply
never optimized.

### Why backend-generic from line one?

The test file is written as `fn helper<B: Backend>(device: &B::Device)`
and the test functions instantiate it with `smoke::<NdArray>(&device)`.
This is not premature abstraction; it is the lesson. The net tutorial
will train on GPU, and the same network code must run on both CPU and
GPU. Putting the type parameter `B: Backend` at the boundary of every
helper makes the dual-backend verification a one-line change in each
test. The default backend is `NdArray`; the optional `gpu` feature
enables `burn/wgpu` and the tests use `burn::backend::Wgpu` behind
`#[cfg(feature = "gpu")]`.

### Why materialize padding instead of using `PaddingConfig2d::Explicit`?

`Conv2dConfig` defaults to `PaddingConfig2d::Valid`
(`burn-nn-0.21.0/src/modules/conv/conv2d.rs:36`), which means no
padding. `PaddingConfig2d::Explicit(top, left, bottom, right)` pads the
input with zeros before the convolution
(`burn-nn-0.21.0/src/padding.rs:61` and
`burn-tensor-0.21.0/src/tensor/module.rs:134`). That is useful for
many networks, but it cannot express the tutorial's requirement that
the border be *blocked* (channel 1 one-padded) while the stones channel
is zero-padded. The framework only knows one padding value: zero. So
the tutorial keeps the 25×25 padded planes from chapter 1 and uses
valid padding in every `Conv2d`. The explicit-padding test in the
spike file exists only to document and verify this limitation.

### How does single-channel cross-correlation lift to multi-channel?

A `Conv2d` with `groups = 1` computes, for each output channel `o`,

```text
bias[o] + sum_{c in input_channels} cross_correlate(input[c], kernel[o][c])
```

The spike test's `naive_multi_channel` helper implements exactly that:
it iterates over output channels, and for each output channel it sums
the per-input-channel cross-correlations produced by your existing
`naive::cross_correlate`. This lets you reuse chapter-1 code to verify
Burn's multi-channel behavior.

## Low-level design

### Files you will create or modify

```text
gomoku/crates/patterns/
├── Cargo.toml        (already has the `gpu` feature and the burn dep)
├── src/
│   ├── lib.rs        (already registers `pub mod naive;` from chapter 1)
│   └── naive.rs      (your chapter-1 reference implementation)
└── tests/
    └── spike.rs      (new integration test)
```

### `tests/spike.rs`

The file is an integration test, so it imports `patterns::naive` as an
external caller would. It is organized as backend-generic helpers plus
concrete test functions.

```rust
use burn::backend::NdArray;
use burn::module::Param;
use burn::nn::PaddingConfig2d;
use burn::nn::conv::{Conv2d, Conv2dConfig};
use burn::tensor::backend::Backend;
use burn::tensor::{Tensor, TensorData};
use patterns::naive;
use rand::SeedableRng;
use rand::distr::{Distribution, Uniform};
use rand::rngs::StdRng;
```

Helper functions (all backend-generic):

* `fn smoke<B: Backend>(device: &B::Device)` — build a 1→1 channel,
  2×2 conv, assign weight `[[1,0],[0,1]]` and bias `0`, run it on a
  4×4 input, and assert the output equals `naive::cross_correlate`.
* `fn single_channel_exact<B: Backend>(device: &B::Device, seed: u64)` —
  20 random shapes, random 2–4 size kernels, values in `−3..=3`, exact
  comparison against the naive reference.
* `fn naive_multi_channel(input, kernels, bias) -> Vec<Vec<Vec<f32>>>` —
  lift the single-channel `cross_correlate` to multi-channel-in,
  multi-channel-out by summing per-input-channel results plus one bias
  per output channel.
* `fn multi_channel_exact<B: Backend>(device: &B::Device, seed: u64)` —
  20 random shapes with `in_ch = 2` and `out_ch = 3`, exact comparison.
* `fn explicit_padding_zero_pads<B: Backend>(device: &B::Device)` —
  build the same conv with `Valid` and with
  `Explicit(1, 1, 1, 1)`, compare the explicit-padded output to a
  materialized zero-padded input run through the valid conv.

Concrete `NdArray` tests:

* `smoke_ndarray`
* `single_channel_exact_ndarray`
* `multi_channel_exact_ndarray`
* `explicit_padding_zero_pads_ndarray`

GPU tests, behind `#[cfg(feature = "gpu")]`:

* `smoke_wgpu`
* `single_channel_exact_wgpu`
* `multi_channel_exact_wgpu`
* `explicit_padding_zero_pads_wgpu`
* `cpu_vs_wgpu_exact` — build the same 2→3 channel, 3×3 conv on both
  backends with identical small-integer weights and input, and assert
  exact output equality.

Implementation notes:

* Use `TensorData::new(values, shape)` and `Tensor::<B, D>::from_data`
  to construct tensors from `Vec<f32>`.
* Use `Param::from_tensor(tensor)` to place the tensors into the
  `Conv2d` struct.
* Keep `stride`, `kernel_size`, `dilation`, `groups`, and `padding`
  consistent. The simplest way is to initialize a throwaway `Conv2d`
  from `Conv2dConfig` and copy its metadata fields into the manually
  assembled struct.
* For exact comparisons, convert Burn output with
  `output.to_data().iter::<f32>().collect()` and compare the resulting
  `Vec<f32>` with `assert_eq!`.
* The multi-channel helper asserts that `kernels.len() == bias.len()`
  and computes the valid-padded output shape from the first input
  channel and the first kernel's spatial size.

### Multi-channel lifting

The `naive_multi_channel` helper is worth seeing explicitly. Given

* `input[c][i][j]` — `c` input channels,
* `kernels[o][c][p][q]` — one kernel per `(output, input)` channel pair,
* `bias[o]` — one bias per output channel,

it computes

```text
out[o][i][j] = bias[o]
    + sum_c sum_p sum_q input[c][i+p][j+q] * kernels[o][c][p][q]
```

This is exactly what Burn's `Conv2d` computes when `groups = 1`, and it
is the same sum you would write by nesting your chapter-1 loop inside
another loop over input channels.

## Solution (opt-in)

The complete reference test file for this chapter lives in
[02-trusting-the-framework/01-solution.md](02-trusting-the-framework/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare. The solution
contains `tests/spike.rs` extracted verbatim from the verified
reference crate.

## TDD checklist

Run these tests from `gomoku/` with `cargo test -p patterns`. Run the
GPU variants with `cargo test -p patterns --features gpu` on a machine
with a working adapter.

The reference `tests/spike.rs` suite contains exactly these tests:

On `NdArray` (always built):

1. `smoke_ndarray`
2. `single_channel_exact_ndarray`
3. `multi_channel_exact_ndarray`
4. `explicit_padding_zero_pads_ndarray`

On `Wgpu` (behind `#[cfg(feature = "gpu")]`):

1. `smoke_wgpu`
2. `single_channel_exact_wgpu`
3. `multi_channel_exact_wgpu`
4. `explicit_padding_zero_pads_wgpu`
5. `cpu_vs_wgpu_exact`

Follow the red-green-refactor rhythm:

1. **Red:** Create `tests/spike.rs` with the imports, the helper
   function signatures, and all the test functions. Implement each
   helper as `todo!()`. Run `cargo test -p patterns`. Expect failures.
2. **Green:** Implement `smoke`, `single_channel_exact`,
   `naive_multi_channel`, `multi_channel_exact`, and
   `explicit_padding_zero_pads`. Run `cargo test -p patterns`. The four
   `NdArray` tests should pass.
3. **GPU check:** If your environment supports it, run
   `cargo test -p patterns --features gpu`. The five GPU tests should
   pass. If the adapter is unavailable, skip this step; the default
   gates do not depend on it.
4. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p patterns` passes from `gomoku/`.
* `cargo test -p patterns --features gpu` passes on a GPU-capable
  machine (documented as environment-dependent and excluded from the
  default gates).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Commit message for the code slice: `feat(patterns): burn conv2d equivalence`.

Next: [Chapter 03 — The kernel language](03-the-kernel-language.md)

## References

* [`01-planes-and-a-conv-by-hand.md`](01-planes-and-a-conv-by-hand.md)
  — the previous chapter: padded planes and the naive cross-correlation
  reference.
* [`docs/specs/2026-09-27-convolutions-tutorial-design.md`](../../specs/2026-09-27-convolutions-tutorial-design.md)
  — §4.4 (backend story, batching, and the opt-in GPU feature).
* [`docs/plans/2026-09-27-convolutions-tutorial.md`](../../plans/2026-09-27-convolutions-tutorial.md)
  — Task T4, which this chapter implements.
* Burn 0.21.0 pinned source — `burn-nn-0.21.0/src/modules/conv/conv2d.rs`
  (struct layout, default `Valid` padding, input/output layouts), cited
  by line number in the body.
* Burn 0.21.0 pinned source — `burn-nn-0.21.0/src/padding.rs:61` and
  `burn-tensor-0.21.0/src/tensor/module.rs:134` — `Explicit` padding
  zero-pads the input before convolution.
* Burn 0.21.0 pinned source — `burn-core-0.21.0/src/module/param/tensor.rs:69`
  for `Param::from_tensor`, and
  `burn-tensor-0.21.0/src/tensor/api/base.rs:1942` for
  `Tensor::from_data`.
