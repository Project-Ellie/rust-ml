# Chapter 04 — Loss + metrics

## Abstract

This chapter adds the loss functions and training metrics to the `net`
crate. The policy loss is a hand-rolled soft-target cross entropy,
because Burn 0.21's built-in cross entropy expects hard class indices
and the policy target `π` is a probability distribution. The value loss
is mean squared error. The two are summed into the combined loss used
for backpropagation. Two metrics — policy top-1 agreement and value-sign
agreement — let the training loop report progress. By the end you will
have `loss.rs` with five hand-verified tests.

## Glossary

| Term | Definition |
|------|------------|
| **Soft target** | A probability distribution over classes, as opposed to a single hard class index. The policy target `π` is a soft target. |
| **Cross entropy** | For a soft target `π` and model probabilities `p`, `CE = −Σᵢ πᵢ log(pᵢ)`. |
| **`log_softmax`** | Numerically stable `log(softmax(logits))`, computed over the move dimension. |
| **Mean squared error (MSE)** | For predictions `v̂` and targets `v`, `(1/N) Σ (v̂ − v)²`. |
| **Policy top-1 accuracy** | Fraction of samples where `argmax(policy_logits)` equals `argmax(policy_target)`. |
| **Value-sign accuracy** | Fraction of samples where the predicted value and target value have the same sign. |
| **`squeeze_dim`** | Burn tensor method that removes a dimension of size 1; needed because `argmax` keeps the reduced axis. |

## Context

Chapter 3 built the network module: a `Model<B>` that maps a `[B, 4, 17,
17]` input to policy logits `[B, 225]` and a value `[B, 1]`. This
chapter closes the supervised-learning loop by defining what "better"
means. The loss function is the scalar that backpropagation minimizes;
the metrics are the diagnostics that tell us whether the network is
learning.

