> **Opt-in solution — read only after you have tried the chapter, or if you have been stuck for more than twenty minutes.**

Verified green from `gomoku/`: `cargo test -p baseline`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`. Quotes slice commit `f4d1bfe` on reference `baseline-reference`.

---

The slice changes three files. `readout.rs` adds the cap and the
`quiet_value` function. `lib.rs` adds the `quiet` flag, the
`without_quiet_value()` switch, and the gated wiring. `tests/acceptance.rs`
adds the ignored A/B measurement.

## `readout.rs` — the cap and the whisper function

`QUIET_CAP` sits one order below the double-three rung. The doc comment
on `quiet_value` writes the formula in plain text so the learner can see
exactly how naive the term is.

`gomoku/crates/baseline/src/readout.rs`, lines 22–68:

```rust
/// [experiment] cap for the quiet-position value whisper.
const QUIET_CAP: f32 = 0.05;

/// Logit weight for an immediate win for the side to move.
pub const T1_WIN: f32 = 64.0;
// ... existing tier constants ...

/// [experiment] Super-naive quiet-position value term.
///
/// Computes `clamp((Σ g·me − Σ g·you) / max(Σ g, 1.0), −0.05, +0.05)`
/// over the unmasked 225-cell gravity map. Positive when the side to
/// move sits in high-gravity cells (clustered, near the fight);
/// negative when the opponent does. The denominator protects the empty
/// board from division by zero.
#[must_use]
pub fn quiet_value(gravity: &[[f32; 15]; 15], planes: &engine::Planes) -> f32 {
    let mut weighted_me = 0.0f32;
    let mut weighted_you = 0.0f32;
    let mut gravity_sum = 0.0f32;
    for (r, row) in gravity.iter().enumerate() {
        for (c, &g) in row.iter().enumerate() {
            let dst = (r + 1) * engine::EXT + (c + 1);
            gravity_sum += g;
            if planes.me[dst] == 1 {
                weighted_me += g;
            } else if planes.you[dst] == 1 {
                weighted_you += g;
            }
        }
    }
    let denom = gravity_sum.max(1.0);
    let raw = (weighted_me - weighted_you) / denom;
    raw.clamp(-QUIET_CAP, QUIET_CAP)
}
```

## `lib.rs` — the flag and the wiring

The `quiet` field is `true` by default. `without_quiet_value()` is the
escape hatch for the A/B arm. The whisper branch runs only when the
honest value is exactly zero, so the threat ladder keeps priority.

`gomoku/crates/baseline/src/lib.rs`, lines 27–82:

```rust
pub struct BaselineEvaluator {
    device: Device,
    kernel: GravityKernel,
    net: ThreatNet,
    quiet: bool,
}

impl BaselineEvaluator {
    /// Create a new [`BaselineEvaluator`] on `device`.
    ///
    /// The [experiment] quiet-position value whisper is enabled by
    /// default; use [`Self::without_quiet_value`] to disable it.
    #[must_use]
    pub fn new(device: Device) -> Self {
        let kernel = GravityKernel::new(&device);
        let net = ThreatNet::new(&device);
        Self {
            device,
            kernel,
            net,
            quiet: true,
        }
    }

