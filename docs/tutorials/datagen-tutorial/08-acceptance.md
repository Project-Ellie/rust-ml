# Chapter 08 — Acceptance: the `datagen` binary + crate polish

## Abstract

This is the final chapter of the datagen tutorial. It promotes the
pipeline from a library API to a runnable binary, locks the crate's
public surface with `#![deny(missing_docs)]`, and defines the
milestone-3 acceptance ritual. By the end you will have
`gomoku/crates/train/src/bin/datagen.rs`, an integration test in
`gomoku/crates/train/tests/datagen.rs`, a documented public surface,
and a written acceptance bar that the QA agent will execute after you
finish.

## Glossary

| Term | Definition |
|------|------------|
| **`datagen` binary** | The command-line front end of the synthetic-data pipeline: `cargo run -p train --bin datagen`. |
| **Hand-rolled arg parsing** | A small loop over `std::env::args` instead of a derive-based CLI crate; keeps `train`'s dependencies locked. |
| **Acceptance ritual** | The repeatable procedure that decides whether the milestone-3 dataset generator is ready for the net tutorial. |
| **Byte-identical regeneration** | Running the binary twice with the same seed and quotas produces bit-for-bit identical shard files and manifest. |
| **Crate polish** | Adding `#![deny(missing_docs)]` so every public item must carry a doc comment. |
| **Soundness gate** | The per-sample validation from [`dataset.rs`](06-reading-back.md): rebuilds the board, checks policy mass, and checks the tactical argmax. |
| **Phase-0 network** | The supervised pre-training step in the net tutorial that consumes this synthetic dataset. |
| **Top-1 held-out benchmark** | The net tutorial's acceptance metric: the network's most-likely move on held-out synthetic threats must be correct >90% of the time. |

## Context

Chapters 1–7 built the complete datagen library: the [`Sample`](01-the-train-crate.md)
record, seeded random playouts, tactics labels, quota collection,
streaming shards, the soundness gate, and the train/held-out split.
This chapter turns that library into a runnable tool and locks its
public surface.

The binary is deliberately small: it parses arguments, calls
[`collect`](04-the-quota-collector.md), calls
[`write_dataset`](05-shards-and-manifest.md), reads the shards back
with [`read_dataset`](06-reading-back.md), runs
[`check_soundness`](06-reading-back.md) on every sample, and prints the
[`stats`](07-the-held-out-split.md) report. The integration test runs
the compiled binary with tiny quotas and asserts that the full pipeline
succeeds.

The acceptance bar is not "the network trains"; that is the next
tutorial. The datagen acceptance bar is:

1. The generator produces the requested dataset (manifest + shards).
2. Every sample passes the soundness gate on read-back.
3. Regeneration with the same seed is byte-identical.
4. The stats report shows the designed class distribution.

The >90% top-1 held-out accuracy benchmark belongs to the net tutorial;
datagen's job is to hand it trustworthy data.

## Intention

1. Create `gomoku/crates/train/src/bin/datagen.rs`.
2. Implement hand-rolled argument parsing with these flags:
   * `--seed <u64>` — default `0`.
   * `--out <DIR>` — required.
   * `--win <n>` — default `20_000`.
   * `--block <n>` — default `20_000`.
   * `--quiet <n>` — default `10_000`.
   * Unknown flag → usage message and exit `2`.
3. Implement the pipeline in `main()`:
   * `collect(args.quotas, &mut rng)`.
   * `write_dataset(&samples, &args.out_dir, args.seed, args.quotas)`.
   * `read_dataset(&args.out_dir)`.
   * `check_soundness` on every read-back sample; fail loudly on the first unsound one.
   * `stats(&read_back)` and `println!("{report:?}")`.
4. Exit non-zero with a clear message on any failure.
5. Add `#![deny(missing_docs)]` to `gomoku/crates/train/src/lib.rs`
   (first line after the inner doc comments). Fix any newly-denied
   undocumented items.
6. Create `gomoku/crates/train/tests/datagen.rs` and implement two
   integration tests:
   * `datagen_runs_end_to_end_with_tiny_quotas` — run the binary with
     `--win 2 --block 2 --quiet 3 --seed 7 --out <tempdir>`, assert
     exit `0`, assert `manifest.json` and at least one shard exist,
     read the dataset back with the train crate's public API, and
     assert every sample passes `check_soundness`.
   * `datagen_fails_without_required_out_flag` — run the binary without
     `--out` and assert it exits non-zero.
   * `datagen_rejects_unknown_flag` — run the binary with an unknown
     flag and assert it exits non-zero.
7. Run a smoke test with small-but-real quotas (e.g. `100/100/50`) into
   a temp dir, confirm the printed stats look sane and runtime is in
   seconds.
8. Document the default quotas and the timing probe reasoning in this
   chapter.

Observable done-state: `cargo test -p train` passes,
`cargo clippy --all-targets -- -D warnings` is green, and
`cargo fmt --all -- --check` makes no changes.

## Mental mapping

### Why hand-rolled arg parsing?

