# Sub-chapter 04 — Wins and blocks: design paper

## Abstract

This paper records the decisions behind adding tactical sight to the
baseline evaluator. The MCTS seam speaks relative planes, but the threat
network speaks absolute colors. A small adapter closes that gap with
stone-count parity and a legal-set assertion. Two new policy tiers then
turn the network's win maps into decisive prior mass. Gravity stays at
the bottom of the trust ladder, where it cannot outvote a forced block.

## Glossary

| Term | Definition |
|------|------------|
| **Adapter** | The planes-to-board reconstruction module that lets the evaluator consume the threat network. |
| **Parity rule** | Black moves first, so equal stone counts mean Black to move. |
| **Trust ladder** | The ordering of policy inputs by certainty: engine-proven wins at the top, unproven gravity at the bottom. |
| **Tier** | A binary membership class in the policy readout, with a single fixed logit weight. |
| **Domination invariant** | Every tier weight exceeds the maximum possible sum of all tiers below it. |

## Context

Sub-chapter 03 wired gravity into the evaluator. Gravity is a useful
heuristic on quiet boards, but it has no notion of urgency. The trust
ladder from the primer says that engine-proven wins must sit above
gravity and must be structurally unable to lose a vote. This sub-chapter
adds the first two proven tiers and the adapter that makes them
reachable.

## Decision 1 — Reconstruct the board instead of changing the seam

The `Evaluator` trait consumes `EvalRequest { planes, legal }`. The
threat network needs an `engine::Board`. Rather than widening the seam
and touching `mcts`, the evaluator rebuilds the board locally. This
keeps the seam small and reusable for the future net tutorial.

The reconstruction uses the same logic as the mock evaluator's private
helper: scan the inner 15×15 region of the planes, collect `me` stones
and `you` stones, decide the side to move by parity, then map `me`/`you`
to Black/White. The function is a deliberate, attributed copy because
the precedent is private.

## Decision 2 — Parity reconstruction is sound for the search

Every position that MCTS evaluates comes from legal alternating play,
starting from an empty board. Black moves first. Therefore the parity
rule is exact: equal counts mean Black to move, and White has one fewer
stone. The debug assertion on the legal set catches the cases where this
assumption breaks, such as handcrafted tests that swap the planes.

## Decision 3 — `analyze_both` returns Black maps first

`patterns::net::analyze_both` returns `[black_maps, white_maps]`. The
adapter must not reverse this order. `BaselineEvaluator::evaluate`
matches on `board.to_move()` and picks `(maps[0], maps[1])` when Black is
to move and `(maps[1], maps[0])` when White is to move. This keeps the
indexing truth visible in one place.

## Decision 4 — Two tiers now, four constants planted

The policy readout declares all seven tier constants in this sub-chapter
(`T1_WIN` through `T4C_DEFENSIVE` plus the implicit T5 gravity maximum),
but only `T1_WIN` and `T2_FORCED_BLOCK` are added to the logits. The
remaining constants are planted seeds. They let the domination invariant
test already prove the full ladder, and they make the next sub-chapters
pure additions rather than renumberings.

## Decision 5 — Tier weights are derived from a safety question

The design question is: what breaks if gravity ever outvotes a forced
block? The answer is a lost game. So the table is built backward from
the worst case. Each tier must exceed every lower tier combined. The
weights are powers of two, so every reachable logit is an exact f32 sum
and the domination invariant can be checked with `==`.

## Decision 6 — Binary membership, not stacking

A cell either belongs to a tier or it does not. The readout never adds
the same tier weight twice for the same cell, even if a move creates
wins in multiple directions. This keeps the maximum lower stack small
and makes the invariant obvious.

## Decision 7 — The value stays flat

This sub-chapter is about policy: where to look first. The value readout
remains `0.0`. The next sub-chapter adds the honest value heuristic.
Keeping them separate lets the learner see that policy alone already
creates tactical behavior.

## Data flow

```text
EvalRequest with planes and legal
        |
        ▼
reconstruct_board
        |
        ▼
absolute-color Board
        |
        ▼
ThreatNet analyze_both
        |
        ▼
[Black maps, White maps]
        |
        ▼
pick own and opp by side to move
        |
        ▼
gravity kernel on occupancy plane
        |
        ▼
policy_logits with T1 and T2 tiers
        |
        ▼
EvalResult with policy and flat value
```

## Noun catalog

| Noun | Role |
|------|------|
| `BaselineEvaluator` | The struct that implements `mcts::Evaluator`. |
| `ThreatNet` | The convolutional threat network owned by the evaluator. |
| `GravityKernel` | The gravity branch owned by the evaluator. |
| `Planes` | The side-to-move-relative encoding from the MCTS seam. |
| `Board` | The absolute-color engine board used by the threat network. |
| `ThreatMaps` | The threat network output for one color. |
| `[ThreatMaps; 2]` | Both colors' maps from `analyze_both`. |
| `Move` | One legal cell. |
| `EvalRequest` / `EvalResult` | The evaluator seam's input and output. |

## Verb catalog

| Verb | Actor | Object | Meaning |
|------|-------|--------|---------|
| reconstruct | `adapter::reconstruct_board` | `Planes` + `legal` | Build an absolute-color board. |
| analyze_both | `ThreatNet` | `Board` | Produce both colors' threat maps. |
| pick | `BaselineEvaluator::evaluate` | maps by `to_move()` | Assign own and opponent maps. |
| compute | `GravityKernel` | `Planes` | Produce the gravity map. |
| build | `readout::policy_logits` | maps + gravity + legal | Produce the 225 logit vector. |
| evaluate | `BaselineEvaluator` | `EvalRequest` | Return `EvalResult`. |

## Central design rule and invariants

**Central rule:** the policy readout is a trust ladder in which every
higher tier structurally dominates every lower tier, so the most urgent
tactical move always receives the highest prior mass.

Invariants:

1. `reconstruct_board` returns a board whose `to_move()` equals the
   parity-decoded side and whose empty cells equal the supplied legal
   set.
2. `analyze_both` returns Black maps at index 0 and White maps at index
   1.
3. A cell with `own.wins` receives `T1_WIN`; a cell with `opp.wins`
   receives `T2_FORCED_BLOCK`; gravity adds a value in `[0, 1]`.
4. `T2_FORCED_BLOCK` exceeds the maximum sum of T3, T4a, T4b, T4c, and
   T5; `T1_WIN` exceeds the maximum sum of T2 through T5.
5. The evaluator contains no RNG and no self-normalization; logits are
   passed to `masked_softmax` unchanged.

## Open questions

* The module doc comment in `baseline/src/lib.rs` still says
  "Burn-free (v0) policy and a flat value. The board planes are ignored."
  This text is left over from sub-chapter 01. The code now owns a
  `ThreatNet` and a `GravityKernel`. A future cleanup pass should update
  the comment.

## References

* [Sub-chapter 04 — Wins and blocks](./wins-and-blocks.md) — the
  tutorial paper.
* [Baseline primer](../00-baseline-primer.md) — design authority for the
  trust ladder and tier derivation.
* `gomoku/crates/baseline/src/adapter.rs` — reconstruction.
* `gomoku/crates/baseline/src/readout.rs` — tier constants and logits.
* `gomoku/crates/baseline/src/lib.rs` — evaluator assembly.
* `gomoku/crates/patterns/src/net.rs` — `analyze_both` indexing.
* `gomoku/crates/mcts/src/mock.rs` — reconstruction precedent.
