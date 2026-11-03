# Sub-chapter 03 — Gravity takes the wheel

## Abstract

This sub-chapter wires the gravity map from sub-chapter 02 into the
`BaselineEvaluator`. The evaluator now returns policy logits that pull
the PUCT search toward existing stones. The value stays flat at `0.0`.
The seat stops spraying simulations across the whole board and starts
to hug the fight.

## Glossary

| Term | Definition |
|------|------------|
| **Policy logit** | An unnormalized log-probability sent to the search. A higher logit asks for more simulations at that cell. |
| **Gravity map** | The 15×15 normalized neighborhood mass produced by `GravityKernel` in sub-chapter 02. Values are in `[0, 1]`. |
| **Policy readout** | `baseline::readout::policy_logits`. It turns a gravity map and a legal-move list into a 225-vector of logits. |
| **Masked softmax** | `mcts::eval::masked_softmax`. The search normalizes the logits over legal cells only. |
| **`baseline::Device`** | A re-export of `burn::tensor::Device`. It lets the `cli` crate pass `Device::flex()` without adding a Burn dependency. |
| **BaselineEvaluator v1** | The evaluator with a `GravityKernel` and the policy readout. It returns gravity-guided priors and a zero value. |

## Context

Sub-chapter 01 built the `BaselineEvaluator` as a uniform policy with a
zero value. Sub-chapter 02 built the `GravityKernel` and printed its
heat map, but the evaluator still ignored it. This sub-chapter closes
the loop: the heat map becomes the policy.

The conceptual bridge to land is that **policy logits are not scores to
maximize**. They are instructions that tell the search where to look.
With gravity as the only tier, 4,000 simulations concentrate near the
stones instead of spreading over 200 empty cells.

## Intention

The learner enables this user-facing behavior:

1. The baseline seat answers beside an opening stone instead of playing
   far away.
2. Self-play shows stones clustering into a local fight.
3. The value is still flat, so every non-terminal Q-value comes from
   real wins and losses the tree finds.
4. The `cli` crate still does not depend on Burn.

## Specification as tests

The tests are ordered along the execution path: build the readout,
then integrate it through `BaselineEvaluator::evaluate`.

### Readout unit tests

* `readout::tests::logits_are_uniform_when_gravity_is_zero` — every
  legal cell gets logit `0.0` when the gravity map is all zeros.
* `readout::tests::logits_mask_illegal_cells` — an occupied cell gets
  `f32::NEG_INFINITY` even when gravity is zero everywhere.

### Evaluator integration tests

* `BaselineEvaluator::tests::v1_highest_logit_hugs_stones` — after a
  centre stone, the maximum legal logit is exactly `2.0 / 32.0`, and a
  far cell stays at `0.0`.
* `BaselineEvaluator::tests::v1_empty_board_is_uniform` — on an empty
  board every legal cell has logit `0.0`.
* `BaselineEvaluator::tests::v1_corner_stone_not_boosted` — after a
  corner stone the maximum legal logit is still `2.0 / 32.0`, and the
  sum over all legal cells is strictly less than `1.0`.
* `BaselineEvaluator::tests::v1_value_still_zero` — the value returned
  by `evaluate` is exactly `0.0`.

### Note on determinism

The shipped commit keeps the `v0_is_deterministic` test from
sub-chapter 01. The build contract also named a `v1_is_deterministic`
test, but that separate copy is not present at commit `0fb4326`.
Determinism is still asserted by the v0 test.

## Walkthrough

The walkthrough follows one evaluation call from the command line to
the logit vector.

### 1. The CLI builds the evaluator

`cli::session::SessionCore::make_evaluator` matches
`EvaluatorKind::Baseline` and builds:

```rust
baseline::BaselineEvaluator::new(baseline::Device::flex())
```

`baseline::Device` is a re-export of `burn::tensor::Device`. The `cli`
crate uses it without adding Burn to its own `Cargo.toml`.

### 2. MCTS reaches a leaf and calls `evaluate`

The search passes an `EvalRequest` with the side-to-move-relative
`planes` and the current `legal` move list.

### 3. `BaselineEvaluator::evaluate` computes the gravity map

It calls `self.kernel.compute(&req.planes)`. The kernel builds the
color-blind occupancy plane, runs the 5×5 convolution, and normalizes
by `32.0`.

### 4. `readout::policy_logits` builds the logit vector

The readout starts with `[f32::NEG_INFINITY; 225]`. For each legal move
it writes `gravity[row][col]`. Occupied or illegal cells keep the
negative-infinity mask.

