//! Hand-written policy/value evaluator for the Gomoku baseline player.
//!
//! This crate implements the [`mcts::Evaluator`] seam with a
//! deterministic policy that combines gravity with both sides' threat
//! maps, including live-open-three blocks and separate opponent-fork
//! defense. Value combines the
//! existing map ladder with a bounded bonus for already formed forks.
//! An optional quiet-position whisper applies only when that value is zero.

#![deny(missing_docs)]

pub mod adapter;
mod forks;
pub mod gravity;
pub mod readout;

pub use burn::tensor::Device;

use engine::Color;
use gravity::GravityKernel;
use mcts::{EvalRequest, EvalResult, Evaluator};
use patterns::net::ThreatNet;

/// The baseline evaluator.
///
/// Combines gravity with threat-map policy tiers (wins, forced blocks,
/// own double threats, live-open-three blocks, potential forks, threes and
/// defensive memberships).
/// Value preserves the map ladder, including ±0.20 potential-fork terms,
/// and adds a guarded ±0.30 formed-fork bonus, clamped to ±0.99.
/// The optional quiet whisper applies only to a zero composed value.
pub struct BaselineEvaluator {
    device: Device,
    kernel: GravityKernel,
    net: ThreatNet,
    quiet: bool,
}

impl BaselineEvaluator {
    /// Create a new [`BaselineEvaluator`] on `device`.
    ///
    /// The [experiment] quiet-position value whisper is enabled by
    /// default; use [`Self::without_quiet_value`] to disable it.
    #[must_use]
    pub fn new(device: Device) -> Self {
        let kernel = GravityKernel::new(&device);
        let net = ThreatNet::new(&device);
        Self {
            device,
            kernel,
            net,
            quiet: true,
        }
    }

    /// Disable the [experiment] quiet-position value whisper.
    ///
    /// Chainable; intended for A/B measurement of the whisper term.
    #[must_use]
    pub fn without_quiet_value(mut self) -> Self {
        self.quiet = false;
        self
    }
}

/// Compute the baseline's prior policy for `board` without any search.
///
/// The evaluator's policy logits over legal moves are normalized with
/// the same masked softmax as MCTS expansion, then sorted by descending
/// probability. Equal probabilities are ordered center-out so the empty
/// board presents the center first. Terminal positions return an empty
/// vector.
#[must_use]
pub fn prior(board: &engine::Board) -> Vec<(engine::Move, f32)> {
    if board.status() != engine::Status::Ongoing {
        return Vec::new();
    }

    let legal: Vec<_> = board.empty_moves().collect();
    if legal.is_empty() {
        return Vec::new();
    }

    let mut evaluator = BaselineEvaluator::new(Device::flex());
    let result = evaluator.evaluate(EvalRequest {
        planes: engine::encode(board),
        legal: legal.clone(),
    });
    let mut priors = mcts::masked_softmax(&result.policy, &legal);
    let center_distance = |mv: engine::Move| {
        let row = mv.row().abs_diff(7);
        let col = mv.col().abs_diff(7);
        row * row + col * col
    };
    priors.sort_by(|(a_move, a_probability), (b_move, b_probability)| {
        b_probability
            .total_cmp(a_probability)
            .then_with(|| center_distance(*a_move).cmp(&center_distance(*b_move)))
    });
    priors
}

