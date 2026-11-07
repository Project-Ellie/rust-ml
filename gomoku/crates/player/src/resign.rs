//! Conservative admission for an explicitly enabled heuristic concession.
use crate::{GuardConfig, ResignReason};
use engine::{Board, Color, Proof, SearchBudget, Status};
use mcts::Edge;

pub(crate) fn consider(
    board: &Board,
    edges: &[Edge],
    mut probe: impl FnMut(&Board, Color, SearchBudget) -> Option<Proof>,
) -> Option<ResignReason> {
    if board.status() != Status::Ongoing {
        return None;
    }
    if !engine::immediate_wins(board, board.to_move()).is_empty() {
        return None;
    }
    let legal_count = board.empty_moves().count();
    if legal_count == 0 || edges.len() != legal_count {
        return None;
    }
    let mut seen = [false; 225];
    for edge in edges {
        if !board.is_legal(edge.mv)
            || !edge.prior.is_finite()
            || std::mem::replace(&mut seen[edge.mv.index()], true)
        {
            return None;
        }
    }
    // Legal unique edges with equal cardinality cover every legal move.
    let mut ranked: Vec<_> = edges.iter().collect();
    ranked.sort_by(|a, b| {
        b.n.cmp(&a.n)
            .then_with(|| b.prior.partial_cmp(&a.prior).expect("finite priors"))
            .then_with(|| a.mv.index().cmp(&b.mv.index()))
    });
    let candidates = ranked.len().min(5);
    for edge in ranked.into_iter().take(candidates) {
        let mut after = board.clone();
        after.play(edge.mv).ok()?;
        if after.status() != Status::Ongoing {
            return None;
        }
        let opponent = after.to_move();
        let proof = probe(&after, opponent, GuardConfig::baseline().budget)?;
        if proof.winner != opponent {
            return None;
        }
        let first = *proof.line.first()?;
        if !after.is_legal(first) || !engine::verify_line(&after, &proof) {
            return None;
        }
    }
    Some(ResignReason::VerifiedOpponentVcf { candidates })
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Move;

    fn mv(row: u8, col: u8) -> Move {
        Move::new(row, col).unwrap()
    }

    fn losing_position() -> Board {
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

    fn certificate(after: &Board) -> Proof {
        let first = [mv(7, 2), mv(7, 7)]
            .into_iter()
            .find(|&m| after.is_legal(m))
            .unwrap();
        let proof = Proof {
            winner: Color::White,
            line: vec![first],
        };
        assert!(engine::verify_line(after, &proof));
        proof
    }

    fn almost_draw() -> Board {
        let mut black = Vec::new();
        let mut white = Vec::new();
        for row in 0..15 {
            for col in 0..15 {
                let stone = mv(row, col);
                if stone == mv(0, 0) {
                    continue;
                }
                if (row + 2 * col) % 4 < 2 {
                    black.push(stone);
                } else {
                    white.push(stone);
                }
            }
        }
        Board::from_position(&black, &white, Color::Black).unwrap()
    }

    #[test]
    fn fewer_than_five_checks_every_legal_candidate_with_real_certificates() {
        // Dense repeating pairs, with one open White four and three empty
        // cells. The root contains no own win; either endpoint survives.
        let holes = [mv(0, 7), mv(5, 7), mv(14, 14)];
        let mut black = Vec::new();
        let mut white = Vec::new();
        for row in 0..15 {
            for col in 0..15 {
                let stone = mv(row, col);
                if holes.contains(&stone) {
                    continue;
                }
                if (row + 2 * col) % 4 < 2 && ![mv(2, 7), mv(3, 7)].contains(&stone) {
                    black.push(stone);
                } else {
                    white.push(stone);
                }
            }
        }
        let board = Board::from_position(&black, &white, Color::Black).unwrap();
        assert_eq!(board.status(), Status::Ongoing);
        assert!(engine::immediate_wins(&board, Color::Black).is_empty());
        assert_eq!(board.empty_moves().collect::<Vec<_>>(), holes);
        let mut checked = Vec::new();
        let result = consider(&board, &edges(&board), |after, side, budget| {
            checked.push(*after.moves().last().unwrap());
            engine::prove_forced_win(after, side, budget)
        });
        assert_eq!(checked, holes);
        assert_eq!(
            result,
            Some(ResignReason::VerifiedOpponentVcf { candidates: 3 })
        );
    }

    #[test]
    fn final_empty_cell_draw_cancels_without_opponent_probe() {
        let board = almost_draw();
        assert_eq!(board.status(), Status::Ongoing);
        assert_eq!(board.empty_moves().count(), 1);
        assert!(engine::immediate_wins(&board, Color::Black).is_empty());
        let mut after = board.clone();
        after.play(mv(0, 0)).unwrap();
        assert_eq!(after.status(), Status::Draw);
        assert_eq!(
            consider(&board, &edges(&board), |_, _, _| panic!("draw")),
            None
        );
    }

    #[test]
    fn unknown_exhausted_and_inappropriate_proofs_short_circuit_at_first_failure() {
        let board = losing_position();
        for bad_at in [1, 3, 5] {
            for mode in 0..10 {
                let mut calls = 0;
                let result = consider(&board, &edges(&board), |after, side, budget| {
                    calls += 1;
                    if calls != bad_at {
                        return Some(certificate(after));
                    }
                    let mut proof = certificate(after);
                    match mode {
                        0 => return None,
                        1 => proof.winner = Color::Black,
                        2 => proof.line.clear(),
                        3 => proof.line[0] = mv(7, 3), // Occupied at root.
                        4 => proof.line[0] = *after.moves().last().unwrap(),
                        5 => proof.line = vec![mv(0, 10), mv(0, 11)], // Legal first, invalid continuation.
                        6 => {
                            proof.line.pop();
                        } // Truncated real proof.
                        7 => proof.line.push(mv(14, 14)),             // Appended real proof.
                        8 => {
                            return engine::prove_forced_win(
                                after,
                                side,
                                SearchBudget {
                                    max_nodes: 0,
                                    ..budget
                                },
                            );
                        }
                        _ => {
                            return engine::prove_forced_win(
                                after,
                                side,
                                SearchBudget {
                                    max_depth: 0,
                                    ..budget
                                },
                            );
                        }
                    }
                    Some(proof)
                });
                assert_eq!(result, None, "mode {mode}");
                assert_eq!(calls, bad_at, "mode {mode}");
            }
        }
    }

    #[test]
    fn nonempty_truncated_multistep_vcf_is_not_a_certificate() {
        let board = Board::from_position(
            &[mv(7, 3), mv(0, 0), mv(0, 2), mv(0, 4), mv(0, 6)],
            &[mv(7, 4), mv(7, 5), mv(7, 6), mv(6, 6), mv(8, 8)],
            Color::Black,
        )
        .unwrap();
        let mut ranked = edges(&board);
        ranked.iter_mut().find(|e| e.mv == mv(14, 14)).unwrap().n = 1;
        let mut calls = 0;
        let result = consider(&board, &ranked, |after, side, budget| {
            calls += 1;
            let mut proof = engine::prove_forced_win(after, side, budget).unwrap();
            assert!(engine::verify_line(after, &proof));
            assert!(proof.line.len() >= 3);
            proof.line.pop();
            assert!(!proof.line.is_empty());
            assert!(!engine::verify_line(after, &proof));
            Some(proof)
        });
        assert_eq!(result, None);
        assert_eq!(calls, 1);
    }

    #[test]
    fn legal_but_nonforcing_line_on_quiet_successor_is_unknown() {
        let board = Board::new();
        let mut calls = 0;
        assert_eq!(
            consider(&board, &edges(&board), |_, side, _| {
                calls += 1;
                Some(Proof {
                    winner: side,
                    line: vec![mv(1, 1)],
                })
            }),
            None
        );
        assert_eq!(calls, 1);
    }

    #[test]
    fn terminal_roots_never_probe() {
        let mut won = losing_position();
        won.play(mv(0, 0)).unwrap();
        won.play(mv(7, 2)).unwrap();
        assert_eq!(won.status(), Status::Won(Color::White));
        let mut drawn = almost_draw();
        drawn.play(mv(0, 0)).unwrap();
        for board in [won, drawn] {
            assert_eq!(
                consider(&board, &edges(&board), |_, _, _| panic!("terminal")),
                None
            );
        }
    }

    #[test]
    fn own_immediate_win_anywhere_cancels_even_outside_top_five() {
        let board = Board::from_position(
            &[mv(14, 1), mv(14, 2), mv(14, 3), mv(14, 4)],
            &[mv(7, 3), mv(7, 4), mv(7, 5), mv(7, 6)],
            Color::Black,
        )
        .unwrap();
        let ranked = edges(&board);
        let own_wins = engine::immediate_wins(&board, Color::Black);
        assert!(!own_wins.is_empty());
        assert!(
            own_wins
                .iter()
                .all(|winning| !ranked[..5].iter().any(|e| e.mv == winning))
        );
        assert_eq!(consider(&board, &ranked, |_, _, _| panic!("own win")), None);
    }

    #[test]
    fn invalid_roots_cancel_before_probing_even_when_bad_edge_is_outside_top_five() {
        let board = losing_position();
        let valid = edges(&board);
        let mut incomplete = valid.clone();
        incomplete.pop();
        let mut duplicate = valid.clone();
        duplicate.last_mut().unwrap().mv = valid[0].mv;
        let mut illegal = valid.clone();
        illegal.last_mut().unwrap().mv = mv(7, 3);
        let mut overfull = valid.clone();
        overfull.push(valid[0].clone());
        let mut invalid = vec![vec![], incomplete, overfull, duplicate, illegal];
        for prior in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut nonfinite = valid.clone();
            nonfinite.last_mut().unwrap().prior = prior;
            invalid.push(nonfinite);
        }
        for root in invalid {
            assert_eq!(
                consider(&board, &root, |_, _, _| panic!("invalid root")),
                None
            );
        }
    }

    #[test]
    fn five_certified_successors_rank_visits_prior_index_including_unvisited() {
        let board = losing_position();
        let original = board.clone();
        let mut ranked = edges(&board);
        for (square, n, prior) in [
            (mv(0, 4), 9, 0.1),
            (mv(0, 3), 8, 0.1),
            (mv(0, 2), 0, 0.9),
            (mv(0, 1), 0, 0.9),
            (mv(0, 0), 0, 0.5),
        ] {
            let edge = ranked.iter_mut().find(|e| e.mv == square).unwrap();
            edge.n = n;
            edge.prior = prior;
        }
        ranked.reverse();
        let before = ranked.clone();
        let mut checked = Vec::new();
        let result = consider(&board, &ranked, |after, side, budget| {
            let candidate = *after.moves().last().unwrap();
            let mut expected = board.clone();
            expected.play(candidate).unwrap();
            assert_eq!(*after, expected);
            assert_eq!(after.status(), Status::Ongoing);
            assert_eq!(side, Color::White);
            assert_eq!(budget, GuardConfig::baseline().budget);
            checked.push(candidate);
            Some(certificate(after))
        });
        assert_eq!(checked, [mv(0, 4), mv(0, 3), mv(0, 1), mv(0, 2), mv(0, 0)]);
        assert_eq!(
            result,
            Some(ResignReason::VerifiedOpponentVcf { candidates: 5 })
        );
        assert_eq!(board, original);
        assert_eq!(ranked, before);
    }
}
