> **Opt-in solution — read only after you have tried the chapter, or if you have been stuck for more than twenty minutes.**

Verified green from `gomoku/`: `cargo test -p baseline`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`. Quotes slice commit `1793029` on reference `baseline-reference`.

---

The slice changes two files. `readout.rs` adds the value constants,
helpers, the `value` function, and the tests. `lib.rs` wires the value
into `BaselineEvaluator::evaluate` and updates the struct doc comment.

## `readout.rs` — value constants

The value constants are declared as private `f32` items. `V_CLAMP` is
strictly less than `1.0` to keep terminal truth reserved for the
search.

`gomoku/crates/baseline/src/readout.rs`, lines 6–17:

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
/// Absolute value ceiling; ±1 is reserved for proven terminals.
const V_CLAMP: f32 = 0.99;
```

## `readout.rs` — the `value` function

The function is the only new public API. It inspects the maps in
priority order, short-circuits on decisive threats, accumulates lesser
threats, and clamps. The doc comment names the missing double-three
rungs so the learner knows the ladder is intentionally incomplete at
this sub-chapter.

`gomoku/crates/baseline/src/readout.rs`, lines 36–66:

```rust
/// Side-to-move value heuristic from two threat maps.
///
/// The ladder (minus the double-three rungs that land in chapter 06):
/// own win ⇒ +0.95; opponent ≥2 wins ⇒ −0.99; otherwise accumulate
/// (−0.75 for opponent's single win, +0.85 for own double threat,
/// −0.85 for opponent double threat) and clamp to ±0.99. A quiet
/// board returns exactly 0.0.
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

    v.clamp(-V_CLAMP, V_CLAMP)
}
```

## `readout.rs` — map helpers

These two helpers keep `value` focused on the ladder. They treat any
positive cell as a firing threat; the maps from `analyze_both` use `1.0`
for positive cells and `0.0` elsewhere.

`gomoku/crates/baseline/src/readout.rs`, lines 68–76:

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

## `lib.rs` — wiring the value into `evaluate`

`BaselineEvaluator::evaluate` already has `own` and `opp` maps and the
policy logits. The change is one field in the returned `EvalResult`.
The struct doc comment is updated to v3.

`gomoku/crates/baseline/src/lib.rs`, lines 20–29:

```rust
/// The baseline evaluator.
///
/// v3 combines the gravity kernel with immediate wins and forced
/// blocks from the threat network, plus the chapter-05 saturating
/// value heuristic.
pub struct BaselineEvaluator {
    device: Device,
    kernel: GravityKernel,
    net: ThreatNet,
}
```

`gomoku/crates/baseline/src/lib.rs`, lines 45–61:

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

## `readout.rs` — selected tests

The tests assert properties, never approximate equality. They reuse
`empty_maps()` to build a quiet starting point.

`gomoku/crates/baseline/src/readout.rs`, lines 133–138:

```rust
#[test]
fn value_quiet_board_is_zero() {
    let own = empty_maps();
    let opp = empty_maps();
    assert_eq!(value(&own, &opp), 0.0f32);
}
```

`gomoku/crates/baseline/src/readout.rs`, lines 140–146:

```rust
#[test]
fn value_own_win_sentinel() {
    let mut own = empty_maps();
    own.wins[7][7] = 1.0f32;
    let opp = empty_maps();
    assert_eq!(value(&own, &opp), V_OWN_WIN);
}
```

`gomoku/crates/baseline/src/readout.rs`, lines 194–228:

```rust
#[test]
fn value_never_touches_one() {
    // Enumerate a small grid of threat combinations and assert the
    // absolute value stays strictly below 1.0 and at most 0.99.
    let configs: Vec<(bool, usize, bool, bool)> = vec![
        (false, 0, false, false),
        (true, 0, false, false),
        (false, 1, false, false),
        (false, 2, false, false),
        (false, 0, true, false),
        (false, 0, false, true),
        (false, 1, true, false),
        (false, 1, false, true),
        (true, 0, true, false),
    ];
    for (own_win, opp_wins, own_dt, opp_dt) in configs {
        let mut own = empty_maps();
        let mut opp = empty_maps();
        if own_win {
            own.wins[0][0] = 1.0f32;
        }
        for cell in opp.wins[0].iter_mut().take(opp_wins) {
            *cell = 1.0f32;
        }
        if own_dt {
            own.double_threats[7][7] = 1.0f32;
        }
        if opp_dt {
            opp.double_threats[7][7] = 1.0f32;
        }
        let v = value(&own, &opp);
        assert!(v.abs() < 1.0f32, "value {v} touched ±1");
        assert!(v.abs() <= V_CLAMP, "value {v} exceeded clamp");
    }
}
```

`gomoku/crates/baseline/src/readout.rs`, lines 261–275:

```rust
#[test]
fn value_is_side_to_move_relative() {
    // Black holds a double threat. From Black's perspective the
    // value is positive; from White's perspective the same
    // absolute position means the opponent holds it, so value is
    // the negative mirror.
    let mut black_maps = empty_maps();
    black_maps.double_threats[7][7] = 1.0f32;
    let white_maps = empty_maps();

    let from_black = value(&black_maps, &white_maps);
    let from_white = value(&white_maps, &black_maps);
    assert_eq!(from_black, V_OWN_DOUBLE_THREAT);
    assert_eq!(from_white, -V_OWN_DOUBLE_THREAT);
}
```
