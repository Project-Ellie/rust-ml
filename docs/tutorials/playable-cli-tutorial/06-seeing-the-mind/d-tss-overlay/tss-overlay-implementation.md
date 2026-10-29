# Sub-chapter 06d — The TSS overlay + mutual exclusion: implementation plan

This document turns the walkthrough in `tss-overlay.md` into a
construction order. It names the files, structs, enums, functions, and
tests. It states behavioral contracts in prose. It does not contain
bodies.

Read `tss-overlay.md` first. Work one step at a time. Write tests before
code. Open `tss-overlay-solution.md` only if you are stuck for more than
twenty minutes, or after the sub-chapter is complete.

---

## Step 0 — `overlay.rs`: pure TSS formatting

### 0.1 Add the TSS types

In `src/tui/overlay.rs`, add the import of `engine::Proof` (and keep
`Color` and `Move`).

#### The `TssSide` enum

Write a public enum:

```rust
pub enum TssSide {
    Attacker,
    Defender,
}
```

#### The `TssOverlay` struct

Write a public struct with a lifetime:

```rust
pub struct TssOverlay<'a> {
    proof: &'a Proof,
    cells: [Option<TssOverlayCell>; 225],
}
```

Add a private helper struct:

```rust
struct TssOverlayCell {
    number: usize,
    side: TssSide,
}
```

Implement `impl<'a> TssOverlay<'a>` with:

* `pub fn new(proof: &'a Proof) -> Self` — build the 225-cell lookup
  table. For each move in `proof.line`, store `number = i + 1` and
  `side = Attacker` for even indices, `Defender` for odd indices.
* `pub fn proof(&self) -> &'a Proof` — return the underlying proof.
* `pub fn get(&self, mv: Move) -> Option<(usize, TssSide)>` — look up
  the cell data.

#### The verdict function

Write:

```rust
pub fn tss_verdict(proof: &Proof) -> String
```

Return `format!("certified forced win for {:?} in {} ply", proof.winner,
proof.line.len())`.

#### The cell render functions

Write:

```rust
pub fn render_tss_cell_plain(n: usize, side: TssSide) -> String
```

Behavior:
* `debug_assert!(n < 10)`.
* `Attacker` returns `format!(" {n} ")`.
* `Defender` returns `format!("[{n}]")`.

Write:

```rust
pub fn render_tss_cell_styled(n: usize, side: TssSide, winner: Color) -> String
```

Behavior:
* `debug_assert!(n < 10)`.
* The effective color is `winner` for `Attacker`, `winner.other()` for
  `Defender`.
* For `Color::Black`, wrap the digit in bold cyan (`\x1b[1;36m…\x1b[0m`).
* For `Color::White`, wrap the digit in yellow (`\x1b[33m…\x1b[0m`).
* `Attacker` returns `format!(" {styled} ")`.
* `Defender` returns `format!("[{styled}]")`.

#### Tests — group A

* `tss_overlay_numbers_attacker_and_defender_cells`
* `tss_verdict_names_winner_and_length`
* `tss_plain_cells_follow_the_old_convention`
* `tss_styled_attacker_uses_winner_color`
* `tss_styled_defender_uses_winner_other_color`

### 0.2 Gate

Run `cargo test -p cli --lib overlay::tests`.

---

## Step 1 — `input.rs`: the `t` outcome

### 1.1 Vocabulary

Add a variant to `InputOutcome`:

```rust
/// Toggle the TSS forced-win overlay on the board.
ToggleTss,
```

Place it between `CyclePolicy` and `None`.

### 1.2 Normal-mode key

In `handle_normal_key`, add `KeyCode::Char('t') => return
InputOutcome::ToggleTss` with the other global keys.

### 1.3 Line-mode mapping

In `line_outcome`, after the `v`/`value` and `p`/`policy` guards and
before `command::parse`, add a guard for `t`/`tss`:

```rust
if trimmed.eq_ignore_ascii_case("t") || trimmed.eq_ignore_ascii_case("tss") {
    return InputOutcome::ToggleTss;
}
```

