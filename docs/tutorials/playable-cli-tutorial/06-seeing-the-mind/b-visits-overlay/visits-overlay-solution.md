# Sub-chapter 06b — The visits heatmap: solution

This file is opt-in. Read it only if you have been stuck for more than
twenty minutes, or after you have finished the sub-chapter and want to
compare your work.

Verified green from `gomoku/`: `cargo test -p cli`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.

---

## `overlay.rs` — policy heatmap types and helpers

`gomoku/crates/cli/src/tui/overlay.rs` pre-exists from sub-chapter 06a.
The value-strip code is unchanged. The new policy-overlay code is added
at the top of the file, before `value_strip`, and the new tests are
added before the value-strip tests in the same module.

The updated module doc comment and new imports:

`gomoku/crates/cli/src/tui/overlay.rs`, lines 1–12:

```rust
//! Search-info overlays — pure functions, no I/O.
//!
//! [`value_strip`] turns a cursor node's [`SearchInfo`] into the lines of
//! the side-panel value strip. The policy heatmap helpers turn the same
//! search info into per-cell render data. They are deliberately testable
//! without a real MCTS search: callers can construct [`SearchInfo`]
//! directly.

use crate::actor::{AiConfig, SearchInfo};
use crate::fmt::format_move;
use crate::session::SessionCore;
use engine::Move;
```

`PolicyOverlay`, `OverlayCell`, and the overlay cell helpers:

`gomoku/crates/cli/src/tui/overlay.rs`, lines 14–138:

```rust
/// Active cell-overlay mode for the policy heatmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PolicyOverlay {
    /// No overlay; show the bare board.
    #[default]
    Off,
    /// Heatmap by share of total visits.
    Visits,
}

impl PolicyOverlay {
    /// Cycle order: Off → Visits → Off.
    #[must_use]
    pub fn cycle(self) -> Self {
        match self {
            PolicyOverlay::Off => PolicyOverlay::Visits,
            PolicyOverlay::Visits => PolicyOverlay::Off,
        }
    }

    /// Short label used in status messages.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            PolicyOverlay::Off => "off",
            PolicyOverlay::Visits => "visits",
        }
    }
}

/// Per-cell render data for a policy overlay cell.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayCell {
    /// Background intensity in 0.0–1.0, used to select the ANSI-256 colour.
    pub intensity: f32,
    /// Visible text for hot cells; empty for dim cells.
    pub text: String,
    /// Whether this cell is hot enough to print its value.
    pub is_hot: bool,
}

/// Produce render data for one board cell under the given overlay mode.
///
/// Returns `None` for empty cells with no overlay contribution (e.g. an
/// unvisited move in Visits mode, or Off mode).
#[must_use]
pub fn overlay_cell(info: &SearchInfo, mv: Move, mode: PolicyOverlay) -> Option<OverlayCell> {
    match mode {
        PolicyOverlay::Off => None,
        PolicyOverlay::Visits => visits_cell(info, mv),
    }
}

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
    let is_hot = share >= 0.10;
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
#[must_use]
pub fn render_overlay_cell_styled(glyph: char, cell: &OverlayCell) -> String {
    let bg = heatmap_bg(cell.intensity);
    let content = cell_content(glyph, cell);
    format!("\x1b[48;5;{bg}m{content}\x1b[0m")
}

/// Render one plain cell (no ANSI escapes).
#[must_use]
pub fn render_overlay_cell_plain(glyph: char, cell: &OverlayCell) -> String {
    cell_content(glyph, cell)
}

fn cell_content(glyph: char, cell: &OverlayCell) -> String {
    if cell.is_hot {
        pad_numeric_cell(&cell.text)
    } else {
        format!(" {} ", glyph)
    }
}

fn pad_numeric_cell(text: &str) -> String {
    if text.len() >= 3 {
        text[..3].to_string()
    } else {
        format!("{:>3}", text)
    }
}
```

The new policy-overlay tests in `overlay.rs`:

`gomoku/crates/cli/src/tui/overlay.rs`, lines 234–319:

```rust
    #[test]
    fn cycle_order_is_off_visits_off() {
        assert_eq!(PolicyOverlay::Off.cycle(), PolicyOverlay::Visits);
        assert_eq!(PolicyOverlay::Visits.cycle(), PolicyOverlay::Off);
    }

    #[test]
    fn overlay_off_returns_none() {
        let info = search_info_with_visits(&[(mv(7, 7), 100)], &[], 0.3, Color::Black);
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
```

