# Sub-chapter 08 — The TSS eye: implementation plan

This document turns the walkthrough in `the-tss-eye.md` into a
construction order. It names the files, structs, constants, functions,
and tests. It states behavioral contracts in prose. It does not
contain bodies.

Read `the-tss-eye.md` first. Work one step at a time. Write tests
before code. Open `the-tss-eye-solution.md` only if you are stuck for
more than twenty minutes, or after the sub-chapter is complete.

---

## Step 0 — Create `baseline/src/probe.rs`

Create the file `gomoku/crates/baseline/src/probe.rs`.

### 0.1 Module documentation

Write a module-level doc comment that explains:

* The probe is a once-per-move short-circuit in the CLI seat path.
* `Some` means a certified forced win.
* `None` only means "not proven within budget" and says nothing about
  the true value.

### 0.2 Imports

Bring in:

* `engine::{Board, Proof, SearchBudget, prove_forced_win}`

### 0.3 `ROOT_BUDGET`

Write the public constant:

```rust
pub const ROOT_BUDGET: SearchBudget
```

Behavior:

* `max_nodes` is `10_000`.
* `max_depth` is `30`.
* The doc comment marks the values as `[experiment]` starting values.

This constant is referenced by the CLI and the tests.

### 0.4 `root_probe`

Write the public function:

```rust
#[must_use]
pub fn root_probe(board: &Board, budget: SearchBudget) -> Option<Proof>
```

Behavior:

* Call `engine::prove_forced_win(board, board.to_move(), budget)`.
* Return the `Option<Proof>` unchanged.
* The doc comment carries the "None says nothing" warning from the
  engine.

Tests that go red then green:

* `probe_finds_vcf_line`
* `probe_quiet_position_is_none`
* `probe_respects_budget`

### 0.5 Tests module

Add a `#[cfg(test)] mod tests` block.

Imports for tests:

* `super::*`
* `engine::{Board, Color, Move, verify_line}`

Test 1 — `probe_finds_vcf_line`:

Behavior:

* Build a board by replaying the engine TSS win-in-3 script (the same
  script used in the engine TSS tests).
* Assert the side to move is Black.
* Call `root_probe(&board, ROOT_BUDGET)`.
* Assert the result is `Some`.
* Assert `proof.winner` is `Color::Black`.
* Assert `proof.line` is non-empty.
* Assert `verify_line(&board, &proof)` returns `true`.

Test 2 — `probe_quiet_position_is_none`:

Behavior:

* Create a fresh `Board::new()`.
* Call `root_probe(&board, ROOT_BUDGET)`.
* Assert the result is `None`.

Test 3 — `probe_respects_budget`:

Behavior:

* Build the same win-in-3 board as test 1.
* Call `root_probe` with a budget of `max_nodes: 1, max_depth: 30` and
  assert `None`.
* Call `root_probe` with a budget of `max_nodes: 10_000, max_depth: 2`
  and assert `None`.

### 0.6 Gate

Run `cargo test -p baseline`. Expect failures until step 1 is done.

---

## Step 1 — Expose the module in `baseline/src/lib.rs`

### 1.1 Add the module declaration

In `gomoku/crates/baseline/src/lib.rs`, add `pub mod probe;` among the
other module declarations (`adapter`, `gravity`, `readout`).

### 1.2 Gate

Run `cargo test -p baseline`. The probe tests should now compile and
pass.

---

## Step 2 — Wire the probe into the CLI step path

### 2.1 Locate the AI-seat step path

Open `gomoku/crates/cli/src/session.rs`. Find `execute_step`. The
function already reads the board, checks game state, gets the side to
move, and resolves the AI config before building the evaluator.

### 2.2 Add the probe short-circuit

After the AI config is resolved and before the evaluator is built, add
a feature-gated block:

Behavior:

* Check `#[cfg(feature = "baseline")]`.
* Check that `config.evaluator == EvaluatorKind::Baseline`.
* Call `baseline::probe::root_probe(&board, baseline::probe::ROOT_BUDGET)`.
* If the result is `Some(proof)`, extract `proof.line[0]` and return
  `self.execute_play(mv)`.
* If the result is `None`, continue to the normal MCTS step.

Idiom: use `if let Some(proof) = ...` to keep the fall-through obvious.

### 2.3 Gate

Run `cargo test -p cli --test acceptance`. The new CLI integration test
is added in step 3; until then the existing acceptance tests must still
pass.

---

## Step 3 — Add the CLI integration test

Open `gomoku/crates/cli/tests/acceptance.rs`.

### 3.1 Add `baseline_root_probe_plays_proof_first_move`

The test must be feature-gated with `#[cfg(feature = "baseline")]`.

Behavior:

* Build the same win-in-3 opening board as the engine TSS test by
  replaying the script.
* Assert the side to move is Black.
* Call `engine::prove_forced_win` directly with the same budget as
  `ROOT_BUDGET` to obtain the expected first move. Store
  `proof.line[0]`.
* Build a `SessionInit` with:
  * Black seat as AI with `EvaluatorKind::Baseline`.
  * White seat as Human.
  * The opening board.
  * A seed for reproducibility.
  * Simulations set to a small number such as 200.
* Create a `SessionCore` from the init.
* Execute `Command::Step`.
* Assert that the emitted events contain `Event::Moved` with:
  * `by: OccupantKind::Ai`
  * `mv` equal to the expected first move.

### 3.2 Gate

Run `cargo test -p cli --test acceptance`. Expect this test to fail
until the probe is wired correctly.

---

## Step 4 — Project gates

From `gomoku/`:

```text
cargo test -p baseline
cargo test -p cli --test acceptance
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

---

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `baseline/src/probe.rs` | module docs | explains root-only, once-per-move, meaning of `None` |
| `baseline/src/probe.rs` | `ROOT_BUDGET` | `max_nodes: 10_000`, `max_depth: 30`, marked `[experiment]` |
| `baseline/src/probe.rs` | `root_probe` | wraps `prove_forced_win` for `board.to_move()` |
| `baseline/src/probe.rs` | `probe_finds_vcf_line` | finds proof, checks winner, verifies line |
| `baseline/src/probe.rs` | `probe_quiet_position_is_none` | empty board returns `None` |
| `baseline/src/probe.rs` | `probe_respects_budget` | tiny/shallow budgets return `None` |
| `baseline/src/lib.rs` | `pub mod probe;` | module is public |
| `cli/src/session.rs` | feature-gated block | only `EvaluatorKind::Baseline` triggers probe |
| `cli/src/session.rs` | `Some(proof)` path | plays `proof.line[0]` via `execute_play` |
| `cli/src/session.rs` | `None` path | falls through to normal MCTS step |
| `cli/tests/acceptance.rs` | `baseline_root_probe_plays_proof_first_move` | step emits the proof's first move |
