# Sub-chapter 06d — The TSS overlay + mutual exclusion

## Abstract

This sub-chapter returns the legacy study-table TSS overlay to the
playable CLI. The user presses `t` in normal mode, or types `t` or `tss`
in line mode, to run the engine's threat-space-search prover at the
cursor position. A verified forced-win line is painted on the board as
numbered cells: attacker moves plain (` 1 `), defender moves bracketed
(`[2]`), 1-based. The verdict appears on the message line. Unverifiable
proofs become an internal-error message, never a picture. The overlay is
refused during the Swap2 ceremony and on finished games. Mutual
exclusion with the policy overlay is enforced in the shell: entering a
policy overlay clears a TSS proof, and a successful `t` clears the
policy overlay. The value strip continues to compose with both.

## Glossary

| Term | Definition |
|------|------------|
| **TSS overlay** | The cell overlay that paints a verified forced-win line from the engine's threat-space-search prover. |
| **`Proof`** | The engine's record of a forced win: a winner color and the line of moves that proves it. |
| **`TssOverlay`** | The pure per-cell mapping built from a `Proof` in `overlay.rs`. |
| **`TssSide`** | Whether a move in the proof line is an attacker move or a defender block. |
| **`tss_verdict`** | The message text `certified forced win for X in N ply`. |
| **`InputOutcome::ToggleTss`** | The input-layer outcome for both the `t` key and the typed `t`/`tss` line. |
| **`toggle_tss`** | The shell helper that runs the prover, verifies the result, and updates `ShellState`. |
| **Mutual exclusion** | The shell invariant that only one cell overlay (policy or TSS) is active at a time. |
| **Verify-before-paint gate** | The `engine::verify_line` call that rejects a proof before it can be rendered. |

## Context

Read [the playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md)
first. Section 9 states that panel elements compose freely while cell
overlays exclude each other. Sub-chapters 06a–06c built the value strip
(panel) and the policy heatmap (cell overlay). This sub-chapter adds the
second cell overlay, the TSS proof overlay, and wires the exclusion rule
between the two.

The starting point is sub-chapter 06c. The shell loop owns `ShellState`,
`InputOutcome` carries overlay toggles, and `render.rs` paints frames
from a `StudyView`.

## Intention

The learner enables this user-facing behavior:

1. Press `t` in normal mode, or type `t` or `tss` in line mode, to run
   the engine's threat-space-search prover at the current board
   position.
2. If a forced win is proven and the resulting line passes
   `engine::verify_line`, paint the proof on the board and show the
   verdict on the message line.
3. If no forced win is found within the search budget, show a
   budget-message instead of a picture.
4. Refuse the prover during the Swap2 ceremony and on finished games.
5. Clear the TSS overlay when a policy overlay is activated, and clear
   the policy overlay when a TSS proof is activated.
6. Let the value strip remain visible alongside the TSS overlay.

## Specification as tests

The tests below are ordered along the execution path of a `t` keypress
from input translation through to the painted frame and the mutual-
exclusion wiring.

### Input translation

* `t_in_normal_mode_toggles_tss_overlay` — the `t` key returns
  `InputOutcome::ToggleTss`.
* `t_and_tss_in_line_mode_toggle_tss_overlay` — the typed words `t`,
  `T`, `tss`, and `TSS` return `InputOutcome::ToggleTss`.

### Overlay formatting

* `tss_overlay_numbers_attacker_and_defender_cells` — a `TssOverlay`
  maps proof-line moves to 1-based numbers and the correct side.
* `tss_verdict_names_winner_and_length` — `tss_verdict` formats the
  message `certified forced win for White in 2 ply`.
* `tss_plain_cells_follow_the_old_convention` — attacker cells render
  plain and defender cells bracketed.
* `tss_styled_attacker_uses_winner_color` — styled attacker output
  contains an ANSI escape and the number.
* `tss_styled_defender_uses_winner_other_color` — styled defender output
  uses the opposite color from the winner.

### Rendering

* `tss_overlay_paints_numbered_cells` — a `StudyView` with a proof shows
  ` 1 ` at the first attacker move and `[2]` at the defender block.
