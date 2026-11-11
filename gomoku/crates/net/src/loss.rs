//! Loss functions and training metrics.
//!
//! Policy loss is hand-rolled soft-target cross entropy (I4):
//! `-(π · log_softmax(logits)).sum(-1).mean()`. Burn's built-in
//! cross entropy expects hard class indices, so we roll our own.
//! Value loss is MSE against the scalar target. The combined loss is
//! the sum.

use burn::tensor::Tensor;
use burn::tensor::activation::log_softmax;

/// Network output and target batch bundled for loss computation.
#[derive(Debug, Clone)]
pub struct LossInput {
    /// Policy logits of shape `[B, 225]`.
    pub policy_logits: Tensor<2>,
    /// Dense policy target of shape `[B, 225]`.
    pub policy_target: Tensor<2>,
    /// Value prediction of shape `[B, 1]`.
    pub value_pred: Tensor<2>,
    /// Value target of shape `[B, 1]`.
    pub value_target: Tensor<2>,
}

/// Loss values for logging.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LossOutput {
    /// Combined loss (policy CE + value MSE).
    pub total: f32,
    /// Soft-target cross-entropy loss.
    pub policy: f32,
    /// Mean squared error on the value head.
    pub value: f32,
}

/// Compute combined policy + value loss.
pub fn compute_loss(input: LossInput) -> Tensor<1> {
    let policy = policy_loss(input.policy_logits, input.policy_target);
    let value = value_loss(input.value_pred, input.value_target);
    policy + value
}

/// Compute combined policy + value loss, returning both the tensor and
/// the scalar decomposition.
///
/// The returned tensor is exactly `policy + value` and remains attached
/// to the computation graph so `.backward()` trains both heads. The
/// scalars are detached copies for logging.
pub fn compute_loss_split(input: LossInput) -> (Tensor<1>, LossOutput) {
    let policy = policy_loss(input.policy_logits, input.policy_target);
    let value = value_loss(input.value_pred, input.value_target);
    let total = policy.clone() + value.clone();
    let output = LossOutput {
        total: total.clone().into_scalar(),
        policy: policy.into_scalar(),
        value: value.into_scalar(),
    };
    (total, output)
}

/// Soft-target cross entropy.
fn policy_loss(logits: Tensor<2>, target: Tensor<2>) -> Tensor<1> {
    let log_probs = log_softmax(logits, 1);
    // Element-wise π · log p, sum over moves, mean over batch.
    -(target * log_probs).sum_dim(1).mean()
}

/// Mean squared error on the value head.
fn value_loss(pred: Tensor<2>, target: Tensor<2>) -> Tensor<1> {
    let diff = pred - target;
    diff.powf_scalar(2.0).mean()
}

/// Whether the predicted policy argmax is any move with maximal positive
/// target mass.
///
/// Tactical targets can assign equal mass to multiple winning or blocking
/// moves; every one of those moves is a correct top-1 prediction.
pub fn policy_top1_correct(policy_logits: &[f32], policy_target: &[f32]) -> bool {
    assert_eq!(
        policy_logits.len(),
        policy_target.len(),
        "policy logits and targets must cover the same moves"
    );
    let Some((predicted, _)) = policy_logits
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
    else {
        return false;
    };
    let Some(target_max) = policy_target.iter().max_by(|a, b| a.total_cmp(b)) else {
        return false;
    };

    policy_target[predicted] > 0.0 && policy_target[predicted] >= *target_max - 1e-6
}

/// Whether a predicted value and target belong to the same exact
/// three-way sign class: negative, zero, or positive.
pub fn value_sign_correct(prediction: f32, target: f32) -> bool {
    fn sign_class(value: f32) -> i8 {
        if value < 0.0 {
            -1
        } else if value > 0.0 {
            1
        } else {
            0
        }
    }

    sign_class(prediction) == sign_class(target)
}

/// Training/evaluation metrics.
pub struct Metrics {
    /// Fraction of samples where the policy argmax matches the target argmax.
    pub policy_top1_accuracy: f32,
    /// Fraction of samples where the predicted value has the same sign as the target.
    pub value_sign_accuracy: f32,
    /// Integer number of policy-correct samples in this batch.
    pub policy_top1_correct: u64,
    /// Integer number of value-sign-correct samples in this batch.
    pub value_sign_correct: u64,
}

