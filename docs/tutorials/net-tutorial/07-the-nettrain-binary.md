# Chapter 07 — The `nettrain` binary

## Abstract

This chapter promotes the `net` crate from a library into a runnable
training tool. It adds `gomoku/crates/net/src/bin/nettrain.rs`, a
hand-rolled command-line parser, three network-size presets, and an
integration test that runs the binary for a bounded number of steps on
a tiny generated dataset. By the end you will have a binary that exits
with documented codes, sends progress to `stderr` and the final report
to `stdout`, and is covered by an end-to-end smoke test.

## Glossary

| Term | Definition |
|------|------------|
| **`nettrain` binary** | The command-line front end for phase-0 supervised training: `cargo run -p net --bin nettrain`. |
| **`parse_args`** | The pure, testable argument parser: `fn parse_args<I: Iterator<Item=String>>(args: I) -> Result<Args, String>`. |
| **Usage error** | A malformed command line: missing required flag, unknown flag, missing value, or invalid value. Exits `2`. |
| **Run failure** | A failure inside training: empty dataset, I/O error, Burn error, or checkpoint error. Exits `1`. |
| **Config preset** | A named network size: `tiny` (8 channels, 1 block), `mid` (32 channels, 4 blocks), or `default` (128 channels, 10 blocks). |
| **Progress stream** | Status messages written to `stderr` during training. |
| **Report stream** | The final summary written to `stdout` so it can be captured or piped. |

## Context

Chapters 1–6 built every library piece of the `net` crate: planes,
batcher, network, loss, the manual training loop, and checkpointing.
This chapter wires those pieces to a command-line entry point so that
a training run can be started with a single shell command.

The binary follows the same contract as the `datagen` binary in the
previous tutorial. That contract is deliberate: pure argument parsing
that can be unit-tested without spawning a process, explicit exit
codes, separation of progress and report streams, and a tiny default
configuration so casual runs finish in seconds. The design spec
captures this as invention I7 (test-scale config) and mirrors the CLI
pattern already established in the datagen tutorial.

## Intention

1. Create `gomoku/crates/net/src/bin/nettrain.rs`.
2. Define an `Args` struct holding `out_dir`, optional `data_dir`,
   `config` (`ModelConfig`), and `train_config` (`TrainConfig`).
3. Implement `parse_args` with signature
   `fn parse_args<I>(mut args: I) -> Result<Args, String> where I: Iterator<Item = String>`.
   Supported flags:
   * `--out <DIR>` — required.
   * `--data <DIR>` — optional; if omitted, generate 200 samples
     in-memory.
   * `--config tiny|mid|default` — default `tiny`.
   * `--seed <u64>` — default `0`.
   * `--steps <n>` — default `20`.
   * `--batch-size <n>` — default `16`.
   * `--eval-interval <n>` — default `0` (no intermediate eval).
   * Unknown flag → `Err("unknown flag: ...")`.
   * Missing value for a flag → `Err("--flag requires a value")`.
4. `main()` prints usage to `stderr` and exits `2` on parse failure.
5. `run(args)` performs the training:
   * Pick `Autodiff<Flex>` and `FlexDevice`.
   * Load or generate the dataset.
   * Split into train and holdout with `train::split::is_holdout`.
   * Initialize the model from the config.
   * Call `net::train::train`.
   * Build a `RunJournal` and save a checkpoint.
   * Print the final report.
6. Progress messages go to `stderr`; the final report goes to `stdout`.
7. Any run failure returns `Err(String)`, which `main()` prints and
   exits `1`.
8. Add unit tests for `parse_args` inside the binary module:
   * `--out` is required.
   * Defaults select the tiny config and the documented step/batch
     defaults.
   * Unknown config name is rejected.
   * All flags roundtrip correctly.
9. Create `gomoku/crates/net/tests/nettrain_smoke.rs` and implement
   one integration test that runs the compiled binary for 20 steps on
   a tiny in-memory dataset, asserts exit `0`, asserts both
   `model.mpk` and `run.json` exist, loads the journal, and asserts
   the final step is `20` and the final loss is finite.

Observable done-state: `cargo test -p net` passes,
`cargo clippy --all-targets -- -D warnings` is green, and `cargo fmt
--all -- --check` makes no changes.

## Mental mapping

### Why hand-rolled argument parsing?

`clap` is excellent, but it adds a dependency for a binary with eight
flags and one required argument. The datagen tutorial kept its binary
dependency-free by hand-rolling a small loop over
`std::env::args().skip(1)`. The `nettrain` binary has more flags but
no subcommands, no positional arguments, and no generated help beyond
a usage string. A hand-rolled parser stays within the project's
dependency budget and keeps the binary self-contained.

