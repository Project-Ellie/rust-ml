# Sub-chapter 08 — The TSS eye: implementation plan

This document turns the walkthrough in `the-tss-eye.md` into a
construction order. It names the files, structs, enums, constants,
functions, and tests. It states behavioral contracts in prose. It does
not contain bodies.

Read `the-tss-eye.md` first. Work one step at a time. Write tests
before code. Open `the-tss-eye-solution.md` only if you are stuck for
more than twenty minutes, or after the sub-chapter is complete.

---

## Step 0 — Extend the `mcts::eval` seam

Open `gomoku/crates/mcts/src/eval.rs`.

### 0.1 Add `RootEval`

Add a public enum `RootEval`.

Behavior:

* Variant `Estimate(EvalResult)` carries the ordinary policy/value
  readout.
* Variant `ProvenWin { mv: engine::Move }` carries only the first move
  of a certified forced win.
* Derive `Debug`, `Clone`, `PartialEq`.
* The enum is large because it contains a `[f32; 225]` policy array;
  allow `clippy::large_enum_variant` with a comment explaining the
  trade-off.

### 0.2 Add `Evaluator::evaluate_root`

Add a public method to the `Evaluator` trait.

Signature prose: `fn evaluate_root(&mut self, req: EvalRequest) -> RootEval`.

Behavior:

* The default implementation calls `self.evaluate(req)` exactly once and
  wraps the result in `RootEval::Estimate`.
* Implementations may override it to run a soundness-guarded forced-win
  probe at the root.
* Existing leaf-only evaluators must compile without changes through
  the default hook.
* Decorator wrappers must forward *both* `evaluate` and
  `evaluate_root`; forwarding only `evaluate` silently strips the
  wrapped evaluator's root TSS certification.

### 0.3 Re-export `RootEval`

Open `gomoku/crates/mcts/src/lib.rs`. Add `RootEval` to the re-export
list so downstream crates can pattern-match on it.

### 0.4 Gate

Run `cargo test -p mcts`. The new test should fail until the enum and
hook are present.

---

## Step 1 — Add `expand_with_evaluation`

Open `gomoku/crates/mcts/src/expand.rs`.

### 1.1 Extract the shared expansion body

The ongoing-branch body of `expand` currently encodes the board, calls
`evaluator.evaluate`, and builds edges from the result. Extract this
into a new `pub(crate)` function `expand_with_evaluation`.

Signature prose:
`fn expand_with_evaluation(tree: &mut Tree, leaf: NodeId, board: &Board, result: &EvalResult) -> f32`.

Behavior:

* The caller guarantees `board.status() == Status::Ongoing`.
* Compute legal moves, run `masked_softmax`, push edges with priors,
  call `tree.add_child` for each edge, and set the node state to
  `Expanded`.
* Return `result.value` clamped to `[-1.0, 1.0]`.
* Do not call any evaluator.

### 1.2 Make `expand` delegate for ongoing leaves

In `expand`, for the `Status::Ongoing` branch, after producing the
`EvalResult`, call `expand_with_evaluation` instead of inlining the
edge-building code.

### 1.3 Gate

Run `cargo test -p mcts`. Existing expansion tests must still pass.

---

## Step 2 — Add `SearchResult` and wire `search`

Open `gomoku/crates/mcts/src/search.rs`.

### 2.1 Add `SearchResult`

Add a public enum `SearchResult`.

Behavior:

* Variant `Searched { tree: Tree, root: NodeId }` carries the tree and
  root after a normal search.
* Variant `ProvenWin { mv: engine::Move }` carries the first move of a
  certified forced win.
* Derive `Debug`, `Clone`, `PartialEq`.

### 2.2 Change `search` to return `SearchResult`

Behavior:

* Create a new tree and root node as before.
* If the root board is terminal, expand the root with `expand` and
  return `SearchResult::Searched { tree, root }`. Do not call any
  evaluator method.
* For an ongoing root, collect legal moves, encode the board, and call
  `evaluator.evaluate_root` exactly once.
* If the result is `RootEval::ProvenWin { mv }`, return
  `SearchResult::ProvenWin { mv }`.
* If the result is `RootEval::Estimate(result)`, call
  `expand_with_evaluation(&mut tree, root, root_board, &result)`, then
  run the simulation loop. Leaf expansions inside the loop continue to
  call `expand`, which uses `evaluate`.
* Return `SearchResult::Searched { tree, root }` after the loop.

### 2.3 Re-export `SearchResult`

Open `gomoku/crates/mcts/src/lib.rs`. Add `SearchResult` to the
re-export list.

### 2.4 Update existing `search` tests

The three existing tests in `search.rs` (`uniform_search_visits_sum_to_simulations`,
`immediate_win_for_side_to_move_gets_most_visits`,
`opponent_immediate_win_forces_block`) currently destructure a tree-only
return value. Change each to match on `SearchResult::Searched { tree, root }`
and panic on any other variant.

Tests that go red then green:

