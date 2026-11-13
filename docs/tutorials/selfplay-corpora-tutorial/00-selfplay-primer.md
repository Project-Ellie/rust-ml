# Self-play primer — strength-graded baseline-vs-baseline game records

## Abstract

This primer is the conceptual guide and build authority for the
selfplay-corpora tutorial, the next tutorial track of milestone 3 in
the rust-ml Gomoku project. It works under the dated design spec
(listed in the References): the spec's decisions S1–S8, quoted in §2
below, remain the locked design authority of the track. The tutorial
builds the `selfplay` crate: a
deterministic generator in which two seats of the hand-written
baseline player play complete Swap2 games against each other at
declared simulation budgets — matched pairs (baseline-800 vs
baseline-800) and graded pairs (baseline-3000 vs baseline-800) — and
in which every game is stored in the project's durable game-record
format. The record carries the move list, the per-ply visit
distributions (the policy training target π), the per-ply root-value
estimates, sparse per-ply priors for audit, the game outcome (from
which the value training target z is derived), and machine-verified
endgame certificates.

The document's conclusions, stated up front. The simulation budget
is the strength axis: the later promotion criterion "the candidate
beats the baseline at an equal simulation budget" only means
something if budget demonstrably grades strength, so the corpora
exercise and measure that relationship from the start. The record
format stores complete games, not encoded planes, so the data stays
raw enough to re-encode under any future plane encoding and rich
enough to train (s, π, z) without re-searching. The runner is
synchronous and single-threaded; parallelism is by process. The
corpora join the training mixture alongside the synthetic tactical
set and the retained external supplements — no source is dropped.
Certified endgame suffixes feed the milestone-4 anchor set; the
record format and the runner are milestone-5 components arriving
early. One measurement is reported honestly and left unresolved: in
a small report-only probe, the lower-budget seat beat the
higher-budget seat 15 games to 5, against the grading expectation.

This is a reading document, not a build tutorial; the step-by-step
build instructions live in the seven chapters listed in the
[README](README.md). The document is self-contained: all
project-specific terms and all symbols are defined in the glossary
and again at first use, and material from other project documents is
summarized at the point of use. The intended reader is the learner
who has completed the engine, Monte Carlo Tree Search, baseline,
datagen, and net tutorials.

## Glossary

**Symbols used throughout** (also defined at first use in the body):

- `s` — a Gomoku position, encoded as planes only at sample time.
- `π` — the **policy target**: the normalized root visit-count
  distribution of the search at a ply (Silver et al. 2017).
- `z` — the **value target**: the game outcome in {+1, 0, −1} from
  the perspective of the side to move at a ply.
- `τ` — the move-selection **temperature**: τ = 1 samples moves
  proportionally to visit counts; τ → 0 plays the argmax (Silver et
  al. 2017).
- `ε`, `α` — the mixing weight and the concentration parameter of the
  root Dirichlet noise added to the priors during exploration plies
  (Silver et al. 2017).

**Project and domain terms:**

- **Anchor seed** — a (position, forcing move, ±1) triple extracted
  from a certified suffix; the input format of the milestone-4 puzzle
  anchor set.
- **Baseline-X** — the baseline player (the hand-written `baseline`
  evaluator behind the `mcts::Evaluator` seam, orchestrated by
  `player::AiPlayer` with its threat-space search probe and guard)
  running X search simulations per move decision. The simulation
  budget is the strength parameter.
- **Certified suffix** — a terminal segment of a game in which every
  attacker move belongs to a machine-verified forced-win line,
  produced by the engine's threat-space search prover and checked by
  `engine::verify_line`.
- **Claim labels** — the project's provenance labels (chapter 12):
  **[paper]** (stated in a cited paper), **[derived]** (computed from
  cited or measured values), **[experiment]** (a free parameter, to
  be settled by measurement). This primer also marks tutorial-local
  design choices as **[invention]**, following the design spec: they
  are choices of this track, not facts of the architecture chapters.
- **Corpus** — a set of game records produced at one declared
  pairing, stored as shards plus a manifest.
- **D4** — the dihedral group of the square: the 8 rotations and
  reflections of the board, applied as data augmentation at sample
  time.
- **Dependency island** — the project's crate rule for `engine`: no
  deep-learning framework, no input/output, no randomness inside, and
  no other crate may make `engine` depend on them. The `selfplay`
  crate depends on `engine`; never the reverse.
- **Exploration schedule** — the per-ply move-selection policy of the
  runner: temperature τ = 1 with root Dirichlet noise for the opening
  plies, then τ → 0.
- **Game record** — the durable serialization of one complete game:
  a header (configuration and provenance), per-ply records (move
  played, visit distribution, prior, root-value estimate), the
  outcome, and an optional certified suffix.
