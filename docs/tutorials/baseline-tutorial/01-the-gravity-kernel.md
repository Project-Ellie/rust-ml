# Chapter 01 — The gravity kernel and the occupancy plane

## Abstract

This chapter scaffolds the `baseline` crate and builds its one
genuinely new component: the **gravity kernel**, a 5×5 convolution
that scores every board cell by how much stone mass sits nearby. You
will learn why the quiet board — no threats anywhere, the common case
in the opening — is the case that needs a hand-written default most;
why the kernel's centre cell is exactly zero; why the weights 2 and 1
with a divisor of 32 keep every output exact in f32; and why the
gravity branch must **not** reuse the convolutions tutorial's
two-channel planes (a tiny grid of arithmetic shows the border ring
dragging the opening toward the edges). The chapter ends with a
differential test: the Burn conv branch must equal a plain-Rust
nested-loop reference, exactly, on seeded random boards.

## Glossary

| Term | Definition |
|------|------------|
| **Gravity kernel** | A 5×5 conv kernel, centre 0, weights 2 (distance 1) and 1 (distance 2), slid over the occupancy plane. Scores each cell by nearby stone mass. |
| **Chebyshev distance** | `max(|dy|, |dx|)` — the "king move" distance from the kernel centre. Distance 1 is the 8-neighbour ring, distance 2 the 16-cell ring around it. |
| **Occupancy plane** | A one-channel 19×19 input: 1 where any stone of either color sits, 0 elsewhere, zero-padded by the kernel radius (2). **No border ring.** |
| **Gravity map** | The branch's 15×15 output divided by 32: `g(r, c) ∈ [0, 1]`, high near stones, zero on an empty board. |
| **Dyadic fraction** | A fraction whose denominator is a power of two — exactly representable in binary floating point. 2/32 and 1/32 are dyadic; 0.1 is not. |
| **Exactness principle** | The project convention (convolutions tutorial): hand-written weights and inputs are small integers or dyadic fractions, so f32 sums are exact and tests compare with `==`. |
| **Valid padding** | The conv mode that only positions the kernel fully inside the input: a 5×5 kernel over 19×19 yields exactly 15×15. The padding is materialized in the plane, not by Burn. |
| **SPIKE idiom** | The Burn 0.22 manual-weight-assignment pattern from convolutions chapter 2: build a `Conv2d` from `Conv2dConfig`, then replace its `weight`/`bias` with `Param::from_tensor(...)`. |
| **Border pull** | The defect this chapter's occupancy plane exists to avoid: a padded all-ones border ring read as phantom "stone mass" by a proximity kernel, dragging scores toward the edges. |
| **Branch** | A small named conv path beside the main network. Gravity is its own branch in `baseline`, not a modification of `patterns`. |

Board-diagram notation (`X` own stone, `O` opponent stone, `_`/`·`
empty, `c` candidate cell) carries over from the convolutions
tutorial's notation section
([03-the-kernel-language.md](../convolutions-tutorial/03-the-kernel-language.md));
diagram-local glyphs are defined where they appear.

## Context

