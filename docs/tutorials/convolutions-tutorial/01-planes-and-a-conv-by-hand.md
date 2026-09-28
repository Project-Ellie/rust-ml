# Chapter 01 — Planes and a convolution by hand

## Abstract

This chapter builds the input representation and the first primitive of
the `patterns` crate: a board position is turned into two fixed-size
`f32` planes, and a hand-written nested loop performs a 2-D
**cross-correlation** (the operation deep-learning frameworks call a
convolution) on a single-channel map. By the end you will have a
`Planes` struct with a stones channel and a blocked channel, both padded
so the board edge behaves like a blocked cell, plus a `cross_correlate`
function with valid padding, a bias, and a ReLU wrapper. The
implementation is deliberately framework-free: no tensors yet, just
arrays and loops, so the sliding-multiply-and-add mechanism is visible
before Burn hides it inside `Conv2d`.

This chapter belongs to the convolutions tutorial, a side quest between
the datagen tutorial and the net tutorial. The tutorial's design
authority is the primer in
[`00-convolutions-primer.md`](00-convolutions-primer.md); read it once
before continuing if you have not already.

## Glossary

| Term | Definition |
|------|------------|
| **Plane** | A 2-D `f32` array representing one channel of a board position. |
| **Channel** | One plane of a tensor. The input has two channels: own stones and blocked cells. |
| **Materialized padding** | Extra border cells written explicitly into the array (`1.0` for blocked), rather than relying on a framework's zero-padding. |
| **Cross-correlation** | The sliding weighted-sum operation that deep-learning frameworks call "convolution". The kernel is not flipped before sliding. |
| **Valid padding** | No padding: the output is smaller than the input by one kernel radius on each side. |
| **Kernel** | A small weight matrix slid across the input; at each position it computes a weighted sum. |
| **ReLU** | Rectified linear unit: `max(0, x)`. Negative values become zero. |
| **Threat map** | A feature map with game meaning: each empty cell is scored by what happens if the side places a stone there. |

## Context

The datagen tutorial has just finished. The `train` crate stores
positions as stone lists and the engine still owns the rules oracle. The
net tutorial, which is next, will introduce Burn tensors, a learnable
convolutional network, training, and loss functions all at once. That is
a lot of new machinery to swallow in one step.

This tutorial is the bridge. It builds a tiny, fully hand-written
convolutional network in a new crate called `patterns`. The network's
weights are manufactured, not trained, and its outputs are threat maps:
spatial scores that say, for every empty cell, "what happens if the side
plays here?" The network will eventually detect immediate wins, double
threats, and double-three forks by differential testing against the
engine and a naive oracle. But before any of that, the crate needs a way
to feed a board into a conv layer and a way to understand what the conv
layer actually does. This chapter supplies both.

The crate skeleton and the Burn idiom spike are already in place (see
[`00-convolutions-primer.md`](00-convolutions-primer.md) §6 for the
crate layout). The files you write in this chapter are `src/planes.rs`
and `src/naive.rs`.

## Intention

1. Implement `src/planes.rs`:
   * `pub const PAD: usize = 5;`
   * `pub const PADDED: usize = 25;`
   * `pub struct Planes { pub stones, pub blocked }`, each a
     `[[f32; PADDED]; PADDED]`.
   * `pub fn planes(b: &Board, s: Color) -> Planes` that builds the two
     channels.
2. Implement `src/naive.rs`:
   * `pub fn cross_correlate(input: &[Vec<f32>], kernel: &[Vec<f32>], bias: f32) -> Vec<Vec<f32>>`
     with valid padding.
   * `pub fn relu(map: &[Vec<f32>]) -> Vec<Vec<f32>>`.
   * `pub fn relu_in_place(map: &mut [Vec<f32>])`.
3. Write the unit tests listed in the TDD checklist below.

Observable done-state: `cargo test -p patterns` passes from `gomoku/`,
`cargo clippy --all-targets -- -D warnings` is green from `gomoku/`,
and `cargo fmt --all` makes no changes from `gomoku/`.