- **Holdout split** — the train/holdout partition derived from
  Zobrist hashes, shared with the synthetic datasets so that holdout
  metrics stay comparable across sources.
- **Manifest** — the JSON file next to the shards: seed range,
  per-run configuration, game counts, and the pairing. The shards are
  the data; the manifest is provenance.
- **Monte Carlo Tree Search (MCTS)** — the search both seats think
  with: repeated simulations of selection, expansion, evaluation, and
  backup over a tree, guided here by PUCT (predictor UCT; Rosin
  2011).
- **Opening offer** — the three-stone opening one side places in the
  Swap2 ceremony.
- **Pairing** — the declared simulation budgets of the two seats of a
  corpus: **matched** (X vs X) or **graded** (X vs Y). The pairing is
  the corpus's declared identity and lives in the record header.
- **Ply** — one move in a game; "ply n" is the position after n
  moves.
- **Prior** — the evaluator's policy probabilities over root moves
  before search (the P in PUCT). Recorded sparsely for audit.
- **Replay buffer** — the milestone-5 storage of self-play games that
  the training loop samples from. The game record of this tutorial is
  the format it will store.
- **Root value** — the search's value estimate at the root of the
  tree after the simulation budget is spent, from the side to move's
  perspective. Recorded as metadata, not as a training target.
- **Seat** — one of the two players of a generated game: an
  `AiPlayer` instance running the baseline evaluator at a declared
  simulation budget. Every generated game pairs two seats.
- **Shard** — a fixed-capacity file of length-delimited bincode
  records; the `train` crate's storage unit.
- **Signal ladder** — chapter 14 §5.2's ordering of training signals
  by noise, from solver-verified labels (cleanest) to self-play
  bootstrap (noisiest early, asymptotically exact).
- **Simulation budget** — the number of search simulations a seat
  runs per move decision; written baseline-X.
- **Swap2** — the opening protocol of this project: one side places
  an opening of three stones, the other side chooses a color or
  places two more stones, and the first side then chooses a color.
  Swap2 keeps self-play on balanced, unsolved ground.
- **Threat-space search (TSS)** — a proof search over forced-win
  threat sequences (Allis 1994). The engine ships a bounded prover
  and a verifier; the prover may be incomplete, never unsound.
- **Zobrist key** — a 64-bit hash of a board position, maintained
  incrementally by the engine; used for identity, deduplication, and
  the holdout split.

## 1. Why self-play corpora

### 1.1 What the synthetic set cannot say

The datagen tutorial built the synthetic tactical set: positions
drawn from random playouts, labelled by the engine's tactics
primitives (immediate win, forced block, quiet) with exact value
targets and shaped policy targets. That set proved the training
pipeline, and the net tutorial's phase-0 network trained on it. But
its positions come from *random playouts*, and two gaps follow
[design spec §1]:

- **The distribution is wrong.** Random play reaches positions that
  reasonable play rarely reaches. The synthetic set's quiet positions
  are off-distribution relative to actual play.
- **The value targets are blind in quiet positions.** The synthetic
  value target is exact about tactics and silent about everything
  else: it cannot distinguish a genuinely balanced position from one
  that is quietly losing.

Self-play corpora fix both gaps with real outcomes z and
search-improved policies π on positions that arise from reasonable
play. Every position in a corpus is on the game's natural path,
because two seats of a real player put it there.

### 1.2 The ladder is load-bearing

Why **strength-graded** pairings rather than a single fixed
self-play strength [design spec §1]:

- **The ladder is load-bearing.** The promotion criterion of the
  project's AlphaZero loop is "the candidate beats the baseline at an
  equal simulation budget". That sentence only means something if the
  simulation budget demonstrably grades strength. Graded corpora
  exercise and measure that relationship from the start (the grading
  gate, §6).
- **Outcome diversity.** Matched-strength pairs produce balanced,
  hard-fought games; graded pairs produce upsets and clear favorites.
  The mixture widens the distribution of reached positions and
  outcomes far beyond what one deterministic pair would produce.
- **Curriculum headroom.** Early corpora at low budgets are cheap
  and fast; higher-budget corpora add search-refined targets later.
  The same generator serves both.

### 1.3 A mixture, not a replacement

The corpora join the training mixture; they replace nothing
(decision S4 below, and decision D6 of
[chapter 14](../../14-openings-and-supervised-curriculum.md)):
the Phase 1 network trains on self-play corpora *plus* the synthetic
tactical set *plus* the retained external supplements. A terminology
note on phase numbers: this primer follows the design spec's S7 —
Phase 1 is mixture training, Phase 2 is candidate self-play. Chapter
14 §5.3 labels the same curriculum stages differently (the mixture
sits in its Phase 0; loop ignition is its Phase 1), and the net
tutorial's "phase-0 network" names the preceding stage from the data
side: the synthetic-only pre-training on the tactical set, which the
design spec's Phase 1 mixture follows. The underlying sequence is the
same in all three documents; only the labels differ. On the signal ladder (chapter 14 §5.2, summarized in the glossary), self-play
corpora sit on tier 3, which they share with the external game
records: search-informed but not exact. A network trained on baseline
records alone would be capped near baseline strength and would
inherit the evaluator's biases — the distribution-anchoring risk of
chapter 14 §5.4. The other sources are the counterweight.

