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

src/session.rs, lines 786–794:

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

## src/tui/render.rs (new, 547 lines)

The module doc (lines 1–12) states the two rules of the module: nothing
prints, and styling is a runtime parameter because the same session
renders to the alternate screen and to plain pipes. The star-point table
follows the imports.

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

The constructor and the setters (lines 55–101) are chainable by taking
and returning `self`. The cursor setter documents the precedence rule
where a user of the API will read it.

src/tui/render.rs, lines 56–67 (the constructor) and 69–79 (one setter;
`command_line` at 81–90 and `styled` at 92–100 have the same shape):

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
column letters over the width-3 cells; `border` builds the `+---…---+`
line; `style_gutter` dims the furniture when styled. Only `style_gutter`
is quoted.

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

### The tests

The tests (lines 336–547) are in-module because the functions are pure.
Two local helpers (`mv`, `play`) build sessions; `strip_ansi`
normalizes styled lines for the equivalence test.

src/tui/render.rs, lines 350–371:

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

Six tests check the board and the panel contents: star points on empty
cells (373–385), the last-move marker and the byte offsets of the width-3
grid (387–399), seats, ply, and side to move (401–420), the game-result
line (422–445), the branch options with their 0-based numbers
(447–467), and the recent events (469–481). The cursor test also covers
the precedence rule: on the last move's square, the angle brackets
replace the square brackets.

src/tui/render.rs, lines 483–502:

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

The command-line test (504–512) asserts that the buffer adds exactly one
bottom line, `:` included. The equivalence test closes the module. It
exercises every styled element at once — stones, star points, gutters,
the last move, the cursor, and the panel lines — and asserts all three
legs of the styling contract.

src/tui/render.rs, lines 514–546:

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

A note on size: `render.rs` is 547 lines, past the chapter's
approximate 250-line budget. The in-module tests account for lines
336–547. The renderer itself is 335 lines, most of them comments, which
the chapter's commenting standard requires. Board and panel stay in one
file because they share the cell and gutter helpers and the styling
contract; splitting them would separate one concern into two files.