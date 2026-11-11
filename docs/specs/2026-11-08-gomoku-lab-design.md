# Gomoku lab — training laboratory console inside the study table

**Status:** SUPERSEDED 2026-11-08 by the gomoku-gui lab spec
(`gomoku-gui/docs/specs/2026-11-08-gomoku-gui-lab-design.md`) — the
lab's home is the native GUI, not the terminal study table. Kept for
history. Original text: design proposal for owner review, written
2026-11-08. **Not
locked.** This spec designs a set of CLI/TUI capabilities that turn the
existing study table into the project's training laboratory. Authority
order where documents disagree: chapters 12/13/14 locked decisions →
[2026-11-08-mentor-datagen-design.md](2026-11-08-mentor-datagen-design.md)
→ the tutorial chapters → this spec.

**Audience:** Wolfie and future tutorial authors who will implement the
lab pieces chapter by chapter.

## Abstract

This document specifies how the Gomoku CLI's study table becomes the
project's training laboratory ("the lab"). The lab lives inside the
existing `gomoku` binary, not as a separate console, because the study
table already owns the project's best visualization surface: a board
with stones, policy/visit/prior/TSS overlays, seat configurations, and a
command language. The lab adds read-only inspection of corpora and
runs, a candidate-model registry, training launch + live watch, and
resume-from-snapshot — all mediated through the same typed command
language and, eventually, form-like TUI screens.

Every invention is marked [invention]; locked decisions are cited; any
proposal that amends a locked decision is flagged explicitly.

## Glossary

- **Study table** — the existing terminal Gomoku UI: a persistent
  variation tree, two seats, typed command language, and board overlays.
- **Lab mode** — a top-level screen family in the CLI (corpus
  inspection, run registry, candidate registry, training launcher/watch,
  resume manager) [invention].
- **Corpus** — a directory of length-delimited bincode shards plus a
  JSON `manifest.json`, per the datagen tutorial's storage discipline.
- **Run** — a `nettrain` output directory holding `model.mpk` (full-
  precision model record), `run.json` (run journal), and optionally
  `metrics.jsonl` (per-step/per-eval metrics) [invention for
  metrics.jsonl].
- **Run journal** — `run.json`: seed, `ModelConfig`, `TrainConfig`,
  final step/eval metrics. The unit of comparison between runs.
- **metrics.jsonl** — proposed run-metrics record: one JSON line per
  training step and per holdout evaluation [invention].
- **Candidate** — a run directory promoted to a named, tagged model that
  can be sent to a seat [invention].
- **Snapshot** — a full training-state checkpoint (model + optimizer
  state + LR scheduler state + RNG/step position + corpus identity)
  enabling resume [invention].
- **Seat** — one of the two sides (Black/White) in the study table; an
  AI seat uses an `mcts::Evaluator`.
- **Overlay renderer** — the existing `cli` policy/visit/prior heatmap
  and TSS proof rendering machinery.
- **analysis_revision** — the existing runtime counter that invalidates
  derived analysis when session state changes.
- **Claim labels** — [paper] / [derived] / [experiment] / [invention] per
  chapter 12; this spec adds [measured] where a value was directly
  observed on this project.

## 1. Why the lab lives inside the study table

The phase-0 training journey
([2026-11-08-phase-0-training-journey.md](2026-11-08-phase-0-training-journey.md))
ends with a trained model at the study table. The same journey repeats:
corpus → run → candidate → seat. A separate training console would
reinvent four things the study table already has:

1. **Board rendering with overlays.** The flagship view of a trained
   model is its policy output painted as a heatmap over the stones.
   The study table's overlay renderer already does this for MCTS
   visits and priors; extending it to a raw network policy on an
   arbitrary position is the same renderer with a different
   `SearchInfo` source.
2. **Seat machinery.** Installing a model as an AI seat already exists
   (`model DIR`). The lab generalizes this to a candidate registry with
   tags, promotion, and per-seat dispatch.
3. **Command language and shell/core split.** The `Command` enum and
   `shell_command::execute` split keep terminal code out of the state
   machine. Lab commands are new variants; the split stays unchanged.
4. **Variation tree.** Inspecting a corpus sample or a game record is
   just loading a position into the tree and rendering it.

The study table also has a property no text console can match: when
Wolfie inspects a sample, a run's policy, or a candidate's prior, the
information is displayed on the board together with the stones.
Therefore the lab is **not a new binary**; it is a set of new commands,
new shell-level screens, and new data readers inside the existing `cli`
crate [invention].

