# Sub-chapter 06b — The visits heatmap

## Abstract

This sub-chapter gives the study table a visits heatmap overlay on the
board. The overlay reads the visit distribution stored in
`SearchInfo::visits`, computes each legal move's share of total visits,
and paints empty cells with an ANSI-256 background whose intensity
matches that share. Hot cells, those with a share of at least 10%, print
their raw visit count. The user cycles the overlay with the `p` key in
normal mode or with the typed word `p`/`policy` in line mode. The cycle
is Off → Visits → Off.

## Glossary

| Term | Definition |
|------|------------|
| **Visits heatmap** | The board cell overlay that shows the share of total MCTS visits per empty cell. |
| **`PolicyOverlay`** | The shell enum that records the active cell-overlay mode. This sub-chapter implements `Off` and `Visits`. |
| **`OverlayCell`** | The render data for one board cell under an overlay: intensity, visible text, and hot flag. |
| **`overlay_cell`** | The pure function that turns `SearchInfo`, a move, and a `PolicyOverlay` into `Option<OverlayCell>`. |
| **`heatmap_bg`** | The function that maps an intensity 0.0–1.0 to an ANSI-256 background colour. |
| **Hot cell** | A cell whose visit share is at least 10%. It prints the raw visit count. |
| **Share of total visits** | `visits_for_move / total_visits`, used both for intensity and for the hot threshold. |
| **`render_overlay_cell_styled`** | Renders one overlay cell with an ANSI-256 background. |
| **`render_overlay_cell_plain`** | Renders one overlay cell without ANSI escapes. |
| **`CyclePolicy`** | The `InputOutcome` variant produced by the `p` key and the typed `p`/`policy` line. |

## Context

Read [the playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md)
first. Section 9 states that cell overlays exclude each other and that
the shell owns overlay state.

Sub-chapter 06a built the value strip, the first panel overlay, and
introduced `ShellState` and `InputOutcome`. This sub-chapter adds the
first cell overlay. It paints board cells, so it must not over-paint
stones, last-move brackets, or the cursor marker.

## Intention

The learner enables this user-facing behavior:

1. Press `p` in normal mode, or type `p` or `policy` in line mode, to
cycle the board overlay Off → Visits → Off.
2. In Visits mode, every empty cell that is neither a stone, the last
move, nor the cursor is painted with a background whose intensity equals
its share of total visits.
3. A cell with a share of at least 10% is hot and prints its raw visit
count. Cells below the threshold stay dim and show the normal empty-cell
glyph.
4. Stones, last-move brackets, and the cursor marker are never
over-painted.
5. When the user turns the overlay on at a node with no stored search,
the message line says `no search here yet` and the board stays bare.

## Specification as tests

The tests below are ordered along the execution path of a `p` keypress
from input translation through to the painted frame.

### Input translation

* `p_in_normal_mode_cycles_policy_overlay` — the `p` key returns
  `InputOutcome::CyclePolicy`.
* `p_and_policy_in_line_mode_cycle_policy_overlay` — the typed words `p`,
  `P`, `policy`, and `POLICY` return `InputOutcome::CyclePolicy`.

### Overlay formatting

* `cycle_order_is_off_visits_off` — `PolicyOverlay::cycle` moves `Off` to
  `Visits` and `Visits` to `Off`.
* `overlay_off_returns_none` — `overlay_cell` returns `None` in `Off`
  mode.
* `zero_visits_gives_zero_intensity_and_not_hot` — an unvisited move has
  intensity `0.0`, is not hot, and has no text.
* `single_hot_move_has_full_intensity` — a move with all visits has
  intensity `1.0`, is hot, and prints its count.
* `visit_share_computes_intensity_correctly` — a 75% cell and a 25% cell
  both compute the correct intensity and are hot.
* `small_visit_share_is_dim` — a 2.5% cell is not hot.
* `a_share_of_exactly_ten_percent_is_hot` — the 10% threshold is
  inclusive.
* `styled_hot_cell_has_ansi_background` — a styled hot cell contains the
  expected ANSI-256 background escape.
* `plain_hot_cell_has_no_ansi_escapes` — a plain hot cell contains no
  escape sequences.

### Rendering

* `policy_overlay_snapshot_with_fake_search_info` — the renderer uses
  the visit count for hot empty cells, keeps last-move brackets and the
  cursor marker, and never paints stones.

### Loop integration

* `p_at_a_fresh_cursor_says_no_search_here_yet` — pressing `p` at a node
  with no search stores a `no search here yet` message and leaves the
  board bare.
* `p_after_an_ai_step_shows_the_visit_heatmap` — pressing `p` after a
  deterministic search info is stored shows the visit count on the board
  and the `policy overlay: visits` message.

## Walkthrough

### 1. The user presses `p` or types `policy`

In normal mode `handle_normal_key` in `input.rs` checks the global keys
first. `KeyCode::Char('p')` returns `InputOutcome::CyclePolicy`
immediately. The key does not depend on the phase: the overlay is a view
toggle, not a command that changes the session.

