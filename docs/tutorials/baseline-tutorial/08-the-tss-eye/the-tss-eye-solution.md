> **Opt-in solution — read only after you have tried the chapter, or if you have been stuck for more than twenty minutes.**

Verified green from `gomoku/`: `cargo test -p baseline`, `cargo test -p cli --test acceptance`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`. Quotes slice commit `0e2fae9` on `baseline-reference`.

---

The slice changes four files. `baseline/src/probe.rs` is new. `baseline/src/lib.rs` gains one module declaration. `cli/src/session.rs` gains the feature-gated short-circuit. `cli/tests/acceptance.rs` gains the integration test.

## The engine warning that the probe inherits

The engine's threat-space search documents the meaning of `None` at
the top of `gomoku/crates/engine/src/tss.rs`, lines 11–13:

```rust
//! The oracle is soundness-first: `Some` means the line is machine-
//! verified by `verify_line`; `None` only means "not proven within
//! budget" and says nothing about the true value of the position.
```

The probe copies this wording into its own doc comment because any
chapter that uses the probe must keep the interpretation load-bearing.

## `baseline/src/lib.rs` — expose the probe module

`gomoku/crates/baseline/src/lib.rs`, line 11:

```rust
pub mod probe;
```

This single line makes `baseline::probe::ROOT_BUDGET` and
`baseline::probe::root_probe` reachable from the CLI. The module order
keeps the public surface alphabetical: `adapter`, `gravity`, `probe`,
`readout`.

## `baseline/src/probe.rs` — the probe module

This file is the whole slice on the baseline side. It has no
knowledge of MCTS, Burn, or patterns; it depends only on `engine`.

### Module-level contract

`gomoku/crates/baseline/src/probe.rs`, lines 1–6:

```rust
//! Root-level threat-space search probe.
//!
//! The probe is a once-per-move short-circuit in the CLI seat path,
//! not a per-node evaluator call. `Some` means a certified forced win;
//! `None` only means "not proven within budget" and says nothing about
//! the true value of the position.
```

The doc comment names the placement (CLI seat path), the frequency
(once per move), and the forbidden placement (per-node evaluator call).
It also repeats the engine's `None` warning.

### The pinned budget

`gomoku/crates/baseline/src/probe.rs`, lines 10–16:

```rust
/// Budget for the root-level TSS probe.
///
/// `[experiment]` starting value: 10,000 threat-space nodes, depth 30.
pub const ROOT_BUDGET: SearchBudget = SearchBudget {
    max_nodes: 10_000,
    max_depth: 30,
};
```

The budget is a public constant so the CLI and the tests share the
same caps. The `[experiment]` tag records that these numbers are a
starting point, not a permanent contract.

### The thin wrapper

`gomoku/crates/baseline/src/probe.rs`, lines 18–27:

```rust
/// Probe the current root for a certified forced win.
///
/// Returns `Some(proof)` when `engine::prove_forced_win` certifies a
/// win for the side to move within `budget`. Returns `None` when the
/// budget is exhausted or no proof is found — `None` says nothing
/// about whether a win exists.
#[must_use]
pub fn root_probe(board: &Board, budget: SearchBudget) -> Option<Proof> {
    prove_forced_win(board, board.to_move(), budget)
}
```

The wrapper exists so callers do not have to pass `board.to_move()`
and so the function name documents the intent. `#[must_use]` reminds
callers that `None` is a meaningful result and should not be dropped.

### Test: VCF line found and verified

`gomoku/crates/baseline/src/probe.rs`, lines 34–59:

```rust
#[test]
fn probe_finds_vcf_line() {
    // Reuse the engine TSS win-in-3 script.
    let mut board = Board::new();
    let script = [
        (7, 4),
        (7, 3),
        (7, 5),
        (0, 0),
        (7, 6),
        (0, 2),
        (6, 6),
        (0, 4),
        (8, 8),
        (0, 6),
    ];
    for &(r, c) in &script {
        board.play(Move::new(r, c).unwrap()).unwrap();
    }
    assert_eq!(board.to_move(), Color::Black);

    let proof = root_probe(&board, ROOT_BUDGET).expect("VCF must be found");
    assert_eq!(proof.winner, Color::Black);
    assert!(!proof.line.is_empty());
    assert!(verify_line(&board, &proof));
}
```

The test reuses the engine's win-in-3 script so the position is
independently known to be a VCF. It checks not only that a proof is
found but also that `verify_line` accepts it, exercising the soundness
net.