## 2. Lab modes and screens

The lab is organized as modes. Each mode is reachable from the study
table command line and renders in the same full-screen shell, falling
back to line mode where appropriate. None of these modes replace the
study table; they are orthogonal views that can push/pop or swap the
visible frame while the underlying session core remains available.

### 2.1 Corpora inspection

**Purpose:** browse the contents of any corpus directory without writing
new data tools.

**Capabilities [invention]:**

- List corpora under a configured root (default `data/`).
- Open one corpus: show its `manifest.json` (seed, quotas/counts,
  shards, workspace version).
- Browse samples by index or by class filter (win/block/quiet).
- Render a sample as a board position in the study table's board
  renderer.
- Overlay the sample's policy target on the board (the policy heatmap
  renderer reused with the sample's π instead of MCTS visits).
- Step through shards or search for a Zobrist key.
- For mentor/game-record corpora (future): replay a game to a chosen
  ply and inspect the stored visit distribution, prior, root value,
  and certified suffix.

**Why this belongs in the CLI:** a corpus is a collection of positions;
positions are boards; boards are the study table's native content.

### 2.2 Run registry

**Purpose:** browse, compare, and diagnose training runs.

**Capabilities [invention]:**

- List runs under a configured root (default
  `artifacts/gomoku/runs/`).
- Show a run's journal: seed, configs, final step/eval metrics.
- Render ASCII/braille loss curves from `metrics.jsonl`.
- Diff two runs: config delta, loss-curve overlay, final metric delta.
- Jump from a run to its output corpus (when the run's data dir is
  recorded in the journal) [invention].
- Tag runs (`baseline`, `candidate`, `failed`, `experiment`) [invention].

**Run diff contract:** given two `run.json` files and their optional
`metrics.jsonl`, emit a line-oriented report of every field that
changed, plus a side-by-side ASCII plot of loss and holdout metrics.

### 2.3 Candidate registry

**Purpose:** promote runs to named models and dispatch them to seats.

**Capabilities [invention]:**

- `promote RUN NAME` — copy or reference a run directory into a
  candidate registry (a subdirectory or a lightweight index file) with
  tags and notes.
- List candidates with name, tags, source run, final metrics, and
  whether the model file is present.
- `send NAME black|white|both` — install the candidate as the evaluator
  for the chosen seat(s). This is the generalization of the existing
  `model DIR` command.
- `unload [seat]` — revert a seat to its configured evaluator kind
  (`tactics`/`baseline`), removing the runtime net overlay.
- Compare two candidates side-by-side on the same position by
  alternating their policy overlays.

**Candidate persistence:** candidates are lightweight metadata that
point at run directories. The registry does **not** duplicate `model.mpk`
files. This preserves the rule "runs are cheap because data + journal
regenerate them" from the phase-0 journey sketch.

### 2.4 Training launcher + live watch

**Purpose:** start `nettrain` from inside the CLI and watch its
progress.

**Capabilities [invention]:**

- `train --data DIR --out DIR [flags]` — launch a training run as a
  child process or, later, an in-process training actor. The first
  implementation shells out to `nettrain` so the CLI does not link
  Burn's training path unless desired.
- Live watch: tail the run's `metrics.jsonl` and render a small
  in-frame plot (train loss, lr, holdout metrics) while the run
  progresses.
- Abort a running run cleanly (SIGINT, or message to the actor).
- On completion, offer to promote the run to a candidate.

**Live-watch contract:** the CLI polls `metrics.jsonl` and the run's
stderr heartbeat (see §5.1). The renderer redraws the frame on each new
line. No direct coupling to Burn's training internals is required.

### 2.5 Resume manager

**Purpose:** resume training from an intermediate snapshot.

**Capabilities [invention]:**

- List snapshots inside a run directory.
- Inspect snapshot metadata (step, corpus identity, parent run, RNG
  state hash).
- `resume RUN --snapshot N --out NEW_RUN` — start a new run that
  continues from the snapshot, writing a new journal with a `parent`
  field pointing at the source run.
- Watch the resumed run as in §2.4.

**Snapshot format:** see §3.

## 3. Resume design and snapshot format

