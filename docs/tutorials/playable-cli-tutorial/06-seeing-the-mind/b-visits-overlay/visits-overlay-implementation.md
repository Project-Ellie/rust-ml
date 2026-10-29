# Sub-chapter 06b — The visits heatmap: implementation plan

This document turns the walkthrough in `visits-overlay.md` into a
construction order. It names the files, structs, enums, functions, and
tests. It states behavioral contracts in prose. It does not contain
bodies.

Read `visits-overlay.md` first. Work one step at a time. Write tests
before code. Open `visits-overlay-solution.md` only if you are stuck for
more than twenty minutes, or after the sub-chapter is complete.

---

## Step 0 — `overlay.rs`: policy overlay types and helpers

### 0.1 Extend the module doc comment

Update the doc comment of `src/tui/overlay.rs` to mention the policy
heatmap helpers as well as `value_strip`.

### 0.2 Add the `engine::Move` import

Import `engine::Move` at the top of `src/tui/overlay.rs`.

### 0.3 Create `PolicyOverlay`

Write a public enum:

```rust
pub enum PolicyOverlay { Off, Visits }
```

* Derive `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, and `Default`.
* `Default` returns `Off`.
* Implement `cycle` that returns `Visits` for `Off` and `Off` for
  `Visits`.
* Implement `label` that returns `"off"` for `Off` and `"visits"` for
  `Visits`.

Mark both methods `#[must_use]`.

### 0.4 Create `OverlayCell`

Write a public struct with three public fields:

```rust
pub struct OverlayCell { intensity: f32, text: String, is_hot: bool }
```

Derive `Debug`, `Clone`, and `PartialEq`.

### 0.5 Create `overlay_cell`

Write a public function:

```rust
pub fn overlay_cell(info: &SearchInfo, mv: Move, mode: PolicyOverlay) -> Option<OverlayCell>
```

Behavior:
* For `PolicyOverlay::Off`, return `None`.
* For `PolicyOverlay::Visits`, call a private `visits_cell` and return
  its result.

Mark it `#[must_use]`.

### 0.6 Create `visits_cell`

Write a private function:

```rust
fn visits_cell(info: &SearchInfo, mv: Move) -> Option<OverlayCell>
```

Behavior:
* Compute `total` as the sum of all visit counts.
* If `total` is zero, return `None`.
* Find the visit count for `mv`; default to zero if absent.
* Compute `share = visits as f32 / total as f32`.
* Set `is_hot = share >= 0.10`.
* Set `text` to `visits.to_string()` when hot, else an empty string.
* Return `Some(OverlayCell { intensity: share, text, is_hot })`.

### 0.7 Create `heatmap_bg`

Write a public function:

```rust
pub fn heatmap_bg(intensity: f32) -> u8
```

Behavior:
* Multiply `intensity` by 10.0, round to the nearest integer, and cast to
  `i32`.
* Return 196 for 10, 208 for 9, 220 for 8, 39 for 3, and 250 for all
  other values.

Mark it `#[must_use]`.

### 0.8 Create the render helpers

Write public functions:

```rust
pub fn render_overlay_cell_styled(glyph: char, cell: &OverlayCell) -> String
pub fn render_overlay_cell_plain(glyph: char, cell: &OverlayCell) -> String
```

Both functions produce a three-character string.

`render_overlay_cell_styled`:
* Compute `bg = heatmap_bg(cell.intensity)`.
* Compute `content = cell_content(glyph, cell)`.
* Return `format!("\x1b[48;5;{bg}m{content}\x1b[0m")`.

`render_overlay_cell_plain`:
* Return `cell_content(glyph, cell)`.

Both are `#[must_use]`.

Write a private helper:

```rust
fn cell_content(glyph: char, cell: &OverlayCell) -> String
```

Behavior:
* If `cell.is_hot`, return `pad_numeric_cell(&cell.text)`.
* Otherwise return `format!(" {} ", glyph)`.

Write a private helper:

```rust
fn pad_numeric_cell(text: &str) -> String
```

Behavior:
* If `text.len() >= 3`, return the first three bytes as a string.
* Otherwise return `format!("{:>3}", text)`.

### 0.9 Tests — group A

* `cycle_order_is_off_visits_off`
* `overlay_off_returns_none`
* `zero_visits_gives_zero_intensity_and_not_hot`
* `single_hot_move_has_full_intensity`
* `visit_share_computes_intensity_correctly`
* `small_visit_share_is_dim`
* `a_share_of_exactly_ten_percent_is_hot`
* `styled_hot_cell_has_ansi_background`
* `plain_hot_cell_has_no_ansi_escapes`

