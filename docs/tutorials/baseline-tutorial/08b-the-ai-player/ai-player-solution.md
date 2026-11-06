> **Opt-in complete solution — read after attempting the chapter, or after being stuck for twenty minutes.**
>
> This is an inspectable reference implementation, not authority over the learner's own code. The artifacts below are sufficient to reconstruct the chapter-08b changes on top of chapter 08. No learner source is changed by this paper.

# Sub-chapter 08b — The AI player: complete solution

## Abstract

A persistent `AiPlayer` owns one evaluator per AI seat and answers a move-decision request without changing the session board. Terminal inputs fail before evaluation or randomness. Certified root wins return a move with **no search statistics**. Ordinary searches preserve the existing search, post-search prior noise, and temperature-selection order. An optional candidate guard then rejects only moves with verified opponent winning continuations; an accepted nonterminal move remains unknown, not safe. This paper supplies every new player source and test body, the CLI changes relative to chapter 08, and the lifecycle and seeded regression tests. Execution stays synchronous and each decision builds a fresh search tree.

## Glossary

| Term | Meaning in this solution |
|------|--------------------------|
| AI seat | Black or White configured to produce moves automatically. |
| CLI | Command-line interface; its session owns history, events, configuration, and randomness. |
| MCTS | Monte Carlo tree search; simulations accumulate root-edge visits and values. |
| Evaluator | The existing `mcts::Evaluator` interface for static estimates and optional root certification. |
| Root proof | `SearchResult::ProvenWin`, supplied by the evaluator's root hook before simulations. |
| Proposal | The ordinary temperature-selected move, before defensive checking. |
| Prior | Policy probability on a root edge, not its visit count. |
| RNG | Random-number generator; the session owns it and lends it to a decision. |
| TSS | Threat-space search, the engine's bounded prover of forcing winning continuations. |
| Refuted | A candidate permits an opponent proof with the correct winner, legal first move, and verified line. |
| Unrefuted | No accepted opponent proof within the bounds: **unknown**, never certified safe. |
| Candidate cap | Maximum hypothetical candidate checks; distinct from each prover's node/depth budget. |
| `[experiment]` | A local tuning choice, not a theorem, strength measurement, or latency guarantee. |

## Provenance, starting point, and verification scope

All numbered source quotations are from the uncommitted **detached** worktree
`../rust-ml-ai-player-ref/gomoku`, whose committed base is `2b6ec75`.
Its working files contain the chapter-08 root-hook overlay plus chapter 08b.
Line numbers are that reference's line numbers, not main's. The comparison
starting point is **`../rust-ml-tss-root-doc-ref/gomoku`**, also at base
`2b6ec75` with the chapter-08 overlay. Comparing against main HEAD would
incorrectly attribute the earlier evaluator/MCTS/root-hook changes to 08b.

Chapter 08 already supplies `RootEval`, `Evaluator::evaluate_root`,
`SearchResult`, the guarded baseline root hook, and its tests. Those remain
unchanged here. See the [chapter-08 complete solution][chapter08-solution]
for that prerequisite. This paper quotes implementation, not raw diffs, and
does not require either reference worktree to remain available for copying.

**Fresh reference QA:** `cargo test --release --locked --all-targets`
exited 0: **574 passed, 0 failed, 6 ignored** across the workspace (42 s
including compilation; 4 s cached). This includes **21 player tests**
(5 private guard + 4 public decision + 12 public guard) and all ten actor
tests, including both real-baseline cases excluded in the earlier filtered
run. `cargo clippy --locked --all-targets -- -D warnings` exited 0 (9 s);
`cargo fmt --all -- --check` and `git diff --check` also exited 0. The six
ignored tests are preexisting: two baseline acceptance, one CLI acceptance,
and three patterns differential tests. Reference verification does not land
learner source on main. No interactive-play or measured guard strength/latency
claim follows. Both design Mermaid fences now parse cleanly under
mermaid 12.3.0 with jsdom (`mermaid.parse`: sequenceDiagram → `sequence`,
stateDiagram-v2 → `stateDiagram`, no errors); syntactic parsing only, no
browser/render validation.
The distinct-line regression below is additional to the older two-ended-line
test; the current inventory is 21, not the earlier draft's 20.

### Reading and reconstruction rules

The order follows a move: seat configuration and player initialization;
terminal/proof/search decision; statistics; candidate admission; CLI commit
and lifecycle; then executable specifications along the same path.

For new Rust files, collect their numbered fragments in increasing source-line
order, removing the `number |` margin. The fragments include all nonblank
source lines, attributes, imports, helper bodies, and closing braces. Some
fragments deliberately continue an open `impl`, function, or test module;
the explanation names that context. Blank separators may be restored freely.
For existing CLI files, insert the new method/fields or replace the named
method, leaving unmentioned chapter-08 code and tests intact. Existing context
is explicitly identified; it is not another implementation to append.

| Artifact | Action relative to chapter 08 |
|----------|-------------------------------|
| `gomoku/Cargo.toml` | Add the player workspace member. |
| `gomoku/crates/player/Cargo.toml` | Create the independent player package. |
| `gomoku/crates/player/src/lib.rs` | Create all public player API and decision/statistics logic. |
| `gomoku/crates/player/src/guard.rs` | Create the private guard module, public guard data, and five private tests. |
| `gomoku/crates/player/tests/decisions.rs` | Create all four public decision tests. |
| `gomoku/crates/player/tests/guard.rs` | Create all twelve real-prover guard tests. |
| `gomoku/crates/cli/Cargo.toml` | Add the player path dependency. |
| `gomoku/crates/cli/src/actor.rs` | Add `SearchInfo::from_stats`; keep `from_searched`. |
| `gomoku/crates/cli/src/session.rs` | Add runtime slots, initialize/replace them, route decisions, add four unit tests and their doubles. |
| `gomoku/crates/cli/tests/actor.rs` | Add the eleven-ply seeded legacy-path regression. |
| `gomoku/Cargo.lock` | Cargo records the new local package and CLI dependency. |

No engine, MCTS, baseline, pattern, command, history, rendering, or testkit
source change is part of this chapter.

## 1. Seat configuration and player initialization

### Register the dependency boundary

The player must not depend on the concrete baseline evaluator, Burn, patterns,
or the CLI. The CLI constructs the evaluator, transfers its ownership to the
player, and continues owning the board/history and RNG. The engine remains
unchanged and dependency-isolated.

At workspace resolution, replace the existing member declaration with this one. Only `crates/player` is new; resolver and earlier members stay as they were.

**Source:** `gomoku/Cargo.toml`, lines 4–7.

```text
  4 | [workspace]
  5 | resolver = "3"
  6 | members = ["crates/engine", "crates/cli", "crates/mcts", "crates/train", "crates/patterns",
  7 |     "crates/testkit", "crates/baseline", "crates/player"]
```

Create the player package with inherited workspace metadata and exactly these three dependencies. The small manifest is complete; the production and integration tests need no additional dev-dependency.

**Source:** `gomoku/crates/player/Cargo.toml`, lines 1–10.

```text
  1 | [package]
  2 | name = "player"
  3 | version.workspace = true
  4 | edition.workspace = true
  5 | license.workspace = true
  6 |
  7 | [dependencies]
  8 | engine = { path = "../engine" }
  9 | mcts = { path = "../mcts" }
 10 | rand = "0.10.3"
```

In the existing CLI dependency table, add `player` beside engine and MCTS. These surrounding entries are context, not a second dependency table; keep all other CLI dependencies and features unchanged.

**Source:** `gomoku/crates/cli/Cargo.toml`, lines 25–28.

```text
 25 | [dependencies]
 26 | engine = { path = "../engine" }
 27 | mcts = { path = "../mcts" }
 28 | player = { path = "../player" }
```

Cargo updates the lockfile when it resolves the new local package; no dependency upgrade is intended. For an exact chapter-08b accounting, the only lockfile changes are the CLI's `player` dependency and the new local package record.

The existing CLI package record gains the player dependency during resolution. This is the resulting record, not an instruction to duplicate the package in the lockfile.

**Source:** `gomoku/Cargo.lock`, lines 876–892.

```text
876 | [[package]]
877 | name = "cli"
878 | version = "0.1.0"
879 | dependencies = [
880 |  "baseline",
881 |  "clap",
882 |  "crossterm",
883 |  "engine",
884 |  "mcts",
885 |  "player",
886 |  "proptest",
887 |  "rand 0.10.3",
888 |  "serde",
889 |  "serde_json",
890 |  "testkit",
891 |  "thiserror 2.0.20",
892 | ]
```

The new local player record names the same three manifest dependencies. Cargo generates this package entry; no registry source or checksum belongs on a local workspace package.

**Source:** `gomoku/Cargo.lock`, lines 3144–3151.

```text
3144 | [[package]]
3145 | name = "player"
3146 | version = "0.1.0"
3147 | dependencies = [
3148 |  "engine",
3149 |  "mcts",
3150 |  "rand 0.10.3",
3151 | ]
```

### Define move-local settings without storing a tree

Begin the new library with its documentation gate, imports, private guard module, and public re-exports. Callers see the actual names `GuardConfig`, `GuardDiagnostics`, and `GuardOutcome`; they cannot call the private resolver.

**Source:** `gomoku/crates/player/src/lib.rs`, lines 1–14.

```text
  1 | //! Reusable AI move decisions, independent of any user interface.
  2 | //!
  3 | //! An [`AiPlayer`] retains its evaluator across decisions. Each decision runs
  4 | //! a fresh MCTS search; certified root wins bypass sampling and carry no
  5 | //! fabricated search statistics.
  6 |
  7 | #![deny(missing_docs)]
  8 |
  9 | use engine::{Board, Color, Move, Status};
 10 | use mcts::{Evaluator, SearchConfig, SearchResult};
 11 | use rand::Rng;
 12 |
 13 | mod guard;
 14 | pub use guard::{GuardConfig, GuardDiagnostics, GuardOutcome};
```

For each decision, the caller supplies current settings through `PlayerOptions`. Defaults use MCTS defaults, deterministic selection, no noise, and a disabled guard. Changing options does not reconstruct the evaluator.

**Source:** `gomoku/crates/player/src/lib.rs`, lines 16–45.

```text
 16 | /// Search and move-selection settings for one decision.
 17 | #[derive(Debug, Clone, Copy, PartialEq)]
 18 | pub struct PlayerOptions {
 19 |     /// Number of MCTS simulations.
 20 |     pub simulations: u32,
 21 |     /// PUCT exploration constant.
 22 |     pub c_puct: f32,
 23 |     /// Visit-count sampling temperature; zero selects the most-visited move.
 24 |     pub temperature: f32,
 25 |     /// Dirichlet noise mix fraction; zero disables noise and its RNG draws.
 26 |     pub noise_epsilon: f32,
 27 |     /// Positive Dirichlet concentration parameter when noise is enabled.
 28 |     pub noise_alpha: f32,
 29 |     /// Optional [experiment] defensive refutation guard; disabled by default.
 30 |     pub guard: GuardConfig,
 31 | }
 32 |
 33 | impl Default for PlayerOptions {
 34 |     fn default() -> Self {
 35 |         let search = SearchConfig::default();
 36 |         Self {
 37 |             simulations: search.simulations,
 38 |             c_puct: search.c_puct,
 39 |             temperature: 0.0,
 40 |             noise_epsilon: 0.0,
 41 |             noise_alpha: 0.3,
 42 |             guard: GuardConfig::disabled(),
 43 |         }
 44 |     }
 45 | }
```

At player initialization, `AiPlayer` takes ownership of a boxed evaluator. This fragment opens `impl AiPlayer`; the later `decide` fragment completes it. There is intentionally no tree field, RNG field, or session reference.

**Source:** `gomoku/crates/player/src/lib.rs`, lines 116–126.

```text
116 | /// A reusable player owning one persistent evaluator, with a fresh tree per move.
117 | pub struct AiPlayer {
118 |     evaluator: Box<dyn Evaluator>,
119 | }
120 |
121 | impl AiPlayer {
122 |     /// Own an evaluator and reuse it for every subsequent decision.
123 |     #[must_use]
124 |     pub fn new(evaluator: Box<dyn Evaluator>) -> Self {
125 |         Self { evaluator }
126 |     }
```

### Retain one runtime instance per configured AI seat

`Occupant` and `AiConfig` remain the existing cloneable/serializable seat
descriptions. In particular, an AI configuration is not made to contain a
boxed evaluator. Runtime state belongs beside it in `SessionCore`.
All CLI references to player names below are qualified with `player::`, so
no additional `use player` import is required.

Keep the session module imports shown here. They already supply the board, actor/configuration, event/history, RNG, and test-supermodule names used by the integration. No import is silently removed by the extraction.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 7–18.

```text
  7 | use crate::actor::{AiConfig, EvaluatorKind, Occupant, OccupantKind, SearchInfo};
  8 | use crate::command::{Command, CommandError, Event, SeatSel};
  9 | use crate::history::NodeId as HistoryNodeId;
 10 | use crate::history::{ForwardOutcome, History, PlayOutcome};
 11 | use crate::opening::{OpeningSnapshot, OpeningState};
 12 | use crate::puzzle::{Puzzle, PuzzleError, PuzzlePack};
 13 | use engine::{Board, Color, Move, Status};
 14 | use rand::SeedableRng;
 15 | use rand::rngs::StdRng;
 16 | use serde::{Deserialize, Deserializer, Serialize, Serializer};
 17 | use std::collections::HashMap;
 18 | use std::path::PathBuf;
```

Replace the session state declaration with this shape, adding only `black_player` and `white_player`. An AI occupant must have `Some(AiPlayer)`; a human must have `None`. The existing history, configuration, records, opening, and puzzle fields remain session-owned.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 265–281.

```text
265 | /// Headless session state machine.
266 | pub struct SessionCore {
267 |     history: History,
268 |     black: Occupant,
269 |     white: Occupant,
270 |     // Evaluators are runtime state, not clonable/serializable seat config.
271 |     black_player: Option<player::AiPlayer>,
272 |     white_player: Option<player::AiPlayer>,
273 |     default_config: AiConfig,
274 |     black_sims: Option<u32>,
275 |     white_sims: Option<u32>,
276 |     rng: StdRng,
277 |     init: SessionInit,
278 |     search_info: HashMap<HistoryNodeId, SearchInfo>,
279 |     opening: Option<OpeningState>,
280 |     puzzle: Option<PuzzleStudy>,
281 | }
```

