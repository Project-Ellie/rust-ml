# Baseline player tutorial design — a hand-written evaluator inside MCTS

**Status:** Approved 2026-10-13.

## Abstract

This document specifies a side-quest tutorial that sits after the
convolutions tutorial (which ends at its chapter-8 acceptance) and
before the net tutorial gets worked. The learner builds a new crate,
`gomoku/crates/baseline`, containing the project's first intelligent
actor: a **hand-written policy/value evaluator** that plugs into the
existing MCTS through its `Evaluator` seam and is playable from the
`gomoku` terminal client. The evaluator reuses the convolutions
tutorial's threat network (`ThreatNet`, `analyze_both`, `ThreatMaps`)
verbatim, adds one small new piece — a **gravity kernel** that rewards
playing near existing stones — and combines both sides' threat maps
plus the gravity map into policy logits and a saturating value
heuristic. Playing strength cannot be verified on paper, so the
acceptance chapter specifies **measured hypotheses** (win rate against
the tactics-shaped mock evaluator over seeded self-play games, with
the bar pre-registered and tuned on the first real run) rather than
strength asserts. The tutorial's lasting value is not the player
itself: it is the Elo anchor the trained network will later be
measured against, better self-play data than the mock evaluator
produces, and the first artifact where `engine`, `mcts`, and
`patterns` meet.

A glossary follows; the body defines every term before use.

## Glossary

- **Baseline player** — the hand-written evaluator of this tutorial,
  wrapped in MCTS, taken as one whole agent. "Baseline" because every
  later trained network is measured against it.
- **Evaluation seam** — the single trait that is the only coupling
  between the `mcts` crate and any network or heuristic (docs/12 §6):
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
  if this side places a stone here" (convolutions tutorial). The
  engine-proven levels are wins and double threats; the oracle-proven
  level is double threes.
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
  (docs/13).
- **Three-maker** — an empty cell where placing a stone creates an
  open three along a direction (the convolutions tutorial's
  per-direction `threes_per_dir` maps).
- **Double-three fork** — an empty cell where placing a stone creates
  open threes in two or more distinct directions
  (`ThreatMaps::double_threes`). The beginning of a threat sequence.
- **The nine layer-2 channels** — the threat network's output channel
  layout: channel 0 = win map, channels 1..=4 = five-completing cells
  created per direction, channels 5..=8 = open-three makers per
  direction. `analyze_both` produces the derived `ThreatMaps` for both
  colors from one batched forward pass.
- **Gravity kernel** — a 5×5 convolution kernel with positive weights
  decaying by distance from the centre and a zero centre cell, slid
  over the occupancy plane. It scores every cell by how much stone
  mass sits nearby.
- **Occupancy plane** — a one-channel plane: the 15×15 board
  zero-padded by the kernel radius to 19×19, no border ring.
- **Gravity map** — the gravity kernel's output: a 15×15 map, high
  near existing stones, zero on an empty board.
- **Readout** — hand-written glue that turns maps into the evaluator's
  two outputs: the *policy readout* (a 225-vector of logits) and the
  *value readout* (a scalar in [−1, 1]).
- **Tier / weight hierarchy** — the policy readout's structure: each
  cell belongs to each tier (own wins, opponent wins, own double
  threats, three-makers, gravity) or not (binary membership), and
  every tier's logit weight exceeds the maximum possible sum of all
  tiers below it.
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
- **Side quest** — a tutorial outside the milestone sequence (the CLI
  and convolutions tutorials are the precedents). It adds teaching
  value and de-risks later work; it is not on the critical path.
- **Exactness principle** — the project's load-bearing convention
  (convolutions tutorial): all hand-written weights and inputs are
  small integers or dyadic fractions, so every f32 sum is exact on
  every backend and comparisons can use `==`.
- **Measured hypothesis** — a falsifiable numeric claim about behavior
  (not code), pre-registered with its protocol, then measured and
  recorded as a real number — never asserted on faith, never invented.
- **Flex device** — `burn::tensor::Device::flex()`, the pure-Rust CPU
  backend the `patterns` crate builds on by default (`burn` feature
  `flex`); the GPU path is an opt-in `gpu` feature.

## 1. Context

The project philosophy is alpha-epsilon (docs/12 §3): hand-woven
tactics give the agent a head start that learning later refines. Three
milestones' worth of machinery now exists and has never met in one
artifact:

- the `engine` computes tactics as bitboard truth (`immediate_wins`,
  `forced_blocks`, `double_threats` — every answer a certainty);
- the `mcts` crate runs full PUCT search behind the `Evaluator` seam,
  but its only non-trivial evaluator is the `TacticsEvaluator` mock,
  whose own source calls it "clearly marked tutorial scaffolding … not
  meant to generalize";
