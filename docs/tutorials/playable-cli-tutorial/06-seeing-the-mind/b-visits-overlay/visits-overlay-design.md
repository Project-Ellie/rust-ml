# Sub-chapter 06b — The visits heatmap: design paper

## Abstract

This paper records the decisions behind the visits heatmap. The heatmap
is the first cell overlay in the study table. It is a pure function of
the `SearchInfo` stored at the cursor node. Intensity equals share of
total visits; the hot threshold is 10% inclusive; hot cells print their
raw visit count. Stones, last-move brackets, and the cursor marker are
never over-painted. The colour ramp is copied from `patterns::render_heatmap`
to keep the visual lineage consistent, but the ramp is duplicated in
`cli` so the `cli` crate can stay free of the `patterns` dependency.

## Glossary

| Term | Definition |
|------|------------|
| **Cell overlay** | An overlay that paints individual board cells, such as the visits heatmap. |
| **`SearchInfo`** | The record of one AI search: visit distribution, root value, prior policy, searching side, and the config that ran the search. |
| **Share of total visits** | `visits_for_move / sum(all visits)`. It is both the visual intensity and the hot threshold. |
| **Hot threshold** | 10% inclusive. A cell at exactly 10% is hot. |
| **ANSI-256 ramp** | The discrete colour mapping used by `patterns::render_heatmap`: 196, 208, 220, 39, 250. |
| **`set_search_info`** | The `#[cfg(test)]` seam on `SessionCore` that inserts deterministic search info without running MCTS. |

## Context

The playable CLI design paper (§9) divides overlays into two kinds:
*panel* elements that compose freely and *cell* overlays that exclude
each other. Sub-chapter 06a implemented the first panel overlay, the
value strip. This sub-chapter implements the first cell overlay, the
visits heatmap. The value strip and the visits heatmap can be visible at
the same time because one is a panel element and the other is a cell
overlay.

The starting point is sub-chapter 06a. `ShellState` holds `show_value_strip`;
`InputOutcome` has `ToggleValueStrip`; `StudyView` carries the flag to the
renderer. This sub-chapter adds `policy_overlay` to the same state path.

## Decision 1 — Visits mode is share-of-total

A visit heatmap could show raw counts or normalized shares. Raw counts
make strong searches look completely different from weak searches even
when the shape of the distribution is the same. Shares make the visual
intensity independent of the simulation count. A move that receives 75%
of the visits glows the same whether the search ran 200 or 2000
simulations. This is the right invariant for a user-facing heatmap.

The intensity is therefore `visits_for_move / total_visits`. The value
is a 32-bit float and is clamped only by the input data: a move cannot
receive more visits than the total, so the intensity is naturally in
0.0–1.0.

## Decision 2 — The hot threshold is 10% inclusive

Hot cells print their raw visit count. The threshold must be high enough
that the board does not become crowded with numbers, but low enough that
interesting moves are labelled. 10% of total visits is a simple,
communicative boundary. It is inclusive because a boundary that is
exclusive would hide a move that received exactly one tenth of the
search effort, which is clearly a significant move.

The threshold is applied to the share, not to the raw count. This keeps
the label behaviour consistent across different simulation counts.

## Decision 3 — Hot cells print raw visit counts

A hot cell is three characters wide. The raw count is right-aligned and
truncated to three characters. For typical searches this is enough to
show counts up to 999; larger counts are clipped to their first three
digits. This is acceptable because the exact count is available in the
value strip, and the heatmap's job is to show relative importance, not
to be a spreadsheet.

## Decision 4 — Stones, last-move brackets, and the cursor are never over-painted

A cell overlay must not make the board unreadable. Stones are the ground
truth of the position; they must stay visible. The last-move brackets
tell the user where the game last progressed. The cursor marker tells
the user where the next action will apply. All three take precedence
over the heatmap.

`study_or_policy_cell` enforces this by checking `is_stone`, `is_cursor`,
and `is_last` first. Only when none of those are true does the overlay
apply. This keeps the overlay a layer on top of empty cells only.

## Decision 5 — The colour ramp is copied, not imported

The `patterns` crate already has a colour ramp in `render_heatmap`:
196 for full intensity, 208 and 220 for high shares, 39 for a muted
band, and 250 for dim values. The visits heatmap uses the same ramp so
that a user who has seen the patterns demo recognises the colour
language.

However, `cli` is a separate crate and should not depend on `patterns`.
The ramp is therefore duplicated in `cli::tui::overlay::heatmap_bg`. The
doc comment documents the lineage. The duplication is intentional and
small: five colour constants and one match expression.

## Decision 6 — `set_search_info` is a `#[cfg(test)]` seam

Rendering tests need deterministic search info. Running MCTS inside a
rendering test would make the test slow and non-deterministic. The
production API should not expose a way to inject fake search info,
because `SearchInfo` is produced only by real searches.

The solution is a `#[cfg(test)]` method `set_search_info` on
`SessionCore`. Tests can call it, but production code cannot. This keeps
the public API clean while making the renderer testable in isolation.

## Decision 7 — Overlay formatting stays pure

`overlay_cell`, `heatmap_bg`, `render_overlay_cell_styled`, and
`render_overlay_cell_plain` are standalone public functions with no I/O.
They take search info and a move and return render data. This makes them
unit-testable without a real MCTS search or a real terminal. All
behavioural decisions — share computation, hot threshold, colour
mapping, styled vs plain — are covered by fast unit tests.

## Data flow

```text
key 'p'            typed 'policy'
   |                       |
handle_normal_key      line_outcome
   |                       |
   +-------> InputOutcome::CyclePolicy <--------+
                       |
                  study_loop
                       |
       ShellState.policy_overlay.cycle()
                       |
                 build_frame
                       |
            StudyView.policy_overlay
                       |
              render_study
                       |
        study_board_lines + study_or_policy_cell
                       |
              overlay_cell / heatmap_bg
                       |
          styled or plain board cells
```

## References

* [Playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md) — §9 overlays.
* [Sub-chapter 06a — The value strip](../a-value-strip/value-strip.md) — panel overlays and `ShellState`.
* [Sub-chapter 05d — The alternate-screen shell](../05-the-real-table/d-shell/shell.md) — the shell loop.
* [`gomoku/crates/patterns/src/render.rs`](../../../../../gomoku/crates/patterns/src/render.rs) — the source of the ANSI-256 colour ramp.
* [docs/12-gomoku-architecture.md](../../../../12-gomoku-architecture.md) — crate topology and the core/shell split.
