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
| **SPIKE idiom** | The Burn 0.22 manual-weight-assignment pattern: build a `Conv2d` from `Conv2dConfig`, then replace its public `weight` and `bias` fields with `Param::from_tensor(...)` tensors. |
| **Threat map** | A feature map with game meaning: each empty cell is scored by what happens if the side places a stone there. |

## Notation: pattern strings and family names

Before the chapter body uses them, this section pins down two small
languages: pattern strings like `_X_XX_`, and enum variant names like
`ThreeXxX`. Both are defined here, once, and used everywhere after.

### The pattern-string alphabet

A pattern string is a picture of one straight line of the board — a
row, column, or diagonal — read left-to-right:

| Symbol | Meaning |
|--------|---------|
| `X` | An own stone (a stone of the side the detector serves). |
| `O` | An opponent stone. |
| `_` | A genuinely empty cell: no stone of either colour, and not the border. |
| `c` | The candidate cell: the empty cell whose score the map computes. |

A string like `_X_XX_` is a **window**: the characters at its edges
are part of the pattern, not background. The leading and trailing `_`
are required-empty conditions — the detector must *verify* them, not
assume them. `_X_XX_` therefore reads: "six cells; own stones at
positions 1, 3, 4; positions 0, 2, 5 must be empty."

### The family-name table

`PatternFamily` is a closed set of six variants. This table is their
definition; later chapters repeat it where each family is built.

Every window is drawn **after** the placement being evaluated: `c`
marks the empty cell whose score the map computes, and the `X` at
`c`'s position is the stone that playing there would add. The kernel
therefore puts no weight on `c` itself — it verifies the *other*
stones and the required-empty cells. A family with several listed
shapes has one base shape per `c` position.

| Variant | Window(s), drawn after placing at `c` | Width | Placing at `c` … | Built in |
|---------|---------------------------------------|-------|-------------------|----------|
| `FiveCompleter` | `cXXXX`, `XcXXX`, `XXcXX`, `XXXcX`, `XXXXc` | 5 | … completes five **now** | ch. 3 |
| `OpenFourMaker` | `_cXXX_`, `_XcXX_`, `_XXcX_`, `_XXXc_` | 6 | … creates an open four | ch. 4 |
| `BrokenFourMaker` | 20 shapes: `c` and one required-empty gap anywhere among the other cells of a 5-window of own stones, e.g. `XcX_X` | 5 | … creates a broken four | ch. 4 |
| `ThreeXxx` | `_cXX_`, `_XcX_`, `_XXc_` (after placement: `_XXX_`) | 5 | … completes a straight open three | ch. 5 |
| `ThreeXxX` | `_c_XX_`, `_X_cX_`, `_X_Xc_` (after placement: `_X_XX_`) | 6 | … completes a broken open three | ch. 5 |
| `ThreeXxXRev` | `_cX_X_`, `_Xc_X_`, `_XX_c_` (after placement: `_XX_X_`) | 6 | … completes the mirror broken open three | ch. 5 |

### Why `ThreeXxX` spells `_X_XX_`

The variant names are CamelCase **labels**, not little programs — the
table above is their definition. But there is a mnemonic, and it is
worth stating once so the names stop looking random.

Read the three cells immediately after the window's left end. Write:

* `X` (uppercase) for a stone that **starts a run** — the cell before
  it is empty, and
* `x` (lowercase) for anything else — an empty cell, or a stone
  continuing a run.

Exactly like CamelCase, an uppercase letter marks the beginning of a
word; here a "word" is a run of adjacent stones.

| Window | Cells 1–3 | Runs | Spelling |
|--------|-----------|------|----------|
| `_XXX_` | `X X X` | one run of three | `Xxx` → `ThreeXxx` |
| `_X_XX_` | `X _ X` | two runs (single + pair) | `XxX` → `ThreeXxX` |
| `_XX_X_` | `X X _` | one run of two, then a gap | would *also* spell `Xxx` — taken! |

