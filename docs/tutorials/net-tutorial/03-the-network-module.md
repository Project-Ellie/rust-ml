# Chapter 03 — The network module

## Abstract

This chapter adds the policy/value network to the `net` crate: a
configurable residual convolutional network that takes the 4-plane
17×17 input from chapter 1 and produces a policy vector and a value
scalar. The locked architecture is the 128-channel, 10-block trunk from
docs/12 §10; the implementation is made configurable so unit tests can
run a tiny variant. By the end you will have a `Model<B: Backend>`
derived with Burn's `Module` trait, a `ModelConfig` derived with
`Config`, a reusable `ResBlock`, and four tests that check output shapes,
determinism, and the value range.

## Glossary

| Term | Definition |
|------|------------|
| **Backend** | A Burn type parameter (`B: Backend` for inference, `B: AutodiffBackend` for training) that selects the tensor implementation. |
| **Stem** | The first convolution that projects the 4 input planes into the trunk channel width. |
| **Residual block** | A pair of 3×3 convolutions with batch normalization and ReLU, plus a skip connection: `ReLU(BN(conv2(BN(conv1(x)))) + x)`. |
| **Policy head** | The branch that emits 225 logits, one per legal move on the 15×15 board. |
| **Value head** | The branch that emits a single scalar in `[-1, 1]` via `tanh`. |
| **`engine::EXT`** | The engine's padded board extent constant (`17`); the input spatial size is `EXT × EXT`. |
| **Tiny config** | A test-scale configuration (8 channels, 1 block) that keeps the suite fast (invention I7). |

## Context

Chapter 1 wired the `net` crate and built the four input planes from an
`engine::Board` plus move history. Chapter 2 turned `train::Sample`
records into batches of tensors: input `[B, 4, 17, 17]`, policy target
`[B, 225]`, and value target `[B, 1]`. This chapter is the module that
consumes those batches — the network itself.

