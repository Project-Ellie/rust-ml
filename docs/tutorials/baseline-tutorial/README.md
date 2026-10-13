# Baseline tutorial — a hand-written evaluator inside MCTS

A side-quest tutorial in the rust-ml Gomoku project, sitting **after
the [convolutions tutorial](../convolutions-tutorial/README.md)** (its
chapter-8 acceptance is the foundation) and **before the net tutorial
gets worked** (so the net inherits something concrete to beat). You
build `gomoku/crates/baseline`: the project's first intelligent actor —
a policy/value evaluator written entirely by hand, plugged into the
existing PUCT search through the `mcts::eval::Evaluator` seam, and
playable from the `gomoku` terminal client.

The evaluator reuses the convolutions tutorial's threat network
verbatim (`ThreatNet`, `analyze_both`, `ThreatMaps`), adds one small
new piece — a **gravity kernel** that prefers playing near existing
stones when no threat is present — and combines both sides' threat
maps plus the gravity map into tiered policy logits and a saturating
value heuristic. No training, no GPU requirement, no strength claims
on faith: playing strength is a **measured hypothesis**, recorded as
real numbers in the acceptance chapter.

The player itself is almost a by-product. The lasting value is the
**Elo anchor** every trained network will later be measured against,
the richer self-play positions a stronger hand-written evaluator will
produce for datagen, and the first artifact where `engine`, `mcts`,
and `patterns` meet.

## Prerequisite: convolutions chapters 4–8 must be COMPLETE

This tutorial consumes `patterns::net::analyze_both` and
`patterns::net::ThreatMaps` — **not just `win_map`**. On a fresh
checkout the `patterns` crate may only have the win map (convolutions
chapter 3). The four-level maps, the three-makers, the fork, and the
`ThreatMaps`/`analyze`/`analyze_both` glue land when you work
convolutions chapters 4–8. Do not start chapter 2 of this tutorial
until those chapters are done and their gates are green; chapter 5's
acceptance ritual re-verifies the full contract before the first
match (the spec's "verify when reached" checklist).

## How this tutorial works

Same conventions as the mcts, datagen, and convolutions tutorials:

- **Read the primer first.** [`00-baseline-primer.md`](00-baseline-primer.md)
  pins the semantics: the trust ladder, the gravity idea, the tier
  derivation, the honest-heuristic framing, and the
  acceptance-by-measurement contract. The chapters derive; the primer
  decides.
- **Chapters give contracts, you write the implementation.** Each
  chapter has learning goals, precise contracts, and a TDD checklist
  with exact test names. The implementation is deliberately withheld.
- **Solutions land as the tutorial is worked.** This tutorial ships
  without companion solution files. Each chapter gains its opt-in
  `NN-name/01-solution.md` — quoting the verified reference
  implementation verbatim, per the datagen/convolutions precedent —
  when the chapter is worked and its gates are green. Stuck for more
  than ~20 minutes? That is a coaching conversation, not a failure.
- **Gates per chapter:** `cargo test -p baseline`,
  `cargo clippy --all-targets -- -D warnings`, and
  `cargo fmt --all -- --check`, all from `gomoku/`.
- **Commit after every chapter**, with the message the chapter names.

## Chapter map

| # | Chapter | What you build |
|---|---------|----------------|
| 00 | [Baseline primer](00-baseline-primer.md) | The design authority: the trust ladder, gravity, the tier derivation, the honest value heuristic, acceptance by measurement. |
| 01 | [The gravity kernel](01-the-gravity-kernel.md) | Crate scaffold; the occupancy plane (any stone, no border); a one-kernel conv branch; the normalized gravity map. |
| 02 | [The policy readout](02-the-policy-readout.md) | The tier table derived from one question; binary membership; powers-of-two domination; a 225-logit vector for `masked_softmax`. |
| 03 | [The value readout](03-the-value-readout.md) | A saturating side-to-move heuristic: the sentinel ladder extended down the threat ladder, clamped away from ±1, honestly labelled. |
| 04 | [The evaluator adapter](04-the-evaluator-adapter.md) | `BaselineEvaluator` behind the `Evaluator` seam; planes → `Board` reconstruction; determinism; the CLI `--versus baseline` play mode. |
| 05 | [Acceptance: measured games](05-acceptance.md) | Tactical asserts, a self-play flood, and the pre-registered strength hypotheses run and recorded as real numbers. |

The order is deliberate: the quiet-board default before the readouts
that use it, the policy before the value, both before the adapter that
assembles the player, and everything before the match that measures it.

## Status

Written 2026-10-13 against the approved
[design spec](../../specs/2026-10-13-baseline-player-design.md);
**not yet worked**. There is no reference implementation yet, no
solution files, and no measured numbers. The per-chapter solutions and
chapter 5's measured results land as the tutorial is worked and
verified — recorded as real numbers, never targets, never invented.
Keep this section current as chapters land.

## References

- [Design spec](../../specs/2026-10-13-baseline-player-design.md) —
  the contract this tutorial implements; every weight, tier, and
  protocol number traces back to it.
- [Convolutions tutorial](../convolutions-tutorial/README.md) — the
  prerequisite: the threat network this tutorial consumes verbatim,
  and the measured-numbers acceptance pattern it extends from
  correctness to strength.
- [mcts tutorial](../mcts-tutorial/README.md) — the search and the
  `Evaluator` seam; the mock `TacticsEvaluator` the baseline replaces
  and meets in the acceptance match.
- [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  alpha-epsilon philosophy (§3), no-tokio (§5), crate boundaries and
  the `Evaluator` rationale (§6).
- [`docs/13-engine-design.md`](../../13-engine-design.md) — tactics
  simplifications and absolute colors, both inherited by the readouts.
