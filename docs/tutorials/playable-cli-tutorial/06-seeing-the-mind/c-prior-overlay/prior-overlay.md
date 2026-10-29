# Sub-chapter 06c — The prior view

## Abstract

This sub-chapter adds the third state to the policy overlay cycle:
Off → Visits → Prior → Off. Prior mode paints empty intersections with
the evaluator's raw belief from `SearchInfo.prior`, normalized to the
maximum prior so the brightest cell is always the evaluator's favourite.
The hot threshold is the same 10% boundary used by visits, but the label
shows the raw prior percentage, not the normalized share.

The sub-chapter's lesson is the contrast between the two beliefs. The
prior is the evaluator's opinion before the search simulates a single
ply. The visits are the search's conclusion after simulation. On a
tactical position they can disagree, and that disagreement is how you
watch the search correct the evaluator. The flat-prior phenomenon is
real: when the evaluator has no strong opinion, normalize-by-max makes
many cells hot with "0%" labels. That is the evaluator saying "no idea".
On a tactical position the prior spikes and only the forcing moves glow.

## Glossary

| Term | Definition |
|------|------------|
| **Policy overlay** | The cell heatmap that paints empty intersections with search data. |
| **`PolicyOverlay`** | The enum with the three modes: `Off`, `Visits`, and `Prior`. |
| **`SearchInfo::prior`** | The evaluator's raw policy vector: a list of legal moves with prior probabilities. |
| **Maximum prior** | The largest probability in `SearchInfo::prior`; used as the denominator for prior intensity. |
| **Normalized intensity** | `prior / max_prior`, in the range 0.0–1.0. |
| **Hot threshold** | The inclusive 0.10 normalized-intensity boundary that decides whether a cell prints a label. |
| **Raw percentage label** | The text of a hot prior cell, e.g. "7%", computed from the raw prior without normalization. |
| **`prior_cell`** | The pure helper that turns one `(SearchInfo, Move)` pair into an `OverlayCell` for Prior mode. |

## Context

Sub-chapter 06b added the policy overlay cycle and the visits flavor.
The cycle lives in `PolicyOverlay::cycle`, the cell math lives in
`overlay.rs`, and the renderer routes visits-mode cells through
`policy_empty_cell` in `render.rs`. The shell loop in `study.rs` handles
`InputOutcome::CyclePolicy` and writes the overlay status to the message
line.

This sub-chapter extends the same three files. It does not add a new key
or a new `InputOutcome`; the existing `p` key cycles through the new
state. It also updates the help texts and the byte-exact line-mode
transcript.

## Intention

The learner enables this user-facing behavior:

1. A second `p` press in normal mode, or a second typed `p`/`policy` in
   line mode, moves the policy overlay from Visits to Prior.
2. Prior mode paints empty intersections with the evaluator's belief.
   The brightest cell is the evaluator's favourite move.
3. A hot cell prints its raw prior percentage (rounded to the nearest
   whole percent). A dim cell prints nothing.
4. Stones, the last-move marker, and the cursor marker are never
   over-painted.
5. If `SearchInfo.prior` is empty or the maximum prior is zero, no
   overlay cells are produced.
6. The status line reads `policy overlay: prior`, and the help texts
   read `off → visits → prior`.

## Specification as tests

The tests below are ordered along the execution path of two `p` presses
from the cycle through the painted cell.

### Policy cycle

* `cycle_order_is_off_visits_prior_off` — the renamed cycle test asserts
  `Off → Visits → Prior → Off`.

### Prior cell math

* `prior_normalises_by_max_prior` — the best move has intensity `1.0`,
  the half-max move has intensity `0.5`, and both cells print their raw
  percentages ("50%", "25%").
* `below_threshold_prior_is_dim` — a move at 8% of the max is not hot
  and prints no label.
* `a_normalized_prior_of_exactly_ten_percent_is_hot` — the 10% boundary
  is inclusive, so a move at exactly 10% of the max is hot.
* `empty_prior_list_gives_no_overlay_cells` — no prior data means
  `overlay_cell` returns `None`.
* `zero_max_prior_gives_no_overlay_cells` — a list whose maximum prior
  is `0.0` produces no cells, avoiding division by zero.
* `prior_for_missing_move_is_zero_intensity` — a move not listed in the
  prior vector gets intensity `0.0`, is not hot, and prints nothing.

### Rendering

* `prior_overlay_shows_percentages_and_respects_precedence` — the prior
  overlay prints "50%" and "25%" on empty cells, keeps the last-move
  brackets on a stone, and lets the cursor marker take precedence on a
  hot empty cell.

### Loop integration

* `two_p_presses_reach_prior_and_show_prior_percentages` — two `p`
  presses in a scripted session reach Prior mode, report it on the
  message line, and show the expected prior percentages in the board
  area.

## Walkthrough

### 1. The user presses `p` a second time

In normal mode `handle_key` in `input.rs` already returns
`InputOutcome::CyclePolicy` for `p`. In line mode `line_outcome` already
recognizes `p`/`policy`. This sub-chapter changes no input translation.

### 2. The shell cycles the overlay

`study_loop` in `study.rs` owns `ShellState.policy_overlay`. The
`CyclePolicy` arm calls `shell.policy_overlay.cycle()`. After two presses
the value moves from `Off` to `Visits` to `Prior`. The message line reads
`policy overlay: prior`.

If there is no stored search info, the message line reads
`no search here yet` instead, and the board is left bare.

### 3. The frame carries the mode

`build_frame` passes `shell.policy_overlay` into
`StudyView::policy_overlay`. The renderer cannot know which mode is
active; the view carries the mode the same way it carries the cursor and
the value-strip flag.

### 4. The board cell path routes any active mode

