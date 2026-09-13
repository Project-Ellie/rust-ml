//! Chapter 3 — Autodiff and optimizers.
//!
//! Run: `cargo run --example 03_autodiff`
//!
//! Companion doc: `docs/04-autodiff-and-optimizers.md`.
//!
//! Two parts:
//! 1. A minimal scalar example: differentiate y = x^2 at x = 3 by hand.
//! 2. A full manual training loop: fit y = 2x + 1 with a `Linear` module,
//!    explicit `backward()`, and an SGD optimizer step — no `Learner`
//!    involved. This is what the framework does for you in chapter 5.

use burn::backend::{Autodiff, Flex};
use burn::nn::{Linear, LinearConfig};
use burn::optim::{GradientsParams, Optimizer, SgdConfig};
use burn::tensor::Tensor;

// `Autodiff<B>` is a backend *decorator*: it wraps any backend and records
// the operations on its float tensors into a tape for reverse-mode
// differentiation. The wrapped backend is reachable as `B::InnerBackend`.
type B = Autodiff<Flex>;

fn main() {
    let device = Default::default();

    // --- Part 1: one derivative, explicitly ------------------------------------

    // Leaf variables must opt in to gradient tracking with `require_grad()`.
    let x = Tensor::<B, 1>::from_floats([3.0], &device).require_grad();

    // y = x^2  =>  dy/dx = 2x = 6 at x = 3
    let y = x.clone().powi_scalar(2);

    // `backward()` runs reverse-mode autodiff from a scalar (or from a
    // weighted sum, for non-scalar outputs) and returns the gradient map.
    let grads = y.backward();

    // Gradients are retrieved per-tensor; they live on the *inner* backend.
    let dy_dx = x.grad(&grads).unwrap();
    println!("dy/dx at x=3     = {dy_dx} (expected [6])");

    // --- Part 2: manual training loop -------------------------------------------

    // Fit a linear model to y = 2x + 1. One Linear layer, MSE loss, SGD.
    let mut model: Linear<B> = LinearConfig::new(1, 1).init(&device);
    let mut optim = SgdConfig::new().init();

    // Training data: 64 samples of (x, 2x+1).
    let xs: Vec<f32> = (0..64).map(|i| i as f32 / 16.0 - 2.0).collect();
    let x_train = Tensor::<B, 2>::from_data(
        burn::tensor::TensorData::new(xs.clone(), [64, 1]),
        &device,
    );
    let y_train = x_train.clone().mul_scalar(2.0).add_scalar(1.0);

    let lr = 0.05;
    for epoch in 0..200 {
        let output = model.forward(x_train.clone());
        let loss = (output - y_train.clone()).powi_scalar(2).mean();

        // Zero-cost in a real framework: gradients, map to module params, step.
        let grads = loss.backward();
        let grads = GradientsParams::from_grads(grads, &model);
        model = optim.step(lr, model, grads);

        if epoch % 50 == 0 || epoch == 199 {
            let loss_value: f32 = loss.into_scalar();
            println!("epoch {epoch:>3}  mse = {loss_value:.6}");
        }
    }

    // The weight should be ~2 and the bias ~1.
    let weight = model.weight.val().into_scalar();
    let bias = model.bias.unwrap().val().into_scalar();
    println!("learned: y = {weight:.3} * x + {bias:.3}");

    // Note what just happened:
    // - `loss.backward()` gives raw gradients keyed by tensor id
    // - `GradientsParams::from_grads` ties them to the module's parameters
    // - `optim.step` consumes the module and returns an updated one — Burn
    //   modules are immutable value types; "updating" means replacing.
    // The `Learner`/`SupervisedTraining` machinery automates exactly this loop.
}
