# How to Make Your Gomoku Agent Smarter

*An operating manual for the Gomoku training pipeline: how datasets are
produced, how training runs are executed and recorded, how the
milestones that structure the work are defined and verified, and how a
trained model is loaded into the terminal client so that progress can
be played against and measured.*

---

## Abstract

This document describes the training pipeline of the Gomoku project
from the operator's perspective. It covers four topics. First, the
milestone framework: milestones 1–8 are defined, with acceptance tests,
in [docs/12-gomoku-architecture.md](12-gomoku-architecture.md) §13, and
their current status is tracked in
[docs/WARM-UP.md](WARM-UP.md); this document summarizes what each
milestone contains and how its completion is established. Second, the
supervised training pipeline of milestone 3: the `datagen` binary that
synthesizes tactical datasets, the `nettrain` binary that trains the
network on a dataset, and the artifacts both produce (shards, manifest,
checkpoint, run journal). Third, the curriculum that orders the
training signals: supervised tactics first, solver-verified puzzles
next, self-play last (locked decisions D1/D2 of
[docs/14-openings-and-supervised-curriculum.md](14-openings-and-supervised-curriculum.md)).
Fourth, the integration of trained models into the terminal client:
how a `model.mpk` checkpoint becomes a seat at the study table, how
model-versus-baseline and model-versus-model matches are run, and how
playing strength is measured before and after the self-play loop
exists. The intended reader is the operator of the pipeline; no other
document is required reading, because every external fact used here is
summarized at the point of use.

## Glossary

- **Milestone** — a unit of work defined in chapter 12 §13 with two
  properties: it ends in something executable, and it has a pass/fail
  acceptance test. Milestones are numbered 1–8.
- **Acceptance test** — the executable criterion attached to a
  milestone. A milestone is complete when its acceptance test passes
  on `main` and the result is recorded in
  [docs/WARM-UP.md](WARM-UP.md).
- **Dataset** — a directory of training samples produced by `datagen`:
  a `manifest.json` plus binary shard files. See §3.2.
- **Sample** — one training record: a board position, a policy target
  (a probability distribution over the 225 cells), and a value target
  in {+1, 0, −1}. See §3.2.
- **Checkpoint** — the file `model.mpk`: the network's weights at full
  precision, written by Burn's `DefaultRecorder`. Loadable by any
  program that knows the architecture. See §3.3 and §5.
- **Run journal** — the file `run.json`: the record of one training
  run (seed, model configuration, training configuration, final
  metrics). Plain JSON, meant to be read by humans.
- **Holdout split** — the fixed partition of a dataset into training
  and evaluation samples, derived from each sample's Zobrist hash (an
  incremental 64-bit position hash maintained by the engine). It is
  identical for every run on a given dataset. See §3.2.
- **PUCT** — predictor UCT (Rosin 2011), the prior-guided selection
  rule used by the project's MCTS implementation.
- **TSS** — threat-space search (Allis 1994): the engine's bounded
  prover for forced wins. Emits certified winning lines; used for the
  milestone-4 anchor set and, at play time, by the `player` crate.
- **Anchor set** — the milestone-4 collection of machine-verified
  forced-win puzzles with exact ±1 labels, contributing a small
  fraction of every training batch from milestone 4 onward.
- **Evaluator** — any implementation of the `mcts::Evaluator` trait:
  one method that maps a position to policy priors plus a value. The
  only coupling between the search and any source of position
  judgment. See §5.1.
- **Study table** — the terminal user interface of the `gomoku`
  binary: two seats (human or AI in any combination) on a persistent
  variation tree, with overlays for the AI's value estimate, visit
  counts, priors, and TSS proofs.
- **Claim labels** — **[paper]** marks a value stated in a cited
  paper; **[derived]** marks a value computed from cited values;
  **[experiment]** marks a free parameter chosen by this project and
  tuned by measurement. Convention defined in chapter 12.

---

## 1. Purpose and scope

The system under construction is an AlphaZero-style Gomoku agent: a
residual convolutional network trained by self-play, guided by Monte
Carlo Tree Search (MCTS), playing freestyle 15×15 Gomoku (overlines
win) under the Swap2 opening protocol. The full design is chapter 12;
this document is the practical companion for operating the training
side of that design.

The document answers four operational questions:

1. Where do the milestones come from, what does each contain, and how
   is completion established? (§2)
