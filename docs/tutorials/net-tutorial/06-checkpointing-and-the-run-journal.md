# Chapter 06 — Checkpointing and the run journal

## Abstract

This chapter makes the training loop durable. It adds a checkpointing
module that writes a full-precision model record plus a JSON sidecar
that captures the run's provenance: seed, network configuration,
training configuration, and final metrics. By the end you will have
`gomoku/crates/net/src/checkpoint.rs`, roundtrip tests that prove a
saved-and-loaded model produces bit-identical outputs, and an explicit
understanding of why `DefaultRecorder` is required and `CompactRecorder`
is a trap.

## Glossary

| Term | Definition |
|------|------------|
| **Checkpoint** | A persistent artifact that lets a training run resume or be reproduced. In this crate it is the pair `model.mpk` + `run.json`. |
| **`DefaultRecorder`** | Burn 0.21's full-precision recorder. It writes `f32` weights and is bit-identical on load. |
| **`CompactRecorder`** | Burn 0.21's f16 recorder. Smaller files, but weights are rounded; a loaded model is not bit-identical to the saved one. |
| **Run journal** | The `run.json` sidecar file: seed, `ModelConfig`, `TrainConfig`, and the final step/eval metrics produced by the loop. |
| **Sidecar** | A separate, human-readable file that travels with the binary model record and records metadata needed to reproduce the run. |
| **Bit-identical forward pass** | A test that saves a model, loads it, runs the same input through both the original and the loaded model, and asserts exact equality of the outputs. |
| **`RecorderError`** | The error type returned by Burn's record save/load operations. |

## Context

Chapters 1–5 built the `net` crate: input planes, the batcher with D4
augmentation, the residual network, the hand-rolled loss, and the
manual training loop. All of that work lived in memory. A real
supervised-training pipeline must be able to stop, resume, and report
what it did. This chapter adds persistence.

The persistence design is invention I6 from the
[net tutorial design spec](../../specs/2026-10-04-net-tutorial-design.md):
checkpoints are `model.mpk` plus `run.json`. The model record uses
`DefaultRecorder` so that save→load is lossless. The sidecar journal
stores everything a human or an automated script needs to know to
reproduce the run: the random seed, the network configuration, the
training configuration, and the final metrics.

The most consequential choice in this chapter is the recorder. Burn
0.21 ships two convenient recorders: `DefaultRecorder` (full precision)
and `CompactRecorder` (f16). The latter is smaller on disk, but it
rounds weights to half precision. For a Gomoku policy/value network
that difference is small in accuracy terms — on the order of 1e-4 — but
it is fatal for exact roundtrip tests and for resuming training from a
checkpoint. The project therefore mandates `DefaultRecorder` for all
model records. The f16 trap is named explicitly in
[docs/11-pitfalls.md](../../11-pitfalls.md).

## Intention

1. Create `gomoku/crates/net/src/checkpoint.rs` and register
   `pub mod checkpoint;` in `gomoku/crates/net/src/lib.rs` (the
   registration is done in the Chapter 08 polish step; the module is
   introduced here).
2. Define `RunJournal` with fields for seed, `ModelConfig`,
   `TrainConfig`, optional final `StepMetrics`, and optional final
   `EvalMetrics`.
3. Implement `RunJournal::new` to assemble a journal from the pieces
   produced by a training run.
4. Implement `save_model<B: Backend>(model, dir)` →
   `dir/model.mpk` using `DefaultRecorder`.
5. Implement `load_model<B: Backend>(dir, config, device)` → model
   by initializing a fresh model from the config and loading the
   record into it.
6. Implement `save_journal(journal, dir)` → `dir/run.json` and
   `load_journal(dir)` → `RunJournal`.
7. Implement `save_checkpoint` that creates the output directory and
   writes both files.
8. Define `CheckpointError` as a `thiserror` enum covering I/O,
   `RecorderError`, and JSON errors.
