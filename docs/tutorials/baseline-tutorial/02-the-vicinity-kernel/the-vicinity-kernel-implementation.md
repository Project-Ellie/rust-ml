# Sub-chapter 02 — The vicinity kernel: implementation plan

This document turns the walkthrough into a construction order. It
names the files, structs, functions, and tests. It states behavioral
contracts in prose. It does not contain bodies.

Read `the-vicinity-kernel.md` and `the-vicinity-kernel-design.md`
first. Work one test at a time. Open `the-vicinity-kernel-solution.md`
only if you are stuck for more than twenty minutes, or after the
sub-chapter is complete.

---

## Step 0 — `Cargo.toml`: add `patterns` and `burn`

In `gomoku/crates/baseline/Cargo.toml`, add two dependencies under
`[dependencies]`:

* `patterns = { path = "../patterns" }`
* `burn = { workspace = true, features = ["flex"] }`

Keep the existing `engine` and `mcts` path dependencies.

Declare the demo binary:

```toml
[[bin]]
name = "gravitydemo"
path = "src/bin/gravitydemo.rs"
```

## Step 1 — `src/lib.rs`: expose the gravity module

In `gomoku/crates/baseline/src/lib.rs`, add `pub mod gravity;` among
the module declarations.

Update the crate-level doc comment so it no longer claims the
`BaselineEvaluator` is Burn-free or ignores the planes. The crate now
contains a Burn-backed gravity branch.

## Step 2 — `src/gravity.rs`: the gravity module

Create `gomoku/crates/baseline/src/gravity.rs`.

### 2.1 Module documentation

Write a module-level doc comment that explains the purpose: a
color-blind, border-free occupancy plane and a 5×5 vicinity kernel
normalized to [0, 1].

### 2.2 Imports

Bring in:

* `burn::nn::PaddingConfig2d`
* `burn::nn::conv::{Conv2d, Conv2dConfig}`
* `burn::tensor::{Device, Tensor, TensorData}`
* `engine::Planes`
* `patterns::tools::conv_from_template`

### 2.3 `occupancy_plane`

Signature:

```rust
pub fn occupancy_plane(planes: &Planes) -> [[f32; 15]; 15]
```

Behavior:

* Allocate a 15×15 array of `0.0f32`.
* For each board row `r` in `0..15` and column `c` in `0..15`, compute
  the 17×17 index `(r + 1) * 17 + (c + 1)`.
* If `planes.me[dst] == 1` or `planes.you[dst] == 1`, set the output
  cell to `1.0f32`.
* Return the array.

This strips the border ring and folds both colors into one plane.

### 2.4 `GravityKernel`

Public struct:

```rust
pub struct GravityKernel {
    conv: Conv2d,
    device: Device,
}
```

Behavior: owns the initialized 5×5 `Conv2d` layer and the device it
runs on.

### 2.5 `GravityKernel::new`

Signature:

```rust
pub fn new(device: &Device) -> Self
```

Behavior:

* Initialize a `Conv2d` template with `Conv2dConfig::new([1, 1], [5, 5])`,
  `PaddingConfig2d::Same`, and bias enabled. This keeps the output at
  15×15.
* Build the 5×5 weight table:
  * center cell `(2, 2)` = `0.0`;
  * Chebyshev distance 1 ring = `2.0`;
  * Chebyshev distance 2 ring = `1.0`.
* Flatten the table into a `Vec<f32>` in row-major order and build a
  `Tensor<4>` with shape `[1, 1, 5, 5]` (NCHW).
* Build a zero bias tensor of shape `[1]`.
* Use `conv_from_template(weight, bias, template)` to produce the final
  `Conv2d`.
* Store the `Conv2d` and a clone of the device.

Idiom: init-then-overwrite. The template carries padding, stride,
dilation, and groups; the weight and bias carry the learned-looking
but hand-written values.

### 2.6 `GravityKernel::compute`

