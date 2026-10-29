# Sub-chapter 06a — The value strip

## Abstract

This sub-chapter gives the study table a side-panel value strip. The strip
shows the root value, total visits, and top-3 moves from the AI search
stored at the cursor position. It reads the searching side from
`SearchInfo::side_to_move`, so the value is always shown from that side's
perspective. The user toggles the strip with the `v` key in normal mode or
with the typed word `v`/`value` in line mode. If the live seat config has
changed since the search, or if the seat is now human, the strip appends a
staleness note.

## Glossary

| Term | Definition |
|------|------------|
| **Value strip** | The side-panel block that displays root value, total visits, and top-3 moves for the search at the cursor. |
| **`SearchInfo`** | The record of one AI search: visit distribution, root value, prior policy, searching side, and the config that ran the search. |
| **`side_to_move`** | The color whose turn it was when the search ran. The root value is from that color's perspective. |
| **`AiConfig`** | The AI seat configuration, including the simulation count used as a staleness fingerprint. |
| **Staleness note** | The line `(!searched at sims=N)` that appears when the stored search config no longer matches the live config or the seat is human. |
| **`ShellState`** | Display state owned by the shell loop in `study.rs`. It survives across loop iterations and resets when the shell restarts. |
| **`InputOutcome::ToggleValueStrip`** | The outcome that the input layer returns for both the `v` key and the typed `v`/`value` line. |
| **`line_outcome`** | The function in `input.rs` that translates one input line into an `InputOutcome`. |
| **`empty_line_outcome`** | The helper that decides what an empty line means in line mode: `step` when the side-to-move seat is AI, nothing otherwise. |

## Context

Read [the playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md)
first. Section 9 states that overlay *state* is a shell concern and that
the value strip lives in the side panel. Section 9 also says that panel
elements compose freely, while cell overlays exclude each other — the
cell overlays arrive in sub-chapters 06b–06d; this sub-chapter adds the
panel element that composes with all of them.

Sub-chapter 05d built the shell loop: `input.rs` translates keys and typed
lines into `InputOutcome` values; `study.rs` runs the loop; `render.rs`
paints frames from a `StudyView`. This sub-chapter adds the first overlay
on top of that loop without changing the session core or the command
grammar.

## Intention

The learner enables this user-facing behavior:

1. Press `v` in normal mode, or type `v` or `value` in line mode, to show
   or hide a value strip in the side panel.
2. When the strip is visible and the cursor position has stored search
   info, the panel shows the root value from the searching side's
   perspective, the total number of visits, and the three most-visited
   moves with their visit shares.
3. When the live config of the searching seat differs from the config that
   ran the search, or when the seat is now human, the strip warns the user
   with a staleness note.
4. The strip composes with the existing board, seat table, phase block,
   and last-events list.

## Specification as tests

The tests below are ordered along the execution path of a `v` keypress
from input translation through to the painted frame.

### Input translation

* `v_in_normal_mode_toggles_value_strip` — the `v` key returns
  `InputOutcome::ToggleValueStrip`.
* `v_and_value_in_line_mode_toggle_value_strip` — the typed words `v`,
  `V`, `value`, and `VALUE` return `InputOutcome::ToggleValueStrip`.
* `the_empty_line_steps_an_ai_seat_and_stays_quiet_for_a_human_seat` —
  line mode keeps the reference behavior for an empty line.
* `the_empty_line_stays_quiet_during_the_ceremony` — the empty line does
  not step while the Swap2 ceremony is in progress.
* `the_empty_line_stays_quiet_after_the_game_ends` — the empty line does
  not step after the game is finished.
* `a_line_maps_through_the_shared_grammar` — non-empty, non-`v`/`value`
  lines still go through `command::parse`.
* `a_parse_error_is_a_message_with_the_parsers_text` — a parse error in
  line mode is shown with the parser's own message.

### Overlay formatting

* `value_strip_formats_sign_and_color` — a positive value gets a `+`
  sign and the searching side's name.
* `value_strip_negative_value_keeps_sign` — a negative value keeps the
  `-` sign.
* `value_strip_appends_staleness_when_config_differs` — a config
  mismatch appends the staleness note.
* `value_strip_is_stale_when_seat_is_human` — a human seat is treated as
  stale.
* `value_strip_no_search_info_returns_empty` — no search info produces
  no lines.

### Rendering

* `the_value_strip_is_hidden_by_default` — a fresh frame does not show
  the strip.
* `the_value_strip_explains_when_there_is_no_search` — with the strip
  enabled but no search info, the panel shows an explanatory line.
* `the_panel_shows_the_value_strip_when_enabled` — with the strip
  enabled and search info present, the panel shows the value, visits,
  and top-moves lines.

### Loop integration