## 2. The owner decisions (S1–S8)

The [design spec](../../specs/2026-11-11-selfplay-corpora-design.md)
locks eight decisions, S1–S8. This section quotes each decision in
the spec's wording and states what it pins for the tutorial. The
order is the design order: how the track is built and learned (S1),
what is stored (S2, S3), what the network trains on (S4), how
strength enters (S5), what shape the runner has (S6), what happens
afterwards (S7), and what the documentation does (S8).

**S1** — "Self-play corpora are a **standalone tutorial track**,
built in digestible baby steps with the four-paper chapter
discipline (chapter, design, implementation plan, meticulously
commented solution). The reference implementation is built TDD-first
in a disposable worktree, gate-verified end-to-end, then deleted; the
solution files are the durable record. **A tutorial that does not run
error-free is a defect.**"

S1 pins the form of everything the learner touches. Each of the
seven chapters ships four papers, and the implementation plan is the
hiding layer for learners who do not read the solution: signatures in
prose, method bodies never. The reference implementation already
exists — built test-driven, gate-verified end-to-end, quoted by the
solution papers — and lives only in a separate read-only worktree
(see [Reference provenance](README.md#reference-provenance) in the
README). The learner's own implementation on `main` is the reference;
the worktree is deleted once the tutorial is complete on `main`.

**S2** — "**Store games, not positions** — the record format is the
final self-play format (chapter 12 §9 discipline: replay, encode,
apply one random D4 transform at sample time). No interim position
format, no legacy."

S2 pins the durability claim. The record written by this tutorial is
not a bootstrap throwaway: it is the format the milestone-5 replay
buffer will store. Section 3 develops what "store games" means and
why planes are never written.

**S3** — "Every ply records the **policy and value information**: the
visit distribution (π, the training target), the root-value estimate,
and a sparse prior for audit. The game carries the outcome z."

S3 pins the per-ply payload. Without the visit distributions, the
policy target would degenerate to the played move — behavior cloning,
not AlphaZero (Silver et al. 2017). Section 3.2 details what is
stored sparse and what is derived at read time.

**S4** — "**The network trains on the mixture**: self-play corpora +
the synthetic tactical set + the retained external supplements
(chapter 14 D6). No source is dropped."

S4 pins the read path's obligation: the sampler must mix sources,
not just read one corpus. Section 1.3 gives the reason; §4.6 names
the artifact (`SourceMixer`).

**S5** — "**Strength grading is a primary axis**: corpora are
generated at declared simulation-budget pairings, matched (X vs X)
and graded (X vs Y). The ladder's first rungs are low (baseline-200,
baseline-800) because cheap, fast, and diagnostic; high-budget
corpora (baseline-3000+) come as needed."

S5 pins the grading axis of §1.2: the pairing is declared per corpus
and recorded in every game header. Note what the axis **is**: the
simulation budget of the two seats, not opponent diversity — both
seats always run the same hand-written baseline evaluator, and no
network plays anywhere in the generator.

**S6** — "The game runner **seeds the `selfplay` crate**
(synchronous, single-threaded, one seed per process; parallelism by
process). Milestone-5 worker actors and the evaluator service build
on the same crate and record format."

S6 pins the runner's shape. No channels, no GPU, no evaluator
service: the runner calls the evaluator in-process, and N parallel
shell processes give N-fold throughput, one seed per process
invocation. The milestone-5 service arrives later *on the same crate
and record format* — which is why S2's "no interim format" is
affordable.

**S7** — "**The follow-through is the AlphaZero loop, not ceremony.**
Phase 1: the net trains on the mixture. Phase 2: the candidate
self-plays, its games feed the replay buffer, and promotion means
beating the baseline at an equal budget. The tactics-mock
playing-strength probe from the superseded spec is **retired** — the
first strength signal on a trained artifact comes from
candidate-vs-baseline ladder matches, starting low."

S7 pins what this tutorial is *for* and what it deliberately does not
build. Phase 1 consumes this tutorial's corpora through the read path
(§4.6). Phase 2 — the candidate generating its own games, the growing
replay buffer, equal-budget promotion matches — is the AlphaZero
loop, and it arrives later; §5 draws the boundary. There is no
playing-strength gate against mock opponents anywhere in this track.

