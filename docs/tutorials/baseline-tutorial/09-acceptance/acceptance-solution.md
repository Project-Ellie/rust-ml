> **Opt-in solution — read only after you have tried the chapter, or if you have been stuck for more than twenty minutes.**
>
> **Stale-quotes warning (2026-11-06).** This file quotes the retired
> hook-era scratch reference
> `/Users/wgiersche/workspace/Project-Ellie/rust-ml-tss-root-doc-ref`,
> which implemented the superseded evaluator-owned root hook
> (`RootEval` / `evaluate_root` / `SearchResult`). The current design —
> the merged sub-chapter 08 — runs the TSS proof at player level: the
> `player` crate's `AiPlayer::decide` probes `engine::prove_forced_win`
> once per move before an ordinary `mcts::search`, which keeps
> returning `SearchOutcome`. The player-level design is verified in the
> uncommitted reference worktree
> `/Users/wgiersche/workspace/Project-Ellie/rust-ml-unified08-ref`
> (563 passed / 0 failed / 5 ignored, clippy and fmt green); the
> acceptance harness has **not** been rebuilt against it yet — the
> sections marked *hook-era* below are kept as historical quotation,
> and the accompanying prose gives the intended player-level target.
> The learner's implementation on `main` may use different names or
> helper locations, but it must satisfy the same behavioral contracts.
> Line numbers are from the reference worktree, not from the learner's
> `main`.

---

Verified green from the hook-era `rust-ml-tss-root-doc-ref/gomoku/`
(historical record):

```text
cargo test --all-targets
cargo test -p baseline --release --test acceptance -- --ignored --nocapture
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

All four commands exited with status 0. Neither reference is a
committed branch; both are disposable worktrees kept for inspection.

---

The current design moves the TSS proof from the evaluator into the
`player` crate's `AiPlayer::decide` (an intermediate redesign had it
behind `BaselineEvaluator::evaluate_root`; retired before landing).
The acceptance harness target in
`gomoku/crates/baseline/tests/acceptance.rs` contains:

* a deliberately duplicated puzzle-pack parser,
* a `best_move` helper that drives the seat's persistent
  `player::AiPlayer` (own probe + guard inside `decide`),
* tactical asserts and a fast-evaluator flood,
* a successive-defender-replies regression test,
* the preserved ignored quiet-whisper A/B from sub-chapter 07,
* the ignored strength-ladder measurement.

(The hook-era list also named a decorator-forwarding regression test;
retired — see §7.)

## 0. Dev-dependency migration

The test file parses JSON puzzle packs, so `baseline` needs
`serde`/`serde_json` as dev-dependencies only, plus the sub-chapter 08
`player` crate so the harness drives games through `AiPlayer`. In
`gomoku/crates/baseline/Cargo.toml`:

```toml
[dev-dependencies]
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
player = { path = "../player" }
```

This is test-only; it does not create a production dependency cycle
with `cli`.

## 1. Top of the test file

`gomoku/crates/baseline/tests/acceptance.rs`, hook-era lines 1–17
(the `RootEval` / `SearchResult` imports are retired; the intended
target imports follow):

```text
  1 | //! Acceptance tests for the baseline evaluator.
  2 | //!
  3 | //! Tactical soundness is asserted every run. Strength is measured by
  4 | //! the ignored `strength_ladder_measured` test and quoted in the
  5 | //! tutorial chapter. The quiet-whisper A/B from sub-chapter 07 is
  6 | //! preserved as `quiet_whisper_ab_measured`.
  7 |
  8 | use baseline::{BaselineEvaluator, Device, adapter};
  9 | use engine::{Board, Color, Move, Status, reference::board_from_ascii};
 10 | use mcts::{
 11 |     EvalRequest, EvalResult, Evaluator, RootEval, SearchConfig, SearchResult, TacticsEvaluator,
 12 |     search, select_move, visit_distribution,
 13 | };
 14 | use rand::rngs::StdRng;
 15 | use rand::{RngExt, SeedableRng};
 16 | use std::cell::Cell;
 17 | use std::time::Instant;
