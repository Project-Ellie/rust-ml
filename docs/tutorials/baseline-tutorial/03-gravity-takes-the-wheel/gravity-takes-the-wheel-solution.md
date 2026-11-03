# Sub-chapter 03 — Gravity takes the wheel: solution

> **Opt-in solution — read only after you have tried the chapter, or
> when you have been stuck for more than twenty minutes.**

Verified green from `gomoku/` at commit `0fb4326`:
`cargo test -p baseline`, `cargo test -p cli`,
`cargo clippy --all-targets -- -D warnings`,
`cargo fmt --all -- --check`.

The slice changes three source files:
`baseline/src/readout.rs` (new), `baseline/src/lib.rs` (v1 evaluator),
and `cli/src/session.rs` (flex device wiring).

---

## `baseline/src/readout.rs` — the policy readout

`gomoku/crates/baseline/src/readout.rs`, lines 1–18:

```rust
//! Policy readout: turn maps into policy logits.

use engine::Move;

/// Build a 225-vector of policy logits from a gravity map.
///
/// Legal cells receive their normalized gravity value; occupied or
/// illegal cells receive `f32::NEG_INFINITY`.
#[must_use]
pub fn policy_logits(gravity: &[[f32; 15]; 15], legal: &[Move]) -> [f32; 225] {
    let mut logits = [f32::NEG_INFINITY; 225];
    for mv in legal {
        let r = mv.row() as usize;
        let c = mv.col() as usize;
        logits[mv.index()] = gravity[r][c];
    }
    logits
}
```

This function is the bridge from map to policy. It starts with a fully
masked vector because every cell is either occupied or unknown. The
loop unmasks only the legal moves by writing the gravity value at the
right row and column. Occupied cells never leave the mask, so
`masked_softmax` later ignores them.

---

## `baseline/src/readout.rs` — readout unit tests

`gomoku/crates/baseline/src/readout.rs`, lines 20–44:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Board, Move};

    #[test]
    fn logits_are_uniform_when_gravity_is_zero() {
        let gravity = [[0.0f32; 15]; 15];
        let board = Board::new();
        let legal: Vec<Move> = board.empty_moves().collect();
        let logits = policy_logits(&gravity, &legal);
        for mv in &legal {
            assert_eq!(logits[mv.index()], 0.0f32);
        }
    }

    #[test]
    fn logits_mask_illegal_cells() {
        let gravity = [[0.0f32; 15]; 15];
        let mut board = Board::new();
        board.play(Move::new(7, 7).unwrap()).unwrap();
        let legal: Vec<Move> = board.empty_moves().collect();
        let logits = policy_logits(&gravity, &legal);
        assert_eq!(logits[Move::new(7, 7).unwrap().index()], f32::NEG_INFINITY);
    }
}
```

The first test proves that a zero gravity map produces a uniform logit
surface over legal cells. The second test proves that the mask survives
when an occupied cell is present. Both tests use exact `==` comparisons
because every value is a dyadic fraction or a sentinel.

---

## `baseline/src/lib.rs` — the v1 evaluator

`gomoku/crates/baseline/src/lib.rs`, lines 7–45:

```rust
#![deny(missing_docs)]

pub mod gravity;
pub mod readout;

pub use burn::tensor::Device;

use gravity::GravityKernel;
use mcts::{EvalRequest, EvalResult, Evaluator};

/// The baseline evaluator.
///
/// v1 builds policy priors from the gravity kernel: it plays near
/// existing stones while leaving the value flat at `0.0`.
pub struct BaselineEvaluator {
    device: Device,
    kernel: GravityKernel,
}

impl BaselineEvaluator {
    /// Create a new [`BaselineEvaluator`] on `device`.
    #[must_use]
    pub fn new(device: Device) -> Self {
        let kernel = GravityKernel::new(&device);
        Self { device, kernel }
    }
}