The loss design is locked in
[`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) §10:
policy cross-entropy against the visit-count distribution `π` plus value
mean squared error, combined by simple summation. The catch is that
`π` is a soft target: it distributes mass over many moves (for example,
90 % over a tactical set and 10 % uniform over the rest, as produced by
the datagen tutorial). Burn 0.21's `CrossEntropyLoss::forward` takes
`targets: Tensor<B, 1, Int>` — hard class indices — so the policy loss
must be written by hand. That is invention I4.

## Intention

1. Create `gomoku/crates/net/src/loss.rs` and register
   `pub mod loss;` in `gomoku/crates/net/src/lib.rs`.
2. Define `LossInput<B: Backend>` to bundle policy logits, policy
target, value prediction, and value target.
3. Define `LossOutput<B: Backend>` to hold the three scalar loss values
   (`total`, `policy`, `value`) pulled back to the host.
4. Implement `compute_loss<B: Backend>(input: LossInput<B>) ->
   Tensor<B, 1>` as `policy_loss + value_loss`.
5. Implement `policy_loss` by hand:
   * `log_probs = log_softmax(logits, 1)`;
   * `-(target * log_probs).sum_dim(1).mean()`.
6. Implement `value_loss` as MSE:
   * `(pred - target).powf_scalar(2.0).mean()`.
7. Implement `compute_metrics<B: Backend>(...)`:
   * policy top-1 accuracy using `argmax(1).squeeze_dim::<1>(1)` and
     element-wise equality;
   * value-sign accuracy using `greater_elem(0.0).int()` and equality.
8. Add a helper `loss_scalar<B: Backend>(loss: &Tensor<B, 1>) -> f32`
   for logging.
9. Write five tests on hand-computed values:
   * one-sample soft-target CE against the known `log_softmax` numbers;
   * MSE on a three-sample batch;
   * combined loss equals the sum of the two losses;
   * metrics count correct argmax and sign correctly on a two-sample
     batch;
   * `loss_scalar` round-trips a known tensor.

Observable done-state: `cargo test -p net` passes, `cargo clippy
--all-targets -- -D warnings` is green, and `cargo fmt --all --
--check` makes no changes.

## Mental mapping

### Why the policy loss cannot use `CrossEntropyLoss`

Burn 0.21 provides `burn::nn::loss::CrossEntropyLoss`. Its `forward`
signature is:

```rust
pub fn forward(&self, logits: Tensor<B, 2>, targets: Tensor<B, 1, Int>) -> Tensor<B, 1>
```

The target is a one-dimensional integer tensor of class indices. The
implementation gathers the log-probability at each target index and
averages the negatives. That is exactly what you want for MNIST, where
each sample has one correct digit. It is not what you want here,
because the policy target is a distribution over 225 moves.

A hand-rolled soft-target cross entropy is therefore not a workaround —
it is the correct formula for the problem. The expression
`-(π · log_softmax(logits)).sum(-1).mean()` is the direct translation of
`− Σᵢ πᵢ log(pᵢ)` averaged over the batch.

> **Excursion — the `argmax` dimension trap**
>
> The metrics need to compare the predicted best move with the target
> best move. A first attempt often looks like:
>
> ```rust
> let pred_argmax = policy_logits.argmax(1);
> let target_argmax = policy_target.argmax(1);
> let correct = pred_argmax.equal(target_argmax);
> ```
>
> This fails to compile in Burn 0.21 because `argmax(1)` on a `[B,
> 225]` tensor returns `[B, 1]`, not `[B]`. The two `[B, 1]` tensors can
> be compared, but the result is also `[B, 1]`, and reducing it requires
> an extra `squeeze`. The documented fix from
> [`docs/11-pitfalls.md`](../../11-pitfalls.md) is:
>
> ```rust
> let pred_argmax = policy_logits.clone().argmax(1).squeeze_dim::<1>(1);
> let target_argmax = policy_target.clone().argmax(1).squeeze_dim::<1>(1);
> ```
>
> This gives two `[B]` integer tensors whose element-wise equality can
> be summed directly. The same issue appears in any metric that compares
> a reduced tensor against a vector; always check whether Burn kept the
> reduced axis.

> **Excursion — metric aggregation and the last batch**
>
> `compute_metrics` returns per-batch fractions. When the training loop
> aggregates over an epoch, it must weight each batch by its size.
> Averaging per-batch accuracies without weighting silently underweights
> the final, possibly shorter batch. Chapter 5's `evaluate` function
> accumulates raw correct counts and total samples, then divides once at
> the end. That pattern is also listed in
> [`docs/11-pitfalls.md`](../../11-pitfalls.md) as the "custom metrics
> must weight by batch size" trap.

### Value sign vs. value magnitude

The value head is trained with MSE, but MSE alone is hard to interpret
early in training. Value-sign accuracy — "did the model at least get the
winner right?" — is a much noisier but more human-readable signal. A
network that predicts `+0.1` for a `+1.0` target is wrong by MSE but
right by sign; both numbers belong in the run journal.

## Low-level design

### Files

```text
gomoku/crates/net/
└── src/
    ├── lib.rs          # add `pub mod loss;`
    └── loss.rs         # new module
```

### `src/lib.rs`

Add `pub mod loss;` with the existing module declarations.

### `src/loss.rs`

Module-level documentation should state that the policy loss is
hand-rolled soft-target cross entropy and cite invention I4.

```rust
use burn::prelude::ElementConversion;
use burn::tensor::Tensor;
use burn::tensor::activation::log_softmax;
use burn::tensor::backend::Backend;
```

`LossInput<B: Backend>`:

```rust
#[derive(Debug, Clone)]
pub struct LossInput<B: Backend> {
    pub policy_logits: Tensor<B, 2>,
    pub policy_target: Tensor<B, 2>,
    pub value_pred: Tensor<B, 2>,
    pub value_target: Tensor<B, 2>,
}
```

`LossOutput<B: Backend>`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LossOutput<B: Backend> {
    pub total: B::FloatElem,
    pub policy: B::FloatElem,
    pub value: B::FloatElem,
}
```

`compute_loss`:

```rust
pub fn compute_loss<B: Backend>(input: LossInput<B>) -> Tensor<B, 1> {
    let policy = policy_loss(input.policy_logits, input.policy_target);
    let value = value_loss(input.value_pred, input.value_target);
    policy + value
}
```

`policy_loss`:

```rust
fn policy_loss<B: Backend>(logits: Tensor<B, 2>, target: Tensor<B, 2>) -> Tensor<B, 1> {
    let log_probs = log_softmax(logits, 1);
    -(target * log_probs).sum_dim(1).mean()
}
```

`value_loss`:

```rust
fn value_loss<B: Backend>(pred: Tensor<B, 2>, target: Tensor<B, 2>) -> Tensor<B, 1> {
    let diff = pred - target;
    diff.powf_scalar(2.0).mean()
}
```

`compute_metrics`:

```rust
pub fn compute_metrics<B: Backend>(
    policy_logits: &Tensor<B, 2>,
    policy_target: &Tensor<B, 2>,
    value_pred: &Tensor<B, 2>,
    value_target: &Tensor<B, 2>,
) -> Metrics {
    let batch = policy_logits.dims()[0];

    let pred_argmax = policy_logits.clone().argmax(1).squeeze_dim::<1>(1);
    let target_argmax = policy_target.clone().argmax(1).squeeze_dim::<1>(1);
    let policy_correct = pred_argmax.equal(target_argmax).int().sum().into_scalar();
    let policy_correct: i32 = policy_correct.elem();

    let pred_sign = value_pred.clone().greater_elem(0.0).int();
    let target_sign = value_target.clone().greater_elem(0.0).int();
    let value_correct = pred_sign.equal(target_sign).int().sum().into_scalar();
    let value_correct: i32 = value_correct.elem();

    Metrics {
        policy_top1_accuracy: policy_correct as f32 / batch as f32,
        value_sign_accuracy: value_correct as f32 / batch as f32,
    }
}
```

`loss_scalar`:

```rust
pub fn loss_scalar<B: Backend>(loss: &Tensor<B, 1>) -> f32 {
    loss.clone().into_scalar().elem()
}
```

The `Metrics` struct:

```rust
pub struct Metrics {
    pub policy_top1_accuracy: f32,
    pub value_sign_accuracy: f32,
}
```

### Tests

Use `burn::backend::Flex` and `burn::backend::flex::FlexDevice`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use burn::backend::Flex;
    use burn::backend::flex::FlexDevice;

    type B = Flex;
}
```

1. `policy_loss_on_hand_computed_example` — one sample, three moves.
   Logits `[2.0, 1.0, 0.0]`, target `[0.8, 0.2, 0.0]`. Compute
   `log_softmax([2,1,0]) ≈ [-0.4076, -1.4076, -2.4076]` and the
   expected CE `≈ 0.6076`. Assert the computed loss is within `1e-3`.
2. `value_loss_is_mse` — predictions `[0.5, -0.5, 0.0]`, targets `[1.0,
   -1.0, 0.0]`. Expected MSE `(0.25 + 0.25 + 0) / 3 = 0.1667`. Assert
   within `1e-5`.
3. `combined_loss_is_sum` — build a `LossInput` with the policy example
   and a single value sample, compute `compute_loss`, then compute the
   two losses separately and assert `total ≈ policy + value`.
4. `metrics_count_correct_argmax_and_sign` — a batch of two samples.
   Sample 0: predicted argmax 1 matches target argmax 1, both values
   positive. Sample 1: predicted argmax 0 does not match target argmax
   2, predicted positive vs. target negative. Expected 0.5 for both
   metrics.
5. `loss_scalar_roundtrips` — create a `[1.25]` tensor, pass it through
   `loss_scalar`, and assert the returned `f32` is close to `1.25`.

## Solution (opt-in)

The complete reference code for this chapter — `loss.rs` and the
`lib.rs` registration line — lives in
[04-loss-and-metrics/01-solution.md](04-loss-and-metrics/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare.

## TDD checklist

1. **Red:** Create `loss.rs` with the structs, function signatures, and
   five tests, leaving the function bodies as `todo!()`. Register `pub
   mod loss;` in `lib.rs`. Run `cargo test -p net` and expect failures
   from the `todo!()` panics.
2. **Green:** Implement the loss functions and metrics. Re-run `cargo
   test -p net`. All nine tests (four from chapter 3 plus five from this
   chapter) should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p net` passes from `gomoku/`.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.

Next: Chapter 05 — The manual training loop.

## References

* [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  §10 the combined loss formula and the AdamW/weight-decay optimizer
  choice.
* [`docs/11-pitfalls.md`](../../11-pitfalls.md) — the `argmax`
  dimension-keeping trap and the batch-size-weighted metric trap.
* [`docs/tutorials/datagen-tutorial/03-tactics-labels.md`](../datagen-tutorial/03-tactics-labels.md) —
  how the policy target `π` and value target `z` are produced.
* Burn 0.21.0 source, `burn-nn-0.21.0/src/loss/cross_entropy.rs` —
  verifies that `CrossEntropyLoss::forward` takes hard `Int` targets,
  which is why the policy loss is hand-rolled.
