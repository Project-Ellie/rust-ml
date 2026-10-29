# Sub-chapter 06a — The value strip: design paper

## Abstract

This paper records the decisions behind the value strip. The strip is the
first search overlay in the study table. It is a panel element, not a cell
overlay, so it composes with everything else. Its state lives in the
shell, not in the session core or the command grammar. The searching
perspective is carried by `SearchInfo::side_to_move`, and staleness is
detected by comparing the live seat config with the config fingerprint
stored in the search record.

## Glossary

| Term | Definition |
|------|------------|
| **Shell** | The terminal-facing loop that draws frames and reads input. It is separate from `SessionCore`. |
| **Command grammar** | `command::parse` and the `Command` enum: the shared vocabulary that changes session state. |
| **Input outcome** | The `InputOutcome` enum produced by the input layer for each key or line. |
| **Overlay state** | Visual-only state owned by the shell, such as whether the value strip is visible. |
| **Cell overlay** | An overlay that paints individual board cells, such as a visit-count heatmap or a TSS proof overlay. |
| **Config fingerprint** | The `AiConfig` value stored in `SearchInfo` when the search ran. |

## Context

The playable CLI design paper (§9) divides overlays into two kinds:
*panel* elements that compose freely and *cell* overlays that exclude each
other. It also states that overlay *state* is a shell concern, while
overlay *data* comes from the core. This sub-chapter implements the first
panel overlay: the value strip.

The starting point is sub-chapter 05d. The shell loop keeps `InputState`
(mode and cursor), a capped event list, and a message line. The input
layer returns `InputOutcome` values. The renderer builds frames from a
`StudyView`.

## Decision 1 — Overlay state stays in the shell

The value strip could have been a new `Command` variant. That would be a
mistake. `Command` is the shared vocabulary for changing session state:
playing moves, stepping the AI, taking or releasing seats. Toggling a
panel element changes no session state, so it does not belong in that
vocabulary.

Instead, the input layer returns `InputOutcome::ToggleValueStrip`. The
shell loop owns the boolean in `ShellState`. This keeps the command
grammar small and keeps the shell responsible for everything visual.

## Decision 2 — `SearchInfo` carries the searching side

The root value is from the searching side's perspective: a value of
`+0.42` means the side that moved next was ahead by that amount. The
`SearchInfo` stored on a node therefore records `side_to_move`.

The old `print_search_info` helper in `repl.rs` printed the raw value
without naming the side. The value strip names the side explicitly:
`value +0.42 for Black`. Callers no longer pass a side; they read it from
the search record. This removes a parameter and removes a source of
inconsistency.

## Decision 3 — Staleness is a config comparison

A stored `SearchInfo` was produced with a specific `AiConfig`. If the
user later changes `simulations`, `c_puct`, or any other config knob, the
old search no longer reflects what the AI would think now. The value
strip must warn the user.

The strip receives the live config from `searching_seat_config`, which
looks up the current occupant of `info.side_to_move`. If the seat is
human, the live config is `None` and the strip is stale. If the seat is
AI, the strip compares the live config with `info.config`. A mismatch
appends `(!searched at sims=N)`.

This fingerprint is exact and cheap. It does not require a timestamp or a
version counter; the config value is the only fact that matters for
interpretation.

## Decision 4 — The strip composes; cell overlays exclude each other

The design paper reserves cell overlays for mutually exclusive heatmaps:
visit counts, prior probabilities, and TSS proofs all paint the same
board cells, so only one can be active at a time. The value strip does
not paint cells. It lives in the side panel, so it can stay visible while
cell overlays cycle. Sub-chapters 06b–06d implement the cell overlays and
enforce mutual exclusion there, not here.

## Decision 5 — `line_outcome` moves to `input.rs`

Before this sub-chapter, `line_outcome` and `empty_line_outcome` lived in
`study.rs`, the loop module. That split the input translation layer
across two files: keys in `input.rs`, lines in `study.rs`.

The move unifies translation. Both keys and lines now produce
`InputOutcome` in the pure `input.rs` module. The loop module only reads
outcomes and acts on them. The `LineInput` doc comment is updated to
point at `input::line_outcome`. This makes the `v`/`value` recognition
live next to the `v` key recognition, where a reader expects it.

## Decision 6 — `ShellState` is separate from `InputState`

`InputState` owns mode and cursor. These are properties of the user's
hands and the command-line editor. `ShellState` owns display-only state:
the value strip visibility. The two states change for different reasons
and reset at different times. Keeping them separate makes the loop's
match arm for `ToggleValueStrip` a pure update of display state, with no
overlap with cursor motion or buffer editing.

## Decision 7 — Overlay formatting is a pure function

`value_strip` is a standalone public function with no I/O. It takes the
search info and the live config and returns lines. This makes it
unit-testable without a real MCTS search: a test can build a `SearchInfo`
value directly and assert on the formatted strings. All formatting
decisions — sign handling, visit shares, staleness — are covered by
fast unit tests.

## Data flow

```text
key 'v'              typed 'value'
   |                       |
handle_normal_key      line_outcome
   |                       |
   +-------> InputOutcome::ToggleValueStrip <--------+
                       |
                  study_loop
                       |
              ShellState.show_value_strip
                       |
                 build_frame
                       |
            StudyView.show_value_strip
                       |
             render_side_panel
                       |
       searching_seat_config + value_strip
                       |
              side panel lines
```

## References

* [Playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md) — §9 overlays.
* [Sub-chapter 05d — The alternate-screen shell](../05-the-real-table/d-shell/shell.md) — the shell loop.
* [Sub-chapter 05a — Refactor](../05-the-real-table/a-refactor/refactor.md) — the model for sub-chapter documentation.
* [docs/12-gomoku-architecture.md](../../../../12-gomoku-architecture.md) — crate topology and the core/shell split.