The convolutions tutorial left `patterns` with a proven threat
network: planes in, threat maps out, every map gated against an
oracle (convolutions chapters 4–8, per the README's prerequisite).
What it does not contain is any opinion about *quiet* boards —
positions where no map fires. The mcts mock evaluator's opinion about
quiet boards is a uniform prior, which spreads the search's 400
simulations near-randomly across 200+ legal cells.

This chapter starts the `baseline` crate — the first crate to depend
on `engine`, `mcts`, and `patterns` at once — and gives the quiet
board a sane default. Nothing here touches the threat network:
gravity is a separate one-kernel conv branch, and this chapter needs
only `engine` (for `Board`) and `burn` (for the branch). The `mcts`
and `patterns` edges are scaffolded in `Cargo.toml` now (the crate
topology is pinned up front, per the spec) and consumed from chapter 2
onward.

## Intention

1. Scaffold `gomoku/crates/baseline` as a workspace member with the
   full dependency topology (engine, mcts, patterns, burn/flex, rand;
   dev-dependency on engine's `testutil`; opt-in `gpu` feature
   mirroring patterns).
2. Implement `src/gravity.rs`:
   * `occupied_plane(&Board) -> [[f32; 19]; 19]` — the dedicated
     one-channel input, any stone, no border;
   * the gravity branch — a `Conv2d` (5×5, 1 → 1 channel, valid
     padding, bias 0) with hand-assigned weights;
   * `gravity_map(&Board, &Device) -> [[f32; 15]; 15]` — the
     normalized map, including the /32;
   * `gravity_reference(&[[f32; 19]; 19]) -> [[f32; 15]; 15]` — a
     deliberately naive nested-loop twin, the differential oracle.
3. Prove the four sanity properties: empty board ⇒ all zero; a single
   centre stone ⇒ the kernel's shape, hand-computed, compared with
   `==`; a corner stone ⇒ truncated, never boosted; Burn branch ==
   plain-Rust reference on seeded random boards.

Observable done-state: the chapter-1 tests listed below pass,
`cargo clippy --all-targets -- -D warnings` is green from `gomoku/`,
and `cargo fmt --all` makes no changes from `gomoku/`.

## Mental mapping

### Why does the quiet board need a default at all?

Picture the search on move 3. No immediate wins, no forced blocks, no
double threats, no threes — every threat map is uniformly zero. A
uniform prior over 200+ cells means PUCT's exploration term starts
nearly indifferent between "play next to the action" and "play in an
empty corner". Gomoku is a local game: the overwhelming majority of
good early moves stand near existing stones. The gravity map is the
smallest possible encoding of that fact — it says nothing about
*tactics* (the tiers above it own tactics, chapter 2) and nothing
about *value* (gravity never enters the value readout, chapter 3). It
only breaks the quiet-board tie in the right direction.

### Why is the centre cell zero?

The kernel:

```text
gravity kernel, 5×5 — weight by Chebyshev distance from the centre:

1 1 1 1 1
1 2 2 2 1
1 2 0 2 1     ← centre weight exactly 0
1 2 2 2 1
1 1 1 1 1
```

A kernel is a picture of a *neighborhood* condition, and "there is a
stone on the candidate cell" is not a neighborhood condition — it is
the move-legality condition, and occupied cells are illegal moves
anyway. This is the same reason the convolutions tutorial's pattern
kernels put no weight on the candidate cell `c` itself. The centre
zero also makes a stone contribute nothing to its own cell, so a stone
never "attracts" the search onto an occupied square.

### Why ring weights 2 and 1 — and why divide by 32?

Any monotone decay by distance works; the tutorial pins this table as
an `[experiment]` choice, not a law. The table does buy three
properties:

1. **Small integers** — every raw output is an exact f32 sum on any
   backend (the exactness principle).
2. **A power-of-two maximum** — the raw output is maximized by a cell
   whose entire 24-cell ring is occupied:
   8·2 + 16·1 = **32**. Dividing by 32 maps gravity into [0, 1], and
   32 being a power of two means every normalized value is a dyadic
   fraction — still exactly representable, still comparable with `==`.
3. **A clean interface to chapter 2** — the policy readout adds
   gravity to the logit stack as a term in [0, 1]; the domination
   arithmetic there assumes exactly this range.

Around a single stone, the normalized map (shown as raw sums, i.e.
×32) is:

```text
1 1 1 1 1
1 2 2 2 1
1 2 ■ 2 1
1 2 2 2 1
1 1 1 1 1

■ = the stone. Its own cell reads 0 (centre weight 0) and is occupied,
so the policy readout masks it to −∞ anyway — chapter 2.
Normalized: the distance-1 ring reads 2/32 = 0.0625, the distance-2
ring 1/32 = 0.03125 — both dyadic, both exact.
```

### Why not reuse the convolutions planes? The border pull, in arithmetic

The `patterns::planes` output is two 25×25 channels built for the
pattern kernels: **stones** (the requested side's stones, zero-padded)
and **blocked** (opponent stones *or border*, one-padded — the border
ring is deliberately all ones, because for a *pattern* kernel "off the
board" must fail a required-empty test). Both semantics are wrong for
proximity:

- **Stones-only is color-biased.** Gravity over channel 0 prefers
  clustering near *our own* stones and is blind to the opponent's
  mass. Quiet-position defense — standing near the opponent's
  developing shape — is exactly what a proximity prior must see.
- **Blocked pulls toward the edges.** Watch the 5×5 gravity window
  anchored on the corner cell of an *empty* board, read over the
  blocked channel:

```text
       dx=  −2   −1    0   +1   +2
dy=−2        B    B    B    B    B
dy=−1        B    B    B    B    B
dy= 0        B    B    c    .    .
dy=+1        B    B    .    .    .
dy=+2        B    B    .    .    .

B = 1.0 in the blocked channel (opponent stone OR border — the kernel
    cannot tell them apart; on an empty board every B is pure padding)
c = the corner candidate cell (kernel centre, weight 0)
. = 0.0, a genuinely empty in-board cell
```

Count the `B` cells by ring: 5 at distance 1 (weight 2) and 11 at
distance 2 (weight 1) — raw score 5·2 + 11·1 = **21 out of a maximum
32**, normalized ≈ 0.66. On a board with *no stones at all*, the
corner reads as more than half as attractive as the densest possible
neighborhood, and every edge cell collects a smaller dose of the same
phantom mass. Reused as-is, the blocked channel would systematically
drag the opening toward the edges — the opposite of good Gomoku.

The fix is not a cleverer kernel; it is an honest input. The
**occupancy plane** holds 1 for a stone of *either* color and is
zero-padded with **no border ring**: off-board reads as "nothing
there", which is the truth. The same window on an empty board then
scores 0 at every cell, and a real corner stone imprints a *truncated*
map — only its 8 in-board neighbors light up, never boosted by
invisible mass. The corner test below is the regression net for this
exact failure.

### Why its own conv branch, instead of surgery on `patterns`?

The threat network is proven: 152 hand-written channels gated against
two oracle tiers at 1000 boards × 2 colors. Adding a proximity channel
to its input planes would change what every one of those kernels sees
— a re-verification project, not a branch. The sane pattern is the one
this crate starts and later heuristics continue: **each heuristic is
its own small named branch**, built in the open in glue, reading an
input built for its semantics. The branches meet only in the readout,
as numbers. (This is also the DeepGomoku pattern — frozen hand-written
features beside learned ones — kept small enough to hold in your
head.)

### Why a Burn conv for a single kernel — and why a plain-loop twin?

One kernel is one line of algebra; a nested loop would do. The conv
branch exists for three reasons beyond this kernel: it keeps the
device story identical to `patterns` (`Device::flex()` by default, an
opt-in `gpu` feature running the same weights); it rehearses, on the
smallest possible stage, the branch shape the trained net's
hand-written feature path will reuse; and it gives the
trusting-the-framework pattern from convolutions chapter 2 a second
exercise — the Burn branch must equal the deliberately naive
`gravity_reference` *exactly* on randomized small-integer inputs, so
the framework is trusted only after it has been checked.

## Low-level design

### Files you will create or modify

```text
gomoku/
├── Cargo.toml                 (workspace members += "crates/baseline")
└── crates/
    └── baseline/
        ├── Cargo.toml         (new)
        └── src/
            ├── lib.rs         (new: crate doc + `pub mod gravity;`)
            └── gravity.rs     (new)
```

### `baseline/Cargo.toml` — the full topology, scaffolded now

```toml
[package]
name = "baseline"
description = "Hand-written policy/value evaluator behind the mcts Evaluator seam"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
engine = { path = "../engine" }
mcts = { path = "../mcts" }
patterns = { path = "../patterns" }
burn = { workspace = true, features = ["flex"] }
rand = "0.10.3"

[dev-dependencies]
engine = { path = "../engine", features = ["testutil"] }

[features]
gpu = ["patterns/gpu"]
```

The dependency rules from docs/12 §6 are untouched and checkable:
`engine` stays a Burn-free island, `mcts` stays Burn-free, and nothing
in those crates knows `baseline` exists. `mcts`, `patterns`, and
`rand` are unused in this chapter; they are part of the pinned
topology (chapters 2–4 consume them), and scaffolding them now keeps
every later diff a pure source diff.

### `src/gravity.rs`

```rust
pub const GRAVITY_KERNEL: usize = 5;
pub const GRAVITY_RADIUS: i32 = 2;
pub const OCC_PADDED: usize = 19;   // 15 + 2 * GRAVITY_RADIUS
pub const GRAVITY_NORM: f32 = 32.0; // 8·2 + 16·1 — the maximum raw output
```

**`occupied_plane(b: &Board) -> [[f32; OCC_PADDED]; OCC_PADDED]`**
— 1.0 at `(r + GRAVITY_RADIUS, c + GRAVITY_RADIUS)` for every board
cell `(r, c)` with `b.stone_at(mv).is_some()` (either color), 0.0
elsewhere. The two-cell padding ring is zeros: no border, ever.

**`gravity_weights() -> [[f32; GRAVITY_KERNEL]; GRAVITY_KERNEL]`**
— the table from the mental mapping: 0 at the centre, 2 at Chebyshev
distance 1, 1 at distance 2.

**`gravity_conv(device: &Device) -> Conv2d`** — a `Conv2d` from
`Conv2dConfig::new([1, 1], [GRAVITY_KERNEL, GRAVITY_KERNEL])` with
`PaddingConfig2d::Valid` and bias enabled, weights assigned from
`gravity_weights()` with the SPIKE idiom (convolutions chapter 2),
bias set to 0. Inputs and weights are non-negative, so no ReLU is
needed — the branch is a pure weighted sum.

**`apply_gravity(conv: &Conv2d, occ: &[[f32; OCC_PADDED]; OCC_PADDED],
device: &Device) -> [[f32; 15]; 15]`** — wrap `occ` as an NCHW tensor
of shape `[1, 1, 19, 19]`, run the branch (output `[1, 1, 15, 15]`),
divide every cell by `GRAVITY_NORM`, and return the 15×15 map. The
division is part of this function, so callers never see unnormalized
values.

**`gravity_map(b: &Board, device: &Device) -> [[f32; 15]; 15]`** —
`occupied_plane` → `gravity_conv` → `apply_gravity`. The convenience
composition used by this chapter's tests. (Chapter 4's evaluator will
hold a pre-built branch and call `occupied_plane` + `apply_gravity`
directly, so it never rebuilds the kernel per position.)

**`gravity_reference(occ: &[[f32; OCC_PADDED]; OCC_PADDED]) ->
[[f32; 15]; 15]`** — the deliberately naive twin: three nested loops
computing the same weighted sum and the same /32, in plain Rust, no
Burn. Public and documented as the differential oracle, on the
`patterns::naive` precedent.

**Semantics worth pinning.** The gravity map is defined on *all*
15×15 cells, occupied or not — the centre weight 0 means a stone never
boosts its own cell, and chapter 2's policy readout masks occupied
cells to `NEG_INFINITY` regardless. The map is exactly 0.0 on an empty
board, and at most 1.0 anywhere (a fully surrounded cell), both
statements exact by the dyadic construction.

## TDD checklist

Run these tests from `gomoku/` with `cargo test -p baseline`.

The chapter's suite (`gravity::tests`):

1. `occupied_plane_marks_both_colors_and_no_border` — a black stone and
   a white stone both read 1.0 at their padded positions; every cell
   of the two-wide padding ring reads 0.0.
2. `empty_board_gravity_is_zero` — all 225 cells exactly 0.0.
3. `center_stone_gravity_matches_hand_computed` — one stone at (7, 7);
   the 5×5 neighborhood equals the hand-computed map from the mental
   mapping (2/32 on the distance-1 ring, 1/32 on the distance-2 ring,
   0.0 at the stone's own cell and everywhere beyond), compared with
   `==`.
4. `corner_stone_is_truncated_not_boosted` — one stone at (0, 0): the
   three in-board distance-1 cells read exactly 2/32, the five
   in-board distance-2 cells exactly 1/32, and every other cell 0.0 —
   the center-stone map restricted to the board, with no phantom
   additions. This is the regression test for the border pull.
5. `burn_branch_matches_naive_reference` — seeded random legal
   playouts (a small test helper: play `k` random legal moves from a
   seeded `StdRng`, stop before terminal); for each board,
   `apply_gravity(&gravity_conv(&device), &occupied_plane(&b), &device)`
   == `gravity_reference(&occupied_plane(&b))`, cell for cell, with
   `==`.

Follow the red-green-refactor rhythm:

1. **Red:** Create the crate scaffold and `src/gravity.rs` with the
   constants, signatures, and `todo!()` bodies. Add the five tests.
   Run `cargo test -p baseline`. Expect panics/failures (`todo!()`
   bodies compile; they panic when a test reaches them).
2. **Green:** Implement `occupied_plane` and `gravity_reference`
   first (plain Rust, no framework), then `gravity_weights`,
   `gravity_conv`, `apply_gravity`, `gravity_map`. Run
   `cargo test -p baseline`. The five tests should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* The crate is scaffolded with the full §6 topology and
  `cargo check -p baseline` passes.
* `cargo test -p baseline` passes the five chapter-1 tests from
  `gomoku/`.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* `cargo check -p engine` still passes with Burn nowhere in sight —
  the island is untouched.
* Commit message for the code slice:
  `feat(baseline): crate scaffold + gravity kernel`.

Next: [Chapter 02 — The policy readout](02-the-policy-readout.md)

## References

* [`../../specs/2026-10-13-baseline-player-design.md`](../../specs/2026-10-13-baseline-player-design.md)
  — §3 (the kernel, the occupancy plane, the sanity properties), §6
  (crate topology).
* [`00-baseline-primer.md`](00-baseline-primer.md) — §3 (the gravity
  idea), §9 (the border-pull and color-bias bugs).
* [`../convolutions-tutorial/01-planes-and-a-conv-by-hand.md`](../convolutions-tutorial/01-planes-and-a-conv-by-hand.md)
  — materialized padding and the hand-rolled cross-correlation.
* [`../convolutions-tutorial/02-trusting-the-framework.md`](../convolutions-tutorial/02-trusting-the-framework.md)
  — the SPIKE idiom, NCHW layout, and the differential-testing pattern
  this chapter reuses.
* `gomoku/crates/patterns/src/planes.rs` — the two-channel semantics
  (stones / blocked-with-border) this chapter deliberately does **not**
  reuse.
