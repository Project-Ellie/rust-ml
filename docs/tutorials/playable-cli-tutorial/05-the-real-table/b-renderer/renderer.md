# Sub-chapter 05b — The study-table renderer

Status: IMPLEMENTED. This sub-chapter adds rendering code. It does not
change what the session core does; it adds the read access the renderer
needs.

## Abstract

The study table needs a picture before it needs an event loop. This
sub-chapter writes the renderer as pure functions: session state in,
display lines out. The frame is the board on the left (star points, a
last-move marker), a side panel on the right (seats, ply, side to move,
game status, branch options, recent events), an optional board cursor,
and an optional command line at the bottom. A runtime switch turns ANSI
styling on or off. Unstyled output contains zero escape sequences, so
pipes and CI logs stay readable.

## Glossary

| Term | Definition |
|------|------------|
| **Frame** | One complete picture of the study table: board, side panel, and optional command line. A frame is a `Vec<String>`. |
| **`StudyView`** | The input bundle of the renderer: the session, the recent events, and three optional display settings. |
| **Star point** | One of the nine marked intersections of the board, at rows and columns 4, 8, and 12. An empty star-point cell renders as `+`. |
| **Last-move marker** | Square brackets around the cell of the most recent move: `[x]`. |
| **Board cursor** | The square the shell's keys act on. The cursor cell renders with angle brackets: `<x>`. |
| **Command line** | The bottom line of the frame while the user edits a `:` command. |
| **Escape sequence** | The ANSI control bytes crossterm emits for colors and attributes. |

## Context

Read [the design paper](../00-design.md) first. Its section
"b — The study-table renderer" assigns this sub-chapter. The dependency
rule matters here: rendering depends on the session; the shell
(sub-chapter d) depends on rendering; never the other direction.
Crossterm is used only inside `src/tui/`.

Two board renderers now exist, on purpose. `src/fmt.rs` renders the
compact board for the repl and the demo transcripts; sub-chapter a
established that code and it stays untouched. This sub-chapter adds the
richer study board in `src/tui/render.rs`: a frame, star points,
styling, and a side panel. The two serve different ports — a scrolling
transcript and a painted screen — and neither must grow the other's
features.

`SessionCore` gains four read accessors (`black`, `white`,
`default_config`, `occupant`). The panel needs them, and the shell in
sub-chapter d will need them. One of them (`occupant`) existed as a
private method; it becomes public.

## Intention

1. Add the four accessors to `SessionCore`, with doc comments and
   `#[must_use]`.
2. Create the study renderer in `src/tui/render.rs`: `StudyView`,
   `render_study`, the board half, the side panel, and their helpers.
3. Cover the renderer with unit tests in the same file. The functions
   are pure, so in-module tests are the right place.
4. Keep the styling contract: unstyled output contains zero escape
   sequences, and styled output differs only by escape sequences.

## Mental mapping

### Rendering is a function

The renderer never prints. It takes a session and returns lines. Three
consequences, the same shape sub-chapter a gave the repl:

1. **Tests assert on the returned lines.** No terminal, no capture
   harness.
2. **The caller owns the output port.** The alternate screen and the
   line-mode fallback will both call the same function.
3. **Rendering bugs are data bugs.** A wrong frame is a wrong
   `Vec<String>`, reproducible in a test.

### Why `styled` is a runtime parameter

The same session renders to two kinds of output: the full-screen shell,
where colors and attributes make the position easier to read, and pipes
or CI logs, where escape sequences appear as raw control bytes. A
compile-time feature flag cannot serve both from one binary. So the view
carries styling as a `bool`, and the contract is testable: the unstyled
frame contains no escape sequences, and the styled frame, stripped of
escape sequences, equals the unstyled frame byte for byte. The last test
in the file enforces exactly that.

### The frame model

The board half is 19 lines tall and 53 characters wide: ruler, border,
15 rows, border, ruler. Every cell is three characters, so the grid
does not shift when a marker appears. The panel half is a list of
lines. The frame zips the two with a two-space gap; the shorter side
contributes empty strings.

The command line, when present, is one extra line below everything:
`:` followed by the user's buffer. The renderer adds the `:` and adds
no styling to the buffer, because the buffer is the user's own text.

### A view struct instead of five parameters

The frame needs five inputs: the session, the recent events, the
cursor, the command-line buffer, and the styling flag. Three of the
five are optional. A five-parameter function would make every call site
spell out `None, None, false`, and a trailing `true` would not say what
it switches. `StudyView` takes the two required inputs in its
constructor and gives each optional input a named, chainable setter.
The line-mode fallback in sub-chapter e will call
`StudyView::new(&core, &events)` and nothing else; the shell in
sub-chapter d adds the cursor and the command line when it has them.

### One cell, two markers

The cursor and the last move can be on the same square. The renderer
draws one marker per cell, and the cursor takes precedence: the cell
shows angle brackets instead of square brackets. The rule exists because
the cursor is where the user's next action applies. In a styled frame
the glyph keeps its stone color and gains reverse video, so no
information is lost. The rule is uniform across both modes, which is
what keeps the styled/unstyled equivalence test possible.