Signature:

```rust
pub fn compute(&self, planes: &Planes) -> [[f32; 15]; 15]
```

Behavior:

* Call `occupancy_plane(planes)`.
* Flatten the 15×15 occupancy array into a `Vec<f32>` in row-major
  order.
* Build an input `Tensor<4>` with shape `[1, 1, 15, 15]` on `self.device`.
* Run `self.conv.forward(input)`.
* Read the output data into a `Vec<f32>`.
* Reshape the output into a 15×15 array and divide every element by
  `32.0f32`.
* Return the normalized array.

Idiom: exact `f32` equality. Because every weight and every
normalization is a dyadic fraction, no epsilon comparisons are needed.

### 2.7 `gravity_map`

Signature:

```rust
pub fn gravity_map(planes: &Planes) -> [[f32; 15]; 15]
```

Behavior:

* Create a flex device.
* Build a temporary `GravityKernel` on that device.
* Return `kernel.compute(planes)`.

This is a convenience wrapper for tests and the demo. Caching the
kernel for repeated evaluation comes later.

### 2.8 Unit tests

Add a `#[cfg(test)] mod tests` block at the bottom of the file.

Helpers:

* `empty_planes()` — encode an empty board.
* `plane_with_stone_at(r, c, channel)` — encode an empty board, then
  set one cell in `planes.me` (channel `0`) or `planes.you` (channel
  `1`) to `1`. Use the 17×17 index `(r + 1) * 17 + (c + 1)`.
* `chebyshev(r, c, sr, sc)` — compute `max(|r - sr|, |c - sc|)`.

Tests, in red-to-green order:

1. `gravity_empty_board_is_zero`
2. `gravity_centre_stone_rings`
3. `gravity_corner_stone_truncated_never_boosted`
4. `gravity_is_color_blind`
5. `gravity_full_neighborhood_is_one`

### 2.9 Gate

Run `cargo test -p baseline`. Expect failures until each test is
implemented.

## Step 3 — `src/bin/gravitydemo.rs`: the demo binary

Create `gomoku/crates/baseline/src/bin/gravitydemo.rs`.

### 3.1 Documentation

Write a short binary-level doc comment: print the normalized gravity
map for a small opening shape.

### 3.2 Imports

Bring in `baseline::gravity::gravity_map` and `engine::{Board, Move}`.

### 3.3 `main`

Behavior:

* Create a new `Board`.
* Play three moves: `(7, 7)`, `(6, 6)`, `(8, 8)`.
* Encode the board with `engine::encode`.
* Call `gravity_map`.
* Print each row of the map, formatting each value as a fixed-width
  number with two decimal places.

You may print numeric values directly, or use `patterns::tools::channel_as_image`
to render the map as glyphs.

## Step 4 — Project gates

From `gomoku/`:

```text
cargo test -p baseline
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `Cargo.toml` | `patterns` and `burn` dependencies | the crate can use the threat-network tools and the flex backend |
| `Cargo.toml` | `[[bin]] gravitydemo` | the demo is buildable |
| `src/lib.rs` | `pub mod gravity;` | module is public |
| `src/lib.rs` | updated crate doc | docs no longer claim Burn-free / planes-ignored |
| `src/gravity.rs` | `occupancy_plane` | returns 15×15 color-blind, border-free plane |
| `src/gravity.rs` | `GravityKernel` | owns `Conv2d` and device |
| `src/gravity.rs` | `GravityKernel::new` | builds the 5×5 kernel via `conv_from_template` with `Same` padding |
| `src/gravity.rs` | `GravityKernel::compute` | convolves and normalizes by 32 |
| `src/gravity.rs` | `gravity_map` | convenience wrapper on flex device |
| `src/gravity.rs` | five tests | every test from the checklist passes with `==` |
| `src/bin/gravitydemo.rs` | `main` | prints the gravity map for a small opening |