Inside the existing `impl SessionCore`, replace `new` with this constructor. It initializes players from the initial occupants once, while preserving the ceremony assertion and session RNG initialization. Baseline construction now happens when its seat runtime is created, not on every step.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 284–315.

```text
284 |     /// Create a new session from the given initial state.
285 |     pub fn new(init: SessionInit) -> Self {
286 |         // A Swap2 ceremony is not a game position: its stone counts can
287 |         // differ by two mid-round, which no valid `Board` can hold. A
288 |         // ceremony on top of a preset opening would mix two sources of
289 |         // truth, so the combination is rejected at construction.
290 |         assert!(
291 |             !init.swap2 || init.opening == Board::new(),
292 |             "swap2 requires an empty opening board"
293 |         );
294 |         let seed = init.seed.unwrap_or_else(rand::random::<u64>);
295 |         let rng = StdRng::seed_from_u64(seed);
296 |         Self {
297 |             history: History::from_opening(&init.opening),
298 |             black: init.black.clone(),
299 |             white: init.white.clone(),
300 |             black_player: Self::make_player(&init.black),
301 |             white_player: Self::make_player(&init.white),
302 |             default_config: init.default_config.clone(),
303 |             black_sims: init.black_sims,
304 |             white_sims: init.white_sims,
305 |             rng,
306 |             opening: if init.swap2 {
307 |                 Some(OpeningState::new())
308 |             } else {
309 |                 None
310 |             },
311 |             puzzle: init.puzzle.clone(),
312 |             init,
313 |             search_info: HashMap::new(),
314 |         }
315 |     }
```

Inside that same session impl, add `make_player` immediately before the existing `make_evaluator`. The latter is unchanged context: it keeps concrete tactics/Burn baseline selection and feature handling in the CLI. Each AI seat receives its own evaluator, while humans allocate none.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 783–803.

```text
783 |     fn make_player(occupant: &Occupant) -> Option<player::AiPlayer> {
784 |         match occupant {
785 |             Occupant::Ai(config) => Some(player::AiPlayer::new(Self::make_evaluator(
786 |                 &config.evaluator,
787 |             ))),
788 |             Occupant::Human => None,
789 |         }
790 |     }
791 |
792 |     fn make_evaluator(kind: &EvaluatorKind) -> Box<dyn mcts::Evaluator> {
793 |         match kind {
794 |             // Currently, there are two.
795 |             EvaluatorKind::Tactics => Box::new(mcts::TacticsEvaluator::new()),
796 |             #[cfg(feature = "baseline")]
797 |             EvaluatorKind::Baseline => {
798 |                 Box::new(baseline::BaselineEvaluator::new(baseline::Device::flex()))
799 |             }
800 |             #[cfg(not(feature = "baseline"))]
801 |             EvaluatorKind::Baseline => unreachable!("Baseline feature is disabled."),
802 |         }
803 |     }
```

## 2. Decide: terminal input, certified root, ordinary search

### Distinguish failure, certification, and measurements

An already terminal input is an error. An ongoing root with a certified
winning move is a successful decision whose `search` is `None`. An ordinary
search is a successful decision with real statistics, even with zero
simulations. None of these states requires a fabricated proof flag or a
one-visit distribution.

These public return/error types belong at library scope. `Decision` separates the move, optional search measurements, and guard diagnostics; the error implements both display and the standard error trait for non-CLI callers.

**Source:** `gomoku/crates/player/src/lib.rs`, lines 88–114.

```text
 88 | /// A chosen move and, when a search ran, its real statistics.
 89 | #[derive(Debug, Clone, PartialEq)]
 90 | pub struct Decision {
 91 |     /// Move to play.
 92 |     pub mv: Move,
 93 |     /// Search statistics, or `None` when a certified root win bypassed search.
 94 |     pub search: Option<SearchStats>,
 95 |     /// Defensive experiment diagnostics, independent of search statistics.
 96 |     pub guard: GuardDiagnostics,
 97 | }
 98 |
 99 | /// Failure to decide a move.
100 | #[derive(Debug, Clone, Copy, PartialEq, Eq)]
101 | pub enum DecisionError {
102 |     /// The board is already won or drawn.
103 |     GameOver,
104 | }
105 |
106 | impl std::fmt::Display for DecisionError {
107 |     fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
108 |         match self {
109 |             Self::GameOver => formatter.write_str("the game is over"),
110 |         }
111 |     }
112 | }
113 |
114 | impl std::error::Error for DecisionError {}
```

Continue the open `impl AiPlayer` from section 1 with the complete decision method and closing brace. It checks terminal input before work, searches with the retained evaluator, bypasses RNG/statistics/guard on root certification, and otherwise preserves noise then selection before guard resolution. The function borrows rather than commits the board.

**Source:** `gomoku/crates/player/src/lib.rs`, lines 128–192.

```text
128 |     /// Decide a move without modifying `board`.
129 |     ///
130 |     /// A certified root win returns immediately without consuming RNG or
131 |     /// constructing search statistics. Otherwise the order is search, optional
132 |     /// root noise, visit extraction, then temperature selection. Noise is
133 |     /// deliberately applied after search to preserve the existing CLI behavior.
134 |     /// The optional defensive guard runs after ordinary temperature selection,
135 |     /// using cloned boards and leaving all search statistics unchanged. An
136 |     /// unrefuted candidate is unknown, not certified safe.
137 |     ///
138 |     /// # Errors
139 |     /// Returns [`DecisionError::GameOver`] for a won or drawn board, without
140 |     /// calling the evaluator or consuming RNG.
141 |     ///
142 |     /// # Panics
143 |     /// Panics if Gamma rejects enabled noise's alpha, or if MCTS returns an
144 |     /// empty move distribution for an ongoing board.
145 |     pub fn decide(
146 |         &mut self,
147 |         board: &Board,
148 |         options: &PlayerOptions,
149 |         rng: &mut impl Rng,
150 |     ) -> Result<Decision, DecisionError> {
151 |         if board.status() != Status::Ongoing {
152 |             return Err(DecisionError::GameOver);
153 |         }
154 |         let config = SearchConfig {
155 |             simulations: options.simulations,
156 |             c_puct: options.c_puct,
157 |         };
158 |         match mcts::search(board, &mut *self.evaluator, &config) {
159 |             SearchResult::ProvenWin { mv } => Ok(Decision {
160 |                 mv,
161 |                 search: None,
162 |                 guard: GuardDiagnostics::new(mv, GuardOutcome::ProvenWin),
163 |             }),
164 |             SearchResult::Searched { mut tree, root } => {
165 |                 if options.noise_epsilon > 0.0 {
166 |                     mcts::add_dirichlet_noise(
167 |                         &mut tree,
168 |                         root,
169 |                         options.noise_epsilon,
170 |                         options.noise_alpha,
171 |                         rng,
172 |                     );
173 |                 }
174 |                 let visits = mcts::visit_distribution(&tree, root);
175 |                 let proposal = mcts::select_move(&visits, options.temperature, rng)
176 |                     .expect("AI must select a move from a non-empty distribution");
177 |                 let (mv, guard) = guard::choose(
178 |                     board,
179 |                     proposal,
180 |                     tree.node(root).edges(),
181 |                     options.guard,
182 |                     engine::prove_forced_win,
183 |                 );
184 |                 Ok(Decision {
185 |                     mv,
186 |                     search: Some(SearchStats::from_searched(&tree, root, board)),
187 |                     guard,
188 |                 })
189 |             }
190 |         }
191 |     }
192 | }
```

The root-proof branch trusts the chapter-08 evaluator seam: certification
was already admitted by the certifying evaluator. There is no second leaf
call, defensive probe, or pretend value of one. `search: None` means that no
searched-tree measurements were returned by this branch, not that a search
was measured and then discarded.

The ordinary branch deliberately retains **post-search** Dirichlet noise.
It changes root priors after simulations, not current visits or accumulated
values. Temperature selection still consumes visits. Noise consumes RNG
before sampling, so deleting or moving it can change a seeded proposal even
though it does not directly reweight current visits. Its post-noise priors
also break equal-visit ties among later guard alternatives. This compatibility
behavior is not AlphaGo Zero's pre-search root-noise algorithm.

Enabled noise panics if Gamma rejects its alpha; the helper also asserts
nonempty root edges. An empty ongoing-root move distribution panics at
selection's `expect`. The helper does not validate `noise_epsilon`; this
implementation adds no parameter validation or configuration-error enum.

## 3. Extract honest statistics

A guard may choose a move that is not first by visits or prior. The returned
statistics still describe the unchanged searched root, not an invented
endorsement of the chosen defense. Their value perspective is the side to
move **before** committing the move.

At library scope, define `SearchStats` and its complete private extraction helper. It includes every root edge, even zero-visit moves; sorts visits and priors separately with move-index ties; and computes `sum(W) / sum(N)`, explicitly zero for no visits. No caller config or proof flag leaks into this reusable record.

**Source:** `gomoku/crates/player/src/lib.rs`, lines 47–86.

```text
 47 | /// Statistics extracted from a real searched tree, never from a root proof.
 48 | #[derive(Debug, Clone, PartialEq)]
 49 | pub struct SearchStats {
 50 |     /// Player whose perspective determines the root value.
 51 |     pub side_to_move: Color,
 52 |     /// All root moves and visit counts, descending by count, then move index.
 53 |     pub visits: Vec<(Move, u32)>,
 54 |     /// All root priors after optional noise, descending by prior, then move index.
 55 |     pub prior: Vec<(Move, f32)>,
 56 |     /// Root value from the side-to-move perspective: `sum(W) / sum(N)`.
 57 |     /// Zero when no simulations have visited an edge.
 58 |     pub value: f32,
 59 | }
 60 |
 61 | impl SearchStats {
 62 |     fn from_searched(tree: &mcts::Tree, root: mcts::NodeId, board: &Board) -> Self {
 63 |         let edges = tree.node(root).edges();
 64 |         let mut visits = mcts::visit_distribution(tree, root);
 65 |         visits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.index().cmp(&b.0.index())));
 66 |         let mut prior: Vec<_> = edges.iter().map(|edge| (edge.mv, edge.prior)).collect();
 67 |         prior.sort_by(|a, b| {
 68 |             b.1.partial_cmp(&a.1)
 69 |                 .unwrap_or(std::cmp::Ordering::Equal)
 70 |                 .then_with(|| a.0.index().cmp(&b.0.index()))
 71 |         });
 72 |         let total_visits: u32 = edges.iter().map(|edge| edge.n).sum();
 73 |         let total_value: f32 = edges.iter().map(|edge| edge.w).sum();
 74 |         let value = if total_visits == 0 {
 75 |             0.0
 76 |         } else {
 77 |             total_value / total_visits as f32
 78 |         };
 79 |         Self {
 80 |             side_to_move: board.to_move(),
 81 |             visits,
 82 |             prior,
 83 |             value,
 84 |         }
 85 |     }
 86 | }
```

## 4. Guard candidates and admit only verified refutations

### Public settings and diagnostics, private implementation

The guard is an opt-in bounded experiment. The CLI enables its baseline
preset later; a reusable caller defaults to disabled and can opt out for any
evaluator. The preset is **five candidate checks, 1,000 nodes and ten plies
per opponent probe**, all `[experiment]`. These are work limits, not a strict
wall-clock bound: cloning, sorting, verification, and the preceding MCTS
search also take time.

Start the private guard module with the engine proof/budget and MCTS edge imports, then define the complete configuration API. `disabled()` preserves the preset work settings but turns checking off; `Default` is disabled independently of evaluator identity.

**Source:** `gomoku/crates/player/src/guard.rs`, lines 1–48.

```text
  1 | use engine::{Board, Color, Move, Proof, SearchBudget, Status};
  2 | use mcts::Edge;
  3 |
  4 | /// Bounded defensive refutation experiment, not a wall-clock deadline.
  5 | ///
  6 | /// Disabled by default. A missing proof means unknown, never safety. The
  7 | /// candidate cap bounds hypothetical checks; each probe has its own node/depth
  8 | /// budget. Proof verification is also required and is not a timed operation.
  9 | #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 10 | pub struct GuardConfig {
 11 |     /// Whether to inspect ordinary search candidates after selection.
 12 |     pub enabled: bool,
 13 |     /// Maximum candidate checks, including the original proposal. Zero skips all.
 14 |     pub max_candidates: usize,
 15 |     /// Per-candidate opponent prover work caps, not a wall-clock deadline.
 16 |     pub budget: SearchBudget,
 17 | }
 18 |
 19 | impl GuardConfig {
 20 |     /// Preserve ordinary move selection without checking any candidate.
 21 |     #[must_use]
 22 |     pub const fn disabled() -> Self {
 23 |         Self {
 24 |             enabled: false,
 25 |             ..Self::baseline()
 26 |         }
 27 |     }
 28 |
 29 |     /// [experiment] Baseline defaults: five candidates, 1,000 nodes / 10 plies
 30 |     /// per opponent probe. These are work bounds, not a response-time guarantee.
 31 |     #[must_use]
 32 |     pub const fn baseline() -> Self {
 33 |         Self {
 34 |             enabled: true,
 35 |             max_candidates: 5,
 36 |             budget: SearchBudget {
 37 |                 max_nodes: 1_000,
 38 |                 max_depth: 10,
 39 |             },
 40 |         }
 41 |     }
 42 | }
 43 |
 44 | impl Default for GuardConfig {
 45 |     fn default() -> Self {
 46 |         Self::disabled()
 47 |     }
 48 | }
```

Still at guard-module scope, define the outcomes, diagnostics, and their initialization helper. These distinguish unchecked, unknown, immediate terminal, capped, and individually all-refuted states without changing root measurements. The public data is re-exported by the library; `new` remains crate-private.

**Source:** `gomoku/crates/player/src/guard.rs`, lines 50–90.

