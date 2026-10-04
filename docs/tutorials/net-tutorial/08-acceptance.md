# Chapter 08 — Acceptance and crate polish

## Abstract

This is the final chapter of the net tutorial. It locks the crate's
public surface with `#![deny(missing_docs)]`, documents the full
acceptance protocol for the phase-0 supervised network, and establishes
the determinism ritual that proves the pipeline is reproducible. By
the end you will have a fully documented `net` crate, a written
acceptance bar, and a reduced-scale verification run whose real numbers
are recorded below.

## Glossary

| Term | Definition |
|------|------------|
| **Acceptance protocol** | The repeatable procedure that decides whether the phase-0 network is ready for integration with search and self-play. |
| **Policy-argmax agreement** | The fraction of held-out win/block samples where the network's most likely move (`argmax` over policy logits) matches the argmax of the soft target policy. |
| **Value-sign accuracy** | The fraction of held-out samples where the sign of the network's value output matches the sign of the target value. |
| **Determinism ritual** | Running `nettrain` twice with the same seed and configuration and verifying that the reported loss trajectories are identical. |
| **Reduced-scale verification** | A small, fast run that proves the pipeline works; its metrics are not expected to meet the full acceptance bar. |
| **`#![deny(missing_docs)]`** | A crate-level attribute that makes a missing doc comment on any public item a compile error. |
| **Crate polish** | The final pass that adds the deny attribute and fixes any newly surfaced missing-docs errors. |

## Context

Chapters 1–7 built the complete `net` crate: planes, batcher, network,
loss, manual training loop, checkpointing, and the `nettrain` binary.
This chapter performs the final quality gates: the public surface is
locked with `#![deny(missing_docs)]`, the acceptance protocol is
written down, and the determinism ritual is executed.

The acceptance bar is invention I5 from the
[net tutorial design spec](../../specs/2026-10-04-net-tutorial-design.md):
after training on the full synthetic dataset, the network must achieve
at least 80% policy-argmax agreement on held-out win/block samples and
at least 85% value-sign accuracy on the held-out set. These numbers
are a starting bar, not a final research result; later milestones may
tune them.

The determinism ritual comes from the project's determinism policy
([docs/12-gomoku-architecture.md](../../12-gomoku-architecture.md)):
the same seed must produce the same result. The ritual in this chapter
runs two short seeded trainings and compares their loss trajectories.

## Intention

1. Ensure `#![deny(missing_docs)]` is present in
   `gomoku/crates/net/src/lib.rs` immediately after the inner doc
   comments and before the module declarations. It was added in
   Chapter 01; this step is the final polish sweep.
2. Run `cargo check -p net` and fix every missing-docs error surfaced
   by the new attribute. At this slice the only public items without
   doc comments should be the modules introduced in earlier chapters;
add a one-line doc comment to each.
3. Document the full acceptance protocol in this chapter.
4. Perform the determinism ritual and record the command and expected
   outcome.
5. Record the reduced-scale verification numbers from the reference
   implementation.

Observable done-state: `cargo test -p net` passes,
`cargo clippy --all-targets -- -D warnings` is green, `cargo fmt --all
-- --check` makes no changes, and the acceptance protocol is written
in this chapter.

## Mental mapping

### Why `#![deny(missing_docs)]` is enforced now, not just declared

The deny attribute was added to `src/lib.rs` in Chapter 01, but its
real enforcement happens in this chapter. Until now the crate's public
surface was still growing: each new module risked introducing an
undocumented public item that would break the build. By Chapter 8 the
surface is complete: `batcher`, `checkpoint`, `loss`, `network`,
`planes`, and `train`. Every public item now carries a documented
contract, so the gate passes cleanly.

This sweep is the crate-polish moment. It makes the `net` crate
navigable for the next milestone, which will consume `Model`,
`ModelConfig`, and the checkpoint functions as an external consumer.

### Why the acceptance bar is framed as a protocol, not a single test

The acceptance bar has two numbers, but the acceptance ritual is a
sequence of checks:

1. **Compilation and lint gate.** `cargo test`, `cargo clippy --all-targets
   -- -D warnings`, and `cargo fmt --all -- --check` all pass.
