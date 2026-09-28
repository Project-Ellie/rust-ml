# Chapter 03 — The kernel language

## Abstract

This chapter introduces the **exact-pattern kernel language**: a way
to write convolution kernels that evaluate precise Boolean conditions
on a board position. You will learn why a kernel is mostly zeros
("don't care"), why the same pattern must be repeated in every board
direction, and how a small integer weight plus a bias plus ReLU
turns a local geometric test into a clean 0/1 signal. The chapter's
implementation target is the `FiveCompleter` family — the five-cell
windows where placing at the candidate cell `c` completes five in a
row — and the first output of the threat network: the **win map**.

All other pattern families are declared now but implemented in
chapters 4 and 5, so the table's vocabulary is fixed while it grows.

## Glossary

| Term | Definition |
|------|------------|
| **Kernel language** | The exact Boolean encoding used here: `+w` on a required own-stone cell in channel 0, `−w` on a required-empty cell in *both* channels, `0` elsewhere, plus a bias so ReLU emits 0/1. |
| **Pattern family** | A group of related shapes that detect the same kind of threat; `PatternFamily` is the crate enum. |
| **Base shape** | A pattern described along the horizontal direction with the candidate cell `c` at the origin. |
| **Direction variant** | The same base shape rotated to one of the four line directions (horizontal, vertical, diagonal down, diagonal up). |
| **Five-completer** | A pattern where `c` is the last stone of a five-in-a-row window; the win-map detector. |
| **Win map** | A 15×15 map that is positive exactly where placing a stone completes five immediately. |
| **Margin** | The difference between the pre-activation on a perfect match and the pre-activation after one violation. |
| **SPIKE idiom** | The Burn 0.21 manual-weight-assignment pattern: build a `Conv2d` from `Conv2dConfig`, then replace its public `weight` and `bias` fields with `Param::from_tensor(...)` tensors. |
| **Threat map** | A feature map with game meaning: each empty cell is scored by what happens if the side places a stone there. |

## Context

Chapter 1 built the padded input planes and a hand-written
cross-correlation: two channels (own stones and blocked cells), a
border treated as blocked, and a sliding multiply-add loop. Chapter 2
replaced the loop with Burn's `Conv2d`, proved that the two produce
identical numbers on small-integer inputs, and established the manual
weight-assignment idiom.

This chapter turns that machinery into a pattern detector. The goal is
not to train anything; it is to **manufacture** a kernel that fires
exactly when a local stone configuration matches a pattern. The
pattern is written as a small declarative table — "these cells must
hold own stones, those cells must be empty" — and the table is
expanded into 11×11 convolution kernels for every direction. The win
map is the first threat-map output; later chapters will add open/broken
four-makers and open-three makers.

## Intention

1. Implement `src/kernels.rs`:
   * `Direction` enum with the four line directions, canonical order,
     index, and step vector.
   * `PatternFamily` enum listing all six families, but implement only
     `FiveCompleter` in this chapter.
   * `PatternEntry` struct capturing the declarative description of
     one kernel.
   * `five_completer_base_shapes()` and `rotate()`.
   * `set_weight()` with the radius assert.
   * `layer1_conv()` built with the SPIKE idiom.
2. Verify that the FiveCompleter kernels, when OR-combined into the
   win map, agree with the engine on ASCII boards.

Observable done-state: the chapter-3 tests listed below pass,
`cargo clippy --all-targets -- -D warnings` is green from `gomoku/`,
and `cargo fmt --all` makes no changes from `gomoku/`.

## Mental mapping

### Why are the don't-care zeros the lesson?

A trained conv net learns kernels that are dense and hard to read. A
hand-written threat kernel is the opposite: it is a sparse picture of a
pattern. Most of the 11×11 footprint is `0.0` — "I don't care what is
here". Only a few cells carry `+1.0` or `−1.0`. The lesson is that a
convolution can express a local logical condition by ignoring almost
everything and attending to a handful of cells.

### How does +w/−w/bias/ReLU encode "exactly this pattern"?

Use weight `w = 1` for every required condition. Place `+1` on each
required own-stone cell in channel 0. Place `−1` on each
required-empty cell in **both** channels: if either the stones channel
or the blocked channel is `1.0`, the cell is not empty, and the
contribution is `−1`. Set the bias to `1 − n_stones`.

For a perfect match the pre-activation is

```text
bias + n_stones = (1 − n_stones) + n_stones = 1.
```

