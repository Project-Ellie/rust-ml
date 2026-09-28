# Chapter 07 — Seeing it

## Abstract

This chapter is the payoff: the threat network has been built and
verified; now we make it visible and portable. You will learn how the
maps already score every legal move, so a small combiner can turn them
into a sparse policy vector with the same shape as the datagen
tutorial's `Sample::policy` and the future mcts `Evaluator` output. You
will wire a terminal demo that renders any threat map as an ANSI
heatmap over a built-in showcase position, so you can run the binary
and **see the fork cell light up**. Finally, you will prove that the
same hand-written network runs unchanged on the `Wgpu` backend, with a
CPU-vs-GPU differential test that asserts exact equality because every
kernel weight and every input plane value is a small integer.

The chapter belongs to the convolutions tutorial; its design authority
is the primer in
[`00-convolutions-primer.md`](00-convolutions-primer.md).

## Glossary

| Term | Definition |
|------|------------|
| **Threat potential** | A sparse vector `Vec<(Move, f32)>` that assigns one tactical score to every legal empty cell. |
| **Policy-shaped** | The vector has the same sparse `(Move, score)` shape as `Sample::policy` and the future `Evaluator` output; this is a shape contract, not an integration. |
| **Priority ladder** | The rule that higher tactical value dominates lower value: win > double threat > double-three fork > single open-three maker. |
| **`[experiment]` weights** | Hyper-parameters chosen to encode the priority ladder; documented as replaceable, not ordained. |
| **Heatmap** | A terminal rendering of a threat map with coloured backgrounds on empty cells and stone glyphs for occupied cells. |
| **Showcase** | One of the built-in demo positions: `double-three-fork`, `classic-fork`, `corner-fork`, or `quiet`. |
| **GPU portability** | The guarantee that the same backend-generic code runs on `NdArray` (default) and on `Wgpu` with the opt-in `gpu` feature. |
| **Exact equality** | A legitimate differential-test assertion here because all conv inputs and weights are small integers, so every `f32` sum is exact on both CPU and GPU. |

## Context

Chapters 1–6 built the `patterns` crate from the ground up: padded
input planes, the hand-rolled cross-correlation reference, Burn's
`Conv2d`, the exact-pattern kernel language, the four-level maps, the
three-maker maps, and the double-three fork. Every map was verified by
differential testing against either the engine or a naive oracle.

This chapter adds three things that make the crate usable and
transparent:

1. **`src/potential.rs`** turns the threat maps into a sparse policy
   vector. The maps already evaluate every legal move; the combiner
   just assigns a priority score and drops empty cells with no
   tactical interest.
2. **`src/bin/patterndemo.rs`** renders a selected threat map as a
   terminal heatmap. Built-in showcase positions include the tutorial's
   headline double-three fork, the engine's own classic fork, a corner
   fork, and a quiet position.
3. **`tests/gpu_port.rs`** runs the full `analyze` pipeline and the
   `policy()` combiner on both `NdArray` and `Wgpu` and asserts exact
   equality.

The chapter is intentionally lightweight in new concepts: the lesson
is that the hard-won maps are now a reusable, inspectable, and
backend-portable artifact.

## Intention

1. Implement `src/potential.rs`:
   * `pub const WIN_SCORE: f32 = 1.0;`
   * `pub const DOUBLE_THREAT_SCORE: f32 = 0.9;`
   * `pub const DOUBLE_THREE_SCORE: f32 = 0.8;`
   * `pub const THREE_SCORE: f32 = 0.3;`
   * `pub fn policy<B: Backend>(net: &ThreatNet<B>, b: &Board, s: Color, device: &B::Device) -> Vec<(Move, f32)>;`
   * `pub fn policy_from_maps(b: &Board, maps: &ThreatMaps) -> Vec<(Move, f32)>;`
2. Implement `src/render.rs`:
   * `MapName` and `ShowcaseName` enums with `parse` and `label`.
   * `pub fn showcase_board(name: ShowcaseName) -> Board;`
   * `pub fn showcase_side(_name: ShowcaseName) -> Color;`
   * `pub fn render_heatmap(...) -> String;`
   * `pub fn map_value(...)` helper.