* `tss_overlay_preserves_stones_and_last_move` — existing stones and the
  last-move marker are not over-painted by TSS numbers.
* `tss_overlay_cursor_takes_precedence_over_numbers` — the cursor marker
  wins on a cell that is also in the proof line.
* `tss_and_policy_overlay_mutual_exclusion_prefers_tss` — when both
  overlays are active in the view, TSS is painted.
* `tss_styled_and_unstyled_frames_differ_only_in_escapes` — styled and
  plain frames are identical after stripping ANSI escapes.

### Loop integration

* `t_paints_the_forced_win_line_and_verdict` — pressing `t` in a
  scripted session paints the proof cells and shows the verdict.
* `t_after_p_clears_the_policy_overlay` — turning TSS on after policy
  removes the policy overlay.
* `p_after_t_clears_the_tss_overlay` — turning policy on after TSS
  removes the TSS overlay.
* `t_toggling_twice_removes_the_overlay` — pressing `t` twice hides the
  overlay.
* `t_with_no_forced_win_shows_the_budget_message` — pressing `t` on an
  empty board shows the budget message.
* `t_during_a_ceremony_is_refused` — pressing `t` during Swap2 shows the
  ceremony refusal message.

## Walkthrough

### 1. The user presses `t` or types `tss`

In normal mode `handle_key` in `input.rs` checks the global keys first.
`KeyCode::Char('t')` returns `InputOutcome::ToggleTss` immediately,
between the `p` and the unmapped-key fallthrough.

In line mode `line_outcome` trims the line. If the trimmed text is `t`
or `tss`, case-insensitively, it returns `InputOutcome::ToggleTss`. The
word sits next to `v`/`value` and `p`/`policy` as a shell-level toggle,
not a `Command` variant.

### 2. The loop receives `ToggleTss`

`study_loop` in `study.rs` calls `toggle_tss(core, &mut shell)`. That
function either hides an existing proof or runs a new one.

### 3. `toggle_tss` decides whether to run the prover

If `shell.tss_proof` is already `Some`, the function clears it and
returns `"TSS overlay hidden"`.

Otherwise it checks two refusals:

* If `core.snapshot().opening` is `Some`, the Swap2 ceremony is in
  progress and the function returns `"TSS is not available during the
  Swap2 ceremony"`.
* If `board.status()` is not `Status::Ongoing`, the game is finished and
  the function returns `"TSS demo is only available during ongoing
  games"`.

### 4. The prover runs with a capped budget

For an ongoing game position the function builds:

```rust
SearchBudget {
    max_nodes: 100_000,
    max_depth: 9,
}
```

It calls `engine::prove_forced_win(board, board.to_move(), budget)`. A
`None` result produces `"no forced win proven within budget"`.

### 5. The verify-before-paint gate

When the prover returns a `Proof`, `toggle_tss` calls
`engine::verify_line(board, &proof)`. If verification fails, it returns
`"internal error: unverifiable proof"`. This gate guarantees that no
painted proof line can be invalid.

### 6. The proof is stored and the verdict is shown

After verification, `toggle_tss` stores the proof in
`shell.tss_proof`, clears `shell.policy_overlay` to enforce mutual
exclusion, and returns `tss_verdict(&proof)` — for example,
`"certified forced win for Black in 3 ply"`.

### 7. The renderer paints numbered cells

`build_frame` passes `shell.tss_proof.as_ref()` to `StudyView::tss_proof`.
`render_study` forwards it into `study_board_lines`, which builds a
`TssOverlay::new(proof)` if a proof is present.

`study_or_policy_cell` checks the TSS overlay first. If it is present,
it delegates to `tss_cell`, which never over-paints stones, the cursor,
or the last move. Empty cells in the proof line show the move number:
attacker moves plain, defender moves bracketed.

### 8. Mutual exclusion is enforced twice

The shell enforces the rule directly: the `CyclePolicy` arm clears
`shell.tss_proof` when the new policy overlay is not `Off`, and
`toggle_tss` sets `shell.policy_overlay = PolicyOverlay::Off` when a
proof is stored.