* `default_evaluate_root_delegates_to_evaluate_once`
* `root_estimate_is_not_evaluated_twice`
* `terminal_root_does_not_call_evaluator`
* `proven_root_returns_immediately_without_tree_or_leaf_eval`
* `repeated_searches_invoke_root_hook_each_time`

### 2.5 Update the `mcts` acceptance test

Open `gomoku/crates/mcts/tests/acceptance.rs`. The `best_move` helper
and the `thousand_full_games_terminate_with_legal_moves` test currently
assume a tree-only return. Update both to match on `SearchResult`:
`ProvenWin` returns the move directly; `Searched` extracts the tree and
root for visit distribution.

### 2.6 Gate

Run `cargo test -p mcts` until all unit and acceptance tests pass.

---

## Step 3 — Implement the baseline root hook

Open `gomoku/crates/baseline/src/lib.rs`.

### 3.1 Add `ROOT_TSS_BUDGET`

Add a public constant:

```rust
pub const ROOT_TSS_BUDGET: engine::SearchBudget
```

Behavior:

* `max_nodes` is `10_000`.
* `max_depth` is `30`.
* The doc comment marks the values as `[experiment]` starting values.

### 3.2 Add `tss_budget` field and builder

In `BaselineEvaluator`, add a private field `tss_budget:
engine::SearchBudget`.

Behavior:

* Initialized to `ROOT_TSS_BUDGET` in `BaselineEvaluator::new`.
* Add a public chainable builder `with_tss_budget(mut self, budget:
  engine::SearchBudget) -> Self` that overwrites the field.

### 3.3 Extract `evaluate_board`

Move the body of `Evaluator::evaluate` into a new private method
`fn evaluate_board(&mut self, board: &engine::Board) -> EvalResult`.

Behavior:

* Runs `patterns::net::analyze_both`, picks own/opponent maps by
  `board.to_move()`, computes gravity, builds policy logits with
  `readout::policy_logits`, computes value with `readout::value`,
  optionally applies the quiet whisper, and returns `EvalResult`.
* Shared by `evaluate` and `evaluate_root` so the board is not
  reconstructed twice when the root falls back to a static estimate.

### 3.4 Update `Evaluator::evaluate`

Behavior:

* Reconstruct the board from `req.planes` and `req.legal`.
* Return `self.evaluate_board(&board)`.
* Do not call `prove_forced_win`.

### 3.5 Implement `Evaluator::evaluate_root`

Behavior:

* Reconstruct the board from `req.planes` and `req.legal`.
* Let `side = board.to_move()`.
* Call `engine::prove_forced_win(&board, side, self.tss_budget)`.
* If the call returns `Some(proof)`, compute a boolean `verified` that
  is true only when all of the following hold:
  * `proof.winner == side`,
  * `!proof.line.is_empty()`,
  * `board.is_legal(proof.line[0])`,
  * `engine::verify_line(&board, &proof)`.
* If `verified` is true, return `RootEval::ProvenWin { mv: proof.line[0] }`.
* Otherwise return `RootEval::Estimate(self.evaluate_board(&board))`.
* If the prover returns `None`, also return
  `RootEval::Estimate(self.evaluate_board(&board))`.

### 3.6 Gate

Run `cargo test -p baseline`. The new baseline tests should fail until
step 3 is complete.

Tests that go red then green:

* `root_tss_budget_default_is_10k_30`
* `with_tss_budget_overrides_default`
* `evaluate_root_delegates_to_estimate_on_quiet_board`
* `evaluate_root_finds_verified_vcf_proof`
* `evaluate_root_falls_back_when_budget_is_exhausted`
* `evaluate_does_not_run_tss`
* `repeated_evaluate_root_probes_each_root`

---

## Step 4 — Match `SearchResult` in the CLI

Open `gomoku/crates/cli/src/session.rs`.

### 4.1 Locate `execute_step`

Find the call to `mcts::search` and the code that currently assumes a
tree-only return.

### 4.2 Replace with a match on `SearchResult`

Behavior:

* After `let outcome = mcts::search(...)`, match on `outcome`.
* `mcts::SearchResult::ProvenWin { mv }` — return
  `self.execute_play(mv)` directly. Do not build `SearchInfo`, do not
  apply Dirichlet noise, do not sample with temperature.
* `mcts::SearchResult::Searched { mut tree, root }` — apply Dirichlet
  noise if configured, build `SearchInfo::from_searched(&tree, root,
  &board, &effective_config)`, sample a move with
  `mcts::select_move`, and continue to the play/storage logic.

This path is generic: it works for any evaluator that returns either
variant, not only `BaselineEvaluator`.

### 4.3 Gate

Run `cargo test -p cli --test actor`. Existing actor tests must still
pass; the new tests are added in step 5.

---

## Step 5 — Add the CLI actor tests

Open `gomoku/crates/cli/tests/actor.rs`.

### 5.1 Add `baseline_ai_plays_proven_vcf_without_search_info`

Behavior:

* Build the engine TSS win-in-3 board by replaying the script.
* Assert the side to move is Black.
* Create a `SessionCore` from a `SessionInit` with:
  * Black seat as AI with `EvaluatorKind::Baseline`, low simulations,
    temperature 0.0, noise 0.0, and `explicit_selection: true`.
  * White seat as Human.
  * The win-in-3 opening board.
  * A seed for reproducibility.
* Execute `Command::Step`.
* Assert the emitted events contain a move to `(7,7)` played by the AI.
* Assert `core.search_info()` is `None`, because a proven win bypasses
  search info storage.

### 5.2 Add `baseline_ai_falls_back_to_search_info_on_quiet_position`

Behavior:

* Create a `SessionCore` from an empty board with Black as baseline AI
  and White as Human.
* Execute `Command::Step`.
* Assert the AI moved.
* Assert `core.search_info()` is `Some` and that the stored info's
  visits sum to the configured simulation count.

### 5.3 Gate

Run `cargo test -p cli --test actor` until both tests pass.

---

## Step 6 — Project gates

From `gomoku/`:

```text
cargo test -p mcts
cargo test -p baseline
cargo test -p cli --test actor
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

---

## Migration inventory

The following existing artifacts change shape because of the new
`SearchResult` enum and the root hook. Update each one as described;
no leaf evaluator implementation needs to change beyond recompiling.

### `mcts` crate

| File | What changes |
|------|--------------|
| `crates/mcts/src/eval.rs` | New `RootEval` enum; new `Evaluator::evaluate_root` method with default body. |
| `crates/mcts/src/lib.rs` | Re-export `RootEval` and `SearchResult`. |
| `crates/mcts/src/expand.rs` | New `pub(crate)` `expand_with_evaluation`; `expand` delegates to it for ongoing leaves. |
| `crates/mcts/src/search.rs` | New `SearchResult` enum; `search` returns it; calls `evaluate_root` once at ongoing root; terminal roots skip the evaluator. |
| `crates/mcts/src/search.rs` tests | Three existing tests now match `SearchResult::Searched`; five new tests added. |
| `crates/mcts/tests/acceptance.rs` | `best_move` helper and the thousand-game flood now match `SearchResult::ProvenWin` or `SearchResult::Searched`. |

### `baseline` crate

| File | What changes |
|------|--------------|
| `crates/baseline/src/lib.rs` | New `ROOT_TSS_BUDGET`; `BaselineEvaluator` gains `tss_budget` field and `with_tss_budget` builder; private `evaluate_board` extracted; `evaluate_root` implemented with verification guards. |
| `crates/baseline/src/lib.rs` tests | Seven new tests added; existing tests are unchanged. |

### `cli` crate

| File | What changes |
|------|--------------|
| `crates/cli/src/session.rs` | `execute_step` matches `SearchResult`; `ProvenWin` returns `execute_play` directly; `Searched` follows the existing noise/temperature/info path. |
| `crates/cli/tests/actor.rs` | Two new tests added; existing tests that do not touch search results are unchanged. |

### Leaf evaluator implementors

`UniformEvaluator` and `TacticsEvaluator` keep their existing
`evaluate` implementations. They receive the new `evaluate_root`
behavior for free through the default hook, so no source changes are
required. Any decorator wrapper around an evaluator must forward both
`evaluate` and `evaluate_root`; otherwise the default `evaluate_root`
will call the wrapper's `evaluate` and silently disable the wrapped
evaluator's root TSS certification.

---

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `mcts/src/eval.rs` | `RootEval` | `Estimate(EvalResult)` / `ProvenWin { mv }` |
| `mcts/src/eval.rs` | `evaluate_root` default | calls `evaluate` exactly once, returns `Estimate` |
| `mcts/src/search.rs` | `SearchResult` | `Searched { tree, root }` / `ProvenWin { mv }` |
| `mcts/src/search.rs` | terminal root | no evaluator call, returns `Searched` |
| `mcts/src/search.rs` | ongoing root | calls `evaluate_root` exactly once |
| `mcts/src/search.rs` | `ProvenWin` path | returns immediately, no searched tree returned, no leaf eval |
| `mcts/src/expand.rs` | `expand_with_evaluation` | builds edges from an existing `EvalResult` without re-evaluating |
| `baseline/src/lib.rs` | `ROOT_TSS_BUDGET` | `max_nodes: 10_000`, `max_depth: 30`, `[experiment]` |
| `baseline/src/lib.rs` | `tss_budget` field / builder | defaults to `ROOT_TSS_BUDGET`, overridable |
| `baseline/src/lib.rs` | `evaluate_root` | reconstructs board, proves for `to_move()`, four guards, falls back to estimate |
| `baseline/src/lib.rs` | `evaluate` | no TSS, returns `evaluate_board` |
| `cli/src/session.rs` | `ProvenWin` branch | `execute_play` directly, no search info |
| `cli/src/session.rs` | `Searched` branch | existing noise/temp/info path |
| `cli/tests/actor.rs` | proven VCF test | AI plays `(7,7)`, no search info stored |
| `cli/tests/actor.rs` | quiet fallback test | AI moves, search info stored with matching visits |
