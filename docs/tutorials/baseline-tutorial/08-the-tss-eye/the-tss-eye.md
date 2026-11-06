# Sub-chapter 08 — The TSS eye

## Abstract

This sub-chapter moves the root-level threat-space search probe from
the CLI seat path into the `mcts::Evaluator` seam. A new
`Evaluator::evaluate_root` hook runs once per search, before any
simulations begin. It returns either a static `Estimate` (the ordinary
policy/value readout) or a `ProvenWin` carrying the first move of a
certified forced-win line. When the search sees `ProvenWin`, it returns
immediately; otherwise it expands the root with the single estimate and
runs the normal PUCT loop. The baseline evaluator implements the hook
with a budgeted call to `engine::prove_forced_win`, guarded by
`verify_line` and by legal-move/winner checks. `None` from the prover
still means only "not proven within budget"; the seat falls back to the
normal estimate and search.

## Glossary

| Term | Definition |
|------|------------|
| **TSS eye** | The root-level call to `engine::prove_forced_win` inside an evaluator's `evaluate_root` method. |
| **Root probe** | A single prover call at the root of one AI move, before MCTS simulations. |
| **Per-node probe** | A prover call inside the evaluator's ordinary `evaluate` call. Out of scope: the leaf path must stay the fast static estimate. |
| **Defensive probe** | A prover call to certify the opponent's forced win. Out of scope: the hook certifies wins for the side to move only; proving a forced loss changes no decision. |
| **`mcts::eval::RootEval`** | The return type of `evaluate_root`: either `Estimate(EvalResult)` or `ProvenWin { mv: Move }`. |
| **`mcts::SearchResult`** | The return type of `mcts::search`: either `Searched { tree, root }` or `ProvenWin { mv }`. |
| **`baseline::ROOT_TSS_BUDGET`** | The pinned `SearchBudget` for the baseline evaluator's root probe: `max_nodes: 10_000`, `max_depth: 30`. |
| **`BaselineEvaluator::tss_budget`** | The per-instance budget field, set by `with_tss_budget`. |
| **`engine::prove_forced_win`** | The engine's bounded threat-space search. `Some` is a certified win; `None` is only "not proven within budget". |
| **`engine::verify_line`** | The soundness net that replays a proof line and rejects doctored proofs. |
| **`engine::SearchBudget`** | The budget struct with `max_nodes` and `max_depth`. |
| **`engine::Proof`** | The certified win, visible only inside the evaluator; the generic seam exposes only the first move. |
| **VCF** | Victory by continuous forcing: a sequence of threats the defender must answer until the attacker wins. |

## Context

Sub-chapters 01 through 07 built the baseline evaluator: a policy
readout with a trust ladder from proven wins down to gravity, a
saturating value readout, and an `[experiment]` quiet-position whisper.
Those readouts answer one question per call: "what does this position
look like right now?" They are fast enough to call thousands of times
per move, and the MCTS search calls them at every leaf.

But Gomoku tactics are sequences. A VCF win is a chain of forcing moves
that the static maps cannot see as a single cell. The engine already has
a soundness-first threat-space search in `engine::prove_forced_win`. It
returns either a certified `Proof` or `None`. The `None` value is
intentionally weak: it says the budget ran out before a proof was found,
not that the position is a draw or a loss.

In the original design the probe lived in the CLI seat path as a
client-side short-circuit. This sub-chapter folds that same root-only
probe into the `Evaluator` seam so every evaluator can decide whether
to offer a proven win at the root. The probe stays root-only: it never
runs inside the per-node `evaluate` call, and there is no defensive
probe.

## Intention

The learner enables this behavior:

1. The `mcts::Evaluator` seam gains an optional `evaluate_root` hook
   with a default implementation that delegates to `evaluate` once.
2. The seam gains two enums: `RootEval` (output of the hook) and
   `SearchResult` (output of `mcts::search`).
3. `mcts::search` calls `evaluate_root` exactly once for an ongoing
   root, expands the root with that single estimate, and runs the normal
   simulation loop. Leaf expansions continue to call `evaluate` only.
4. If `evaluate_root` returns `ProvenWin`, `mcts::search` returns
   `SearchResult::ProvenWin` immediately — no searched tree is
   returned, no edges are expanded, no simulations run, and no leaf
   evaluator is called.