`study_or_policy_cell` in `render.rs` checks the active mode. If the cell
is a stone, the cursor, or the last move, it keeps its normal marker.
Otherwise, if the mode is not `Off`, it calls `policy_empty_cell` with
the active mode. The visits-only path from 06b becomes a generic policy
path.

### 5. `policy_empty_cell` dispatches to the mode

`policy_empty_cell` now takes the `PolicyOverlay` mode and calls
`overlay_cell(info, mv, mode)`. For Prior mode this routes to
`prior_cell`.

### 6. `prior_cell` computes the cell

`prior_cell` in `overlay.rs` finds the maximum prior in
`SearchInfo.prior`. If the maximum is `0.0`, it returns `None`. It then
looks up the prior for the current move (default `0.0`), computes
`intensity = prior / max_prior`, and decides hotness with `intensity >=
0.10`. A hot cell formats the raw prior as `{:.0}%`.

The brightest cell is always the move with the maximum prior because its
intensity is `1.0`. The label, however, is the raw percentage, so a cell
at 50% intensity may still read "25%" if the maximum prior was 0.5.

### 7. The status line and help texts name the flavor

The frame's bottom message line reads `policy overlay: prior`. The
normal-mode key table in `study.rs` and the line interpreter's `HELP` in
`repl.rs` both read `off → visits → prior`. The byte-exact
`TWO_MOVE_TRANSCRIPT` in `tests/line_mode.rs` grows by that one word.

## Low-level design

### [`gomoku/crates/cli/src/tui/overlay.rs`](../../../../../gomoku/crates/cli/src/tui/overlay.rs)

* Add the `Prior` variant to `PolicyOverlay` with a doc comment that
  explains the max-normalization rule.
* Extend `cycle` to `Off → Visits → Prior → Off`.
* Extend `label` to return `"prior"`.
* Extend `overlay_cell` to call `prior_cell` in `Prior` mode.
* Add the private `prior_cell` helper.
* Add the six prior-cell unit tests and rename the cycle test.

### [`gomoku/crates/cli/src/tui/render.rs`](../../../../../gomoku/crates/cli/src/tui/render.rs)

* Generalize `study_or_policy_cell`: route any active mode through the
  policy cell path, not only `Visits`.
* Generalize `policy_empty_cell`: take the active `PolicyOverlay` mode
  and pass it to `overlay_cell`.
* Add `prior_overlay_shows_percentages_and_respects_precedence`.

### [`gomoku/crates/cli/src/tui/study.rs`](../../../../../gomoku/crates/cli/src/tui/study.rs)

* Update `NORMAL_MODE_KEYS` to read `off → visits → prior`.
* Add `two_p_presses_reach_prior_and_show_prior_percentages`.

### [`gomoku/crates/cli/src/repl.rs`](../../../../../gomoku/crates/cli/src/repl.rs)

* Update the `p, policy` help line to read
  `cycle policy overlay (off → visits → prior)`.

### [`gomoku/crates/cli/tests/line_mode.rs`](../../../../../gomoku/crates/cli/tests/line_mode.rs)

* Update `TWO_MOVE_TRANSCRIPT` with the same help line.

## TDD checklist

Work one test group at a time, red to green.

From `gomoku/crates/cli/src/tui/overlay.rs`:

1. `cycle_order_is_off_visits_prior_off`
2. `prior_normalises_by_max_prior`
3. `below_threshold_prior_is_dim`
4. `a_normalized_prior_of_exactly_ten_percent_is_hot`
5. `empty_prior_list_gives_no_overlay_cells`
6. `zero_max_prior_gives_no_overlay_cells`
7. `prior_for_missing_move_is_zero_intensity`

From `gomoku/crates/cli/src/tui/render.rs`:

8. `prior_overlay_shows_percentages_and_respects_precedence`

From `gomoku/crates/cli/src/tui/study.rs`:

9. `two_p_presses_reach_prior_and_show_prior_percentages`

## Verification commands

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Done when

* Two `p` presses reach Prior mode and the message line reads
  `policy overlay: prior`.
* Prior mode paints empty intersections with raw prior percentages.
* The brightest cell is always the evaluator's favourite move.
* Hot cells print rounded raw percentages; dim cells print nothing.
* Empty prior lists and zero maximum priors produce no overlay cells.
* Stones, cursor, and last-move markers keep precedence.
* All nine tests above pass.
* The three project gates pass.
* A scripted session shows the prior overlay on a tactical position.
  From `gomoku/`, run:

  ```text
  printf 'h8\nstep\ni8\nstep\nj8\nstep\nk8\nstep\np\np\nq\n' | cargo run -q -p cli --bin gomoku -- play --ai tactics --no-swap2 --sims 200
  ```

  A typical prior frame looks like this:

```text
 8 | .  .  .  +  .  .  7% x  x  x  x  7% .  .  . | 8     Moved b1 by Ai
```

## References

* [Sub-chapter 06b — The visits view](../b-visits-overlay/visits-overlay.md) — the policy overlay cycle this sub-chapter extends.
* [Sub-chapter 06a — The value strip](../a-value-strip/value-strip.md) — the panel overlay that composes with the cell overlays.
* [Sub-chapter 05d — The alternate-screen shell](../05-the-real-table/d-shell/shell.md) — the shell loop and `InputOutcome`.
* [Playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md) — §9 overlays.
* [`gomoku/crates/cli/src/tui/overlay.rs`](../../../../../gomoku/crates/cli/src/tui/overlay.rs) — pure overlay formatting.
* [`gomoku/crates/cli/src/tui/render.rs`](../../../../../gomoku/crates/cli/src/tui/render.rs) — frame rendering.
* [`gomoku/crates/cli/src/tui/study.rs`](../../../../../gomoku/crates/cli/src/tui/study.rs) — the shell loop.