### Test: quiet board returns `None`

`gomoku/crates/baseline/src/probe.rs`, lines 61–65:

```rust
#[test]
fn probe_quiet_position_is_none() {
    let board = Board::new();
    assert!(root_probe(&board, ROOT_BUDGET).is_none());
}
```

This is the cheapest possible guard against the bug of treating `None`
as an error. An empty opening is unproven, which is the correct answer.

### Test: budget exhaustion returns `None`

`gomoku/crates/baseline/src/probe.rs`, lines 67–97:

```rust
#[test]
fn probe_respects_budget() {
    let mut board = Board::new();
    let script = [
        (7, 4),
        (7, 3),
        (7, 5),
        (0, 0),
        (7, 6),
        (0, 2),
        (6, 6),
        (0, 4),
        (8, 8),
        (0, 6),
    ];
    for &(r, c) in &script {
        board.play(Move::new(r, c).unwrap()).unwrap();
    }

    let tiny = SearchBudget {
        max_nodes: 1,
        max_depth: 30,
    };
    assert!(root_probe(&board, tiny).is_none());

    let shallow = SearchBudget {
        max_nodes: 10_000,
        max_depth: 2,
    };
    assert!(root_probe(&board, shallow).is_none());
}
```

The same VCF position is submitted with two under-powered budgets. A
single node is not enough to see past the first attacker move, and
depth 2 cannot fit a three-ply line. Both return `None`, proving that
the probe respects the budget honestly.

## `cli/src/session.rs` — the step-path short-circuit

The probe is wired into `execute_step` after the AI config is known and
before the evaluator is built.

`gomoku/crates/cli/src/session.rs`, lines 432–438:

```rust
#[cfg(feature = "baseline")]
if config.evaluator == EvaluatorKind::Baseline
    && let Some(proof) = baseline::probe::root_probe(&board, baseline::probe::ROOT_BUDGET)
{
    let mv = proof.line[0];
    return self.execute_play(mv);
}
```

The feature gate keeps the CLI compilable when `baseline` is disabled.
The `if let` chain keeps the happy path and the fall-through path on
one screen. When a proof is found, the function returns through
`execute_play`, so the same `Event::Moved` and `Event::GameOver`
logic runs as for any other move. When `None` is returned, execution
falls through to the normal MCTS path below.

## `cli/tests/acceptance.rs` — the integration test

The integration test proves that the CLI step path actually uses the
probe result rather than the search result.

`gomoku/crates/cli/tests/acceptance.rs`, lines 269–335:

```rust
/// The baseline seat, when faced with a known VCF root, must play the
/// first move of the certified proof line instead of searching.
#[cfg(feature = "baseline")]
#[test]
fn baseline_root_probe_plays_proof_first_move() {
    // Reuse the engine TSS win-in-3 script.
    let mut opening = Board::new();
    let script = [
        (7, 4),
        (7, 3),
        (7, 5),
        (0, 0),
        (7, 6),
        (0, 2),
        (6, 6),
        (0, 4),
        (8, 8),
        (0, 6),
    ];
    for &(r, c) in &script {
        opening.play(Move::new(r, c).unwrap()).unwrap();
    }
    assert_eq!(opening.to_move(), Color::Black);

    let proof = engine::prove_forced_win(
        &opening,
        Color::Black,
        engine::SearchBudget {
            max_nodes: 10_000,
            max_depth: 30,
        },
    )
    .expect("VCF must be proven");
    let expected_first = proof.line[0];

    let ai_cfg = AiConfig {
        evaluator: EvaluatorKind::Baseline,
        simulations: 200,
        c_puct: 1.5,
        temperature: 0.0,
        noise_epsilon: 0.0,
        explicit_selection: true,
    };
    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Ai(ai_cfg.clone()),
        white: Occupant::Human,
        default_config: ai_cfg,
        black_sims: None,
        white_sims: None,
        seed: Some(7),
        opening,
        swap2: false,
        puzzle: None,
    });

    let events = step(&mut core);
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Moved {
                mv,
                by: OccupantKind::Ai,
            } if *mv == expected_first
        )),
        "baseline AI must play the proof's first move {expected_first:?}, got {events:?}"
    );
}
```

The test sets the simulation count to 200, low enough that a normal
search might not find the full VCF line. It then asserts that the move
played is exactly the first move of the engine-certified proof. If the
probe were not wired in, the test would likely fail because 200
simulations would not reliably discover a win-in-3.
