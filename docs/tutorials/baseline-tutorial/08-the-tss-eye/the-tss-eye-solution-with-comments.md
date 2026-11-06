# Annotated solution: the TSS eye (chapter 08)

This file is a true annotated copy of `the-tss-eye-solution.md` in the same directory. Every original paragraph and every original numbered source line is preserved byte-for-exact-byte and in order. Text in **ANNOTATION** and **COMMENT** blocks is added commentary; all other text is the original solution. Long numbered listings are split into small chunks so each **COMMENT** block sits next to the lines it explains; splitting a fence never changes the numbered lines inside it, only the fence delimiters.

---

## Quick execution route (runtime order)

What actually happens when the AI makes a move, in the order it happens. This is a call chain, not a list of implementation tasks.

1. `cli::SessionCore::execute_step` refuses to run while the Swap2 opening is unfinished or the game is over, resolves the seat's `AiConfig` (an active opening schedule forces temperature 1.0 and noise 0.25), builds the evaluator, and calls `mcts::search`.
2. `mcts::search` allocates a tree holding one root node, then checks the terminal guard: a non-ongoing root is marked terminal and returned as `SearchResult::Searched` without any evaluator call.
3. An ongoing root calls `evaluator.evaluate_root(...)` exactly once.
4. Inside `BaselineEvaluator::evaluate_root`: reconstruct the board from the request, run `engine::prove_forced_win` for the side to move, guard any returned proof (winner matches, line non-empty, first move legal, `engine::verify_line` passes), and return either `RootEval::ProvenWin { mv }` or a static `evaluate_board` estimate.
5. Back in `search`, the match routes the result: `RootEval::ProvenWin` skips expansion and the simulation loop and returns `SearchResult::ProvenWin { mv }`; `RootEval::Estimate` expands the root with the already-produced `EvalResult` via `expand_with_evaluation`, runs the PUCT simulation loop, and returns `SearchResult::Searched { tree, root }`.
6. Back in `execute_step`: the proven arm plays the move directly through `self.execute_play(mv)` and returns. The searched arm applies Dirichlet noise, builds `SearchInfo`, samples a move from the visit counts, then calls `self.history.play(mv)?`, stores the info at the new history cursor, and emits the move (and any game-over) events.

## Behavior to observe

- A quiet root position still runs MCTS and stores `SearchInfo`.
- A root VCF position returns the forcing first move without root expansion, simulations, noise, temperature sampling, or `SearchInfo` storage. `search` still allocates its one-root-node tree up front; on the proven path that tree is dropped unexpanded and is not part of the returned value.
- A terminal root never calls the evaluator at all.
- The default `evaluate_root` calls `evaluate` exactly once and wraps the result.
- Wrappers that override `evaluate` but forget to forward `evaluate_root` silently lose the TSS bypass, because the default hook just calls their `evaluate`.

---

> **Opt-in solution — read only after you have tried the chapter, or if you have been stuck for more than twenty minutes.**
>
> This solution quotes a verified reference build that is preserved as
> an uncommitted detached worktree beside the main repository:
> `/Users/wgiersche/workspace/Project-Ellie/rust-ml-tss-root-doc-ref`.
> The learner's implementation on `main` may use different names or
> helper locations, but it must satisfy the same behavioral contracts.
> Line numbers are from the reference worktree, not from the learner's
> `main`.

---

**ANNOTATION.** The reference worktree is disposable. Wolfie's own `main` is the reference for behavior. The line numbers in this document come from `rust-ml-tss-root-doc-ref`.

---

Verified green from `rust-ml-tss-root-doc-ref/gomoku/`:

```text
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

All three commands exited with status 0. The reference is not a
committed branch; it is a disposable worktree kept for inspection.

---

The slice reshapes three crates:

* `mcts` — new `RootEval` and `SearchResult` enums, a default root hook,
  and a single root evaluation path.
* `baseline` — `BaselineEvaluator` implements the root hook with a
  budgeted, guarded TSS probe.
* `cli` — the AI step matches `SearchResult` generically.

The old `baseline::probe` module and the CLI-side feature-gated
short-circuit are gone; the root probe now lives inside the evaluator
contract.

---

**ANNOTATION — why two enums at the seam.**

- Both enums are public API of the `mcts` crate (`pub enum`, re-exported from `lib.rs`). "Vocabulary" here describes which type each side of the seam produces and consumes, not visibility.
- The two results carry different payloads. `RootEval::Estimate` wraps an `EvalResult` (a 225-float policy array plus a value); `SearchResult::Searched` wraps a whole `Tree` plus the root `NodeId`. A `RootEval` cannot be returned from `search` unchanged: an `Estimate` must first be transformed into a built tree, and that transformation is the simulation loop.
- Two enums are a chosen encoding, not a forced one. A single enum with more variants, or a struct with an `Option<Tree>` field, could express the same outcomes. The chosen split keeps the producer's answer (what the evaluator knows: estimate or certified win) separate from the consumer's answer (what `search` hands back: a tree to sample from, or a move to play), so each signature says exactly what it can produce.
- A static estimate cannot express the bypass. If a proven position came back as `EvalResult { value: 1.0, .. }`, `search` would still expand the root, run every simulation, and leave the final choice to the visit counts; nothing in `EvalResult` means "skip the search". Visit counts from a finite simulation budget are only an estimate of move quality, and with `temperature > 0` the CLI samples instead of taking argmax, so a certified line is not guaranteed to be picked. `RootEval::ProvenWin` carries the extra bit of meaning that `value: 1.0` cannot: the move is certified, do not search.
- MCTS itself is not inherently stochastic: backup is deterministic arithmetic, and with `temperature: 0.0` and no noise the final selection is deterministic argmax over visits. The variant exists to bypass budget-limited search, not to fight randomness.
- Why a root hook instead of probing inside `evaluate`: `evaluate_root` runs once per search. `evaluate` runs at every expanded leaf, so a TSS probe there would run up to one bounded proof search per simulation — fewer in practice, because some simulations hit terminal or already-expanded nodes — which is potentially hundreds of proof searches per move instead of one.

---

## The evaluator seam gains a root hook

`gomoku/crates/mcts/src/eval.rs`, lines 30–61, introduces the return
enum and the optional hook. The default implementation is the only
behavior `UniformEvaluator` and `TacticsEvaluator` need.

```text
 30 | /// Root evaluation result: either a static estimate or a proven win.
 31 | // EvalResult carries a 225-element policy array; boxing would change the
 32 | // public enum shape and is not worth the indirection for a value that is
 33 | // passed once per search.
 34 | #[allow(clippy::large_enum_variant)]
 35 | #[derive(Debug, Clone, PartialEq)]
 36 | pub enum RootEval {
 37 |     /// Static policy/value estimate for the root.
 38 |     Estimate(EvalResult),
 39 |     /// A certified forced win; the first move of the proof line.
 40 |     ProvenWin {
 41 |         /// The move that begins the forced-win sequence.
 42 |         mv: engine::Move,
 43 |     },
 44 | }
```

**COMMENT — the `RootEval` enum (lines 30–44).**

- Why this exists: the evaluator's root probe needs two answers, "here is a static estimate" and "here is a certified winning move". The two-enum annotation above covers why a sentinel `value: 1.0` inside `EvalResult` cannot carry the second answer.
- Why here: `RootEval` is the return type of an `Evaluator` trait method, so it lives in `eval.rs` next to `EvalRequest` and `EvalResult`; `search.rs` imports it from there.
- Rust mechanics: `Estimate(EvalResult)` is a tuple variant holding the full policy array; `ProvenWin { mv }` is a struct variant holding one `engine::Move`. The proof line itself is verified inside the evaluator and then dropped — `search` never sees the line, only the first move to play.
- The derives match `EvalResult`: `Debug` and `Clone` are ergonomics (nothing clones implicitly), and `PartialEq` enables `assert_eq!` in tests. `Eq` is impossible here because `EvalResult` contains `[f32; 225]`, and `f32` has no `Eq` implementation: NaN is not equal to itself, which breaks the reflexivity that `Eq` requires.
- Optional design, not a necessity: `#[allow(clippy::large_enum_variant)]` silences a clippy style lint about the size gap between the roughly 900-byte inline `Estimate` payload and the small `ProvenWin` payload. Boxing the array would shrink the enum at the cost of a heap allocation and a changed public shape. The inline array was kept as a tradeoff; its runtime impact was not measured.

```text
 45 |
 46 | /// the contract with any evaluator implementation
 47 | pub trait Evaluator {
 48 |     /// return policy and value for current board position
 49 |     fn evaluate(&mut self, req: EvalRequest) -> EvalResult;
```

**COMMENT — the pre-existing contract (lines 45–49).** This part is unchanged from before the slice and is shown so the new method below sits in context. `&mut self` is an exclusive borrow, which permits implementations to update internal state (caches, backends) during a call. `req: EvalRequest` passes the request by value; the callee may consume it. The new `evaluate_root` below copies both choices for consistency with the existing method.

