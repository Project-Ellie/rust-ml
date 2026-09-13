//! rust-ml: a curriculum for deep learning in Rust with the Burn framework.
//!
//! The library crate holds the shared MNIST code (data pipeline, model,
//! training, inference). The curriculum itself lives in two places:
//!
//! - `docs/` — the wiki. Start at `docs/README.md`.
//! - `examples/` — one runnable example per chapter, numbered to match the
//!   docs. Run them with `cargo run --example 01_tensors` etc.

pub mod data;
pub mod inference;
pub mod model;
pub mod training;
