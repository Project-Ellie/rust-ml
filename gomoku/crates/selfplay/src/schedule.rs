//! The exploration-schedule vocabulary recorded in every corpus header.
//!
//! Vocabulary only: the type, its fields, its serde derives, and its
//! `Default` exist so the header can record the corpus's identity. Per-ply
//! schedule behavior is a later chapter's subject.

use serde::{Deserialize, Serialize};

/// Number of post-opening plies that use temperature and root noise under
/// the default schedule. Mirrors the CLI's first-ten-ply AI-vs-AI practice.
pub const EXPLORATION_PLIES: usize = 10;

/// Serializable exploration schedule parameters
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct ExplocationSchedule {
    /// the exploration window's length
    pub plies: usize,

    /// Move-selection temperature during the exploration window.
    pub temperature: f32,

    /// Dirichlet noise mix fraction at the search root.
    pub noise_eplilon: f32,

    /// Dirichlet noise concentration parameter.
    pub noise_alpha: f32,
}

impl Default for ExplocationSchedule {
    /// Setings from Silver et al. 2017
    fn default() -> Self {
        Self {
            plies: EXPLORATION_PLIES,
            temperature: 1.0,
            noise_alpha: 0.3,
            noise_eplilon: 0.25,
        }
    }
}
