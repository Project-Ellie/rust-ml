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
 45 |
 46 | /// the contract with any evaluator implementation
 47 | pub trait Evaluator {
 48 |     /// return policy and value for current board position
 49 |     fn evaluate(&mut self, req: EvalRequest) -> EvalResult;
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

`lib.rs` re-exports `RootEval` alongside the other public names so
`BaselineEvaluator` can implement the hook and tests can pattern-match
on it.

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
 70 |
 71 |     let legal: Vec<_> = root_board.empty_moves().collect();
 72 |     let planes = engine::encode(root_board);
 73 |     let root_eval = evaluator.evaluate_root(EvalRequest {
 74 |         planes,
 75 |         legal: legal.clone(),
 76 |     });
 77 |
 78 |     match root_eval {
 79 |         RootEval::ProvenWin { mv } => SearchResult::ProvenWin { mv },
 80 |         RootEval::Estimate(result) => {
 81 |             let _ = expand_with_evaluation(&mut tree, root, root_board, &result);
 82 |
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
 66 |     debug_assert_eq!(
 67 |         board.status(),
 68 |         Status::Ongoing,
 69 |         "expand_with_evaluation is for ongoing leaves"
 70 |     );
 71 |
 72 |     let legal: Vec<_> = board.empty_moves().collect();
 73 |     let dist = masked_softmax(&result.policy, &legal);
 74 |
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

## `BaselineEvaluator` implements the guarded root probe

`gomoku/crates/baseline/src/lib.rs`, lines 21–133, shows the budget
constant, the new field and builder, the shared `evaluate_board`
helper, and the `evaluate_root` implementation.

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
110 |
111 | impl Evaluator for BaselineEvaluator {
112 |     fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
113 |         let board = adapter::reconstruct_board(&req.planes, &req.legal);
114 |         self.evaluate_board(&board)
115 |     }
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
462 |
463 |                 let info = SearchInfo::from_searched(&tree, root, &board, &effective_config);
464 |                 let visits = mcts::visit_distribution(&tree, root);
465 |                 let mv = mcts::select_move(&visits, effective_config.temperature, &mut self.rng)
466 |                     .expect("AI must select a move from a non-empty distribution");
467 |                 (mv, info)
468 |             }
469 |         };
```

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