- the `patterns` crate reproduces the engine's tactics as a
  hand-written convolutional network and goes beyond it (double-three
  forks), proven equal to independent oracles on a thousand boards.

The pedagogical gap this tutorial fills: between "threat maps proven
correct" (convolutions chapter 8) and "a trained network inside
search" (the net tutorial and milestone 4+) there is no *player* —
nothing that uses the maps to actually choose moves, nothing a human
can sit down and beat, and nothing the future arena can anchor Elo
to. The datagen tutorial's synthetic data is shaped by the same crude
90/10 tactics shaping the mock uses; a stronger hand-written player
produces richer self-play positions than the mock ever will.

The tutorial is a **side quest** (like the CLI and convolutions
tutorials), positioned deliberately: after the convolutions tutorial's
chapter-8 acceptance (its final-form contracts are this tutorial's
foundation) and before the net tutorial gets worked (so the net
tutorial inherits a motivation — something concrete to beat — rather
than training into a vacuum).

**Caveat on ground truth.** At the time of writing, the `patterns`
crate on `main` is mid-tutorial (it has `planes`, the full 152-channel
kernel table, both conv layers, `ThreatNet`, and `win_map`); the
`ThreatMaps`/`analyze`/`analyze_both` glue lands when the owner works
convolutions chapters 4–8. This spec therefore pins itself to the
**final-form contracts** quoted in the convolutions tutorial's
verified solution files — normatively
`docs/tutorials/convolutions-tutorial/04-the-four-level/01-solution.md`
— not to the crate's current intermediate state. §13 ("Verify when
reached") operationalizes this; §12 lists the drift risk.

## 2. Goals and non-goals

### Goals

1. Assemble the project's first intelligent actor: a hand-written
   evaluator behind the existing `Evaluator` seam, driving the
   existing single-threaded PUCT search, playable from the `gomoku`
   CLI against a human.
2. Teach the gravity kernel as the sane default: when no threat is
   present, prefer playing near existing stones instead of the
   near-random preferences a uniform or mock-shaped prior gives MCTS.
3. Teach the tier hierarchy as a *derived* artifact: the reader must
   be able to re-derive the policy weights from one question — "what
   breaks if gravity ever outvotes a forced block?"
4. Teach the value readout as an honest heuristic: saturating,
   side-to-move-relative, shaped like the mock's sentinels, extended
   down the threat ladder, and labelled as a heuristic everywhere.
5. Teach the adapter pattern end-to-end: `engine::Planes` in →
   reconstructed `Board` → `analyze_both` → maps + gravity →
   `EvalResult` out — the same shape the trained `net` crate's
   adapter will later have.
6. Establish the acceptance-by-measurement habit for *strength*
   claims: hypotheses, protocols, pre-registered bars, recorded
   numbers.

### Non-goals

- **No scientific contribution and no strength claims.** This is a
  baseline and a motivation engine. Every statement about playing
  strength is a measured hypothesis (§10), never a design-time claim.
- **No training, no autodiff, no changes to the datagen pipeline.**
  The "better self-play data" payoff (§11) is future work; this
  tutorial does not re-run datagen.
- **No changes to `engine`, `mcts`, or `patterns` public APIs.** They
  are consumed as-is. The engine stays a dependency island; `mcts`
  stays Burn-free by design (docs/12 §6).
- **No Elo rigor, no `arena` crate.** The match runner records a raw
  match score as the anchor's seed datum; real rating is the future
  arena's job.
- **No threat-space search coupling, no threat sequencing** beyond the
  static maps (the engine's TSS prover owns that; TSS-inside-MCTS is a
  registered upgrade in docs/12).
- **No concurrency.** Single-threaded, matching the `mcts` crate's
  design; no tokio anywhere (docs/12 §5, locked).
- **No GPU requirement.** `Device::flex()` is the default and the
  acceptance runs on CPU; a `gpu` feature mirroring `patterns` is an
  opt-in extra, not part of the default gates.
- **No renju forbidden-move rules**; freestyle Gomoku only, consistent
  with the engine.

## 3. The gravity kernel and the occupancy plane

### 3.1 The problem

When no threat is present — the common case in the opening and early
midgame — the maps are all zero and every legal cell looks alike. The
mock evaluator answers this case with a uniform prior, so the search
spreads its 400 simulations near-randomly across 200+ cells. Gomoku is
a local game: almost every good early move is near existing stones.
The gravity kernel encodes exactly that, and nothing more.

### 3.2 The kernel

A 5×5 kernel, weights decaying by Chebyshev distance from the centre,
centre cell exactly 0:

| ring | cells | weight |
|------|-------|--------|
| centre (distance 0) | 1 | 0 |
| distance 1 | 8 | 2 |
| distance 2 | 16 | 1 |

