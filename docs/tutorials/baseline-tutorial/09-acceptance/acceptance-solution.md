> **Opt-in solution — read only after you have tried the chapter, or if you have been stuck for more than twenty minutes.**
>
> This file quotes the reference implementation of sub-chapter 09:
> `gomoku/crates/baseline/tests/acceptance.rs` and the one-line flip in
> `gomoku/crates/cli/src/main.rs`. Verified against the reference build,
> branch `baseline-reference`, commit `adac9a0`.

# Sub-chapter 09 — opt-in solution

## The deliberate duplication

The acceptance file opens with its honest confession: the pack parser
it needs cannot come from `cli`, because `cli` already depends on
`baseline` — a dev-dependency the other way would close a cycle. So a
minimal copy lives in the test, and the comment names the precedent
(`acceptance.rs:14-18`):

```rust
// ------------------------------------------------------------------
// Puzzle pack parsing — deliberate minimal duplication of
// `cli::puzzle::PuzzlePack` to avoid a dev-dependency cycle (cli
// already depends on baseline).
// ------------------------------------------------------------------
```

The parser itself (`acceptance.rs:21-170`) mirrors the CLI sub-chapter
07a format exactly: serde mirrors of `{count, items}` and `{i, a, b, w,
l}`, count-mismatch rejection, per-color duplicate detection, and the
coordinate crossing — file coordinates are `[x, y]`, so the move is
`Move::new(y, x)` (`acceptance.rs:162-169`):

```rust
    fn parse_stones(item: u32, coords: &[[u8; 2]]) -> Result<Vec<Move>, String> {
        coords
            .iter()
            .map(|&[x, y]| {
                Move::new(y, x).ok_or(format!("puzzle {item} coord ({x},{y}) off board"))
            })
            .collect()
    }
```

Paths anchor at `env!("CARGO_MANIFEST_DIR")`, so the test finds
`data/puzzles/` regardless of the invocation directory
(`acceptance.rs:73-80`).

## The drivers

The cast is one enum (`acceptance.rs:179-186`):

```rust
/// The brain seated at one side.
enum Brain {
    /// Uniform random legal moves.
    Random,
    /// Baseline evaluator with the given simulation budget.
    Baseline(u32),
    /// Tactics evaluator with the given simulation budget.
    Tactics(u32),
}
```

A searched move is the `mcts` public API end to end — search, visit
distribution, select at temperature 0.0 (`acceptance.rs:193-208`):

```rust
fn baseline_best_move(
    board: &Board,
    simulations: u32,
    rng: &mut StdRng,
    ev: &mut BaselineEvaluator,
) -> Move {
    let cfg = SearchConfig {
        simulations,
        c_puct: 1.5,
    };
    let outcome = search(board, ev, &cfg);
    let dist = visit_distribution(&outcome.tree, outcome.root);
    select_move(&dist, 0.0, rng).expect("root has legal edges")
}
```

The evaluator arrives as a parameter: constructing the `ThreatNet`
takes real work, and creating it per move would multiply the cost of
every game. `play_game` creates one evaluator per brain kind up front
and threads them through the loop.

`play_game` (`acceptance.rs:226-260`) owns the one assert every game
shares — legality — and returns the throughput raw material
`(status, plies, baseline_evals, baseline_eval_time)` alongside the
result.

## The examiner

`solve_puzzle` (`acceptance.rs:263-304`) seats the baseline as the
attacker with the CLI seat's exact wiring — the root probe first, the
search on `None`:

```rust
        let mv = if side == attacker {
            // Root probe first, exactly as the CLI seat does.
            if let Some(proof) = baseline::probe::root_probe(&board, baseline::probe::ROOT_BUDGET) {
                proof.line[0]
            } else {
                baseline_best_move(&board, simulations, &mut rng, &mut ev)
            }
        } else {
            // Follow the pack's claimed defender move when available.
            let solution_mv = puzzle
                .solution
                .get(ply as usize)
                .filter(|(_, c)| *c == side)
                .map(|(mv, _)| *mv);
            solution_mv.unwrap_or_else(|| random_move(&board, &mut rng))
        };
```

The defender follows the pack's claimed line, so a *solved* puzzle is
one the baseline converted against the reference defense — not against
a cooperative opponent.

## The tactical asserts

Both asserts reuse the same shape: build the board, 50 simulations,
assert membership in the engine's truth set. The win-in-1 board
(`acceptance.rs:309-340`):

```rust
#[test]
fn win_in_one_is_solved_at_50_sims() {
    let b = board_from_ascii(
        "
        O . O . O . O . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . X X X X . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        ",
    );
    assert_eq!(b.to_move(), Color::Black);

    let mut rng = StdRng::seed_from_u64(1);
    let mut ev = BaselineEvaluator::new(Device::flex());
    let mv = baseline_best_move(&b, 50, &mut rng, &mut ev);
    let wins = engine::immediate_wins(&b, Color::Black);
    assert!(
        wins.contains(mv),
        "best move {mv:?} must be an immediate win"
    );
}
```

The forced-block assert (`acceptance.rs:342-371`) scripts eight plies
and pins the setup first — `blocks.len() == 1` — so the test fails on
a broken *board* with a clear message, not on a confused search.

The flood's honesty comment (`acceptance.rs:377-383`) records the
trade-off the chapter discusses: the fast evaluator stands in because
the plumbing under test is evaluator-agnostic.

## The measurement

The protocol is constants, pre-registered before the first run
(`acceptance.rs:407-411`):

```rust
fn strength_ladder_measured() {
    const RANDOM_GAMES: usize = 10;
    const MOCK_GAMES: usize = 20;
    const VCF_SAMPLE: usize = 100;
    const BASE_SEED: u64 = 20_260_801;
```

The match loop alternates colors by game index and carries its bar as
an assertion (`acceptance.rs:472-525`); the seed arithmetic keeps
every game independently reproducible:

```rust
        for g in 0..MOCK_GAMES {
            let seed = BASE_SEED + g as u64 + 200 * 100_000;
            let (black, white) = if g % 2 == 0 {
                (Brain::Baseline(200), Brain::Tactics(200))
            } else {
                (Brain::Tactics(200), Brain::Baseline(200))
            };
```

The VCF section (`acceptance.rs:533-590`) samples a seeded
Fisher-Yates prefix across the five pack files, prints progress every
ten puzzles, and asserts its own bar: `solved >= VCF_SAMPLE / 2`.

## The flip

One line in `ai_config_from` (`crates/cli/src/main.rs:309`):

```rust
        evaluator: search.ai.map(Into::into).unwrap_or(EvaluatorKind::Baseline),
```

The baseline is the default brain; the mock stays one flag away.
