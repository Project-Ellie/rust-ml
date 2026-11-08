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