### 0.10 Gate

Run `cargo test -p cli --lib overlay::tests`.

---

## Step 1 — `session.rs`: the test seam

### 1.1 Add `set_search_info`

After `search_info`, add a `#[cfg(test)]` method:

```rust
#[cfg(test)]
pub fn set_search_info(&mut self, info: SearchInfo)
```

Behavior:
* Insert `info` into `self.search_info` at `self.history.cursor()`.

### 1.2 Gate

Run `cargo test -p cli --lib session::tests`.

---

## Step 2 — `input.rs`: the `p` outcome

### 2.1 Extend `InputOutcome`

Add a variant after `ToggleValueStrip`:

```rust
/// Cycle the policy heatmap overlay on the board.
CyclePolicy,
```

### 2.2 Normal-mode key

In `handle_normal_key`, add `KeyCode::Char('p') => return
InputOutcome::CyclePolicy` with the other global keys.

### 2.3 Line-mode mapping

In `line_outcome`, after the `v`/`value` guard and before
`command::parse`, add:

* If the trimmed text equals `p` or `policy` case-insensitively, return
  `InputOutcome::CyclePolicy`.

### 2.4 Tests — group B

* `p_in_normal_mode_cycles_policy_overlay`
* `p_and_policy_in_line_mode_cycle_policy_overlay`

### 2.5 Gate

Run `cargo test -p cli --lib input::tests`.

---

## Step 3 — `render.rs`: board cell overlay

### 3.1 Import the overlay helpers

In `src/tui/render.rs`, import from `crate::tui::overlay`:

```rust
PolicyOverlay, overlay_cell, render_overlay_cell_plain, render_overlay_cell_styled
```

Keep the existing `searching_seat_config` and `value_strip` imports.

### 3.2 Extend `StudyView`

Add a private field:

```rust
policy_overlay: PolicyOverlay,
```

Initialize it to `PolicyOverlay::Off` in `StudyView::new`.

Add a public chainable setter:

```rust
pub fn policy_overlay(mut self, overlay: PolicyOverlay) -> Self
```

Mark it `#[must_use]`.

### 3.3 Forward the overlay through `render_study`

In `render_study`:
* Fetch `search_info = core.search_info()` once.
* Pass `view.policy_overlay` and `search_info` to `study_board_lines`.

### 3.4 Extend `study_board_lines`

Add parameters:

```rust
policy_overlay: PolicyOverlay,
search_info: Option<&crate::actor::SearchInfo>,
```

For each cell, call `study_or_policy_cell` instead of `study_cell`,
passing all arguments.

### 3.5 Add `study_or_policy_cell`

Write a private function:

```rust
fn study_or_policy_cell(
    board: &Board,
    opening: Option<&OpeningSnapshot>,
    mv: Move,
    last: Option<Move>,
    cursor: Option<Move>,
    styled: bool,
    policy_overlay: PolicyOverlay,
    search_info: Option<&crate::actor::SearchInfo>,
) -> String
```

Behavior:
* Compute `is_stone = stone_at(board, opening, mv).is_some()`.
* Compute `is_cursor = cursor == Some(mv)`.
* Compute `is_last = last == Some(mv)`.
* If any of those is true, return `study_cell(board, opening, mv, last,
  cursor, styled)`.
* Else if `policy_overlay == PolicyOverlay::Visits`:
  * If `search_info` is `Some(info)`, return `policy_empty_cell(mv, info,
    styled)`.
  * Else return `study_cell(...)`.
* Else return `study_cell(...)`.

Allow `clippy::too_many_arguments` on this function.

### 3.6 Add `stone_at`

Write a private helper:

```rust
fn stone_at(board: &Board, opening: Option<&OpeningSnapshot>, mv: Move) -> Option<engine::Color>
```

Behavior:
* Return `board.stone_at(mv)` if present.
* Otherwise look up `mv` in `opening.placements` and return the stored
  color.

### 3.7 Add `policy_empty_cell`

Write a private helper:

```rust
fn policy_empty_cell(mv: Move, info: &crate::actor::SearchInfo, styled: bool) -> String
```

Behavior:
* Choose `glyph = '+'` if `mv` is on a hoshi point, else `'.'`.
* Match `overlay_cell(info, mv, PolicyOverlay::Visits)`:
  * `Some(cell)` and `styled` → `render_overlay_cell_styled(glyph, &cell)`.
  * `Some(cell)` and not styled → `render_overlay_cell_plain(glyph, &cell)`.
  * `None` → `format!(" {} ", glyph)`.