The centre is zero for the same reason the convolutions tutorial's
kernel language never conditions on the candidate cell `c` itself: a
kernel is a picture of the *neighborhood* condition, and "there is a
stone here" is not a neighborhood condition (occupied cells are
illegal moves anyway). Weights are small integers, so the raw output
is an exact f32 sum on any backend (the exactness principle); the
maximum raw output is 8·2 + 16·1 = 32, and the readout normalizes by
32 — a power of two, so the normalized gravity map `g(r, c) ∈ [0, 1]`
is still exactly representable. These starting weights are an
`[experiment]` value: any monotone decay works; the tutorial pins this
table and records it as a choice, not a law.

### 3.3 The occupancy plane — and why not the existing channels

The gravity branch deliberately does **not** reuse the two-channel
`patterns::planes` output (owner's decision). Channel 0 (own stones
only) would make gravity color-biased; channel 1 (opponent stones *or*
the border frame) would pull toward the board edges — the border ring
is all-ones, so a corner cell would score phantom "neighbor mass"
from outside the board. Both defects come from reusing a plane whose
semantics serve the pattern kernels, not proximity.

Instead, glue code builds a dedicated one-channel **occupancy plane**:
1 where any stone of either color sits, 0 elsewhere, zero-padded by
the kernel radius (2) to 19×19 — no border ring. The gravity branch is
then its own small conv branch: `Conv2d`, 5×5, 1 → 1 channel, valid
padding, bias 0, weights from §3.2, no ReLU needed (inputs and weights
are non-negative). This is a deliberate, un-magical data-flow choice —
one kernel, one channel, built in the open in the `baseline` crate —
and it leaves room for more advanced heuristic branches later (each
new heuristic is another small named branch, not a surgery on
`patterns`).

### 3.4 Sanity properties (tested)

- Empty board ⇒ gravity map all zero ⇒ uniform policy floor (the
  masked-softmax degenerate case, already tested in `mcts`).
- A single stone at the centre ⇒ the kernel's shape appears around it,
  hand-computed, compared with `==` (exactness).
- A stone in the corner ⇒ the map is *truncated*, never boosted — the
  regression test for the border-pull defect of §3.3.
- The Burn conv branch equals a plain-Rust nested-loop reference on
  randomized small-integer inputs (the "trusting the framework"
  pattern from convolutions chapter 2, reused).

## 4. The policy readout

### 4.1 Inputs

The readout consumes, for the side to move and its opponent:

- both sides' derived `ThreatMaps` from `patterns::net::analyze_both`
  (one batched forward pass — this is why `analyze_both` exists):
  `wins`, `double_threats`, `double_threes`, and `threes_per_dir` per
  side. (`fours_created_per_dir` is a visualization aid per its own
  docs — the readout uses the unique-completing-cell `double_threats`
  map, not the raw per-pattern sums.)
- the normalized gravity map from §3.

The top two tiers are *oracular*: the convolutions tutorial's
differential gates prove the win maps equal `engine::immediate_wins`
and the double-threat map equals `engine::double_threats` at 1000
boards × 2 colors. The three-level tiers are oracle-proven against the
naive three-enumerator. Gravity alone is unproven heuristic — and it
sits at the bottom of the hierarchy, where it can do the least harm.
The hierarchy is therefore also a **trust ladder**: certainty at the
top, increasingly heuristic downward.

### 4.2 The derivation the reader must produce

One question forces the whole design: **what breaks if gravity ever
outvotes a forced block?** Answer: the side fails to block the
opponent's immediate five and loses on the spot — so a forced-block
cell must win the softmax against *every* cell that is merely
gravity-attractive, no matter how attractive, and no matter how many
such cells compete. Softmax mass ratios are exponentials of logit
gaps, and the worst case is bounded (at most 224 competing legal
cells, gravity ≤ 1 after normalization): any fixed logit gap makes the
failure probability arbitrarily small, and a gap that exceeds the
maximum possible sum of *all* lower-tier contributions makes a lower
tier unable to outvote a higher one even by stacking.

Generalize from that one pair to every adjacent pair, and the rule
falls out: **each tier's weight must exceed the maximum possible sum
of all tiers below it.** Binary membership per tier (a cell is in a
tier or not — no per-direction stacking within a tier) makes the sums
small and obvious, and choosing powers of two makes the whole table
exact in f32 — the exactness principle applied to the readout itself.
"Orders of magnitude between tiers" is then not a tuning claim but a
theorem about the table, stated in two parts. First, **argmax safety**:
because no stacking of lower tiers can reach the next tier's weight, a
cell in the highest occupied tier always holds the strict logit
maximum — at temperature 0 the right move is always there to be
picked. Second, **softmax mass**: a lone forced-block cell outweighs
every cell that is not itself a forced block or a double threat
(maximum lower total 8 + 4 + 2 + 1 = 15, gap ≥ 32 − 15 = 17) by
e^17 ≈ 2 × 10⁷ per cell — even against 224 such competitors, the miss
probability sits below 10⁻³. Against a cell that *also* carries the
double-threat tier the guaranteed margin shrinks to the invariant's
minimum of 1 — and there the tutorial makes the game-theoretic point:
blocking and claiming an unblockable threat are both defensible, so a
near coin-flip between them is acceptable, and the value readout plus
search adjudicate.

