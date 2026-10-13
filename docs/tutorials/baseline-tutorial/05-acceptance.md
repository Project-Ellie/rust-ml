# Chapter 05 — Acceptance: measured games

## Abstract

This is the final chapter of the baseline tutorial, and it decides
what "done" means for a *player* — which is a different question than
"done" for a *library*. Correctness is asserted the usual way
(tactical soundness, a self-play health flood, the derivation
properties, workspace hygiene). **Strength cannot be asserted** — no
test can prove on paper that one evaluator beats another — so strength
arrives as **measured hypotheses**: falsifiable numeric claims,
pre-registered with their protocol, then run and recorded as real
numbers, whatever they turn out to be. The chapter's lasting export is
the habit: write the bar down *before* running the protocol, and
record the real number *next to the bar*, including when the number
fails it.

## Glossary

| Term | Definition |
|------|------------|
| **Assert** | A certainty verified every run: it passes or the work is not done. Reserved for behavior that *can* be proven. |
| **Measured hypothesis** | A falsifiable numeric claim about behavior, pre-registered with its protocol, then measured and recorded as a real number — never asserted on faith, never invented. |
| **Pre-registration** | Writing the bar and the protocol down before the first run. The bar's value is that it exists *before* the number it will be compared against. |
| **Match runner** | The `basematch` binary: seeded self-play matches between evaluators, printing a recordable report (protocol parameters plus results). |
| **Health flood** | Many seeded full games played end-to-end, asserting only legality, termination, and no panics — the mcts 1000-game flood, scaled down for evaluation cost. |
| **Double-threat creation** | A played move that was in `engine::double_threats` on the pre-move board — a countable event, used as the threat-construction statistic. |
| **Elo anchor seed datum** | The H1 match score, kept as the first entry in the baseline's match record. When the `arena` crate lands, ratings grow from here. |
| **Release-mode ritual** | The convention (convolutions chapter 8) that exhaustive proofs and matches run in release: unoptimized convolution is genuinely slow, and debug numbers mislead. |

## Context

Chapters 1–4 built the player: the gravity kernel, the tiered policy
readout, the saturating value readout, the adapter, and the CLI versus
mode. Everything so far was *construction*. This chapter is the
*measurement*: the baseline meets the `TacticsEvaluator` — the mock it
replaces — over a seeded match, and the tutorial's one strength claim
is tested instead of asserted.

The opponent is not a straw man. The mock's 90/10 shaping is
tactically sound for win-in-1 and forced blocks (its sentinels drive
the mcts acceptance suite), and at 400 simulations the search covers
some of what its prior lacks. The baseline's edge is everything the
mock *doesn't* see: double threats, the three-level, and a quiet-board
default. Whether that edge wins games is exactly the open question —
which is why it is a hypothesis, not a claim.

## Step 0 — re-verify the ground this tutorial stands on

This tutorial was written against the convolutions tutorial's
*final-form* contracts. Before the first acceptance run, re-verify
that the ground has not drifted (the spec's "verify when reached"
checklist); where reality disagrees, **amend the tutorial first** —
never patch the readouts to match a stale chapter.

1. **The `patterns` final form landed:**
   `gomoku/crates/patterns/src/net.rs` contains `pub struct ThreatMaps`
   with exactly the fields `wins`, `double_threats`, `double_threes`,
   `threes_per_dir`, `fours_created_per_dir`, plus `pub fn analyze`
   and `pub fn analyze_both` — semantics diffed against
   [`../convolutions-tutorial/04-the-four-level/01-solution.md`](../convolutions-tutorial/04-the-four-level/01-solution.md).
2. **The convolutions gates are still green:**
   `cargo test -p patterns --release -- --ignored`.
3. **The mcts seam is unchanged:** `Evaluator`, `EvalRequest`,
   `EvalResult`, `masked_softmax`, `search`, `visit_distribution`,
   `select_move` still match the quotes in chapter 4 and its
   references (`crates/mcts/src/`).
4. **The mock is still `mcts::mock::TacticsEvaluator`** with the
   documented 90/10 shaping and the +0.95 / −0.90 / 0.0 sentinels
   (`crates/mcts/src/mock.rs`).
5. **The CLI seams are intact:** `GameBoard::engine_board`,
   `ui::run_ui`, `ui::run_plain`, and the flag-parsing shape in
   `cli/src/main.rs`.
6. **The workspace pin still reads `burn = "=0.22.0"`**, and
   `patterns` still builds on `features = ["flex"]` with the opt-in
   `gpu` feature.
