//! Established open-three forks on occupied stones, independent of maker maps.

use engine::{Board, Color, Move};

/// Inspect occupied anchors for two distinct open-four-capable three directions.
/// This geometry is a heuristic, not a forced-win or counterattack check.
pub(super) fn has_open_three_fork(board: &Board, color: Color) -> bool {
    for r in 0..15 {
        for c in 0..15 {
            let anchor = Move::new(r, c).expect("in-board anchor");
            if board.stone_at(anchor) != Some(color) {
                continue;
            }
            let mut directions = 0;
            for (dr, dc) in [(0, 1), (1, 0), (1, 1), (1, -1)] {
                if (1..=4).any(|interior| {
                    open_three_window(board, color, r as i8, c as i8, dr, dc, interior)
                }) {
                    directions += 1;
                    if directions == 2 {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn open_three_window(
    board: &Board,
    color: Color,
    r: i8,
    c: i8,
    dr: i8,
    dc: i8,
    anchor_interior: i8,
) -> bool {
    let mut own = 0;
    for cell in 0..6 {
        let offset = cell - anchor_interior;
        let (row, col) = (r + dr * offset, c + dc * offset);
        if !(0..15).contains(&row) || !(0..15).contains(&col) {
            return false;
        }
        let mv = Move::new(row as u8, col as u8).expect("checked window bounds");
        match board.stone_at(mv) {
            None => (),
            Some(stone) if stone == color && cell != 0 && cell != 5 => own += 1,
            Some(_) => return false,
        }
    }
    own == 3
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Board, Color, Move, Transform};

    const QUIET_FIVE: [(u8, u8); 5] = [(14, 0), (14, 3), (14, 6), (14, 9), (14, 12)];
    const QUIET_SIX: [(u8, u8); 6] = [(14, 0), (14, 3), (14, 6), (14, 9), (14, 12), (0, 0)];

    // Both complete stone lists are explicit: no automatic count repair or filler.
    fn position(own: &[(u8, u8)], opp: &[(u8, u8)], color: Color, t: Transform) -> Board {
        let moves = |cells: &[(u8, u8)]| -> Vec<Move> {
            cells
                .iter()
                .map(|&(r, c)| t.transform_move(Move::new(r, c).unwrap()))
                .collect()
        };
        let (own, opp) = (moves(own), moves(opp));
        let (black, white) = match color {
            Color::Black => (&own, &opp),
            Color::White => (&opp, &own),
        };
        let to_move = if black.len() > white.len() {
            Color::White
        } else {
            Color::Black
        };
        Board::from_position(black, white, to_move).unwrap()
    }

    fn assert_geometry(own: &[(u8, u8)], opp: &[(u8, u8)], expected: bool) {
        for color in [Color::Black, Color::White] {
            for t in Transform::ALL {
                let board = position(own, opp, color, t);
                let before = (
                    engine::encode(&board),
                    board.zobrist(),
                    board.status(),
                    board.to_move(),
                );
                assert_eq!(
                    has_open_three_fork(&board, color),
                    expected,
                    "{color:?} {t:?}"
                );
                assert!(
                    !has_open_three_fork(&board, color.other()),
                    "unexpected opponent fork"
                );
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
    fn empty_board_has_no_established_fork() {
        for color in [Color::Black, Color::White] {
            assert!(!has_open_three_fork(&Board::new(), color));
        }
    }

    #[test]
    fn overlapping_windows_in_one_direction_are_not_a_fork() {
        assert_geometry(
            &[(7, 6), (7, 7), (7, 8)],
            &[(14, 0), (14, 3), (14, 6)],
            false,
        );
    }

    #[test]
    fn disjoint_perpendicular_threes_do_not_share_an_anchor() {
        assert_geometry(
            &[(5, 4), (5, 5), (5, 6), (7, 10), (8, 10), (9, 10)],
            &QUIET_SIX,
            false,
        );
    }

    #[test]
    fn separated_parallel_threes_are_not_a_fork() {
        assert_geometry(
            &[(5, 4), (5, 5), (5, 6), (9, 4), (9, 5), (9, 6)],
            &QUIET_SIX,
            false,
        );
    }

    #[test]
    fn valid_edge_windows_need_no_extra_margin() {
        assert_geometry(&[(1, 1), (1, 2), (1, 3), (2, 1), (3, 1)], &QUIET_FIVE, true);
    }

    #[test]
    fn off_board_endpoints_are_not_empty() {
        assert_geometry(
            &[(0, 0), (0, 1), (0, 2), (1, 0), (2, 0)],
            &QUIET_FIVE,
            false,
        );
    }

    #[test]
    fn opponent_at_either_end_blocks_the_straight_three() {
        for end in [5, 9] {
            assert_geometry(
                &[(7, 6), (7, 7), (7, 8), (6, 7), (8, 7)],
                &[(7, end), (14, 0), (14, 3), (14, 6), (14, 9)],
                false,
            );
        }
    }

    #[test]
    fn occupied_interior_gap_blocks_the_gapped_three() {
        assert_geometry(
            &[(7, 5), (7, 7), (7, 8), (6, 7), (8, 7)],
            &[(7, 6), (14, 0), (14, 3), (14, 6), (14, 9)],
            false,
        );
    }

    #[test]
    fn four_own_interior_stones_are_not_a_three() {
        assert_geometry(
            &[(7, 5), (7, 6), (7, 7), (7, 8), (6, 7), (8, 7)],
            &QUIET_SIX,
            false,
        );
    }

    #[test]
    fn potential_intersection_requires_the_anchor_to_be_played() {
        let mut board = position(
            &[(7, 5), (7, 6), (5, 7), (6, 7)],
            &[(14, 0), (14, 3), (14, 6), (14, 9)],
            Color::Black,
            Transform::Id,
        );
        assert!(!has_open_three_fork(&board, Color::Black));
        board.play(Move::new(7, 7).unwrap()).unwrap();
        assert!(has_open_three_fork(&board, Color::Black));
        assert!(!has_open_three_fork(&board, Color::White));
    }

    #[test]
    fn established_straight_and_gapped_forks_all_direction_pairs_colors_and_d4() {
        let dirs = [(0i8, 1i8), (1, 0), (1, 1), (1, -1)];
        // Anchor at each possible occupied interior position; every possible gap.
        for anchor in 1..=4 {
            for gap in 1..=4 {
                if gap == anchor {
                    continue;
                }
                for (i, &(dr, dc)) in dirs.iter().enumerate() {
                    for &(er, ec) in &dirs[i + 1..] {
                        let mut own = vec![(7, 7)];
                        for (r, c) in [(dr, dc), (er, ec)] {
                            for cell in 1..=4 {
                                if cell != anchor && cell != gap {
                                    let offset = cell - anchor;
                                    own.push(((7 + r * offset) as u8, (7 + c * offset) as u8));
                                }
                            }
                        }
                        for color in [Color::Black, Color::White] {
                            for t in Transform::ALL {
                                let board = position(&own, &QUIET_FIVE, color, t);
                                assert!(
                                    has_open_three_fork(&board, color),
                                    "{own:?} {color:?} {t:?}"
                                );
                                assert!(!has_open_three_fork(&board, color.other()), "filler fork");
                            }
                        }
                    }
                }
            }
        }
    }
}
