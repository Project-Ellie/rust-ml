# Sub-chapter 01 — The seat

## Abstract

This sub-chapter creates the `baseline` crate and seats it at the study
table. The `BaselineEvaluator` does almost nothing: it returns a uniform
policy over legal moves and a flat value of `0.0`. But it is your
evaluator, behind your `mcts::Evaluator` implementation, selected by the
`--ai` flag. The search, the CLI seat, and the wiring all become real
before the smarts arrive.

## Glossary

| Term | Definition |
|------|------------|
| **`BaselineEvaluator`** | The v0 hand-written evaluator. It is a unit struct that implements the `mcts::Evaluator` seam. |
| **`mcts::Evaluator`** | The single trait that couples the search to any brain. It asks for one `EvalResult` per leaf. |
| **`EvalRequest`** | The search's question: board `planes` plus a vector of `legal` moves. |
| **`EvalResult`** | The evaluator's answer: a `[f32; 225]` policy vector and a scalar `value`. |
| **`TacticsEvaluator`** | The existing mock evaluator from the `mcts` crate. It is the only brain before this chapter. |
| **`EvaluatorKind`** | The CLI's internal enum that names the evaluator backend. |
| **`AiKind`** | The clap `ValueEnum` behind the `--ai` flag. |
| **`baseline` feature** | The optional `cli` dependency that gates the `baseline` crate. It is on by default. |
| **uniform prior** | A policy where every legal move receives the same logit. |
| **masked cell** | An occupied cell that carries `f32::NEG_INFINITY`, so `masked_softmax` gives it zero mass. |

## Context

The `mcts` crate already ships a complete PUCT search. Its only
non-trivial evaluator is the `TacticsEvaluator` mock. The `cli` study
table can seat an AI, but the only brain it can load is that mock. This
chapter adds a second brain path: the `baseline` crate, wired through
the same `Evaluator` seam and selected by the same seat flag.

## Intention

The learner's own evaluator plays legal Gomoku at the study table today.
It does not play well. It only plays legally, through code the learner
owns. That is enough: once the pipeline is real, every later chapter
drops smarter logic into the same socket.

## Specification as tests

The tests are ordered along the execution path: the evaluator returns
correct logits, masks occupied cells, returns a zero value, and is
deterministic; the CLI flag parses the new value and rejects garbage.

### Evaluator behavior

* `v0_policy_is_uniform_over_legal` — on an empty board, every legal
cell receives logit `0.0`.
* `v0_occupied_cells_are_masked` — after two stones are played, those
two indices carry `f32::NEG_INFINITY`.
* `v0_value_is_zero` — the scalar value is exactly `0.0` on an empty
board.
* `v0_is_deterministic` — two fresh evaluators return the same
`EvalResult` for the same `EvalRequest`.

### CLI flag

* `play_ai_baseline_parses` — `gomoku play --ai baseline --no-swap2`
exits successfully.
* `play_ai_rejects_invalid_value` — `gomoku play --ai nonsense
--no-swap2` exits with code `2`.

## Walkthrough

The order is the construction order. Each step makes one small piece of
the pipeline real.

### 1. Add the crate to the workspace

`gomoku/Cargo.toml` adds `crates/baseline` to the workspace `members`
list. This is the only change at the workspace root.

### 2. Create the crate skeleton

`gomoku/crates/baseline/Cargo.toml` depends on `engine` and `mcts` by
path only. `gomoku/crates/baseline/src/lib.rs` carries a module doc
comment and `#![deny(missing_docs)]`. The crate stays Burn-free at this
stage.

### 3. Implement the `Evaluator` trait

`BaselineEvaluator` is a unit struct. Its `evaluate` method initializes
a `[f32; 225]` with `f32::NEG_INFINITY`, then writes `0.0` at every index
in `req.legal`. It returns `EvalResult { policy, value: 0.0 }`. The board
`planes` are ignored; the doc comment says so.

### 4. Add the internal evaluator variant

`gomoku/crates/cli/src/actor.rs` extends `EvaluatorKind` with a
`Baseline` variant. The serde derive is already present, so serialization
keeps working.

### 5. Add the CLI enum variant

`gomoku/crates/cli/src/main.rs` extends `AiKind` with a `Baseline`
variant. The clap `ValueEnum` rename keeps the flag value lowercase.
The `From<AiKind>` impl maps `AiKind::Baseline` to
`EvaluatorKind::Baseline`. The `--ai` flag already seats the AI and
already maps onto `EvaluatorKind`; this change only adds a new value.

### 6. Wire the evaluator factory

`gomoku/crates/cli/src/session.rs::make_evaluator` adds a `Baseline`
arm. The arm is gated on `#[cfg(feature = "baseline")]`. When the
feature is off, the arm is replaced by `unreachable!` so the compiler
still sees an exhaustive match.

### 7. Declare the optional dependency and feature

`gomoku/crates/cli/Cargo.toml` adds `baseline = { path = "../baseline",
optional = true }`, a `baseline = ["dep:baseline"]` feature, and
includes `baseline` in `default`. The default build seats the new brain;
a no-default-features build can still compile without it.

### 8. Write the tests

`baseline/src/lib.rs` holds four unit tests. `cli/tests/args.rs` holds
two flag tests. Run them red, then make them green.

## TDD checklist

Work one artifact at a time, red to green.

From `gomoku/crates/baseline/src/lib.rs`:

1. `v0_policy_is_uniform_over_legal`
2. `v0_occupied_cells_are_masked`
3. `v0_value_is_zero`
4. `v0_is_deterministic`

From `gomoku/crates/cli/tests/args.rs`:

5. `play_ai_baseline_parses`
6. `play_ai_rejects_invalid_value`

## Verification commands

From `gomoku/`:

```text
cargo test -p baseline
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Capstone

Run the baseline brain at the study table:

```text
cargo run -q -p cli --bin gomoku -- play --ai baseline --no-swap2 --sims 4000
```

Press Enter to step. The AI plays legal, clueless moves through your own
code path. Every move has equal prior mass, so the search explores
almost randomly. Try `--color white` to sit in the White seat and watch
the baseline play Black.

## Done when

* The four `baseline` unit tests pass.
* The two `cli` flag tests pass.
* `cargo clippy --all-targets -- -D warnings` passes.
* `cargo fmt --all -- --check` makes no changes.
* The commit message is `feat(baseline): the crate and the seat — baseline sub-chapter 01`.

## References

* [`00-baseline-primer.md`](../00-baseline-primer.md) — the design authority: the MCTS interlock, the trust ladder, and the adapter shape.
* [`../README.md`](../README.md) — the baseline tutorial chapter map.
* [`../../mcts-tutorial/README.md`](../../mcts-tutorial/README.md) — the search and the `Evaluator` seam.
* [`../../playable-cli-tutorial/README.md`](../../playable-cli-tutorial/README.md) — the study table and the four-paper sub-chapter layout.
* `gomoku/crates/mcts/src/eval.rs` — the `Evaluator` trait, `EvalRequest`, and `EvalResult`.
* `gomoku/crates/mcts/src/mock.rs` — the `TacticsEvaluator` the baseline joins and later replaces.
* [`the-seat-design.md`](./the-seat-design.md) — why the socket comes before the smarts.
* [`the-seat-implementation.md`](./the-seat-implementation.md) — the hiding layer.
* [`the-seat-solution.md`](./the-seat-solution.md) — annotated reference snippets.
