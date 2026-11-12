# Selfplay-corpora tutorial — strength-graded baseline self-play records

## Abstract

This document introduces the selfplay-corpora tutorial, the next
tutorial track of milestone 3 in the rust-ml Gomoku project. The
tutorial builds the `selfplay` crate: a generator in which two seats
of the hand-written baseline player play complete Swap2 games against
each other at declared simulation budgets, and in which every game is
stored in the project's durable game-record format. The record carries
per-ply visit distributions, root-value estimates, sparse priors, the
game outcome, and machine-verified endgame certificates. It is the
same format the milestone-5 replay buffer will consume. The tutorial is
being written as a primer plus seven chapters. Each chapter will
have four papers: a chapter paper, a design paper, an implementation
plan, and a solution. The `selfplay` crate lands on `main` chapter
by chapter as the learner works them. A verified reference
implementation exists in a separate read-only worktree and serves
quotation only; the learner never builds against it. Read this
document before starting the primer. The intended reader is the
learner who has completed the engine, Monte Carlo Tree Search (MCTS),
baseline, datagen, and net tutorials.

## Glossary

- **Anchor seed** — a (position, forcing move, ±1) triple extracted
  from a certified suffix; the input format of the milestone-4 puzzle
  anchor set.
- **Baseline-X** — the baseline player (the hand-written `baseline`
  evaluator behind the `mcts::Evaluator` seam, orchestrated by
  `player::AiPlayer`) running X search simulations per move decision.
  The simulation budget is the strength parameter: baseline-3000 is
  stronger than baseline-200.
- **Certified suffix** — a terminal segment of a game in which every
  attacker move belongs to a machine-verified forced-win line produced
  by the engine's threat-space search prover.
- **Corpus** — a set of game records produced at one declared pairing
  (for example baseline-800 vs baseline-800), stored as shards plus a
  manifest.
- **D4** — the dihedral group of the square: the 8 rotations and
  reflections of the board, applied as data augmentation at sample
  time.
- **Game record** — the durable serialization of one complete game:
  a header (configuration and provenance), per-ply records (move
  played, visit distribution, prior, root-value estimate), the
  outcome, and an optional certified suffix.
- **Holdout split** — the train/holdout partition derived from Zobrist
  hashes, shared with the synthetic datasets so that holdout metrics
  stay comparable across sources.
- **Manifest** — a JSON file that describes a shard set: seed range,
  per-run configuration, game counts, and the pairing.
- **π (visit-distribution target)** — the normalized root visit-count
  distribution of the search at a ply; the policy training target
  (Silver et al. 2017).
- **Prior** — the evaluator's policy probabilities over root moves
  before search; recorded sparsely for audit.
- **Seat** — one of the two players of a generated game: an
  `AiPlayer` instance running the baseline evaluator at a declared
  simulation budget. Every generated game pairs two seats.
- **Shard** — a fixed-capacity file of length-delimited bincode
  records; the `train` crate's storage unit.
- **Simulation budget** — the number of search simulations a seat runs
  per move decision; written baseline-X.
- **Swap2** — the opening protocol of this project: one side places an
  opening of three stones, the other side chooses a color or places
  two more stones, and the first side then chooses a color. Swap2
  keeps self-play on balanced, unsolved ground.
- **Threat-space search (TSS)** — a proof search over forced-win
  threat sequences (Allis 1994). The engine ships a bounded prover and
  a verifier; the prover may be incomplete, never unsound.
- **z (outcome target)** — the game result in {+1, 0, −1} from the
  perspective of the side to move at a ply; the value training target.
- **Zobrist key** — a 64-bit hash of a board position, maintained
  incrementally by the engine; used for identity, deduplication, and
  the holdout split.

## Where this tutorial sits

Milestone 3 builds the bootstrap data and the first network. The
[datagen tutorial](../datagen-tutorial/README.md) built the synthetic
tactical set and the `train` crate; the
[net tutorial](../net-tutorial/README.md) trained the phase-0
network (the first supervised network, trained on synthetic data) on
that set. This tutorial adds the third data source: self-play
corpora with real outcomes z and search-improved policies π on
positions that arise from reasonable play. Per decision D6 of
[chapter 14](../../14-openings-and-supervised-curriculum.md), the
phase-1 network trains on the mixture: self-play corpora plus the
synthetic tactical set plus the retained external supplements. (Phase
labels differ across documents: the net tutorial's phase-0 network is
the synthetic-only pre-training that precedes the design spec's Phase
1 mixture; primer §1.3 reconciles the labels.) Two
further milestones consume what this tutorial builds: certified
suffixes feed the milestone-4 anchor set, and the game-record format
plus the runner are milestone-5 components arriving early.