```text
 50 | /// What the defensive experiment established (or deliberately left unknown).
 51 | #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 52 | pub enum GuardOutcome {
 53 |     /// Guard disabled; the original candidate is unchecked.
 54 |     Disabled,
 55 |     /// Certified root win bypassed both ordinary selection and this guard.
 56 |     ProvenWin,
 57 |     /// No verified opponent win found within the probe budget: unknown, not safe.
 58 |     Unrefuted,
 59 |     /// The checked candidate immediately won for us or drew the game.
 60 |     Terminal,
 61 |     /// Candidate cap reached; selected move is untested, not certified safe.
 62 |     BudgetExhausted,
 63 |     /// Every legal candidate was individually refuted; original move retained.
 64 |     /// This is not a proof that the root position is lost.
 65 |     AllRefuted,
 66 | }
 67 |
 68 | /// Decision-local diagnostics; these never alter MCTS visits or priors.
 69 | #[derive(Debug, Clone, PartialEq, Eq)]
 70 | pub struct GuardDiagnostics {
 71 |     /// Original ordinary selection (or the certified root winning move).
 72 |     pub proposal: Move,
 73 |     /// Candidates refuted by verified opponent proofs, in check order.
 74 |     pub refuted: Vec<Move>,
 75 |     /// Hypothetical candidates checked, including immediate wins/draws.
 76 |     pub checked: usize,
 77 |     /// Result of the defensive experiment, not a position-value judgement.
 78 |     pub outcome: GuardOutcome,
 79 | }
 80 |
 81 | impl GuardDiagnostics {
 82 |     pub(crate) fn new(proposal: Move, outcome: GuardOutcome) -> Self {
 83 |         Self {
 84 |             proposal,
 85 |             refuted: Vec::new(),
 86 |             checked: 0,
 87 |             outcome,
 88 |         }
 89 |     }
 90 | }
```

### Preserve the proposal, then rank retries

The original temperature-selected proposal is checked first even when it
ranks below another move. Alternatives are descending visits, then descending
post-noise prior, then ascending move index. An expanded MCTS root supplies
all legal moves, including unvisited ones. The resolver neither resamples nor
consumes RNG.

Begin `choose` at guard-module scope. Its private callback seam lets tests observe or corrupt proofs without modifying `Evaluator`. Disabled and zero-cap returns do no probe work; the remaining prefix constructs the deterministic candidate iterator. The next fragment continues this function directly.

**Source:** `gomoku/crates/player/src/guard.rs`, lines 92–125.

```text
 92 | // Private injection seam for bounded bookkeeping tests, never an evaluator hook.
 93 | // Expanded MCTS roots contain every legal edge, including unvisited edges.
 94 | pub(crate) fn choose(
 95 |     board: &Board,
 96 |     proposal: Move,
 97 |     edges: &[Edge],
 98 |     config: GuardConfig,
 99 |     mut probe: impl FnMut(&Board, Color, SearchBudget) -> Option<Proof>,
100 | ) -> (Move, GuardDiagnostics) {
101 |     let mut diagnostics = GuardDiagnostics::new(proposal, GuardOutcome::Disabled);
102 |     if !config.enabled {
103 |         return (proposal, diagnostics);
104 |     }
105 |     diagnostics.outcome = GuardOutcome::BudgetExhausted;
106 |     if config.max_candidates == 0 {
107 |         return (proposal, diagnostics);
108 |     }
109 |     let mut ranked: Vec<_> = edges.iter().collect();
110 |     ranked.sort_by(|a, b| {
111 |         b.n.cmp(&a.n)
112 |             .then_with(|| {
113 |                 b.prior
114 |                     .partial_cmp(&a.prior)
115 |                     .unwrap_or(std::cmp::Ordering::Equal)
116 |             })
117 |             .then_with(|| a.mv.index().cmp(&b.mv.index()))
118 |     });
119 |     let candidates = std::iter::once(proposal).chain(
120 |         ranked
121 |             .into_iter()
122 |             .map(|edge| edge.mv)
123 |             .filter(|&mv| mv != proposal),
124 |     );
125 |     for candidate in candidates {
```

Continue `choose` through its full loop and return. Check the cap before playing the next candidate, clone and play before probing, and use the resulting board's actual opponent. Only a correct-winner proof with a present legal first move and accepted `verify_line` refutes the candidate. The final assertion protects the all-legal-edge invariant.

**Source:** `gomoku/crates/player/src/guard.rs`, lines 126–164.

```text
126 |         if diagnostics.refuted.contains(&candidate) {
127 |             continue;
128 |         }
129 |         // All previously checked candidates were refuted. The next ranked
130 |         // candidate is untested, so the cap must not send us back to a loser.
131 |         if diagnostics.checked == config.max_candidates {
132 |             return (candidate, diagnostics);
133 |         }
134 |         let mut after = board.clone();
135 |         after
136 |             .play(candidate)
137 |             .expect("root candidates must be legal");
138 |         diagnostics.checked += 1;
139 |         if matches!(after.status(), Status::Won(side) if side == board.to_move())
140 |             || after.status() == Status::Draw
141 |         {
142 |             diagnostics.outcome = GuardOutcome::Terminal;
143 |             return (candidate, diagnostics);
144 |         }
145 |         let opponent = after.to_move();
146 |         let refuted = probe(&after, opponent, config.budget).is_some_and(|proof| {
147 |             proof.winner == opponent
148 |                 && proof.line.first().is_some_and(|&mv| after.is_legal(mv))
149 |                 && engine::verify_line(&after, &proof)
150 |         });
151 |         if !refuted {
152 |             diagnostics.outcome = GuardOutcome::Unrefuted;
153 |             return (candidate, diagnostics);
154 |         }
155 |         diagnostics.refuted.push(candidate);
156 |     }
157 |     assert_eq!(
158 |         diagnostics.refuted.len(),
159 |         board.empty_moves().count(),
160 |         "expanded root must include every legal candidate"
161 |     );
162 |     diagnostics.outcome = GuardOutcome::AllRefuted;
163 |     (proposal, diagnostics)
164 | }
```

There are three distinct nonterminal conclusions:

- `None` from the prover, or any rejected proof, yields `Unrefuted`: **UNKNOWN**,
  not safe. Bounded/selective TSS can miss winning continuations.
- If the cap is reached after refutations, the returned candidate is the
  highest-ranked remaining **UNTESTED** move, not the already-refuted proposal.
  With a zero cap, the proposal itself is untested. Both use `BudgetExhausted`.
- `AllRefuted` is reached only after every legal candidate was individually
  refuted. It falls back to the original legal proposal, which can be a known
  loser. The diagnostic is **not a composed root-position loss proof**, and
  the CLI must not infer resignation.

An immediate own win or draw returns `Terminal` without an opponent probe.
It still counts as one hypothetical check. The borrowed board and immutable
root edges are unchanged on every branch.

## 5. CLI decision, application, history, and lifecycle

### Attach the effective configuration without altering measurements

The player does not know about seats, opening schedules, history cursors, or
command events. Those remain in the session. `SearchInfo::from_searched` stays
unchanged for existing callers and the seeded comparison test.

Insert this new method into the existing `impl SearchInfo`. It moves the player statistics into the CLI record and clones only the effective `AiConfig`. Qualified `player::SearchStats` needs no actor import change; all older actor types and imports remain as in chapter 08.

**Source:** `gomoku/crates/cli/src/actor.rs`, lines 39–49.

```text
 39 |     /// Attach the effective seat configuration to a player's search statistics.
 40 |     #[must_use]
 41 |     pub fn from_stats(stats: player::SearchStats, config: &AiConfig) -> Self {
 42 |         Self {
 43 |             side_to_move: stats.side_to_move,
 44 |             visits: stats.visits,
 45 |             prior: stats.prior,
 46 |             value: stats.value,
 47 |             config: config.clone(),
 48 |         }
 49 |     }
```

### Route one AI step

Replace the old `execute_step` search/noise/selection implementation with the
following three adjacent fragments in the existing session impl. Together
they are the entire method, without a second old search path left behind.

The step prefix preserves ceremony, terminal, and AI-seat checks in that order. It clones the occupant settings into an effective configuration and applies the existing opening schedule before borrowing the player.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 417–442.

```text
417 |     fn execute_step(&mut self) -> Result<Vec<Event>, CommandError> {
418 |         // The AI searches game positions. Ceremony stones are not a
419 |         // game position, and a move searched now would be discarded
420 |         // when the ceremony finishes and resets the tree. Refuse
421 |         // instead.
422 |         if self.opening.is_some() {
423 |             return Err(CommandError::OpeningInProgress);
424 |         }
425 |         let board = self.history.board();
426 |         if board.status() != Status::Ongoing {
427 |             return Err(CommandError::GameOver);
428 |         }
429 |         let side = board.to_move();
430 |         let config = match self.occupant(side) {
431 |             Occupant::Ai(cfg) => cfg.clone(),
432 |             Occupant::Human => {
433 |                 return Err(CommandError::StepRequiresAi { seat: side });
434 |             }
435 |         };
436 |
437 |         let mut effective_config = config.clone();
438 |         if self.opening_schedule_active(side) {
439 |             effective_config.temperature = 1.0;
440 |             effective_config.noise_epsilon = 0.25;
441 |         }
442 |
```

Continue the step by mapping effective settings to player options. Baseline gets the experimental guard; tactics stays disabled. Borrow the correct persistent seat and session RNG, translate the terminal error, and route a proof decision through normal `execute_play` without inserting new search information.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 443–469.

```text
443 |         let options = player::PlayerOptions {
444 |             simulations: effective_config.simulations,
445 |             c_puct: effective_config.c_puct,
446 |             temperature: effective_config.temperature,
447 |             noise_epsilon: effective_config.noise_epsilon,
448 |             noise_alpha: 0.3,
449 |             guard: match effective_config.evaluator {
450 |                 EvaluatorKind::Baseline => player::GuardConfig::baseline(),
451 |                 EvaluatorKind::Tactics => player::GuardConfig::disabled(),
452 |             },
453 |         };
454 |         let player = match side {
455 |             Color::Black => &mut self.black_player,
456 |             Color::White => &mut self.white_player,
457 |         }
458 |         .as_mut()
459 |         .expect("AI seat must own a persistent player");
460 |         let decision = player
461 |             .decide(&board, &options, &mut self.rng)
462 |             .map_err(|player::DecisionError::GameOver| CommandError::GameOver)?;
463 |         let mv = decision.mv;
464 |         let Some(stats) = decision.search else {
465 |             // A certified forced win bypasses search statistics.
466 |             return self.execute_play(mv);
467 |         };
468 |         let info = SearchInfo::from_stats(stats, &effective_config);
469 |
```

Complete the searched branch and method. Attach effective settings, commit through `history.play`, store the search record at the new cursor, and preserve move/branch/terminal event order. This path deliberately remains explicit rather than being rerouted through `execute_play`. The current CLI neither stores nor renders the returned guard diagnostics.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 470–496.

```text
470 |         let old_cursor = self.history.cursor();
471 |         let play_outcome = self.history.play(mv)?;
472 |         let new_cursor = self.history.cursor();
473 |         self.search_info.insert(new_cursor, info);
474 |
475 |         let mut events = Vec::new();
476 |         events.push(Event::Moved {
477 |             mv,
478 |             by: OccupantKind::Ai,
479 |         });
480 |
481 |         if matches!(play_outcome, PlayOutcome::Moved(_)) {
482 |             let siblings = self.history.children(old_cursor);
483 |             if siblings.len() > 1 {
484 |                 events.push(Event::Branched { options: siblings });
485 |             }
486 |         }
487 |
488 |         let new_board = self.history.board();
489 |         if new_board.status() != Status::Ongoing {
490 |             events.push(Event::GameOver {
491 |                 result: new_board.status(),
492 |             });
493 |         }
494 |
495 |         Ok(events)
496 |     }
```

Keep this existing schedule helper in the session. It is unchanged context needed to understand effective settings: eligible empty-root AI-versus-AI lines use temperature 1 and noise fraction 0.25 for the first ten current-path plies; explicit settings suppress the schedule. History navigation can re-enter those plies without replacing either evaluator.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 805–823.

```text
805 |     /// [experiment] Opening schedule for AI-vs-AI from an empty root.
806 |     ///
807 |     /// When the user has not explicitly configured temperature or
808 |     /// Dirichlet noise, the first 10 plies of each line use temperature
809 |     /// 1.0 with ε = 0.25 / α = 0.3 Dirichlet noise to introduce opening
810 |     /// variety. After ply 10 (and for any explicitly configured session)
811 |     /// the configured values are used verbatim.
812 |     ///
813 |     /// The schedule is keyed to the cursor ply along the current path, so
814 |     /// branching back into the first 10 plies re-enters the schedule.
815 |     fn opening_schedule_active(&self, _side: Color) -> bool {
816 |         self.init.opening == Board::new()
817 |             && self.black.is_ai()
818 |             && self.white.is_ai()
819 |             && !self.default_config.explicit_selection
820 |             && self.default_config.temperature == 0.0
821 |             && self.default_config.noise_epsilon == 0.0
822 |             && self.history.ply() < 10
823 |     }
```

A proof decision adds no new `SearchInfo`: the unchanged `execute_play`
handler still performs the authoritative history move and normal move,
branch, and game-over events. Searched decisions store the actual search
record even if the guard changed the move. The selected move and the visit
leader therefore need not agree.

### Replace occupants and runtimes together

All command dispatch remains unchanged. `Command::New` calls `execute_new`,
which calls reset; release/take call `set_occupant`. `SetSims` changes only
configuration and overrides. This resource boundary is what makes reuse
observable, not just a cache-like naming change.

Replace `set_occupant` in the session impl. Construct the selected seat's new optional runtime from the new occupant, then assign occupant and runtime together. Taking drops its evaluator; releasing creates one; the other seat is untouched.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 749–761.

```text
749 |     fn set_occupant(&mut self, color: Color, occupant: Occupant) {
750 |         let player = Self::make_player(&occupant);
751 |         match color {
752 |             Color::Black => {
753 |                 self.black = occupant;
754 |                 self.black_player = player;
755 |             }
756 |             Color::White => {
757 |                 self.white = occupant;
758 |                 self.white_player = player;
759 |             }
760 |         }
761 |     }
```

