# Chapter 06 — The fork

## Abstract

This chapter is the tutorial's climax: the **double-three fork**.
You will learn why a fork is two open threes in **distinct**
directions through the new stone (two overlapping threes on the same
line do not count), how the per-direction channel layout from chapter
5 makes that distinction fall out naturally, and why the acceptance
is a direct comparison against chapter 5's **naive oracle**.

The oracle exists and is trusted (chapter 5). The `threes_per_dir`
channels exist since chapter 3 and are populated into `ThreatMaps`
since chapter 4. This chapter turns the oracle loose on the network:
`tests/three_maps.rs` proves that the network's per-direction maps
match `oracle::open_three_makers`, that `double_threes` matches
`oracle::double_threes`, and that the distinct-direction assumption
holds on a thousand random boards for both colours.

By the end, every empty cell on the board is scored by one clean
question: *does this move fork two open threes?* The four well-placed
stones of a double-three are the beginning of the end.

## Glossary

| Term | Definition |
|------|------------|
| **Double-three fork** | An empty cell where playing creates open threes in two or more distinct directions. |
| **Distinct-direction assumption** | The design guarantee that at most one three-maker fires per direction at a given cell, so summing per-direction maps counts directions, not shapes. |
| **Naive oracle** | A slow, obviously-correct reference implementation used as ground truth in a differential test. This chapter depends on the oracle built in chapter 5 (`src/oracle.rs`). |
| **Differential test** | A test that runs two independent implementations on the same inputs and requires identical outputs; discrepancies localize the bug. |
| **Three-level gate** | The acceptance suite that checks `ThreatMaps::double_threes` against `oracle::double_threes` on seeded random boards. |
| **Anti-four margin** | The cells immediately outside an open-three window's empty ends; an own stone there turns the pattern into a four or five. |
| **Overline rejection** | A move that completes five is a win-map cell; the three-level code must never also label it a three-maker. |

## Context

Chapter 5 defined and demonstrated the open-three oracle: a plain-Rust
enumerator of the three exact windows, with its own ASCII test suite.
This chapter uses that oracle as ground truth.

Chapter 3 produced the four per-direction open-three maker channels.
Chapter 4's `ThreatMaps` glue already derives `double_threes` from
those channels by summing the four maps and thresholding at `>= 2`.
This chapter's job is to *prove* that derivation correct.

The four-level maps from chapter 4 were provable against the engine
itself (`engine::immediate_wins`, `engine::double_threats`). The
engine has no open-three concept, so the three level is verified by
comparing the network against the crate's own oracle. The network and
the oracle are two independent implementations of spec §3.4. If they
disagree, at least one is wrong — and the oracle is small enough that
you can read it line by line and decide which.

The fixtures in this chapter deliberately revisit the semantics you
tested on the oracle in chapter 5 — the same families, the same edge,
opponent-blocked, and anti-four margin cases. That overlap is the
point, not redundancy: chapter 5 pinned the ground truth, this chapter
proves the network reproduces it. `tests/oracle.rs` tests the oracle;
`tests/three_maps.rs` tests the network against the oracle. Until both
exist, the three level is unverified.

The verified reference build confirmed this agreement on 1000 seeded
random boards for both colours: the simple `sum >= 2` rule in the
`ThreatMaps` glue matched the oracle's explicit per-direction count
exactly, cell by cell.

## Intention

1. Create `tests/three_maps.rs`, which compares `ThreatMaps` to the
   chapter-5 oracle on hand-built ASCII boards:
   * exact cell-set comparison per direction vs
     `oracle::open_three_makers`, and
   * `double_three_fork_matches_oracle` vs
     `oracle::double_threes`.
2. Understand and document the `double_threes` derivation already
   present in chapter 4's `ThreatMaps` glue: sum the four
   `threes_per_dir` maps and threshold at `>= 2`. Add the
   distinct-direction assumption comment if it is not already there.
3. Extend `tests/differential.rs` with `double_threes_default` and
   the ignored `double_threes_full` gate.
4. Confirm that the oracle's edge and overline semantics, tested in
   chapter 5, are reproduced by the network without extra code.

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

This chapter proves the last derived map:

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

The verified reference build proved this assumption empirically on
1000 random boards per colour: no cell ever accumulated `2.0` from a
single direction, and the `sum >= 2` rule matched the oracle's
direction count exactly.

### The naive oracle as second implementation

The network is one implementation of spec §3.4. The oracle is
another. They are independent in almost every way:

| Network | Oracle |
|---------|--------|
| 11×11 conv kernels | Plain `for` loops over board windows |
| Declarative pattern table | Three hard-coded boolean windows |
| Type-erased `Device` tensors | Direct `Board` / `MoveSet` engine types |
| Layer-2 1×1 combiner | Per-direction `MoveSet` union |