In line mode the source reads one complete line. `line_outcome` in
`input.rs` trims the line. If the trimmed text is `p` or `policy`,
case-insensitively, it returns `InputOutcome::CyclePolicy`. Every other
non-empty line still goes through `command::parse`. The overlay is a
shell concern, so the command grammar does not gain a new variant for it.

### 2. The loop receives `CyclePolicy`

`study_loop` in `study.rs` owns `ShellState`. The variant cycles
`shell.policy_overlay` through `Off → Visits → Off`. It writes `policy
overlay: off`, `policy overlay: visits`, or `no search here yet` to the
message line. The last message appears only when the new mode is not
`Off` and the cursor has no stored search info.

### 3. The frame carries the overlay

`build_frame` passes `shell.policy_overlay` into
`StudyView::policy_overlay`. The renderer cannot know whether the overlay
is enabled; the view carries that flag the same way it carries the value
strip flag.

### 4. The board renders the overlay

`render_study` in `render.rs` fetches `core.search_info()` once and
passes it to `study_board_lines`. For each cell, `study_or_policy_cell`
decides what to draw. If the cell is a stone, the last move, or the
cursor, it calls `study_cell` and the overlay has no effect. Otherwise,
in `Visits` mode with search info present, it calls `policy_empty_cell`.

### 5. `policy_empty_cell` formats the cell

`policy_empty_cell` chooses the empty-cell glyph: `+` on a star point,
`.` elsewhere. It calls `overlay_cell` with `PolicyOverlay::Visits`. If
the result is hot, the styled renderer applies the ANSI-256 background
and prints the visit count; the plain renderer prints the count without
escapes. If the result is dim, the normal empty glyph is padded to three
characters. If the move is absent from the visit distribution, the
cell keeps the normal empty glyph.

### 6. `overlay.rs` computes the cell data

`PolicyOverlay` is an enum with `Off` and `Visits`. It has a `cycle`
method and a `label` method. `overlay_cell` dispatches to `visits_cell`
for `Visits` mode. `visits_cell` sums the visit counts, looks up the
move's visits, computes the share, and marks the cell hot when the share
is at least 10%. `heatmap_bg` maps the intensity to an ANSI-256 colour:
196 for full intensity, 208 and 220 for high shares, 39 for the muted
band, and 250 for dim values. `render_overlay_cell_styled` and
`render_overlay_cell_plain` turn the cell data into a three-character
string.

### 7. Help texts stay consistent

The normal-mode key table in `study.rs` gains `p cycle policy overlay
(off → visits)`. The line interpreter's `HELP` in `repl.rs` gains `p,
policy cycle policy overlay (off → visits)`. The byte-exact integration
transcript in `tests/line_mode.rs` grows by one matching line.

## Low-level design

### [`gomoku/crates/cli/src/tui/overlay.rs`](../../../../../gomoku/crates/cli/src/tui/overlay.rs)

Pure functions only. New public items:

* `PolicyOverlay` — enum with `Off` and `Visits`. Implements `Default`
  with `Off`, `cycle`, and `label`.
* `OverlayCell` — struct with `intensity: f32`, `text: String`, and
  `is_hot: bool`.
* `overlay_cell(info: &SearchInfo, mv: Move, mode: PolicyOverlay) ->
  Option<OverlayCell>` — returns `None` in `Off` mode or when the cell
  contributes nothing.
* `heatmap_bg(intensity: f32) -> u8` — maps intensity to ANSI-256 colour.
* `render_overlay_cell_styled(glyph: char, cell: &OverlayCell) -> String`
  — renders with ANSI-256 background.
* `render_overlay_cell_plain(glyph: char, cell: &OverlayCell) -> String`
  — renders without escapes.

The module keeps `value_strip` and `searching_seat_config` from
sub-chapter 06a and adds unit tests for the new items.

### [`gomoku/crates/cli/src/tui/input.rs`](../../../../../gomoku/crates/cli/src/tui/input.rs)

* Add `InputOutcome::CyclePolicy` after `ToggleValueStrip`.
* In `handle_normal_key`, match `KeyCode::Char('p')` with the other
  global keys.
* In `line_outcome`, add the `p`/`policy` guard before
  `command::parse`.
* Add the two `p` tests.

### [`gomoku/crates/cli/src/tui/study.rs`](../../../../../gomoku/crates/cli/src/tui/study.rs)

* Import `PolicyOverlay` from `overlay.rs`.
* Extend `ShellState` with `policy_overlay: PolicyOverlay`.
* In `study_loop`, handle `InputOutcome::CyclePolicy` by cycling the
  overlay and setting the message line. Pass the whole `ShellState` to
  `build_frame` so it can forward both overlay flags.
* In `build_frame`, set `StudyView::policy_overlay` from the shell.
* Add `p cycle policy overlay (off → visits)` to `NORMAL_MODE_KEYS`.
* Add the two loop integration tests.

### [`gomoku/crates/cli/src/tui/render.rs`](../../../../../gomoku/crates/cli/src/tui/render.rs)

* Import the new overlay helpers.
* Add `policy_overlay: PolicyOverlay` to `StudyView` and initialize it to
  `PolicyOverlay::Off`.
* Add the chainable `policy_overlay` setter.
* In `render_study`, fetch `search_info` once and pass it through
  `study_board_lines`.
* Extend `study_board_lines` with `policy_overlay` and `search_info`
  parameters.
* Add `study_or_policy_cell`, `stone_at`, and `policy_empty_cell`.
* Add the rendering test with hand-built search info.

### [`gomoku/crates/cli/src/session.rs`](../../../../../gomoku/crates/cli/src/session.rs)

* Add a `#[cfg(test)]` method `set_search_info` that inserts `SearchInfo`
  at the current cursor. Tests use it to render deterministic overlays
  without running MCTS.