The tutorial deliberately does **not** build:

- No evaluator service, no dynamic batching, no graphics processing
  unit (GPU) path — milestone 5. The runner is synchronous and
  single-threaded; parallelism is by process, one seed per invocation.
- No continuous self-play service, no worker actors. The generator
  runs on demand through its binaries.
- No arena infrastructure and no candidate promotion.
  Candidate-vs-baseline ladder matches arrive with the AlphaZero loop,
  not with this tutorial.
- No network involvement anywhere in the generator. Every seat is the
  hand-written baseline.
- No playing-strength gate against mock opponents. The first strength
  signal on a trained artifact comes from candidate-vs-baseline ladder
  matches, starting low.

## Prerequisites

Work these tutorials first. Each summary states what the tutorial
delivered and why this tutorial needs it.

- [Engine tutorial](../13-engine-tutorial/README.md) — builds
  `engine`, the rules crate: the bitboard `Board`, win detection,
  Zobrist keys, D4 transforms, the Swap2 typestate, and the
  threat-space search (TSS) prover and verifier. The runner drives
  every game through this crate's public API, and chapter 05
  re-verifies every certificate with it.
- [MCTS tutorial](../mcts-tutorial/README.md) — builds `mcts`, the
  MCTS implementation: PUCT (predictor UCT; Rosin 2011) selection
  over an arena tree, hidden behind an
  `Evaluator` trait seam. Both seats of every generated game think
  with this search.
- [Baseline tutorial](../baseline-tutorial/README.md) — builds
  `baseline`, the hand-written policy/value evaluator, and `player`,
  whose per-seat `AiPlayer` runs a bounded TSS proof before the search
  and guards the chosen move against certified opponent refutations.
  These two crates are the seats of every generated game. (The
  baseline tutorial itself consumes the threat network of the
  [convolutions tutorial](../convolutions-tutorial/README.md); that
  track is complete.)
- [Datagen tutorial](../datagen-tutorial/README.md) — builds `train`:
  the `Sample` record, the shard and manifest storage discipline, and
  the Zobrist-keyed holdout split. Chapter 02 writes game corpora on
  the same storage mechanics; chapter 06 exports corpora back into
  `Sample` shards.
- [Net tutorial](../net-tutorial/README.md) — builds `net` and the
  `nettrain` binary. The acceptance chapter trains `nettrain` on
  exported corpus samples as the integration gate.

## How this tutorial works

Same conventions as the datagen and net tutorials, with the
four-paper layout of the baseline tutorial:

- **Read the primer first.** [00-selfplay-primer.md](00-selfplay-primer.md)
  pins the semantics: why strength-graded self-play data, the game
  record as the project's durable format, and where this track sits
  between milestones 3, 4, and 5. The dated design spec decides; the
  primer applies it; the chapters derive.
- **Every chapter ships four papers.** The chapter paper
  (`NN-name/name.md`) gives the intention, the specification as tests,
  and the execution-order walkthrough. The design paper
  (`name-design.md`) tells the conceptual story and names every
  artifact. The implementation plan (`name-implementation.md`) is the
  hiding layer: very low-level construction instructions for learners
  who should not read the solution — signatures in prose, method
  bodies never. The solution (`name-solution.md`) quotes the verified
  reference implementation. Open it only after twenty stuck minutes,
  or afterwards to compare.
- **The rhythm is test-driven development (TDD).** The tests are the
  specification, ordered along the execution path of the future
  program. Each chapter ends with a TDD checklist, a done-when list,
  and the verification commands.
- **Gates before every commit**, from `gomoku/`:
  `cargo test -p selfplay` (plus `-p train` when the storage mechanics
  are touched and `-p player` when the proof threading is touched),
  `cargo clippy --all-targets -- -D warnings`, and
  `cargo fmt --all -- --check`. All green, no exceptions.