```

Intended player-level target (no line numbers yet — the harness is
rebuilt when sub-chapter 09 is worked, then re-quoted from the
verified reference):

```rust
use baseline::{BaselineEvaluator, Device, adapter};
use engine::{Board, Color, Move, Status, reference::board_from_ascii};
use mcts::{Evaluator, TacticsEvaluator};
use player::{AiPlayer, GuardConfig, OwnTssConfig, PlayerOptions};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use std::time::Instant;
```

## 2. Puzzle pack parser

The parser is deliberately duplicated from `cli::puzzle` to avoid a
dev-dependency cycle (`cli` depends on `baseline`). The comment at the
top of the module names the precedent.

Module header, `Puzzle`, and `PuzzlePack` (lines 19–49):

```text
 19 | // ------------------------------------------------------------------
 20 | // Puzzle pack parsing — deliberate minimal duplication of
 21 | // `cli::puzzle::PuzzlePack` to avoid a dev-dependency cycle (cli
 22 | // already depends on baseline).
 23 | // ------------------------------------------------------------------
 24 |
 25 | mod pack {
 26 |     use engine::{Board, Color, Move, PositionError};
 27 |     use serde::Deserialize;
 28 |     use std::collections::HashSet;
 29 |     use std::path::PathBuf;
 30 |
 31 |     /// One parsed puzzle position.
 32 |     #[derive(Debug, Clone, PartialEq, Eq)]
 33 |     pub struct Puzzle {
 34 |         /// Zero-based index within the pack.
 35 |         pub index: usize,
 36 |         /// Side to move (the attacker).
 37 |         pub attacker: Color,
 38 |         /// The board position with the attacker to move.
 39 |         pub board: Board,
 40 |         /// Claimed forced-win line, `(move, side)` per ply.
 41 |         pub solution: Vec<(Move, Color)>,
 42 |     }
 43 |
 44 |     /// A parsed puzzle pack.
 45 |     #[derive(Debug, Clone, PartialEq, Eq)]
 46 |     pub struct PuzzlePack {
 47 |         /// All puzzles in file order.
 48 |         pub items: Vec<Puzzle>,
 49 |     }
```

`PuzzlePack::from_json` and `from_path` (lines 51–83):

```text
 51 |     impl PuzzlePack {
 52 |         /// Parse a pack from JSON text.
 53 |         ///
 54 |         /// # Errors
 55 |         /// Returns a string describing the first parsing or validation
 56 |         /// problem.
 57 |         pub fn from_json(text: &str) -> Result<Self, String> {
 58 |             let file: PackFile = serde_json::from_str(text).map_err(|e| e.to_string())?;
 59 |             if file.count != file.items.len() {
 60 |                 return Err(format!(
 61 |                     "count mismatch: declared {}, actual {}",
 62 |                     file.count,
 63 |                     file.items.len()
 64 |                 ));
 65 |             }
 66 |             let items = file
 67 |                 .items
 68 |                 .into_iter()
 69 |                 .enumerate()
 70 |                 .map(|(pos, item)| parse_item(pos, item))
 71 |                 .collect::<Result<Vec<_>, _>>()?;
 72 |             Ok(Self { items })
 73 |         }
 74 |
 75 |         /// Load a pack from a path relative to the workspace root.
 76 |         pub fn from_path(path: &str) -> Self {
 77 |             let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
 78 |             let full = manifest.join(path);
 79 |             let text = std::fs::read_to_string(&full)
 80 |                 .unwrap_or_else(|e| panic!("cannot read {}: {e}", full.display()));
 81 |             Self::from_json(&text).unwrap_or_else(|e| panic!("invalid {}: {e}", full.display()))
 82 |         }
 83 |     }
```

Serde mirror of the JSON format (lines 85–101):

```text
 85 |     #[derive(Debug, Deserialize)]
 86 |     struct PackFile {
 87 |         count: usize,
 88 |         items: Vec<PackItem>,
 89 |     }
 90 |
 91 |     #[derive(Debug, Deserialize)]
 92 |     struct PackItem {
 93 |         i: u32,
 94 |         a: String,
 95 |         #[serde(rename = "b")]
 96 |         black: Vec<[u8; 2]>,
 97 |         #[serde(rename = "w")]
 98 |         white: Vec<[u8; 2]>,
 99 |         #[serde(rename = "l", default)]
100 |         line: Vec<[u8; 3]>,
101 |     }
```

`parse_item` — attacker validation, duplicate/overlap checks, board
reconstruction, and the coordinate crossing (lines 103–174):

```text
103 |     fn parse_item(pos: usize, item: PackItem) -> Result<Puzzle, String> {
104 |         let attacker = match item.a.as_str() {
105 |             "black" => Color::Black,
106 |             "white" => Color::White,
107 |             other => return Err(format!("puzzle {} invalid attacker {other}", item.i)),
108 |         };
109 |         let black = parse_stones(item.i, &item.black)?;
110 |         let white = parse_stones(item.i, &item.white)?;
111 |
112 |         let mut black_seen = HashSet::new();
113 |         for mv in &black {
114 |             if !black_seen.insert(mv.index()) {
115 |                 return Err(format!("puzzle {} duplicate black stone", item.i));
116 |             }
117 |         }
118 |         let mut white_seen = HashSet::new();
119 |         for mv in &white {
120 |             if !white_seen.insert(mv.index()) {
121 |                 return Err(format!("puzzle {} duplicate white stone", item.i));
122 |             }
123 |             if black_seen.contains(&mv.index()) {
124 |                 return Err(format!("puzzle {} overlapping colors", item.i));
125 |             }
126 |         }
127 |
128 |         let board = Board::from_position(&black, &white, attacker).map_err(|e| match e {
129 |             PositionError::InvalidCounts => format!(
130 |                 "puzzle {} invalid counts (black={}, white={}, attacker={attacker:?})",
131 |                 item.i,
132 |                 black.len(),
133 |                 white.len()
134 |             ),
135 |             PositionError::OverlappingColors => {
136 |                 format!("puzzle {} overlapping colors", item.i)
137 |             }
138 |         })?;
139 |
140 |         let mut solution = Vec::with_capacity(item.line.len());
141 |         for (ply, &[x, y, side]) in item.line.iter().enumerate() {
142 |             let color = match side {
143 |                 1 => Color::Black,
144 |                 2 => Color::White,
145 |                 other => return Err(format!("puzzle {} invalid side {other}", item.i)),
146 |             };
147 |             if ply == 0 && color != attacker {
148 |                 return Err(format!(
149 |                     "puzzle {} line starts with {color:?}, expected {attacker:?}",
150 |                     item.i
151 |                 ));
152 |             }
153 |             let mv =
154 |                 Move::new(y, x).ok_or(format!("puzzle {} coord ({x},{y}) off board", item.i))?;
155 |             solution.push((mv, color));
156 |         }
157 |
158 |         Ok(Puzzle {
159 |             index: pos,
160 |             attacker,
161 |             board,
162 |             solution,
163 |         })
164 |     }
165 |
166 |     fn parse_stones(item: u32, coords: &[[u8; 2]]) -> Result<Vec<Move>, String> {
167 |         coords
168 |             .iter()
169 |             .map(|&[x, y]| {
170 |                 Move::new(y, x).ok_or(format!("puzzle {item} coord ({x},{y}) off board"))
171 |             })
172 |             .collect()
173 |     }
174 | }
```

The coordinate crossing happens at lines 152–153 for solution moves
and lines 168–169 for stones: file coordinates are `[x, y]`, so the
move is `Move::new(y, x)`.

## 3. Driver helpers

`Brain` and `random_move` are unchanged; the generic root-aware
`best_move` below is *hook-era quotation* — under the player-level
design there is no `SearchResult` to match, because the own probe
lives inside `AiPlayer::decide` (hook-era lines 182–215):

```text
182 | /// The brain seated at one side.
183 | enum Brain {
184 |     /// Uniform random legal moves.
185 |     Random,
186 |     /// Baseline evaluator with the given simulation budget.
187 |     Baseline(u32),
188 |     /// Tactics evaluator with the given simulation budget.
189 |     Tactics(u32),
190 | }
191 |
192 | fn random_move(board: &Board, rng: &mut StdRng) -> Move {
193 |     let legal: Vec<Move> = board.empty_moves().collect();
194 |     legal[rng.random_range(0..legal.len())]
195 | }
```

Intended player-level target: one `AiPlayer` per seat, built once over
the seat's evaluator, with `PlayerOptions` carrying the simulation
budget, `OwnTssConfig::baseline()` and `GuardConfig::baseline()` for
the baseline seat (both disabled for the tactics seat):

```rust
/// Player-level move helper.
///
/// The own TSS probe, the single `mcts::search`, and the
/// opponent-proof guard all run inside `decide`. A certified own
/// proof returns its move directly, with no search record.
fn best_move(
    player: &mut AiPlayer,
    board: &Board,
    options: &PlayerOptions,
    rng: &mut StdRng,
) -> Move {
    player.decide(board, options, rng).expect("game ongoing").mv
}
```

Thin wrappers for the two concrete evaluators (hook-era lines
217–233): under the player-level design these wrap the seat's
persistent `AiPlayer` instead of the bare evaluator.

```text
217 | fn baseline_best_move(
218 |     board: &Board,
219 |     simulations: u32,
220 |     rng: &mut StdRng,
221 |     ev: &mut BaselineEvaluator,
222 | ) -> Move {
223 |     best_move(board, simulations, rng, ev)
224 | }
225 |
226 | fn tactics_best_move(
227 |     board: &Board,
228 |     simulations: u32,
229 |     rng: &mut StdRng,
230 |     ev: &mut TacticsEvaluator,
231 | ) -> Move {
232 |     best_move(board, simulations, rng, ev)
233 | }
```

`BaselineMetrics` (lines 235–252):

```text
235 | /// Per-search configured-budget metrics for the baseline seat.
236 | ///
237 | /// * `searches` — how many times the baseline was asked to move.
238 | /// * `requested_sims` — sum of configured `simulations` arguments passed
239 | ///   to those searches. This is intentionally a budget-rate metric, NOT
240 | ///   a count of performed leaf evaluations. A root TSS proof may credit
241 | ///   the full configured budget while running zero simulations.
242 | /// * `search_time_secs` — wall-clock time spent inside
243 | ///   `baseline_best_move`, including any root TSS probe and all root
244 | ///   overhead. Therefore `requested_sims / search_time_secs` is a
245 | ///   configured-budget rate only; it is neither a lower bound nor a
246 | ///   measure of actual leaf-evaluation throughput.
247 | #[derive(Debug, Default)]
248 | struct BaselineMetrics {
249 |     searches: u64,
250 |     requested_sims: u64,
251 |     search_time_secs: f64,
252 | }
```

`play_game` (hook-era lines 254–292): the loop and metrics accounting
stand; under the player-level target the function holds one `AiPlayer`
per seat instead of a bare evaluator, and `baseline_best_move` takes
that player (the metrics' configured-budget semantics are unchanged —
a certified proof credits the full configured budget while running
zero simulations):

```text
254 | /// Play one game to completion.
255 | ///
256 | /// Returns `(status, plies, baseline_metrics)`.
257 | fn play_game(black: Brain, white: Brain, seed: u64) -> (Status, u32, BaselineMetrics) {
258 |     let mut board = Board::new();
259 |     let mut rng = StdRng::seed_from_u64(seed);
260 |     let mut plies = 0u32;
261 |     let mut baseline_ev = BaselineEvaluator::new(Device::flex());
262 |     let mut tactics_ev = TacticsEvaluator::new();
263 |     let mut metrics = BaselineMetrics::default();
264 |
265 |     while board.status() == Status::Ongoing && plies < 225 {
266 |         let side = board.to_move();
267 |         let brain = match side {
268 |             Color::Black => &black,
269 |             Color::White => &white,
270 |         };
271 |         let mv = match brain {
272 |             Brain::Random => random_move(&board, &mut rng),
273 |             Brain::Baseline(sims) => {
274 |                 let start = Instant::now();
275 |                 let mv = baseline_best_move(&board, *sims, &mut rng, &mut baseline_ev);
276 |                 metrics.search_time_secs += start.elapsed().as_secs_f64();
277 |                 metrics.searches += 1;
278 |                 metrics.requested_sims += u64::from(*sims);
279 |                 mv
280 |             }
281 |             Brain::Tactics(sims) => tactics_best_move(&board, *sims, &mut rng, &mut tactics_ev),
282 |         };
283 |         assert!(
284 |             board.is_legal(mv),
285 |             "illegal move {mv:?} at ply {plies} for {side:?}"
286 |         );
287 |         board.play(mv).unwrap();
288 |         plies += 1;
289 |     }
290 |
291 |     (board.status(), plies, metrics)
292 | }
```

## 4. Puzzle solver

Generic solver that indexes the defender reply by ply (hook-era lines
294–347; under the player-level target the attacker moves through the
seat's `AiPlayer`, so the own probe fires on the puzzle root):

```text
294 | /// Generic puzzle solver: the attacker uses `ev` through `best_move`,
295 | /// the defender follows the pack's solution line indexed by ply.
296 | ///
297 | /// Returns `(solved, defender_replies)` so that tests can assert the
298 | /// defender used successive distinct moves from the solution line.
299 | fn solve_puzzle_with<E: Evaluator>(
300 |     puzzle: &Puzzle,
301 |     simulations: u32,
302 |     ev: &mut E,
303 |     rng: &mut StdRng,
304 | ) -> (bool, Vec<Move>) {
305 |     let mut board = puzzle.board.clone();
306 |     let mut defender_replies = Vec::new();
307 |     let max_plies = puzzle.solution.len() as u32 + 4;
308 |     let attacker = puzzle.attacker;
309 |
310 |     for ply in 0..max_plies {
311 |         match board.status() {
312 |             Status::Won(c) if c == attacker => return (true, defender_replies),
313 |             Status::Won(_) | Status::Draw => return (false, defender_replies),
314 |             Status::Ongoing => {}
315 |         }
316 |
317 |         let side = board.to_move();
318 |         let mv = if side == attacker {
319 |             // The generic helper returns a certified win when the root
320 |             // TSS probe finds one, otherwise the MCTS best move.
321 |             best_move(&board, simulations, rng, ev)
322 |         } else {
323 |             // Follow the pack's claimed defender move for this ply.
324 |             // The solution is indexed by ply from the puzzle root; the
325 |             // color guard keeps us from replaying an attacker move when
326 |             // the line is shorter than expected.
327 |             let solution_mv = puzzle
328 |                 .solution
329 |                 .get(ply as usize)
330 |                 .filter(|(_, c)| *c == side)
331 |                 .map(|(mv, _)| *mv);
332 |             solution_mv.unwrap_or_else(|| random_move(&board, rng))
333 |         };
334 |
335 |         if side != attacker {
336 |             defender_replies.push(mv);
337 |         }
338 |
339 |         if !board.is_legal(mv) {
340 |             return (false, defender_replies);
341 |         }
342 |         board.play(mv).unwrap();
343 |     }
344 |
345 |     let solved = matches!(board.status(), Status::Won(c) if c == attacker);
346 |     (solved, defender_replies)
347 | }
```

Production wrapper (lines 349–353):

```text
349 | fn solve_puzzle(puzzle: &Puzzle, simulations: u32, seed: u64) -> bool {
350 |     let mut rng = StdRng::seed_from_u64(seed);
351 |     let mut ev = BaselineEvaluator::new(Device::flex());
352 |     solve_puzzle_with(puzzle, simulations, &mut ev, &mut rng).0
353 | }
```

## 5. Tactical asserts

Win-in-one at 50 simulations (lines 359–390):

```text
359 | #[test]
360 | fn win_in_one_is_solved_at_50_sims() {
361 |     let b = board_from_ascii(
362 |         "
363 |         O . O . O . O . . . . . . . .
364 |         . . . . . . . . . . . . . . .
365 |         . . . . . . . . . . . . . . .
366 |         . . . . . . . . . . . . . . .
367 |         . . . . . . . . . . . . . . .
368 |         . . . . . . . . . . . . . . .
369 |         . . . . . . . . . . . . . . .
370 |         . . . . X X X X . . . . . . .
371 |         . . . . . . . . . . . . . . .
372 |         . . . . . . . . . . . . . . .
373 |         . . . . . . . . . . . . . . .
374 |         . . . . . . . . . . . . . . .
375 |         . . . . . . . . . . . . . . .
376 |         . . . . . . . . . . . . . . .
377 |         . . . . . . . . . . . . . . .
378 |         ",
379 |     );
380 |     assert_eq!(b.to_move(), Color::Black);
381 |
382 |     let mut rng = StdRng::seed_from_u64(1);
383 |     let mut ev = BaselineEvaluator::new(Device::flex());
384 |     let mv = baseline_best_move(&b, 50, &mut rng, &mut ev);
385 |     let wins = engine::immediate_wins(&b, Color::Black);
386 |     assert!(
387 |         wins.contains(mv),
388 |         "best move {mv:?} must be an immediate win"
389 |     );
390 | }
```

Forced block at 50 simulations (lines 392–419):

```text
392 | #[test]
393 | fn forced_block_is_taken_at_50_sims() {
394 |     let mut b = Board::new();
395 |     let script = [
396 |         (7, 2),
397 |         (7, 3),
398 |         (0, 0),
399 |         (7, 4),
400 |         (0, 1),
401 |         (7, 5),
402 |         (0, 2),
403 |         (7, 6),
404 |     ];
405 |     for &(r, c) in &script {
406 |         b.play(Move::new(r, c).unwrap()).unwrap();
407 |     }
408 |     assert_eq!(b.to_move(), Color::Black);
409 |     let blocks = engine::forced_blocks(&b);
410 |     assert_eq!(blocks.len(), 1, "White must have exactly one winning cell");
411 |
412 |     let mut rng = StdRng::seed_from_u64(2);
413 |     let mut ev = BaselineEvaluator::new(Device::flex());
414 |     let mv = baseline_best_move(&b, 50, &mut rng, &mut ev);
415 |     assert!(
416 |         blocks.contains(mv),
417 |         "best move {mv:?} must be the forced block"
418 |     );
419 | }
```

## 6. Self-play flood

Lines 425–442:

```text
425 | #[test]
426 | fn fifty_seeded_self_play_games_are_legal() {
427 |     // Generic MCTS health flood: exercises the search/evaluator plumbing
428 |     // for 50 full games without panics. The baseline evaluator is too
429 |     // slow in debug for a 50-game flood, so the fast TacticsEvaluator is
430 |     // used here; baseline behavior is covered by the tactical asserts
431 |     // above and the release-only strength ladder measurement below.
432 |     let mut rng = StdRng::seed_from_u64(2026);
433 |     for game in 0..50 {
434 |         let seed = rng.random::<u64>();
435 |         let (status, plies, _) = play_game(Brain::Tactics(1), Brain::Tactics(1), seed);
436 |         assert!(
437 |             status != Status::Ongoing || plies == 225,
438 |             "game {game} did not terminate"
439 |         );
440 |         assert!(plies <= 225, "game {game} exceeded the maximal move count");
441 |     }
442 | }
```

## 7. Decorator-forwarding regression — retired

*Hook-era quotation removed.* The test
`decorated_baseline_forwards_evaluate_root_and_bypasses_leaf_eval`
shielded the contract "a wrapper must forward *both* `evaluate` and
`evaluate_root`". The root hook was retired before landing
(2026-11-06): there is no `evaluate_root` to forward, and no wrapper
around the one-method `Evaluator` can strip root certification,
because certification lives in the `player` crate's `decide`, outside
the evaluator. The probe's behavior is pinned by the `player` crate's
own suite (sub-chapter 08), so this section intentionally specifies no
replacement test.

## 8. Pack parser sanity

Lines 530–558:

```text
530 | #[test]
531 | fn kaibao_pack_parses_with_expected_count() {
532 |     let pack = PuzzlePack::from_path("../../../data/puzzles/kaibao-vcf-676.json");
533 |     assert_eq!(pack.items.len(), 676);
534 |     assert!(pack.items.iter().all(|p| !p.solution.is_empty()));
535 | }
536 |
537 | #[test]
538 | fn all_vcf_packs_parse_and_have_nonempty_lines() {
539 |     let packs = [
540 |         ("kaibao-vcf-676.json", 676usize),
541 |         ("renju-portal-vcf-783-1of4.json", 783),
542 |         ("renju-portal-vcf-783-2of4.json", 783),
543 |         ("renju-portal-vcf-783-3of4.json", 783),
544 |         ("renju-portal-vcf-783-4of4.json", 783),
545 |         ("vcf-material-763.json", 763),
546 |     ];
547 |     for (name, expected) in packs {
548 |         let pack = PuzzlePack::from_path(&format!("../../../data/puzzles/{name}"));
549 |         assert_eq!(
550 |             pack.items.len(),
551 |             expected,
552 |             "{name} should contain {expected} puzzles"
553 |         );
554 |         assert!(
555 |             pack.items.iter().all(|p| !p.solution.is_empty()),
556 |             "{name} contains puzzles with empty solution lines"
557 |         );
558 |     }
559 | }
```

## 9. Successive-defender-replies regression

A deterministic `SolutionFollowingEvaluator` forces the attacker to
follow the claimed line so the test isolates the defender branch
(hook-era lines 565–654). The quote's `evaluate_root` override is
retired with the hook — the trait is one-method again; when this
regression is rebuilt through `AiPlayer`, construct the player with
`OwnTssConfig::disabled()` so the deterministic policy, not the real
prover, drives the attacker.

```text
565 | /// Deterministic evaluator that forces the attacker to follow the
566 | /// puzzle's claimed solution line. Used only to make the defender-reply
567 | /// regression independent of baseline search variability.
568 | struct SolutionFollowingEvaluator<'a> {
569 |     puzzle: &'a Puzzle,
570 |     puzzle_empty: usize,
571 | }
572 |
573 | impl<'a> SolutionFollowingEvaluator<'a> {
574 |     fn new(puzzle: &'a Puzzle) -> Self {
575 |         Self {
576 |             puzzle,
577 |             puzzle_empty: puzzle.board.empty_moves().count(),
578 |         }
579 |     }
580 |
581 |     fn policy_for(&self, req: &EvalRequest) -> EvalResult {
582 |         let board = adapter::reconstruct_board(&req.planes, &req.legal);
583 |         let side = board.to_move();
584 |         let ply = self.puzzle_empty.saturating_sub(req.legal.len());
585 |
586 |         let mut policy = [f32::NEG_INFINITY; 225];
587 |         if side == self.puzzle.attacker
588 |             && let Some((target, color)) = self.puzzle.solution.get(ply)
589 |             && *color == side
590 |             && req.legal.contains(target)
591 |         {
592 |             policy[target.index()] = 1.0;
593 |             return EvalResult { policy, value: 0.0 };
594 |         }
595 |         for mv in &req.legal {
596 |             policy[mv.index()] = 0.0;
597 |         }
598 |         EvalResult { policy, value: 0.0 }
599 |     }
600 | }
601 |
602 | impl Evaluator for SolutionFollowingEvaluator<'_> {
603 |     fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
604 |         self.policy_for(&req)
605 |     }
606 |
607 |     fn evaluate_root(&mut self, req: EvalRequest) -> RootEval {
608 |         RootEval::Estimate(self.policy_for(&req))
609 |     }
610 | }
611 |
612 | #[test]
613 | fn solve_puzzle_follows_successive_defender_replies() {
614 |     // Regression for the chapter-09 restore bug: the defender branch
615 |     // used `find(|(_, c)| c == side)`, which replayed the *first*
616 |     // defender move forever. This puzzle's solution line contains
617 |     // three distinct White defender replies; with the bug the second
618 |     // White turn tries to replay an occupied cell and solve_puzzle
619 |     // returns false.
620 |     let pack = PuzzlePack::from_path("../../../data/puzzles/vcf-material-763.json");
621 |     let puzzle = pack
622 |         .items
623 |         .iter()
624 |         .find(|p| p.index == 7)
625 |         .expect("vcf-material puzzle 18 should be at pack index 7");
626 |
627 |     let defender_moves: Vec<Move> = puzzle
628 |         .solution
629 |         .iter()
630 |         .filter(|(_, c)| *c != puzzle.attacker)
631 |         .map(|(mv, _)| *mv)
632 |         .collect();
633 |     assert!(
634 |         defender_moves.len() >= 2,
635 |         "regression needs at least two distinct defender replies"
636 |     );
637 |     assert!(
638 |         defender_moves.iter().any(|mv| *mv != defender_moves[0]),
639 |         "defender replies must not all be identical"
640 |     );
641 |
642 |     let mut ev = SolutionFollowingEvaluator::new(puzzle);
643 |     let mut rng = StdRng::seed_from_u64(1);
644 |     let (_solved, replies) = solve_puzzle_with(puzzle, 1, &mut ev, &mut rng);
645 |
646 |     assert!(
647 |         replies.len() >= 2,
648 |         "solver should reach at least two defender replies, got {replies:?}"
649 |     );
650 |     assert!(
651 |         replies.iter().any(|mv| *mv != replies[0]),
652 |         "solver should use successive distinct defender replies, got {replies:?}"
653 |     );
654 | }
```

## 10. Preserved ignored A/B: quiet-whisper measurement

Sub-chapter 07's measurement is preserved here, still `#[ignore]`d
(lines 660–744):

