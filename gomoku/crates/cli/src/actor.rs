//! Seat occupants and AI configuration.
//!
//! The study table has two seats (Black and White). Each seat is occupied
//! by either a human or an AI. For Task 2 both seats are human; the AI
//! plumbing lands in Task 3, but the configuration shapes are defined now
//! so the `SessionCore` API stays stable.

use engine::{Board, Color, Move};
use serde::{Deserialize, Serialize};

/// Record of one AI search: visit distribution, root value, and prior policy.
///
/// `value` is the estimated value of the root position **from the
/// side-to-move's perspective at search time**. The sign is derived from
/// the MCTS backup convention in `mcts::backup`: the leaf value (from the
/// leaf player's perspective) is propagated up the selection path with the
/// sign flipped at every edge. Each edge therefore accumulates a value `W`
/// from the perspective of the player who chose at its parent node. For a
/// root edge, that parent is the root, so `W` is already from the root
/// player's (i.e. side-to-move's) point of view. The root value estimate is
/// the visit-weighted average of those root-edge values:
/// `sum(W) / sum(N)`. A positive value favours the side to move.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchInfo {
    /// Side-to-move
    pub side_to_move: Color,
    /// Visit counts for each legal move at the root, sorted by count descending
    pub visits: Vec<(Move, u32)>,
    /// Root edge prior probabilities after softmax and optional Dirichlet noice,
    /// sorted by probability, descending
    pub prior: Vec<(Move, f32)>,
    /// Estimated root value from the side-to-move's perspective
    pub value: f32,
    /// AI configuration used for the search
    pub config: AiConfig,
}

impl SearchInfo {
    /// Build a `SearchInfo` from an MCTS search outcome.
    ///
    /// `board` is the position that was searched; it is retained in the
    /// signature so callers document the side-to-move perspective that
    /// determines the sign of [`Self::value`].
    #[must_use]
    pub fn from_search(outcome: &mcts::SearchOutcome, board: &Board, config: &AiConfig) -> Self {
        let edges = outcome.tree.node(outcome.root).edges();
        let mut visits: Vec<(Move, u32)> = edges.iter().map(|e| (e.mv, e.n)).collect();

        visits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.index().cmp(&b.0.index())));

        let mut prior: Vec<(Move, f32)> = edges.iter().map(|e| (e.mv, e.prior)).collect();
        prior.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.index().cmp(&b.0.index()))
        });

        let total_visits: u32 = edges.iter().map(|e| e.n).sum();
        let total_value: f32 = edges.iter().map(|e| e.w).sum();
        let value = if total_visits == 0 {
            0.0
        } else {
            total_value / total_visits as f32
        };

        Self {
            side_to_move: board.to_move(),
            visits,
            prior,
            value,
            config: config.clone(),
        }
    }
}

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

    /// Returns `true` if this seat is occupied by a human.
    #[must_use]
    pub fn is_human(&self) -> bool {
        matches!(self, Occupant::Human)
    }

    /// Returns `true` if this seat is occupied by an AI.
    #[must_use]
    pub fn is_ai(&self) -> bool {
        matches!(self, Occupant::Ai(_))
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
    /// Whether `temperature` and `noise_epsilon` were set explicitly by the
    /// user (e.g. via CLI flags). When `false` and both values are at their
    /// defaults (0.0), an AI-vs-AI line from an empty root may use the
    /// opening schedule instead.
    #[serde(default)]
    pub explicit_selection: bool,
}

impl Default for AiConfig {
    /// Default AlphaZero-style search settings.
    ///
    /// - 400 simulations
    /// - c_puct 1.5
    /// - deterministic move selection (temperature 0.0)
    /// - no Dirichlet noise
    /// - selection parameters are not explicit, so the opening schedule may
    ///   apply in AI-vs-AI from an empty root
    fn default() -> Self {
        Self {
            evaluator: EvaluatorKind::Tactics,
            simulations: 400,
            c_puct: 1.5,
            temperature: 0.0,
            noise_epsilon: 0.0,
            explicit_selection: false,
        }
    }
}

/// Evaluator backend available to an AI seat.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum EvaluatorKind {
    /// The hand-woven tactics evaluator (Burn-free baseline).
    Tactics,
}
