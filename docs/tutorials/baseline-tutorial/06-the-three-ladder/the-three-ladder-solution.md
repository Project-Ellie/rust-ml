> **Opt-in solution — read only after you have tried the chapter, or if
> you have been stuck for more than twenty minutes.**

This file quotes the verified reference implementation for sub-chapter
06, commit `b8e4168` on the `baseline-reference` worktree. It changes
`gomoku/crates/baseline/src/readout.rs` and updates the doc comments in
`gomoku/crates/baseline/src/lib.rs`.

Verified green from `gomoku/`:

* `cargo test -p baseline`
* `cargo clippy --all-targets -- -D warnings`
* `cargo fmt --all -- --check`

## `lib.rs` — the evaluator now advertises v4

The struct and `evaluate` body do not change. Only the doc comment is
updated so the next reader knows the full tier table and the
double-three value rungs are active.

`gomoku/crates/baseline/src/lib.rs`, lines 20–30:

```rust
/// The baseline evaluator.
///
/// v4 combines the gravity kernel with the full policy tier table
/// (wins, forced blocks, double threats, double threes, threes,
/// defensive threes) and the chapter-06 value ladder including
/// double-three rungs.
pub struct BaselineEvaluator {
    device: Device,
    kernel: GravityKernel,
    net: ThreatNet,
}
```

`gomoku/crates/baseline/src/lib.rs`, lines 46–62:

```rust
impl Evaluator for BaselineEvaluator {
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
        let _ = &self.device;
        let board = adapter::reconstruct_board(&req.planes, &req.legal);
        let maps = patterns::net::analyze_both(&self.net, &board, &self.device);
        let (own, opp) = match board.to_move() {
            Color::Black => (&maps[0], &maps[1]),
            Color::White => (&maps[1], &maps[0]),
        };
        let gravity = self.kernel.compute(&req.planes);
        let policy = readout::policy_logits(&gravity, own, opp, &req.legal);
        EvalResult {
            policy,
            value: readout::value(own, opp),
        }
    }
}
```

The execution path is unchanged: reconstruct, analyze both sides, pick
own/opponent by `to_move`, compute gravity, then hand the three maps to
the readout functions.

## `readout.rs` — value constants and value ladder

The value half of the readout adds two new rungs for double-three forks.
The fast paths for own win and two opponent wins stay at the top.

`gomoku/crates/baseline/src/readout.rs`, lines 6–21:

```rust
/// Value of an own immediate win.
const V_OWN_WIN: f32 = 0.95;
/// Value near-loss for two or more opponent immediate wins.
const V_TWO_OPP_WINS: f32 = -0.99;
/// Value contribution for exactly one opponent immediate win.
const V_OPP_WIN: f32 = -0.75;
/// Value contribution for an own double threat.
const V_OWN_DOUBLE_THREAT: f32 = 0.85;
/// Value contribution for an opponent double threat.
const V_OPP_DOUBLE_THREAT: f32 = -0.85;
/// Value contribution for an own double-three fork.
const V_OWN_DOUBLE_THREE: f32 = 0.20;
/// Value contribution for an opponent double-three fork.
const V_OPP_DOUBLE_THREE: f32 = -0.20;
/// Absolute value ceiling; ±1 is reserved for proven terminals.
const V_CLAMP: f32 = 0.99;
```

`gomoku/crates/baseline/src/readout.rs`, lines 40–76:

```rust
/// Side-to-move value heuristic from two threat maps.
///
/// The ladder: own win ⇒ +0.95; opponent ≥2 wins ⇒ −0.99; otherwise
/// accumulate (−0.75 for opponent's single win, +0.85 for own double
/// threat, −0.85 for opponent double threat, +0.20 for own double
/// three, −0.20 for opponent double three) and clamp to ±0.99. A
/// quiet board returns exactly 0.0.
#[must_use]
pub fn value(own: &ThreatMaps, opp: &ThreatMaps) -> f32 {
    if has_positive(&own.wins) {
        return V_OWN_WIN;
    }

    let opp_wins = count_positive(&opp.wins);
    if opp_wins >= 2 {
        return V_TWO_OPP_WINS;
    }

    let mut v = 0.0f32;
    if opp_wins == 1 {
        v += V_OPP_WIN;
    }
    if has_positive(&own.double_threats) {
        v += V_OWN_DOUBLE_THREAT;
    }
    if has_positive(&opp.double_threats) {
        v += V_OPP_DOUBLE_THREAT;
    }
    if has_positive(&own.double_threes) {
        v += V_OWN_DOUBLE_THREE;
    }
    if has_positive(&opp.double_threes) {
        v += V_OPP_DOUBLE_THREE;
    }

    v.clamp(-V_CLAMP, V_CLAMP)
}
```

