//! Microbenchmark: which primitive makes wgpu training slow?
//!
//! Scratch diagnostic for the 2026-11-11 nettrain throughput
//! investigation. Not part of the crate's test surface.
//!
//! Run: cargo run --release -p net --features gpu --example layerbench

use std::time::Instant;

use burn::module::Module;
use burn::nn::conv::Conv2dConfig;
use burn::nn::norm::BatchNormConfig;
use burn::nn::{PaddingConfig2d, Relu};
use burn::tensor::{Device, Tensor};
use net::network::{Model, ModelConfig};

const EXT: usize = 17;

/// Time `f` with warmup; `f` must return a scalar readback so the GPU
/// queue is fully drained at the end of every iteration (mirrors
/// nettrain's per-step loss readback).
fn time_op(label: &str, mut f: impl FnMut() -> f32) {
    // Warmup: kernel compilation, allocator growth.
    for _ in 0..2 {
        f();
    }
    // One timed iteration to calibrate the count.
    let t0 = Instant::now();
    f();
    let one = t0.elapsed().as_secs_f64();
    let iters = ((3.0 / one).ceil() as usize).clamp(2, 60);

    let t0 = Instant::now();
    for _ in 0..iters {
        f();
    }
    let total = t0.elapsed().as_secs_f64();
    let per = total / iters as f64;
    println!("{label:<44} {per:>10.4} s/op   ({iters} iters)");
}

fn sync_scalar(t: Tensor<1>) -> f32 {
    t.into_scalar()
}

fn bench_matmul(device: &Device, tag: &str) {
    let a = Tensor::<2>::ones([4096, 4096], device);
    let b = Tensor::<2>::ones([4096, 4096], device);
    time_op(
        &format!("{tag} matmul 4096^3 (expect many TFLOP/s)"),
        || {
            let c = a.clone().matmul(b.clone());
            // 2*4096^3 flops; read one scalar to sync.
            sync_scalar(c.sum().reshape([1]))
        },
    );
}

/// GEMM with the exact shape of one implicit-GEMM 3x3 conv:
/// M = batch*17*17, K = 128*9, N = 128.
fn bench_gemm_conv_shape(device: &Device, tag: &str, batch: usize) {
    let m = batch * EXT * EXT;
    let a = Tensor::<2>::ones([m, 128 * 9], device);
    let b = Tensor::<2>::ones([128 * 9, 128], device);
    time_op(&format!("{tag} gemm conv-shape b={batch}"), || {
        sync_scalar(a.clone().matmul(b.clone()).sum().reshape([1]))
    });
}

fn bench_conv3x3(device: &Device, tag: &str, batch: usize) {
    let conv = Conv2dConfig::new([128, 128], [3, 3])
        .with_padding(PaddingConfig2d::Same)
        .with_bias(false)
        .init(device);
    let conv_inf = conv.clone().valid();
    let x = Tensor::<4>::ones([batch, 128, EXT, EXT], device);
    time_op(&format!("{tag} conv3x3 128->128 fwd   b={batch}"), || {
        let xi = x.clone().valid();
        sync_scalar(conv_inf.forward(xi).sum().reshape([1]))
    });
    time_op(&format!("{tag} conv3x3 128->128 fwd+bwd b={batch}"), || {
        let out = conv.forward(x.clone());
        let loss = out.sum();
        let _ = loss.clone().backward();
        sync_scalar(loss.reshape([1]))
    });
}

fn bench_conv1x1(device: &Device, tag: &str, batch: usize) {
    let conv = Conv2dConfig::new([128, 2], [1, 1])
        .with_bias(false)
        .init(device);
    let conv_inf = conv.clone().valid();
    let x = Tensor::<4>::ones([batch, 128, EXT, EXT], device);
    time_op(&format!("{tag} conv1x1 128->2 fwd   b={batch}"), || {
        let xi = x.clone().valid();
        sync_scalar(conv_inf.forward(xi).sum().reshape([1]))
    });
    time_op(&format!("{tag} conv1x1 128->2 fwd+bwd b={batch}"), || {
        let out = conv.forward(x.clone());
        let loss = out.sum();
        let _ = loss.clone().backward();
        sync_scalar(loss.reshape([1]))
    });
}