2. **Unit/integration gate.** Every test in the crate passes, including
   the checkpoint roundtrip, the binary smoke test, and the loss/metric
   tests from earlier chapters.
3. **Full-scale training gate.** Train on the default 50 000-sample
   synthetic dataset and evaluate on the held-out split.
4. **Metric gate.** The held-out evaluation meets the bar: ≥80%
   policy-argmax agreement on win/block samples and ≥85% value-sign
   accuracy across all held-out samples.
5. **Determinism gate.** Two short seeded runs produce identical loss
   trajectories.

Only when all five checks pass is the crate accepted. The reduced-scale
verification run below satisfies gates 1, 2, and 5 and provides a
smoke signal for gate 3; gate 4 is the bar that requires the full
protocol.

### Why the reduced-scale metrics are near chance

The reference verification run used only 1 500 samples (500 win, 500
block, 500 quiet), a `mid` network of 32 channels and 4 residual
blocks, 200 training steps, and batch size 64. That is intentionally
small enough to finish in minutes, but it is far below the data and
compute budget needed for the network to learn the tactical signal.

At that scale the training loss does decrease — from 6.239 to 6.001 —
because the network is fitting something. The held-out policy top-1
accuracy, however, is 0% and the value-sign accuracy is 47.9%. Those
numbers are essentially chance. They are not a failure; they are the
expected outcome of asking a small network to generalize from 200
gradient steps on 1 500 synthetic positions. The full protocol exists
precisely because the reduced-scale run cannot reach the bar.

This honesty matters. It would be easy to report only the decreasing
training loss and imply success. The acceptance protocol reports both
numbers: training loss goes down (the pipeline works), and holdout
metrics are at chance (the scale is too small to meet the bar).

### Why the full protocol targets win/block policy accuracy specifically

The synthetic dataset is built around tactical positions. Win and
block samples are the signal-carrying classes; quiet samples provide
background distribution. A network that cannot find the winning move
or the forced block on held-out tactical positions has not learned the
rules-true signal that the datagen pipeline was designed to teach.
Value-sign accuracy is broader because the value head must separate
winning (+1), blocking (−1), and quiet (0) positions across the whole
held-out set.

## Low-level design

### Files

```text
gomoku/crates/net/
└── src/
    └── lib.rs          # add #![deny(missing_docs)]
```

### `src/lib.rs`

The crate doc comment is already in place from Chapter 01 and the
`#![deny(missing_docs)]` attribute was added there. Confirm it sits on
its own line immediately after the closing `//!` and before the
`pub mod` declarations:

```rust
//! Neural network training for the Gomoku AlphaZero-style agent.
//!
//! ...

#![deny(missing_docs)]

pub mod batcher;
pub mod checkpoint;
...
```

Then run `cargo check -p net` and add a short `//!` or `///` doc
comment to any public item that the compiler flags. The reference
implementation has doc comments on every module, so the gate passes
cleanly.

### Full acceptance protocol

Environment: release build (`cargo build --release -p net`), a
dataset generated by the datagen tutorial's default quotas.

1. Generate the dataset:
   ```bash
   cargo run -p train --bin datagen -- --seed 42 --out data/phase0
   ```
   This produces 50 000 samples: 20 000 win, 20 000 block, 10 000
   quiet.

2. Train the default network:
   ```bash
   cargo run --release -p net --bin nettrain -- \
       --data data/phase0 \
       --out checkpoints/phase0-default \
       --config default \
       --seed 42 \
       --steps 20000 \
       --batch-size 1024 \
       --eval-interval 1000
   ```

3. Wait for completion. The default network is 128 channels, 10
   residual blocks, and the full protocol may take hours depending on
   hardware.

4. Inspect the final report printed to `stdout`. It should show a
   training loss that decreased over the run and holdout metrics that
   meet the bar.

5. Assert the bar:
   * Holdout policy-argmax agreement on win/block samples ≥ 80%.
   * Holdout value-sign accuracy ≥ 85%.

If both numbers are below the bar, the usual causes are: insufficient
steps, too small a batch size, or a dataset that does not match the
expected class distribution. Diagnose with the `run.json` journal and
the `datagen` stats report before changing the bar.

