# Sub-chapter 05 — The honest value

## Abstract

This sub-chapter gives the baseline evaluator a **value readout**: a
scalar that tells the search how good the current position looks for
the side to move. The value is not a prophecy. It is a saturating
heuristic built from the same threat maps that already drive the
policy, and it is deliberately clamped away from ±1. Those extremes are
reserved for proven terminal outcomes that the search handles without
calling the evaluator. With the value in place, every branch of the tree carries a graded
guess, not just the branches that end in a win or loss.

## Glossary

| Term | Definition |
|------|------------|
| **Value readout** | The scalar `f32` in `EvalResult::value`, from the side to move's perspective. |
| **`readout::value`** | The public function that computes the value from two `ThreatMaps`. |
| **Saturating heuristic** | A heuristic whose output is clamped to a range strictly inside ±1, so it cannot impersonate a proven terminal. |
| **Side-to-move perspective** | Positive numbers favor the player whose turn it is; negative numbers favor the opponent. |
| **Backup sign flip** | During MCTS backup, a child's value is added to its parent with the opposite sign, because one ply's gain is the other ply's loss. |
| **Terminal reservation** | The convention that `+1.0` and `−1.0` belong to the search's terminal handling, not to heuristics. |
| **Quiet board** | A board where no threat map fires: no wins, no forced blocks, no double threats. |
| **Honesty clause** | The explicit admission that this heuristic cannot see threat sequences; it sees only one move deep. |
| **V_CLAMP** | The value ceiling `0.99`; the magnitude never reaches `1.0`. |

## Context

Sub-chapter 04 wired wins and forced blocks into the policy. The seat
now takes its own five and blocks yours, but it still reports a flat
`0.0` value at every leaf. The PUCT search can therefore only learn
from terminal outcomes it reaches by chance. A graded value turns every
leaf into information: the tree stops flailing and starts judging which
branches are worth exploring.

The value uses the same `ThreatMaps` the policy already consumes. No
new dependency, no new network, no randomness. The only new code is one
function and the constants that shape it.

## Intention

The learner enables this behavior:

1. The evaluator returns a value from the side-to-move perspective.
2. The value stays strictly inside `[-1, 1]`; ±1 remain reserved for
   proven terminals.
3. A quiet board returns exactly `0.0`.
4. Own wins, opponent single wins, and own/opponent double threats move
   the value in the expected direction.
5. Two opponent wins produce a near-loss value, because one block
   cannot cover two finishing cells.
6. The value is side-to-move relative: swapping the maps swaps the
   sign.

## Specification as tests

These tests live in `baseline/src/readout.rs` and exercise
`readout::value` directly. They assert properties, not approximations,
because every input is a dyadic fraction or a small integer.

* `value_quiet_board_is_zero` — both maps empty ⇒ `value` returns
  exactly `0.0`.
* `value_own_win_sentinel` — the side to move has an immediate win ⇒
  value equals `V_OWN_WIN` (`0.95`).
* `value_two_opp_wins_is_near_loss` — the opponent has two immediate
  wins ⇒ value equals `V_TWO_OPP_WINS` (`-0.99`).
* `value_single_opp_win_is_negative` — the opponent has exactly one
  immediate win ⇒ value equals `V_OPP_WIN` (`-0.75`).
* `value_own_double_threat_is_positive` — the side to move has a double
  threat ⇒ value equals `V_OWN_DOUBLE_THREAT` (`0.85`).
* `value_saturation_clamps_near_one` — combinations that would push
  beyond the ceiling or floor short-circuit or clamp to the pinned
  sentinel or `V_CLAMP`.
* `value_never_touches_one` — across a grid of threat combinations the
  absolute value stays strictly below `1.0` and at most `V_CLAMP`
  (`0.99`).
* `value_monotonic_in_threats` — adding own threats never decreases the
  value; adding opponent threats never increases it; stronger threats
  dominate weaker ones.
* `value_is_side_to_move_relative` — the same absolute position is
  positive from the winner's perspective and the negative mirror from
  the loser's perspective.

## Walkthrough

The value path runs inside `BaselineEvaluator::evaluate`, but the new
logic lives almost entirely in `readout::value`.

### 1. The evaluator receives the request

`BaselineEvaluator::evaluate` gets an `EvalRequest` with `planes` and
`legal`. This is unchanged from sub-chapter 04.

### 2. Reconstruct the board

`adapter::reconstruct_board` turns the side-to-move-relative planes
into an absolute `Board`. The parity rule still holds: equal stone
counts mean Black to move.

### 3. Analyze both sides

`patterns::net::analyze_both` returns `[ThreatMaps; 2]`: index 0 for
Black, index 1 for White. The result is oracular for wins and double
threats, oracle-proven for three-level tiers.

### 4. Pick own and opponent maps

`board.to_move()` decides which index is `own` and which is `opp`. This
is the side-to-move relativity: the value is always from the viewpoint
of the player about to move.

### 5. Compute the value

`readout::value(own, opp)` inspects the threat maps in priority order:

* If `own.wins` has any positive cell, return `V_OWN_WIN` (`+0.95`).
  A win now ends the game before anything else matters.
* Count positive cells in `opp.wins`. If the count is two or more,
  return `V_TWO_OPP_WINS` (`-0.99`). One block cannot stop two fives.
* Otherwise accumulate: `V_OPP_WIN` for exactly one opponent win,
  `V_OWN_DOUBLE_THREAT` for any own double threat,
  `V_OPP_DOUBLE_THREAT` for any opponent double threat.
