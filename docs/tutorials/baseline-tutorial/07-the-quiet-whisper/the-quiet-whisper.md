# Sub-chapter 07 — The quiet whisper

## Abstract

This sub-chapter adds an `[experiment]` value term for quiet positions.
When the honest threat-ladder value is exactly `0.0`, the evaluator
whispers a tiny side-to-move-relative number based on one naive idea:
"my stones in the gravity mass minus yours". The whisper is capped an
order of magnitude below the smallest threat rung, it is gated by a
chainable `without_quiet_value()` switch, and its fate is decided by a
pre-registered A/B measurement. The measured verdict is **KEEP** — but
the interesting lesson is not the win rate; it is efficiency.

## Glossary

| Term | Definition |
|------|------------|
| **Quiet whisper** | The `[experiment]` value term computed by `readout::quiet_value`. |
| **Threat-ladder silence** | The condition `readout::value(own, opp) == 0.0`: no wins, forced blocks, double threats, or double-threes fire. |
| **`QUIET_CAP`** | The cap `0.05`; the whisper never exceeds this magnitude. |
| **`quiet_value(gravity, planes)`** | The public function that computes the gravity-weighted stone differential. |
| **`quiet` flag** | The `bool` field on `BaselineEvaluator` that enables or disables the whisper. |
| **`without_quiet_value()`** | The chainable constructor method that sets `quiet = false`. |
| **A/B arms** | Whisper ON (default) versus whisper OFF, played against the same opponent. |
| **Efficiency metric** | Average plies and seconds per game, because the weak yardstick cannot discriminate win rate. |

## Context

Sub-chapters 05 and 06 built a value readout that is deliberately
silent on quiet boards. That silence is honest: a position with stones
but no firing threat map has no proven advantage. But PUCT backup then
has no gradient in quiet branches. The tree can flail, spreading
simulations almost uniformly until someone creates a threat.

The whisper is a gamble: give the tree a faint signal where the threat
ladder has none. Because the signal is naive — clustering near existing
stones is not the same as winning — it is flagged `[experiment]`,
capped far below any threat rung, and measured before it is allowed to
stay.

## Intention

The learner enables this behavior:

1. When the threat-ladder value is exactly `0.0`, the evaluator adds a
   small side-to-move-relative value based on gravity-weighted stone
   clustering.
2. The whisper is capped at `±0.05`, one order below the smallest
   threat rung (`±0.20` for a double-three fork).
3. The whisper never overrides a firing threat ladder; an own win still
   returns `0.95`.
4. The evaluator can disable the whisper with `without_quiet_value()`.
5. A quiet cluster returns `0.0` when the whisper is disabled.
6. A pre-registered A/B measures whether the whisper helps.

## Specification as tests

### Unit tests in `baseline/src/readout.rs`

* `quiet_value_empty_board_is_zero` — on an empty board the gravity map
  is all zeros, so `quiet_value` returns exactly `0.0`.
* `quiet_value_is_always_capped` — across 200 random quiet positions the
  absolute value never exceeds `QUIET_CAP`.
* `quiet_value_clustered_own_stones_is_positive` — a Black central
  cluster against scattered White corners yields a positive quiet value
  from Black's perspective.

### Integration tests in `baseline/src/lib.rs`

* `quiet_whisper_t1_win_still_returns_nine_five` — when Black has an
  immediate win, the evaluator returns `0.95`; the whisper does not
  override the threat ladder.
* `quiet_whisper_without_quiet_is_zero_on_quiet_cluster` — with the
  whisper disabled, the same cluster board returns exactly `0.0`.

### A/B measurement in `baseline/tests/acceptance.rs`

* `quiet_whisper_ab_measured` (`#[ignore]`, release-only) — 20 games
  versus `TacticsEvaluator` at 200 simulations, alternating colors,
  base seed `20_260_901`. One arm uses the default whisper-on evaluator;
  the other uses `.without_quiet_value()`.

## Walkthrough

The evaluator's execution path is unchanged until the value is chosen.
Only the final value branch is new.

### 1. The request arrives

`BaselineEvaluator::evaluate` receives an `EvalRequest` with relative
planes and the legal move list. It reconstructs the absolute-color
board, runs `patterns::net::analyze_both`, picks `own` and `opp` maps by
`board.to_move()`, and computes the gravity map.

### 2. The honest value speaks first

`readout::value(own, opp)` returns the threat-ladder value. If any
threat fires, the value is non-zero and the whisper is skipped. This
keeps the whisper from overriding a win, a forced block, a double
threat, or a double-three fork.

### 3. When the ladder is silent, the whisper may speak

If `value == 0.0` and the `quiet` flag is true, `evaluate` calls
`readout::quiet_value(&gravity, &req.planes)`. Otherwise it keeps the
honest value, which is `0.0`.

The condition `value == 0.0` also covers symmetric threat cancellation.
That counts as quiet for this experiment.

### 4. `quiet_value` computes the gravity-weighted differential

The function walks the 15×15 gravity map. For each cell it adds the
gravity score to a running total, and adds it to `weighted_me` or
`weighted_you` if the corresponding side-to-move-relative plane has a
stone there. The 17×17 planes are indexed with the border offset
`(r + 1) * engine::EXT + (c + 1)`.

The raw score is `(weighted_me - weighted_you) / max(gravity_sum, 1.0)`.
The denominator protects the empty board from division by zero. The
result is clamped to `[-QUIET_CAP, QUIET_CAP]`.

### 5. Why the cap is `0.05`

The smallest value-ladder rung is the double-three fork at `±0.20`.
`QUIET_CAP = 0.05` sits one order below it. Even a maximally wrong
whisper cannot pretend to be a fork. The threat ladder keeps priority.