### Determinism ritual

Run two short trainings with the same seed and compare the reported
loss trajectories:

```bash
rm -rf /tmp/net-det-a /tmp/net-det-b
cargo run --release -p net --bin nettrain -- \
    --out /tmp/net-det-a --config tiny --seed 123 --steps 50 --batch-size 16 >/tmp/net-det-a.log 2>&1
cargo run --release -p net --bin nettrain -- \
    --out /tmp/net-det-b --config tiny --seed 123 --steps 50 --batch-size 16 >/tmp/net-det-b.log 2>&1
```

Then compare the loss lines:

```bash
grep "Train loss" /tmp/net-det-a.log /tmp/net-det-b.log
```

A successful ritual shows identical initial and final losses in both
runs. The `run.json` journals should also be byte-identical in their
seed, config, and metrics fields.

### Reduced-scale verification evidence

The reference implementation was verified at reduced scale with:

| Setting | Value |
|---------|-------|
| Dataset | 1 500 samples (win 500, block 500, quiet 500) |
| Network | `mid` — 32 channels, 4 residual blocks |
| Steps | 200 |
| Batch size | 64 |
| Seed | 42 |
| Build | release |
| Wall time | ~83 seconds |
| Initial train loss | 6.239 |
| Final train loss | 6.001 |
| Holdout policy top-1 | 0% |
| Holdout value-sign accuracy | 47.9% |

Interpretation: the pipeline runs end-to-end, the loss decreases, and
the checkpoint/journal machinery works. The holdout metrics are near
chance because 200 steps on 1 500 samples is far below the full
protocol. The full protocol is the bar; this smoke proves the pipeline.

## Solution (opt-in)

The complete reference code for this chapter — the `#![deny(missing_docs)]`
line and the final `src/lib.rs` — lives in
[08-acceptance/01-solution.md](08-acceptance/01-solution.md). Open it
only if you have been stuck for more than twenty minutes, or after you
have finished the chapter and want to compare.

## TDD checklist

1. **Red:** Confirm `#![deny(missing_docs)]` is in `src/lib.rs` and
   run `cargo check -p net`. Expect missing-docs errors from any
   newly-public items that lack doc comments.
2. **Green:** Add the missing module-level doc comments and re-run
   `cargo check -p net` until it is clean.
3. **Smoke:** Run the determinism ritual with `--config tiny --steps
   50 --seed 123` twice into two temp dirs and compare the final loss
   lines.
4. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p net` passes from `gomoku/`.
* `cargo test -p engine --features testutil` still passes (no engine
  changes this slice).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* `cargo check -p net` reports no missing-docs errors.
* The determinism ritual succeeds: two short seeded runs produce
  identical loss trajectories.
* This chapter documents the full acceptance protocol, the bar, and
  the reduced-scale verification numbers.
* Commit message in the reference worktree:
  `feat(net): acceptance protocol + crate polish`.
* Commit message in the main repository:
  `docs(tutorials): net tutorial chapter 8`.

## References

* [`docs/specs/2026-10-04-net-tutorial-design.md`](../../specs/2026-10-04-net-tutorial-design.md)
  — net tutorial design authority, inventions I5 (acceptance bar),
  I6 (checkpoint layout), and I7 (test-scale config).
* [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  workspace crate layout, the locked 128×10 architecture, and the
  determinism policy.
* [`docs/11-pitfalls.md`](../../11-pitfalls.md) — the
  `CompactRecorder` trap and other Burn 0.21 pitfalls relevant to
  acceptance runs.
* [`docs/tutorials/datagen-tutorial/08-acceptance.md`](../datagen-tutorial/08-acceptance.md)
  — the datagen acceptance ritual and default quotas that feed the
  full net protocol.
* [`docs/tutorials/net-tutorial/06-checkpointing-and-the-run-journal.md`](06-checkpointing-and-the-run-journal.md)
  — the `RunJournal` and `save_checkpoint` used to record run
  provenance.
* [`docs/tutorials/net-tutorial/07-the-nettrain-binary.md`](07-the-nettrain-binary.md)
  — the `nettrain` binary that executes the acceptance protocol.
