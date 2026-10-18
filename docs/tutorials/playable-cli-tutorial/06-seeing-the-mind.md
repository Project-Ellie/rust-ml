# Chapter 06 — Seeing the mind

## Abstract

This chapter is the emotional peak of the tutorial: **now you can see
what the AI thinks.** Everything before it — the variation tree, the
command language, the seats, the rendered board — was scaffolding. This
chapter puts the search's reasoning on the screen.

You will add a policy overlay that cycles through two views — **visits**
and **prior** — and a **value strip** that quantifies the search's
assessment. Together they answer three questions at a glance:

* *Where* did the search spend its time? (visits heatmap)
* *What* did the evaluator believe before the search started? (prior
  heatmap)
* *How* does the search rate the position? (value strip)

The mental-mapping centerpiece is the **two policy flavors**: prior is
the evaluator's raw belief, visits is the search's improved belief. On a
tactical position they can disagree, and that disagreement — search
correcting the prior — is the lesson this chapter points at.

## Glossary

| Term | Definition |
|------|------------|
| **Policy overlay** | A per-cell heatmap over the board, sourced from the cursor node's `SearchInfo`. Cycles off → visits → prior with the `p` key. |
| **Visits overlay** | Heatmap by share of total visits — what the search concluded after simulations. |
| **Prior overlay** | Heatmap by the evaluator's post-softmax probabilities — what the evaluator believed before search. |
| **Hot cell** | A cell whose share is at least 10%; its value is printed inside the cell. |
| **Value strip** | The side-panel readout showing root value, total visits, and top-3 moves, plus a staleness note when configs changed since the search. |
| **Searching side** | The side that ran the search stored on the cursor node; it is the opposite of `board().to_move()` because `SearchInfo` lives on the child node. |
| **Config fingerprint** | The `AiConfig` copied into `SearchInfo` at search time; used to detect staleness. |
| **Mutual exclusion** | Cell overlays (`p` policy and `t` TSS) clear each other; the value strip composes with everything. |
| **ANSI-256 heatmap** | The discrete colour ramp copied from `patterns::render_heatmap`: red → orange → yellow for hot cells, fading to grey for dim cells. |

## Context

Chapter 05 put the study table on screen: a rendered board, a side
panel, the command loop, and the non-tty fallback. The table works, but
it only shows *moves*. This chapter shows the *thinking* behind them.

The data already exists. Chapter 03 stored a `SearchInfo` on every node
produced by an AI `Step`: sorted visits, sorted prior, root value, and a
config fingerprint. Chapter 06 is purely a visualisation layer on top of
that data.

The design authority is the playable CLI spec §9 (overlays and the value
strip). Three constraints carry over from earlier chapters:

1. **Overlay state lives in the shell.** `PolicyOverlay` and
   `show_value_strip` are crossterm concerns; the core only provides
   `search_info()`.
2. **Cell overlays are mutually exclusive.** Activating the policy
   overlay clears the TSS overlay and vice versa, just as the legacy UI
   never showed two cell overlays at once.
3. **The value strip composes with everything.** It is a panel element,
   not a cell overlay, so it stays visible alongside policy or TSS.

## Intention

1. Create `gomoku/crates/cli/src/tui/overlay.rs`:
   * define `PolicyOverlay` and its `cycle()` order
     (Off → Visits → Prior → Off);
   * implement `overlay_cell` for visits and prior, with the 10% hot-cell
     threshold and the ANSI-256 `heatmap_bg` ramp;
   * implement `value_strip` with side-to-move sign, total visits, top-3
     moves, and the staleness note when the current config differs from
     the fingerprint.
2. Extend `gomoku/crates/cli/src/tui/render.rs`:
   * render the study board with a policy overlay when the shell asks;
   * add `render_study_with_overlays` and wire it into
     `render_study`;
   * keep stones visible — they are never over-painted by the heatmap.
3. Extend `gomoku/crates/cli/src/tui/study.rs`:
   * add `p` to cycle the policy overlay and clear any TSS overlay;
   * add `v` to toggle the value strip;
   * keep the `t` TSS command working (it already clears the policy
     overlay in the reference).
4. Understand and document the **searching-side derivation**: because
   `SearchInfo` is stored on the child node produced by the search, the
   side that searched is `core.board().to_move().other()`.

Observable done-state: a scripted non-tty session builds a tactical
position, toggles visits, prior, and the value strip, and the TDD
checklist passes.

## Mental mapping

### Two flavours of belief

