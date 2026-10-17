# Chapter 05 — Three-makers

## Abstract

This chapter studies the **open-three maker** families that are
already wired into the pattern table, and introduces the **naive
oracle** that defines what "open three" means for the rest of the
crate. You will learn the three exact window patterns (`_XXX_`,
`_X_XX_`, `_XX_X_`), the **anti-four margin** that keeps the
three-level and four-level maps cleanly partitioned, and why the
11×11 kernel footprint from chapter 3 had to be radius 5. Then you
will write `src/oracle.rs` — a deliberately slow, obviously correct
reference implementation in plain Rust — and `tests/oracle.rs`, which
watches it behave on ASCII boards you can read by eye. The oracle is
introduced and tested before it is ever used to judge a conv kernel;
chapter 6 will turn it loose on the network.

## Glossary

| Term | Definition |
|------|------------|
| **Open three** | One of the three exact windows `_XXX_`, `_X_XX_`, `_XX_X_` with both window ends empty and no own stone immediately beyond either end. |
| **Three-maker** | A pattern family whose kernel fires when playing at `c` creates an open three. |
| **Anti-four margin** | The cells immediately outside a three-window's empty ends. They must *not* hold an own stone; otherwise the pattern is a four or five. |
| **Required-empty cell** | A window cell that must be genuinely empty (no stone of either colour, not the border). Encoded as `−1` on both input channels. |
| **Maturation room** | Empty cells beyond the anti-four margins that would let the open three grow into an open four. v1 ignores this. |
| **Per-direction map** | One of four layer-2 channels that counts open-three makers along horizontal, vertical, diagonal-down, or diagonal-up lines. |
| **OR-combine** | Layer-2 wiring that sums nine 0/1 family channels per direction; ReLU turns any positive sum into a marker. |
| **Naive oracle** | A slow, obviously-correct reference implementation used as ground truth in a differential test. This chapter's new artifact is the open-three oracle in `src/oracle.rs`. |

## Context

Chapter 3 ended with the complete 152-channel pattern table: 20
five-completers, 16 open-four makers, 80 broken-four makers, and 36
three-maker channels (3 families × 3 candidate positions × 4
directions). Layer 2 already emits all nine channels: the win map,
four per-direction four-created maps, and four per-direction
open-three maps. Channels 5..=8 are the per-direction open-three maps.

Chapter 4 added the `ThreatMaps` interpretation layer: it reads the
network outputs into named fields and derives `double_threes` from
the per-direction maps.

This chapter does not change `src/kernel.rs` or `src/net.rs`. Its new
artifacts are `src/oracle.rs` and `tests/oracle.rs`. The engine has no
open-three concept, so the tutorial crate carries its own reference:
a plain-Rust enumerator of the three exact windows. Before we trust it
to judge 108 hand-written kernel channels, we demonstrate it on
hand-built boards.

## Intention

1. Read and understand the three-maker base-shape generators in
   `src/kernel.rs`: `three_xxx_base_shapes`, `three_x_xx_base_shapes`,
   `three_xx_x_base_shapes`, and their shared helper
   `make_three_shape`.
2. Connect each base-shape contract to the exact window it
   implements (`_XXX_`, `_X_XX_`, `_XX_X_`) and to the anti-four
   margin rule.
3. Create `src/oracle.rs` with the naive open-three enumerator:
   * `Direction` enum in canonical order (Horizontal, Vertical,
     DiagDown, DiagUp) with `#[repr(usize)]` discriminants matching
     the `[MoveSet; 4]` index order.
   * Internal `CellState` enum (`Empty`, `Own`, `Blocked`).
   * `open_three_makers(b, s) -> [MoveSet; 4]`.
   * `double_threes(b, s) -> MoveSet`.
4. Keep the oracle deliberately naive: plain array scans, no
   bitboards, no convolution. Readability and obvious correctness are
   the whole point.
5. Add `tests/oracle.rs` with ASCII unit tests covering each pattern
   family in each direction, edge cases, anti-four margins, and
   overlines.

Observable done-state: the chapter-5 tests listed below pass,
`cargo clippy --all-targets -- -D warnings` is green from `gomoku/`,
and `cargo fmt --all` makes no changes from `gomoku/`.

## Mental mapping

### The three exact windows

An open three is not a single shape. Along a line there are three
ways to place three own stones and two empty ends so that a further
stone at `c` completes the shape:

| Family | Window | Stone positions | Empty positions | `c` can be at |
|--------|--------|-----------------|-----------------|---------------|
| `ThreeXxx` | `_XXX_` | 1, 2, 3 | 0, 4 | 1, 2, 3 |
| `ThreeXxX` | `_X_XX_` | 1, 3, 4 | 0, 2, 5 | 1, 3, 4 |
| `ThreeXxXRev` | `_XX_X_` | 1, 2, 4 | 0, 3, 5 | 1, 2, 4 |

Positions are indexed inside the window reading left-to-right. `c`
must be one of the stone positions; the other stone positions become
`stone_offsets`, the empty positions become `empty_offsets`.

### Anti-four margins vs. required-empty cells

The cells immediately outside the window ends are **not** required to
be empty. They are only forbidden to hold an own stone. Why? If a
cell just outside the window held an own stone, the pattern would be a
four or a five, which belongs to the four-level maps, not the three-
level maps. The partition stays clean by construction.

This is the difference:

* **Required-empty window cells** (`empty_offsets`) get `−1` on **both**
  input channels. An opponent stone, an own stone, or the materialised
  border all violate emptiness.
* **Anti-four margin cells** (`anti_margin_offsets`) get `−1` on
  **channel 0 only**. An opponent stone or the border is allowed; only
  an own stone is forbidden.

In the bias language from chapter 3, the margin is just another
condition that drops the pre-activation by 1 when violated.

### The radius-5 payoff

Chapter 3 promised that the 11×11 kernel was needed for the three-
makers. Here is why. The widest three window is six cells long
(`_X_XX_` or `_XX_X_`). When `c` is the far stone of that window,
the anti-four margin beyond the far empty end is **five** cells away
from `c`. A kernel with radius 4 would truncate that margin and would
accept open threes that are actually fours. The 11×11 footprint from
chapter 3 is paid off here.

### Per-direction three maps

Each direction has nine three-maker channels: three families × three
candidate positions. Layer 2 adds a 1×1 weight of `1.0` from each of
those nine channels to the direction's output channel. Because every
activation is already 0/1 from ReLU, the sum is positive iff at least
one three-maker fires. ReLU keeps that marker non-negative. The base
shapes are mutually exclusive for a fixed `c` and direction, so the
sum never exceeds 1 in practice, but even if it did the post-processing
threshold in chapter 6 is `>= 2` *directions*, not a per-direction
count.

### The v1 maturation-room simplification

The v1 definition matches the spec, §3.4: the window ends must be
empty and the margins must not hold an own stone. It does **not**
require extra empty cells beyond the margins. In threat-theory terms
this is too permissive: an open three that cannot grow into an open
four because the board edge or an opponent stone sits one cell further
out is not a "real" threat. v1 keeps the kernels small and the
definition local. Extending the kernels with maturation-room
conditions is left as an end-of-chapter exercise.

### Why define the open three in plain Rust first?

The `engine` crate computes wins, forced blocks, and double threats
as bitboard operations. It has no concept of "open three" because the
engine's tactics head start stops one rung higher on the threat ladder.
That leaves the `patterns` crate without an independent authority for
the three level — so we write one.

`src/oracle.rs` is that authority. It is intentionally slow and
obviously correct: it walks every empty cell, aligns the three exact
windows along each of the four directions, checks the required-empty
cells and the anti-four margins, and returns a `MoveSet` per
direction. You can read it line by line and map each line onto the
spec definition.

The oracle is not production code. It exists only so that the fast
conv network can be tested against something you can reason about by
hand. Chapter 6 will compare the network's `threes_per_dir` channels
to `oracle::open_three_makers` and the network's `double_threes` map
to `oracle::double_threes`. If the two implementations disagree, the
oracle gives you a concrete expected set to debug against.

This is the philosophy of **differential testing**: two independent
implementations of one definition. It is the same discipline that made
the chapter-4 engine gate trustworthy, extended to a level the engine
does not compute.

## Low-level design

### Files you will create or modify

```text
gomoku/crates/patterns/
├── src/
│   ├── kernel.rs   (no changes: the three-maker shapes are already complete)
│   ├── net.rs      (no changes: ThreatMaps reads channels 5..=8 from layer 2)
│   └── oracle.rs   (new: the reference enumerator for open threes)
└── tests/
    └── oracle.rs   (new: ASCII unit tests for the oracle)
```

