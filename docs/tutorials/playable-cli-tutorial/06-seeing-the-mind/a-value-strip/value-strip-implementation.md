# Sub-chapter 06a — The value strip: implementation plan

This document turns the walkthrough in `value-strip.md` into a
construction order. It names the files, structs, enums, functions, and
tests. It states behavioral contracts in prose. It does not contain
bodies.

Read `value-strip.md` first. Work one step at a time. Write tests before
code. Open `value-strip-solution.md` only if you are stuck for more than
twenty minutes, or after the sub-chapter is complete.

---

## Step 0 — `overlay.rs`: pure value-strip formatting

### 0.1 Add the module declaration

In `src/tui/mod.rs`, add `pub mod overlay;` between `opening` and
`render`.

### 0.2 Create `src/tui/overlay.rs`

#### The `value_strip` function

Write a public function:

```rust
pub fn value_strip(info: Option<&SearchInfo>, current_config: Option<&AiConfig>) -> Vec<String>
```

Behavior:
* If `info` is `None`, return an empty vector.
* Compute `total` as the sum of visit counts.
* Format the value line as `value {sign}{abs} for {side:?}`. The sign is
  `+` for non-negative values and `-` for negative values; the absolute
  value is printed with two decimal places. The side comes from
  `info.side_to_move`.
* Add the line `total visits: {total}`.
* Add the line `top moves:`.
* For the first three entries in `info.visits`, print
  `  {rank}. {move} {share}%`, where `share` is `visits / total * 100`
  printed right-aligned in five characters with one decimal place. Use
  the existing `format_move` helper to format the move.
* Compute `stale`: true when `current_config` is `None` or when it is not
  equal to `info.config`.
* If stale, append `(!searched at sims={info.config.simulations})`.
* Return the vector.

Mark the function `#[must_use]`.

#### The `searching_seat_config` helper

Write a public function:

```rust
pub fn searching_seat_config<'a>(core: &'a SessionCore, info: &SearchInfo) -> Option<&'a AiConfig>
```

Behavior:
* Look up `core.occupant(info.side_to_move)`.
* Return `Some(cfg)` for `Occupant::Ai(cfg)`, `None` for
  `Occupant::Human`.

Mark it `#[must_use]`.

#### Tests — group A

Use private test helpers to build `SearchInfo` values directly.

* `value_strip_formats_sign_and_color`
* `value_strip_negative_value_keeps_sign`
* `value_strip_appends_staleness_when_config_differs`
* `value_strip_is_stale_when_seat_is_human`
* `value_strip_no_search_info_returns_empty`

### 0.3 Gate

Run `cargo test -p cli --lib overlay::tests`.

---

## Step 1 — `input.rs`: the `v` outcome and line-mode mapping

### 1.1 Vocabulary

Add `engine::Status` to the `engine` import.

Add a variant to `InputOutcome`:

```rust
/// Toggle the value strip in the side panel.
ToggleValueStrip,
```

### 1.2 Normal-mode key

In `handle_normal_key`, add `KeyCode::Char('v') => return
InputOutcome::ToggleValueStrip` with the other global keys.

### 1.3 Move the line-mode mapping

Move `line_outcome` and `empty_line_outcome` from `src/tui/study.rs` into
`src/tui/input.rs`.

`line_outcome` contract:
* Trim the line.
* If empty, return `empty_line_outcome(core)`.
* If the trimmed text equals `v` or `value` case-insensitively, return
  `InputOutcome::ToggleValueStrip`.
* Otherwise parse with `command::parse`. On success return
  `InputOutcome::Command(cmd)`; on error return
  `InputOutcome::Message(err.to_string())`.

`empty_line_outcome` contract:
* Get the board from `core.board()`.
* If a Swap2 ceremony is in progress or the board status is not
  `Status::Ongoing`, return `InputOutcome::None`.
* If the side-to-move seat is AI, return
  `InputOutcome::Command(Command::Step)`.
* Otherwise return `InputOutcome::None`.

### 1.4 Tests — group B

* `v_in_normal_mode_toggles_value_strip`
* `v_and_value_in_line_mode_toggle_value_strip`
* `the_empty_line_steps_an_ai_seat_and_stays_quiet_for_a_human_seat`
* `the_empty_line_stays_quiet_during_the_ceremony`
* `the_empty_line_stays_quiet_after_the_game_ends`
* `a_line_maps_through_the_shared_grammar`
* `a_parse_error_is_a_message_with_the_parsers_text`

### 1.5 Gate

Run `cargo test -p cli --lib input::tests`.

---

## Step 2 — `study.rs`: shell state and loop wiring

### 2.1 Clean up imports

Remove `command` from the imports and remove `Status` from the engine
import. Add `line_outcome` to the `input` import.