Resume is the deepest change because the current `nettrain` writes only
model + journal. Burn 0.22 provides the pieces: `ModuleOptimizer` state
implements `RecordState` (burnpack-serializable), `ModuleLrScheduler` has
`LrSchedulerRecord`, the batcher uses a seeded deterministic counter,
and the `DataLoader` shuffle is seeded. A full resume additionally needs
the model, optimizer state, LR scheduler state, RNG/step position, and
corpus identity.

The proposed snapshot writes the following into the run directory [invention]:

```text
artifacts/gomoku/runs/<run-name>/
├── model.mpk              # full-precision model record (existing)
├── run.json               # run journal (existing)
├── metrics.jsonl          # per-step + per-eval records (new, §5.1)
└── snapshots/
    ├── snapshot-0001-STEP.json   # snapshot metadata
    ├── snapshot-0001-OPT.mpk     # optimizer state record
    └── snapshot-0001-SCHED.mpk   # LR scheduler state record
```

**Snapshot metadata fields:**

| Field | Meaning |
|---|---|
| `step` | The step completed when the snapshot was taken. |
| `corpus` | Absolute or workspace-relative path to the corpus directory. |
| `rng_seed`, `rng_offset` | Position in the deterministic RNG stream. |
| `batcher_counter` | Position in the deterministic batcher counter. |
| `parent_run` | Optional path to the run this run resumed from. |
| `timestamp` | ISO timestamp of snapshot creation. |

**Journal lineage:** `run.json` gains an optional `parent_run` field
pointing to the source run directory [invention — amends the current
`RunJournal` shape, which is not a locked architecture decision but is
an existing data contract]. This lets the run registry draw a lineage
graph.

**Snapshot cadence:** periodic snapshots (e.g., every `eval_interval` or
every N steps, configurable) plus an explicit `snapshot` command
[invention]. The checkpoint store already writes `model.mpk`; snapshots
add optimizer + scheduler + metadata. Disk cost is dominated by the
model record, so storing a few optimizer snapshots is acceptable.

## 4. Reuse of existing study-table machinery

The lab deliberately reuses the following existing contracts rather
than inventing parallel ones.

### 4.1 Command language

New commands are new variants of `cli::command::Command`, parsed by the
existing `command::parse` function. Examples [invention]:

- `corpus [DIR]` — enter corpus-browser mode; with a path, open that
  corpus directly.
- `runs [DIR]` — enter run-registry mode.
- `candidates` — enter candidate-registry mode.
- `promote RUN NAME` — promote a run to candidate.
- `send NAME black|white|both` — install candidate on seat(s).
- `train --data DIR --out DIR ...` — launch training.
- `resume RUN --snapshot N --out NEW_RUN` — resume from snapshot.
- `snapshot` — trigger a snapshot of a running in-process training
  actor.

These commands go through `shell_command::execute`, which already
handles file-system operations and returns `SharedShellReply`.

### 4.2 Shell/core split

`SessionCore` remains the headless game state machine. Lab operations
that do not touch the board (browsing corpora, runs, candidates) are
handled at the shell level and do not mutate `SessionCore`. Operations
that load a position or install a model do mutate the core through its
existing methods (`install_net_model`, history navigation).

### 4.3 Overlay renderer

The existing `tui::overlay` module computes `OverlayCell` from
`SearchInfo`. For lab views, a synthetic `SearchInfo` can be built from
any policy/value source: a corpus sample's π and z, a candidate model's
raw forward output, or a run's recorded prior. The renderer needs no
change beyond accepting these synthetic info objects [invention].

**Flagged contract extension:** `SearchInfo` currently stores MCTS
visits, prior, value, and the searching seat's config. Using it as a
generic "policy/value view" carrier for non-MCTS sources is a mild
semantic stretch. The spec proposes keeping the same struct and adding
a constructor/path from raw policy arrays, rather than creating a
parallel overlay type. This is an [invention] that touches a CLI
internal, not a locked architecture decision.

### 4.4 Seats and the Evaluator seam