### 6. What the tree does with it

The whisper gives backup a faint signal through quiet branches. A
position where own stones sit in the gravity mass gets a small positive
nudge; one where the opponent's stones sit there gets a small negative
udge. The search still verifies with terminals and visit counts. The
whisper is not proof; it is a nudge.

### 7. Why it stays `[experiment]`

A systematic value bias gets amplified by PUCT, not averaged out. The
whisper is deliberately naive: it rewards clustering, and clustering
can mislead. The flag, the cap, and the A/B are the discipline. If a
future stronger opponent exploits the bias, the whisper can be disabled
in one line. The net tutorial will replace the whole hand-written value
head.

## Low-level design

### [`gomoku/crates/baseline/src/readout.rs`](../../../../gomoku/crates/baseline/src/readout.rs)

Add one private constant:

* `QUIET_CAP: f32 = 0.05`

Add one public function:

```rust
#[must_use]
pub fn quiet_value(gravity: &[[f32; 15]; 15], planes: &engine::Planes) -> f32
```

Behavior:

* Sum `g * me` and `g * you` over the 15×15 gravity map, indexing
  `planes.me` and `planes.you` through the 17×17 border offset.
* Compute `(weighted_me - weighted_you) / max(gravity_sum, 1.0)`.
* Clamp the result to `[-QUIET_CAP, QUIET_CAP]`.
* Return `0.0` on an empty board because every gravity cell is zero.

Add the three `quiet_value` tests in the existing `#[cfg(test)] mod tests`
block.

### [`gomoku/crates/baseline/src/lib.rs`](../../../../gomoku/crates/baseline/src/lib.rs)

Add one field to `BaselineEvaluator`:

* `quiet: bool`

In `BaselineEvaluator::new`, set `quiet: true`. Add a chainable method:

```rust
#[must_use]
pub fn without_quiet_value(mut self) -> Self
```

which sets `quiet = false` and returns `self`.

In `BaselineEvaluator::evaluate`, after computing the base `value`, add
this branch:

```rust
let value = if value == 0.0 && self.quiet {
    readout::quiet_value(&gravity, &req.planes)
} else {
    value
};
```

Document the flag and the method with `[experiment]`.

### [`gomoku/crates/baseline/tests/acceptance.rs`](../../../../gomoku/crates/baseline/tests/acceptance.rs)

Add one ignored test:

```rust
#[ignore]
#[test]
fn quiet_whisper_ab_measured()
```

The test plays 20 games versus `TacticsEvaluator` at 200 simulations
with alternating colors. One arm uses
`BaselineEvaluator::new(Device::flex())`; the other uses
`BaselineEvaluator::new(Device::flex()).without_quiet_value()`. Base
seed `20_260_901`. It prints wins, losses, draws, average plies, and
average seconds per game for each arm.

## TDD checklist

Work one test at a time, red to green.

1. `quiet_value_empty_board_is_zero`
2. `quiet_value_is_always_capped`
3. `quiet_value_clustered_own_stones_is_positive`
4. `quiet_whisper_t1_win_still_returns_nine_five`
5. `quiet_whisper_without_quiet_is_zero_on_quiet_cluster`

After the unit and integration tests pass, add the ignored A/B test and
run it in release to record the verdict.

## Verification commands

From `gomoku/`:

```text
cargo test -p baseline
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Run the A/B measurement explicitly:

```text
cargo test -p baseline --release --test acceptance quiet_whisper_ab_measured -- --ignored --nocapture
```

## Capstone

From `gomoku/`:

```text
gomoku play --ai baseline --sims 4000
```

Play into the early midgame. Watch the value strip on quiet positions:
it now shows a tiny non-zero nudge instead of sitting flat at `0.0`.
When you build an open four or a fork, the threat ladder overrides the
whisper and the strip jumps to the appropriate sentinel. The seat still
wins when it should; the only change is the faint signal it carries
through the quiet stretches.

Then run the A/B. The expected result is that both arms win every game
against the mock — the mock is too weak to discriminate — but the
whisper arm finishes its wins in fewer plies. Efficiency, not win rate,
is the discriminating metric.

## Done when

* The five chapter tests pass.
* All earlier baseline tests still pass.
* `cargo clippy --all-targets -- -D warnings` passes.
* `cargo fmt --all -- --check` makes no changes.
* The A/B measurement is run and its numbers are recorded.
* The worktree is committed with:
  `feat(baseline): the quiet whisper — [experiment] quiet-position value term`.

## References

* [`../00-baseline-primer.md`](../00-baseline-primer.md) — §2 "What the
tree does with it" lists the whisper row; §6 describes the value ladder
and the `±0.20` double-three rung that sets the cap.
* [`../06-the-three-ladder/the-three-ladder.md`](../06-the-three-ladder/the-three-ladder.md)
— the previous sub-chapter: the value ladder rungs that must stay above
the whisper.
* [`../08-the-tss-eye/the-tss-eye.md`](../08-the-tss-eye/the-tss-eye.md)
— the next sub-chapter: the root probe that sees sequences the whisper
cannot.
* [`../09-acceptance/acceptance.md`](../09-acceptance/acceptance.md) —
where the A/B numbers feed the measured ladder.
* [`../../convolutions-tutorial/07-seeing-it.md`](../../convolutions-tutorial/07-seeing-it.md)
— the `[experiment]` weight convention this sub-chapter follows.
* `gomoku/crates/mcts/src/eval.rs` — the `EvalResult::value` contract
and the backup sign flip.
* `gomoku/crates/mcts/src/expand.rs` — where terminals receive exact
`±1.0` values and the evaluator is never called.
