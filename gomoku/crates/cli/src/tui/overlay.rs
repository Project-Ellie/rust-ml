//! Search-info overlays — pure functions, no I/O.
//!
//! Two overlays read a cursor node's [`SearchInfo`]. [`value_strip`]
//! turns it into the lines of the side-panel value strip;
//! [`overlay_cell`] — with [`heatmap_bg`] and the two
//! `render_overlay_cell_*` functions — turns one board cell into a
//! heatmapped empty-cell glyph. Both are deliberately testable without
//! a real MCTS search: callers can construct [`SearchInfo`] directly.

use crate::actor::{AiConfig, SearchInfo};
use crate::fmt::format_move;
use crate::session::SessionCore;
use engine::{Color, Move, Proof};

/// Active cell-overlay mode for the board heatmap.
///
/// The design paper's rule: cell overlays exclude each other, so at most
/// one mode paints the empty cells at a time. Every mode is a view of the
/// same stored search; none of them changes the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PolicyOverlay {
    /// No overlay; show the bare board
    #[default]
    Off,
    /// Heatmap by share of total visits
    Visits,
    /// Heatmap by relative policy prior
    ///
    /// The evaluator's opinion before any simulation. Intensity is the
    /// prior divided by the maximum prior, so the brightest cell is always
    /// the evaluator's favourite; the label is the raw percentage.
    Prior,
}

impl PolicyOverlay {
    /// Advance to the next mode: `Off → Visits → Prior → Off`.
    ///
    /// The cycle lives here rather than in the key handler, so one key can
    /// keep meaning "cycle" as modes are added (the prior view joins the
    /// walk in sub-chapter 06c).
    #[must_use]
    pub fn cycle(self) -> Self {
        match self {
            PolicyOverlay::Off => Self::Visits,
            PolicyOverlay::Visits => Self::Prior,
            PolicyOverlay::Prior => Self::Off,
        }
    }

    /// The lowercase word the message line shows after a cycle.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            PolicyOverlay::Off => "off",
            PolicyOverlay::Visits => "visits",
            PolicyOverlay::Prior => "prior",
        }
    }
}

/// What one board cell shows under a policy overlay.
///
/// The decision (is this cell hot? what text does it carry?) is kept
/// separate from its presentation, so the styled and plain renderers
/// share one computation and a test can assert the numbers without a
/// terminal.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayCell {
    /// Background intensity in 0.0–1.0, used to select the ANSI-256 colour.
    pub intensity: f32,
    /// Visible text for hot cells; empty for dim cells.
    pub text: String,
    /// Whether this cell is hot enough to print its value.
    pub is_hot: bool,
}

/// Compute the overlay data for `mv` under `mode`.
///
/// Returns `None` in [`PolicyOverlay::Off`] mode, and also when the stored
/// search holds nothing to paint with: no visits at all, or a prior vector
/// whose maximum is zero. A cell with nothing to show must render as a bare
/// empty cell, not as a dimmed one. A move that is absent from the visit
/// distribution or the prior vector is the coldest cell (intensity `0.0`,
/// no text), because both vectors may be partial — absent means unvisited,
/// not unknown.
#[must_use]
pub fn overlay_cell(info: &SearchInfo, mv: Move, mode: PolicyOverlay) -> Option<OverlayCell> {
    match mode {
        PolicyOverlay::Off => None,
        PolicyOverlay::Visits => visits_cell(info, mv),
        PolicyOverlay::Prior => prior_cell(info, mv),
    }
}

fn prior_cell(info: &SearchInfo, mv: Move) -> Option<OverlayCell> {
    let max_prior = info
        .prior
        .iter()
        .map(|(_, p)| p)
        .fold(0.0f32, |a, &b| a.max(b));
    if max_prior == 0.0 {
        return None;
    }
    let prior = info
        .prior
        .iter()
        .find(|(m, _)| *m == mv)
        .map(|(_, p)| *p)
        .unwrap_or(0.0);
    let intensity = prior / max_prior;
    let is_hot = intensity >= 0.1;
    let text = if is_hot {
        format!("{:.0}%", prior * 100.0)
    } else {
        String::new()
    };
    Some(OverlayCell {
        intensity,
        is_hot,
        text,
    })
}

/// The visits-mode computation: share of total visits, hot from 10%.
fn visits_cell(info: &SearchInfo, mv: Move) -> Option<OverlayCell> {
    let total: u32 = info.visits.iter().map(|(_, n)| n).sum();
    if total == 0 {
        return None;
    }
    let visits = info
        .visits
        .iter()
        .find(|(m, _)| *m == mv)
        .map(|(_, n)| *n)
        .unwrap_or(0);
    let share = visits as f32 / total as f32;
    let is_hot = share >= 0.1;
    let text = if is_hot {
        visits.to_string()
    } else {
        String::new()
    };
    Some(OverlayCell {
        intensity: share,
        text,
        is_hot,
    })
}