The double-three additions are the two new `if has_positive` blocks.
They sit after the double-threat rungs, so a quiet board still returns
exactly `0.0` and the clamp still keeps the result strictly below `±1`.

## `readout.rs` — helper predicates

`gomoku/crates/baseline/src/readout.rs`, lines 78–86:

```rust
fn has_positive(map: &[[f32; 15]; 15]) -> bool {
    map.iter().any(|row| row.iter().any(|&v| v > 0.0))
}

fn count_positive(map: &[[f32; 15]; 15]) -> usize {
    map.iter()
        .map(|row| row.iter().filter(|&&v| v > 0.0).count())
        .sum()
}
```

These predicates are the binary-membership engine. `has_positive` asks
"is this side's map non-zero anywhere?" for the value ladder.
`count_positive` asks "how many win cells does the opponent have?" for
the two-or-more-wins fast path.

## `readout.rs` — policy tier constants

The policy ladder now spans T1 through T4c. T5 is gravity itself, capped
at `1.0` after normalization. `T5_MAX` is only used in the invariant
test, so it carries the dead-code allow.

`gomoku/crates/baseline/src/readout.rs`, lines 23–38:

```rust
/// Logit weight for an immediate win for the side to move.
pub const T1_WIN: f32 = 64.0;
/// Logit weight for a forced block (opponent immediate win).
pub const T2_FORCED_BLOCK: f32 = 32.0;
/// Logit weight for an own double threat.
pub const T3_DOUBLE_THREAT: f32 = 16.0;
/// Logit weight for an own double-three fork.
pub const T4A_DOUBLE_THREE: f32 = 8.0;
/// Logit weight for any own three-maker.
pub const T4B_THREE: f32 = 4.0;
/// Logit weight for any opponent three-maker or double-three.
pub const T4C_DEFENSIVE: f32 = 2.0;

/// Maximum possible gravity contribution after normalization.
#[allow(dead_code)]
const T5_MAX: f32 = 1.0;
```

The gaps are powers of two so every reachable logit sum is an exact f32
value. That lets the tests assert with `==`.

## `readout.rs` — policy_logits

`gomoku/crates/baseline/src/readout.rs`, lines 88–125:

```rust
/// Build a 225-vector of policy logits from threat maps and gravity.
///
/// Tiers are applied by binary membership; occupied or illegal cells
/// receive `f32::NEG_INFINITY`.
#[must_use]
pub fn policy_logits(
    gravity: &[[f32; 15]; 15],
    own: &ThreatMaps,
    opp: &ThreatMaps,
    legal: &[Move],
) -> [f32; 225] {
    let mut logits = [f32::NEG_INFINITY; 225];
    for mv in legal {
        let r = mv.row() as usize;
        let c = mv.col() as usize;
        let mut logit = gravity[r][c];
        if own.wins[r][c] > 0.0 {
            logit += T1_WIN;
        }
        if opp.wins[r][c] > 0.0 {
            logit += T2_FORCED_BLOCK;
        }
        if own.double_threats[r][c] > 0.0 {
            logit += T3_DOUBLE_THREAT;
        }
        if own.double_threes[r][c] > 0.0 {
            logit += T4A_DOUBLE_THREE;
        }
        if own.threes_per_dir.iter().any(|dir| dir[r][c] > 0.0) {
            logit += T4B_THREE;
        }
        if opp.double_threes[r][c] > 0.0 || opp.threes_per_dir.iter().any(|dir| dir[r][c] > 0.0) {
            logit += T4C_DEFENSIVE;
        }
        logits[mv.index()] = logit;
    }
    logits
}
```

The key design points live in the last four checks:

* `T3_DOUBLE_THREAT` is added once per cell in the own double-threat map.
* `T4A_DOUBLE_THREE` is added once per cell in the own double-three map.
* `T4B_THREE` uses `iter().any(...)` to add the weight once, no matter
  how many directions produce a three.