2. How are training datasets produced and what exactly do they
   contain? (§3.2)
3. How is a training run executed, recorded, and reproduced? (§3.3)
4. How does a trained checkpoint become an opponent at the terminal
   client, and how is its strength measured? (§5)

Section 4 places the pipeline in the curriculum that orders training
signals. Sections 6–8 give the standard operating procedures, the
metrics, and the explicit scope limits.

## 2. Milestones: origin, content, verification

### 2.1 Where the milestones are defined

The milestones originate from
[docs/12-gomoku-architecture.md](12-gomoku-architecture.md) §13, "The
runbook: milestones to a running system". That section is the
authoritative definition. Its design rule: **each milestone ends with
something executable and a pass/fail criterion.** A milestone is not a
time estimate and not a feature list; it is a verified state of the
repository.

Two further documents refine milestones without redefining them:

- [docs/14-openings-and-supervised-curriculum.md](14-openings-and-supervised-curriculum.md)
  locks the curriculum decisions that attach to milestones 3–6 (D1:
  the Swap2 opening procedure for self-play; D2: the phase structure
  summarized in §4).
- [docs/WARM-UP.md](WARM-UP.md) records the current status of each
  milestone: what is complete, what is verified, and the exact
  commands that reproduce the verification. The status section is
  updated as work lands; it is the place to check before trusting any
  claim in any document, including this one.

Completion of a milestone is established by running its acceptance
test on `main` and recording the result in WARM-UP. The standing
quality gates — `cargo test`, `cargo clippy --all-targets --
-D warnings`, `cargo fmt --all`, all green — apply to every commit and
are not a substitute for the milestone's own acceptance test.

### 2.2 The milestone table

| # | Content | Acceptance test (abbreviated from ch. 12 §13) | Status |
|---|---|---|---|
| 1 | `engine` crate: rules, bitboard Board, win detection, Zobrist keys, D4 symmetry (the 8 board symmetries) + encoding, tactics module, TSS prover, Swap2 typestate, benchmarks | property tests vs the naive oracle; 10k-game differential; TSS soundness gate at 100 %; win detection ≥ 50 M checks/s | **complete** |
| 2 | `mcts` crate: PUCT search behind the `Evaluator` trait; temperature + Dirichlet policy | solves the tactical suite (win-in-1 at 50 simulations); 1000 legal games vs a uniform evaluator | **complete** |
| 3 | `train` crate (synthetic data) + `net` crate (network + `nettrain`) + mentor self-play records (chapter 14 decision D6) | single-batch overfit test passes; >90 % top-1 on held-out synthetic threats | **in progress**: `train` + `datagen` complete and verified (50k samples in 45 s debug, 13 shards / 41 MB, 9.84 % holdout, byte-identical regeneration); `net` pending; mentor datagen specified ([spec](specs/2026-11-08-mentor-datagen-design.md)), tutorial queued after the net tutorial |
| 4 | TSS anchor set | ≥10k machine-verified forced-win puzzles; a net trained with a small anchor fraction solves >95 % of held-out proven puzzles without regressing the milestone-3 benchmark | planned |
| 5 | `selfplay` crate + evaluator service | 14 workers, stable queue depth, ≥400 games/hour measured | planned |
| 6 | The phased loop: `gomoku run` | 10 unattended iterations; arena Elo increasing over iterations; first agent beating raw-MCTS-only >90 % over 200 games | planned |
| 7 | Hardening | 7-day unattended run; crash recovery verified by kill -9 drill | planned |
| 8 | Registered upgrades, one at a time | each lands only if arena Elo improves beyond noise (±30) | planned |

Side quests outside the numbered runbook but on `main` today: the
playable terminal client (`cli` crate — the study table, Swap2
ceremony, puzzle mode, save/load), the `baseline` crate (a
hand-written evaluator behind the `Evaluator` seam), and the `player`
crate (per-seat AI orchestration: bounded TSS probe, PUCT search,
candidate guard). These matter here for one reason: they are the
integration surface that trained models plug into (§5).

### 2.3 What a milestone commits to

Because every milestone ends in an executable artifact, progress is
physically verifiable at each step. Milestone 1 produced a rules
engine that can play and adjudicate games. Milestone 2 produced a
search that solves tactical puzzles. Milestone 3 produces the first
learned component — and with it, per §5, the first trained opponent
that can be seated at the study table.

## 3. The supervised training pipeline (milestone 3)

