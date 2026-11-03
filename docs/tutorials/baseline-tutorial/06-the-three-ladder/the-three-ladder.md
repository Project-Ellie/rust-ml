# Sub-chapter 06 — The three ladder

## Abstract

This sub-chapter gives the baseline player the middle rungs of the
policy ladder: own double threats, own double-three forks, own
three-makers, and a defensive tier for the opponent's threes. It also
adds the matching value rungs for double-three forks. The seat now
builds traps two moves early and reacts when you try to build one.

## Glossary

| Term | Definition |
|------|------------|
| **Double threat** | A move that leaves the side with two or more immediate win cells. The opponent cannot stop both. `ThreatMaps::double_threats`. |
| **Double-three fork** | A move that creates open threes in two or more directions. It is not yet a win, but it often becomes one. `ThreatMaps::double_threes`. |
| **Three-maker** | A move that creates an open three along one direction. `ThreatMaps::threes_per_dir`. |
| **Binary membership** | A cell is in a tier or it is not. Per-direction stacking inside one tier is not allowed. |
| **Defensive tier (T4c)** | A logit boost for cells that block an opponent three-maker or double-three. Weight +2. |
| **Domination invariant** | Each tier weight exceeds the maximum possible sum of every tier below it. This makes the highest occupied tier win the argmax and keeps softmax mass safe. |
| **Fork board** | The CLI chapter-09 acceptance board: Black plays `(7,7)` to create a forcing double threat. |

## Context

Sub-chapters 01–04 put wins and forced blocks at the top of the policy
readout. Sub-chapter 05 added the value heuristic. Between "I win now"
and "the board is quiet" there is a lot of Gomoku: forks, double-threes,
open threes, and the need to see the opponent's threes before they turn
into fours. This chapter fills that middle ground.

## Intention

The learner enables this behavior:

1. The seat plays its own double threats with near-certainty, just below
   wins and forced blocks.
2. The seat builds double-three forks and gives them more policy mass
   than plain three-makers.
3. A double-three cell gets the double-three tier **and** the three-maker
   tier, but the three-maker tier is added only once even when the cell
   makes threes in two directions.
4. The seat sees the opponent's threes early and gives blocking cells a
   small defensive mass.
5. The seat does **not** waste policy mass on opponent double threats;
   those belong to the value readout.
6. The value readout feels double-three forks: +0.20 for its own,
   −0.20 for the opponent's.
7. The domination invariant still holds after the new tiers are added.

## Specification as tests

The tests below are ordered along the execution path of
`BaselineEvaluator::evaluate` → `readout::policy_logits` and
`readout::value`. They all live in
[`gomoku/crates/baseline/src/readout.rs`](../../../../gomoku/crates/baseline/src/readout.rs).

### Policy ladder

* `t3_double_threat_outranks_gravity_stack` — the fork cell `(7,7)` on
the CLI chapter-09 fork board receives `T3_DOUBLE_THREAT + gravity`. It
strictly outranks every other legal cell by more than `1.0`, so gravity
stacking cannot catch it.
* `t4_binary_membership_no_double_count` — the double-three fork cell
`(7,8)` receives `T4A_DOUBLE_THREE + T4B_THREE + gravity`. The
three-maker tier is added once, not once per direction.
* `t4c_defensive_tier_sees_opponent_threes` — when White has open-three
threats at `(7,5)` and `(7,7)`, the block at `(7,6)` receives
`T4C_DEFENSIVE + gravity`.
* `fork_board_argmax_is_the_fork` — on the CLI chapter-09 fork board, the
policy argmax over empty cells is exactly `(7,7)`.

### Value ladder

* `value_double_three_rungs` — when the side to move owns a double-three
fork and the opponent has only quiet filler stones, `evaluate` returns
`+0.20`.

### Derivation

* `full_table_domination_invariant` — the constants prove the
invariant:
  * `T2_FORCED_BLOCK > T3_DOUBLE_THREAT + T4A_DOUBLE_THREE + T4B_THREE + T4C_DEFENSIVE + T5_MAX`
  * `T1_WIN > T2_FORCED_BLOCK + T3_DOUBLE_THREAT + T4A_DOUBLE_THREE + T4B_THREE + T4C_DEFENSIVE + T5_MAX`

## Walkthrough

The evaluator's execution path is the same as in the previous chapters.
Only the body of `readout::policy_logits` and `readout::value` changes.

### 1. The request arrives

`BaselineEvaluator::evaluate` receives an `EvalRequest` with relative
planes and the legal move list. It reconstructs the absolute-color board
with `adapter::reconstruct_board`, runs `patterns::net::analyze_both`,
and picks the own and opponent maps by `board.to_move()`.