When two such different pieces of code agree on thousands of random
boards, you have strong evidence that the definition is implemented
correctly. When they disagree, the bug is almost always in the more
complex network — but the oracle gives you a concrete expected set
to debug against.

This is the philosophy of **differential testing**: two independent
implementations of one definition. Chapter 5 introduced the oracle on
its own; this chapter extends the discipline to the three level.

### Edge and overline behavior

The oracle's edge and overline semantics were exercised directly in
chapter 5. Here the point is that the network reproduces them for
free.

The border-as-blocked input planes from chapter 1 pay off again. A
window whose empty end would be off the board sees the border as
`Blocked` (encoded as `−1` on both channels), so the required-empty
condition fails automatically. The oracle mirrors this by treating
off-board cells as `Blocked`. Either way, edge-hugging pseudo-threes
are rejected without special cases.

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
├── src/
│   ├── oracle.rs   (exists from chapter 5)
│   └── net.rs      (no changes needed if chapter 4's ThreatMaps glue is in place)
└── tests/
    ├── three_maps.rs   (new: ASCII verification of threes_per_dir against the oracle)
    └── differential.rs (extend: double_threes_default and double_threes_full)
```

Note: `tests/common/mod.rs` from chapter 4 supplies `seeded_board`
(needed by `tests/differential.rs`). The ASCII fixtures in
`tests/three_maps.rs` are parsed with
`engine::reference::board_from_ascii` — the `testutil` dev-dependency
introduced in chapter 5.

### The double-three combiner in `src/net.rs`

Chapter 4's `ThreatMaps` glue already derives `double_threes` from
the four per-direction three maps. The essence of that computation
is:

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
cell cannot accumulate `2.0` from a single direction. The verified
reference build confirmed this on 1000 random boards per colour. If
you ever find yourself wanting to change that comment, stop and prove
the mutual-exclusion property first — or switch to counting
directions explicitly.

Occupied cells are zeroed before this step, exactly as for the other
maps, because the kernel language does not condition on the candidate
cell `c` itself.

## Solution (opt-in)

The complete reference `tests/three_maps.rs` for this chapter lives in
[06-the-fork/01-solution.md](06-the-fork/01-solution.md). Open it
only if you have been stuck for more than twenty minutes, or after
you have finished the chapter and want to compare. The file is
quoted verbatim from the verified reference build consistent with
main's `patterns` crate.

## TDD checklist

Run these tests from `gomoku/` with `cargo test -p patterns`.

### `tests/three_maps.rs`

The reference file defines 11 named `#[test]` functions and 4
supporting helpers. The exact names are:

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

The file imports `patterns::oracle::{Direction, double_threes, open_three_makers}` for its ground-truth checks. That oracle is chapter 5's module; by this chapter it is already trusted.

### `tests/differential.rs`

* `double_threes_default`
* `double_threes_full` — `#[ignore = "full 1000-board differential gate; run explicitly with -- --ignored"]`

Follow the red-green-refactor rhythm:

1. **Red:** Add `tests/three_maps.rs` with all tests and extend
   `tests/differential.rs` with the two double-three gates. Run
   `cargo test -p patterns`. Expect failures.
2. **Green:** Implement the three-map tests against the chapter-5
   oracle and verify the `double_threes` combiner in `src/net.rs`.
   Run `cargo test -p patterns`. The default tests should pass.
3. **Full proof:** Run `cargo test -p patterns --release -- --ignored`
   to exercise the 1000-board three-level gate.
4. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

All tests from previous chapters continue to pass and are part of
the chapter-6 gate. That includes the win-map tests, the four-level
engine differential gates, and the chapter-5 oracle tests.

## Done when

* `cargo test -p patterns` passes from `gomoku/` (including
  `double_threes_default` and all earlier tests).
* `cargo test -p patterns -- --ignored` passes in release as the full
  proof.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Commit message for the code slice:
  `feat(patterns): the fork — three-level maps proven against the oracle`.

Next: [Chapter 07 — Seeing it](07-seeing-it.md)

## References

* [`05-three-makers.md`](05-three-makers.md) — the previous chapter:
  the open-three definition and the naive oracle.
* [`docs/specs/2026-09-27-convolutions-tutorial-design.md`](../../specs/2026-09-27-convolutions-tutorial-design.md)
  — §3.4 (three-level semantics) and §3.5 (the oracle ladder).
* [`docs/plans/2026-09-27-convolutions-tutorial.md`](../../plans/2026-09-27-convolutions-tutorial.md)
  — Task T8, which this chapter implements.
* `docs/13-engine-design.md` — tactics semantics and the documented
  v1 simplifications that the four-level maps reproduce verbatim.
