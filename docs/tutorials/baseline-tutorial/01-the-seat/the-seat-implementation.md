# Sub-chapter 01 — The seat: implementation plan

This document turns the walkthrough in `the-seat.md` into a construction
order. It names the files, structs, enums, functions, and tests. It
states behavioral contracts in prose. It does not contain bodies.

Read `the-seat.md` first. Work one step at a time. Write tests before
code. Open `the-seat-solution.md` only if you are stuck for more than
twenty minutes, or after the sub-chapter is complete.

---

## Step 0 — `gomoku/Cargo.toml`: add the workspace member

In the `workspace.members` array, add `"crates/baseline"`. Keep the
existing members in their current order.

---

## Step 1 — `gomoku/crates/baseline/Cargo.toml`: the crate manifest

Create the file.

### 1.1 Package metadata

Use `name = "baseline"`. Pull `version`, `edition`, and `license` from
the workspace.

### 1.2 Dependencies

Add `engine` and `mcts` as path dependencies. No other dependencies in
this sub-chapter.

---

## Step 2 — `gomoku/crates/baseline/src/lib.rs`: the v0 evaluator

Create the file.

### 2.1 Module documentation

Write a module-level doc comment that names the crate's purpose and
states that the board planes are ignored in v0.

### 2.2 Deny missing docs

Add `#![deny(missing_docs)]` so every public item needs a doc comment.

### 2.3 Imports

Bring in `mcts::{EvalRequest, EvalResult, Evaluator}`.

### 2.4 `BaselineEvaluator`

Write the public unit struct:

```rust
pub struct BaselineEvaluator;
```

Derive `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, and `Default`.

Behavior: a stateless evaluator placeholder.

Tests that go red then green: `v0_is_deterministic`.

### 2.5 `BaselineEvaluator::new`

Signature:

```rust
pub fn new() -> Self
```

Behavior: return the unit value.

### 2.6 `impl Evaluator for BaselineEvaluator`

Implement the `evaluate` method.

Signature:

```rust
fn evaluate(&mut self, req: EvalRequest) -> EvalResult
```

Behavior:

* Allocate a `[f32; 225]` initialized to `f32::NEG_INFINITY`.
* For each `Move` in `req.legal`, set the corresponding index to `0.0`.
* Return `EvalResult { policy, value: 0.0 }`.
* Ignore `req.planes`.

Tests that go red then green: `v0_policy_is_uniform_over_legal`,
`v0_occupied_cells_are_masked`, `v0_value_is_zero`.

### 2.7 Test helper

In `#[cfg(test)] mod tests`, write a helper:

```rust
fn request_from_board(board: &Board) -> EvalRequest
```

Behavior: build an `EvalRequest` whose `planes` come from
`engine::encode(board)` and whose `legal` vector contains
`board.empty_moves()`.

### 2.8 Tests

In the same test module, write these tests:

* `v0_policy_is_uniform_over_legal` — create an empty board, build a
request, evaluate, and assert that every empty cell has logit `0.0`.
* `v0_occupied_cells_are_masked` — play two moves, build a request,
evaluate, and assert that the played indices carry
`f32::NEG_INFINITY`.
* `v0_value_is_zero` — evaluate an empty board and assert that the
value equals `0.0`.
* `v0_is_deterministic` — evaluate the same request with two fresh
evaluators and assert the results are equal.

### 2.9 Gate

Run `cargo test -p baseline`. Expect failures until the implementation
is written.

---

## Step 3 — `gomoku/crates/cli/src/actor.rs`: add `EvaluatorKind::Baseline`

In the `EvaluatorKind` enum, add a `Baseline` variant with a doc
comment. Keep the existing `Tactics` variant and the serde derive.

Tests that go red then green: none directly; compilation will fail until
the session factory is updated.

---

## Step 4 — `gomoku/crates/cli/src/main.rs`: add `AiKind::Baseline`

### 4.1 Extend the CLI enum

In the `AiKind` enum, add a `Baseline` variant with a doc comment. Keep
the clap `ValueEnum` derive and the lowercase rename.

### 4.2 Extend the `From<AiKind>` impl

Map `AiKind::Baseline` to `EvaluatorKind::Baseline`. Keep the existing
`Tactics` arm.

Tests that go red then green: `play_ai_baseline_parses`,
`play_ai_rejects_invalid_value`.

---

## Step 5 — `gomoku/crates/cli/src/session.rs`: wire `make_evaluator`

### 5.1 Add the baseline arm

In the private `make_evaluator` function, add a match arm for
`EvaluatorKind::Baseline`.

Behavior:

* Under `#[cfg(feature = "baseline")]`, return a boxed
`baseline::BaselineEvaluator::new()`.
* Under `#[cfg(not(feature = "baseline"))]`, return `unreachable!` with
a message that the feature is disabled.

Tests that go red then green: `play_ai_baseline_parses`.

---

## Step 6 — `gomoku/crates/cli/Cargo.toml`: feature wiring

### 6.1 Optional dependency

Add `baseline = { path = "../baseline", optional = true }` under
`[dependencies]`.

### 6.2 Feature

Add `baseline = ["dep:baseline"]` under `[features]`.

### 6.3 Default features

Change `default` to include both `"naive-engine"` and `"baseline"`.

Keep the rest of the manifest unchanged.

---

## Step 7 — `gomoku/crates/cli/tests/args.rs`: flag tests

### 7.1 Add `play_ai_baseline_parses`

Run the `gomoku` binary with `play --ai baseline --no-swap2`. Assert that
it exits successfully.

### 7.2 Add `play_ai_rejects_invalid_value`

Run the `gomoku` binary with `play --ai nonsense --no-swap2`. Assert that
it fails and exits with code `2`.

---

## Step 8 — Project gates

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
| `gomoku/Cargo.toml` | `crates/baseline` in `workspace.members` | the workspace knows the new crate |
| `baseline/Cargo.toml` | `name = "baseline"` | crate identity |
| `baseline/Cargo.toml` | `engine` and `mcts` path deps | no Burn, no I/O, no rand |
| `baseline/src/lib.rs` | module doc + `#![deny(missing_docs)]` | every public item is documented |
| `baseline/src/lib.rs` | `BaselineEvaluator` | public unit struct |
| `baseline/src/lib.rs` | `BaselineEvaluator::new` | constructor returns the unit |
| `baseline/src/lib.rs` | `impl Evaluator` | uniform policy, zero value, planes ignored |
| `baseline/src/lib.rs` | `request_from_board` helper | builds `EvalRequest` from a `Board` |
| `baseline/src/lib.rs` | four unit tests | all behavioral contracts asserted |
| `cli/src/actor.rs` | `EvaluatorKind::Baseline` | internal backend enum extended |
| `cli/src/main.rs` | `AiKind::Baseline` | CLI enum extended |
| `cli/src/main.rs` | `From<AiKind>` impl | maps `Baseline` to `Baseline` |
| `cli/src/session.rs` | `make_evaluator` Baseline arm | feature-gated factory wiring |
| `cli/Cargo.toml` | optional `baseline` dep | dependency is conditional |
| `cli/Cargo.toml` | `baseline` feature | feature controls the dep |
| `cli/Cargo.toml` | default features | baseline is on by default |
| `cli/tests/args.rs` | `play_ai_baseline_parses` | flag value accepted |
| `cli/tests/args.rs` | `play_ai_rejects_invalid_value` | invalid flag value rejected |
