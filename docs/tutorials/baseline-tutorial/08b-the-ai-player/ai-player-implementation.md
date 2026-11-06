# Sub-chapter 08b — The AI player: construction paper

## Abstract

Build a reusable AI player in two independently testable milestones. First
extract the existing search and selection path, keep one evaluator per study
seat, and preserve decisions, random-number consumption, events, and search
records with defensive checking disabled. Then add bounded, verified opponent
proof checks after ordinary move selection and enable them for baseline seats
only. This paper specifies artifacts, fixtures, test order, method contracts,
and every return branch, but deliberately supplies no implementation bodies.
An accepted nonterminal candidate remains unknown; only a verified opponent
proof certifies a candidate's refutation.

## Glossary

| Term | Meaning in this construction |
|------|------------------------------|
| CLI | Command-line interface; the study session owns commands, history, configuration, and events. |
| AI seat | A Black or White seat whose moves are produced automatically. |
| MCTS | Monte Carlo tree search; simulations build move visits and accumulated values on tree edges. |
| Evaluator | An implementation of `mcts::Evaluator`, estimating positions and optionally certifying a root winning move. |
| Root hook | `Evaluator::evaluate_root`, called once before simulations on an ongoing root. |
| Root proof | The certified move returned by `SearchResult::ProvenWin`, distinct from a measured search result. |
| Proposal | The ordinary temperature-selected move before candidate checking. |
| Prior | An edge's policy probability, not its visit count. |
| Temperature | A selection setting: effectively zero chooses the most-visited move; positive values sample visit-derived weights. |
| Dirichlet noise | Random perturbation of root priors; this extraction preserves its existing post-search placement. |
| TSS | Threat-space search, the engine's bounded search through forcing threats and replies. |
| Candidate guard | Optional decision-local checks for verified opponent wins after hypothetical candidate moves. |
| Refuted | A candidate permits an opponent proof passing winner, legal-first-move, and line verification checks. |
| Unrefuted | No accepted opponent proof was found; unknown, not certified safe. |
| RNG | Random-number generator; the session owns the stream and lends it to one decision. |
| TDD | Test-driven development: establish a failing behavioral test, make it pass, then refactor while it remains green. |
| Work bounds | Candidate-count and per-probe node/depth limits, not a response-time deadline. |
| `[experiment]` | A local tuning choice without a published or measured strength/latency guarantee. |

## 1. Starting point, source links, and construction discipline

The [chapter][chapter] specifies the behavior; the [design paper][design]
explains ownership and transitions. Chapter 08's [root-hook construction][root]
already distinguishes a certified root move from a searched tree. A proof
returns through the evaluator seam; ordinary leaf evaluation remains
estimate-only. This chapter consumes that distinction without changing the
engine, evaluator interface, or search algorithm.

The implemented source inspected for this paper is the isolated sibling
worktree `../rust-ml-ai-player-ref`, relative to the main repository root.
Main may not contain a player crate yet. Every linked source path below
therefore points to that sibling, not to a missing main artifact. For example,
[player library][player] resolves through five parent directories from this
paper to the sibling workspace. Artifact names describe what the learner
creates or changes in **their own** `gomoku/` workspace. Source links are
inspection aids, not permission to copy implementation bodies.

There are exactly two milestones:

1. **Persistent player, guard disabled:** preserve the ordinary decision path
   and CLI lifecycle while moving reusable decision work into a new crate.
2. **Verified opponent candidate guard:** add private candidate resolution,
   then map only baseline CLI seats to the enabled preset.

Keep each test group ahead of its production slice. Missing declarations can
initially produce compiler errors; add the smallest documented type and method
surface needed to compile, then obtain a behavioral red assertion before
implementing the contract. Run one named test, observe why it fails, implement
that behavior, rerun it, then rerun the earlier green tests. Do not implement
all branches first and add tests afterward. No source solution is printed
here: signatures are described in prose, and fixture recipes describe the
observations the learner must implement.

### Artifact map

| Learner workspace artifact | Construction responsibility |
|----------------------------|-----------------------------|
| [Cargo.toml][workspace] | Register `crates/player` as a member of the separate Gomoku workspace. |
| [crates/player/Cargo.toml][player-manifest] | Define the `player` package and its three dependencies. |
| [crates/player/src/lib.rs][player] | Public types, evaluator ownership, decision execution, and private statistics extraction. |
| [crates/player/src/guard.rs][guard] | Private module; public guard data, crate-private constructor and resolver, private tests. |
| [crates/player/tests/decisions.rs][decisions] | Four public-interface extraction/persistence tests and local doubles. |
| [crates/player/tests/guard.rs][guard-tests] | Twelve public guard tests, including the distinct-line regression. |
| [crates/cli/Cargo.toml][cli-manifest] | Add the path dependency on `player`. |
| [crates/cli/src/actor.rs][actor] | Add `SearchInfo::from_stats`; preserve `from_searched`. |
| [crates/cli/src/session.rs][session] | Runtime player slots, lifecycle, per-step options, decision routing, and private-access tests. |
| [crates/cli/tests/actor.rs][actor-tests] | Seeded eleven-ply comparison with the old decision path. |

## 2. Milestone 1 — persistent player with checking disabled

### 2.1 Register the boundary and declare only its data surface

Start the first public decision test in [decision tests][decisions], then
create the package it imports. In the [workspace manifest][workspace], append
`crates/player` to the existing members; do not create another workspace. In
its [manifest][player-manifest], name the package `player`, inherit workspace
version, edition, and license, and depend on `engine` and `mcts` through sibling
paths plus `rand` version `0.10.3`. Do not add Burn, baseline, patterns, CLI,
serialization, or testkit dependencies. The engine remains an independent
rules library; concrete evaluator construction remains in the CLI.

In [the player library][player], enable `#![deny(missing_docs)]`, declare the
private `guard` module, and re-export only `GuardConfig`, `GuardDiagnostics`,
and `GuardOutcome` from it. Define the following exact public data surface;
all listed fields are public unless explicitly described as private.

| Type | Fields and traits |
|------|-------------------|
| `PlayerOptions` | `simulations: u32`, `c_puct: f32`, `temperature: f32`, `noise_epsilon: f32`, `noise_alpha: f32`, `guard: GuardConfig`; derive `Debug`, `Clone`, `Copy`, `PartialEq`. |
| `SearchStats` | `side_to_move: Color`, `visits: Vec<(Move, u32)>`, `prior: Vec<(Move, f32)>`, `value: f32`; derive `Debug`, `Clone`, `PartialEq`. |
| `Decision` | `mv: Move`, `search: Option<SearchStats>`, `guard: GuardDiagnostics`; derive `Debug`, `Clone`, `PartialEq`. |
| `DecisionError` | Only `GameOver`; derive `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`. |
| `AiPlayer` | One private field, `evaluator: Box<dyn Evaluator>`; no board, options, RNG, history, or retained tree. |
| `GuardConfig` | `enabled: bool`, `max_candidates: usize`, `budget: SearchBudget`; derive `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`. |
| `GuardDiagnostics` | `proposal: Move`, `refuted: Vec<Move>`, `checked: usize`, `outcome: GuardOutcome`; derive `Debug`, `Clone`, `PartialEq`, `Eq`. |
| `GuardOutcome` | `Disabled`, `ProvenWin`, `Unrefuted`, `Terminal`, `BudgetExhausted`, `AllRefuted`; derive `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`. |