Update the `LineInput` doc comment to say that lines map through
`crate::tui::input::line_outcome` and that `v`/`value` toggle the value
strip.

### 2.2 Add `ShellState`

Write a private struct:

```rust
#[derive(Debug, Default)]
struct ShellState {
    show_value_strip: bool,
}
```

### 2.3 Wire the loop

In `study_loop`:
* Create `let mut shell = ShellState::default();` before the loop.
* Pass `shell.show_value_strip` to `build_frame`.
* Add an arm for `InputOutcome::ToggleValueStrip` that flips
  `shell.show_value_strip` and sets the message line to `"value strip on"`
  or `"value strip off"`.

### 2.4 Update `build_frame`

Add a `show_value_strip: bool` parameter. Forward it to
`StudyView::new(...).styled(styled).show_value_strip(show_value_strip)`.

### 2.5 Update help text

Add `v              toggle value strip` to `NORMAL_MODE_KEYS`.

### 2.6 Tests — group C

* `v_toggles_the_value_strip_in_the_frame`

Remove the line-mode tests that now live in `input.rs`.

### 2.7 Gate

Run `cargo test -p cli --lib study::tests`.

---

## Step 3 — `render.rs`: the side-panel block

### 3.1 Import the overlay helpers

In `src/tui/render.rs`, import `searching_seat_config` and `value_strip`
from `crate::tui::overlay`.

### 3.2 Extend `StudyView`

Add a private field:

```rust
show_value_strip: bool,
```

Initialize it to `false` in `StudyView::new`.

Add a public chainable setter:

```rust
pub fn show_value_strip(mut self, show: bool) -> Self
```

Mark it `#[must_use]`.

### 3.3 Forward the flag

In `render_study`, pass `view.show_value_strip` to `render_side_panel`.
Add the matching parameter to `render_side_panel`.

### 3.4 Draw the strip in the side panel

In `render_side_panel`, after the phase/game-state block and before the
`Last events` block, conditionally append:

* a blank line,
* the header `Value strip:`,
* if `core.search_info()` is `Some(info)`: the lines returned by
  `value_strip(Some(info), searching_seat_config(core, info))`,
* else the line `  (no search here yet)`.

### 3.5 Tests — group D

* `the_value_strip_is_hidden_by_default`
* `the_value_strip_explains_when_there_is_no_search`
* `the_panel_shows_the_value_strip_when_enabled`

### 3.6 Gate

Run `cargo test -p cli --lib render::tests`.

---

## Step 4 — Help texts and byte-exact transcript

### 4.1 `src/repl.rs`

Add `v, value         toggle the value strip` to `HELP` between `info`
and `new`.

### 4.2 `tests/line_mode.rs`

Add the same line to the byte-exact `TWO_MOVE_TRANSCRIPT` constant.

### 4.3 Gate

Run `cargo test -p cli`.

---

## Step 5 — Project gates

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
| `src/tui/mod.rs` | `pub mod overlay;` | declared |
| `src/tui/overlay.rs` | `value_strip` | returns formatted lines, handles `None`, appends staleness |
| `src/tui/overlay.rs` | `searching_seat_config` | returns live AI config or `None` for human |
| `src/tui/overlay.rs` | group A tests | all pass |
| `src/tui/input.rs` | `Status` import | present for `empty_line_outcome` |
| `src/tui/input.rs` | `InputOutcome::ToggleValueStrip` | variant added |
| `src/tui/input.rs` | `handle_normal_key` | `v` returns toggle outcome |
| `src/tui/input.rs` | `line_outcome` | moved from `study.rs`, recognizes `v`/`value` |
| `src/tui/input.rs` | `empty_line_outcome` | moved from `study.rs`, same guards |
| `src/tui/input.rs` | group B tests | all pass |
| `src/tui/study.rs` | imports | no `command`, no `Status`, imports `line_outcome` |
| `src/tui/study.rs` | `ShellState` | private struct with `show_value_strip` |
| `src/tui/study.rs` | `study_loop` | toggles flag, sets message, passes flag to frame |
| `src/tui/study.rs` | `build_frame` | forwards flag to `StudyView` |
| `src/tui/study.rs` | `NORMAL_MODE_KEYS` | lists `v` |
| `src/tui/study.rs` | group C tests | all pass |
| `src/tui/render.rs` | imports overlay helpers | present |
| `src/tui/render.rs` | `StudyView.show_value_strip` | field and setter |
| `src/tui/render.rs` | `render_side_panel` | accepts flag, inserts strip block |
| `src/tui/render.rs` | group D tests | all pass |
| `src/repl.rs` | `HELP` | lists `v, value` |
| `tests/line_mode.rs` | transcript | contains the new help line |
