# Chapter 06 — Seeing the mind

Chapter 05 put the study table on the screen. This chapter puts the AI's
search reasoning on the screen. You can now toggle a value strip in the
side panel and a policy heatmap over the board. The heatmap cycles
through two readings of the same search: visits show where the search
spent its time; prior shows what the evaluator believed before the
search started. A third overlay, threat-space search, paints a verified
forced-win line. The table still shows moves; now it also shows the
thinking behind them.

## Architecture

```text
SessionCore
   │
   ▼
search_info() at cursor ──► SearchInfo
                                 │
                                 ▼
overlay.rs       pure functions: value_strip, overlay_cell,
                 heatmap_bg, TssOverlay, tss_verdict
                                 │
                                 ▼
StudyView        ShellState holds the toggles:
                 show_value_strip, policy_overlay, tss_proof
                                 │
                                 ▼
render.rs        builds the frame: board cells + side panel
                                 │
                                 ▼
Screen::draw     AltScreen or LineScreen

input.rs         translates p, v, t into InputOutcome values
                 (CyclePolicy, ToggleValueStrip, ToggleTss)
```

The rule is the same as in chapter 05: rendering depends on the session,
the shell depends on rendering, never the other direction. The overlay
data comes from `SessionCore::search_info()`. The overlay state lives in
`ShellState`. `input.rs` turns keys and typed words into outcomes, but
the outcomes are shell concerns; they never become `Command` variants.

## The sub-chapters

The old monolithic chapter 06 has been restructured into four
sub-chapters. Each sub-chapter has its own tutorial, design paper,
implementation notes, and opt-in solution file in its folder.

| sub-chapter | what it teaches |
|---|---|
| [a — The value strip](06-seeing-the-mind/a-value-strip/value-strip.md) | The first panel overlay: root value, total visits, top-3 moves, and the staleness note. |
| [b — The visits heatmap](06-seeing-the-mind/b-visits-overlay/visits-overlay.md) | The first cell heatmap: share of total visits as intensity, raw visit count in hot cells. |
| [c — The prior view](06-seeing-the-mind/c-prior-overlay/prior-overlay.md) | The second policy flavor: max-normalized prior probabilities and the raw-percentage label. |
| [d — The TSS overlay + mutual exclusion](06-seeing-the-mind/d-tss-overlay/tss-overlay.md) | The verified forced-win line and the rule that policy and TSS cannot both be active. |

## Background documents

* [00-design.md](06-seeing-the-mind/00-design.md) — the design paper:
  the two policy flavors, the searching-side derivation, why overlay
  state lives in the shell, mutual exclusion versus composition, the
  copied heatmap idiom, and the verify-before-paint gate.

Next: [Chapter 07 — Positions from files: puzzles](07-positions-from-files-puzzles.md)

## Glossary

| Term | Definition |
|------|------------|
| **Policy overlay** | A per-cell heatmap over the board, sourced from the cursor node's `SearchInfo`. Cycles off → visits → prior with the `p` key. |
| **Visits overlay** | Heatmap by share of total visits — what the search concluded after simulations. |
| **Prior overlay** | Heatmap by the evaluator's post-softmax probabilities, normalized to the maximum prior — what the evaluator believed before search. |
| **TSS overlay** | The cell overlay that paints a verified forced-win line from the engine's threat-space-search prover, numbered and bracketed. |
| **Hot cell** | A cell whose share is at least 10%; its value is printed inside the cell. |
| **Value strip** | The side-panel readout showing root value, total visits, and top-3 moves, plus a staleness note when configs changed since the search. |
| **Searching side** | The side that ran the search stored on the cursor node; it is the opposite of `board().to_move()` because `SearchInfo` lives on the child node. The `SearchInfo::side_to_move` field carries it explicitly. |
| **Config fingerprint** | The `AiConfig` copied into `SearchInfo` at search time; used to detect staleness. |
| **Mutual exclusion** | Cell overlays (`p` policy and `t` TSS) clear each other; the value strip composes with everything. |
| **ANSI-256 heatmap** | The discrete colour ramp copied from `patterns::render_heatmap`: red → orange → yellow for hot cells, fading to grey for dim cells. |

## References

* [00-playable-cli-primer.md](00-playable-cli-primer.md) — the study
  table, core/shell, the four knobs, and the overlay summary in §9.
* [03-the-ai-takes-a-seat.md](03-the-ai-takes-a-seat.md) —
  `SearchInfo`, value sign, and the negate-once backup convention.
* [05-the-real-table.md](05-the-real-table.md) — the study-table
  screen and side panel that this chapter extends.
* [README.md](README.md) — chapter map and tutorial conventions.
* [docs/plans/2026-10-17-playable-cli.md](../../plans/2026-10-17-playable-cli.md)
  — Task 6 (this chapter).
* [docs/specs/2026-10-17-playable-cli-design.md](../../specs/2026-10-17-playable-cli-design.md)
  — §9 (overlays and the value strip).
* The per-sub-chapter solution files live in the `a-value-strip`,
  `b-visits-overlay`, `c-prior-overlay`, and `d-tss-overlay` folders.