The `mcts::Evaluator` trait is the boundary. A candidate model is
installed as a `NetEvaluator` behind this seam, exactly as the existing
`model DIR` command does. The generalization to per-seat models means
`SessionCore` may hold one installed net per seat rather than one
shared net [invention — amends the current v1 limitation "No per-seat
networks" documented in `docs/how-to-first-model-in-the-cli.md`].

**Flagged amendment to a locked seam:** enriching `mcts::EvalRequest`
to carry last-move planes (so the network's history planes are not
zero-filled) would amend the current seam contract documented in
chapter 12 and the how-to-first-model doc. The current seam carries
only `planes: engine::Planes { me, you }` and `legal: Vec<Move>`. Adding
history planes is desirable but is **flagged here as a locked-decision
amendment**, not silently relitigated. The lab can function with the
existing seam; history-plane enrichment is a later upgrade that
requires updating `mcts`, `net::planes`, and `cli::net_eval` together.

### 4.5 Analysis invalidation

The lab's derived views (policy overlay on a corpus sample, candidate
comparison, live-watch plots) must invalidate when their source data
changes. The existing `analysis_revision` counter is reused: any lab
command that changes the viewed position, the installed candidate, or
the corpus selection bumps the revision. Views that cache derived data
check the revision and drop stale results, exactly as `ShellState`
does for TSS proofs and decision previews.

## 5. Forms and parameter editing

The CLI is hand-rolled on crossterm; there is no ratatui dependency.
Forms are therefore a built widget, not a free one. The design is
phased: commands and flags come first; form screens come after the
parameter set stabilizes [invention].

### 5.1 Form widget contract

A form is a full-screen or panel widget with:

- A list of **fields**; exactly one field has focus.
- Each field has a label, a current value string, a validation rule,
  and an error message slot.
- Navigation keys (`Tab`/`Shift-Tab`, arrows) move focus.
- Typing edits the focused field's value string.
- `Enter` attempts **commit**: validate every field; on success, emit
  the form's result and close the form; on failure, focus the first
  invalid field and display its error.
- `Esc` **cancels** without emitting a result.

**Field kinds:** plain text, integer, float, path (with existence
validation), enum choice (cyclic via space/arrow), boolean (toggle via
space), and a read-only summary panel.

**Validation contract:** validators are pure functions
`&str -> Result<T, String>`. The form collects all errors before
reporting; it does not commit partially valid data.

### 5.2 Phased introduction

Forms appear only after the corresponding command-line interface has
been used and the parameter set is stable. The order is:

1. `train` flags (§2.4).
2. `resume` flags and snapshot selection (§2.5).
3. Candidate promotion metadata (name, tags, notes) (§2.3).
4. Corpus/run root configuration (a one-time settings form) [invention].

## 6. Crate-level contract changes

This section states responsibilities only; it does not contain
implementation code. The tutorials are the owner's learn-by-doing
material.

### 6.1 `cli`

Owns the lab UI. New responsibilities:

- Add lab commands to `Command` and `command::parse`.
- Add lab modes to the study-table shell (`tui::study`).
- Add corpus/run/candidate readers (new module, file-system only).
- Add the form widget (new module, crossterm only).
- Extend `shell_command::execute` for lab commands.
- Extend `SessionCore` to hold per-seat net models (optional v2) and
to expose hooks for loading a position from a sample/game record.
- Reuse `tui::overlay` for corpus/candidate policy heatmaps.

Dependencies added: none beyond existing project crates and `serde`
for reading manifests/journals. No Burn dependency in `cli` beyond the
optional `net` feature path already present.

### 6.2 `net`

Owns training-state persistence. New responsibilities:

- Define and write `metrics.jsonl` records during `train`.
- Define and write snapshot records (optimizer + scheduler + metadata)
during `nettrain`.
- Support resuming from a snapshot: load model + optimizer + scheduler +
RNG/step position into the training loop.
- Extend `RunJournal` with optional `parent_run` and `data_dir` fields.

These changes live in `net::checkpoint` and `net::train`; the
`network` and `loss` modules are untouched.

### 6.3 `train`

Owns corpus manifests and sample reading. New responsibilities:

- Provide a public reader for a single sample by index within a corpus.
- Provide a public function to reconstruct a `Sample`'s board and
policy target.
- Provide a public iterator over samples matching a class filter.

No format change to `manifest.json` or shard files is required for the
first phase.

### 6.4 `mcts` and `engine`

No changes required for phases 1–4. If history-plane enrichment is
adopted later, `mcts::EvalRequest` and `engine::encode`/`Planes` would
need coordinated amendment; this is flagged in §4.4 as a locked-decision
amendment.

### 6.5 `selfplay`

No changes required for the lab itself. Mentor/game-record corpora
produced by the `selfplay` crate are consumed by the corpus browser
using the same `train` crate read path once the mentor-datagen tutorial
adds game-record support.

## 7. Phasing and acceptance gates

The implementation is ordered so that each phase leaves the repo in a
usable, gate-verified state.

