# Baseline tutorial — a hand-written evaluator inside MCTS

A side-quest tutorial in the rust-ml Gomoku project, sitting **after
the [convolutions tutorial](../convolutions-tutorial/README.md)** (its
threat network is consumed verbatim) and **before the net tutorial
gets worked** (so the net inherits something concrete to beat). You
build `gomoku/crates/baseline`: the project's first intelligent actor —
a policy/value evaluator written entirely by hand, plugged into the
existing PUCT search through the `mcts::eval::Evaluator` seam, and
playable from the `gomoku` terminal client.

The through-line: **the seat is playable from sub-chapter 01, and
every sub-chapter makes it smarter.** The search never changes — only
the evaluator's two answers (policy logits, value) grow layers:
vicinity gravity first, then engine-proven wins and blocks, an honest
value, the three ladder, and finally a root-level TSS probe that sees
proven forced wins. Playing strength is a **measured hypothesis**: the
same protocol reruns after every sub-chapter, and the numbers climb
(or you find out why not).

## How this tutorial works

Same conventions as the mcts, datagen, and convolutions tutorials,
with the four-paper layout of the playable-CLI sub-chapters:

- **Read the primer first.** [`00-baseline-primer.md`](00-baseline-primer.md)
  pins the semantics: the MCTS interlock, the trust ladder, the
  gravity idea, the tier derivation, the honest-heuristic framing, the
  root-only TSS wiring, and the strength-ladder contract. The chapters
  derive; the primer decides.
- **Every sub-chapter ships four papers.** The chapter
  (`NN-name/name.md`) gives the intention, the specification as tests,
  and the execution-order walkthrough. The design paper
  (`name-design.md`) tells the conceptual story and names every
  artifact. The implementation plan (`name-implementation.md`) is the
  hiding layer: very low-level construction instructions for learners
  who should not read the solution. The solution (`name-solution.md`)
  quotes the verified reference implementation — open it only after
  twenty stuck minutes, or to compare after finishing.
- **Gates per sub-chapter:** `cargo test -p baseline` (and `-p cli`
  when the CLI is touched), `cargo clippy --all-targets -- -D
  warnings`, and `cargo fmt --all -- --check`, all from `gomoku/`.
- **Commit after every sub-chapter**, with the message the chapter
  names.
- **Play after every sub-chapter.** Each one ends at the study table:
  `gomoku play --ai baseline --sims 4000`, against a seat that
  does one thing it could not do before.

## Chapter map

| # | Sub-chapter | What you add | The seat learns to… |
|---|-------------|--------------|---------------------|
| 00 | [Baseline primer](00-baseline-primer.md) | The design authority: the MCTS interlock, trust ladder, tier derivation, root-only TSS, strength ladder. | — |
| 01 | [The seat](01-the-seat/the-seat.md) | The `baseline` crate, a uniform/zero v0 evaluator, `EvaluatorKind::Baseline`, and the `--ai baseline` flag variant. | …play legal moves through *your* code path. |
| 02 | [The vicinity kernel](02-the-vicinity-kernel/the-vicinity-kernel.md) | The occupancy plane and the 5×5 gravity kernel — a one-kernel conv branch, normalized to [0, 1]. | …(nothing yet — you *see* the gravity map first). |
| 03 | [Gravity takes the wheel](03-gravity-takes-the-wheel/gravity-takes-the-wheel.md) | Policy readout v1: gravity logits, masked style. | …hug the fight instead of spraying the board. |
| 04 | [Wins and blocks](04-wins-and-blocks/wins-and-blocks.md) | Planes→`Board` reconstruction; tiers T1/T2 from both `wins` maps. | …take its own five and never allow yours. |
| 05 | [The honest value](05-the-honest-value/the-honest-value.md) | The saturating value readout, clamped away from ±1. | …know when it is winning — backup informs every branch. |
| 06 | [The three ladder](06-the-three-ladder/the-three-ladder.md) | Tiers T3–T4c: double threats, double-threes, three-makers, defensive threes. | …build forks and defend threats two moves early. |
| 07 | [The quiet whisper](07-the-quiet-whisper/the-quiet-whisper.md) | `[experiment]` A super-naive quiet-position value term (gravity differential, capped ±0.05) with a measured A/B. | …hear a whisper where the threat ladder is silent — and check whether it helps. |
| 08 | [The TSS eye](08-the-tss-eye/the-tss-eye.md) | The root probe: `prove_forced_win` once per move, budgeted, pre-empting search. | …*see* proven forced wins instead of stumbling into them. |
| 09 | [Acceptance: the measured ladder](09-acceptance/acceptance.md) | Tactical asserts, self-play flood, the match vs `TacticsEvaluator`, the sims gradient, the VCF solve rate; the default flips. | …answer for its strength in recorded numbers. |

The order is the motivation: the smallest experiencable increment
first, the socket before the smarts, and every layer of the trust
ladder in its own sub-chapter. Nothing is longer than an evening.

## Status

Redesigned 2026-10-26 (playable from sub-chapter 01, TSS probe
included, nine small sub-chapters with four papers each; sub-chapter
07 (the quiet whisper, an `[experiment]` gamble with a measured A/B)
was inserted 2026-10-26); the
original flat chapter set written 2026-10-13 is superseded (git
history keeps it). The primer is the design authority;
per-sub-chapter solutions quote the verified reference build.
**Not yet worked.** Keep this section current as sub-chapters land.

## References

- [Design spec](../../specs/2026-10-13-baseline-player-design.md) —
  the original contract; the primer supersedes its chapter order, the
  weights and protocols stand.
- [Convolutions tutorial](../convolutions-tutorial/README.md) — the
  threat network this tutorial consumes verbatim, and the
  measured-numbers acceptance pattern it extends.
- [mcts tutorial](../mcts-tutorial/README.md) — the search and the
  `Evaluator` seam; the mock `TacticsEvaluator` the baseline replaces
  and meets in the acceptance match.
- [Playable CLI tutorial](../playable-cli-tutorial/README.md) — the
  study table the seat plays at, and the four-paper sub-chapter
  layout this tutorial follows.
- [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  alpha-epsilon philosophy (§3), no-tokio (§5), crate boundaries and
  the `Evaluator` rationale (§6).
- [`docs/13-engine-design.md`](../../13-engine-design.md) — tactics
  simplifications and absolute colors, both inherited by the readouts.
