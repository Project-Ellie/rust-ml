//! Study-table rendering — pure functions, no I/O.
//!
//! One frame of the study table is the board on the left, a side panel on
//! the right, and an optional command line at the bottom. [`render_study`]
//! builds that frame from a [`StudyView`]: session state and display
//! options in, display lines out. Nothing here prints. The caller decides
//! where the lines go: the alternate screen, a pipe, or a test assertion.
//!
//! Styling is a runtime parameter, not a compile-time feature flag. The
//! same session must render to the full-screen shell (ANSI styling through
//! crossterm) and to plain pipes and CI logs (no escape sequences). That
//! choice belongs to the screen, so the parameter is part of the view.

use crate::actor::{Occupant, SearchInfo};
use crate::command::{Event, OpeningPhase};
use crate::notation::algebraic;
use crate::opening::OpeningSnapshot;
use crate::session::{SessionCore, Snapshot};
use crate::tui::overlay::{
    PolicyOverlay, TssOverlay, overlay_cell, render_overlay_cell_plain, render_overlay_cell_styled,
    render_tss_cell_plain, render_tss_cell_styled, searching_seat_config, value_strip,
};
use crossterm::style::Stylize;
use engine::{Board, Color, Move, Proof, Status};

/// The zero-based rows and columns of the nine star points: 4, 8, and 12
/// in one-based notation. An empty cell on a star point renders as `+`
/// instead of `.`.
const HOSHI: [u8; 3] = [3, 7, 11];

/// Everything one frame of the study table needs.
///
/// The constructor takes the two required inputs, the session and the
/// recent events. The cursor, the command-line buffer, and styling are
/// optional and have chainable setters, so a caller states only what it
/// has:
///
/// ```
/// # use cli::session::{SessionCore, SessionInit};
/// # use cli::tui::render::{render_study, StudyView};
/// let core = SessionCore::new(SessionInit::new_hotseat());
/// // The line-mode fallback asks for exactly this: no cursor, no
/// // command line, no styling.
/// let frame = render_study(&StudyView::new(&core, &[]));
/// assert_eq!(frame.len(), 19);
/// ```
///
/// The struct exists because a frame has five inputs and three of them
/// are optional. Five positional parameters would force every call site
/// to spell out `None, None, false`, and a bare `true` or `false` at the
/// end of a call does not say what it switches.
pub struct StudyView<'a> {
    core: &'a SessionCore,
    recent_events: &'a [Event],
    cursor: Option<Move>,
    command_line: Option<&'a str>,
    message: Option<&'a str>,
    styled: bool,
    show_value_strip: bool,
    policy_overlay: PolicyOverlay,
    tss_proof: Option<&'a Proof>,
}

impl<'a> StudyView<'a> {
    /// A plain view of the session: no cursor, no command line, no
    /// styling.
    #[must_use]
    pub fn new(core: &'a SessionCore, recent_events: &'a [Event]) -> Self {
        Self {
            core,
            recent_events,
            cursor: None,
            command_line: None,
            message: None,
            styled: false,
            show_value_strip: false,
            policy_overlay: PolicyOverlay::Off,
            tss_proof: None,
        }
    }

    /// Highlight `mv` as the board cursor.
    ///
    /// The cursor cell gets angle brackets (`<x>`); a styled frame adds
    /// reverse video on top. When the cursor is on the last move, the cursor
    /// marker takes precedence, and the last-move brackets are not drawn on
    /// that cell.
    #[must_use]
    pub fn cursor(mut self, mv: Move) -> Self {
        self.cursor = Some(mv);
        self
    }

    /// Show `buffer` as the bottom line of the frame, behind a `:` prompt.
    ///
    /// The buffer is the text the user typed after opening the command
    /// line. The renderer adds the prompt and applies no styling, so the
    /// text is shown verbatim in both modes.
    #[must_use]
    pub fn command_line(mut self, buffer: &'a str) -> Self {
        self.command_line = Some(buffer);
        self
    }

    /// Show `text` as the bottom line of the frame.
    ///
    /// One bottom line exists. While the command line is open it shows
    /// the buffer and the message is not drawn; in normal mode the
    /// message carries the last event summary or the last error text.
    /// The line is never styled: it is feedback text, not furniture.
    #[must_use]
    pub fn message(mut self, text: &'a str) -> Self {
        self.message = Some(text);
        self
    }

    /// Turn ANSI styling on (`true`) or off (`false`).
    ///
    /// Unstyled output contains no escape sequences, so it is safe to pipe
    /// into files and to compare in tests.
    #[must_use]
    pub fn styled(mut self, styled: bool) -> Self {
        self.styled = styled;
        self
    }

    /// Show the value strip in the side panel.
    ///
    /// The strip shows the search info at the cursor position, or an
    /// explanatory line when the cursor has no stored search.
    #[must_use]
    pub fn show_value_strip(mut self, show: bool) -> Self {
        self.show_value_strip = show;
        self
    }

    /// Set the board heatmap overlay.
    ///
    /// `Off` leaves empty cells bare; `Visits` paints each empty cell with
    /// the intensity of its share of the cursor's total visits. Stones,
    /// the last move, and the cursor keep their own markers in every mode,
    /// so the overlay never hides information the board already carries.
    #[must_use]
    pub fn policy_overlay(mut self, overlay: PolicyOverlay) -> Self {
        self.policy_overlay = overlay;
        self
    }

    /// Set the TSS forced-win overlay.
    #[must_use]
    pub fn tss_proof(mut self, proof: Option<&'a Proof>) -> Self {
        self.tss_proof = proof;
        self
    }
}

