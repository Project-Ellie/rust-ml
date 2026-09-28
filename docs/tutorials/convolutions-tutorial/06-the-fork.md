# Chapter 06 — The fork

## Abstract

This chapter is the tutorial's climax: the **double-three fork**.
You will learn why a fork is two open threes in **distinct**
directions through the new stone (two overlapping threes on the same
line do not count), how the per-direction channel layout from chapter
5 makes that distinction fall out naturally, and why the acceptance
is a **naive oracle** — a second, deliberately simple implementation
of the same spec. You will also see how the border-as-blocked trick
from chapter 1 rejects edge-hugging "threes" for free, and why an
overline is a win, never a three.

By the end, every empty cell on the board is scored by one clean
question: *does this move fork two open threes?* The four well-placed
stones of a double-three are the beginning of the end.

## Glossary

| Term | Definition |
|------|------------|
| **Double-three fork** | An empty cell where playing creates open threes in two or more distinct directions. |
| **Distinct-direction assumption** | The design guarantee that at most one three-maker fires per direction at a given cell, so summing per-direction maps counts directions, not shapes. |
| **Naive oracle** | A slow, obviously-correct reference implementation used as ground truth in a differential test. |
| **Differential test** | A test that runs two independent implementations on the same inputs and requires identical outputs; discrepancies localize the bug. |
| **Three-level gate** | The acceptance suite that checks `ThreatMaps::double_threes` against `oracle::double_threes` on seeded random boards. |
| **Anti-four margin** | The cells immediately outside an open-three window's empty ends; an own stone there turns the pattern into a four or five. |
| **Overline rejection** | A move that completes five is a win-map cell; the three-level code must never also label it a three-maker. |
| **Carried-over gate** | A test that was already named in an earlier chapter but becomes runnable only now; it remains part of this chapter's gate. |

## Context

Chapter 5 ended with layer 2 emitting four per-direction open-three
maker channels. This chapter adds the tiny cross-channel step that
turns those four maps into the `double_threes` map, and it adds the
naive oracle that defines what "correct" means for the three level.

The four-level maps from chapter 4 were provable against the engine
itself (`engine::immediate_wins`, `engine::double_threats`). The
engine has no open-three concept, so the tutorial crate carries its
own reference: `src/oracle.rs`. The oracle and the conv network are
two independent implementations of spec §3.4. If they disagree, at
least one is wrong — and the oracle is small enough that you can
read it line by line and decide which.

## Intention

1. Create `src/oracle.rs` with the naive open-three enumerator:
   * `Direction` enum in canonical order (Horizontal, Vertical,
     DiagDown, DiagUp).
   * `CellState` enum (`Empty`, `Own`, `Blocked`).
   * `open_three_makers(b, s) -> [MoveSet; 4]`.
   * `double_threes(b, s) -> MoveSet`.
2. Keep the oracle deliberately naive: plain array scans, no
   bitboards, no convolution. Readability and obvious correctness are
   the whole point.
3. In `src/net.rs`, derive `double_threes` from the four
   `threes_per_dir` maps by summing per-direction values and
   thresholding at `>= 2`. Add the distinct-direction assumption
   comment.
4. Add `tests/oracle.rs` with ASCII unit tests covering each pattern
   family in each direction, edge cases, anti-four margins, and
   overlines.
5. Extend `tests/differential.rs` with `double_threes_default` and
   the ignored `double_threes_full` gate.
6. Confirm the carried-over `double_three_fork_matches_oracle` test
   in `net::tests` passes.

Observable done-state: the chapter-6 tests listed below pass,
`cargo clippy --all-targets -- -D warnings` is green from `gomoku/`,
and `cargo fmt --all` makes no changes from `gomoku/`.

## Mental mapping

### The map: every empty cell asks one question

At this point in the tutorial the network already produces:

| Map | Meaning |
|-----|---------|
| `wins` | Playing here completes five immediately. |
| `double_threats` | Playing here leaves two or more immediate wins. |
| `threes_per_dir[4]` | Playing here creates an open three along this direction. |

This chapter adds the last derived map:

| Map | Meaning |
|-----|---------|
| `double_threes` | Playing here creates open threes in two or more **distinct** directions. |

That is the classic fork. Four stones, placed so that the fifth
splits into two separate open-three lines, is one of the decisive
moments in a Gomoku game. The map simply marks the intersection.

### Why distinct directions matter

Two open threes on the **same** line are not a fork. They share the
same future five-window and the opponent can block both with a single
stone. The definition therefore counts **directions**, not raw
three-maker activations.

The channel layout makes this easy. Layer 2 gives us one map per
direction. Because the pattern table is constructed so that a fixed
cell and direction can match at most one three-maker family, adding
the four per-direction maps and thresholding at `>= 2` counts
directions by construction. That is the **distinct-direction
assumption**.

### The naive oracle as second implementation

The network is one implementation of spec §3.4. The oracle is
another. They are independent in almost every way:

| Network | Oracle |
|---------|--------|
| 9×9 conv kernels | Plain `for` loops over board windows |
| Declarative pattern table | Three hard-coded boolean windows |
| Backend-generic tensors | Direct `Board` / `MoveSet` engine types |
| Layer-2 1×1 combiner | Per-direction `MoveSet` union |

