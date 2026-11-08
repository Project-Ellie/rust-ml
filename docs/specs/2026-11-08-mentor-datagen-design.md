# Mentor datagen design — baseline-vs-baseline game records

**Status:** Approved 2026-11-08 (owner decisions from design
discussion; see §2). **Audience:** the tutorial authors. This spec is
the design authority for the mentor-datagen tutorial track. Locked
decisions of chapters 12–14 are cited; everything else is an
**invention** and marked as such. Inventions must be presented in the
tutorial as choices, not as architecture-doc facts.

## Abstract

This document specifies a standalone tutorial track that adds the
project's second data generator: **mentor self-play**. Two seats of
the existing `baseline` player (hand-written evaluator + MCTS + the
`player` crate's TSS probe and guard) play complete Swap2 games
against each other; every game is recorded in the project's durable
**game-record format** — move list, per-ply visit-distribution targets,
per-ply root-value estimates, game outcome, and TSS-certified endgame
suffixes. The record format is the same one the milestone-5 replay
buffer will consume, so mentor data flows into the self-play loop
without conversion and the tutorial exercises the milestone-5 data
path before self-play exists. The generator seeds the `selfplay`
crate with a synchronous, single-threaded game runner; the milestone-5
worker actors and evaluator service arrive later on the same record
format. Certified endgame suffixes feed the milestone-4 anchor set.
Mentor records join tier 3 of the curriculum's signal ladder
*alongside* the external engine records, which are retained as a
counterweight to mentor bias (chapter 14, decision D6). The tutorial
is queued after the net tutorial; the generator itself depends only on
crates already on `main` (`engine`, `mcts`, `baseline`, `player`,
`train`).

## Glossary

- **Mentor** — the hand-written baseline agent (the `baseline`
  evaluator behind the `mcts::Evaluator` seam, orchestrated by
  `player::AiPlayer` with its TSS probe and candidate guard). Called
  "mentor" because the network imitates it before self-play takes
  over; the term follows AlphaGomoku's rule-based-mentor curriculum
  stage (Xie, Fu, Yu 2018).
- **Mentor records** — the dataset produced by this tutorial: game
  records from mentor-vs-mentor self-play.
- **Game record** — the durable serialization of one complete game:
  header (configuration and provenance), per-ply records (move
  played, visit distribution, root-value estimate, prior), outcome.
  This is the format the milestone-5 replay buffer stores.
- **π (visit-distribution target)** — the normalized root visit-count
  distribution of the search at a given ply; the policy training
  target (AlphaGo Zero).
- **z (outcome target)** — the game result in {+1, 0, −1} from the
  perspective of the side to move at a given ply; the value training
  target.
- **Prior** — the evaluator's policy probabilities over root moves
  before search (the `P` in PUCT). Recorded sparsely for audit.
- **Certified suffix** — a terminal segment of a game in which every
  attacker move belongs to a machine-verified forced-win line (the
  engine's TSS prover + `verify_line`).
- **Shard / manifest / holdout split** — the `train` crate's storage
  discipline from the datagen tutorial: length-delimited bincode
  shards of fixed capacity, a JSON manifest, and a train/holdout
  partition derived from Zobrist hashes. Reused unchanged.
- **Claim labels** — [paper] / [derived] / [experiment] per chapter
  12: stated in a cited paper / computed from cited values / free
  parameter tuned by measurement.

## 1. Purpose and placement in the curriculum

The signal ladder (chapter 14 §5.2) orders training signals from
cleanest to noisiest: solver-verified labels, synthetic tactical data,
strong-engine game records, self-play bootstrap. Mentor records are
tier 3 — the in-house member of the game-record tier. Compared to the
external tier-3 sources (Gomocup archives, Rapfi-generated games),
mentor records have three advantages: provenance is perfect (our
engine adjudicates every position, so decision D5's re-adjudication is
satisfied by construction), licensing is trivial (self-generated,
decision D3), and the distribution matches our rules exactly
(freestyle 15×15, overlines win, Swap2).

Compared to milestone 3's synthetic tactical set, mentor records fix a
real gap: the synthetic set labels positions from *random playouts*,
so its quiet positions (value target 0) are off-distribution relative
to actual play, and its value targets cannot distinguish a quiet
position that is genuinely balanced from one that is quietly losing.
Mentor games supply real outcomes z and search-improved policies π on
positions that arise from reasonable play.

The retained external supplements counter the mentor's blind spots
(decision D6): a network trained on mentor records alone is capped
near mentor strength and inherits the baseline evaluator's biases
(distribution anchoring, chapter 14 §5.4 risk 1). The phase-2 decay
schedule applies to the mentor fraction unchanged.

Milestones touched: 3 (a second phase-0 data source), 4 (certified
suffixes as anchor-set puzzle roots — on-distribution roots, better
than the planned random-play roots), 5 (the record format and the
game-runner crate are milestone-5 components arriving early).

## 2. Owner decisions (locked 2026-11-08)

| # | Decision |
|---|---|
| M1 | Mentor datagen is a **standalone tutorial track** of its own, with deep references into the datagen tutorial where infrastructure is reused. No existing tutorial is reworked. |
| M2 | **Store games, not positions** — the record format is the final self-play format (chapter 12 §9 discipline: replay, encode, apply one random D4 transform at sample time). No interim position format, no legacy. |
| M3 | Every ply records the **policy and value information**: the visit distribution (π, the training target), the root-value estimate, and a sparse prior for audit. The game carries the outcome z. |
| M4 | **External supplements are retained** alongside mentor records, as a counterweight to mentor bias (amends chapter 14 as decision D6). |
| M5 | The game runner **seeds the `selfplay` crate** (synchronous, single-threaded); milestone 5's worker actors and evaluator service build on the same crate and record format. |
| M6 | Sequence: net tutorial first, then this tutorial, then (queued, later) the client model-integration tutorial with its journal-aware model picker. |
| M7 | Documentation is amended to the new direction immediately: chapters 12, 14, the how-to document, WARM-UP, AGENTS.md. |

## 3. Architecture

### 3.1 The game runner (the `selfplay` crate, first inhabitant)

A synchronous, single-threaded runner [invention]:

- Two `player::AiPlayer` seats, each owning a
  `baseline::BaselineEvaluator`. Seat configurations (simulations,
  temperature, noise, TSS probe, guard) are explicit run parameters.
- The Swap2 ceremony is executed through the engine's typestate —
  illegal openings are unrepresentable. Opening offers are drawn from
  the verified tournament opening lists (chapter 14 §3.2 tier 1)
  [invention: uniform sampling over the committed lists].
- Move selection follows the exploration schedule already used by the
  CLI's AI-vs-AI sessions [invention, mirroring existing practice]:
  temperature τ = 1 with root Dirichlet noise (ε = 0.25, α = 0.3) for
  the first ~10 plies, then τ → 0 [experiment].
- **No channels, no GPU, no evaluator service.** Dynamic batching is a
  milestone-5 concern; the mentor runner calls the evaluator
  in-process. Parallelism is by process: one seed per process
  invocation; N parallel shells give N-fold throughput. Each game's
  record carries its seed, so any game is individually reproducible.

Throughput estimate [derived from measured baseline timings]: ~6 s per
game at 200 simulations single-threaded; ~50 plies per game → ~50
positions per game. A 10k-game corpus (~500k positions) is a
single-threaded week or an overnight run on 12–14 parallel processes.
Recalibrate by measurement in the acceptance chapter.

### 3.2 The game-record format (durable)

```text
GameRecord
├── header
│   ├── format_version: u16
│   ├── seed: u64                    # this game's RNG stream
│   ├── config: { sims, c_puct, temperature schedule, noise,
│   │             tss probe, guard }  # per seat
│   ├── evaluator: identity string   # e.g. "baseline@<workspace rev>"
│   └── opening: provenance          # list id + index, or "generated"
├── plies: Vec<PlyRecord>
│   ├── mv: Move                     # the move played
│   ├── visits: Vec<(Move, u32)>     # sparse π; normalized at read time
│   ├── prior: Vec<(Move, f32)>      # sparse top-k audit record
│   └── root_value: f32              # search's root estimate (metadata)
├── outcome: { winner | draw }       # z is derived per ply at read time
└── certified_suffix: Option<{ from_ply: u16, line: Vec<Move> }>
```

Notes on the format:

- **π is stored sparse** as (move, visit count) pairs over the root's
  visited children; normalization happens at sample time. Under the
  baseline's concentrated policy this is ~10–30 entries per ply.
- **The prior is stored sparse top-k** [invention, k = 16 experiment]:
  an audit record for comparing the mentor's priors against a trained
  network's priors on identical positions. The baseline evaluator is
  deterministic, so the full prior is recomputable from the position;
  the stored top-k makes casual inspection cheap.
- **Size** [derived]: ~1–3 KB per game depending on visit-record
  width; 100k games ≈ 100–300 MB. Trivial against the memory budget
  (chapter 12 §4).
- **Flagged refinement of chapter 12 §9.** Chapter 12 sizes the stored
  game at "~60 bytes" (move list + outcome + metadata). That figure
  omits the per-ply visit distributions, without which the policy
  target degenerates to the played move (behavior cloning, not
  AlphaZero). The full record above — movelist, per-ply sparse visits
  and priors, outcome — is the correct reading of "store games": raw
  enough to re-encode under any future plane encoding, rich enough to
  train (s, π, z) without re-searching. Chapter 12 §13 carries a dated
  amendment pointer to this section.
- **Serialization reuses the `train` crate's discipline**: bincode 2 +
  serde, length-delimited records in fixed-capacity shard files, a
  JSON manifest (seed range, per-run configuration, game counts),
  overwrite protection. The manifest schema is extended for game
  records; the shard mechanics are imported, not duplicated.

### 3.3 The read path: from game records to training samples

At sample time (the `train` crate's sampler, extended for game
records): draw a random game, replay it through the engine to a random
ply [the store-games discipline of chapter 12 §9], encode planes, and
emit:

- position s — from the replay,
- policy target π — the normalized visit distribution of that ply,
- value target z — the game outcome from the side-to-move's
  perspective at that ply,
- augmentation — one random D4 transform applied to planes and policy
  (locked: chapter 12 §7, §11).

The train/holdout split is derived from the position's Zobrist hash —
the same `is_holdout` discipline as the synthetic sets, so mentor
holdout metrics are fixed per dataset and comparable across runs.

### 3.4 Certified endgames (milestone-4 feed)

The mentor's per-seat TSS probe already proves forced wins during play
(10k nodes / 30 plies budget). When the probe fires and the forced
line is then played out, the game record's `certified_suffix` marks
the ply from which the line is certified. An extraction pass over a
corpus re-verifies every certified line with `verify_line` (the
soundness gate is unchanged: 100 % must verify; the prover may be
incomplete, never unsound) and emits **anchor seeds**: (position,
forcing move, ±1) triples in the milestone-4 anchor-set format.

Scope note: budget exhaustion yields *unknown*, never *loss* — many
games will carry a certified suffix, not all, and quiet endings and
draws carry none. The extraction pass treats absence of a certificate
as "no anchor data", not as a negative label.

## 4. Diversity mechanics

A deterministic evaluator with argmax selection would generate the
same game twice. The corpus's value depends on deliberate diversity:

1. **Openings**: Swap2 offers sampled from the tournament lists; the
   answering side's choice perturbed in self-play (chapter 14 §3.3).
2. **Exploration**: the temperature/Dirichlet schedule of §3.1.
3. **Seed discipline**: one seed per game; the corpus manifest records
   the seed set; regeneration of any single game is byte-exact.
4. **Deduplication**: games whose full move list collides (Zobrist
   walk equality) are rejected and re-rolled with a derived seed
   [invention].
5. **Asymmetry** [experiment]: occasional per-seat simulation
   asymmetry (e.g. 400 vs 200) produces Elo-graded corpora and
   upsets, which widen the outcome distribution.

## 5. Acceptance gates for the tutorial

1. **Determinism gate**: same seed → byte-identical game record,
   verified across process restarts.
2. **Legality gate**: every recorded game replays through the engine
   without error; every move legal; every certified suffix verifies
   (100 %).
3. **Corpus gate** [experiment]: build a 1,000-game corpus; report
   wall time, games/hour, the game-length histogram, the opening
   distribution (no opening above a small share), and the outcome
   balance under Swap2 (Black/White/draw shares reported, no bar
   pre-registered — first corpus establishes the baseline).
4. **Integration gate**: `nettrain` reads a mentor shard set and
   trains on it (the net tutorial's data-loading chapter anticipates
   the game-record kind; this gate runs after both tutorials exist).
5. **Playing-strength probe** [experiment, bar pre-registered in the
   acceptance chapter]: a network trained with a mentor fraction
   beats the tactics mock at equal simulations in a client match —
   the project's first playing-strength gate on a trained artifact.

## 6. Tutorial structure (sketch)

Format precedent: the datagen tutorial (primer + numbered chapters +
opt-in solution papers; reference implementation built TDD-first in a
disposable worktree, gate-verified, then deleted).

| Ch | Working title | Content |
|---|---|---|
| 00 | Primer | Why mentor data; the record format as the project's durable game record; where this sits between milestones 3, 4, 5 |
| 01 | The `selfplay` crate + game runner | Two `AiPlayer` seats, Swap2 ceremony, full-game loop |
| 02 | Recording games | `GameRecord`, header/provenance, shard writer + manifest (reusing the datagen tutorial's chapter 5 infrastructure) |
| 03 | Recording the search | Sparse visit records, sparse priors, root values, sign conventions |
| 04 | Diversity | Opening lists, temperature/Dirichlet schedule, seed discipline, process parallelism |
| 05 | Certified endgames | Suffix detection, the `verify_line` gate, anchor-seed emission |
| 06 | Reading back | Replay-to-sample derivation, holdout split, D4 augmentation at sample time |
| 07 | Acceptance | The gates of §5, corpus build, quality report |

## 7. Documentation amendments executed with this spec

- **Chapter 12** §9: dated amendment noting the full game record
  carries per-ply visit distributions (§3.2 above). §13: milestone-3
  row gains mentor records; milestone-4 row notes certified mentor
  suffixes as a puzzle-root source.
- **Chapter 14**: new locked decision **D6** (mentor records join
  tier 3; external supplements retained); §5.2 ladder and §5.3 phase
  text updated; glossary entry added.
- **`docs/how-to-make-your-agent-smarter.md`**: mentor stage added to
  the curriculum section and milestone table; this spec referenced.
- **`docs/WARM-UP.md` / `AGENTS.md`**: direction recorded; tutorial
  queue now reads net tutorial → mentor-datagen tutorial → (queued)
  client model-integration tutorial with a journal-aware model picker.

## 8. Explicit non-goals

- No evaluator service, dynamic batching, or GPU path — milestone 5.
- No arena, no Elo gating — milestone 6. (The client-match probe of §5
  gate 5 is a tutorial acceptance ritual, not an arena.)
- Mentor records never replace the bootstrap signal; their fraction
  decays in phase 2 (chapter 14 §5.3).
- No network involvement anywhere in the generator — the mentor is
  purely the hand-written baseline.

## References

**Project documents:**

- [../12-gomoku-architecture.md](../12-gomoku-architecture.md) — §6
  (crate map, `Evaluator` seam), §7 (D4 augmentation, Zobrist), §9
  (store-games replay discipline), §12 (soundness gate), §13
  (milestones).
- [../14-openings-and-supervised-curriculum.md](../14-openings-and-supervised-curriculum.md)
  — §3 (Swap2 opening procedure, D1), §5 (signal ladder, phases,
  risks), decisions D2/D3/D5.
- [../tutorials/datagen-tutorial/README.md](../tutorials/datagen-tutorial/README.md)
  — the reused infrastructure: shards, manifest, holdout split,
  soundness gate.
- [../tutorials/baseline-tutorial/README.md](../tutorials/baseline-tutorial/README.md)
  — the mentor itself: evaluator, `player::AiPlayer`, TSS probe,
  guard.
- [../../gomoku/crates/cli/README.md](../../gomoku/crates/cli/README.md)
  — the CLI's AI-vs-AI session: existing temperature/Dirichlet
  exploration schedule and per-seat configuration.

**External sources:**

- Xie, Fu, Yu (2018). *AlphaGomoku*. https://arxiv.org/abs/1809.10595
  — the mentor-imitation curriculum stage this tutorial implements.
- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). https://doi.org/10.1038/nature24270 —
  the (s, π, z) record format.
- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://project.dke.maastrichtuniversity.nl/games/files/phd/SearchingForSolutions.pdf
  — threat-space search.