Use `Box<dyn Evaluator>` for owned runtime polymorphism: callers choose a
concrete evaluator without the player importing its implementation. Do not
make the player cloneable to satisfy configuration types.

`AiPlayer::new` is public, takes ownership of the boxed evaluator, and returns
`Self`; mark it `must_use`. `AiPlayer::decide` is public, mutably borrows the
player, immutably borrows a `Board` and `PlayerOptions`, and takes a mutable
borrow of an `impl Rng`. It returns `Result<Decision, DecisionError>`. These
borrows mean the evaluator may retain mutable state, but the session still
owns its position, current settings, and randomness. No decision commits a
move to the caller's board.

`PlayerOptions::default` obtains simulations and `c_puct` from
`SearchConfig::default`—currently 400 and 1.5—rather than duplicating them.
Its temperature is `0.0`, noise fraction `0.0`, concentration `0.3`, and guard
`GuardConfig::disabled()`. In [the guard module][guard], `disabled` and
`baseline` are public `const` associated constructors returning `Self`, both
`must_use`. The baseline preset enables five candidates with
`SearchBudget { max_nodes: 1_000, max_depth: 10 }`; disabled retains these
settings but sets `enabled` false. `GuardConfig::default` delegates to disabled.
These are `[experiment]` defaults, not a five-move proof depth or shared
1,000-node pool. The enabled behavior is built only in milestone 2.

`GuardDiagnostics::new` is crate-private. It takes a proposal and outcome and
returns diagnostics with an empty refutation vector and zero checks. In this
milestone ordinary decisions need only disabled diagnostics; root certificates
need `ProvenWin`. Keep the other outcome variants available for later tests.
Implement `Display` for `DecisionError` with the text `the game is over`, and
implement `std::error::Error`. Do not add an unknown-proof error or treat
`GameOver` as a successful root certificate. Document all public items, fields,
variants, error behavior, and invariant panics as they become implemented.

### 2.2 First red: reject a finished input before doing any work

In [decision tests][decisions], create a local `winning_position` helper that
starts from `Board::new` and replays these alternating moves:

- Black `(7,3)`, White `(0,0)`;
- Black `(7,4)`, White `(0,2)`;
- Black `(7,5)`, White `(0,4)`;
- Black `(7,6)`, White `(0,6)`.

All coordinates in this paper are zero-based `(row,column)` and constructed
with `Move::new`. Replaying avoids accidental terminal positions during setup
and supplies real move history. Define `NeverEvaluate` with both `evaluate`
and `evaluate_root` panicking: either call violates this test's boundary.

Write `terminal_board_returns_game_over_without_evaluation_or_rng` first.
Complete Black's five at `(7,7)`, assert that the fixture is no longer ongoing,
and create two `StdRng` instances with seed `7`. Call `decide` with default
options using one stream. Require `DecisionError::GameOver`, then compare the
next `u64` draw from each stream. Import `SeedableRng` for construction and
`RngExt` for `random` in rand 0.10; equal seeds alone do not prove equal stream
consumption.

**Red:** a decision which reaches search panics in the double or returns the
wrong result. **Green:** at the start of `AiPlayer::decide` in [the
library][player], inspect `board.status()`. Any status other than
`Status::Ongoing` returns `GameOver` before constructing work that calls the
evaluator, applies noise, or samples. The same branch rejects drawn inputs;
the replay-built draw fixture in §3.5 supplies a way to exercise that status
without changing the engine. Rerun this test before adding the ordinary path.

### 2.3 Second red: reproduce ordinary decisions and real measurements

Next write
`ordinary_decisions_match_manual_search_stats_and_seeded_selection` in
[decision tests][decisions]. Start from a board with only `(7,7)` played. Use
separate `UniformEvaluator::new(0.6)` instances for the manual path and player,
and equal-seed RNGs (`42`) for each case. Exercise these tuples of simulation
count, temperature, and noise fraction:

| Simulations | Temperature | Noise fraction |
|-------------|-------------|----------------|
| 17 | 0.0 | 0.0 |
| 19 | 1.0 | 0.0 |
| 23 | 0.7 | 0.25 |
| 0 | 1.0 | 0.25 |

Use `c_puct` `1.7`, concentration `0.3`, and disabled guard. The independent
expected path uses the existing [search][search] and [policy helpers][policy]:
search, optional noise, visit extraction, and selection. Match the manual
search result explicitly and fail the test if a uniform evaluator somehow
returns a proof. Independently collect all root priors, sort the expected
records, and calculate expected value from the root edges. Do not use the
new private statistics helper to generate the expected answer.

Assert the same legal move, original searched side, complete sorted visits,
complete sorted priors, exact root value, visit sum equal to the requested
simulations, and identical next RNG draw. Require real `Some` statistics even
at zero simulations. An omitted zero-visit move, changed noise placement, or
extra sampling draw must fail this comparison.

**Red:** the incomplete ordinary branch cannot match the manual path.
**Green:** in `AiPlayer::decide`, construct `SearchConfig` with the two search
settings supplied in options, call `mcts::search` once with the retained
boxed evaluator, and explicitly match its two result variants. Borrowing the
trait object through `&mut *self.evaluator` is the dereference-and-reborrow
idiom needed by this call; do not take ownership out of the player.

For `SearchResult::Searched`, retain its mutable tree and root locally:

1. Only if the noise fraction is positive, call the existing
   `add_dirichlet_noise` with fraction, concentration, and borrowed RNG.
2. Obtain `visit_distribution` from this root, including zero visits.
3. Call `select_move` exactly once with that distribution, temperature, and
   the same stream. The returned move is the original proposal.
4. For now retain it unchanged, attach disabled diagnostics, and attach
   `Some(SearchStats::from_searched(...))` from the real tree.

Temperature below the existing helper's `1e-8` threshold chooses maximal
visits with lowest move index breaking ties. Positive temperature samples
visit-derived weights; all-zero visits fall back to uniform sampling. Reuse
these helpers rather than implementing another selector. An empty
ongoing-root distribution is an invariant failure handled with `expect`, not
a new public error. Enabled noise panics if Gamma rejects its alpha; its
helper also asserts a nonempty root. It does not validate `noise_epsilon`.
Do not add parameter validation or new configuration-error behavior.