* Clamp the sum to `[-V_CLAMP, V_CLAMP]` (`[-0.99, 0.99]`).

A quiet board skips every branch and lands on `0.0` exactly.

### 6. Why ±1 is reserved

`mcts::expand` checks `board.status()` before it ever calls the
evaluator. A won position returns `+1.0`; a lost position returns
`−1.0`; a draw returns `0.0`. Those values are exact, and backup
assumes they are truth. If the heuristic also returned `±1.0`, the
search could not tell a heuristic opinion from a proven outcome. The
value readout therefore saturates at `0.99`, leaving a clear gap.

### 7. Why quiet is exactly 0.0

Gravity guides the policy, but it never enters the value. A position
with stones but no threats is honestly neutral: neither side has a
forced advantage yet. Keeping gravity out of the value prevents the
heuristic from pretending that clustering stones is the same as
winning.

### 8. Return the result

`evaluate` packs the existing policy logits and the new value into
`EvalResult`. The search's backup pass flips the sign per ply, so the
value must already be from the side-to-move perspective.

## Low-level design

### [`gomoku/crates/baseline/src/readout.rs`](../../../../gomoku/crates/baseline/src/readout.rs)

Add six private value constants:

* `V_OWN_WIN: f32 = 0.95`
* `V_TWO_OPP_WINS: f32 = -0.99`
* `V_OPP_WIN: f32 = -0.75`
* `V_OWN_DOUBLE_THREAT: f32 = 0.85`
* `V_OPP_DOUBLE_THREAT: f32 = -0.85`
* `V_CLAMP: f32 = 0.99`

Add two private helpers:

* `fn has_positive(map: &[[f32; 15]; 15]) -> bool`
* `fn count_positive(map: &[[f32; 15]; 15]) -> usize`

Add the public function:

```rust
#[must_use]
pub fn value(own: &ThreatMaps, opp: &ThreatMaps) -> f32
```

Behavior:

* Own win ⇒ `V_OWN_WIN`.
* Opponent wins ≥ 2 ⇒ `V_TWO_OPP_WINS`.
* Else accumulate opponent single win, own double threat, opponent
  double threat, then clamp to `[-V_CLAMP, V_CLAMP]`.
* Quiet board ⇒ `0.0`.

Add the nine value tests in a `#[cfg(test)] mod tests` block. Reuse the
existing `empty_maps()` helper.

### [`gomoku/crates/baseline/src/lib.rs`](../../../../gomoku/crates/baseline/src/lib.rs)

In `BaselineEvaluator::evaluate`, after building `policy`, set
`EvalResult::value` to `readout::value(own, opp)`. Update the doc
comment on `BaselineEvaluator` to call the implementation v3 and to
mention the saturating value heuristic.

## TDD checklist

Work one test at a time, red to green, in `baseline/src/readout.rs`.

1. `value_quiet_board_is_zero`
2. `value_own_win_sentinel`
3. `value_two_opp_wins_is_near_loss`
4. `value_single_opp_win_is_negative`
5. `value_own_double_threat_is_positive`
6. `value_saturation_clamps_near_one`
7. `value_never_touches_one`
8. `value_monotonic_in_threats`
9. `value_is_side_to_move_relative`

After the unit tests pass, wire `readout::value` into
`BaselineEvaluator::evaluate` and confirm the existing integration
tests still pass.

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

The value strip from the CLI's "seeing the mind" chapter now shows a
number that means something. Watch three things:

* At the start, with no threats, the strip sits at `0.0`.
* When you build an open four, the value drops toward the near-loss
  sentinel — the seat concedes the position gracefully instead of
  hoping random search will save it.
* When the seat builds its own double threat, the value rises; if the
  tree finds the forced win, the strip locks to the terminal value and
  the seat presses the advantage.

The value is still a guess. It cannot see a ten-move VCF, and it does
not try to. That honesty is the point.

## Done when

* `cargo test -p baseline` passes.
* `cargo clippy --all-targets -- -D warnings` passes.
* `cargo fmt --all -- --check` makes no changes.
* The nine value tests are green.
* `BaselineEvaluator::evaluate` returns the wired value.
* The worktree is committed with:
  `feat(baseline): the honest value — baseline sub-chapter 05`.

## References

* [`../00-baseline-primer.md`](../00-baseline-primer.md) — §6 "The
  honest value heuristic" pins the ladder, the saturation rule, and the
  property-vs-constant split.
* [`../04-wins-and-blocks/wins-and-blocks.md`](../04-wins-and-blocks/wins-and-blocks.md)
  — the previous sub-chapter: policy tiers T1/T2 and the adapter.
* [`../06-the-three-ladder/the-three-ladder.md`](../06-the-three-ladder/the-three-ladder.md)
  — the next sub-chapter: double-three rungs added to the value.
* [`../07-the-tss-eye/the-tss-eye.md`](../07-the-tss-eye/the-tss-eye.md)
  — the root probe that sees threat sequences this heuristic cannot.
* [`../../playable-cli-tutorial/06-seeing-the-mind/a-value-strip/value-strip.md`](../../playable-cli-tutorial/06-seeing-the-mind/a-value-strip/value-strip.md)
  — the value strip overlay you watch during the capstone.
* `gomoku/crates/mcts/src/eval.rs` — the `Evaluator` seam and the
  `EvalResult::value` contract.
* `gomoku/crates/mcts/src/expand.rs` — where terminals receive exact
  `±1.0` values and the evaluator is never called.