    /// Disable the [experiment] quiet-position value whisper.
    ///
    /// Chainable; intended for A/B measurement of the whisper term.
    #[must_use]
    pub fn without_quiet_value(mut self) -> Self {
        self.quiet = false;
        self
    }
}

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
        let value = readout::value(own, opp);
        // [experiment] quiet whisper: only when the threat ladder is silent.
        // `value == 0.0` also covers symmetric threat cancellation — that
        // counts as "quiet" for this experiment.
        let value = if value == 0.0 && self.quiet {
            readout::quiet_value(&gravity, &req.planes)
        } else {
            value
        };
        EvalResult { policy, value }
    }
}
```

## `readout.rs` — selected unit tests

The tests assert exact properties. `quiet_value_is_always_capped` is a
randomized stress test; the cap is the load-bearing claim.

`gomoku/crates/baseline/src/readout.rs`, lines 560–566:

```rust
#[test]
fn quiet_value_empty_board_is_zero() {
    let board = Board::new();
    let planes = engine::encode(&board);
    let gravity = gravity::gravity_map(&planes);
    assert_eq!(quiet_value(&gravity, &planes), 0.0f32);
}
```

`gomoku/crates/baseline/src/readout.rs`, lines 568–591:

```rust
#[test]
fn quiet_value_is_always_capped() {
    // The whisper is clamped to ±QUIET_CAP regardless of position.
    let mut rng = StdRng::seed_from_u64(20_260_901);
    for _ in 0..200 {
        let mut board = Board::new();
        let stones = rng.random_range(8..=24);
        for _ in 0..stones {
            let legal: Vec<Move> = board.empty_moves().collect();
            if legal.is_empty() {
                break;
            }
            let mv = legal[rng.random_range(0..legal.len())];
            let _ = board.play(mv);
        }
        let planes = engine::encode(&board);
        let gravity = gravity::gravity_map(&planes);
        let v = quiet_value(&gravity, &planes);
        assert!(
            v.abs() <= QUIET_CAP,
            "quiet value {v} exceeded cap {QUIET_CAP}"
        );
    }
}
```

`gomoku/crates/baseline/src/readout.rs`, lines 593–620:

```rust
#[test]
fn quiet_value_clustered_own_stones_is_positive() {
    // Black cluster in the centre, White stones scattered in corners.
    // Moves alternate, so play Black stones on odd plies and White on even.
    // No side has an immediate threat, so this is a "quiet" position.
    let mut board = Board::new();
    let script = [
        (7, 7),
        (0, 0),
        (7, 8),
        (0, 14),
        (8, 7),
        (14, 0),
        (8, 8),
        (14, 14),
    ];
    for &(r, c) in &script {
        board.play(Move::new(r, c).unwrap()).unwrap();
    }
    assert_eq!(board.to_move(), Color::Black);
    let planes = engine::encode(&board);
    let gravity = gravity::gravity_map(&planes);
    let v = quiet_value(&gravity, &planes);
    assert!(
        v > 0.0f32,
        "clustered own stones should yield positive quiet value, got {v}"
    );
}
```

## `lib.rs` — selected integration tests

These tests guard the two critical boundaries: the whisper must not
override a win, and disabling the whisper must return a quiet board to
exactly `0.0`.

`gomoku/crates/baseline/src/lib.rs`, lines 275–298:

```rust
#[test]
fn quiet_whisper_t1_win_still_returns_nine_five() {
    // Four Black stones in a row with a gap at each end.
    // Black to move has an immediate win; the whisper must not
    // override the threat ladder.
    let mut board = Board::new();
    let script = [
        (7, 3),
        (0, 0),
        (7, 4),
        (0, 2),
        (7, 5),
        (0, 4),
        (7, 6),
        (0, 6),
    ];
    for &(r, c) in &script {
        board.play(Move::new(r, c).unwrap()).unwrap();
    }
    let req = request_from_board(&board);
    let mut ev = BaselineEvaluator::new(Device::flex());
    let res = ev.evaluate(req);
    assert_eq!(res.value, 0.95f32);
}
```

`gomoku/crates/baseline/src/lib.rs`, lines 300–322:

```rust
#[test]
fn quiet_whisper_without_quiet_is_zero_on_quiet_cluster() {
    // Black cluster vs scattered White corners: a quiet position
    // where the whisper would otherwise be non-zero.
    let mut board = Board::new();
    let script = [
        (7, 7),
        (0, 0),
        (7, 8),
        (0, 14),
        (8, 7),
        (14, 0),
        (8, 8),
        (14, 14),
    ];
    for &(r, c) in &script {
        board.play(Move::new(r, c).unwrap()).unwrap();
    }
    let req = request_from_board(&board);
    let mut ev = BaselineEvaluator::new(Device::flex()).without_quiet_value();
    let res = ev.evaluate(req);
    assert_eq!(res.value, 0.0f32);
}
```

## `tests/acceptance.rs` — the A/B measurement

The ignored test implements the pre-registered protocol. It reuses the
existing `baseline_best_move` and `tactics_best_move` helpers from the
strength-ladder test.

`gomoku/crates/baseline/tests/acceptance.rs`, lines 596–681:

```rust
// ------------------------------------------------------------------
// [experiment] Quiet whisper A/B
// ------------------------------------------------------------------