5. The baseline evaluator implements `evaluate_root` by reconstructing
   the board, calling `prove_forced_win` for the side to move, and
   accepting the proof only after winner, non-empty line, legal first
   move, and `verify_line` checks.
6. A failed or absent proof falls back to the same static estimate that
   `evaluate` would have returned.
7. The generic CLI step path matches `SearchResult`: a proven win plays
   directly through `execute_play`; a searched tree follows the existing
   noise, temperature, visit-count path.
8. There is no defensive probe and no per-node probe.

## Specification as tests

### `mcts` crate tests (`mcts/src/search.rs`)

* `default_evaluate_root_delegates_to_evaluate_once` — a counting
  wrapper around `UniformEvaluator` shows that the default hook calls
  `evaluate` exactly once and produces a searched tree.
* `root_estimate_is_not_evaluated_twice` — a custom evaluator with an
  explicit `evaluate_root` returning `RootEval::Estimate` is invoked
  exactly once at the root; leaves use the ordinary `evaluate` method.
* `terminal_root_does_not_call_evaluator` — a terminal root runs
  `expand` directly and never calls the evaluator.
* `proven_root_returns_immediately_without_tree_or_leaf_eval` — a
  custom evaluator returning `RootEval::ProvenWin` makes `search` return
  `SearchResult::ProvenWin` without building a tree or calling the leaf
  evaluator.
* `repeated_searches_invoke_root_hook_each_time` — running `search`
  twice on the same evaluator increments the `evaluate_root` counter to
  two; there is no first-call caching.

### `baseline` crate tests (`baseline/src/lib.rs`)

* `root_tss_budget_default_is_10k_30` — a fresh `BaselineEvaluator`
  carries `ROOT_TSS_BUDGET`.
* `with_tss_budget_overrides_default` — the builder replaces the stored
  budget.
* `evaluate_root_delegates_to_estimate_on_quiet_board` — on an empty
  board `evaluate_root` returns `RootEval::Estimate` equal to the result
  of `evaluate`.
* `evaluate_root_finds_verified_vcf_proof` — on the engine win-in-3
  script, Black to move gets `RootEval::ProvenWin { mv: (7,7) }`.
* `evaluate_root_falls_back_when_budget_is_exhausted` — the same VCF
  position with a zero budget returns `RootEval::Estimate`.
* `evaluate_does_not_run_tss` — calling `evaluate` on the VCF position
  returns a finite `EvalResult`. This is a smoke test that the ordinary
  leaf path does not claim a proven win; the dispatch is enforced by
  the root-hook-once and no-leaf-root-hook tests, not by this
  assertion alone.
* `repeated_evaluate_root_probes_each_root` — two different quiet roots
  both call `evaluate_root` successfully and fall back to estimates.

### `cli` actor tests (`cli/tests/actor.rs`)

* `baseline_ai_plays_proven_vcf_without_search_info` — from the
  win-in-3 root with Black set to `EvaluatorKind::Baseline`, one `Step`
  plays `(7,7)` and stores no `SearchInfo`.
* `baseline_ai_falls_back_to_search_info_on_quiet_position` — from an
  empty board with the same baseline config, one `Step` stores
  `SearchInfo` whose visits sum to the configured simulations.

## Walkthrough

The probe is no longer a client-side wrapper; it is a first-class
extension of the evaluator contract. The real work is reshaping the
contract so the search can receive either an estimate or a proven win.

### 1. The evaluator contract grows a root hook

The `Evaluator` trait in `mcts::eval` already has one required method:
`evaluate(&mut self, EvalRequest) -> EvalResult`. This method answers the
question "what does this position look like?" and it is called at every
leaf.

Add an optional second method, `evaluate_root`, with the same request
shape but a different return type. The default implementation calls
`evaluate` once and wraps the result in `RootEval::Estimate`. Existing
leaf-only evaluators therefore need no changes. Any decorator wrapper
around an evaluator must forward *both* trait methods; a wrapper that
only forwards `evaluate` will see the default `evaluate_root` call its
own `evaluate`, silently stripping the wrapped evaluator's root TSS
certification.