### 1.4 Tests — group B

* `t_in_normal_mode_toggles_tss_overlay`
* `t_and_tss_in_line_mode_toggle_tss_overlay`

### 1.5 Gate

Run `cargo test -p cli --lib input::tests`.

---

## Step 2 — `render.rs`: painting the proof cells

### 2.1 Import the TSS helpers

In `src/tui/render.rs`, import `TssOverlay`,
`render_tss_cell_plain`, and `render_tss_cell_styled` from
`crate::tui::overlay`. Import `Proof` from `engine`.

### 2.2 Extend `StudyView`

Add a private field:

```rust
tss_proof: Option<&'a Proof>,
```

Initialize it to `None` in `StudyView::new`.

Add a public chainable setter:

```rust
pub fn tss_proof(mut self, proof: Option<&'a Proof>) -> Self
```

Mark it `#[must_use]`.

### 2.3 Forward the proof

In `render_study`, pass `view.tss_proof` into `study_board_lines`.

In `study_board_lines`:
* Add a `tss_proof: Option<&Proof>` parameter.
* Build `let tss_overlay = tss_proof.map(TssOverlay::new);`.
* Pass `tss_overlay.as_ref()` to `study_or_policy_cell` for every cell.

### 2.4 Give TSS precedence and implement `tss_cell`

Extend `study_or_policy_cell` with a `tss_overlay: Option<&TssOverlay>`
parameter. At the top of the function:

```rust
if let Some(overlay) = tss_overlay {
    return tss_cell(board, opening, mv, last, cursor, styled, overlay);
}
```

Implement `tss_cell`:

```rust
fn tss_cell(
    board: &Board,
    opening: Option<&OpeningSnapshot>,
    mv: Move,
    last: Option<Move>,
    cursor: Option<Move>,
    styled: bool,
    overlay: &TssOverlay,
) -> String
```

Behavior:
* If the cell is a stone, the cursor, or the last move, return
  `study_cell(...)` unchanged.
* If `overlay.get(mv)` returns `Some((n, side))`, return
  `render_tss_cell_styled(n, side, overlay.proof().winner)` when styled,
  else `render_tss_cell_plain(n, side)`.
* Otherwise return `study_cell(...)`.

Both `study_board_lines` and `study_or_policy_cell` now have many
arguments; allow `clippy::too_many_arguments` on both.

### 2.5 Tests — group C

* `tss_overlay_paints_numbered_cells`
* `tss_overlay_preserves_stones_and_last_move`
* `tss_overlay_cursor_takes_precedence_over_numbers`
* `tss_and_policy_overlay_mutual_exclusion_prefers_tss`
* `tss_styled_and_unstyled_frames_differ_only_in_escapes`

Use the existing VCF puzzle helpers from the solution to build a
verifiable proof.

### 2.6 Gate

Run `cargo test -p cli --lib render::tests`.

---

## Step 3 — `study.rs`: shell state, loop wiring, and `toggle_tss`

### 3.1 Clean up imports

Import `SearchBudget` and `Status` from `engine`. Import `tss_verdict`
from `crate::tui::overlay`.

### 3.2 Extend `ShellState`

Add a field:

```rust
/// The active TSS forced-win proof overlay.
tss_proof: Option<engine::Proof>,
```

### 3.3 Wire the loop

In `study_loop`:
* Pass `&shell` to `build_frame` (it now carries the proof).
* Add an arm for `InputOutcome::ToggleTss` that sets the message line to
  `toggle_tss(core, &mut shell)`.

Update the `CyclePolicy` arm: when the new policy overlay is not
`PolicyOverlay::Off`, set `shell.tss_proof = None` to enforce mutual
exclusion.

### 3.4 Update `build_frame`

Change the signature to take `shell: &ShellState` instead of individual
flags. Forward:

```rust
.tss_proof(shell.tss_proof.as_ref())
```

Keep forwarding `show_value_strip` and `policy_overlay` from `shell`.

### 3.5 Add `toggle_tss`

Write:

