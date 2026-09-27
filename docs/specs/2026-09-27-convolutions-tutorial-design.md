# Convolutions tutorial design — hand-written kernels for Gomoku threat detection

**Status:** Draft, 2026-09-27. Awaiting owner review before the
implementation plan is written.

## Abstract

This document specifies a side-quest tutorial that sits between the
datagen tutorial and the net tutorial in milestone 3. The learner
builds a new crate, `gomoku/crates/patterns`, containing a small
convolutional network in Burn 0.21 whose weights are written by hand,
not trained. The network reads a board position and produces *threat
maps*: spatial maps that mark every empty cell where placing a stone
completes five, creates an open four (the engine's "double threat"),
or forks two open threes (the classic double-three). Every map is
verified by differential testing — the four-level maps against the
engine's tactics functions, the three-level maps against an in-crate
naive oracle. The tutorial teaches what a convolution is, what a
convolutional layer can express, how Burn represents tensors and conv
layers, and how feed-forward evaluation works — with no training loop,
no GPU, and instant, deterministic results. A terminal demo renders
threat maps as heatmaps over the board.

A glossary follows; the body defines every term before use.

## Glossary

- **Kernel** — a small weight matrix slid across the input planes; at
  each position it computes a weighted sum. The elementary pattern
  detector of a convolutional network.
- **Cross-correlation** — the sliding weighted-sum operation that deep
  learning frameworks call "convolution". (A mathematical convolution
  flips the kernel first; deep learning does not, and neither does
  this tutorial.)
- **Channel** — one plane of a tensor. The input has two channels
  (side's stones, blocked cells); a conv layer's output has one
  channel per kernel.
- **Feature map** — one output channel of a conv layer, read as a
  spatial map over the board.
- **Threat map** — a feature map with game meaning: each empty cell is
  scored by what happens if the side places a stone there.
- **Five-completer / immediate win** — an empty cell where placing a
  stone completes five in a row. Engine: `immediate_wins`.
- **Open four** — four stones in a line with both ends empty; it has
  two five-completing cells and is unanswerable.
- **Broken four** — four stones in a five-cell window with one gap;
  only the gap completes five.
- **Double threat** — an empty cell where placing a stone leaves the
  side with two or more five-completing cells. Engine:
  `double_threats`.
- **Open three** — the v1 definition in §3.4: one of the exact window
  patterns `_XXX_`, `_X_XX_`, `_XX_X_` (both ends empty, no fourth
  same-color stone adjacent).
- **Double-three fork** — an empty cell where placing a stone creates
  open threes in two or more distinct directions. The beginning of a
  threat sequence; the tutorial's headline target.
- **Oracle** — a trusted implementation used as ground truth in a
  differential test.
- **Differential test** — a test that runs two independent
  implementations over the same inputs and requires identical outputs.

## 1. Context

The project philosophy is "alpha-epsilon" (docs/12, §3): hand-woven
tactics give the agent a head start that learning later refines. The
engine already computes tactics as bitboard truth (`immediate_wins`,
`forced_blocks`, `double_threats`). The owner's earlier Python project
(DeepGomoku) went further: hand-manufactured neural networks provided
guidance to the trained policy.

The pedagogical gap this tutorial fills: the net tutorial must teach
Burn's tensor API, conv layers, and module system *while also*
teaching training, loss functions, and data loading. That is a lot of
new machinery at once, and nothing visible happens until training has
run. A hand-written network inverts the experience: the learner writes
every weight, understands exactly what each kernel detects, and sees
correct threat maps appear immediately — deterministic, debuggable,
no training.

The tutorial is a **side quest** (like the CLI tutorial), not a
milestone. It adds no game capability the engine lacks; its value is
teaching and de-risking. Its position: after the datagen tutorial
(which produces data) and before the net tutorial (which trains on
it), so that the net tutorial can focus on training.

## 2. Goals and non-goals

### Goals

1. Teach convolution as *parallel local pattern detection*: a conv
   layer is an array of pattern detectors applied at every position.
2. Teach the exact-pattern kernel language: how `+1`/`−1` weights, a
   bias, and a ReLU express precise Boolean stone/empty conditions —
   the "power of convolutional networks" made concrete.
3. Introduce Burn 0.21 tensors, `Conv2d`, manual weight assignment,
   batching, and the backend abstraction (CPU by default, GPU on an
   opt-in feature) — the exact machinery the net tutorial will reuse.
4. Reproduce the engine's tactics *as a feed-forward network* and
   prove it by differential testing.
5. Deliver the double-three fork detector — an empty-cell map that
   marks moves creating two open threes.
6. Instant visibility: a terminal demo rendering threat maps as
   heatmaps.

### Non-goals

- No training, no autodiff, no loss functions (net tutorial's job).
- No new game capability over the engine (side quest, not milestone).
- No integration with `mcts`, `train`, or `cli` (a future `Evaluator`
  adapter is noted as motivation only).
- No threat-space search, no threat-sequence analysis beyond the
  static maps (the engine's TSS prover owns that).
- No renju forbidden-move rules; freestyle Gomoku only, consistent
  with the engine.

## 3. The threat ladder — map semantics

All maps are functions from board cells to scores, defined for a given
side `s` on a non-terminal board `b`. Maps are evaluated at *empty*
cells and answer: "what happens if `s` places a stone here?"

### 3.1 Win map

`win_map(c)` fires iff placing at `c` completes five in a row
(overlines count, per freestyle rules). This is exactly
`engine::immediate_wins(b, s)`.

### 3.2 Four-level maps

- `open_four_maker(c, d)` fires iff placing at `c` creates four
  consecutive stones along direction `d` with both ends empty (and no
  fifth same-color stone adjacent — that would be a five, not a four).
- `broken_four_maker(c, d)` fires iff placing at `c` creates four
  stones in a five-cell window along `d` with one gap (the gap is the
  unique five-completing cell of that four).
- `double_threat_map(c)` fires iff placing at `c` leaves `s` with two
  or more five-completing cells in total across all directions, and
  `c` itself does not complete five. This is exactly
  `engine::double_threats(b, s)` — including its documented
  simplifications (the four-three blind spot, opponent-wins-first
  acceptance, immediate wins excluded).

Note the counting subtlety the network must reproduce: one open four
contributes *two* five-completing cells, one broken four contributes
*one*. The map is a count threshold (`≥ 2`), not a direction count —
an open four alone suffices, as do two broken fours. The 1×1 combiner
layer implements this by letting channels carry counts (§4.3).

### 3.3 Oracle for levels 3.1–3.2

The engine. `win_map` is differentially tested against
`immediate_wins`, `double_threat_map` against `double_threats`, over
at least 1000 seeded random boards each.

### 3.4 Three-level maps (v1 definition)

An **open three along direction `d`** is one of three exact window
patterns along `d`:

- `_XXX_` (five cells: three stones, both ends empty),
- `_X_XX_` (six cells),
- `_XX_X_` (six cells),

with the additional anti-four condition: the cells immediately beyond
the window's empty ends hold no stone of `s` (otherwise the pattern is
a four or five, handled by the four-level maps — the partition stays
clean by construction).

`open_three_maker(c, d)` fires iff placing at `c` creates an open
three along `d` that includes `c`.

`double_three_map(c)` fires iff `open_three_maker` fires for two or
more **distinct** directions. (Two threes along the same line are one
three; the per-direction channel structure makes this fall out.)

**Documented v1 simplification** (engine-tactics style): the
definition requires the three's window ends to be empty, but does not
require *maturation room* — an open three that cannot actually become
an open four because the board edge or an opponent stone sits one cell
further out still counts. Threat theory would exclude it; v1 keeps the
patterns small and the definition local. Extending the kernels with
maturation-room conditions is an explicit end-of-chapter exercise.

### 3.5 Oracle for level 3.4

The engine has no open-three concept, so the crate carries a **naive
oracle**: a plain, slow, obviously-correct enumerator (no convolution,
no bitboards — array scans) that implements §3.4 directly: for each
empty cell and each direction, place the stone hypothetically and scan
the line's windows. The oracle is unit-tested against hand-written
ASCII boards, then the conv network is differentially tested against
the oracle over at least 1000 seeded random boards. Two independent
implementations of one definition; a discrepancy localizes the bug to
kernel enumeration or oracle.

## 4. The convolutional architecture

### 4.1 Input planes

Two channels, 15×15, for side `s`:

- Channel 0: stones of `s`.
- Channel 1: **blocked** cells — opponent stones *or* the board
  border.

The border-as-blocked trick makes edge behavior fall out of the
patterns: a required-empty cell off the board is not empty, which is
exactly right (a three whose open end is the board edge is not open).
Because Burn's padding only zero-pads, the padding is materialized:
`planes.rs` returns both channels explicitly padded by 4 cells on
every side (23×23), channel 0 zero-padded, channel 1 one-padded. The
conv layer then uses valid (no) padding and emits 15×15 maps.

Colors are absolute per the locked ch. 12 decision; the caller picks
`s`. Evaluating both colors is a batch of two (§4.4).

The engine's `encode` is *not* reused: it pads for stride-16 alignment
and carries side-to-move-relative planes for the future policy net —
different semantics, different consumer. (One sentence in the tutorial
says so.)

### 4.2 The exact-pattern kernel language

Every pattern in §3 reduces to a local condition of the form "these
cells hold stones of `s`, those cells are empty, the rest don't
care". The kernel language expresses each such condition as one
output channel:

- `+w` on required-stone cells in channel 0,
- `−w` on required-empty cells in *both* channels (a stone of either
  color — or the border, via channel 1 — violates emptiness),
- `0` (don't care) everywhere else,
- bias set so the pre-activation exceeds zero only on an exact match,
  with a margin of at least one violation unit,
- ReLU turns the result into a clean 0/1 (or a scaled count, §4.3).

All kernels use a uniform 9×9 footprint (radius 4 covers the largest
pattern window plus its anti-four margins); the don't-care zeros *are*
part of the lesson — a kernel is a picture of a pattern. The complete
kernel set is generated programmatically from a **declarative pattern
table** (shape, direction, required-stone offsets, required-empty
offsets), so the table — not hand-drawn weight matrices — is the
artifact under review. The channel count (on the order of one hundred)
is pinned by the Task-0 spike (§9), not by this document.

### 4.3 The two-layer network

- **Layer 1** (`Conv2d`, 9×9, 2 → N channels, valid padding, ReLU):
  every pattern kernel, for every direction, evaluated at every cell.
  Output: the raw per-direction maker channels.
- **Layer 2** (`Conv2d`, 1×1, N → M channels, ReLU): per-cell
  cross-channel logic. It sums the shape channels within each
  direction (logical OR via threshold), scales the open-four channel
  by two so it contributes two five-completing cells to the count,
  and thresholds at two to produce `double_threat_map` and
  `double_three_map`. The 1×1 convolution as cross-channel combiner —
  and channels carrying counts rather than bits — are explicit
  teaching points.

The public result is a `ThreatMaps` struct carrying the named maps
(wins, double threats, double threes, plus the per-direction
three/four maps the heatmap demo visualizes).

### 4.4 Burn usage, backends, and batching

The crate is **backend-generic** (`B: Backend`) from the start — the
backend abstraction is itself a teaching goal, and the net tutorial
will train on GPU (ch. 12's phased scheduling). No autodiff:
evaluation only.

- **Default backend: `NdArray`** (pure Rust, CPU). The differential
  gates and per-chapter tests run on it: deterministic on every
  machine, light dependency tree, fast iteration.
- **Optional `gpu` feature** (`patterns/gpu = ["burn/wgpu"]`):
  chapter 7 runs the *same* network with the *same* weights on the
  `Wgpu` backend and adds a CPU-vs-GPU differential-equality test.
  Numerical exactness is guaranteed by construction — all inputs and
  weights are small integers, so f32 sums are exact on any backend.
  The feature is off by default so the heavy wgpu dependency tree
  costs nothing until the learner opts in; the GPU test is run
  explicitly (`cargo test -p patterns --features gpu`) and is
  documented as environment-dependent (it needs a working GPU
  adapter), so it is not part of the default gates.

Weights are assigned manually by constructing each `Conv2d` from its
config and overwriting the parameter tensors with the values from the
pattern table. The exact Burn 0.21 idiom for manual weight assignment
— on both backends — is verified against the pinned source in the
Task-0 spike (hard rule 1: no main-branch or 0.22 APIs).

Both colors are evaluated as a batch of two (black's planes, white's
planes), introducing the batch dimension on something the learner can
see.

### 4.5 Threat potential as policy-shaped output

The maps already score every legal move, so a small combiner produces
`policy(b, s) -> Vec<(Move, f32)>` — the same sparse shape as the
datagen tutorial's `Sample::policy` and the mcts crate's evaluator
output. Combination weights (win > double threat > double three >
single three) are `[experiment]` values documented as such. This is a
*shape* contract for future integration, not an integration.

## 5. Crate layout

`gomoku/crates/patterns` — new workspace member. Dependencies:
`engine` (workspace), `burn` (workspace, `features = ["ndarray"]`);
dev-dependencies: `engine` with `testutil`, `rand` (0.10.3). No edges
to `mcts`, `train`, `cli`. The engine is untouched (dependency island;
`cargo check -p engine` stays Burn-free).

- `planes.rs` — board → two padded channels (§4.1).
- `naive.rs` — the learner's hand-rolled cross-correlation on plain
  Rust arrays (chapter 1); retained as the chapter-2 differential
  reference against Burn's `conv2d`.
- `kernels.rs` — the declarative pattern table + kernel/bias tensor
  construction (§4.2).
- `net.rs` — the Burn `ThreatNet` module with hand-assigned weights,
  `forward` → `ThreatMaps` (§4.3).
- `oracle.rs` — the naive open-three enumerator (§3.5), public and
  documented as a reference oracle, not production code.
- `potential.rs` — maps → sparse policy vector (§4.5).
- `src/bin/patterndemo.rs` — terminal heatmap demo on built-in
  showcase positions (§6).
- `tests/` — the differential gates (§7).

Random boards for the gates come from a small test-only generator
(seedable random playouts via engine rules, stopped before terminal).
This deliberately duplicates a few lines of the datagen `playout`
module: the `train` crate is not on `main` yet, and the dependency
must not appear. A comment says so.

## 6. Tutorial structure

Modeled on the mcts/datagen tutorials: a primer as design authority,
chapters with learning goals + contracts + TDD checklists and
implementations withheld, opt-in per-chapter solution files quoting
the verified reference verbatim, `feat(patterns): …` commit messages,
per-chapter gates (`cargo test -p patterns`, clippy, fmt).

- **00 — primer**: the threat ladder (§3), the kernel language (§4.2),
  the oracle ladder (§5 of this document), what the tutorial defers.
- **01 — planes and a conv by hand**: board → channels; the learner
  writes nested-loop cross-correlation on plain arrays; tiny
  hand-computed examples.
- **02 — trusting the framework**: Burn tensors and `Conv2d`; manual
  weight assignment; the learner's loop must match Burn exactly on
  random inputs (test inputs and weights are small integers, so the
  f32 sums are exact and order-independent) — differential testing as
  the way to learn what
  `conv2d` actually does, including padding and stride semantics.
- **03 — the kernel language**: exact-pattern kernels; the
  five-completer kernels; unit tests on ASCII boards (engine
  `testutil`).
- **04 — the four-level**: open/broken four makers, the counting
  combiner, `double_threat_map`; differential gate against the engine
  (≥ 1000 boards).
- **05 — three-makers**: the three pattern families as kernels;
  per-direction three maps; ASCII unit tests.
- **06 — the fork**: 1×1 direction combiner, `double_three_map`; the
  naive oracle; differential gate (≥ 1000 boards); edge and overline
  cases.
- **07 — seeing it**: threat potential as sparse policy; the heatmap
  demo binary on showcase positions (the classic fork and the corner
  fork from the engine's own tests); the GPU section — same network,
  same weights, `Wgpu` backend, with the CPU-vs-GPU equality test
  (opt-in `gpu` feature).
- **08 — acceptance**: full gates at scale, showcase map assertions,
  crate polish (`#![deny(missing_docs)]`), workspace gates green.

## 7. Acceptance

1. `win_map` == `immediate_wins` and `double_threat_map` ==
   `double_threats` on ≥ 1000 seeded boards each.
2. `double_three_map` == naive oracle on ≥ 1000 seeded boards,
   including boards with edge threes and overlines.
3. Naive cross-correlation == Burn `conv2d` on randomized tensors.
4. Showcase positions produce exactly the documented cell sets
   (regression-locked).
5. `patterndemo` renders the showcase heatmaps.
6. Workspace gates: `cargo test --workspace`,
   `cargo clippy --all-targets -- -D warnings`,
   `cargo fmt --all -- --check`; `cargo check -p engine` stays
   Burn-free.
7. GPU portability: `cargo test -p patterns --features gpu` passes on
   a GPU-capable machine (documented as environment-dependent and
   excluded from the default gates).

## 8. What this is for, later

Motivation only — none of this is built in the tutorial:

- The milestone-3 network's first conv layer will *learn* kernels;
  this tutorial makes concrete what that means and what such kernels
  can express.
- A frozen hand-written first layer under a learned network (the
  DeepGomoku pattern) becomes an option the learner can evaluate.
- `potential.rs` has the `Evaluator`-compatible shape, so an mcts
  prior adapter is a thin future wrapper.

## 9. Risks

1. **Kernel-enumeration completeness** (the main risk). The pattern
   table must cover every window arrangement the oracle recognizes —
   including edge cases, anti-four margins, and overlines. Mitigation:
   a Task-0 *spike* before chapter writing begins: implement the
   pattern table, the naive oracle, and the differential gate first;
   iterate until the gates pass at scale; pin the channel count. The
   tutorial chapters then present the verified semantics
   pedagogically (same discipline as datagen: chapters describe
   verified code).
2. **Burn 0.21 manual weight assignment.** The exact idiom
   (`Param::from_tensor`, module field types) must be checked against
   the pinned `v0.21.0` source — hard rule 1 — on both the `NdArray`
   and `Wgpu` backends. Part of the spike.
3. **Semantic drift from the engine.** Where the tutorial claims
   equality with `double_threats`, it must reproduce the engine's
   documented simplifications exactly. The gate enforces it; the
   primer documents it.
4. **Scope creep** into threat sequences, TSS, or renju. Guarded by
   the non-goals; reviewers reject additions.
5. **Performance.** ~100 channels of 9×9 conv on 23×23 planes, CPU:
   negligible. The oracle is O(empties × windows) per board: fine for
   gate-scale board counts. Not a risk in practice; listed for
   completeness.

## 10. Workflow and deliverables

Same pipeline as the datagen tutorial:

1. This spec, reviewed by the owner, committed to `main`.
2. An implementation plan (`docs/plans/`), reviewed by the owner.
3. Reference implementation in a disposable git worktree on
   `reference/convolutions`, built slice by slice with visible
   subagents (implement → spec review → quality review → amendments →
   re-review; controller commits). The branch is deleted after final
   verification, per the convention set after the datagen tutorial —
   the tutorial's solution files are the durable record.
4. Tutorial docs land on `main` per slice; no pushes without the
   owner's request; WARM-UP/AGENTS status updates remain a
   conversation with the owner.

## References

- `docs/12-gomoku-architecture.md` — §3 alpha-epsilon philosophy,
  determinism policy, dependency rules.
- `docs/13-engine-design.md` — tactics semantics and their documented
  simplifications.
- `docs/tutorials/mcts-tutorial/00-mcts-primer.md` — tutorial format
  precedent.
- `docs/specs/2026-09-27-datagen-tutorial-design.md` — workflow
  precedent (spec → plan → subagent slices).
- `gomoku/crates/engine/src/tactics.rs` — the four-level oracle.
- Burn 0.21.0 pinned sources — conv2d API authority (hard rule 1).
