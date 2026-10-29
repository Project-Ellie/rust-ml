# Sub-chapter 06c — The prior view: implementation plan

This document turns the walkthrough in `prior-overlay.md` into a
construction order. It names the files, structs, enums, functions, and
tests. It states behavioral contracts in prose. It does not contain
bodies.

Read `prior-overlay.md` first. Work one step at a time. Write tests
before code. Open `prior-overlay-solution.md` only if you are stuck for
more than twenty minutes, or after the sub-chapter is complete.

---

## Step 0 — `overlay.rs`: the `Prior` variant and `prior_cell`

### 0.1 Extend `PolicyOverlay`

Add the `Prior` variant between `Visits` and the closing brace. Document
it as the max-normalized prior heatmap.

### 0.2 Extend `PolicyOverlay::cycle`

The cycle must be `Off → Visits → Prior → Off`.

### 0.3 Extend `PolicyOverlay::label`

Return `"prior"` for the `Prior` variant.

### 0.4 Extend `overlay_cell`

In the `match`, route `PolicyOverlay::Prior` to `prior_cell`.

### 0.5 Add `prior_cell`

Write a private function:

```rust
fn prior_cell(info: &SearchInfo, mv: Move) -> Option<OverlayCell>
```

Behavior:
* Compute `max_prior` as the maximum value in `info.prior`.
* If `max_prior == 0.0`, return `None`.
* Compute `prior` as the value paired with `mv`, or `0.0` if `mv` is not
  in the list.
* Compute `intensity = prior / max_prior`.
* Compute `is_hot = intensity >= 0.10`.
* If `is_hot`, set `text` to the raw prior formatted as a rounded
  percentage with no decimal places and a `%` sign. Otherwise set
  `text` to an empty string.
* Return an `OverlayCell` with `intensity`, `text`, and `is_hot`.

### 0.6 Tests — group A

Rename the existing cycle test to `cycle_order_is_off_visits_prior_off`
and add the `Prior → Off` assertion.

Add these tests:

* `prior_normalises_by_max_prior`
* `below_threshold_prior_is_dim`
* `a_normalized_prior_of_exactly_ten_percent_is_hot`
* `empty_prior_list_gives_no_overlay_cells`
* `zero_max_prior_gives_no_overlay_cells`
* `prior_for_missing_move_is_zero_intensity`

### 0.7 Gate

Run `cargo test -p cli --lib overlay::tests`.

---

## Step 1 — `render.rs`: route any active policy mode

### 1.1 Generalize `study_or_policy_cell`

Change the condition from `policy_overlay == PolicyOverlay::Visits` to
`policy_overlay != PolicyOverlay::Off`. Keep the same branches: stones,
cursor, and last move render as normal; an active mode routes through
the policy cell path when search info is present.

### 1.2 Generalize `policy_empty_cell`

Change the signature to accept the active `PolicyOverlay` mode:

```rust
fn policy_empty_cell(
    mv: Move,
    info: &crate::actor::SearchInfo,
    styled: bool,
    mode: PolicyOverlay,
) -> String
```

Use `mode` when calling `overlay_cell(info, mv, mode)`.

### 1.3 Tests — group B

Add `prior_overlay_shows_percentages_and_respects_precedence`.

### 1.4 Gate

Run `cargo test -p cli --lib render::tests`.

---

## Step 2 — `study.rs`: cycle help and loop test

### 2.1 Update help text

In `NORMAL_MODE_KEYS`, change the `p` line to:

```text
  p              cycle policy overlay (off → visits → prior)
```

### 2.2 Tests — group C

Add `two_p_presses_reach_prior_and_show_prior_percentages`.

### 2.3 Gate

Run `cargo test -p cli --lib study::tests`.

---

## Step 3 — `repl.rs` and `tests/line_mode.rs`: help texts

### 3.1 `src/repl.rs`

Update the `p, policy` help line to:

```text
  p, policy        cycle policy overlay (off → visits → prior)
```

### 3.2 `tests/line_mode.rs`

Update `TWO_MOVE_TRANSCRIPT` with the same help line.

### 3.3 Gate

Run `cargo test -p cli`.

---

## Step 4 — Project gates

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
| `src/tui/overlay.rs` | `PolicyOverlay::Prior` | variant added and documented |
| `src/tui/overlay.rs` | `PolicyOverlay::cycle` | `Off → Visits → Prior → Off` |
| `src/tui/overlay.rs` | `PolicyOverlay::label` | returns `"prior"` for `Prior` |
| `src/tui/overlay.rs` | `overlay_cell` | routes `Prior` to `prior_cell` |
| `src/tui/overlay.rs` | `prior_cell` | normalizes by max, guards zero max, returns raw percentage label |
| `src/tui/overlay.rs` | group A tests | all pass |
| `src/tui/render.rs` | `study_or_policy_cell` | routes any active mode, not only `Visits` |
| `src/tui/render.rs` | `policy_empty_cell` | takes mode and passes it through |
| `src/tui/render.rs` | group B test | passes |
| `src/tui/study.rs` | `NORMAL_MODE_KEYS` | reads `off → visits → prior` |
| `src/tui/study.rs` | group C test | passes |
| `src/repl.rs` | `HELP` | reads `off → visits → prior` |
| `tests/line_mode.rs` | `TWO_MOVE_TRANSCRIPT` | contains the updated help line |