Replace `reset` in the same impl. It now restores both initial occupants through `set_occupant`, thereby replacing both old runtimes. All prior history/configuration/ceremony/puzzle reset behavior stays intact, and the existing RNG stream is deliberately not reseeded.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 763–781.

```text
763 |     fn reset(&mut self) {
764 |         self.history = History::from_opening(&self.init.opening);
765 |         self.set_occupant(Color::Black, self.init.black.clone());
766 |         self.set_occupant(Color::White, self.init.white.clone());
767 |         self.default_config = self.init.default_config.clone();
768 |         self.black_sims = self.init.black_sims;
769 |         self.white_sims = self.init.white_sims;
770 |         self.search_info.clear();
771 |         // `new` returns the session to its initial phase. A Swap2
772 |         // session started in the ceremony, so it resets into the
773 |         // ceremony.
774 |         self.opening = if self.init.swap2 {
775 |             Some(OpeningState::new())
776 |         } else {
777 |             None
778 |         };
779 |         self.puzzle = self.init.puzzle.clone();
780 |         // Keep the same RNG; reset does not reseed.
781 |     }
```

These existing `execute_new` and `execute_set_sims` methods stay unchanged; retain them, do not append duplicates. Their complete bodies show why New replaces runtimes through reset, whereas changing simulation counts only mutates options/configuration and leaves the evaluators alive.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 498–528.

```text
498 |     /// Reset the session to its initial opening.
499 |     pub fn execute_new(&mut self) -> Result<Vec<Event>, CommandError> {
500 |         self.reset();
501 |         Ok(vec![Event::Reset])
502 |     }
503 |
504 |     /// Set the simulation count for a seat or seats.
505 |     pub fn execute_set_sims(
506 |         &mut self,
507 |         seat: SeatSel,
508 |         sims: u32,
509 |     ) -> Result<Vec<Event>, CommandError> {
510 |         self.validate_seat_arg(seat)?;
511 |         if sims == 0 {
512 |             return Err(CommandError::ZeroSims);
513 |         }
514 |         let mut events = Vec::new();
515 |         self.apply_to_seats(seat, |this, color| {
516 |             match color {
517 |                 Color::Black => this.black_sims = Some(sims),
518 |                 Color::White => this.white_sims = Some(sims),
519 |             }
520 |             if let Occupant::Ai(config) = this.occupant_mut(color) {
521 |                 config.simulations = sims;
522 |             }
523 |             events.push(Event::ConfigChanged { seat: color, sims })
524 |         });
525 |
526 |         Ok(events)
527 |     }
528 |
```

Keep the unchanged release handler in the session impl. It applies per-seat simulation overrides before calling the newly runtime-aware `set_occupant`, so the released seat receives a newly constructed evaluator with the intended settings.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 530–549.

```text
530 |     pub fn execute_release(&mut self, seat: SeatSel) -> Result<Vec<Event>, CommandError> {
531 |         self.validate_seat_arg(seat)?;
532 |         let mut events = Vec::new();
533 |         self.apply_to_seats(seat, |this, color| {
534 |             let sims = match color {
535 |                 Color::Black => this.black_sims,
536 |                 Color::White => this.white_sims,
537 |             };
538 |             let mut config = this.default_config.clone();
539 |             if let Some(s) = sims {
540 |                 config.simulations = s;
541 |             }
542 |             this.set_occupant(color, Occupant::Ai(config));
543 |             events.push(Event::SeatChanged {
544 |                 seat: color,
545 |                 now: OccupantKind::Ai,
546 |             })
547 |         });
548 |         Ok(events)
549 |     }
```

Keep the unchanged take handler as well. Its existing call to `set_occupant` now clears the selected runtime along with assigning a human occupant, without disturbing the other seat.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 638–650.

```text
638 |     /// Take a seat or seats for the human player.
639 |     pub fn execute_take(&mut self, seat: SeatSel) -> Result<Vec<Event>, CommandError> {
640 |         let mut events = Vec::new();
641 |         self.apply_to_seats(seat, |this, color| {
642 |             let new_occupant = Occupant::Human;
643 |             this.set_occupant(color, new_occupant);
644 |             events.push(Event::SeatChanged {
645 |                 seat: color,
646 |                 now: OccupantKind::Human,
647 |             })
648 |         });
649 |         Ok(events)
650 |     }
```

The existing `apply_to_seats`, `occupant`, `occupant_mut`, and
`validate_seat_arg` helpers need no edits; they already supply seat iteration,
configuration access, and argument validation. Ordinary play, back/forward/
variation navigation, opening completion, and next/previous puzzle navigation
also need no player reconstruction. Puzzle navigation replaces history and
clears search records, but retains occupants and runtime instances.

| Operation | Runtime effect | RNG effect |
|-----------|----------------|------------|
| Initialize session | Construct one player for each initial AI; none for humans. | Seed once. |
| Decide/play/navigate/SetSims | Retain both evaluators. | Decision borrows the current stream; navigation/config changes do not reseed. |
| Release seat | Construct that seat's evaluator. | Retain stream. |
| Take seat | Drop that seat's evaluator. | Retain stream. |
| New/reset | Drop old players and restore players from initial occupants. | Retain stream. |

There is no background worker, asynchronous step, network service, or tree
reuse in this slice. Persistence means evaluator lifetime only.

## 6. Executable specifications: input, proof, search, persistence

The following are complete new test artifacts, not test-name checklists.
For a test-first implementation, write each relevant group before the
production behavior it exercises. All public decision tests live in the new
`gomoku/crates/player/tests/decisions.rs`; their numbered fragments reconstruct
that entire file.

Start the decision integration-test file with these imports. Shared mutable observations are confined to test doubles; the production player requires no Clone or thread-safety bounds on evaluators.

**Source:** `gomoku/crates/player/tests/decisions.rs`, lines 1–9.

```text
  1 | use std::cell::RefCell;
  2 | use std::rc::Rc;
  3 |
  4 | use engine::{Board, Move, Status};
  5 | use mcts::{
  6 |     EvalRequest, EvalResult, Evaluator, RootEval, SearchConfig, SearchResult, UniformEvaluator,
  7 | };
  8 | use player::{AiPlayer, DecisionError, GuardConfig, GuardOutcome, PlayerOptions};
  9 | use rand::{RngExt, SeedableRng, rngs::StdRng};
```

At test-file scope, replay a nearly won board and define the panic-on-use evaluator. Replaying preserves ordinary engine history; both evaluation entry points panic so terminal rejection cannot accidentally do evaluator work.

**Source:** `gomoku/crates/player/tests/decisions.rs`, lines 11–38.

```text
 11 | fn winning_position() -> Board {
 12 |     let mut board = Board::new();
 13 |     for (row, col) in [
 14 |         (7, 3),
 15 |         (0, 0),
 16 |         (7, 4),
 17 |         (0, 2),
 18 |         (7, 5),
 19 |         (0, 4),
 20 |         (7, 6),
 21 |         (0, 6),
 22 |     ] {
 23 |         board.play(Move::new(row, col).unwrap()).unwrap();
 24 |     }
 25 |     board
 26 | }
 27 |
 28 | struct NeverEvaluate;
 29 |
 30 | impl Evaluator for NeverEvaluate {
 31 |     fn evaluate(&mut self, _: EvalRequest) -> EvalResult {
 32 |         panic!("terminal boards must not reach the evaluator");
 33 |     }
 34 |
 35 |     fn evaluate_root(&mut self, _: EvalRequest) -> RootEval {
 36 |         panic!("terminal boards must not reach the root probe");
 37 |     }
 38 | }
```

On the terminal-input path, complete the win and assert the public error plus the next RNG draw. Equal moves alone would not prove equal RNG consumption; this compares the streams directly.

**Source:** `gomoku/crates/player/tests/decisions.rs`, lines 40–53.

```text
 40 | #[test]
 41 | fn terminal_board_returns_game_over_without_evaluation_or_rng() {
 42 |     let mut board = winning_position();
 43 |     board.play(Move::new(7, 7).unwrap()).unwrap();
 44 |     assert_ne!(board.status(), Status::Ongoing);
 45 |     let mut player = AiPlayer::new(Box::new(NeverEvaluate));
 46 |     let mut rng = StdRng::seed_from_u64(7);
 47 |     let mut untouched_rng = StdRng::seed_from_u64(7);
 48 |
 49 |     let result = player.decide(&board, &PlayerOptions::default(), &mut rng);
 50 |
 51 |     assert_eq!(result.unwrap_err(), DecisionError::GameOver);
 52 |     assert_eq!(rng.random::<u64>(), untouched_rng.random::<u64>());
 53 | }
```

For the certified-root path, define a root-only evaluator double with a shared call counter. Its leaf method panics, and its hook asserts that the chosen proof move is legal in the request.

**Source:** `gomoku/crates/player/tests/decisions.rs`, lines 132–147.

```text
132 | struct ProofEvaluator {
133 |     mv: Move,
134 |     root_calls: Rc<RefCell<u32>>,
135 | }
136 |
137 | impl Evaluator for ProofEvaluator {
138 |     fn evaluate(&mut self, _: EvalRequest) -> EvalResult {
139 |         panic!("a root proof must bypass all leaf evaluation");
140 |     }
141 |
142 |     fn evaluate_root(&mut self, request: EvalRequest) -> RootEval {
143 |         assert!(request.legal.contains(&self.mv));
144 |         *self.root_calls.borrow_mut() += 1;
145 |         RootEval::ProvenWin { mv: self.mv }
146 |     }
147 | }
```

Exercise root certification with noise, temperature, and guard all enabled. The assertions establish no fabricated stats, no defensive work, exactly one root call, and no RNG consumption despite those enabled options.

**Source:** `gomoku/crates/player/tests/decisions.rs`, lines 149–179.

```text
149 | #[test]
150 | fn root_proof_has_no_search_stats_leaf_calls_or_rng_consumption() {
151 |     let board = winning_position();
152 |     let mv = Move::new(7, 7).unwrap();
153 |     assert!(engine::immediate_wins(&board, board.to_move()).contains(mv));
154 |     let root_calls = Rc::new(RefCell::new(0));
155 |     let mut player = AiPlayer::new(Box::new(ProofEvaluator {
156 |         mv,
157 |         root_calls: Rc::clone(&root_calls),
158 |     }));
159 |     let options = PlayerOptions {
160 |         simulations: 100,
161 |         temperature: 1.0,
162 |         noise_epsilon: 0.25,
163 |         guard: GuardConfig::baseline(),
164 |         ..PlayerOptions::default()
165 |     };
166 |     let mut rng = StdRng::seed_from_u64(8);
167 |     let mut untouched_rng = StdRng::seed_from_u64(8);
168 |
169 |     let decision = player.decide(&board, &options, &mut rng).unwrap();
170 |
171 |     assert_eq!(decision.mv, mv);
172 |     assert!(decision.search.is_none());
173 |     assert_eq!(decision.guard.outcome, GuardOutcome::ProvenWin);
174 |     assert_eq!(decision.guard.proposal, mv);
175 |     assert_eq!(decision.guard.checked, 0);
176 |     assert!(decision.guard.refuted.is_empty());
177 |     assert_eq!(*root_calls.borrow(), 1);
178 |     assert_eq!(rng.random::<u64>(), untouched_rng.random::<u64>());
179 | }
```

For ordinary decisions and statistics, compare the full manual pre-extraction path with a player under equal RNG seeds. The four cases include deterministic and sampled choices, noisy priors, and zero simulations. Full sorted visits/priors, value, visit totals, legality, and the next RNG draw must match.

**Source:** `gomoku/crates/player/tests/decisions.rs`, lines 55–130.

```text
 55 | #[test]
 56 | fn ordinary_decisions_match_manual_search_stats_and_seeded_selection() {
 57 |     let mut board = Board::new();
 58 |     board.play(Move::new(7, 7).unwrap()).unwrap();
 59 |     for (simulations, temperature, noise_epsilon) in [
 60 |         (17, 0.0, 0.0),
 61 |         (19, 1.0, 0.0),
 62 |         (23, 0.7, 0.25),
 63 |         (0, 1.0, 0.25),
 64 |     ] {
 65 |         let options = PlayerOptions {
 66 |             simulations,
 67 |             c_puct: 1.7,
 68 |             temperature,
 69 |             noise_epsilon,
 70 |             noise_alpha: 0.3,
 71 |             guard: GuardConfig::disabled(),
 72 |         };
 73 |         let mut manual_rng = StdRng::seed_from_u64(42);
 74 |         let mut player_rng = StdRng::seed_from_u64(42);
 75 |         let mut evaluator = UniformEvaluator::new(0.6);
 76 |         let SearchResult::Searched { mut tree, root } = mcts::search(
 77 |             &board,
 78 |             &mut evaluator,
 79 |             &SearchConfig {
 80 |                 simulations,
 81 |                 c_puct: options.c_puct,
 82 |             },
 83 |         ) else {
 84 |             panic!("uniform evaluator cannot prove a win");
 85 |         };
 86 |         if noise_epsilon > 0.0 {
 87 |             mcts::add_dirichlet_noise(
 88 |                 &mut tree,
 89 |                 root,
 90 |                 noise_epsilon,
 91 |                 options.noise_alpha,
 92 |                 &mut manual_rng,
 93 |             );
 94 |         }
 95 |         let visits = mcts::visit_distribution(&tree, root);
 96 |         let expected_move = mcts::select_move(&visits, temperature, &mut manual_rng).unwrap();
 97 |         let mut expected_visits = visits;
 98 |         expected_visits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.index().cmp(&b.0.index())));
 99 |         let edges = tree.node(root).edges();
100 |         let mut expected_prior: Vec<_> = edges.iter().map(|e| (e.mv, e.prior)).collect();
101 |         expected_prior.sort_by(|a, b| {
102 |             b.1.partial_cmp(&a.1)
103 |                 .unwrap()
104 |                 .then_with(|| a.0.index().cmp(&b.0.index()))
105 |         });
106 |         let expected_value = if simulations == 0 {
107 |             0.0
108 |         } else {
109 |             edges.iter().map(|e| e.w).sum::<f32>() / simulations as f32
110 |         };
111 |         let mut player = AiPlayer::new(Box::new(UniformEvaluator::new(0.6)));
112 |
113 |         let decision = player.decide(&board, &options, &mut player_rng).unwrap();
114 |
115 |         assert_eq!(decision.mv, expected_move);
116 |         assert!(board.is_legal(decision.mv));
117 |         let stats = decision
118 |             .search
119 |             .expect("ordinary search has real statistics");
120 |         assert_eq!(stats.side_to_move, board.to_move());
121 |         assert_eq!(stats.visits, expected_visits);
122 |         assert_eq!(
123 |             stats.visits.iter().map(|(_, n)| n).sum::<u32>(),
124 |             simulations
125 |         );
126 |         assert_eq!(stats.prior, expected_prior);
127 |         assert_eq!(stats.value, expected_value);
128 |         assert_eq!(player_rng.random::<u64>(), manual_rng.random::<u64>());
129 |     }
130 | }
```

