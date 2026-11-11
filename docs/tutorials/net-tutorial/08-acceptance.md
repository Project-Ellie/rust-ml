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

### Reality check (2026-11-11): the gate 4 metric and evaluation mode

The shipped `EvalMetrics` adds the two fields that gate 4 needs:

* `win_block_policy_top1_accuracy` is an `Option<f32>`. It is `None`
  (`null` in JSON) when the holdout has no Win or Block samples.
* `win_block_policy_samples` counts the tactical samples behind that
  accuracy.

Evaluation filters the holdout to Win and Block samples and makes a
second pass over them. This pass is unshuffled and uses the canonical
`NetBatcher::holdout()` orientation. Both holdout passes run under
`model.valid()`.

Policy correctness accepts any move with maximal positive target mass.
This matters when several winning or blocking moves share the target
maximum. Value-sign accuracy is a true three-way comparison: negative,
zero, and positive are distinct classes.

The new fields appear in every `Eval` record in `metrics.jsonl`, in the
final metrics in `run.json`, in the final `stdout` report, and in the line
reporter. Use `win_block_policy_top1_accuracy`, not the all-class
`policy_top1_accuracy`, for the 80% tactical-policy gate.

Holdout evaluation did not always use inference mode. Runs made before
the `model.valid()` fix used training-statistics forwards. Their old
value-sign calculation also folded zero and negative predictions into one
class, which could inflate the reported value-sign accuracy. This
includes the earlier `data/check-points/v0` run. Those metrics are
historical diagnostics, **not acceptance evidence**.

### Determinism ritual

Run two short trainings with the same seed, the Flex device, no
dataloader workers, and no optional display:

```bash
rm -rf /tmp/net-det-a /tmp/net-det-b
cargo run --release -p net --bin nettrain -- \
    --out /tmp/net-det-a --config tiny --seed 123 --steps 50 --batch-size 16 \
    --device flex --dataloader-workers 0 --progress off
cargo run --release -p net --bin nettrain -- \
    --out /tmp/net-det-b --config tiny --seed 123 --steps 50 --batch-size 16 \
    --device flex --dataloader-workers 0 --progress off
```

Compare the ordered `Step` records, preserving their original JSON lines:

```bash
python3 - <<'PY'
import json
from pathlib import Path

def step_records(path):
    records = []
    for line in Path(path).read_bytes().splitlines():
        if "Step" in json.loads(line):
            records.append(line)
    return records

a = step_records("/tmp/net-det-a/metrics.jsonl")
b = step_records("/tmp/net-det-b/metrics.jsonl")
assert len(a) == len(b) == 50
assert a == b, "Step records differ"
print("50/50 Step records are byte-identical")
PY
cmp /tmp/net-det-a/run.json /tmp/net-det-b/run.json
```

Do not substitute `grep "Train loss"`: the final summary contains only
the first and last training losses, and the line reporter now has a
different, throttled per-step format. Do not compare the complete JSONL
files either. `Started.started_at` is a wall-clock timestamp and can
differ between runs. It is not a field in `run.json`; the journals should
be byte-identical.

**[measured 2026-11-11, revision `8361ae3`]** The ritual passed on this
revision: 50/50 ordered `Step` records were byte-identical. The first
step loss was `5.899358`; the step 50 loss was `5.979966`. The two
`run.json` journals were byte-identical. The JSONL timestamp was excluded
from the comparison.

### Full-scale acceptance run (executed 2026-11-11, stopped early)

The run used this exact command:

```bash
nettrain --data data/synthetic/v0 --out data/check-points/phase0-default --config default --seed 42 --steps 20000 --batch-size 1024 --eval-interval 1000 --device metal --progress off
```

Metal training numerics are experimental and are not parity-verified
against Flex. This is an experimental run, not backend-parity evidence.

**Gate 4 result [measured]:** the owner stopped the run at step 1490 of
20000 — by then the evidence was unambiguous, and the remaining hours
were ceremonial. From `data/check-points/phase0-default/metrics.jsonl`,
eval at step 1000 (4 056 tactical holdout samples):

| Metric | Bar | Measured | Verdict |
|--------|-----|----------|---------|
| `win_block_policy_top1_accuracy` | ≥ 80% | **97.14%** | met, decisively |
| `value_sign_accuracy` | ≥ 85% | **78.87%** and climbing | not yet met at stop time |

The value gate was accepted on trend, by owner decision: the metric had
risen from 47.9% (reduced-scale) to 78.9% in the first 5% of the budget,
the pipeline defects that once inflated it were fixed, and the phase-0
model is a smoke artifact, not a contender — the mentor data, not this
model, is where strength comes from. No checkpoint or journal was kept:
the run was terminated rather than gracefully stopped, and the eval
trajectory in `metrics.jsonl` is the evidence.

**Protocol amendment.** The acceptance criterion is the bar, not the
budget: *train until the metric gates are met or their trend is
unambiguous, then stop; 20 000 steps is the budget, not the
requirement.* A future run that needs the full budget is free to use it.

### Reduced-scale verification evidence

> **Historical evidence [measured].** This table predates the fast
> device/dataloader path and the 2026-11-11 metric and inference-mode
> fixes. Its holdout rows use the superseded evaluation and value-sign
> semantics. Keep them as historical output, not as acceptance evidence.

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
   50 --seed 123 --device flex --dataloader-workers 0 --progress off`
   twice into two temp dirs and compare the ordered `Step` records in
   `metrics.jsonl`.
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
  `CompactRecorder` trap and other Burn 0.22 pitfalls relevant to
  acceptance runs.
* [`docs/tutorials/datagen-tutorial/08-acceptance.md`](../datagen-tutorial/08-acceptance.md)
  — the datagen acceptance ritual and default quotas that feed the
  full net protocol.
* [`docs/tutorials/net-tutorial/06-checkpointing-and-the-run-journal.md`](06-checkpointing-and-the-run-journal.md)
  — the `RunJournal` and `save_checkpoint` used to record run
  provenance.
* [`docs/tutorials/net-tutorial/07-the-nettrain-binary.md`](07-the-nettrain-binary.md)
  — the `nettrain` binary that executes the acceptance protocol.
