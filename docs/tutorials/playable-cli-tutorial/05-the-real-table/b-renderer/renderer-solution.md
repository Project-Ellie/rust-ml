# Sub-chapter 05b — The study-table renderer: solution

> **Opt-in solution — read only after you have tried the sub-chapter, or
> if you have been stuck for more than twenty minutes.**
>
> This file quotes the code that sub-chapter b adds or changes, as
> annotated snippets with line numbers. Line numbers refer to the files
> after the sub-chapter is applied. Deletions are described in prose; raw
> diffs are not shown. Verified green: `cargo test -p cli`,
> `cargo clippy --all-targets -- -D warnings`, and
> `cargo fmt --all -- --check` all pass from `gomoku/`.

Reading order: the accessors first (the panel reads them), then the view
struct (the renderer's input), then the frame assembly, then the board
half, then the panel half, then the tests.

## src/session.rs — four accessors

The four accessors follow `search_info`. Three are new;
`occupant` was a private method and becomes public with a doc comment
and `#[must_use]`. Its two existing call sites (`execute_step`,
`execute_play`) keep working unchanged.

src/session.rs, lines 243–269:

```rust
    /// The occupant of the Black seat.
    #[must_use]
    pub fn black(&self) -> &Occupant {
        &self.black
    }

    /// The occupant of the White seat.
    #[must_use]
    pub fn white(&self) -> &Occupant {
        &self.white
    }

    /// The default AI configuration, applied when a seat is released to
    /// the AI.
    #[must_use]
    pub fn default_config(&self) -> &AiConfig {
        &self.default_config
    }

    /// The occupant of the seat for `color`.
    #[must_use]
    pub fn occupant(&self, color: Color) -> &Occupant {
        match color {
            Color::Black => &self.black,
            Color::White => &self.white,
        }
    }
```

One unit test reads all four. It lives in the existing session test
module.

src/session.rs, lines 783–791:

```rust
    #[test]
    fn seat_accessors_report_the_occupants() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        assert_eq!(core.black(), &Occupant::Human);
        assert_eq!(core.white(), &Occupant::Human);
        assert_eq!(core.occupant(Color::Black), core.black());
        assert_eq!(core.occupant(Color::White), core.white());
        assert_eq!(core.default_config(), &AiConfig::default());
    }
```

## src/tui/render.rs (new, 544 lines)

The module doc (lines 1–12) states the two rules of the module: nothing
prints, and styling is a runtime parameter because the same session
renders to the alternate screen and to plain pipes. The star-point table
follows the imports.

src/tui/render.rs, lines 1–19:

```rust
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

use crate::actor::Occupant;
use crate::command::Event;
use crate::notation::algebraic;
use crate::session::{SessionCore, Snapshot};
use crossterm::style::Stylize;
use engine::{Board, Color, Move, Status};
```

src/tui/render.rs, lines 21–24:

```rust
/// The zero-based rows and columns of the nine star points: 4, 8, and 12
/// in one-based notation. An empty cell on a star point renders as `+`
/// instead of `.`.
const HOSHI: [u8; 3] = [3, 7, 11];
```

### The view struct

The input bundle. The doc comment carries a runnable example (it runs as
a doctest) and the reason the struct exists: five inputs, three of them
optional, do not fit a positional parameter list.

src/tui/render.rs, lines 26–53 (shown with a four-backtick fence
because the doc comment itself contains a doctest fence):

````rust
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
    styled: bool,
}
````

The constructor and the three setters (lines 55–101) are chainable by
taking and returning `self`. The cursor setter documents the precedence
rule where a user of the API will read it. The command-line setter
states that the renderer adds the `:` prompt and applies no styling to
the buffer. The styled setter states that unstyled output is safe to
pipe and to compare in tests.

src/tui/render.rs, lines 56–67 (the constructor), 69–79 (the cursor
setter), 81–90 (the command-line setter), and 92–100 (the styled
setter):

```rust
    /// A plain view of the session: no cursor, no command line, no
    /// styling.
    #[must_use]
    pub fn new(core: &'a SessionCore, recent_events: &'a [Event]) -> Self {
        Self {
            core,
            recent_events,
            cursor: None,
            command_line: None,
            styled: false,
        }
    }
```

```rust
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
```

```rust
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
```

```rust
    /// Turn ANSI styling on (`true`) or off (`false`).
    ///
    /// Unstyled output contains no escape sequences, so it is safe to pipe
    /// into files and to compare in tests.
    #[must_use]
    pub fn styled(mut self, styled: bool) -> Self {
        self.styled = styled;
        self
    }
```

