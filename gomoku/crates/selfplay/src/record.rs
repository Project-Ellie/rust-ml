//! The record vocabulary
//!

use crate::game::{Game, Outcome};
use crate::opening::Opening;
use crate::schedule::ExplorationSchedule;
use engine::{Move, SearchBudget};
use player::{GuardConfig, OwnTssConfig, PlayerOptions};
use serde::{Deserialize, Serialize};

/// Current durable game-record layout version.
pub const GAME_RECORD_FORMAT_VERSION: u16 = 1;

/// Maximum number of root prior entries stored in a durable record.
///
/// This sparse audit width is a selfplay-corpora format invention
/// ([design spec](../../../docs/specs/2026-11-11-selfplay-corpora-design.md) §3.2);
/// k = 16 is an **[experiment]** parameter, not an AlphaZero or architecture-document constant.
/// The complete prior remains recomputable from the position and the deterministic baseline
/// evaluator, so the truncation is safe.
pub const RECORDED_PRIOR_TOP_K: usize = 16;

/// Serializable own-side TSS probe settings for one seeat
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct TssProbeSettings {
    /// whether the probe runs before ordinary search
    pub enabled: bool,
    /// maximum threat space nodes to be explored by one probe
    pub max_nodes: u32,
    /// Maximum proof depth in plies.
    pub max_depth: u8,
}

impl From<OwnTssConfig> for TssProbeSettings {
    fn from(config: OwnTssConfig) -> Self {
        let SearchBudget {
            max_nodes,
            max_depth,
        } = config.budget;
        Self {
            enabled: config.enabled,
            max_nodes,
            max_depth,
        }
    }
}

/// Serializable defensive refutation guard settings for one seat
#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct GuardSettings {
    /// Whether candidate refutation probes are enabled
    pub enabled: bool,
    /// Maximum candates inspected after move selection
    pub max_candidates: usize,
    /// Maximum threat-space nodes explored per candiate.
    pub max_nodes: u32,
    /// Maximum reputation depth in plies
    pub max_depth: u8,
}

impl From<GuardConfig> for GuardSettings {
    fn from(config: GuardConfig) -> Self {
        let SearchBudget {
            max_depth,
            max_nodes,
        } = config.budget;
        Self {
            enabled: config.enabled,
            max_candidates: config.max_candidates,
            max_nodes,
            max_depth,
        }
    }
}

/// Seat configuration
#[derive(Debug, Copy, Clone, PartialEq, Deserialize, Serialize)]
pub struct SeatConfig {
    /// Number of MCTS simulations
    pub simulations: u32,
    /// PUCT exploration constant
    pub c_puct: f32,
    /// sampling temperature
    pub temperature: f32,
    /// Dirichlet noise mix fraction
    pub noise_epsilon: f32,
    /// Dirichtlet noice concentration parameter
    pub noise_alpha: f32,
    /// ownn-side tss probe settings
    pub tss_probe: TssProbeSettings,
    /// defensive refutation guard settings
    pub guard: GuardSettings,
    /// whether heuristic resignation is enabled
    pub resign: bool,
}

impl From<PlayerOptions> for SeatConfig {
    fn from(options: PlayerOptions) -> Self {
        Self {
            simulations: options.simulations,
            c_puct: options.c_puct,
            temperature: options.temperature,
            noise_epsilon: options.noise_epsilon,
            noise_alpha: options.noise_alpha,
            tss_probe: options.own.into(),
            guard: options.guard.into(),
            resign: options.resign,
        }
    }
}

/// The declared Black-vs-White seat pairing.
#[derive(Debug, Copy, Clone, PartialEq, Deserialize, Serialize)]
pub struct SeatPairing {
    /// Base config for black
    pub black: SeatConfig,
    /// Base config for white
    pub white: SeatConfig,
    /// Per-ply selection schedule
    pub exploration: ExplorationSchedule,
}

/// the certified vcf sequence
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CertifiedSuffix {
    /// Zero-based ply from which the terminal segment is certified.
    pub from_ply: u16,
    /// The verified forced-win line.
    pub line: Vec<Move>,
}

/// Configuration and provenance for one recorded game.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct GameHeader {
    /// Durable layout version
    pub format_version: u16,
    /// Seed of the game's complete RNG stream
    pub seed: u64,
    /// Exact per-seat configuration,
    pub config: SeatPairing,
    /// Name of the evaluator that produced both seat's decision
    pub evaluator: String,
    /// Complete Swap2 description
    pub opening: Opening,
}

/// One played move and its durable search targets
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct RecordedPly {
    /// Move played at this ply
    pub mv: Move,
    /// Sparse raw root visits
    pub visits: Vec<(Move, u32)>,
    /// Up to [`RECORDED_PRIOR_TOP_K`] root priors after Dirichlet noise, in descending order;
    /// empty when search did not run.
    pub prior: Vec<(Move, f32)>,
    /// Root value estimate from the recording side-to-move's perspective.
    /// None, when VCF skipped the search.
    pub root_value: Option<f32>,
}

/// Durable, sede-serializable form of one complete self-play game.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct GameRecord {
    /// Configuration and provenance
    pub header: GameHeader,
    /// Playeed post-opening moves, in order
    pub plies: Vec<RecordedPly>,
    /// Terminal winner, or draw
    pub outcome: Outcome,
    /// verified VCF sequence
    pub certified_suffix: Option<CertifiedSuffix>,
}

impl GameRecord {
    /// Convert a finished in-memory game into its durable record.
    #[must_use]
    pub fn from_game(game: &Game) -> Self {
        Self::from(game)
    }
}

impl From<&Game> for GameRecord {
    fn from(game: &Game) -> Self {
        Self {
            header: GameHeader {
                format_version: GAME_RECORD_FORMAT_VERSION,
                seed: game.seed,
                config: SeatPairing {
                    black: game.black_options.into(),
                    white: game.white_options.into(),
                    exploration: ExplorationSchedule::default(),
                },
                evaluator: format!("baseline@{}", env!("CARGO_PKG_VERSION")),
                opening: game.opening.clone(),
            },
            plies: game
                .plies
                .iter()
                .map(|ply| RecordedPly {
                    mv: ply.mv,
                    visits: ply
                        .search
                        .as_ref()
                        .map_or_else(Vec::new, |search| search.visits.clone()),
                    prior: ply
                        .search
                        .as_ref()
                        .map_or_else(Vec::new, |search| search.prior.clone()),
                    root_value: ply.search.as_ref().map(|search| search.root_value),
                })
                .collect(),

            outcome: game.outcome,
            certified_suffix: game.certified_suffix.clone(),
        }
    }
}