Note: `tests/common/mod.rs` with `board_from_ascii` is available from
chapter 4, and the same 4 generator helper tests from `common` run
whenever you invoke an integration test that includes `mod common;`.

### `PatternFamily` channel counts

Each three family has:

```rust
PatternFamily::ThreeXxx => 3,
PatternFamily::ThreeXxX => 3,
PatternFamily::ThreeXxXRev => 3,
```

so each contributes `3 × 4 = 12` channels. The final table totals:

```text
FiveCompleter:     20
OpenFourMaker:     16
BrokenFourMaker:   80
ThreeXxx:          12
ThreeXxX:          12
ThreeXxXRev:       12
--------------------
LAYER1_NUM_CHANNELS: 152
```

`LAYER2_NUM_CHANNELS` is `9`.

### Base shape contracts

All offsets are relative to `c` along the horizontal direction; the
rotation helper from chapter 3 maps a horizontal offset `(0, p)` to
`(dr * p, dc * p)` for the other directions.

#### `ThreeXxx` — `_XXX_`

```text
window positions :  0   1   2   3   4
contents         :  _   X   X   X   _
```

For each `c_pos` in `1..=3`:

* `stone_offsets`: `{1, 2, 3} \ {c_pos}`, shifted by `shift = -c_pos`.
* `empty_offsets`: `{0, 4}`, shifted by `shift`.
* `anti_margin_offsets`: `shift - 1` and `shift + 5`.
* `completing_offsets`: empty.

#### `ThreeXxX` — `_X_XX_`

```text
window positions :  0   1   2   3   4   5
contents         :  _   X   _   X   X   _
```

For each `c_pos` in `[1, 3, 4]`:

* `stone_offsets`: `{1, 3, 4} \ {c_pos}`, shifted by `shift = -c_pos`.
* `empty_offsets`: `{0, 2, 5}`, shifted by `shift`.
* `anti_margin_offsets`: `shift - 1` and `shift + 6`.
* `completing_offsets`: empty.

#### `ThreeXxXRev` — `_XX_X_`

```text
window positions :  0   1   2   3   4   5
contents         :  _   X   X   _   X   _
```

For each `c_pos` in `[1, 2, 4]`:

* `stone_offsets`: `{1, 2, 4} \ {c_pos}`, shifted by `shift = -c_pos`.
* `empty_offsets`: `{0, 3, 5}`, shifted by `shift`.
* `anti_margin_offsets`: `shift - 1` and `shift + 6`.
* `completing_offsets`: empty.

### Layer-2 wiring for `threes_per_dir`

Output channels 5..=8:

| Output channel | Direction | Input channels |
|---|---|---|
| 5 | Horizontal | all `ThreeXxx`/`ThreeXxX`/`ThreeXxXRev` with `direction == Horizontal` |
| 6 | Vertical | all three families with `direction == Vertical` |
| 7 | DiagDown | all three families with `direction == DiagDown` |
| 8 | DiagUp | all three families with `direction == DiagUp` |

Each selected input channel gets weight `1.0`; bias is `0.0`. ReLU
produces a non-zero marker iff at least one three-maker fires.

### `ThreatMaps::threes_per_dir`

Chapter 4's `net.rs` glue extracts these four channels into
`threes_per_dir[direction][row][col]`. Occupied cells are zeroed by
the glue, exactly as for `wins` and `fours_created_per_dir`. The
oracle does not need this masking because it only inspects empty
cells, but the network does because the kernel language deliberately
ignores what sits at `c`.

### `src/oracle.rs` contracts

The module is public and documented as a reference oracle, not
production code. Its contracts are:

```rust
/// Direction order must match the array index order returned by
/// `open_three_makers`.
#[repr(usize)]
pub enum Direction {
    Horizontal = 0,
    Vertical = 1,
    DiagDown = 2,
    DiagUp = 3,
}

impl Direction {
    pub const ALL: [Direction; 4];
    pub fn step(self) -> (i32, i32);
}

/// Per-direction open-three makers for side `s`.
pub fn open_three_makers(b: &Board, s: Color) -> [MoveSet; 4];

/// Empty cells that create open threes in two or more distinct directions.
pub fn double_threes(b: &Board, s: Color) -> MoveSet;
```

The internal `CellState` is:

```rust
enum CellState {
    Empty,   // genuinely empty in-board cell
    Own,     // side `s` stone (including the hypothetical new stone)
    Blocked, // opponent stone or off-board
}
```