- **Game-playing verification runs in release mode.** Debug builds
  play slowly: an 8-simulation game takes roughly 25 s in debug. Every
  verification that plays games runs with `--release`. The slow
  game-playing tests are marked `#[ignore]` with a MANUAL comment and
  run deliberately:
  `cargo test -p selfplay --release -- --ignored`.
- **The engine stays a dependency island.** `selfplay` depends on
  `engine`, never the reverse. No Burn, no GPU, no network anywhere in
  the crate: every seat is the hand-written baseline.
- **Commit after every chapter**, with the message the chapter names.
  The `selfplay` crate lands on `main` chapter by chapter as the
  learner works the tutorial.
- Stuck more than 20 minutes → coaching conversation, or the solution
  paper. Struggle is the point; misery is not.

## Reference provenance

The chapters quote a reference implementation that was built
test-first and verified end-to-end before the tutorial was written.
It lives only in a separate, read-only worktree beside this
repository (`rust-ml-selfplay-ref`, branch `selfplay-ref-v2`, all
changes uncommitted). Its purposes are quotation and verification of
the tutorial text. Never point a `Cargo.toml` at that checkout, never
build the learner's crate from it, and never commit there. The
solution papers quote the reference; they are inspiration, not
authority — the learner's own implementation on `main` is the
reference. The worktree is deleted once the tutorial is complete on
`main`.

## Chapter map

| # | Chapter | What lands | Companion papers |
|---|---------|------------|------------------|
| 00 | [Self-play primer](00-selfplay-primer.md) | The conceptual guide under the design spec: why strength-graded self-play data, the game record as the project's durable format, the placement between milestones 3, 4, and 5. | — (the primer is a single paper) |
| 01 | [The game runner](01-the-game-runner/the-game-runner.md) | The `selfplay` crate: two `AiPlayer` seats with per-seat simulation budgets, the Swap2 ceremony through the engine typestate, the full-game loop. | [design](01-the-game-runner/the-game-runner-design.md) · [implementation plan](01-the-game-runner/the-game-runner-implementation.md) · [solution](01-the-game-runner/the-game-runner-solution.md) |
| 02 | [Recording games](02-recording-games/recording-games.md) *(planned)* | `GameRecord` with header and provenance (the pairing lives in the header); the corpus shard writer and manifest on the shared `train::storage` mechanics. | [design](02-recording-games/recording-games-design.md) · [implementation plan](02-recording-games/recording-games-implementation.md) · [solution](02-recording-games/recording-games-solution.md) |
| 03 | [Recording the search](03-recording-the-search/recording-the-search.md) *(planned)* | Sparse per-ply visit records (the policy target π), sparse top-k priors, root-value estimates, sign conventions. | [design](03-recording-the-search/recording-the-search-design.md) · [implementation plan](03-recording-the-search/recording-the-search-implementation.md) · [solution](03-recording-the-search/recording-the-search-solution.md) |
| 04 | [Diversity and grading](04-diversity-and-grading/diversity-and-grading.md) *(planned)* | Tournament opening lists, the exploration schedule (temperature 1 with root Dirichlet noise (Silver et al. 2017) for the opening plies, then greedy), the pairing matrix, seed discipline with deduplication, the `selfplaygen` binary. | [design](04-diversity-and-grading/diversity-and-grading-design.md) · [implementation plan](04-diversity-and-grading/diversity-and-grading-implementation.md) · [solution](04-diversity-and-grading/diversity-and-grading-solution.md) |
| 05 | [Certified endgames](05-certified-endgames/certified-endgames.md) *(planned)* | Suffix detection from the seat's own TSS proof, the 100 % `verify_line` soundness gate, anchor-seed extraction, the `selfplayanchors` binary. | [design](05-certified-endgames/certified-endgames-design.md) · [implementation plan](05-certified-endgames/certified-endgames-implementation.md) · [solution](05-certified-endgames/certified-endgames-solution.md) |
| 06 | [Reading back](06-reading-back/reading-back.md) *(planned)* | Replay-to-sample derivation, the holdout split, D4 augmentation at sample time, weighted source mixing, the `selfplayexport` binary. | [design](06-reading-back/reading-back-design.md) · [implementation plan](06-reading-back/reading-back-implementation.md) · [solution](06-reading-back/reading-back-solution.md) |
| 07 | [Acceptance](07-acceptance/acceptance.md) *(planned)* | The determinism, legality, grading, corpus, and integration gates; the first corpora and their quality report; the `selfplayverify` binary. | [design](07-acceptance/acceptance-design.md) · [implementation plan](07-acceptance/acceptance-implementation.md) · [solution](07-acceptance/acceptance-solution.md) |