The pipeline has two executables and two artifact kinds. `datagen`
(part of the `train` crate, complete) produces datasets. `nettrain`
(part of the `net` crate, pending) consumes a dataset and produces a
checkpoint plus a run journal.

### 3.1 Directories and artifacts

```text
gomoku/
├── data/                        # datasets, created by datagen
│   └── tactics-50k/
│       ├── manifest.json        # seed, quotas, sample counts
│       ├── shard-000.bin        # length-delimited bincode records
│       ├── shard-001.bin
│       └── ...
└── runs/                        # training runs, created by nettrain
    └── 010-default-20k/
        ├── model.mpk            # checkpoint: full-precision weights
        └── run.json             # run journal: seed, config, metrics
```

Both locations are operator choices; both binaries take `--out <DIR>`.
Keep datasets and runs in separate trees and out of version control.
A run directory named with a sequence number and the dial that changed
(`010-default-20k`, `011-more-steps`) is sufficient bookkeeping,
because `run.json` inside each directory carries the complete record.

**`model.mpk`** is written by Burn's `DefaultRecorder` at full
precision. The f16 `CompactRecorder` is disqualified for this project
(documented pitfall, [docs/11-pitfalls.md](11-pitfalls.md)): strength
comparisons between checkpoints must reflect training, not
quantization. A checkpoint is loadable by any program that knows the
architecture — by `nettrain` itself to resume or extend a run, and by
the terminal client to seat the trained model (§5.2).

**`run.json`** is the run journal: seed, model configuration, training
configuration, final step, holdout metrics. It answers "what exactly
was done in run N?" without executing anything, and it is the input to
crash recovery in the milestone-6 loop.

### 3.2 `datagen` — synthetic tactical datasets

```text
cargo run -p train --bin datagen -- \
    --out data/tactics-50k \
    --seed 1 \
    --win 20000 --block 20000 --quiet 10000
```

`datagen` plays seeded random playouts through the engine and labels
positions with the engine's own tactics module. Because the labels are
computed by the rules engine itself, the dataset cannot contain an
illegal position or a false tactic: labels are rules-true by
construction, not learned and not scraped.

**Sample content.** Each sample is one record:

- the board position,
- a **policy target**: a distribution over the 225 cells with 90 % of
  the probability mass on the tactically correct moves and 10 %
  spread uniformly over the remaining legal moves (the 90/10 shaping
  keeps the target soft, so training optimizes cross-entropy against
  a distribution rather than a one-hot label),
- a **value target**: +1 for a forced-win position, −1 for a
  must-block position, 0 for a quiet position.

**Quotas.** The three flags set the sampling quotas per tactical
class: `--win` (side to move has an immediate win), `--block` (side to
move must answer an opponent's winning threat), `--quiet` (no
immediate tactics). The defaults above produce the ~50 k reference
dataset. A quota collector samples until each class is filled, with
Zobrist-hash deduplication so no position enters twice.

**Storage.** Samples are written as length-delimited bincode records
in shard files of 4096 samples each; `manifest.json` records the seed,
the quotas, and the per-class counts. The reference dataset is 13
shards / 41 MB and generates in ~45 s in a debug build. The writer
refuses to overwrite an existing dataset directory.

**Reproducibility.** Same seed and same quotas produce byte-identical
files. A dataset can be deleted and regenerated exactly; nothing about
it is precious.

**The holdout split.** The train/holdout partition is derived from
each sample's Zobrist hash (`train::split::is_holdout`), not drawn per
run. Consequences: the holdout set is the same set of positions for
every training run ever executed on a dataset (9.84 % on the reference
dataset), so run-to-run comparisons across seeds, configurations, and
step counts measure the same positions. This fixed split is what makes
the comparison procedure in §6 sound.

`datagen` has a permanent role beyond milestone 3: it is the clean
mid-tier signal of the curriculum (§4), and its generator discipline —
engine-computed labels, seeded inputs, byte-identical regeneration —
is the template the milestone-4 anchor-set generator follows.

### 3.3 `nettrain` — supervised training runs

```text
cargo run -p net --bin nettrain -- \
    --out runs/010-default-20k \
    --data data/tactics-50k \
    --config default \
    --seed 42 \
    --steps 20000 \
    --batch-size 64 \
    --eval-interval 500
```

