# Sub-chapter 07 — The quiet whisper: implementation plan

This document turns the walkthrough in `the-quiet-whisper.md` into a
construction order. It names the files, structs, functions, constants,
and tests. It states behavioral contracts in prose. It does not contain
method bodies.

Read `the-quiet-whisper.md` first. Work one step at a time. Write tests
before code. Open `the-quiet-whisper-solution.md` only if you are stuck
for more than twenty minutes, or after the sub-chapter is complete.

---

## Step 0 — Understand the boundary

This sub-chapter changes only the `baseline` crate. No `engine`, `mcts`,
`patterns`, or `cli` code changes. The `BaselineEvaluator` already has
access to reconstructed `Board`, threat maps, and gravity map from
sub-chapter 06. You add one function, one flag, one method, and the A/B
test.

---

## Step 1 — Add `QUIET_CAP` to `readout.rs`

### 1.1 Location

In `gomoku/crates/baseline/src/readout.rs`, add one private `const`
item near the existing value constants.

### 1.2 Constant

* `QUIET_CAP: f32` — the absolute ceiling for the quiet-position value
  whisper.

### 1.3 Contract

The value is `0.05`. This is one order of magnitude below the smallest
threat-ladder rung (`±0.20` for a double-three fork).

---

## Step 2 — Add `readout::quiet_value`

### 2.1 Signature

```rust
#[must_use]
pub fn quiet_value(gravity: &[[f32; 15]; 15], planes: &engine::Planes) -> f32
```

### 2.2 Contract

Behavior, in execution order:

1. Initialize `weighted_me`, `weighted_you`, and `gravity_sum` to
   `0.0f32`.
2. For each row `r` and column `c` in the 15×15 `gravity` map:
   * Compute the 17×17 plane index `dst = (r + 1) * engine::EXT + (c + 1)`.
   * Add `gravity[r][c]` to `gravity_sum`.
   * If `planes.me[dst] == 1`, add `gravity[r][c]` to `weighted_me`.
   * Else if `planes.you[dst] == 1`, add `gravity[r][c]` to `weighted_you`.
3. Compute `denom = gravity_sum.max(1.0)`.
4. Compute `raw = (weighted_me - weighted_you) / denom`.
5. Return `raw.clamp(-QUIET_CAP, QUIET_CAP)`.

On an empty board every gravity cell is zero, so the function returns
`0.0` exactly.

### 2.3 Doc comment

Document the formula, the `[experiment]` status, and the fact that the
term is intentionally naive.

---

## Step 3 — Write the `quiet_value` unit tests

### 3.1 Location

In `gomoku/crates/baseline/src/readout.rs`, inside the existing
`#[cfg(test)] mod tests` block.

### 3.2 Test order

1. `quiet_value_empty_board_is_zero`
2. `quiet_value_is_always_capped`
3. `quiet_value_clustered_own_stones_is_positive`

### 3.3 Gate

Run `cargo test -p baseline quiet_value`. Expect the first tests to fail
until the function is implemented.

---

## Step 4 — Add the `quiet` flag to `BaselineEvaluator`

### 4.1 Location

In `gomoku/crates/baseline/src/lib.rs`.

### 4.2 Field

Add one field to `BaselineEvaluator`:

* `quiet: bool`

### 4.3 Contract

The field is `true` by default. It controls whether the whisper runs.

---

## Step 5 — Add `without_quiet_value`

### 5.1 Signature

```rust
#[must_use]
pub fn without_quiet_value(mut self) -> Self
```

### 5.2 Contract

Set `self.quiet = false` and return `self`. The method is chainable and
intended only for A/B measurement.

### 5.3 Doc comment

Document that it disables the `[experiment]` quiet-position value
whisper.

---

## Step 6 — Wire the whisper into `BaselineEvaluator::evaluate`

### 6.1 Location

In `gomoku/crates/baseline/src/lib.rs`.

### 6.2 Contract

After computing:

```rust
let value = readout::value(own, opp);
```

add a conditional replacement:

```rust
let value = if value == 0.0 && self.quiet {
    readout::quiet_value(&gravity, &req.planes)
} else {
    value
};
```

Then return `EvalResult { policy, value }`.

The check uses exact equality because the threat ladder returns exact
`0.0` on a quiet board and exact non-zero sentinels otherwise.

---

## Step 7 — Add the integration tests in `lib.rs`

### 7.1 Test order

1. `quiet_whisper_t1_win_still_returns_nine_five`
2. `quiet_whisper_without_quiet_is_zero_on_quiet_cluster`

### 7.2 Gate

Run `cargo test -p baseline quiet_whisper`. Confirm the T1 win still
returns `0.95` and the disabled whisper returns `0.0` on the cluster
board.

---

## Step 8 — Add the A/B measurement test

### 8.1 Location

In `gomoku/crates/baseline/tests/acceptance.rs`.

### 8.2 Test

Add:

```rust
#[ignore]
#[test]
fn quiet_whisper_ab_measured()
```

### 8.3 Contract

* Constants: `GAMES = 20`, `SIMS = 200`, `BASE_SEED = 20_260_901`.
* Define a local helper `play_game_ab(baseline_color, quiet, seed)` that
  constructs one `BaselineEvaluator` with the whisper either on or off,
  one `TacticsEvaluator`, and plays a full game.
* Loop over `quiet in [true, false]`.
* For each game, alternate `baseline_color` by game index.
* Seed each game with `BASE_SEED + g as u64`.
* Tally wins, losses, draws, total plies, and total wall-clock time.
* Print a table with columns: arm, wins, losses, draws, average plies,
  average seconds per game.

The test does not need to assert a win-rate bar because the opponent is
expected to be too weak to discriminate. The measurement is the result.

---

## Step 9 — Project gates

From `gomoku/`:

```text
cargo test -p baseline
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Run the A/B in release:

```text
cargo test -p baseline --release --test acceptance quiet_whisper_ab_measured -- --ignored --nocapture
```

---

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `src/readout.rs` | `QUIET_CAP` | `0.05`, documented |
| `src/readout.rs` | `quiet_value` | gravity-weighted differential, clamped |
| `src/readout.rs` | `quiet_value` tests | three tests, all exact equality |
| `src/lib.rs` | `BaselineEvaluator::quiet` | `bool`, default `true` |
| `src/lib.rs` | `without_quiet_value` | chainable, sets `quiet = false` |
| `src/lib.rs` | `evaluate` | whisper only when `value == 0.0 && self.quiet` |
| `src/lib.rs` | integration tests | T1 win unchanged; disabled whisper returns `0.0` |
| `tests/acceptance.rs` | A/B test | ignored, 20 games, 200 sims, seed `20_260_901` |
| gates | `cargo test -p baseline` | all green |
| gates | `cargo clippy` | no warnings |
| gates | `cargo fmt --check` | no changes |