```text
660 | /// Run explicitly in release:
661 | /// `cargo test -p baseline --release --test acceptance quiet_whisper_ab_measured -- --ignored --nocapture`
662 | ///
663 | /// Protocol: 20 games vs the `TacticsEvaluator` at 200 simulations with
664 | /// alternating colors. One arm uses the default whisper-on evaluator,
665 | /// the other uses `.without_quiet_value()`. Base seed 20_260_901.
666 | ///
667 | /// The test does not assert a win-rate bar: the mock is expected to be
668 | /// too weak to discriminate. The measurement is the result.
669 | #[ignore]
670 | #[test]
671 | fn quiet_whisper_ab_measured() {
672 |     const GAMES: usize = 20;
673 |     const SIMS: u32 = 200;
674 |     const BASE_SEED: u64 = 20_260_901;
675 |
676 |     fn play_game_ab(baseline_color: Color, quiet: bool, seed: u64) -> (Status, u32, f64) {
677 |         let mut board = Board::new();
678 |         let mut rng = StdRng::seed_from_u64(seed);
679 |         let mut plies = 0u32;
680 |         let start = Instant::now();
681 |         let mut baseline_ev = if quiet {
682 |             BaselineEvaluator::new(Device::flex())
683 |         } else {
684 |             BaselineEvaluator::new(Device::flex()).without_quiet_value()
685 |         };
686 |         let mut tactics_ev = TacticsEvaluator::new();
687 |
688 |         while board.status() == Status::Ongoing && plies < 225 {
689 |             let side = board.to_move();
690 |             let mv = if side == baseline_color {
691 |                 baseline_best_move(&board, SIMS, &mut rng, &mut baseline_ev)
692 |             } else {
693 |                 tactics_best_move(&board, SIMS, &mut rng, &mut tactics_ev)
694 |             };
695 |             assert!(
696 |                 board.is_legal(mv),
697 |                 "illegal move {mv:?} at ply {plies} for {side:?}"
698 |             );
699 |             board.play(mv).unwrap();
700 |             plies += 1;
701 |         }
702 |         (board.status(), plies, start.elapsed().as_secs_f64())
703 |     }
704 |
705 |     println!("\n=== quiet whisper A/B ===\n");
706 |     println!(
707 |         "{:<12} {:>6} {:>6} {:>6} {:>10} {:>12}",
708 |         "arm", "wins", "losses", "draws", "avg_plies", "avg_s/game"
709 |     );
710 |     for quiet in [true, false] {
711 |         let label = if quiet { "whisper ON" } else { "whisper OFF" };
712 |         let mut wins = 0usize;
713 |         let mut losses = 0usize;
714 |         let mut draws = 0usize;
715 |         let mut total_plies = 0u32;
716 |         let mut total_time = 0.0f64;
717 |         for g in 0..GAMES {
718 |             let baseline_color = if g % 2 == 0 {
719 |                 Color::Black
720 |             } else {
721 |                 Color::White
722 |             };
723 |             let seed = BASE_SEED + g as u64;
724 |             let (status, plies, secs) = play_game_ab(baseline_color, quiet, seed);
725 |             total_plies += plies;
726 |             total_time += secs;
727 |             match status {
728 |                 Status::Won(c) if c == baseline_color => wins += 1,
729 |                 Status::Won(_) => losses += 1,
730 |                 Status::Draw => draws += 1,
731 |                 Status::Ongoing => panic!("game {g} did not terminate"),
732 |             }
733 |         }
734 |         println!(
735 |             "{:<12} {:>6} {:>6} {:>6} {:>10.1} {:>12.3}",
736 |             label,
737 |             wins,
738 |             losses,
739 |             draws,
740 |             total_plies as f64 / GAMES as f64,
741 |             total_time / GAMES as f64
742 |         );
743 |     }
744 | }
```

