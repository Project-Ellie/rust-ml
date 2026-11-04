//! Thbe gravity kernel lives here.
//!

use burn::Tensor;
use burn::nn::PaddingConfig2d;
use burn::nn::conv::{Conv2d, Conv2dConfig};
use burn::tensor::{Device, TensorData};
use engine::Planes;

/// Produce a color-blind occupancy plane from relative planes.
///
/// Returns 1.0 where either `me` or `you` has a stone on the 15×15
/// board. The 17×17 border ring is stripped.
#[must_use]
pub fn occupancy_plane(planes: &Planes) -> [[f32; 15]; 15] {
    let mut out = [[0.0f32; 15]; 15];
    for (r, row) in out.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            let dst = (r + 1) * engine::EXT + (c + 1);
            if planes.me[dst] == 1 || planes.you[dst] == 1 {
                *cell = 1.0f32;
            }
        }
    }
    out
}

/// A 5x5 gravity conv layer on a given device
pub struct GravityKernel {
    conv: Conv2d,
    device: Device,
}

impl GravityKernel {
    /// Build a new gravity kernel on `device`.
    #[must_use]
    pub fn new(device: &Device) -> Self {
        let template = Conv2dConfig::new([1, 1], [5, 5])
            .with_padding(PaddingConfig2d::Same)
            .with_bias(true)
            .init(device);

        // Centre cell is 0: a kernel describes the neighbourhood, not
        // the candidate cell itself (which is always occupied/illegal).
        // Chebyshev distance 1 ring weighs 2, distance 2 ring weighs 1.
        let flat = vec![
            1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 2.0, 0.0, 2.0, 1.0, 1.0, 2.0,
            2.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
        ];
        let weight = Tensor::<4>::from_data(TensorData::new(flat, [1, 1, 5, 5]), device);

        let bias = Tensor::<1>::from_data(TensorData::new(vec![0.0f32], [1]), device);

        let conv = patterns::tools::conv_from_template(weight, bias, template);

        Self {
            conv,
            device: device.clone(),
        }
    }

    /// Compute the normalized gravity map for `planes`
    #[must_use]
    pub fn compute(&self, planes: &Planes) -> [[f32; 15]; 15] {
        let occupancy = occupancy_plane(planes);
        let mut data = Vec::with_capacity(15 * 15);
        for row in &occupancy {
            for &value in row {
                data.push(value);
            }
        }

        let input = Tensor::<4>::from_data(TensorData::new(data, [1, 1, 15, 15]), &self.device);

        let out = self.conv.forward(input);
        let values: Vec<f32> = out.to_data().iter::<f32>().collect();

        // Wow! This is an exceptional beauty. The compiler infers the ranges for r, c from the
        // return type of this method.
        std::array::from_fn(|r| std::array::from_fn(|c| values[r * 15 + c] / 32.0))
    }
}

/// Compute the normalized gravity map for `planes` on the flex device.
///
/// This is a convenience wrapper that builds a temporary kernel.
#[must_use]
pub fn gravity_map(planes: &Planes) -> [[f32; 15]; 15] {
    let device = Device::flex();
    let kernel = GravityKernel::new(&device);
    kernel.compute(planes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Board, Color, Move};

    fn empty_planes() -> Planes {
        engine::encode(&Board::new())
    }

    fn planes_with_stone_at(r: u8, c: u8, channel: u8) -> Planes {
        let mut planes = empty_planes();
        let dst = (r as usize + 1) * engine::EXT + (c as usize + 1);
        if channel == 0 {
            planes.me[dst] = 1;
        } else {
            planes.you[dst] = 1;
        }
        planes
    }

    fn chebyshev(r: usize, c: usize, sr: usize, sc: usize) -> usize {
        r.abs_diff(sr).max(c.abs_diff(sc))
    }

    #[test]
    fn gravity_empty_board_is_zero() {
        let planes = empty_planes();
        let map = gravity_map(&planes);
        assert!(map.iter().all(|row| row.iter().all(|&v| v == 0.0f32)));
    }

    #[test]
    fn gravity_centre_stone_rings() {
        let planes = planes_with_stone_at(7, 7, 0);
        let map = gravity_map(&planes);
        for (r, row) in map.iter().enumerate() {
            for (c, &value) in row.iter().enumerate() {
                let dist = chebyshev(r, c, 7, 7);
                let expected = match dist {
                    0 => 0.0f32,
                    1 => 2.0f32 / 32.0f32,
                    2 => 1.0f32 / 32.0f32,
                    _ => 0.0f32,
                };
                assert_eq!(value, expected, "at ({r}, {c}) with dist {dist}.");
            }
        }
    }

    #[test]
    fn gravity_corner_stone_truncated_never_boosted() {
        let planes = planes_with_stone_at(0, 0, 0);
        let map = gravity_map(&planes);

        let mut expected = [[0.0f32; 15]; 15];
        expected[0][1] = 2.0f32 / 32.0f32;
        expected[1][0] = 2.0f32 / 32.0f32;
        expected[1][1] = 2.0f32 / 32.0f32;
        expected[0][2] = 1.0f32 / 32.0f32;
        expected[1][2] = 1.0f32 / 32.0f32;
        expected[2][0] = 1.0f32 / 32.0f32;
        expected[2][1] = 1.0f32 / 32.0f32;
        expected[2][2] = 1.0f32 / 32.0f32;

        for (r, row) in map.iter().enumerate() {
            for (c, &value) in row.iter().enumerate() {
                assert_eq!(value, expected[r][c], "at ({r}, {c})");
            }
        }

        let corner_sum: f32 = map.iter().flatten().sum();
        assert!(
            corner_sum < 1.0f32,
            "corner map sum {corner_sum} must be < 1.0"
        );
        assert!(map.iter().flatten().all(|&v| v <= 2.0f32 / 32.0f32));
    }

    #[test]
    fn gravity_is_color_blind() {
        let black = planes_with_stone_at(7, 7, 0);
        let white = planes_with_stone_at(7, 7, 1);
        assert_eq!(gravity_map(&black), gravity_map(&white));
    }

    #[test]
    fn gravity_full_neighborhood_is_one() {
        // Fill every cell with Chebyshev distance 1 or 2 from (7,7).
        let mut black = Vec::new();
        let mut white = Vec::new();
        let mut toggle = true;
        for dr in -2i32..=2 {
            for dc in -2i32..=2 {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let r = (7i32 + dr) as u8;
                let c = (7i32 + dc) as u8;
                let mv = Move::new(r, c).unwrap();
                if toggle {
                    black.push(mv);
                } else {
                    white.push(mv);
                }
                toggle = !toggle;
            }
        }
        let board = Board::from_position(&black, &white, Color::Black).unwrap();
        let planes = engine::encode(&board);
        let map = gravity_map(&planes);
        assert_eq!(map[7][7], 1.0f32);
    }
}
