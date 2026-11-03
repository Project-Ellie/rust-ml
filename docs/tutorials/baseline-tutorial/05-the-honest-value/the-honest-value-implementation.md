# Sub-chapter 05 — The honest value: implementation plan

This document turns the walkthrough in `the-honest-value.md` into a
construction order. It names the files, structs, functions, constants,
and tests. It states behavioral contracts in prose. It does not contain
method bodies.

Read `the-honest-value.md` first. Work one step at a time. Write tests
before code. Open `the-honest-value-solution.md` only if you are stuck
for more than twenty minutes, or after the sub-chapter is complete.

---

## Step 0 — Understand the boundary

This sub-chapter changes only the `baseline` crate. No `engine`, `mcts`,
`patterns`, or `cli` code changes. The `BaselineEvaluator` already has
access to reconstructed `Board`, threat maps, and gravity map from
sub-chapter 04. You add one function and wire it into `evaluate`.

---

## Step 1 — Add value constants to `readout.rs`

### 1.1 Location

In `gomoku/crates/baseline/src/readout.rs`, add six private `const`
items near the existing tier constants.

### 1.2 Constants

* `V_OWN_WIN: f32` — value of an own immediate win.
* `V_TWO_OPP_WINS: f32` — value when the opponent has two or more
  immediate wins.
* `V_OPP_WIN: f32` — contribution when the opponent has exactly one
  immediate win.
* `V_OWN_DOUBLE_THREAT: f32` — contribution when the side to move has
  any double threat.
* `V_OPP_DOUBLE_THREAT: f32` — contribution when the opponent has any
  double threat.
* `V_CLAMP: f32` — absolute value ceiling, strictly less than `1.0`.

### 1.3 Contract

Every constant is a documented `f32`. Their exact magnitudes are
starting values; the tests assert properties, not the numbers.

---

## Step 2 — Add map helpers to `readout.rs`

### 2.1 `has_positive`

Signature:

```rust
fn has_positive(map: &[[f32; 15]; 15]) -> bool
```

Behavior: return `true` when at least one cell in the map is strictly
greater than `0.0`.

### 2.2 `count_positive`

Signature:

```rust
fn count_positive(map: &[[f32; 15]; 15]) -> usize
```

Behavior: return the number of cells strictly greater than `0.0`.

---

## Step 3 — Add `readout::value`

### 3.1 Signature

```rust
#[must_use]
pub fn value(own: &ThreatMaps, opp: &ThreatMaps) -> f32
```

### 3.2 Contract

Behavior, in execution order:

1. If `has_positive(&own.wins)` is true, return `V_OWN_WIN`.
2. Count opponent wins with `count_positive(&opp.wins)`.
3. If the count is `>= 2`, return `V_TWO_OPP_WINS`.
4. Start with `v = 0.0f32`.
5. If the opponent win count is exactly `1`, add `V_OPP_WIN` to `v`.
6. If `has_positive(&own.double_threats)` is true, add
   `V_OWN_DOUBLE_THREAT` to `v`.
7. If `has_positive(&opp.double_threats)` is true, add
   `V_OPP_DOUBLE_THREAT` to `v`.
8. Return `v.clamp(-V_CLAMP, V_CLAMP)`.

When every map is empty, the function falls through to `0.0` exactly.

### 3.3 Doc comment

Document the ladder, the clamp, and the fact that double-three rungs
arrive in sub-chapter 06.

---

## Step 4 — Write the value tests in `readout.rs`

### 4.1 Reuse the existing helper

The `empty_maps()` function already creates a `ThreatMaps` with all
zeros. Use it in every test.

### 4.2 Test order

Add these tests in `#[cfg(test)] mod tests`:

1. `value_quiet_board_is_zero`
2. `value_own_win_sentinel`
3. `value_two_opp_wins_is_near_loss`
4. `value_single_opp_win_is_negative`
5. `value_own_double_threat_is_positive`
6. `value_saturation_clamps_near_one`
7. `value_never_touches_one`
8. `value_monotonic_in_threats`
9. `value_is_side_to_move_relative`

### 4.3 Gate

Run `cargo test -p baseline`. Expect the first tests to fail until
`value` is implemented.

---

## Step 5 — Wire the value into `BaselineEvaluator::evaluate`

### 5.1 Location

In `gomoku/crates/baseline/src/lib.rs`.

### 5.2 Contract

After computing `policy` in `BaselineEvaluator::evaluate`, set
`EvalResult::value` to `readout::value(own, opp)`. Keep the existing
policy computation unchanged.

### 5.3 Update the struct doc comment

Change the `BaselineEvaluator` doc comment to call the implementation
v3 and mention the saturating value heuristic.

---

## Step 6 — Confirm existing tests still pass

### 6.1 Tests to watch

The existing integration tests in `lib.rs` include `v0_value_is_zero`,
`v1_value_still_zero`, and the T1/T2 policy tests. After wiring the
value, `v0_value_is_zero` and `v1_value_still_zero` must still pass
because an empty board is still quiet.

### 6.2 Gate

Run `cargo test -p baseline` until all tests pass.

---

## Step 7 — Project gates

From `gomoku/`:

```text
cargo test -p baseline
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

---

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `src/readout.rs` | value constants | six documented `f32` constants |
| `src/readout.rs` | `has_positive` | true iff any cell > 0.0 |
| `src/readout.rs` | `count_positive` | count of cells > 0.0 |
| `src/readout.rs` | `value` | saturating side-to-move heuristic |
| `src/readout.rs` | value tests | nine tests, all assert properties |
| `src/lib.rs` | `BaselineEvaluator` doc | calls implementation v3 |
| `src/lib.rs` | `evaluate` | returns `readout::value(own, opp)` |
| gates | `cargo test -p baseline` | all green |
| gates | `cargo clippy` | no warnings |
| gates | `cargo fmt --check` | no changes |
