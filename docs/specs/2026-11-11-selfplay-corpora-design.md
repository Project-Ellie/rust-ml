# Self-play corpora design — strength-graded baseline-vs-baseline game records

**Status:** Approved 2026-11-11 (owner redirect after the net
tutorial's acceptance; see §2). **Supersedes:**
[2026-11-08-mentor-datagen-design.md](2026-11-08-mentor-datagen-design.md)
— its technical core (record format, diversity mechanics, determinism
discipline) is adopted; its mentor-imitation framing, its sequencing,
and its playing-strength gate are retired. **Audience:** the tutorial
authors. This spec is the design authority for the selfplay-corpora
tutorial track. Locked decisions of chapters 12–14 are cited;
everything else is an **invention** and marked as such. Inventions
must be presented in the tutorial as choices, not as
architecture-doc facts.

## Abstract

This document specifies a standalone tutorial track that builds the
project's strength-graded data generator: **baseline self-play
corpora**. Two seats of the `baseline` player (hand-written evaluator
+ MCTS + the `player` crate's TSS probe and guard) play complete
Swap2 games against each other at **chosen simulation budgets** —
matched pairs (baseline-800 vs baseline-800) and graded pairs
(baseline-3000 vs baseline-800) — and every game is recorded in the
project's durable **game-record format**: move list, per-ply
visit-distribution targets, per-ply root-value estimates, game
outcome, and TSS-certified endgame suffixes.

The corpora are the bootstrap fuel for the project's AlphaZero loop:
the phase-1 network trains on self-play corpora *plus* the synthetic
tactical set *plus* the retained external supplements; afterwards the
candidate network generates its own self-play data, trains on the
growing replay buffer, and is promoted when it beats the baseline at
an equal simulation budget. The record format is the same one the
milestone-5 replay buffer will consume, so baseline data flows into
the self-play loop without conversion. The generator seeds the
`selfplay` crate with a synchronous, single-threaded game runner;
worker actors and the evaluator service arrive later on the same
record format. Certified endgame suffixes feed the milestone-4 anchor
set.

The generator depends only on crates already on `main` (`engine`,
`mcts`, `baseline`, `player`, `train`).

## Glossary

- **Baseline-X** — the `baseline` player (the `baseline` evaluator
  behind the `mcts::Evaluator` seam, orchestrated by
  `player::AiPlayer` with its TSS probe and guard) running **X
  simulations per decision**. The simulation budget is the strength
  parameter: baseline-3000 is measurably stronger than baseline-200.
- **Corpus** — a set of game records produced at one declared pairing
  (e.g. baseline-800 vs baseline-800), stored as shards plus a
  manifest.
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

The synthetic tactical set (datagen tutorial) proved the training
pipeline, but its positions come from *random playouts*: its quiet
positions are off-distribution relative to actual play, and its value
targets cannot distinguish a genuinely balanced position from one
that is quietly losing. Self-play corpora fix this with real outcomes
z and search-improved policies π on positions that arise from
reasonable play.

Why **strength-graded** pairings rather than a single mentor
strength:

- **The ladder is load-bearing.** The promotion criterion of the
  AlphaZero loop is "the candidate beats the baseline at an equal
  simulation budget". That sentence only means something if
  simulation budget demonstrably grades strength. Graded corpora
  exercise and measure that relationship from the start (gate 3).
- **Outcome diversity.** Matched-strength pairs produce balanced,
  hard-fought games; graded pairs produce upsets and clear
  favorites. The mixture widens the distribution of reached
  positions and outcomes far beyond what one deterministic mentor
  pair would produce.
- **Curriculum headroom.** Early corpora at low budgets are cheap and
  fast; higher-budget corpora add search-refined targets later. The
  same generator serves both.

The external supplements and the synthetic set are retained alongside
(self-play corpora are tier 3 of the chapter-14 §5.2 signal ladder,
which they share with the external game records): a network trained
on baseline records alone is capped near baseline strength and
inherits the evaluator's biases (distribution anchoring, chapter 14
§5.4 risk 1).

Milestones touched: 3 (the bootstrap data source), 4 (certified
suffixes as anchor-set puzzle roots — on-distribution roots, better
than random-play roots), 5 (the record format and the game-runner
crate are milestone-5 components arriving early).

## 2. Owner decisions (locked 2026-11-11)

| # | Decision |
|---|---|
| S1 | Self-play corpora are a **standalone tutorial track**, built in digestible baby steps with the four-paper chapter discipline (chapter, design, implementation plan, meticulously commented solution). The reference implementation is built TDD-first in a disposable worktree, gate-verified end-to-end, then deleted; the solution files are the durable record. **A tutorial that does not run error-free is a defect.** |
| S2 | **Store games, not positions** — the record format is the final self-play format (chapter 12 §9 discipline: replay, encode, apply one random D4 transform at sample time). No interim position format, no legacy. |
| S3 | Every ply records the **policy and value information**: the visit distribution (π, the training target), the root-value estimate, and a sparse prior for audit. The game carries the outcome z. |
| S4 | **The network trains on the mixture**: self-play corpora + the synthetic tactical set + the retained external supplements (chapter 14 D6). No source is dropped. |
| S5 | **Strength grading is a primary axis**: corpora are generated at declared simulation-budget pairings, matched (X vs X) and graded (X vs Y). The ladder's first rungs are low (baseline-200, baseline-800) because cheap, fast, and diagnostic; high-budget corpora (baseline-3000+) come as needed. |
| S6 | The game runner **seeds the `selfplay` crate** (synchronous, single-threaded, one seed per process; parallelism by process). Milestone-5 worker actors and the evaluator service build on the same crate and record format. |
| S7 | **The follow-through is the AlphaZero loop, not ceremony.** Phase 1: the net trains on the mixture. Phase 2: the candidate self-plays, its games feed the replay buffer, and promotion means beating the baseline at an equal budget. The tactics-mock playing-strength probe from the superseded spec is **retired** — the first strength signal on a trained artifact comes from candidate-vs-baseline ladder matches, starting low. |
| S8 | Documentation is amended to the new direction immediately: chapters 12, 14, the how-to document, WARM-UP, AGENTS.md. |

## 3. Architecture

### 3.1 The game runner (the `selfplay` crate, first inhabitant)

A synchronous, single-threaded runner [invention]:

- Two `player::AiPlayer` seats, each owning a
  `baseline::BaselineEvaluator`. Seat configurations (simulations,
  temperature, noise, TSS probe, guard) are explicit run parameters;
  the pairing (sims per seat) is the corpus's declared identity.
- The Swap2 ceremony is executed through the engine's typestate —
  illegal openings are unrepresentable. Opening offers are drawn from
  the verified tournament opening lists (chapter 14 §3.2 tier 1)
  [invention: uniform sampling over the committed lists].
- Move selection follows the exploration schedule already used by the
  CLI's AI-vs-AI sessions [invention, mirroring existing practice]:
  temperature τ = 1 with root Dirichlet noise (ε = 0.25, α = 0.3) for
  the first ~10 plies, then τ → 0 [experiment].
- **No channels, no GPU, no evaluator service.** The runner calls the
  evaluator in-process. Parallelism is by process: one seed per
  process invocation; N parallel shells give N-fold throughput. Each
  game's record carries its seed, so any game is individually
  reproducible.

Throughput estimate [derived from measured baseline timings]: ~6 s
per game at 200 simulations single-threaded; a 10k-game low-budget
corpus is an overnight run on 12–14 parallel processes. Recalibrate
by measurement in the acceptance chapter.

### 3.2 The game-record format (durable)

```text
GameRecord
├── header
│   ├── format_version: u16
│   ├── seed: u64                    # this game's RNG stream
│   ├── config: { sims, c_puct, temperature schedule, noise,
│   │             tss probe, guard }  # per seat — the pairing lives here
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
- **The prior is stored sparse top-k** [invention, k = 16
  experiment]: an audit record for comparing the baseline's priors
  against a trained network's priors on identical positions. The
  baseline evaluator is deterministic, so the full prior is
  recomputable from the position; the stored top-k makes casual
  inspection cheap.
- **Size** [derived]: ~1–3 KB per game depending on visit-record
  width; 100k games ≈ 100–300 MB. Trivial against the memory budget
  (chapter 12 §4).
- **Flagged refinement of chapter 12 §9.** Chapter 12 sizes the
  stored game at "~60 bytes" (move list + outcome + metadata). That
  figure omits the per-ply visit distributions, without which the
  policy target degenerates to the played move (behavior cloning, not
  AlphaZero). The full record above is the correct reading of "store
  games": raw enough to re-encode under any future plane encoding,
  rich enough to train (s, π, z) without re-searching. Chapter 12
  §13 carries a dated amendment pointer to this section.
- **Serialization reuses the `train` crate's discipline**: bincode 2
  + serde, length-delimited records in fixed-capacity shard files, a
  JSON manifest (seed range, per-run configuration, game counts,
  pairing), overwrite protection.

### 3.3 The read path: from game records to training samples

At sample time (the `train` crate's sampler, extended for game
records): draw a random game, replay it through the engine to a
random ply, encode planes, and emit:

- position s — from the replay,
- policy target π — the normalized visit distribution of that ply,
- value target z — the game outcome from the side-to-move's
  perspective at that ply,
- augmentation — one random D4 transform applied to planes and policy
  (locked: chapter 12 §7, §11).

The train/holdout split is derived from the position's Zobrist hash —
the same `is_holdout` discipline as the synthetic sets, so holdout
metrics are fixed per dataset and comparable across runs and sources.

### 3.4 Certified endgames (milestone-4 feed)

The baseline's per-seat TSS probe already proves forced wins during
play (10k nodes / 30 plies budget). When the probe fires and the
forced line is then played out, the game record's `certified_suffix`
marks the ply from which the line is certified. An extraction pass
over a corpus re-verifies every certified line with `verify_line`
(the soundness gate is unchanged: 100 % must verify; the prover may
be incomplete, never unsound) and emits **anchor seeds**: (position,
forcing move, ±1) triples in the milestone-4 anchor-set format.

Scope note: budget exhaustion yields *unknown*, never *loss* — many
games will carry a certified suffix, not all, and quiet endings and
draws carry none. Absence of a certificate is "no anchor data", not a
negative label.

## 4. Diversity mechanics

A deterministic evaluator with argmax selection would generate the
same game twice. The corpus's value depends on deliberate diversity:

1. **Strength grading** (primary, decision S5): the pairing itself.
   Matched pairs for balanced games, graded pairs for upsets and
   Elo-graded outcomes. The manifest records the pairing.
2. **Openings**: Swap2 offers sampled from the tournament lists; the
   answering side's choice perturbed in self-play (chapter 14 §3.3).
3. **Exploration**: the temperature/Dirichlet schedule of §3.1.
4. **Seed discipline**: one seed per game; the corpus manifest
   records the seed set; regeneration of any single game is
   byte-exact.
5. **Deduplication**: games whose full move list collides (Zobrist
   walk equality) are rejected and re-rolled with a derived seed
   [invention].

## 5. Acceptance gates for the tutorial

1. **Determinism gate**: same seed → byte-identical game record,
   verified across process restarts.
2. **Legality gate**: every recorded game replays through the engine
   without error; every move legal; every certified suffix verifies
   (100 %).
3. **Grading gate** [experiment, bar pre-registered in the acceptance
   chapter]: at matched conditions, a higher-budget seat beats a
   lower-budget seat by a pre-registered margin over a fixed number
   of games (e.g. baseline-800 vs baseline-200). This validates the
   ladder's load-bearing assumption — simulation budget grades
   strength — and replaces the retired tactics-mock probe.
4. **Corpus gate** [experiment]: build the first corpora at the
   declared pairings; report wall time, games/hour, the game-length
   histogram (the owner's prediction: many games end tactically,
   inside ~16 plies), the opening distribution (no opening above a
   small share), and the outcome balance per pairing (no bar
   pre-registered — the first corpora establish the baseline).
5. **Integration gate**: `nettrain` reads a game-record shard set and
   trains on it, mixing sources per decision S4.

## 6. Tutorial structure (sketch)

Format precedent: the datagen and baseline tutorials (primer +
numbered chapters; each chapter ships four papers — chapter, design,
implementation plan, solution; reference implementation built
TDD-first in a disposable worktree, gate-verified, then deleted).

| Ch | Working title | Content |
|---|---|---|
| 00 | Primer | Why strength-graded self-play data; the record format as the project's durable game record; where this sits between milestones 3, 4, 5 |
| 01 | The `selfplay` crate + game runner | Two `AiPlayer` seats with per-seat budgets, Swap2 ceremony, full-game loop |
| 02 | Recording games | `GameRecord`, header/provenance (the pairing lives in the header), shard writer + manifest (reusing the datagen tutorial's chapter 5 infrastructure) |
| 03 | Recording the search | Sparse visit records, sparse priors, root values, sign conventions |
| 04 | Diversity and grading | Opening lists, temperature/Dirichlet schedule, the pairing matrix, seed discipline, process parallelism |
| 05 | Certified endgames | Suffix detection, the `verify_line` gate, anchor-seed emission |
| 06 | Reading back | Replay-to-sample derivation, holdout split, D4 augmentation at sample time, source mixing |
| 07 | Acceptance | The gates of §5, first corpora, quality report |

## 7. Documentation amendments executed with this spec

- **Chapter 12** §9: dated amendment noting the full game record
  carries per-ply visit distributions (§3.2 above). §13: milestone-3
  row gains self-play corpora; milestone-4 row notes certified
  suffixes as a puzzle-root source.
- **Chapter 14**: decision **D6** reworded to the new direction
  (self-play corpora join tier 3; external supplements retained);
  §5.2 ladder and §5.3 phase text updated; the mentor-imitation
  framing removed.
- **`docs/how-to-make-your-agent-smarter.md`**: the mentor stage
  becomes the self-play-corpora stage; this spec referenced.
- **`docs/WARM-UP.md` / `AGENTS.md`**: the tutorial queue reads
  selfplay-corpora tutorial next; the loop direction of S7 recorded.
- **The superseded spec** carries a banner pointing here.

## 8. Explicit non-goals

- No evaluator service, dynamic batching, or GPU path — milestone 5.
- No arena infrastructure — promotion matches against the baseline
  ladder arrive with the loop (S7), not with this tutorial.
- No network involvement anywhere in the generator — every seat is
  the hand-written baseline.
- No playing-strength gate against mock opponents (retired, S7).

## References

**Project documents:**

- [../12-gomoku-architecture.md](../12-gomoku-architecture.md) — §6
  (crate map, `Evaluator` seam), §7 (D4 augmentation, Zobrist), §9
  (store-games replay discipline), §12 (soundness gate), §13
  (milestones).
- [../14-openings-and-supervised-curriculum.md](../14-openings-and-supervised-curriculum.md)
  — §3 (Swap2 opening procedure, D1), §5 (signal ladder, phases,
  risks), decisions D2/D3/D5/D6.
- [../tutorials/datagen-tutorial/README.md](../tutorials/datagen-tutorial/README.md)
  — the reused infrastructure: shards, manifest, holdout split,
  soundness gate.
- [../tutorials/baseline-tutorial/README.md](../tutorials/baseline-tutorial/README.md)
  — the seats: evaluator, `player::AiPlayer`, TSS probe, guard.
- [../../gomoku/crates/cli/README.md](../../gomoku/crates/cli/README.md)
  — the CLI's AI-vs-AI session: existing temperature/Dirichlet
  exploration schedule and per-seat configuration.
- [2026-11-08-mentor-datagen-design.md](2026-11-08-mentor-datagen-design.md)
  — the superseded mentor-framing spec.

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). https://doi.org/10.1038/nature24270 —
  the (s, π, z) record format and the self-play loop this track
  bootstraps.
- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://project.dke.maastrichtuniversity.nl/games/files/phd/SearchingForSolutions.pdf
  — threat-space search.