/// Render one frame of the study table.
///
/// The board is 19 lines tall and the panel is usually shorter, so the
/// two are zipped line by line and the shorter side contributes empty
/// strings. The board and the snapshot are computed once and shared by
/// both halves; each is a replay of the move list, cheap at interactive
/// rates but not worth repeating inside helpers.
#[must_use]
pub fn render_study(view: &StudyView) -> Vec<String> {
    let core = view.core;
    let board = core.board();
    let snapshot = core.snapshot();
    // During a Swap2 ceremony the tree board is empty by design (the
    // placements are not game moves), so the stones to draw come from
    // the opening snapshot.
    let opening = snapshot.opening.as_ref();
    let search_info = core.search_info();
    let board_lines = study_board_lines(
        &board,
        opening,
        snapshot.last_move,
        view.cursor,
        view.styled,
        view.policy_overlay,
        view.tss_proof,
        search_info,
    );
    let panel_lines = render_side_panel(
        core,
        &board,
        &snapshot,
        view.recent_events,
        view.styled,
        view.show_value_strip,
    );
    let rows = board_lines.len().max(panel_lines.len());
    let mut out = Vec::with_capacity(rows + 1);
    for i in 0..rows {
        let left = board_lines.get(i).map(String::as_str).unwrap_or("");
        let right = panel_lines.get(i).map(String::as_str).unwrap_or("");
        out.push(format!("{left}  {right}"));
    }
    // One bottom line: the command line while it is open (the user's
    // text always at the bottom of the screen), else the message when
    // the loop has one to show.
    if let Some(buffer) = view.command_line {
        out.push(format!(":{buffer}"));
    } else if let Some(text) = view.message {
        out.push(text.to_string());
    }
    out
}

/// The board half of the frame: ruler, border, 15 rows, border, ruler —
/// 19 lines, each 53 characters wide before styling.
///
/// `last` is the most recent move, drawn with square brackets. `cursor`
/// is the shell's board cursor, drawn with angle brackets. Both are
/// optional.
#[allow(clippy::too_many_arguments)]
fn study_board_lines(
    board: &Board,
    opening: Option<&OpeningSnapshot>,
    last: Option<Move>,
    cursor: Option<Move>,
    styled: bool,
    overlay: PolicyOverlay,
    tss_proof: Option<&Proof>,
    search_info: Option<&SearchInfo>,
) -> Vec<String> {
    // TSS and policy overlays are mutually exclusive in the shell. The
    // shell enforces it, but the renderer gives TSS precedence if both
    // are somehow active: a deterministic proof overlay outranks a
    // sampled heatmap.
    let tss_overlay = tss_proof.map(TssOverlay::new);
    let mut lines = Vec::with_capacity(19);
    lines.push(style_gutter(&build_ruler(), styled));
    lines.push(style_gutter(&border(), styled));
    for row in 0..15u8 {
        let label = style_gutter(&format!("{:>2}", row + 1), styled);
        let rlabel = style_gutter(&format!("{:<2}", row + 1), styled);
        let mut line = format!("{label} |");
        for col in 0..15u8 {
            let mv = Move::new(row, col).expect("0..15 is always on board");
            let cell = study_or_policy_cell(
                board,
                opening,
                mv,
                last,
                cursor,
                styled,
                overlay,
                tss_overlay.as_ref(),
                search_info,
            );

            line.push_str(&cell);
        }
        line.push_str(&format!("| {rlabel}"));
        lines.push(line);
    }
    lines.push(style_gutter(&border(), styled));
    lines.push(style_gutter(&build_ruler(), styled));
    lines
}

/// Render one cell, using the policy overlay only for bare empty cells.
///
/// Stones, the last-move brackets, and the cursor marker are never
/// over-painted by the heatmap. The overlay paints only empty cells that
/// are neither the cursor nor the last move.
#[allow(clippy::too_many_arguments)]
fn study_or_policy_cell(
    board: &Board,
    opening: Option<&OpeningSnapshot>,
    mv: Move,
    last: Option<Move>,
    cursor: Option<Move>,
    styled: bool,
    policy_overlay: PolicyOverlay,
    tss_overlay: Option<&TssOverlay>,
    search_info: Option<&SearchInfo>,
) -> String {
    // TSS and policy are mutually exclusive; if both are active, TSS
    // wins. The shell clears one when the other is activated.
    if let Some(overlay) = tss_overlay {
        return tss_cell(board, opening, mv, last, cursor, styled, overlay);
    }

    // Ceremony placements are not in the tree board, so the board
    // lookup falls back to the placement list. The list holds at most
    // five entries; a linear scan is fast enough for a per-frame draw.
    let is_stone = stone_at(board, opening, mv).is_some();
    let is_cursor = cursor == Some(mv);
    let is_last = last == Some(mv);

    // An overlay that is off must leave the bare board untouched, dim grey
    // empty cells included, so the mode is checked before the policy path.
    if is_stone || is_cursor || is_last || policy_overlay == PolicyOverlay::Off {
        study_cell(board, opening, mv, last, cursor, styled)
    } else {
        match search_info {
            Some(info) => policy_empty_cell(mv, info, styled, policy_overlay),
            None => study_cell(board, opening, mv, last, cursor, styled),
        }
    }
}

/// Render an empty cell under an active policy overlay.
///
/// The empty-cell glyph is the one the bare board uses too (`+` on a star
/// point, `.` elsewhere), so the overlay changes only the background, never
/// the board's furniture. Callers reach this only in a mode other than
/// [`PolicyOverlay::Off`].
fn policy_empty_cell(
    mv: Move,
    info: &crate::actor::SearchInfo,
    styled: bool,
    mode: PolicyOverlay,
) -> String {
    let glyph = if HOSHI.contains(&mv.row()) && HOSHI.contains(&mv.col()) {
        '+'
    } else {
        '.'
    };
    match overlay_cell(info, mv, mode) {
        Some(cell) if styled => render_overlay_cell_styled(glyph, &cell),
        Some(cell) => render_overlay_cell_plain(glyph, &cell),
        None => format!(" {} ", glyph),
    }
}