7. **The engine tactics signatures are unchanged:**
   `immediate_wins(&Board, Color) -> MoveSet`,
   `forced_blocks(&Board) -> MoveSet`,
   `double_threats(&Board, Color) -> MoveSet`.
8. **The mcts acceptance boards still exist** in
   `crates/mcts/tests/` — the win-in-1 at 50 simulations and the
   forced block at 50 simulations this chapter reuses.

## The acceptance ritual

Two kinds of checks, kept scrupulously separate.

### Asserts (certainties, verified every run)

1. **Tactical soundness** (`baseline/tests/acceptance.rs`). On the
   mcts acceptance boards, the baseline takes the win-in-1 at 50
   simulations and the forced block at 50 simulations — the same
   boards and simulation counts the mcts suite pins, now against the
   real evaluator inside the real search.
2. **Game-health flood.** 100 seeded self-play games (baseline on both
   sides, 25 simulations per move, mirroring the mcts flood's config,
   scaled 1000 → 100 for evaluation cost): every move legal, every
   game ≤ 225 plies, no panics. Scheduled like the convolutions full
   proof — `#[ignore]`d and run in release, with a short strict-prefix
   smoke (5 games, same seed range) in the default suite.
3. **Derivation checks.** The chapter-2 domination invariant and the
   chapter-3 pinned value properties (saturation, monotonicity, quiet
   ⇒ exactly 0.0) — the per-chapter suites, re-run as part of the
   ritual.
4. **Workspace hygiene.** `cargo test --workspace`;
   `cargo clippy --all-targets -- -D warnings`;
   `cargo fmt --all -- --check`; `cargo check -p engine` confirms the
   island; `cargo tree -p mcts` shows no Burn.

### Measured hypotheses (strength, recorded as real numbers)

The match runner is `baseline/src/bin/basematch.rs` — three
subcommands (`match`, `threats`, `speed`), each printing a report
header with every protocol parameter (seed, game counts, simulations,
opening-sampling plies `k`, build profile) so a recorded number is
always reproducible. Variety comes from the search side: the first
`k` plies of each game sample the visit distribution at temperature
1.0 from a seeded RNG, then play argmax (temperature 0) — `k` is
pinned in the implementation plan and recorded in the header. The
evaluator never rolls dice (chapter 4), so a seed fully determines a
match.

**H1 — headline.** In N = 200 seeded games against the
`TacticsEvaluator` (alternating colors — game *i* gives the baseline
Black iff *i* is even — 400 simulations, release mode), the baseline
wins **≥ 60 %**, draws and losses recorded separately, draws counting
as non-wins for the bar.

*Pre-registered 2026-10-13, before the first run.* The null is 50 %:
the setup is symmetric, colors alternate, and the mock shares the
baseline's tactical top tiers through the engine. Sixty is a
deliberately modest bar above the null: the baseline adds three tiers
of shaping and a quiet-board default the mock entirely lacks, so a
clear majority is expected — but first contact may surface a
mis-scaled tier or a misleading value, and the protocol's honesty rule
covers that case: **if the first real run misses the bar, the bar is
tuned once, the tuning is documented, and both numbers are recorded**
(the net-tutorial spec's I5 precedent: a starting bar the owner may
tune). What may never happen is the bar quietly moving to wherever the
number landed.

The H1 match score is also the **Elo anchor's seed datum**: when the
`arena` crate lands, "beats the baseline in ≥ 60 % of 200" becomes the
trained network's first concrete target — the symmetric echo of this
bar.

**H2 — threat construction.** Baseline self-play games contain
measurably more **double-threat creations per game** than mock
self-play games. Protocol: two populations (baseline vs baseline, mock
vs mock), M = 20 games each (a tutorial-chosen population size; the
spec pins only the statistic) at 400 simulations with the same
opening-sampling protocol as H1; after every played move, the runner
counts it as a double-threat creation iff the move was in
`engine::double_threats` on the pre-move board; report each
population's mean per game, side by side. No numeric bar — this is the
qualitative claim "gravity + three-makers produce richer positions"
made falsifiable. Whatever the means are, they are recorded.

**H3 — throughput.** Seconds per game and **evaluations per second**
at 400 simulations in release (the runner wraps the evaluator in a
counting decorator in the binary, so the number is measured, not
estimated). No bar attached: the numbers are context for the self-play
planning in docs/12. Set the expectation from the convolutions
tutorial's measurement — its three 1000-board × 2-color proofs ran
8.9 s total in release, ~620 s in debug — and treat matches and CLI
play as release-mode activities.