### 4.3 The tier table

Logit contributions, binary membership per tier, additive across
tiers:

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
8 + 4 + 2 + 1 = 15 < 16; 16 + 15 = 31 < 32; 32 + 31 = 63 < 64. Every
constant is a power of two; every reachable logit is an exact f32 sum.
The only floating-point freedom left in the pipeline lives inside
`mcts`'s already-tested `masked_softmax`.

Two deliberate absences, documented in the tutorial:

- **The opponent's double threats are not a policy tier.** A double
  threat is by definition unanswerable; spending prior mass on
  "defense" against it is waste. It belongs to the value readout (§5),
  where it correctly makes the position look lost.
- **The opponent's wins outrank everything except playing our own win
  now** (T1 > T2): if we complete five, the game ends before the
  opponent's threat matters — the engine's tactics module documents
  the same partition (win now / must block / unblockable threat).

### 4.4 Output shape

The readout writes a full `[f32; 225]` logit vector: tier sums at
empty cells, `f32::NEG_INFINITY` at occupied cells (matching the
mock's masked style; expansion masks to legal moves regardless). The
`mcts` crate's `masked_softmax` — mask-first, f64 accumulator,
degenerate-uniform fallback — is the only softmax; the evaluator never
normalizes.

## 5. The value readout

A saturating heuristic from the same maps, from the side-to-move's
perspective, shaped like the mock's sentinels and extended down the
threat ladder. Starting values (all `[experiment]`, tuned during
acceptance measurement, §10):

1. `s` has any immediate win ⇒ return **+0.95** (about to win; the
   mock's sentinel, kept).
2. `o` has two or more immediate wins ⇒ return **−0.99** (one block
   cannot cover two finishing cells; effectively lost).
3. Otherwise accumulate, then clamp to [−0.99, +0.99]:
   - `o` has exactly one immediate win ⇒ −0.75 (a forced block: alive,
     but bad);
   - `s` has any double threat ⇒ +0.85 (unanswerable once the
     opponent's win is accounted for — cases 1–2 already returned);
   - `o` has any double threat ⇒ −0.85;
   - `s` has any double-three ⇒ +0.20; `o` has any ⇒ −0.20.

Pinned properties (the tests assert these, not the constants):

- **Perspective and sign.** Value from the side to move, in [−1, 1],
  matching `EvalResult::value`'s contract and the search's
  sign-flipped backup.
- **Saturation.** |v| ≤ 0.99 always; ±1 is reserved for terminals,
  which the search values exactly without calling the evaluator.
- **Monotonicity.** Adding own threats never decreases v; adding
  opponent threats never increases it.
- **Quiet board ⇒ exactly 0.0.** Gravity never enters the value; a
  position with no maps firing is honestly neutral.
- **Honesty.** The chapter presents this as a heuristic with named
  failure modes (it cannot see threat *sequences*; the engine's own
  four-three blind spot applies; the opponent-wins-first
  simplification is inherited). It is the mock's scaffolding made one
  notch less crude — and the tutorial says so.

## 6. Crate topology

New workspace member `gomoku/crates/baseline`:

```text
baseline/   deps: engine (workspace), mcts (workspace),
                  patterns (workspace), burn (workspace, features = ["flex"]),
                  rand (0.10.3)
            dev-deps: engine with testutil
            features: gpu = ["patterns/gpu"]   # opt-in, mirrors patterns
cli/        adds dep: baseline (workspace)
```

The dependency rules from docs/12 §6 are untouched and checkable:

- `engine` is unchanged — still a dependency island;
  `cargo check -p engine` stays Burn-free.
- `mcts` is unchanged — still no Burn, no I/O beyond its current deps;
  the baseline implements `mcts`'s trait from *outside*, which is
  exactly what the seam is for (docs/12: "a direct `net::Model`
  wrapper implements it for tests and arena play" — the baseline is
  such a wrapper, one tutorial early).
- `patterns` carries Burn; `baseline` carries Burn (directly, for
  `Device` and the gravity branch's `Conv2d`; transitively via
  `patterns`).
- No reverse edges: nothing in `engine`/`mcts`/`patterns` knows
  `baseline` exists. `cli` gains its first `baseline` edge; the
  architecture doc's seven planned crates (engine, net, mcts,
  selfplay, train, arena, cli) are unaffected — like `patterns`,
  `baseline` is a tutorial crate, not a new subsystem.

## 7. The Evaluator adapter

The seam, quoted exactly from `gomoku/crates/mcts/src/eval.rs` (verify
against the source when reached, §13):

```rust
/// Evaluation request, contains the board as Planes and a vector of legal moves
#[derive(Debug, Clone, PartialEq)]
pub struct EvalRequest {
    /// the board in its 17x17 representation
    pub planes: Planes,

    /// Vector with all legal moves
    pub legal: Vec<Move>,
}

/// The result: Policy and Value
#[derive(Debug, Clone, PartialEq)]
pub struct EvalResult {
    /// the policy vector: 15x15
    pub policy: [f32; 225],

    /// the value of the position `[-1.0 .. 1.0]` from current player's point of view
    pub value: f32,
}

/// the contract with any evaluator implementation
pub trait Evaluator {
    /// return policy and value for current board position
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult;
}
```

The adapter:

```rust
pub struct BaselineEvaluator {
    net: patterns::net::ThreatNet,
    gravity: burn::nn::conv::Conv2d,   // the §3 branch
    device: burn::tensor::Device,
}
```

- `BaselineEvaluator::new()` builds everything on `Device::flex()`
  (the default, matching `patterns`); `with_device(device)` supports
  the opt-in `gpu` feature.
- `evaluate` reconstructs the `Board` from `req.planes` (the engine's
  17×17 side-to-move-relative `me`/`you` encoding) via stone-count
  parity, asserting the supplied legal set equals the reconstructed
  board's — the same approach as the mock's private
  `reconstruct_board`. Because that helper is private to `mcts`, the
  baseline carries its own small copy with a comment saying so —
  documented, deliberate duplication, with the precedent set by
  `patterns` duplicating datagen's playout generator rather than
  adding a dependency. During normal search every evaluated position
  is alternation-legal, so parity reconstruction is sound.
- Colors are absolute (locked, docs/13): `analyze_both(&net, &board,
  &device) -> [ThreatMaps; 2]` returns `[black, white]`; the adapter
  picks own/opponent by `board.to_move()`.
- Then §4's policy readout and §5's value readout produce the
  `EvalResult` directly.
- The evaluator is **fully deterministic** — no RNG anywhere in
  evaluation. Match-play variety comes from the search side
  (temperature sampling in `mcts::policy`), never from the evaluator.
- Terminals never reach the evaluator: `mcts::expand` checks
  `board.status()` first and returns exact values, so the heuristic's
  saturation never collides with proven outcomes.

The search-side APIs the tutorial consumes, quoted from
`gomoku/crates/mcts/src/`:

```rust
pub fn search(
    root_board: &Board,
    evaluator: &mut dyn Evaluator,
    config: &SearchConfig,
) -> SearchOutcome;

pub fn visit_distribution(tree: &Tree, root: NodeId) -> Vec<(Move, u32)>;

pub fn select_move(dist: &[(Move, u32)], temperature: f32, rng: &mut impl Rng)
    -> Option<Move>;
```

with `SearchConfig { simulations: 400, c_puct: 1.5 }` as the default
(ELF's published value — the honesty ledger applies).

## 8. The CLI play mode

The existing `gomoku` binary is human-vs-human hotseat
(`--engine naive|fast`, `--plain`, `--no-swap2`, `--puzzle`). The
tutorial adds one new play mode (exact flag names pinned in the
implementation plan):

```text
gomoku --versus baseline [--color black|white] [--sims N]
```

- The Swap2 opening flow is unchanged: the human answers for both
  parties during the opening (hotseat behavior), and the baseline
  takes over its assigned color once a normal position exists;
  `--no-swap2` starts from an empty board as today. One opening code
  path, no special cases.
- On each baseline turn, the CLI calls a small `baseline::play` helper
  (`choose_move(&engine::Board, sims) -> Move`): `search` with
  `SearchConfig { simulations: sims, ..Default::default() }`, then
  `visit_distribution` + `select_move` at temperature 0 (argmax,
  deterministic). The CLI depends on `baseline` only — it never
  touches `mcts` directly.
- The integration seam already exists: `cli::board::GameBoard` exposes
  `engine_board() -> Option<&engine::Board>` for engine-specific
  features (the TSS overlay is the precedent). Versus mode requires
  the fast engine; combining it with `--engine naive` is a clean
  startup error, mirroring the existing Swap2/naive check.
- Both front-ends (`run_ui` alternate-screen and `run_plain`
  scrolling) support the mode through the same dispatch seam; a
  scripted-stdin `--plain` game doubles as the smoke test.
- `--sims N` defaults to 400 and exists so the owner can trade
  thinking time for strength on first contact — an `[experiment]`
  knob, documented as such.

## 9. Tutorial structure

Modeled on the mcts/datagen/convolutions tutorials: a primer as design
authority, chapters with learning goals + contracts + TDD checklists
and implementations withheld, opt-in per-chapter solution files
quoting the verified reference verbatim, `feat(baseline): …` commit
messages, per-chapter gates (`cargo test -p baseline`, clippy, fmt,
all from `gomoku/`). Location: `docs/tutorials/baseline-tutorial/`.

### 00 — primer

The design authority: the trust ladder (§4.1), the gravity idea (§3),
the tier derivation (§4.2), the honest-heuristic framing (§5), the
acceptance-by-measurement contract (§10), what the tutorial defers.

### 01 — the gravity kernel and the occupancy plane

- **Intention:** give the search a sane default when no threat is
  present; teach that a conv "branch" can be one small kernel built in
  glue, and why the existing planes are the wrong input (border pull).
- **Key APIs:** the occupancy builder `occupied_plane(&Board) ->
  [[f32; 19]; 19]`; the gravity branch `Conv2d` (5×5, 1→1, valid
  padding, bias 0, weights from §3.2); `gravity_map(&Board, &Device)
  -> [[f32; 15]; 15]` including the /32 normalization.
- **Tests:** §3.4's four sanity properties (empty board ⇒ zeros;
  hand-computed center-stone map with `==`; corner truncation, no
  border boost; Burn branch == plain-Rust loop reference on randomized
  small-integer inputs), plus the occupancy-plane unit test.
- **Done when:** the crate is scaffolded as in §6, the gravity suite
  is green, and the gates pass.

### 02 — the policy readout

- **Intention:** derive the tier table from "what breaks if gravity
  ever outvotes a forced block"; teach binary tier membership and the
  powers-of-two domination invariant.
- **Key APIs:** `policy_logits(own: &ThreatMaps, opp: &ThreatMaps,
  gravity: &[[f32; 15]; 15], board: &Board) -> [f32; 225]`.
- **Tests:** synthetic `ThreatMaps` (public array fields make them
  constructible by hand) — the domination invariant per adjacent tier
  pair; argmax safety in the fully stacked worst case (one T2 cell vs
  a T3+everything-else cell: the block's logit is strictly higher);
  the softmax-mass bound (one T2 cell vs 224 cells each carrying every
  tier below T3: after `mcts::eval::masked_softmax`, the block holds
  ≥ 1 − 10⁻⁴ of the mass); ASCII-board integration via
  `analyze` (a win beats a forced block; a forced block beats a double
  threat; a double threat beats three-makers; gravity only breaks
  ties inside the quiet floor); empty board ⇒ all-225 logits equal;
  occupied cells carry `NEG_INFINITY`.
- **Done when:** the suite is green and the chapter text reproduces
  the §4.2 derivation, ending with the §4.3 table.

### 03 — the value readout

- **Intention:** an honest saturating heuristic; teach the difference
  between certainty (terminals, ±1, owned by the search) and heuristic
  (everything else, clamped away from ±1).
- **Key APIs:** `value(own: &ThreatMaps, opp: &ThreatMaps) -> f32`.
- **Tests:** the §5 sentinel ladder (win-in-1 ⇒ +0.95; opponent's two
  wins ⇒ −0.99; forced-block-only ≤ −0.7; own double threat with no
  opponent wins ≥ +0.8; quiet ⇒ exactly 0.0); monotonicity on
  constructive cases; |v| ≤ 0.99 across a seeded random-board sweep.
- **Done when:** the suite is green and the chapter carries the
  honesty box naming the heuristic's failure modes.

### 04 — the Evaluator adapter and the CLI play mode

- **Intention:** assemble the player; teach the adapter shape the
  trained `net` will later reuse; make it playable.
- **Key APIs:** `reconstruct` (planes → `Board`, §7);
  `impl mcts::eval::Evaluator for BaselineEvaluator`;
  `baseline::play::choose_move(&Board, u32) -> Move`; the CLI's
  `--versus` mode (§8).
- **Tests:** reconstruction round-trips (encode → reconstruct → same
  position and Zobrist key; legal-set assertion fires on a doctored
  request); determinism (same request twice ⇒ identical `EvalResult`);
  adapter vs mock on the mcts acceptance sentinels (win-in-1 and
  forced-block boards: the baseline's argmax logit matches the mock's
  highlighted set — the baseline is *at least* as tactical as what it
  replaces); `choose_move` takes win-in-1 at 50 simulations;
  scripted-stdin `--plain --versus baseline` game completes; `--versus`
  with `--engine naive` fails cleanly.
- **Done when:** `cargo run --release -p cli -- --versus baseline` is
  playable end-to-end and the suite is green.

### 05 — acceptance: measured games

- **Intention:** replace strength claims with measured hypotheses;
  teach the pre-registration habit (write the bar down, run the
  protocol, record the real number).
- **Key APIs:** the match runner (`src/bin/basematch.rs`): N seeded
  games vs `mcts::mock::TacticsEvaluator`, alternating colors, variety
  from seeded temperature sampling in the first k plies (k pinned in
  the plan), 400 simulations, run in release mode.
- **Tests:** the §10 protocol, executed and recorded; workspace gates;
  crate polish (`#![deny(missing_docs)]`).
- **Done when:** the chapter's Status section quotes the measured
  numbers (never targets, never invented — the convolutions chapter-8
  and net-tutorial re-verification pattern), the hypothesis table
  shows pre-registered bar vs measured result side by side, and the
  full gates are green.

## 10. Acceptance — measured hypotheses, not asserts

Two kinds of checks, kept scrupulously separate.

**Asserts (certainties, verified every run):**

1. Tactical soundness: on the mcts acceptance boards, the baseline
   takes win-in-1 at 50 simulations and the forced block at 50
   simulations.
2. Game health flood: 100 seeded self-play games — every move legal,
   every game ≤ 225 plies, no panics (the mcts 1000-game flood,
   scaled down for evaluation cost).
3. Derivation checks: the §4.3 domination invariant and the §5 pinned
   value properties (saturation, monotonicity, quiet ⇒ 0).
4. Workspace hygiene: `cargo test --workspace`,
   `cargo clippy --all-targets -- -D warnings`,
   `cargo fmt --all -- --check`; `cargo check -p engine` confirms the
   island; `cargo tree -p mcts` shows no Burn.

**Measured hypotheses (strength, recorded as real numbers):**

- **H1 (headline).** In N = 200 seeded self-play games against the
  `TacticsEvaluator` (alternating colors, release mode, 400
  simulations), the baseline wins **≥ X %** — X pre-registered in the
  acceptance chapter and tuned on the first real run, exactly as the
  net-tutorial spec handles its acceptance bar. The measured result is
  recorded next to the bar, whatever it is.
- **H2 (threat construction).** Baseline self-play games contain
  measurably more double-threat creations per game than
  mock-vs-mock games (a statistic, recorded — the qualitative claim
  "gravity + three-makers produce richer positions" made falsifiable).
- **H3 (throughput).** Seconds per game and evaluations per second at
  400 simulations in release: recorded as context for the self-play
  planning in docs/12, with no bar attached. (The convolutions
  tutorial measured the three 1000-board × 2-color
  differential proofs at 8.9 s total in release, ~620 s in debug — set
  the expectation: matches and CLI play are release-mode activities.)

The match score from H1 is the Elo anchor's seed datum: when the
`arena` crate lands, "beats the baseline in ≥ X % of 200" becomes the
trained network's first concrete target.

## 11. What this is for, later

Motivation only — none of this is built in the tutorial:

1. **The Elo anchor.** The trained network's strength will be
   expressed against this fixed, reproducible opponent (docs/12's
   arena milestone).
2. **Better self-play data.** Datagen re-run with the baseline
   evaluator yields positions shaped by gravity and three-level
   threats instead of 90/10 tactics shaping — richer phase-1 training
   data than the mock produces.
3. **The first meeting of engine + mcts + patterns.** Every later
   subsystem (selfplay service, arena, the net's own evaluator
   adapter) reuses this adapter's shape; the tutorial is the dry run.
4. **The gravity branch as a pattern.** Future hand-written heuristics
   (and the option of frozen hand-written features beside learned
   ones, the DeepGomoku pattern) plug in as more small named branches.

## 12. Risks

1. **Drift vs the not-yet-worked convolutions chapters 4–8** (the main
   risk). `ThreatMaps`, `analyze`, and `analyze_both` exist today only
   in the verified final-form solution files, not in the `patterns`
   crate on `main`. Mitigation: this spec pins every consumed contract
   to the final form quoted in
   `docs/tutorials/convolutions-tutorial/04-the-four-level/01-solution.md`;
   §13 makes verification the first task when the tutorial is reached;
   if the owner worked those chapters to a different final form, this
   spec is amended *before* the implementation plan is written.
2. **Strength claims cannot be verified on paper.** The baseline might
   lose to the mock on the first run (a mis-scaled tier, a value
   heuristic that misleads backup). Mitigation: §10's structure —
   tactical soundness is asserted (certainties only), strength is
   hypothesized and measured; every weight is a documented
   `[experiment]` knob with a tuning protocol; the acceptance chapter
   records real numbers including failures.
3. **Evaluation cost.** One 152-channel 11×11 forward pass per
   expansion × hundreds of simulations × full games: fine in release
   (convolutions chapter 8 measured the three 1000-board × 2-color
   proofs at 8.9 s total), painful in debug (~620 s for the same). Mitigation: matches and play are
   release-mode by documentation; `--sims` is the owner's knob; H3
   records the real cost. Caching/batching is the future self-play
   service's job (docs/12), explicitly out of scope here.
4. **Scope creep** into training, TSS coupling, Elo rigor, or datagen
   re-runs. Guarded by the non-goals; reviewers reject additions.
5. **Burn API drift.** The workspace pins `=0.22.0` exactly (hard rule
   1); every Burn idiom in the tutorial (manual `Conv2d` weight
   assignment on `flex`, `TensorData` construction) is verified
   against the pinned source, on the precedent of the convolutions
   tutorial's Task-0 spike.
6. **Semantic drift from the engine.** Where the readout treats maps
   as oracular (T1–T3), it inherits the engine's documented v1
   simplifications (four-three blind spot, opponent-wins-first,
   immediate wins excluded from double threats). The primer names
   them; the convolutions differential gates already enforce the
   equality.

## 13. Verify when reached

Before the implementation plan is written, run this checklist (from
`gomoku/` unless noted) and amend this spec where reality disagrees:

1. `patterns` final form landed: `crates/patterns/src/net.rs` contains
   `pub struct ThreatMaps` with exactly the fields `wins`,
   `double_threats`, `double_threes`, `threes_per_dir`,
   `fours_created_per_dir`, plus `pub fn analyze` and
   `pub fn analyze_both` — diff semantics against
   `docs/tutorials/convolutions-tutorial/04-the-four-level/01-solution.md`.
2. The convolutions gates are still green:
   `cargo test -p patterns --release -- --ignored`.
3. The `Evaluator` trait, `EvalRequest`, `EvalResult`,
   `masked_softmax`, `search`, `visit_distribution`, and `select_move`
   signatures are unchanged from the quotes in §7 (check against
   `crates/mcts/src/`).
4. The mock is still `mcts::mock::TacticsEvaluator` with the
   documented shaping and sentinel values (§7, `crates/mcts/src/mock.rs`).
5. CLI seams intact: `GameBoard::engine_board`, `ui::run_ui`,
   `ui::run_plain`, the flag-parsing shape in `cli/src/main.rs`.
6. Workspace pin still `burn = "=0.22.0"`; `patterns` still on
   `features = ["flex"]` with the opt-in `gpu` feature.
7. Engine tactics signatures unchanged:
   `immediate_wins(&Board, Color) -> MoveSet`,
   `forced_blocks(&Board) -> MoveSet`,
   `double_threats(&Board, Color) -> MoveSet`.
8. The mcts acceptance boards used in §10 (win-in-1 @ 50 sims, forced
   block @ 50) still exist in `crates/mcts/tests/`.

## 14. Workflow and deliverables

Same pipeline as the datagen and convolutions tutorials:

1. This spec, reviewed by the owner, committed to `main`.
2. An implementation plan (`docs/plans/`), reviewed by the owner.
3. Reference implementation in a disposable git worktree on
   `reference/baseline`, built slice by slice with visible subagents
   (implement → spec review → quality review → amendments → re-review;
   controller commits). The branch is deleted after final
   verification, per convention — the tutorial's solution files are
   the durable record.
4. Tutorial docs land on `main` per slice; no pushes without the
   owner's request; WARM-UP/AGENTS status updates remain a
   conversation with the owner.

## References

- `docs/specs/2026-09-27-convolutions-tutorial-design.md` — the style
  and pipeline template; the threat-ladder semantics this tutorial
  consumes.
- `docs/tutorials/convolutions-tutorial/04-the-four-level/01-solution.md`
  — the normative final form of `patterns::net`: `ThreatNet`,
  `ThreatMaps`, `analyze`, `analyze_both`, the unique-completing-cell
  glue.
- `docs/tutorials/convolutions-tutorial/08-acceptance.md` — the
  measured-numbers acceptance pattern this tutorial extends from
  correctness to strength.
- `gomoku/crates/mcts/src/eval.rs` — the `Evaluator` seam (quoted in
  §7); `mock.rs` — the opponent the baseline replaces and meets;
  `search.rs`, `policy.rs` — the search-side APIs.
- `gomoku/crates/patterns/src/{planes,kernel,net}.rs` — the current
  crate state (2-channel planes with border-blocked semantics; the
  152-channel table; `ThreatNet` + `win_map`).
- `gomoku/crates/cli/src/{main,board,ui}.rs` — the hotseat client and
  the `GameBoard::engine_board` seam the play mode uses.
- `docs/12-gomoku-architecture.md` — §3 alpha-epsilon, §5 no-tokio
  reasoning, §6 crate boundaries and the `Evaluator` rationale, the
  honesty ledger; `docs/13-engine-design.md` — tactics simplifications
  and absolute colors; `docs/WARM-UP.md` — current status and
  vocabulary.
- `docs/specs/2026-10-04-net-tutorial-design.md` — the measured
  acceptance-bar and re-verification precedent (I5); the neighbor this
  side quest now precedes.
- `docs/specs/2026-09-27-datagen-tutorial-design.md` — workflow
  precedent (spec → plan → subagent slices).