/// Which side a move in a TSS line belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TssSide {
    /// The side that can force the win.
    Attacker,
    /// The opposite side, forced to block.
    Defender,
}

/// A verified TSS proof turned into per-cell render data.
///
/// The overlay numbers cells in the order they appear in the proof line.
/// Attacker moves render plain; defender moves render bracketed.
#[derive(Debug, Clone)]
pub struct TssOverlay<'a> {
    proof: &'a Proof,
    cells: [Option<TssOverlayCell>; 225],
}

#[derive(Debug, Clone, Copy)]
struct TssOverlayCell {
    number: usize,
    side: TssSide,
}

impl<'a> TssOverlay<'a> {
    /// Build an overlay from a verified proof.
    #[must_use]
    pub fn new(proof: &'a Proof) -> Self {
        let mut cells = [None; 225];
        for (i, &mv) in proof.line.iter().enumerate() {
            let side = if i % 2 == 0 {
                TssSide::Attacker
            } else {
                TssSide::Defender
            };
            cells[mv.index()] = Some(TssOverlayCell {
                number: i + 1,
                side,
            });
        }
        Self { proof, cells }
    }

    /// The underlying proof.
    #[must_use]
    pub fn proof(&self) -> &'a Proof {
        self.proof
    }

    /// Look up the overlay data for one cell.
    #[must_use]
    pub fn get(&self, mv: Move) -> Option<(usize, TssSide)> {
        self.cells[mv.index()].map(|c| (c.number, c.side))
    }
}

/// Verdict text for a verified proof overlay.
#[must_use]
pub fn tss_verdict(proof: &Proof) -> String {
    format!(
        "certified forced win for {:?} in {} ply",
        proof.winner,
        proof.line.len()
    )
}

/// Render one plain TSS overlay cell.
///
/// Numbers stay single-digit because the CLI budget caps `max_depth` at 9.
#[must_use]
pub fn render_tss_cell_plain(n: usize, side: TssSide) -> String {
    debug_assert!(n < 10, "single-digit overlay numbers (max_depth = 9)");
    match side {
        TssSide::Attacker => format!(" {n} "),
        TssSide::Defender => format!("[{n}]"),
    }
}

/// Render one styled TSS overlay cell.
#[must_use]
pub fn render_tss_cell_styled(n: usize, side: TssSide, winner: Color) -> String {
    debug_assert!(n < 10, "single-digit overlay numbers (max_depth = 9)");
    let s = n.to_string();
    let color = match side {
        TssSide::Attacker => winner,
        TssSide::Defender => winner.other(),
    };
    // Raw SGR escapes, like the policy renderer above: overlay.rs
    // stays crossterm-free. The winner's side is cyan and bold, the
    // loser yellow — the legacy UI's convention.
    let styled = match color {
        Color::Black => format!("\x1b[1;36m{s}\x1b[0m"),
        Color::White => format!("\x1b[33m{s}\x1b[0m"),
    };
    match side {
        TssSide::Attacker => format!(" {styled} "),
        TssSide::Defender => format!("[{styled}]"),
    }
}

/// Map an intensity 0.0–1.0 to an ANSI-256 background colour.
///
/// This is the same discrete ramp used by `patterns::render_heatmap`:
/// hot values glow red→orange→yellow, lower values fade to light grey,
/// and the near-zero dim band uses a muted grey. The ramp is deliberately
/// copied here so the `cli` crate can stay free of the `patterns`
/// dependency.
#[must_use]
pub fn heatmap_bg(intensity: f32) -> u8 {
    match (intensity * 10.0).round() as i32 {
        10 => 196,
        9 => 208,
        8 => 220,
        3 => 39,
        _ => 250,
    }
}

/// Render one styled cell with an ANSI-256 background.
/// Render one styled cell: an ANSI-256 background plus three characters.
///
/// The background encodes the intensity; a hot cell puts its visit count
/// in the three-character slot, a cold cell its empty-cell glyph. Every
/// cell ends with a reset sequence, so a background never bleeds into the
/// next cell.
#[must_use]
pub fn render_overlay_cell_styled(glyph: char, cell: &OverlayCell) -> String {
    let bg = heatmap_bg(cell.intensity);
    let content = cell_content(glyph, cell);
    format!("\x1b[48;5;{bg}m{content}\x1b[0m")
}