9. Write three tests:
   * `save_load_roundtrip_is_bit_identical` — initialize a tiny model,
     run a zero input forward pass, save, load, run the same input
     again, and assert the policy outputs are exactly equal.
   * `journal_roundtrips_through_json` — build a journal with known
     values, save it, load it, and assert equality.
   * `checkpoint_saves_both_model_and_journal` — call
     `save_checkpoint` and assert both files exist.

Observable done-state: `cargo test -p net checkpoint` passes,
`cargo clippy --all-targets -- -D warnings` is green, and `cargo fmt
--all -- --check` makes no changes.

## Mental mapping

### Why a separate journal instead of metadata inside `model.mpk`?

Burn's record format is designed for weights, not for arbitrary
metadata. A `RunJournal` sidecar is a plain JSON file that humans can
read, scripts can parse, and version control can diff. Keeping it
separate also means the model record stays a pure tensor container:
any loader that knows the architecture can load `model.mpk` without
needing to understand the project's training provenance schema.

### Why full precision is non-negotiable for this project

`CompactRecorder` stores weights as `f16`. On reload they are converted
back to `f32`, but the low bits are gone. For inference-only use that
loss is usually invisible; for exact reproducibility it is not. Two
consequences matter here:

1. **Determinism tests fail.** The acceptance ritual in Chapter 08
   includes a determinism check: two short seeded runs must produce
   identical loss trajectories. If a checkpoint is saved and reloaded
   mid-run with f16 rounding, the second run diverges at the reload
   point.
2. **Resume training drifts.** AdamW's optimizer state is sensitive to
   small weight perturbations. Resuming from an f16 checkpoint changes
   the loss landscape slightly and breaks the guarantee that the same
   seed always reaches the same place.

The design spec records this as invention I6 and the pitfalls document
names it as the first trap hit during the making of the course. The
tutorial presents `DefaultRecorder` as the required choice and
`CompactRecorder` as an opt-in compression step that is only safe after
training is finished and exact reproducibility is no longer required.

### Why `model.clone().save_file(...)`?

`save_file` is a method on `Module` that consumes `self` by value. The
save path therefore needs an owned model. The training loop still needs
the model after saving (for example, to continue training or to return
it to the caller), so the checkpoint function takes a shared reference
and clones before saving. The clone is cheap relative to the cost of
disk I/O and keeps the caller's ownership semantics simple.

### Why initialize a fresh model before `load_record`?

Burn's record system separates *config* from *state*. A config knows
how to build a module (`config.init::<B>(device)`). A record is only
the state. Loading a record therefore has two steps: build the module
skeleton from the config, then pour the record into it. This matches
the `Config::init` + `load_record` pattern from Burn 0.21 and avoids
the stale `InitRecord` API that older tutorials use.

## Low-level design

### Files

```text
gomoku/crates/net/
└── src/
    ├── checkpoint.rs   # new module
    └── lib.rs          # add `pub mod checkpoint;` in Chapter 08
```

### `src/checkpoint.rs`

```rust
//! Checkpointing and run journal.
//!
//! Checkpoints are written as:
//!
//! * `model.mpk` — full-precision model record via [`DefaultRecorder`] (I6).
//! * `run.json` — run journal with seed, config, and final metrics.
//!
//! Full precision matters: `CompactRecorder` is f16 and not bit-identical
//! on resume (docs/11).

use std::path::{Path, PathBuf};

use burn::prelude::Module;
use burn::record::{DefaultRecorder, Recorder};
use burn::tensor::backend::Backend;
use serde::{Deserialize, Serialize};

use crate::network::{Model, ModelConfig};
use crate::train::{EvalMetrics, StepMetrics, TrainConfig};
```

Implementation notes:

* `CheckpointError` is a `#[derive(Debug, thiserror::Error)]` enum with
  three variants: `Io(std::io::Error)`, `Record(burn::record::RecorderError)`,
  and `Json(serde_json::Error)`. Derive `#[from]` for each source type.
* `RunJournal` derives `Debug, Clone, PartialEq, Serialize, Deserialize`.
  All nested types (`ModelConfig`, `TrainConfig`, `StepMetrics`,
  `EvalMetrics`) must already derive `Serialize`, `Deserialize`, and
  `PartialEq` so the journal can roundtrip through JSON and be compared
  in tests.
