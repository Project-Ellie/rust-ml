//! Hand-written convolutional pattern network for Gomoku threat detection.
//!
//! The `patterns` crate builds a tiny two-layer convnet whose weights are
//! manufactured by hand from a declarative pattern language. It produces
//! exact maps for immediate wins, double threats, double-three forks and
//! per-direction open-three makers, and it proves them equal to independent
//! oracles at 1000-board scale.

#![deny(missing_docs)]

pub mod naive;
pub mod planes;

pub mod kernel;
pub mod tools;

pub mod net;
pub mod oracle;

pub mod potential;
pub mod render;