```text
 50 |
 51 |     /// Optional root-level probe.
 52 |     ///
 53 |     /// The default implementation delegates to [`Self::evaluate`] and
 54 |     /// wraps the result in [`RootEval::Estimate`]. Implementations may
 55 |     /// override this to run a soundness-guarded forced-win probe at the
 56 |     /// root; when they return [`RootEval::ProvenWin`], the search returns
 57 |     /// immediately without expanding the tree or running simulations.
 58 |     fn evaluate_root(&mut self, req: EvalRequest) -> RootEval {
 59 |         RootEval::Estimate(self.evaluate(req))
 60 |     }
 61 | }
```

**COMMENT — the default hook (lines 50–61).**

- Why a default body: "no special root handling" means "treat the root like any leaf", so the default delegates to `evaluate` and wraps the result. Existing evaluators (`UniformEvaluator`, `TacticsEvaluator`, and any downstream implementations) keep compiling unchanged — adding a defaulted trait method is backwards-compatible for implementors.
- The slice is not backwards-compatible for callers of `search`: the return type changes from `SearchOutcome` to `SearchResult`, so every call site that matched on the old type must be updated (the CLI section and the Notes cover this).
- Rust mechanics — object safety: `search` takes `evaluator: &mut dyn Evaluator`, so the trait must stay dyn-compatible. The rules that matter here: no generic type parameters on methods, and no by-value `Self` in a signature. Both hold. Lifetimes would not break object safety — a hypothetical `&EvalRequest` parameter would be just as dyn-compatible; the owned parameter simply matches the existing `evaluate` signature and avoids threading a borrow through the API. The return type `RootEval` is a concrete `Sized` type, which `dyn` also requires.
- Behavior pinned by tests: the default calls `evaluate` exactly once per search (first `mcts` test below).

`lib.rs` re-exports `RootEval` alongside the other public names so
`BaselineEvaluator` can implement the hook and tests can pattern-match
on it.

**ANNOTATION — re-export.** `crates/mcts/src/lib.rs` now contains:

```text
pub use eval::{EvalRequest, EvalResult, Evaluator, RootEval, UniformEvaluator, masked_softmax};
```

`RootEval` is added to the existing `eval` re-export list. The module `eval` is already `pub mod eval`, so callers could also reach it as `mcts::eval::RootEval`. The re-export is for ergonomics; both paths name the same type.


## `search` calls the hook once and returns `SearchResult`

`gomoku/crates/mcts/src/search.rs`, lines 36–98, shows the new
`SearchResult` enum and the `search` body. A terminal root is resolved
without calling any evaluator. An ongoing root calls `evaluate_root`
exactly once. `RootEval::Estimate` expands through
`expand_with_evaluation`; `RootEval::ProvenWin` returns immediately.

```text
 36 | /// The result of a single search.
 37 | #[derive(Debug, Clone, PartialEq)]
 38 | pub enum SearchResult {
 39 |     /// A completed search with its tree and root node.
 40 |     Searched {
 41 |         /// The whole tree after the search.
 42 |         tree: Tree,
 43 |         /// The starting node.
 44 |         root: NodeId,
 45 |     },
 46 |     /// A certified forced win found at the root; play this move.
 47 |     ProvenWin {
 48 |         /// The move that begins the forced-win sequence.
 49 |         mv: engine::Move,
 50 |     },
 51 | }
```

**COMMENT — `SearchResult` (lines 36–51).**

- Why this exists: `search` now hands back two genuinely different things — a built tree the caller can read visit counts and priors from, or one certified move with nothing to sample. The two-enum annotation above covers why this is a separate enum from `RootEval` rather than the same one reused.
- Why here: it is the return type of `search`, so it lives in `search.rs` next to the function.
- Rust mechanics: `Searched` owns the `Tree`; returning moves ownership to the caller. `#[derive(Clone)]` only makes the value cloneable when someone asks — no implicit clone happens on return. `PartialEq` is derived for tests; `Eq` is impossible because `Tree` contains `f32` fields (`Edge::prior`, `Edge::w`), and floats have no `Eq` (NaN is not equal to itself). `PartialEq` is all `assert_eq!` needs.

```text
 52 |
 53 | /// Run MCTS from `root_board` using `evaluator` and `config`.
 54 | ///
 55 | /// Returns either a full tree so callers can extract visit counts,
 56 | /// priors, or reuse the subtree later, or a proven win when the
 57 | /// evaluator's root probe finds one.
 58 | pub fn search(
 59 |     root_board: &Board,
 60 |     evaluator: &mut dyn Evaluator,
 61 |     config: &SearchConfig,
 62 | ) -> SearchResult {
 63 |     // Terminal roots are resolved exactly without calling any evaluator.
 64 |     let mut tree = Tree::new();
 65 |     let root = tree.root();
 66 |     if root_board.status() != Status::Ongoing {
 67 |         let _ = expand(&mut tree, root, root_board, evaluator);
 68 |         return SearchResult::Searched { tree, root };
 69 |     }
```

**COMMENT — signature and terminal guard (lines 52–69).**

- Why: a finished game has an exact outcome, so calling a network or a prover would be wasted work and could mask bugs.
- Why here, before the hook: the guard guarantees `evaluate_root` is only ever called on ongoing positions, so the baseline probe can assume a playable board.
- Rust mechanics: `Tree::new()` allocates the arena vector with exactly one root node. On the terminal path that tree (root marked terminal, no edges) is returned. On the proven path below, the same one-node tree is dropped unused when the `ProvenWin` value is returned. So the proven path skips expansion, simulations, and a returned tree — it does not skip the initial root-node allocation.
- The terminal path calls `expand`, which marks the node terminal without touching the evaluator, and returns the tree in the same `Searched` shape the caller already handles.

```text
 70 |
 71 |     let legal: Vec<_> = root_board.empty_moves().collect();
 72 |     let planes = engine::encode(root_board);
 73 |     let root_eval = evaluator.evaluate_root(EvalRequest {
 74 |         planes,
 75 |         legal: legal.clone(),
 76 |     });
```

**COMMENT — building the root request (lines 70–76).**

- Why: the evaluator seam speaks only planes plus legal moves — `EvalRequest` carries no `Board` — so `search` encodes the root board here.
- This encode does not duplicate the encodes inside `expand`: those run on different boards (leaf positions reached during simulations), not on the root. Before this slice, the root's encode and evaluation happened inside the first `expand` call; now `evaluate_root` takes over that job and the encode above feeds it. The genuinely repeated encode is elsewhere: `BaselineEvaluator::evaluate_board` re-encodes the same root board again (see the baseline section).
- Rust mechanics: `Vec<_>` lets the compiler infer the element type (`engine::Move`) from the iterator. `legal.clone()` copies the move list into the request; moving `legal` directly would also compile, because the local is never used again. The clone is a small optional inefficiency, not a defect.

```text
 77 |
 78 |     match root_eval {
 79 |         RootEval::ProvenWin { mv } => SearchResult::ProvenWin { mv },
 80 |         RootEval::Estimate(result) => {
 81 |             let _ = expand_with_evaluation(&mut tree, root, root_board, &result);
 82 |
```

**COMMENT — the routing match (lines 77–82).**

- Why `expand_with_evaluation` here: the root estimate was already computed inside `evaluate_root`. Expanding through the old `expand` would call `evaluate` a second time for the same position. The helper consumes the existing `EvalResult` and never re-evaluates.
- Rust mechanics: `match root_eval` moves the enum; the `Estimate(result)` binding owns the inner value, and `&result` passes a borrow of it to the helper. Binding a borrow directly would require changing the scrutinee or the pattern — `match &root_eval { .. }` or `RootEval::Estimate(ref result)` — not writing `(&result)` in pattern position. The by-value binding is fine because `result` is used nowhere else.
- In the proven arm, `mv` is copied out of the moved enum; `engine::Move` is `Copy`.
- The match is exhaustive over two variants. Adding a third `RootEval` variant later would fail compilation at every `match` — a deliberate property, because it forces every call site to confront the new case.

```text
 83 |             for _ in 0..config.simulations {
 84 |                 let selection = select(&tree, root_board, config.c_puct);
 85 |                 let leaf_value = match tree.node(selection.leaf).state() {
 86 |                     NodeState::Terminal => terminal_value(&selection.board),
 87 |                     NodeState::Unexpanded => {
 88 |                         expand(&mut tree, selection.leaf, &selection.board, evaluator)
 89 |                     }
 90 |                     NodeState::Expanded => 0.0,
 91 |                 };
 92 |                 backup(&mut tree, &selection.path, leaf_value);
 93 |             }
 94 |
 95 |             SearchResult::Searched { tree, root }
 96 |         }
 97 |     }
 98 | }
```

**COMMENT — the unchanged simulation loop (lines 83–98).**

- This loop is the pre-slice MCTS body, unchanged: select a leaf, evaluate or score it, back the value up. The only structural difference from before sits above the loop: the root is pre-expanded by `expand_with_evaluation` instead of by a bare `expand` call.
- Root pre-expansion keeps the ongoing-path invariant that the root edge visit counts sum to `config.simulations`: every simulation descends through exactly one root edge. (A terminal root has no edges and runs no simulations; the invariant statement concerns the ongoing path.)
- Line 88 is the ordinary leaf evaluation site: leaves still go through `expand` → `evaluate`. The TSS probe runs once at the root, not per leaf; the two-enum annotation above shows what per-leaf probing would cost.
- Both match arms end in a `SearchResult` value with no trailing semicolon, so the `match` expression is the function's return value.
- Trust boundary: `search` routes whatever the evaluator certified; it does not re-verify the proof. Downstream, the CLI still plays the move through the legality-checked `history.play`, so an illegal move becomes an error rather than a played stone. A legal but unsound move would be played unchallenged — soundness lives entirely in the evaluator's `verify_line` guard.


