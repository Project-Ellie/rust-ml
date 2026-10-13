# Baseline primer — a hand-written evaluator inside MCTS

## Abstract

This primer is the design authority for the baseline tutorial: a side
quest between the convolutions tutorial and the net tutorial in which
you build the project's first intelligent actor — a **hand-written
policy/value evaluator** that plugs into the existing PUCT search
through the `mcts::eval::Evaluator` seam and is playable from the
`gomoku` terminal client. The evaluator reuses the convolutions
tutorial's threat network verbatim, adds one small new piece — a
**gravity kernel** that rewards playing near existing stones — and
combines both sides' threat maps plus the gravity map into two
**readouts**: a 225-vector of policy logits built from a derived tier
table, and a saturating value heuristic clamped away from ±1. Playing
strength cannot be verified on paper, so acceptance is by **measured
hypothesis**: pre-registered bars, a fixed protocol, recorded numbers.
The player is almost a by-product; the lasting artifacts are the Elo
anchor, the adapter shape the trained network will reuse, and the
first meeting of `engine`, `mcts`, and `patterns` in one crate. The
chapters derive the details; this primer pins the semantics they must
not drift from.

## Glossary

- **Baseline player** — the hand-written evaluator of this tutorial,
  wrapped in MCTS, taken as one whole agent. "Baseline" because every
  later trained network is measured against it.
- **Evaluation seam** — the single trait that is the only coupling
  between the `mcts` crate and any network or heuristic:
  `mcts::eval::Evaluator`. The search asks "what is fθ(s)?" — nothing
  else.
- **Mock evaluator / `TacticsEvaluator`** — the deterministic
  evaluator shipped with the `mcts` crate (`mcts::mock`). It shapes
  priors from engine truth (`immediate_wins`, `forced_blocks`) and
  returns crude sentinel values (+0.95 / −0.90 / 0.0), documented in
  its own source as tutorial scaffolding. The baseline replaces it as
  the project's default evaluator and meets it in the acceptance
  match.
- **Threat map** — a 15×15 map over empty cells scoring "what happens
  if this side places a stone here" (convolutions tutorial).
- **`ThreatMaps`** — the convolutions tutorial's output struct:
  `wins`, `double_threats`, `double_threes`, `threes_per_dir`,
  `fours_created_per_dir`, all `[[f32; 15]; 15]` (the per-direction
  maps `[[[f32; 15]; 15]; 4]`, direction order horizontal, vertical,
  diagonal down-right, diagonal up-right).
- **Five-completer / immediate win** — an empty cell where placing a
  stone completes five in a row. Engine: `immediate_wins`.
- **Forced block** — a cell the side to move must play because the
  *opponent* completes five there. Engine: `forced_blocks`. Exactly
  the opponent's win map.
- **Double threat** — an empty cell where placing a stone leaves the
  side with two or more five-completing cells; unanswerable next move.
  Engine: `double_threats`, with its documented v1 simplifications
  (four-three blind spot, opponent-wins-first, immediate wins
  excluded).
- **Three-maker** — an empty cell where placing a stone creates an
  open three along a direction (`ThreatMaps::threes_per_dir`).
- **Double-three fork** — an empty cell where placing a stone creates
  open threes in two or more distinct directions
  (`ThreatMaps::double_threes`).
- **Gravity kernel** — a 5×5 convolution kernel with positive weights
  decaying by distance from the centre and a zero centre cell, slid
  over the occupancy plane. It scores every cell by how much stone
  mass sits nearby.
- **Occupancy plane** — a one-channel plane: the 15×15 board
  zero-padded by the kernel radius to 19×19, no border ring.
- **Gravity map** — the gravity kernel's output, normalized into
  [0, 1]: high near existing stones, zero on an empty board.
- **Readout** — hand-written glue that turns maps into the evaluator's
  two outputs: the *policy readout* (a 225-vector of logits) and the
  *value readout* (a scalar in [−1, 1]).
- **Tier / weight hierarchy** — the policy readout's structure: each
  cell belongs to each tier or not (binary membership), and every
  tier's logit weight exceeds the maximum possible sum of all tiers
  below it.
- **Logit** — an unnormalized log-probability. `EvalResult::policy`
  carries logits, not probabilities; the `mcts` crate's
  `masked_softmax` normalizes them over the legal moves.
- **Masked softmax** — `mcts::eval::masked_softmax`: softmax over the
  legal (empty) cells only, computed in f64 with a mask-first guard
  and a degenerate-uniform fallback. Expansion applies it; evaluators
  never normalize themselves.