/// Compute metrics from raw model outputs and targets.
///
/// All tensors are on the backend; the result is pulled back to the host.
pub fn compute_metrics(
    policy_logits: &Tensor<2>,
    policy_target: &Tensor<2>,
    value_pred: &Tensor<2>,
    value_target: &Tensor<2>,
) -> Metrics {
    let batch = policy_logits.dims()[0];

    // Policy top-1 agreement, accepting every max-mass target move.
    let moves = policy_logits.dims()[1];
    let logits: Vec<f32> = policy_logits
        .clone()
        .into_data()
        .try_to_vec::<f32>()
        .expect("policy logits are f32");
    let policy_targets: Vec<f32> = policy_target
        .clone()
        .into_data()
        .try_to_vec::<f32>()
        .expect("policy targets are f32");
    let policy_correct = logits
        .chunks_exact(moves)
        .zip(policy_targets.chunks_exact(moves))
        .filter(|(logits, target)| policy_top1_correct(logits, target))
        .count() as u64;

    // Value sign agreement: negative, zero, and positive are distinct.
    let predictions: Vec<f32> = value_pred
        .clone()
        .into_data()
        .try_to_vec::<f32>()
        .expect("value predictions are f32");
    let targets: Vec<f32> = value_target
        .clone()
        .into_data()
        .try_to_vec::<f32>()
        .expect("value targets are f32");
    let value_correct = predictions
        .iter()
        .zip(&targets)
        .filter(|(prediction, target)| value_sign_correct(**prediction, **target))
        .count() as u64;

    Metrics {
        policy_top1_accuracy: policy_correct as f32 / batch as f32,
        value_sign_accuracy: value_correct as f32 / batch as f32,
        policy_top1_correct: policy_correct,
        value_sign_correct: value_correct,
    }
}

/// Convenience: extract a scalar loss value for logging.
pub fn loss_scalar(loss: &Tensor<1>) -> f32 {
    loss.clone().into_scalar()
}

#[cfg(test)]
mod tests {
    use super::*;
    use burn::tensor::Device;

    fn logits_and_target() -> (Device, Tensor<2>, Tensor<2>) {
        let device = Device::flex();
        // One sample, three moves. Logits strongly favor move 0.
        let logits = Tensor::<2>::from_floats([[2.0, 1.0, 0.0]], &device);
        // Target puts 80% on move 0, 20% on move 1.
        let target = Tensor::<2>::from_floats([[0.8, 0.2, 0.0]], &device);
        (device, logits, target)
    }

    #[test]
    fn policy_loss_on_hand_computed_example() {
        let (_device, logits, target) = logits_and_target();
        let loss = policy_loss(logits, target);
        let loss: f32 = loss.into_scalar();

        // log_softmax([2,1,0]) = [-0.4076, -1.4076, -2.4076]
        // CE = -(0.8 * -0.4076 + 0.2 * -1.4076) = 0.3261 + 0.2815 = 0.6076
        assert!(
            (loss - 0.6076).abs() < 1e-3,
            "loss {loss} not close to 0.6076"
        );
    }

    #[test]
    fn value_loss_is_mse() {
        let device = Device::flex();
        let pred = Tensor::<2>::from_floats([[0.5], [-0.5], [0.0]], &device);
        let target = Tensor::<2>::from_floats([[1.0], [-1.0], [0.0]], &device);
        let loss = value_loss(pred, target);
        let loss: f32 = loss.into_scalar();
        // MSE = (0.25 + 0.25 + 0) / 3 = 0.1667
        assert!((loss - 0.1666667).abs() < 1e-5);
    }

    #[test]
    fn combined_loss_is_sum() {
        let (device, logits, target) = logits_and_target();
        let value_pred = Tensor::<2>::from_floats([[0.5]], &device);
        let value_target = Tensor::<2>::from_floats([[1.0]], &device);

        let total = compute_loss(LossInput {
            policy_logits: logits.clone(),
            policy_target: target.clone(),
            value_pred: value_pred.clone(),
            value_target: value_target.clone(),
        });
        let p = policy_loss(logits, target);
        let v = value_loss(value_pred, value_target);

        let total_f: f32 = total.into_scalar();
        let p_f: f32 = p.into_scalar();
        let v_f: f32 = v.into_scalar();
        assert!((total_f - (p_f + v_f)).abs() < 1e-5);
    }