### Branch numbers are the `var` index

The panel numbers branch options with the 0-based index the `var`
command takes: `[0] h7 [1] i8`. What the user reads is what the user
types. (The reference build printed 1-based numbers here. The current
command language is 0-based, and the panel follows the command
language.)

## Low-level design

### `src/session.rs` — four accessors

```rust
pub fn black(&self) -> &Occupant;
pub fn white(&self) -> &Occupant;
pub fn default_config(&self) -> &AiConfig;
pub fn occupant(&self, color: Color) -> &Occupant;
```

All four carry doc comments and `#[must_use]`. `occupant` replaces the
private method of the same name; its existing call sites keep working.

### `src/tui/render.rs` (new)

```rust
pub struct StudyView<'a> { /* session, events, cursor, command line, styled */ }

impl<'a> StudyView<'a> {
    pub fn new(core: &'a SessionCore, recent_events: &'a [Event]) -> Self;
    pub fn cursor(mut self, mv: Move) -> Self;
    pub fn command_line(mut self, buffer: &'a str) -> Self;
    pub fn styled(mut self, styled: bool) -> Self;
}

pub fn render_study(view: &StudyView) -> Vec<String>;
```

Board contract:

- 19 lines: ruler, border, 15 rows, border, ruler. Each line is 53
  characters wide before styling.
- Row format: `{:>2} |`, 15 cells, `| {:<2}` — row numbers on both
  sides.
- Cell glyphs, always three characters: `x` (Black), `o` (White), `+`
  (empty star point), `.` (empty elsewhere).
- Markers: the cursor cell is shown with `< >`; the last-move cell with
  `[ ]`; every other cell is padded with spaces. The cursor takes
  precedence when both apply.
- Star points are rows and columns 4, 8, 12 in one-based notation.

Panel contract, top to bottom:

- Header `-- Study table --`, then `Black: …` and `White: …` seat
  lines (`Human`, or `AI(N)` with the simulation count).
- `Ply: N`.
- One state line: `To move: <Color>` while the game is ongoing, or
  `Game over: <Color> wins` / `Game over: draw` when it is finished.
- `Path: …` — the last eight moves in algebraic notation, prefixed with
  `...` when earlier moves are elided; `(root)` on an empty board.
- `N variations: [0] … [1] …` — only at a branch point (more than one
  child). The numbers are the 0-based `var` index.
- `Last events:` — the five most recent events, oldest first;
  `  (none)` when empty.
- Footer `? help · q quit`.

Styling contract:

- Stones: cyan bold (Black), yellow (White). Empty cells, star points,
  rulers, borders, row labels, and the panel header and footer: dark
  grey.
- The cursor adds reverse video and keeps the glyph's own color.
- The command line is never styled.
- Unstyled output contains zero escape sequences.

## Solution (opt-in)

The added and changed code, as annotated snippets with line numbers, is
in [renderer-solution.md](renderer-solution.md). Open it only if you
have been stuck for more than twenty minutes, or after you have
finished the sub-chapter and want to compare your work.

## TDD checklist

The rhythm is red, green, extend. Run from `gomoku/`.

1. **Accessors.** Add the four accessors to `SessionCore` and one unit
   test that reads them. `cargo test -p cli session` passes.
2. **Board half.** Write the star-point test and the last-move-marker
   test against `render_study`. Watch them fail — the stub renders
   nothing. Implement `StudyView`, `render_study`,
   `study_board_lines`, and `study_cell`. Watch the tests pass.
3. **Side panel.** Add the seats/ply/side-to-move test, the game-result
   test, the branch-options test, and the recent-events test. Implement
   `render_side_panel` and its helpers. Run the gates.
4. **Cursor and command line.** Add the cursor test (including the
   precedence rule) and the command-line test. Extend `study_cell` and
   `render_study`. Run the gates.
5. **Equivalence.** Add the styled/unstyled equivalence test with the
   `strip_ansi` helper. It must assert three things: the styled frame
   contains escape sequences, the unstyled frame contains none, and the
   two frames are equal after stripping. Run the gates.

## Verification commands

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Done when

* `SessionCore` has the four accessors with doc comments and
  `#[must_use]`.
* `src/tui/render.rs` exposes `StudyView` and `render_study`, and
  nothing in the module prints.
* The in-module tests cover star points, the last-move marker, seats,
  ply, side to move, the game result, branch options, recent events,
  the cursor, the command line, and styled/unstyled equivalence.
* Unstyled frames contain zero escape sequences.
* All three gates pass. (Result here: 74 tests, up from 63.)

Next: sub-chapter c — Swap2 as a session phase.

## References

* [00-design.md](../00-design.md) — the contract for all six
  sub-chapters, including the dependency rule and the commenting
  standard.
* [a-refactor/refactor.md](../a-refactor/refactor.md) — the previous
  sub-chapter; the repl renderer in `fmt.rs` is deliberately not
  touched here.
* [Chapter 04 — The study loop](../04-the-study-loop.md) — the session
  API the renderer reads.