**S8** — "Documentation is amended to the new direction immediately:
chapters 12, 14, the how-to document, WARM-UP, AGENTS.md."

S8 was executed with the spec, before this tutorial was written:
chapter 12 §9 carries a dated amendment noting that the full game
record carries per-ply visit distributions (§3.2 below), §13's
milestone-3 row includes the self-play corpora, and chapter 14's
decision D6 is reworded to the mixture direction. The primer cites
the amended chapters; the learner never needs the superseded
mentor-imitation framing.

## 3. The game record — store games, not positions

### 3.1 Why games

Chapter 12 §9 locks the replay-buffer discipline: store games, and at
sample time replay the move list through the engine, encode planes,
and apply one random D4 transform. Storing encoded planes instead
would freeze one encoding into the data and forbid augmentation;
storing games keeps the data raw enough to re-encode under any future
plane encoding and rich enough to train (s, π, z) without
re-searching. The datagen tutorial applied the same principle one
level down (stone lists, not planes); the self-play record applies it
to whole games.

The design spec flags one refinement of chapter 12 §9 [design spec
§3.2]: chapter 12 sizes the stored game at "~60 bytes" (move list,
outcome, metadata). That figure omits the per-ply visit
distributions, without which the policy target degenerates to the
played move. The full record below is the correct reading of "store
games", and chapter 12 §9 carries the dated amendment with a pointer
to the spec section.

### 3.2 What one record contains

One complete game serializes as one `GameRecord`:

```text
GameRecord
├── header (GameHeader)
│   ├── format version
│   ├── seed                     # this game's random number
│   │                            # generator (RNG) stream
│   ├── per-seat configuration   # simulations, c_puct, temperature
│   │                            # schedule, noise, threat-space
│   │                            # search (TSS) probe, guard — the
│   │                            # pairing lives here (SeatPairing)
│   ├── evaluator identity       # e.g. "baseline@<workspace rev>"
│   └── opening provenance       # opening-list id + index, or
│                                # "generated"
├── plies: one RecordedPly per move
│   ├── the move played
│   ├── visits: sparse π         # (move, visit count) pairs,
│   │                            # normalized at read time
│   ├── prior: sparse top-k      # an audit record
│   └── root value               # the search's root estimate
│                                # (metadata, not a target)
├── outcome                      # win for a side, or draw; z is
│                                # derived per ply at read time
└── certified suffix (optional)  # from which ply the endgame is a
                                 # machine-verified forced-win line
```

Three storage decisions matter [design spec §3.2]:

- **π is stored sparse**, as (move, visit count) pairs over the
  root's visited children, and normalized at sample time. Under the
  baseline's concentrated policy this is roughly 10–30 entries per
  ply.
- **The prior is stored sparse top-k** [invention, k = 16
  experiment]: an audit record for comparing the baseline's priors
  against a trained network's priors on identical positions. The
  baseline evaluator is deterministic, so the full prior is
  recomputable from the position; the stored top-k makes casual
  inspection cheap.
- **Size** [derived]: roughly 1–3 KB per game depending on
  visit-record width; 100 000 games are roughly 100–300 MB — trivial
  against the project's memory budget (chapter 12 §4).

Serialization reuses the `train` crate's storage discipline from the
datagen tutorial, unchanged: bincode 2 with serde, length-delimited
records in fixed-capacity shard files, a JSON manifest (seed range,
per-run configuration, game counts, the pairing), and overwrite
protection.

## 4. One execution path — the seven chapters

The seven chapters are one pipeline. Each chapter exists because the
previous one produced something the next one consumes:

```text
 opening offers         two baseline seats
 (drawn or generated)   (player::AiPlayer)
        │                       │
        ▼                       ▼
 ch 01  play one complete Swap2 game  ──►  Game (plies carry PlySearch)
        │                                │
 ch 02  record the game  ──►  GameRecord ──►  corpus: shards + manifest
 ch 03  record the search inside the record:
        sparse visits (π), sparse prior, root value, sign conventions
        │
 ch 04  diversify and grade at scale: TournamentOpenings,
        ExplorationSchedule, the pairing matrix, seed discipline,
        dedup ──► selfplaygen
        │
 ch 05  certify endgames: verify_line gate ──► AnchorSeed ──► selfplayanchors
        │
 ch 06  read back: replay ──► sample (s, π, z) ──► D4 ──► holdout
        CorpusSampler, SourceMixer ──► selfplayexport ──► nettrain
        │
 ch 07  acceptance: determinism, legality, grading, corpus,
        integration gates ──► selfplayverify
```

The walkthrough below names each central artifact at its first
conceptual mention. The chapters derive the contracts; this primer
fixes what they must not drift from.

### 4.1 Chapter 01 — the game runner ([the-game-runner.md](01-the-game-runner/the-game-runner.md))