## `expand_with_evaluation` prevents a second root evaluation

`gomoku/crates/mcts/src/expand.rs`, lines 54–92, is the helper that
builds edges from an already-produced `EvalResult`. It is used both by
the root path in `search` and by the ordinary leaf path in `expand`.

```text
 54 | /// Expand an ongoing leaf using an already-produced evaluation.
 55 | ///
 56 | /// This is the shared expansion body used by [`expand`] and by the
 57 | /// root-path optimisation in [`search`](crate::search), so the root's
 58 | /// single [`Evaluator::evaluate_root`] call is not followed by a second
 59 | /// network evaluation.
 60 | pub(crate) fn expand_with_evaluation(
 61 |     tree: &mut Tree,
 62 |     leaf: NodeId,
 63 |     board: &Board,
 64 |     result: &EvalResult,
 65 | ) -> f32 {
```

**COMMENT — helper signature (lines 54–65).**

- Why this exists: both call sites — the root path in `search` and the leaf path in `expand` — must build edges from an `EvalResult` in exactly the same way. Extracting the shared body makes the root optimisation possible without a second algorithm to keep in sync. The algorithm inside is the pre-existing expansion logic, moved — not a new one.
- Rust mechanics: `&mut Tree` grants permission to mutate the arena. `board: &Board` is read-only, so the caller keeps ownership and no board clone is needed. `result: &EvalResult` is borrowed; a by-value parameter would avoid a clone just as well, because the current callers do nothing with `result` afterwards — the borrow is a convenience, not a necessity. `leaf: NodeId` is a `Copy` index into the arena vector, not a reference, so it never conflicts with the `&mut Tree` borrow.
- `pub(crate)`: shared inside the `mcts` crate only; not part of the public API.

```text
 66 |     debug_assert_eq!(
 67 |         board.status(),
 68 |         Status::Ongoing,
 69 |         "expand_with_evaluation is for ongoing leaves"
 70 |     );
 71 |
 72 |     let legal: Vec<_> = board.empty_moves().collect();
 73 |     let dist = masked_softmax(&result.policy, &legal);
 74 |
```

**COMMENT — preconditions and softmax (lines 66–74).**

- `debug_assert_eq!` documents the precondition (an ongoing board) and checks it in debug and test builds. Release builds normally compile `debug_assert!` out (the `debug-assertions` profile setting controls this), so it is a development safety net, not runtime validation.
- The helper does not re-check whether the node is already `Expanded`; the callers ensure the preconditions. `expand` checks the board status and returns early for already-expanded nodes; `search` calls the helper once, for a fresh ongoing root.
- `masked_softmax` converts the policy logits into probabilities over the legal moves only; illegal moves get zero probability. An ongoing board always has at least one legal move (a full board is a draw), so the distribution is never empty on this path.

```text
 75 |     let mut sum = 0.0f32;
 76 |     for (mv, prob) in dist {
 77 |         let edge_index = tree.node_mut(leaf).edges_mut().len();
 78 |         tree.node_mut(leaf).edges_mut().push(Edge {
 79 |             mv,
 80 |             prior: prob,
 81 |             n: 0,
 82 |             w: 0.0,
 83 |             child: None,
 84 |         });
 85 |         let _child_id = tree.add_child(leaf, edge_index);
 86 |         sum += prob;
 87 |     }
 88 |     debug_assert!((sum - 1.0).abs() < 1e-4, "Priors must sum to 1, got {sum}");
 89 |
 90 |     *tree.node_mut(leaf).state_mut() = NodeState::Expanded;
 91 |     result.value.clamp(-1.0, 1.0)
 92 | }
```

**COMMENT — edge building (lines 75–92).**

- The loop is the extracted old algorithm, unchanged: one edge per legal move, then `add_child` allocates the child node and wires `edge.child`. Writing `child: None` in the literal is correct because `add_child` fills the pointer in afterwards.
- The separate statements exist because of borrow scoping. `tree.node_mut(leaf)` mutably borrows the arena, and `tree.add_child(...)` needs another mutable borrow of the same arena. The arena is a `Vec` that can grow — and reallocate — when children are added, so Rust forbids overlapping the two borrows. Each `node_mut` borrow ends at its semicolon, which keeps the sequence legal.
- `sum` accumulates the priors; the trailing `debug_assert!` checks they sum to 1 within tolerance, catching `masked_softmax` bugs in debug builds only.
- `*tree.node_mut(leaf).state_mut() = NodeState::Expanded` assigns through the returned `&mut NodeState`; the leading `*` is the dereference that writes into the node.
- `result.value.clamp(-1.0, 1.0)` returns the value, the same clamp the old `expand` applied.

The ordinary `expand` function that calls this helper is shown below for context. It is not part of the original solution's numbered snippet, but it is needed to see where `expand_with_evaluation` is reused at the leaf path.

```rust
// Supplemental verified source from expand.rs in the reference worktree.
pub fn expand(tree: &mut Tree, leaf: NodeId, board: &Board, evaluator: &mut dyn Evaluator) -> f32 {
    match board.status() {
        Status::Won(_) => { *tree.node_mut(leaf).state_mut() = NodeState::Terminal; -1.0 }
        Status::Draw => { *tree.node_mut(leaf).state_mut() = NodeState::Terminal; 0.0 }
        Status::Ongoing => {
            if tree.node(leaf).state() == NodeState::Expanded { return 0.0; }
            let legal: Vec<_> = board.empty_moves().collect();
            let planes = engine::encode(board);
            let result = evaluator.evaluate(EvalRequest { planes, legal: legal.clone() });
            expand_with_evaluation(tree, leaf, board, &result)
        }
    }
}
```

**ANNOTATION — `expand` reuses the helper.** The leaf path evaluates the board, then calls `expand_with_evaluation` with the fresh `EvalResult`. The root path evaluated the board inside `evaluate_root` and calls the same helper with that same `EvalResult`. The helper is the extracted pre-existing expansion algorithm; what this slice adds is the second call site, not a new algorithm.


## `mcts` tests for the new root contract

`gomoku/crates/mcts/src/search.rs`, lines 236–434, add the behavioral
tests that pin the contract. The snippets below are the new test
bodies.

The default hook must delegate to `evaluate` exactly once:

```text
236 |     #[test]
237 |     fn default_evaluate_root_delegates_to_evaluate_once() {
238 |         struct CountingEvaluator<'a> {
239 |             inner: &'a mut dyn Evaluator,
240 |             evaluate_calls: Cell<usize>,
241 |         }
242 |
243 |         impl Evaluator for CountingEvaluator<'_> {
244 |             fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
245 |                 self.evaluate_calls.set(self.evaluate_calls.get() + 1);
246 |                 self.inner.evaluate(req)
247 |             }
248 |         }
249 |
250 |         let board = Board::new();
251 |         let mut uniform = UniformEvaluator::new(0.0);
252 |         let mut counting = CountingEvaluator {
253 |             inner: &mut uniform,
254 |             evaluate_calls: Cell::new(0),
255 |         };
256 |
257 |         let _ = search(
258 |             &board,
259 |             &mut counting,
260 |             &SearchConfig {
261 |                 simulations: 0,
262 |                 c_puct: 1.5,
263 |             },
264 |         );
265 |
266 |         assert_eq!(
267 |             counting.evaluate_calls.get(),
268 |             1,
269 |             "default evaluate_root must call evaluate exactly once"
270 |         );
271 |     }
```

**COMMENT — default hook test.**

- Arrange: an empty board, a `UniformEvaluator`, and a local wrapper that counts `evaluate` calls. Act: `search` with zero simulations. Assert: `evaluate` ran exactly once.
- Zero simulations is what isolates the root hook: the simulation loop never runs, so no leaf evaluation can add to the count.
- `CountingEvaluator<'a>` stores a `&'a mut dyn Evaluator`; the named lifetime `'a` ties the wrapper's borrow to the inner evaluator, so the wrapper cannot outlive what it wraps. In `impl Evaluator for CountingEvaluator<'_>`, the anonymous `'_` means the impl applies for any lifetime the struct is instantiated with — the name does not need repeating.
- The `req` parameter is typed `EvalRequest` explicitly and is used (forwarded to the inner evaluator). In later tests the same parameter is named `_req`; the type is still explicit, and the leading underscore only suppresses the unused-variable warning.
- `evaluate_calls: Cell<usize>` is an owned field, not a shared one. `Cell` allows mutation through a shared reference, but `evaluate` takes `&mut self`, so a plain `usize` incremented with `+= 1` would work equally well; `Cell` is optional here and matches the style of the earlier `expand.rs` tests.
- What the test proves: the default `evaluate_root` delegates to `evaluate` exactly once per search. What it does not cover: a custom `evaluate_root` — the next test pins that.


A custom `evaluate_root` returning `Estimate` is called exactly once,
while leaves still use `evaluate`:

```text
273 |     #[test]
274 |     fn root_estimate_is_not_evaluated_twice() {
275 |         struct RootOnceEvaluator {
276 |             root_calls: Cell<usize>,
277 |         }
278 |
279 |         impl Evaluator for RootOnceEvaluator {
280 |             fn evaluate(&mut self, _req: EvalRequest) -> EvalResult {
281 |                 // Leaves use the ordinary evaluator method.
282 |                 EvalResult {
283 |                     policy: [0.0f32; 225],
284 |                     value: 0.0,
285 |                 }
286 |             }
287 |
288 |             fn evaluate_root(&mut self, _req: EvalRequest) -> RootEval {
289 |                 let n = self.root_calls.get();
290 |                 assert_eq!(
291 |                     n, 0,
292 |                     "evaluate_root must be called exactly once at the root"
293 |                 );
294 |                 self.root_calls.set(n + 1);
295 |                 RootEval::Estimate(EvalResult {
296 |                     policy: [0.0f32; 225],
297 |                     value: 0.0,
298 |                 })
299 |             }
300 |         }
301 |
302 |         let board = Board::new();
303 |         let mut ev = RootOnceEvaluator {
304 |             root_calls: Cell::new(0),
305 |         };
306 |
307 |         let outcome = search(
308 |             &board,
309 |             &mut ev,
310 |             &SearchConfig {
311 |                 simulations: 10,
312 |                 c_puct: 1.5,
313 |             },
314 |         );
315 |
316 |         let SearchResult::Searched { .. } = outcome else {
317 |             panic!("estimate root must produce a searched tree");
318 |         };
319 |         assert_eq!(ev.root_calls.get(), 1);
320 |     }
```

**COMMENT — exactly-once hook test.**

- Arrange: an evaluator whose `evaluate_root` panics on a second call and whose `evaluate` returns a quiet estimate. Act: `search` with 10 simulations on an empty board. Assert: the outcome is `Searched` and the hook ran once.
- The 10 simulations matter: they prove the leaf path still evaluates through `evaluate` (leaves get expanded) without ever re-entering `evaluate_root`.
- `let SearchResult::Searched { .. } = outcome else { panic!(...) }` is a let-else: the pattern must match, otherwise the `else` block runs and must diverge (here, panic). The `..` in the pattern ignores the variant's fields. It reads as "assert this variant or fail the test".
- Scope note: the counter pins the hook call count. It does not by itself prove `evaluate` is never called on the root position — that falls out of the code path (`expand_with_evaluation` consumes the estimate), and the baseline's quiet-root test below cross-checks that the fallback estimate equals a direct `evaluate`.


A terminal root never calls the evaluator:

```text
322 |     #[test]
323 |     fn terminal_root_does_not_call_evaluator() {
324 |         struct PanicEvaluator;
325 |
326 |         impl Evaluator for PanicEvaluator {
327 |             fn evaluate(&mut self, _req: EvalRequest) -> EvalResult {
328 |                 panic!("evaluator must not be called on a terminal root");
329 |             }
330 |         }
331 |
332 |         let mut board = Board::new();
333 |         let script = [
334 |             (7, 3),
335 |             (0, 0),
336 |             (7, 4),
337 |             (0, 2),
338 |             (7, 5),
339 |             (0, 4),
340 |             (7, 6),
341 |             (0, 6),
342 |             (7, 7),
343 |         ];
344 |         for &(r, c) in &script {
345 |             board.play(engine::Move::new(r, c).unwrap()).unwrap();
346 |         }
347 |         assert_ne!(board.status(), Status::Ongoing);
348 |
349 |         let outcome = search(
350 |             &board,
351 |             &mut PanicEvaluator,
352 |             &SearchConfig {
353 |                 simulations: 10,
354 |                 c_puct: 1.5,
355 |             },
356 |         );
357 |
358 |         let SearchResult::Searched { tree, root } = outcome else {
359 |             panic!("terminal root must still return a searched tree");
360 |         };
361 |         assert_eq!(tree.node(root).state(), NodeState::Terminal);
362 |     }
```

**COMMENT — terminal root test.**

- Arrange: play a fixed alternating script that gives Black five on row 7, and an evaluator that panics if `evaluate` is called. Act: `search` on the terminal board. Assert: `Searched` with the root marked `Terminal`.
- `PanicEvaluator` implements only `evaluate`, so it uses the default `evaluate_root`, which would call `evaluate`. The terminal guard in `search` returns before either method runs — exactly what the panic pins.
- The `unwrap()` calls are deliberate fail-loudly checks on a fully controlled fixture: `Move::new` fails only for out-of-board coordinates and `board.play` only for illegal moves, so a panic here means the fixture or the engine rules broke, and the test should fail loudly in that case. It is not a claim that `unwrap` is generally safe.
- `assert_ne!(board.status(), Status::Ongoing)` pins the fixture assumption before the behavior under test runs.


A proven root returns immediately without expanding the tree or calling
the leaf evaluator:

```text
364 |     #[test]
365 |     fn proven_root_returns_immediately_without_tree_or_leaf_eval() {
366 |         struct ProvenEvaluator;
367 |
368 |         impl Evaluator for ProvenEvaluator {
369 |             fn evaluate(&mut self, _req: EvalRequest) -> EvalResult {
370 |                 panic!("evaluator leaf method must not be called when root is proven");
371 |             }
372 |
373 |             fn evaluate_root(&mut self, _req: EvalRequest) -> RootEval {
374 |                 RootEval::ProvenWin {
375 |                     mv: engine::Move::new(7, 7).unwrap(),
376 |                 }
377 |             }
378 |         }
379 |
380 |         let board = Board::new();
381 |         let outcome = search(
382 |             &board,
383 |             &mut ProvenEvaluator,
384 |             &SearchConfig {
385 |                 simulations: 10,
386 |                 c_puct: 1.5,
387 |             },
388 |         );
389 |
390 |         assert_eq!(
391 |             outcome,
392 |             SearchResult::ProvenWin {
393 |                 mv: engine::Move::new(7, 7).unwrap(),
394 |             }
395 |         );
396 |     }
```

**COMMENT — proven root test.**

- Arrange: an evaluator that returns `ProvenWin { mv: (7,7) }` from the hook and panics in `evaluate`. Act: `search` with 10 simulations. Assert: the outcome equals `SearchResult::ProvenWin { mv: (7,7) }`.
- The panicking `evaluate` proves the leaf path never runs; the equality assertion proves the certified move is routed through unchanged.
- `assert_eq!` uses the derived `PartialEq`. `Eq` is intentionally not derivable for `SearchResult`: the `Searched` variant holds a `Tree`, whose edges contain `f32` fields (`prior`, `w`), and `f32` cannot implement `Eq` because NaN is not equal to itself. `PartialEq` is sufficient for `assert_eq!`.
- The move (7,7) is arbitrary: `search` does not check that it wins, or even that it makes sense on an empty board — it routes what the evaluator certified. Verification is the evaluator's responsibility, which is why the baseline guards the proof with `verify_line`.
- The test name says "without tree": precisely, no tree is returned and the root node is never expanded; `search` still allocated its one-node tree internally and dropped it.


Repeated searches invoke the hook each time; there is no first-call
cache:

```text
398 |     #[test]
399 |     fn repeated_searches_invoke_root_hook_each_time() {
400 |         struct CountingRootEvaluator {
401 |             calls: Cell<usize>,
402 |         }
403 |
404 |         impl Evaluator for CountingRootEvaluator {
405 |             fn evaluate(&mut self, _req: EvalRequest) -> EvalResult {
406 |                 EvalResult {
407 |                     policy: [0.0f32; 225],
408 |                     value: 0.0,
409 |                 }
410 |             }
411 |
412 |             fn evaluate_root(&mut self, _req: EvalRequest) -> RootEval {
413 |                 self.calls.set(self.calls.get() + 1);
414 |                 RootEval::Estimate(EvalResult {
415 |                     policy: [0.0f32; 225],
416 |                     value: 0.0,
417 |                 })
418 |             }
419 |         }
420 |
421 |         let board = Board::new();
422 |         let mut ev = CountingRootEvaluator {
423 |             calls: Cell::new(0),
424 |         };
425 |         let cfg = SearchConfig {
426 |             simulations: 0,
427 |             c_puct: 1.5,
428 |         };
429 |
430 |         let _ = search(&board, &mut ev, &cfg);
431 |         let _ = search(&board, &mut ev, &cfg);
432 |
433 |         assert_eq!(ev.calls.get(), 2, "each search must invoke evaluate_root");
434 |     }
```

**COMMENT — repeated hook test.**

- Arrange: a hook that counts calls and always returns `Estimate`. Act: two searches on the same empty board. Assert: the count is 2.
- What it proves: there is no first-call cache in `search` — every search asks the evaluator again. What it does not prove: anything about caching inside an evaluator (a cache could sit behind the hook and this test would still pass), and — because every call returns `Estimate` — nothing about how often the baseline's prover re-runs.


## `BaselineEvaluator` implements the guarded root probe

`gomoku/crates/baseline/src/lib.rs`, lines 21–133, shows the budget
constant, the new field and builder, the shared `evaluate_board`
helper, and the `evaluate_root` implementation.

**ANNOTATION — evaluator state and builders.**