### 2. `RootEval` distinguishes estimate from proof

`RootEval` is an enum with two variants:

* `Estimate(EvalResult)` — the ordinary policy/value readout.
* `ProvenWin { mv: Move }` — a certified forced win; only the first move
  of the proof line crosses the seam.

The generic seam never exposes `engine::Proof`. The move is enough for
the search to return, and hiding the full line keeps the contract small.

### 3. `SearchResult` replaces the old tree-only outcome

`mcts::search` now returns `SearchResult`, also with two variants:

* `Searched { tree: Tree, root: NodeId }` — the normal tree after
  simulations.
* `ProvenWin { mv: Move }` — the certified first move.

Tests and CLI code that formerly destructured a tree-only outcome must
now match the enum.

### 4. `search` calls the hook exactly once at the root

For an ongoing root, `search` collects the legal moves, encodes the
board, and calls `evaluator.evaluate_root` exactly once. The result
decides the rest of the function.

If the root is already terminal, the function creates a tree, expands
the root with `expand`, and returns `Searched`. No evaluator method is
called. This keeps terminal values exact and preserves the existing
"evaluator never sees terminals" invariant.

### 5. `Estimate` expands via `expand_with_evaluation`

When `evaluate_root` returns `Estimate(result)`, the root must be
expanded with that result. A new `pub(crate)` helper,
`expand_with_evaluation(tree, leaf, board, &result)`, performs the
expansion without calling the evaluator a second time. The existing
`expand` function delegates to this helper for ongoing leaves.

The simulation loop then runs as before: select, expand leaf with
`evaluate`, backup. Only the root expansion path changes.

### 6. `ProvenWin` returns immediately

When `evaluate_root` returns `ProvenWin { mv }`, `search` returns
`SearchResult::ProvenWin { mv }`. No searched tree is returned (the tree
allocated before the hook is not expanded and is discarded), no
simulations run, and the leaf evaluator is never invoked.

### 7. The baseline evaluator implements the hook

`BaselineEvaluator` already reconstructs a `Board` from the request
planes and runs the policy/value readout. Refactor that readout into a
private `evaluate_board(&mut self, &Board) -> EvalResult` helper shared
by both `evaluate` and `evaluate_root`.

Add a `tss_budget: engine::SearchBudget` field, initialized from
`ROOT_TSS_BUDGET`, and a `with_tss_budget` builder for tests.

`evaluate_root` reconstructs the board, reads `board.to_move()`, and
calls `engine::prove_forced_win(&board, side, self.tss_budget)`. If the
prover returns `Some(proof)`, run four guards:

* `proof.winner == side` — the proof is for the player to move.
* `!proof.line.is_empty()` — the line has at least one move.
* `board.is_legal(proof.line[0])` — the first move is legal now.
* `engine::verify_line(&board, &proof)` — the line is airtight.

Only if all four guards pass does `evaluate_root` return
`RootEval::ProvenWin { mv: proof.line[0] }`. Any failure, or a `None`
from the prover, falls back to `RootEval::Estimate(self.evaluate_board(&board))`.

### 8. The CLI matches `SearchResult`

`SessionCore::execute_step` already builds an evaluator and calls
`mcts::search`. Replace the tree-only handling with a match on
`SearchResult`:

* `SearchResult::ProvenWin { mv }` — return `self.execute_play(mv)`
  directly. Skip Dirichlet noise, temperature, visit sampling, and
  `SearchInfo` storage.
* `SearchResult::Searched { tree, root }` — apply noise if configured,
  build `SearchInfo::from_searched`, sample a move, play it, and store
  the info. This path is unchanged except for destructuring the enum.

### 9. There is no defensive probe

The hook certifies wins for `board.to_move()`. Proving that the
opponent has a forced win is out of scope: the static readout does not
guarantee to see sequence losses, and confirming a forced loss changes
no decision. The value readout's opponent-double-threat rung already
paints such positions as lost. The prover is always called with
`board.to_move()`, never with the opponent's side.

### 10. There is no per-node probe

The ordinary `evaluate` method on `BaselineEvaluator` calls
`evaluate_board` directly and never touches `prove_forced_win`. The TSS
budget is consulted only inside `evaluate_root`, and `search` calls that
hook only once per search.