Noise and temperature appear in AlphaGo Zero [1], but this extraction does
**not** move noise to its pre-search location in that algorithm. Here the
simulations have already completed. Noise modifies root **priors**, not the
current visits or accumulated values. It consumes random draws before
selection, so moving or removing it may still change a seeded proposal.
Later its priors also affect the guard's retry tie-break. Never describe this
path as selecting from noise-modified visits.

Implement the private associated helper `SearchStats::from_searched` in
[the library][player]. It immutably borrows a `mcts::Tree`, takes the
`mcts::NodeId` root, immutably borrows the searched `Board`, and returns `Self`:

- Reuse `visit_distribution`; sort descending count then ascending
  `Move::index`. Include every edge, not just visited or favored moves.
- Collect `(move, prior)` from every root edge after optional noise. Sort
  descending prior then ascending index. Use comparator chaining with
  `then_with`; `partial_cmp` on floats falls back to `Ordering::Equal` before
  the index comparison when values are incomparable.
- Sum edge counts and accumulated values separately. Zero total visits gives
  value `0.0`; otherwise divide summed values by total visits as `f32`.
  Do not divide by edge count or use an unweighted average of edge means.
- Store `board.to_move()`. Root edge values already have that player's
  perspective; do not flip the sign because the CLI will play a move later.

These statistics describe the entire search, not necessarily the eventual
chosen move. There is no `proven` flag or one-visit proof distribution.

### 2.4 Third red: the root certificate bypasses all ordinary work

Before completing the proof branch, define `ProofEvaluator` in
[decision tests][decisions]. It stores a winning `Move` and an
`Rc<RefCell<u32>>` root-call counter. Its root hook asserts that the requested
legal set contains that move, increments the counter, and supplies the root
certificate. Its leaf method panics. `Rc` shares ownership only with the test;
`RefCell` provides observable single-threaded mutation without imposing
thread-safety or cloning requirements on production evaluators.

Write `root_proof_has_no_search_stats_leaf_calls_or_rng_consumption`. Reuse
`winning_position`, choose `(7,7)`, and first assert membership in the
engine's immediate wins for the actual mover. Set 100 simulations,
temperature `1.0`, noise fraction `0.25`, and enabled guard deliberately.
Compare a seed-`8` decision RNG with an untouched equal-seed RNG.

Require that winning move, `search.is_none()`, exactly one root call,
`ProvenWin` diagnostics naming that move as proposal, zero checks, no
refutations, and unchanged next draw. **Red:** fabricated statistics, leaf
work, noise, sampling, or candidate checks break the test. **Green:** the
`SearchResult::ProvenWin` match arm returns immediately with those values.
Trust certification through the existing evaluator seam; do not re-prove it
in the player. A successful proof decision and a terminal-input error are
different branches.

### 2.5 Fourth red: one evaluator survives multiple decisions

In [decision tests][decisions], define `StatefulEvaluator` with a local
`calls: u32` and shared `Rc<RefCell<Vec<u32>>>` observations. Its ordinary
method increments the local counter, records the new value, and returns
zero logits with value `0.1`. Keep the default root-hook delegation so a
one-simulation ongoing search performs one root evaluation and one leaf
evaluation.

Write `evaluator_instance_persists_across_two_decisions`: seed `9`, one
simulation, default disabled guard, initially empty board. Decide once and
require observations `[1,2]`; play that returned move on the test's board,
decide again, require a legal second move and `[1,2,3,4]`.
**Red:** rebuilding the evaluator restarts its local counter. **Green:**
`new` transfers ownership once, and `decide` merely mutably borrows it.
Every call still gets a fresh local tree; persistence is not tree reuse.
Run all four decision tests before proceeding into the CLI.

### 2.6 Tests first for runtime seat ownership

Add the `player` path dependency in [the CLI manifest][cli-manifest]. Put
these tests inside the existing test module of [the session module][session],
not in external integration tests: they must inject private runtime slots.
Reuse `testkit::mv`. Keep existing local `play`/`play_some` helpers for session
unit tests; the testkit's session-taking helpers refer to the linked library's
type, not this unit-test build's `SessionCore`. Integration tests can continue
using the testkit versions.

Define a local `CountingEvaluator` with shared `Rc<Cell<u32>>` call and drop
counters plus an inner `mcts::TacticsEvaluator`. Delegate ordinary evaluation
to that inner evaluator after incrementing calls; implement `Drop` to increment
drops. `Cell` is sufficient for copyable counters. This cheaply observes
resource identity without running the Burn baseline or adding a public
injection API. Construct a tactics session and replace only its private
player slot with this double.

Write each of the following before its corresponding lifecycle changes:

1. **`persistent_player_reuses_evaluator_across_steps_and_set_sims`:**
   initialize `SessionInit::versus_ai(Color::White, config)` with one
   simulation, so Black is AI and White human. Inject Black's counter double.
   `Step` must make two calls. `Back` one ply, then `SetSims` Black to three;
   require zero drops. Step again and require cumulative calls six,
   stored effective simulation count three, and total visits three. Take
   Black; require exactly one drop and no Black player.
2. **`persistent_players_follow_init_release_take_and_reset`:** start with
   the same one-simulation arrangement and save the initial state. Require
   Black's player present and White's absent. Inject a counted Black
   evaluator, release White, and require no Black drop and a White player.
   Replace White's new player with another double sharing the drop counter.
   `SetSims` Both to three must drop neither. `New` emits only `Reset`, drops
   both doubles, restores initial occupants, gives Black a new player and
   White none, and restores one simulation on the next step. Take Both,
   require neither player; release Both, require both; step and require the
   initial one-simulation setting rather than the discarded override.
3. **`persistent_player_proof_plays_without_search_info_or_rng_draws`:**
   define `WinningEvaluator` with the same legal-move root certificate and
   panic-on-leaf behavior as the earlier proof double. Replay the winning
   fixture, initialize seed `42`, temperature `1.0`, and noise `0.25`, then
   inject it for Black. Step must emit `Moved` by AI followed by
   `GameOver(Won(Black))`, store no search record at the new cursor, and leave
   the next RNG draw equal to an untouched stream.

**Red:** absent slots initially fail to compile; after declarations, missing
construction, replacement on every step, rebuilding on `SetSims`, or missing
proof routing fails the behavioral assertions. **Green:** implement ownership
and step routing below, then run the three tests individually and together.
The baseline policy test belongs to milestone 2, not this extraction gate.

### 2.7 Construct and replace runtime players at the exact lifecycle points

In `SessionCore` in [the session module][session], add private
`black_player` and `white_player`, each `Option<player::AiPlayer>`, beside
Black/White `Occupant` configuration. Do not embed evaluators in `AiConfig` or
`Occupant`: those remain cloneable, comparable, and serializable.

`SessionCore::make_player` is a private associated helper borrowing an
`Occupant` and returning `Option<AiPlayer>`. Human maps to `None`; AI maps to a
new player owning the result of existing `make_evaluator` for that config's
kind. Reuse the existing tactics/baseline construction and feature gates;
there is no new evaluator factory in `player`.

