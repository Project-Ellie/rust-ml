# Chapter 10 — Acceptance

## Context

Everything works in unit tests. The tree selects, expands, backs up,
and chooses moves; the mock evaluator gives it taste; the sign
sentinels guard the backup convention. Now you prove the whole crate
against milestone 2's acceptance bar (ch. 12 §12–13, primer §7 item 7).

This chapter is the capstone. It is also, deliberately, the chapter
where you stop adding behavior and start *locking* the surface that
later milestones will import. After this, milestone 2 is done.

## Intention

Land three things:

1. **Tactical acceptance suite** in `tests/acceptance.rs`:
   * win-in-1 solved at 50 simulations,
   * forced blocks taken at 50 simulations,
   * win-in-3 first move found at 200 simulations.
2. **Sanity acceptance suite**: 1,000 full games against
   `UniformEvaluator`, 25 simulations per move, seeded RNG — every move
   legal, every game terminates within 225 plies, no panic, no hang.
3. **Crate polish**: `#![deny(missing_docs)]`, a reviewed public
   re-export surface in `lib.rs`, and a dead-code sweep.

Done means `cargo test -p mcts` passes, clippy is clean, fmt is clean,
and you can answer the question: "what does milestone 2 leave for
milestone 3, 4, and 5?"

## Mental mapping

### The two acceptance properties and why each is shaped as it is

**Tactical property.** Gomoku has a rich corpus of solved positions —
threat puzzles are free oracle data (primer §7, ch. 12 §3). If MCTS
with a sane prior cannot find a win-in-1, a forced block, or a simple
win-in-3, then either the sign convention is wrong, the prior is not
being applied, or the selection formula is broken. The tactical suite
is therefore a *proof of direction*, not a proof of strength. The
numbers are modest (50 or 200 simulations) precisely because a correct
search with a concentrating prior should solve these positions with a
tiny budget.

**Sanity property.** The sanity suite asks a different question: does
the tree stay legal and terminate when nobody is steering it? For that
you want the *least* opinionated evaluator possible —
`UniformEvaluator` — because any failure there is the tree's fault, not
the prior's. A thousand full games, with only 25 simulations per move,
exercises expansion, legality checking, terminal detection, move
selection, and the simulation loop at scale. It is a *no-panic, no-
illegality, no-hang* property, not a *play-well* property.

> **Excursion — why 1,000 games and 25 simulations per move.**
> One thousand is large enough that low-probability crashes usually
> surface, and small enough that the test finishes in seconds on a
> laptop. Twenty-five simulations per move is far below competitive
> strength, but it is enough for the tree to grow a few nodes and
> exercise the full select/expand/backup cycle on every ply. The point
> is coverage of the state space, not quality of play.

### Seeded RNG for reproducible failure

The sanity suite uses a seeded `StdRng`. This is non-negotiable:
reproducible failure is debuggable failure. If a game hangs or panics
on a random move, you must be able to rerun the exact same sequence and
attach a debugger. The tactical tests also take a seeded RNG even when
the answer is nearly forced, because `select_move` expects an RNG and a
fixed seed removes one source of flakiness.

### Integration tests vs unit tests

The acceptance suite lives in `tests/acceptance.rs`, not in a module's
`#[cfg(test)]` block. That is deliberate: `tests/` files can import only
the crate's **public** surface. If an acceptance test needs something
that is not exported in `lib.rs`, the test tells you that the public API
is incomplete. This doubles as the surface audit that milestone 3 will
rely on when it imports `mcts::{search, SearchConfig, TacticsEvaluator,
...}`.

Unit tests, by contrast, can reach `pub(crate)` items and are the right
place for internal invariants (visit counts sum correctly, PUCT scores
behave as expected, backup flips signs). The acceptance suite is the
external contract.

### The polish pass: public surface and docs

Before calling milestone 2 done, review `src/lib.rs` as if you were the
author of milestone 3 reading it cold:

* Add `#![deny(missing_docs)]` to match the engine crate's discipline.
* Add `#![warn(clippy::pedantic)]` or the clippy-allowed list from the
  reference if you want to keep the same lint posture.
* Re-export exactly what milestone 3/5 will import: `search`,
  `SearchConfig`, `SearchOutcome`, `Evaluator`, `EvalRequest`,
  `EvalResult`, `TacticsEvaluator`, `UniformEvaluator`,
  `visit_distribution`, `select_move`, `add_dirichlet_noise`, and the
  tree primitives (`Tree`, `Node`, `NodeId`, `Edge`, `NodeState`) for
  anyone who wants to inspect the tree.
* Do not re-export internal helpers like `puct_score`, `expand`, or
  `backup` unless you are willing to support them as public API.
* Run a dead-code sweep: anything `pub` that nothing imports, or any
  helper that no test exercises, should be private or removed.

### What milestone 2 explicitly did NOT build

Primer §7's last paragraph and §8's shelf list the things that are out
of scope. Be honest about them in your head and in the crate docs:

* **Resignation** — lands with the self-play crate.
* **Gating / curriculum / Elo** — evaluation infrastructure, later.
* **Transposition merging** — deliberately shelved; the tree stays a
  tree.