## TDD checklist

Work one test group at a time, red to green.

From `mcts/src/search.rs`:

1. `default_evaluate_root_delegates_to_evaluate_once`
2. `root_estimate_is_not_evaluated_twice`
3. `terminal_root_does_not_call_evaluator`
4. `proven_root_returns_immediately_without_tree_or_leaf_eval`
5. `repeated_searches_invoke_root_hook_each_time`

From `baseline/src/lib.rs`:

6. `root_tss_budget_default_is_10k_30`
7. `with_tss_budget_overrides_default`
8. `evaluate_root_delegates_to_estimate_on_quiet_board`
9. `evaluate_root_finds_verified_vcf_proof`
10. `evaluate_root_falls_back_when_budget_is_exhausted`
11. `evaluate_does_not_run_tss`
12. `repeated_evaluate_root_probes_each_root`

From `cli/tests/actor.rs`:

13. `baseline_ai_plays_proven_vcf_without_search_info`
14. `baseline_ai_falls_back_to_search_info_on_quiet_position`

## Verification commands

From `gomoku/`:

```text
cargo test -p mcts
cargo test -p baseline
cargo test -p cli --test actor
```

When the workspace is touched, also run:

```text
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Capstone

Take the defender seat and watch the baseline seat execute a forcing
line instantly:

```text
cargo run -q -p cli --bin gomoku -- play --puzzle ../data/puzzles/kaibao-vcf-676.json --index 0 --ai baseline --sims 50
```

The puzzle attacker is to move. You sit opposite the AI. Step once. The
baseline seat should play the first move of the engine-certified proof
immediately, with no visible search delay. Step a few more times and
watch the forcing line unfold.

Then try an index deeper in the same pack:

```text
cargo run -q -p cli --bin gomoku -- play --puzzle ../data/puzzles/kaibao-vcf-676.json --index 12 --ai baseline --sims 50
```

Some positions solve in one move; some need a short sequence. If the
probe returns `None`, the seat falls back to MCTS and the move takes
longer. That difference is the TSS eye opening and closing.

## Done when

* `cargo test -p mcts` passes.
* `cargo test -p baseline` passes.
* `cargo test -p cli --test actor` passes.
* `cargo clippy --all-targets -- -D warnings` passes.
* `cargo fmt --all -- --check` makes no changes.
* All new public items are documented and the crate passes
  `#![deny(missing_docs)]`.
* The commit is `feat(baseline): the TSS eye — baseline sub-chapter 08`.

## References

* [Baseline primer](../00-baseline-primer.md) — §7 pins the root-only
  probe design and the meaning of `None`. (Note: this chapter
  implements the probe inside the `Evaluator` seam rather than as a
  client-side wrapper; the root-only policy and the interpretation of
  `None` remain unchanged.)
* [Sub-chapter 07 — The quiet whisper](../07-the-quiet-whisper/the-quiet-whisper.md)
  — the readout layer the probe sits on top of.
* [`gomoku/crates/mcts/src/eval.rs`](../../../../gomoku/crates/mcts/src/eval.rs)
  — the `Evaluator` seam, `RootEval`, and the default hook.
* [`gomoku/crates/mcts/src/search.rs`](../../../../gomoku/crates/mcts/src/search.rs)
  — `SearchResult` and the root-level call to `evaluate_root`.
* [`gomoku/crates/mcts/src/expand.rs`](../../../../gomoku/crates/mcts/src/expand.rs)
  — the `expand_with_evaluation` helper.
* [`gomoku/crates/baseline/src/lib.rs`](../../../../gomoku/crates/baseline/src/lib.rs)
  — `BaselineEvaluator::evaluate_root`, `ROOT_TSS_BUDGET`, and the
  verification guards.
* [`gomoku/crates/cli/src/session.rs`](../../../../gomoku/crates/cli/src/session.rs)
  — the generic match on `SearchResult` in the AI-seat step path.
* [`gomoku/crates/engine/src/tss.rs`](../../../../gomoku/crates/engine/src/tss.rs)
  — `prove_forced_win`, `SearchBudget`, `Proof`, `verify_line`, and the
  documented `None` warning.
