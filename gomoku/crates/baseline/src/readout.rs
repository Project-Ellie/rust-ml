//! Policy and value readouts from threat maps and gravity.

use engine::Move;
use patterns::net::ThreatMaps;

/// Logit weight for an immediate win for the side to move.
pub const T1_WIN: f32 = 64.0;
/// Logit weight for a forced block (opponent immediate win).
pub const T2_FORCED_BLOCK: f32 = 32.0;
/// Logit weight for an own double threat.
pub const T3_DOUBLE_THREAT: f32 = 16.0;
/// Logit weight for an own double-three fork.
pub const T4A_DOUBLE_THREE: f32 = 8.0;
/// Logit weight for any own three-maker.
pub const T4B_THREE: f32 = 4.0;
/// Logit weight for any opponent three-maker or double-three.
pub const T4C_DEFENSIVE: f32 = 2.0;

/// Maximum possible gravity contribution after normalization.
#[allow(dead_code)]
const T5_MAX: f32 = 1.0;

/// Build a 225-vector of policy logits from threat maps and gravity.
///
/// Tiers are applied by binary membership; occupied or illegal cells
/// receive `f32::NEG_INFINITY`.
#[must_use]
pub fn policy_logits(
    gravity: &[[f32; 15]; 15],
    own: &ThreatMaps,
    opp: &ThreatMaps,
    legal: &[Move],
) -> [f32; 225] {
    let mut logits = [f32::NEG_INFINITY; 225];
    for mv in legal {
        let r = mv.row() as usize;
        let c = mv.col() as usize;
        let mut logit = gravity[r][c];
        if own.wins[r][c] > 0.0 {
            logit += T1_WIN;
        }
        if opp.wins[r][c] > 0.0 {
            logit += T2_FORCED_BLOCK;
        }
        logits[mv.index()] = logit;
    }
    logits
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Board, Move};

    fn empty_maps() -> ThreatMaps {
        ThreatMaps {
            wins: [[0.0f32; 15]; 15],
            double_threats: [[0.0f32; 15]; 15],
            double_threes: [[0.0f32; 15]; 15],
            threes_per_dir: [[[0.0f32; 15]; 15]; 4],
            fours_created_per_dir: [[[0.0f32; 15]; 15]; 4],
        }
    }

    #[test]
    fn logits_are_uniform_when_gravity_is_zero() {
        let gravity = [[0.0f32; 15]; 15];
        let board = Board::new();
        let legal: Vec<Move> = board.empty_moves().collect();
        let own = empty_maps();
        let opp = empty_maps();
        let logits = policy_logits(&gravity, &own, &opp, &legal);
        for mv in &legal {
            assert_eq!(logits[mv.index()], 0.0f32);
        }
    }

    #[test]
    fn logits_mask_illegal_cells() {
        let gravity = [[0.0f32; 15]; 15];
        let mut board = Board::new();
        board.play(Move::new(7, 7).unwrap()).unwrap();
        let legal: Vec<Move> = board.empty_moves().collect();
        let own = empty_maps();
        let opp = empty_maps();
        let logits = policy_logits(&gravity, &own, &opp, &legal);
        assert_eq!(logits[Move::new(7, 7).unwrap().index()], f32::NEG_INFINITY);
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn domination_invariant_holds() {
        assert!(
            T2_FORCED_BLOCK
                > T3_DOUBLE_THREAT + T4A_DOUBLE_THREE + T4B_THREE + T4C_DEFENSIVE + T5_MAX
        );
        assert!(
            T1_WIN
                > T2_FORCED_BLOCK
                    + T3_DOUBLE_THREAT
                    + T4A_DOUBLE_THREE
                    + T4B_THREE
                    + T4C_DEFENSIVE
                    + T5_MAX
        );
    }
}
