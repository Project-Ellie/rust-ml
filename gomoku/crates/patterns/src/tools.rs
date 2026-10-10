//!
//! Tools for developing gomoku heuristics

use burn::Tensor;
use burn::module::Param;
use burn::nn::PaddingConfig2d;
use burn::nn::conv::{Conv2d, Conv2dConfig};
use burn::prelude::Backend;
use burn::tensor::TensorData;

/// Assembles C→O channel `Conv2d` detector layers from one flat kernel per
/// (output, input) channel pair, hiding the manual `Param` construction and
/// the init-then-overwrite struct plumbing.
///
/// The intended use is independent per-color pattern detection: output
/// channel 0 carries the black detector (stripe kernel on the black plane,
/// zeros on the white plane), output channel 1 the white detector. Each
/// output channel gets its own bias, since the two detections must not
/// share a threshold offset.
///
/// The kernel spatial shape `[kh, kw]` and the padding are fixed at builder
/// creation time (`Valid` unless overridden with `with_padding`); `build`
/// checks that each channel's kernel has exactly `kh * kw` entries.
pub struct DetectorBuilder {
    kernel_size: [usize; 2],
    padding: PaddingConfig2d,
}

impl DetectorBuilder {
    pub fn new(kernel_size: [usize; 2]) -> Self {
        Self {
            kernel_size,
            padding: PaddingConfig2d::Valid,
        }
    }

    /// Override the padding. `Explicit(2, 2, 2, 2)` keeps a 15x15 board at
    /// 15x15 with a 5x5 kernel; zero padding means "no stones beyond the
    /// edge".
    pub fn with_padding(mut self, padding: PaddingConfig2d) -> Self {
        self.padding = padding;
        self
    }

    /// Build a `Conv2d<B>` with weight shape `[O, C, kh, kw]` from one
    /// row-major flat kernel per (output, input) channel pair, and one bias
    /// per output channel.
    ///
    /// # Panics
    /// Panics if a channel kernel does not have exactly `kh * kw` entries.
    pub fn build<B: Backend, const O: usize, const C: usize, const MN: usize>(
        &self,
        weights: [[[f32; MN]; C]; O],
        bias: [f32; O],
        device: &B::Device,
    ) -> Conv2d<B> {
        let [kh, kw] = self.kernel_size;
        assert_eq!(MN, kh * kw, "kernel size must be {kh}x{kw}");

        let template: Conv2d<B> = Conv2dConfig::new([C, O], self.kernel_size)
            .with_padding(self.padding.clone())
            .with_bias(true)
            .init(device);

        let weight: Tensor<B, 4> = Tensor::from_data(
            TensorData::new(weights.concat().concat(), [O, C, kh, kw]),
            device,
        );

        let bias = Tensor::from_data(TensorData::new(bias.to_vec(), [O]), device);

        Conv2d {
            weight: Param::from_tensor(weight),
            bias: Some(Param::from_tensor(bias)),
            stride: template.stride,
            kernel_size: template.kernel_size,
            dilation: template.dilation,
            groups: template.groups,
            padding: template.padding,
        }
    }
}

/// Render a single channel marking cells > 0.0 with 'x'
/// # Parameter `values`: Row-major representation of a 15x15 field
pub fn channel_as_image(values: &[f32]) -> String {
    let image_vec = &mut [[" . "; 15]; 15];
    for (i, v) in values.iter().enumerate() {
        if *v > 0.0 {
            image_vec[i / 15][i % 15] = " # ";
        }
    }
    image_vec
        .iter()
        .map(|r| r.concat())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render the two channels of a tensor with two channels
/// This is tailor-made for tensors representing valid boards.
/// Means there are only 1s and 0s and no position has ones in both channels
pub fn board_tensor_as_image<B: Backend>(tensor: Tensor<B, 4>) -> String {
    let image_vec = &mut [[" . "; 15]; 15];
    // 2 channels, each with 15 rows, each with 15 cols
    for (i, v) in tensor.to_data().iter::<f32>().enumerate() {
        if v != 1.0 {
            continue;
        }
        let channel = i / 225;
        let row = (i % 225) / 15;
        let col = i % 15;
        image_vec[row][col] = if channel == 0 { " x " } else { " o " }
    }
    image_vec
        .iter()
        .map(|r| r.concat())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Read a human-readable string representation of a gomoku position into a tensor
pub fn image_as_tensor<B: Backend>(image: &str, device: &B::Device) -> Tensor<B, 4> {
    let field = &mut [[[0u8; 15]; 15]; 2];
    let rows = image.split("\n").filter(|r| !r.trim().is_empty());
    for (i, r) in rows.enumerate() {
        let stones = r.split_ascii_whitespace();
        for (j, c) in stones.enumerate() {
            match c {
                "x" => field[0][i][j] = 1,
                "o" => field[1][i][j] = 1,
                _ => {}
            }
        }
    }

    let data: Vec<f32> = field
        .iter()
        .flat_map(|channel| channel.iter().flat_map(|row| row.iter().map(|&v| v as f32)))
        .collect();

    Tensor::from_data(TensorData::new(data, [1, 2, 15, 15]), device)
}

#[cfg(test)]
#[cfg(feature = "gpu")]
mod tests {
    use super::*;
    const BOARD: &str = "
     .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
     .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
     .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
     .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
     .  .  .  .  .  o  .  o  .  .  .  .  .  .  .
     .  .  .  .  x  .  x  o  .  .  .  .  .  .  .
     .  .  .  .  x  x  o  x  .  .  .  .  .  .  .
     .  .  .  .  .  o  o  x  .  .  .  .  .  .  .
     .  .  .  .  o  .  .  x  o  .  .  .  .  .  .
     .  .  .  .  o  .  .  x  .  .  .  .  .  .  .
     .  .  .  .  o  .  .  x  .  .  .  .  .  .  .
     .  .  .  .  o  .  .  .  .  .  .  .  .  .  .
     .  .  .  .  o  .  .  .  .  .  .  .  .  .  .
     .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
     .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    ";

    #[test]
    fn vertical_five_detection_on_wgpu() {
        use burn::backend::Wgpu;
        use burn::tensor::activation;

        let device = Default::default();

        let builder =
            DetectorBuilder::new([5, 5]).with_padding(PaddingConfig2d::Explicit(2, 2, 2, 2));

        // the flattening 5x5->25 is for illustrative purposes.
        let vertical: [f32; 25] = [[0.0, 0.0, 1.0, 0.0, 0.0]; 5].concat().try_into().unwrap();
        let zeros: [f32; 25] = [[0.0; 5]; 5].concat().try_into().unwrap();
        let layer: Conv2d<Wgpu> = builder.build(
            [[vertical, zeros], [zeros, vertical]],
            [-4.0, -4.0],
            &device,
        );

        let input: Tensor<Wgpu, 4> = image_as_tensor(BOARD, &device);
        let out: Tensor<Wgpu, 4> = activation::relu(layer.forward(input));
        assert_eq!(out.dims(), [1, 2, 15, 15]);

        let detected: Vec<f32> = out.to_data().to_vec().unwrap();
        let mut expected: Vec<f32> = vec![0.0f32; 2 * 15 * 15];
        expected[8 * 15 + 7] = 1.0;
        expected[225 + 10 * 15 + 4] = 1.0;
        assert_eq!(detected, expected);

        println!("{}", channel_as_image(&detected[..15 * 15]));
        println!();
        println!("{}", channel_as_image(&detected[15 * 15..]));
    }
}