For every violated condition the pre-activation drops by `1`:

* a missing required stone removes a `+1`;
* an occupied required-empty cell adds a `−1` in one or both channels.

So one violation gives `0`, two give `−1`, and so on. ReLU
(`max(0, x)`) turns `1` into `1` and everything else into `0`. The
margin is exactly `1`, which is enough because all weights are
integers and every sum is exact.

The `FiveCompleter` family has four required stones (the other cells
in the five-window), so `bias = 1 − 4 = −3`. No required-empty cells.

### Why generate kernels from a table instead of drawing them?

Drawing 11×11 weight matrices by hand is error-prone and hard to
review. The artifact under review is the **declarative pattern table**:
a list of which cells must be stones, which must be empty, and which
must not be own stones. The kernels are generated from that table by
a small amount of mechanical code. The table is readable; the weight
matrices are not.

### Why one kernel per direction?

A base shape is described horizontally with `c` at the origin. The four
board directions are related by rotation: the horizontal step vector
`(0, 1)` maps to `(1, 0)` for vertical, `(1, 1)` for diagonal down,
and `(−1, 1)` for diagonal up. So a single base shape generates four
kernel channels. The `FiveCompleter` family has five horizontal base
shapes (one for each position of `c` in the five-window), giving
`5 × 4 = 20` layer-1 channels.

### Why 11×11 kernels?

The largest patterns are the broken-three windows `_X_XX_` and
`_XX_X_`, whose anti-four margins sit **five** cells away from the
candidate move `c`. Those families are chapter 5, but the kernel
footprint must already be large enough to hold them. Therefore the
footprint is `11×11` with radius `5`, and the padded planes from
chapter 1 are `25×25`. If any generated offset is outside radius `5`,
`set_weight` panics during construction — the assert is the guard that
catches a too-small kernel early.

## Low-level design

### Files you will create or modify

```text
gomoku/crates/patterns/
└── src/
    ├── lib.rs   (already registers `pub mod kernels;`)
    ├── kernels.rs
    └── net.rs   (used by the win-map tests; unchanged in this chapter)
```

### `Direction`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction { Horizontal, Vertical, DiagDown, DiagUp }
```

Provide:

* `pub const ALL: [Direction; 4]` in canonical order.
* `pub fn index(self) -> usize` in `0..4`.
* `pub fn step(self) -> (i32, i32)` — the vector for walking one cell.

### `PatternFamily`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatternFamily {
    FiveCompleter,
    OpenFourMaker,
    BrokenFourMaker,
    ThreeXxx,
    ThreeXxX,
    ThreeXxXRev,
}
```

Declare all six variants now so the table's vocabulary is pinned. In
chapter 3 implement only:

* `FiveCompleter::base_shape_count() == 5`.
* `PatternFamily::channels() == base_shape_count() * 4`.

The remaining families return placeholder counts (for example `0`) and
are filled in later.

### `PatternEntry`

```rust
pub struct PatternEntry {
    pub family: PatternFamily,
    pub direction: Direction,
    pub stone_offsets: Vec<(i32, i32)>,
    pub empty_offsets: Vec<(i32, i32)>,
    pub anti_margin_offsets: Vec<(i32, i32)>,
    pub completing_offsets: Vec<(i32, i32)>,
}
```

For `FiveCompleter`:

* `stone_offsets`: the four cells in the five-window other than `c`.
* `empty_offsets`: empty.
* `anti_margin_offsets`: empty.
* `completing_offsets`: `vec![(0, 0)]` — playing at `c` itself completes five.

### Base shapes and rotation

`five_completer_base_shapes()` produces five horizontal entries:
for `k in 0..5`, the window is `[-k, -k+1, -k+2, -k+3, -k+4]` and
`c` is at offset `0`. The four non-zero positions become stone
offsets.

`rotate(entry, dir)` maps each horizontal offset `(0, p)` to
`(dr * p, dc * p)` where `(dr, dc) = dir.step()`.

### Kernel construction helpers

```rust
pub const KERNEL: usize = 11;
pub const RADIUS: i32 = 5;

fn set_weight(
    weight: &mut [f32],
    out_ch: usize,
    in_ch: usize,
    dy: i32,
    dx: i32,
    value: f32,
);
```

`set_weight` asserts `dy.abs() <= RADIUS && dx.abs() <= RADIUS`, then
writes to the flat `[out_ch, in_ch, ky, kx]` index of the Burn weight
buffer.