The renderer adds a defensive default: if both overlays are somehow
active in the same `StudyView`, TSS takes precedence. This is stated in
the code, not relied on, because the shell should never hand both in at
once.

### 9. The value strip composes freely

The value strip is a panel element, not a cell overlay. Its flag in
`ShellState` is independent of `policy_overlay` and `tss_proof`. The side
panel renders the strip whenever `shell.show_value_strip` is true,
regardless of which board overlay is active.

### 10. Help texts stay consistent

The normal-mode key table in `study.rs` gains `t              toggle TSS
forced-win overlay`. The line interpreter's `HELP` in `repl.rs` gains
`t, tss           toggle the TSS forced-win overlay`. The byte-exact
integration transcript in `tests/line_mode.rs` grows by the matching
line.

## Low-level design

### [`gomoku/crates/cli/src/tui/overlay.rs`](../../../../../gomoku/crates/cli/src/tui/overlay.rs)

New pure items:

* `TssSide` — `Attacker` or `Defender`.
* `TssOverlay<'a>` — builds a `[Option<TssOverlayCell>; 225]` lookup
  table from a `Proof`, with `get(mv: Move) -> Option<(usize, TssSide)>`
  and `proof() -> &'a Proof`.
* `tss_verdict(proof: &Proof) -> String` — formats the verdict line.
* `render_tss_cell_plain(n: usize, side: TssSide) -> String` — plain
  text, attacker ` {n} `, defender `[{n}]`.
* `render_tss_cell_styled(n: usize, side: TssSide, winner: Color) ->
  String` — raw SGR escapes, cyan-bold for the winner's side, yellow for
  the other.

### [`gomoku/crates/cli/src/tui/input.rs`](../../../../../gomoku/crates/cli/src/tui/input.rs)

* Add `InputOutcome::ToggleTss` between `CyclePolicy` and `None`.
* In `handle_normal_key`, match `KeyCode::Char('t')`.
* In `line_outcome`, recognize `t`/`tss` before falling back to
  `command::parse`.
* Add the two `t` tests.

### [`gomoku/crates/cli/src/tui/study.rs`](../../../../../gomoku/crates/cli/src/tui/study.rs)

* Import `SearchBudget` and `Status` from `engine`, and `tss_verdict`
  from `overlay`.
* Extend `ShellState` with `tss_proof: Option<engine::Proof>`.
* Add the `ToggleTss` arm in `study_loop`; it sets the message line from
  `toggle_tss`.
* Update `build_frame` to pass `shell.tss_proof.as_ref()` to
  `StudyView::tss_proof`.
* Add `toggle_tss` with refusal checks, budget, prover call,
  `verify_line` gate, verdict formatting, and policy clearing.
* Add `t` to `NORMAL_MODE_KEYS`.
* Add the six loop integration tests.

### [`gomoku/crates/cli/src/tui/render.rs`](../../../../../gomoku/crates/cli/src/tui/render.rs)

* Import `TssOverlay`, `render_tss_cell_plain`, and
  `render_tss_cell_styled` from `overlay`.
* Add `tss_proof: Option<&'a Proof>` to `StudyView` and a chainable
  setter `tss_proof`.
* Forward the proof through `render_study`, `study_board_lines`, and
  `study_or_policy_cell`.
* Give TSS precedence in `study_or_policy_cell`; implement `tss_cell`
  with the stone/cursor/last-move guard.
* Add the five rendering tests.

### [`gomoku/crates/cli/src/repl.rs`](../../../../../gomoku/crates/cli/src/repl.rs)

* Add `t, tss           toggle the TSS forced-win overlay` to `HELP`.

### [`gomoku/crates/cli/tests/line_mode.rs`](../../../../../gomoku/crates/cli/tests/line_mode.rs)

* Add the same `t, tss` line to the byte-exact transcript constant.

## TDD checklist

Work one test group at a time, red to green.

From `gomoku/crates/cli/src/tui/input.rs`:

1. `t_in_normal_mode_toggles_tss_overlay`
2. `t_and_tss_in_line_mode_toggle_tss_overlay`

From `gomoku/crates/cli/src/tui/overlay.rs`:

3. `tss_overlay_numbers_attacker_and_defender_cells`
4. `tss_verdict_names_winner_and_length`
5. `tss_plain_cells_follow_the_old_convention`
6. `tss_styled_attacker_uses_winner_color`
7. `tss_styled_defender_uses_winner_other_color`

From `gomoku/crates/cli/src/tui/render.rs`:

8. `tss_overlay_paints_numbered_cells`
9. `tss_overlay_preserves_stones_and_last_move`
10. `tss_overlay_cursor_takes_precedence_over_numbers`
11. `tss_and_policy_overlay_mutual_exclusion_prefers_tss`
12. `tss_styled_and_unstyled_frames_differ_only_in_escapes`

From `gomoku/crates/cli/src/tui/study.rs`:

13. `t_paints_the_forced_win_line_and_verdict`
14. `t_after_p_clears_the_policy_overlay`
15. `p_after_t_clears_the_tss_overlay`
16. `t_toggling_twice_removes_the_overlay`
17. `t_with_no_forced_win_shows_the_budget_message`
18. `t_during_a_ceremony_is_refused`

## Verification commands

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Done when

* The `t` key and the typed words `t`/`tss` run the TSS prover.
* A verified proof is painted as numbered cells, attacker plain and
  defender bracketed.
* The verdict `certified forced win for X in N ply` appears on the
  message line.
* Unverifiable proofs, empty results, the Swap2 ceremony, and finished
  games produce the correct messages instead of a picture.
* Mutual exclusion works in both directions between TSS and policy
  overlays.
* The value strip continues to compose with the TSS overlay.
* All eighteen tests above pass.
* The three project gates pass.
* A scripted session shows the TSS overlay. From `gomoku/`, run:

  ```text
  printf 'h8\nh9\ni8\ni9\nj8\nj9\nt\nq\n' | cargo run -q -p cli --bin gomoku -- play --no-swap2
  ```

  The last frame looks like this:

```text
     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o       -- Study table --
   +---------------------------------------------+     Black: Human
 1 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 1   White: Human
 2 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 2   
 3 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 3   Ply: 6
 4 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 4   To move: Black
 5 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 5   Path: h8 h9 i8 i9 j8 j9
 6 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 6   
 7 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 7   Last events:
 8 | .  .  .  +  .  .  1  x  x  x  .  +  .  .  . | 8     Moved h9 by Human
 9 | .  .  .  .  .  .  .  o  o [o] .  .  .  .  . | 9     Moved i8 by Human
10 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 10    Moved i9 by Human
11 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 11    Moved j8 by Human
12 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 12    Moved j9 by Human
13 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 13  
14 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 14  ? help · q quit
15 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 15  
   +---------------------------------------------+     
     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o       
certified forced win for Black in 1 ply
```

## References

* [Playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md) — §9 overlays and mutual exclusion.
* [Sub-chapter 06a — The value strip](../a-value-strip/value-strip.md) — the first overlay and `ShellState`.
* [Sub-chapter 06c — The prior view](../c-prior-overlay/prior-overlay.md) — the policy heatmap that this overlay excludes.
* [Milestone 1 TSS slice](../../../../gomoku/crates/engine/src/tss.rs) — `prove_forced_win` and `verify_line`.
* [`gomoku/crates/cli/src/tui/input.rs`](../../../../../gomoku/crates/cli/src/tui/input.rs) — input translation.
* [`gomoku/crates/cli/src/tui/overlay.rs`](../../../../../gomoku/crates/cli/src/tui/overlay.rs) — pure overlay formatting.
* [`gomoku/crates/cli/src/tui/render.rs`](../../../../../gomoku/crates/cli/src/tui/render.rs) — frame rendering.
* [`gomoku/crates/cli/src/tui/study.rs`](../../../../../gomoku/crates/cli/src/tui/study.rs) — the shell loop.
