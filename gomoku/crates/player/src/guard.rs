//! Bounded defensive refutation experiment (`choose`) and its data types.
//!
//! After ordinary move selection, an enabled guard probes the opponent on
//! cloned boards. A missing or unverified proof means unknown, never safe;
//! the chosen move and the search statistics stay separate concerns.

use engine::{Board, Color, Move, Proof, SearchBudget, Status};
use mcts::Edge;

/// Bounded defensive refutation experiment, not a wall-clock deadline.
///
/// Disabled by default. A missing proof means unknown, never safety. The
/// candidate cap bounds hypothetical checks; each probe has its own node/depth
/// budget. Proof verification is also required and is not a timed operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuardConfig {
    /// Whether to inspect ordinary search candidates after selection.
    pub enabled: bool,
    /// Maximum candidate checks, including the original proposal. Zero skips all.
    pub max_candidates: usize,
    /// Per-candidate opponent prover work caps, not a wall-clock deadline.
    pub budget: SearchBudget,
}

impl GuardConfig {
    /// Preserve ordinary move selection without checking any candidate.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::baseline()
        }
    }

    /// [experiment] Baseline defaults: five candidates, 1,000 nodes / 10 plies
    /// per opponent probe. These are work bounds, not a response-time guarantee.
    #[must_use]
    pub const fn baseline() -> Self {
        Self {
            enabled: true,
            max_candidates: 5,
            budget: SearchBudget {
                max_nodes: 1_000,
                max_depth: 10,
            },
        }
    }
}

impl Default for GuardConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

/// What the defensive experiment established (or deliberately left unknown).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardOutcome {
    /// Guard disabled; the original candidate is unchecked.
    Disabled,
    /// Certified own forced win bypassed both ordinary selection and this guard.
    ProvenWin,
    /// No verified opponent win found within the probe budget: unknown, not safe.
    Unrefuted,
    /// The checked candidate immediately won for us or drew the game.
    Terminal,
    /// Candidate cap reached; selected move is untested, not certified safe.
    BudgetExhausted,
    /// Every legal candidate was individually refuted; original move retained.
    /// This is not a proof that the root position is lost.
    AllRefuted,
}

/// Decision-local diagnostics; these never alter MCTS visits or priors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardDiagnostics {
    /// Original ordinary selection (or the certified own winning move).
    pub proposal: Move,
    /// Candidates refuted by verified opponent proofs, in check order.
    pub refuted: Vec<Move>,
    /// Hypothetical candidates checked, including immediate wins/draws.
    pub checked: usize,
    /// Result of the defensive experiment, not a position-value judgement.
    pub outcome: GuardOutcome,
}

impl GuardDiagnostics {
    pub(crate) fn new(proposal: Move, outcome: GuardOutcome) -> Self {
        Self {
            proposal,
            refuted: Vec::new(),
            checked: 0,
            outcome,
        }
    }
}