## `session.rs` — test seam for deterministic search info

`SessionCore` gains a `#[cfg(test)]` method that inserts `SearchInfo` at
the current cursor. Production code cannot call it.

`gomoku/crates/cli/src/session.rs`, lines 284–289:

```rust
    /// Insert search info at the current cursor, for tests that need a
    /// deterministic overlay without running MCTS.
    #[cfg(test)]
    pub fn set_search_info(&mut self, info: SearchInfo) {
        self.search_info.insert(self.history.cursor(), info);
    }
```

## `input.rs` — the `p` key and typed `policy`

`InputOutcome` gains `CyclePolicy` after `ToggleValueStrip`.

`gomoku/crates/cli/src/tui/input.rs`, lines 88–105:

```rust
pub enum InputOutcome {
    /// Execute this command on the session core.
    Command(Command),
    /// Show the full-screen help; any key returns to the table.
    Help,
    /// Leave the shell.
    Quit,
    /// Show this text on the message line. Parse errors and keys that
    /// do not apply in the current phase use this variant.
    Message(String),
    /// Toggle the value strip in the side panel.
    ToggleValueStrip,
    /// Cycle the policy heatmap overlay on the board.
    CyclePolicy,
    /// The key changed only input state (cursor moved, buffer edited),
    /// or the key is not mapped. Nothing reaches the session.
    None,
}
```

The normal-mode global-key table returns the new outcome for `p`.

`gomoku/crates/cli/src/tui/input.rs`, lines 178–205:

```rust
fn handle_normal_key(key: KeyEvent, state: &mut InputState, core: &SessionCore) -> InputOutcome {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            move_cursor(&mut state.cursor, -1, 0);
            return InputOutcome::None;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            move_cursor(&mut state.cursor, 1, 0);
            return InputOutcome::None;
        }
        KeyCode::Left | KeyCode::Char('h') => {
            move_cursor(&mut state.cursor, 0, -1);
            return InputOutcome::None;
        }
        KeyCode::Right | KeyCode::Char('l') => {
            move_cursor(&mut state.cursor, 0, 1);
            return InputOutcome::None;
        }
        KeyCode::Char(':') => {
            state.mode = InputMode::CommandLine(String::new());
            return InputOutcome::None;
        }
        KeyCode::Char('?') => return InputOutcome::Help,
        KeyCode::Char('q') => return InputOutcome::Quit,
        KeyCode::Char('v') => return InputOutcome::ToggleValueStrip,
        KeyCode::Char('p') => return InputOutcome::CyclePolicy,
        _ => {}
    }
```

`line_outcome` recognises `p`/`policy` before falling back to the shared
command grammar.

`gomoku/crates/cli/src/tui/input.rs`, lines 285–306:

```rust
/// Everything non-empty goes through [`command::parse`] except the
/// shell-level words `v` and `value`, which toggle the value strip.
/// The overlay is a shell concern, so the command grammar does not
/// gain a variant for it. A parse error becomes an
/// [`InputOutcome::Message`] with the parser's own text, the same as
/// on the `:` command line of the full-screen shell.
pub fn line_outcome(line: &str, core: &SessionCore) -> InputOutcome {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return empty_line_outcome(core);
    }
    if trimmed.eq_ignore_ascii_case("v") || trimmed.eq_ignore_ascii_case("value") {
        return InputOutcome::ToggleValueStrip;
    }
    if trimmed.eq_ignore_ascii_case("p") || trimmed.eq_ignore_ascii_case("policy") {
        return InputOutcome::CyclePolicy;
    }
    match command::parse(trimmed) {
        Ok(cmd) => InputOutcome::Command(cmd),
        Err(err) => InputOutcome::Message(err.to_string()),
    }
}
```

The two new input tests cover the `p` key and the typed words.

`gomoku/crates/cli/src/tui/input.rs`, lines 749–769:

```rust
    #[test]
    fn p_in_normal_mode_cycles_policy_overlay() {
        let core = playing_core();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(char_key('p'), &mut state, &core),
            InputOutcome::CyclePolicy
        );
    }

    #[test]
    fn p_and_policy_in_line_mode_cycle_policy_overlay() {
        let core = playing_core();
        for word in ["p", "P", "policy", "POLICY"] {
            assert_eq!(
                line_outcome(word, &core),
                InputOutcome::CyclePolicy,
                "{word} cycles the policy overlay"
            );
        }
    }
```