| Flag | Default | Meaning |
|---|---|---|
| `--out <DIR>` | *(required)* | Destination of `model.mpk` and `run.json`. |
| `--data <DIR>` | *(omitted)* | Dataset directory. When omitted, the binary generates a 200-sample set in memory — a smoke test of the machinery, not training. |
| `--config` | `tiny` | Network size. `tiny` (8 channels, 1 residual block) for second-long checks; `mid` (32 channels, 4 blocks) for experiments; `default` (128 channels, 10 residual blocks, 3.17 M parameters) is the production architecture of chapter 12 §10. |
| `--seed` | `0` | Governs weight initialization, sample shuffling, and augmentation. |
| `--steps` | `20` | Optimizer steps. |
| `--batch-size` | `16` | Samples per step. |
| `--eval-interval` | `0` | Steps between holdout evaluations; 0 evaluates only at the end. |

**Output streams.** Progress (configuration summary, step counts,
periodic holdout metrics) goes to `stderr`. The final report —
checkpoint path, initial and final training loss, closing holdout
metrics — goes to `stdout`, so `nettrain ... --out runs/011 >
report.txt` captures the summary alone.

**Exit codes.** 0: trained successfully. 2: command-line error
(unknown flag, missing `--out`). 1: command valid, run failed
(dataset unreadable, checkpoint unwritable). Scripts can branch on
this distinction.

**Augmentation.** On the training split, every sample is presented
under a random one of the eight D4 symmetries of the square board,
re-drawn each epoch (D4 is the dihedral group of the square: four
rotations and their mirrors). Holdout samples stay in canonical
orientation, so the metrics in §7 remain comparable across runs.

### 3.4 The reproducibility contract

Given a seed, every artifact the pipeline produces can be reproduced:

- `datagen --seed 1 --win 20000 --block 20000 --quiet 10000`
  regenerates the reference dataset byte-identically.
- `nettrain` with a fixed seed on a fixed dataset reproduces the final
  losses exactly (verified by the determinism procedure in §6).
- The holdout split depends only on the dataset, not on the run.

Determinism is promised on the Flex (pure-Rust CPU) backend. On the
Wgpu/Metal backend, kernel autotuning is not bitwise deterministic, so
determinism there is best-effort and recorded as such in the run
journal (chapter 12 §12 item 6).

## 4. The curriculum: three phases

Training signals ordered by noise, from cleanest to noisiest (chapter
14 §5.2):

```text
cleanest   1. solver-verified labels      exact, narrow slice of position space
           2. synthetic tactical data     rules-true by construction (datagen)
           3. strong-engine game records  strong, occasionally wrong
noisiest   4. self-play bootstrap         noisy early, exact eventually
```

The curriculum descends this ladder in three phases (locked decision
D2 of chapter 14). The rule: **a phase must earn the next one** — each
phase has a pass/fail gate, and the next phase does not start until
the gate is green.

### Phase 0 — supervised pre-training (milestone 3)

The untrained network trains on the synthetic tactical dataset of
§3.2, joined by two further supervised sources (chapter 14, decisions
D2 and D6): **mentor records** — baseline-vs-baseline self-play games,
stored as games with per-ply visit-distribution targets (π) and real
outcomes (z), which fix the synthetic set's main gap (its quiet
positions come from random playouts and all carry value target 0) —
and the verified external game-record supplements, retained as a
counterweight to mentor bias. Purpose: before any self-play game
exists, the network learns
what a five, a four, and a threat look like, so that early MCTS is
guided by a usable prior rather than by noise.

Gate (the milestone-3 acceptance protocol): a `--config default` run
on the ~50 k dataset reaches ≥80 % holdout policy-argmax agreement on
win/block samples and ≥85 % holdout value-sign accuracy (definitions
in §7). Chapter 12 §13 states the same gate as ">90 % top-1 on
held-out synthetic threats" for the tactics benchmark; both numbers
are recorded in the run journal and in WARM-UP.

### Phase 1 — ignition (milestones 4 and 5)

Two components enter.

First, the **anchor set** (milestone 4): a permanent collection of
machine-verified forced-win puzzles produced by the TSS prover. The
labels are exact — policy = the forcing move, value = rules-truth ±1 —
and are aimed at the network's weakest region, sharp tactical lines
where a weak prior makes search blunder. The anchor set lives outside
the replay buffer, is never evicted, contributes a small
[experiment] fraction of every batch from milestone 4 onward, and is
regenerated as the agent improves so that its positions track the
distribution the agent actually visits. Its puzzle roots come from
random play, Swap2 openings, and the TSS-certified endgame suffixes
of mentor games at first, from real self-play games later. Two guards apply (chapter 12
§13): soundness over completeness — only lines verified by replaying
every defender alternative become labels — and a small fraction,
because forcing positions are a narrow, atypical slice of the game and
overfeeding them degrades the policy in quiet positions.