For persistence, define a stateful evaluator whose local counter is observable through shared test state. This tests retained evaluator identity without exposing it in the public API.

**Source:** `gomoku/crates/player/tests/decisions.rs`, lines 181–195.

```text
181 | struct StatefulEvaluator {
182 |     calls: u32,
183 |     observed: Rc<RefCell<Vec<u32>>>,
184 | }
185 |
186 | impl Evaluator for StatefulEvaluator {
187 |     fn evaluate(&mut self, _: EvalRequest) -> EvalResult {
188 |         self.calls += 1;
189 |         self.observed.borrow_mut().push(self.calls);
190 |         EvalResult {
191 |             policy: [0.0; 225],
192 |             value: 0.1,
193 |         }
194 |     }
195 | }
```

Decide twice with one player and play the first move between requests. The monotonically increasing observations prove reuse across decisions rather than reconstruction of a new evaluator counter.

**Source:** `gomoku/crates/player/tests/decisions.rs`, lines 197–218.

```text
197 | #[test]
198 | fn evaluator_instance_persists_across_two_decisions() {
199 |     let observed = Rc::new(RefCell::new(Vec::new()));
200 |     let mut player = AiPlayer::new(Box::new(StatefulEvaluator {
201 |         calls: 0,
202 |         observed: Rc::clone(&observed),
203 |     }));
204 |     let options = PlayerOptions {
205 |         simulations: 1,
206 |         ..PlayerOptions::default()
207 |     };
208 |     let mut board = Board::new();
209 |     let mut rng = StdRng::seed_from_u64(9);
210 |
211 |     let first = player.decide(&board, &options, &mut rng).unwrap();
212 |     assert_eq!(*observed.borrow(), [1, 2]);
213 |     board.play(first.mv).unwrap();
214 |     let second = player.decide(&board, &options, &mut rng).unwrap();
215 |
216 |     assert!(board.is_legal(second.mv));
217 |     assert_eq!(*observed.borrow(), [1, 2, 3, 4]);
218 | }
```

## 7. Executable specifications: guarded candidates

### Public fixtures and preset bounds

Create `gomoku/crates/player/tests/guard.rs` from the fragments in this
subsection. These twelve tests use the real engine prover and verifier where
refutations are required. Most use zero MCTS simulations: the lowest-index
legal proposal is predictable, while a high prior independently ranks a retry.
This is a control of the test, not a claimed baseline playing-strength result.

Begin the public guard tests with all imports and complete coordinate/position/single-threat helpers. The single threat leaves exactly one White winning endpoint, so Black can block it without the fixture confusing two separate threats.

**Source:** `gomoku/crates/player/tests/guard.rs`, lines 1–25.

```text
  1 | use engine::{Board, Color, Move, SearchBudget, Status};
  2 | use mcts::{EvalRequest, EvalResult, Evaluator, UniformEvaluator};
  3 | use player::{AiPlayer, GuardConfig, GuardOutcome, PlayerOptions};
  4 | use rand::{RngExt, SeedableRng, rngs::StdRng};
  5 |
  6 | fn mv(row: u8, col: u8) -> Move {
  7 |     Move::new(row, col).unwrap()
  8 | }
  9 |
 10 | fn position(black: &[(u8, u8)], white: &[(u8, u8)]) -> Board {
 11 |     Board::from_position(
 12 |         &black.iter().map(|&(r, c)| mv(r, c)).collect::<Vec<_>>(),
 13 |         &white.iter().map(|&(r, c)| mv(r, c)).collect::<Vec<_>>(),
 14 |         Color::Black,
 15 |     )
 16 |     .unwrap()
 17 | }
 18 |
 19 | // White's four has one winning endpoint; Black must block (7, 7).
 20 | fn single_threat() -> Board {
 21 |     position(
 22 |         &[(7, 2), (2, 0), (2, 2), (2, 4)],
 23 |         &[(7, 3), (7, 4), (7, 5), (7, 6)],
 24 |     )
 25 | }
```

At test-file scope, define `Prefer` and the zero-simulation options helper. The evaluator only supplies logits and an estimate; it cannot certify a root win, so these tests must reach candidate guarding rather than bypass it.

**Source:** `gomoku/crates/player/tests/guard.rs`, lines 27–42.

```text
 27 | struct Prefer(Move);
 28 | impl Evaluator for Prefer {
 29 |     fn evaluate(&mut self, _: EvalRequest) -> EvalResult {
 30 |         let mut policy = [0.0; 225];
 31 |         policy[self.0.index()] = 10.0;
 32 |         EvalResult { policy, value: 0.0 }
 33 |     }
 34 | }
 35 |
 36 | fn options(guard: GuardConfig) -> PlayerOptions {
 37 |     PlayerOptions {
 38 |         simulations: 0,
 39 |         guard,
 40 |         ..PlayerOptions::default()
 41 |     }
 42 | }
```

First pin the public opt-in contract and exact experimental preset limits. Default player options must remain disabled even though the CLI later chooses the preset for baseline seats.

**Source:** `gomoku/crates/player/tests/guard.rs`, lines 44–57.

```text
 44 | #[test]
 45 | fn baseline_guard_is_opt_in_with_fixed_experiment_work_bounds() {
 46 |     assert_eq!(PlayerOptions::default().guard, GuardConfig::disabled());
 47 |     let baseline = GuardConfig::baseline();
 48 |     assert!(baseline.enabled);
 49 |     assert_eq!(baseline.max_candidates, 5);
 50 |     assert_eq!(
 51 |         baseline.budget,
 52 |         SearchBudget {
 53 |             max_nodes: 1_000,
 54 |             max_depth: 10
 55 |         }
 56 |     );
 57 | }
```

### A verified opponent continuation rejects only its candidate

On the ordinary-proposal path, demonstrate a real accepted opponent proof, then find the unvisited block. Compare enabled and disabled results to establish unchanged board, Zobrist key, and full search statistics after the guarded move changes.

**Source:** `gomoku/crates/player/tests/guard.rs`, lines 59–100.

```text
 59 | #[test]
 60 | fn real_proof_refutes_proposal_and_guard_finds_unvisited_block_without_mutating_board_or_stats() {
 61 |     let board = single_threat();
 62 |     let original = board.clone();
 63 |     let block = mv(7, 7);
 64 |     let mut disabled = AiPlayer::new(Box::new(Prefer(block)));
 65 |     let mut enabled = AiPlayer::new(Box::new(Prefer(block)));
 66 |     let initial = disabled
 67 |         .decide(
 68 |             &board,
 69 |             &options(GuardConfig::disabled()),
 70 |             &mut StdRng::seed_from_u64(1),
 71 |         )
 72 |         .unwrap();
 73 |     assert_eq!(initial.mv, mv(0, 0));
 74 |     let mut hypothetical = board.clone();
 75 |     hypothetical.play(initial.mv).unwrap();
 76 |     let opponent = hypothetical.to_move();
 77 |     let proof =
 78 |         engine::prove_forced_win(&hypothetical, opponent, GuardConfig::baseline().budget).unwrap();
 79 |     assert_eq!(proof.winner, Color::White);
 80 |     assert!(!proof.line.is_empty());
 81 |     assert!(hypothetical.is_legal(proof.line[0]));
 82 |     assert!(engine::verify_line(&hypothetical, &proof));
 83 |
 84 |     let guarded = enabled
 85 |         .decide(
 86 |             &board,
 87 |             &options(GuardConfig::baseline()),
 88 |             &mut StdRng::seed_from_u64(1),
 89 |         )
 90 |         .unwrap();
 91 |     assert_eq!(guarded.mv, block);
 92 |     assert_eq!(guarded.guard.proposal, initial.mv);
 93 |     assert_eq!(guarded.guard.refuted, vec![initial.mv]);
 94 |     assert_eq!(guarded.guard.checked, 2);
 95 |     assert_eq!(guarded.guard.outcome, GuardOutcome::Unrefuted);
 96 |     assert_eq!(guarded.search, initial.search);
 97 |     assert_eq!(board, original);
 98 |     assert_eq!(board.zobrist(), original.zobrist());
 99 |     assert_eq!(initial.guard.outcome, GuardOutcome::Disabled);
100 | }
```

When the opponent has two winning endpoints, a defensive block is insufficient. This test refutes the original proposal, then accepts an immediate own win as terminal rather than running an unnecessary opponent probe.

**Source:** `gomoku/crates/player/tests/guard.rs`, lines 102–124.

```text
102 | #[test]
103 | fn two_opponent_winning_threats_require_our_immediate_winning_alternative() {
104 |     let board = position(
105 |         &[(7, 2), (7, 3), (7, 4), (7, 5)],
106 |         &[(9, 3), (9, 4), (9, 5), (9, 6)],
107 |     );
108 |     let winning = mv(7, 6);
109 |     let mut after_bad = board.clone();
110 |     after_bad.play(mv(0, 0)).unwrap();
111 |     assert_eq!(engine::immediate_wins(&after_bad, Color::White).len(), 2);
112 |     let decision = AiPlayer::new(Box::new(Prefer(winning)))
113 |         .decide(
114 |             &board,
115 |             &options(GuardConfig::baseline()),
116 |             &mut StdRng::seed_from_u64(1),
117 |         )
118 |         .unwrap();
119 |     assert_eq!(decision.guard.proposal, mv(0, 0));
120 |     assert_eq!(decision.guard.refuted, vec![mv(0, 0)]);
121 |     assert_eq!(decision.mv, winning);
122 |     assert_eq!(decision.guard.outcome, GuardOutcome::Terminal);
123 |     assert_eq!(decision.guard.checked, 2);
124 | }
```

This additional regression separates two distinct winning lines rather than two endpoints of one line. Blocking a1 leaves k1 winning for White; the verified one-move continuation refutes that block, and Black's f10 wins immediately. Complete board/statistics/RNG assertions prevent the guard from manufacturing a stronger search result.

**Source:** `gomoku/crates/player/tests/guard.rs`, lines 126–209.

```text
126 | #[test]
127 | fn blocking_one_of_two_distinct_opponent_lines_is_refuted_in_favor_of_own_immediate_win() {
128 |     // White's horizontal b1-e1 needs a1 (f1 is Black); its vertical
129 |     // k2-k5 needs k1 (k6 is Black). Blocking one does not cover the other.
130 |     let board = position(
131 |         &[
132 |             (0, 5),
133 |             (5, 10),
134 |             (13, 5),
135 |             (13, 7),
136 |             (9, 1),
137 |             (9, 2),
138 |             (9, 3),
139 |             (9, 4),
140 |         ],
141 |         &[
142 |             (0, 1),
143 |             (0, 2),
144 |             (0, 3),
145 |             (0, 4),
146 |             (1, 10),
147 |             (2, 10),
148 |             (3, 10),
149 |             (4, 10),
150 |         ],
151 |     );
152 |     let original = board.clone();
153 |     let proposal = mv(0, 0); // a1: lowest-index legal move at zero visits.
154 |     let remaining_win = mv(0, 10); // k1: the distinct vertical line.
155 |     let winning = mv(9, 5); // f10: Black completes b10-f10.
156 |     let guard = GuardConfig {
157 |         enabled: true,
158 |         max_candidates: 2,
159 |         budget: SearchBudget {
160 |             max_nodes: 1_000,
161 |             max_depth: 10,
162 |         },
163 |     };
164 |     assert_eq!(
165 |         engine::immediate_wins(&board, Color::White)
166 |             .iter()
167 |             .collect::<Vec<_>>(),
168 |         vec![proposal, remaining_win]
169 |     );
170 |     let mut after_block = board.clone();
171 |     after_block.play(proposal).unwrap();
172 |     assert_eq!(
173 |         engine::immediate_wins(&after_block, Color::White)
174 |             .iter()
175 |             .collect::<Vec<_>>(),
176 |         vec![remaining_win]
177 |     );
178 |     let proof =
179 |         engine::prove_forced_win(&after_block, after_block.to_move(), guard.budget).unwrap();
180 |     assert_eq!(proof.winner, Color::White);
181 |     assert_eq!(proof.line, vec![remaining_win]);
182 |     assert!(engine::verify_line(&after_block, &proof));
183 |
184 |     let mut plain_rng = StdRng::seed_from_u64(1);
185 |     let mut guarded_rng = StdRng::seed_from_u64(1);
186 |     let plain = AiPlayer::new(Box::new(Prefer(winning)))
187 |         .decide(&board, &options(GuardConfig::disabled()), &mut plain_rng)
188 |         .unwrap();
189 |     let guarded = AiPlayer::new(Box::new(Prefer(winning)))
190 |         .decide(&board, &options(guard), &mut guarded_rng)
191 |         .unwrap();
192 |     assert_eq!(plain.mv, proposal);
193 |     assert_eq!(plain.guard.outcome, GuardOutcome::Disabled);
194 |     let stats = plain.search.as_ref().unwrap();
195 |     assert!(stats.visits.iter().all(|&(_, visits)| visits == 0));
196 |     assert_eq!(stats.prior[0].0, winning);
197 |     assert_eq!(guarded.guard.proposal, proposal);
198 |     assert_eq!(guarded.guard.refuted, vec![proposal]);
199 |     assert_eq!(guarded.guard.checked, 2);
200 |     assert_eq!(guarded.guard.outcome, GuardOutcome::Terminal);
201 |     assert_eq!(guarded.mv, winning);
202 |     assert_eq!(guarded.search, plain.search);
203 |     assert_eq!(plain_rng.random::<u64>(), guarded_rng.random::<u64>());
204 |     let mut after_win = board.clone();
205 |     after_win.play(guarded.mv).unwrap();
206 |     assert_eq!(after_win.status(), Status::Won(Color::Black));
207 |     assert_eq!(board, original);
208 |     assert_eq!(board.zobrist(), original.zobrist());
209 | }
```