* `save_model` calls `model.clone().save_file(path, &DefaultRecorder::new())`
  and returns the `PathBuf` of the written file.
* `load_model` builds a fresh model with `config.init::<B>(device)` and
  loads the record with `DefaultRecorder::new().load(path, device)?`.
* `save_journal` uses `serde_json::to_string_pretty` and writes with
  `std::fs::write`.
* `save_checkpoint` creates the directory with
  `std::fs::create_dir_all` and delegates to `save_model` and
  `save_journal`.

### Tests

Inside `#[cfg(test)] mod tests` in `checkpoint.rs`:

1. `save_load_roundtrip_is_bit_identical`:
   * `type B = Flex;`
   * `let device = FlexDevice;`
   * Build a tiny model from `ModelConfig::tiny()`.
   * Run a `[1, 4, 17, 17]` zero tensor through `model.forward`.
   * Save with `save_model`.
   * Load with `load_model` using the same config and device.
   * Run the same input through the loaded model.
   * Convert both policy outputs to `Vec<f32>` and assert exact
     equality.
2. `journal_roundtrips_through_json`:
   * Build a `RunJournal` with seed `42`, tiny configs, and some
     `StepMetrics`/`EvalMetrics` values.
   * Save and load through JSON.
   * Assert `journal == loaded`.
3. `checkpoint_saves_both_model_and_journal`:
   * Build a tiny model and a minimal journal.
   * Call `save_checkpoint`.
   * Assert `dir.join("model.mpk").exists()` and
     `dir.join("run.json").exists()`.

Use the same unique-temp-directory pattern as earlier crates: combine
`SystemTime` nanoseconds and process ID to avoid collisions.

## Solution (opt-in)

The complete reference code for this chapter lives in
[06-checkpointing-and-the-run-journal/01-solution.md](06-checkpointing-and-the-run-journal/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare.

## TDD checklist

1. **Red:** Create `src/checkpoint.rs` with the `RunJournal` struct,
   the `CheckpointError` enum, and function signatures, leaving every
   body as `todo!()`. Write the three tests. Run
   `cargo test -p net checkpoint` and expect failures from the
   `todo!()` panics.
2. **Green:** Implement `save_model`, `load_model`, `save_journal`,
   `load_journal`, and `save_checkpoint`. Re-run
   `cargo test -p net checkpoint`. All three tests should pass.
3. **Verify the f16 trap is avoided:** Confirm that the code uses
   `DefaultRecorder` everywhere. Search for `CompactRecorder` and
   ensure it does not appear in this crate.
4. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p net checkpoint` passes from `gomoku/`.
* `cargo test -p engine --features testutil` still passes (no engine
  changes this slice).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* `CompactRecorder` is not used anywhere in `gomoku/crates/net`.
* Commit message in the reference worktree:
  `feat(net): checkpointing + run journal`.
* Commit message in the main repository:
  `docs(tutorials): net tutorial chapter 6`.

Next: [Chapter 07 — The `nettrain` binary](07-the-nettrain-binary.md).

## References

* [`docs/specs/2026-10-04-net-tutorial-design.md`](../../specs/2026-10-04-net-tutorial-design.md)
  — net tutorial design authority, inventions I6 (checkpoint layout)
  and I7 (test-scale config).
* [`docs/11-pitfalls.md`](../../11-pitfalls.md) — the `CompactRecorder`
  f16 trap and other Burn 0.21 pitfalls.
* [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  network anatomy, optimizer/schedule decisions, and the Burn/application
  split that makes a manual training loop necessary.
* [`docs/tutorials/net-tutorial/05-the-manual-training-loop.md`](05-the-manual-training-loop.md)
  — the training loop that produces the metrics stored in the run
  journal.
* [Burn 0.21 `Recorder` trait source](https://github.com/tracel-ai/burn/blob/v0.21.0/crates/burn-core/src/record/recorder.rs)
  — the authoritative definition of `DefaultRecorder` and
  `CompactRecorder`.