`_XX_X_` collides with `_XXX_` under this scheme, so it borrows the
name of its mirror image (`_X_XX_` read right-to-left) plus `Rev`:
`ThreeXxXRev`. On an undirected line the two broken-three windows are
the same shape read in opposite directions; they are separate
families only because each generates different kernel positions for
`c`.

If the mnemonic does not stick, ignore it — the names are defined by
the table, not by grammar.

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

## Worked example: from pattern table to win map

This section traces **one** kernel — the vertical FiveCompleter with
`c` in the middle of its window — from a line of stones to a lit cell
on the win map. Every grid below is produced by the code quoted next
to it. When you implement the chapter, this is the mental movie to
replay for every family; only the shapes change, never the mechanics.

### Step 1 — the shape on the line

```
line offset p:   -3  -2  -1   0  +1  +2  +3
                  .   X   X   c   X   X   .
```

`c` is empty; the other four cells of the 5-window hold own stones.
Playing at `c` completes five. In `five_completer_base_shapes()` this
is `k = 2`: the window is `[-2, -1, 0, +1, +2]`, and the four nonzero
positions become stone offsets.

### Step 2 — the PatternEntry

```rust
PatternEntry {
    family: PatternFamily::FiveCompleter,
    direction: Direction::Horizontal,     // base shapes are horizontal
    stone_offsets: vec![(0, -2), (0, -1), (0, 1), (0, 2)],
    empty_offsets: vec![],                // no required-empty cells
    anti_margin_offsets: vec![],
    completing_offsets: vec![(0, 0)],     // playing at c completes five
}
```

Offsets are `(dy, dx)` relative to `c`, which sits at the origin.

### Step 3 — rotation to vertical

`rotate(&base, Direction::Vertical)` uses the step vector `(1, 0)` and
maps each horizontal offset `(0, p)` to `(1·p, 0·p)`:

```
stone_offsets: [(-2, 0), (-1, 0), (1, 0), (2, 0)]

dy = -2      X
dy = -1      X
dy =  0      c      ← the kernel centre
dy = +1      X
dy = +2      X
```

### Step 4 — writing the kernel

The kernel is an 11×11 grid **per input channel**, centred on `c`.
The offset-to-kernel-cell translation is the whole trick:

```
kernel cell (ky, kx) = (RADIUS + dy, RADIUS + dx) = (5 + dy, 5 + dx)
```

`layer1_conv()` writes `+1` at each stone offset in input channel 0
(the own-stones plane) and leaves everything else `0`. Zoomed to the
5×3 region around the centre, input channel 0 looks like this:

```
               kx=4   kx=5   kx=6
ky=3   dy=-2     0     +1      0
ky=4   dy=-1     0     +1      0
ky=5   dy= 0     0      0      0     ← c itself: no weight, anything may be here
ky=6   dy=+1     0     +1      0
ky=7   dy=+2     0     +1      0
```

Every other cell of the 11×11 is `0`, and input channel 1 (the blocked
plane) is entirely `0` because FiveCompleter has no required-empty
cells. The bias is `1 − n_stones = 1 − 4 = −3`. The code that writes
all of this:

```rust
for &(dy, dx) in &entry.stone_offsets {
    set_weight(&mut weight, out_ch, 0, dy, dx, 1.0);
}
bias[out_ch] = 1.0 - entry.stone_offsets.len() as f32;   // 1 − 4 = −3
```

The kernel is a sparse picture of the pattern: four `+1` cells saying
"these must be own stones", zeros everywhere else saying "don't care".

### Step 5 — the sliding dot product

Now the board. Take one column of the own-stones input plane
(`1` = own stone, `0` = empty, opponent, or border):

```
board row:     6   7   8   9  10  11  12
own stones:    0   1   1   0   1   1   0
```

The conv anchors the kernel's centre at every board cell at once —
that is the "parallel" in parallel local pattern detection, and the
25×25 padding with `Valid` padding is what lets an 11×11 kernel centre
itself on edge cells too. With the centre at **row 9**, the five
kernel cells cover rows 7–11:

```
row            7    8    9   10   11
input          1    1    0    1    1
kernel        +1   +1    0   +1   +1
product        1    1    0    1    1
                                 sum = 4
pre-activation = sum + bias = 4 − 3 = +1
ReLU(+1) = 1    →  the channel lights up at row 9
```

One row down (centre at row 10): the `+1` cells now sit at rows
8, 9, 11, 12, whose inputs are `1, 0, 1, 0` — the sum is `2`, the
pre-activation is `2 − 3 = −1`, and ReLU emits `0`. The channel
lights up **only** where `c` completes five.

Why other situations stay dark:

| Situation | sum | pre-act | ReLU |
|-----------|-----|---------|------|
| One of the four required stones missing | 3 | 0 | 0 |
| Opponent stone at a required cell | 3 | 0 | 0 — that cell reads `0` in the own-stones plane, so the `+1` simply does not fire |
| Four own stones, but not at the four required offsets | ≤ 3 | ≤ 0 | 0 — stones elsewhere multiply `0` weights |
| Perfect match | 4 | +1 | 1 |

This is the margin from the mental mapping, made concrete: a perfect
match scores exactly `+1`, one violation scores `0`, and ReLU
separates them. The bias sets how many conditions may fail (zero);
the weights check the conditions.

### Step 6 — twenty channels become the win map

The family has 5 base shapes × 4 directions = 20 layer-1 channels;
each lights up where *its* window completes five. Layer 2 OR-combines
them with a 1×1 convolution: weight `1.0` from every FiveCompleter
channel into output channel 0. At each cell the 1×1 conv sums twenty
0/1 values and ReLU keeps any positive sum:

```
layer 1 at cell (9, 5):   ch 0..6 = 0,  ch 7 = 1,  ch 8..19 = 0
                          (ch 0–4 horizontal, ch 5–9 vertical — the
                          table order is the channel order — so our
                          vertical k = 2 variant is channel 5 + 2 = 7)
layer 2 win map:          ReLU(0 + … + 1 + … + 0) = 1
```

```rust
if matches!(entry.family, PatternFamily::FiveCompleter) {
    set_weight_1x1(&mut weight, 0, in_ch, 1.0);
}
```

The final picture — this is what the chapter's tests compare against
`engine::immediate_wins`:

```
     0 1 2 3 4 5 6 7 8 9
  6  . . . . . . . . . .
  7  . . . . . X . . . .
  8  . . . . . X . . . .
  9  . . . . . W . . . .     ← win map = 1 here, and nowhere else
 10  . . . . . X . . . .
 11  . . . . . X . . . .
 12  . . . . . . . . . .
```

Every other family in the table is the same movie with more
conditions: `OpenFourMaker` adds required-empty cells (`−1` in **both**
input channels, so a stone of either colour violates them), and the
three families add anti-four margins (`−1` in channel 0 only, so an
own stone there is forbidden but an opponent stone is fine). The
grids get busier; the arithmetic never changes.

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
pub fn layer1_conv(device: &Device) -> Conv2d;
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

The crate's actual `src/kernel.rs` — quoted verbatim from the
repository — lives in
[03-the-kernel-language/01-solution.md](03-the-kernel-language/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare. The file is
quoted in its final form: on `main` the chapter-3 slice implemented
all six families at once (not just `FiveCompleter`), so the quoted
file is also what chapters 4–6 build on.

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

Next: [Epilogue — the picture so far](03-epilogue-the-picture-so-far.md),
then [Chapter 04 — The four level](04-the-four-level.md)

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
* Burn 0.22.0 pinned source — `burn-nn-0.22.0/src/modules/conv/conv2d.rs`
  (struct layout, `Conv2dConfig::init`, NCHW weight shape), cited in
  chapter 2.
* Burn 0.22.0 pinned source — `burn-core-0.22.0/src/module/param/tensor.rs:69`
  for `Param::from_tensor`, cited in chapter 2.