When two such different pieces of code agree on thousands of random
boards, you have strong evidence that the definition is implemented
correctly. When they disagree, the bug is almost always in the more
complex network — but the oracle gives you a concrete expected set
to debug against.

This is the philosophy of **differential testing**: two independent
implementations of one definition. It is the same discipline that
made the chapter-4 engine gate trustworthy, extended to a level the
engine does not compute.

### Edge and overline behavior

The border-as-blocked input planes from chapter 1 pay off again. A
window whose empty end would be off the board sees the border as
`Blocked`, so the required-empty condition fails automatically. The
oracle mirrors this: a cell outside the 15×15 grid is `Blocked`.
Either way, edge-hugging pseudo-threes are rejected without special
cases.

An **overline** (five or more in a line) is a win. If four stones
already exist and the candidate cell completes the line, the win map
fires and the move ends the game. The three-level code must not also
label that cell a three-maker. In the oracle this falls out of the
window logic: there is no empty end, so no open-three window matches.
In the network, the five-completer kernels and three-maker kernels
are disjoint by construction.

## Low-level design

### Files you will create or modify

```text
gomoku/crates/patterns/
└── src/
    ├── oracle.rs   (new: the reference enumerator)
    └── net.rs      (derive double_threes from threes_per_dir)
```

### `src/oracle.rs` contracts

The module is public and documented as a reference oracle, not
production code. Its contracts are:

```rust
/// Direction order must match the array index order returned by
/// `open_three_makers`.
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

### The double-three combiner in `src/net.rs`

After extracting the four per-direction three maps from the layer-2
output, the glue computes:

```rust
let mut three_sum = 0.0f32;
for dir_map in &threes_per_dir {
    three_sum += dir_map[r][c];
}
// The pattern table never fires two three-makers for the same
// cell in the same direction, so the sum counts distinct directions.
if three_sum >= 2.0 {
    double_threes[r][c] = 1.0;
}
```

The comment is load-bearing. It records why a simple sum is enough:
the layer-1 kernels are mutually exclusive within a direction, so a
cell cannot accumulate `2.0` from a single direction. If you ever
find yourself wanting to change that comment, stop and prove the
mutual-exclusion property first — or switch to counting directions
explicitly.

Occupied cells are zeroed before this step, exactly as for the other
maps, because the kernel language does not condition on the candidate
cell `c` itself.

## Solution (opt-in)

The complete reference `src/oracle.rs` for this chapter lives in
[06-the-fork/01-solution.md](06-the-fork/01-solution.md). Open it
only if you have been stuck for more than twenty minutes, or after
you have finished the chapter and want to compare. The file is
quoted verbatim from the verified reference crate and then verified
by extract-and-diff.

## TDD checklist

Run these tests from `gomoku/` with `cargo test -p patterns`.

### `tests/oracle.rs`

The reference file defines the following `#[test]` functions. The
exact set verified from the reference is 16 tests (the three vertical
variants come from the Task-1 quality round):

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

### `tests/differential.rs`

* `double_threes_default`
* `double_threes_full` — `#[ignore = "full 1000-board differential gate; run explicitly with -- --ignored"]`

### `net::tests`

* `double_three_fork_matches_oracle` — this was named in chapter 5's
  checklist as a carried-over test; it becomes fully runnable once
  the oracle exists.

### Carried-over note

All tests from previous chapters continue to pass and are part of
the chapter-6 gate. That includes the win-map tests, the four-level
engine differential gates, and the chapter-5 three-maker ASCII tests.

Follow the red-green-refactor rhythm:

1. **Red:** Add `src/oracle.rs` with the enum shells and `todo!()`
   function bodies, wire `double_threes` in `src/net.rs`, and add all
   tests above. Run `cargo test -p patterns`. Expect failures.
2. **Green:** Implement the oracle's window scans, make sure the
   `Direction` order matches the array indices, and fill in the
   `double_threes` combiner. Run `cargo test -p patterns`. The
   default tests should pass.
3. **Full proof:** Run `cargo test -p patterns --release -- --ignored`
   to exercise the 1000-board three-level gate.
4. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p patterns` passes from `gomoku/` (including
  `double_threes_default` and all earlier tests).
* `cargo test -p patterns -- --ignored` passes in release as the full
  proof.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Commit message for the code slice:
  `feat(patterns): the fork — double-three maps + oracle gate`.

Next: [Chapter 07 — Seeing it](07-seeing-it.md)

## References

* [`05-three-makers.md`](05-three-makers.md) — the previous chapter:
  three-maker kernels and per-direction maps.
* [`docs/specs/2026-09-27-convolutions-tutorial-design.md`](../../specs/2026-09-27-convolutions-tutorial-design.md)
  — §3.4 (three-level semantics) and §3.5 (the oracle ladder).
* [`docs/plans/2026-09-27-convolutions-tutorial.md`](../../plans/2026-09-27-convolutions-tutorial.md)
  — Task T8, which this chapter implements.
* `docs/13-engine-design.md` — tactics semantics and the documented
  v1 simplifications that the four-level maps reproduce verbatim.