The `selfplay` crate's first inhabitant is a synchronous,
single-threaded game runner (decision S6). Two seats, each a
persistent `player::AiPlayer` owning a `baseline::BaselineEvaluator`,
play one complete game; the seat configurations (simulations, c_puct,
temperature, noise, TSS probe, guard — the per-seat list of §3.2) are
explicit run parameters.
The Swap2 ceremony is executed through the engine's typestate, so
illegal openings are unrepresentable. The in-memory result is a
`Game`: the opening, the plies, and the outcome. Each ply that went
through search also carries a `PlySearch` — the visit distribution,
the prior, and the root value that §3.2 stores. Nothing is written
yet; chapter 01 ends with games that replay legally through the
engine.

### 4.2 Chapter 02 — recording games ([recording-games.md](02-recording-games/recording-games.md))

`GameRecord` is the durable form of a `Game`: the header
(`GameHeader`) carries the format version, the seed, the per-seat
configuration (`SeatConfig`) and the pairing (`SeatPairing`) — the
pairing lives in the header, because the pairing is the corpus's
declared identity (decision S5) — plus evaluator identity and opening
provenance. The corpus writer streams records to length-delimited
shards on the shared `train::storage` mechanics and writes the JSON
manifest, with overwrite protection. Chapter 02 ends with corpora on
disk that read back equal to what was written.

### 4.3 Chapter 03 — recording the search ([recording-the-search.md](03-recording-the-search/recording-the-search.md))

Chapter 03 fills the per-ply payload (decision S3): the sparse visit
distribution (the training target π), the sparse top-k prior, and the
root-value estimate, with the sign conventions pinned — values are
from the side to move's perspective, and z is derived per ply from
the stored outcome at read time. One honesty rule comes from the
baseline tutorial and is load-bearing here: a move played from a
verified TSS proof carries **no search record** — the ply keeps empty
visits and priors and no root value, because fabricating statistics
for a move no search produced would be dishonest telemetry.

### 4.4 Chapter 04 — diversity and grading ([diversity-and-grading.md](04-diversity-and-grading/diversity-and-grading.md))

A deterministic evaluator with argmax selection would generate the
same game twice; the corpus's value depends on deliberate diversity
[design spec §4]:

1. **Strength grading** (primary, decision S5): the pairing itself —
   matched pairs for balanced games, graded pairs for upsets and
   outcomes graded on the Elo scale (the relative-strength rating
   named after Arpad Elo). The manifest records the pairing.
2. **Openings**: Swap2 offers are drawn from the verified tournament
   opening lists (chapter 14 §3.2 tier 1) by `TournamentOpenings`
   [invention: uniform sampling over the committed lists]; the
   answering side's choice is perturbed in self-play (chapter 14
   §3.3). An opening can also be generated rather than drawn from a
   list; the record then marks its opening provenance as "generated".
3. **Exploration**: the `ExplorationSchedule`, mirroring the CLI's
   AI-vs-AI sessions [invention, mirroring existing practice]:
   temperature τ = 1 with root Dirichlet noise (ε = 0.25, α = 0.3;
   Silver et al. 2017) for the first 10 plies, then τ → 0
   [experiment].
4. **Seed discipline**: one seed per game; the corpus manifest
   records the seed set; regeneration of any single game is
   byte-exact. Parallelism is by process (decision S6): one seed base
   per invocation.
5. **Deduplication**: games whose full move list collides (Zobrist
   walk equality) are rejected and re-rolled with a derived seed
   [invention].

Chapter 04 also lands the corpus generator binary `selfplaygen`:
declared pairings, declared seed ranges, declared output — a corpus
per invocation.

### 4.5 Chapter 05 — certified endgames (planned: [certified-endgames.md](05-certified-endgames/certified-endgames.md))

Each seat's TSS probe already proves forced wins during play (10 000
nodes / 30 plies budget). When the probe fires and the forced line is
then played out, the game record's certified suffix marks the ply
from which the line is certified. Chapter 05 runs an extraction pass
over a corpus: every certified line is re-verified with
`engine::verify_line` — the soundness gate is unchanged: 100 % must
verify; the prover may be incomplete, never unsound — and every
verified line emits an `AnchorSeed`: a (position, forcing move, ±1)
triple in the milestone-4 anchor-set format. The `selfplayanchors`
binary runs the pass.

One scope note carries the project's honesty convention [design spec
§3.4]: budget exhaustion yields *unknown*, never *loss*. Many games
carry a certified suffix, not all, and quiet endings and draws carry
none. Absence of a certificate is "no anchor data", not a negative
label.

### 4.6 Chapter 06 — reading back (planned: [reading-back.md](06-reading-back/reading-back.md))