## Mental mapping

### Why two channels, and why is the border blocked?

A conv layer looks at a local window and decides whether a pattern is
present. For Gomoku patterns, the decision often depends on "this cell
must be empty". An empty cell is defined negatively: it contains no
stone of either color. If the framework zero-pads the edge, a cell just
off the board looks empty, so an open three hugging the edge would be
accepted incorrectly.

The fix is to materialize the padding. `planes` returns two channels:

* `stones` — `1.0` where the side `s` has a stone, `0.0` elsewhere
  (including the border).
* `blocked` — `1.0` on opponent stones *and* on every border cell,
  `0.0` on empty in-board cells.

Now a kernel can require emptiness by placing a negative weight on the
*sum* of both channels at a cell (each channel has its own kernel
weight at that offset, and a conv layer adds the per-channel weighted
sums — so this means a negative weight on both the stones and the
blocked channel there): if either channel is `1.0`, the cell is not
empty. The border is treated as blocked, not empty, so edge
patterns fall out correctly without any special-case geometry.

The padding radius is `5`, not `4`, because the broken-three patterns
(`_X_XX_`, `_XX_X_`) have an anti-four margin five cells away from the
candidate move. A smaller pad would cut off the kernel's view of that
margin. See the reference comment in `src/planes.rs` for the exact
rationale.

### Why cross-correlation, not "convolution"?

In signal processing, a convolution flips the kernel both horizontally
and vertically before sliding it. Deep-learning frameworks do not flip
the kernel; they slide it as-is. That operation is technically a
cross-correlation. The naming footnote matters only because the
difference is real: if you ever compare your hand-rolled output against
a textbook convolution, you will get the wrong answer unless you flip
the kernel first. Burn's `Conv2d` and PyTorch's `conv2d` follow the
deep-learning convention — no flip — and so does your `cross_correlate`.

### Why write the loop by hand before touching Burn?

Burn's `Conv2d` is a black box until you know what it computes. Writing
the same operation in plain Rust makes the mechanism explicit: for every
output position, multiply overlapping input cells by kernel weights, sum
the products, add a bias. Later, when `Conv2d` produces the same numbers
on random inputs, you will know *why* they match rather than trusting
the framework by faith.

## Low-level design

### Files you will create or modify

```text
gomoku/crates/patterns/
└── src/
    ├── lib.rs   (already exists; declare the new modules)
    ├── planes.rs
    └── naive.rs
```

### `src/planes.rs`

```rust
/// Padding radius around the 15×15 board.
pub const PAD: usize = 5;

/// Padded plane side length: `15 + 2 * PAD`.
pub const PADDED: usize = 25;

/// Two-channel padded input for side `s`.
pub struct Planes {
    /// The side's own stones as a 25×25 `f32` array.
    pub stones: [[f32; PADDED]; PADDED],
    /// Blocked cells (opponent stones + border) as a 25×25 `f32` array.
    pub blocked: [[f32; PADDED]; PADDED],
}

/// Build the two padded input channels for side `s`.
pub fn planes(b: &Board, s: Color) -> Planes;
```

Behavior:

* Initialize `stones` to all `0.0` and `blocked` to all `1.0`.
* For every in-board cell `(r, c)`:
  * If it holds a stone of `s`, set `stones[r+PAD][c+PAD] = 1.0` and
    `blocked[r+PAD][c+PAD] = 0.0`.
  * If it holds an opponent stone, set `stones` to `0.0` and `blocked`
    to `1.0`.
  * If it is empty, set both to `0.0`.

The border ring therefore stays `blocked = 1.0` and `stones = 0.0`.

### `src/naive.rs`