- **Saturating heuristic** — a value function that rises with
  advantage but is deliberately clamped away from ±1, because ±1 is
  reserved for proven terminal outcomes.
- **Elo anchor** — a fixed reference opponent with a measured match
  record. Once the `arena` crate exists, the trained network's
  strength is expressed relative to this baseline.
- **Exactness principle** — the project's load-bearing convention
  (convolutions tutorial): all hand-written weights and inputs are
  small integers or dyadic fractions, so every f32 sum is exact on
  every backend and comparisons can use `==`.
- **Measured hypothesis** — a falsifiable numeric claim about behavior
  (not code), pre-registered with its protocol, then measured and
  recorded as a real number — never asserted on faith, never invented.
- **Flex device** — `burn::tensor::Device::flex()`, the pure-Rust CPU
  backend the `patterns` crate builds on by default; the GPU path is
  an opt-in `gpu` feature.
- **Side quest** — a tutorial outside the milestone sequence (the CLI
  and convolutions tutorials are the precedents). It adds teaching
  value and de-risks later work; it is not on the critical path.

## 1. Why a hand-written player

The project philosophy is alpha-epsilon: hand-woven tactics give the
agent a head start that learning later refines (docs/12, §3). Three
milestones' worth of machinery now exists, and **it has never met in
one artifact**:

- the `engine` computes tactics as bitboard truth — every answer a
  certainty;
- the `mcts` crate runs full PUCT search behind the `Evaluator` seam,
  but its only non-trivial evaluator is the `TacticsEvaluator` mock,
  whose own source calls it tutorial scaffolding, "not meant to
  generalize";
- the `patterns` crate reproduces the engine's tactics as a
  hand-written convolutional network and goes beyond it (double-three
  forks), proven equal to independent oracles on a thousand boards.

Between "threat maps proven correct" and "a trained network inside
search" there is no *player* — nothing that uses the maps to actually
choose moves, nothing a human can sit down and beat, and nothing the
future arena can anchor Elo to. The datagen tutorial's synthetic data
is shaped by the same crude 90/10 tactics shaping the mock uses; a
stronger hand-written player produces richer self-play positions than
the mock ever will.

This tutorial builds that player. Its three payoffs, in order of
durability:

1. **The Elo anchor.** Once the `arena` crate exists, the trained
   network's strength is expressed against this fixed, reproducible
   opponent. The acceptance match score is the anchor's seed datum.
2. **The adapter shape.** `engine::Planes` in → reconstructed `Board`
   → `analyze_both` → maps + gravity → `EvalResult` out is the same
   shape the trained `net` crate's evaluator adapter will later have.
   This tutorial is the dry run.
3. **Better self-play data, later.** Re-running datagen with the
   baseline evaluator yields positions shaped by gravity and
   three-level threats instead of 90/10 shaping. That re-run is
   future work; this tutorial does not do it.

## 2. The trust ladder

The policy readout's inputs are not equally trustworthy, and the
design exploits that:

```text
certainty ──────────────────────────────────────────► heuristic

own wins            engine-proven   (convolutions differential gate,
opponent's wins     engine-proven    1000 boards × 2 colors)
own double threats  engine-proven   (same gate)
three-makers        oracle-proven   (naive three-enumerator, 1000×2)
gravity             unproven heuristic — sits at the bottom,
                    where it can do the least harm
```

The top tiers are *oracular*: the convolutions tutorial's differential
gates prove the win maps equal `engine::immediate_wins` and the
double-threat map equals `engine::double_threats`. The three-level
tiers are oracle-proven against the naive enumerator. Gravity alone is
an unproven heuristic — so the tier hierarchy is also a **trust
ladder**: certainty at the top, increasingly heuristic downward, and
the least trusted signal structurally unable to outvote any trusted
one (§4 makes "unable" precise).

Where the oracular tiers inherit the engine's documented v1
simplifications — the four-three blind spot, opponent-wins-first,
immediate wins excluded from double threats — the readout inherits
them too. The primer names them so the chapters never pretend
otherwise.

## 3. The gravity idea

When no threat is present — the common case in the opening and early
midgame — the threat maps are all zero and every legal cell looks
alike. The mock answers this case with a uniform prior, so the search
spreads its 400 simulations near-randomly across 200+ cells. Gomoku is
a local game: almost every good early move is near existing stones.