The read path turns game records into training samples (decision S2's
discipline): draw a random game, replay it through the engine to a
random ply, encode planes, and emit position s, the normalized visit
distribution π of that ply, and the outcome z from the side to move's
perspective — plus one random D4 transform applied to planes and
policy (locked: chapter 12 §7, §11). The train/holdout split is
derived from the position's Zobrist key — the same discipline as the
synthetic sets, so holdout metrics are fixed per dataset and
comparable across runs and sources.

Three artifacts own the path. The `CorpusSampler` draws seeded random
game → ply → transform samples from a corpus. The `SourceMixer`
mixes weighted, seeded streams from several sources (decision S4:
self-play corpora plus the synthetic tactical set plus the external
supplements). The `selfplayexport` binary materializes a corpus as
`train`-format sample shards, so the existing `nettrain` binary
trains on corpora without modification.

### 4.7 Chapter 07 — acceptance (planned: [acceptance.md](07-acceptance/acceptance.md))

Chapter 07 runs the gates of §6 and publishes the quality report of
the first corpora. The verifier binary `selfplayverify` replays every
recorded game through the engine (hard error on an illegal move or an
outcome mismatch) and reports corpus statistics: game lengths,
opening distribution, outcome balance per pairing.

## 5. Where the outputs feed

Three milestones consume what this tutorial builds:

- **Milestone 3 (now).** The Phase 1 network trains on the mixture
  (decision S4): self-play corpora through the read path of §4.6,
  plus the synthetic tactical set, plus the retained external
  supplements.
- **Milestone 4.** Certified suffixes become anchor seeds
  (§4.5) — puzzle roots that are on-distribution, because they come
  from real games rather than random play.
- **Milestone 5 (early arrival).** The record format and the runner
  are milestone-5 components landing ahead of schedule: the replay
  buffer will store this tutorial's `GameRecord`, and the worker
  actors and the evaluator service will build on the `selfplay`
  crate (decision S6).

Two boundaries keep the scope honest, and the primer states them
plainly:

- **This is not the self-play service.** The milestone-5 system is a
  high-throughput service: worker actors, an evaluator service with
  dynamic batching, a graphics processing unit (GPU) path, 14 workers
  at a measured games-per-hour bar (chapter 12 §13). This tutorial builds none of that. The runner
  here is synchronous and single-threaded, and parallelism is shell
  processes (decision S6). What carries over is the crate and the
  record format — deliberately, so that nothing built here is thrown
  away.
- **This is not the AlphaZero loop.** The loop (decision S7) is Phase
  2: the candidate network self-plays, its games feed the replay
  buffer, and promotion means beating the baseline at an equal
  simulation budget. Arena infrastructure and candidate-vs-baseline
  ladder matches arrive with the loop, not with this tutorial. What
  this tutorial *does* establish is the ladder's premise: corpora at
  declared budgets, plus a grading gate that measures whether budget
  grades strength (§6).

## 6. Acceptance gates and measured evidence

The design spec §5 fixes five gates. The reference implementation has
been measured against them; the numbers below are the acceptance
evidence the chapters quote. Full detail lives in the reference
worktree's acceptance log, and the
[hand-off](../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md)
carries the headline numbers; the worktree and its raw logs are
temporary provenance, deleted only after the tutorial is complete and
learner-worked, with the measured numbers preserved in the hand-off
and quoted in the chapters.

1. **Determinism gate** — same seed, byte-identical game record,
   across process restarts. Measured: two release runs (3 games,
   baseline-8 vs baseline-8, seed base 424242) produced byte-identical
   shards (SHA-256 `4388d84e…ae52`) and manifests.
2. **Legality gate** — every recorded game replays through the engine
   without error; every certified suffix verifies. Measured:
   `selfplayverify` on 48 games replayed 48/48 games and
   1 860/1 860 moves legal with outcomes matching;
   `selfplayanchors` verified 47/47 certified suffixes (100 %).
3. **Grading gate** [experiment, bar pre-registered in the acceptance
   chapter] — at matched conditions, a higher-budget seat beats a
   lower-budget seat by a pre-registered margin over a fixed number
   of games (for example baseline-800 vs baseline-200). This gate
   validates the ladder's load-bearing assumption — simulation budget
   grades strength — and replaces the retired tactics-mock probe
   (decision S7). The pre-registered run is the acceptance chapter's
   work; the small probe already run is the open question of §7.1.
