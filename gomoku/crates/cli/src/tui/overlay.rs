//! Search-info overlays — pure functions, no I/O.
//!
//! [`value_strip`] turns a cursor node's [`SearchInfo`] into the lines of
//! the side-panel value strip. The helper is deliberately testable
//! without a real MCTS search: callers can construct [`SearchInfo`]
//! directly.

use crate::actor::{AiConfig, SearchInfo};
use crate::fmt::format_move;
use crate::session::SessionCore;

/// Build the value-strip panel lines from a cursor's search info.
///
/// `current_config` is the live config of the seat that searched the
/// cursor position. If it differs from the fingerprint stored in
/// [`SearchInfo::config`], or if the seat is now human, a staleness note
/// is appended. The searching side is read from
/// [`SearchInfo::side_to_move`], so the caller does not pass it.
#[must_use]
pub fn value_strip(info: Option<&SearchInfo>, current_config: Option<&AiConfig>) -> Vec<String> {
    let Some(info) = info else {
        return Vec::new();
    };

    let total: u32 = info.visits.iter().map(|(_, n)| n).sum();
    let value_sign = if info.value >= 0.0 { '+' } else { '-' };
    let side = info.side_to_move;

    let mut lines = Vec::new();
    lines.push(format!(
        "value {}{:.2} for {side:?}",
        value_sign,
        info.value.abs()
    ));
    lines.push(format!("total visits: {total}"));
    lines.push("top moves:".to_string());
    for (i, (mv, n)) in info.visits.iter().take(3).enumerate() {
        let share = if total > 0 {
            *n as f32 / total as f32
        } else {
            0.0
        };
        lines.push(format!(
            "  {}. {} {:>5.1}%",
            i + 1,
            format_move(*mv),
            share * 100.0
        ));
    }

    let stale = match current_config {
        None => true,
        Some(cfg) => cfg != &info.config,
    };
    if stale {
        lines.push(format!("(!searched at sims={})", info.config.simulations));
    }

    lines
}

/// The current effective AI configuration for the seat that searched
/// the cursor position, if that seat is still AI.
///
/// This is a small helper so the renderer does not reach into the
/// occupant enum twice.
#[must_use]
pub fn searching_seat_config<'a>(core: &'a SessionCore, info: &SearchInfo) -> Option<&'a AiConfig> {
    match core.occupant(info.side_to_move) {
        crate::actor::Occupant::Ai(cfg) => Some(cfg),
        crate::actor::Occupant::Human => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Color, Move};

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    fn dummy_config(simulations: u32) -> AiConfig {
        AiConfig {
            simulations,
            ..AiConfig::default()
        }
    }

    fn search_info_with_visits(
        visits: &[(Move, u32)],
        prior: &[(Move, f32)],
        value: f32,
        side_to_move: Color,
    ) -> SearchInfo {
        SearchInfo {
            side_to_move,
            visits: visits.to_vec(),
            prior: prior.to_vec(),
            value,
            config: dummy_config(400),
        }
    }

    #[test]
    fn value_strip_formats_sign_and_color() {
        let info =
            search_info_with_visits(&[(mv(7, 7), 300), (mv(6, 6), 100)], &[], 0.42, Color::White);
        let lines = value_strip(Some(&info), Some(&dummy_config(400)));
        assert_eq!(lines[0], "value +0.42 for White");
        assert!(lines.iter().any(|l| l.contains("total visits: 400")));
        assert!(lines.iter().any(|l| l.contains("75.0%")));
    }

    #[test]
    fn value_strip_negative_value_keeps_sign() {
        let info = search_info_with_visits(&[(mv(7, 7), 100)], &[], -0.17, Color::Black);
        let lines = value_strip(Some(&info), Some(&dummy_config(400)));
        assert_eq!(lines[0], "value -0.17 for Black");
    }

    #[test]
    fn value_strip_appends_staleness_when_config_differs() {
        let info = search_info_with_visits(&[(mv(7, 7), 100)], &[], 0.0, Color::Black);
        let current = dummy_config(800);
        let lines = value_strip(Some(&info), Some(&current));
        assert!(lines.iter().any(|l| l.contains("(!searched at sims=400)")));
    }

    #[test]
    fn value_strip_is_stale_when_seat_is_human() {
        let info = search_info_with_visits(&[(mv(7, 7), 100)], &[], 0.0, Color::Black);
        let lines = value_strip(Some(&info), None);
        assert!(lines.iter().any(|l| l.contains("(!searched at sims=400)")));
    }

    #[test]
    fn value_strip_no_search_info_returns_empty() {
        let lines = value_strip(None, None);
        assert!(lines.is_empty());
    }
}