Second, **self-play begins** (milestone 5). Games are opened under the
Swap2 protocol by the procedure of chapter 14 §3 (locked decision D1):
offers are composed to minimize the absolute value estimate |v|,
answers are value comparisons, and all evaluations go through MCTS.
The effect: games start near-balanced, which forces genuine learning.
Self-play produces the bootstrap signals — the search-improved policy
π (the normalized visit-count distribution over root moves) and the
game outcome z. These are noisy early and are the only signals that
follow the agent's own distribution of positions.

### Phase 2 — maturity (milestones 6–8)

The full loop runs unattended (milestone 6): self-play workers
generate games, the replay buffer feeds training batches, the network
updates, and an arena gates promotion — a new checkpoint replaces the
current one only if it beats it. The supervised fractions decay as
arena Elo confirms promotions [experiment: halve the anchor fraction
per confirmed promotion]; the opening mixture shifts from tournament
lists toward the network's own balance search. Milestone 7 hardens the
loop (7-day unattended run, crash recovery from the run journal).
Milestone 8 admits the registered upgrades of chapter 12 §11 — one at
a time, each only if arena Elo improves beyond noise.

The operator's role changes per phase. Phase 0: select datasets, judge
runs against the gate. Phase 1: confirm the loop is stable. Phase 2:
observe the Elo curve and decide on registered upgrades.

## 5. Playing against the trained models

This section specifies how a checkpoint produced by `nettrain` becomes
an opponent in the terminal client, and how playing strength is
measured at every stage of the curriculum. The integration design is
stated as a contract: the `net` crate and its client integration land
together in milestone 3, so the flags below are the interface to
implement, not an existing one.

### 5.1 The evaluation seam

The terminal client never sees the network directly. The coupling is a
single trait in the `mcts` crate (chapter 12 §6):

```rust
pub trait Evaluator {
    /// Evaluate one position: policy priors over legal moves + value.
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult;
}
```

An `Evaluator` receives encoded planes plus the legal-move list and
returns policy logits plus a value for the side to move. Two
implementations exist today: `mcts::TacticsEvaluator` (the hand-woven
tactics module shaping priors from engine truth) and
`baseline::BaselineEvaluator` (the hand-written evaluator: vicinity
gravity kernel, tiered threat policy, saturating value). The CLI
selects between them through `EvaluatorKind` (`cli/src/actor.rs`) and
constructs them in `make_evaluator` (`cli/src/session.rs`).

One layer above the evaluator sits the `player` crate: `AiPlayer`
owns an evaluator and orchestrates one move decision as (1) an
optional bounded TSS probe — if the engine certifies a forced win for
the side to move, play it directly — (2) a PUCT search guided by the
evaluator, and (3) an optional guard that checks the chosen candidate
against certified opponent refutations. This orchestration is
evaluator-agnostic; a network-backed seat inherits it unchanged.

### 5.2 The `NetEvaluator`: from checkpoint to seat

The third implementation is the integration point for trained models:

- **Construction.** Load `model.mpk` with Burn's `DefaultRecorder`
  (full precision; the f16 `CompactRecorder` is disqualified per §3.1)
  into the architecture named in the run journal's model
  configuration. Loading a checkpoint into a mismatched `--config` is
  a hard error, not a silent reinitialization.
- **Device.** CPU (`Device::flex()`) is the default: deterministic and
  sufficient at study-table latency. GPU (`Device::wgpu`) is a flag,
  gated on the backend parity test of chapter 12 §12 item 5.
- **Evaluation.** Encode the position through the engine's encoder
  (4 planes at 17×17, border ring as opponent stones), run one forward
  pass, return the 225 policy logits and the tanh value. No batching
  in the client: the study table evaluates one position at a time, and
  dynamic batching is a self-play-service concern (milestone 5), not a
  client concern.

### 5.3 Client integration

The `gomoku` binary gains one evaluator kind and one flag:

```text
# Human (Black) against a trained checkpoint (White)
gomoku play --ai net --model runs/010-default-20k/model.mpk

# The same, with the model on the GPU and more search
gomoku play --ai net --model runs/010-default-20k/model.mpk \
    --device wgpu --sims 800

# Model vs baseline, unattended, Enter steps the search
gomoku selfplay --ai baseline --black-ai net \
    --black-model runs/010-default-20k/model.mpk --black-sims 400
```

- `--ai net` selects the `NetEvaluator`; `--model <PATH>` (required
  with it) points at a checkpoint file.
- In `selfplay`, per-seat evaluator overrides (`--black-ai`,
  `--white-ai` with matching `--black-model`, `--white-model`) allow
  model-versus-baseline and checkpoint-versus-checkpoint matches,
  reusing the existing per-seat simulation overrides
  (`--black-sims`, `--white-sims`).
- All existing study-table instrumentation works unchanged on a
  network-backed seat, because it reads the search, not the
  evaluator's internals: the value strip shows the network's
  evaluation, the visits heatmap and prior view show the network's
  policy sharpened by search, and the TSS overlay still proves forced
  wins independently. These overlays are the direct visual evidence
  of training progress: a trained prior concentrates visits where the
  hand-written evaluators spread them.

The terminal client is the only client today. A game server and
further applications are roadmap items beyond milestone 8 (chapter 12;
WARM-UP roadmap section); the evaluator seam is the interface they
will use as well, so the integration above is the permanent
integration, not a temporary one.

### 5.4 Measuring strength before the arena exists

The milestone-6 arena provides Elo tracking from the loop onward.
Before that, and for any ad-hoc comparison, the client itself is the
measurement harness:

1. **Fixed conditions.** Compare seats at equal simulation counts
   (e.g. `--black-sims 400 --white-sims 400`), fixed seed, explicit
   `--temperature 0 --noise 0` so the opening schedule does not blur
   the measurement.
2. **Sufficient games.** `gomoku selfplay` matches in batches; win
   rates under ~50 games per pairing are noise, not signal.
3. **The strength ladder.** The known ordering to calibrate against:
   `tactics` (weakest, mock priors) < `baseline` (hand-written
   evaluator with TSS probe and guard) < a network that has earned
   promotion. The baseline acceptance record (WARM-UP) is 10/10
   against uniform-random search and 20/20 against the tactics mock
   at 200 simulations.

A phase-0 network should be expected to lose against the `baseline`
seat. The phase-0 gate (§4) measures label agreement on tactical
positions, not playing strength; the hand-written baseline additionally
carries a certified-tactics probe and a refutation guard that a raw
network evaluation does not replicate. This is a measurement, not a
defect: the phase-0 network's function is to give MCTS a non-random
prior so that self-play (phase 1) can start producing useful games.
The milestones where playing strength becomes the metric are 6 (first
agent beating raw-MCTS-only >90 % over 200 games) and 8 (each upgrade
must beat its predecessor beyond ±30 Elo). The expected observable
sequence at the study table is therefore: the phase-0 model plays
coherent tactics but loses to the baseline; early self-play iterations
close the gap; later iterations overtake it, and each overtake is
confirmed by a gated arena match before the checkpoint becomes the
default seat.

### 5.5 The end state

Milestone 6 delivers `gomoku run`: the phased loop (self-play → train
→ arena) executing unattended, with the arena promoting checkpoints
and recording Elo per iteration in the run journal. The human-facing
product of the whole pipeline is then one command — `gomoku play
--ai net --model <latest promoted checkpoint>` — and the strength of
the opponent is a measured quantity, not an assumption. Crash recovery
(milestone 7) restores the loop from the same `run.json` discipline
introduced in §3.3, so the training state is never more than one phase
away from the journal.

## 6. Standard operating procedures

A supervised session, end to end:

```text
# 1. Produce a dataset (once per dataset; regenerable on demand)
cargo run -p train --bin datagen -- --out data/tactics-50k --seed 1

# 2. Smoke-test the pipeline (seconds; tiny network, in-memory data)
cargo run -p net --bin nettrain -- --out /tmp/smoke --steps 20

# 3. A production run
cargo run -p net --bin nettrain -- \
    --out runs/010-default-20k --data data/tactics-50k \
    --config default --seed 42 --steps 20000 --eval-interval 500

# 4. Inspect the result
cat runs/010-default-20k/run.json

# 5. Play against the result
gomoku play --ai net --model runs/010-default-20k/model.mpk
```