    #[test]
    fn metrics_count_correct_argmax_and_sign() {
        let device = Device::flex();
        // Batch of 2.
        // Sample 0: pred argmax=1, target argmax=1; pred sign +, target sign +.
        // Sample 1: pred argmax=0, target argmax=2; pred sign +, target sign -.
        let policy_logits = Tensor::<2>::from_floats([[0.0, 2.0, 1.0], [2.0, 0.0, 0.0]], &device);
        let policy_target = Tensor::<2>::from_floats([[0.1, 0.8, 0.1], [0.2, 0.2, 0.6]], &device);
        let value_pred = Tensor::<2>::from_floats([[0.5], [0.1]], &device);
        let value_target = Tensor::<2>::from_floats([[0.3], [-0.5]], &device);

        let metrics = compute_metrics(&policy_logits, &policy_target, &value_pred, &value_target);
        assert!((metrics.policy_top1_accuracy - 0.5).abs() < 1e-5);
        assert!((metrics.value_sign_accuracy - 0.5).abs() < 1e-5);
    }

    #[test]
    fn policy_metric_accepts_any_max_mass_target_move() {
        let device = Device::flex();
        let policy_logits = Tensor::<2>::from_floats([[1.0, 2.0, 0.0]], &device);
        let policy_target = Tensor::<2>::from_floats([[0.45, 0.45, 0.1]], &device);
        let value_pred = Tensor::<2>::from_floats([[1.0]], &device);
        let value_target = Tensor::<2>::from_floats([[1.0]], &device);

        let metrics = compute_metrics(&policy_logits, &policy_target, &value_pred, &value_target);

        assert_eq!(metrics.policy_top1_correct, 1);
        assert_eq!(metrics.policy_top1_accuracy, 1.0);
    }

    #[test]
    fn metrics_return_integer_correct_counts() {
        let device = Device::flex();
        let policy_logits = Tensor::<2>::from_floats([[1.0, 0.0]; 22], &device);
        let policy_target = Tensor::<2>::from_floats(
            std::array::from_fn::<[f32; 2], 22, _>(|index| {
                if index < 13 { [1.0, 0.0] } else { [0.0, 1.0] }
            }),
            &device,
        );
        let value_pred = Tensor::<2>::from_floats([[0.5]; 22], &device);
        let value_target = Tensor::<2>::from_floats([[1.0]; 22], &device);

        let metrics = compute_metrics(&policy_logits, &policy_target, &value_pred, &value_target);

        assert_eq!(metrics.policy_top1_correct, 13);
        assert_eq!(metrics.value_sign_correct, 22);
        assert_eq!(metrics.policy_top1_accuracy, 13.0 / 22.0);
        assert_eq!(metrics.value_sign_accuracy, 1.0);
    }

    #[test]
    fn metrics_distinguish_negative_and_zero_value_signs() {
        let device = Device::flex();
        let policy_logits = Tensor::<2>::from_floats([[1.0, 0.0], [1.0, 0.0]], &device);
        let policy_target = Tensor::<2>::from_floats([[1.0, 0.0], [1.0, 0.0]], &device);
        let value_pred = Tensor::<2>::from_floats([[-0.5], [0.0]], &device);
        let value_target = Tensor::<2>::from_floats([[0.0], [-1.0]], &device);

        let metrics = compute_metrics(&policy_logits, &policy_target, &value_pred, &value_target);

        assert_eq!(
            metrics.value_sign_accuracy, 0.0,
            "negative-vs-zero must be incorrect in both directions"
        );
    }

    #[test]
    fn loss_scalar_roundtrips() {
        let device = Device::flex();
        let loss = Tensor::<1>::from_floats([1.25], &device);
        assert!((loss_scalar(&loss) - 1.25).abs() < 1e-6);
    }
}