impl Evaluator for BaselineEvaluator {
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
        let _ = &self.device;
        let gravity = self.kernel.compute(&req.planes);
        let policy = readout::policy_logits(&gravity, &req.legal);
        EvalResult {
            policy,
            value: 0.0f32,
        }
    }
}
```

The crate root now declares `pub mod readout` and re-exports
`burn::tensor::Device`. The re-export matters: `cli` builds the
evaluator with `baseline::Device::flex()` and keeps its own
`Cargo.toml` free of Burn.

`BaselineEvaluator` holds a `device` field and a `GravityKernel`. The
constructor builds the kernel on the supplied device. `evaluate` runs
the kernel, calls the readout, and returns a flat value of `0.0`. The
`let _ = &self.device;` line silences the unused-field warning until a
later chapter uses the device for the value network.

Note: the module-level doc comment on lines 1–5 of this file still says
"Burn-free (v0) policy" and "planes are ignored." That comment is stale
at this commit; the per-struct doc on lines 17–20 correctly describes
v1.

---

## `baseline/src/lib.rs` — v1 integration tests

`gomoku/crates/baseline/src/lib.rs`, lines 108–162:

```rust
    #[test]
    fn v1_highest_logit_hugs_stones() {
        let mut board = Board::new();
        board.play(Move::new(7, 7).unwrap()).unwrap();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);

        let max_logit = board
            .empty_moves()
            .map(|mv| res.policy[mv.index()])
            .fold(f32::NEG_INFINITY, f32::max);
        assert_eq!(max_logit, 2.0f32 / 32.0f32);

        // Cells far from the stone (distance > 2) remain at zero.
        assert_eq!(res.policy[Move::new(0, 0).unwrap().index()], 0.0f32);
    }

    #[test]
    fn v1_empty_board_is_uniform() {
        let board = Board::new();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);
        for mv in board.empty_moves() {
            assert_eq!(res.policy[mv.index()], 0.0f32);
        }
    }

    #[test]
    fn v1_corner_stone_not_boosted() {
        let mut board = Board::new();
        board.play(Move::new(0, 0).unwrap()).unwrap();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);

        let max_logit = board
            .empty_moves()
            .map(|mv| res.policy[mv.index()])
            .fold(f32::NEG_INFINITY, f32::max);
        assert_eq!(max_logit, 2.0f32 / 32.0f32);

        let corner_sum: f32 = board.empty_moves().map(|mv| res.policy[mv.index()]).sum();
        assert!(corner_sum < 1.0f32);
    }

    #[test]
    fn v1_value_still_zero() {
        let board = Board::new();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);
        assert_eq!(res.value, 0.0f32);
    }
```

These tests exercise the full `evaluate` path. The centre-stone test
checks that the highest legal logit is the Chebyshev-distance-1 weight
of `2/32`. The empty-board test checks uniformity. The corner-stone
test is the regression net from sub-chapter 02 carried through the
evaluator: the maximum is still `2/32`, and the total boosted mass is
strictly less than `1.0`. The value test keeps the flat-value contract.

The build contract also named a `v1_is_deterministic` test, but the
shipped commit does not add it. Determinism is still covered by the
`v0_is_deterministic` test from sub-chapter 01.

---

## `cli/src/session.rs` — seat wiring

`gomoku/crates/cli/src/session.rs`, lines 767–778:

```rust
    fn make_evaluator(kind: &EvaluatorKind) -> Box<dyn mcts::Evaluator> {
        match kind {
            // Currently, there are two.
            EvaluatorKind::Tactics => Box::new(mcts::TacticsEvaluator::new()),
            #[cfg(feature = "baseline")]
            EvaluatorKind::Baseline => {
                Box::new(baseline::BaselineEvaluator::new(baseline::Device::flex()))
            }
            #[cfg(not(feature = "baseline"))]
            EvaluatorKind::Baseline => unreachable!("baseline feature is disabled"),
        }
    }
```

This is the only `cli` change. The Baseline arm uses the re-exported
`baseline::Device::flex()` to build the evaluator. The `cli` crate does
not import `burn` anywhere, so the crate-dependency boundary from the
primer stays clean.

---

## What changed in the data flow

```text
EvalRequest { planes, legal }
        │
        ▼  GravityKernel::compute
gravity map g(r,c) ∈ [0, 1]
        │
        ▼  readout::policy_logits
[ f32::NEG_INFINITY everywhere; g(r,c) at legal cells ]
        │
        ▼  mcts::expand → masked_softmax
visit distribution concentrated near stones
        │
        ▼  PUCT search
4,000 simulations hug the fight
```

The value stays `0.0`. The search still decides the move; gravity only
tells it where to look.