### 2. Gravity is computed

The `GravityKernel` produces a 15×15 gravity map from the color-blind
occupancy plane. The values are exact dyadic fractions in `[0, 1]`.

### 3. Policy logits are built for every legal cell

`readout::policy_logits` starts with a `[f32; 225]` of
`f32::NEG_INFINITY`. For each legal `Move` it:

1. Reads the gravity value at `(row, col)`.
2. Adds `T1_WIN` if the cell is an own immediate win.
3. Adds `T2_FORCED_BLOCK` if the cell is an opponent immediate win.
4. Adds `T3_DOUBLE_THREAT` if the cell is an own double threat.
5. Adds `T4A_DOUBLE_THREE` if the cell is an own double-three fork.
6. Adds `T4B_THREE` if the cell is an own three-maker in **any**
   direction. The check uses `iter().any(...)`, so a cell that makes
   threes in two directions still adds this tier only once.
7. Adds `T4C_DEFENSIVE` if the cell is an opponent double-three fork or
   an opponent three-maker in any direction.

The membership checks all use `> 0.0`. That is the binary-membership
idiom: a cell is in the tier or it is not.

### 4. Binary membership is load-bearing

A double-three fork cell is both a double-three (T4a, +8) and a
three-maker (T4b, +4). It receives `+8 + 4`. It does **not** receive
`+4` twice, because `T4B_THREE` is added once per cell, not once per
direction. The domination invariant depends on this: if a single cell
could collect `+4` per direction, the maximum lower-tier sum would be
large enough to break the invariant.

### 5. The defensive tier has limits

`T4C_DEFENSIVE` gives mass to cells that block opponent threes. That is
sound: a three can become a four, and blocking it early is good. But the
opponent's **double threats** get no policy mass at all. A double threat
is already unblockable; spending prior mass on a block is waste. The
value readout owns that despair.

### 6. The domination invariant protects the argmax

With the new constants, the worst-case lower-tier sum below T3 is
`8 + 4 + 2 + 1 = 15`, which is less than `T3_DOUBLE_THREAT = 16`. Below
T2 the worst case is `16 + 15 = 31`, less than `32`. Below T1 the worst
case is `32 + 31 = 63`, less than `64`. Therefore a cell in the highest
occupied tier always holds the strict logit maximum. The search's
`masked_softmax` turns that gap into mass.

### 7. Value gains the double-three rungs

`readout::value` already had rungs for wins, opponent wins, and double
threats. This chapter adds two more: `+0.20` when the side to move has a
double-three fork, `−0.20` when the opponent has one. The value is still
clamped to `±0.99`; a quiet board still returns exactly `0.0`.

### 8. The result goes back to the search

`evaluate` returns `EvalResult { policy, value }`. The policy is logits,
not probabilities. `mcts::eval::masked_softmax` normalizes them over the
legal moves during expansion.

## TDD checklist

Work one test at a time, red to green.

1. `t3_double_threat_outranks_gravity_stack`
2. `t4_binary_membership_no_double_count`
3. `t4c_defensive_tier_sees_opponent_threes`
4. `fork_board_argmax_is_the_fork`
5. `value_double_three_rungs`
6. `full_table_domination_invariant`

Then run the existing tests from earlier chapters to make sure nothing
regressed.

## Verification commands

From `gomoku/`:

```text
cargo test -p baseline
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Capstone

From `gomoku/`:

```text
gomoku play --ai baseline --sims 4000
```

Play a few moves near the center. The seat now builds double-threes: it
will create two open threes with one stone, and a beginner gets forked.
Try to defend against its threes — it will stack them into a sequence of
threats that the search verifies before it commits.

## Done when

* The six chapter tests pass.
* All earlier baseline tests still pass.
* `cargo clippy --all-targets -- -D warnings` passes.
* `cargo fmt --all -- --check` makes no changes.
* Every new tier and every new value rung is exercised by at least one
test.
* The worktree is committed with:
  `feat(baseline): the three ladder — baseline sub-chapter 06`

## References

* [`00-baseline-primer.md`](../00-baseline-primer.md) — the tier
derivation, the trust ladder, and the two deliberate absences.
* [`README.md`](../README.md) — the baseline-tutorial chapter map.
* [`gomoku/crates/baseline/src/readout.rs`](../../../../gomoku/crates/baseline/src/readout.rs)
— the chapter's code.
* [`gomoku/crates/baseline/src/lib.rs`](../../../../gomoku/crates/baseline/src/lib.rs)
— the `BaselineEvaluator` wiring.
* [`gomoku/crates/engine/src/reference.rs`](../../../../gomoku/crates/engine/src/reference.rs)
— `board_from_ascii` for the fork-board tests.
