# Convolutions primer — hand-written kernels for Gomoku threat maps

## Abstract

This primer is the design authority for the convolutions tutorial: a
side quest between the datagen tutorial and the net tutorial in which
you build a small convolutional network whose weights are written by
hand, not trained. The network reads a Gomoku position and produces
*threat maps* — spatial scores for every empty cell answering "what
happens if we play here?" — culminating in the double-three fork map,
which marks moves that create two open threes at once. Every map is
proven correct by differential testing against an independent oracle.
There is no training loop, no GPU requirement, and nothing to wait
for: the network is right by construction, and you can watch it work
from the first chapter. The chapters derive the details; this primer
pins the semantics they must not drift from.

## Glossary

- **Threat map** — a 15×15 score map over the board's empty cells:
  what happens if the given side places a stone there.
- **Kernel** — a small weight matrix slid across the input planes; the
  elementary pattern detector of a convolutional network.
- **Channel / feature map** — one plane of a tensor; a conv layer's
  output has one channel per kernel, read as a spatial map.
- **Cross-correlation** — the sliding weighted sum deep learning calls
  "convolution" (no kernel flip; the tutorial uses the deep-learning
  convention).
- **Five-completer / immediate win** — an empty cell where playing
  completes five in a row. Engine: `immediate_wins`.
- **Open four / broken four** — four in a line with both ends open
  (two five-completing cells) / four stones in a five-cell window with
  one gap (one completing cell).
- **Double threat** — a move leaving the side with ≥ 2 five-completing
  cells. Engine: `double_threats`.
- **Open three** — the v1 definition: one of the exact windows
  `_XXX_`, `_X_XX_`, `_XX_X_` (both ends empty, no fourth own stone
  adjacent).
- **Double-three fork** — a move creating open threes in ≥ 2 distinct
  directions. The tutorial's headline map.
- **Oracle** — a trusted, independent implementation used as ground
  truth in a differential test.

## 1. Why hand-written kernels

The project philosophy is alpha-epsilon: hand-woven tactics give the
agent a head start that learning later refines (docs/12, §3). The
engine computes tactics as bitboard truth; this tutorial asks a
different question: *can a convolutional network express the same
truth, and what does that teach us?*

The pedagogical bet: a hand-written kernel makes convolution legible.
A filter that fires on `_XXX_` is an object you can hold in your head
— the weight pattern *is* the concept "open three". You learn conv,
channels, padding, and feed-forward with deterministic, debuggable
behavior and zero training infrastructure. And there is a second,
strategic payoff: the net tutorial will introduce Burn's tensors and
conv layers while *also* teaching training. This tutorial front-loads
the machinery, so that tutorial can focus on the learning.

It adds no game capability the engine lacks — it is a side quest, like
the CLI tutorial. Its deliverables are understanding, a reusable
threat-map component, and a de-risked path into the net tutorial.

## 2. The threat ladder — the semantics that must not drift

All maps are defined for a side `s` on a non-terminal board, at empty
cells, answering "what if `s` plays here?"

1. **Win map** — playing here completes five (overlines count,
   freestyle). Exactly `engine::immediate_wins`.
2. **Double-threat map** — after playing here, the side has ≥ 2
   five-completing cells *in total* (an open four contributes two; a
   broken four one; pre-existing completing cells persist). Immediate
   wins themselves are excluded. Exactly `engine::double_threats`,
   including its documented simplifications (four-three blind spot,
   opponent-wins-first, wins excluded).
3. **Three-maker maps, per direction** — playing here creates an open
   three along that direction, including the new stone: one of
   `_XXX_`, `_X_XX_`, `_XX_X_` with both window ends genuinely empty
   and no fourth own stone immediately beyond either end (the
   anti-four margin).
4. **Double-three map** — three-makers fire in ≥ 2 **distinct**
   directions. Two threes along the same line are one three.

**Documented v1 simplification** (engine-tactics style): open threes
are not required to have *maturation room* — a three that cannot
actually become an open four because the edge or an opponent stone
sits one cell further out still counts. The windows stay small and
the definition stays local. Refining this is a chapter-5 exercise.

## 3. The kernel language

Every pattern above reduces to a local condition: "these cells hold
our stones, those cells are empty, the rest don't care." Each such
condition is one conv output channel:

- `+1` on required-stone cells in the stones channel,
- `−1` on required-empty cells in **both** channels (an own stone, an
  opponent stone, or the border — via the blocked channel — all
  violate emptiness),
- `−1` on anti-margin cells in the stones channel only (must not hold
  an *own* stone; opponent or border is fine),
- `0` — don't care — everywhere else,
- bias `1 − n_stones`, so an exact match yields pre-activation exactly
  `1` and any single violation drops it to `0` or below; ReLU emits a
  clean `1.0` / `0.0`.

Input planes are two 25×25 channels (15×15 board + padding 5):
**stones** (zero-padded) and **blocked** = opponent stones *or border*
(one-padded). The border-as-blocked trick makes edge behavior fall out
of the patterns — a required-empty cell off the board is not empty,
which is exactly right. Burn's built-in padding only zero-pads, so the
padding is materialized in `planes.rs` and the conv uses valid
padding.

All kernels share a uniform 11×11 footprint (radius 5: the anti-four
margin of a six-cell broken-three window sits at distance 5 from the
candidate cell in the extreme positions). Kernels are **generated from
a declarative pattern table** — shapes × directions × offsets — not
hand-drawn; the table is the artifact under review. The table holds
152 layer-1 channels: 20 five-completers, 16 open-four makers, 80
broken-four makers, 36 three-makers (3 families × 3 shapes × 4
directions).

## 4. The architecture, honestly