### Phase 0 — metrics.jsonl + stderr heartbeat

**Goal:** make training runs observable without changing the CLI.

**Changes:**
- `net::train::train` writes `metrics.jsonl` in the run directory.
- `nettrain` emits a compact stderr heartbeat each step and each eval.

**Acceptance gates:**
1. A `nettrain` run produces a valid `metrics.jsonl` with one line per
   step and one line per eval.
2. The file parses with the defined schema and matches the final
   `run.json` metrics.
3. `cargo test`, `cargo clippy --all-targets -- -D warnings`,
   `cargo fmt --all` are green.

### Phase 1 — read-only inspection

**Goal:** browse corpora and runs from the CLI without launching training.

**Changes:**
- Lab commands: `corpus`, `runs`, `candidates` (list-only), `send` for
  existing run directories.
- Corpus browser: manifest view, sample list, sample board render with
  policy overlay.
- Run registry: list, journal view, ASCII/braille loss curve from
  `metrics.jsonl`, run diff.

**Acceptance gates:**
1. `corpus DIR` renders the manifest and can display any sample as a
   board with its policy target overlay.
2. `runs DIR` lists runs, shows journals, and plots loss curves.
3. Diffing two runs reports every changed config field and overlays
   their loss curves.
4. The existing `model DIR` command continues to work unchanged.
5. All gates from Phase 0 remain green.

### Phase 2 — training-state snapshots + resume

**Goal:** resume training from an intermediate state.

**Changes:**
- `net::checkpoint` defines snapshot records and writes them
  periodically during `nettrain`.
- `nettrain` gains `--resume SNAPSHOT_DIR` (or a dedicated `resume`
  binary).
- `RunJournal` gains `parent_run`.
- CLI `resume` command (shell-only or in-process).

**Acceptance gates:**
1. A run interrupted at step S and resumed from its latest snapshot
   reproduces the same loss trajectory (on Flex) as an uninterrupted
   run from the same initial seed and corpus.
2. The resumed run's `run.json` points to the parent run.
3. Snapshot files are full-precision and round-trip correctly.
4. All gates from Phase 1 remain green.

### Phase 3 — candidate registry + launch/watch

**Goal:** promote runs, dispatch models to seats, and launch training
from the CLI.

**Changes:**
- Candidate registry: metadata file or subdirectory under
  `artifacts/gomoku/candidates/` [invention].
- `promote`, `send`, `unload` commands.
- `train` command that launches `nettrain` and enters live-watch mode.
- Live-watch frame: loss plot + recent stderr lines + abort key.

**Acceptance gates:**
1. `promote RUN NAME` creates a candidate that survives CLI restart.
2. `send NAME white` installs the candidate only on White; `unload`
   reverts to the configured evaluator.
3. `train --data ... --out ...` launches a run and the CLI renders a
   live loss curve updated from `metrics.jsonl`.
4. Aborting a run from the CLI leaves a usable partial run directory
   (model + journal + metrics).
5. All gates from Phase 2 remain green.

### Phase 4 — forms

**Goal:** replace the most common flag sequences with form screens.

**Changes:**
- Form widget implemented and tested.
- Form screens for `train`, `resume`, `promote`, and settings.

**Acceptance gates:**
1. Each form validates its fields and refuses commit on invalid input.
2. Form output matches the equivalent command-line invocation.
3. `Esc` cancels without side effects.
4. All gates from Phase 3 remain green.

## 8. Explicit non-goals

- **No second metadata store.** The lab reads `manifest.json` and
  `run.json` directly. It does not maintain a separate database or
  index file except for the lightweight candidate registry metadata
  [invention].
- **No arena or Elo machinery.** Milestone 6 owns match play, rating,
  and promotion gating. The lab may display candidate tags, but it
  does not compute Elo or run arena sessions.
