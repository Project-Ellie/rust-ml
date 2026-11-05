//! Policy and value readouts from threat maps and gravity.

use engine::Move;
use patterns::net::ThreatMaps;

/// Value of an own immediate win.
const V_OWN_WIN: f32 = 0.95;
/// Value near-loss for two or more opponent immediate wins.
const V_TWO_OPP_WINS: f32 = -0.99;
/// Value contribution for exactly one opponent immediate win.
const V_OPP_WIN: f32 = -0.75;
/// Value contribution for an own double threat.
const V_OWN_DOUBLE_THREAT: f32 = 0.85;
/// Value contribution for an opponent double threat.
const V_OPP_DOUBLE_THREAT: f32 = -0.85;
/// Absolute value ceiling; ±1 is reserved for proven terminals.
const V_CLAMP: f32 = 0.99;

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

/// Side-to-move value heuristic from two threat maps.
///
/// The ladder (minus the double-three rungs that land in chapter 06):
/// own win ⇒ +0.95; opponent ≥2 wins ⇒ −0.99; otherwise accumulate
/// (−0.75 for opponent's single win, +0.85 for own double threat,
/// −0.85 for opponent double threat) and clamp to ±0.99. A quiet
/// board returns exactly 0.0.
#[must_use]
pub fn value(own: &ThreatMaps, opp: &ThreatMaps) -> f32 {
    if has_positive(&own.wins) {
        return V_OWN_WIN;
    }

    let opp_wins = count_positive(&opp.wins);
    if opp_wins >= 2 {
        return V_TWO_OPP_WINS;
    }

    let mut v = 0.0f32;
    if opp_wins == 1 {
        v += V_OPP_WIN;
    }

    if has_positive(&own.double_threats) {
        v += V_OWN_DOUBLE_THREAT;
    }
    if has_positive(&opp.double_threats) {
        v += V_OPP_DOUBLE_THREAT;
    }

    v.clamp(-V_CLAMP, V_CLAMP)
}

fn has_positive(map: &[[f32; 15]; 15]) -> bool {
    map.iter().any(|row| row.iter().any(|&v| v > 0.0))
}

fn count_positive(map: &[[f32; 15]; 15]) -> usize {
    map.iter()
        .map(|row| row.iter().filter(|&&v| v > 0.0).count())
        .sum()
}

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

    #[test]
    fn value_quiet_board_is_zero() {
        let own = empty_maps();
        let opp = empty_maps();
        assert_eq!(value(&own, &opp), 0.0f32);
    }

    #[test]
    fn value_own_win_sentinel() {
        let mut own = empty_maps();
        own.wins[7][7] = 1.0f32;
        let opp = empty_maps();
        assert_eq!(value(&own, &opp), V_OWN_WIN);
    }

    #[test]
    fn value_two_opp_wins_is_near_loss() {
        let own = empty_maps();
        let mut opp = empty_maps();
        opp.wins[7][3] = 1.0f32;
        opp.wins[7][8] = 1.0f32;
        assert_eq!(value(&own, &opp), V_TWO_OPP_WINS);
    }

    #[test]
    fn value_single_opp_win_is_negative() {
        let own = empty_maps();
        let mut opp = empty_maps();
        opp.wins[7][3] = 1.0f32;
        assert_eq!(value(&own, &opp), V_OPP_WIN);
    }

    #[test]
    fn value_own_double_threat_is_positive() {
        let mut own = empty_maps();
        own.double_threats[7][7] = 1.0f32;
        let opp = empty_maps();
        assert_eq!(value(&own, &opp), V_OWN_DOUBLE_THREAT);
    }

    #[test]
    fn value_saturation_clamps_near_one() {
        // Own double threat (+0.85) plus opponent single win (−0.75)
        // would sum to +0.10, so this board stays inside the clamp.
        // To hit the ceiling we add an own win, which is a fast path.
        let mut own = empty_maps();
        own.wins[0][0] = 1.0f32;
        let mut opp = empty_maps();
        opp.double_threats[7][7] = 1.0f32;
        assert_eq!(value(&own, &opp), V_OWN_WIN);

        // Two opponent wins short-circuit to −0.99 even if we also
        // hold a double threat.
        let mut own = empty_maps();
        own.double_threats[7][7] = 1.0f32;
        let mut opp = empty_maps();
        opp.wins[0][0] = 1.0f32;
        opp.wins[0][1] = 1.0f32;
        assert_eq!(value(&own, &opp), V_TWO_OPP_WINS);
    }

    #[test]
    fn value_never_touches_one() {
        // Enumerate a small grid of threat combinations and assert the
        // absolute value stays strictly below 1.0 and at most 0.99.
        let configs: Vec<(bool, usize, bool, bool)> = vec![
            (false, 0, false, false),
            (true, 0, false, false),
            (false, 1, false, false),
            (false, 2, false, false),
            (false, 0, true, false),
            (false, 0, false, true),
            (false, 1, true, false),
            (false, 1, false, true),
            (true, 0, true, false),
        ];
        for (own_win, opp_wins, own_dt, opp_dt) in configs {
            let mut own = empty_maps();
            let mut opp = empty_maps();
            if own_win {
                own.wins[0][0] = 1.0f32;
            }
            for cell in opp.wins[0].iter_mut().take(opp_wins) {
                *cell = 1.0f32;
            }
            if own_dt {
                own.double_threats[7][7] = 1.0f32;
            }
            if opp_dt {
                opp.double_threats[7][7] = 1.0f32;
            }
            let v = value(&own, &opp);
            assert!(v.abs() < 1.0f32, "value {v} touched ±1");
            assert!(v.abs() <= V_CLAMP, "value {v} exceeded clamp");
        }
    }

    #[test]
    fn value_monotonic_in_threats() {
        // Start from a quiet position.
        let base_own = empty_maps();
        let base_opp = empty_maps();
        let quiet = value(&base_own, &base_opp);

        // Adding an own double threat cannot decrease value.
        let mut own_dt = empty_maps();
        own_dt.double_threats[5][5] = 1.0f32;
        assert!(value(&own_dt, &base_opp) >= quiet);

        // Adding an opponent double threat cannot increase value.
        let mut opp_dt = empty_maps();
        opp_dt.double_threats[5][5] = 1.0f32;
        assert!(value(&base_own, &opp_dt) <= quiet);

        // Own win dominates own double threat.
        let mut own_win = empty_maps();
        own_win.wins[5][5] = 1.0f32;
        assert!(value(&own_win, &base_opp) >= value(&own_dt, &base_opp));

        // Two opponent wins dominate one opponent win.
        let mut opp_one = empty_maps();
        opp_one.wins[5][5] = 1.0f32;
        let mut opp_two = empty_maps();
        opp_two.wins[5][5] = 1.0f32;
        opp_two.wins[5][6] = 1.0f32;
        assert!(value(&base_own, &opp_two) <= value(&base_own, &opp_one));
    }

    #[test]
    fn value_is_side_to_move_relative() {
        // Black holds a double threat. From Black's perspective the
        // value is positive; from White's perspective the same
        // absolute position means the opponent holds it, so value is
        // the negative mirror.
        let mut black_maps = empty_maps();
        black_maps.double_threats[7][7] = 1.0f32;
        let white_maps = empty_maps();
        let from_black = value(&black_maps, &white_maps);
        let from_white = value(&white_maps, &black_maps);
        assert_eq!(from_black, V_OWN_DOUBLE_THREAT);
        assert_eq!(from_white, -V_OWN_DOUBLE_THREAT);
    }
}