## `render.rs` — painting the heatmap on empty cells

`render.rs` imports the new overlay helpers alongside the existing ones.

`gomoku/crates/cli/src/tui/render.rs`, lines 19–22:

```rust
use crate::tui::overlay::{
    PolicyOverlay, overlay_cell, render_overlay_cell_plain, render_overlay_cell_styled,
    searching_seat_config, value_strip,
};
```

`StudyView` stores the overlay flag and initializes it to `Off`.

`gomoku/crates/cli/src/tui/render.rs`, lines 52–77:

```rust
pub struct StudyView<'a> {
    core: &'a SessionCore,
    recent_events: &'a [Event],
    cursor: Option<Move>,
    command_line: Option<&'a str>,
    message: Option<&'a str>,
    styled: bool,
    show_value_strip: bool,
    policy_overlay: PolicyOverlay,
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
        }
```

The chainable setter turns the overlay on or off.

`gomoku/crates/cli/src/tui/render.rs`, lines 135–140:

```rust
    /// Set the board policy heatmap overlay.
    #[must_use]
    pub fn policy_overlay(mut self, overlay: PolicyOverlay) -> Self {
        self.policy_overlay = overlay;
        self
    }
```

`render_study` fetches the cursor's search info once and passes it into
the board-line builder.

`gomoku/crates/cli/src/tui/render.rs`, lines 159–168:

```rust
    let search_info = core.search_info();
    let board_lines = study_board_lines(
        &board,
        opening,
        snapshot.last_move,
        view.cursor,
        view.styled,
        view.policy_overlay,
        search_info,
    );
```

`study_board_lines` carries the overlay and the search info through to
each cell.

`gomoku/crates/cli/src/tui/render.rs`, lines 201–237:

```rust
fn study_board_lines(
    board: &Board,
    opening: Option<&OpeningSnapshot>,
    last: Option<Move>,
    cursor: Option<Move>,
    styled: bool,
    policy_overlay: PolicyOverlay,
    search_info: Option<&crate::actor::SearchInfo>,
) -> Vec<String> {
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
                policy_overlay,
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
```

`study_or_policy_cell` decides whether the overlay applies. Stones,
cursor, and last-move brackets take precedence.

`gomoku/crates/cli/src/tui/render.rs`, lines 239–296:

```rust
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
    search_info: Option<&crate::actor::SearchInfo>,
) -> String {
    let is_stone = stone_at(board, opening, mv).is_some();
    let is_cursor = cursor == Some(mv);
    let is_last = last == Some(mv);

    if is_stone || is_cursor || is_last {
        study_cell(board, opening, mv, last, cursor, styled)
    } else if policy_overlay == PolicyOverlay::Visits {
        match search_info {
            Some(info) => policy_empty_cell(mv, info, styled),
            None => study_cell(board, opening, mv, last, cursor, styled),
        }
    } else {
        study_cell(board, opening, mv, last, cursor, styled)
    }
}

/// Look up the stone at a cell, including Swap2 ceremony placements.
fn stone_at(board: &Board, opening: Option<&OpeningSnapshot>, mv: Move) -> Option<engine::Color> {
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

/// Render an empty cell under the visits overlay.
fn policy_empty_cell(mv: Move, info: &crate::actor::SearchInfo, styled: bool) -> String {
    let glyph = if HOSHI.contains(&mv.row()) && HOSHI.contains(&mv.col()) {
        '+'
    } else {
        '.'
    };
    match overlay_cell(info, mv, PolicyOverlay::Visits) {
        Some(cell) if styled => render_overlay_cell_styled(glyph, &cell),
        Some(cell) => render_overlay_cell_plain(glyph, &cell),
        None => format!(" {} ", glyph),
    }
}
```

The rendering test uses `set_search_info` to avoid real MCTS and asserts
that hot empty cells show counts, stones/cursor/last-move stay visible,
and the panel is unchanged.

`gomoku/crates/cli/src/tui/render.rs`, lines 897–1002:

```rust
    #[test]
    fn policy_overlay_snapshot_with_fake_search_info() {
        use crate::actor::{AiConfig, SearchInfo};
        use engine::{Color, Move};

        // A hand-built search info so the test needs no real MCTS.
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
        // Store the fake info at the cursor manually.
        core.set_search_info(info);

        let frame = render_study(&StudyView::new(&core, &[]).policy_overlay(PolicyOverlay::Visits));
        let text = frame.join("\n");

        let row7 = &frame[2 + 7];
        // The row prefix is "{:>2} |" = 4 characters, then 3 characters
        // per cell.
        let h8_start = 4 + 7 * 3;
        let i8_start = 4 + 8 * 3;
        let j8_start = 4 + 9 * 3;

        // h8 is the hottest empty cell and shows its visit count.
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
```

## `study.rs` — shell state and loop handling

`study.rs` imports `PolicyOverlay`.

`gomoku/crates/cli/src/tui/study.rs`, lines 25–34:

```rust
use crate::command::{Command, Event};
use crate::repl;
use crate::session::{SessionCore, SessionInit};
use crate::tui::input::{InputMode, InputOutcome, InputState, handle_key, line_outcome};
use crate::tui::overlay::PolicyOverlay;
use crate::tui::render::{StudyView, render_study};
use crate::tui::screen::{AltScreen, LineScreen, Screen};
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use engine::Move;
use std::io::{self, BufRead, IsTerminal};
```

`build_frame` now receives `&shell` and forwards both overlay flags.

`gomoku/crates/cli/src/tui/study.rs`, lines 275–283:

```rust
        let frame = build_frame(
            core,
            &state,
            cursor,
            &events,
            message.as_deref(),
            screen.styled(),
            &shell,
        );
```

`study_loop` handles `CyclePolicy` by cycling the overlay and writing
either the mode label or `no search here yet`.

`gomoku/crates/cli/src/tui/study.rs`, lines 299–308:

```rust
            InputOutcome::CyclePolicy => {
                shell.policy_overlay = shell.policy_overlay.cycle();
                message = Some(
                    if shell.policy_overlay != PolicyOverlay::Off && core.search_info().is_none() {
                        "no search here yet".to_string()
                    } else {
                        format!("policy overlay: {}", shell.policy_overlay.label())
                    },
                );
            }
```

`ShellState` gains `policy_overlay`.

`gomoku/crates/cli/src/tui/study.rs`, lines 328–338:

```rust
/// Display and overlay state owned by the shell loop.
///
/// This is not session state: it belongs to the user's view of the
/// table and is reset when the shell restarts.
#[derive(Debug, Default)]
struct ShellState {
    /// Whether the side panel shows the value strip.
    show_value_strip: bool,
    /// The active board policy heatmap overlay.
    policy_overlay: PolicyOverlay,
}
```

`build_frame` accepts `&ShellState` and forwards both flags to
`StudyView`.

`gomoku/crates/cli/src/tui/study.rs`, lines 348–373:

```rust
fn build_frame(
    core: &SessionCore,
    state: &InputState,
    cursor: Option<Move>,
    events: &[Event],
    message: Option<&str>,
    styled: bool,
    shell: &ShellState,
) -> Vec<String> {
    let view = StudyView::new(core, events)
        .styled(styled)
        .show_value_strip(shell.show_value_strip)
        .policy_overlay(shell.policy_overlay);
    let view = match cursor {
        Some(mv) => view.cursor(mv),
        None => view,
    };
    let view = match &state.mode {
        InputMode::CommandLine(buffer) => view.command_line(buffer),
        InputMode::Normal => match message {
            Some(text) => view.message(text),
            None => view,
        },
    };
    render_study(&view)
}
```

The normal-mode key table lists `p`.

`gomoku/crates/cli/src/tui/study.rs`, lines 430–443:

```rust
const NORMAL_MODE_KEYS: &str = r#"Normal-mode keys (single keys, no Enter):
  arrows, hjkl   move the board cursor
  Enter          play at the cursor (human seat), step the AI (AI seat);
                 during Swap2 placement: place the next stone
  u              undo: remove a Swap2 placement / go back one ply
  d              Swap2 placement: end the round (done)
  b, w           Swap2 choice: choose that color
  m              Swap2 first choice: place two more stones (more)
  v              toggle value strip
  p              cycle policy overlay (off → visits)
  :              open the command line (Esc cancels, Enter executes)
  ?              show this help
  q              quit
"#;
```

The tests module imports `AiConfig` for the deterministic search info
used in the loop tests.