### The frame assembly

`render_study` computes the board and the snapshot once and shares them
between the two halves. (The reference build queried the core inside
each helper, which replays the move list three times per frame. One
replay is enough, and the sharing makes the data flow explicit.) The zip
pads the shorter half with empty strings; the command line goes below
everything.

src/tui/render.rs, lines 103–130:

```rust
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
    let board_lines = study_board_lines(&board, snapshot.last_move, view.cursor, view.styled);
    let panel_lines = render_side_panel(core, &board, &snapshot, view.recent_events, view.styled);
    let rows = board_lines.len().max(panel_lines.len());
    let mut out = Vec::with_capacity(rows + 1);
    for i in 0..rows {
        let left = board_lines.get(i).map(String::as_str).unwrap_or("");
        let right = panel_lines.get(i).map(String::as_str).unwrap_or("");
        out.push(format!("{left}  {right}"));
    }
    // The command line is the last line of the frame, so the user's text
    // always appears at the bottom of the screen.
    if let Some(buffer) = view.command_line {
        out.push(format!(":{buffer}"));
    }
    out
}
```

### The board half

The layout: ruler, border, 15 rows, border, ruler. Row labels appear on
both sides, so a row number can be read without returning to the left
edge.

src/tui/render.rs, lines 132–161:

```rust
/// The board half of the frame: ruler, border, 15 rows, border, ruler —
/// 19 lines, each 53 characters wide before styling.
///
/// `last` is the most recent move, drawn with square brackets. `cursor`
/// is the shell's board cursor, drawn with angle brackets. Both are
/// optional.
fn study_board_lines(
    board: &Board,
    last: Option<Move>,
    cursor: Option<Move>,
    styled: bool,
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
            line.push_str(&study_cell(board, mv, last, cursor, styled));
        }
        line.push_str(&format!("| {rlabel}"));
        lines.push(line);
    }
    lines.push(style_gutter(&border(), styled));
    lines.push(style_gutter(&build_ruler(), styled));
    lines
}
```

The three furniture helpers (lines 163–189): `build_ruler` centers the
column letters over the width-3 cells, and `border` builds the
`+---…---+` line.

src/tui/render.rs, lines 163–172 (the ruler) and 174–177 (the border):

```rust
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
```

```rust
/// The border line between ruler and grid: 15 cells of 3 characters.
fn border() -> String {
    format!("   +{}+   ", "-".repeat(45))
}
```

`style_gutter` dims the furniture when styled.

src/tui/render.rs, lines 179–189:

```rust
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
```

### The cell

The function that draws one cell. It makes three decisions: the glyph
(stone, star point, or dot), the styling (color, and reverse video for
the cursor), and the marker (angle brackets for the cursor, square
brackets for the last move, spaces otherwise). The marker decision comes
last and applies to both modes identically, which is what makes the
styled/unstyled equivalence test possible.

src/tui/render.rs, lines 191–238:

```rust
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
    mv: Move,
    last: Option<Move>,
    cursor: Option<Move>,
    styled: bool,
) -> String {
    let stone = board.stone_at(mv);
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
```

### The side panel

Top to bottom: header, seat table, ply, the one state line, the path
tail, the branch options, the recent events, the footer. Two details to
notice: the branch numbers are the 0-based index the `var` command
takes (the reference build printed 1-based numbers; the panel follows
the command language), and the event list is the five most recent
events, oldest first, so the latest event is the bottom line of the list.

src/tui/render.rs, lines 240–293:

```rust
/// The side panel: seat table, ply, side to move or game result, path
/// tail, branch options, recent events, and the key hints.
fn render_side_panel(
    core: &SessionCore,
    board: &Board,
    snapshot: &Snapshot,
    recent_events: &[Event],
    styled: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(panel_line("-- Study table --", styled));
    lines.push(format!("Black: {}", seat_label(core.black())));
    lines.push(format!("White: {}", seat_label(core.white())));
    lines.push(String::new());
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
```

The three panel helpers (lines 295–334): `panel_line` dims a line when
styled, `seat_label` prints `Human` or `AI(N)` with the simulation
count, and `path_tail_string` prints the last eight moves in algebraic
notation, prefixed with `...` when earlier moves are elided and `(root)`
on an empty board.

src/tui/render.rs, lines 295–302 (`panel_line`), 304–311
(`seat_label`), and 313–334 (`path_tail_string`):