### 3.8 Tests — group C

* `policy_overlay_snapshot_with_fake_search_info`

### 3.9 Gate

Run `cargo test -p cli --lib render::tests`.

---

## Step 4 — `study.rs`: shell state and loop wiring

### 4.1 Import `PolicyOverlay`

In `src/tui/study.rs`, add:

```rust
use crate::tui::overlay::PolicyOverlay;
```

### 4.2 Extend `ShellState`

Add a field:

```rust
/// The active board policy heatmap overlay.
policy_overlay: PolicyOverlay,
```

`ShellState` derives `Default`; `PolicyOverlay` defaults to `Off`.

### 4.3 Wire the loop

In `study_loop`:
* Change the `build_frame` call to pass `&shell` instead of
  `shell.show_value_strip`.
* Add an arm for `InputOutcome::CyclePolicy` that:
  * Cycles `shell.policy_overlay`.
  * Sets `message` to `"no search here yet"` when the new overlay is not
    `Off` and `core.search_info()` is `None`.
  * Otherwise sets `message` to `format!("policy overlay: {}",
    shell.policy_overlay.label())`.

### 4.4 Update `build_frame`

Change the signature to accept `shell: &ShellState` instead of
`show_value_strip: bool`. Forward both flags into `StudyView`:

```rust
.show_value_strip(shell.show_value_strip)
.policy_overlay(shell.policy_overlay)
```

### 4.5 Update help text

Add `p              cycle policy overlay (off → visits)` to
`NORMAL_MODE_KEYS` after the `v` line.

### 4.6 Tests — group D

* `p_at_a_fresh_cursor_says_no_search_here_yet`
* `p_after_an_ai_step_shows_the_visit_heatmap`

### 4.7 Gate

Run `cargo test -p cli --lib study::tests`.

---

## Step 5 — Help texts and byte-exact transcript

### 5.1 `src/repl.rs`

Add `p, policy        cycle policy overlay (off → visits)` to `HELP`
between the `v, value` line and the `new` line.

### 5.2 `tests/line_mode.rs`

Add the same line to the byte-exact transcript constant.

### 5.3 Gate

Run `cargo test -p cli`.

---

## Step 6 — Project gates

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

---

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `src/tui/overlay.rs` | `PolicyOverlay` | enum `Off`/`Visits`, `Default` → `Off`, `cycle`, `label` |
| `src/tui/overlay.rs` | `OverlayCell` | `intensity`, `text`, `is_hot` |
| `src/tui/overlay.rs` | `overlay_cell` | dispatches to `visits_cell` for `Visits` mode |
| `src/tui/overlay.rs` | `visits_cell` | share-of-total, 10% inclusive hot threshold |
| `src/tui/overlay.rs` | `heatmap_bg` | ANSI-256 ramp 196/208/220/39/250 |
| `src/tui/overlay.rs` | render helpers | styled adds escapes, plain does not, both three characters |
| `src/tui/overlay.rs` | group A tests | all pass |
| `src/session.rs` | `set_search_info` | `#[cfg(test)]`, inserts at cursor |
| `src/tui/input.rs` | `InputOutcome::CyclePolicy` | variant added |
| `src/tui/input.rs` | `handle_normal_key` | `p` returns cycle outcome |
| `src/tui/input.rs` | `line_outcome` | recognizes `p`/`policy` |
| `src/tui/input.rs` | group B tests | all pass |
| `src/tui/render.rs` | imports | `PolicyOverlay`, `overlay_cell`, render helpers |
| `src/tui/render.rs` | `StudyView.policy_overlay` | field and setter |
| `src/tui/render.rs` | `render_study` | passes overlay and search info to board lines |
| `src/tui/render.rs` | `study_or_policy_cell` | stones/cursor/last take precedence |
| `src/tui/render.rs` | `policy_empty_cell` | hoshi glyph, styled/plain dispatch |
| `src/tui/render.rs` | group C test | passes |
| `src/tui/study.rs` | import | `PolicyOverlay` |
| `src/tui/study.rs` | `ShellState` | has `policy_overlay` |
| `src/tui/study.rs` | `study_loop` | cycles overlay, sets message, passes `&shell` |
| `src/tui/study.rs` | `build_frame` | accepts `&ShellState`, forwards both flags |
| `src/tui/study.rs` | `NORMAL_MODE_KEYS` | lists `p` |
| `src/tui/study.rs` | group D tests | all pass |
| `src/repl.rs` | `HELP` | lists `p, policy` |
| `tests/line_mode.rs` | transcript | contains the new help line |