- `ROOT_TSS_BUDGET` is `pub` so tests and binaries can read and reuse the default (for example to reset after an override). The visibility is a choice; no rule forces a default constant to be public.
- The budget units are `max_nodes` (threat-space search nodes) and `max_depth` (plies). They bound the prover's search only: they are not a wall-clock budget, and the `verify_line` cost is not separately budgeted.
- `BaselineEvaluator` gains one field, `tss_budget`. `SearchBudget` derives `Copy`, so reading `self.tss_budget` in the probe copies the small struct by value — no borrow or ownership issue.
- `with_tss_budget` is a new associated method in the same consuming-builder style as the pre-existing `without_quiet_value`: `mut self` in, `Self` out, chainable, with `#[must_use]` so ignoring the result warns.
- There are two `impl BaselineEvaluator` blocks. An `impl` block itself has no visibility modifier — each method's own `pub` (or its absence) decides who can call it. The split is organizational: public constructors in the first block, the private evaluation helper in the second.


```text
 21 | /// Default root threat-space-search budget for the baseline evaluator.
 22 | ///
 23 | /// The values are labelled [experiment]: they cap the root probe that
 24 | /// looks for a certified forced win before the MCTS simulations begin.
 25 | pub const ROOT_TSS_BUDGET: engine::SearchBudget = engine::SearchBudget {
 26 |     max_nodes: 10_000,
 27 |     max_depth: 30,
 28 | };
 29 |
```

**COMMENT — the budget constant (lines 21–29).** Why: the root probe must be bounded, or a hard position could stall every move while the prover explores. The values `10_000` nodes and `30` plies are the reference build's experimental knobs, labelled `[experiment]` — tuning choices, not derived constants.

```text
 30 | /// The baseline evaluator.
 31 | ///
 32 | /// v4 combines the gravity kernel with the full policy tier table
 33 | /// (wins, forced blocks, double threats, double threes, threes,
 34 | /// defensive threes) and the chapter-06 value ladder including
 35 | /// double-three rungs.
 36 | pub struct BaselineEvaluator {
 37 |     device: Device,
 38 |     kernel: GravityKernel,
 39 |     net: ThreatNet,
 40 |     quiet: bool,
 41 |     tss_budget: engine::SearchBudget,
 42 | }
 43 |
```

**COMMENT — the struct (lines 30–43).** One new field, `tss_budget`, joins the pre-existing `device`, `kernel`, `net`, and `quiet`. Why a field: the budget must live per-evaluator so tests can shrink it (the budget-exhaustion test below) without touching a global or changing the prover's API.

```text
 44 | impl BaselineEvaluator {
 45 |     /// Create a new [`BaselineEvaluator`] on `device`.
 46 |     ///
 47 |     /// The [experiment] quiet-position value whisper is enabled by
 48 |     /// default; use [`Self::without_quiet_value`] to disable it.
 49 |     #[must_use]
 50 |     pub fn new(device: Device) -> Self {
 51 |         let kernel = GravityKernel::new(&device);
 52 |         let net = ThreatNet::new(&device);
 53 |         Self {
 54 |             device,
 55 |             kernel,
 56 |             net,
 57 |             quiet: true,
 58 |             tss_budget: ROOT_TSS_BUDGET,
 59 |         }
 60 |     }
 61 |
```

**COMMENT — the constructor (lines 44–61).** `new` is unchanged except for initializing the new field from `ROOT_TSS_BUDGET`. `#[must_use]` on a constructor warns when the caller drops the result, which catches a bare `BaselineEvaluator::new(dev);` statement.

```text
 62 |     /// Set the root threat-space-search budget.
 63 |     ///
 64 |     /// The default is [`ROOT_TSS_BUDGET`]; this builder is mainly
 65 |     /// useful for tests that want to exercise budget exhaustion.
 66 |     #[must_use]
 67 |     pub fn with_tss_budget(mut self, budget: engine::SearchBudget) -> Self {
 68 |         self.tss_budget = budget;
 69 |         self
 70 |     }
 71 |
 72 |     /// Disable the [experiment] quiet-position value whisper.
 73 |     ///
 74 |     /// Chainable; intended for A/B measurement of the whisper term.
 75 |     #[must_use]
 76 |     pub fn without_quiet_value(mut self) -> Self {
 77 |         self.quiet = false;
 78 |         self
 79 |     }
 80 | }
 81 |
```

**COMMENT — the builders (lines 62–81).** `with_tss_budget` is the new method; `without_quiet_value` is pre-existing and unchanged, shown for context. Both are consuming builders: they take `mut self` by value, set a field, and return `Self`, which allows chaining off `new(...)`. A consuming builder is a style choice shared by both methods; a `&mut self` setter would work too but would not chain the same way.

```text
 82 | impl BaselineEvaluator {
 83 |     /// Evaluate an already-reconstructed absolute-color board.
 84 |     ///
 85 |     /// Shared by [`Evaluator::evaluate`] and the root probe so the
 86 |     /// board is not reconstructed twice when the root falls back to
 87 |     /// a static estimate.
 88 |     fn evaluate_board(&mut self, board: &engine::Board) -> EvalResult {
 89 |         let maps = patterns::net::analyze_both(&self.net, board, &self.device);
 90 |         let (own, opp) = match board.to_move() {
 91 |             Color::Black => (&maps[0], &maps[1]),
 92 |             Color::White => (&maps[1], &maps[0]),
 93 |         };
```

**COMMENT — `evaluate_board`, part 1 (lines 82–93).**

- Why this helper exists: both `evaluate` and the `evaluate_root` fallback need the same static computation over a reconstructed board. Extracting it lets the fallback path reconstruct once and share the board between the prover and the estimate.
- The body is the pre-chapter static evaluation logic, moved out of `evaluate`. One behavioral difference lands in part 2 below: the helper regenerates `planes` and `legal` from the board, where the old `evaluate` reused `req.planes` and `req.legal`.
- Why an inherent method and not a new `Evaluator` trait method: the trait is the seam the `mcts` crate depends on. Adding a method there would force every evaluator implementation to provide it, or require a default body with no useful implementation. Keeping `evaluate_board` inherent localizes the change to the baseline crate.
- `&mut self` mirrors the `Evaluator` method receivers so both trait methods can call it. The body itself only reads the fields today (`analyze_both` and `kernel.compute` take shared references), so the mutable receiver is consistency with the trait, not a requirement of the computation.
- `patterns::net::analyze_both` runs the threat net for both colors; the `match board.to_move()` picks which threat map belongs to the player to move. This is unchanged logic.

```text
 94 |         let planes = engine::encode(board);
 95 |         let gravity = self.kernel.compute(&planes);
 96 |         let legal: Vec<engine::Move> = board.empty_moves().collect();
 97 |         let policy = readout::policy_logits(&gravity, own, opp, &legal);
 98 |         let value = readout::value(own, opp);
 99 |         // [experiment] quiet whisper: only when the threat ladder is
100 |         // silent. `value == 0.0` also covers symmetric threat
101 |         // cancellation — that counts as "quiet" for this experiment.
102 |         let value = if value == 0.0 && self.quiet {
103 |             readout::quiet_value(&gravity, &planes)
104 |         } else {
105 |             value
106 |         };
107 |         EvalResult { policy, value }
108 |     }
109 | }
```

**COMMENT — `evaluate_board`, part 2 (lines 94–109).**

- Lines 94–96 regenerate `planes` (`engine::encode`) and `legal` (`empty_moves().collect()`) from the reconstructed board. Before this slice, `evaluate` used `req.planes` and `req.legal` directly. The tradeoff, stated exactly: relative to the pre-chapter code, each static evaluation now does one extra encode and one extra legal-move collection; in exchange, the `evaluate_root` fallback pays one board reconstruction for prover plus estimate instead of two (calling `evaluate` from `evaluate_root` would reconstruct again, because `evaluate` owns reconstruction).
- Reconstruction itself is not new: `evaluate` on `main` already rebuilt the board from the request before this slice. The prover needs a `Board`, and `EvalRequest` deliberately carries none, so somebody must reconstruct. Passing a `Board` through the request would remove this work — a different, more invasive design the slice did not choose.
- The quiet-whisper block (lines 99–106) is pre-existing chapter-07 logic, unchanged.

```text
110 |
111 | impl Evaluator for BaselineEvaluator {
112 |     fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
113 |         let board = adapter::reconstruct_board(&req.planes, &req.legal);
114 |         self.evaluate_board(&board)
115 |     }
```

**COMMENT — `evaluate` shrinks to reconstruct plus delegate (lines 110–115).** The reconstruction call existed on `main` before this slice; what changes is that the static computation behind it moved into `evaluate_board`. Leaf evaluations behave as before, one reconstruction per call.

```text
116 |
117 |     fn evaluate_root(&mut self, req: EvalRequest) -> RootEval {
118 |         let board = adapter::reconstruct_board(&req.planes, &req.legal);
119 |         let side = board.to_move();
120 |
121 |         if let Some(proof) = engine::prove_forced_win(&board, side, self.tss_budget) {
122 |             let verified = proof.winner == side
123 |                 && !proof.line.is_empty()
124 |                 && board.is_legal(proof.line[0])
125 |                 && engine::verify_line(&board, &proof);
126 |             if verified {
127 |                 return RootEval::ProvenWin { mv: proof.line[0] };
128 |             }
129 |         }
130 |
131 |         RootEval::Estimate(self.evaluate_board(&board))
132 |     }
133 | }
```