Three procedures keep the numbers honest:

- **Determinism check.** Run the same command twice; the final losses
  must match. A mismatch means the seed discipline is broken; no
  number from that state is trustworthy.
- **Comparison discipline.** Compare only runs that share a dataset
  and differ in one flag. The fixed holdout split (§3.2) makes the
  comparison valid; the run journal makes it auditable.
- **The phase gate.** Before phase 0 is called complete: `--config
  default`, ~50 k dataset, holdout metrics at or above the §4 bar,
  read from `run.json`.

## 7. Metrics

Two holdout metrics carry the assessment of a supervised run:

- **Policy-argmax agreement** — the fraction of labeled win/block
  positions where the network's highest-logit move equals the label's
  highest-weight move. Measures tactical recognition.
- **Value-sign accuracy** — the fraction of positions where the sign
  of the network's value equals the sign of the label. Measures
  outcome assessment, and it is the quantity the Swap2 opening
  procedure (chapter 14 §3) depends on, because opening decisions are
  value comparisons.

Training loss falling is necessary but not sufficient — the
single-batch overfit test inside the `net` crate already proves the
machinery can memorize; the holdout metrics measure generalization.
Because the holdout split is fixed by position hash, both metrics are
comparable across every run on a given dataset.

## 8. Scope limits

`nettrain` deliberately does not generate self-play games, does not
call MCTS, and does not manage a replay buffer. Those functions belong
to the `selfplay` and `train` crates in milestones 5–6. The training
loop built for milestone 3 is the same loop the self-play stage
drives; only the data source changes. The one-sentence warranty, valid
in every phase: **given a seed, everything the pipeline produces can
be produced again.**

## References

**Project documents:**

- [docs/12-gomoku-architecture.md](12-gomoku-architecture.md) — system
  design; §6 (crate boundaries, `Evaluator` trait), §10 (network
  architecture), §11 (training configuration), §12 (verification
  gates), §13 (the milestone runbook — the authoritative milestone
  definition).
- [docs/13-engine-design.md](13-engine-design.md) — milestone 1 in
  detail; locked engine decisions (absolute colors, overlines win,
  TSS soundness over completeness).
- [docs/14-openings-and-supervised-curriculum.md](14-openings-and-supervised-curriculum.md)
  — the Swap2 self-play opening procedure (D1), the curriculum phases
  (D2), the signal ladder, external-data licensing.
- [docs/11-pitfalls.md](11-pitfalls.md) — full-precision recorder
  requirement; Burn API traps.
- [docs/WARM-UP.md](WARM-UP.md) — current milestone status, verified
  acceptance results, repository layout.
- [docs/tutorials/datagen-tutorial/README.md](tutorials/datagen-tutorial/README.md)
  — the `datagen` construction: sample record, quota collector,
  shards and manifest, holdout split, acceptance measurements.
- [docs/specs/2026-10-04-net-tutorial-design.md](specs/2026-10-04-net-tutorial-design.md)
  — the `nettrain` design: phase-0 acceptance bar (I5), checkpoint
  layout (I6), the binary's CLI contract (chapter table, row 07).
- [docs/specs/2026-11-08-mentor-datagen-design.md](specs/2026-11-08-mentor-datagen-design.md)
  — mentor self-play records: the game-record format (the
  milestone-5 replay-buffer format), the `selfplay`-crate game
  runner, certified endgame extraction.
- [gomoku/crates/cli/README.md](../gomoku/crates/cli/README.md) —
  the terminal client: study table, AI seats, usage.

**External sources:**

- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://project.dke.maastrichtuniversity.nl/games/files/phd/SearchingForSolutions.pdf
  — threat-space search; freestyle Gomoku is a first-player win.
- Rosin, C. D. (2011). *Multi-armed bandits with episode context*.
  Annals of Mathematics and Artificial Intelligence 61(3), 203–230.
  https://doi.org/10.1007/s10472-011-9258-6 — PUCT.
- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). Nature 550, 354–359.
  https://doi.org/10.1038/nature24270 — the self-play training loop.
- Silver, D. et al. (2018). *A general reinforcement learning
  algorithm that masters chess, shogi, and Go through self-play*
  (AlphaZero). Science 362, 1140–1144. https://arxiv.org/abs/1712.01815
  — symmetry augmentation, training configuration.