3. Implement `src/bin/patterndemo.rs`:
   * Parse `--map`, `--showcase`, `--gpu`, and `--help`.
   * Run `analyze` on `NdArray` by default, or on `Wgpu` when `--gpu`
     is supplied and the `gpu` feature is enabled.
   * Print the rendered heatmap.
4. Implement `tests/gpu_port.rs`:
   * CPU-vs-GPU exact-equality tests for all four showcase boards,
   * for both `ThreatMaps` and `policy()` output.

Observable done-state: `cargo test -p patterns` passes from `gomoku/`,
`cargo test -p patterns --features gpu` passes on a machine with a
working adapter, and the formatting/lint gates are green.

## Mental mapping

### Why is `policy()` just a shape contract?

The datagen tutorial stores positions with a sparse `Sample::policy`
vector. The mcts crate will eventually ask an `Evaluator` for a sparse
policy. The `patterns` crate is not wired to either of them in this
side quest; the point is that its output *already has the right shape*.

A future adapter would be a thin wrapper: call `policy()` and, if
necessary, re-normalise the scores into probabilities. The tutorial
leaves that wrapper unwritten so the crate stays focused on teaching
convolutions.

### Why a priority ladder, and why label it `[experiment]`?

The maps give four mutually exclusive categories for a legal move.
Rather than inventing a smooth function, the combiner encodes the
obvious ordering:

| Category | Score |
|----------|-------|
| Immediate win | `1.0` |
| Double threat | `0.9` |
| Double-three fork | `0.8` |
| Single open-three maker | `0.3` |

These numbers are not learned; they are a placeholder prior. Labeling
them `[experiment]` makes that status explicit. A later learning step
will replace them, but the structure — one score per cell, higher
tactical value dominates — is what matters here.

### Why does the demo matter?

Differential tests are the truth, but they are not intuitive. The demo
lets you *see* the network's belief about a position. When the centre
cell of the plus-sign fork lights up, the kernel language from chapter
3 stops being an abstraction and becomes a picture on the board.

The demo is also a regression tool. The showcase positions are adapted
from engine tests and the crate's own unit tests; if a refactor ever
breaks the kernel table, the demo will show the wrong cell highlighted.

### Why does `NdArray` stay the default?

The `gpu` feature adds the `burn/wgpu` dependency tree and needs a
working GPU adapter. The default build uses `NdArray` because it is:

* deterministic on every machine,
* light to compile,
* sufficient for the tutorial's CPU-only gates.

The backend abstraction is the real lesson: the same
`ThreatNet<B>::new(device)` call works for `B = NdArray` and `B =
Wgpu`. The net tutorial will train on GPU using exactly this
abstraction.

### Why is CPU-vs-GPU exact equality legitimate?

Floating-point equality is usually a red flag, but here it is a
deliberate teaching instrument. Every weight in the network is a small
integer (`−1`, `0`, `1`, `2`). Every input plane value is `0.0` or
`1.0`. An `f32` can represent every integer exactly up to `2^24`, and
the sums inside the kernels are far below that threshold. Therefore
both backends produce the same exact `f32` bit pattern, and the test
can use `assert_eq!`.

If the test ever fails, the bug is in shape handling, padding, or
backend setup — not in numerical tolerance.

## Low-level design

### Files you will create or modify

```text
gomoku/crates/patterns/
├── src/
│   ├── lib.rs       (already registers `pub mod potential;` and `pub mod render;`)
│   ├── potential.rs (new)
│   ├── render.rs    (new)
│   └── bin/
│       └── patterndemo.rs (new)
└── tests/
    └── gpu_port.rs  (new)
```

### `src/potential.rs`

The public contract is sparse and simple:

```rust
pub const WIN_SCORE: f32 = 1.0;
pub const DOUBLE_THREAT_SCORE: f32 = 0.9;
pub const DOUBLE_THREE_SCORE: f32 = 0.8;
pub const THREE_SCORE: f32 = 0.3;

pub fn policy<B: Backend>(
    net: &ThreatNet<B>,
    b: &Board,
    s: Color,
    device: &B::Device,
) -> Vec<(Move, f32)>;

pub fn policy_from_maps(b: &Board, maps: &ThreatMaps) -> Vec<(Move, f32)>;
```