### Layer 1

```rust
pub fn layer1_conv<B: Backend>(device: &B::Device) -> Conv2d<B>;
```

* Create a `Conv2d` from `Conv2dConfig::new([2, LAYER1_OUT], [KERNEL, KERNEL])`
  with `PaddingConfig2d::Valid` and bias enabled.
* Allocate a flat weight vector of zeros and a bias vector.
* For each table entry set:
  * `+1.0` on channel 0 at every `stone_offset`;
  * `−1.0` on **both** channels at every `empty_offset`;
  * `−1.0` on channel 0 at every `anti_margin_offset`;
  * `bias[out_ch] = 1.0 - stone_offsets.len() as f32`.
* Wrap the weight and bias tensors in `Param::from_tensor(...)` and
  assemble the final `Conv2d` struct, copying the metadata fields from
  the config-built module.

For chapter 3, `LAYER1_OUT` is `PatternFamily::FiveCompleter.channels()
== 20`.

### The win map

The win map is layer 2's output channel 0: a `1×1` convolution that
sums all `FiveCompleter` layer-1 activations with weight `1.0`. Because
each FiveCompleter activation is already 0/1, the sum is positive iff
at least one five-completer pattern fires. ReLU then keeps it as a
positive marker. The win-map tests compare this map against
`engine::immediate_wins` on ASCII boards.

## Solution (opt-in)

The complete reference `src/kernels.rs` for the finished crate lives
in
[03-the-kernel-language/01-solution.md](03-the-kernel-language/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare. The file is
quoted in its final form; families beyond `FiveCompleter` are added in
chapters 4 and 5.

## TDD checklist

Run these tests from `gomoku/` with `cargo test -p patterns`.

The reference suite contains the following chapter-3 tests:

From `kernels::tests`:

1. `every_offset_fits_in_11x11_kernel`

From `net::tests`:

1. `open_four_win_map_matches_engine`
2. `broken_four_win_map_matches_engine`

Follow the red-green-refactor rhythm:

1. **Red:** Create `src/kernels.rs` with the enum/struct signatures,
   `five_completer_base_shapes` returning empty, and `layer1_conv` as
   `todo!()`. Add the three tests above. Run `cargo test -p patterns`.
   Expect failures.
2. **Green:** Implement `Direction`, `PatternFamily` (FiveCompleter
   only), `PatternEntry`, the base-shape generator, rotation,
   `set_weight`, and `layer1_conv`. Run `cargo test -p patterns`. The
   three tests should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

The remaining reference tests exercise families added later:

* `kernels::tests::channel_counts_match_table` and
  `family_counts_are_correct` verify the full 152-channel table and
  belong to chapter 5/6 (once all families are present).
* `net::tests::classic_fork_double_threat`,
  `open_four_maker_is_double_threat`, and
  `two_broken_fours_make_double_threat` test the four-level maps and
  belong to chapter 4.
* `net::tests::double_three_fork_matches_oracle` tests the
  double-three fork and belongs to chapter 6.

## Done when

* `cargo test -p patterns` passes the chapter-3 tests from `gomoku/`.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Commit message for the code slice:
  `feat(patterns): exact-pattern kernels + win map`.

Next: [Chapter 04 — The four level](04-the-four-level.md)

## References

* [`01-planes-and-a-conv-by-hand.md`](01-planes-and-a-conv-by-hand.md)
  — padded planes and the hand-rolled cross-correlation reference.
* [`02-trusting-the-framework.md`](02-trusting-the-framework.md) —
  Burn `Conv2d`, NCHW layout, manual weight assignment, and exact
  differential testing.
* [`docs/specs/2026-09-27-convolutions-tutorial-design.md`](../../specs/2026-09-27-convolutions-tutorial-design.md)
  — §3.1 (win-map semantics) and §4.2 (the exact-pattern kernel
  language).
* [`docs/plans/2026-09-27-convolutions-tutorial.md`](../../plans/2026-09-27-convolutions-tutorial.md)
  — Task T5, which this chapter implements.
* Burn 0.21.0 pinned source — `burn-nn-0.21.0/src/modules/conv/conv2d.rs`
  (struct layout, `Conv2dConfig::init`, NCHW weight shape), cited in
  chapter 2.
* Burn 0.21.0 pinned source — `burn-core-0.21.0/src/module/param/tensor.rs:69`
  for `Param::from_tensor`, cited in chapter 2.