### 5. `evaluate` returns the result

It returns `EvalResult { policy: logits, value: 0.0 }`.

### 6. The search normalizes the logits

`mcts::expand` calls `masked_softmax` over the legal cells only. A cell
near a stone has a larger logit and receives more prior mass.

### 7. PUCT concentrates simulations

Selection walks the tree using priors and backed-up Q-values. With
4,000 simulations, most of them now visit the neighborhood around the
stones.

### 8. Move selection follows visits

`select_move` samples from the visit distribution. The chosen move is
likely to be near the fight.

## Low-level design

### `gomoku/crates/baseline/src/readout.rs`

New module. Public surface:

```rust
pub fn policy_logits(gravity: &[[f32; 15]; 15], legal: &[Move]) -> [f32; 225]
```

Behavior: start with all cells masked to `f32::NEG_INFINITY`; for each
legal move write the gravity value at that row and column.

### `gomoku/crates/baseline/src/lib.rs`

Add:

```rust
pub mod readout;
pub use burn::tensor::Device;
```

The re-export exists so `cli` can pass `Device::flex()` without taking
a direct Burn dependency.

Change `BaselineEvaluator` to:

```rust
pub struct BaselineEvaluator {
    device: Device,
    kernel: GravityKernel,
}
```

Add constructor:

```rust
pub fn new(device: Device) -> Self
```

Behavior: build a `GravityKernel::new(&device)` and store both fields.

Change `evaluate`:

```rust
fn evaluate(&mut self, req: EvalRequest) -> EvalResult
```

Behavior: compute the gravity map, build policy logits, return value
`0.0`.

### `gomoku/crates/cli/src/session.rs`

In `make_evaluator`, the `EvaluatorKind::Baseline` arm passes
`baseline::Device::flex()` to `BaselineEvaluator::new`.

## TDD checklist

Work one test at a time, red to green.

From `baseline/src/readout.rs`:

1. `logits_are_uniform_when_gravity_is_zero`
2. `logits_mask_illegal_cells`

From `baseline/src/lib.rs`:

3. `v1_highest_logit_hugs_stones`
4. `v1_empty_board_is_uniform`
5. `v1_corner_stone_not_boosted`
6. `v1_value_still_zero`

## Verification commands

From `gomoku/`:

```text
cargo test -p baseline
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Capstone

Play against the baseline seat at 4,000 simulations:

```text
cargo run -q -p cli --bin gomoku -- play --ai baseline --no-swap2 --sims 4000
```

Make an opening move in a corner or near the centre, then press Enter
to let the AI step. Watch the answer land beside your stone, not on the
far side of the board.

Then run self-play:

```text
cargo run -q -p cli --bin gomoku -- selfplay --ai baseline --sims 4000
```

Press Enter a few times. The stones cluster into a local fight because
the priors pull both seats toward the existing mass.

### Why this matters

The raw gravity value `g ∈ [0, 1]` becomes the logit unchanged. Later,
in sub-chapter 04 and beyond, that same value will sit at the bottom of
the tier table as **T5**. Using it raw now plants that seed: the trust
ladder starts with gravity at the base, unable to outvote any higher
tier once they arrive.

## Done when

* The six tests above pass.
* `cargo test -p cli` passes.
* `cargo clippy --all-targets -- -D warnings` passes.
* `cargo fmt --all -- --check` makes no changes.
* The capstone shows the AI answering beside your stone and self-play
  clustering.
* The worktree commit is:
  `feat(baseline): gravity takes the wheel — baseline sub-chapter 03`

## References

* [`../00-baseline-primer.md`](../00-baseline-primer.md) — §2 MCTS
  interlock, §4 gravity, §5 the tier table that gravity's logit will
  become.
* [`../02-the-vicinity-kernel/the-vicinity-kernel.md`](../02-the-vicinity-kernel/the-vicinity-kernel.md)
  — the `GravityKernel` this sub-chapter wires in.
* [`../04-wins-and-blocks/wins-and-blocks.md`](../04-wins-and-blocks/wins-and-blocks.md)
  — the next sub-chapter, where T1 and T2 tiers outrank gravity.
* `gomoku/crates/baseline/src/readout.rs` — the policy readout module.
* `gomoku/crates/baseline/src/lib.rs` — the v1 evaluator.
* `gomoku/crates/cli/src/session.rs` — the `make_evaluator` seat wiring.