/// Render one cell under a TSS proof overlay.
///
/// Stones, the cursor marker, and the last-move marker are never
/// over-painted. Empty cells in the proof line show the move number;
/// attacker moves are plain, defender moves are bracketed.
fn tss_cell(
    board: &Board,
    opening: Option<&OpeningSnapshot>,
    mv: Move,
    last: Option<Move>,
    cursor: Option<Move>,
    styled: bool,
    overlay: &TssOverlay,
) -> String {
    let proof = overlay.proof();
    let is_stone = stone_at(board, opening, mv).is_some();
    let is_cursor = cursor == Some(mv);
    let is_last = last == Some(mv);

    if is_stone || is_cursor || is_last {
        return study_cell(board, opening, mv, last, cursor, styled);
    }

    if let Some((n, side)) = overlay.get(mv) {
        if styled {
            return render_tss_cell_styled(n, side, proof.winner);
        }
        return render_tss_cell_plain(n, side);
    }

    study_cell(board, opening, mv, last, cursor, styled)
}

/// The stone at `mv`, from the tree board or the ceremony's placements.
///
/// Ceremony placements are not game moves, so they live in the opening
/// snapshot rather than in the board. The overlay has to see them anyway:
/// heat painted over a placed stone would report a cell as playable.
fn stone_at(board: &Board, opening: Option<&OpeningSnapshot>, mv: Move) -> Option<Color> {
    board.stone_at(mv).or_else(|| {
        opening.and_then(|opening| {
            opening
                .placements
                .iter()
                .find(|(m, _)| *m == mv)
                .map(|(_, color)| *color)
        })
    })
}

/// The column letters above and below the grid.
fn build_ruler() -> String {
    let mut ruler = String::with_capacity(53);
    ruler.push_str("    ");
    for c in 0..15u8 {
        ruler.push_str(&format!("{:^3}", (b'a' + c) as char));
    }
    ruler.push_str("    ");
    ruler
}

/// The border line between ruler and grid: 15 cells of 3 characters.
fn border() -> String {
    format!("   +{}+   ", "-".repeat(45))
}

/// Dim the frame furniture (rulers, borders, row labels) when styled.
///
/// The furniture is context, not content. Dimming the furniture keeps the
/// stones the most visible part of the screen.
fn style_gutter(text: &str, styled: bool) -> String {
    if styled {
        text.dark_grey().to_string()
    } else {
        text.to_string()
    }
}

/// One board cell, always three characters wide.
///
/// A stone renders as `x` (Black) or `o` (White). An empty cell renders
/// as `+` on a star point and `.` elsewhere. A cell can be eligible for
/// two markers at once, so the order is fixed: the cursor cell gets angle
/// brackets, else the last-move cell gets square brackets, else the cell
/// is padded with spaces. The cursor takes precedence because it is where
/// the user's next action applies. The glyph keeps its color under the
/// cursor in a styled frame, so the stone information is not lost.
fn study_cell(
    board: &Board,
    opening: Option<&OpeningSnapshot>,
    mv: Move,
    last: Option<Move>,
    cursor: Option<Move>,
    styled: bool,
) -> String {
    let stone = stone_at(board, opening, mv);
    let glyph = match stone {
        Some(Color::Black) => "x",
        Some(Color::White) => "o",
        None if HOSHI.contains(&mv.row()) && HOSHI.contains(&mv.col()) => "+",
        None => ".",
    };
    let is_cursor = cursor == Some(mv);
    let body = if styled {
        let styled_glyph = match stone {
            Some(Color::Black) => glyph.cyan().bold(),
            Some(Color::White) => glyph.yellow(),
            None => glyph.dark_grey(),
        };
        // Reverse video marks the cursor without changing the glyph's own
        // color, so a cursor on a stone keeps the stone's color.
        if is_cursor {
            styled_glyph.reverse().to_string()
        } else {
            styled_glyph.to_string()
        }
    } else {
        glyph.to_string()
    };
    if is_cursor {
        format!("<{body}>")
    } else if Some(mv) == last {
        format!("[{body}]")
    } else {
        format!(" {body} ")
    }
}

/// The side panel: seat table, then the phase-dependent middle block
/// (the ceremony during a Swap2 opening, the game state afterwards),
/// then recent events and the key hints.
fn render_side_panel(
    core: &SessionCore,
    board: &Board,
    snapshot: &Snapshot,
    recent_events: &[Event],
    styled: bool,
    show_value_strip: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(panel_line("-- Study table --", styled));
    lines.push(format!("Black: {}", seat_label(core.black())));
    lines.push(format!("White: {}", seat_label(core.white())));
    lines.push(String::new());
    // The middle block is phase-dependent: during the ceremony the tree
    // is empty, so ply and side-to-move would describe a game that has
    // not started. The ceremony block takes their place.
    match &snapshot.opening {
        Some(opening) => ceremony_lines(&mut lines, opening),
        None => position_lines(&mut lines, board, snapshot),
    }
    if show_value_strip {
        lines.push(String::new());
        lines.push("Value strip:".to_string());
        if let Some(info) = core.search_info() {
            let current_config = searching_seat_config(core, info);
            lines.extend(value_strip(Some(info), current_config));
        } else {
            lines.push("  (no search here yet)".to_string());
        }
    }
    lines.push(String::new());
    lines.push("Last events:".to_string());
    if recent_events.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        // The five most recent events, oldest first: the latest event is
        // the bottom line of the list.
        for ev in recent_events.iter().rev().take(5).rev() {
            lines.push(format!("  {ev}"));
        }
    }
    lines.push(String::new());
    lines.push(panel_line("? help · q quit", styled));
    lines
}

