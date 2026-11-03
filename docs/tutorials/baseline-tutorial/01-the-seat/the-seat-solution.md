# Sub-chapter 01 — The seat: solution

This file is opt-in. Read it only if you have been stuck for more than
twenty minutes, or after you have finished the sub-chapter and want to
compare your work.

Verified green from `gomoku/`: `cargo test -p baseline`, `cargo test -p cli`,
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.
Quotes commit `bd63e74` on `baseline-reference`.

---

The slice changes five files. `gomoku/Cargo.toml` and
`gomoku/crates/baseline/Cargo.toml` are small mechanical changes.
`gomoku/crates/baseline/src/lib.rs` is new. The three `cli` files carry
tiny additions.

## `gomoku/Cargo.toml` — workspace member

The workspace must know the new crate before any package can depend on
it. The member is appended to the existing list.

`gomoku/Cargo.toml`, lines 5–7:

```toml
[workspace]
resolver = "3"
members = ["crates/engine", "crates/cli", "crates/mcts", "crates/train", "crates/patterns",
    "crates/testkit", "crates/baseline"]
```

## `gomoku/crates/baseline/Cargo.toml` — the crate manifest

The `baseline` crate depends only on `engine` and `mcts`. It stays
Burn-free at this stage.

`gomoku/crates/baseline/Cargo.toml`, lines 1–10:

```toml
[package]
name = "baseline"
description = "Hand-written policy/value evaluator for the Gomoku baseline player."
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
engine = { path = "../engine" }
mcts = { path = "../mcts" }
```

## `gomoku/crates/baseline/src/lib.rs` — the v0 evaluator and tests

This file is the whole slice for the evaluator. It declares the public
`BaselineEvaluator`, implements the `mcts::Evaluator` seam, and tests the
v0 contract. The module doc and `#![deny(missing_docs)]` enforce the
tutorial's documentation discipline.

`gomoku/crates/baseline/src/lib.rs`, lines 1–99:

```rust
//! Hand-written policy/value evaluator for the Gomoku baseline player.
//!
//! This crate implements the [`mcts::Evaluator`] seam with a
//! deterministic, Burn-free (v0) policy and a flat value. The board
//! planes are ignored in this sub-chapter.

#![deny(missing_docs)]

use mcts::{EvalRequest, EvalResult, Evaluator};

/// The baseline evaluator.
///
/// v0 returns a uniform policy over legal moves and a value of `0.0`.
/// The board planes are ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BaselineEvaluator;

impl BaselineEvaluator {
    /// Create a new [`BaselineEvaluator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Evaluator for BaselineEvaluator {
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
        let mut policy = [f32::NEG_INFINITY; 225];
        for mv in &req.legal {
            policy[mv.index()] = 0.0f32;
        }
        EvalResult {
            policy,
            value: 0.0f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Board, Move};

    fn request_from_board(board: &Board) -> EvalRequest {
        EvalRequest {
            planes: engine::encode(board),
            legal: board.empty_moves().collect(),
        }
    }

    #[test]
    fn v0_policy_is_uniform_over_legal() {
        let board = Board::new();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new();
        let res = ev.evaluate(req);
        for mv in board.empty_moves() {
            assert_eq!(res.policy[mv.index()], 0.0f32);
        }
    }

    #[test]
    fn v0_occupied_cells_are_masked() {
        let mut board = Board::new();
        board.play(Move::new(7, 7).unwrap()).unwrap();
        board.play(Move::new(0, 0).unwrap()).unwrap();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new();
        let res = ev.evaluate(req);
        assert_eq!(
            res.policy[Move::new(7, 7).unwrap().index()],
            f32::NEG_INFINITY
        );
        assert_eq!(
            res.policy[Move::new(0, 0).unwrap().index()],
            f32::NEG_INFINITY
        );
    }

    #[test]
    fn v0_value_is_zero() {
        let board = Board::new();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new();
        let res = ev.evaluate(req);
        assert_eq!(res.value, 0.0f32);
    }

    #[test]
    fn v0_is_deterministic() {
        let board = Board::new();
        let req = request_from_board(&board);
        let mut ev1 = BaselineEvaluator::new();
        let mut ev2 = BaselineEvaluator::new();
        let res1 = ev1.evaluate(req.clone());
        let res2 = ev2.evaluate(req);
        assert_eq!(res1, res2);
    }
}
```

## `gomoku/crates/cli/Cargo.toml` — feature wiring

The CLI gains an optional dependency on `baseline`, a feature that turns
it on, and a default feature set that includes it. The rest of the
manifest is unchanged.

`gomoku/crates/cli/Cargo.toml`, lines 20–28:

```toml
[features]
default = ["naive-engine", "baseline"]
naive-engine = ["engine/testutil"]
baseline = ["dep:baseline"]