## 11. Strength-ladder measurement

This is the `#[ignore]`d measurement. Its doc comment states the
metric semantics explicitly (lines 750–775):

```text
750 | /// Run explicitly in release:
751 | /// `cargo test -p baseline --release --test acceptance -- --ignored --nocapture`
752 | ///
753 | /// Protocol: 10 games per level against the uniform-random driver,
754 | /// a 20-game match against the `TacticsEvaluator` at 200 sims with
755 | /// alternating colors, and a seeded sample of 100 engine-proven VCF
756 | /// puzzles. A full run takes roughly two hours in release on a fast
757 | /// desktop — it is a measurement, not a gate.
758 | ///
759 | /// Historical note: the old CLI-probe baseline reported ~599
760 | /// "evals/sec", which was actually the configured-budget rate
761 | /// `requested_sims / search_wall_time`. The player-level own TSS
762 | /// probe performs extra root work that is included in wall time, and
763 | /// a certified proof may credit the full configured simulation
764 | /// budget while running zero leaf evaluations. The numbers below are
765 | /// therefore
766 | /// labelled "configured budget rate (requested_sims/search_wall_time)"
767 | /// and are explicitly NOT a lower bound or a measure of actual
768 | /// leaf-evaluation throughput. The old 599 figure is preserved for
769 | /// historical comparison only and is not reproduced here.
769 | #[ignore]
770 | #[test]
771 | fn strength_ladder_measured() {
772 |     const RANDOM_GAMES: usize = 10;
773 |     const MOCK_GAMES: usize = 20;
774 |     const VCF_SAMPLE: usize = 100;
775 |     const BASE_SEED: u64 = 20_260_801;
```