With White to move, verify that the guard actually probes Black after the hypothetical move. This catches hard-coded opponent-color assumptions that a Black-only fixture cannot detect.

**Source:** `gomoku/crates/player/tests/guard.rs`, lines 211–232.

```text
211 | #[test]
212 | fn white_player_guard_probes_actual_black_opponent() {
213 |     let board = Board::from_position(
214 |         &[mv(7, 3), mv(7, 4), mv(7, 5), mv(7, 6), mv(14, 14)],
215 |         &[mv(7, 2), mv(2, 0), mv(2, 2), mv(2, 4)],
216 |         Color::White,
217 |     )
218 |     .unwrap();
219 |     let block = mv(7, 7);
220 |     let original = board.clone();
221 |     let decision = AiPlayer::new(Box::new(Prefer(block)))
222 |         .decide(
223 |             &board,
224 |             &options(GuardConfig::baseline()),
225 |             &mut StdRng::seed_from_u64(1),
226 |         )
227 |         .unwrap();
228 |     assert_eq!(decision.guard.refuted, vec![mv(0, 0)]);
229 |     assert_eq!(decision.mv, block);
230 |     assert_eq!(decision.guard.outcome, GuardOutcome::Unrefuted);
231 |     assert_eq!(board, original);
232 | }
```

### Missing evidence and candidate caps are not safety proofs

A zero-depth probe cannot establish the opponent win even on this immediate-win fixture. The proposal remains unknown with one check, not refuted or certified safe.

**Source:** `gomoku/crates/player/tests/guard.rs`, lines 234–251.

```text
234 | #[test]
235 | fn zero_depth_probe_is_unknown_even_with_opponent_immediate_win() {
236 |     let board = single_threat();
237 |     let config = GuardConfig {
238 |         budget: SearchBudget {
239 |             max_nodes: 1_000,
240 |             max_depth: 0,
241 |         },
242 |         ..GuardConfig::baseline()
243 |     };
244 |     let decision = AiPlayer::new(Box::new(Prefer(mv(7, 7))))
245 |         .decide(&board, &options(config), &mut StdRng::seed_from_u64(1))
246 |         .unwrap();
247 |     assert_eq!(decision.mv, mv(0, 0));
248 |     assert_eq!(decision.guard.outcome, GuardOutcome::Unrefuted);
249 |     assert_eq!(decision.guard.checked, 1);
250 |     assert!(decision.guard.refuted.is_empty());
251 | }
```

A zero-node probe has the same unknown semantics. This independent budget case prevents the node cap from being treated as evidence about the position.

**Source:** `gomoku/crates/player/tests/guard.rs`, lines 253–270.

```text
253 | #[test]
254 | fn zero_node_probe_is_unknown_even_with_opponent_immediate_win() {
255 |     let board = single_threat();
256 |     let config = GuardConfig {
257 |         budget: SearchBudget {
258 |             max_nodes: 0,
259 |             max_depth: 10,
260 |         },
261 |         ..GuardConfig::baseline()
262 |     };
263 |     let decision = AiPlayer::new(Box::new(UniformEvaluator::new(0.0)))
264 |         .decide(&board, &options(config), &mut StdRng::seed_from_u64(1))
265 |         .unwrap();
266 |     assert_eq!(decision.mv, mv(0, 0));
267 |     assert_eq!(decision.guard.outcome, GuardOutcome::Unrefuted);
268 |     assert_eq!(decision.guard.checked, 1);
269 |     assert!(decision.guard.refuted.is_empty());
270 | }
```

After one verified refutation and a one-candidate cap, return the next ranked block untested. The outcome and count distinguish this from either an accepted probed defense or fallback to the known-refuted proposal.

**Source:** `gomoku/crates/player/tests/guard.rs`, lines 272–287.

```text
272 | #[test]
273 | fn cap_falls_back_to_unchecked_block_not_refuted_original() {
274 |     let board = single_threat();
275 |     let block = mv(7, 7);
276 |     let guard = GuardConfig {
277 |         max_candidates: 1,
278 |         ..GuardConfig::baseline()
279 |     };
280 |     let decision = AiPlayer::new(Box::new(Prefer(block)))
281 |         .decide(&board, &options(guard), &mut StdRng::seed_from_u64(1))
282 |         .unwrap();
283 |     assert_eq!(decision.mv, block);
284 |     assert_eq!(decision.guard.outcome, GuardOutcome::BudgetExhausted);
285 |     assert_eq!(decision.guard.checked, 1);
286 |     assert_eq!(decision.guard.refuted, vec![mv(0, 0)]);
287 | }
```

With no candidate capacity, retain the original proposal unchecked and report BudgetExhausted with zero checks/refutations. No proof may be inferred from disabling all work.

**Source:** `gomoku/crates/player/tests/guard.rs`, lines 289–303.

```text
289 | #[test]
290 | fn zero_candidate_cap_keeps_original_unchecked() {
291 |     let board = single_threat();
292 |     let guard = GuardConfig {
293 |         max_candidates: 0,
294 |         ..GuardConfig::baseline()
295 |     };
296 |     let decision = AiPlayer::new(Box::new(Prefer(mv(7, 7))))
297 |         .decide(&board, &options(guard), &mut StdRng::seed_from_u64(1))
298 |         .unwrap();
299 |     assert_eq!(decision.mv, mv(0, 0));
300 |     assert_eq!(decision.guard.checked, 0);
301 |     assert!(decision.guard.refuted.is_empty());
302 |     assert_eq!(decision.guard.outcome, GuardOutcome::BudgetExhausted);
303 | }
```

Only this exhaustive legal-move fixture reaches AllRefuted. Independently re-prove and verify every recorded candidate refutation, then assert the original legal fallback and unchanged input. This is still diagnostics, not a root loss certificate.

**Source:** `gomoku/crates/player/tests/guard.rs`, lines 305–337.

```text
305 | #[test]
306 | fn every_legal_move_really_refuted_reports_all_refuted_and_original_fallback() {
307 |     // Two independent winning endpoints: neither can be covered with one move.
308 |     let board = position(
309 |         &[(2, 0), (2, 2), (2, 4), (2, 6)],
310 |         &[(7, 3), (7, 4), (7, 5), (7, 6)],
311 |     );
312 |     let original = board.clone();
313 |     let guard = GuardConfig {
314 |         max_candidates: board.empty_moves().count(),
315 |         budget: SearchBudget {
316 |             max_nodes: 1,
317 |             max_depth: 1,
318 |         },
319 |         ..GuardConfig::baseline()
320 |     };
321 |     let decision = AiPlayer::new(Box::new(UniformEvaluator::new(0.0)))
322 |         .decide(&board, &options(guard), &mut StdRng::seed_from_u64(1))
323 |         .unwrap();
324 |     assert_eq!(decision.guard.outcome, GuardOutcome::AllRefuted);
325 |     assert_eq!(decision.guard.checked, board.empty_moves().count());
326 |     assert_eq!(decision.guard.refuted.len(), board.empty_moves().count());
327 |     for candidate in board.empty_moves() {
328 |         assert!(decision.guard.refuted.contains(&candidate));
329 |         let mut after = board.clone();
330 |         after.play(candidate).unwrap();
331 |         let proof = engine::prove_forced_win(&after, after.to_move(), guard.budget).unwrap();
332 |         assert!(engine::verify_line(&after, &proof));
333 |     }
334 |     assert_eq!(decision.mv, decision.guard.proposal);
335 |     assert!(board.is_legal(decision.mv));
336 |     assert_eq!(board, original);
337 | }
```

Accept an immediate own win even when both prover bounds are zero. The terminal check must precede probing, so a missing proof cannot override a game already won by the candidate.

**Source:** `gomoku/crates/player/tests/guard.rs`, lines 339–362.

```text
339 | #[test]
340 | fn own_immediate_win_is_terminal_not_opponent_probe_unknown() {
341 |     let board = position(
342 |         &[(0, 1), (0, 2), (0, 3), (0, 4)],
343 |         &[(2, 0), (2, 2), (2, 4), (2, 6)],
344 |     );
345 |     let guard = GuardConfig {
346 |         budget: SearchBudget {
347 |             max_nodes: 0,
348 |             max_depth: 0,
349 |         },
350 |         ..GuardConfig::baseline()
351 |     };
352 |     let decision = AiPlayer::new(Box::new(UniformEvaluator::new(0.0)))
353 |         .decide(&board, &options(guard), &mut StdRng::seed_from_u64(1))
354 |         .unwrap();
355 |     assert_eq!(decision.mv, mv(0, 0));
356 |     assert_eq!(decision.guard.outcome, GuardOutcome::Terminal);
357 |     assert_eq!(decision.guard.checked, 1);
358 |     assert!(decision.guard.refuted.is_empty());
359 |     let mut after = board.clone();
360 |     after.play(decision.mv).unwrap();
361 |     assert_eq!(after.status(), Status::Won(Color::Black));
362 | }
```

An enabled guard with unknown probes preserves the original seeded proposal. Compare zero and eleven simulations under noisy, sampled settings, including complete statistics, input board, and next RNG draw. Retrying must not consume extra randomness.

**Source:** `gomoku/crates/player/tests/guard.rs`, lines 364–398.

```text
364 | #[test]
365 | fn enabled_unknown_preserves_seeded_first_candidate_noise_stats_and_rng() {
366 |     let board = Board::new();
367 |     let original = board.clone();
368 |     for simulations in [0, 11] {
369 |         let mut plain_options = options(GuardConfig::disabled());
370 |         plain_options.simulations = simulations;
371 |         plain_options.temperature = 0.7;
372 |         plain_options.noise_epsilon = 0.25;
373 |         let guarded_options = PlayerOptions {
374 |             guard: GuardConfig {
375 |                 budget: SearchBudget {
376 |                     max_nodes: 0,
377 |                     max_depth: 10,
378 |                 },
379 |                 ..GuardConfig::baseline()
380 |             },
381 |             ..plain_options
382 |         };
383 |         let mut plain_rng = StdRng::seed_from_u64(42);
384 |         let mut guarded_rng = StdRng::seed_from_u64(42);
385 |         let plain = AiPlayer::new(Box::new(UniformEvaluator::new(0.3)))
386 |             .decide(&board, &plain_options, &mut plain_rng)
387 |             .unwrap();
388 |         let guarded = AiPlayer::new(Box::new(UniformEvaluator::new(0.3)))
389 |             .decide(&board, &guarded_options, &mut guarded_rng)
390 |             .unwrap();
391 |         assert_eq!(guarded.mv, plain.mv);
392 |         assert_eq!(guarded.guard.proposal, plain.mv);
393 |         assert_eq!(guarded.guard.outcome, GuardOutcome::Unrefuted);
394 |         assert_eq!(guarded.search, plain.search);
395 |         assert_eq!(plain_rng.random::<u64>(), guarded_rng.random::<u64>());
396 |         assert_eq!(board, original);
397 |     }
398 | }
```

### Private tests isolate admission and bookkeeping

Append these fragments to the same production guard module, after `choose`.
The callback is private test injection, not another evaluator hook. Public
real-prover tests establish integration; these five tests establish ordering,
malformed-proof rejection, and branches that must never call the prover.

Open the private test module and define all its helpers. `edges` constructs every legal root edge, including unvisited moves, and the two-ended-four fixture lets the real prover refute controlled candidates cheaply. The final draw fragment closes this module.

**Source:** `gomoku/crates/player/src/guard.rs`, lines 166–194.

```text
166 | #[cfg(test)]
167 | mod tests {
168 |     use super::*;
169 |
170 |     fn mv(row: u8, col: u8) -> Move {
171 |         Move::new(row, col).unwrap()
172 |     }
173 |
174 |     fn two_ended_four() -> Board {
175 |         Board::from_position(
176 |             &[mv(2, 0), mv(2, 2), mv(2, 4), mv(2, 6)],
177 |             &[mv(7, 3), mv(7, 4), mv(7, 5), mv(7, 6)],
178 |             Color::Black,
179 |         )
180 |         .unwrap()
181 |     }
182 |
183 |     fn edges(board: &Board) -> Vec<Edge> {
184 |         board
185 |             .empty_moves()
186 |             .map(|mv| Edge {
187 |                 mv,
188 |                 prior: 0.0,
189 |                 n: 0,
190 |                 w: 0.0,
191 |                 child: None,
192 |             })
193 |             .collect()
194 |     }
```

Reverse edge creation order, then independently control visits, priors, and indices. Assert exact proposal-first retry order, actual opponent, forwarded budget, immutable edges/board, and selection of an unvisited untested candidate when the cap is reached.

**Source:** `gomoku/crates/player/src/guard.rs`, lines 196–239.

