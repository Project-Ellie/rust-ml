# Net integration design — trained-prior seat end to end

**Status:** proposed (2026-11-20), **amended 2026-11-20** after
implementation scouting corrected two factual errors (see the
amendment note). Supersedes the queued *client model-integration
tutorial*: the tutorial ambition is retired, the functionality is
delivered as regular feature work.

## Amendment note (2026-11-20)

The original draft claimed (a) that no net-backed `mcts::Evaluator`
exists and (b) that the `EvalRequest` seam must grow a move history
because "training used last-move history planes". Both wrong:

- `crates/cli/src/net_eval.rs` already implements a journal-aware
  `NetEvaluator` behind the `net` feature, and the study table already
  has a `load-model <dir>` shell command that installs the model on
  the AI seats.
- Training **zero-fills** the last-move planes: `net::batcher`
  rebuilds every sample board via `Board::from_position`, which
  carries no move history (`batcher.rs` pins this in a comment). The
  existing evaluator's zero-filled history planes are therefore
  *consistent with training*, not an out-of-distribution defect.

Milestone A is re-scoped accordingly: extraction and verification
instead of a seam change.

## Context and goal

The corpus pipeline is complete: `selfplaygen` (baseline-vs-baseline
Swap2 corpora), `selfplayexport` (corpus → `train` Sample shards), and
`nettrain` (phase-0 trainer writing `model.mpk` + `run.json` +
`metrics.jsonl` per run). What remains between a trained checkpoint
and priors visible on the gomoku-gui board:

1. **The net evaluator lives in the wrong crate.** `cli::net_eval` is
  unusable for the GUI — the GUI cannot depend on the TUI crate.
2. **No checkpoint picker.** Locating a run directory is manual path
  typing.
3. **gomoku-gui only knows the baseline seat** — `ai_worker.rs`
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
- **No move-history input planes.** `train::Sample` stores stones, not
  history; making the network see the last moves means changing the
  sample format and retraining — a separate, deliberately deferred
  decision. The phase-0 network's planes 2/3 are dead features in both
  training and inference, which is consistent.
- No changes to the engine, the selfplay record formats, or the
  tutorial materials.

## Current state (verified 2026-11-20)

- `mcts::Evaluator`: `fn evaluate(&mut self, req: EvalRequest) ->
  EvalResult`; `EvalRequest { planes, legal }` — side-to-move-relative
  `engine::Planes` (2 planes, 17×17) plus legal moves; `EvalResult`
  carries raw policy logits `[f32; 225]` + value (masked softmax is
  mcts's job, `expand.rs`). **The seam stays as is.**
- `cli::net_eval::NetEvaluator` (feature `net`): `load(dir)` reads
  `run.json` for `ModelConfig`, loads `model.mpk` full precision via
  `net::checkpoint::load_model`, batch-1 forward on plain (non-autodiff)
  Flex — which also selects the BatchNorm inference path (Burn 0.22 keys
  training behavior on `device.is_autodiff()`); zero-filled history
  planes matching training. Tests cover load+evaluate on a tiny run dir
  and missing-dir failure. Known gap: the module doc frames the zero
  history planes as a "limitation" — misleading, to be corrected.
- CLI study table: `load-model <dir>` shell command
  (`shell_command.rs`) transactionally installs the model on AI seats;
  `session.rs` builds per-seat evaluators via `NetEvaluator::from_model`.
- `net::checkpoint`: a checkpoint dir holds `model.mpk`
  (full-precision burnpack) and `run.json` (`RunJournal` with
  `model_config`, `train_config`, final metrics, stop reason).
- `net::Model::forward` takes `[B, 4, 17, 17]` and returns raw policy
  logits `[B, 225]` + tanh value `[B, 1]`.
- gomoku-gui `Worker::spawn(factory, sink)` is evaluator-agnostic;
  `Worker::baseline` is just one factory. rust-ml crates enter the GUI
  as git deps pinned by rev (`engine`, `mcts`, `player`, `baseline`).
  GUI seats are per-color with persisted settings (simulations,
  temperature, own_tss, guard, resign).

## Decisions

### D1 — retracted: the `EvalRequest` seam does not change

Originally: add `history: Vec<Move>` so the evaluator can fill the
last-move planes. Retracted because training never fills them either
(`net::batcher` uses `Board::from_position`). Instead: correct the
misleading "v1 limitation" module doc in the extracted evaluator to
state the planes are zero *by design, matching the training inputs*.
Revisit only if `train::Sample` ever grows history — a training-format
decision, not a search-seam one.

### D2 — extract `cli::net_eval` into a new crate `neteval`

New workspace crate `crates/neteval` (deps: `engine`, `mcts`, `net`,
`burn`) owns `NetEvaluator`, moved verbatim in behavior from
`cli::net_eval`; `cli` depends on `neteval` and re-exports for its
existing call sites. Rationale: evaluator implementations live
downstream of `mcts` (the `baseline` pattern); `net` stays a pure
network+training crate; the GUI gains a dependency it can actually
take. The eval seam's own docs anticipate this ("Production
implementations (direct net wrapper, channel client) live in
downstream crates", `mcts/src/eval.rs`).

### D3 — checkpoint picker is a small library, not UI logic

`neteval::picker`: `scan(root) -> Vec<CheckpointInfo>` where
`CheckpointInfo { dir, journal }`, skipping dirs without a valid
`model.mpk` + `run.json` pair, sorted by modification time descending.
The CLI and the GUI both consume it; "journal-aware model picker"
means every listing shows what the journal knows (config, seed, final
metrics, stop reason), so a run directory is self-describing.

Checkpoint root is **caller's choice**: rust-ml binaries default to
`data/check-points/` (git-ignored, already the training output
convention); the GUI gets its own configurable root (the repos are
separate; the GUI must not hard-code a rust-ml path).

### D4 — backend acceptance is the existing CLI path, verified

The CLI smoke the original draft planned to build already exists:
`load-model <dir>` in the study table. Milestone A verifies it end to
end against a real short `nettrain` run (load a checkpoint, play legal
moves against the baseline at low sims, prior overlay renders network
priors) and adds a `models` listing command backed by the D3 picker so
run directories are discoverable from the study table.

### D5 — GUI: device Flex first, settings follow the one-layout rule

- `Worker::net(checkpoint_dir, sink)` factory beside
  `Worker::baseline`, building `player::AiPlayer` with
  `neteval::NetEvaluator` on plain Flex — consistent with the existing
  GUI worker and deterministic. GPU device selection is a follow-up
  only if measurements demand it (see risks).
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

- A1: extract `cli::net_eval` → `crates/neteval` (`#![deny(missing_docs)]`),
  module doc corrected per D1, tests move; `cli` depends on `neteval`;
  workspace wiring.
- A2: `neteval::picker` + tests over synthetic run dirs (D3).
- A3: study-table `models` listing command backed by the picker;
  verify `load-model` end to end against a real short `nettrain`
  run (D4).
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
- **Dead input planes.** The network trains and plays on effectively
  two planes (me/you); the last-move planes are zero on both sides.
  Consistent, but a real limitation of the phase-0 input encoding —
  recorded here so the deferred history-plane decision stays visible.
- The GUI record-format bump (B2) touches a loader with strict
  validation; old saves must keep loading.