Sims gradient vs the random driver (lines 806–834):

```text
806 |     // 2. Sims gradient vs uniform-random driver.
807 |     println!("vs uniform-random driver (baseline Black, random White)");
808 |     println!(
809 |         "{:<8} {:>6} {:>6} {:>6} {:>8}",
810 |         "sims", "wins", "losses", "draws", "win_rate"
811 |     );
812 |     for sims in [400, 4_000, 10_000] {
813 |         let mut wins = 0;
814 |         let mut losses = 0;
815 |         let mut draws = 0;
816 |         for g in 0..RANDOM_GAMES {
817 |             let seed = BASE_SEED + g as u64 + sims as u64 * 10_000;
818 |             match play_game(Brain::Baseline(sims), Brain::Random, seed).0 {
819 |                 Status::Won(Color::Black) => wins += 1,
820 |                 Status::Won(Color::White) => losses += 1,
821 |                 Status::Draw => draws += 1,
822 |                 Status::Ongoing => panic!("game did not terminate"),
823 |             }
824 |         }
825 |         let rate = wins as f64 / RANDOM_GAMES as f64;
826 |         println!(
827 |             "{:<8} {:>6} {:>6} {:>6} {:>7.0}%",
828 |             sims,
829 |             wins,
830 |             losses,
831 |             draws,
832 |             rate * 100.0
833 |         );
834 |     }
```