### Crate polish

Add `#![deny(missing_docs)]` to `baseline/src/lib.rs`, directly after
the module documentation (the convolutions chapter-8 precedent). On a
crate documented as it grew this compiles without a single new doc
comment; if it does not, write the missing docs now — they are part of
the public contract the net tutorial will read.

## Status

*Acceptance not yet run.* The table below is the pre-registration; the
measured column is filled in at the first real run, with the date and
machine, and is never a target and never invented.

| Hypothesis | Pre-registered bar (2026-10-13) | Measured |
|------------|----------------------------------|----------|
| H1 — match vs `TacticsEvaluator` | win fraction ≥ 60 % of 200 (alternating colors, 400 sims, release) | *not yet run* |
| H2 — double-threat creations / game | none — recorded statistic, baseline self-play vs mock self-play (M = 20 each) | *not yet run* |
| H3 — throughput | none — seconds/game and evals/s at 400 sims, release | *not yet run* |

## TDD checklist

The ritual, in order, all from `gomoku/`:

1. Step 0 re-verified (or the tutorial amended first).
2. `cargo test -p baseline` — default suite green: every chapter's
   tests, the tactical-soundness asserts, and the 5-game flood prefix.
3. `cargo test -p baseline --release -- --ignored` — the full 100-game
   health flood green.
4. `cargo run --release -p baseline --bin basematch -- match` — H1
   run; the report header and result recorded in the Status table.
5. `cargo run --release -p baseline --bin basematch -- threats` — H2
   recorded.
6. `cargo run --release -p baseline --bin basematch -- speed` — H3
   recorded.
7. `cargo test --workspace` green; `cargo clippy --all-targets --
   -D warnings` and `cargo fmt --all -- --check` clean;
   `cargo check -p engine` green; `cargo tree -p mcts` shows no Burn.
8. `#![deny(missing_docs)]` is in `baseline/src/lib.rs` and the crate
   compiles.

## Done when

* Step 0's eight checks re-verified — or this tutorial amended where
  reality disagreed, before any match was run.
* Every assert green: tactical soundness (both boards, 50 sims), the
  100-game flood, the derivation checks, workspace hygiene.
* The hypothesis table shows the pre-registered bar and the measured
  result side by side — real numbers, including failures, with the
  bar-tuning note if one happened.
* `cargo clippy --all-targets -- -D warnings` and
  `cargo fmt --all -- --check` clean from `gomoku/`.
* Commit message for the code slice:
  `feat(baseline): acceptance — measured games`.

## What this was for

Looking back: the project now has a *player* — hand-written top to
bottom, every weight understood, its tactics proven by inheritance
from the engine and its strength measured rather than claimed. It is
beatable by a human at the terminal, and that is a feature: something
concrete to sit down and lose to.

Looking forward — motivation, not promises:

* **The Elo anchor.** The H1 score is the first entry in the match
  record; the arena will express every trained network against this
  fixed, reproducible opponent.
* **Better self-play data.** Re-running datagen with the baseline
  evaluator yields positions shaped by gravity and three-level threats
  instead of 90/10 tactics shaping — richer phase-1 training data.
  The re-run is future work.
* **The adapter shape.** Every later subsystem — the selfplay service,
  the arena, the trained net's own evaluator adapter — reuses the
  planes → `Board` → maps → `EvalResult` shape this tutorial built
  first. The net tutorial starts with something concrete to beat.

## References

* [`../../specs/2026-10-13-baseline-player-design.md`](../../specs/2026-10-13-baseline-player-design.md)
  — §10 (asserts vs measured hypotheses), §12 (risks), §13 (verify
  when reached).
* [`00-baseline-primer.md`](00-baseline-primer.md) — §7 (acceptance by
  measurement).
* [`../convolutions-tutorial/08-acceptance.md`](../convolutions-tutorial/08-acceptance.md)
  — the measured-numbers acceptance pattern this chapter extends from
  correctness to strength, and the release-mode ritual.
* [`../../specs/2026-10-04-net-tutorial-design.md`](../../specs/2026-10-04-net-tutorial-design.md)
  — the I5 precedent: a pre-registered starting bar the owner may
  tune, with both numbers kept.
* `gomoku/crates/mcts/tests/acceptance.rs` — the tactical boards and
  the 1000-game flood this chapter reuses and scales.
