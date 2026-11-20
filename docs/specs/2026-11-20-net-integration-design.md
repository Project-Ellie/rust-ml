# Net integration design — trained-prior seat end to end

**Status:** proposed (2026-11-20). Supersedes the queued *client
model-integration tutorial*: the tutorial ambition is retired, the
functionality is delivered as regular feature work.

## Context and goal

The corpus pipeline is complete: `selfplaygen` (baseline-vs-baseline
Swap2 corpora), `selfplayexport` (corpus → `train` Sample shards), and
`nettrain` (phase-0 trainer writing `model.mpk` + `run.json` +
`metrics.jsonl` per run). Two gaps remain between a trained checkpoint
and priors visible on the gomoku-gui board:

1. **No net-backed `mcts::Evaluator` exists.** Nothing wraps a loaded
   checkpoint behind the evaluation seam the way
   `baseline::BaselineEvaluator` does.
2. **gomoku-gui only knows the baseline seat** — `ai_worker.rs`
   constructs `BaselineEvaluator` on Flex; there is no model loading
   and no seat/model selection.

Goal: play against (or watch) a checkpoint trained on a first
self-play corpus in gomoku-gui, with the prior overlay showing the
network's priors.

## Non-goals

- No continuous/parallel AlphaZero loop (milestone 5+ territory).
- No strength evaluation or gating of the trained net (the grading
  gate stays unexecuted per the 2026-11-19 ruling).
- No GPU training changes; training stays on the existing `nettrain`
  paths.
- No changes to the engine, the record formats of the selfplay crate,
  or the tutorial materials.

## Current state (verified 2026-11-20)