```text
196 |     #[test]
197 |     fn alternatives_rank_visits_then_prior_then_index_without_repeats_and_include_unvisited() {
198 |         let board = two_ended_four();
199 |         let original = board.clone();
200 |         let proposal = mv(0, 4);
201 |         let mut edges = edges(&board);
202 |         for (candidate, n, prior) in [
203 |             (proposal, 100, 1.0),
204 |             (mv(0, 3), 4, 0.1),
205 |             (mv(0, 2), 3, 0.9),
206 |             (mv(0, 1), 3, 0.9),
207 |             (mv(0, 0), 3, 0.5),
208 |             (mv(0, 5), 0, 0.9),
209 |         ] {
210 |             let edge = edges.iter_mut().find(|e| e.mv == candidate).unwrap();
211 |             edge.n = n;
212 |             edge.prior = prior;
213 |         }
214 |         edges.reverse(); // Creation order must not leak into tie-breaking.
215 |         let before = edges.clone();
216 |         let mut seen = Vec::new();
217 |         let config = GuardConfig {
218 |             budget: SearchBudget {
219 |                 max_nodes: 1,
220 |                 max_depth: 1,
221 |             },
222 |             ..GuardConfig::baseline()
223 |         };
224 |         let (chosen, diagnostics) =
225 |             choose(&board, proposal, &edges, config, |after, side, budget| {
226 |                 seen.push(*after.moves().last().unwrap());
227 |                 assert_eq!(side, after.to_move());
228 |                 assert_eq!(side, Color::White);
229 |                 assert_eq!(budget, config.budget);
230 |                 engine::prove_forced_win(after, side, budget)
231 |             });
232 |         assert_eq!(seen, [proposal, mv(0, 3), mv(0, 1), mv(0, 2), mv(0, 0)]);
233 |         assert_eq!(diagnostics.refuted, seen);
234 |         assert_eq!(diagnostics.checked, 5);
235 |         assert_eq!(diagnostics.outcome, GuardOutcome::BudgetExhausted);
236 |         assert_eq!(chosen, mv(0, 5)); // Unvisited, untested, not a known loser.
237 |         assert_eq!(edges, before);
238 |         assert_eq!(board, original);
239 |     }
```

Inject wrong-winner, empty, occupied-first-move, just-played-first-move, and unverified proofs. Each must leave the proposal unknown with exactly one probe, rather than merely trusting Some(proof).

**Source:** `gomoku/crates/player/src/guard.rs`, lines 241–285.

```text
241 |     #[test]
242 |     fn malformed_or_unverified_proofs_leave_candidate_unknown() {
243 |         let board = Board::from_position(&[mv(7, 3)], &[mv(7, 4)], Color::Black).unwrap();
244 |         let proposal = mv(0, 0);
245 |         let edges = edges(&board);
246 |         for proof in [
247 |             Proof {
248 |                 winner: Color::Black,
249 |                 line: vec![mv(7, 7)],
250 |             },
251 |             Proof {
252 |                 winner: Color::White,
253 |                 line: vec![],
254 |             },
255 |             Proof {
256 |                 winner: Color::White,
257 |                 line: vec![mv(7, 3)],
258 |             },
259 |             Proof {
260 |                 winner: Color::White,
261 |                 line: vec![proposal],
262 |             },
263 |             Proof {
264 |                 winner: Color::White,
265 |                 line: vec![mv(1, 0)],
266 |             },
267 |         ] {
268 |             let mut calls = 0;
269 |             let (chosen, diagnostics) = choose(
270 |                 &board,
271 |                 proposal,
272 |                 &edges,
273 |                 GuardConfig::baseline(),
274 |                 |_, _, _| {
275 |                     calls += 1;
276 |                     Some(proof.clone())
277 |                 },
278 |             );
279 |             assert_eq!(chosen, proposal);
280 |             assert_eq!(diagnostics.outcome, GuardOutcome::Unrefuted);
281 |             assert_eq!(diagnostics.checked, 1);
282 |             assert!(diagnostics.refuted.is_empty());
283 |             assert_eq!(calls, 1);
284 |         }
285 |     }
```

Inject a panic callback into disabled and zero-cap paths. Both must avoid probing while retaining their distinct diagnostics, counts, and original proposal.

**Source:** `gomoku/crates/player/src/guard.rs`, lines 287–310.

```text
287 |     #[test]
288 |     fn disabled_and_zero_cap_never_call_probe() {
289 |         let board = two_ended_four();
290 |         let proposal = mv(0, 0);
291 |         let edges = edges(&board);
292 |         for (config, outcome) in [
293 |             (GuardConfig::disabled(), GuardOutcome::Disabled),
294 |             (
295 |                 GuardConfig {
296 |                     max_candidates: 0,
297 |                     ..GuardConfig::baseline()
298 |                 },
299 |                 GuardOutcome::BudgetExhausted,
300 |             ),
301 |         ] {
302 |             let (chosen, diagnostics) = choose(&board, proposal, &edges, config, |_, _, _| {
303 |                 panic!("no probe allowed")
304 |             });
305 |             assert_eq!(chosen, proposal);
306 |             assert_eq!(diagnostics.outcome, outcome);
307 |             assert_eq!(diagnostics.checked, 0);
308 |             assert!(diagnostics.refuted.is_empty());
309 |         }
310 |     }
```

An immediate own win must also avoid the injected panic callback. The hypothetical candidate is checked once and accepted as terminal before any opponent proof request.

**Source:** `gomoku/crates/player/src/guard.rs`, lines 312–331.

```text
312 |     #[test]
313 |     fn immediate_win_never_calls_opponent_probe() {
314 |         let board = Board::from_position(
315 |             &[mv(0, 1), mv(0, 2), mv(0, 3), mv(0, 4)],
316 |             &[mv(2, 0), mv(2, 2), mv(2, 4), mv(2, 6)],
317 |             Color::Black,
318 |         )
319 |         .unwrap();
320 |         let proposal = mv(0, 0);
321 |         let (chosen, diagnostics) = choose(
322 |             &board,
323 |             proposal,
324 |             &edges(&board),
325 |             GuardConfig::baseline(),
326 |             |_, _, _| panic!("terminal candidate must not be probed"),
327 |         );
328 |         assert_eq!(chosen, proposal);
329 |         assert_eq!(diagnostics.outcome, GuardOutcome::Terminal);
330 |         assert_eq!(diagnostics.checked, 1);
331 |     }
```

**Preexisting engine limitation, not fixed in this chapter:**
`Board::from_position` stores imported stones without reconstructing the move
history. Subsequent `Board::play` detects a draw using `moves.len() == 225`,
so filling the final square of such an imported almost-full board does not
exercise normal replay-built draw bookkeeping. The next fixture deliberately
replays all 224 preceding plies. This chapter does not alter engine draw
semantics or claim to repair imported-board history.

Replay a valid non-winning near-full board, choose its only remaining cell, and forbid opponent probing. The assertions check Terminal, one hypothetical check, an unchanged input, and an actual draw after playing the result on a clone. The final brace closes the private test module opened above.

**Source:** `gomoku/crates/player/src/guard.rs`, lines 333–376.

```text
333 |     #[test]
334 |     fn immediate_draw_never_calls_opponent_probe() {
335 |         let proposal = mv(0, 0);
336 |         let mut black = Vec::new();
337 |         let mut white = Vec::new();
338 |         for r in 0..15 {
339 |             for c in 0..15 {
340 |                 let stone = mv(r, c);
341 |                 if stone == proposal {
342 |                     continue;
343 |                 }
344 |                 // Repeating pairs avoid five on rows, columns and diagonals.
345 |                 if (r + 2 * c) % 4 < 2 {
346 |                     black.push(stone);
347 |                 } else {
348 |                     white.push(stone);
349 |                 }
350 |             }
351 |         }
352 |         // Replay stones so the engine also has the full pre-draw move history.
353 |         let mut board = Board::new();
354 |         for (black, white) in black.into_iter().zip(white) {
355 |             board.play(black).unwrap();
356 |             board.play(white).unwrap();
357 |         }
358 |         assert_eq!(board.status(), Status::Ongoing);
359 |         let original = board.clone();
360 |         let (chosen, diagnostics) = choose(
361 |             &board,
362 |             proposal,
363 |             &edges(&board),
364 |             GuardConfig::baseline(),
365 |             |_, _, _| panic!("draw must not be probed"),
366 |         );
367 |         assert_eq!(chosen, proposal);
368 |         assert_eq!(diagnostics.outcome, GuardOutcome::Terminal);
369 |         assert_eq!(diagnostics.checked, 1);
370 |         assert!(diagnostics.refuted.is_empty());
371 |         assert_eq!(board, original);
372 |         let mut after = board.clone();
373 |         after.play(chosen).unwrap();
374 |         assert_eq!(after.status(), Status::Draw);
375 |     }
376 | }
```

## 8. Executable specifications: CLI ownership and seeded behavior

### Session test context and observable runtime doubles

The next fragments are additions inside the **existing** test module of
`gomoku/crates/cli/src/session.rs`. Keep its other tests and helper functions.
There is no second nested `mod tests` to add.

The existing session test-module header/imports are shown for placement and complete name resolution. `super::*` supplies session/actor/command/RNG names; `PlayError` is retained for older tests, and `testkit::mv` is the existing coordinate helper.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 835–839.

```text
835 | #[cfg(test)]
836 | mod tests {
837 |     use super::*;
838 |     use engine::PlayError;
839 |     use testkit::mv;
```

This unchanged shared testkit helper supplies the `mv` imported by both CLI test groups. Its complete body is included so no fixture builder needed by the new tests is hidden behind an external source link. Do not create another copy in testkit.

**Source:** `gomoku/crates/testkit/src/lib.rs`, lines 13–16.

```text
 13 | /// Build a `Move` from zero-based row/column, panicking if out of range.
 14 | pub fn mv(r: u8, c: u8) -> Move {
 15 |     Move::new(r, c).unwrap()
 16 | }
```

Add the counting evaluator and its Drop implementation inside the existing session test module. It forwards real tactics estimates while separately counting calls and drops, allowing lifetime assertions without making production evaluator state accessible.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 858–875.

```text
858 |     struct CountingEvaluator {
859 |         calls: std::rc::Rc<std::cell::Cell<u32>>,
860 |         drops: std::rc::Rc<std::cell::Cell<u32>>,
861 |         inner: mcts::TacticsEvaluator,
862 |     }
863 |
864 |     impl mcts::Evaluator for CountingEvaluator {
865 |         fn evaluate(&mut self, request: mcts::EvalRequest) -> mcts::EvalResult {
866 |             self.calls.set(self.calls.get() + 1);
867 |             mcts::Evaluator::evaluate(&mut self.inner, request)
868 |         }
869 |     }
870 |
871 |     impl Drop for CountingEvaluator {
872 |         fn drop(&mut self) {
873 |             self.drops.set(self.drops.get() + 1);
874 |         }
875 |     }
```

Step, backtrack, change simulations, and step again with the injected evaluator. Cumulative evaluation counts and real visit totals prove that SetSims changes per-decision work without replacement; taking the seat then drops exactly that runtime.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 877–913.

```text
877 |     #[test]
878 |     fn persistent_player_reuses_evaluator_across_steps_and_set_sims() {
879 |         let config = AiConfig {
880 |             simulations: 1,
881 |             ..AiConfig::default()
882 |         };
883 |         let mut core = SessionCore::new(SessionInit::versus_ai(Color::White, config));
884 |         let calls = std::rc::Rc::new(std::cell::Cell::new(0));
885 |         let drops = std::rc::Rc::new(std::cell::Cell::new(0));
886 |         core.black_player = Some(player::AiPlayer::new(Box::new(CountingEvaluator {
887 |             calls: calls.clone(),
888 |             drops: drops.clone(),
889 |             inner: mcts::TacticsEvaluator::new(),
890 |         })));
891 |
892 |         core.execute(Command::Step).unwrap();
893 |         assert_eq!(calls.get(), 2, "root plus one simulation");
894 |         core.execute(Command::Back { n: 1 }).unwrap();
895 |         core.execute(Command::SetSims {
896 |             seat: SeatSel::Black,
897 |             n: 3,
898 |         })
899 |         .unwrap();
900 |         assert_eq!(drops.get(), 0, "SetSims must retain the evaluator");
901 |         core.execute(Command::Step).unwrap();
902 |         assert_eq!(calls.get(), 6, "same evaluator: root plus three more sims");
903 |         let info = core.search_info().unwrap();
904 |         assert_eq!(info.config.simulations, 3);
905 |         assert_eq!(info.visits.iter().map(|(_, n)| n).sum::<u32>(), 3);
906 |
907 |         core.execute(Command::Take {
908 |             seat: SeatSel::Black,
909 |         })
910 |         .unwrap();
911 |         assert_eq!(drops.get(), 1, "taking a seat drops its evaluator");
912 |         assert!(core.black_player.is_none());
913 |     }
```

Exercise initialization, release of the other seat, both-seat SetSims, New/reset, take, and release. Drop counts establish resource replacement, while occupants and post-reset search configuration establish restoration of initial seats and simulation settings.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 915–974.

```text
915 |     #[test]
916 |     fn persistent_players_follow_init_release_take_and_reset() {
917 |         let config = AiConfig {
918 |             simulations: 1,
919 |             ..AiConfig::default()
920 |         };
921 |         let init = SessionInit::versus_ai(Color::White, config);
922 |         let mut core = SessionCore::new(init.clone());
923 |         assert!(core.black_player.is_some());
924 |         assert!(core.white_player.is_none());
925 |
926 |         let calls = std::rc::Rc::new(std::cell::Cell::new(0));
927 |         let drops = std::rc::Rc::new(std::cell::Cell::new(0));
928 |         core.black_player = Some(player::AiPlayer::new(Box::new(CountingEvaluator {
929 |             calls: calls.clone(),
930 |             drops: drops.clone(),
931 |             inner: mcts::TacticsEvaluator::new(),
932 |         })));
933 |         core.execute(Command::Release {
934 |             seat: SeatSel::White,
935 |         })
936 |         .unwrap();
937 |         assert_eq!(drops.get(), 0, "releasing White must retain Black's player");
938 |         assert!(core.white_player.is_some());
939 |         core.white_player = Some(player::AiPlayer::new(Box::new(CountingEvaluator {
940 |             calls,
941 |             drops: drops.clone(),
942 |             inner: mcts::TacticsEvaluator::new(),
943 |         })));
944 |         core.execute(Command::SetSims {
945 |             seat: SeatSel::Both,
946 |             n: 3,
947 |         })
948 |         .unwrap();
949 |         assert_eq!(drops.get(), 0);
950 |
951 |         assert_eq!(core.execute(Command::New).unwrap(), vec![Event::Reset]);
952 |         assert_eq!(drops.get(), 2, "reset discards both old evaluators");
953 |         assert!(core.black_player.is_some());
954 |         assert!(core.white_player.is_none());
955 |         assert_eq!(core.black(), &init.black);
956 |         assert_eq!(core.white(), &init.white);
957 |         core.execute(Command::Step).unwrap();
958 |         assert_eq!(core.search_info().unwrap().config.simulations, 1);
959 |
960 |         core.execute(Command::Take {
961 |             seat: SeatSel::Both,
962 |         })
963 |         .unwrap();
964 |         assert!(core.black_player.is_none());
965 |         assert!(core.white_player.is_none());
966 |         core.execute(Command::Release {
967 |             seat: SeatSel::Both,
968 |         })
969 |         .unwrap();
970 |         assert!(core.black_player.is_some());
971 |         assert!(core.white_player.is_some());
972 |         core.execute(Command::Step).unwrap();
973 |         assert_eq!(core.search_info().unwrap().config.simulations, 1);
974 |     }
```