Match vs `TacticsEvaluator`, with the honest configured-budget rate
(lines 836–895):

```text
836 |     // 3. Match vs TacticsEvaluator at 200 sims, colors alternate by seed parity.
837 |     println!("\nvs TacticsEvaluator at 200 sims");
838 |     println!(
839 |         "{:<8} {:>6} {:>6} {:>6} {:>8}",
840 |         "games", "wins", "losses", "draws", "win_rate"
841 |     );
842 |     {
843 |         let mut wins = 0;
844 |         let mut losses = 0;
845 |         let mut draws = 0;
846 |         let mut total_requested_sims = 0u64;
847 |         let mut total_search_time = 0.0f64;
848 |         let mut total_game_time = 0.0f64;
849 |         for g in 0..MOCK_GAMES {
850 |             let seed = BASE_SEED + g as u64 + 200 * 100_000;
851 |             let (black, white) = if g % 2 == 0 {
852 |                 (Brain::Baseline(200), Brain::Tactics(200))
853 |             } else {
854 |                 (Brain::Tactics(200), Brain::Baseline(200))
855 |             };
856 |             let start = Instant::now();
857 |             let (status, _, metrics) = play_game(black, white, seed);
858 |             total_game_time += start.elapsed().as_secs_f64();
859 |             total_requested_sims += metrics.requested_sims;
860 |             total_search_time += metrics.search_time_secs;
861 |
862 |             let baseline_won = match status {
863 |                 Status::Won(Color::Black) => g % 2 == 0,
864 |                 Status::Won(Color::White) => g % 2 == 1,
865 |                 Status::Draw => {
866 |                     draws += 1;
867 |                     continue;
868 |                 }
869 |                 Status::Ongoing => panic!("game did not terminate"),
870 |             };
871 |             if baseline_won {
872 |                 wins += 1;
873 |             } else {
874 |                 losses += 1;
875 |             }
876 |         }
877 |         let rate = wins as f64 / MOCK_GAMES as f64;
878 |         println!(
879 |             "{:<8} {:>6} {:>6} {:>6} {:>7.0}%",
880 |             MOCK_GAMES,
881 |             wins,
882 |             losses,
883 |             draws,
884 |             rate * 100.0
885 |         );
886 |         println!(
887 |             "  seconds/game: {:.3}, configured_budget_rate: {:.0} requested_sims/search_wall_time (not leaf-eval throughput; root proof may credit budget without running simulations; root overhead included in wall time)",
888 |             total_game_time / MOCK_GAMES as f64,
889 |             total_requested_sims as f64 / total_search_time
890 |         );
891 |         assert!(
892 |             rate >= 0.50,
893 |             "baseline must win at least 50% against TacticsEvaluator at 200 sims"
894 |         );
895 |     }
```