`SearchInfo` carries two policies:

| Field | Meaning | Tuning surface |
|-------|---------|----------------|
| `visits` | Root-edge visit counts after `simulations` PUCT descents. | More simulations → more reliable. |
| `prior` | Root-edge probabilities right after the evaluator's softmax (after Dirichlet noise, if any). | A property of the evaluator; the tactics evaluator shapes it from immediate wins/blocks. |

The **prior** is the evaluator's raw opinion before it has seen any
search. The **visits** are the search's opinion after it has explored
the tree. They can differ, and when they do the disagreement is the
whole point: MCTS is designed to improve on the prior through
simulation.

With the tactics evaluator the prior is already tactical, so on an open
four the two overlays often point at the same block cells. The lesson is
still there: prior says "this move looks important because the evaluator
says so"; visits say "this move is important because the simulations
kept coming back to it". In a trained net the prior can be confidently
wrong about a deep tactic, and the visit map is how you catch it.

### Why the searching side is the opposite of to_move

This is easy to get backwards. `SearchInfo` is stored on the **child
node** that the AI's move created. If the cursor is on that child, then:

* `core.board().to_move()` is the side whose turn it is *now*.
* The side that *ran* the search is the one who just moved, i.e.
  `core.board().to_move().other()`.

The value strip therefore labels the value as "for White" when the
cursor shows Black to move and the last move was White's search. The
`overlay.rs` helper takes `side_at_search` explicitly; `study.rs` passes
`core.board().to_move().other()`.

### Mutual exclusion and composition

The shell has three visual toggles:

| Toggle | Cell effect | Panel effect |
|--------|-------------|--------------|
| `p` | cycles policy heatmap | none |
| `t` | toggles TSS numbered line | none |
| `v` | none | toggles value strip |

`p` and `t` clear each other because they both paint cells. `v` clears
neither because it only changes the side panel. This matches the legacy
CLI behaviour and the spec §9.

### The heatmap idiom, copied not imported

The colour ramp is the same discrete ANSI-256 ramp used by
`patterns::render_heatmap` for the patterndemo binary. The `cli` crate
could import `patterns`, but the design deliberately keeps `cli`
`patterns`-free: the CLI's AI is the `mcts` tactics evaluator, and
introducing a `patterns` dependency just for a six-entry helper would
violate the crate's clean layering. So `overlay.rs` re-implements the
tiny `heatmap_bg` helper. The lineage is documented in the source, not
hidden.

## Low-level design

### Files changed vs chapter 05

The diff from chapter 05 (`8ad0061`) to this chapter (`ee4a66a`) is
confined to four files under `src/tui/`:

```text
$ git diff 8ad0061..ee4a66a --stat -- gomoku/crates/cli
 gomoku/crates/cli/src/tui/mod.rs     |   1 +
 gomoku/crates/cli/src/tui/overlay.rs | 409 +++++++++++++++++++++++++++++++++++
 gomoku/crates/cli/src/tui/render.rs  | 241 ++++++++++++++++++++-
 gomoku/crates/cli/src/tui/study.rs   | 130 ++++++++++-
 4 files changed, 767 insertions(+), 14 deletions(-)
```

Everything outside `src/tui/` is unchanged. In particular,
`src/session.rs`, `src/actor.rs`, `src/command.rs`, and `src/history.rs`
need no changes — `SearchInfo` and `search_info()` already exist from
chapter 03.

### `src/tui/overlay.rs`

New file. Pure functions, no I/O, fully unit-testable.

Key types:

```rust
pub enum PolicyOverlay { Off, Visits, Prior }

pub struct OverlayCell {
    pub intensity: f32, // 0.0–1.0
    pub text: String,
    pub is_hot: bool,
}
```

* `visits_cell`: total visits as denominator, share as intensity,
  `is_hot` when share ≥ 10%, hot label is the raw visit count.