### [`gomoku/crates/cli/src/repl.rs`](../../../../../gomoku/crates/cli/src/repl.rs)

* Add `p, policy        cycle policy overlay (off → visits)` to `HELP`.

### [`gomoku/crates/cli/tests/line_mode.rs`](../../../../../gomoku/crates/cli/tests/line_mode.rs)

* Add the same `p, policy` line to the byte-exact transcript constant.

## TDD checklist

Work one test group at a time, red to green.

From `gomoku/crates/cli/src/tui/input.rs`:

1. `p_in_normal_mode_cycles_policy_overlay`
2. `p_and_policy_in_line_mode_cycle_policy_overlay`

From `gomoku/crates/cli/src/tui/overlay.rs`:

3. `cycle_order_is_off_visits_off`
4. `overlay_off_returns_none`
5. `zero_visits_gives_zero_intensity_and_not_hot`
6. `single_hot_move_has_full_intensity`
7. `visit_share_computes_intensity_correctly`
8. `small_visit_share_is_dim`
9. `a_share_of_exactly_ten_percent_is_hot`
10. `styled_hot_cell_has_ansi_background`
11. `plain_hot_cell_has_no_ansi_escapes`

From `gomoku/crates/cli/src/tui/render.rs`:

12. `policy_overlay_snapshot_with_fake_search_info`

From `gomoku/crates/cli/src/tui/study.rs`:

13. `p_at_a_fresh_cursor_says_no_search_here_yet`
14. `p_after_an_ai_step_shows_the_visit_heatmap`

## Verification commands

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Done when

* The `p` key and the typed words `p`/`policy` cycle the visits overlay
  Off → Visits → Off.
* Visits mode paints empty cells with a background intensity equal to
  their share of total visits.
* Hot cells, those with share ≥ 10%, print their raw visit count.
* Stones, last-move brackets, and the cursor marker are never
  over-painted.
* All fourteen tests above pass.
* The three project gates pass.
* A scripted session shows the visits overlay on the board. From
  `gomoku/`, run:

  ```text
  printf 'h8\nstep\ni8\nstep\nj8\nstep\nk8\nstep\np\nq\n' | cargo run -q -p cli --bin gomoku -- play --ai tactics --no-swap2 --sims 200
  ```

  A captured frame looks like this:

```text
     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o       -- Study table --
   +---------------------------------------------+     Black: Human
 1 | o  o  o [o] .  .  .  .  .  .  .  .  .  .  . | 1   White: AI(200)
 2 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 2
 3 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 3   Ply: 8
 4 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 4   To move: Black
 5 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 5   Path: h8 a1 i8 b1 j8 c1 k8 d1
 6 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 6
 7 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 7   Last events:
 8 | .  .  .  +  .  .  .  x  x  x  x  +  .  .  . | 8     Moved b1 by Ai
 9 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 9     Moved j8 by Human
10 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 10    Moved c1 by Ai
11 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 11    Moved k8 by Human
12 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 12    Moved d1 by Ai
13 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 13
14 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 14  ? help · q quit
15 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 15
   +---------------------------------------------+
     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
policy overlay: visits
```

## References

* [Playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md) — §9 overlays and the cell-overlay rule.
* [Sub-chapter 06a — The value strip](../a-value-strip/value-strip.md) — `ShellState`, `InputOutcome`, and panel overlays.
* [Sub-chapter 05d — The alternate-screen shell](../05-the-real-table/d-shell/shell.md) — the loop and `InputOutcome` this sub-chapter extends.
* [`gomoku/crates/cli/src/tui/input.rs`](../../../../../gomoku/crates/cli/src/tui/input.rs) — input translation.
* [`gomoku/crates/cli/src/tui/overlay.rs`](../../../../../gomoku/crates/cli/src/tui/overlay.rs) — pure overlay formatting.
* [`gomoku/crates/cli/src/tui/render.rs`](../../../../../gomoku/crates/cli/src/tui/render.rs) — frame rendering.
* [`gomoku/crates/cli/src/tui/study.rs`](../../../../../gomoku/crates/cli/src/tui/study.rs) — the shell loop.