- `mcts::Evaluator`: `fn evaluate(&mut self, req: EvalRequest) ->
  EvalResult`; `EvalRequest { planes, legal }` — side-to-move-relative
  `engine::Planes` (2 planes, 17×17) plus legal moves; `EvalResult`
  carries raw policy logits `[f32; 225]` + value (masked softmax is
  mcts's job, `expand.rs`).
- `mcts::expand` receives the leaf `&Board` — full move history is
  available exactly where `EvalRequest` is built.
- `net::checkpoint`: `load_model(dir, &ModelConfig, &Device)` +
  `load_journal(dir)`; a checkpoint dir holds `model.mpk`
  (full-precision burnpack) and `run.json` (`RunJournal` with
  `model_config`, `train_config`, final metrics, stop reason).
- `net::planes::assemble(&Board) -> InputPlanes` builds the four
  17×17 input planes — **including the two last-move history planes,
  derived from `Board::moves()`**. Training samples store move
  history (`train::Sample`), so the net was trained with history
  planes; evaluating without them is out of distribution.
- `net::Model::forward` takes a `[B, 4, 17, 17]` tensor and returns
  raw policy logits `[B, 225]` + tanh value `[B, 1]`; inference uses
  `model.valid()` (BatchNorm inference statistics — established
  pattern in `net::train`).
- gomoku-gui `Worker::spawn(factory, sink)` is evaluator-agnostic;
  `Worker::baseline` is just one factory. rust-ml crates enter the GUI
  as git deps pinned by rev (`engine`, `mcts`, `player`, `baseline`).
  GUI seats are per-color with persisted settings (simulations,
  temperature, own_tss, guard, resign).

## Decisions

### D1 — `EvalRequest` gains the move history

Add `history: Vec<Move>` to `mcts::EvalRequest`, populated in
`expand.rs` from `board.moves()`. The seam keeps its "no `Board` in
the request" rule; history is data, not engine API. The in-crate
stubs (`UniformEvaluator`, `TacticsEvaluator`) ignore it. Full
history, not just the last two moves: replaying it doubles as a
validity check, the clone cost is negligible (≤225 small moves per
expansion), and future input-encodings with longer memory need no
second seam change.

### D2 — new crate `neteval`, mirroring `baseline`

A new workspace crate `crates/neteval` (deps: `engine`, `mcts`,
`net`, `burn`) owns `NetEvaluator`. Rationale: evaluator
implementations live downstream of `mcts` (the `baseline` pattern);
`net` stays a pure network+training crate and never learns about the
search seam. Chapter 12's planned crate list is not violated —
`neteval` is the "direct net wrapper" the eval seam's own docs
anticipate (`mcts/src/eval.rs`: "Production implementations (direct
net wrapper, channel client) live in downstream crates").

`NetEvaluator`:

- `NetEvaluator::load(checkpoint_dir, device)` — **journal-aware**:
  reads `run.json` for `ModelConfig`, then `load_model`; the caller
  never configures channels/blocks by hand. Errors are typed
  (missing/corrupt journal, missing `model.mpk`, forward failures
  stay impossible by construction).
- Inference via `model.valid()`; batch-1 forward.
- Input assembly: replay `req.history` into a `Board`
  (`Board::from_position` style, absolute colors), then
  `net::planes::assemble`. Replay failure means a corrupt request —
  return a typed error rather than guessing planes. (Note: this makes
  `evaluate` fallible in principle; the `Evaluator` trait is
  infallible, so a replay failure panics with a descriptive message —
  it indicates an mcts-side bug, not user input. Reconsidered-and-
  rejected alternative: zeroed history planes on failure — silent
  out-of-distribution evaluation is worse than a loud panic.)
- Output: raw policy logits + clamped value straight into
  `EvalResult` — masking stays in mcts.

### D3 — checkpoint picker is a small library, not UI logic

`neteval::picker` (or a sibling module): `scan(root) ->
Vec<CheckpointInfo>` where `CheckpointInfo { dir, journal }`,
skipping dirs without a valid `model.mpk` + `run.json` pair, sorted
by modification time descending. The CLI smoke and the GUI both
consume it; "journal-aware model picker" means every listing shows
what the journal knows (config, seed, final metrics, stop reason), so
a run directory is self-describing.

Checkpoint root is **caller's choice**: rust-ml binaries default to
`data/check-points/` (git-ignored, already the training output
convention); the GUI gets its own configurable root (the repos are
separate; the GUI must not hard-code a rust-ml path).

### D4 — backend acceptance is a CLI smoke, not the GUI

Before any GUI work, prove the evaluator end-to-end inside rust-ml:
extend the `cli` crate's seat plumbing (`--ai baseline` exists) with
`--ai net --model <DIR>`. Acceptance: a checkpoint produced by a
short `nettrain` run plays legal moves against the baseline at low
sims, and the study table's prior overlay renders the network's
priors. This is the backend milestone's check-in gate.

### D5 — GUI: device Flex first, settings follow the one-layout rule

- `Worker::net(checkpoint_dir, sink)` factory beside
  `Worker::baseline`, building `player::AiPlayer` with `NetEvaluator`
  on `Device::flex()` — consistent with the existing GUI worker and
  deterministic. GPU device selection is a follow-up only if
  measurements demand it (see risks).
- Players section gains a per-color seat-kind row
  (Human / Baseline AI / Trained net) and, for the net kind, a
  checkpoint picker fed by D3 — **laid out in the established
  temperature/simulations row style** (alignment, font, size; the
  standing UI convention).
- Session persistence: the seat kind and checkpoint path are
  persisted per seat. This extends the GUI's record format — version
  bump with the same transactional-load care as the existing reader.

## Task split (check-in milestones)

**Milestone A — backend `neteval` (rust-ml repo)**

- A1: D1 seam change — `EvalRequest.history`, `expand.rs` population,
  stub/mock adjustments, mcts tests green.
- A2: `neteval` crate — `NetEvaluator::load` + `evaluate`, typed
  errors, unit tests with a freshly saved tiny-config checkpoint
  (save → load → evaluate roundtrip, determinism on Flex, policy
  shape, value range), `#![deny(missing_docs)]`.
- A3: picker module + tests over synthetic run dirs.
- A4: CLI `--ai net --model <DIR>` smoke + README note.
- Gates (per AGENTS.md): `cargo test`, `cargo clippy --all-targets --
  -D warnings`, `cargo fmt --all` — all green. Wolfie reviews.

**Milestone B — GUI seat (gomoku-gui repo)**

- B1: rust-ml rev bump + `neteval` dependency; `Worker::net` factory;
  failure surfacing through the existing reply path.
- B2: seat-kind + checkpoint picker UI (one-layout rule), session
  persistence with record-format versioning.
- B3: Wolfie's manual acceptance — generate corpus → export →
  `nettrain` → pick the checkpoint in the GUI → priors on the board.

## Risks and honest caveats

- **Speed.** A default-config net (128ch × 10 blocks) at batch 1 on
  Flex is far slower than the hand-written baseline evaluator; at
  hundreds of simulations per move, a move can take minutes. The first
  corpus run should train `--config tiny` (or accept low sims in the
  GUI seat). The metal path is designed-in follow-up, not part of
  these milestones.
- **History-plane OOD** is real if the seam change is skipped or the
  replay is wrong — A2's roundtrip tests must assert the assembled
  planes match `net::planes::assemble` on the replayed board exactly.
- The GUI record-format bump (B2) touches a loader with strict
  validation; old saves must keep loading.
