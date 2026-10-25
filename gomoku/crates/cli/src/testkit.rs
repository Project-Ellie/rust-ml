//! The shared test kit for the `cli` unit tests.
//!
//! Tests live in two worlds that cannot see each other:
//!
//! - Unit tests (`#[cfg(test)] mod tests` inside `src/`) share helpers
//!   through this module.
//! - Integration tests (the binaries under `tests/`) share helpers
//!   through `tests/common/mod.rs`.
//!
//! Rust's test architecture compiles the two worlds separately: a unit
//! test cannot import from `tests/`, and an integration-test binary
//! cannot reach a `#[cfg(test)]` module inside the crate. A helper that
//! both worlds need therefore exists once per world — `mv` here and in
//! `tests/common/mod.rs` — with the same name, body, and comment.

use engine::Move;

/// Build a move from zero-based row and column.
///
/// Test scripts are full of coordinates; the `unwrap` is intentional in
/// tests: a test that constructs an invalid move should fail loudly.
pub(crate) fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}
