//! Gomoku rules engine — milestone 1 of the AlphaZero-style agent.
//!
//! Design: `docs/13-engine-design.md` in the rust-ml repository.
//! Pure Rust: no Burn, no I/O, no GPU. Runs on every self-play worker
//! thread simultaneously, inside the hottest loop in the system.
//!
//! Public surface is deliberately small (visibility as enforcement).
//! Rules: freestyle Gomoku on 15×15 — overlines count as a win, draw at
//! 225 moves, Swap2 opening protocol supported.

mod bitboard;
mod board;
mod encode;
mod moveset;
mod opening;
mod symmetry;
mod tactics;
mod win;
mod zobrist;

#[cfg(any(test, feature = "testutil"))]
pub mod reference;
