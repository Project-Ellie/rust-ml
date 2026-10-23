//! Seat occupants and AI configuration.
//!
//! The study table has two seats (Black and White). Each seat is occupied
//! by either a human or an AI. For Task 2 both seats are human; the AI
//! plumbing lands in Task 3, but the configuration shapes are defined now
//! so the `SessionCore` API stays stable.

use serde::{Deserialize, Serialize};

/// Who is responsible for producing a seat's moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OccupantKind {
    /// A human player typing commands.
    Human,
    /// An AI player using MCTS search.
    Ai,
}

/// A concrete seat occupant, including AI configuration when applicable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Occupant {
    /// A human player.
    Human,
    /// An AI player with its search configuration.
    ///
    /// `Ai` arrives in Task 3; the variant is defined now for
    /// forward-compatibility.
    Ai(AiConfig),
}

impl Occupant {
    /// The kind of occupant, without configuration details.
    #[must_use]
    pub fn kind(&self) -> OccupantKind {
        match self {
            Occupant::Human => OccupantKind::Human,
            Occupant::Ai(_) => OccupantKind::Ai,
        }
    }
}

/// Search configuration for an AI seat.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AiConfig {
    /// Which evaluator drives MCTS.
    pub evaluator: EvaluatorKind,
    /// Number of MCTS simulations per search.
    pub simulations: u32,
    /// PUCT exploration constant.
    pub c_puct: f32,
    /// Temperature applied when selecting a move from the visit distribution.
    pub temperature: f32,
    /// Dirichlet noise mix fraction (0.0 disables noise).
    pub noise_epsilon: f32,
}

impl Default for AiConfig {
    /// Default AlphaZero-style search settings.
    ///
    /// - 400 simulations
    /// - c_puct 1.5
    /// - deterministic move selection (temperature 0.0)
    /// - no Dirichlet noise
    fn default() -> Self {
        Self {
            evaluator: EvaluatorKind::Tactics,
            simulations: 400,
            c_puct: 1.5,
            temperature: 0.0,
            noise_epsilon: 0.0,
        }
    }
}

/// Evaluator backend available to an AI seat.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum EvaluatorKind {
    /// The hand-woven tactics evaluator (Burn-free baseline).
    Tactics,
}
