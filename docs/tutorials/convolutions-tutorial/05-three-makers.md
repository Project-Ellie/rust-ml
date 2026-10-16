# Chapter 05 — Three-makers

## Abstract

This chapter studies the **open-three maker** families that are
already wired into the pattern table, and adds the verification file
that proves them correct. You will learn the three exact window
patterns (`_XXX_`, `_X_XX_`, `_XX_X_`), the **anti-four margin**
that keeps the three-level and four-level maps cleanly partitioned,
and why the 11×11 kernel footprint from chapter 3 had to be radius 5.
The layer-2 1×1 combiner already emits four per-direction channels
that mark every empty cell where playing creates an open three. The
acceptance is a direct ASCII comparison: the network's
`threes_per_dir` channels must match the same-window logic of the
naive oracle on hand-built boards.

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

## Context

Chapter 3 ended with the complete 152-channel pattern table: 20
five-completers, 16 open-four makers, 80 broken-four makers, and 36
three-maker channels (3 families × 3 candidate positions × 4
directions). Layer 2 already emits all nine channels: the win map,
four per-direction four-created maps, and four per-direction
open-three maps. Channels 5..=8 are the per-direction open-three maps.

Chapter 4 added the `ThreatMaps` interpretation layer: it reads the
network outputs into named fields and derives `double_threes` from
the per-direction maps. This chapter does not change `src/kernel.rs`
or `src/net.rs`; its new artifact is `tests/three_maps.rs`, which
proves that the three-maker channels mean exactly what the spec
says.

## Intention

1. Read and understand the three-maker base-shape generators in
   `src/kernel.rs`: `three_xxx_base_shapes`, `three_x_xx_base_shapes`,
   `three_xx_x_base_shapes`, and their shared helper
   `make_three_shape`.
2. Connect each base-shape contract to the exact window it
   implements (`_XXX_`, `_X_XX_`, `_XX_X_`) and to the anti-four
   margin rule.
3. Verify `ThreatMaps::threes_per_dir` — populated by chapter 4's
   `analyze` glue — against direct window checks on ASCII boards in
   `tests/three_maps.rs`.

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

## Low-level design

### Files you will create or modify

```text
gomoku/crates/patterns/
├── src/
│   ├── kernel.rs   (no changes: the three-maker shapes are already complete)
│   └── net.rs      (no changes: ThreatMaps reads channels 5..=8 from layer 2)
└── tests/
    └── three_maps.rs   (new: ASCII verification of threes_per_dir)
```

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
the glue, exactly as for `wins` and `fours_created_per_dir`.

## Solution (opt-in)

The complete reference `tests/three_maps.rs` for this chapter lives in
[05-three-makers/01-solution.md](05-three-makers/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare. The file is
quoted verbatim from the verified reference build that evolves the
real crate; the three-maker kernels it tests already exist in your
`src/kernel.rs` from chapter 3.

Note: the file imports `patterns::oracle` for ground-truth
comparisons. That oracle is the subject of chapter 6, so if you are
working strictly in order you can either inline equivalent window
checks now or run these tests after the oracle exists.

## TDD checklist

Run these tests from `gomoku/` with `cargo test -p patterns`.

The reference `tests/three_maps.rs` file defines 11 named `#[test]`
functions and 4 supporting helpers. The exact names are:

Tests:

1. `quiet_board_has_empty_three_maps`
2. `xxx_pattern_matches_oracle_horizontally`
3. `xxx_pattern_matches_oracle_diagonally_down`
4. `x_xx_pattern_matches_oracle_horizontally`
5. `x_xx_pattern_matches_oracle_diagonally_down`
6. `xx_x_pattern_matches_oracle_horizontally`
7. `xx_x_pattern_matches_oracle_diagonally_down`
8. `edge_hugging_three_does_not_fire`
9. `opponent_blocked_end_does_not_fire`
10. `anti_four_margin_rejects_three_maker`
11. `double_three_fork_matches_oracle`

Supporting helpers (not `#[test]`):

1. `analyze_maps`
2. `assert_threes_match`
3. `threes_set`
4. `double_three_set`

The reference file imports `patterns::oracle::{Direction, double_threes, open_three_makers}` for its ground-truth checks. Chapter 6 builds that oracle. If you are working strictly in chapter order, you can either inline the same window checks here or defer running these tests until the oracle exists; the test names and board semantics are the same either way.

As in chapter 4, all tests from previous chapters (planes, naive,
spike, kernels, win map, four-level maps, and the default differential
gates) continue to pass and are part of the chapter-5 gate.

Follow the red-green-refactor rhythm:

1. **Red:** Add `tests/three_maps.rs` with all tests. Run
   `cargo test -p patterns`. Expect failures because the oracle import
   does not exist yet (or, if you inlined the window checks, because
   the tests themselves fail before the glue is correct).
2. **Green:** Once chapter 4's `ThreatMaps` glue and chapter 6's
   `oracle` are in place, run `cargo test -p patterns`. The three-map
   tests should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

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

## Done when

* `cargo test -p patterns` passes from `gomoku/` (including the
  chapter-5 tests and all earlier tests).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Commit message for the code slice:
  `test(patterns): three-maker maps verified against the window oracle`.

Next: [Chapter 06 — The fork](06-the-fork.md)

## References

* [`04-the-four-level.md`](04-the-four-level.md) — the previous
  chapter: four-level maps and the `ThreatMaps` glue.
* [`docs/specs/2026-09-27-convolutions-tutorial-design.md`](../../specs/2026-09-27-convolutions-tutorial-design.md)
  — §3.4 (three-level semantics and the maturation simplification).
* [`docs/plans/2026-09-27-convolutions-tutorial.md`](../../plans/2026-09-27-convolutions-tutorial.md)
  — Task T7, which this chapter implements.