Construct both slots in `SessionCore::new` from the initial occupants. Extend
private `set_occupant`, which takes mutable session access, a `Color`, and an
owned replacement `Occupant`: construct the new optional player from the
incoming occupant, then assign the matching occupant and slot together.
Assignment drops the old runtime player. Preserve color-locality: replacing
White must not replace Black.

Follow all lifecycle paths, including their existing event behavior:

| Method/path in the session module | Required player lifetime and surrounding behavior |
|-----------------------------------|----------------------------------------------------|
| `new` | One player for each initial AI, none for humans; initialize the session RNG once. |
| `execute_release` | Reuse `apply_to_seats`; clone `default_config`, apply that color's simulation override, and use `set_occupant` with AI. Release constructs a fresh instance even for an already-AI seat. Emit the existing `SeatChanged` per selected color. |
| `execute_take` | Use `set_occupant` with Human for each selected seat; old evaluators drop. Preserve simulation overrides and `SeatChanged` events. |
| `execute_set_sims` | Preserve seat validation and `ZeroSims` rejection. Update each selected color's override, update the live AI config if present, and emit `ConfigChanged`. A human's override is remembered for later release. Do not touch either runtime slot. |
| `reset` | Restore initial history, both occupants through `set_occupant`, default config, both overrides, empty search records, initial Swap2 phase, and initial puzzle context. Drop both old evaluators and construct only initial AI players. Keep the existing RNG stream; do not reseed. |
| `execute_new` | Continue calling `reset` and returning `Reset`. |
| `execute_play` and ordinary `Step` | Commit history without replacing either player. |
| `execute_back`, `execute_forward`, `execute_var` | Navigate existing history; retain both evaluators and RNG. |
| `execute_choose_color` | Finish the Swap2 ceremony and replace the history root as before; do not rebuild seats. |
| `execute_puzzle_nav` | Replace the puzzle/history root and clear search records on successful navigation; retain seats, configs, players, and RNG. |

`apply_to_seats` already supplies Black then White for `Both`; reuse it rather
than adding a second seat-dispatch mechanism. No new threads, actors, locks,
async execution, or tree reuse are part of this lifecycle.

### 2.8 Route one session step and attach effective statistics

First declare `SearchInfo::from_stats` in [the actor module][actor]. It is a
public `must_use` associated method, consuming `player::SearchStats`, borrowing
an `AiConfig`, and returning `Self`. Transfer the four measurement fields and
clone the effective config. It must not sort, recompute, flip values, or
invent proof records. Keep the existing `SearchInfo::from_searched` unchanged
for other callers and the independent legacy comparison.

Now change `SessionCore::execute_step` in [the session module][session], in
execution order:

1. Preserve the opening-ceremony rejection, then the finished-board rejection,
   then the side-to-move occupant check. A human yields
   `StepRequiresAi { seat: side }`. These happen before borrowing a player.
2. Clone the current AI config into an effective config. Reuse
   `opening_schedule_active`: an empty initial root, both seats AI, no explicit
   selection settings, zero default temperature/noise, and current history
   ply less than ten enable temperature `1.0` and noise fraction `0.25`.
   Returning to an early variation re-enters the schedule. Explicit settings
   or a preset nonempty opening suppress it.
3. Build per-decision `PlayerOptions` from the effective simulation,
   exploration, temperature, and noise values; concentration is `0.3`.
   **Keep guard disabled for every evaluator in milestone 1.** Options are
   not retained inside the player or used to reconstruct its evaluator.
4. Select the matching optional runtime slot, use `Option::as_mut` to borrow
   its player, and call `decide` with the borrowed board and session RNG.
   `expect` documents the invariant that an AI occupant has a player; a missing
   slot is not another command error. Map `DecisionError::GameOver` to the
   existing command `GameOver` error.
5. Destructure the returned optional statistics with the `let Some(...) else`
   idiom. For `None`, commit the proof move through `execute_play` and return
   its events without inserting a search record. Do not clear or fabricate
   history records elsewhere just to represent this decision.
6. For `Some`, build `SearchInfo::from_stats` with the **effective** config.
   Preserve the explicit searched branch: remember the old cursor,
   `history.play`, get the new cursor, store info there, then emit `Moved`
   by AI, `Branched` if a newly moved child has siblings, and `GameOver` if
   applicable, in that order. The searched branch is not rerouted through
   `execute_play`; it needs its existing record insertion and event path.

The player cannot emit CLI events or advance history. The CLI currently
neither stores nor renders guard diagnostics; keep them separate from
`SearchInfo`.

### 2.9 Independent seeded comparison before enabling the guard

Write `persistent_players_match_seeded_legacy_search_through_opening_schedule`
in [actor integration tests][actor-tests] before accepting the extraction.
Reuse `ai_config_tactics` from testkit, request three simulations, and create
an AI-versus-AI session with seed `42`. Independently keep an empty board and
seed-`42` RNG for the manual old path.

For eleven successive plies, create a **fresh** tactics evaluator in the
manual path. Derive effective settings: temperature `1.0` and noise `0.25`
for plies zero through nine, configured defaults for ply ten. Run manual
search, optional post-search noise with concentration `0.3`, build expected
`SearchInfo` with the retained old `from_searched`, and sample once. Compare
session step events to the single expected AI move event, compare full stored
info, play that move on the manual board, and compare boards each iteration.

**Red:** different RNG timing, settings, statistics, evaluator semantics, or
schedule boundary causes divergence. **Green:** fix the extraction rather
than changing expectations. This tactics-only test avoids baseline
initialization costs and proves compatibility across the boundary; it is
not a playing-strength measurement. Pause with the guard disabled and all
milestone-1 tests green before starting milestone 2.

## 3. Milestone 2 — bounded verified opponent candidate guard

### 3.1 Build cheap public guard fixtures before implementing checks

Create [public guard tests][guard-tests] with these local helpers:

- `mv`: take two `u8` coordinates and unwrap `Move::new` for known-valid cells.
- `position`: accept Black and White slices of coordinate pairs, map them to
  moves, and call `Board::from_position` with Black to move. Use balanced
  counts and disjoint stones. The White-to-move test constructs its own board.
- `single_threat`: Black `(7,2)`, `(2,0)`, `(2,2)`, `(2,4)`; White `(7,3)`,
  `(7,4)`, `(7,5)`, `(7,6)`. White's only immediate winning endpoint is `(7,7)`.
- `Prefer`: store one preferred `Move`; ordinary evaluation returns value
  `0.0` and 225 zero logits except logit `10.0` at that move. Leave the default
  root hook; this double never certifies a root win.
- `options`: accept a `GuardConfig`, set simulations to zero, and otherwise
  use `PlayerOptions::default`.