**COMMENT — the root probe (lines 116–133).**

- Line 118 reconstructs the board exactly as `evaluate` does; the prover (`prove_forced_win`) and the verifier (`verify_line`) both need a `Board`, and the request carries none (see the helper comments above).
- Line 119 fixes `side` as the current side to move. The probe only ever asks "can the player to move force a win?" — never a fixed color.
- Line 121 runs the bounded prover. `if let Some(proof)` moves the `Proof` into the arm; `None` means "not proven within budget" and, by engine contract, says nothing about the true value of the position. `self.tss_budget` is copied into the call because `SearchBudget` is `Copy`.
- Lines 122–125 are the acceptance guard, evaluated left to right with short-circuiting `&&`:
  1. `proof.winner == side` — the proof is for the player to move.
  2. `!proof.line.is_empty()` — the line has at least one move. This check must precede `proof.line[0]` or the index could panic; a `line.first()`-style formulation would be equally safe.
  3. `board.is_legal(proof.line[0])` — the first move is playable now.
  4. `engine::verify_line(&board, &proof)` — the line is machine-verified.
- Against the current engine verifier, checks 1–3 do not change the verdict: `verify_line` already rejects a proof whose winner is not the side to move, and its recursive checker rejects an empty line and an illegal first move before replaying anything. Check 2 additionally makes the indexing in check 3 safe. The explicit repetition is defensive overlap — the acceptance policy is visible at the consumption site and robust to future engine changes — not a set of four independently necessary conditions.
- What `verify_line` actually verifies: it replays the line on a board clone and checks, move by move, that every move is legal and alternating; that each attacker move creates a genuine forced threat (an immediate win the defender must address); that the defender has no counter-win that lands first; and that every forced-block alternative loses to the same suffix (a gapped four admits two blocks; all are enumerated). It does not enumerate every legal defender reply, and it is not a whole-board brute force. It checks the line independently rather than trusting the prover's internal structure.
- Soundness does not come from this guard alone: the engine intends `Some` to mean certified, and the prover even calls `verify_line` internally on continuation suffixes. But there is no universal top-level verification inside `prove_forced_win`, so the consumption-site `verify_line` is what guarantees that whatever reaches `RootEval::ProvenWin` passed the verifier against this board.
- Line 127 returns early, before the static estimate on line 131 — a proven root does no kernel or threat-net work. `proof.line[0]` copies the `Move` out (`Move` is `Copy`); the rest of the proof is dropped when the arm ends. Nothing is cached; the next `evaluate_root` call probes anew.
- `ProvenWin` does not mean the game is already over: a line may end at an unblockable double threat, several plies before five stones land. The CLI will play the first move; the win is forced later.


The `evaluate_root` method reconstructs the board, proves only for the
side to move, and accepts the proof only after four guards. Any failure
falls back to the same static estimate that `evaluate` would return.

## Baseline tests for the root probe

`gomoku/crates/baseline/src/lib.rs`, lines 405–555, add the tests that
exercise the budget, the proof path, the fallback path, and the
leaf-only path.

Default budget and builder:

```text
405 |     #[test]
406 |     fn root_tss_budget_default_is_10k_30() {
407 |         let ev = BaselineEvaluator::new(Device::flex());
408 |         assert_eq!(ev.tss_budget, ROOT_TSS_BUDGET);
409 |         assert_eq!(ROOT_TSS_BUDGET.max_nodes, 10_000);
410 |         assert_eq!(ROOT_TSS_BUDGET.max_depth, 30);
411 |     }
412 |
413 |     #[test]
414 |     fn with_tss_budget_overrides_default() {
415 |         let ev = BaselineEvaluator::new(Device::flex()).with_tss_budget(engine::SearchBudget {
416 |             max_nodes: 1,
417 |             max_depth: 1,
418 |         });
419 |         assert_eq!(ev.tss_budget.max_nodes, 1);
420 |         assert_eq!(ev.tss_budget.max_depth, 1);
421 |     }
```

**COMMENT — budget builder tests.** Arrange, act, assert: read the default budget; then override it and read the override. `assert_eq!(ev.tss_budget, ROOT_TSS_BUDGET)` works because `SearchBudget` derives `PartialEq` (and `Eq` — all its fields are integers, so the float problem does not arise). These tests pin the wiring (constant → field, builder → field), not any search behavior.


Quiet root falls back to an estimate equal to `evaluate`:

```text
423 |     #[test]
424 |     fn evaluate_root_delegates_to_estimate_on_quiet_board() {
425 |         let board = Board::new();
426 |         let req = request_from_board(&board);
427 |         let mut ev = BaselineEvaluator::new(Device::flex());
428 |
429 |         let estimate = ev.evaluate(req.clone());
430 |         let root = ev.evaluate_root(req);
431 |
432 |         assert!(
433 |             matches!(root, RootEval::Estimate(_)),
434 |             "quiet root must fall back to a static estimate"
435 |         );
436 |         if let RootEval::Estimate(res) = root {
437 |             assert_eq!(res, estimate);
438 |         }
439 |     }
```

**COMMENT — quiet fallback test.**

- Arrange: an empty board and its `EvalRequest`. Act: call `evaluate` once and `evaluate_root` once. Assert: the root result is an `Estimate`, and its payload equals the direct `evaluate` result.
- `req.clone()` exists because both calls consume the request (`evaluate` takes `req` by value). Cloning is one of two workable approaches; rebuilding with `request_from_board(&board)` a second time would do the same job.
- `matches!(root, RootEval::Estimate(_))` is a boolean expression: true if `root` is the `Estimate` variant, with `_` ignoring the payload. The following `if let RootEval::Estimate(res) = root` re-matches to bind the payload for the equality check — two steps because `matches!` cannot bind.
- What it proves: on a quiet board the fallback equals a plain `evaluate`. What it does not prove: whether the prover ran or how often, and whether anything is cached between the two calls — the assertions never observe the prover.


Proven win on the engine win-in-3 script:

```text
441 |     #[test]
442 |     fn evaluate_root_finds_verified_vcf_proof() {
443 |         // Black has a win-in-3 VCF; the root probe should certify it.
444 |         let mut board = Board::new();
445 |         let script = [
446 |             (7, 4),
447 |             (7, 3),
448 |             (7, 5),
449 |             (0, 0),
450 |             (7, 6),
451 |             (0, 2),
452 |             (6, 6),
453 |             (0, 4),
454 |             (8, 8),
455 |             (0, 6),
456 |         ];
457 |         for &(r, c) in &script {
458 |             board.play(Move::new(r, c).unwrap()).unwrap();
459 |         }
460 |         assert_eq!(board.to_move(), Color::Black);
461 |
462 |         let req = request_from_board(&board);
463 |         let mut ev = BaselineEvaluator::new(Device::flex());
464 |         let root = ev.evaluate_root(req);
465 |
466 |         assert!(
467 |             matches!(root, RootEval::ProvenWin { mv } if mv == Move::new(7, 7).unwrap()),
468 |             "expected proven win starting at h8, got {root:?}"
469 |         );
470 |     }
```

**COMMENT — VCF proof test.**

- Arrange: a fixed 10-move script that leaves Black a win-in-3 VCF, verified by `assert_eq!(board.to_move(), Color::Black)`. Act: `evaluate_root`. Assert: `ProvenWin` starting at h8.
- The `matches!` guard `if mv == Move::new(7, 7).unwrap()` adds a boolean condition to the pattern: the variant must be `ProvenWin` and the bound `mv` must equal h8 ((7,7) in zero-based row/column). The `{root:?}` in the message formats the actual value with `Debug` on failure.
- The fixture `unwrap()`s fail loudly if the script ever stops being legal — deliberate, as in the terminal-root test above.
- This is the end-to-end pin of the whole slice: real board → real prover → real verifier → certified first move.


Budget exhaustion forces a fallback:

```text
472 |     #[test]
473 |     fn evaluate_root_falls_back_when_budget_is_exhausted() {
474 |         // Same VCF position, but with a zero budget the prover cannot
475 |         // see the win and must return a static estimate instead.
476 |         let mut board = Board::new();
477 |         let script = [
478 |             (7, 4),
479 |             (7, 3),
480 |             (7, 5),
481 |             (0, 0),
482 |             (7, 6),
483 |             (0, 2),
484 |             (6, 6),
485 |             (0, 4),
486 |             (8, 8),
487 |             (0, 6),
488 |         ];
489 |         for &(r, c) in &script {
490 |             board.play(Move::new(r, c).unwrap()).unwrap();
491 |         }
492 |
493 |         let req = request_from_board(&board);
494 |         let mut ev = BaselineEvaluator::new(Device::flex()).with_tss_budget(engine::SearchBudget {
495 |             max_nodes: 0,
496 |             max_depth: 0,
497 |         });
498 |         let root = ev.evaluate_root(req);
499 |
500 |         assert!(
501 |             matches!(root, RootEval::Estimate(_)),
502 |             "zero budget must force a fallback estimate"
503 |         );
504 |     }
```

