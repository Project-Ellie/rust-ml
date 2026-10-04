# Chapter 03 solution — The network module

This file shows the reference implementation for chapter 03. It is
identical to the verified `network.rs` in the reference worktree, plus
the one-line registration change in `lib.rs`.

## `gomoku/crates/net/src/lib.rs`

Add `pub mod network;` with the other module declarations:

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

## `gomoku/crates/net/src/network.rs`

```rust
//! Policy/value residual network for Gomoku.
//!
//! Architecture (docs/12, §10):
//!
//! * Input: `[B, 4, 17, 17]`
//! * Stem: Conv2d 3×3, 4 → channels, same-pad, BatchNorm, ReLU
//! * `num_blocks` residual blocks:
//!   Conv2d 3×3 → BatchNorm → ReLU → Conv2d 3×3 → BatchNorm → +skip → ReLU
//! * Policy head: Conv2d 1×1 channels→2, BatchNorm, ReLU,
//!   flatten, Linear 2·17·17 → 225
//! * Value head: Conv2d 1×1 channels→1, BatchNorm, ReLU,
//!   flatten, Linear 17·17 → 256, ReLU, Linear 256 → 1, tanh

use burn::nn::conv::Conv2d;
use burn::nn::norm::BatchNorm;
use burn::nn::{Linear, LinearConfig, PaddingConfig2d, Relu};
use burn::tensor::Tensor;
use burn::tensor::activation::tanh;
use burn::tensor::backend::Backend;
use burn::{config::Config, module::Module};

use engine::EXT;

/// Network configuration.
#[derive(Config, Debug, PartialEq)]
pub struct ModelConfig {
    /// Number of channels in the residual trunk.
    #[config(default = "128")]
    pub channels: usize,
    /// Number of residual blocks.
    #[config(default = "10")]
    pub num_blocks: usize,
}

impl ModelConfig {
    /// Initialize the model on `device`.
    pub fn init<B: Backend>(&self, device: &B::Device) -> Model<B> {
        Model::new(self.channels, self.num_blocks, device)
    }

    /// A tiny config for fast tests.
    pub fn tiny() -> Self {
        Self {
            channels: 8,
            num_blocks: 1,
        }
    }

    /// A mid-sized config for reduced-scale smoke runs.
    pub fn mid() -> Self {
        Self {
            channels: 32,
            num_blocks: 4,
        }
    }
}

/// Policy/value network.
#[derive(Module, Debug)]
pub struct Model<B: Backend> {
    /// Initial convolution.
    pub stem_conv: Conv2d<B>,
    /// Initial batch norm.
    pub stem_bn: BatchNorm<B>,
    /// Residual trunk.
    pub blocks: Vec<ResBlock<B>>,
    /// Policy head convolution.
    pub policy_conv: Conv2d<B>,
    /// Policy head batch norm.
    pub policy_bn: BatchNorm<B>,
    /// Policy head linear layer.
    pub policy_linear: Linear<B>,
    /// Value head convolution.
    pub value_conv: Conv2d<B>,
    /// Value head batch norm.
    pub value_bn: BatchNorm<B>,
    /// Value head hidden layer.
    pub value_hidden: Linear<B>,
    /// Value head output layer.
    pub value_out: Linear<B>,
    /// ReLU activation (shared, stateless).
    pub activation: Relu,
}

/// Forward outputs.
#[derive(Debug, Clone)]
pub struct ModelOutput<B: Backend> {
    /// Policy logits of shape `[B, 225]`.
    pub policy: Tensor<B, 2>,
    /// Value in [-1, 1] of shape `[B, 1]`.
    pub value: Tensor<B, 2>,
}

impl<B: Backend> Model<B> {
    /// Build a model with the given trunk width and depth.
    pub fn new(channels: usize, num_blocks: usize, device: &B::Device) -> Self {
        let stem_conv = burn::nn::conv::Conv2dConfig::new([4, channels], [3, 3])
            .with_padding(PaddingConfig2d::Same)
            .with_bias(false)
            .init(device);
        let stem_bn = BatchNormConfig::new(channels).init(device);

        let blocks = (0..num_blocks)
            .map(|_| ResBlock::new(channels, device))
            .collect();

        let policy_conv = burn::nn::conv::Conv2dConfig::new([channels, 2], [1, 1])
            .with_padding(PaddingConfig2d::Same)
            .with_bias(false)
            .init(device);
        let policy_bn = BatchNormConfig::new(2).init(device);
        let policy_linear = LinearConfig::new(2 * EXT * EXT, 225).init(device);

        let value_conv = burn::nn::conv::Conv2dConfig::new([channels, 1], [1, 1])
            .with_padding(PaddingConfig2d::Same)
            .with_bias(false)
            .init(device);
        let value_bn = BatchNormConfig::new(1).init(device);
        let value_hidden = LinearConfig::new(EXT * EXT, 256).init(device);
        let value_out = LinearConfig::new(256, 1).init(device);

        Self {
            stem_conv,
            stem_bn,
            blocks,
            policy_conv,
            policy_bn,
            policy_linear,
            value_conv,
            value_bn,
            value_hidden,
            value_out,
            activation: Relu::new(),
        }
    }

    /// Forward pass: input shape `[B, 4, EXT, EXT]`.
    pub fn forward(&self, input: Tensor<B, 4>) -> ModelOutput<B> {
        let x = self.stem_conv.forward(input);
        let x = self.stem_bn.forward(x);
        let mut x = self.activation.forward(x);

        for block in &self.blocks {
            x = block.forward(x);
        }

        // Policy head.
        let p = self.policy_conv.forward(x.clone());
        let p = self.policy_bn.forward(p);
        let p = self.activation.forward(p);
        let p = p.flatten(1, 3);
        let policy = self.policy_linear.forward(p);

        // Value head.
        let v = self.value_conv.forward(x);
        let v = self.value_bn.forward(v);
        let v = self.activation.forward(v);
        let v = v.flatten(1, 3);
        let v = self.value_hidden.forward(v);
        let v = self.activation.forward(v);
        let v = self.value_out.forward(v);
        let value = tanh(v);

        ModelOutput { policy, value }
    }
}

/// A single residual block.
#[derive(Module, Debug)]
pub struct ResBlock<B: Backend> {
    conv1: Conv2d<B>,
    bn1: BatchNorm<B>,
    conv2: Conv2d<B>,
    bn2: BatchNorm<B>,
    activation: Relu,
}

impl<B: Backend> ResBlock<B> {
    fn new(channels: usize, device: &B::Device) -> Self {
        let conv1 = burn::nn::conv::Conv2dConfig::new([channels, channels], [3, 3])
            .with_padding(PaddingConfig2d::Same)
            .with_bias(false)
            .init(device);
        let bn1 = BatchNormConfig::new(channels).init(device);
        let conv2 = burn::nn::conv::Conv2dConfig::new([channels, channels], [3, 3])
            .with_padding(PaddingConfig2d::Same)
            .with_bias(false)
            .init(device);
        let bn2 = BatchNormConfig::new(channels).init(device);

        Self {
            conv1,
            bn1,
            conv2,
            bn2,
            activation: Relu::new(),
        }
    }

    fn forward(&self, x: Tensor<B, 4>) -> Tensor<B, 4> {
        let residual = x.clone();
        let x = self.conv1.forward(x);
        let x = self.bn1.forward(x);
        let x = self.activation.forward(x);
        let x = self.conv2.forward(x);
        let x = self.bn2.forward(x);
        let x = x + residual;
        self.activation.forward(x)
    }
}

use burn::nn::norm::BatchNormConfig;

#[cfg(test)]
mod tests {
    use super::*;
    use burn::backend::Flex;

    type B = Flex;

    #[test]
    fn default_model_output_shapes() {
        let device = Default::default();
        let model: Model<B> = ModelConfig::new().init(&device);
        let input = Tensor::<B, 4>::zeros([2, 4, EXT, EXT], &device);
        let out = model.forward(input);
        assert_eq!(out.policy.dims(), [2, 225]);
        assert_eq!(out.value.dims(), [2, 1]);
    }

    #[test]
    fn tiny_model_output_shapes() {
        let device = Default::default();
        let model: Model<B> = ModelConfig::tiny().init(&device);
        let input = Tensor::<B, 4>::zeros([1, 4, EXT, EXT], &device);
        let out = model.forward(input);
        assert_eq!(out.policy.dims(), [1, 225]);
        assert_eq!(out.value.dims(), [1, 1]);
    }

    #[test]
    fn forward_is_deterministic_on_same_input() {
        let device = Default::default();
        let model: Model<B> = ModelConfig::tiny().init(&device);
        let input = Tensor::<B, 4>::zeros([2, 4, EXT, EXT], &device);
        let out_a = model.forward(input.clone());
        let out_b = model.forward(input);
        let policy_a: Vec<f32> = out_a.policy.into_data().to_vec::<f32>().unwrap();
        let policy_b: Vec<f32> = out_b.policy.into_data().to_vec::<f32>().unwrap();
        assert_eq!(policy_a, policy_b);
    }

    #[test]
    fn value_is_in_range() {
        let device = Default::default();
        let model: Model<B> = ModelConfig::tiny().init(&device);
        let input = Tensor::<B, 4>::zeros([4, 4, EXT, EXT], &device);
        let out = model.forward(input);
        let values: Vec<f32> = out.value.into_data().to_vec::<f32>().unwrap();
        for &v in &values {
            assert!((-1.0..=1.0).contains(&v), "tanh output {v} out of [-1, 1]");
        }
    }
}
```