/// Render one plain cell (no ANSI escapes).
/// Render one plain cell: the same three characters, no escape sequences.
///
/// The plain twin serves pipes, CI logs, and the line-mode fallback. It
/// drops the colour, which is decoration, but keeps the visit count, which
/// is information.
#[must_use]
pub fn render_overlay_cell_plain(glyph: char, cell: &OverlayCell) -> String {
    cell_content(glyph, cell)
}

/// The three characters inside a cell: the visit count when hot, the
/// empty-cell glyph otherwise.
fn cell_content(glyph: char, cell: &OverlayCell) -> String {
    if cell.is_hot {
        pad_numeric_cell(&cell.text)
    } else {
        format!(" {} ", glyph)
    }
}

/// Fit a visit count into the three-character cell.
///
/// Cells are exactly three characters wide or the board's columns drift.
/// A count longer than three digits truncates instead of breaking the
/// grid; at 400 visits a single move never gets that hot in practice.
fn pad_numeric_cell(text: &str) -> String {
    if text.len() >= 3 {
        text[..3].to_string()
    } else {
        format!("{:>3}", text)
    }
}

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

    #[test]
    fn cycle_order_is_off_visits_prior_off() {
        assert_eq!(PolicyOverlay::Off.cycle(), PolicyOverlay::Visits);
        assert_eq!(PolicyOverlay::Visits.cycle(), PolicyOverlay::Prior);
        assert_eq!(PolicyOverlay::Prior.cycle(), PolicyOverlay::Off);
    }

    #[test]
    fn overlay_off_returns_none() {
        let info = search_info_with_visits(&[(mv(7, 7), 100)], &[], 0.0, Color::Black);
        assert!(overlay_cell(&info, mv(7, 7), PolicyOverlay::Off).is_none());
    }

    #[test]
    fn zero_visits_gives_zero_intensity_and_not_hot() {
        let info = search_info_with_visits(&[(mv(7, 7), 100)], &[], 0.0, Color::Black);
        let cell = overlay_cell(&info, mv(0, 0), PolicyOverlay::Visits).unwrap();
        assert_eq!(cell.intensity, 0.0);
        assert!(!cell.is_hot);
        assert!(cell.text.is_empty());
    }

    #[test]
    fn single_hot_move_has_full_intensity() {
        let info = search_info_with_visits(&[(mv(7, 7), 400)], &[], 0.5, Color::Black);
        let cell = overlay_cell(&info, mv(7, 7), PolicyOverlay::Visits).unwrap();
        assert_eq!(cell.intensity, 1.0);
        assert!(cell.is_hot);
        assert_eq!(cell.text, "400");
    }

    #[test]
    fn visit_share_computes_intensity_correctly() {
        let info =
            search_info_with_visits(&[(mv(7, 7), 300), (mv(6, 6), 100)], &[], 0.0, Color::Black);
        let hot = overlay_cell(&info, mv(7, 7), PolicyOverlay::Visits).unwrap();
        assert!((hot.intensity - 0.75).abs() < 1e-6);
        assert!(hot.is_hot);

        let warm = overlay_cell(&info, mv(6, 6), PolicyOverlay::Visits).unwrap();
        assert!((warm.intensity - 0.25).abs() < 1e-6);
        assert!(warm.is_hot); // 25% is above the 10% threshold
    }

    #[test]
    fn small_visit_share_is_dim() {
        let info =
            search_info_with_visits(&[(mv(7, 7), 390), (mv(6, 6), 10)], &[], 0.0, Color::Black);
        let dim = overlay_cell(&info, mv(6, 6), PolicyOverlay::Visits).unwrap();
        assert_eq!(dim.intensity, 0.025);
        assert!(!dim.is_hot);
    }
    #[test]
    fn a_share_of_exactly_ten_percent_is_hot() {
        // The threshold is inclusive: 10% of the visits is hot.
        let info =
            search_info_with_visits(&[(mv(7, 7), 900), (mv(6, 6), 100)], &[], 0.0, Color::Black);
        let boundary = overlay_cell(&info, mv(6, 6), PolicyOverlay::Visits).unwrap();
        assert_eq!(boundary.intensity, 0.1);
        assert!(boundary.is_hot);
    }

    #[test]
    fn styled_hot_cell_has_ansi_background() {
        let cell = OverlayCell {
            intensity: 1.0,
            text: "400".to_string(),
            is_hot: true,
        };
        let out = render_overlay_cell_styled('.', &cell);
        assert!(out.contains("\x1b[48;5;196m"));
        assert!(out.contains("400"));
        assert!(out.contains("\x1b[0m"));
    }

    #[test]
    fn plain_hot_cell_has_no_ansi_escapes() {
        let cell = OverlayCell {
            intensity: 1.0,
            text: "400".to_string(),
            is_hot: true,
        };
        let out = render_overlay_cell_plain('.', &cell);
        assert!(!out.contains('\x1b'));
        assert_eq!(out, "400");
    }

    #[test]
    fn prior_normalises_by_max_prior() {
        let info =
            search_info_with_visits(&[], &[(mv(7, 7), 0.5), (mv(6, 6), 0.25)], 0.0, Color::Black);
        let best = overlay_cell(&info, mv(7, 7), PolicyOverlay::Prior).unwrap();
        assert_eq!(best.intensity, 1.0);
        assert!(best.is_hot);
        assert_eq!(best.text, "50%");

        let half = overlay_cell(&info, mv(6, 6), PolicyOverlay::Prior).unwrap();
        assert_eq!(half.intensity, 0.5);
        assert!(half.is_hot);
        assert_eq!(half.text, "25%");
    }

    #[test]
    fn below_threshold_prior_is_dim() {
        let info =
            search_info_with_visits(&[], &[(mv(7, 7), 0.5), (mv(6, 6), 0.04)], 0.0, Color::Black);
        let dim = overlay_cell(&info, mv(6, 6), PolicyOverlay::Prior).unwrap();
        assert_eq!(dim.intensity, 0.08);
        assert!(!dim.is_hot);
        assert!(dim.text.is_empty());
    }

    #[test]
    fn a_normalized_prior_of_exactly_ten_percent_is_hot() {
        // The threshold is inclusive, as in the visits path: 0.05 /
        // 0.5 = 0.10 normalized is hot.
        let info =
            search_info_with_visits(&[], &[(mv(7, 7), 0.5), (mv(6, 6), 0.05)], 0.0, Color::Black);
        let boundary = overlay_cell(&info, mv(6, 6), PolicyOverlay::Prior).unwrap();
        assert_eq!(boundary.intensity, 0.1);
        assert!(boundary.is_hot);
    }

    #[test]
    fn empty_prior_list_gives_no_overlay_cells() {
        let info = search_info_with_visits(&[(mv(7, 7), 100)], &[], 0.0, Color::Black);
        assert!(overlay_cell(&info, mv(7, 7), PolicyOverlay::Prior).is_none());
    }

    #[test]
    fn zero_max_prior_gives_no_overlay_cells() {
        let info = search_info_with_visits(&[], &[(mv(7, 7), 0.0)], 0.0, Color::Black);
        assert!(overlay_cell(&info, mv(7, 7), PolicyOverlay::Prior).is_none());
    }

    #[test]
    fn prior_for_missing_move_is_zero_intensity() {
        let info = search_info_with_visits(&[], &[(mv(7, 7), 0.5)], 0.0, Color::Black);
        let cell = overlay_cell(&info, mv(0, 0), PolicyOverlay::Prior).unwrap();
        assert_eq!(cell.intensity, 0.0);
        assert!(!cell.is_hot);
        assert!(cell.text.is_empty());
    }

    // -------------------------------------------------------- TSS overlay

    #[test]
    fn tss_overlay_numbers_attacker_and_defender_cells() {
        let proof = Proof {
            winner: Color::Black,
            line: vec![mv(7, 7), mv(6, 7), mv(5, 5)],
        };
        let overlay = TssOverlay::new(&proof);

        let first = overlay.get(mv(7, 7)).unwrap();
        assert_eq!(first, (1, TssSide::Attacker));

        let second = overlay.get(mv(6, 7)).unwrap();
        assert_eq!(second, (2, TssSide::Defender));

        let third = overlay.get(mv(5, 5)).unwrap();
        assert_eq!(third, (3, TssSide::Attacker));

        assert!(overlay.get(mv(0, 0)).is_none());
    }

    #[test]
    fn tss_verdict_names_winner_and_length() {
        let proof = Proof {
            winner: Color::White,
            line: vec![mv(7, 7), mv(6, 7)],
        };
        assert_eq!(
            tss_verdict(&proof),
            "certified forced win for White in 2 ply"
        );
    }

    #[test]
    fn tss_plain_cells_follow_the_old_convention() {
        assert_eq!(render_tss_cell_plain(3, TssSide::Attacker), " 3 ");
        assert_eq!(render_tss_cell_plain(2, TssSide::Defender), "[2]");
    }

    #[test]
    fn tss_styled_attacker_uses_winner_color() {
        let out = render_tss_cell_styled(1, TssSide::Attacker, Color::Black);
        assert!(out.contains('\x1b'));
        assert!(out.contains('1'));
    }

    #[test]
    fn tss_styled_defender_uses_winner_other_color() {
        let out = render_tss_cell_styled(1, TssSide::Defender, Color::Black);
        assert!(out.contains('\x1b'));
        assert!(out.contains('1'));
    }
}