The chapter-01 chapter link and its companion-paper links are live.
The chapter links 02–07 and their companion-paper links name planned
documents (marked *(planned)*), consistently with the primer's
per-chapter headings. The README, the primer, and all chapter-1 paper
links are live; the chapter paper sets 2–7 remain planned. The order
is the dependency chain: a game before its record, the
record before the search data inside it, recorded games before
diversity at scale, corpora before the certificates they carry, and
everything before reading back and acceptance.

## Status

- 2026-11-13 — the tutorial is **being written**. This README, the
  [primer](00-selfplay-primer.md), and the chapter-1 four-paper set
  ([chapter](01-the-game-runner/the-game-runner.md),
  [design](01-the-game-runner/the-game-runner-design.md),
  [implementation plan](01-the-game-runner/the-game-runner-implementation.md),
  [solution](01-the-game-runner/the-game-runner-solution.md)) are
  written; the chapter paper sets 2–7 remain planned. Nothing is learner-worked: the `selfplay` crate
  does not exist on `main`, and no chapter has landed. The reference implementation is
  built and gate-verified in the read-only reference worktree
  described above: the workspace gates are green, the `selfplay` suite
  reports 50 passed / 4 ignored, two identical release runs produce
  byte-identical shards, the verifier replays 48/48 games with every
  move legal, and a 48-game low-budget corpus measures roughly
  1 400 games/hour. Two further observations keep the picture honest.
  A report-only 20-game 32-vs-1 grading probe had the budget-1 seat
  win 15/20 against the budget-32 seat — an unresolved observation
  discussed honestly in chapters 4 and 7 (small sample, the
  exploration schedule handicapping the stronger seat, fixed colors);
  the tutorial does not explain it away. The integration smoke
  exported 1 860 samples in 4 shards and trained the tiny `nettrain`
  configuration for 50 steps on them, with train loss moving from
  6.548 to 6.252.

Keep this section current as chapters land.

## References

**Project documents:**

- [Design spec: self-play corpora](../../specs/2026-11-11-selfplay-corpora-design.md) —
  the design authority for this track: decisions S1–S8, the
  game-record format, the diversity mechanics, and the acceptance
  gates.
- [Tutorial hand-off](../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  the writing brief: reference-implementation layout, per-chapter
  public API, measured acceptance evidence, and open questions the
  chapters must address honestly.
- [docs/12-gomoku-architecture.md](../../12-gomoku-architecture.md) —
  the system design: crate map and the `Evaluator` seam (§6), D4
  augmentation (§7), the store-games replay discipline (§9), the
  soundness gate (§12), and the milestone table (§13).
- [docs/14-openings-and-supervised-curriculum.md](../../14-openings-and-supervised-curriculum.md) —
  the Swap2 opening procedure (§3), the signal ladder, the training
  phases, and decision D6 (§5).
- [docs/WARM-UP.md](../../WARM-UP.md) — the maintained project
  context and the tutorial queue.
- [Engine tutorial](../13-engine-tutorial/README.md),
  [MCTS tutorial](../mcts-tutorial/README.md),
  [baseline tutorial](../baseline-tutorial/README.md),
  [datagen tutorial](../datagen-tutorial/README.md),
  [net tutorial](../net-tutorial/README.md),
  [convolutions tutorial](../convolutions-tutorial/README.md) — the
  prerequisite tracks; see the Prerequisites section for what each
  delivers.
- [gomoku/crates/cli/README.md](../../../gomoku/crates/cli/README.md) —
  the CLI's AI-vs-AI session: the existing per-seat configuration and
  exploration schedule that the runner mirrors.

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). https://doi.org/10.1038/nature24270 —
  the (s, π, z) record format, root Dirichlet noise, and the self-play
  loop this track bootstraps.
- Rosin, C. D. (2011). *Multi-armed bandits with episode context*
  (PUCT). https://doi.org/10.1007/s10472-011-9258-6 — the selection
  rule of the `mcts` search.
- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://doi.org/10.26481/dis.19940923la — threat-space search.