* **Tree parallelism / virtual loss** — game-level parallelism wins in
  our design.
* **The network implementation** — milestone 3, behind the same
  `Evaluator` trait.
* **Workers, channels, batched GPU evaluator** — milestone 5.

Saying what you did not build is as important as saying what you did,
because it protects the crate from scope creep and tells the reader
where each missing piece belongs.

### "You are here" closing

After this chapter, milestone 2 is proven. You have a single-threaded,
Burn-free, GPU-free MCTS crate that passes tactical puzzles and a
thousand-game sanity flood. Milestone 3 brings the `net` crate and the
first Burn-backed `Evaluator`. Milestone 4's anchor set already has its
oracle in the engine's TSS prover. Milestone 5 adds the self-play
workers and the batched evaluator service. The foundation you just laid
is the part every later milestone depends on, so take the time to make
it clean.

## Low-level design

### File: `tests/acceptance.rs`

Create a new integration test file. It imports only the public surface
of `mcts` and the public engine API:

```rust
use engine::reference::board_from_ascii;
use engine::{Board, Color, Move, Status};
use mcts::{
    SearchConfig, TacticsEvaluator, UniformEvaluator, search, select_move,
    visit_distribution,
};
use rand::SeedableRng;
use rand::rngs::StdRng;
```

Add a small helper:

```rust
fn best_move(board: &Board, simulations: u32, rng: &mut StdRng) -> Move {
    let mut ev = TacticsEvaluator::new();
    let cfg = SearchConfig { simulations, c_puct: 1.5 };
    let outcome = search(board, &mut ev, &cfg);
    let dist = visit_distribution(&outcome.tree, outcome.root);
    select_move(&dist, 0.0, rng).expect("root has edges")
}
```

The tactical suite:

1. `win_in_one_is_solved_at_50_sims` — ASCII puzzle, Black open four on
   row 7, assert the chosen move is in `engine::immediate_wins`.
2. `forced_block_is_taken_at_50_sims` — scripted play producing a White
   closed four that Black must block; assert the chosen move is in
   `engine::forced_blocks`.
3. `win_in_three_first_move_is_found` — reuse the engine TSS win-in-3
   script (the reference shows the exact move sequence); assert the
   chosen move is `(7, 7)`.

The sanity suite:

1. `thousand_full_games_terminate_with_legal_moves` — outer loop over
   1,000 games, inner loop plays moves until `Status` is terminal or
   225 moves reached. Each move: build `UniformEvaluator`, run `search`,
   extract `visit_distribution`, `select_move` with temperature 0.0,
   assert legality, play the move. After the loop, assert termination
   and move count ≤ 225.

### Win-in-3 puzzle reuse

The win-in-3 test reuses the move script from the engine's TSS tests.
The sequence is:

```rust
let script = [
    (7, 4), (7, 3),
    (7, 5), (0, 0),
    (7, 6), (0, 2),
    (6, 6), (0, 4),
    (8, 8), (0, 6),
];
```

After this sequence Black plays `(7, 7)`, creating a closed four; White
must block; Black follows with a double threat. The reference
implementation asserts `mv == Move::new(7, 7).unwrap()`.

### File: `src/lib.rs`

Review and lock the public surface. The reference `lib.rs` is the
target; it is quoted in the deep-dive solution.

### Dead-code sweep

Run `cargo clippy -p mcts --all-targets -- -D warnings` and address any
`dead_code` or `missing_docs` warnings. With `#![deny(missing_docs)]`
enabled, every public item needs a doc comment.

## Solution (opt-in)

A complete, compiled, tested reference lives in
[10-deep-dive/01-solution.md](10-deep-dive/01-solution.md). It quotes
`tests/acceptance.rs` and the final `src/lib.rs` verbatim from the
verified reference crate.

## TDD checklist

1. `tests/acceptance.rs` compiles and links against the public API
   only.
2. Tactical: win-in-1 at 50 simulations passes.
3. Tactical: forced block at 50 simulations passes.
4. Tactical: win-in-3 first move at 200 simulations passes.
5. Sanity: 1,000 seeded games against `UniformEvaluator` terminate with
   only legal moves.
6. Polish: `#![deny(missing_docs)]` added, all public items documented.
7. Polish: `lib.rs` re-exports reviewed and match milestone 3/5 needs.
8. Gates green: `cargo test -p mcts`, `cargo clippy -p mcts
   --all-targets -- -D warnings`, `cargo fmt --all`.

## Done when

From `gomoku/`:

```bash
cargo test -p mcts
cargo clippy -p mcts --all-targets -- -D warnings
cargo fmt --all
```

All green.

Commit: `test(mcts): acceptance suite (tactical + 1000-game sanity)`

Then, **with the user**, update `docs/WARM-UP.md` and `AGENTS.md` to
record that milestone 2 is complete. Do not edit those status sections
silently; the tutorial's convention is that status updates are a
conversation.

Milestone 2 is now proven. The `mcts` crate can search Gomoku positions,
finding wins, blocks, and short forcing sequences, and it can play a
thousand random-evaluator games without a single illegal move or hang —
all without Burn, without a GPU, and without a single thread.