/// Run explicitly in release:
/// `cargo test -p baseline --release --test acceptance quiet_whisper_ab_measured -- --ignored --nocapture`
///
/// Protocol: 20 games vs the `TacticsEvaluator` at 200 simulations with
/// alternating colors. One arm uses the default whisper-on evaluator,
/// the other uses `.without_quiet_value()`. Base seed 20_260_901.
#[ignore]
#[test]
fn quiet_whisper_ab_measured() {
    const GAMES: usize = 20;
    const SIMS: u32 = 200;
    const BASE_SEED: u64 = 20_260_901;

    fn play_game_ab(baseline_color: Color, quiet: bool, seed: u64) -> (Status, u32, f64) {
        let mut board = Board::new();
        let mut rng = StdRng::seed_from_u64(seed);
        let mut plies = 0u32;
        let start = Instant::now();
        let mut baseline_ev = if quiet {
            BaselineEvaluator::new(Device::flex())
        } else {
            BaselineEvaluator::new(Device::flex()).without_quiet_value()
        };
        let mut tactics_ev = mcts::TacticsEvaluator::new();

        while board.status() == Status::Ongoing && plies < 225 {
            let side = board.to_move();
            let mv = if side == baseline_color {
                baseline_best_move(&board, SIMS, &mut rng, &mut baseline_ev)
            } else {
                tactics_best_move(&board, SIMS, &mut rng, &mut tactics_ev)
            };
            assert!(
                board.is_legal(mv),
                "illegal move {mv:?} at ply {plies} for {side:?}"
            );
            board.play(mv).unwrap();
            plies += 1;
        }
        (board.status(), plies, start.elapsed().as_secs_f64())
    }

    println!("\n=== quiet whisper A/B ===\n");
    println!(
        "{:<12} {:>6} {:>6} {:>6} {:>10} {:>12}",
        "arm", "wins", "losses", "draws", "avg_plies", "avg_s/game"
    );
    for quiet in [true, false] {
        let label = if quiet { "whisper ON" } else { "whisper OFF" };
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut draws = 0usize;
        let mut total_plies = 0u32;
        let mut total_time = 0.0f64;
        for g in 0..GAMES {
            let baseline_color = if g % 2 == 0 {
                Color::Black
            } else {
                Color::White
            };
            let seed = BASE_SEED + g as u64;
            let (status, plies, secs) = play_game_ab(baseline_color, quiet, seed);
            total_plies += plies;
            total_time += secs;
            match status {
                Status::Won(c) if c == baseline_color => wins += 1,
                Status::Won(_) => losses += 1,
                Status::Draw => draws += 1,
                Status::Ongoing => panic!("game {g} did not terminate"),
            }
        }
        println!(
            "{:<12} {:>6} {:>6} {:>6} {:>10.1} {:>12.3}",
            label,
            wins,
            losses,
            draws,
            total_plies as f64 / GAMES as f64,
            total_time / GAMES as f64
        );
    }
}
```

## Measured result

The reference run produced:

| Arm | Wins | Losses | Draws | Avg plies | Avg s/game |
|-----|------|--------|-------|-----------|------------|
| Whisper ON | 20 | 0 | 0 | 23.5 | 3.842 |
| Whisper OFF | 20 | 0 | 0 | 34.5 | 5.847 |

Both arms win every game. The whisper arm finishes in 32% fewer plies
and 34% less time. The verdict is **KEEP**, with the residual risk
honestly noted: the value term is still `[experiment]`, and the net
tutorial will replace it.