// Private injection seam for bounded bookkeeping tests, never an evaluator hook.
// Expanded MCTS roots contain every legal edge, including unvisited edges.
pub(crate) fn choose(
    board: &Board,
    proposal: Move,
    edges: &[Edge],
    config: GuardConfig,
    mut probe: impl FnMut(&Board, Color, SearchBudget) -> Option<Proof>,
) -> (Move, GuardDiagnostics) {
    let mut diagnostics = GuardDiagnostics::new(proposal, GuardOutcome::Disabled);
    if !config.enabled {
        return (proposal, diagnostics);
    }
    diagnostics.outcome = GuardOutcome::BudgetExhausted;
    if config.max_candidates == 0 {
        return (proposal, diagnostics);
    }
    let mut ranked: Vec<_> = edges.iter().collect();
    ranked.sort_by(|a, b| {
        b.n.cmp(&a.n)
            .then_with(|| {
                b.prior
                    .partial_cmp(&a.prior)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.mv.index().cmp(&b.mv.index()))
    });
    let candidates = std::iter::once(proposal).chain(
        ranked
            .into_iter()
            .map(|edge| edge.mv)
            .filter(|&mv| mv != proposal),
    );
    for candidate in candidates {
        if diagnostics.refuted.contains(&candidate) {
            continue;
        }
        // All previously checked candidates were refuted. The next ranked
        // candidate is untested, so the cap must not send us back to a loser.
        if diagnostics.checked == config.max_candidates {
            return (candidate, diagnostics);
        }
        let mut after = board.clone();
        after
            .play(candidate)
            .expect("root candidates must be legal");
        diagnostics.checked += 1;
        if matches!(after.status(), Status::Won(side) if side == board.to_move())
            || after.status() == Status::Draw
        {
            diagnostics.outcome = GuardOutcome::Terminal;
            return (candidate, diagnostics);
        }
        let opponent = after.to_move();
        let refuted = probe(&after, opponent, config.budget).is_some_and(|proof| {
            proof.winner == opponent
                && proof.line.first().is_some_and(|&mv| after.is_legal(mv))
                && engine::verify_line(&after, &proof)
        });
        if !refuted {
            diagnostics.outcome = GuardOutcome::Unrefuted;
            return (candidate, diagnostics);
        }
        diagnostics.refuted.push(candidate);
    }
    assert_eq!(
        diagnostics.refuted.len(),
        board.empty_moves().count(),
        "expanded root must include every legal candidate"
    );
    diagnostics.outcome = GuardOutcome::AllRefuted;
    (proposal, diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv(row: u8, col: u8) -> Move {
        Move::new(row, col).unwrap()
    }

    fn two_ended_four() -> Board {
        Board::from_position(
            &[mv(2, 0), mv(2, 2), mv(2, 4), mv(2, 6)],
            &[mv(7, 3), mv(7, 4), mv(7, 5), mv(7, 6)],
            Color::Black,
        )
        .unwrap()
    }

    fn edges(board: &Board) -> Vec<Edge> {
        board
            .empty_moves()
            .map(|mv| Edge {
                mv,
                prior: 0.0,
                n: 0,
                w: 0.0,
                child: None,
            })
            .collect()
    }

    #[test]
    fn alternatives_rank_visits_then_prior_then_index_without_repeats_and_include_unvisited() {
        let board = two_ended_four();
        let original = board.clone();
        let proposal = mv(0, 4);
        let mut edges = edges(&board);
        for (candidate, n, prior) in [
            (proposal, 100, 1.0),
            (mv(0, 3), 4, 0.1),
            (mv(0, 2), 3, 0.9),
            (mv(0, 1), 3, 0.9),
            (mv(0, 0), 3, 0.5),
            (mv(0, 5), 0, 0.9),
        ] {
            let edge = edges.iter_mut().find(|e| e.mv == candidate).unwrap();
            edge.n = n;
            edge.prior = prior;
        }
        edges.reverse(); // Creation order must not leak into tie-breaking.
        let before = edges.clone();
        let mut seen = Vec::new();
        let config = GuardConfig {
            budget: SearchBudget {
                max_nodes: 1,
                max_depth: 1,
            },
            ..GuardConfig::baseline()
        };
        let (chosen, diagnostics) =
            choose(&board, proposal, &edges, config, |after, side, budget| {
                seen.push(*after.moves().last().unwrap());
                assert_eq!(side, after.to_move());
                assert_eq!(side, Color::White);
                assert_eq!(budget, config.budget);
                engine::prove_forced_win(after, side, budget)
            });
        assert_eq!(seen, [proposal, mv(0, 3), mv(0, 1), mv(0, 2), mv(0, 0)]);
        assert_eq!(diagnostics.refuted, seen);
        assert_eq!(diagnostics.checked, 5);
        assert_eq!(diagnostics.outcome, GuardOutcome::BudgetExhausted);
        assert_eq!(chosen, mv(0, 5)); // Unvisited, untested, not a known loser.
        assert_eq!(edges, before);
        assert_eq!(board, original);
    }

    #[test]
    fn malformed_or_unverified_proofs_leave_candidate_unknown() {
        let board = Board::from_position(&[mv(7, 3)], &[mv(7, 4)], Color::Black).unwrap();
        let proposal = mv(0, 0);
        let edges = edges(&board);
        for proof in [
            Proof {
                winner: Color::Black,
                line: vec![mv(7, 7)],
            },
            Proof {
                winner: Color::White,
                line: vec![],
            },
            Proof {
                winner: Color::White,
                line: vec![mv(7, 3)],
            },
            Proof {
                winner: Color::White,
                line: vec![proposal],
            },
            Proof {
                winner: Color::White,
                line: vec![mv(1, 0)],
            },
        ] {
            let mut calls = 0;
            let (chosen, diagnostics) = choose(
                &board,
                proposal,
                &edges,
                GuardConfig::baseline(),
                |_, _, _| {
                    calls += 1;
                    Some(proof.clone())
                },
            );
            assert_eq!(chosen, proposal);
            assert_eq!(diagnostics.outcome, GuardOutcome::Unrefuted);
            assert_eq!(diagnostics.checked, 1);
            assert!(diagnostics.refuted.is_empty());
            assert_eq!(calls, 1);
        }
    }

    #[test]
    fn disabled_and_zero_cap_never_call_probe() {
        let board = two_ended_four();
        let proposal = mv(0, 0);
        let edges = edges(&board);
        for (config, outcome) in [
            (GuardConfig::disabled(), GuardOutcome::Disabled),
            (
                GuardConfig {
                    max_candidates: 0,
                    ..GuardConfig::baseline()
                },
                GuardOutcome::BudgetExhausted,
            ),
        ] {
            let (chosen, diagnostics) = choose(&board, proposal, &edges, config, |_, _, _| {
                panic!("no probe allowed")
            });
            assert_eq!(chosen, proposal);
            assert_eq!(diagnostics.outcome, outcome);
            assert_eq!(diagnostics.checked, 0);
            assert!(diagnostics.refuted.is_empty());
        }
    }

    #[test]
    fn immediate_win_never_calls_opponent_probe() {
        let board = Board::from_position(
            &[mv(0, 1), mv(0, 2), mv(0, 3), mv(0, 4)],
            &[mv(2, 0), mv(2, 2), mv(2, 4), mv(2, 6)],
            Color::Black,
        )
        .unwrap();
        let proposal = mv(0, 0);
        let (chosen, diagnostics) = choose(
            &board,
            proposal,
            &edges(&board),
            GuardConfig::baseline(),
            |_, _, _| panic!("terminal candidate must not be probed"),
        );
        assert_eq!(chosen, proposal);
        assert_eq!(diagnostics.outcome, GuardOutcome::Terminal);
        assert_eq!(diagnostics.checked, 1);
    }

    #[test]
    fn immediate_draw_never_calls_opponent_probe() {
        let proposal = mv(0, 0);
        let mut black = Vec::new();
        let mut white = Vec::new();
        for r in 0..15 {
            for c in 0..15 {
                let stone = mv(r, c);
                if stone == proposal {
                    continue;
                }
                // Repeating pairs avoid five on rows, columns and diagonals.
                if (r + 2 * c) % 4 < 2 {
                    black.push(stone);
                } else {
                    white.push(stone);
                }
            }
        }
        // Replay stones so the engine also has the full pre-draw move history.
        let mut board = Board::new();
        for (black, white) in black.into_iter().zip(white) {
            board.play(black).unwrap();
            board.play(white).unwrap();
        }
        assert_eq!(board.status(), Status::Ongoing);
        let original = board.clone();
        let (chosen, diagnostics) = choose(
            &board,
            proposal,
            &edges(&board),
            GuardConfig::baseline(),
            |_, _, _| panic!("draw must not be probed"),
        );
        assert_eq!(chosen, proposal);
        assert_eq!(diagnostics.outcome, GuardOutcome::Terminal);
        assert_eq!(diagnostics.checked, 1);
        assert!(diagnostics.refuted.is_empty());
        assert_eq!(board, original);
        let mut after = board.clone();
        after.play(chosen).unwrap();
        assert_eq!(after.status(), Status::Draw);
    }
}