Zero simulations still expand every legal root edge. With zero temperature,
all visits tie and the proposal is the lowest-index legal move, usually
`(0,0)`. A high logit does **not** change this proposal; it ranks a retry by
prior after refutation. This fixture design separates proposal preservation
from defensive retry ordering and avoids Burn or long searches.

Write `baseline_guard_is_opt_in_with_fixed_experiment_work_bounds` before
accepting preset behavior. Assert default player guard equals disabled,
baseline enabled, candidate count five, nodes `1_000`, depth ten. The reference
also makes `GuardConfig::default` disabled. **Red:** an enabled reusable
default or different work bounds fails. **Green:** complete the constructors
specified in §2.1. Verification and board-copy work are outside those bounds;
there is no strict elapsed-time deadline.

### 3.2 Private resolver interface and tests before its branches

The implementation artifact is crate-private `guard::choose` in
[the guard module][guard]. Describe its signature precisely when declaring
it: borrow the input `Board`, take the `Move` proposal, borrow a slice of
`mcts::Edge`, take `GuardConfig` by value, and take a mutable callback through
`impl FnMut(&Board, Color, SearchBudget) -> Option<Proof>`. Return a pair of
chosen `Move` and `GuardDiagnostics`.

The module is private and the helper is `pub(crate)`, not publicly re-exported.
The callback is a local test seam, not a new evaluator hook or public probe
trait. Production will pass `engine::prove_forced_win`. **The reference has no
separate named proof-validator helper:** proof admission is checked inside
`choose`; use the contract in §3.4 rather than inventing another public API.

Inside its private test module, define local `mv`, `two_ended_four`, and
`edges` helpers. The board has Black `(2,0)`, `(2,2)`, `(2,4)`, `(2,6)` and
White `(7,3)` through `(7,6)`, with Black to move. The edge helper enumerates
**all** `board.empty_moves()` and gives each edge its move, prior `0.0`,
count `0`, accumulated value `0.0`, and `child: None`. Do not truncate the
slice to preferred candidates: a fully expanded root represents every legal
move, an invariant used by the all-refuted branch.

First write `disabled_and_zero_cap_never_call_probe`. Supply a panic callback
and test both disabled configuration and enabled configuration with zero
candidate cap. Both return the proposal, zero checks, and no refutations;
outcomes differ: `Disabled` versus `BudgetExhausted`.
**Red:** any probe or wrong outcome fails. **Green:** initialize diagnostics
as disabled; return immediately if disabled. For enabled checking switch the
pending outcome to `BudgetExhausted`, and return the proposal immediately if
the cap is zero. Neither branch clones/plays a candidate or invokes a probe.

Then write `zero_candidate_cap_keeps_original_unchecked` through the public
player using `single_threat`, `Prefer(7,7)`, and zero cap. Require `(0,0)`,
zero checks, empty refutations, and `BudgetExhausted`. Wire the searched branch
of `AiPlayer::decide` to call `choose` **after** ordinary sampling, supplying
immutable root edges, the guard config, and the real prover. Use its returned
move and diagnostics; still construct statistics from the unchanged real tree.
Rerun all milestone-1 tests after this wiring.

### 3.3 Proposal first, deterministic alternatives, and cap fallback

Before implementing ranking, write the private test
`alternatives_rank_visits_then_prior_then_index_without_repeats_and_include_unvisited`
in [the guard module][guard]. Use `two_ended_four`, proposal `(0,4)`, all legal
edges, and assign these special edge statistics:

| Move | Visits | Prior |
|------|--------|-------|
| proposal `(0,4)` | 100 | 1.0 |
| `(0,3)` | 4 | 0.1 |
| `(0,2)` | 3 | 0.9 |
| `(0,1)` | 3 | 0.9 |
| `(0,0)` | 3 | 0.5 |
| `(0,5)` | 0 | 0.9 |

Reverse the edge vector after assignment to prevent creation order from
passing as a tie-break. Clone board and edges for equality checks. Keep the
five-candidate cap but set each probe to one node and one ply. Inject a
callback which records the hypothetical board's last move, asserts the
forwarded side equals `after.to_move()` and White, asserts the exact budget,
and delegates to the real prover.

Require observed checks `[proposal,(0,3),(0,1),(0,2),(0,0)]`, identical ordered
refutations, check count five, and chosen move `(0,5)` with
`BudgetExhausted`. That sixth candidate has zero visits and is **UNTESTED**.
Require unchanged input board and edges.

Also write `cap_falls_back_to_unchecked_block_not_refuted_original` through
[the public player][guard-tests]: `single_threat`, high-prior `(7,7)`, cap
one. Require the original `(0,0)` refuted once and the untested `(7,7)` chosen
with `BudgetExhausted`. This must not return the known-refuted proposal.

**Red:** visit-only ranking, creation-order ties, repeats, visited-only
filtering, or original fallback at the cap fails these tests. **Green:**
`choose` ranks borrowed edge references by descending count, descending
prior, then ascending move index. Use `then_with` for comparison stages and
`partial_cmp` with `Ordering::Equal` fallback for prior comparisons. Build
candidates with the iterator idiom `std::iter::once` followed by `chain` over
ranked moves excluding the proposal. Keep already-refuted candidates from
being checked twice. The ordinary temperature proposal always goes first;
ranking never replaces it without a refutation. No retry resamples or uses
an RNG.

For each candidate, check the cap **before** cloning/playing it. If previous
candidates were refuted and the cap is reached, return this next candidate
with the pending `BudgetExhausted` outcome and unchanged check count. This
fallback is neither checked nor certified safe. Exhausting the cap and
exhausting all legal moves are separate control-flow outcomes.

### 3.4 Check on a clone, identify the actual opponent, and admit proofs

Write
`real_proof_refutes_proposal_and_guard_finds_unvisited_block_without_mutating_board_or_stats`
in [public guard tests][guard-tests]. Use `single_threat`, two separate
`Prefer(7,7)` players, equal seed `1`, and disabled/enabled options. Before
checking the guarded decision, require the plain proposal `(0,0)`; play it
on a clone and independently obtain a real opponent proof using the preset
budget. Assert White winner, nonempty line, legal first move, and successful
`verify_line`. Then require guarded `(7,7)`, proposal `(0,0)`, ordered
refutations containing only `(0,0)`, two checks, and `Unrefuted`. Compare full
search statistics to the plain decision and compare the original board and
Zobrist key to saved values.

Write `white_player_guard_probes_actual_black_opponent` next. Construct a
White-to-move board with Black `(7,3)` through `(7,6)` plus `(14,14)`, and
White `(7,2)`, `(2,0)`, `(2,2)`, `(2,4)`. Use `Prefer(7,7)`; require `(0,0)`
refuted, block `(7,7)` selected as `Unrefuted`, and unchanged input. This is
not merely a coordinate mirror: the probed winner must now be Black.