```rust
fn toggle_tss(core: &SessionCore, shell: &mut ShellState) -> String
```

Behavior:
* If `shell.tss_proof.is_some()`, clear it and return `"TSS overlay
  hidden"`.
* If `core.snapshot().opening.is_some()`, return `"TSS is not available
  during the Swap2 ceremony"`.
* Let `board = core.board()`. If `board.status() != Status::Ongoing`,
  return `"TSS demo is only available during ongoing games"`.
* Build the budget `max_nodes: 100_000, max_depth: 9`.
* Let `side = board.to_move()`.
* Match on `engine::prove_forced_win(&board, side, budget)`:
  * `None` → `"no forced win proven within budget"`.
  * `Some(proof)` → if `!engine::verify_line(&board, &proof)` return
    `"internal error: unverifiable proof"`; otherwise store the proof,
    clear `shell.policy_overlay`, and return `tss_verdict(&proof)`.

### 3.6 Update help text

Add `t              toggle TSS forced-win overlay` to
`NORMAL_MODE_KEYS`.

### 3.7 Tests — group D

* `t_paints_the_forced_win_line_and_verdict`
* `t_after_p_clears_the_policy_overlay`
* `p_after_t_clears_the_tss_overlay`
* `t_toggling_twice_removes_the_overlay`
* `t_with_no_forced_win_shows_the_budget_message`
* `t_during_a_ceremony_is_refused`

### 3.8 Gate

Run `cargo test -p cli --lib study::tests`.

---

## Step 4 — Help texts and byte-exact transcript

### 4.1 `src/repl.rs`

Add `t, tss           toggle the TSS forced-win overlay` to `HELP`
between `p, policy` and `new`.

### 4.2 `tests/line_mode.rs`

Add the same line to the byte-exact transcript constant.

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
| `src/tui/overlay.rs` | `TssSide` | enum with `Attacker` and `Defender` |
| `src/tui/overlay.rs` | `TssOverlay::new` | builds 225-cell lookup from proof |
| `src/tui/overlay.rs` | `TssOverlay::get` | returns `(number, side)` for proof cells |
| `src/tui/overlay.rs` | `tss_verdict` | formats the verdict line |
| `src/tui/overlay.rs` | `render_tss_cell_plain` | plain ` N ` / `[N]` |
| `src/tui/overlay.rs` | `render_tss_cell_styled` | raw SGR, cyan-bold/yellow convention |
| `src/tui/overlay.rs` | group A tests | all pass |
| `src/tui/input.rs` | `InputOutcome::ToggleTss` | variant added |
| `src/tui/input.rs` | `handle_normal_key` | `t` returns toggle outcome |
| `src/tui/input.rs` | `line_outcome` | recognizes `t`/`tss` |
| `src/tui/input.rs` | group B tests | all pass |
| `src/tui/render.rs` | imports TSS helpers | present |
| `src/tui/render.rs` | `StudyView.tss_proof` | field and setter |
| `src/tui/render.rs` | `study_board_lines` | builds `TssOverlay` and forwards it |
| `src/tui/render.rs` | `study_or_policy_cell` | TSS checked first |
| `src/tui/render.rs` | `tss_cell` | preserves stones/cursor/last-move |
| `src/tui/render.rs` | group C tests | all pass |
| `src/tui/study.rs` | imports | `SearchBudget`, `Status`, `tss_verdict` |
| `src/tui/study.rs` | `ShellState.tss_proof` | field added |
| `src/tui/study.rs` | `study_loop` | `ToggleTss` arm, `CyclePolicy` clears TSS |
| `src/tui/study.rs` | `build_frame` | forwards `tss_proof.as_ref()` |
| `src/tui/study.rs` | `toggle_tss` | refusals, budget, prover, verify, verdict, exclusion |
| `src/tui/study.rs` | `NORMAL_MODE_KEYS` | lists `t` |
| `src/tui/study.rs` | group D tests | all pass |
| `src/repl.rs` | `HELP` | lists `t, tss` |
| `tests/line_mode.rs` | transcript | contains the new help line |
