# Chapter 08 — Solution (opt-in)

Try the polish yourself first: add the attribute, fix what the
compiler points at, run the ritual. Twenty minutes of honest struggle
before you open this file — that is the deal.

This chapter's only code artifact is the final `src/lib.rs` — the
crate root with `#![deny(missing_docs)]` locking the public surface.
The acceptance ritual itself is commands, not code; chapter 8's
"Measured numbers" table quotes the reference implementation's actual
runs.

The file below is the complete `src/lib.rs` from the verified
reference crate, quoted verbatim (extract-and-diff verified).

```rust
//! Hand-written convolution kernels for Gomoku threat maps.
//!
//! This is a side-quest tutorial crate for the rust-ml Gomoku project. It
//! implements a small feed-forward convolutional network whose weights are
//! manufactured by hand, not trained, and whose outputs are spatial *threat
//! maps*: scores for every empty cell according to what happens if the side
//! plays there.
//!
//! The crate is backend-generic (`B: Backend`) from the first line. The
//! default backend is `NdArray`; an opt-in `gpu` feature enables the `Wgpu`
//! backend for the same code.
//!
//! See `SPIKE.md` for the verified Burn 0.21 idioms used to build the
//! network.

#![deny(missing_docs)]

pub mod kernels;

pub mod naive;

pub mod net;

pub mod oracle;

pub mod planes;

pub mod potential;

pub mod render;
```