```rust
/// A panel line, dimmed when styled.
fn panel_line(text: &str, styled: bool) -> String {
    if styled {
        text.dark_grey().to_string()
    } else {
        text.to_string()
    }
}
```

```rust
/// How a seat appears in the panel: `Human`, or `AI(N)` with the seat's
/// simulation count.
fn seat_label(occupant: &Occupant) -> String {
    match occupant {
        Occupant::Human => "Human".to_string(),
        Occupant::Ai(cfg) => format!("AI({})", cfg.simulations),
    }
}
```

```rust
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
```

### The tests

The tests (lines 336–544) are in-module because the functions are pure.
The module header imports `mv` from the shared kit. `play` is a local
helper because it takes cli types, so the unit-test build cannot import
it from the kit crate (sub-chapter a states the two-versions rule).

src/tui/render.rs, lines 336–345 (the test module header and the `play`
helper):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::session::SessionInit;
    use testkit::mv;

    fn play(core: &mut SessionCore, r: u8, c: u8) {
        core.execute(Command::Play(mv(r, c))).unwrap();
    }
```

`strip_ansi` normalizes styled lines for the equivalence test.

src/tui/render.rs, lines 347–368:

```rust
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
```

Six tests check the board and the panel contents. The first checks the
star points: board row 7 is frame line 9, and an empty board shows
` + ` at columns d, h, and l.

src/tui/render.rs, lines 370–382:

```rust
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
```

The second checks the last-move marker and the byte offsets of the
width-3 grid: four gutter bytes, then three bytes per column, so h8
starts at byte 25.

src/tui/render.rs, lines 384–396:

```rust
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
```

The third checks the seats, the ply, and the side to move, before and
after one play.

src/tui/render.rs, lines 398–417:

```rust
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
```

The fourth checks the game-result line: a completed five reads
`Game over: Black wins`, and a finished game has no side to move.

src/tui/render.rs, lines 419–442:

```rust
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
```

The fifth checks the branch options with their 0-based numbers.

src/tui/render.rs, lines 444–464:

```rust
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
```

The sixth checks the recent events and the `(none)` line on an empty
list.

src/tui/render.rs, lines 466–478:

```rust
    #[test]
    fn the_panel_lists_recent_events() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        let events = core.execute(Command::Play(mv(7, 7))).unwrap();
        let frame = render_study(&StudyView::new(&core, &events));
        let text = frame.join("\n");
        assert!(text.contains("Last events:"));
        assert!(text.contains("Moved h8 by Human"));

        let frame = render_study(&StudyView::new(&core, &[]));
        let text = frame.join("\n");
        assert!(text.contains("  (none)"), "no events reads as (none)");
    }
```

The cursor test also covers the precedence rule: on the last move's
square, the angle brackets replace the square brackets.

src/tui/render.rs, lines 480–499:

```rust
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
```

The command-line test asserts that the buffer adds exactly one bottom
line, `:` included.

src/tui/render.rs, lines 501–509:

```rust
    #[test]
    fn the_command_line_is_the_bottom_line() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        let bare = render_study(&StudyView::new(&core, &[]));
        let with_line = render_study(&StudyView::new(&core, &[]).command_line("play h8"));
        // The command line adds exactly one line: the buffer behind ':'.
        assert_eq!(with_line.len(), bare.len() + 1);
        assert_eq!(with_line.last().unwrap(), ":play h8");
    }
```

The equivalence test closes the module. It exercises every styled
element at once — stones, star points, gutters, the last move, the
cursor, and the panel lines — and asserts all three legs of the styling
contract.

src/tui/render.rs, lines 511–543:

```rust
    #[test]
    fn styled_and_unstyled_frames_differ_only_in_escape_sequences() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        let mut events = core.execute(Command::Play(mv(7, 7))).unwrap();
        events.extend(core.execute(Command::Play(mv(6, 7))).unwrap());

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
```

## Verification

From `gomoku/`, after the sub-chapter:

* `cargo test -p cli` — 74 tests pass (63 before): 40 lib tests, of
  which 9 are the new render tests and 1 is the new accessor test, plus
  one new doctest on `StudyView`.
* `cargo clippy --all-targets -- -D warnings` — clean.
* `cargo fmt --all -- --check` — clean.

A note on size: `render.rs` is 544 lines, past the chapter's
approximate 250-line budget. The in-module tests account for lines
336–544. The renderer itself is 335 lines, most of them comments, which
the chapter's commenting standard requires. Board and panel stay in one
file because they share the cell and gutter helpers and the styling
contract; splitting them would separate one concern into two files.