* `v_toggles_the_value_strip_in_the_frame` — pressing `v` in a scripted
  session adds the strip block to the next frame.

## Walkthrough

### 1. The user presses `v` or types `value`

In normal mode `handle_key` in `input.rs` checks the global keys first.
`KeyCode::Char('v')` returns `InputOutcome::ToggleValueStrip` immediately.
The key does not depend on the phase: the strip is a view toggle, not a
command that changes the session.

In line mode the source reads one complete line. `line_outcome` in
`input.rs` trims the line. If the trimmed text is `v` or `value`,
case-insensitively, it returns `InputOutcome::ToggleValueStrip`. Every
other non-empty line still goes through `command::parse`. The overlay is a
shell concern, so the command grammar does not gain a new variant for it.

### 2. The line-mode move

This sub-chapter moves `line_outcome` and `empty_line_outcome` from
`study.rs` into `input.rs`. Both normal-mode keys and typed lines now
produce `InputOutcome` values in the same pure module. The line-mode
source in `study.rs` calls `line_outcome` and then sees the same outcome
variants as a key-driven source. The empty-line rule stays unchanged:
step when the side-to-move seat is AI, with guards for a finished game
and for the Swap2 ceremony.

### 3. The loop receives `ToggleValueStrip`

`study_loop` in `study.rs` owns a new `ShellState` value. The variant
flips `shell.show_value_strip` and writes `"value strip on"` or `"value
strip off"` to the message line. `ShellState` is separate from
`InputState` because the cursor and mode are input state, while the strip
is display state. It resets when the shell restarts.

### 4. The frame carries the flag

`build_frame` passes `shell.show_value_strip` into
`StudyView::show_value_strip`. The renderer cannot know whether the strip
is enabled; the view carries that flag the same way it carries the cursor
and the message line.

### 5. The side panel renders the strip

`render_side_panel` in `render.rs` checks the flag. When the flag is true
it inserts a blank line, the header `Value strip:`, and either the search
readout or the explanatory line `(no search here yet)`. The block sits
between the phase/game-state block and the `Last events` block, so it
composes with the rest of the panel.

If search info exists, the panel calls `searching_seat_config` to get the
live config of the seat that searched the cursor position. It then calls
`value_strip` with both the stored `SearchInfo` and the live config.

### 6. `value_strip` formats the readout

`value_strip` in `overlay.rs` is a pure function. It takes
`Option<&SearchInfo>` and `Option<&AiConfig>` and returns `Vec<String>`.
It formats the value with a sign, the searching side from
`info.side_to_move`, the total visits, and the top-3 moves with visit
shares. It computes staleness by comparing the live config against
`info.config`; `None` means the seat is human, which is also stale. The
function has no I/O, so every case is unit-testable with a hand-built
`SearchInfo`.

### 7. Help texts stay consistent

The normal-mode key table in `study.rs` gains `v toggle value strip`. The
line interpreter's `HELP` in `repl.rs` gains `v, value toggle the value
strip`. The byte-exact integration transcript in `tests/line_mode.rs`
grows by one matching line.

## Low-level design

### New file: [`gomoku/crates/cli/src/tui/overlay.rs`](../../../../../gomoku/crates/cli/src/tui/overlay.rs)

Pure functions only. Two public items:

* `value_strip(info: Option<&SearchInfo>, current_config: Option<&AiConfig>) -> Vec<String>`
  — formats the strip lines. Returns an empty vector when `info` is
  `None`. Appends the staleness note when the live config differs or is
  absent.
* `searching_seat_config<'a>(core: &'a SessionCore, info: &SearchInfo) -> Option<&'a AiConfig>`
  — returns the live AI config for the seat that searched the cursor
  position, or `None` if the seat is human.

The module has its own unit tests for all five formatting cases.

### [`gomoku/crates/cli/src/tui/input.rs`](../../../../../gomoku/crates/cli/src/tui/input.rs)

* Import `engine::Status` so `empty_line_outcome` can check the game
  status.
* Add `InputOutcome::ToggleValueStrip` between `Message` and `None`.
* In `handle_normal_key`, match `KeyCode::Char('v')` with the other
  global keys.
* Move `line_outcome` and `empty_line_outcome` from `study.rs` into this
  module. Add the `v`/`value` guard before `command::parse`.
* Add the two `v` tests and keep the existing line-mode tests that now
  live here.

### [`gomoku/crates/cli/src/tui/study.rs`](../../../../../gomoku/crates/cli/src/tui/study.rs)

* Remove `command` from imports and remove `Status` from the engine
  import. Import `line_outcome` from `input.rs`.
* Update the `LineInput` doc comment to mention `line_outcome` and the
  two shell-level words.
* Add `ShellState { show_value_strip: bool }` next to the loop state.
* In `study_loop`, create `ShellState::default()` and pass
  `shell.show_value_strip` to `build_frame`. Handle
  `InputOutcome::ToggleValueStrip` by toggling the flag and setting the
  message line.