Before admitting any returned proof, write the private test
`malformed_or_unverified_proofs_leave_candidate_unknown`. Use Black `(7,3)`,
White `(7,4)`, Black to move, proposal `(0,0)`, and all legal edges. In
separate callback runs return each of these `Proof` values:

- winner Black with line `[(7,7)]`: wrong winner;
- winner White with an empty line: no first move;
- winner White with `[(7,3)]`: originally occupied first move;
- winner White with `[(0,0)]`: first move occupied by the hypothetical proposal;
- winner White with `[(1,0)]`: legal-looking first move but no verified win.

Require exactly one probe/check, original proposal returned, no refutations,
and `Unrefuted` for every case. Returning `Some` is not enough to refute.

**Red:** unconditional acceptance of `Some`, wrong-side probing, or mutation
of the input fails these tests. **Green:** for each candidate still within
cap, clone the input board, play the candidate with an invariant `expect`
that root candidates are legal, and increment checks once. Terminal candidates
are handled as specified in §3.5. Otherwise derive the opponent from
`after.to_move()`, not a hard-coded color or evaluator reconstruction from
stone counts. Invoke the callback on that hypothetical board with that color
and the unchanged per-probe budget.

The proof-admission predicate inside `choose` has three prerequisites in
short-circuit order: winner equals that opponent; a first line move exists
and is legal on `after`; `engine::verify_line(&after, &proof)` accepts the
whole proof. `Option::is_some_and` handles the optional proof and optional
first move without indexing an empty vector. Only after all prerequisites
succeed append the candidate to ordered refutations and continue. Missing or
rejected evidence immediately returns that candidate as `Unrefuted`.
Do not retry merely because the engine could not establish a proof.

The existing [engine prover/verifier][tss] implements bounded TSS [2]. Its
`None` is UNKNOWN, never a certified defense. Explicitly test this with
`zero_depth_probe_is_unknown_even_with_opponent_immediate_win` and
`zero_node_probe_is_unknown_even_with_opponent_immediate_win` in
[public guard tests][guard-tests]. Use `single_threat`; respectively set
nodes/depth to `1_000/0` and `0/10`. The first uses `Prefer(7,7)`, the second
a zero-valued uniform evaluator. Both retain `(0,0)`, one check, no
refutations, and `Unrefuted`, even though White can win. The candidate cap
has not been exhausted by a failed proof: acceptance ends checking.

### 3.5 Immediate own wins and draws bypass the opponent probe

Write `own_immediate_win_is_terminal_not_opponent_probe_unknown` in
[public guard tests][guard-tests]. Black has `(0,1)` through `(0,4)`; White
has `(2,0)`, `(2,2)`, `(2,4)`, `(2,6)`. With zero simulations, uniform
zero-valued evaluator, and zero nodes **and** depth, the proposal `(0,0)`
wins immediately. Require one check, `Terminal`, no refutations, and an
independent replay of the returned move producing `Won(Black)`.

Write private `immediate_win_never_calls_opponent_probe` on that same geometry
with a panic callback. Public zero-budget behavior alone cannot establish
that no callback was invoked. Require chosen proposal, `Terminal`, and one
check.

Next write private `immediate_draw_never_calls_opponent_probe` in
[the guard module][guard]. Build a valid 224-ply board rather than an imported
position: reserve `(0,0)` as the last move, enumerate all other 15×15 cells,
and partition by `(row + 2 * column) % 4 < 2` into Black versus White lists.
Replay paired Black/White entries in that order on `Board::new`. There are
112 of each; the repeated-pair pattern avoids fives horizontally, vertically,
and diagonally. Assert the constructed board remains ongoing, save a clone,
and pass its only empty move and all edges to `choose` with a panic callback.
Require `Terminal`, one check, no refutations, and unchanged input. On a
separate clone play the selected last move and require `Draw`.

Do not use `Board::from_position` for this draw recipe. Imported positions do
not populate the move history used by subsequent play's draw bookkeeping.
This is a fixture requirement, not permission to modify the engine.

**Red:** probing before terminal detection panics, while treating a zero
budget as automatic unknown returns the wrong outcome. **Green:** immediately
after hypothetical play and incrementing checks, accept `Won` for the
original `board.to_move()` or `Draw` as `Terminal`; return before determining
an opponent proof. These checks are counted candidates but zero probe calls.
A root-hook proof remains the separate earlier `ProvenWin`/zero-check branch.

### 3.6 Distinguish two endpoints from two distinct opponent lines

Keep the existing public test
`two_opponent_winning_threats_require_our_immediate_winning_alternative`.
Its Black stones `(7,2)` through `(7,5)` have own winning alternative `(7,6)`;
White's `(9,3)` through `(9,6)` have two winning endpoints. After a bad
`(0,0)` proposal assert White has two immediate wins. Use `Prefer(7,6)` and
require proposal refuted, own win selected, `Terminal`, and two checks. This
covers endpoints of one opponent line, not two distinct geometries.

Before accepting the terminal/retry integration, add the separate regression
**`blocking_one_of_two_distinct_opponent_lines_is_refuted_in_favor_of_own_immediate_win`**
in [public guard tests][guard-tests]. Construct this exact position:

| Color | Stones |
|-------|--------|
| Black | `(0,5)`, `(5,10)`, `(13,5)`, `(13,7)`, `(9,1)`, `(9,2)`, `(9,3)`, `(9,4)` |
| White | `(0,1)`, `(0,2)`, `(0,3)`, `(0,4)`, `(1,10)`, `(2,10)`, `(3,10)`, `(4,10)` |

Black moves next. White's horizontal four needs `(0,0)` because `(0,5)` is
Black; its distinct vertical four needs `(0,10)` because `(5,10)` is Black.
Black's own immediate winning alternative is `(9,5)`. Before calling the
player, assert the exact White immediate-win set is `[(0,0),(0,10)]`. Clone
and play `(0,0)`, then require the exact remaining White win set `[(0,10)]`.
Obtain a real proof with the actual resulting side and budget `1_000/10`;
require White winner, exact one-move line `[(0,10)]`, and successful verifier.
This establishes why blocking the first line is insufficient independently
of the guard's answer.

Use separate `Prefer(9,5)` players with zero simulations, seed-`1` RNGs, and
disabled versus enabled checking. The enabled config has cap **two**, nodes
`1_000`, depth ten. Require the disabled result `(0,0)` with `Disabled`, all
visits zero, and first prior `(9,5)`. Require guarded original proposal
`(0,0)`, ordered refutations `[(0,0)]`, two checks, `Terminal`, and selected
`(9,5)`. Require identical complete search statistics and next RNG draws
between runs. Play the guarded move on another clone and require
`Won(Black)`; compare the input board and Zobrist key with their saved values.