4. **Corpus gate** [experiment] — build the first corpora at the
   declared pairings and report the numbers. Measured on the first
   corpus (48 games, baseline-32 vs baseline-32, seeds
   100000–100047, recorded in the reference worktree): 122.68 s of
   wall time, **1 408 games/hour**, about 2.56 s per game in release
   mode. Outcomes: Black 23, White 24, draw 1. Game length in plies
   after the opening: min 7, median 27, mean 38.75, max 222 (the
   draw); 20.8 % of games end within 16 plies — consistent with the
   prediction that many games end tactically. Opening diversity:
   28 of 36 opening offers seen, maximum share 8.33 %. Certified
   suffixes: 47 of 48 games carry one. No outcome-balance bar is
   pre-registered; the first corpora establish the baseline.
5. **Integration gate** — `nettrain` reads a game-record shard set
   and trains on it, mixing sources per decision S4. Measured as a
   smoke test: `selfplayexport` produced 1 860 samples in 4 shards,
   and `nettrain` with the tiny configuration trained 50 steps on
   them, train loss moving from 6.548 to 6.252.

For planning, the design spec derives throughput from measured
baseline timings [derived]: roughly 6 s per game at 200 simulations
single-threaded, so a 10 000-game low-budget corpus is an overnight
run on 12–14 parallel processes. The acceptance chapter recalibrates
by measurement.

## 7. Open questions — reported, not resolved

Three observations from building and measuring the reference
implementation stay open. The chapters present them honestly; this
primer does not explain them away.

### 7.1 The grading inversion

In a report-only probe of 20 games at baseline-32 vs baseline-1, the
budget-1 seat won 15 of 20 — the opposite of the grading expectation.
The caveats are real and stated: the sample is small; the exploration
schedule (τ = 1 with root Dirichlet noise for the first 10 plies)
handicaps the stronger seat, because noise perturbs the seat whose
search would otherwise exploit the weak opponent; and the probe used
fixed colors (the stronger seat always played Black). None of these
caveats resolves the observation. The tutorial does not resolve it
either: the acceptance chapter pre-registers the real grading gate
(§6, gate 3), and the owner will explore the inversion manually.

### 7.2 The piskvork opening file

[`data/openings/piskvork-openings.txt`](../../../data/openings/piskvork-openings.txt)
line 41 contains an off-board coordinate (`-8,-6` on the 15×15
board). The opening parser correctly rejects the file — but a
generator run pointing at it then failed fast and produced no games.
The working pool is the committed Gomocup lists
([2024](../../../data/openings/gomocup-2024-freestyle15.txt),
[2025](../../../data/openings/gomocup-2025-freestyle15.txt),
[2026](../../../data/openings/gomocup-2026-freestyle15.txt)),
36 offers in total; 28 of the 36 appeared in the first corpus (§6,
gate 4). Whether and how the piskvork list joins the pool is
deliberately left open.

### 7.3 Debug-mode slowness

An 8-simulation game takes roughly 25 s in a debug build. The
consequence is procedural, not architectural: every game-playing
verification runs in release mode (`--release`), and the slow
game-playing tests are marked `#[ignore]` with a MANUAL comment and
run deliberately
(`cargo test -p selfplay --release -- --ignored`). The chapters steer
every game-playing command accordingly.

## 8. Invariants

The chapters derive; these invariants must not drift:

1. **Soundness**: 100 % of certified suffixes verify with
   `engine::verify_line`. The prover may be incomplete, never
   unsound. Absence of a certificate is "no anchor data", never a
   negative label.
2. **Determinism**: same seed, byte-identical game record, across
   process restarts. One seed per game; the manifest records the seed
   set; any single game regenerates byte-exact.
3. **Legality by construction and by check**: the Swap2 ceremony runs
   through the engine's typestate (illegal openings are
   unrepresentable), and the verifier replays every recorded game
   through the engine — every move legal, every outcome matching.
4. **Store games, never planes**: planes are encoded and augmented
   (one random D4 transform) only at sample time.
5. **π normalized at read time**: the record stores raw visit counts,
   sparse; normalization happens when the sample is drawn.
6. **z derived at read time**: the record stores the outcome once;
   the per-ply value target is computed from the side to move at read
   time, from the side to move's perspective.
7. **Honest search records**: a move played from a verified TSS proof
   carries no visit distribution, no prior, and no root value.
8. **Holdout by content**: the train/holdout split is derived from
   the position's Zobrist key, identical in discipline to the
   synthetic sets, so holdout metrics stay comparable across sources.
9. **The dependency island holds**: `selfplay` depends on `engine`,
   never the reverse; no Burn, no GPU, no network, no evaluator
   service anywhere in the generator — every seat is the hand-written
   baseline.
10. **Storage discipline reused**: bincode 2 with serde,
    length-delimited fixed-capacity shards, a JSON manifest, and
    overwrite protection — the `train` crate's mechanics, unchanged.

## 9. What this tutorial defers

- **The evaluator service, dynamic batching, and the GPU path** —
  milestone 5 (decision S6).
- **Worker actors and the continuous self-play service** — milestone
  5, on the same crate and record format.
