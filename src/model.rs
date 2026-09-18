//! The MNIST classifier: a small CNN.
//!
//! This is the reference model of the curriculum's first milestone. It follows
//! the idiomatic Burn structure:
//!
//! - A `#[derive(Config)]` struct (`ModelConfig`) holds *hyperparameters only*.
//!   Configs are serializable, so the training run can save them next to the
//!   weights and inference can rebuild the exact same architecture.
//! - `ModelConfig::init` turns the config into a `Model<B>` with freshly
//!   initialized parameters on a given device.
//! - `#[derive(Module)]` on `Model<B>` registers every field as a submodule.
//!   The derive macro generates parameter collection, `load_record` /
//!   `save_file`, `to_device`, `valid()` (eval mode) and more.
//!
//! Architecture (deliberately modest — this is a teaching model, not a record
//! attempt; expect ~98.5-99% test accuracy after a few epochs):
//!
//! ```text
//! [B, 1, 28, 28]
//!   -> Conv2d 1->8, 3x3, no padding  -> [B, 8, 26, 26]  -> Dropout
//!   -> Conv2d 8->16, 3x3, no padding -> [B, 16, 24, 24] -> Dropout -> ReLU
//!   -> AdaptiveAvgPool2d [8, 8]      -> [B, 16, 8, 8]
//!   -> flatten                       -> [B, 1024]
//!   -> Linear 1024->hidden           -> Dropout -> ReLU
//!   -> Linear hidden->10             -> logits [B, 10]
//! ```

use burn::{
    nn::{
        conv::{Conv2d, Conv2dConfig},
        pool::{AdaptiveAvgPool2d, AdaptiveAvgPool2dConfig},
        Dropout, DropoutConfig, Linear, LinearConfig, Relu,
    },
    prelude::*,
};

/// Hyperparameters of the model. Everything needed to rebuild the exact same
/// architecture at inference time — and nothing else.
#[derive(Config, Debug)]
pub struct ModelConfig {
    num_classes: usize,
    hidden_size: usize,
    #[config(default = "0.5")]
    dropout: f64,
}

impl ModelConfig {
    /// Returns the initialized model with random weights on `device`.
    pub fn init<B: Backend>(&self, device: &B::Device) -> Model<B> {
        Model {
            conv1: Conv2dConfig::new([1, 8], [3, 3]).init(device),
            conv2: Conv2dConfig::new([8, 16], [3, 3]).init(device),
            pool: AdaptiveAvgPool2dConfig::new([8, 8]).init(),
            activation: Relu::new(),
            linear1: LinearConfig::new(16 * 8 * 8, self.hidden_size).init(device),
            linear2: LinearConfig::new(self.hidden_size, self.num_classes).init(device),
            dropout: DropoutConfig::new(self.dropout).init(),
        }
    }
}

/// The model, generic over the backend. The same struct definition is used for
/// training (with an `Autodiff` backend) and inference (with a plain backend).
#[derive(Module, Debug)]
pub struct Model<B: Backend> {
    conv1: Conv2d<B>,
    conv2: Conv2d<B>,
    pool: AdaptiveAvgPool2d,
    dropout: Dropout,
    linear1: Linear<B>,
    linear2: Linear<B>,
    activation: Relu,
}

impl<B: Backend> Model<B> {
    /// Forward pass.
    ///
    /// # Shapes
    ///   - Images: `[batch_size, height, width]`
    ///   - Output: `[batch_size, num_classes]` (raw logits, no softmax)
    pub fn forward(&self, images: Tensor<B, 3>) -> Tensor<B, 2> {
        let [batch_size, height, width] = images.dims();

        // Add the channel dimension: MNIST is single-channel.
        let x = images.reshape([batch_size, 1, height, width]);

        let x = self.conv1.forward(x); // [batch, 8, 26, 26]
        let x = self.dropout.forward(x);
        let x = self.conv2.forward(x); // [batch, 16, 24, 24]
        let x = self.dropout.forward(x);
        let x = self.activation.forward(x);

        let x = self.pool.forward(x); // [batch, 16, 8, 8]
        let x = x.reshape([batch_size, 16 * 8 * 8]);
        let x = self.linear1.forward(x);
        let x = self.dropout.forward(x);
        let x = self.activation.forward(x);

        self.linear2.forward(x) // [batch, num_classes]
    }
}
