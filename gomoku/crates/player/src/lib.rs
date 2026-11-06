//! Reusable AI move decisions, independent of any user interface.
//!
//! An [`AiPlayer`] retains its evaluator across decisions. Each decision
//! runs a fresh MCTS search; certified own forced wins bypass sampling
//! and carry no fabricated search statistics.

#![deny(missing_docs)]

use engine::{Board, SearchBudget, Status};
use mcts::{Evaluator, SearchConfig};
use rand::Rng;

mod guard;
pub use guard::{GuardConfig, GuardDiagnostics, GuardOutcome};

/// Own-side bounded TSS probe settings for one decision.
///
/// When enabled, [`AiPlayer::decide`] asks the engine for a certified forced
/// win for the side to move before searching. The budget bounds prover work;
/// exhausting it means unknown, never loss, and ordinary search runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnTssConfig {
    /// Whether to run the own-side proof probe before search.
    pub enabled: bool,
    /// Prover work caps, not a wall-clock deadline.
    pub budget: SearchBudget,
}

impl OwnTssConfig {
    /// Baseline preset: enabled, 10,000 nodes / 30 plies (the retired
    /// chapter-08 `ROOT_TSS_BUDGET` provenance, relocated to the player).
    #[must_use]
    pub const fn baseline() -> Self {
        Self {
            enabled: true,
            budget: SearchBudget {
                max_nodes: 10_000,
                max_depth: 30,
            },
        }
    }

    /// Disabled preset keeping the baseline budget bounds.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::baseline()
        }
    }
}

impl Default for OwnTssConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Search and move-selection settings for one decision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerOptions {
    /// Number of MCTS simulations.
    pub simulations: u32,
    /// PUCT exploration constant.
    pub c_puct: f32,
    /// Visit-count sampling temperature; zero selects the most-visited move.
    pub temperature: f32,
    /// Dirichlet noise mix fraction; zero disables noise and its RNG draws.
    pub noise_epsilon: f32,
    /// Positive Dirichlet concentration parameter when noise is enabled.
    pub noise_alpha: f32,
    /// Own-side TSS probe before search; disabled by default.
    pub own: OwnTssConfig,
    /// Optional [experiment] defensive refutation guard; disabled by default.
    pub guard: GuardConfig,
}

impl Default for PlayerOptions {
    fn default() -> Self {
        let search = SearchConfig::default();
        Self {
            simulations: search.simulations,
            c_puct: search.c_puct,
            temperature: 0.0,
            noise_epsilon: 0.0,
            noise_alpha: 0.3,
            own: OwnTssConfig::disabled(),
            guard: GuardConfig::disabled(),
        }
    }
}

/// A chosen move and, when a search ran, its real statistics.
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    /// Move to play.
    pub mv: engine::Move,
    /// Search statistics, or `None` when a certified own win bypassed search.
    pub search: Option<SearchStats>,
    /// Defensive experiment diagnostics, independent of search statistics.
    pub guard: GuardDiagnostics,
}

/// Statistics extracted from a real searched tree, never from a proof.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchStats {
    /// Player whose perspective determines the root value.
    pub side_to_move: engine::Color,
    /// All root moves and visit counts, descending by count, then move index.
    pub visits: Vec<(engine::Move, u32)>,
    /// All root priors after optional noise, descending by prior, then move index.
    pub prior: Vec<(engine::Move, f32)>,
    /// Root value from the side-to-move perspective: `sum(W) / sum(N)`.
    /// Zero when no simulations have visited an edge.
    pub value: f32,
}

impl SearchStats {
    fn from_searched(tree: &mcts::Tree, root: mcts::NodeId, board: &Board) -> Self {
        let edges = tree.node(root).edges();
        let mut visits = mcts::visit_distribution(tree, root);
        visits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.index().cmp(&b.0.index())));
        let mut prior: Vec<_> = edges.iter().map(|edge| (edge.mv, edge.prior)).collect();
        prior.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.index().cmp(&b.0.index()))
        });
        let total_visits: u32 = edges.iter().map(|edge| edge.n).sum();
        let total_value: f32 = edges.iter().map(|edge| edge.w).sum();
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
        }
    }
}

/// Failure to decide a move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionError {
    /// The board is already won or drawn.
    GameOver,
}

impl std::fmt::Display for DecisionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GameOver => formatter.write_str("the game is over"),
        }
    }
}

impl std::error::Error for DecisionError {}

/// A reusable player owning one persistent evaluator, with a fresh tree per move.
pub struct AiPlayer {
    evaluator: Box<dyn Evaluator>,
}

/// Admit a claimed own forced win, or reject it as unknown.
///
/// The player is the certifier: the proof must name the side to move, start
/// with a legal move on this board, and survive the engine's line verifier.
/// Anything less is unknown, and ordinary search runs instead.
fn certified_own_move(board: &Board, proof: &engine::Proof) -> Option<engine::Move> {
    if proof.winner != board.to_move() {
        return None;
    }
    let first = *proof.line.first()?;
    if !board.is_legal(first) {
        return None;
    }
    engine::verify_line(board, proof).then_some(first)
}

