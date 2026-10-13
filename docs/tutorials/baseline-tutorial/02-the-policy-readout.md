# Chapter 02 — The policy readout

## Abstract

This chapter turns maps into moves. The policy readout combines both
sides' `ThreatMaps` and the gravity map into a single `[f32; 225]`
logit vector — the `policy` half of the evaluator's contract with the
search. Its structure is a **tier table** (own wins ≫ opponent's wins
≫ own double threats ≫ three-makers ≫ gravity), and the chapter's
real content is that the table is **derived, not tuned**: one question
— *what breaks if gravity ever outvotes a forced block?* — forces a
domination invariant, binary membership per tier, and powers-of-two
weights. You will prove two theorems about the table (argmax safety
and a softmax-mass bound), learn why the opponent's double threats are
deliberately absent from it, and why the readout emits raw logits and
never a normalized distribution.

## Glossary

| Term | Definition |
|------|------------|
| **Policy readout** | The glue from maps to a `[f32; 225]` logit vector — one logit per board cell, row-major (`r * 15 + c`, the engine's `Move::index`). |
| **Tier** | One row of the policy table: a membership condition on the maps plus a fixed logit weight. Tiers are ordered; higher means more urgent. |
| **Binary membership** | A cell is in a tier or not. Within a tier there is no stacking: three directions firing still contribute the tier weight once. |
| **Domination invariant** | Each tier's weight exceeds the maximum possible sum of all tiers below it: 8+4+2+1 = 15 < 16; 16+15 = 31 < 32; 32+31 = 63 < 64. |
| **Argmax safety** | The theorem that a cell in the highest occupied tier always holds the strict logit maximum — so at temperature 0 the right move is always there to be picked. |
| **Softmax mass** | The share of probability a cell holds after softmax; ratios are exponentials of logit gaps, so a gap of 17 is a per-cell factor of e¹⁷ ≈ 2 × 10⁷. |
| **Logit** | An unnormalized log-probability. `EvalResult::policy` carries logits; normalization is the search's job. |
| **Masked softmax** | `mcts::eval::masked_softmax` — softmax over the legal cells only, f64 accumulator, mask-first guard, degenerate-uniform fallback. The only softmax in the system. |
| **Forced block** | A cell the side to move must play because the opponent completes five there — exactly the opponent's win map. |
| **Quiet floor** | The set of cells in no threat tier: their logits are gravity only, in [0, 1]. |

## Context

Chapter 1 built the gravity map: a normalized [0, 1] quiet-board
default, exact in f32. The convolutions tutorial (chapters 4–8, which
you have completed — see the README's prerequisite) built the rest of
the input: `patterns::net::analyze_both` produces both sides'
`ThreatMaps` from one batched forward pass, with the win and
double-threat maps engine-proven and the three-maps oracle-proven.

What does not exist yet is any connection between those maps and the
search. The mcts `Evaluator` contract wants a policy vector per
position; the mock answers with 90/10 shaping on wins and blocks and
uniform noise elsewhere. This chapter writes the baseline's answer —
and unlike the mock's ad-hoc constants, every number in it falls out
of one derivation.

The consumed `patterns::net` contract, quoted from the verified final
form (doc comments elided)
([04-the-four-level/01-solution.md](../convolutions-tutorial/04-the-four-level/01-solution.md)):

```rust
pub struct ThreatMaps {
    pub wins: [[f32; 15]; 15],
    pub double_threats: [[f32; 15]; 15],
    pub double_threes: [[f32; 15]; 15],
    pub threes_per_dir: [[[f32; 15]; 15]; 4],
    pub fours_created_per_dir: [[[f32; 15]; 15]; 4],
}

pub fn analyze_both(net: &ThreatNet, b: &Board, device: &Device) -> [ThreatMaps; 2];
```

Colors are absolute (docs/13, locked): `analyze_both` returns
`[black_maps, white_maps]`; the readout's caller picks own/opponent by
`board.to_move()`. `fours_created_per_dir` is a visualization aid (a
raw per-pattern sum that may double-count a completing cell); the
readout uses the unique-completing-cell `double_threats` map instead.

## Intention

1. Implement `src/policy.rs`: the tier constants and
   `policy_logits(own, opp, gravity, board) -> [f32; 225]`, with
   binary membership per tier, tier sums at empty cells, and
   `f32::NEG_INFINITY` at occupied cells.
2. Prove the derivation's two theorems as tests: argmax safety in the
   fully stacked worst case, and the softmax-mass bound for a lone
   forced block against 224 maximally stacked competitors.
3. Verify the ordering end-to-end on ASCII boards through the real
   `analyze`: a win beats a forced block; a forced block beats a
   double threat; a double threat beats three-makers; gravity only
   breaks ties inside the quiet floor.

Observable done-state: the chapter-2 tests listed below pass,
`cargo clippy --all-targets -- -D warnings` is green from `gomoku/`,
and `cargo fmt --all` makes no changes from `gomoku/`.

## Mental mapping

### Why must these weights be derived, not tuned?

Start from the failure, not the table: **what breaks if gravity ever
outvotes a forced block?** The side to move, facing the opponent's
immediate five, plays somewhere "near the action" instead — and loses
on the spot. Whatever else the readout does, a forced-block cell must
win the softmax against *every* cell that is merely
gravity-attractive, no matter how attractive, and no matter how many
such cells compete.

That one requirement already has teeth. Softmax mass ratios are
exponentials of logit gaps: a competitor favored by Δ logits holds
e^Δ times the mass. The worst case is bounded — at most 224 competing
legal cells, gravity at most 1 after normalization — so:

* any fixed positive gap makes the per-cell failure factor a constant
  e^−gap, and the total failure mass is at most 224 times that — a
  fixed gap of, say, 17 already caps the miss below 10⁻³; but
* a cell might carry *several* weak signals at once. If weak signals
  stacked without limit, no fixed gap survives: stack enough +1s and
  they beat any +32.

The second bullet is the real constraint, and it generalizes from the
one pair (block vs gravity) to every adjacent pair of signals:
**each tier's weight must exceed the maximum possible sum of all tiers
below it.** Then no cell's stacked lower-tier total can ever reach the
next tier up, and "more urgent always wins" stops being a hope and
becomes arithmetic.

### Why binary membership per tier?

The domination rule is only usable if "the maximum possible sum of all
tiers below" is small and obvious. That is what binary membership
buys: a cell is in a tier or not — a cell making threes in two
directions is a three-maker once, not twice — so each tier contributes
its weight at most once per cell, and the worst-case stack is the
finite sum 8 + 4 + 2 + 1, not an unbounded pile.

The `ThreatMaps` contract already speaks this language: the maps are
0/1-style markers read with `> 0.0` (`double_threes` has the
distinct-directions counting already done in glue; the four
`threes_per_dir` maps collapse to "any direction"). Membership tests,
not magnitudes.

### Why powers of two?

With membership binary, the tier weights themselves are free — choose
them so the arithmetic stays exact. Powers of two make every reachable
logit (a sum of distinct tier weights plus a dyadic gravity term) an
exact f32 value: the exactness principle, applied to the readout
itself. The only floating-point freedom left in the whole pipeline
lives inside `mcts`'s already-tested `masked_softmax`.

### The derivation, walked to the table

Order the signals by urgency — the game-theoretic content, where the
engine's tactics partition (win now / must block / unblockable threat)
is the guide:

1. **Own wins above everything** (T1): if we complete five, the game
   ends before any opponent threat matters.
2. **The opponent's wins next** (T2): the forced blocks — failing to
   play one loses immediately.
3. **Own double threats next** (T3): unblockable by definition, so
   they end the game next move — but not this one, which is why they
   sit below the two immediate tiers.
4. **The three-level** (T4): the beginning of threat sequences —
   own double-three forks, own three-makers, and *defensive*
   attention to the opponent's three-makers and forks.
5. **Gravity last** (T5): the unproven heuristic, at the bottom of the
   trust ladder where it can do the least harm.

Now assign the smallest powers of two that satisfy domination, bottom
up: gravity lives in [0, 1]; the defensive three-tier must exceed 1 →
**2**; own three-makers must exceed 2 + 1 → **4**; the fork must
exceed 4 + 2 + 1 → **8**; the double threat must exceed
8 + 4 + 2 + 1 = 15 → **16**; the forced block must exceed
16 + 15 = 31 → **32**; the win must exceed 32 + 31 = 63 → **64**.

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

### What the invariant proves — two theorems and a near-coin-flip

**Argmax safety.** No stacking of lower tiers can reach the next
tier's weight, so a cell in the highest occupied tier always holds the
strict logit maximum. At temperature 0 (`select_move`'s argmax path)
the right move is always there to be picked — the search can fumble
it, the prior cannot hide it. Picture the worst case the table admits:

```text
cell A — a forced block and nothing else:                 +32
cell B — the fully stacked challenger:
         own double threat  (T3)          +16
         own double-three   (T4a)          +8
         own three-maker    (T4b)          +4
         opponent three     (T4c)          +2
         gravity, maximal   (T5)           +1
                                           ──
                                            31    < 32 by exactly 1
```

(Whether a real board realizes cell B is beside the point — the bound
is arithmetical, so it covers every board.)

**Softmax mass.** A lone forced-block cell outweighs every cell that
is not itself a forced block or a double threat: such competitors
carry at most 8 + 4 + 2 + 1 = 15, so the gap is at least
32 − 15 = 17, a per-cell mass factor of e¹⁷ ≈ 2 × 10⁷. Even against
224 maximally stacked competitors the miss probability sits below
10⁻³ (224 · e⁻¹⁷ ≈ 10⁻⁵). The test below pins the stronger bound:
after `masked_softmax`, the block holds ≥ 1 − 10⁻⁴ of the mass.

**The minimum margin, honestly.** Against a cell that *also* carries
the double-threat tier, the forced block's guaranteed margin shrinks
to the invariant's minimum of 1 — a near coin-flip per cell. That is
not a table bug: blocking the immediate loss and claiming an
unblockable threat are both defensible moves, so the readout calls it
close and lets the value readout plus the search adjudicate. (The dual
case is the pleasant one: a block that *also* creates a double threat
scores 32 + 16 = 48 — the table naturally prefers counter-attacking
blocks.)

### Why is the opponent's double threat not a tier?

Because there is no defense to spend prior mass on: a double threat is
unanswerable by definition, so "blocking" it is waste. Its absence
from the policy table is not optimism — it reappears in chapter 3's
value readout at −0.85, where it correctly makes the position look
lost. Policy answers "where should I look?"; value answers "how
worried should I be?". The opponent's double threat has a clear answer
to the second and no useful answer to the first.

### Why one `analyze_both` call — and why logits, never probabilities?

The readout needs *both* sides' maps: the opponent's win map is the
forced-block tier, and the opponent's three-maps feed the defensive
tier. `analyze_both` produces both colors' `ThreatMaps` from one
batched forward pass (batch of two) — that batching is the reason the
function exists — and colors are absolute, so the caller picks
own/opponent by `board.to_move()`.

The output is a full `[f32; 225]` logit vector: tier sums at empty
cells, `f32::NEG_INFINITY` at occupied cells — the mock's masked
style, so illegal cells are excluded even before masking. The readout
deliberately does **not** normalize: `EvalResult::policy` carries
logits by contract, and `masked_softmax` (mask-first, f64 accumulator,
degenerate-uniform fallback) is the only softmax in the system, applied
at expansion. An evaluator that normalizes itself double-shapes the
priors the search expects.

## Low-level design

### Files you will create or modify

```text
gomoku/crates/baseline/src/
├── lib.rs       (adds `pub mod policy;`)
├── gravity.rs   (chapter 1; unchanged)
└── policy.rs    (new)
```

### `src/policy.rs`

The tier constants — the derived table, pinned as names so tests and
the chapter text can speak them:

```rust
pub const TIER_WIN_NOW: f32 = 64.0;       // T1: own immediate wins
pub const TIER_FORCED_BLOCK: f32 = 32.0;  // T2: opponent's immediate wins
pub const TIER_DOUBLE_THREAT: f32 = 16.0; // T3: own double threats
pub const TIER_DOUBLE_THREE: f32 = 8.0;   // T4a: own double-three forks
pub const TIER_OWN_THREE: f32 = 4.0;      // T4b: own three-makers, any direction
pub const TIER_OPP_THREE: f32 = 2.0;      // T4c: opponent three-makers or forks
```

The readout:

```rust
pub fn policy_logits(
    own: &ThreatMaps,
    opp: &ThreatMaps,
    gravity: &[[f32; 15]; 15],
    board: &Board,
) -> [f32; 225];
```

Semantics, precisely:

* For each cell `(r, c)` with `mv = Move::new(r, c)`:
  * if `board.stone_at(mv).is_some()`, the logit is
    `f32::NEG_INFINITY` — occupied cells are illegal, and the mock's
    masked style is kept;
  * otherwise the logit is the sum of:
    * `TIER_WIN_NOW` if `own.wins[r][c] > 0.0`;
    * `TIER_FORCED_BLOCK` if `opp.wins[r][c] > 0.0`;
    * `TIER_DOUBLE_THREAT` if `own.double_threats[r][c] > 0.0`;
    * `TIER_DOUBLE_THREE` if `own.double_threes[r][c] > 0.0`;
    * `TIER_OWN_THREE` if `own.threes_per_dir[d][r][c] > 0.0` for
      **any** `d` in 0..4 (once, however many directions fire);
    * `TIER_OPP_THREE` if `opp.threes_per_dir[d][r][c] > 0.0` for any
      `d` **or** `opp.double_threes[r][c] > 0.0` (once, however many
      fire);
    * `gravity[r][c]` — always, including alongside threat tiers.
* The result is indexed row-major: logit for `(r, c)` at
  `mv.index() == r * 15 + c`, matching `EvalResult::policy`'s
  contract and `masked_softmax`'s indexing.
* Membership is binary (`> 0.0`); contributions are additive **across
  tiers** and idempotent **within** a tier. Every reachable logit is a
  sum of distinct powers of two plus a dyadic gravity term — exact in
  f32, comparable with `==`.

Keep a doc comment on the module that reproduces the tier table and
the domination invariant: the derivation lives next to the code, so a
future reader (or tuner) meets the reasoning before the numbers.

## TDD checklist

Run these tests from `gomoku/` with `cargo test -p baseline`.

The chapter's suite (`policy::tests`). Synthetic `ThreatMaps` are
constructible by hand — every field is a public array — so most tests
need no network at all; a small `fn empty_maps() -> ThreatMaps`
helper (zero arrays everywhere) is the fixture. The integration tests
use `patterns::net::{ThreatNet, analyze}` on
`engine::reference::board_from_ascii` boards (engine's `testutil`
feature, already a dev-dependency).

1. `tier_weights_dominate_all_lower_tiers` — the invariant, per
   adjacent pair, computed from the constants: T4a > T4b+T4c+1;
   T3 > T4a+T4b+T4c+1; T2 > T3+T4a+T4b+T4c+1; T1 > T2+T3+T4a+T4b+T4c+1
   (with `1.0` standing for gravity's maximum).
2. `argmax_safe_in_worst_case_stack` — synthetic maps: one T2-only
   cell vs one cell carrying T3+T4a+T4b+T4c and gravity 1.0; the
   block's logit is strictly higher.
3. `forced_block_holds_softmax_mass` — synthetic maps: one T2-only
   cell, all 224 other cells each carrying every tier below T3 plus
   gravity 1.0; after `mcts::eval::masked_softmax` over all 225 legal
   moves, the block's share is ≥ 1 − 10⁻⁴.
4. `own_win_beats_forced_block` — ASCII board where Black (to move)
   has an open four and White a closed four; through `analyze`, the
   argmax logit lands on Black's win, not on the block.
5. `forced_block_beats_double_threat` — ASCII board where White has a
   closed four and Black (to move) also has a double threat available
   elsewhere; the argmax logit is the block.
6. `double_threat_beats_three_makers` — ASCII board with a Black
   double-threat cell and separate three-maker cells; the double
   threat's logit exceeds every three-maker's.
7. `gravity_breaks_ties_in_quiet_floor` — an early position with no
   tiers firing; the argmax logit is a cell adjacent to existing
   stones, never a distant cell (gravity 0).
8. `empty_board_logits_all_equal` — all 225 logits exactly equal
   (gravity 0 everywhere), so `masked_softmax` returns the uniform
   distribution — the quiet-floor base case, already covered from the
   softmax side in `mcts`'s own tests.
9. `occupied_cells_carry_neg_infinity` — every occupied cell's logit
   is `f32::NEG_INFINITY`.

Follow the red-green-refactor rhythm:

1. **Red:** Create `src/policy.rs` with the constants, the signature,
   and a `todo!()` body. Add the nine tests. Run
   `cargo test -p baseline`. Expect failures.
2. **Green:** Implement `policy_logits` exactly per the semantics
   above. Run `cargo test -p baseline`. All nine tests should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p baseline` passes the nine chapter-2 tests from
  `gomoku/` (chapter 1's suite stays green).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* The module doc comment on `policy.rs` reproduces the tier table and
  the domination invariant — the derivation next to the code.
* Commit message for the code slice:
  `feat(baseline): tiered policy readout`.

Next: [Chapter 03 — The value readout](03-the-value-readout.md)

## References

* [`../../specs/2026-10-13-baseline-player-design.md`](../../specs/2026-10-13-baseline-player-design.md)
  — §4 (the readout: inputs, derivation, table, output shape).
* [`00-baseline-primer.md`](00-baseline-primer.md) — §2 (the trust
  ladder), §4 (the tier derivation), §9 (stacking and normalizing
  bugs).
* [`../convolutions-tutorial/04-the-four-level/01-solution.md`](../convolutions-tutorial/04-the-four-level/01-solution.md)
  — the normative `ThreatMaps` / `analyze` / `analyze_both` contract.
* `gomoku/crates/mcts/src/eval.rs` — `masked_softmax` and the
  `EvalResult::policy` logits contract; `mock.rs` — the 90/10 shaping
  this readout replaces, and the `NEG_INFINITY` masked style.
* `gomoku/crates/engine/src/tactics.rs` — the win-now / must-block /
  unblockable-threat partition the tier order follows.
