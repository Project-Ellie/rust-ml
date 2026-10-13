# Chapter 03 — The value readout

## Abstract

This chapter builds the evaluator's second output: a scalar **value**
in [−1, 1] from the side-to-move's perspective — the answer to "how
worried should I be?" where chapter 2 answered "where should I look?".
The value readout is a **saturating heuristic** from the same
`ThreatMaps`: shaped like the mock's sentinels, extended down the
threat ladder, and deliberately clamped away from ±1, because ±1 is
reserved for proven terminal outcomes the search values exactly. The
chapter's discipline is to pin **properties** (saturation,
monotonicity, quiet ⇒ exactly 0.0) while treating the constants as
`[experiment]` starting values — and to say, in an honesty box, that
this is a heuristic with named failure modes, not a learned
evaluation.

## Glossary

| Term | Definition |
|------|------------|
| **Value readout** | The glue from both sides' `ThreatMaps` to one f32 in [−1, 1], from the side-to-move's perspective — the `value` half of `EvalResult`. |
| **Saturating heuristic** | A value function that rises with advantage but is clamped away from ±1: |v| ≤ 0.99 always. Saturation keeps heuristic numbers from impersonating proven outcomes. |
| **Sentinel** | A fixed named constant for a recognized situation (+0.95 for "about to win"). Inherited from the mock's scaffolding, extended here. |
| **Terminal value** | The exact value of a won or drawn position (−1.0 / 0.0 from the side to move), computed by `mcts::expand` from `board.status()` — never by the evaluator. |
| **Perspective** | Whose point of view the value expresses. The contract is the side to move; the search's backup flips the sign per ply. |
| **Quiet board** | A position where no threat map fires for either side. Its value is exactly 0.0 — honestly neutral. |
| **Monotonicity** | Adding own threats never decreases v; adding opponent threats never increases it. A pinned property of the readout. |
| **`[experiment]`** | The project's label for a starting value that is a recorded choice, not a derived law — tuned during acceptance measurement, with both numbers kept. |

## Context

Chapter 2 built the policy readout: a 225-logit answer to "where
should I look?", derived from one question and proven as two theorems.
The search, however, needs a second number at every leaf: backed up
the path with flipped signs, the value is what makes deep lines
comparable. The mock's answer is three sentinels — +0.95 with an
immediate win, −0.90 facing a forced block, 0.0 otherwise — which its
own source calls tutorial scaffolding.

This chapter keeps the mock's honest shape and extends it one notch
down the threat ladder: double threats and double-three forks carry
real game content, and the value should say so. What it must never do
is touch ±1: `mcts::expand` checks `board.status()` before any
evaluation and returns exact terminal values (−1.0 for a loss, 0.0
for a draw) without calling the evaluator at all. The heuristic lives
strictly between the terminals it can never see.

## Intention

1. Implement `src/value.rs`: the sentinel/step constants and
   `value(own: &ThreatMaps, opp: &ThreatMaps) -> f32`, with the
   early-return ladder and the ±0.99 clamp.
2. Pin the **properties** as tests — perspective/sign, saturation,
   monotonicity, quiet ⇒ exactly 0.0 — with the constants asserted
   only where the spec pins them (the two early returns).
3. Carry the honesty box: the failure modes are named, in the chapter
   and in the module doc.

Observable done-state: the chapter-3 tests listed below pass,
`cargo clippy --all-targets -- -D warnings` is green from `gomoku/`,
and `cargo fmt --all` makes no changes from `gomoku/`.

## Mental mapping

### Why is ±1 forbidden territory?

Because ±1 means something in this system: *proven*. A terminal node
is worth exactly −1 (the side to move has already lost) or exactly 0
(a draw), and `mcts::expand` computes that from the engine's status
check before any evaluator is consulted — "The evaluator is never
called for terminals" is right there in `expand`'s doc comment. If a
heuristic could also emit ±1, backup could not distinguish "the search
proved this" from "the heuristic felt strongly". The clamp to
[−0.99, +0.99] is the heuristic keeping its hands off the truth's
vocabulary.