- **No GPU parity work.** GPU training correctness is noted as an
  adjacent concern (Burn issues #5162, #5626); the lab does not try
  to fix or verify it. Flex CPU remains the deterministic reference.
- **No distributed training.** The lab targets a single machine (Mac
  Studio). Multi-process parallelism for mentor datagen is handled
  by shelling out multiple `selfplay` processes, not by the lab.
- **No replacement of `nettrain` as the training authority.** The CLI
  may launch `nettrain` or call the `net` crate, but the training
  loop remains in `net::train`.
- **No change to the study table's core game contract.** Lab commands
  that load positions still produce legal `Board` states; lab modes
  that do not touch the board leave `SessionCore` unchanged.
- **No telemetry or remote reporting.** All lab state is local files.

## 9. Honesty conventions

Numeric and empirical claims are labelled:

- **[measured]** — directly observed on this project.
- **[derived]** — computed from measured values.
- **[experiment]** — free parameter chosen by measurement.
- **[invention]** — design choice not derived from a locked decision.

Locked decisions cited in this spec:

- Chapter 12 §6: `mcts` must not depend on `net`; the `Evaluator`
  trait is the seam.
- Chapter 12 §9: store games, not positions; replay to sample.
- Chapter 12 §13 milestones: milestone 6 owns arena/Elo.
- Chapter 14 D2: phase-0 supervised pre-training before self-play.
- Chapter 14 D6: mentor records join tier 3 alongside external
  engine records.
- `docs/how-to-first-model-in-the-cli.md` v1 limitations: last-move
  planes zero-filled, net seat runtime-only, no per-seat networks,
  no resume, overwrite hazard.

Places where this spec proposes amending or extending a locked or
existing contract (flagged explicitly):

1. **Per-seat network models** (§4.4, §6.1): amends the v1 limitation
   "no per-seat networks" in `docs/how-to-first-model-in-the-cli.md`.
2. **`RunJournal` fields** (§3): extends the existing `run.json` schema
   with `parent_run` and optionally `data_dir`. This is an existing
   data contract, not a locked architecture decision.
3. **History-plane enrichment of `mcts::EvalRequest`** (§4.4): would
   amend the current `EvalRequest` seam that carries only me/you
   planes. This is flagged but not required for the lab's first
   phases.
4. **Using `SearchInfo` as a generic policy/value view carrier**
   (§4.3): extends the internal CLI meaning of `SearchInfo` without
   changing its fields. Not a locked contract.

## 10. References

**Project documents:**

- [../12-gomoku-architecture.md](../12-gomoku-architecture.md) — §6
  crate map and `Evaluator` seam, §9 store-games discipline, §13
  milestones, honesty convention.
- [../14-openings-and-supervised-curriculum.md](../14-openings-and-supervised-curriculum.md)
  — signal ladder §5.2, phases D2, licensing D3, re-adjudication D5,
  mentor records D6.
- [../how-to-first-model-in-the-cli.md](../how-to-first-model-in-the-cli.md)
  — current model-loading manual and v1 limitations.
- [../specs/2026-11-08-phase-0-training-journey.md](2026-11-08-phase-0-training-journey.md)
  — the journey sketch this lab serves.
- [../specs/2026-11-08-mentor-datagen-design.md](2026-11-08-mentor-datagen-design.md)
  — format precedent for this spec; game-record format and mentor
  datagen design.
- [../../gomoku/crates/cli/README.md](../../gomoku/crates/cli/README.md)
  — existing CLI usage, command language, save/load contract.
- [../../gomoku/crates/cli/src/command.rs](../../gomoku/crates/cli/src/command.rs)
  — `Command` enum and parser.
- [../../gomoku/crates/cli/src/session.rs](../../gomoku/crates/cli/src/session.rs)
  — `SessionCore`, `analysis_revision`, seat/model installation.
- [../../gomoku/crates/cli/src/shell_command.rs](../../gomoku/crates/cli/src/shell_command.rs)
  — shell/core split and `LoadModel` handling.
- [../../gomoku/crates/cli/src/tui/study.rs](../../gomoku/crates/cli/src/tui/study.rs)
  — study-table shell loop and `ShellState` invalidation.
- [../../gomoku/crates/cli/src/tui/overlay.rs](../../gomoku/crates/cli/src/tui/overlay.rs)
  — policy/visit/prior heatmap and TSS overlay renderer.
- [../../gomoku/crates/cli/src/net_eval.rs](../../gomoku/crates/cli/src/net_eval.rs)
  — `NetEvaluator` and the current `EvalRequest` limitation.
- [../../gomoku/crates/net/src/checkpoint.rs](../../gomoku/crates/net/src/checkpoint.rs)
  — `RunJournal`, model save/load.
- [../../gomoku/crates/net/src/train.rs](../../gomoku/crates/net/src/train.rs)
  — `StepMetrics`, `EvalMetrics`, training loop.
- [../../gomoku/crates/train/src/shard.rs](../../gomoku/crates/train/src/shard.rs)
  — `Manifest` and shard discipline.