Behavior:

* Iterate over the 15×15 board.
* Skip occupied cells.
* For each empty cell, return the highest-priority score it qualifies
  for:
  * `WIN_SCORE` if `maps.wins[r][c] > 0.0`,
  * `DOUBLE_THREAT_SCORE` if `maps.double_threats[r][c] > 0.0`,
  * `DOUBLE_THREE_SCORE` if `maps.double_threes[r][c] > 0.0`,
  * `THREE_SCORE` if any `maps.threes_per_dir[d][r][c] > 0.0`,
  * otherwise omit the cell.
* Sort the result lexicographically by `(row, col)` so the order is
deterministic.
* The vector may be empty on a quiet board.

This is the **one-score-per-cell** contract: a `Move` appears at most
once, and the score is the highest matching category.

### `src/render.rs`

Public types and functions:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapName { Wins, DoubleThreats, DoubleThrees, ThreesH, ThreesV, ThreesDd, ThreesDu, Policy }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShowcaseName { DoubleThreeFork, ClassicFork, CornerFork, Quiet }

impl MapName { pub fn parse(s: &str) -> Option<Self>; pub fn label(self) -> &'static str; }
impl ShowcaseName { pub fn parse(s: &str) -> Option<Self>; pub fn label(self) -> &'static str; }

pub fn showcase_board(name: ShowcaseName) -> Board;
pub fn showcase_side(_name: ShowcaseName) -> Color;
pub fn map_value(maps: &ThreatMaps, map: MapName, r: usize, c: usize) -> f32;
pub fn render_heatmap(
    maps: &ThreatMaps,
    board: &Board,
    showcase: ShowcaseName,
    map: MapName,
    backend: &str,
) -> String;
```

The four showcases (controller amendment: the demo has four, not two):

| Showcase | Source | What it shows |
|----------|--------|---------------|
| `double-three-fork` | `net::tests::double_three_fork_matches_oracle` | Plus-sign fork; the default showcase. |
| `classic-fork` | Engine tactics tests | A double-threat fork — the engine's exact board. |
| `corner-fork` | Engine tactics tests | A corner double-threat position. |
| `quiet` | Crate's own test corpus | A position with no tactics. |

All showcases are built by replaying a legal move sequence rather than
using `board_from_ascii`, because `board_from_ascii` lives behind the
engine's `testutil` feature and must not leak into non-test or binary
code.

`render_heatmap` prints a title line, column header, each board row
with stones and highlighted empty cells, and a legend. Occupied cells
show `●` for Black and `○` for White. Empty cells show `.` when the map
value is zero and a coloured background otherwise.

### `src/bin/patterndemo.rs`

```text
Usage: patterndemo [OPTIONS]

Options:
  --map <NAME>        Threat map to render (default: double_threes)
                      wins | double_threats | double_threes |
                      threes_h | threes_v | threes_dd | threes_du | policy
  --showcase <NAME>   Built-in position (default: double-three-fork)
                      double-three-fork | classic-fork | corner-fork | quiet
  --gpu               Run on the Wgpu backend (requires `gpu` feature)
  --help              Print this help message
```

CLI conventions:

* `--help` / `-h` prints help and exits successfully.
* Usage errors (unknown flag, missing value, unknown map/showcase name)
  print a message to stderr, print a concise usage block, and exit
  with status **2**.
* Runtime failures (for example `--gpu` without the `gpu` feature)
  print to stderr and exit with status **1**.

The binary is split into `run_cpu` and `run_gpu` so that the `Wgpu`
import is behind `#[cfg(feature = "gpu")]`.

### `tests/gpu_port.rs`

The test file is compiled only when the `gpu` feature is enabled:

```rust
#![cfg(feature = "gpu")]
```

For each showcase it:

1. Builds a `ThreatNet<NdArray>` and a `ThreatNet<Wgpu>`.
2. Runs `analyze` on both backends.
3. Asserts `cpu_maps == gpu_maps` exactly.
4. Runs `policy()` on both backends.
5. Asserts `cpu_policy == gpu_policy` exactly.

The contract is **full pipeline equality**: not just the final maps,
but also the policy-shaped output must be identical across backends.

## The payoff: run the demo

The default command renders the double-three fork showcase:

```text
cargo run -p patterns --bin patterndemo
```

Captured output (verbatim from the reference worktree):

```text
Showcase: double-three fork | Map: double-three forks | Backend: NdArray
    0 1 2 3 4 5 6 7 8 9 10 11 12 13 14
 0   ○ . . . . . . . . . . . . . ○
 1   . . . . . . . . . . . . . . .
 2   . . . . . . . . . . . . . . .
 3   . . . . . . . . . . . . . . .
 4   . . . . . . . . . . . . . . .
 5   . . . . . . . . . . . . . . .
 6   . . . . . . . ● . . . . . . .
 7   . . . . . . ●[48;5;196m1 [0m ● . . . . . .
 8   . . . . . . . ● . . . . . . .
 9   . . . . . . . . . . . . . . .
10   . . . . . . . . . . . . . . .
11   . . . . . . . . . . . . . . .
12   . . . . . . . . . . . . . . .
13   . . . . . . . . . . . . . . .
14   ○ . . . . . . . . . . . . . ○

Legend: ● Black  ○ White  highlighted = map value > 0
```

The highlighted cell at row 7, column 7 is the fork intersection. On a
binary map the highlight simply means "value > 0"; the colour code is
shared with the win-map renderer.

The engine's classic fork is a double threat, not a double three, so it
only lights up under `--map double_threats`:

```text
cargo run -p patterns --bin patterndemo -- --showcase classic-fork --map double_threats
```

Captured output:

```text
Showcase: classic fork | Map: double threats | Backend: NdArray
    0 1 2 3 4 5 6 7 8 9 10 11 12 13 14
 0   . . . . . . . . . . . . . . ○
 1   . . . . . . . . . . . . . . .
 2   . . . . . . . . . . . . . . .
 3   . . . . . . . ○ . . . . . . .
 4   . . . . . . . ● . . . . . . .
 5   . . . . . . . ● . . . . . . .
 6   . . . . . . . ● . . . . . . .
 7   . . . ○ ● ● ●[48;5;196m1 [0m . . . . . . .
 8   . . . . . . . . . . . . . . .
 9   . . . . . . . . . . . . . . .
10   . . . . . . . . . . . . . . .
11   . . . . . . . . . . ○ . . . .
12   . . . . . . . . . . . . . . .
13   . . . . . . . . . . . . . . .
14   ○ . . . . . . . . . . . . . ○

Legend: ● Black  ○ White  highlighted = map value > 0
```

The policy map renders the `[experiment]` scores directly, so the fork
cell shows `8` (double three), the surrounding three-maker cells show
`3`, and wins or double threats would show `1` or `9` respectively:

```text
cargo run -p patterns --bin patterndemo -- --showcase double-three-fork --map policy
```

Captured output:

```text
Showcase: double-three fork | Map: threat potential (policy) | Backend: NdArray
    0 1 2 3 4 5 6 7 8 9 10 11 12 13 14
 0   ○ . . . . . . . . . . . . . ○
 1   . . . . . . . . . . . . . . .
 2   . . . . . . . . . . . . . . .
 3   . . . . . . . . . . . . . . .
 4   . . . . .[48;5;39m3 [0m . . .[48;5;39m3 [0m . . . . .
 5   . . . .[48;5;39m3 [0m .[48;5;39m3 [0m[48;5;39m3 [0m[48;5;39m3 [0m .[48;5;39m3 [0m . . . .
 6   . . . . .[48;5;39m3 [0m . ● .[48;5;39m3 [0m . . . . .
 7   . . . . .[48;5;39m3 [0m ●[48;5;220m8 [0m ●[48;5;39m3 [0m . . . . .
 8   . . . . .[48;5;39m3 [0m . ● .[48;5;39m3 [0m . . . . .
 9   . . . .[48;5;39m3 [0m .[48;5;39m3 [0m[48;5;39m3 [0m[48;5;39m3 [0m .[48;5;39m3 [0m . . . .
10   . . . . .[48;5;39m3 [0m . . .[48;5;39m3 [0m . . . . .
11   . . . . . . . . . . . . . . .
12   . . . . . . . . . . . . . . .
13   . . . . . . . . . . . . . . .
14   ○ . . . . . . . . . . . . . ○

Legend: 1=win (1.0)  9=double threat (0.9)  8=double three (0.8)  3=three maker (0.3)
```

The `8` at the centre is the double-three fork; the ring of `3`s are
the single open-three makers that feed it.

## Solution (opt-in)

The complete reference files for this chapter live in
[07-seeing-it/01-solution.md](07-seeing-it/01-solution.md). Open it
only if you have been stuck for more than twenty minutes, or after you
have finished the chapter and want to compare. The solution contains
`src/potential.rs`, `src/render.rs`, `src/bin/patterndemo.rs`, and
`tests/gpu_port.rs` quoted verbatim from the verified reference crate.

## TDD checklist

Run these tests from `gomoku/` with `cargo test -p patterns`. Run the
GPU variants with `cargo test -p patterns --features gpu` on a machine
with a working adapter.

From `potential::tests` (5 tests):

1. `double_three_fork_scores_zero_point_eight`
2. `open_four_maker_scores_zero_point_nine`
3. `win_scores_one_point_zero`
4. `quiet_board_returns_empty_policy`
5. `only_legal_empty_cells_are_present`

From `render::tests` (7 tests):

1. `map_name_parsing`
2. `showcase_name_parsing`
3. `classic_fork_is_exactly_one_double_threat_at_centre`
4. `double_three_fork_has_fork_at_centre`
5. `corner_fork_has_black_stones_in_corner`
6. `quiet_position_has_no_black_tactics`
7. `render_includes_title_and_backend`

From `patterndemo` parse tests (3 tests):

1. `parse_args_rejects_unknown_flag`
2. `parse_args_accepts_map_and_showcase`
3. `parse_args_parses_gpu_flag`

From `tests/gpu_port.rs` (4 tests, requires `--features gpu`):

1. `double_three_fork_cpu_gpu_equal`
2. `classic_fork_cpu_gpu_equal`
3. `corner_fork_cpu_gpu_equal`
4. `quiet_position_cpu_gpu_equal`

### Carried-over note

All tests from previous chapters continue to pass and are part of the
chapter-7 gate. That includes the win-map tests, the four-level engine
differential gates, the three-maker ASCII tests, the oracle tests, and
the chapter-2 spike tests.

Follow the red-green-refactor rhythm:

1. **Red:** Create `src/potential.rs`, `src/render.rs`,
   `src/bin/patterndemo.rs`, and `tests/gpu_port.rs` with the public
   signatures and all tests above, but implement the functions as
   `todo!()`. Run `cargo test -p patterns`. Expect failures.
2. **Green:** Implement the priority combiner, the rendering helpers,
   the demo binary, and the GPU portability test. Run
   `cargo test -p patterns`. The default tests should pass.
3. **GPU check:** If your environment supports it, run
   `cargo test -p patterns --features gpu`. The four `gpu_port` tests
   should pass. If the adapter is unavailable, skip this step; the
   default gates do not depend on it.
4. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p patterns` passes from `gomoku/` (including the new
  potential, render, and patterndemo tests, and all earlier tests).
* `cargo test -p patterns --features gpu` passes on a GPU-capable
  machine (documented as environment-dependent and excluded from the
  default gates).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Code slice commit message: `feat(patterns): threat potential + heatmap demo + gpu`.

Next: [Chapter 08 — Acceptance](08-acceptance.md)

## References

* [`06-the-fork.md`](06-the-fork.md) — the previous chapter:
  double-three maps and the naive oracle.
* [`docs/specs/2026-09-27-convolutions-tutorial-design.md`](../../specs/2026-09-27-convolutions-tutorial-design.md)
  — §4.4 (backend story, batching, and the opt-in GPU feature) and
  §4.5 (policy-shaped output).
* [`docs/plans/2026-09-27-convolutions-tutorial.md`](../../plans/2026-09-27-convolutions-tutorial.md)
  — Task T9, which this chapter implements.
* `gomoku/crates/patterns/tests/gpu_port.rs` — the CPU-vs-GPU exact
  equality contract.
* `gomoku/crates/patterns/src/bin/patterndemo.rs` — the demo binary's
  CLI conventions and exit codes.