`clap` is excellent, but adding it to `train` would expand the crate's
dependency footprint for a grand total of five flags and one required
argument. The datagen tutorial deliberately keeps `train`'s
dependencies to the locked set (`engine`, `rand`, `bincode`, `serde`,
`thiserror`, `serde_json`). A thirty-line loop over
`std::env::args().skip(1)` is enough:

* `--flag value` pairs are easy to parse.
* Unknown flags produce an explicit error.
* Missing `--out` is the only required-field error.

If the binary grows many subcommands later, `clap` becomes worthwhile.
For a single command with five optional flags, hand-rolled parsing is
simpler, compiles faster, and respects the dependency budget.

### Why `#![deny(missing_docs)]` now and not in chapter 1?

The mcts tutorial also locked its public surface in its final chapter.
The reason is the same: the crate's API is not stable until the last
slice lands. Adding the deny attribute in chapter 1 would have forced
the learner to write doc comments for structs whose responsibilities
were still changing. By chapter 8 the public surface is complete:
`Sample`, `Quotas`, `Manifest`, `Stats`, `TacticalClass`, and the
functions `collect`, `label`, `write_dataset`, `read_dataset`,
`check_soundness`, `is_holdout`, and `stats`. Every one of them now
carries a documented contract.

This attribute is the crate-polish moment: it makes the train crate
navigable for the net tutorial, which will import `Sample` and the
dataset functions as an external consumer.

### The acceptance ritual

Acceptance is not a single number. It is a sequence of checks that
either pass or fail:

1. **Generation check.** The binary exits `0` and writes `manifest.json`
   plus at least one `shard-NNN.bin`.
2. **Soundness check.** `read_dataset` plus `check_soundness` on every
   sample returns `Ok(())`. This is the strongest gate: it re-derives
   every board from stored stones and re-runs the engine tactics
   classification.
3. **Byte-identical regeneration check.** Run the binary twice with the
   same seed and quotas. Compare the output directories with `diff -r`.
   They must be identical. This proves determinism end-to-end, covering
   the RNG stream, the collector's Zobrist dedup ordering, and the
   shard writer's framing.
4. **Stats check.** The `Stats` report printed at the end shows:
   * `total` equals the sum of the requested quotas.
   * `per_class` matches the `--win`, `--block`, `--quiet` inputs.
   * `holdout` is approximately 10% of `total` (because the split rule
     is `zobrist % 10 == 0`).
   * `holdout + train == total`.
   * `ply_histogram` sums to `total`.

If all four checks pass, the dataset generator is ready for the net
tutorial. The actual full-scale acceptance run is performed by a
separate QA agent after this chapter is complete; this chapter
*describes* the ritual and the expected shape of the output, but does
not claim specific measured numbers that have not been observed.

### What the net tutorial will consume

The net tutorial reads the dataset produced by this binary. Its
contract with `train` is:

* **Encode on read.** The net tutorial calls `engine::encode` on each
  `Sample` to derive the 17×17×4 planes at batch time. The dataset
  stores positions, not planes.
* **D4 augment on read.** For every sample loaded into a training
  batch, the net tutorial applies one random D4 transform. Because the
  dataset holds canonical positions, the same record can be augmented
  differently on every epoch, multiplying the effective dataset size
  for free.
* **Held-out benchmark.** The net tutorial trains on the training split
  and reports top-1 accuracy on the held-out split. The target is >90%
  top-1 on synthetic threats. Datagen does not need to reach that
  number; datagen needs to produce the dataset that the net tutorial
  can learn from.

### Default quotas and the timing probe

The default quotas are:

```text
--win   20_000
--block 20_000
--quiet 10_000
```

These are `[experiment]` values. The design goal is a tactical dataset
large enough for phase-0 supervised training while keeping generation
time in the "minutes, not hours" range. A timing probe with quotas
`--win 100 --block 100 --quiet 50` (250 samples total) into a temp
directory produced output in roughly one second on a modern laptop.
Linear extrapolation suggests the default 50 000-sample dataset takes
on the order of a few minutes in debug mode. The QA agent will time the
real run and adjust the documented numbers if the extrapolation proves
too optimistic.

The win/block quotas are equal because both classes are tactical and
rare relative to quiet positions. The quiet quota is half the tactical
quotas because random playouts already produce quiet positions
naturally; capping quiet keeps the dataset from being dominated by
uninteresting positions.

## Low-level design

### Files

```text
gomoku/crates/train/
├── src/
│   ├── lib.rs          # add #![deny(missing_docs)]
│   └── bin/
│       └── datagen.rs  # new binary
└── tests/
    └── datagen.rs      # integration test
```

### `src/lib.rs`

Add `#![deny(missing_docs)]` immediately after the inner doc comments
and before the module declarations. Run `cargo check -p train` and fix
any missing-docs errors. At this slice all public items already carry
doc comments, so the gate should pass cleanly.

### `src/bin/datagen.rs`

```rust
const DEFAULT_WIN: usize = 20_000;
const DEFAULT_BLOCK: usize = 20_000;
const DEFAULT_QUIET: usize = 10_000;

struct Args {
    seed: u64,
    out_dir: PathBuf,
    quotas: Quotas,
}

fn main()
fn run(args: Args) -> Result<(), String>
fn parse_args<I: Iterator<Item = String>>(args: I) -> Result<Args, String>
fn print_manifest_summary(manifest: &Manifest, out_dir: &Path)
```