/// The game-state block of the side panel: ply, side to move or game
/// result, path tail, and branch options.
fn position_lines(lines: &mut Vec<String>, board: &Board, snapshot: &Snapshot) {
    lines.push(format!("Ply: {}", snapshot.ply));
    // One line covers both states a game can be in: waiting for a side,
    // or finished with a result.
    match snapshot.status {
        Status::Ongoing => lines.push(format!("To move: {:?}", board.to_move())),
        Status::Won(color) => lines.push(format!("Game over: {color:?} wins")),
        Status::Draw => lines.push("Game over: draw".to_string()),
    }
    lines.push(format!("Path: {}", path_tail_string(board)));
    // One continuation is the main line, not a choice. The panel lists
    // options only when the cursor is at a real branch point.
    if snapshot.branch_options.len() > 1 {
        let opts: Vec<String> = snapshot
            .branch_options
            .iter()
            // The numbers are the 0-based index the `var` command takes,
            // so what the user reads is what the user types.
            .map(|(i, m)| format!("[{i}] {}", algebraic(*m)))
            .collect();
        lines.push(format!(
            "{} variations: {}",
            snapshot.branch_options.len(),
            opts.join(" ")
        ));
    }
}

/// The ceremony block of the side panel: the phase, the placements,
/// the color of the next stone at a placement phase, and the keys that
/// apply now.
///
/// The panel teaches the phase-dependent keys at the point where the
/// user needs them: a placement phase shows the placement keys, a
/// choice phase shows the choice keys. The guidance strings match the
/// key table in the help screen word for word.
fn ceremony_lines(lines: &mut Vec<String>, opening: &OpeningSnapshot) {
    lines.push(format!("Swap2: {:?}", opening.phase));
    let stones = if opening.placements.is_empty() {
        "(none)".to_string()
    } else {
        opening
            .placements
            .iter()
            .map(|(mv, color)| format!("{}:{color:?}", algebraic(*mv)))
            .collect::<Vec<_>>()
            .join(" ")
    };
    lines.push(format!("Stones: {stones}"));
    match opening.phase {
        OpeningPhase::Placing3 | OpeningPhase::Placing2 => {
            // The strict alternation fixes the next stone's color, so
            // the panel states it without a source: there is nothing
            // to select. The snapshot guarantees a color at a
            // placement phase.
            let color = opening
                .next_color
                .expect("a placement phase always has a next color");
            lines.push(format!("Next: {color:?}"));
            lines.push("Enter place · u undo · d done".to_string());
        }
        OpeningPhase::FirstChoice => {
            // A choice phase has no next stone, so there is no `Next:`
            // line; showing a default here would report a stone that
            // does not exist.
            lines.push("b black · w white · m more".to_string());
        }
        OpeningPhase::FinalChoice => {
            lines.push("b black · w white".to_string());
        }
    }
}

/// A panel line, dimmed when styled.
fn panel_line(text: &str, styled: bool) -> String {
    if styled {
        text.dark_grey().to_string()
    } else {
        text.to_string()
    }
}

/// How a seat appears in the panel: `Human`, or `AI(N)` with the seat's
/// simulation count.
fn seat_label(occupant: &Occupant) -> String {
    match occupant {
        Occupant::Human => "Human".to_string(),
        Occupant::Ai(cfg) => format!("AI({})", cfg.simulations),
    }
}