Add a separate root-proof evaluator double inside that test module. A root proof asserts legal membership and returns the certified move; the leaf method panics to catch accidental searched-path work.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 976–989.

```text
976 |     struct WinningEvaluator {
977 |         mv: Move,
978 |     }
979 |
980 |     impl mcts::Evaluator for WinningEvaluator {
981 |         fn evaluate(&mut self, _: mcts::EvalRequest) -> mcts::EvalResult {
982 |             panic!("a root proof must bypass leaf evaluation");
983 |         }
984 |
985 |         fn evaluate_root(&mut self, request: mcts::EvalRequest) -> mcts::RootEval {
986 |             assert!(request.legal.contains(&self.mv));
987 |             mcts::RootEval::ProvenWin { mv: self.mv }
988 |         }
989 |     }
```

On an immediate-win replay fixture, assert the complete AI move and GameOver event sequence, terminal board, no search record at the new cursor, and unchanged RNG stream. This checks the CLI proof route in addition to the public player test.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 991–1039.

```text
991 |     #[test]
992 |     fn persistent_player_proof_plays_without_search_info_or_rng_draws() {
993 |         use rand::RngExt;
994 |
995 |         let mut board = Board::new();
996 |         for (r, c) in [
997 |             (7, 3),
998 |             (0, 0),
999 |             (7, 4),
1000 |             (0, 2),
1001 |             (7, 5),
1002 |             (0, 4),
1003 |             (7, 6),
1004 |             (0, 6),
1005 |         ] {
1006 |             board.play(mv(r, c)).unwrap();
1007 |         }
1008 |         let config = AiConfig {
1009 |             temperature: 1.0,
1010 |             noise_epsilon: 0.25,
1011 |             ..AiConfig::default()
1012 |         };
1013 |         let mut core = SessionCore::new(SessionInit {
1014 |             opening: board,
1015 |             seed: Some(42),
1016 |             ..SessionInit::versus_ai(Color::White, config)
1017 |         });
1018 |         let winning_move = mv(7, 7);
1019 |         core.black_player = Some(player::AiPlayer::new(Box::new(WinningEvaluator {
1020 |             mv: winning_move,
1021 |         })));
1022 |         let mut untouched_rng = StdRng::seed_from_u64(42);
1023 |
1024 |         assert_eq!(
1025 |             core.execute(Command::Step).unwrap(),
1026 |             vec![
1027 |                 Event::Moved {
1028 |                     mv: winning_move,
1029 |                     by: OccupantKind::Ai
1030 |                 },
1031 |                 Event::GameOver {
1032 |                     result: Status::Won(Color::Black)
1033 |                 },
1034 |             ]
1035 |         );
1036 |         assert_eq!(core.board().status(), Status::Won(Color::Black));
1037 |         assert!(core.search_info().is_none());
1038 |         assert_eq!(core.rng.random::<u64>(), untouched_rng.random::<u64>());
1039 |     }
```

Pin the CLI evaluator-kind policy with a cheap injected estimate-only evaluator. Initialize a tactics seat to avoid constructing Burn, then label/inject each configured kind: baseline selects the guarded block, tactics retains the proposal, and both preserve real zero-visit search info. This is a policy-mapping test, not execution of the actual baseline evaluator.

**Source:** `gomoku/crates/cli/src/session.rs`, lines 1041–1089.

```text
1041 |     #[test]
1042 |     fn baseline_enables_defensive_guard_but_tactics_preserves_proposal() {
1043 |         struct PreferBlock;
1044 |         impl mcts::Evaluator for PreferBlock {
1045 |             fn evaluate(&mut self, _: mcts::EvalRequest) -> mcts::EvalResult {
1046 |                 let mut policy = [0.0; 225];
1047 |                 policy[mv(7, 7).index()] = 10.0;
1048 |                 mcts::EvalResult { policy, value: 0.0 }
1049 |             }
1050 |         }
1051 |         let board = Board::from_position(
1052 |             &[mv(7, 2), mv(2, 0), mv(2, 2), mv(2, 4)],
1053 |             &[mv(7, 3), mv(7, 4), mv(7, 5), mv(7, 6)],
1054 |             Color::Black,
1055 |         )
1056 |         .unwrap();
1057 |         for (kind, expected) in [
1058 |             (EvaluatorKind::Baseline, mv(7, 7)),
1059 |             (EvaluatorKind::Tactics, mv(0, 0)),
1060 |         ] {
1061 |             // Initialize a cheap tactics seat, then inject a fixed evaluator.
1062 |             // This tests the CLI policy mapping without running Burn in debug.
1063 |             let config = AiConfig {
1064 |                 evaluator: EvaluatorKind::Tactics,
1065 |                 simulations: 0,
1066 |                 explicit_selection: true,
1067 |                 ..AiConfig::default()
1068 |             };
1069 |             let mut core = SessionCore::new(SessionInit {
1070 |                 opening: board.clone(),
1071 |                 seed: Some(42),
1072 |                 ..SessionInit::versus_ai(Color::White, config.clone())
1073 |             });
1074 |             core.black = Occupant::Ai(AiConfig {
1075 |                 evaluator: kind,
1076 |                 ..config
1077 |             });
1078 |             core.black_player = Some(player::AiPlayer::new(Box::new(PreferBlock)));
1079 |             let events = core.execute(Command::Step).unwrap();
1080 |             assert!(events.contains(&Event::Moved {
1081 |                 mv: expected,
1082 |                 by: OccupantKind::Ai
1083 |             }));
1084 |             assert_eq!(core.board().stone_at(expected), Some(Color::Black));
1085 |             let info = core.search_info().expect("guard keeps real search info");
1086 |             assert!(info.visits.iter().all(|(_, n)| *n == 0));
1087 |             assert_eq!(info.prior[0].0, mv(7, 7));
1088 |         }
1089 |     }
```

### Compare eleven plies against the pre-player path

Add only the following regression to the existing actor integration-test
file. This compares tactics seats against a freshly constructed legacy-path
evaluator each ply, including the change from scheduled selection on plies
zero through nine to configured selection on the eleventh move.

Retain these existing actor-test imports. The new regression additionally imports SearchInfo and seeded RNG names locally; existing imports remain necessary for the rest of the actor suite.

**Source:** `gomoku/crates/cli/tests/actor.rs`, lines 6–10.

```text
  6 | use cli::actor::{AiConfig, EvaluatorKind, Occupant, OccupantKind};
  7 | use cli::command::{Command, CommandError, Event, SeatSel};
  8 | use cli::session::{SessionCore, SessionInit};
  9 | use engine::{Board, Color, Status};
 10 | use testkit::{ai_config_tactics, mv, play};
```

This unchanged shared `ai_config_tactics` helper is the only configuration builder used by the seeded regression. Its full body fixes the non-explicit, deterministic starting settings without requiring another source lookup or a new helper implementation.

**Source:** `gomoku/crates/testkit/src/lib.rs`, lines 87–98.

```text
 87 | /// The actor tests' baseline AI config: tactics evaluator, 200
 88 | /// simulations, deterministic selection (temperature 0, no noise).
 89 | pub fn ai_config_tactics() -> AiConfig {
 90 |     AiConfig {
 91 |         evaluator: EvaluatorKind::Tactics,
 92 |         simulations: 200,
 93 |         c_puct: 1.5,
 94 |         temperature: 0.0,
 95 |         noise_epsilon: 0.0,
 96 |         explicit_selection: false,
 97 |     }
 98 | }
```

Insert the complete eleven-ply regression at actor-test module scope. Search, optional noise, statistics extraction, and sampling reproduce chapter 08's path; compare every event batch, complete SearchInfo, and resulting board against the persistent-player session. This tests extraction/scheduling, not playing strength.

**Source:** `gomoku/crates/cli/tests/actor.rs`, lines 146–203.

```text
146 | #[test]
147 | fn persistent_players_match_seeded_legacy_search_through_opening_schedule() {
148 |     use cli::actor::SearchInfo;
149 |     use rand::{SeedableRng, rngs::StdRng};
150 |
151 |     let config = AiConfig {
152 |         simulations: 3,
153 |         ..ai_config_tactics()
154 |     };
155 |     let mut core = SessionCore::new(SessionInit::ai_vs_ai(config.clone(), 42));
156 |     let mut rng = StdRng::seed_from_u64(42);
157 |     let mut board = Board::new();
158 |     for ply in 0..11 {
159 |         let mut effective = config.clone();
160 |         if ply < 10 {
161 |             effective.temperature = 1.0;
162 |             effective.noise_epsilon = 0.25;
163 |         }
164 |         // Reproduce the pre-player CLI path, including a fresh evaluator.
165 |         let mut evaluator = mcts::TacticsEvaluator::new();
166 |         let mcts::SearchResult::Searched { mut tree, root } = mcts::search(
167 |             &board,
168 |             &mut evaluator,
169 |             &mcts::SearchConfig {
170 |                 simulations: effective.simulations,
171 |                 c_puct: effective.c_puct,
172 |             },
173 |         ) else {
174 |             panic!("Tactics does not emit root proofs");
175 |         };
176 |         if effective.noise_epsilon > 0.0 {
177 |             mcts::add_dirichlet_noise(&mut tree, root, effective.noise_epsilon, 0.3, &mut rng);
178 |         }
179 |         let expected_info = SearchInfo::from_searched(&tree, root, &board, &effective);
180 |         let expected_move = mcts::select_move(
181 |             &mcts::visit_distribution(&tree, root),
182 |             effective.temperature,
183 |             &mut rng,
184 |         )
185 |         .unwrap();
186 |
187 |         assert_eq!(
188 |             core.execute(Command::Step).unwrap(),
189 |             vec![Event::Moved {
190 |                 mv: expected_move,
191 |                 by: OccupantKind::Ai
192 |             }],
193 |             "same seeded move at ply {ply}"
194 |         );
195 |         assert_eq!(
196 |             core.search_info(),
197 |             Some(&expected_info),
198 |             "stats at ply {ply}"
199 |         );
200 |         board.play(expected_move).unwrap();
201 |         assert_eq!(core.board(), board);
202 |     }
203 | }
```

### Existing chapter-08 tests are retained, not recopied

The baseline proof and quiet-root actor tests are **unchanged relative to
chapter 08**. Their complete bodies already appear in the
[chapter-08 solution's CLI actor tests][chapter08-actor-tests], so they are
not duplicated here. Likewise, MCTS root-hook and baseline admission tests
belong to that prerequisite, not new 08b artifacts. Existing session
integration tests and the rest of the actor suite stay intact.

## 9. Reconstruction audit and verification commands

The new library fragments cover all of `lib.rs`: module/imports (1–14),
options (16–45), statistics (47–86), decision/error (88–114), player/new
(116–126), and decide (128–192). The guard fragments cover configuration
(1–48), diagnostics (50–90), resolver (92–164), and the whole private test
module (166–376). Gaps between those ranges contain only blank separators.
Both new integration-test files are fully quoted in logical groups; no test
body, evaluator double, or fixture helper is omitted. The session comparison
against chapter 08 adds exactly two runtime fields, two constructor entries,
the extracted decision path, runtime-aware occupant/reset helpers,
`make_player`, and four tests with their doubles. The actor comparison adds
only `from_stats` and the seeded actor regression. All other quoted CLI
methods are marked unchanged context.

The authoring checks matched all **68 numbered snippets** byte-for-byte to
those reference ranges, checked complete nonblank coverage of the four new
Rust files and the substantive CLI additions against chapter 08, and checked
local reference paths and trailing whitespace. These are document/source
coverage checks, not Rust tests or workspace gates. The temporary extraction
script was removed; the paper has no generator dependency.

For **targeted construction checkpoints**, run from the implementation's
`gomoku/`. These earlier scoped commands are not the final QA evidence:

```text
cargo check -p player
cargo test -p player
cargo test -p cli --lib
cargo test -p cli --test session
cargo test -p cli --test actor -- --skip baseline_ai_plays_proven_vcf_without_search_info --skip baseline_ai_falls_back_to_search_info_on_quiet_position
```

The fresh full QA recorded in the provenance section ran these commands
from the reference's `gomoku/`, all with exit 0:

```text
cargo test --release --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

The full release suite includes real-baseline actor execution; the cheap
mapping test alone would not establish that. Interactive study-table smoke
testing is not claimed. Both design Mermaid fences now parse cleanly
under mermaid 12.3.0 with jsdom (`mermaid.parse`: sequenceDiagram →
`sequence`, stateDiagram-v2 → `stateDiagram`, no errors); syntactic parsing
only, no browser/render validation. No guard strength or latency claim follows from the preset or the
correctness regressions.
The preexisting imported-board draw limitation remains explicitly outside
this slice. No commits, pushes, or learner-source changes are part of this
solution authoring task.

## References

- [Chapter 08b tutorial][tutorial] — behavioral contracts and test-first milestones.
- [Chapter 08b design][design] — ownership, outcomes, and runtime lifecycle.
- [Chapter 08 complete solution][chapter08-solution] — prerequisite evaluator root hook, baseline certification, and unchanged chapter-08 tests.
- Source provenance: detached sibling reference `rust-ml-ai-player-ref`, base `2b6ec75` plus uncommitted chapter-08 and 08b overlays; chapter-08 comparison reference `rust-ml-tss-root-doc-ref`. All new source required for 08b is quoted above, so no external code link is needed to reconstruct the slice.

[tutorial]: ./ai-player.md
[design]: ./ai-player-design.md
[chapter08-solution]: ../08-the-tss-eye/the-tss-eye-solution.md
[chapter08-actor-tests]: ../08-the-tss-eye/the-tss-eye-solution.md#cli-actor-tests