`gomoku/crates/cli/src/tui/study.rs`, lines 445–453:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::AiConfig;
    use crate::session::SessionInit;
    use crossterm::event::{KeyCode, KeyModifiers};
    use engine::{Color, Move};
    use std::collections::VecDeque;
    use std::io::Cursor;
```

The two loop integration tests press `p` and assert on the resulting
frame.

`gomoku/crates/cli/src/tui/study.rs`, lines 791–864:

```rust
    #[test]
    fn p_at_a_fresh_cursor_says_no_search_here_yet() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        // `p` cycles the policy overlay on at a node with no search.
        let keys = vec![char_key('p'), char_key('q')];
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        let frame1 = &screen.frames[1];
        let text = frame1.join("\n");
        assert!(
            text.contains("no search here yet"),
            "the message line explains the empty search: {text}"
        );
        // The board stays bare: no heatmap background escapes and no
        // numeric overlay cells beyond the row labels.
        let board_text: String = frame1.iter().take(17).skip(2).cloned().collect();
        assert!(
            !board_text.contains('\x1b'),
            "the board has no ANSI escapes: {board_text}"
        );
        // Every interior cell is either a stone glyph, a star point, or
        // an empty marker; none are numeric.
        for line in frame1.iter().take(17).skip(2) {
            let interior = &line[5..50];
            for cell in interior.as_bytes().chunks(3) {
                let s = std::str::from_utf8(cell).unwrap();
                assert!(
                    !s.chars().any(|c| c.is_ascii_digit()),
                    "no numeric overlay cell in '{s}' of: {line}"
                );
            }
        }
    }

    #[test]
    fn p_after_an_ai_step_shows_the_visit_heatmap() {
        use engine::{Color, Move};

        // Build a deterministic search info directly so the test does not
        // depend on the MCTS visit distribution. h7 is used instead of h8
        // because the cursor starts at h8 and the cursor marker must not be
        // over-painted.
        let h7 = Move::new(6, 7).unwrap();
        let info = crate::actor::SearchInfo {
            side_to_move: Color::Black,
            visits: vec![(h7, 300)],
            prior: vec![],
            value: 0.25,
            config: AiConfig::default(),
        };

        let mut core = SessionCore::new(SessionInit::new_hotseat());
        core.set_search_info(info);

        let keys = vec![char_key('p'), char_key('q')];
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        let frame1 = &screen.frames[1];
        let text = frame1.join("\n");
        assert!(
            text.contains("policy overlay: visits"),
            "the message line reports visits mode: {text}"
        );

        // h7 is the hottest empty cell and shows its raw count in the
        // board area.
        let row_line = &frame1[2 + h7.row() as usize];
        let cell_start = 4 + h7.col() as usize * 3;
        let cell = &row_line[cell_start..cell_start + 3];
        assert_eq!(
            cell, "300",
            "the hottest cell h7 shows 300, got '{cell}' in: {row_line}"
        );
    }
```

## `repl.rs` and `tests/line_mode.rs` — help text and transcript

The line interpreter's help text documents the typed command.

`gomoku/crates/cli/src/repl.rs`, lines 23–39:

```rust
pub const HELP: &str = r#"Commands (case-insensitive):
  SQUARE           play a move, e.g. "h8"
  play SQUARE      same as above
  step             let the AI play
  back [N]         go back N plies (default 1)
  forward [N]      go forward N plies (default 1)
  var N            choose child variation N
  take black|white|both      make seat(s) human
  release black|white|both   release seat(s) to AI
  sims black|white N         set per-seat simulations
  info             show AI search info at the cursor
  v, value         toggle the value strip
  p, policy        cycle policy overlay (off → visits)
  new              reset to opening
  h, help, ?       show this help
  q, quit, exit    quit

```

The byte-exact integration transcript grows by the matching help line.

`gomoku/crates/cli/tests/line_mode.rs`, lines 113–129:

```rust
Commands (case-insensitive):
  SQUARE           play a move, e.g. "h8"
  play SQUARE      same as above
  step             let the AI play
  back [N]         go back N plies (default 1)
  forward [N]      go forward N plies (default 1)
  var N            choose child variation N
  take black|white|both      make seat(s) human
  release black|white|both   release seat(s) to AI
  sims black|white N         set per-seat simulations
  info             show AI search info at the cursor
  v, value         toggle the value strip
  p, policy        cycle policy overlay (off → visits)
  new              reset to opening
  h, help, ?       show this help
  q, quit, exit    quit

```
