# Sub-chapter 02 — The vicinity kernel

## Abstract

This sub-chapter builds the first non-uniform signal of the baseline
player: a **vicinity kernel** that turns "play near stones" into a
number for every empty cell. You build a color-blind occupancy plane
and convolve it with a 5×5 kernel whose weights decay by Chebyshev
distance. The result is a 15×15 gravity map in [0, 1]. The evaluator
still plays uniform moves; here you only *look* at the map.

The emotional center is the **border-pull bug**. Reusing the `you`
plane would drag the kernel toward the board edges because the 17×17
`you` channel carries a ring of ones. This chapter shows the bug as
arithmetic on a tiny grid, then builds a dedicated occupancy plane and
uses a corner-stone regression test to make sure it never returns.

## Glossary

| Term | Definition |
|------|------------|
| **Vicinity kernel** | A 5×5 convolution kernel with a zero center, weight 2 on the eight Chebyshev-distance-1 neighbors, and weight 1 on the sixteen distance-2 neighbors. Also called the gravity kernel. |
| **Occupancy plane** | A 15×15 single-channel plane: `1.0` where either color has a stone, `0.0` elsewhere. It has no border ring. |
| **Border ring** | The one-cell frame of ones around the 17×17 `Planes::you` channel. It tells threat detectors that the board edge is a wall. It must not leak into gravity. |
| **Chebyshev distance** | `max(|dr|, |dc|)`. The kernel rings are defined by this distance. |
| **`GravityKernel`** | The `baseline` struct that owns the `Conv2d` layer and the device. |
| **`gravity_map`** | The convenience function that builds a temporary `GravityKernel` on the flex device and returns the normalized map. |
| **`occupancy_plane`** | The function that strips the 17×17 relative planes down to the border-free 15×15 occupancy plane. |
| **`conv_from_template`** | The `patterns::tools` helper that rebuilds a `Conv2d` from explicit weight and bias tensors while keeping the padding, stride, and dilation of an initialized template. |
| **Flex device** | `burn::tensor::Device::flex()`, the pure-Rust CPU backend used by the baseline crate. |
| **Border-pull bug** | The artifact that appears when you reuse `Planes::you` for gravity: edge and corner cells score phantom mass from the all-ones border ring. |

## Context

Sub-chapter 01 built the `baseline` crate and a uniform v0 evaluator.
That evaluator returns the same logit for every legal move and a value
of `0.0`. It proved the seat works, but it has no opinion about *where*
on the board to look.

The `patterns` crate already exists. It provides the threat network
and small tools such as `conv_from_template` and `channel_as_image`.
This sub-chapter adds a new `baseline` module that consumes `patterns`
and `burn` (flex feature) for the first time.

The `engine::encode` function returns `Planes`: two 17×17 `u8` arrays.
`me` holds the side-to-move stones; `you` holds the opponent stones
*and* a one-cell border ring of ones. That ring is correct for threat
detectors but fatal for gravity. This chapter keeps the two uses
apart.

## Intention

The learner sees, with their own eyes, what "play near stones" means
as a number per cell. After this sub-chapter:

1. An empty board gives a gravity map of all zeros.
2. A single stone throws a symmetrical halo of small positive numbers.
3. A corner stone produces a truncated halo, never a boosted one.
4. The map is the same for Black stones and White stones.
5. A fully populated 5×5 neighborhood (minus the center) normalizes to
   exactly `1.0` at the center cell.

The evaluator still plays uniform moves. The map is observed first.
Policy use comes in sub-chapter 03.

## Specification as tests

The tests are ordered along the execution path: empty input → single
stone in the center → single stone in the corner → color swap → full
neighborhood.

### Empty board

* `gravity_empty_board_is_zero` — `gravity_map` on an empty board
  returns a 15×15 array of exactly `0.0`.

### Center stone

* `gravity_centre_stone_rings` — one stone at `(7, 7)` produces:
  * `0.0` at the stone cell and at every cell with Chebyshev distance
    greater than 2;
  * `2.0 / 32.0` on the eight distance-1 cells;
  * `1.0 / 32.0` on the sixteen distance-2 cells.

### Corner stone (the regression net)

* `gravity_corner_stone_truncated_never_boosted` — one stone at `(0, 0)`
  produces only the truncated halo that fits inside the board. The
  expected map is:
  * `(0, 1)`, `(1, 0)`, `(1, 1)` = `2.0 / 32.0`;
  * `(0, 2)`, `(1, 2)`, `(2, 0)`, `(2, 1)`, `(2, 2)` = `1.0 / 32.0`;
  * every other cell = `0.0`.

  The test also asserts that the corner-map sum is strictly less than
  `1.0` and that no cell exceeds `2.0 / 32.0`. This is the net that
  catches the border-pull bug.

### Color blindness

* `gravity_is_color_blind` — a Black stone at `(7, 7)` and a White
  stone at `(7, 7)` produce identical gravity maps.

### Full neighborhood

* `gravity_full_neighborhood_is_one` — every cell with Chebyshev
  distance 1 or 2 from `(7, 7)` is occupied, alternating colors. The
  gravity value at `(7, 7)` is exactly `1.0`, because the raw sum is
  `8·2 + 16·1 = 32` and the normalization divides by `32`.