The architecture is locked in
[`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) §10:
a 3×3 convolutional stem maps 4 input channels to the trunk width, a
stack of identical residual blocks forms the trunk, and two separate
heads produce policy logits over 225 moves and a `tanh`-bounded value.
The locked production size is 128 channels and 10 blocks; the code makes
both numbers configurable so tests can use a tiny model.

## Intention

1. Create `gomoku/crates/net/src/network.rs` and register
   `pub mod network;` in `gomoku/crates/net/src/lib.rs`.
2. Define `ModelConfig` with `#[derive(Config)]`:
   * `channels: usize` default `128`.
   * `num_blocks: usize` default `10`.
   * `init<B: Backend>(&self, device: &B::Device) -> Model<B>`.
   * `tiny()` → `{ channels: 8, num_blocks: 1 }`.
   * `mid()` → `{ channels: 32, num_blocks: 4 }`.
3. Define `Model<B: Backend>` with `#[derive(Module)]` containing the
   stem conv+batch norm, a `Vec<ResBlock<B>>`, the policy head
   conv+batch norm+linear, and the value head conv+batch norm+two
   linears.
4. Define `ModelOutput<B: Backend>` `{ policy: Tensor<B, 2>, value:
   Tensor<B, 2> }` to bundle the two forward outputs.
5. Define `ResBlock<B: Backend>` with two 3×3 convolutions, batch norms,
   and ReLU, and implement its forward with the skip add.
6. Implement `Model::forward`:
   * stem conv → batch norm → ReLU;
   * pass through every residual block;
   * policy head: conv 1×1 channels→2 → BN → ReLU → flatten → linear
     `2·EXT·EXT → 225`;
   * value head: conv 1×1 channels→1 → BN → ReLU → flatten → linear
     `EXT·EXT → 256` → ReLU → linear `256 → 1` → `tanh`.
7. Write four tests:
   * `default_model_output_shapes` — `ModelConfig::new()` on a `[2, 4,
     EXT, EXT]` zero tensor yields policy `[2, 225]` and value `[2, 1]`.
   * `tiny_model_output_shapes` — `ModelConfig::tiny()` on a `[1, 4,
     EXT, EXT]` tensor yields the same ranks.
   * `forward_is_deterministic_on_same_input` — running the same model
     twice on the same input gives identical policy vectors.
   * `value_is_in_range` — every value output is inside `[-1, 1]`.

Observable done-state: `cargo test -p net` passes, `cargo clippy
--all-targets -- -D warnings` is green, and `cargo fmt --all --
--check` makes no changes.

## Mental mapping

### From the architecture diagram to Burn types

The architecture sketch in docs/12 §10 is a stack of boxes. In Burn
0.21 each box is a typed field in a `#[derive(Module)]` struct:

| Diagram box | Burn type | Notes |
|-------------|-----------|-------|
| 3×3 conv, 4→C | `Conv2d<B>` | same padding, no bias. |
| Batch norm | `BatchNorm<B>` | channel dimension. |
| ReLU | `Relu` | stateless; one shared instance is fine. |
| Residual block | `ResBlock<B>` | nested `Module` struct. |
| 1×1 conv | `Conv2d<B>` | same padding, no bias. |
| Flatten | `Tensor::flatten(1, 3)` | keep batch dim 0. |
| Linear | `Linear<B>` | configured with `LinearConfig`. |
| tanh | `burn::tensor::activation::tanh` | applied to the value head. |

The `Module` derive does two jobs: it makes the struct a learnable
Burn model, and it gives you `clone()`, device movement, and record
serialization for free. The `Config` derive does a different job: it
generates a serializable config struct with `Default` values and builder
methods. Keeping them separate — `ModelConfig` for construction
arguments, `Model` for the initialized weights — is the Burn idiom.

> **Excursion — why the policy head emits 225 logits, not 289**
>
> The input spatial size is `17 × 17 = 289` because the encoder added a
> one-cell border ring of opponent stones around the 15×15 board. That
> border is never a legal move target. The policy head convolves 1×1
> over the full 17×17 extent, producing `2·17·17 = 578` features, then
> the linear layer maps down to 225 — one logit per cell on the
> original 15×15 board. The network may learn to place low mass on the
> border cells, but the loss and metrics are defined over the 225
> legal-move logits. At inference time MCTS will mask illegal moves
> (including the border) before applying the softmax.

> **Excursion — seeded init and backend RNG behavior**
>
> A natural test idea is: "seed the backend, initialize two models, and
> assert their outputs are bit-identical." The reference implementation
> tried exactly that and discovered that Burn 0.21's `B::seed` does
> **not** make two separate `Model::new` or `ModelConfig::init` calls
> produce bit-identical weights in the same process. This is not a bug
> in the network module; it is a backend-level quirk of how weight
> initialization draws randomness.
>
> The practical consequence is that reproducibility tests should not
> rely on two independent seeded inits. Chapter 5's reproducibility
> test initializes one model and `clone()`s it for the second run; that
> comparison isolates training-loop determinism from init-time
> nondeterminism. If you write a seeded-init test and it fails on two
> fresh models, document the behavior and move on — the module is
> correct.

## Low-level design

### Files

```text
gomoku/crates/net/
└── src/
    ├── lib.rs          # add `pub mod network;`
    └── network.rs      # new module
```

### `src/lib.rs`

Add `pub mod network;` with the existing module declarations.

### `src/network.rs`

Module-level documentation should state the input shape and the
architecture lock (docs/12 §10).

```rust
use burn::nn::conv::Conv2d;
use burn::nn::norm::BatchNorm;
use burn::nn::{Linear, LinearConfig, PaddingConfig2d, Relu};
use burn::tensor::Tensor;
use burn::tensor::activation::tanh;
use burn::tensor::backend::Backend;
use burn::{config::Config, module::Module};

use engine::EXT;
```

`ModelConfig`:

```rust
#[derive(Config, Debug, PartialEq)]
pub struct ModelConfig {
    #[config(default = "128")]
    pub channels: usize,
    #[config(default = "10")]
    pub num_blocks: usize,
}
```

with `init`, `tiny`, and `mid` methods.

`Model<B: Backend>`:

```rust
#[derive(Module, Debug)]
pub struct Model<B: Backend> {
    pub stem_conv: Conv2d<B>,
    pub stem_bn: BatchNorm<B>,
    pub blocks: Vec<ResBlock<B>>,
    pub policy_conv: Conv2d<B>,
    pub policy_bn: BatchNorm<B>,
    pub policy_linear: Linear<B>,
    pub value_conv: Conv2d<B>,
    pub value_bn: BatchNorm<B>,
    pub value_hidden: Linear<B>,
    pub value_out: Linear<B>,
    pub activation: Relu,
}
```

All fields are `pub` so tests and the training loop can inspect them;
they are still read-only to callers outside the module because the
module owns mutation.

`ModelOutput<B: Backend>`:

```rust
#[derive(Debug, Clone)]
pub struct ModelOutput<B: Backend> {
    pub policy: Tensor<B, 2>,
    pub value: Tensor<B, 2>,
}
```

`Model::new` builds the layers:

* Stem: `Conv2dConfig::new([4, channels], [3, 3])`
  `.with_padding(PaddingConfig2d::Same)` `.with_bias(false)` `.init(device)`.
* `BatchNormConfig::new(channels).init(device)` for the stem batch norm.
* `(0..num_blocks).map(|_| ResBlock::new(channels, device)).collect()`.
* Policy head: `Conv2dConfig::new([channels, 2], [1, 1])` same-pad no-bias;
  `BatchNormConfig::new(2)`; `LinearConfig::new(2 * EXT * EXT, 225)`.
* Value head: `Conv2dConfig::new([channels, 1], [1, 1])` same-pad no-bias;
  `BatchNormConfig::new(1)`; `LinearConfig::new(EXT * EXT, 256)`;
  `LinearConfig::new(256, 1)`.
* Store `Relu::new()` once.

`Model::forward`:

1. `let x = self.stem_conv.forward(input);`
2. `let x = self.stem_bn.forward(x);`
3. `let mut x = self.activation.forward(x);`
4. For each block, `x = block.forward(x);`
5. Policy head on `x.clone()`; value head on `x`.
6. Return `ModelOutput { policy, value }`.

`ResBlock<B: Backend>`:

```rust
#[derive(Module, Debug)]
pub struct ResBlock<B: Backend> {
    conv1: Conv2d<B>,
    bn1: BatchNorm<B>,
    conv2: Conv2d<B>,
    bn2: BatchNorm<B>,
    activation: Relu,
}
```

`ResBlock::new` builds two same-padded 3×3 convolutions with batch norms
in between. `ResBlock::forward` is:

```rust
let residual = x.clone();
let x = self.conv1.forward(x);
let x = self.bn1.forward(x);
let x = self.activation.forward(x);
let x = self.conv2.forward(x);
let x = self.bn2.forward(x);
let x = x + residual;
self.activation.forward(x)
```

The skip connection requires same padding so the spatial sizes match.

You will need `use burn::nn::norm::BatchNormConfig;` at module scope.

### Tests

Use `burn::backend::Flex` as the concrete test backend, matching the
convolutions tutorial precedent:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use burn::backend::Flex;

    type B = Flex;

    // tests follow...
}
```

1. `default_model_output_shapes` — create the device with
   `Default::default()`, init `ModelConfig::new()`, forward a zero
   tensor of shape `[2, 4, EXT, EXT]`, assert `out.policy.dims() ==
   [2, 225]` and `out.value.dims() == [2, 1]`.
2. `tiny_model_output_shapes` — same with `ModelConfig::tiny()` and a
   batch size of 1.
3. `forward_is_deterministic_on_same_input` — init a tiny model,
   forward the same input twice, convert the policy tensors to
   `Vec<f32>`, and assert equality.
4. `value_is_in_range` — forward a batch of 4 zeros through a tiny
   model, collect the value scalars, and assert each is in `[-1, 1]`.

> **Burn trap — `argmax` keeps the reduced dimension**
>
> This chapter does not need `argmax`, but chapter 4 does. Remember
> from [`docs/11-pitfalls.md`](../../11-pitfalls.md) that
> `tensor.argmax(1)` on `[B, 225]` returns `[B, 1]`. Comparing against
> a `[B]` tensor fails with a confusing type error; squeeze with
> `.argmax(1).squeeze_dim::<1>(1)`.

## Solution (opt-in)

The complete reference code for this chapter — `network.rs` and the
`lib.rs` registration line — lives in
[03-the-network-module/01-solution.md](03-the-network-module/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare.

## TDD checklist

1. **Red:** Create `network.rs` with the `ModelConfig`, `Model`,
   `ResBlock`, and `ModelOutput` declarations, the `forward` and
   `ResBlock::forward` signatures, and the four tests, leaving bodies as
   `todo!()`. Register `pub mod network;` in `lib.rs`. Run `cargo test
   -p net` and expect failures from the `todo!()` panics.
2. **Green:** Implement the layers and forward pass. Re-run `cargo test
   -p net`. All four network tests should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p net` passes from `gomoku/`.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.

Next: Chapter 04 — Loss + metrics.

## References

* [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  §10 the residual network anatomy and the locked 128×10 size.
* [`docs/11-pitfalls.md`](../../11-pitfalls.md) — Burn traps including
  `argmax` dimension keeping.
* [`docs/tutorials/convolutions-tutorial/`](../convolutions-tutorial/) —
  Burn 0.21 `Conv2d`, `BatchNorm`, and `Linear` usage precedent.
* Burn 0.21.0 source, `burn-nn-0.21.0/src/loss/cross_entropy.rs` —
  confirms the built-in cross entropy expects hard class-index targets,
  motivating the hand-rolled soft-target loss in chapter 4.
