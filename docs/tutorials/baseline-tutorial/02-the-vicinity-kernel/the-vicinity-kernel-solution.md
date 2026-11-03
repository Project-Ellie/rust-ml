> **Opt-in solution — read only after you have tried the chapter, or
> when you have been stuck for more than twenty minutes.**
>
> This file quotes the reference implementation from commit `62e73df`.
> Use it to compare your work, not to replace it.

Verified green from `gomoku/`: `cargo test -p baseline`,
`cargo clippy --all-targets -- -D warnings`,
`cargo fmt --all -- --check`.

The commit adds five files: `Cargo.toml`, `src/lib.rs`,
`src/gravity.rs`, `src/bin/gravitydemo.rs`, and updates
`Cargo.lock`.

---

## `Cargo.toml` — add `patterns` and `burn`

The crate now depends on the threat-network tools and the flex backend.

`gomoku/crates/baseline/Cargo.toml`, lines 1–16:

```toml
[package]
name = "baseline"
description = "Hand-written policy/value evaluator for the Gomoku baseline player."
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
engine = { path = "../engine" }
mcts = { path = "../mcts" }
patterns = { path = "../patterns" }
burn = { workspace = true, features = ["flex"] }

[[bin]]
name = "gravitydemo"
path = "src/bin/gravitydemo.rs"
```

## `src/lib.rs` — expose the gravity module

`gomoku/crates/baseline/src/lib.rs`, lines 1–9:

```rust
//! Hand-written policy/value evaluator for the Gomoku baseline player.
//!
//! This crate implements the [`mcts::Evaluator`] seam with a
//! deterministic, Burn-free (v0) policy and a flat value. The board
//! planes are ignored in this sub-chapter.

#![deny(missing_docs)]

pub mod gravity;
```

Note: the crate-level doc comment is still the sub-chapter-01 text.
It now says "Burn-free" and "planes are ignored", which is no longer
true. Update it when you reach a natural stopping point.

## `patterns::tools::conv_from_template` — the helper reused here

The `patterns` crate already solved the problem of rebuilding a
`Conv2d` from explicit tensors while preserving the template's padding
and stride.

`gomoku/crates/patterns/src/tools.rs`, lines 76–88:

```rust
/// Rebuild a `Conv2d` from explicit weight and bias tensors while preserving
/// the stride, dilation, groups and padding of an initialized template.
pub fn conv_from_template(weight: Tensor<4>, bias: Tensor<1>, template: Conv2d) -> Conv2d {
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
```

## `src/gravity.rs` — the occupancy plane

The first function strips the border ring and folds both colors into
one plane.

`gomoku/crates/baseline/src/gravity.rs`, lines 12–28:

```rust
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
```

## `src/gravity.rs` — the kernel struct and constructor

The constructor initializes a template `Conv2d` with `Same` padding,
then overwrites the weight and bias with the hand-written gravity
table.

`gomoku/crates/baseline/src/gravity.rs`, lines 30–63:

```rust
/// A 5×5 gravity conv layer on a given device.
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
        let kernel: [[f32; 5]; 5] = [
            [1.0, 1.0, 1.0, 1.0, 1.0],
            [1.0, 2.0, 2.0, 2.0, 1.0],
            [1.0, 2.0, 0.0, 2.0, 1.0],
            [1.0, 2.0, 2.0, 2.0, 1.0],
            [1.0, 1.0, 1.0, 1.0, 1.0],
        ];
        let flat: Vec<f32> = kernel.iter().flat_map(|row| row.iter().copied()).collect();
        let weight = Tensor::<4>::from_data(TensorData::new(flat, [1, 1, 5, 5]), device);
        let bias = Tensor::<1>::from_data(TensorData::new(vec![0.0f32], [1]), device);
        let conv = patterns::tools::conv_from_template(weight, bias, template);
        Self {
            conv,
            device: device.clone(),
        }
    }
```

## `src/gravity.rs` — compute the normalized map

`compute` builds a tensor from the occupancy plane, runs the conv,
reads the result back, and divides by 32.

`gomoku/crates/baseline/src/gravity.rs`, lines 65–86:

```rust
    /// Compute the normalized gravity map for `planes`.
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
        let mut map = [[0.0f32; 15]; 15];
        for r in 0..15 {
            for c in 0..15 {
                map[r][c] = values[r * 15 + c] / 32.0f32;
            }
        }
        map
    }
}
```
```

## `src/gravity.rs` — the convenience wrapper

`gravity_map` lets tests and the demo call the kernel without
managing a device.

`gomoku/crates/baseline/src/gravity.rs`, lines 88–96:

```rust
/// Compute the normalized gravity map for `planes` on the flex device.
///
/// This is a convenience wrapper that builds a temporary kernel.
#[must_use]
pub fn gravity_map(planes: &Planes) -> [[f32; 15]; 15] {
    let device = Device::flex();
    let kernel = GravityKernel::new(&device);
    kernel.compute(planes)
}
```

## `src/gravity.rs` — the test module

The helpers build `Planes` values for the tests without going through
the full `Evaluator` path. The five tests assert with `==` because
every value is a dyadic fraction.

`gomoku/crates/baseline/src/gravity.rs`, lines 98–210:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Board, Color, Move};

    fn empty_planes() -> Planes {
        engine::encode(&Board::new())
    }

    fn plane_with_stone_at(r: u8, c: u8, channel: u8) -> Planes {
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
        let planes = plane_with_stone_at(7, 7, 0);
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
                assert_eq!(value, expected, "at ({r}, {c}) with dist {dist}");
            }
        }
    }

    #[test]
    fn gravity_corner_stone_truncated_never_boosted() {
        let planes = plane_with_stone_at(0, 0, 0);
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
        let black = plane_with_stone_at(7, 7, 0);
        let white = plane_with_stone_at(7, 7, 1);
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
```

## `src/bin/gravitydemo.rs` — the demo binary

The demo builds a small opening and prints the gravity map as numbers.

`gomoku/crates/baseline/src/bin/gravitydemo.rs`, lines 1–21:

```rust
//! Print the normalized gravity map for a small opening shape.

use baseline::gravity::gravity_map;
use engine::{Board, Move};

fn main() {
    let mut board = Board::new();
    board.play(Move::new(7, 7).unwrap()).unwrap();
    board.play(Move::new(6, 6).unwrap()).unwrap();
    board.play(Move::new(8, 8).unwrap()).unwrap();

    let planes = engine::encode(&board);
    let map = gravity_map(&planes);

    for row in &map {
        for &value in row {
            print!("{:>5.2} ", value);
        }
        println!();
    }
}
```

Run it with `cargo run -p baseline --bin gravitydemo`.
