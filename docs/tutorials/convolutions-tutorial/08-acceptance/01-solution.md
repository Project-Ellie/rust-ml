# Chapter 08 — Solution (opt-in)

Try the polish yourself first: add the attribute, fix what the
compiler points at, run the ritual. Twenty minutes of honest struggle
before you open this file — that is the deal.

This chapter's only code artifact is the final `src/lib.rs` — the
crate root with `#![deny(missing_docs)]` locking the public surface.
The acceptance ritual itself is commands, not code; chapter 8's
"Measured numbers" table quotes the verified reference build's actual
runs on an Apple-silicon Mac.

The file below is the complete `src/lib.rs` from the verified
reference crate, quoted verbatim (extract-and-diff verified). It
registers the final module set: `naive`, `planes`, `kernel`, `tools`,
`net`, `oracle`, `potential`, and `render`.

```rust
//! Hand-written convolution kernels for Gomoku threat maps.
//!
//! This is a side-quest tutorial crate for the rust-ml Gomoku project. It
//! implements a small feed-forward convolutional network whose weights are
//! manufactured by hand, not trained, and whose outputs are spatial *threat
//! maps*: scores for every empty cell according to what happens if the side
//! plays there.
//!
//! The crate has no backend type parameter. The default device is
//! `Device::flex()`; an opt-in `gpu` feature enables the `wgpu` device for
//! the same code.
//!
//! See `gomoku/crates/patterns/tests/spike.rs` for the verified Burn 0.22
//! idioms used to build the network.

#![deny(missing_docs)]

/// Learner's hand-rolled cross-correlation on plain Rust arrays.
pub mod naive;

/// A representation of a gomoku board ready for pattern processing.
pub mod planes;

/// Declarative pattern table and hand-manufactured convolution kernels.
pub mod kernel;

/// Tools for developing gomoku heuristics and building convolution layers.
pub mod tools;

/// The hand-written threat network and the win map.
pub mod net;

/// Naive reference oracle for open-three threat detection.
pub mod oracle;

/// Threat potential: turn the threat maps into a sparse policy vector.
pub mod potential;

/// Heatmap rendering for the `patterndemo` binary.
pub mod render;
```