fn bench_batchnorm(device: &Device, tag: &str, batch: usize) {
    let bn = BatchNormConfig::new(128).init(device);
    let bn_inf = bn.clone().valid();
    let x = Tensor::<4>::ones([batch, 128, EXT, EXT], device);
    time_op(&format!("{tag} batchnorm fwd        b={batch}"), || {
        let xi = x.clone().valid();
        sync_scalar(bn_inf.forward(xi).sum().reshape([1]))
    });
    time_op(&format!("{tag} batchnorm fwd+bwd    b={batch}"), || {
        let out = bn.forward(x.clone());
        let loss = out.sum();
        let _ = loss.clone().backward();
        sync_scalar(loss.reshape([1]))
    });
}

fn bench_resblock(device: &Device, tag: &str, batch: usize) {
    let conv1 = Conv2dConfig::new([128, 128], [3, 3])
        .with_padding(PaddingConfig2d::Same)
        .with_bias(false)
        .init(device);
    let bn1 = BatchNormConfig::new(128).init(device);
    let conv2 = Conv2dConfig::new([128, 128], [3, 3])
        .with_padding(PaddingConfig2d::Same)
        .with_bias(false)
        .init(device);
    let bn2 = BatchNormConfig::new(128).init(device);
    let relu = Relu::new();
    let x = Tensor::<4>::ones([batch, 128, EXT, EXT], device);
    time_op(&format!("{tag} resblock fwd+bwd     b={batch}"), || {
        let r = x.clone();
        let h = relu.forward(bn1.forward(conv1.forward(x.clone())));
        let h = bn2.forward(conv2.forward(h));
        let out = relu.forward(h + r);
        let loss = out.sum();
        let _ = loss.clone().backward();
        sync_scalar(loss.reshape([1]))
    });
}

fn bench_model(device: &Device, tag: &str, batch: usize) {
    let model: Model = ModelConfig::new().init(device);
    let model_inf = model.clone().valid();
    let x = Tensor::<4>::ones([batch, 4, EXT, EXT], device);
    time_op(&format!("{tag} FULL MODEL fwd       b={batch}"), || {
        let xi = x.clone().valid();
        let out = model_inf.forward(xi);
        sync_scalar(out.policy.sum().reshape([1]) + out.value.sum().reshape([1]))
    });
    time_op(&format!("{tag} FULL MODEL fwd+bwd   b={batch}"), || {
        let out = model.forward(x.clone());
        let loss = out.policy.sum() + out.value.sum();
        let _ = loss.clone().backward();
        sync_scalar(loss.reshape([1]))
    });
}

fn run_device(tag: &str, device: Device) {
    println!("\n=== device: {tag} ===");
    bench_matmul(&device, tag);
    for &b in &[128usize, 1024] {
        bench_gemm_conv_shape(&device, tag, b);
    }
    for &b in &[128usize, 512, 1024, 2048] {
        bench_conv3x3(&device, tag, b);
    }
    bench_conv1x1(&device, tag, 1024);
    bench_batchnorm(&device, tag, 1024);
    bench_resblock(&device, tag, 1024);
    for &b in &[128usize, 1024] {
        bench_model(&device, tag, b);
    }
}

fn main() {
    let only = std::env::args().nth(1);
    match only.as_deref() {
        Some("flex") => run_device("flex", Device::flex().autodiff()),
        Some("metal") => run_device("metal", Device::metal(Default::default()).autodiff()),
        Some("wgpu") => run_device("wgpu", Device::wgpu(Default::default()).autodiff()),
        _ => {
            run_device("wgpu", Device::wgpu(Default::default()).autodiff());
            run_device("flex", Device::flex().autodiff());
        }
    }
}