```rust
/// Apply a 2-D cross-correlation with valid padding and a scalar bias.
///
/// The output at `(i, j)` is
///
/// ```text
/// bias + sum_{p,q} input[i + p][j + q] * kernel[p][q]
/// ```
///
/// with `p` in `0..kernel.len()` and `q` in `0..kernel[0].len()`.
///
/// # Returns
/// An output map of shape `(H - kh + 1) × (W - kw + 1)` where `H × W` is
/// the input shape and `kh × kw` is the kernel shape.
///
/// # Panics
/// Panics if any of the following preconditions is violated:
///
/// * `input` is empty, or any input row is empty.
/// * `input` rows have differing lengths (i.e. the map is not rectangular).
/// * `kernel` is empty, or any kernel row is empty.
/// * `kernel` rows have differing lengths.
/// * The input height or width is smaller than the kernel height or width.
pub fn cross_correlate(input: &[Vec<f32>], kernel: &[Vec<f32>], bias: f32) -> Vec<Vec<f32>>;

/// Return a new map with ReLU applied: negative values become zero.
pub fn relu(map: &[Vec<f32>]) -> Vec<Vec<f32>>;

/// Apply the ReLU activation in-place: negative values become zero.
pub fn relu_in_place(map: &mut [Vec<f32>]);
```

Implementation notes:

* Assert the rectangularity preconditions with clear messages; the unit
  tests match the exact panic strings.
* Use valid padding: the output has `in_h - k_h + 1` rows and
  `in_w - k_w + 1` columns.
* Do not flip the kernel. This is cross-correlation, the deep-learning
  convention.

### Module declarations in `src/lib.rs`

The crate root already exists from the Task 0 spike. Add:

```rust
pub mod naive;
pub mod planes;
```

## Solution (opt-in)

The complete reference code for this chapter lives in
[01-planes-and-a-conv-by-hand/01-solution.md](01-planes-and-a-conv-by-hand/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare.

## TDD checklist

Run these tests from `gomoku/` with `cargo test -p patterns`.

The reference suite contains exactly these chapter-1 tests:

From `planes::tests`:

1. `empty_board_has_zero_stones_and_blocked_border_only`
2. `stones_are_placed_for_the_requested_side`
3. `corner_and_edge_stones_are_mapped_correctly`
4. `border_ring_is_exactly_five_cells_wide`

From `naive::tests`:

1. `cross_correlate_4x4_with_2x2`
2. `cross_correlate_with_bias`
3. `cross_correlate_1x1_kernel`
4. `cross_correlate_non_square_kernel`
5. `relu_returns_non_negative`
6. `relu_in_place_mutates`
7. `cross_correlate_kernel_larger_than_input` — `#[should_panic]`
8. `cross_correlate_empty_input` — `#[should_panic]`
9. `cross_correlate_jagged_input` — `#[should_panic]`
10. `cross_correlate_rectangular_non_square_input`

Follow the red-green-refactor rhythm:

1. **Red:** Create `src/planes.rs` and `src/naive.rs` with the structs,
   constants, and function signatures, but implement the functions as
   `todo!()`. Add all the tests above. Run `cargo test -p patterns`.
   Expect failures.
2. **Green:** Implement `planes`, `cross_correlate`, `relu`, and
   `relu_in_place`. Re-run `cargo test -p patterns`. All fourteen tests
   should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p patterns` passes from `gomoku/`.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Commit message: `feat(patterns): planes + hand-rolled cross-correlation`.

Next: [Chapter 02 — Trusting the framework](02-trusting-the-framework.md)

## References

* [`00-convolutions-primer.md`](00-convolutions-primer.md) — the
  tutorial's design authority: threat-ladder semantics, kernel language,
  and crate layout.
* [`docs/specs/2026-09-27-convolutions-tutorial-design.md`](../../specs/2026-09-27-convolutions-tutorial-design.md)
  — §4.1 (input planes) and §4.2 (kernel language).
* [`docs/plans/2026-09-27-convolutions-tutorial.md`](../../plans/2026-09-27-convolutions-tutorial.md)
  — Task T3, which this chapter implements.
* [`docs/tutorials/datagen-tutorial/01-the-train-crate.md`](../datagen-tutorial/01-the-train-crate.md)
  — the most recent tutorial-chapter convention (abstract, glossary,
  contracts, TDD checklist).
