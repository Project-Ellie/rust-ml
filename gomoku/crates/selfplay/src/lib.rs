//! Synchronous, single-threaded baseline self-play.
//!
//! One seeded game runs a complete Swap2 ceremony and then alternates two
//! persistent baseline players. The result is entirely in memory: opening
//! provenance, per-seat options, ordered plies, search metadata, and outcome.

#![deny(missing_docs)]

pub mod game;
pub mod opening;
