# Chapter 04 solution — Loss + metrics

This file shows the reference implementation for chapter 04. It is
identical to the verified `loss.rs` in the reference worktree, plus the
one-line registration change in `lib.rs`.

## `gomoku/crates/net/src/lib.rs`

Add `pub mod loss;` with the other module declarations:

```rust
//! Neural network training for the Gomoku AlphaZero-style agent.
//!
//! This crate implements the policy/value network and the manual
//! supervised-training loop for phase 0. It consumes labelled samples
//! from the `train` crate and derives input planes from engine boards.

#![deny(missing_docs)]

pub mod batcher;
pub mod checkpoint;
pub mod loss;
pub mod network;
pub mod planes;
pub mod train;
```

## `gomoku/crates/net/src/loss.rs`

```rust
//! Loss functions and training metrics.
//!
//! Policy loss is hand-rolled soft-target cross entropy (I4):
//! `-(π · log_softmax(logits)).sum(-1).mean()`. Burn's built-in
//! cross entropy expects hard class indices, so we roll our own.
//! Value loss is MSE against the scalar target. The combined loss is
//! the sum.

use burn::prelude::ElementConversion;
use burn::tensor::Tensor;
use burn::tensor::activation::log_softmax;
use burn::tensor::backend::Backend;

/// Network output and target batch bundled for loss computation.
#[derive(Debug, Clone)]
pub struct LossInput<B: Backend> {
    /// Policy logits of shape `[B, 225]`.
    pub policy_logits: Tensor<B, 2>,
    /// Dense policy target of shape `[B, 225]`.
    pub policy_target: Tensor<B, 2>,
    /// Value prediction of shape `[B, 1]`.
    pub value_pred: Tensor<B, 2>,
    /// Value target of shape `[B, 1]`.
    pub value_target: Tensor<B, 2>,
}

/// Loss values for logging.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LossOutput<B: Backend> {
    /// Combined loss (policy CE + value MSE).
    pub total: B::FloatElem,
    /// Soft-target cross-entropy loss.
    pub policy: B::FloatElem,
    /// Mean squared error on the value head.
    pub value: B::FloatElem,
}

/// Compute combined policy + value loss.
pub fn compute_loss<B: Backend>(input: LossInput<B>) -> Tensor<B, 1> {
    let policy = policy_loss(input.policy_logits, input.policy_target);
    let value = value_loss(input.value_pred, input.value_target);
    policy + value
}

/// Soft-target cross entropy.
fn policy_loss<B: Backend>(logits: Tensor<B, 2>, target: Tensor<B, 2>) -> Tensor<B, 1> {
    let log_probs = log_softmax(logits, 1);
    // Element-wise π · log p, sum over moves, mean over batch.
    -(target * log_probs).sum_dim(1).mean()
}

/// Mean squared error on the value head.
fn value_loss<B: Backend>(pred: Tensor<B, 2>, target: Tensor<B, 2>) -> Tensor<B, 1> {
    let diff = pred - target;
    diff.powf_scalar(2.0).mean()
}

/// Training/evaluation metrics.
pub struct Metrics {
    /// Fraction of samples where the policy argmax matches the target argmax.
    pub policy_top1_accuracy: f32,
    /// Fraction of samples where the predicted value has the same sign as the target.
    pub value_sign_accuracy: f32,
}

/// Compute metrics from raw model outputs and targets.
///
/// All tensors are on the backend; the result is pulled back to the host.
pub fn compute_metrics<B: Backend>(
    policy_logits: &Tensor<B, 2>,
    policy_target: &Tensor<B, 2>,
    value_pred: &Tensor<B, 2>,
    value_target: &Tensor<B, 2>,
) -> Metrics {
    let batch = policy_logits.dims()[0];

    // Policy argmax agreement.
    let pred_argmax = policy_logits.clone().argmax(1).squeeze_dim::<1>(1);
    let target_argmax = policy_target.clone().argmax(1).squeeze_dim::<1>(1);
    let policy_correct = pred_argmax.equal(target_argmax).int().sum().into_scalar();
    let policy_correct: i32 = policy_correct.elem();

    // Value sign agreement.
    let pred_sign = value_pred.clone().greater_elem(0.0).int();
    let target_sign = value_target.clone().greater_elem(0.0).int();
    let value_correct = pred_sign.equal(target_sign).int().sum().into_scalar();
    let value_correct: i32 = value_correct.elem();

    Metrics {
        policy_top1_accuracy: policy_correct as f32 / batch as f32,
        value_sign_accuracy: value_correct as f32 / batch as f32,
    }
}

/// Convenience: extract a scalar loss value for logging.
pub fn loss_scalar<B: Backend>(loss: &Tensor<B, 1>) -> f32 {
    loss.clone().into_scalar().elem()
}

#[cfg(test)]
mod tests {
    use super::*;
    use burn::backend::Flex;
    use burn::backend::flex::FlexDevice;

    type B = Flex;

    fn logits_and_target() -> (FlexDevice, Tensor<B, 2>, Tensor<B, 2>) {
        let device = FlexDevice;
        // One sample, three moves. Logits strongly favor move 0.
        let logits = Tensor::<B, 2>::from_floats([[2.0, 1.0, 0.0]], &device);
        // Target puts 80% on move 0, 20% on move 1.
        let target = Tensor::<B, 2>::from_floats([[0.8, 0.2, 0.0]], &device);
        (device, logits, target)
    }

    #[test]
    fn policy_loss_on_hand_computed_example() {
        let (_device, logits, target) = logits_and_target();
        let loss = policy_loss(logits, target);
        let loss: f32 = loss.into_scalar().elem();

        // log_softmax([2,1,0]) = [-0.4076, -1.4076, -2.4076]
        // CE = -(0.8 * -0.4076 + 0.2 * -1.4076) = 0.3261 + 0.2815 = 0.6076
        assert!(
            (loss - 0.6076).abs() < 1e-3,
            "loss {loss} not close to 0.6076"
        );
    }

    #[test]
    fn value_loss_is_mse() {
        let device = FlexDevice;
        let pred = Tensor::<B, 2>::from_floats([[0.5], [-0.5], [0.0]], &device);
        let target = Tensor::<B, 2>::from_floats([[1.0], [-1.0], [0.0]], &device);
        let loss = value_loss(pred, target);
        let loss: f32 = loss.into_scalar().elem();
        // MSE = (0.25 + 0.25 + 0) / 3 = 0.1667
        assert!((loss - 0.1666667).abs() < 1e-5);
    }

    #[test]
    fn combined_loss_is_sum() {
        let (device, logits, target) = logits_and_target();
        let value_pred = Tensor::<B, 2>::from_floats([[0.5]], &device);
        let value_target = Tensor::<B, 2>::from_floats([[1.0]], &device);

        let total = compute_loss(LossInput {
            policy_logits: logits.clone(),
            policy_target: target.clone(),
            value_pred: value_pred.clone(),
            value_target: value_target.clone(),
        });
        let p = policy_loss(logits, target);
        let v = value_loss(value_pred, value_target);

        let total_f: f32 = total.into_scalar().elem();
        let p_f: f32 = p.into_scalar().elem();
        let v_f: f32 = v.into_scalar().elem();
        assert!((total_f - (p_f + v_f)).abs() < 1e-5);
    }

    #[test]
    fn metrics_count_correct_argmax_and_sign() {
        let device = FlexDevice;
        // Batch of 2.
        // Sample 0: pred argmax=1, target argmax=1; pred sign +, target sign +.
        // Sample 1: pred argmax=0, target argmax=2; pred sign +, target sign -.
        let policy_logits =
            Tensor::<B, 2>::from_floats([[0.0, 2.0, 1.0], [2.0, 0.0, 0.0]], &device);
        let policy_target =
            Tensor::<B, 2>::from_floats([[0.1, 0.8, 0.1], [0.2, 0.2, 0.6]], &device);
        let value_pred = Tensor::<B, 2>::from_floats([[0.5], [0.1]], &device);
        let value_target = Tensor::<B, 2>::from_floats([[0.3], [-0.5]], &device);

        let metrics = compute_metrics(&policy_logits, &policy_target, &value_pred, &value_target);
        assert!((metrics.policy_top1_accuracy - 0.5).abs() < 1e-5);
        assert!((metrics.value_sign_accuracy - 0.5).abs() < 1e-5);
    }

    #[test]
    fn loss_scalar_roundtrips() {
        let device = FlexDevice;
        let loss = Tensor::<B, 1>::from_floats([1.25], &device);
        assert!((loss_scalar(&loss) - 1.25).abs() < 1e-6);
    }
}
```
