//! The exploration-schedule vocabulary recorded in every corpus header.
//!
//! Vocabulary only: the type, its fields, its serde derives, and its
//! `Default` exist so the header can record the corpus's identity. Per-ply
//! schedule behavior is a later chapter's subject.

use player::PlayerOptions;
use serde::{Deserialize, Serialize};

/// Number of post-opening plies that use temperature and root noise under
/// the default schedule. Mirrors the CLI's first-ten-ply AI-vs-AI practice.
pub const EXPLORATION_PLIES: usize = 10;

/// Serializable exploration schedule parameters
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct ExplorationSchedule {
    /// the exploration window's length
    pub plies: usize,

    /// Move-selection temperature during the exploration window.
    pub temperature: f32,

    /// Dirichlet noise mix fraction at the search root.
    pub noise_epsilon: f32,

    /// Dirichlet noise concentration parameter.
    pub noise_alpha: f32,
}

impl ExplorationSchedule {
    /// Apply this schedule to one post-opening ply.
    ///
    /// Inside the exploration window (`ply < plies`) the seat uses the
    /// schedule's temperature, noise epsilon, and noise alpha. Outside the
    /// window temperature and noise epsilon are set to zero, making move
    /// selection deterministic. The simulation budget, PUCT constant,
    /// own-side TSS probe, guard, and resignation settings are preserved
    /// exactly.
    #[must_use]
    pub fn options_for_ply(&self, base: PlayerOptions, ply: usize) -> PlayerOptions {
        if ply < self.plies {
            PlayerOptions {
                temperature: self.temperature,
                noise_epsilon: self.noise_epsilon,
                noise_alpha: self.noise_alpha,
                ..base
            }
        } else {
            PlayerOptions {
                temperature: 0.0,
                noise_epsilon: 0.0,
                ..base
            }
        }
    }
}

impl Default for ExplorationSchedule {
    /// Setings from Silver et al. 2017
    fn default() -> Self {
        Self {
            plies: EXPLORATION_PLIES,
            temperature: 1.0,
            noise_alpha: 0.3,
            noise_epsilon: 0.25,
        }
    }
}