The more important reason is testability. `parse_args` takes a generic
`Iterator<Item = String>` rather than reading `env::args()` itself.
That lets unit tests feed it vectors of strings and assert on the
result without spawning processes. It also lets the integration test
use `std::process::Command` while the unit tests stay fast and
hermetic.

### Why `tiny` is the default config

The default config is the one used when a user runs `nettrain` with no
`--config` argument. The reference implementation uses `tiny` so that
casual runs finish quickly. The production default — the locked
128-channel, 10-block architecture from
[docs/12-gomoku-architecture.md](../../12-gomoku-architecture.md) — is
available as `default`. Naming the presets `tiny`, `mid`, and
`default` keeps the CLI honest: `default` is the architecture decision
from the architecture document, while `tiny` and `mid` are test and
experiment presets.

### Why exit 2 for usage errors and exit 1 for run failures?

This convention matches the datagen binary and the common Unix
pattern:

* Exit `0` — success.
* Exit `2` — the user invoked the tool incorrectly. This is the
  domain of Bash's `exit 2` convention for misuse and is distinct
  from a runtime failure.
* Exit `1` — the tool was invoked correctly but failed while doing
  its job (dataset read failed, training panicked, checkpoint write
  failed).

The integration test in this chapter only checks the success path;
the unit tests cover the usage-error paths.

### Why progress goes to `stderr` and the report goes to `stdout`?

`stderr` is for human-oriented progress and diagnostics. `stdout` is
for the machine-readable final report. Separating them lets a script
run `nettrain --out ... > report.txt` and capture only the summary,
while still seeing progress on the terminal. It also mirrors the
datagen binary's output contract.

### Why the binary generates a tiny dataset when `--data` is omitted?

The `--data` flag lets the binary train on a real dataset produced by
the datagen tutorial. When it is omitted, the binary falls back to a
tiny in-memory dataset (`win: 50, block: 50, quiet: 100`) so that a
bare `cargo run --bin nettrain -- --out /tmp/x` is a useful smoke run
rather than an error. The fallback is deterministic under `--seed` and
uses `train::collect` directly.

## Low-level design

### Files

```text
gomoku/crates/net/
├── src/
│   └── bin/
│       └── nettrain.rs   # new binary
└── tests/
    └── nettrain_smoke.rs # new integration test
```

### `src/bin/nettrain.rs`

```rust
//! Train the Gomoku policy/value network from synthetic data.
//!
//! Usage:
//!
//! ```text
//! nettrain --out <DIR> [--data <DIR>] [--config tiny|mid|default]
//!          [--seed <u64>] [--steps <n>] [--batch-size <n>]
//!          [--eval-interval <n>]
//! ```
//!
//! If `--data` is omitted, a tiny synthetic dataset is generated in-memory
//! for a quick smoke run.
```

Implementation notes:

* `use burn::backend::flex::FlexDevice; use burn::backend::{Autodiff, Flex};`
* `use net::checkpoint::{RunJournal, save_checkpoint};`
* `use net::network::{Model, ModelConfig};`
* `use net::train::{EvalMetrics, StepMetrics, TrainConfig, train};`
* `use train::collect::{Quotas, collect};`
* `use train::dataset::read_dataset;`
* `use train::split::is_holdout;`
* `main()` calls `parse_args(env::args().skip(1))`. On error, print the
  message, print usage, and `process::exit(2)`. On `run` error, print
  the message and `process::exit(1)`.
* `run(args)`:
  1. `type B = Autodiff<Flex>; let device = FlexDevice;`
  2. Print config summary to `stderr`.
  3. `let (train_samples, holdout_samples) = load_or_generate_data(&args)?;`
  4. `let model: Model<B> = args.config.init(&device);`
  5. `let (model, step_metrics, eval_metrics) = train(...);`
  6. Build `RunJournal` from the last step/eval metrics.
  7. `save_checkpoint::<B>(&model, &journal, &args.out_dir)`.
  8. `print_report(...)`.
* `load_or_generate_data` reads `train::dataset::read_dataset` if
  `data_dir` is set, otherwise calls `collect` with tiny quotas and
  splits with `is_holdout`. Returns `Err("dataset is empty")` if no
  samples were loaded.
* `print_report` prints to `stdout`:
  * `Checkpoint written to <out_dir>`.
  * `Train loss: initial=... final=... over <steps> steps`.
  * `Holdout @ step <step>: loss=... policy_top1=...% value_sign=...%`.
* `parse_args` uses `Option<T>` for every flag and applies defaults
  only after the loop. This keeps the parsing logic and the default
  values separate.
* The `--config` value maps to `ModelConfig::tiny()`, `ModelConfig::mid()`,
  or `ModelConfig::new()`; any other value returns an error.
* The parsed `train_config` sets `lr_warmup_steps = (steps / 10).max(1)`
  and `lr_cosine_steps = steps` so the learning-rate schedule stays
  sane for short smoke runs.

### Unit tests inside `src/bin/nettrain.rs`

Under `#[cfg(test)] mod tests`:

1. `parse_args_requires_out` — feed `["--seed", "42"]` and assert the
   error contains `"--out is required"`.
2. `parse_args_default_to_tiny_config` — feed `["--out", "/tmp/net"]`
   and assert `args.config == ModelConfig::tiny()` and the default
   steps/batch-size match.
3. `parse_args_rejects_unknown_config` — feed `["--out", "/tmp/net",
   "--config", "huge"]` and assert error.
4. `parse_args_roundtrips_values` — feed every flag with non-default
   values and assert each lands in the right field.

### `tests/nettrain_smoke.rs`

```rust
//! Integration smoke test for the `nettrain` binary.
//!
//! Runs `nettrain` for a bounded number of steps on a tiny in-memory
//! generated dataset and verifies that it produces a checkpoint and a
//! decreasing training loss.
```

Implementation notes:

* Use `env!("CARGO_BIN_EXE_nettrain")` to run the compiled binary,
  matching the datagen tutorial's integration-test pattern.
* Create a unique temp directory from nanoseconds + PID.
* Run with `--out <dir> --steps 20 --batch-size 8 --seed 42`.
* Assert `status.success()`.
* Assert `out_dir.join("model.mpk").exists()` and
  `out_dir.join("run.json").exists()`.
* Load the journal with `net::checkpoint::load_journal`.
* Assert `journal.seed == 42`.
* Assert `journal.final_step_metrics` is `Some`, its step is `20`, and
  its loss is finite.
* Clean up the temp directory.

## Solution (opt-in)

The complete reference code for this chapter lives in
[07-the-nettrain-binary/01-solution.md](07-the-nettrain-binary/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare.

## TDD checklist

1. **Red:** Create `src/bin/nettrain.rs` with the `Args` struct,
   `parse_args` signature, `run` returning `Err(...)` for the
   unimplemented pipeline, and the four `parse_args` unit tests.
   Create `tests/nettrain_smoke.rs` with the integration test. Run
   `cargo test -p net` and expect failures.
2. **Green:** Implement `run`, `load_or_generate_data`, `print_report`,
   and `parse_args`. Re-run `cargo test -p net` until both unit and
   integration tests pass.
3. **Smoke:** Run the binary directly:
   `cargo run -p net --bin nettrain -- --out /tmp/net-smoke`.
   Confirm it exits `0`, writes `model.mpk` and `run.json`, and prints
   the report to `stdout` while progress goes to `stderr`.
4. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p net` passes from `gomoku/`.
* `cargo test -p engine --features testutil` still passes (no engine
  changes this slice).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* `cargo run -p net --bin nettrain -- --out /tmp/net-smoke` succeeds
  and writes both checkpoint files.
* Commit message in the reference worktree:
  `feat(net): nettrain binary + smoke test`.
* Commit message in the main repository:
  `docs(tutorials): net tutorial chapter 7`.

Next: [Chapter 08 — Acceptance and crate polish](08-acceptance.md).

## References

* [`docs/specs/2026-10-04-net-tutorial-design.md`](../../specs/2026-10-04-net-tutorial-design.md)
  — net tutorial design authority, inventions I6 (checkpoint layout)
  and I7 (test-scale config).
* [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  workspace crate layout, network anatomy, and the locked 128×10
  architecture that becomes the `default` preset.
* [`docs/tutorials/datagen-tutorial/08-acceptance.md`](../datagen-tutorial/08-acceptance.md)
  — the datagen binary contract that `nettrain` mirrors.
* [`docs/tutorials/net-tutorial/05-the-manual-training-loop.md`](05-the-manual-training-loop.md)
  — the manual training loop called by `run`.
* [`docs/tutorials/net-tutorial/06-checkpointing-and-the-run-journal.md`](06-checkpointing-and-the-run-journal.md)
  — `RunJournal` and `save_checkpoint` used by the binary.