* `prior_cell`: max prior as denominator (so the brightest cell is the
  evaluator's favourite), intensity = prior / max_prior, hot label is
  the percentage rounded to the nearest integer.
* `heatmap_bg`: the discrete ANSI-256 ramp. Hot values glow red
  (`196`), orange (`208`), yellow (`220`); lower values fade to light
  grey (`250`); a muted band at intensity `0.3` uses cyan (`39`),
  matching the patterndemo convention.
* `value_strip`: returns the panel lines. `side_at_search` determines
  the colour label; `current_config` is compared to `info.config` and a
  `(!searched at sims=N)` staleness note is appended when they differ
  or when the seat is now human.

### `src/tui/render.rs`

* `render_study_with_overlays`: the new entry point used by both the
  alternate-screen and line-mode paths. It composes the board lines
  with the side panel, padding the shorter side with empty strings.
* `render_study_board_with_overlays`: decides which board rendering path
  to take. TSS and policy overlays are mutually exclusive; if both are
  active, TSS wins (it is a deterministic proof overlay). If policy is
  on but the cursor has no `SearchInfo`, the bare board is shown.
* `study_board_lines_with_policy`: builds the board line by line,
  calling `overlay::overlay_cell` for empty cells and preserving stone
  cells (including the last-move brackets).
* `render_side_panel_with_value_strip`: extends the chapter-05 panel
  with the value strip when `show_value_strip` is true.

### `src/tui/study.rs`

* Extend `ShellState` with `policy_overlay: PolicyOverlay` and
  `show_value_strip: bool`.
* Extend `parse_study_input` to recognise `p` and `v` before falling
  through to `command::parse`.
* In the `Parsed::Policy` branch: cycle the overlay and clear
  `tss_proof`. If the cursor has no `SearchInfo`, set the message to
  "no search here yet".
* In the `Parsed::Value` branch: toggle `show_value_strip`.
* The existing `Parsed::Tss` branch already clears
  `shell.policy_overlay = PolicyOverlay::Off`.

### `src/tui/mod.rs`

One line: add `pub mod overlay;`.

## Solution (opt-in)

The complete reference files for this chapter are quoted verbatim in
[06-seeing-the-mind/01-solution.md](06-seeing-the-mind/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare your work.

## TDD checklist

Run from `gomoku/` with `cargo test -p cli`.

### `src/tui/overlay.rs`

Exact `#[test]` function names from commit `ee4a66a`:

1. `cycle_order_is_off_visits_prior_off`
2. `overlay_off_returns_none`
3. `zero_visits_gives_zero_intensity_and_not_hot`
4. `single_hot_move_has_full_intensity`
5. `visit_share_computes_intensity_correctly`
6. `small_visit_share_is_dim`
7. `prior_normalises_by_max_prior`
8. `below_threshold_prior_is_dim`
9. `value_strip_formats_sign_and_color`
10. `value_strip_negative_value_keeps_sign`
11. `value_strip_appends_staleness_when_config_differs`
12. `value_strip_is_stale_when_seat_is_human`
13. `value_strip_no_search_info_returns_empty`
14. `styled_hot_cell_has_ansi_background`
15. `plain_hot_cell_has_no_ansi_escapes`

### `src/tui/render.rs`

Exact `#[test]` function name added at `ee4a66a`:

1. `policy_overlay_snapshot_with_fake_search_info`

All earlier render tests continue to pass and are part of the chapter-06
gate.

### Red-green-refactor rhythm

1. **Red:** Create `src/tui/overlay.rs`, extend `src/tui/render.rs` and
   `src/tui/study.rs`, and add `pub mod overlay` to
   `src/tui/mod.rs`. Run `cargo test -p cli`. Expect failures.
2. **Green:** Implement the contracts above until `cargo test -p cli`
   passes.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any issues.

## Capstone: now you can see what the AI thinks

Run the scripted session in the reference worktree. The captures below
use the non-tty fallback (stdout is a pipe), so the frames are plain
text without ANSI escapes. The alternate-screen path paints the same
cells with `\x1b[48;5;...m` backgrounds, using the `^[` convention from
the convolutions chapters.

```text
cd gomoku
printf '7 7\nstep\n7 8\nstep\n7 9\nstep\n7 10\nstep\np\np\nv\nq\n' | \
  cargo run -q -p cli --bin gomoku -- play --ai tactics --no-swap2 --sims 800
```

The human builds an open four on row 7. After `step`, the AI blocks at
`g8`. The cursor is now at ply 8, Black to move, and the stored
`SearchInfo` is White's search from ply 7.

### Visits overlay

After the first `p`:

```text
policy overlay: visits
>      a  b  c  d  e  f  g  h  i  j  k  l  m  n  o       -- Study table --
   +---------------------------------------------+     Black: Human
 0 | o  o  o  .  .  .  .  .  .  .  .  .  .  .  . | 0   White: AI(800)
 1 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 1
 2 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 2   Ply: 8
 3 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 3   Path: h8 a1 i8 b1 j8 c1 k8 g8
 4 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 4
 5 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 5   Last events:
 6 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 6     Moved b1 by Ai
 7 | .  .  .  +  .  . [o] x  x  x  x  7% .  .  . | 7     Moved j8 by Human
 8 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 8     Moved c1 by Ai
 9 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 9     Moved k8 by Human
10 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 10    Moved g8 by Ai
11 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 11
12 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 12  h help · q quit
13 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 13  p policy overlay (ch.06)
14 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 14  v value strip (ch.06)
   +---------------------------------------------+
     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
ply 8 · Black to move · status ongoing
```

The search spent its budget on the two cells that stop Black's open
four: `g8` — already played by White — and `l8`. The occupied cell
shows the stone glyph; the empty block cell is highlighted. In a colour
terminal the hot cells glow red/orange on an ANSI-256 background; the
non-tty capture above uses the plain-cell convention.

### Prior overlay

After the second `p`:

```text
policy overlay: prior
>      a  b  c  d  e  f  g  h  i  j  k  l  m  n  o       -- Study table --
   +---------------------------------------------+     Black: Human
 0 | o  o  o  .  .  .  .  .  .  .  .  .  .  .  . | 0   White: AI(800)
 1 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 1
 2 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 2   Ply: 8
 3 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 3   Path: h8 a1 i8 b1 j8 c1 k8 g8
 4 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 4
 5 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 5   Value strip:
 6 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 6   value -0.99 for White
 7 | .  .  .  +  .  . [o] x  x  x  x  7% .  .  . | 7   total visits: 800
 8 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 8   top moves:
 9 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 9     1. g8   6.0%
10 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 10    2. l8   6.0%
11 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 11    3. d1   0.5%
12 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 12
13 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 13  Last events:
14 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 14    Moved b1 by Ai
   +---------------------------------------------+       Moved j8 by Human
     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o         Moved c1 by Ai
    Moved k8 by Human
    Moved g8 by Ai

  h help · q quit
  p policy overlay (ch.06)
  v value strip (ch.06)
ply 8 · Black to move · status ongoing
```

The prior overlay normalises by the maximum prior, so the brightest
cell is always the evaluator's favourite. Here the tactics evaluator
already spotted the same two block cells. On a positional position the
prior would be flat and the visits overlay would reveal where the search
found something the evaluator missed.

### Value strip

After `v`:

```text
value strip on
>
```

The value strip was already visible in the prior frame because it
composes with the overlay. It reads:

```text
value -0.99 for White
total visits: 800
top moves:
  1. g8   6.0%
  2. l8   6.0%
  3. d1   0.5%
```

The value is from White's perspective (the searching side), and it is
strongly negative because Black's open four is almost decisive. The
top-3 moves are the two blocks plus a corner move the search sampled.
There is no staleness note because the White seat is still an AI with
the same `sims=800` config that ran the search.

### TSS remains available

The `t` key still runs a TSS proof and paints the forced-win line as
numbered cells. Activating `t` clears the policy overlay, and activating
`p` clears the TSS overlay. No feature regression from the legacy UI.

## Done when

* `cargo test -p cli` passes from `gomoku/` (including the 15 overlay
  unit tests and the render snapshot test).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* The scripted non-tty demo above produces the quoted frames.
* Commit message for the code slice:
  `feat(cli): policy overlay + value strip`.

Next: [Chapter 07 — Acceptance](07-acceptance.md)

## References

* [`00-playable-cli-primer.md`](00-playable-cli-primer.md) — the study
  table, core/shell, the four knobs, and the overlay summary in §9.
* [`03-the-ai-takes-a-seat.md`](03-the-ai-takes-a-seat.md) —
  `SearchInfo`, value sign, and the negate-once backup convention.
* [`05-the-real-table.md`](05-the-real-table.md) — the study-table
  screen and side panel that this chapter extends.
* [`README.md`](README.md) — chapter map and tutorial conventions.
* [`docs/plans/2026-10-17-playable-cli.md`](../../plans/2026-10-17-playable-cli.md)
  — Task 6 (this chapter).
* [`docs/specs/2026-10-17-playable-cli-design.md`](../../specs/2026-10-17-playable-cli-design.md)
  — §9 (overlays and the value strip).
* [`docs/tutorials/convolutions-tutorial/06-the-fork.md`](../convolutions-tutorial/06-the-fork.md)
  — chapter template and solution-file precedent.
* `gomoku/crates/patterns/src/render.rs` — the ANSI-256 heatmap idiom
  that `overlay.rs` deliberately copies.