* `T4C_DEFENSIVE` looks at opponent double-threes and opponent
  three-makers, but **not** opponent double threats. That is the
  deliberate absence from the primer.

## `readout.rs` — the chapter tests

The tests are in the same `#[cfg(test)] mod tests` block. They reuse
`empty_maps`, `board_from_ascii`, `BaselineEvaluator`, and
`gravity_map`.

### `t3_double_threat_outranks_gravity_stack`

`gomoku/crates/baseline/src/readout.rs`, lines 315–357:

```rust
#[test]
fn t3_double_threat_outranks_gravity_stack() {
    // Classic fork: Black plays (7,7) and creates two closed fours
    // whose winning cells are different, i.e. an own double threat.
    let board = board_from_ascii(
        "
        . . . . . . . . . . . . . . O
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . O . . . . . . .
        . . . . . . . X . . . . . . .
        . . . . . . . X . . . . . . .
        . . . . . . . X . . . . . . .
        . . . O X X X . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . O . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        O . . . . . . . . . . . . . O
        ",
    );
    let req = EvalRequest {
        planes: engine::encode(&board),
        legal: board.empty_moves().collect(),
    };
    let gravity = gravity::gravity_map(&req.planes);
    let mut ev = BaselineEvaluator::new(Device::flex());
    let res = ev.evaluate(req);

    let fork = Move::new(7, 7).unwrap();
    let expected = T3_DOUBLE_THREAT + gravity[fork.row() as usize][fork.col() as usize];
    assert_eq!(res.policy[fork.index()], expected);

    // The fork move strictly outranks every purely gravitational cell.
    let max_other = board
        .empty_moves()
        .filter(|&mv| mv != fork)
        .map(|mv| res.policy[mv.index()])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(res.policy[fork.index()] > max_other + 1.0f32);
}
```

This test shows that `T3_DOUBLE_THREAT` is large enough to beat every
other cell even if all other cells stack gravity and the lower tiers.
The `+ 1.0` margin is the smallest guaranteed gap from the invariant.

### `t4_binary_membership_no_double_count`

`gomoku/crates/baseline/src/readout.rs`, lines 359–397:

```rust
#[test]
fn t4_binary_membership_no_double_count() {
    // Verified double-three fork from patterns/oracle: Black stones
    // at (5,8),(6,8),(7,6),(7,7); playing (7,8) creates an open
    // three horizontally and vertically.
    let board = board_from_ascii(
        "
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . X . . . . . .
        . . . . . . . . X . . . . . .
        . . . . . . X X . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . O . . . . . . . . . . .
        . . . O . . . . . . . . . . .
        . . . . O O . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        ",
    );
    let req = EvalRequest {
        planes: engine::encode(&board),
        legal: board.empty_moves().collect(),
    };
    let gravity = gravity::gravity_map(&req.planes);
    let mut ev = BaselineEvaluator::new(Device::flex());
    let res = ev.evaluate(req);

    let fork = Move::new(7, 8).unwrap();
    // T4a (double-three) + T4b (any own three) stack by design;
    // T4b must not be added once per direction.
    let expected =
        T4A_DOUBLE_THREE + T4B_THREE + gravity[fork.row() as usize][fork.col() as usize];
    assert_eq!(res.policy[fork.index()], expected);
}
```

The assertion is `T4A + T4B + gravity`, not `T4A + 2·T4B + gravity`.
That is the binary-membership property that holds the domination
invariant together.

### `t4c_defensive_tier_sees_opponent_threes`

`gomoku/crates/baseline/src/readout.rs`, lines 399–434:

```rust
#[test]
fn t4c_defensive_tier_sees_opponent_threes() {
    // White stones at (7,5) and (7,7): (7,6) completes an open
    // three for White. From Black's perspective it is a defensive
    // three-maker (T4c).
    let board = board_from_ascii(
        "
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . O . O . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        X X . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        ",
    );
    let req = EvalRequest {
        planes: engine::encode(&board),
        legal: board.empty_moves().collect(),
    };
    let gravity = gravity::gravity_map(&req.planes);
    let mut ev = BaselineEvaluator::new(Device::flex());
    let res = ev.evaluate(req);

    let block = Move::new(7, 6).unwrap();
    let expected = T4C_DEFENSIVE + gravity[block.row() as usize][block.col() as usize];
    assert_eq!(res.policy[block.index()], expected);
}
```