Implementation notes:

* `parse_args` walks `--flag value` pairs. Unknown flags and missing
  values return `Err(String)`; `main()` prints the error, prints usage,
  and exits `2`. `--out` is required.
* `run` creates a seeded `StdRng`, calls `collect`, calls
  `write_dataset`, prints a short manifest summary to `stderr`, calls
  `read_dataset`, checks every sample with `check_soundness`, and
  prints `stats(...)` via `Debug`.
* All user-facing progress messages go to `stderr`; the final `Stats`
  report goes to `stdout` so it can be captured or piped.
* Any I/O or soundness error becomes a `String` and exits `1`.

### `tests/datagen.rs`

Integration tests use `std::process::Command` and
`env!("CARGO_BIN_EXE_datagen")` to run the compiled binary:

1. `datagen_runs_end_to_end_with_tiny_quotas`:
   * Create a unique temp dir.
   * Run the binary with `--seed 7 --win 2 --block 2 --quiet 3 --out <dir>`
     and capture `stdout` and `stderr`.
   * Assert `status.success()`.
   * Assert `stderr` contains the soundness-pass confirmation
     (`"Soundness gate passed"`).
   * Assert `stdout` contains the stats report (`"Stats {"`).
   * Assert `manifest.json` exists and parses into a `Manifest` with
     matching seed and quotas.
   * Assert at least one `shard-*.bin` exists.
   * Call `train::read_dataset(&out_dir)` and assert the sample count
     equals `2 + 2 + 3`.
   * Call `train::check_soundness` on every sample and assert `Ok(())`.
   * Clean up the temp dir.
2. `datagen_fails_without_required_out_flag`:
   * Run the binary without `--out` and assert the status is not success.
3. `datagen_rejects_unknown_flag`:
   * Run the binary with `--nonsense` and assert the status is not
     success.

### Byte-identical regeneration ritual

After the code is complete, run the binary twice into two temp dirs
with identical arguments:

```bash
rm -rf /tmp/datagen-a /tmp/datagen-b
cargo run -p train --bin datagen -- --seed 42 --win 100 --block 100 --quiet 50 --out /tmp/datagen-a >/dev/null 2>&1
cargo run -p train --bin datagen -- --seed 42 --win 100 --block 100 --quiet 50 --out /tmp/datagen-b >/dev/null 2>&1
diff -r /tmp/datagen-a /tmp/datagen-b && echo "BYTE-IDENTICAL"
```

A successful diff confirms that the dataset is deterministic from seed
to bytes. This is the ritual the QA agent will repeat at the default
quotas.

## Solution (opt-in)

The complete reference code for this chapter — `datagen.rs`,
`tests/datagen.rs`, and the `lib.rs` polish line — lives in
[08-acceptance/01-solution.md](08-acceptance/01-solution.md). Open it
only if you have been stuck for more than twenty minutes, or after you
have finished the chapter and want to compare.

## TDD checklist

1. **Red:** Create `src/bin/datagen.rs` with the `Args` struct, the
   parsing function, and `run()` returning `Err(...)` for the
   unimplemented pipeline. Create `tests/datagen.rs` with the three
   integration tests. Add `#![deny(missing_docs)]` to `lib.rs`. Run
   `cargo test -p train` and expect the integration tests to fail.
2. **Green:** Implement the pipeline in `run()`, fill in `parse_args`,
   and run the integration tests until they pass. Fix any missing-docs
   errors surfaced by the new deny attribute.
3. **Smoke:** Run the binary with `--win 100 --block 100 --quiet 50`
   into a temp dir. Inspect the stats report and confirm runtime is
   seconds. Run the byte-identical regeneration ritual with two temp
   dirs.
4. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p train` passes from `gomoku/`.
* `cargo test -p engine --features testutil` still passes (no engine
  changes this slice).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* The byte-identical regeneration ritual succeeds for at least the
  smoke-test quotas.
* Commit message in the reference worktree:
  `feat(train): datagen binary + crate polish`.
* Commit message in the main repository:
  `docs(tutorials): datagen chapter 8`.

## References

* [`docs/specs/2026-09-27-datagen-tutorial-design.md`](../../specs/2026-09-27-datagen-tutorial-design.md)
  — synthetic data generator design, acceptance bar, and default quota
  rationale.
* [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  workspace crate layout, store-games replay-buffer decision, and the
  phase-0 network plan.
* [`docs/tutorials/datagen-tutorial/04-the-quota-collector.md`](04-the-quota-collector.md)
  — `collect` and `Quotas`.
* [`docs/tutorials/datagen-tutorial/05-shards-and-manifest.md`](05-shards-and-manifest.md)
  — `write_dataset` and `Manifest`.
* [`docs/tutorials/datagen-tutorial/06-reading-back.md`](06-reading-back.md)
  — `read_dataset` and `check_soundness`.
* [`docs/tutorials/datagen-tutorial/07-the-held-out-split.md`](07-the-held-out-split.md)
  — `stats` and the train/held-out split.
