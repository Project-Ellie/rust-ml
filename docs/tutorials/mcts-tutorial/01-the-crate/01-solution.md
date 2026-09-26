# Chapter 01 — Deep-dive solution: crate skeleton

This is the opt-in reference for Chapter 01. It matches the verified
reference crate exactly.

## `Cargo.toml`

```toml
[package]
name = "mcts"
version = "0.1.0"
edition = "2024"

[dependencies]
engine = { path = "/Users/wgiersche/workspace/Project-Ellie/rust-ml/gomoku/crates/engine" }
rand = "0.9"
rand_distr = "0.5"

[dev-dependencies]
engine = { path = "/Users/wgiersche/workspace/Project-Ellie/rust-ml/gomoku/crates/engine", features = ["testutil"] }
rand = "0.9"
```

## `src/lib.rs`

```rust
//! Reference implementation of single-threaded PUCT MCTS for Gomoku.
//!
//! This crate is milestone 2 of the rust-ml AlphaZero-style Gomoku
//! agent. It depends only on the `engine` crate (rules oracle) and on
//! `rand`/`rand_distr` for root Dirichlet noise and temperature
//! sampling. No Burn, no GPU, no threads: the search is deliberately
//! single-threaded so it can be tested and reasoned about in
//! isolation.
//!
//! Design document (locked): `docs/tutorials/mcts-tutorial/00-mcts-primer.md`
//! in the rust-ml repository. The code follows the notation and sign
//! convention of primer §§3–6 exactly.
//!
//! Public modules:
//!
//! * [`tree`] — arena tree with edge statistics.
//! * [`select`] — PUCT descent from root to leaf.
//! * [`eval`] — `Evaluator` trait, `masked_softmax`, and test stubs.
//! * [`expand`] — terminal check + network expansion of a leaf.
//! * [`backup`] — sign-flipped value propagation up the path.
//! * [`search`] — the top-level `search(root_board, evaluator, config)`.
//! * [`policy`] — visit-count extraction, temperature sampling,
//!   Dirichlet noise at the root.
//! * [`mock`] — deterministic `TacticsEvaluator` for the tactical suite.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
// PUCT and probability arithmetic intentionally mix integer counts
// with f32/f64. Visit counts stay far below f32 precision limits in
// all realistic searches, and f64->f32 truncation is explicit storage
// narrowing, not a logic bug.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

pub mod backup;
pub mod eval;
pub mod expand;
pub mod mock;
pub mod policy;
pub mod search;
pub mod select;
pub mod tree;

pub use backup::backup;
pub use eval::{EvalRequest, EvalResult, Evaluator, UniformEvaluator, masked_softmax};
pub use expand::expand;
pub use mock::TacticsEvaluator;
pub use policy::{add_dirichlet_noise, select_move, visit_distribution};
pub use search::{SearchConfig, SearchOutcome, search};
pub use select::{Selection, puct_score, select};
pub use tree::{Edge, Node, NodeId, NodeState, Tree};
```

In Chapter 01 the other module files are empty placeholders. The
`tree_module_exists` test from the chapter body is sufficient to prove
the crate skeleton compiles.