[dependencies]
engine = { path = "../engine" }
mcts = { path = "../mcts" }
baseline = { path = "../baseline", optional = true }
```

## `gomoku/crates/cli/src/actor.rs` — `EvaluatorKind::Baseline`

The internal backend enum gains a second variant. The serde derive stays,
so saved sessions can still round-trip.

`gomoku/crates/cli/src/actor.rs`, lines 162–169:

```rust
/// Evaluator backend available to an AI seat.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum EvaluatorKind {
    /// The hand-woven tactics evaluator (Burn-free baseline).
    Tactics,
    /// The hand-written baseline evaluator.
    Baseline,
}
```

## `gomoku/crates/cli/src/main.rs` — `AiKind::Baseline`

The CLI enum and its `From` impl give `--ai baseline` a path into the
internal `EvaluatorKind`. The `ValueEnum` rename keeps the flag value
lowercase.

`gomoku/crates/cli/src/main.rs`, lines 109–125:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
enum AiKind {
    /// The hand-woven tactics evaluator.
    Tactics,
    /// The hand-written baseline evaluator.
    Baseline,
}

impl From<AiKind> for EvaluatorKind {
    fn from(arg: AiKind) -> Self {
        match arg {
            AiKind::Tactics => EvaluatorKind::Tactics,
            AiKind::Baseline => EvaluatorKind::Baseline,
        }
    }
}
```

The default evaluator remains `Tactics` until the acceptance sub-chapter
flips it. `ai_config_from` keeps the current default explicit.

`gomoku/crates/cli/src/main.rs`, lines 307–316:

```rust
fn ai_config_from(search: &SearchArgs) -> AiConfig {
    AiConfig {
        evaluator: search.ai.map(Into::into).unwrap_or(EvaluatorKind::Tactics),
        simulations: search.sims,
        c_puct: search.c_puct,
        temperature: search.temperature.unwrap_or(0.0),
        noise_epsilon: search.noise.unwrap_or(0.0),
        explicit_selection: search.temperature.is_some() || search.noise.is_some(),
    }
}
```

## `gomoku/crates/cli/src/session.rs` — `make_evaluator`

The evaluator factory adds a feature-gated arm for `Baseline`. When the
`baseline` feature is off, the arm compiles to `unreachable!` so the
match stays exhaustive.

`gomoku/crates/cli/src/session.rs`, lines 767–776:

```rust
    fn make_evaluator(kind: &EvaluatorKind) -> Box<dyn mcts::Evaluator> {
        match kind {
            // Currently, there are two.
            EvaluatorKind::Tactics => Box::new(mcts::TacticsEvaluator::new()),
            #[cfg(feature = "baseline")]
            EvaluatorKind::Baseline => Box::new(baseline::BaselineEvaluator::new()),
            #[cfg(not(feature = "baseline"))]
            EvaluatorKind::Baseline => unreachable!("baseline feature is disabled"),
        }
    }
```

## `gomoku/crates/cli/tests/args.rs` — flag tests

The two integration tests pin the new `--ai` value at the clap level.

`gomoku/crates/cli/tests/args.rs`, lines 43–63:

```rust
#[test]
fn play_ai_baseline_parses() {
    let output = run_gomoku(&["play", "--ai", "baseline", "--no-swap2"]);
    assert!(
        output.status.success(),
        "baseline AI kind should parse: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn play_ai_rejects_invalid_value() {
    let output = run_gomoku(&["play", "--ai", "nonsense", "--no-swap2"]);
    assert!(!output.status.success(), "expected failure");
    assert_eq!(
        output.status.code(),
        Some(2),
        "exit code should be 2, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
```