* Extend `build_frame` with a `show_value_strip` parameter and forward
  it to `StudyView::show_value_strip`.
* Add `v toggle value strip` to `NORMAL_MODE_KEYS`.
* Add `v_toggles_the_value_strip_in_the_frame`.
* Remove the line-mode tests that moved to `input.rs`.

### [`gomoku/crates/cli/src/tui/render.rs`](../../../../../gomoku/crates/cli/src/tui/render.rs)

* Import `searching_seat_config` and `value_strip` from `overlay.rs`.
* Add `show_value_strip: bool` to `StudyView` and initialize it to
  `false`.
* Add the chainable `show_value_strip` setter.
* Forward the flag through `render_study` into `render_side_panel`.
* In `render_side_panel`, between the phase/game-state block and the
  `Last events` block, conditionally append the strip block.
* Add the three value-strip rendering tests.

### [`gomoku/crates/cli/src/tui/mod.rs`](../../../../../gomoku/crates/cli/src/tui/mod.rs)

* Add `pub mod overlay;`.

### [`gomoku/crates/cli/src/repl.rs`](../../../../../gomoku/crates/cli/src/repl.rs)

* Add `v, value         toggle the value strip` to `HELP`.

### [`gomoku/crates/cli/tests/line_mode.rs`](../../../../../gomoku/crates/cli/tests/line_mode.rs)

* Add the same `v, value` line to the byte-exact transcript constant.

## TDD checklist

Work one test group at a time, red to green.

From `gomoku/crates/cli/src/tui/input.rs`:

1. `v_in_normal_mode_toggles_value_strip`
2. `v_and_value_in_line_mode_toggle_value_strip`
3. `the_empty_line_steps_an_ai_seat_and_stays_quiet_for_a_human_seat`
4. `the_empty_line_stays_quiet_during_the_ceremony`
5. `the_empty_line_stays_quiet_after_the_game_ends`
6. `a_line_maps_through_the_shared_grammar`
7. `a_parse_error_is_a_message_with_the_parsers_text`

From `gomoku/crates/cli/src/tui/overlay.rs`:

8. `value_strip_formats_sign_and_color`
9. `value_strip_negative_value_keeps_sign`
10. `value_strip_appends_staleness_when_config_differs`
11. `value_strip_is_stale_when_seat_is_human`
12. `value_strip_no_search_info_returns_empty`

From `gomoku/crates/cli/src/tui/render.rs`:

13. `the_value_strip_is_hidden_by_default`
14. `the_value_strip_explains_when_there_is_no_search`
15. `the_panel_shows_the_value_strip_when_enabled`

From `gomoku/crates/cli/src/tui/study.rs`:

16. `v_toggles_the_value_strip_in_the_frame`

## Verification commands

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Done when

* The `v` key and the typed words `v`/`value` toggle the value strip.
* The strip shows root value, total visits, top-3 moves, and a staleness
  note when appropriate.
* The strip is hidden by default and can explain that no search exists
  yet.
* All sixteen tests above pass.
* The three project gates pass.
* A scripted session shows the strip in the side panel. From `gomoku/`,
  run:

  ```text
  printf 'h8\nstep\nv\nq\n' | cargo run -q -p cli --bin gomoku -- play --ai tactics --no-swap2 --sims 200
  ```

  A typical frame looks like this:

```text
 7 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 7   Value strip:
 8 | .  .  .  +  .  .  .  x  .  .  .  +  .  .  . | 8   value +0.00 for White
 9 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 9   total visits: 200
10 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 10  top moves:
11 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 11    1. a1   0.5%
12 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 12    2. b1   0.5%
13 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 13    3. c1   0.5%
14 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 14  
15 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 15  Last events:
```

## References

* [Playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md) — §9 overlays and the value strip.
* [Sub-chapter 05d — The alternate-screen shell](../05-the-real-table/d-shell/shell.md) — the loop and `InputOutcome` this sub-chapter extends.
* [Sub-chapter 05a — Refactor](../05-the-real-table/a-refactor/refactor.md) — the chapter shape and TDD checklist style.
* [`gomoku/crates/cli/src/tui/input.rs`](../../../../../gomoku/crates/cli/src/tui/input.rs) — input translation.
* [`gomoku/crates/cli/src/tui/overlay.rs`](../../../../../gomoku/crates/cli/src/tui/overlay.rs) — pure overlay formatting.
* [`gomoku/crates/cli/src/tui/render.rs`](../../../../../gomoku/crates/cli/src/tui/render.rs) — frame rendering.
* [`gomoku/crates/cli/src/tui/study.rs`](../../../../../gomoku/crates/cli/src/tui/study.rs) — the shell loop.
