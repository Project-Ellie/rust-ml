//! Threat potential: turn the threat maps into a sparse policy vector.
//!
//! The maps already score every legal move, so a small combiner produces
//! `policy(b, s) -> Vec<(Move, f32)>` — the same sparse shape as the
//! datagen tutorial's `Sample::policy` and the future mcts `Evaluator`
//! output. This is a *shape* contract for future integration, not an
//! integration itself.
//!
//! # Combination weights (`[experiment]`)
//!
//! The following weights are documented as experimental hyper-parameters.
//! They encode the threat ladder: a win now is unbeatable, a double
//! threat is next, a double-three fork is a promising sequence starter,
//! and a single open-three maker is the weakest signal. Cells that do
//! not fall into any category are omitted.
//!
//! | Category | Condition | Score |
//! |----------|-----------|-------|
//! | Win | `wins[r][c] > 0` | `1.0` |
//! | Double threat | `double_threats[r][c] > 0` | `0.9` |
//! | Double-three fork | `double_threes[r][c] > 0` | `0.8` |
//! | Single three maker | any `threes_per_dir[d][r][c] > 0` | `0.3` |
//!
//! A cell receives exactly one score: the highest-priority category it
//! satisfies. The result is sorted lexicographically by row then column so
//! the order is deterministic and matches the natural board scan.

use crate::net::{ThreatMaps, ThreatNet, analyze};
use burn::tensor::Device;
use engine::{Board, Color, Move};

/// Score assigned to an immediate winning move.
pub const WIN_SCORE: f32 = 1.0;

/// Score assigned to a double-threat move (open four or equivalent).
pub const DOUBLE_THREAT_SCORE: f32 = 0.9;

/// Score assigned to a double-three fork move.
pub const DOUBLE_THREE_SCORE: f32 = 0.8;

/// Score assigned to a single open-three maker move.
pub const THREE_SCORE: f32 = 0.3;

/// Build a sparse policy vector from the threat network.
///
/// The returned vector contains one entry for every legal empty in-board
/// cell that satisfies at least one threat category. Occupied cells and
/// cells with score `0.0` are omitted, so the vector may be empty.
///
/// The scores are chosen so that higher tactical value always dominates
/// lower value: a cell is never represented by more than one score.
pub fn policy(net: &ThreatNet, b: &Board, s: Color, device: &Device) -> Vec<(Move, f32)> {
    let maps = analyze(net, b, s, device);
    policy_from_maps(b, &maps)
}

/// Build a sparse policy vector from already-computed [`ThreatMaps`].
///
/// This is useful when the maps have already been computed for
/// visualisation or for comparing backends on the same position.
pub fn policy_from_maps(b: &Board, maps: &ThreatMaps) -> Vec<(Move, f32)> {
    let mut out = Vec::new();

    for r in 0..15u8 {
        for c in 0..15u8 {
            let mv = Move::new(r, c).expect("in-board indices");
            if b.stone_at(mv).is_some() {
                continue;
            }

            let score = score_cell(maps, r as usize, c as usize);
            if score > 0.0 {
                out.push((mv, score));
            }
        }
    }

    // Deterministic lexicographic order by row, then column (row-major scan).
    out.sort_by_key(|&(mv, _)| (mv.row(), mv.col()));
    out
}

/// Return the priority score for a single empty cell.
///
/// The first matching category wins; a cell receives at most one score.
pub(crate) fn score_cell(maps: &ThreatMaps, r: usize, c: usize) -> f32 {
    if maps.wins[r][c] > 0.0 {
        return WIN_SCORE;
    }
    if maps.double_threats[r][c] > 0.0 {
        return DOUBLE_THREAT_SCORE;
    }
    if maps.double_threes[r][c] > 0.0 {
        return DOUBLE_THREE_SCORE;
    }
    if maps.threes_per_dir.iter().any(|dir| dir[r][c] > 0.0) {
        return THREE_SCORE;
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use burn::tensor::Device;
    use engine::reference::board_from_ascii;
    use engine::{Board, Color, Move};

    #[test]
    fn double_three_fork_scores_zero_point_eight() {
        // Four black stones plus four white filler stones to keep the
        // board counts valid.
        let b = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . X . . . . . . .
            . . . . . . X . X . . . . . .
            . . . . . . . X . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            O . . . . . . . . . . . . . O
            O . . . . . . . . . . . . . O
            ",
        );
        let device = Device::flex();
        let net = ThreatNet::new(&device);
        let p = policy(&net, &b, Color::Black, &device);

        assert!(
            p.iter()
                .any(|(mv, s)| { (mv.row(), mv.col()) == (7, 7) && *s == DOUBLE_THREE_SCORE })
        );
    }

    #[test]
    fn open_four_maker_scores_zero_point_nine() {
        let b = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . X X X . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . O
            O . . . . . . . . . . . . . O
            ",
        );
        let device = Device::flex();
        let net = ThreatNet::new(&device);
        let p = policy(&net, &b, Color::Black, &device);

        assert!(
            p.iter()
                .any(|(mv, s)| { (mv.row(), mv.col()) == (7, 4) && *s == DOUBLE_THREAT_SCORE })
        );
        assert!(
            p.iter()
                .any(|(mv, s)| { (mv.row(), mv.col()) == (7, 8) && *s == DOUBLE_THREAT_SCORE })
        );
    }

    #[test]
    fn win_scores_one_point_zero() {
        let b = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . X X X X . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            O . . . . . . . . . . . . . O
            O . . . . . . . . . . . . . O
            ",
        );
        let device = Device::flex();
        let net = ThreatNet::new(&device);
        let p = policy(&net, &b, Color::Black, &device);

        assert!(
            p.iter()
                .any(|(mv, s)| { (mv.row(), mv.col()) == (7, 3) && *s == WIN_SCORE })
        );
        assert!(
            p.iter()
                .any(|(mv, s)| { (mv.row(), mv.col()) == (7, 8) && *s == WIN_SCORE })
        );
    }

    #[test]
    fn quiet_board_returns_empty_policy() {
        let b = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . X . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . O . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . O . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . X . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            ",
        );
        let device = Device::flex();
        let net = ThreatNet::new(&device);
        let p = policy(&net, &b, Color::Black, &device);
        assert!(p.is_empty());
    }

    #[test]
    fn only_legal_empty_cells_are_present() {
        // Build a position where Black has an open four, then have White
        // block both winning cells through legal alternating play.
        let mut b = Board::new();
        let seq: &[(u8, u8, Color)] = &[
            (7, 4, Color::Black),
            (0, 0, Color::White),
            (7, 5, Color::Black),
            (0, 1, Color::White),
            (7, 6, Color::Black),
            (7, 3, Color::White), // block left winning cell
            (7, 7, Color::Black),
            (7, 8, Color::White), // block right winning cell
        ];
        for &(r, c, side) in seq {
            assert_eq!(b.to_move(), side, "play order mismatch");
            b.play(Move::new(r, c).unwrap()).unwrap();
        }

        let device = Device::flex();
        let net = ThreatNet::new(&device);
        let p = policy(&net, &b, Color::Black, &device);
        assert!(
            p.is_empty(),
            "occupied winning cells must not appear in policy"
        );
    }
}