- **Arena infrastructure and candidate-vs-baseline promotion** — the
  AlphaZero loop (decision S7), not this tutorial.
- **Any network involvement in the generator** — never: every seat is
  the hand-written baseline (the design spec's explicit non-goal).
- **A playing-strength gate against mock opponents** — retired
  (decision S7); the first strength signal on a trained artifact
  comes from candidate-vs-baseline ladder matches, starting low.
- **Resolution of the grading inversion** (§7.1) — the acceptance
  chapter pre-registers the real gate; the observation itself is the
  owner's deliberate manual exploration.
- **The piskvork opening list** (§7.2) — the Gomocup lists are the
  working pool; extending the pool is later work.

## 10. Summary

- Self-play corpora supply what the synthetic set cannot: real
  outcomes z and search-improved policies π on positions that arise
  from reasonable play.
- The simulation budget is the strength axis (decision S5); the
  later promotion criterion depends on it, so the grading gate
  measures it from the start.
- The durable record stores complete games — header with the pairing,
  per-ply sparse visits, sparse top-k prior, root value, outcome,
  optional certified suffix — never planes (decisions S2, S3).
- The seven chapters are one execution path: play, record, record the
  search, diversify and grade, certify, read back, accept.
- The runner is synchronous and single-threaded, one seed per
  process, parallelism by process (decision S6); the milestone-5
  service and the AlphaZero loop build later on the same crate and
  record format.
- Certified suffixes feed the milestone-4 anchor set under a 100 %
  `verify_line` gate; the read path feeds Phase 1 mixture training
  (decision S4).
- The acceptance evidence exists and is quoted, including the parts
  that do not fit the expectation: the grading inversion, the
  rejected piskvork file, and the debug-mode slowness are reported,
  not resolved.

## References

**Project documents:**

- [Tutorial README](README.md) — the chapter map, the conventions,
  and the reference provenance for this track.
- [Design spec: self-play corpora](../../specs/2026-11-11-selfplay-corpora-design.md) —
  the design authority: decisions S1–S8, the record format, the
  diversity mechanics, the acceptance gates.
- [Tutorial hand-off](../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  the writing brief: reference-implementation layout, measured
  acceptance evidence, and the open questions.
- [docs/12-gomoku-architecture.md](../../12-gomoku-architecture.md) —
  the system design: the `Evaluator` seam (§6), D4 augmentation and
  Zobrist keys (§7), the store-games replay discipline (§9), the
  soundness gate (§12), and the milestone table (§13). Cited here for
  the claim-label convention, the replay discipline and its dated
  amendment, and the milestone-5 service boundary.
- [docs/14-openings-and-supervised-curriculum.md](../../14-openings-and-supervised-curriculum.md) —
  the Swap2 opening procedure and tournament opening lists (§3), the
  signal ladder, phases, and distribution-anchoring risk (§5), and
  decision D6 (the mixture).
- [Datagen tutorial README](../datagen-tutorial/README.md) and
  [datagen primer](../datagen-tutorial/00-datagen-primer.md) — the
  synthetic tactical set and the `train` crate's storage discipline
  (shards, manifest, holdout split) that this track reuses.
- [Baseline tutorial README](../baseline-tutorial/README.md) and
  [baseline primer](../baseline-tutorial/00-baseline-primer.md) — the
  seats: the hand-written evaluator, `player::AiPlayer`, the root TSS
  probe and the opponent-proof guard, and the "no search record for
  certified moves" honesty rule.
- [MCTS tutorial README](../mcts-tutorial/README.md) — the PUCT
  search both seats think with.
- [Net tutorial README](../net-tutorial/README.md) — `net` and
  `nettrain`, the integration gate's consumer.
- [Engine tutorial README](../13-engine-tutorial/README.md) — the
  rules crate: Swap2 typestate, Zobrist keys, D4 transforms, the TSS
  prover and `verify_line`.
- [Convolutions tutorial README](../convolutions-tutorial/README.md) —
  the threat network the baseline evaluator builds on.
- [gomoku/crates/cli/README.md](../../../gomoku/crates/cli/README.md) —
  the CLI's AI-vs-AI session: the existing per-seat configuration and
  exploration schedule the runner mirrors.
- [data/openings/README.md](../../../data/openings/README.md) — the
  opening corpora; the Gomocup lists are the working pool (§7.2).

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). https://doi.org/10.1038/nature24270 —
  the (s, π, z) record format, temperature sampling, root Dirichlet
  noise, and the self-play loop this track bootstraps.
- Rosin, C. D. (2011). *Multi-armed bandits with episode context*
  (PUCT). https://doi.org/10.1007/s10472-011-9258-6 — the selection
  rule of the `mcts` search.
- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://doi.org/10.26481/dis.19940923la — threat-space search.