The **gravity kernel** encodes exactly that, and nothing more: a 5×5
kernel, centre cell exactly 0, ring weights decaying by Chebyshev
distance (2 for the 8 neighbours at distance 1, 1 for the 16 cells at
distance 2), slid over a dedicated **occupancy plane** — 1 where any
stone of either color sits, 0 elsewhere, zero-padded with **no border
ring**. The maximum raw output is 8·2 + 16·1 = 32, and the readout
normalizes by 32 — a power of two, so the normalized gravity map
g(r, c) ∈ [0, 1] stays exactly representable in f32 (the exactness
principle). These starting weights are an `[experiment]` value: any
monotone decay works; the tutorial pins this table and records it as a
choice, not a law.

Two properties are deliberate and load-bearing:

- **The centre is zero** for the same reason the convolutions kernel
  language never conditions on the candidate cell itself: a kernel is
  a picture of a *neighborhood* condition, and "there is a stone
  here" is not a neighborhood condition (occupied cells are illegal
  moves anyway).
- **The existing two-channel planes are not reused.** Channel 0 (own
  stones only) would make gravity color-biased; channel 1 (opponent
  stones *or* the border frame) would pull toward the board edges —
  the border ring is all-ones, so a corner cell would score phantom
  "neighbor mass" from outside the board. Chapter 1 shows the failure
  as arithmetic on a tiny grid, builds the dedicated plane in glue,
  and regression-tests that a corner stone's map is *truncated, never
  boosted*.

Gravity is its own small conv branch — one `Conv2d`, 5×5, one channel
in, one out, built in the open in the `baseline` crate — not a surgery
on `patterns`. Every future hand-written heuristic is another small
named branch (the DeepGomoku frozen-features pattern); the proven
threat network is never touched.

## 4. The tier derivation

The policy readout's weights are **derived, not tuned**. One question
forces the whole design:

> **What breaks if gravity ever outvotes a forced block?**

Answer: the side fails to block the opponent's immediate five and
loses on the spot. So a forced-block cell must win the softmax against
*every* cell that is merely gravity-attractive — no matter how
attractive, and no matter how many such cells compete. Softmax mass
ratios are exponentials of logit gaps, and the worst case is bounded
(at most 224 competing legal cells, gravity ≤ 1 after normalization):
a fixed logit gap that exceeds the maximum possible sum of *all*
lower-tier contributions makes a lower tier unable to outvote a higher
one even by stacking.

Generalize from that one pair to every adjacent pair and the rule
falls out: **each tier's weight must exceed the maximum possible sum
of all tiers below it.** Binary membership per tier (a cell is in a
tier or not — no per-direction stacking within a tier) keeps the sums
small and obvious, and choosing powers of two keeps every reachable
logit an exact f32 sum. The resulting table:

| Tier | Cell membership (maps for side to move `s`, opponent `o`) | Logit |
|------|------------------------------------------------------------|-------|
| T1 | `s`'s `wins` — play now | +64 |
| T2 | `o`'s `wins` — the forced blocks | +32 |
| T3 | `s`'s `double_threats` — unblockable next move | +16 |
| T4a | `s`'s `double_threes` | +8 |
| T4b | any of `s`'s `threes_per_dir[d]` | +4 |
| T4c | any of `o`'s `threes_per_dir[d]` or `o`'s `double_threes` (defensive) | +2 |
| T5 | gravity, normalized | +g(r, c) ∈ [0, 1] |

The domination invariant, checkable by inspection:
8 + 4 + 2 + 1 = 15 < 16; 16 + 15 = 31 < 32; 32 + 31 = 63 < 64.

"Orders of magnitude between tiers" is then a theorem about the table,
in two parts. **Argmax safety**: no stacking of lower tiers can reach
the next tier's weight, so a cell in the highest occupied tier always
holds the strict logit maximum — at temperature 0 the right move is
always there to be picked. **Softmax mass**: a lone forced-block cell
outweighs every cell that is not itself a forced block or a double
threat (maximum lower total 15, gap ≥ 32 − 15 = 17) by e¹⁷ ≈ 2 × 10⁷
per cell — even against 224 such competitors, the miss probability
sits below 10⁻³. Against a cell that *also* carries the double-threat
tier the guaranteed margin shrinks to the invariant's minimum of 1 —
and there the tutorial makes the game-theoretic point: blocking and
claiming an unblockable threat are both defensible, so a near
coin-flip between them is acceptable, and the value readout plus
search adjudicate.

Two deliberate absences:

- **The opponent's double threats are not a policy tier.** A double
  threat is by definition unanswerable; spending prior mass on
  "defense" against it is waste. It belongs to the value readout,
  where it correctly makes the position look lost.
- **The opponent's wins outrank everything except playing our own win
  now** (T1 > T2): if we complete five, the game ends before the
  opponent's threat matters — the engine's tactics module documents
  the same partition (win now / must block / unblockable threat).

