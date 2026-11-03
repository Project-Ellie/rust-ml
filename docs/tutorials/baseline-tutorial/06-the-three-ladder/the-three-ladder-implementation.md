# Sub-chapter 06 — The three ladder: implementation plan

This document turns the walkthrough into a construction order. It names
the files, the changed items, and the behavioral contracts. It does not
contain method bodies. Read `the-three-ladder.md` first. Open
`the-three-ladder-solution.md` only if you are stuck for more than
twenty minutes, or after the sub-chapter is complete.

## What changes

Only two files change in this sub-chapter:

1. `gomoku/crates/baseline/src/readout.rs` — add the new tier constants,
   extend `policy_logits`, and extend `value`.
2. `gomoku/crates/baseline/src/lib.rs` — update the module-level and
   `BaselineEvaluator` doc comments to describe v4.

The `adapter`, `gravity`, and `mcts` wiring stay exactly as they are.

## Step 0 — Read the prerequisites

Re-read:

* `docs/tutorials/baseline-tutorial/00-baseline-primer.md` §5 and §6.
* The previous sub-chapters' code in your worktree.
* The test fixtures you will use: `engine::reference::board_from_ascii`.

## Step 1 — Add the policy tier constants in `readout.rs`

Add public constants after the existing `T1_WIN` and `T2_FORCED_BLOCK`:

* `T3_DOUBLE_THREAT: f32 = 16.0`
* `T4A_DOUBLE_THREE: f32 = 8.0`
* `T4B_THREE: f32 = 4.0`
* `T4C_DEFENSIVE: f32 = 2.0`

Also add a private `T5_MAX: f32 = 1.0` for the domination-invariant
test. Because it is only used in a test, mark it with the idiom that
keeps dead-code lints away.

Each constant needs a doc comment that says what tier it represents.

## Step 2 — Extend `policy_logits`

The function already receives `gravity`, `own`, `opp`, and `legal`.
After the existing checks for `own.wins` and `opp.wins`, add four more
binary-membership checks in this order:

1. Own double threats: if `own.double_threats[r][c] > 0.0`, add
   `T3_DOUBLE_THREAT`.
2. Own double-threes: if `own.double_threes[r][c] > 0.0`, add
   `T4A_DOUBLE_THREE`.
3. Own three-makers: if **any** direction in `own.threes_per_dir` is
   positive at `(r, c)`, add `T4B_THREE` exactly once. Use `iter().any(...)`.
4. Opponent defensive threes: if `opp.double_threes[r][c] > 0.0` or any
   direction in `opp.threes_per_dir` is positive at `(r, c)`, add
   `T4C_DEFENSIVE` exactly once.

The order matters for readability, but because membership is binary and
the tiers are independent, the arithmetic is the same in any order.

## Step 3 — Extend `value` with double-three rungs

The function already has fast paths for own win and two or more
opponent wins, then accumulates opponent single win, own double threat,
and opponent double threat. Add two more accumulation steps:

* If `own.double_threes` has any positive cell, add `V_OWN_DOUBLE_THREE`
  (`+0.20`).
* If `opp.double_threes` has any positive cell, add `V_OPP_DOUBLE_THREE`
  (`−0.20`).

Keep the final `clamp(-V_CLAMP, V_CLAMP)`. Quiet boards still return
`0.0`.

## Step 4 — Update `lib.rs` documentation

Update the module-level doc comment and the `BaselineEvaluator` doc
comment to say v4 includes the full tier table and the double-three
value rungs. The struct and the `Evaluator` impl do not change.

## Step 5 — Add the chapter tests in `readout.rs`

All new tests live in the existing `#[cfg(test)] mod tests` block in
`readout.rs`. They reuse:

* `empty_maps()` — the helper that builds zeroed `ThreatMaps`.
* `board_from_ascii` from `engine::reference`.
* `BaselineEvaluator::new(Device::flex())`.
* `gravity::gravity_map(&req.planes)`.

Add the tests in this order. Each test should fail before the
implementation is written and pass after.

### 5.1 `t3_double_threat_outranks_gravity_stack`

Load the CLI chapter-09 fork board with `board_from_ascii`. Build an
`EvalRequest`, compute gravity, run the evaluator, and assert that the
fork cell `(7,7)` receives exactly `T3_DOUBLE_THREAT + gravity`. Then
compute the maximum logit among all other empty cells and assert that
the fork cell exceeds it by more than `1.0`.

### 5.2 `t4_binary_membership_no_double_count`

Load the double-three fork board from the test body. Run the evaluator.
Assert that cell `(7,8)` receives exactly
`T4A_DOUBLE_THREE + T4B_THREE + gravity`. This proves the three-maker
tier is added once, not twice.

### 5.3 `t4c_defensive_tier_sees_opponent_threes`

Load the board where White threatens an open three along row 7 through
`(7,5)` and `(7,7)`. Run the evaluator as Black. Assert that the block
at `(7,6)` receives exactly `T4C_DEFENSIVE + gravity`.

### 5.4 `fork_board_argmax_is_the_fork`

Load the same fork board as in 5.1. Compute the policy argmax over empty
cells and assert it equals `(7,7)`.

### 5.5 `value_double_three_rungs`

Load the quiet board where Black owns a double-three fork and White has
only filler stones. Run the evaluator and assert `res.value == +0.20`.

### 5.6 `full_table_domination_invariant`

A constants-only test. Assert the two inequalities from the primer. Use
the idiom that silences the lint for assertions on constants.

## Step 6 — Run the gates

From `gomoku/`:

```text
cargo test -p baseline
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `readout.rs` | `T3_DOUBLE_THREAT` | public constant, value `16.0`, documented |
| `readout.rs` | `T4A_DOUBLE_THREE` | public constant, value `8.0`, documented |
| `readout.rs` | `T4B_THREE` | public constant, value `4.0`, documented |
| `readout.rs` | `T4C_DEFENSIVE` | public constant, value `2.0`, documented |
| `readout.rs` | `policy_logits` | adds T3, T4a, T4b, T4c by binary membership |
| `readout.rs` | `value` | adds `±0.20` for own/opp double-three |
| `readout.rs` | six chapter tests | each fails red then passes green |
| `lib.rs` | doc comments | describe v4 full table and double-three rungs |

## Done when

* The six new tests pass.
* All earlier baseline tests still pass.
* Clippy and format gates pass.
* The worktree is committed with:
  `feat(baseline): the three ladder — baseline sub-chapter 06`

## References

* [`the-three-ladder.md`](./the-three-ladder.md) — the tutorial.
* [`00-baseline-primer.md`](../00-baseline-primer.md) — tier derivation
  and value ladder.
* [`gomoku/crates/baseline/src/readout.rs`](../../../../gomoku/crates/baseline/src/readout.rs)
— the file you will change.
* [`gomoku/crates/baseline/src/lib.rs`](../../../../gomoku/crates/baseline/src/lib.rs)
— the evaluator wiring.
