# Sub-chapter 03 — Gravity takes the wheel: implementation plan

This document turns the walkthrough into a construction order. It names
the files, structs, functions, and tests. It states behavioral
contracts in prose. It does not contain method bodies.

Read [`gravity-takes-the-wheel.md`](./gravity-takes-the-wheel.md) first.
Work one step at a time. Write tests before code. Open
[`gravity-takes-the-wheel-solution.md`](./gravity-takes-the-wheel-solution.md)
only if you are stuck for more than twenty minutes, or after the
sub-chapter is complete.

---

## Step 0 — Confirm the workspace state

Sub-chapter 02 left these files in place:

* `gomoku/crates/baseline/src/gravity.rs` with `GravityKernel`.
* `baseline` `Cargo.toml` with `burn` (flex) and `patterns`.
* `cli` `Cargo.toml` with the optional `baseline` dependency and the
  `baseline` feature.

No `Cargo.toml` changes are needed for this sub-chapter.

---

## Step 1 — `baseline/src/readout.rs`: the policy readout

Create `gomoku/crates/baseline/src/readout.rs`.

### 1.1 Module documentation

Write a module-level doc comment that explains the purpose: turn a
gravity map into a masked 225-vector of policy logits.

### 1.2 Imports

Bring in `engine::Move`.

### 1.3 `policy_logits`

Signature:

```rust
pub fn policy_logits(gravity: &[[f32; 15]; 15], legal: &[Move]) -> [f32; 225]
```

Behavior:

* Start with an array of `f32::NEG_INFINITY` for all 225 cells.
* For each move in `legal`, compute its row and column, then write
  `gravity[row][col]` into the array at the move's index.
* Return the array.

### 1.4 Tests

Add unit tests in `readout.rs`:

* `logits_are_uniform_when_gravity_is_zero`
  * Build a gravity map of all `0.0`.
  * Build the legal list from an empty board.
  * Assert every legal cell has logit `0.0`.

* `logits_mask_illegal_cells`
  * Build a gravity map of all `0.0`.
  * Build the legal list from a board with one stone played.
  * Assert the occupied cell has logit `f32::NEG_INFINITY`.

### 1.5 Gate

Run `cargo test -p baseline`. Expect failures from the missing
`readout` module in `lib.rs` and from the not-yet-written v1 tests.

---

## Step 2 — `baseline/src/lib.rs`: wire the readout into the evaluator

### 2.1 Add the module declaration

Add `pub mod readout;` next to `pub mod gravity;`.

### 2.2 Re-export `Device`

Add:

```rust
pub use burn::tensor::Device;
```

Document why the re-export exists: the `cli` crate must pass the flex
device when it builds the evaluator, but `cli` must not depend on Burn
directly.

### 2.3 Change `BaselineEvaluator`

Change the struct from a zero-sized unit struct to:

```rust
pub struct BaselineEvaluator {
    device: Device,
    kernel: GravityKernel,
}
```

Update the doc comment to say v1 uses gravity policy and a flat value.

### 2.4 Add `BaselineEvaluator::new`

Signature:

```rust
pub fn new(device: Device) -> Self
```

Behavior: build a `GravityKernel::new(&device)` and store both fields.

### 2.5 Change `Evaluator::evaluate`

Signature stays the same:

```rust
fn evaluate(&mut self, req: EvalRequest) -> EvalResult
```

Behavior:

* Compute the gravity map with `self.kernel.compute(&req.planes)`.
* Build policy logits with `readout::policy_logits(&gravity, &req.legal)`.
* Return `EvalResult { policy: logits, value: 0.0 }`.

### 2.6 Tests

Add these integration tests to the existing test module in `lib.rs`:

* `v1_highest_logit_hugs_stones`
  * Play one stone at the centre.
  * Build an `EvalRequest` from the board.
  * Evaluate and find the maximum logit over legal cells.
  * Assert the maximum is exactly `2.0 / 32.0`.
  * Assert a far cell such as `(0, 0)` still has logit `0.0`.

* `v1_empty_board_is_uniform`
  * Evaluate on an empty board.
  * Assert every legal cell has logit `0.0`.

* `v1_corner_stone_not_boosted`
  * Play one stone at `(0, 0)`.
  * Evaluate and find the maximum legal logit.
  * Assert the maximum is exactly `2.0 / 32.0`.
  * Sum all legal logits and assert the sum is strictly less than `1.0`.

* `v1_value_still_zero`
  * Evaluate on an empty board.
  * Assert `res.value == 0.0`.

### 2.7 Gate

Run `cargo test -p baseline`. The v0 tests from sub-chapter 01 should
still pass. The v1 tests should pass after the implementation is
correct.

---

## Step 3 — `cli/src/session.rs`: pass the flex device

### 3.1 Update `make_evaluator`

Find the `EvaluatorKind::Baseline` arm in
`cli::session::SessionCore::make_evaluator`. Replace the placeholder
with a call that passes the flex device:

```rust
Box::new(baseline::BaselineEvaluator::new(baseline::Device::flex()))
```

Behavior: the CLI builds the baseline evaluator on the CPU flex device
without importing Burn.

### 3.2 Gate

Run `cargo test -p cli`. The existing CLI tests should still pass.

---

## Step 4 — Project gates

From `gomoku/`:

```text
cargo test -p baseline
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

---

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `baseline/src/readout.rs` | `policy_logits` | legal cells get gravity value, others get `NEG_INFINITY` |
| `baseline/src/readout.rs` | `logits_are_uniform_when_gravity_is_zero` | zero gravity ⇒ uniform `0.0` logits |
| `baseline/src/readout.rs` | `logits_mask_illegal_cells` | occupied cells stay masked |
| `baseline/src/lib.rs` | `pub mod readout;` | module is public |
| `baseline/src/lib.rs` | `pub use burn::tensor::Device;` | cli can use `baseline::Device` without Burn dep |
| `baseline/src/lib.rs` | `BaselineEvaluator` fields | holds `device` and `kernel` |
| `baseline/src/lib.rs` | `BaselineEvaluator::new` | builds kernel on given device |
| `baseline/src/lib.rs` | `evaluate` | gravity map → logits → value `0.0` |
| `baseline/src/lib.rs` | `v1_highest_logit_hugs_stones` | max logit is `2.0 / 32.0` after centre stone |
| `baseline/src/lib.rs` | `v1_empty_board_is_uniform` | empty board has uniform logits |
| `baseline/src/lib.rs` | `v1_corner_stone_not_boosted` | corner sum is strictly less than `1.0` |
| `baseline/src/lib.rs` | `v1_value_still_zero` | value stays `0.0` |
| `cli/src/session.rs` | `make_evaluator` Baseline arm | passes `baseline::Device::flex()` |

## Done when

* All checklist items are satisfied.
* The six new tests pass.
* The project gates pass.
* The capstone in `gravity-takes-the-wheel.md` shows clustering.
* The worktree commit is:
  `feat(baseline): gravity takes the wheel — baseline sub-chapter 03`