The `open_three_at` helper scans the three windows `_XXX_`,
`_X_XX_`, and `_XX_X_` along one direction, checks that the
candidate cell is inside the window, checks the anti-four margins,
and returns `true` only when the window ends are genuinely empty.
Off-board cells are `Blocked`, so edge-hugging pseudo-threes are
rejected without special cases.

## Solution (opt-in)

The complete reference `src/oracle.rs` for this chapter lives in
[05-three-makers/01-solution.md](05-three-makers/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare. The file is
quoted verbatim from the verified reference build that evolves the
real crate; the three-maker kernels it presumes already exist in your
`src/kernel.rs` from chapter 3.

`tests/oracle.rs` is intentionally **not** quoted. Writing the
demonstration tests from the checklist is the core work of this
chapter.

## TDD checklist

Run these tests from `gomoku/` with `cargo test -p patterns`.

### `tests/oracle.rs`

The reference file defines the following `#[test]` functions. The
exact set verified from the reference is 16 tests (the three vertical
variants come from the Task-1 quality round). Note that
`cargo test -p patterns --test oracle -- --list` reports 20 tests:
the binary also runs the 4 generator helper tests from
`tests/common/mod.rs`; the 16 below are the oracle assertions:

1. `quiet_board_has_no_three_makers`
2. `xxx_pattern_fires_horizontally`
3. `xxx_pattern_fires_vertically`
4. `xxx_pattern_fires_diagonally_down`
5. `x_xx_pattern_fires_horizontally`
6. `x_xx_pattern_fires_vertically`
7. `x_xx_pattern_fires_diagonally_down`
8. `xx_x_pattern_fires_horizontally`
9. `xx_x_pattern_fires_vertically`
10. `xx_x_pattern_fires_diagonally_down`
11. `xxx_does_not_fire_when_end_is_opponent_blocked`
12. `xxx_does_not_fire_when_edge_hugging`
13. `anti_four_margin_rejects_extension_to_four`
14. `overline_present_rejects_adjacent_three_maker`
15. `double_three_fork_fires_in_two_directions`
16. `diagonal_up_direction_is_independent`

Follow the red-green-refactor rhythm:

1. **Red:** Add `src/oracle.rs` with the enum shells and `todo!()`
   function bodies, and add `tests/oracle.rs` with all tests. Run
   `cargo test -p patterns --test oracle`. Expect failures.
2. **Green:** Implement the oracle's window scans. Pay special
   attention to `Direction` order: the discriminants must match the
   array indices returned by `open_three_makers`, because chapter 6
   will index `threes_per_dir[dir as usize]` using these same values.
   Run `cargo test -p patterns --test oracle`. The 16 oracle
   assertions plus the 4 helper tests should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

As in chapter 4, all tests from previous chapters (planes, naive,
spike, kernels, win map, and four-level maps) continue to pass and
are part of the chapter-5 gate.

## End-of-chapter exercise

The v1 kernels accept open threes that have no room to mature. Extend
the three-maker kernels so that the cells just beyond the anti-four
margins are required to be empty in-board cells as well. This means
adding new `empty_offsets` one cell outside each margin (or, if you
prefer, turning the margins themselves into required-empty cells and
adding new anti-four margins one step further out). Describe how the
base-shape offsets would change and which test cases would now be
expected to fail; do not implement the full solution unless you have
time and curiosity.

## Bridge to chapter 6

An open three is a strong threat, but it is answerable: the opponent
can block one of the two empty ends. **Two** open threes in distinct
directions through the same cell are not answerable — that is the
**double-three fork**. The oracle already computes it; chapter 6
proves that the network sees it.

## Done when

* `cargo test -p patterns --test oracle` passes from `gomoku/`.
* `cargo test -p patterns` passes from `gomoku/` (including all
  earlier tests).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Commit message for the code slice:
  `feat(patterns): the open-three oracle + its test suite`.

Next: [Chapter 06 — The fork](06-the-fork.md)

## References

* [`04-the-four-level.md`](04-the-four-level.md) — the previous
  chapter: four-level maps and the `ThreatMaps` glue.
* [`docs/specs/2026-09-27-convolutions-tutorial-design.md`](../../specs/2026-09-27-convolutions-tutorial-design.md)
  — §3.4 (three-level semantics and the maturation simplification).
* [`docs/plans/2026-09-27-convolutions-tutorial.md`](../../plans/2026-09-27-convolutions-tutorial.md)
  — Task T7, which this chapter implements.