**Red:** accepting a move merely because it blocks one visible threat,
failing actual-opponent verification, replacing zero visits with fabricated
winning visits, or probing past an own win breaks this regression.
**Green:** the already specified clone/proof/retry/terminal branches should
pass without special-case geometry logic. Do not replace the older
single-line test with this distinct-line fixture.

### 3.7 Exhaust every legal candidate before reporting all refuted

Write
`every_legal_move_really_refuted_reports_all_refuted_and_original_fallback`
in [public guard tests][guard-tests]. Use the `two_ended_four` geometry:
scattered Black stones `(2,0)`, `(2,2)`, `(2,4)`, `(2,6)` and White `(7,3)`
through `(7,6)`. Black has no own immediate win and cannot cover both White
endpoints with one move. Use a uniform zero-valued evaluator, zero
simulations, cap equal to `board.empty_moves().count()`, and one node/one ply
per probe.

Require `AllRefuted`, checks and refutations both equal to the number of
legal moves, and every empty move present in the refutation list. For every
legal move independently clone/play it, obtain a fresh actual-opponent proof
with that budget, and verify it. Require the returned move equals the
original proposal, remains legal on the original board, and leaves the input
unchanged. Do not fake all-refuted by testing only a short top-move list.

**Red:** returning `AllRefuted` after five checks, returning an untested move
when none remains, or losing a legal edge fails. **Green:** only after the
candidate iterator is exhausted, assert refutation count equals
`board.empty_moves().count()`, change outcome to `AllRefuted`, and return the
original proposal. If the final legal candidate was the final permitted
check, exhaustion still means `AllRefuted`, not a cap fallback to a nonexistent
next move. This return can knowingly choose a refuted move. Individual
verified continuations are diagnostics, not a composed root-position loss
proof, and do not imply resignation.

### 3.8 Preserve temperature proposals, statistics, and RNG when unknown

Write `enabled_unknown_preserves_seeded_first_candidate_noise_stats_and_rng`
in [public guard tests][guard-tests] before accepting the completed player
wiring. On an empty board compare disabled checking against enabled checking
with zero-node/ten-ply probes at simulation counts zero and eleven. Use
separate uniform evaluators of value `0.3`, temperature `0.7`, noise fraction
`0.25`, and seed `42` for both RNGs.

Require the same selected move, the guarded proposal equal to the plain
selection, `Unrefuted`, identical full `SearchStats`, equal next `u64` draws,
and unchanged input. **Red:** sorting candidates before respecting the
sampled proposal, resampling alternatives, mutating edges, or using RNG in
checking breaks this test. **Green:** the guard receives only immutable root
edges and cloned boards; `AiPlayer::decide` extracts real statistics from
that tree after resolution. It never rewrites visits, priors, or value to
make a guarded choice look like the search favorite. A zero-simulation
terminal choice still has real zero-visit statistics, unlike a root proof.

### 3.9 Map only baseline CLI seats to the enabled guard

Before changing the final policy mapping, write
`baseline_enables_defensive_guard_but_tactics_preserves_proposal` in
[the session unit tests][session]. Reuse the single-threat coordinates and a
local `PreferBlock` evaluator with only `(7,7)` logit raised to `10.0` and
zero value. For each configured kind, **initialize a cheap tactics seat
first** with zero simulations, explicit selection true, nonempty fixture
opening, and seed `42`. Then change the private Black occupant's configured
kind to Baseline or Tactics and inject `PreferBlock` into Black's player slot.
Do not construct a real baseline just to test this policy switch.

Require Baseline to play `(7,7)` and Tactics to retain `(0,0)`, each as an AI
move event and matching Black stone on the resulting board. Both decisions
must retain real search info: every visit zero and leading prior `(7,7)`.
Direct private injection deliberately separates configured policy from the
concrete test evaluator. Production code must continue enforcing its normal
occupant/player invariant.

**Red:** disabled-for-all policy leaves the baseline case at `(0,0)`;
enabled-for-all changes tactics unexpectedly. **Green:** in the per-step
options construction in `SessionCore::execute_step`, map
`EvaluatorKind::Baseline` to `GuardConfig::baseline()` and
`EvaluatorKind::Tactics` to `GuardConfig::disabled()`. This is the only concrete
kind policy added here. Reusable defaults remain disabled; `AiPlayer` neither
identifies evaluator types nor imports baseline. No CLI guard flag or toggle
is introduced. Selection/simulation/guard options can change while retaining
the evaluator, and the current UI does not display the returned diagnostics.

## 4. Final branch audit

Review these observable results against the implementation and tests; do not
collapse different outcomes to a generic success/failure flag.

| Execution branch | Chosen move, statistics, and diagnostics |
|------------------|------------------------------------------|
| Already won/drawn input | `GameOver` error before evaluation/RNG; no decision. |
| Certified root win | Certified move, `search: None`, `ProvenWin`, zero checks/refutations, no RNG/guard work. |
| Ordinary search, disabled guard | Original proposal, real `Some` statistics, `Disabled`, zero checks. |
| Enabled zero cap | Original **unchecked** proposal, real statistics, `BudgetExhausted`, zero checks. |
| Checked own win/draw | Checked candidate, real statistics, `Terminal`, one check per played hypothetical candidate, no terminal opponent probe. |
| No proof or rejected proof | Current candidate, real statistics, `Unrefuted`; acceptance means unknown. |
| Accepted opponent proof | Record candidate once and advance; keep original proposal in diagnostics. |
| Cap with remaining candidate | Next ranked **UNTESTED** move, real statistics, `BudgetExhausted`; do not return a known-refuted proposal. |
| Every legal candidate refuted | Original legal proposal, real statistics, `AllRefuted`; not a root loss certificate. |

The check count includes terminal hypothetical moves but excludes untested
fallbacks. Refutations preserve checking order. All ordinary branches keep
the searched side's perspective and the actual post-noise prior distribution.
No branch grants certification to a nonterminal defense.

## 5. TDD checklist and verification commands

Run commands from the learner's **`gomoku/`**, not the root MNIST package.
Running them from the sibling reference's `gomoku/` verifies that source
instead; it does not land implementation on main. For a single red/green
cycle, use the exact test name supplied in the relevant section. Integration
tests can use `-- --exact`; private session tests have module-qualified names,
so a substring filter is convenient.

```text
cargo check -p player
cargo test -p player --test decisions terminal_board_returns_game_over_without_evaluation_or_rng -- --exact
cargo test -p player --test decisions
cargo test -p cli --lib persistent_player
cargo test -p cli --test actor persistent_players_match_seeded_legacy_search_through_opening_schedule -- --exact
cargo test -p player --test guard
cargo test -p player --lib
cargo test -p cli --lib baseline_enables_defensive_guard_but_tactics_preserves_proposal
cargo test -p player
```

For the new distinct-line regression, isolate its red/green cycle explicitly:

```text
cargo test -p player --test guard blocking_one_of_two_distinct_opponent_lines_is_refuted_in_favor_of_own_immediate_win -- --exact
```