impl AiPlayer {
    /// Own an evaluator and reuse it for every subsequent decision.
    #[must_use]
    pub fn new(evaluator: Box<dyn Evaluator>) -> Self {
        Self { evaluator }
    }

    /// Decide a move without modifying `board`.
    ///
    /// A certified own forced win returns immediately without consuming RNG,
    /// calling the evaluator, or constructing search statistics. Otherwise
    /// the order is search, optional root noise, then temperature selection
    /// over the root edges (zero temperature breaks visit ties by prior). Noise is deliberately applied after search to
    /// preserve the existing CLI behavior. An enabled guard then checks the
    /// selected proposal and ranked alternatives on cloned boards; it may
    /// substitute the returned move, but the search statistics always describe
    /// the real proposal search, never the guard's hypothetical probes. An
    /// unrefuted candidate is unknown, not certified safe.
    ///
    /// # Errors
    /// Returns [`DecisionError::GameOver`] for a won or drawn board, without
    /// calling the evaluator or consuming RNG.
    ///
    /// # Panics
    /// Panics if Gamma rejects enabled noise's alpha, or if MCTS returns an
    /// empty move distribution for an ongoing board.
    pub fn decide(
        &mut self,
        board: &Board,
        options: &PlayerOptions,
        rng: &mut impl Rng,
    ) -> Result<Decision, DecisionError> {
        if board.status() != Status::Ongoing {
            return Err(DecisionError::GameOver);
        }
        if options.own.enabled {
            // One direct engine probe per decision, never cached: AiPlayer
            // holds no state besides the evaluator.
            if let Some(mv) = engine::prove_forced_win(board, board.to_move(), options.own.budget)
                .and_then(|proof| certified_own_move(board, &proof))
            {
                return Ok(Decision {
                    mv,
                    search: None,
                    guard: GuardDiagnostics::new(mv, GuardOutcome::ProvenWin),
                });
            }
        }
        let config = SearchConfig {
            simulations: options.simulations,
            c_puct: options.c_puct,
        };
        let mcts::SearchOutcome { mut tree, root } =
            mcts::search(board, &mut *self.evaluator, &config);
        if options.noise_epsilon > 0.0 {
            mcts::add_dirichlet_noise(
                &mut tree,
                root,
                options.noise_epsilon,
                options.noise_alpha,
                rng,
            );
        }
        let proposal =
            mcts::select_move_with_priors(tree.node(root).edges(), options.temperature, rng)
                .expect("AI must select a move from a non-empty distribution");
        let (mv, guard) = guard::choose(
            board,
            proposal,
            tree.node(root).edges(),
            options.guard,
            engine::prove_forced_win,
        );
        Ok(Decision {
            mv,
            search: Some(SearchStats::from_searched(&tree, root, board)),
            guard,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Color, Move, Proof};

    /// The win-in-3 VCF fixture from the engine prover tests: Black to move
    /// forces a win starting with (7, 7).
    fn vcf_fixture() -> Board {
        let mut board = Board::new();
        for (row, col) in [
            (7, 4),
            (7, 3),
            (7, 5),
            (0, 0),
            (7, 6),
            (0, 2),
            (6, 6),
            (0, 4),
            (8, 8),
            (0, 6),
        ] {
            board.play(Move::new(row, col).unwrap()).unwrap();
        }
        board
    }

    #[test]
    fn admission_rejects_wrong_winner_empty_line_illegal_first_move_and_unverified_line() {
        let board = vcf_fixture();
        let real =
            engine::prove_forced_win(&board, board.to_move(), OwnTssConfig::baseline().budget)
                .expect("the engine proves this fixture within the baseline budget");

        // A genuine, verified proof admits its first move.
        assert_eq!(
            certified_own_move(&board, &real),
            Some(Move::new(7, 7).unwrap())
        );

        // Wrong winner: the opponent cannot own our certificate.
        let wrong_winner = Proof {
            winner: Color::White,
            line: real.line.clone(),
        };
        assert_eq!(certified_own_move(&board, &wrong_winner), None);

        // Empty line: no move to play, nothing to admit.
        let empty = Proof {
            winner: Color::Black,
            line: Vec::new(),
        };
        assert_eq!(certified_own_move(&board, &empty), None);

        // Illegal first move: (0, 0) is already occupied in the fixture.
        let illegal_first = Proof {
            winner: Color::Black,
            line: vec![Move::new(0, 0).unwrap()],
        };
        assert_eq!(certified_own_move(&board, &illegal_first), None);

        // Unverified lines: the real proof, truncated or appended to.
        let mut truncated = real.clone();
        truncated.line.pop();
        assert_eq!(certified_own_move(&board, &truncated), None);
        let mut appended = real.clone();
        appended.line.push(Move::new(14, 14).unwrap());
        assert_eq!(certified_own_move(&board, &appended), None);
    }
}