The network is two conv layers plus a little glue, and the tutorial
says so:

- **Layer 1**: 11×11 conv, 2 → 152 channels, valid padding, ReLU.
  Every pattern kernel, every direction, every cell.
- **Layer 2**: 1×1 conv, 152 → 9 channels, ReLU. Per-cell
  cross-channel logic: per-direction OR-combines, and *counting* — the
  open-four channels carry weight 2 (two completing cells), the
  broken-four channels weight 1. The 1×1 convolution as cross-channel
  combiner, and channels carrying counts rather than bits, are two of
  the tutorial's explicit teaching points.
- **The glue** (a few honest lines of code, not a layer): the engine
  counts *unique* completing cells, including ones that already
  existed before the move. So the double-threat map is
  `|win_set ∪ created_set(c)| ≥ 2` with immediate wins excluded —
  implemented with a `before` scalar (`immediate_wins.len()`) and a
  per-direction dedup of newly created completing cells. Occupied
  cells are zeroed. This glue exists because convolutions see local
  patterns, and "unique cells, globally counted" is not a local
  property.

## 5. The oracle ladder

Nothing in this tutorial is trusted because it looks right. Two
oracle tiers, two independent implementations of each definition:

1. **Four-level and below: the engine.** `wins == immediate_wins` and
   `double_threats == engine::double_threats` on ≥ 1000 seeded random
   boards, both colors. The engine's documented simplifications are
   reproduced exactly — the gate enforces it.
2. **Three-level: the naive oracle.** The engine has no open-three
   concept, so the crate carries a deliberately slow, obviously
   correct enumerator (plain array scans, no bitboards, no conv)
   implementing §2's definition directly. It is unit-tested on
   hand-written ASCII boards, then `double_threes == oracle` is gated
   on ≥ 1000 seeded boards, both colors. A discrepancy localizes the
   bug to kernel enumeration or oracle — which is the entire point of
   writing two implementations.

Random boards come from a small seeded generator (random legal
playouts, stopped before terminal). Deterministic seeds throughout.

## 6. The backend story

The crate is backend-generic (`B: Backend`) from the first line. The
default is `NdArray` (pure Rust, CPU): the gates must pass identically
on every machine, and the dependency tree stays light. An opt-in
`gpu` feature (`burn/wgpu`) runs the *same* network with the *same*
weights on GPU; `tests/gpu_port.rs` asserts exact CPU-vs-GPU equality.
Exactness is by construction: every input and weight is a small
integer, so f32 sums are exact on any backend — no epsilons anywhere
in this tutorial. The GPU section is not about speed (the workload is
microscopic); it is about making Burn's backend abstraction concrete
before the net tutorial trains on GPU.

## 7. Gate scheduling

The per-chapter gate is the default suite (`cargo test -p patterns`,
~1 minute in debug): all unit and ASCII tests plus the differential
gates at 100 boards. The full proof — 1000 boards × 2 colors — is
`#[ignore]`d and run explicitly, in **release** mode
(`cargo test -p patterns --release -- --ignored`, ~9 seconds on the
reference machine; ~620 seconds in debug, which is why the release
habit matters). The default gate uses a strict prefix of the full
proof's seed range.

## 8. What this tutorial defers

- Training, autodiff, loss functions — the net tutorial.
- Integration with `mcts` (a prior adapter) or `train` — noted as
  motivation only; `potential.rs` already speaks the
  `Vec<(Move, f32)>` shape.
- Maturation-room refinement of open threes — a chapter-5 exercise.
- Threat-space search and threat sequences — the engine's TSS prover
  owns that.
- Renju forbidden-move rules — freestyle only, consistent with the
  engine.

## 9. Common bugs (the ones the gates were built to catch)

- **Padding semantics**: zero-padding the blocked channel makes the
  board edge look empty — edge threes suddenly "open". Materialize the
  padding; border is blocked.
- **Counting windows instead of cells**: two broken-four windows can
  share one completing cell; the engine counts unique cells. Sum
  counts naively and the gate will find your mistake on a random
  board (it did — see the tutorial's SPIKE story).
- **Forgetting pre-existing wins**: with an open four already on the
  board, *every* legal move is a double threat. The `before` scalar is
  not optional.
- **Evaluating occupied cells**: kernels fire anywhere the pattern
  matches; the maps are defined on empty cells. Zero the rest.
- **Epsilon comparisons**: with integer inputs and weights, outputs
  are exact. Reaching for `approx` here hides real bugs; assert
  equality.
- **Same-direction double-counting**: a fork is ≥ 2 *distinct*
  directions; the per-direction channel structure exists to make that
  the natural reading.

## Summary

Hand-written kernels turn "the network learns features" from an act of
faith into an engineering fact: you wrote the features, one channel at
a time, and proved each one against an independent oracle at scale.
The threat ladder — wins, double threats, the fork — is the content;
convolution is the language; differential testing is the method. The
net tutorial will let the first layer learn. You will know what it is
trying to say.

## References

- [`../../specs/2026-09-27-convolutions-tutorial-design.md`](../../specs/2026-09-27-convolutions-tutorial-design.md)
  — the design spec (map semantics, kernel language, acceptance bar).
- [`../../plans/2026-09-27-convolutions-tutorial.md`](../../plans/2026-09-27-convolutions-tutorial.md)
  — the implementation plan and its amendments.
- `gomoku/crates/engine/src/tactics.rs` — the four-level oracle (repo
  root relative).
- [`../../12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  alpha-epsilon philosophy (§3), determinism policy, dependency rules.
- [`../mcts-tutorial/00-mcts-primer.md`](../mcts-tutorial/00-mcts-primer.md)
  — tutorial format precedent.