**COMMENT — budget exhaustion test.** Same VCF position, but `with_tss_budget` sets zero nodes and zero depth, so the prover cannot explore anything and returns `None`. The assertion only requires an `Estimate` fallback. "Budget exhausted" is not "position is lost" — the position is still a forced win; the probe simply cannot afford to prove it. This is exactly the engine's `None` semantics.


`evaluate` does not run TSS:

```text
506 |     #[test]
507 |     fn evaluate_does_not_run_tss() {
508 |         // Same VCF position; a direct leaf evaluation must not claim a
509 |         // proven win.
510 |         let mut board = Board::new();
511 |         let script = [
512 |             (7, 4),
513 |             (7, 3),
514 |             (7, 5),
515 |             (0, 0),
516 |             (7, 6),
517 |             (0, 2),
518 |             (6, 6),
519 |             (0, 4),
520 |             (8, 8),
521 |             (0, 6),
522 |         ];
523 |         for &(r, c) in &script {
524 |             board.play(Move::new(r, c).unwrap()).unwrap();
525 |         }
526 |
527 |         let req = request_from_board(&board);
528 |         let mut ev = BaselineEvaluator::new(Device::flex());
529 |         let res = ev.evaluate(req);
530 |
531 |         // The static estimate may or may not see a strong threat, but
532 |         // it must return a finite result and never a proven win.
533 |         assert!(res.value.is_finite());
534 |     }
```

**COMMENT — leaf-only path test.**

- Act: call `evaluate` (not `evaluate_root`) on the VCF position. Assert: the value is finite.
- Part of the contract is type-level: `evaluate` returns `EvalResult`, which has no way to express a proof at all — "never a proven win" is enforced by the return type, not just the implementation.
- The `is_finite` assertion pins the output contract (a usable estimate), but it cannot observe whether TSS ran internally: a hidden prover call would still return a finite value. The test name states the intent; the assertion covers the observable part of it.


Repeated root calls probe each new root:

```text
536 |     #[test]
537 |     fn repeated_evaluate_root_probes_each_root() {
538 |         let mut board1 = Board::new();
539 |         board1.play(Move::new(7, 7).unwrap()).unwrap();
540 |         board1.play(Move::new(0, 0).unwrap()).unwrap();
541 |
542 |         let mut board2 = Board::new();
543 |         board2.play(Move::new(6, 6).unwrap()).unwrap();
544 |         board2.play(Move::new(0, 0).unwrap()).unwrap();
545 |
546 |         let mut ev = BaselineEvaluator::new(Device::flex());
547 |         let root1 = ev.evaluate_root(request_from_board(&board1));
548 |         let root2 = ev.evaluate_root(request_from_board(&board2));
549 |
550 |         // Both calls must complete without panic, must not share
551 |         // state, and must both fall back to static estimates on these
552 |         // quiet positions.
553 |         assert!(matches!(root1, RootEval::Estimate(_)));
554 |         assert!(matches!(root2, RootEval::Estimate(_)));
555 |     }
```

**COMMENT — repeated root probe test.**

- Arrange: two different quiet positions, one evaluator. Act: `evaluate_root` on each. Assert: both return `Estimate`.
- The in-code comment says the calls "must not share state", but the test contains no prover counter and no cache inspection, so shared state is not actually observed. What the assertions do pin: two different roots both fall back to estimates on quiet positions, and nothing panics. Two `Estimate` outcomes also say nothing about how often the prover ran.


## The CLI matches `SearchResult` generically

`gomoku/crates/cli/src/session.rs`, lines 444–469, is the only CLI
change. A proven win bypasses noise, temperature, visit sampling, and
`SearchInfo` storage.

```text
444 |         let outcome = mcts::search(&board, &mut *evaluator, &search_cfg);
445 |
446 |         let (mv, info) = match outcome {
447 |             mcts::SearchResult::ProvenWin { mv } => {
448 |                 // A certified forced win bypasses noise, temperature,
449 |                 // and visit sampling. Execute it directly.
450 |                 return self.execute_play(mv);
451 |             }
```

**COMMENT — search call and proven arm (lines 444–451).**

- `&mut *evaluator`: `evaluator` is a `Box<dyn Evaluator>`; `*` dereferences the box to the trait object and `&mut` reborrows it as the `&mut dyn Evaluator` that `search` takes. `evaluator.as_mut()` would be an equivalent spelling.
- The proven arm ends in `return self.execute_play(mv)`. `execute_play` returns `Result<Vec<Event>, CommandError>` — exactly `execute_step`'s return type — so the early return type-checks, and because that arm diverges, only the `Searched` arm needs to build the `(mv, info)` tuple.
- `execute_play` plays through `history.play`, which validates legality against the engine: an illegal "proven" move becomes a returned error, not a played stone. A legal but unsound move would be played — nothing downstream re-runs `verify_line`.
- What the proven path skips: noise, `SearchInfo` construction, visit sampling, and the post-match tail of this function (`history.play`, info insertion, event assembly — `execute_play` does its own play and events). The one-node tree inside `search` was still allocated and then dropped; what is bypassed is expansion, simulations, and the returned tree.

```text
452 |             mcts::SearchResult::Searched { mut tree, root } => {
453 |                 if effective_config.noise_epsilon > 0.0 {
454 |                     mcts::add_dirichlet_noise(
455 |                         &mut tree,
456 |                         root,
457 |                         effective_config.noise_epsilon,
458 |                         0.3,
459 |                         &mut self.rng,
460 |                     );
461 |                 }
```

**COMMENT — noise (lines 452–461).**

- `mut tree` in the pattern: `add_dirichlet_noise` takes `&mut Tree`, so the destructured binding must be declared mutable. This is the only reason for `mut`.
- What noise does and does not change at this point in the pipeline. It runs after all simulations, so the visit counts are already fixed. `select_move` below reads only visit counts — it never looks at priors — so noise does not alter move-selection weights at all. What noise does change: (a) the stored root-edge priors, which `SearchInfo::from_searched` on the next line reads into `info.prior`, so the recorded policy shows the noised priors; and (b) the shared `self.rng`, which the noise sampler advances, so the temperature sampling below — and any later random draws in the session — see a different RNG state than a no-noise run would.

```text
462 |
463 |                 let info = SearchInfo::from_searched(&tree, root, &board, &effective_config);
464 |                 let visits = mcts::visit_distribution(&tree, root);
465 |                 let mv = mcts::select_move(&visits, effective_config.temperature, &mut self.rng)
466 |                     .expect("AI must select a move from a non-empty distribution");
467 |                 (mv, info)
468 |             }
469 |         };
```

**COMMENT — info, visits, selection (lines 462–469).**

- `SearchInfo::from_searched` builds an owned `SearchInfo` from borrows of the tree, root, board (still the pre-move board), and config. The pre-slice constructor `from_search` took `&SearchOutcome`; that type no longer exists, so the signature change is forced. The rename itself is cosmetic — the supplementary listing below shows the body.
- `visit_distribution` reads the root edges' visit counts; `select_move` samples from those counts at the configured temperature (argmax at temperature 0). The `.expect` relies on an invariant, not luck: the root was expanded on the ongoing path, so the distribution is non-empty.
- Immediately after this match (just past the numbered snippet), the searched path calls `self.history.play(mv)?` — not `execute_play` — then inserts the info at the new cursor and assembles events. Only the proven arm uses `execute_play`.
- About `search_info`: it is a `HashMap` keyed by history node id, and old entries are kept. The proven path inserts no new `SearchInfo` at the new cursor, but "no info at this cursor" is only guaranteed when the cursor was never searched before — revisiting an old line can surface a pre-existing entry. The tests below use fresh sessions, where `search_info()` after a proven move is `None`.

The `SearchInfo` constructor lives in `actor.rs`, which also changed in this slice. The reference body is shown below for context; it is not part of the original solution's numbered snippet.

```rust
// Supplemental verified source from cli/src/actor.rs in the reference worktree.
impl SearchInfo {
    pub fn from_searched(
        tree: &mcts::Tree,
        root: mcts::NodeId,
        board: &Board,
        config: &AiConfig,
    ) -> Self {
        let edges = tree.node(root).edges();
        let mut visits: Vec<(Move, u32)> = edges.iter().map(|e| (e.mv, e.n)).collect();
        visits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.index().cmp(&b.0.index())));
        let mut prior: Vec<(Move, f32)> = edges.iter().map(|e| (e.mv, e.prior)).collect();
        prior.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.index().cmp(&b.0.index()))
        });
        let total_visits: u32 = edges.iter().map(|e| e.n).sum();
        let total_value: f32 = edges.iter().map(|e| e.w).sum();
        let value = if total_visits == 0 { 0.0 } else { total_value / total_visits as f32 };
        Self { side_to_move: board.to_move(), visits, prior, value, config: config.clone() }
    }
}
```

**COMMENT — `from_searched` body.** Visits and priors are read from the root edges and each sorted (visits by count descending, priors by probability descending, both with a deterministic move-index tiebreak); the root value is the visit-weighted average of edge values, matching the backup convention documented on `SearchInfo`. The borrows end when the constructor returns, so the owned `tree` can be dropped afterwards. Nothing here is baseline-specific — the same code serves every `Evaluator` implementation, which is what "matches `SearchResult` generically" means; the seat's evaluator kind was already resolved in `make_evaluator`. The prior list is read after noise was applied, so it records the noised priors (see the noise comment above).