### Why keep the mock's sentinels — and why refine one?

The mock's +0.95 for "we have an immediate win" is worth keeping
verbatim: the side to move is about to end the game, and 0.95 says
"almost proven, not proven" in the reserved scale. The mock's −0.90
for *any* forced block, though, lumps two very different situations:

* the opponent has **exactly one** immediate win — you must block,
  you *can* block, the game continues: bad, but alive;
* the opponent has **two or more** immediate wins — one block cannot
  cover two finishing cells: effectively lost.

The readout splits them: −0.75 for the single forced block, and the
−0.99 floor (saturation's lower bound) for two or more. Everything
else accumulates from there.

### The ladder, and why it accumulates

```text
1. own side has any immediate win        ⇒  return +0.95
   (about to win — the mock's sentinel, kept)
2. opponent has two or more immediate wins
                                        ⇒  return −0.99
   (one block cannot cover two finishing cells)
3. otherwise accumulate, then clamp to [−0.99, +0.99]:
     opponent has exactly one immediate win   −0.75
     own side has any double threat           +0.85
     opponent has any double threat           −0.85
     own side has any double-three fork       +0.20
     opponent has any double-three fork       −0.20
```

The early returns encode the same partition the policy tiers encode:
an own win ends the game before anything else matters (case 1 first),
and a defenseless double finishing threat ends it too (case 2). Only
then does accumulation speak. Double threats sit at ±0.85 — high,
because they are unanswerable once the opponent's immediate wins are
accounted for (cases 1–2 already returned); a double threat coexisting
with a single forced block nets +0.85 − 0.75 = +0.10: "bad, with
counterplay", which is the honest reading of such positions. Forks are
real but distant ±0.20: the beginning of a threat sequence, not its
end.

Accumulation with a clamp is what makes the two pinned properties
fall out: **monotonicity** (every own term is positive, every opponent
term negative, so adding threats can only move v one way) and
**saturation** (the clamp caps what accumulation can reach).

### Why does gravity never enter the value?

Gravity is a *prior* — an answer to "where is the action likely to
be?" — not an evaluation. A quiet board with stones huddled in a
corner is not better or worse than one with stones near the centre;
nobody is closer to five. Keeping gravity out of the value yields the
cleanest pinned property of all: **a position with no maps firing is
worth exactly 0.0** — honestly neutral, and exactly comparable across
positions, because 0.0 is the same exact f32 everywhere.

### The honesty box

> **This is a heuristic, not a learned evaluation.** It reads static
> maps; it cannot see threat *sequences* — a position won by a forcing
> line (the engine's TSS prover owns those) reads as merely good.
> Through the oracular maps it inherits the engine's documented v1
> simplifications: the four-three blind spot, the opponent-wins-first
> rule, immediate wins excluded from double threats. And its constants
> are `[experiment]` starting values, tuned during acceptance
> measurement (chapter 5), not derived laws. It is the mock's
> scaffolding made one notch less crude — no more, and the tutorial
> says so everywhere. The trained network's value head is what
> replaces it.

## Low-level design

### Files you will create or modify

```text
gomoku/crates/baseline/src/
├── lib.rs       (adds `pub mod value;`)
├── gravity.rs   (chapter 1; unchanged)
├── policy.rs    (chapter 2; unchanged)
└── value.rs     (new)
```

### `src/value.rs`

```rust
pub const VALUE_OWN_WIN: f32 = 0.95;        // case 1: about to win
pub const VALUE_LOST: f32 = -0.99;          // case 2: defenseless vs two wins
pub const VALUE_FORCED_BLOCK: f32 = -0.75;  // exactly one opponent win
pub const VALUE_DOUBLE_THREAT: f32 = 0.85;  // any double threat, per side (signed)
pub const VALUE_DOUBLE_THREE: f32 = 0.20;   // any double-three fork, per side (signed)
pub const VALUE_CLAMP: f32 = 0.99;          // saturation bound
```

The readout:

```rust
pub fn value(own: &ThreatMaps, opp: &ThreatMaps) -> f32;
```

Semantics, precisely:

* Count immediate wins by scanning the 15×15 `wins` maps for cells
  `> 0.0`; detect double threats and forks likewise on
  `double_threats` and `double_threes`.
* Case 1: `own` has at least one immediate win ⇒ return
  `VALUE_OWN_WIN`.
* Case 2: `opp` has **two or more** immediate wins ⇒ return
  `VALUE_LOST`.
* Otherwise sum the signed terms — `opp` exactly one win ⇒
  `VALUE_FORCED_BLOCK`; `own` any double threat ⇒
  `+VALUE_DOUBLE_THREAT`; `opp` any ⇒ `−VALUE_DOUBLE_THREAT`; `own`
  any fork ⇒ `+VALUE_DOUBLE_THREE`; `opp` any ⇒
  `−VALUE_DOUBLE_THREE` — and clamp the sum to
  `[−VALUE_CLAMP, +VALUE_CLAMP]`.
* No gravity input, by design: the signature takes exactly the two
  `ThreatMaps`.
* Membership per term is binary (`> 0.0` anywhere ⇒ the term fires
  once), mirroring the policy readout.
* The module doc carries the honesty box verbatim.

## TDD checklist

Run these tests from `gomoku/` with `cargo test -p baseline`.

The chapter's suite (`value::tests`), on synthetic `ThreatMaps` (the
`empty_maps()` fixture from chapter 2, with cells set by hand), plus a
seeded random-board sweep using `analyze_both` on random legal
playouts:

1. `own_win_value_is_sentinel` — any own immediate win ⇒ exactly
   `+0.95` (this constant is spec-pinned; assert with `==`).
2. `two_opponent_wins_value_is_floor` — two opponent wins ⇒ exactly
   `−0.99` (spec-pinned; `==`).
3. `forced_block_only_is_bad_but_alive` — exactly one opponent win and
   nothing else firing: v ≤ −0.7 **and** v > −0.99 (a property, not a
   constant).
4. `own_double_threat_without_opponent_wins_is_strong` — own double
   threat, no opponent wins: v ≥ +0.8 (a property).
5. `quiet_board_value_is_exactly_zero` — no maps firing for either
   side: v == 0.0, exactly.
6. `own_threats_never_decrease_value` — constructive monotonicity:
   start from a quiet fixture, add own terms one at a time; v never
   decreases at any step.
7. `opponent_threats_never_increase_value` — the mirror image.
8. `value_saturates_on_random_boards` — seeded random legal playouts
   (the chapter-1 helper, reused); evaluate `value` on both colors'
   real `ThreatMaps` from `analyze_both`; assert |v| ≤ 0.99 on every
   board.

Follow the red-green-refactor rhythm:

1. **Red:** Create `src/value.rs` with the constants, the signature,
   and a `todo!()` body. Add the eight tests. Run
   `cargo test -p baseline`. Expect failures.
2. **Green:** Implement `value` exactly per the semantics above. Run
   `cargo test -p baseline`. All eight tests should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p baseline` passes the eight chapter-3 tests from
  `gomoku/` (chapters 1–2 stay green).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* The honesty box is in the `value.rs` module doc, verbatim.
* Commit message for the code slice:
  `feat(baseline): saturating value readout`.

Next: [Chapter 04 — The evaluator adapter](04-the-evaluator-adapter.md)

## References

* [`../../specs/2026-10-13-baseline-player-design.md`](../../specs/2026-10-13-baseline-player-design.md)
  — §5 (the value readout: ladder, pinned properties, honesty).
* [`00-baseline-primer.md`](00-baseline-primer.md) — §5 (the honest
  value heuristic), §9 (perspective and terminal bugs).
* `gomoku/crates/mcts/src/mock.rs` — the sentinel scaffolding
  (+0.95 / −0.90 / 0.0) this readout keeps and refines.
* `gomoku/crates/mcts/src/expand.rs` — the terminal check that values
  won/drawn positions exactly and never calls the evaluator on them.
* `gomoku/crates/mcts/src/backup.rs` — the sign-flipped backup that
  defines the perspective contract.