impl Evaluator for BaselineEvaluator {
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
        let board = adapter::reconstruct_board(&req.planes, &req.legal);
        let maps = patterns::net::analyze_both(&self.net, &board, &self.device);
        let black_formed = forks::has_open_three_fork(&board, Color::Black);
        let white_formed = forks::has_open_three_fork(&board, Color::White);
        let (own, opp, own_formed, opp_formed) = match board.to_move() {
            Color::Black => (&maps[0], &maps[1], black_formed, white_formed),
            Color::White => (&maps[1], &maps[0], white_formed, black_formed),
        };
        let gravity = self.kernel.compute(&req.planes);
        let policy = readout::policy_logits(&gravity, own, opp, &req.legal);
        let value = readout::value_with_forks(own, opp, own_formed, opp_formed);
        // [experiment] quiet whisper: only when the threat ladder is
        // silent. `value == 0.0` also covers symmetric threat
        // cancellation — that counts as "quiet" for this experiment.
        let value = if value == 0.0 && self.quiet {
            readout::quiet_value(&gravity, &req.planes)
        } else {
            value
        };
        EvalResult { policy, value }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Board, Move};

    fn request_from_board(board: &Board) -> EvalRequest {
        EvalRequest {
            planes: engine::encode(board),
            legal: board.empty_moves().collect(),
        }
    }

    fn best_move(board: &Board, res: EvalResult) -> Move {
        board
            .empty_moves()
            .max_by(|a, b| {
                res.policy[a.index()]
                    .partial_cmp(&res.policy[b.index()])
                    .unwrap()
            })
            .unwrap()
    }

    const SCATTERED: [(u8, u8); 6] = [(0, 0), (0, 3), (0, 6), (0, 9), (0, 12), (14, 0)];
    const FORMED: [(u8, u8); 5] = [(7, 5), (7, 6), (7, 7), (5, 7), (6, 7)];

    fn position(black: &[(u8, u8)], white: &[(u8, u8)], to_move: Color) -> Board {
        let moves = |cells: &[(u8, u8)]| -> Vec<Move> {
            cells
                .iter()
                .map(|&(r, c)| Move::new(r, c).unwrap())
                .collect()
        };
        Board::from_position(&moves(black), &moves(white), to_move).unwrap()
    }

    fn any(map: &[[f32; 15]; 15]) -> bool {
        map.iter().flatten().any(|&v| v > 0.0)
    }

    fn assert_three_maps(formed_color: Color, maps: &[patterns::net::ThreatMaps; 2]) {
        let (threat, quiet) = match formed_color {
            Color::Black => (&maps[0], &maps[1]),
            Color::White => (&maps[1], &maps[0]),
        };
        assert!(!any(&threat.wins));
        assert!(any(&threat.double_threats));
        assert!(!any(&threat.double_threes));
        assert!(!any(&quiet.wins));
        assert!(!any(&quiet.double_threats));
        assert!(!any(&quiet.double_threes));
        assert_eq!(readout::value(threat, quiet), 0.85);
    }

    #[test]
    fn established_fork_saturates_for_both_colors_and_side_to_move_perspectives() {
        let mut ev = BaselineEvaluator::new(Device::flex()).without_quiet_value();
        for color in [Color::Black, Color::White] {
            for to_move in [Color::Black, Color::White] {
                // Explicit, quiet opposite-side lists for each count-valid perspective.
                let fillers = match (color, to_move) {
                    (_, Color::Black) => &SCATTERED[..5],
                    (Color::Black, Color::White) => &SCATTERED[..4],
                    (Color::White, Color::White) => &SCATTERED[..6],
                };
                let (black, white) = if color == Color::Black {
                    (&FORMED[..], fillers)
                } else {
                    (fillers, &FORMED[..])
                };
                let board = position(black, white, to_move);
                let req = request_from_board(&board);
                let before = (
                    req.planes.clone(),
                    board.zobrist(),
                    board.status(),
                    board.to_move(),
                );
                let maps = patterns::net::analyze_both(&ev.net, &board, &ev.device);
                assert_three_maps(color, &maps);
                assert!(forks::has_open_three_fork(&board, color));
                assert!(!forks::has_open_three_fork(&board, color.other()));
                let expected = if color == to_move { 0.99 } else { -0.99 };
                assert_eq!(
                    ev.evaluate(req.clone()).value,
                    expected,
                    "{color:?} {to_move:?}"
                );
                assert_eq!(ev.evaluate(req).value, expected);
                assert_eq!(
                    (
                        engine::encode(&board),
                        board.zobrist(),
                        board.status(),
                        board.to_move()
                    ),
                    before
                );
            }
        }
    }

    #[test]
    fn one_established_three_retains_point_eighty_five_without_fork_bonus() {
        let mut ev = BaselineEvaluator::new(Device::flex()).without_quiet_value();
        let three = [(7, 5), (7, 6), (7, 7)];
        for color in [Color::Black, Color::White] {
            for to_move in [Color::Black, Color::White] {
                let fillers = match (color, to_move) {
                    (_, Color::Black) => &SCATTERED[..3],
                    (Color::Black, Color::White) => &SCATTERED[..2],
                    (Color::White, Color::White) => &SCATTERED[..4],
                };
                let (black, white) = if color == Color::Black {
                    (&three[..], fillers)
                } else {
                    (fillers, &three[..])
                };
                let board = position(black, white, to_move);
                let maps = patterns::net::analyze_both(&ev.net, &board, &ev.device);
                assert_three_maps(color, &maps);
                assert!(!forks::has_open_three_fork(&board, Color::Black));
                assert!(!forks::has_open_three_fork(&board, Color::White));
                let expected = if color == to_move { 0.85 } else { -0.85 };
                assert_eq!(ev.evaluate(request_from_board(&board)).value, expected);
            }
        }
    }

    #[test]
    fn potential_maker_is_not_established_until_the_move_is_played() {
        let mut board = position(
            &[(7, 5), (7, 6), (5, 7), (6, 7)],
            &SCATTERED[..4],
            Color::Black,
        );
        let mut ev = BaselineEvaluator::new(Device::flex()).without_quiet_value();
        let before = patterns::net::analyze_both(&ev.net, &board, &ev.device);
        assert!(!any(&before[0].wins));
        assert!(!any(&before[0].double_threats));
        assert!(before[0].double_threes[7][7] > 0.0);
        assert!(!any(&before[1].wins));
        assert!(!any(&before[1].double_threats));
        assert!(!any(&before[1].double_threes));
        assert!(!forks::has_open_three_fork(&board, Color::Black));
        assert!(!forks::has_open_three_fork(&board, Color::White));
        assert_eq!(ev.evaluate(request_from_board(&board)).value, 0.20);

        board.play(Move::new(7, 7).unwrap()).unwrap();
        let after = patterns::net::analyze_both(&ev.net, &board, &ev.device);
        assert_three_maps(Color::Black, &after);
        assert!(forks::has_open_three_fork(&board, Color::Black));
        assert!(!forks::has_open_three_fork(&board, Color::White));
        // All maker maps mask the occupied anchor, but its geometry still counts.
        assert_eq!(after[0].double_threes[7][7], 0.0);
        assert_eq!(after[0].double_threats[7][7], 0.0);
        assert_eq!(board.to_move(), Color::White);
        assert_eq!(ev.evaluate(request_from_board(&board)).value, -0.99);
    }

    #[test]
    fn both_established_forks_cancel_and_still_reach_the_existing_quiet_gate() {
        let white = [(3, 9), (3, 10), (3, 11), (1, 11), (2, 11)];
        let mut ev = BaselineEvaluator::new(Device::flex());
        let mut no_quiet = BaselineEvaluator::new(Device::flex()).without_quiet_value();
        for to_move in [Color::Black, Color::White] {
            let mut black = FORMED.to_vec();
            if to_move == Color::White {
                // A nearby sixth Black stone makes the whisper nonzero without
                // creating an immediate win or changing the map-value memberships.
                black.push((4, 5));
            }
            let board = position(&black, &white, to_move);
            let maps = patterns::net::analyze_both(&ev.net, &board, &ev.device);
            for (side, map) in maps.iter().enumerate() {
                assert!(!any(&map.wins));
                assert!(any(&map.double_threats));
                assert!(!any(&map.double_threes), "{to_move:?} side {side}");
            }
            assert!(forks::has_open_three_fork(&board, Color::Black));
            assert!(forks::has_open_three_fork(&board, Color::White));
            assert_eq!(readout::value(&maps[0], &maps[1]), 0.0);
            let req = request_from_board(&board);
            let gravity = ev.kernel.compute(&req.planes);
            let whisper = readout::quiet_value(&gravity, &req.planes);
            if to_move == Color::White {
                assert_ne!(whisper, 0.0);
            }
            assert_eq!(no_quiet.evaluate(req.clone()).value, 0.0);
            assert_eq!(ev.evaluate(req).value, whisper);
        }
    }

    #[test]
    fn multiple_own_anchors_still_cancel_one_opponent_fork_as_binary_presence() {
        // Black shares the horizontal three at row 7 between two vertical
        // threes, anchored at (7,5) and (7,7). White has one fork at (3,11).
        let board = position(
            &[(7, 5), (7, 6), (7, 7), (5, 7), (6, 7), (5, 5), (6, 5)],
            &[(3, 9), (3, 10), (3, 11), (1, 11), (2, 11), (0, 0), (0, 3)],
            Color::Black,
        );
        let mut ev = BaselineEvaluator::new(Device::flex()).without_quiet_value();
        let maps = patterns::net::analyze_both(&ev.net, &board, &ev.device);
        for map in &maps {
            assert!(!any(&map.wins));
            assert!(any(&map.double_threats));
        }
        assert!(any(&maps[0].double_threes));
        assert!(!any(&maps[1].double_threes));
        assert!(forks::has_open_three_fork(&board, Color::Black));
        assert!(forks::has_open_three_fork(&board, Color::White));
        assert_eq!(readout::value(&maps[0], &maps[1]), 0.20);
        assert_eq!(ev.evaluate(request_from_board(&board)).value, 0.20);
    }

    #[test]
    fn v0_policy_is_uniform_over_legal() {
        let board = Board::new();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);
        for mv in board.empty_moves() {
            assert_eq!(res.policy[mv.index()], 0.0f32);
        }
    }

    #[test]
    fn v0_occupied_cells_are_masked() {
        let mut board = Board::new();
        board.play(Move::new(7, 7).unwrap()).unwrap();
        board.play(Move::new(0, 0).unwrap()).unwrap();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);
        assert_eq!(
            res.policy[Move::new(7, 7).unwrap().index()],
            f32::NEG_INFINITY
        );
        assert_eq!(
            res.policy[Move::new(0, 0).unwrap().index()],
            f32::NEG_INFINITY
        );
    }

    #[test]
    fn v0_value_is_zero() {
        let board = Board::new();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);
        assert_eq!(res.value, 0.0f32);
    }

    #[test]
    fn v0_is_deterministic() {
        let board = Board::new();
        let req = request_from_board(&board);
        let mut ev1 = BaselineEvaluator::new(Device::flex());
        let mut ev2 = BaselineEvaluator::new(Device::flex());
        let res1 = ev1.evaluate(req.clone());
        let res2 = ev2.evaluate(req);
        assert_eq!(res1, res2);
    }

    #[test]
    fn v1_highest_logit_hugs_stones() {
        let mut board = Board::new();
        board.play(Move::new(7, 7).unwrap()).unwrap();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);

        let max_logit = board
            .empty_moves()
            .map(|mv| res.policy[mv.index()])
            .fold(f32::NEG_INFINITY, f32::max);
        assert_eq!(max_logit, 2.0f32 / 32.0f32);

        // Cells far from the stone (distance > 2) remain at zero.
        assert_eq!(res.policy[Move::new(0, 0).unwrap().index()], 0.0f32);
    }

    #[test]
    fn v1_empty_board_is_uniform() {
        let board = Board::new();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);
        for mv in board.empty_moves() {
            assert_eq!(res.policy[mv.index()], 0.0f32);
        }
    }

    #[test]
    fn v1_corner_stone_not_boosted() {
        let mut board = Board::new();
        board.play(Move::new(0, 0).unwrap()).unwrap();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);

        let max_logit = board
            .empty_moves()
            .map(|mv| res.policy[mv.index()])
            .fold(f32::NEG_INFINITY, f32::max);
        assert_eq!(max_logit, 2.0f32 / 32.0f32);

        let corner_sum: f32 = board.empty_moves().map(|mv| res.policy[mv.index()]).sum();
        assert!(corner_sum < 1.0f32);
    }

    #[test]
    fn v1_value_still_zero() {
        let board = Board::new();
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);
        assert_eq!(res.value, 0.0f32);
    }

    #[test]
    fn t1_win_now_is_argmax() {
        let mut board = Board::new();
        let script = [
            (7, 3),
            (0, 0),
            (7, 4),
            (0, 2),
            (7, 5),
            (0, 4),
            (7, 6),
            (0, 6),
        ];
        for &(r, c) in &script {
            board.play(Move::new(r, c).unwrap()).unwrap();
        }
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);

        let wins = engine::immediate_wins(&board, board.to_move());

        let argmax = best_move(&board, res);

        assert!(
            wins.contains(argmax),
            "argmax {argmax:?} must be a winning move"
        );
    }

    #[test]
    fn t2_forced_block_is_argmax() {
        use engine::reference::board_from_ascii;
        let board = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . O O O O . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            X . . . X . . . X . . . X . .
            ",
        );
        // White's open four makes almost every non-winning White cell a
        // double-threat maker. Scattered Black fillers keep this focused
        // on T2 rather than a T3+T3B tie decided by gravity.
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req.clone());

        let argmax = best_move(&board, res);
        let blocks = engine::forced_blocks(&board);
        assert!(blocks.contains(argmax), "argmax {argmax:?} must be a block");
    }

    #[test]
    fn t2_exact_logit_on_crafted_board() {
        use engine::reference::board_from_ascii;
        let board = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . O O O O . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            X . X . X . X . . . . . . . .
            ",
        );
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req.clone());

        let gravity = gravity::gravity_map(&req.planes);
        let blocks = engine::forced_blocks(&board);
        let mv = blocks.iter().next().unwrap();
        let expected = readout::T2_FORCED_BLOCK + gravity[mv.row() as usize][mv.col() as usize];
        assert_eq!(res.policy[mv.index()], expected);
    }

    #[test]
    fn quiet_whisper_t1_win_still_returns_nine_five() {
        // Four Black stones in a row: Black to move has an immediate
        // win; the whisper must not override the threat ladder.
        let mut board = Board::new();
        let script = [
            (7, 3),
            (0, 0),
            (7, 4),
            (0, 2),
            (7, 5),
            (0, 4),
            (7, 6),
            (0, 6),
        ];
        for &(r, c) in &script {
            board.play(Move::new(r, c).unwrap()).unwrap();
        }
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);
        assert_eq!(res.value, 0.95f32);
    }

    #[test]
    fn quiet_whisper_without_quiet_is_zero_on_quiet_cluster() {
        // Black cluster vs scattered White corners: a quiet position
        // where the whisper would otherwise be non-zero.
        let mut board = Board::new();
        let script = [
            (7, 7),
            (0, 0),
            (7, 8),
            (0, 14),
            (8, 7),
            (14, 0),
            (8, 8),
            (14, 14),
        ];
        for &(r, c) in &script {
            board.play(Move::new(r, c).unwrap()).unwrap();
        }
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex()).without_quiet_value();
        let res = ev.evaluate(req);
        assert_eq!(res.value, 0.0f32);
    }

    #[test]
    fn prior_empty_board_is_normalized_sorted_and_centered() {
        let priors = prior(&Board::new());

        assert_eq!(priors.len(), 225);
        let sum: f32 = priors.iter().map(|&(_, probability)| probability).sum();
        assert!((sum - 1.0).abs() < 1e-5, "sum = {sum}");
        assert!(
            priors.windows(2).all(|pair| pair[0].1 >= pair[1].1),
            "priors must be sorted by descending probability"
        );
        assert_eq!(priors[0].0, Move::new(7, 7).unwrap());
    }

    #[test]
    fn prior_immediate_win_is_top() {
        let mut board = Board::new();
        let script = [
            (7, 3),
            (0, 0),
            (7, 4),
            (0, 2),
            (7, 6),
            (0, 4),
            (7, 7),
            (0, 6),
        ];
        for &(row, col) in &script {
            board.play(Move::new(row, col).unwrap()).unwrap();
        }
        let winning_move = Move::new(7, 5).unwrap();
        assert!(engine::immediate_wins(&board, board.to_move()).contains(winning_move));

        let priors = prior(&board);

        assert_eq!(priors[0].0, winning_move);
        assert!(priors[0].1 > priors[1].1);
    }

    #[test]
    fn prior_live_open_three_blocks_are_top_two() {
        let board = position(
            &[(6, 6), (7, 7), (7, 8), (6, 10)],
            &[(6, 8), (8, 8), (7, 9), (8, 10)],
            Color::Black,
        );

        let priors = prior(&board);
        let top_two = [priors[0].0, priors[1].0];

        assert!(
            top_two.contains(&Move::new(5, 7).unwrap()),
            "H6 must be a top-two block; prior leaders: {:?}",
            &priors[..4]
        );
        assert!(
            top_two.contains(&Move::new(9, 11).unwrap()),
            "L10 must be a top-two block; prior leaders: {:?}",
            &priors[..4]
        );
    }

    #[test]
    fn prior_terminal_positions_are_empty() {
        let mut won = Board::new();
        let script = [
            (7, 3),
            (0, 0),
            (7, 4),
            (0, 2),
            (7, 5),
            (0, 4),
            (7, 6),
            (0, 6),
            (7, 7),
        ];
        for &(row, col) in &script {
            won.play(Move::new(row, col).unwrap()).unwrap();
        }
        assert_eq!(won.status(), engine::Status::Won(Color::Black));
        assert!(prior(&won).is_empty());

        let mut draw = Board::new();
        let mut black = Vec::new();
        let mut white = Vec::new();
        for row in 0..15u8 {
            for col in 0..15u8 {
                let stripe = (col % 4) < 2;
                let is_black = stripe != (row % 2 == 1);
                let mv = Move::new(row, col).unwrap();
                if is_black {
                    black.push(mv);
                } else {
                    white.push(mv);
                }
            }
        }
        for index in 0..112 {
            draw.play(black[index]).unwrap();
            draw.play(white[index]).unwrap();
        }
        draw.play(black[112]).unwrap();
        assert_eq!(draw.status(), engine::Status::Draw);
        assert_eq!(draw.empty_moves().count(), 0);
        assert!(prior(&draw).is_empty());
    }
}