- [ ] Four public decision tests precede their corresponding branches.
- [ ] Terminal rejection and root proof preserve an untouched RNG stream.
- [ ] Manual search comparison covers zero simulations, noisy sampling, full
  sorted statistics, and the next random draw.
- [ ] Evaluator observations persist; session counters and drops establish
  reuse through `Step`, navigation, and `SetSims`, and replacement on
  take/release/reset.
- [ ] Seeded eleven-ply legacy comparison passes before enabling any guard.
- [ ] Public guard tests use real proofs and cheap evaluator fixtures.
- [ ] Private tests establish retry order, budget/color forwarding, proof
  admission, and callback non-use for disabled/zero-cap/win/draw branches.
- [ ] Both zero-node and zero-depth outcomes remain unknown.
- [ ] Distinct horizontal/vertical lines have exact independently asserted
  threat sets and a real verified remaining-line proof.
- [ ] All-refuted independently rechecks every legal candidate; cap fallback
  remains explicitly untested.
- [ ] Enabled unknown preserves temperature proposal, RNG, input, and stats.
- [ ] CLI policy mapping is baseline-only without constructing Burn in its
  cheap unit test.
- [ ] Refactor only after each group is green; rerun earlier groups afterward.

### Targeted integration versus slow real-baseline validation

After the construction groups, run the broader inexpensive targets:

```text
cargo test -p cli --lib
cargo test -p cli --test session
cargo test -p cli --test actor -- --skip baseline_ai_plays_proven_vcf_without_search_info --skip baseline_ai_falls_back_to_search_info_on_quiet_position
```

The two named actor tests are different from the cheap policy test. They
construct the real baseline, including its Burn-backed pattern evaluator.
Especially `baseline_ai_falls_back_to_search_info_on_quiet_position` performs
50 real simulations and can be slow in debug; a long-running debug case is
not evidence that the private guard callback is stuck. Run those cases
separately, preferably in release for practical feedback, and report their
results separately from the filtered suite:

```text
cargo test --release -p cli --test actor baseline_ai_plays_proven_vcf_without_search_info -- --exact
cargo test --release -p cli --test actor baseline_ai_falls_back_to_search_info_on_quiet_position -- --exact
cargo test -p cli --test actor
```

Individual release cases and filtered checkpoints do not replace full
workspace QA. Fresh QA in the sibling reference's `gomoku/` reports:

| Command | Result |
|---------|--------|
| `cargo test --release --locked --all-targets` | Exit 0; 574 passed, 0 failed, 6 ignored across the workspace; 42 s including compilation, 4 s cached. All ten actor tests passed, including both real-baseline cases previously skipped. |
| `cargo clippy --locked --all-targets -- -D warnings` | Exit 0; 9 s. |
| `cargo fmt --all -- --check` | Exit 0. |
| `git diff --check` | Exit 0. |

The **21 player tests** comprise five private guard, four public decision,
and twelve public guard tests. The distinct-geometry regression is the
additional twelfth public guard test, not a rename of the two-ended-line test.
The six ignored tests are preexisting: two baseline acceptance, one CLI
acceptance, and three patterns differential tests. Reference verification
does not mean learner source has landed on main. Interactive play, measured
guard strength, and measured guard latency are not claimed. Both design Mermaid
fences now parse cleanly under mermaid 12.3.0 with jsdom (`mermaid.parse`:
sequenceDiagram → `sequence`, stateDiagram-v2 → `stateDiagram`, no errors);
this is syntactic parsing only, not browser/render validation.

## 6. Done when

- Both milestones have separate green checkpoints on the learner's source:
  preserved disabled decisions first, verified defensive checking second.
- The player depends only on engine, MCTS, and rand, owns one evaluator, and
  borrows board/options/RNG; every tree is fresh and no move is committed by
  the reusable library.
- Search measurements remain real, complete, deterministically sorted, and
  in searched-side perspective. Root certificates have no statistics.
- Session construction, take, release, new/reset, simulation updates, and
  navigation honor the specified resource lifetime and existing events.
- The ordinary order is still search, optional post-search **prior** noise,
  visit selection, then optional guard; RNG equivalence is checked explicitly.
- Proof admission uses the actual post-candidate side, legal first move, and
  verifier. Unknown is never advertised as safe.
- A reached cap returns an untested alternative when one exists; only actual
  refutation of every legal move permits original-move `AllRefuted` fallback.
- The exact distinct-line regression and the other 20 player tests pass on
  the source being accepted; CLI policy tests retain honest zero-visit info.
- Baseline-only mapping remains a CLI policy; no new evaluator hook, CLI
  toggle, engine behavior, asynchronous work, or search-tree persistence was
  introduced.
- Unfiltered integration and repository gates have actually been run and
  recorded for acceptance, with every exclusion stated explicitly.

## References

- [The AI player: chapter][chapter] — behavioral specification and vocabulary.
- [The AI player: design paper][design] — ownership, transitions, and invariants.
- [The TSS eye: construction][root] — prerequisite evaluator-owned root hook.
- Isolated reference manifests: [Gomoku workspace][workspace],
  [player package][player-manifest], [CLI package][cli-manifest].
- Isolated reference implementation: [player library][player],
  [private guard and unit tests][guard], [session and unit tests][session],
  [actor records][actor].
- Isolated reference executable specifications: [decision tests][decisions],
  [public guard tests][guard-tests], [actor tests][actor-tests],
  [session integration tests][session-tests].
- Reused source: [MCTS search][search], [move policy helpers][policy],
  [engine prover/verifier][tss], [engine board][board].
- [1] Silver et al. (2017), [Mastering the game of Go without human
  knowledge](https://doi.org/10.1038/nature24270). Temperature and root noise;
  this extraction's post-search timing is a local compatibility requirement.
- [2] Allis (1994), [Searching for Solutions in Games and Artificial
  Intelligence](https://doi.org/10.26481/dis.19940923la). Threat-space search;
  the five-candidate/1,000-node/ten-ply work bounds are local experiments.

[chapter]: ./ai-player.md
[design]: ./ai-player-design.md
[root]: ../08-the-tss-eye/the-tss-eye-implementation.md
[workspace]: ../../../../../rust-ml-ai-player-ref/gomoku/Cargo.toml
[player-manifest]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/player/Cargo.toml
[player]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/player/src/lib.rs
[guard]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/player/src/guard.rs
[decisions]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/player/tests/decisions.rs
[guard-tests]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/player/tests/guard.rs
[cli-manifest]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/cli/Cargo.toml
[session]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/cli/src/session.rs
[actor]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/cli/src/actor.rs
[actor-tests]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/cli/tests/actor.rs
[session-tests]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/cli/tests/session.rs
[search]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/mcts/src/search.rs
[policy]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/mcts/src/policy.rs
[tss]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/engine/src/tss.rs
[board]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/engine/src/board.rs