VCF solve-rate sample (lines 897–958):

```text
897 |     // 4. VCF solve rate.
898 |     println!("\nVCF solve rate");
899 |     {
900 |         let kaibao = PuzzlePack::from_path("../../../data/puzzles/kaibao-vcf-676.json");
901 |         let shards = [
902 |             PuzzlePack::from_path("../../../data/puzzles/renju-portal-vcf-783-1of4.json"),
903 |             PuzzlePack::from_path("../../../data/puzzles/renju-portal-vcf-783-2of4.json"),
904 |             PuzzlePack::from_path("../../../data/puzzles/renju-portal-vcf-783-3of4.json"),
905 |             PuzzlePack::from_path("../../../data/puzzles/renju-portal-vcf-783-4of4.json"),
906 |         ];
907 |
908 |         // Seeded sample of VCF_SAMPLE puzzles across all packs.
909 |         let mut all: Vec<(String, usize)> = Vec::new();
910 |         for (name, pack) in [(String::from("kaibao"), &kaibao)].into_iter().chain(
911 |             shards
912 |                 .iter()
913 |                 .enumerate()
914 |                 .map(|(i, p)| (format!("renju-{}", i + 1), p)),
915 |         ) {
916 |             for idx in 0..pack.items.len() {
917 |                 all.push((name.clone(), idx));
918 |             }
919 |         }
920 |
921 |         let mut rng = StdRng::seed_from_u64(BASE_SEED);
922 |         // Fisher-Yates shuffle a prefix sample.
923 |         for i in 0..all.len().min(VCF_SAMPLE) {
924 |             let j = rng.random_range(i..all.len());
925 |             all.swap(i, j);
926 |         }
927 |         all.truncate(VCF_SAMPLE);
928 |
929 |         let mut solved = 0;
930 |         let vcf_start = Instant::now();
931 |         for (idx, (name, puzzle_idx)) in all.iter().enumerate() {
932 |             let pack = if name == "kaibao" {
933 |                 &kaibao
934 |             } else {
935 |                 let shard: u32 = name.strip_prefix("renju-").unwrap().parse().unwrap();
936 |                 &shards[shard as usize - 1]
937 |             };
938 |             let puzzle = &pack.items[*puzzle_idx];
939 |             let seed = BASE_SEED + idx as u64;
940 |             if solve_puzzle(puzzle, 4_000, seed) {
941 |                 solved += 1;
942 |             }
943 |             if (idx + 1) % 10 == 0 {
944 |                 println!(
945 |                     "  VCF progress: {}/{} solved in {:.1}s",
946 |                     solved,
947 |                     idx + 1,
948 |                     vcf_start.elapsed().as_secs_f64()
949 |                 );
950 |             }
951 |         }
952 |         println!("solved {solved}/{VCF_SAMPLE}");
953 |         assert!(
954 |             solved >= VCF_SAMPLE / 2,
955 |             "baseline must solve at least half of the {VCF_SAMPLE} VCF puzzles"
956 |         );
957 |     }
958 | }
```

## 12. The flip

The proposed default-brain change is one line in
`gomoku/crates/cli/src/main.rs`:

```rust
        evaluator: search.ai.map(Into::into).unwrap_or(EvaluatorKind::Baseline),
```

This line is not yet verified in the current main source or the
reference scratch. It remains a proposed landing action. The mock
stays available as `--ai tactics`.

## Notes

* `baseline::probe` does not exist in any current design; do not call
  it from the acceptance harness. The retired hook-era API
  (`RootEval` / `evaluate_root` / `SearchResult`) does not exist
  either: the proof lives in the `player` crate's `AiPlayer::decide`,
  and `mcts::search` returns `SearchOutcome`.
* The `vcf-material-763.json` pack was added to the VCF sample; the
  total puzzle count across all packs is 4,571.
* The strength-ladder measurement is `#[ignore]`; the tactical asserts
  and the flood run by default.
* All measured numbers are historical unless you rerun the ladder
  yourself; the old `599 evals/sec` label is preserved only as a
  configured-budget-rate proxy and is explicitly not comparable to
  leaf-evaluation throughput under the player-level proof path.