## CLI actor tests

`gomoku/crates/cli/tests/actor.rs`, lines 226–322, add the two tests
that prove the generic routing works for the baseline seat.

Proven win plays directly with no search info:

```text
226 | #[test]
227 | fn baseline_ai_plays_proven_vcf_without_search_info() {
228 |     let mut board = Board::new();
229 |     let script = [
230 |         (7, 4),
231 |         (7, 3),
232 |         (7, 5),
233 |         (0, 0),
234 |         (7, 6),
235 |         (0, 2),
236 |         (6, 6),
237 |         (0, 4),
238 |         (8, 8),
239 |         (0, 6),
240 |     ];
241 |     for &(r, c) in &script {
242 |         board.play(mv(r, c)).unwrap();
243 |     }
244 |     assert_eq!(board.to_move(), Color::Black);
245 |
246 |     let mut core = SessionCore::new(SessionInit {
247 |         black: Occupant::Ai(AiConfig {
248 |             evaluator: EvaluatorKind::Baseline,
249 |             simulations: 50,
250 |             c_puct: 1.5,
251 |             temperature: 0.0,
252 |             noise_epsilon: 0.0,
253 |             explicit_selection: true,
254 |         }),
255 |         white: Occupant::Human,
256 |         default_config: AiConfig::default(),
257 |         black_sims: None,
258 |         white_sims: None,
259 |         seed: Some(1),
260 |         opening: board,
261 |         swap2: false,
262 |         puzzle: None,
263 |     });
264 |
265 |     let events = core.execute(Command::Step).unwrap();
266 |     let moved = events
267 |         .iter()
268 |         .find_map(|e| match e {
269 |             Event::Moved { mv, .. } => Some(*mv),
270 |             _ => None,
271 |         })
272 |         .expect("AI should move.");
273 |     assert_eq!(
274 |         moved,
275 |         mv(7, 7),
276 |         "proven VCF must play the forcing first move"
277 |     );
278 |     assert!(
279 |         core.search_info().is_none(),
280 |         "proven win must not store search info"
281 |     );
282 | }
```

**COMMENT — proven VCF CLI test.**

- Arrange: the same VCF script as the baseline unit test, a session with an AI Black seat (`EvaluatorKind::Baseline`, 50 simulations, temperature 0, noise 0, explicit selection) and a fixed seed. Act: `core.execute(Command::Step)`. Assert: the emitted move is h8 and `core.search_info()` is `None`.
- The move is extracted with `find_map` over `&Event`: the closure returns `Some(*mv)` for `Event::Moved` and `None` otherwise, and `find_map` yields the first `Some`. `*mv` copies the move out of the shared reference (`Move` is `Copy`); `..` ignores the event's other fields. `.expect("AI should move.")` fails loudly if no move event exists.
- Temperature 0 and noise 0 keep the session deterministic, but they are not what makes the bypass observable — the bypass is structural: the proven arm returns before noise, sampling, and info storage. No configuration of noise or temperature reaches the proven path.
- `search_info().is_none()`: in this fresh session exactly one step ran and the proven arm inserted nothing, so the current cursor has no entry. The `HashMap` note in the CLI section explains why this is not a universal guarantee.


Quiet position falls back to search and stores info:

```text
284 | #[test]
285 | fn baseline_ai_falls_back_to_search_info_on_quiet_position() {
286 |     let mut core = SessionCore::new(SessionInit {
287 |         black: Occupant::Ai(AiConfig {
288 |             evaluator: EvaluatorKind::Baseline,
289 |             simulations: 50,
290 |             c_puct: 1.5,
291 |             temperature: 0.0,
292 |             noise_epsilon: 0.0,
293 |             explicit_selection: true,
294 |         }),
295 |         white: Occupant::Human,
296 |         default_config: AiConfig::default(),
297 |         black_sims: None,
298 |         white_sims: None,
299 |         seed: Some(1),
300 |         opening: Board::new(),
301 |         swap2: false,
302 |         puzzle: None,
303 |     });
304 |
305 |     let events = core.execute(Command::Step).unwrap();
306 |     assert!(
307 |         events.iter().any(|e| matches! {
308 |             e, Event::Moved { by: OccupantKind::Ai, ..}
309 |         }),
310 |         "Step with an AI seat must emit Move{{by: Ai}}, got {events:?}"
311 |     );
312 |
313 |     let info = core
314 |         .search_info()
315 |         .expect("searched position must store info");
316 |     assert_eq!(info.config.simulations, 50);
317 |     assert_eq!(
318 |         info.visits.iter().map(|(_, n)| n).sum::<u32>(),
319 |         50,
320 |         "total root visits must match simulations"
321 |     );
322 | }
```

**COMMENT — quiet fallback CLI test.**

- Arrange: an empty-board session with the same baseline seat. Act: one `Step`. Assert: an AI `Moved` event exists, and the stored `SearchInfo` records 50 simulations with root visit counts summing to 50.
- The visit-sum assertion is the pre-expansion invariant from the `search` comments, observed end-to-end through the CLI: every simulation passed through exactly one root edge.
- `matches! { e, Event::Moved { by: OccupantKind::Ai, .. } }` checks the variant plus one field, ignoring the rest with `..`. The `{events:?}` failure message prints the whole event list via `Debug`.
- The test checks the structural contract (move emitted, info stored, counts consistent), not move quality.


## Notes

* The `mcts` acceptance test (`mcts/tests/acceptance.rs`) and the
  existing `mcts/src/search.rs` tactical tests must also be updated to
  match on `SearchResult`; the snippets above show only the new tests.
* The old `baseline::probe` module and the CLI feature-gated
  `baseline::probe::root_probe` short-circuit are removed. The probe
  lives entirely inside `BaselineEvaluator::evaluate_root`.
* The TSS visualization overlay in the CLI (`t` key) is unchanged; it
  calls `prove_forced_win` independently for display purposes and is
  not part of the move-selection path.


---

**ANNOTATION — mechanical migration and decorator trap.**

- The original solution focuses on the three changed files, but the mechanical migration is larger. Every call site that consumed `mcts::SearchOutcome` switches to `mcts::SearchResult`. On `main` before this redesign, `SearchInfo::from_search` took `&mcts::SearchOutcome`; in the reference worktree it is `from_searched(&Tree, NodeId, &Board, &AiConfig)`. The `mcts` crate also renames the export in `lib.rs`.
- The decorator trap: a wrapper that overrides `evaluate` but relies on the default `evaluate_root` wraps every position — including the root — in `RootEval::Estimate`, silently disabling the baseline's certification. The reference carries a regression test in `baseline/tests/acceptance.rs` whose wrapper forwards both methods:

```rust
// Supplemental verified source from baseline/tests/acceptance.rs in the reference worktree.
struct CountingRootForward<'a> {
    inner: &'a mut BaselineEvaluator,
    root_calls: Cell<usize>,
    eval_calls: Cell<usize>,
}

impl Evaluator for CountingRootForward<'_> {
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
        self.eval_calls.set(self.eval_calls.get() + 1);
        self.inner.evaluate(req)
    }

    fn evaluate_root(&mut self, req: EvalRequest) -> RootEval {
        self.root_calls.set(self.root_calls.get() + 1);
        self.inner.evaluate_root(req)
    }
}
```

- The `baseline::probe` removal note is historical context for versions of `main` that had it; the CLI `t` overlay still calls `prove_forced_win` on its own and is independent of the move-selection hook.

---

## Minimal necessary mechanism versus optional choices

**Essential behaviors (any correct implementation of this slice has them):**

- A root-level evaluation hook that runs exactly once per search, before the simulation loop, on ongoing positions only.
- A way for the evaluator to say "certified move, skip the search" that a static `EvalResult` cannot express.
- `search` returning either something the caller can sample visit counts from, or the certified move.
- Reusing the root's already-computed estimate for root expansion instead of evaluating the root twice.
- A baseline probe that bounds the prover with a budget and accepts a proof only after `verify_line` passes, with safe access to the proof line.
- CLI routing that plays a certified move directly (still through legality-checked play) and otherwise samples from visit counts and stores search info.

**Chosen encodings (the reference picks these; alternatives exist):**

- Two enums (`RootEval`, `SearchResult`) with these names and payloads — one enum, or a struct with an `Option<Tree>`, could encode the same outcomes.
- The helper split: `expand_with_evaluation` for shared edge-building, `evaluate_board` for shared static evaluation.
- Two inherent `impl BaselineEvaluator` blocks; `Cell<usize>` test counters; a public `ROOT_TSS_BUDGET` constant.
- The budget values `10_000` nodes and `30` plies, labelled `[experiment]`.
- The four-part guard expression. Of the four checks, `verify_line` is the substantive soundness check and the non-empty check makes the `line[0]` index safe; the winner and first-move-legality checks duplicate what the current verifier already rejects, and survive as defensive overlap rather than necessity.

**What this design does not promise:**

- It does not prove positions are lost when the budget is exhausted.
- It does not cache proofs between searches.
- It does not verify the proof inside `search`; verification stays in the evaluator.
- It does not make the TSS overlay part of move selection.