## Walkthrough

### 1. The border-pull bug, shown on a tiny grid

The 17×17 `you` plane carries a ring of ones. Suppose you reuse it as
the occupancy plane. On a small 3×3 board with a one-cell border ring,
a 3×3 kernel sees the border as extra stones.

Imagine one opponent stone at the center and the border ring all ones:

```text
1 1 1 1 1
1 . . . 1
1 . O . 1
1 . . . 1
1 1 1 1 1
```

For the corner empty cell `(1, 1)`, the kernel window overlaps the
center stone and five border ones. The raw score is `6`. For a central
edge cell, the window overlaps fewer border cells. The corner looks
more attractive than it should. On the real 15×15 board the same thing
happens: cells near the edge score phantom mass from the 64-cell border
ring. That would pull the AI toward the sides.

The fix is to build a dedicated occupancy plane that reads only the
inner 15×15 cells and treats everything outside as `0`.

### 2. Build the occupancy plane

`occupancy_plane(planes: &Planes) -> [[f32; 15]; 15]` walks the inner
15×15 region. For each board cell `(r, c)` it computes the 17×17 index
`(r + 1) * 17 + (c + 1)`. If either `planes.me[dst]` or
`planes.you[dst]` is `1`, the output cell is `1.0`; otherwise it is
`0.0`.

This strips the border ring and makes gravity color-blind in one step.

### 3. Choose the kernel weights

The kernel is 5×5. The center is `0.0` because a kernel describes the
neighborhood, not the candidate cell itself. Occupied cells are illegal
moves anyway, so a non-zero center would be wasted information.

The Chebyshev rings are:

```text
1 1 1 1 1
1 2 2 2 1
1 2 0 2 1
1 2 2 2 1
1 1 1 1 1
```

The maximum raw output is `8·2 + 16·1 = 32`. The readout normalizes by
`32.0`, so every gravity value is a dyadic fraction and exact under
`==`.

### 4. Build the `Conv2d` with `conv_from_template`

The `patterns` crate already knows how to rebuild a `Conv2d` from
explicit tensors while preserving the padding and stride of an
initialized template. You initialize a template with
`Conv2dConfig::new([1, 1], [5, 5])`, `PaddingConfig2d::Same`, and bias
enabled. Then you overwrite the weight and bias with the gravity table.

This is the init-then-overwrite idiom from the convolutions tutorial.

### 5. Normalize by 32

The `Conv2d` output is the raw sum of neighbor weights. Divide every
cell by `32.0` to get the normalized gravity map. Because `32` is a
power of two, every normalized value is exact in `f32`.

### 6. Run the demo

`cargo run -p baseline --bin gravitydemo` builds a small opening
position (a stone at `(7, 7)` and replies on the diagonal), encodes it,
computes the gravity map, and prints the 15×15 heat map as numbers.

Watch the halo. The center is dark because it is occupied. The cells
next to the stones glow. Cells far away stay at `0.00`.

## TDD checklist

Work one test at a time, red to green.

1. `gravity_empty_board_is_zero`
2. `gravity_centre_stone_rings`
3. `gravity_corner_stone_truncated_never_boosted`
4. `gravity_is_color_blind`
5. `gravity_full_neighborhood_is_one`

## Verification commands

From `gomoku/`:

```text
cargo test -p baseline
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Done when

* The five gravity tests pass.
* `cargo clippy --all-targets -- -D warnings` passes.
* `cargo fmt --all -- --check` makes no changes.
* The gravity demo prints a 15×15 numeric heat map.
* The commit message is:
  `feat(baseline): the vicinity kernel — baseline sub-chapter 02`.

## Capstone

Run:

```text
cargo run -p baseline --bin gravitydemo
```

You will see a heat map. The occupied cells are not the brightest
points. The brightest points sit one or two cells away from the stones.
That is what "play near stones" means as a number. In the next
sub-chapter that number becomes a policy logit.

## References

* [`../00-baseline-primer.md`](../00-baseline-primer.md) — §4 the
  gravity idea: the kernel shape, the occupancy plane, and the
  border-pull trap.
* [`../README.md`](../README.md) — the baseline-tutorial chapter map.
* [`../01-the-seat/the-seat.md`](../01-the-seat/the-seat.md) — the
  crate and v0 uniform evaluator this chapter builds on.
* [`../../convolutions-tutorial/03-the-kernel-language.md`](../../convolutions-tutorial/03-the-kernel-language.md)
  — the kernel notation and the init-then-overwrite idiom.
* [`../../convolutions-tutorial/04-the-four-level/01-solution.md`](../../convolutions-tutorial/04-the-four-level/01-solution.md)
  — the normative `ThreatNet` and `analyze_both` that later readouts
  consume.
* `gomoku/crates/engine/src/encode.rs` — the `Planes` struct and the
  border-ring decision.
* `gomoku/crates/patterns/src/tools.rs` — `conv_from_template` and
  `channel_as_image`.
* `gomoku/crates/baseline/src/gravity.rs` — the module built in this
  sub-chapter.
* `gomoku/crates/baseline/src/bin/gravitydemo.rs` — the heat-map demo.