The readout writes a full `[f32; 225]` logit vector: tier sums at
empty cells, `f32::NEG_INFINITY` at occupied cells (matching the
mock's masked style). `masked_softmax` is the only softmax; the
evaluator never normalizes.

## 5. The honest value heuristic

The value readout is a **saturating heuristic** from the same maps,
from the side-to-move's perspective, shaped like the mock's sentinels
and extended down the threat ladder:

1. `s` has any immediate win ⇒ **+0.95** (about to win; the mock's
   sentinel, kept).
2. `o` has two or more immediate wins ⇒ **−0.99** (one block cannot
   cover two finishing cells; effectively lost).
3. Otherwise accumulate, then clamp to [−0.99, +0.99]: `o` has exactly
   one immediate win ⇒ −0.75; `s` has any double threat ⇒ +0.85;
   `o` has any double threat ⇒ −0.85; `s` has any double-three
   ⇒ +0.20; `o` has any ⇒ −0.20.

The constants are `[experiment]` starting values, tuned during
acceptance measurement. The *properties* are pinned, and the tests
assert the properties, not the constants:

- **Perspective and sign** — value from the side to move, in [−1, 1],
  matching `EvalResult::value`'s contract and the search's
  sign-flipped backup.
- **Saturation** — |v| ≤ 0.99 always; ±1 is reserved for terminals,
  which the search values exactly without calling the evaluator.
- **Monotonicity** — adding own threats never decreases v; adding
  opponent threats never increases it.
- **Quiet board ⇒ exactly 0.0** — gravity never enters the value; a
  position with no maps firing is honestly neutral.
- **Honesty** — this is a heuristic with named failure modes: it
  cannot see threat *sequences* (the engine's TSS prover owns those);
  it inherits the engine's v1 simplifications through the oracular
  maps; it is the mock's scaffolding made one notch less crude. The
  tutorial says so everywhere, and the trained network's value head is
  what replaces it.

## 6. The adapter, end to end

The evaluator is a thin adapter with no state of its own beyond two
modules (the threat network and the gravity branch):

```text
EvalRequest { planes, legal }            (the mcts seam speaks Planes)
        │
        ▼  reconstruct (parity + legal-set assert)
engine::Board                            (absolute colors, docs/13)
        │
        ▼  patterns::net::analyze_both   (one batched forward pass)
[ThreatMaps; 2]  ── pick own/opponent by board.to_move()
        │
        ▼  gravity branch on the occupancy plane
gravity map ∈ [0, 1]
        │
        ▼  policy readout (§4)  +  value readout (§5)
EvalResult { policy: [f32; 225], value: f32 }
```

Three invariants make it trustworthy:

- **Determinism.** No RNG anywhere in evaluation. Match-play variety
  comes from the search side (temperature sampling in
  `mcts::policy`), never from the evaluator — so seeded matches are
  reproducible.
- **Terminals never reach it.** `mcts::expand` checks
  `board.status()` first and returns exact values for won and drawn
  positions, so the heuristic's saturation never collides with proven
  outcomes, and ±1 stays reserved for truth.
- **The planes round-trip.** Reconstruction from the 17×17
  side-to-move-relative `me`/`you` planes uses stone-count parity
  (Black moves first, so equal counts mean Black to move) and asserts
  the supplied legal set equals the reconstructed board's — the same
  approach as the mock's private `reconstruct_board`, carried as a
  small documented copy (deliberate duplication; the precedent is set
  in the convolutions design spec: `patterns` duplicating datagen's
  playout generator rather than adding a dependency). During normal
  search every evaluated position is alternation-legal, so parity
  reconstruction is sound.

## 7. Acceptance by measurement

Correctness is asserted; **strength is measured**. The acceptance
chapter keeps the two scrupulously separate:

- **Asserts** (certainties, verified every run): tactical soundness on
  the mcts acceptance boards (win-in-1 and the forced block, each at
  50 simulations), a 100-game seeded self-play health flood (every
  move legal, every game ≤ 225 plies, no panics), the derivation
  checks (the domination invariant, the pinned value properties), and
  workspace hygiene (the engine stays a Burn-free island; `mcts` stays
  Burn-free).
- **Measured hypotheses** (strength, recorded as real numbers): a
  200-game seeded match against the `TacticsEvaluator` with a
  pre-registered win-rate bar (tuned on the first real run, with both
  numbers recorded); a threat-construction statistic (double-threat
  creations per game, baseline self-play vs mock self-play — the
  qualitative claim "gravity plus three-makers produce richer
  positions" made falsifiable); and throughput numbers (seconds per
  game, evaluations per second) recorded as context for the self-play
  planning in docs/12, with no bar attached.

The habit is the point: write the bar down *before* running the
protocol, then record the real number *whatever it is*.

## 8. What this tutorial defers

- Training, autodiff, and any change to the datagen pipeline — the net
  tutorial; the datagen re-run is future work.
- Elo rigor and the `arena` crate — the match records a raw score as
  the anchor's seed datum, nothing more.
- Threat-space search coupling and threat sequencing beyond the static
  maps — the engine's TSS prover owns that (TSS-inside-MCTS is a
  registered upgrade in docs/12).
- Concurrency — single-threaded, matching the `mcts` crate's design;
  no tokio anywhere (docs/12 §5, locked).
- GPU as a requirement — `Device::flex()` is the default and the
  acceptance runs on CPU; a `gpu` feature mirroring `patterns` is an
  opt-in extra.
- Renju forbidden-move rules — freestyle only, consistent with the
  engine.
- Changes to `engine`, `mcts`, or `patterns` public APIs — they are
  consumed as-is.

## 9. Common bugs (the ones the chapters' tests are built to catch)

- **Border pull**: reusing the two-channel pattern planes for gravity.
  The blocked channel's all-ones border ring makes edge and corner
  cells score phantom neighbor mass. The occupancy plane has no
  border; chapter 1's corner test is the regression net.
- **Color-biased gravity**: scoring proximity to *own* stones only.
  Proximity is color-blind; the occupancy plane holds either color.
- **Per-direction stacking within a tier**: letting a cell that makes
  threes in two directions collect the three-tier weight twice.
  Membership is binary; the domination invariant's proof depends on
  it.
- **Normalizing the policy yourself**: `EvalResult::policy` carries
  logits. `masked_softmax` — mask-first, f64 accumulator — is the only
  softmax; double-normalizing distorts the priors the search expects.
- **Epsilon comparisons**: every weight is a small integer or dyadic
  fraction, so every logit and every gravity value is an exact f32
  sum. Reaching for `approx` here hides real bugs; assert equality.
- **Evaluating terminals**: the search never calls the evaluator on
  won or drawn positions; if your adapter sees one, the bug is
  upstream. Do not add a terminal branch to the evaluator "just in
  case" — it would mask the real bug.
- **Value from the wrong perspective**: `EvalResult::value` is from
  the side to move, and backup flips the sign per ply. An absolute
  "good for Black" value quietly inverts the search.
- **RNG in the evaluator**: variety belongs to the search side
  (temperature in `mcts::policy`). An evaluator that rolls dice makes
  matches irreproducible and the acceptance numbers meaningless.

## Summary

The baseline player is three proven parts and one honest heuristic,
assembled behind a seam that already exists. The threat maps bring
engine-proven tactics and oracle-proven three-detection; gravity
brings a sane quiet-board default that knows its place at the bottom
of the trust ladder; the tier table is a theorem, not a tuning; the
value is a clearly labelled saturating heuristic. Wrapped in the
existing PUCT search, the result is the project's first beatable
opponent, its first Elo anchor, and the adapter shape everything after
will reuse. The chapters derive the details; this primer has pinned
what they must not drift from.

## References

- [`../../specs/2026-10-13-baseline-player-design.md`](../../specs/2026-10-13-baseline-player-design.md)
  — the design spec (gravity §3, tiers §4, value §5, crate topology
  §6, adapter §7, CLI §8, acceptance §10).
- [`../convolutions-tutorial/00-convolutions-primer.md`](../convolutions-tutorial/00-convolutions-primer.md)
  — the threat ladder, the exactness principle, and the oracle
  ladder.
- [`../convolutions-tutorial/03-the-kernel-language.md`](../convolutions-tutorial/03-the-kernel-language.md)
  — its notation section pins the `X`/`O`/`_`/`c` board notation the
  tutorial reuses.
- [`../convolutions-tutorial/04-the-four-level/01-solution.md`](../convolutions-tutorial/04-the-four-level/01-solution.md)
  — the normative final form of `patterns::net`: `ThreatNet`,
  `ThreatMaps`, `analyze`, `analyze_both`.
- `gomoku/crates/mcts/src/eval.rs` — the `Evaluator` seam and
  `masked_softmax`; `mock.rs` — the opponent the baseline replaces and
  meets; `expand.rs` — the terminal check that keeps ±1 reserved.
- [`../../12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  alpha-epsilon (§3), no-tokio (§5), the `Evaluator` rationale (§6).
- [`../../13-engine-design.md`](../../13-engine-design.md) — tactics
  simplifications and absolute colors.
