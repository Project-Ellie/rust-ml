//! Reference implementation of single-threaded PUCT MCTS for Gomoku.
//!
//! This crate depends only on the `engine` crate (rules oracle) and on
//! `rand`/`rand_distr` for root Dirichlet noise and temperature
//! sampling. No Burn, no GPU, no threads: the search is deliberately
//! single-threaded so it can be tested and reasoned about in
//! isolation.
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

pub mod eval;
pub mod select;
pub mod tree;
