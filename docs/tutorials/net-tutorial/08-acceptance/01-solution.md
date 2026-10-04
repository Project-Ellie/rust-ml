# Chapter 08 — Deep-dive solution: crate polish

This is the opt-in reference for Chapter 08. It matches the verified reference worktree exactly. The acceptance protocol itself is documented in the chapter; the only code change at this slice is the `#![deny(missing_docs)]` attribute in `src/lib.rs`.

## `src/lib.rs`

```rust
//! Neural network training for the Gomoku AlphaZero-style agent.
//!
//! This crate implements the policy/value network and the manual
//! supervised-training loop for phase 0. It consumes labelled samples
//! from the `train` crate and derives input planes from engine boards.

#![deny(missing_docs)]

pub mod batcher;
pub mod checkpoint;
pub mod loss;
pub mod network;
pub mod planes;
pub mod train;

```