/// The tail of the current line, in algebraic notation.
///
/// The full path can grow past the panel's width, so only the last eight
/// moves are shown, prefixed with `...` when earlier moves are elided.
fn path_tail_string(board: &Board) -> String {
    let moves = board.moves();
    if moves.is_empty() {
        return "(root)".to_string();
    }
    let tail: Vec<String> = moves
        .iter()
        .rev()
        .take(8)
        .rev()
        .map(|m| algebraic(*m))
        .collect();
    if moves.len() > tail.len() {
        format!("... {}", tail.join(" "))
    } else {
        tail.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::AiConfig;
    use crate::command::Command;
    use crate::session::SessionInit;
    use testkit::mv;

    fn play(core: &mut SessionCore, r: u8, c: u8) {
        core.execute(Command::Play { mv: mv(r, c) }).unwrap();
    }

    /// Remove ANSI escape sequences from a line.
    ///
    /// Crossterm emits only CSI sequences: ESC '[', parameter bytes, and
    /// one final byte in '@'..='~'. The equivalence test compares the
    /// styled and unstyled frames after this normalization.
    fn strip_ansi(line: &str) -> String {
        let mut out = String::new();
        let mut chars = line.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                assert_eq!(chars.next(), Some('['), "expected a CSI sequence");
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn star_points_appear_on_empty_cells() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        let frame = render_study(&StudyView::new(&core, &[]));
        // Frame lines 0 and 1 are the ruler and the border, so board row 7
        // is line 9. Row 7 is a star-point row: an empty board shows
        // " + " at columns d, h, and l.
        let row7 = &frame[2 + 7];
        assert!(
            row7.matches(" + ").count() >= 3,
            "expected star points on row 7, got: {row7}"
        );
    }

    #[test]
    fn the_last_move_is_bracketed() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        play(&mut core, 7, 7); // h8, a star point
        play(&mut core, 7, 8); // i8
        let frame = render_study(&StudyView::new(&core, &[]));
        let row7 = &frame[2 + 7];
        // Four gutter bytes, then three bytes per column: h8 starts at
        // byte 25. The stone covers the star point, and only the last
        // move gets brackets.
        assert_eq!(&row7[25..28], " x ");
        assert_eq!(&row7[28..31], "[o]");
    }

    #[test]
    fn the_panel_shows_seats_ply_and_side_to_move() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        let frame = render_study(&StudyView::new(&core, &[]));
        let text = frame.join("\n");
        assert!(text.contains("Black: Human"), "panel shows Black seat");
        assert!(text.contains("White: Human"), "panel shows White seat");
        assert!(text.contains("Ply: 0"), "panel shows the initial ply");
        assert!(
            text.contains("To move: Black"),
            "panel shows the side to move"
        );

        play(&mut core, 7, 7);
        let frame = render_study(&StudyView::new(&core, &[]));
        let text = frame.join("\n");
        assert!(text.contains("Ply: 1"), "panel updates the ply");
        assert!(text.contains("To move: White"), "panel updates the side");
        assert!(text.contains("h8"), "path tail shows the move");
    }

    #[test]
    fn the_panel_shows_the_game_result() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        // Black completes five on row 8; White parks on row 1.
        let script = [
            (7, 3),
            (0, 1),
            (7, 4),
            (0, 2),
            (7, 5),
            (0, 3),
            (7, 6),
            (0, 4),
            (7, 7),
        ];
        for &(r, c) in &script {
            play(&mut core, r, c);
        }
        let frame = render_study(&StudyView::new(&core, &[]));
        let text = frame.join("\n");
        assert!(text.contains("Game over: Black wins"));
        // A finished game has no side to move.
        assert!(!text.contains("To move:"));
    }

    #[test]
    fn the_panel_lists_branch_options() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        // Main line h8 h7; back one and branch to i8, then return to the
        // branch point.
        play(&mut core, 7, 7);
        play(&mut core, 6, 7);
        core.execute(Command::Back { n: 1 }).unwrap();
        play(&mut core, 7, 8);
        core.execute(Command::Back { n: 1 }).unwrap();

        let frame = render_study(&StudyView::new(&core, &[]));
        let text = frame.join("\n");
        assert!(
            text.contains("2 variations:"),
            "panel lists the branches, got: {text}"
        );
        // The numbers are the 0-based index the `var` command takes.
        assert!(text.contains("[0] h7"), "panel shows the first branch");
        assert!(text.contains("[1] i8"), "panel shows the second branch");
    }

    #[test]
    fn the_panel_lists_recent_events() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        let events = core.execute(Command::Play { mv: mv(7, 7) }).unwrap();
        let frame = render_study(&StudyView::new(&core, &events));
        let text = frame.join("\n");
        assert!(text.contains("Last events:"));
        assert!(text.contains("Moved h8 by Human"));

        let frame = render_study(&StudyView::new(&core, &[]));
        let text = frame.join("\n");
        assert!(text.contains("  (none)"), "no events reads as (none)");
    }

    #[test]
    fn the_cursor_is_marked_with_angle_brackets() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        // On an empty board the cursor marker surrounds the cell's own
        // glyph; h8 is a star point.
        let frame = render_study(&StudyView::new(&core, &[]).cursor(mv(7, 7)));
        let row7 = &frame[2 + 7];
        assert_eq!(&row7[25..28], "<+>");

        // On the last move the cursor marker takes precedence over the
        // brackets.
        play(&mut core, 7, 7);
        let frame = render_study(&StudyView::new(&core, &[]).cursor(mv(7, 7)));
        let text = frame.join("\n");
        assert!(text.contains("<x>"), "cursor on a stone shows the stone");
        assert!(
            !text.contains("[x]"),
            "the cursor marker replaces the last-move marker"
        );
    }

    #[test]
    fn the_command_line_is_the_bottom_line() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        let bare = render_study(&StudyView::new(&core, &[]));
        let with_line = render_study(&StudyView::new(&core, &[]).command_line("play h8"));
        // The command line adds exactly one line: the buffer behind ':'.
        assert_eq!(with_line.len(), bare.len() + 1);
        assert_eq!(with_line.last().unwrap(), ":play h8");
    }

    #[test]
    fn the_ceremony_draws_its_placements_as_stones() {
        // The tree board stays empty during the ceremony by design;
        // the stones on the frame come from the opening snapshot.
        let mut core = SessionCore::new(SessionInit::new_hotseat().with_swap2(true));
        assert!(core.board().moves().is_empty());
        play(&mut core, 7, 7); // h8, Black by the alternation
        play(&mut core, 7, 8); // i8, White by the alternation

        let frame = render_study(&StudyView::new(&core, &[]));
        let row7 = &frame[2 + 7];
        // Four gutter bytes, then three bytes per column: h8 starts at
        // byte 25. Ceremony stones get no last-move brackets: the
        // marker belongs to the tree, and the tree is empty.
        assert_eq!(&row7[25..28], " x ");
        assert_eq!(&row7[28..31], " o ");
    }

    #[test]
    fn the_ceremony_panel_shows_phase_next_color_and_keys() {
        let mut core = SessionCore::new(SessionInit::new_hotseat().with_swap2(true));
        let frame = render_study(&StudyView::new(&core, &[]));
        let text = frame.join("\n");
        assert!(text.contains("Swap2: Placing3"), "the phase: {text}");
        assert!(text.contains("Stones: (none)"), "no placements yet: {text}");
        assert!(
            text.contains("Next: Black"),
            "the color of the first stone: {text}"
        );
        assert!(
            text.contains("Enter place · u undo · d done"),
            "the placement keys: {text}"
        );
        // The ceremony block replaces the game-state lines: the tree is
        // empty, so there is no side to move.
        assert!(
            !text.contains("To move:"),
            "no game state during the ceremony: {text}"
        );

        play(&mut core, 7, 7);
        let text = render_study(&StudyView::new(&core, &[])).join("\n");
        assert!(text.contains("Stones: h8:Black"), "the placements: {text}");
        assert!(
            text.contains("Next: White"),
            "the alternated color of the next stone: {text}"
        );
    }

    #[test]
    fn the_choice_phase_panel_shows_the_choice_keys_and_no_next_stone() {
        let mut core = SessionCore::new(SessionInit::new_hotseat().with_swap2(true));
        for (r, c) in [(7, 7), (8, 8), (9, 9)] {
            play(&mut core, r, c);
        }
        core.execute(Command::Done).unwrap();

        let text = render_study(&StudyView::new(&core, &[])).join("\n");
        assert!(text.contains("Swap2: FirstChoice"), "the phase: {text}");
        assert!(
            text.contains("b black · w white · m more"),
            "the first-choice keys: {text}"
        );
        // A choice phase has no next stone, so the panel must not
        // report one.
        assert!(
            !text.contains("Next:"),
            "no next stone at a choice phase: {text}"
        );
    }

    #[test]
    fn the_message_is_the_bottom_line_unless_the_command_line_is_open() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        let bare = render_study(&StudyView::new(&core, &[]));
        let with_message = render_study(&StudyView::new(&core, &[]).message("Moved h8 by Human"));
        // The message adds exactly one line, at the bottom.
        assert_eq!(with_message.len(), bare.len() + 1);
        assert_eq!(with_message.last().unwrap(), "Moved h8 by Human");

        // While the command line is open it owns the bottom line.
        let both = render_study(
            &StudyView::new(&core, &[])
                .message("Moved h8 by Human")
                .command_line("pla"),
        );
        assert_eq!(both.len(), bare.len() + 1);
        assert_eq!(both.last().unwrap(), ":pla");
    }

    #[test]
    fn styled_and_unstyled_frames_differ_only_in_escape_sequences() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        let mut events = core.execute(Command::Play { mv: mv(7, 7) }).unwrap();
        events.extend(core.execute(Command::Play { mv: mv(6, 7) }).unwrap());

        // Exercise every styled element at once: stones, star points,
        // gutters, the last move, the cursor, and the panel lines.
        let build = |styled: bool| {
            render_study(
                &StudyView::new(&core, &events)
                    .cursor(mv(3, 3))
                    .command_line("back 2")
                    .styled(styled),
            )
        };
        let styled = build(true);
        let unstyled = build(false);

        // The test is vacuous unless the styled frame really carries
        // escape sequences and the unstyled frame carries none.
        assert!(
            styled.iter().any(|l| l.contains('\x1b')),
            "the styled frame contains escape sequences"
        );
        assert!(
            unstyled.iter().all(|l| !l.contains('\x1b')),
            "the unstyled frame is plain text"
        );

        let stripped: Vec<String> = styled.iter().map(|l| strip_ansi(l)).collect();
        assert_eq!(stripped, unstyled);
    }

    #[test]
    fn the_value_strip_is_hidden_by_default() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        let frame = render_study(&StudyView::new(&core, &[]));
        let text = frame.join("\n");
        assert!(
            !text.contains("Value strip:"),
            "the strip is off by default: {text}"
        );
    }

    #[test]
    fn the_value_strip_explains_when_there_is_no_search() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        let frame = render_study(&StudyView::new(&core, &[]).show_value_strip(true));
        let text = frame.join("\n");
        assert!(text.contains("Value strip:"), "the header appears: {text}");
        assert!(
            text.contains("(no search here yet)"),
            "the no-search note appears: {text}"
        );
    }

    #[test]
    fn the_panel_shows_the_value_strip_when_enabled() {
        let mut core = SessionCore::new(SessionInit::ai_vs_ai(AiConfig::default(), 1));
        core.execute(Command::Step).unwrap();
        assert!(core.search_info().is_some(), "a step stores search info");

        let frame = render_study(&StudyView::new(&core, &[]).show_value_strip(true));
        let text = frame.join("\n");
        assert!(text.contains("Value strip:"), "the header appears: {text}");
        assert!(text.contains("value "), "the value line appears: {text}");
        assert!(
            text.contains("total visits:"),
            "the visits line appears: {text}"
        );
        assert!(
            text.contains("top moves:"),
            "the top moves line appears: {text}"
        );
    }

    #[test]
    fn policy_overlay_snapshot_is_displayed() {
        use crate::actor::{AiConfig, SearchInfo};
        use engine::{Color, Move};

        let h8 = Move::new(7, 7).unwrap();
        let i8 = Move::new(7, 8).unwrap();
        let j8 = Move::new(7, 9).unwrap();
        let info = SearchInfo {
            side_to_move: Color::Black,
            visits: vec![(h8, 300), (i8, 100)],
            prior: vec![],
            value: 0.25,
            config: AiConfig::default(),
        };

        let mut core = SessionCore::new(SessionInit::new_hotseat());
        core.execute(Command::Play { mv: j8 }).unwrap();
        core.set_search_info(info);

        let frame = render_study(&StudyView::new(&core, &[]).policy_overlay(PolicyOverlay::Visits));
        let text = frame.join("\n");

        let row7 = &frame[2 + 7];
        let h8_start = 4 + 7 * 3;
        let i8_start = 4 + 8 * 3;
        let j8_start = 4 + 9 * 3;

        assert_eq!(
            &row7[h8_start..h8_start + 3],
            "300",
            "h8 shows the hot visit count: {row7}"
        );

        // i8 is warm (25%) and also hot; it shows its count too.
        assert_eq!(
            &row7[i8_start..i8_start + 3],
            "100",
            "i8 shows the warm visit count: {row7}"
        );

        // j8 is a stone and the last move, so it keeps its brackets and
        // must not show a count.
        assert_eq!(
            &row7[j8_start..j8_start + 3],
            "[x]",
            "j8 keeps its last-move brackets: {row7}"
        );
        // The cursor on a hot empty cell takes precedence over the count.
        let frame_with_cursor = render_study(
            &StudyView::new(&core, &[])
                .policy_overlay(PolicyOverlay::Visits)
                .cursor(h8),
        );
        let row7_cursor = &frame_with_cursor[2 + 7];
        assert_eq!(
            &row7_cursor[h8_start..h8_start + 3],
            "<+>",
            "the cursor on a hot empty cell keeps the cursor marker: {row7_cursor}"
        );

        // The cursor on a stone keeps the stone marker.
        let frame_with_cursor_on_stone = render_study(
            &StudyView::new(&core, &[])
                .policy_overlay(PolicyOverlay::Visits)
                .cursor(j8),
        );
        let row7_cursor_stone = &frame_with_cursor_on_stone[2 + 7];
        assert_eq!(
            &row7_cursor_stone[j8_start..j8_start + 3],
            "<x>",
            "the cursor on a stone keeps the stone marker: {row7_cursor_stone}"
        );

        // The last-move marker also takes precedence over the count.
        // Make h8 the last move by playing only that stone.
        let mut core2 = SessionCore::new(SessionInit::new_hotseat());
        core2.execute(Command::Play { mv: h8 }).unwrap();
        core2.set_search_info(SearchInfo {
            side_to_move: Color::White,
            visits: vec![(h8, 300)],
            prior: vec![],
            value: -0.25,
            config: AiConfig::default(),
        });
        let frame_last =
            render_study(&StudyView::new(&core2, &[]).policy_overlay(PolicyOverlay::Visits));
        let row7_last = &frame_last[2 + 7];
        assert_eq!(
            &row7_last[h8_start..h8_start + 3],
            "[x]",
            "the last-move marker takes precedence: {row7_last}"
        );

        // Sanity: the panel is unchanged by the overlay.
        assert!(
            text.contains("Ply: 1"),
            "the panel still shows game state: {text}"
        );
    }

    #[test]
    fn prior_overlay_shows_percentages_and_respects_precedence() {
        let h8 = Move::new(7, 7).unwrap();
        let i8 = Move::new(7, 8).unwrap();
        let j8 = Move::new(7, 9).unwrap();
        let info = SearchInfo {
            side_to_move: Color::Black,
            visits: vec![],
            prior: vec![(h8, 0.5), (i8, 0.25)],
            value: 0.25,
            config: AiConfig::default(),
        };

        let mut core = SessionCore::new(SessionInit::new_hotseat());
        core.execute(Command::Play { mv: j8 }).unwrap();
        core.set_search_info(info);

        let frame = render_study(&StudyView::new(&core, &[]).policy_overlay(PolicyOverlay::Prior));
        let row7 = &frame[2 + 7];
        let h8_start = 4 + 7 * 3;
        let i8_start = 4 + 8 * 3;
        let j8_start = 4 + 9 * 3;

        // h8 is the evaluator's favourite and shows 50%.
        assert_eq!(
            &row7[h8_start..h8_start + 3],
            "50%",
            "h8 shows the hot prior percentage: {row7}"
        );

        // i8 is warm (50% of max) and also hot; it shows 25%.
        assert_eq!(
            &row7[i8_start..i8_start + 3],
            "25%",
            "i8 shows the warm prior percentage: {row7}"
        );

        // j8 is a stone and the last move, so it keeps its brackets.
        assert_eq!(
            &row7[j8_start..j8_start + 3],
            "[x]",
            "j8 keeps its last-move brackets: {row7}"
        );

        // The cursor on a hot empty cell takes precedence.
        let frame_with_cursor = render_study(
            &StudyView::new(&core, &[])
                .policy_overlay(PolicyOverlay::Prior)
                .cursor(h8),
        );
        let row7_cursor = &frame_with_cursor[2 + 7];
        assert_eq!(
            &row7_cursor[h8_start..h8_start + 3],
            "<+>",
            "the cursor on a hot empty cell keeps the cursor marker: {row7_cursor}"
        );
    }

    #[test]
    fn overlay_off_draws_the_same_board_as_no_search_info() {
        use crate::actor::SearchInfo;
        use engine::{Color, Move};

        // Two `p` presses away from Off must not leave a trace. The policy
        // cell path paints empty cells without the study board's dim grey,
        // so Off has to fall back to `study_cell` rather than reuse it.
        let h7 = Move::new(6, 7).unwrap();

        let mut bare = SessionCore::new(SessionInit::new_hotseat());
        bare.execute(Command::Play { mv: h7 }).unwrap();
        let no_search = render_study(&StudyView::new(&bare, &[]).styled(true));

        let mut searched = SessionCore::new(SessionInit::new_hotseat());
        searched.execute(Command::Play { mv: h7 }).unwrap();
        searched.set_search_info(SearchInfo {
            side_to_move: Color::Black,
            visits: vec![(h7, 100)],
            prior: vec![(h7, 0.5)],
            value: 0.25,
            config: AiConfig::default(),
        });
        let off = render_study(
            &StudyView::new(&searched, &[])
                .styled(true)
                .policy_overlay(PolicyOverlay::Off),
        );

        assert_eq!(
            off, no_search,
            "an overlay that is off must draw the board exactly as the bare board does"
        );
    }

    // -------------------------------------------------------- TSS overlay

    /// Build the engine's VCF win-in-3 test position and prove it.
    fn vcf_proof() -> engine::Proof {
        let mut b = engine::Board::new();
        for &(r, c) in &VCF_SCRIPT {
            b.play(engine::Move::new(r, c).unwrap()).unwrap();
        }
        engine::prove_forced_win(
            &b,
            engine::Color::Black,
            engine::SearchBudget {
                max_nodes: 2000,
                max_depth: 6,
            },
        )
        .expect("the VCF puzzle is proven")
    }

    /// The VCF position as a session: a black three on row 8, a black two
    /// that the proof turns into a double threat.
    const VCF_SCRIPT: [(u8, u8); 10] = [
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
    ];

    fn core_with_vcf_position() -> SessionCore {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        for &(r, c) in &VCF_SCRIPT {
            core.execute(Command::Play { mv: mv(r, c) }).unwrap();
        }
        core
    }

    #[test]
    fn tss_overlay_paints_numbered_cells() {
        let proof = vcf_proof();
        let core = core_with_vcf_position();
        let frame = render_study(&StudyView::new(&core, &[]).tss_proof(Some(&proof)));
        let row7 = &frame[2 + 7];
        let row5 = &frame[2 + 5];

        // Move 1: the attacker plays h8 (7,7), rendered plain.
        let h8_start = 4 + 7 * 3;
        assert_eq!(
            &row7[h8_start..h8_start + 3],
            " 1 ",
            "the attacker cell shows a plain number: {row7}"
        );

        // Move 2: the defender blocks at i8 (7,8), rendered bracketed.
        let i8_start = 4 + 8 * 3;
        assert_eq!(
            &row7[i8_start..i8_start + 3],
            "[2]",
            "the defender cell shows a bracketed number: {row7}"
        );

        // Move 3: the attacker plays f6 (5,5), rendered plain.
        let f6_start = 4 + 5 * 3;
        assert_eq!(
            &row5[f6_start..f6_start + 3],
            " 3 ",
            "the final attacker cell shows a plain number: {row5}"
        );

        // The verdict line is not part of the rendered frame; the shell
        // puts it on the message line. The board alone carries the overlay.
        let board_text: String = frame.iter().take(19).cloned().collect();
        assert!(!board_text.contains("certified"));
    }

    #[test]
    fn tss_overlay_preserves_stones_and_last_move() {
        let proof = vcf_proof();
        let core = core_with_vcf_position();
        let frame = render_study(&StudyView::new(&core, &[]).tss_proof(Some(&proof)));
        let row7 = &frame[2 + 7];

        // (7,6) is g8, an existing Black stone in the puzzle and not the
        // last move, so it keeps the plain stone glyph.
        let g8_start = 4 + 6 * 3;
        assert_eq!(
            &row7[g8_start..g8_start + 3],
            " x ",
            "an existing stone keeps its glyph: {row7}"
        );

        // The last move is (0,6) = g1 and keeps its brackets.
        let row0 = &frame[2];
        let g1_start = 4 + 6 * 3;
        assert_eq!(
            &row0[g1_start..g1_start + 3],
            "[o]",
            "the last move keeps its brackets: {row0}"
        );
    }

    #[test]
    fn tss_overlay_cursor_takes_precedence_over_numbers() {
        let proof = vcf_proof();
        let core = core_with_vcf_position();
        let h8 = Move::new(7, 7).unwrap();
        let frame = render_study(
            &StudyView::new(&core, &[])
                .tss_proof(Some(&proof))
                .cursor(h8),
        );
        let row7 = &frame[2 + 7];
        let h8_start = 4 + 7 * 3;
        assert_eq!(
            &row7[h8_start..h8_start + 3],
            "<+>",
            "the cursor on a numbered empty cell keeps the cursor marker: {row7}"
        );
    }

    #[test]
    fn tss_and_policy_overlay_mutual_exclusion_prefers_tss() {
        use crate::actor::SearchInfo;

        let proof = vcf_proof();
        let mut core = core_with_vcf_position();
        // Plant a policy overlay on the same cell.
        let h8 = Move::new(7, 7).unwrap();
        core.set_search_info(SearchInfo {
            side_to_move: Color::Black,
            visits: vec![(h8, 999)],
            prior: vec![],
            value: 0.0,
            config: AiConfig::default(),
        });

        // Both overlays active: TSS wins.
        let frame = render_study(
            &StudyView::new(&core, &[])
                .policy_overlay(PolicyOverlay::Visits)
                .tss_proof(Some(&proof)),
        );
        let row7 = &frame[2 + 7];
        let h8_start = 4 + 7 * 3;
        assert_eq!(
            &row7[h8_start..h8_start + 3],
            " 1 ",
            "TSS takes precedence over policy: {row7}"
        );
    }

    #[test]
    fn tss_styled_and_unstyled_frames_differ_only_in_escapes() {
        let proof = vcf_proof();
        let core = core_with_vcf_position();
        let styled = render_study(
            &StudyView::new(&core, &[])
                .tss_proof(Some(&proof))
                .styled(true),
        );
        let plain = render_study(
            &StudyView::new(&core, &[])
                .tss_proof(Some(&proof))
                .styled(false),
        );

        assert!(
            styled.iter().any(|l| l.contains('\x1b')),
            "the styled frame has escape sequences"
        );
        assert!(
            plain.iter().all(|l| !l.contains('\x1b')),
            "the plain frame has no escape sequences"
        );

        let stripped: Vec<String> = styled.iter().map(|l| strip_ansi(l)).collect();
        assert_eq!(stripped, plain);
    }
}