This test confirms that the defensive tier fires on an opponent
three-maker. It also shows the tier is small: it cannot outrank a real
own threat, but it gives the search a nudge toward urgent blocks.

### `fork_board_argmax_is_the_fork`

`gomoku/crates/baseline/src/readout.rs`, lines 436–475:

```rust
#[test]
fn fork_board_argmax_is_the_fork() {
    // The chapter-09 CLI acceptance fork board: Black to move and
    // (7,7) is the forcing double-threat maker.
    let board = board_from_ascii(
        "
        . . . . . . . . . . . . . . O
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . O . . . . . . .
        . . . . . . . X . . . . . . .
        . . . . . . . X . . . . . . .
        . . . . . . . X . . . . . . .
        . . . O X X X . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . O . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        O . . . . . . . . . . . . . O
        ",
    );
    let req = EvalRequest {
        planes: engine::encode(&board),
        legal: board.empty_moves().collect(),
    };
    let mut ev = BaselineEvaluator::new(Device::flex());
    let res = ev.evaluate(req);

    let argmax = board
        .empty_moves()
        .max_by(|a, b| {
            res.policy[a.index()]
                .partial_cmp(&res.policy[b.index()])
                .unwrap()
        })
        .unwrap();
    assert_eq!(argmax, Move::new(7, 7).unwrap());
}
```

This is the behavioral proof: at temperature 0, the seat picks the fork
cell on a known board.

### `value_double_three_rungs`

`gomoku/crates/baseline/src/readout.rs`, lines 477–507:

```rust
#[test]
fn value_double_three_rungs() {
    // The Black fork from t4_binary_membership_no_double_count,
    // with quiet White filler so only Black owns a double-three.
    let board = board_from_ascii(
        "
        O . . . . . . . . . . . . . .
        O . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . O
        . . . . . . . . . . . . . . O
        . . . . . . . . . . . . . . .
        . . . . . . . . X . . . . . .
        . . . . . . . . X . . . . . .
        . . . . . . X X . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        ",
    );
    let req = EvalRequest {
        planes: engine::encode(&board),
        legal: board.empty_moves().collect(),
    };
    let mut ev = BaselineEvaluator::new(Device::flex());
    let res = ev.evaluate(req);
    assert_eq!(res.value, V_OWN_DOUBLE_THREE);
}
```

The value test reuses the double-three board but swaps White's stones
for quiet filler. That removes any White threats and leaves only Black's
double-three, so the value ladder lands exactly on `+0.20`.

### `full_table_domination_invariant`

`gomoku/crates/baseline/src/readout.rs`, lines 509–525:

```rust
#[test]
#[allow(clippy::assertions_on_constants)]
fn full_table_domination_invariant() {
    assert!(
        T2_FORCED_BLOCK
            > T3_DOUBLE_THREAT + T4A_DOUBLE_THREE + T4B_THREE + T4C_DEFENSIVE + T5_MAX
    );
    assert!(
        T1_WIN
            > T2_FORCED_BLOCK
                + T3_DOUBLE_THREAT
                + T4A_DOUBLE_THREE
                + T4B_THREE
                + T4C_DEFENSIVE
                + T5_MAX
    );
}
```

This is the derivation test. It proves that the numbers were chosen so
that no stack of lower tiers can reach the next tier. The first
inequality says `32 > 16 + 8 + 4 + 2 + 1 = 31`. The second says
`64 > 32 + 16 + 8 + 4 + 2 + 1 = 63`.

## References

* [`the-three-ladder.md`](./the-three-ladder.md) — the tutorial.
* [`the-three-ladder-implementation.md`](./the-three-ladder-implementation.md)
— the construction plan.
* [`00-baseline-primer.md`](../00-baseline-primer.md) — the design
authority for the tier table and value ladder.
* [`gomoku/crates/baseline/src/readout.rs`](../../../../gomoku/crates/baseline/src/readout.rs)
— the quoted source file.
* [`gomoku/crates/baseline/src/lib.rs`](../../../../gomoku/crates/baseline/src/lib.rs)
— the evaluator wiring.
