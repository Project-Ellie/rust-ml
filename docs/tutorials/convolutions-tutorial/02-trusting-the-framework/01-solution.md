# Chapter 02 — Deep-dive solution: trusting the framework

This is the opt-in reference for Chapter 02. Try the chapter for at
least twenty minutes before opening this file; then use it to compare
or to unblock yourself.

`tests/spike.rs` is reference test code: it is not a module the learner
copies unchanged into production, but a differential harness they adapt
into their own test file. The file below was extracted verbatim from
the verified reference crate and a byte-identical copy was confirmed
with `diff -u` against the reference worktree before this file was
written.

## `tests/spike.rs`

```rust
//! Spike A: verify Burn 0.21 convolution idioms against the hand-rolled
//! cross-correlation in `patterns::naive`.
//!
//! All numeric comparisons are exact: inputs and weights are small integers,
//! so the `f32` sums are exact on every backend.

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

/// A backend-generic smoke test: build a tiny conv, assign weights manually,
/// and run a single forward pass.
fn smoke<B: Backend>(device: &B::Device) {
    // 1 input channel, 1 output channel, 2×2 kernel.
    let conv: Conv2d<B> = Conv2dConfig::new([1, 1], [2, 2])
        .with_padding(PaddingConfig2d::Valid)
        .with_bias(true)
        .init(device);

    // Manually overwrite weight and bias.
    // Weight shape: [out_ch, in_ch, kh, kw] = [1, 1, 2, 2].
    let weight_data = TensorData::new(vec![1.0f32, 0.0, 0.0, 1.0], [1, 1, 2, 2]);
    let weight = Tensor::<B, 4>::from_data(weight_data, device);

    let bias_data = TensorData::new(vec![0.0f32], [1]);
    let bias = Tensor::<B, 1>::from_data(bias_data, device);

    let conv = Conv2d {
        weight: Param::from_tensor(weight),
        bias: Some(Param::from_tensor(bias)),
        stride: conv.stride,
        kernel_size: conv.kernel_size,
        dilation: conv.dilation,
        groups: conv.groups,
        padding: conv.padding,
    };

    // Input shape: [batch, channels, height, width] = [1, 1, 4, 4].
    let input = Tensor::<B, 4>::from_data(
        TensorData::new((1..=16).map(|x| x as f32).collect(), [1, 1, 4, 4]),
        device,
    );

    let output = conv.forward(input);
    assert_eq!(output.dims(), [1, 1, 3, 3]);

    // Compare the actual values against the hand-rolled reference.
    let output_flat: Vec<f32> = output.to_data().iter::<f32>().collect();

    let input_2d: Vec<Vec<f32>> = (0..4)
        .map(|i| (0..4).map(|j| (i * 4 + j + 1) as f32).collect())
        .collect();
    let kernel_2d = vec![vec![1.0f32, 0.0], vec![0.0, 1.0]];
    let expected = naive::cross_correlate(&input_2d, &kernel_2d, 0.0);
    let expected_flat: Vec<f32> = expected
        .iter()
        .flat_map(|row| row.iter().copied())
        .collect();

    assert_eq!(output_flat, expected_flat);
}

/// Run `naive::cross_correlate` on a single-channel slice and compare to
/// Burn's `Conv2d` output, requiring exact `f32` equality.
fn single_channel_exact<B: Backend>(device: &B::Device, seed: u64) {
    let mut rng = StdRng::seed_from_u64(seed);
    let value_dist = Uniform::new_inclusive(-3i8, 3).unwrap();
    let size_dist = Uniform::new_inclusive(4usize, 8).unwrap();
    let k_dist = Uniform::new_inclusive(2usize, 4).unwrap();

    for _ in 0..20 {
        let h = size_dist.sample(&mut rng);
        let w = size_dist.sample(&mut rng);
        let kh = k_dist.sample(&mut rng).min(h);
        let kw = k_dist.sample(&mut rng).min(w);

        let input: Vec<Vec<f32>> = (0..h)
            .map(|_| (0..w).map(|_| value_dist.sample(&mut rng) as f32).collect())
            .collect();
        let kernel: Vec<Vec<f32>> = (0..kh)
            .map(|_| {
                (0..kw)
                    .map(|_| value_dist.sample(&mut rng) as f32)
                    .collect()
            })
            .collect();
        let bias = value_dist.sample(&mut rng) as f32;

        let expected = naive::cross_correlate(&input, &kernel, bias);

        // Flatten input in NCHW order: [batch=1, channels=1, h, w].
        let input_flat: Vec<f32> = input.iter().flat_map(|row| row.iter().copied()).collect();
        let input_tensor =
            Tensor::<B, 4>::from_data(TensorData::new(input_flat, [1, 1, h, w]), device);

        // Flatten kernel in [out_ch=1, in_ch=1, kh, kw] order.
        let kernel_flat: Vec<f32> = kernel.iter().flat_map(|row| row.iter().copied()).collect();
        let weight =
            Tensor::<B, 4>::from_data(TensorData::new(kernel_flat, [1, 1, kh, kw]), device);
        let bias_tensor = Tensor::<B, 1>::from_data(TensorData::new(vec![bias], [1]), device);

        let conv = Conv2d {
            weight: Param::from_tensor(weight),
            bias: Some(Param::from_tensor(bias_tensor)),
            stride: [1, 1],
            kernel_size: [kh, kw],
            dilation: [1, 1],
            groups: 1,
            padding: PaddingConfig2d::Valid,
        };

        let output = conv.forward(input_tensor);
        let output_data = output.to_data();
        let output_flat: Vec<f32> = output_data.iter::<f32>().collect();

        let expected_flat: Vec<f32> = expected
            .iter()
            .flat_map(|row| row.iter().copied())
            .collect();
        assert_eq!(
            output_flat, expected_flat,
            "single-channel exact mismatch for shape {h}×{w} kernel {kh}×{kw}"
        );
    }
}

/// Lift the single-channel naive cross-correlation to multi-channel.
/// Returns one output map per output channel, matching Burn's output shape.
fn naive_multi_channel(
    input: &[Vec<Vec<f32>>],
    kernels: &[Vec<Vec<Vec<f32>>>],
    bias: &[f32],
) -> Vec<Vec<Vec<f32>>> {
    assert_eq!(kernels.len(), bias.len());
    let out_h = input[0].len() - kernels[0][0].len() + 1;
    let out_w = input[0][0].len() - kernels[0][0][0].len() + 1;

    kernels
        .iter()
        .zip(bias.iter())
        .map(|(kernel, &b)| {
            (0..out_h)
                .map(|i| {
                    (0..out_w)
                        .map(|j| {
                            let mut acc = b;
                            for c in 0..input.len() {
                                for p in 0..kernel[c].len() {
                                    for q in 0..kernel[c][p].len() {
                                        acc += input[c][i + p][j + q] * kernel[c][p][q];
                                    }
                                }
                            }
                            acc
                        })
                        .collect()
                })
                .collect()
        })
        .collect()
}

/// Multi-channel-in, multi-channel-out exact comparison.
fn multi_channel_exact<B: Backend>(device: &B::Device, seed: u64) {
    let mut rng = StdRng::seed_from_u64(seed);
    let value_dist = Uniform::new_inclusive(-3i8, 3).unwrap();
    let size_dist = Uniform::new_inclusive(5usize, 9).unwrap();
    let k_dist = Uniform::new_inclusive(2usize, 4).unwrap();

    for _ in 0..20 {
        let h = size_dist.sample(&mut rng);
        let w = size_dist.sample(&mut rng);
        let kh = k_dist.sample(&mut rng).min(h);
        let kw = k_dist.sample(&mut rng).min(w);
        let in_ch = 2usize;
        let out_ch = 3usize;

        let input: Vec<Vec<Vec<f32>>> = (0..in_ch)
            .map(|_| {
                (0..h)
                    .map(|_| (0..w).map(|_| value_dist.sample(&mut rng) as f32).collect())
                    .collect()
            })
            .collect();

        let kernels: Vec<Vec<Vec<Vec<f32>>>> = (0..out_ch)
            .map(|_| {
                (0..in_ch)
                    .map(|_| {
                        (0..kh)
                            .map(|_| {
                                (0..kw)
                                    .map(|_| value_dist.sample(&mut rng) as f32)
                                    .collect()
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect();
        let bias: Vec<f32> = (0..out_ch)
            .map(|_| value_dist.sample(&mut rng) as f32)
            .collect();

        let expected = naive_multi_channel(&input, &kernels, &bias);

        // Flatten input [batch=1, in_ch, h, w] in channel-major order.
        let input_flat: Vec<f32> = input
            .iter()
            .flat_map(|ch| ch.iter().flat_map(|row| row.iter().copied()))
            .collect();
        let input_tensor =
            Tensor::<B, 4>::from_data(TensorData::new(input_flat, [1, in_ch, h, w]), device);

        // Weight [out_ch, in_ch, kh, kw].
        let weight_flat: Vec<f32> = kernels
            .iter()
            .flat_map(|out| {
                out.iter()
                    .flat_map(|ch| ch.iter().flat_map(|row| row.iter().copied()))
            })
            .collect();
        let weight = Tensor::<B, 4>::from_data(
            TensorData::new(weight_flat, [out_ch, in_ch, kh, kw]),
            device,
        );
        let bias_tensor =
            Tensor::<B, 1>::from_data(TensorData::new(bias.clone(), [out_ch]), device);

        let conv = Conv2d {
            weight: Param::from_tensor(weight),
            bias: Some(Param::from_tensor(bias_tensor)),
            stride: [1, 1],
            kernel_size: [kh, kw],
            dilation: [1, 1],
            groups: 1,
            padding: PaddingConfig2d::Valid,
        };

        let output = conv.forward(input_tensor);
        let output_data = output.to_data();
        let output_flat: Vec<f32> = output_data.iter::<f32>().collect();

        // Expected flatten order: [out_ch, out_h, out_w].
        let expected_flat: Vec<f32> = expected
            .iter()
            .flat_map(|ch| ch.iter().flat_map(|row| row.iter().copied()))
            .collect();
        assert_eq!(
            output_flat, expected_flat,
            "multi-channel exact mismatch for shape {h}×{w} kernel {kh}×{kw} in_ch={in_ch} out_ch={out_ch}"
        );
    }
}

/// Verify that `PaddingConfig2d::Explicit(p, p, p, p)` zero-pads by
/// comparing Burn output to a materialized padded input + valid conv.
fn explicit_padding_zero_pads<B: Backend>(device: &B::Device) {
    let input = Tensor::<B, 4>::from_data(
        TensorData::new(
            vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
            [1, 1, 3, 3],
        ),
        device,
    );

    // 3×3 kernel of ones with no bias.
    let weight = Tensor::<B, 4>::from_data(TensorData::new(vec![1.0f32; 9], [1, 1, 3, 3]), device);

    let conv_valid = Conv2d {
        weight: Param::from_tensor(weight.clone()),
        bias: None,
        stride: [1, 1],
        kernel_size: [3, 3],
        dilation: [1, 1],
        groups: 1,
        padding: PaddingConfig2d::Valid,
    };

    // Burn conv with explicit padding.
    let conv_explicit = Conv2d {
        weight: Param::from_tensor(weight),
        bias: None,
        stride: [1, 1],
        kernel_size: [3, 3],
        dilation: [1, 1],
        groups: 1,
        padding: PaddingConfig2d::Explicit(1, 1, 1, 1),
    };

    // Materialize padding: 5×5 zero-padded version of the 3×3 input.
    let padded = Tensor::<B, 4>::from_data(
        TensorData::new(
            vec![
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 0.0, 0.0, 4.0, 5.0, 6.0, 0.0, 0.0,
                7.0, 8.0, 9.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            ],
            [1, 1, 5, 5],
        ),
        device,
    );

    let out_explicit = conv_explicit.forward(input.clone());
    let out_padded_valid = conv_valid.forward(padded);

    assert_eq!(out_explicit.dims(), [1, 1, 3, 3]);
    let explicit_flat: Vec<f32> = out_explicit.to_data().iter::<f32>().collect();
    let padded_flat: Vec<f32> = out_padded_valid.to_data().iter::<f32>().collect();
    assert_eq!(explicit_flat, padded_flat);
}

#[test]
fn smoke_ndarray() {
    let device = Default::default();
    smoke::<NdArray>(&device);
}

#[test]
fn single_channel_exact_ndarray() {
    let device = Default::default();
    single_channel_exact::<NdArray>(&device, 42);
}

#[test]
fn multi_channel_exact_ndarray() {
    let device = Default::default();
    multi_channel_exact::<NdArray>(&device, 123);
}

#[test]
fn explicit_padding_zero_pads_ndarray() {
    let device = Default::default();
    explicit_padding_zero_pads::<NdArray>(&device);
}

#[cfg(feature = "gpu")]
#[test]
fn smoke_wgpu() {
    use burn::backend::Wgpu;
    let device = Default::default();
    smoke::<Wgpu>(&device);
}

#[cfg(feature = "gpu")]
#[test]
fn single_channel_exact_wgpu() {
    use burn::backend::Wgpu;
    let device = Default::default();
    single_channel_exact::<Wgpu>(&device, 42);
}

#[cfg(feature = "gpu")]
#[test]
fn multi_channel_exact_wgpu() {
    use burn::backend::Wgpu;
    let device = Default::default();
    multi_channel_exact::<Wgpu>(&device, 123);
}

#[cfg(feature = "gpu")]
#[test]
fn explicit_padding_zero_pads_wgpu() {
    use burn::backend::Wgpu;
    let device = Default::default();
    explicit_padding_zero_pads::<Wgpu>(&device);
}

#[cfg(feature = "gpu")]
#[test]
fn cpu_vs_wgpu_exact() {
    use burn::backend::Wgpu;

    type Cpu = NdArray;
    type Gpu = Wgpu;

    let cpu_device = Default::default();
    let gpu_device = Default::default();

    // Fixed small-integer weights and biases.
    let weight_values: Vec<f32> =
        std::iter::repeat_with(|| vec![1.0, 0.0, -1.0, 2.0, -2.0, 0.0, 0.0, 1.0, 1.0])
            .take(6)
            .flatten()
            .collect();

    // Build the CPU conv.
    let conv_cpu: Conv2d<Cpu> = {
        let conv: Conv2d<Cpu> = Conv2dConfig::new([2, 3], [3, 3])
            .with_padding(PaddingConfig2d::Valid)
            .with_bias(true)
            .init(&cpu_device);
        let weight = Tensor::<Cpu, 4>::from_data(
            TensorData::new(weight_values.clone(), [3, 2, 3, 3]),
            &cpu_device,
        );
        let bias =
            Tensor::<Cpu, 1>::from_data(TensorData::new(vec![0.5f32, -0.5, 1.0], [3]), &cpu_device);
        Conv2d {
            weight: Param::from_tensor(weight),
            bias: Some(Param::from_tensor(bias)),
            stride: conv.stride,
            kernel_size: conv.kernel_size,
            dilation: conv.dilation,
            groups: conv.groups,
            padding: conv.padding,
        }
    };

    // Replicate the same architecture on the GPU with identical tensors.
    let conv_gpu: Conv2d<Gpu> = {
        let weight =
            Tensor::<Gpu, 4>::from_data(TensorData::new(weight_values, [3, 2, 3, 3]), &gpu_device);
        let bias =
            Tensor::<Gpu, 1>::from_data(TensorData::new(vec![0.5f32, -0.5, 1.0], [3]), &gpu_device);
        Conv2d {
            weight: Param::from_tensor(weight),
            bias: Some(Param::from_tensor(bias)),
            stride: [1, 1],
            kernel_size: [3, 3],
            dilation: [1, 1],
            groups: 1,
            padding: PaddingConfig2d::Valid,
        }
    };

    let input_values: Vec<f32> = (-3i8..=3).cycle().map(|x| x as f32).take(98).collect();
    let input_cpu = Tensor::<Cpu, 4>::from_data(
        TensorData::new(input_values.clone(), [1, 2, 7, 7]),
        &cpu_device,
    );
    let input_gpu =
        Tensor::<Gpu, 4>::from_data(TensorData::new(input_values, [1, 2, 7, 7]), &gpu_device);

    let out_cpu = conv_cpu.forward(input_cpu);
    let out_gpu = conv_gpu.forward(input_gpu);

    assert_eq!(out_cpu.dims(), out_gpu.dims());

    let cpu_flat: Vec<f32> = out_cpu.to_data().iter::<f32>().collect();
    let gpu_flat: Vec<f32> = out_gpu.to_data().iter::<f32>().collect();
    assert_eq!(cpu_flat, gpu_flat);
}
```
