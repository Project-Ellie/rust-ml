# Sub-chapter 08b — The AI player

## Abstract

This chapter gives an AI seat a reusable move-deciding object, then adds a
bounded check for moves that permit a verified opponent win. First preserve
existing moves, random-number consumption, and search records while retaining
one evaluator per seat. Then enable defensive candidate checking for baseline
seats in the command-line interface. The check can certify a refutation, not a
safe defense. This is a learn-by-doing chapter: implement the named executable
tests before the corresponding production behavior; no implementation bodies
are supplied here.

## Glossary

| Term | Meaning here |
|------|--------------|
| AI seat | A Black or White seat whose moves are produced automatically. |
| CLI | Command-line interface; its study session owns commands, history, and events. |
| MCTS | Monte Carlo tree search: repeated simulations accumulate root move visits and values. |
| Evaluator | An implementation of `mcts::Evaluator`, providing position estimates and optionally a certified root winning move. |
| Root hook | `Evaluator::evaluate_root`, called before simulations on an ongoing root. |
| Root proof | `SearchResult::ProvenWin`; certification supplied through the evaluator seam, not an estimated value. |
| Proposal | The ordinary move selected from root visits with the requested temperature, before defensive checking. |
| Prior | A root edge's policy probability; distinct from its visit count. |
| Temperature | The control used by `select_move` to choose the most-visited move or sample from visit-derived weights. |
| Dirichlet noise | Random perturbation of root priors; in the preserved path it happens **after** simulations. |
| TSS | Threat-space search: the engine's bounded search through forcing threats and replies. |
| Candidate guard | A decision-local check of hypothetical moves for verified opponent winning proofs. |
| Refuted | A candidate after which the opponent has a proof passing winner, legal-first-move, and line verification checks. |
| Unrefuted | No accepted opponent proof was found within the work bounds; **unknown**, not safe. |
| RNG | Random-number generator, owned by the session and borrowed by a decision. |
| `[experiment]` | A project tuning choice, not a theorem or published strength/latency guarantee. |
| TDD | Test-driven development: write a failing behavioral test, make it pass, then improve structure without changing behavior. |

## Context

[Chapter 08][chapter08] introduced a root hook on the evaluator. On an ongoing
board, `mcts::search` asks that hook once. A certified winning move returns as
`SearchResult::ProvenWin`; otherwise `SearchResult::Searched` carries a real
tree after simulations. Ordinary leaf evaluation remains estimate-only. This
chapter consumes that seam without changing it.

The current CLI step path also applies selection settings, samples a move,
and stores search information. That mixes a reusable question—“which move
should this evaluator play from this board?”—with session responsibilities.
The baseline evaluator also constructs its pattern network and gravity
kernel; reconstructing it on every move is unnecessary. Persistence here means
retaining the evaluator, **not** retaining the MCTS tree.

The reference for this chapter is the implemented sibling worktree
`rust-ml-ai-player-ref`, especially the [player library][player-lib],
[guard module][guard-src], and [CLI session][session-src]. These are source
references, not code to copy into the tutorial. Paths in the artifact table
below are files to create or change in the learner's `gomoku/` workspace; links
point to the implemented reference. Main need not contain those artifacts yet.

## Intention

At the study table, a user can step an AI seat repeatedly, change its
simulation count, explore a variation, take the seat back, or release it to the
AI again. Those actions must retain their existing command behavior. Repeated
steps and simulation-count changes should reuse the seat's evaluator rather
than initialize it again. Taking a seat should release that runtime resource;
starting a new session position should restore the initial seats.

After that behavior is preserved, a baseline seat should avoid its proposed
move when the engine can verify an opponent winning continuation after it and
another candidate is available. It should still keep a temperature-selected
proposal if no such refutation is established. The table must continue to show
what the search actually measured, not invented visits for a proof or adjusted
visits that make a guarded choice look like the search's favorite.

There are two milestones:

1. **Persistent player, guard disabled.** Same seeded move path and search
   records, evaluator retained per seat.
2. **Defensive candidate guard.** Opt-in at the reusable API, enabled by CLI
   policy for baseline seats only; no claim of certified defensive safety.

Neither milestone adds asynchronous execution, threads, a new evaluator hook,
search-tree reuse, or changes to engine rules.

## Artifacts and contracts

| Workspace file | Named artifact and contract |
|----------------|-----------------------------|
| [Cargo.toml][workspace-manifest] | Add `crates/player` to workspace members. |
| [crates/player/Cargo.toml][player-manifest] | Dependencies are `engine`, `mcts`, and `rand` (`0.10.3` in the reference), not Burn, `baseline`, `patterns`, or `cli`. |
| [crates/player/src/lib.rs][player-lib] | `AiPlayer` owns `Box<dyn Evaluator>`; `new` takes ownership, `decide` borrows a board, per-move options, and mutable RNG. Public items satisfy `#![deny(missing_docs)]`. |
| [crates/player/src/guard.rs][guard-src] | Private module containing candidate resolution and publicly re-exported `GuardConfig`, `GuardDiagnostics`, `GuardOutcome`. The `choose` helper stays private to the crate. |
| [crates/player/tests/decisions.rs][decision-tests] | Public-interface extraction and persistence tests. |
| [crates/player/tests/guard.rs][guard-tests] | Public-interface guard tests with the real engine prover. |
| [crates/cli/Cargo.toml][cli-manifest] | Add the path dependency on `player`. |
| [crates/cli/src/session.rs][session-src] | `SessionCore` retains optional Black/White players and owns their lifecycle, effective options, history, RNG, and events. |
| [crates/cli/src/actor.rs][actor-src] | `SearchInfo::from_stats` adds the effective `AiConfig` to player statistics. Retain `from_searched` for existing callers and comparison tests. |
| [crates/cli/tests/actor.rs][actor-tests] | Seeded comparison with the pre-extraction move path, across the opening schedule boundary. |

The public decision contract in the [player library][player-lib] is:

- `AiPlayer::new(evaluator: Box<dyn Evaluator>) -> Self`.
- `AiPlayer::decide(&mut self, board: &Board, options: &PlayerOptions,
  rng: &mut impl Rng) -> Result<Decision, DecisionError>`.
- `PlayerOptions`: `simulations`, `c_puct`, `temperature`, `noise_epsilon`,
  `noise_alpha`, `guard`. Defaults derive simulation count and exploration
  constant from `SearchConfig::default`; temperature and noise fraction are
  zero, noise concentration is `0.3`, and the guard is disabled.
- `Decision`: `mv`, `search: Option<SearchStats>`, `guard: GuardDiagnostics`.
- `SearchStats`: `side_to_move`, `visits`, `prior`, `value`; no proof flag and
  no manufactured proof distribution.
- `DecisionError::GameOver`: an already won or drawn board is rejected before
  evaluation or RNG consumption. This is not the root-proof result.
- `GuardConfig`: `enabled`, `max_candidates`, `budget: SearchBudget`.
  Both its default and `PlayerOptions::default().guard` are disabled.
- `GuardDiagnostics`: `proposal`, ordered `refuted` moves, `checked`, `outcome`.
  Outcomes are `Disabled`, `ProvenWin`, `Unrefuted`, `Terminal`,
  `BudgetExhausted`, and `AllRefuted`.

For ordinary searches, visits include **every** root edge, even zero-visit
moves, sorted by descending visits then ascending move index. Priors include
all root edges after optional noise, sorted by descending prior then ascending
index. Value is the sum of root edge accumulated values divided by total root
visits, or zero when there are no visits. Its perspective is the side to move
on the searched board, not the side to move after playing the decision.

## Specification as executable tests — write these first

The following names are executable targets in the linked reference test
files. In the learner's workspace, create each group before implementing the
behavior it specifies. Supply only enough declarations to compile, run the
named target to see the missing behavior fail, and then implement it. Do not
write the whole player before returning to these tests.

### Milestone 1A: public decision behavior

Create [crates/player/tests/decisions.rs][decision-tests]. Use these four tests
in this order; their fixtures are small enough to isolate the contracts.

| Test | Setup and required assertions |
|------|-------------------------------|
| `terminal_board_returns_game_over_without_evaluation_or_rng` | Replay a won board. Both methods of `NeverEvaluate` panic if called. Expect `GameOver`; compare the next draw with an untouched equally seeded RNG. |
| `ordinary_decisions_match_manual_search_stats_and_seeded_selection` | On a board containing the center stone, compare a player with a manual `search` → optional noise → visit selection path using separate equal-seed RNGs and uniform evaluators of value `0.6`. Cover `(simulations, temperature, noise fraction)` of `(17, 0, 0)`, `(19, 1, 0)`, `(23, 0.7, 0.25)`, `(0, 1, 0.25)` with exploration constant `1.7` and noise concentration `0.3`. Assert the same legal move, sorted visits and priors, root value, visit sum equal to simulations, and next RNG draw. |
| `root_proof_has_no_search_stats_leaf_calls_or_rng_consumption` | `ProofEvaluator` returns a legal immediate winning move from its root hook; its leaf method panics. Enable noise, temperature, and guard deliberately. Assert the move, `search.is_none()`, one root call, zero checked candidates, `ProvenWin` diagnostics, no refutations, and unchanged RNG state. |
| `evaluator_instance_persists_across_two_decisions` | `StatefulEvaluator` keeps a local call counter observed through shared test state. With one simulation per decision, decide, play, then decide again. Observations must be `[1, 2, 3, 4]`, not a restarted counter. |

`Rc<RefCell<_>>` in these doubles observes mutable evaluator state without
making the production evaluator clonable or thread-safe. `StdRng::seed_from_u64`
and comparing the *next* draw test stream consumption, not just coincidentally
equal moves. Keep assertions on public decisions and statistics.

### Milestone 1B: CLI integration and evaluator lifetime

Add the following tests to the test module in
[crates/cli/src/session.rs][session-src], where private player slots can be
replaced by cheap counting or proof evaluators.

| Test | Required behavior |
|------|-------------------|
| `persistent_player_reuses_evaluator_across_steps_and_set_sims` | One-simulation step makes two evaluator calls. Go back, set Black to three simulations, step again: cumulative calls become six and visits/config show three. No evaluator drop on `SetSims`; taking the seat drops it once. |
| `persistent_players_follow_init_release_take_and_reset` | Initial AI seats have players; humans do not. Releasing White retains Black's evaluator. Setting both simulation counts retains both. `New` drops both injected evaluators and restores initial occupants and counts. Taking both clears both players; releasing both constructs both. |
| `persistent_player_proof_plays_without_search_info_or_rng_draws` | Inject a root-proof evaluator. `Step` emits the normal AI move and terminal event on its immediate-win fixture, stores no search record at the new cursor, and consumes no RNG. |

Then add
`persistent_players_match_seeded_legacy_search_through_opening_schedule` in
[crates/cli/tests/actor.rs][actor-tests]. Compare eleven plies of two tactics
seats against a manual old path that constructs a fresh evaluator each time.
Use seed `42` and three simulations. Plies zero through nine use temperature
`1.0` and noise fraction `0.25`; the eleventh move uses configured defaults.
Assert the events, board, and full `SearchInfo` after each step. This checks the
extraction and schedule boundary, not baseline playing strength.

### Milestone 2A: guard behavior through the public player

Create [crates/player/tests/guard.rs][guard-tests]. Most fixtures use zero
simulations: ordinary temperature-zero selection then chooses the lowest-index
legal move, while a deliberately high prior makes an alternative rank first
after refutation. This separates proposal selection from retry ordering.

| Test | Required assertion |
|------|--------------------|
| `baseline_guard_is_opt_in_with_fixed_experiment_work_bounds` | Default player options disable the guard; `baseline()` enables five candidate checks with `1_000` nodes and ten plies per probe. |
| `real_proof_refutes_proposal_and_guard_finds_unvisited_block_without_mutating_board_or_stats` | White has a single winning endpoint at `(7,7)`; Black's initial `(0,0)` proposal permits a real verified opponent proof. With the guard, choose the unvisited block, record one refutation and two checks, and keep board, Zobrist key, and search statistics unchanged. Outcome is `Unrefuted`, not safe. |
| `two_opponent_winning_threats_require_our_immediate_winning_alternative` | Refute the initial proposal against White's two-ended four; accept Black's immediate winning alternative as `Terminal`. |
| `blocking_one_of_two_distinct_opponent_lines_is_refuted_in_favor_of_own_immediate_win` | Independently establish separate horizontal and vertical White wins. Blocking one still permits a real verified proof on the other; retry Black's own immediate win. Assert `Terminal`, ordered refutation, two checks, unchanged statistics, next RNG draw, board, and Zobrist key; see the exact fixture below. |
| `white_player_guard_probes_actual_black_opponent` | Mirror the responsibility, not just the coordinates: White to move must probe Black after hypothetical play and select the block. |
| `zero_depth_probe_is_unknown_even_with_opponent_immediate_win` | A zero-depth prover budget leaves the proposal `Unrefuted` with one check and no recorded refutation, even though the fixture permits an opponent win. |
| `zero_node_probe_is_unknown_even_with_opponent_immediate_win` | The same unknown contract holds for zero nodes. |
| `cap_falls_back_to_unchecked_block_not_refuted_original` | With a one-candidate cap, refute `(0,0)`, choose the highest-ranked remaining untested move `(7,7)`, and report `BudgetExhausted` with one check. |
| `zero_candidate_cap_keeps_original_unchecked` | Keep the proposal with zero checks and `BudgetExhausted`. |
| `every_legal_move_really_refuted_reports_all_refuted_and_original_fallback` | Give the cap room for every legal move on a two-ended opponent-four fixture. Recheck each recorded refutation with the real prover and verifier. Only after all legal candidates are refuted, retain the original legal proposal and report `AllRefuted`. |
| `own_immediate_win_is_terminal_not_opponent_probe_unknown` | Even with a zero prover budget, accept an immediate own win as `Terminal`, with one check. |
| `enabled_unknown_preserves_seeded_first_candidate_noise_stats_and_rng` | With zero-node probes, compare enabled and disabled guards at zero and eleven simulations, temperature `0.7`, noise fraction `0.25`, seed `42`. The same proposal is retained, statistics and next RNG draw match, and the board is unchanged. |

For the single-threat fixture, Black stones are `(7,2)`, `(2,0)`, `(2,2)`,
`(2,4)`; White stones are `(7,3)` through `(7,6)`; Black moves next. The
`Prefer` test evaluator raises the `(7,7)` policy logit but offers no root
proof. Consequently the test exercises the guard rather than the root hook.

For the distinct-line regression, use these exact zero-based `(row,column)`
stones with Black to move:

| Color | Stones |
|-------|--------|
| Black | `(0,5)`, `(5,10)`, `(13,5)`, `(13,7)`, `(9,1)`, `(9,2)`, `(9,3)`, `(9,4)` |
| White | `(0,1)`, `(0,2)`, `(0,3)`, `(0,4)`, `(1,10)`, `(2,10)`, `(3,10)`, `(4,10)` |

Assert White's exact immediate-win set is `[(0,0),(0,10)]`: the horizontal
line needs `(0,0)`, the vertical line needs `(0,10)`. Clone/play `(0,0)` and
assert the remaining set is exactly `[(0,10)]`. Independently call the real
prover for `after.to_move()` with `1_000` nodes and ten plies; require White
winner, exact line `[(0,10)]`, and successful `verify_line`. This is not the
older two-endpoints-of-one-line fixture; retain both tests.

Use separate `Prefer(9,5)` players, zero simulations, seed-`1` RNGs, and
disabled versus enabled checking; enabled cap is two with that same budget.
The disabled decision must select `(0,0)` with `Disabled`, all visits zero,
and first prior `(9,5)`. The guarded decision must retain proposal `(0,0)`,
record only that refutation, check two candidates, and select `(9,5)` with
`Terminal`. Require identical complete search statistics and next RNG draws.
On another clone play `(9,5)` and require `Won(Black)`; the original board
and Zobrist key must equal their saved values. These assertions specify
behavior without supplying a test or production implementation body.

### Milestone 2B: private guard edge cases and CLI policy

Write these tests beside `choose` in
[crates/player/src/guard.rs][guard-src]. Its injected probe callback allows
precise observations that an external test cannot obtain.

| Test | Required assertion |
|------|--------------------|
| `alternatives_rank_visits_then_prior_then_index_without_repeats_and_include_unvisited` | Reverse edge creation order. Check proposal first, then descending visits, descending prior, ascending index; each once. Five refutations must lead to the next ranked, unvisited, untested move. Assert correct opponent and budget forwarding; board and edges stay equal. |
| `malformed_or_unverified_proofs_leave_candidate_unknown` | Wrong winner, empty line, occupied/illegal first move, and an unverified line cannot refute the proposal. Expect one probe and `Unrefuted`. |
| `disabled_and_zero_cap_never_call_probe` | Inject a callback that panics. Disabled guard and enabled zero cap must return the original proposal without calling it, with their distinct outcomes. |
| `immediate_win_never_calls_opponent_probe` | An immediate own win returns `Terminal` after one hypothetical check without invoking the callback. |
| `immediate_draw_never_calls_opponent_probe` | Replay a valid 224-ply non-winning board, accept its last move as `Terminal`, and never invoke the callback. Keep the input board unchanged. |

The draw fixture replays stones rather than importing them with
`Board::from_position`: imported positions do not populate the move history
used by subsequent `Board::play` draw bookkeeping. This is a fixture constraint,
not an engine redesign in this chapter.

Finally add `baseline_enables_defensive_guard_but_tactics_preserves_proposal`
to the CLI [session test module][session-src]. Initialize a cheap tactics seat,
then inject the same fixed evaluator under each configured evaluator kind.
Baseline policy selects the unrefuted block; tactics policy retains the
proposal. Both retain real zero-visit search information. This test verifies
**CLI policy mapping without constructing or running the Burn baseline**.

## Walkthrough — implement in execution order

### Milestone 1: retain the evaluator and preserve decisions

#### 1. Establish the reusable boundary

After the first decision tests are in place, register the crate and implement
the public contracts in the [player library][player-lib]. `Box<dyn Evaluator>`
provides runtime evaluator selection without a dependency on a concrete
baseline type. `&mut self` permits the evaluator to retain mutable state;
`&Board` prevents the player from committing session moves; the borrowed RNG
keeps the session's seed stream authoritative. Options belong to a decision,
not to evaluator construction.

For this milestone, `GuardConfig::disabled()` and disabled diagnostics are the
only candidate path needed. Define the guard data surface without implementing
its probes yet. No change to `mcts::Evaluator`, `RootEval`, or `SearchResult`
is necessary.

#### 2. Reject a terminal input before work

At the start of `AiPlayer::decide`, inspect `board.status()`. Return
`DecisionError::GameOver` for won or drawn inputs before evaluation, noise,
or sampling. The CLI already checks this, but a reusable API cannot rely on
its caller being the CLI. Enabled noise panics if Gamma rejects its alpha;
the noise helper also asserts that root edges exist. An empty ongoing-root
move distribution panics at selection's `expect`. The helper does not validate
`noise_epsilon`; do not add parameter validation or new error variants.

#### 3. Search once and distinguish proof from measurements

Construct `SearchConfig` from `simulations` and `c_puct`, then call
`mcts::search` with the retained evaluator. Match both results explicitly.

A `ProvenWin` returns its move immediately, `search: None`, and diagnostics
whose proposal is that move, outcome is `ProvenWin`, and check count is zero.
The evaluator has already certified the root move. Do not run defensive
probes, consume RNG, manufacture visits, or attach a value of one as if it
were a measured search value.

A `Searched` result proceeds through the existing move-selection path.

#### 4. Preserve noise timing, sampling, and RNG consumption

On the searched path, apply `add_dirichlet_noise` only when the noise fraction
is positive. Extract `visit_distribution`, then call `select_move` with the
requested temperature and borrowed RNG. Temperature zero selects the largest
visit count with lowest-index tie-breaking; positive temperature samples
visit-derived weights, with uniform fallback when all visits are zero.

The noise and temperature mechanisms are described in AlphaGo Zero
[1], but **this implementation's timing is not its pre-search noise policy**.
Here noise is added after simulations. It changes root **priors**, not current
visits or accumulated values. It therefore does not directly reweight the
current temperature distribution. It does consume RNG before sampling, so
removing or moving it can still change a seeded proposal. Later it can also
change the prior tie-break among guard alternatives. Preserve this ordering
rather than claiming the guard receives a “noisy visit distribution.”

#### 5. Extract honest search statistics

Implement `SearchStats::from_searched` as a private helper in the
[player library][player-lib]. Collect all root visits and priors, sort each
with its own deterministic tie-break, and compute the root value with an
explicit zero-visit branch. Use `then_with` to make tie rules visible;
`partial_cmp` on priors uses `Ordering::Equal` when incomparable, matching the
reference. These records describe the tree, not an endorsement of the final
move. The guard milestone must not rewrite them.

#### 6. Give each session seat a runtime player

Now make the lifecycle tests pass in the [CLI session][session-src]. Keep
`black_player` and `white_player` as `Option<AiPlayer>` beside the serializable
occupants, not inside `AiConfig` or `Occupant`. Those configuration types retain
their existing clone, equality, and serialization behavior; a boxed evaluator
is runtime state, not a configuration value.

Use `make_player(&Occupant)` to map an AI occupant to a newly owned evaluator
through `make_evaluator`, and a human occupant to `None`. Construct initial
players in `SessionCore::new`. Centralize replacement in `set_occupant`, so
`execute_release` constructs the selected seat's player, `execute_take` drops
it, and `reset` replaces both using the initial occupants. `execute_new` calls
`reset`. Reset retains the existing session RNG rather than reseeding it.

`execute_set_sims` changes the occupant's configuration and seat override,
**not** the player. Moves, history navigation, and puzzle navigation also do
not reconstruct the evaluator. A later release or reset is a deliberate new
runtime instance.

#### 7. Route a step without moving session responsibilities

In `SessionCore::execute_step`, retain the existing checks in order: reject
an opening ceremony, reject a finished board, and require the side-to-move
seat to be AI. Clone its config into `effective_config`; the existing opening
schedule changes temperature to `1.0` and noise fraction to `0.25` for the first
ten plies of an eligible empty-root AI-versus-AI line. It is disabled by
explicit selection settings and is keyed to the current history ply.

Build `PlayerOptions` from that effective config with noise concentration
`0.3`; keep the guard disabled until milestone 2. Borrow the side's stored
player with `as_mut()` and pass `&mut self.rng` to `decide`. The reference uses
`expect` for a missing player after the AI-occupant check: lifecycle invariants,
not user error, guarantee its presence. Map the player's terminal error to the
existing `CommandError::GameOver`.

For a proof decision, use `execute_play` and return without adding a search
record. For a searched decision, use `SearchInfo::from_stats` in
[crates/cli/src/actor.rs][actor-src] to attach the **effective** config. Preserve
the existing searched branch: `history.play`, storage at the new cursor, AI
move event, branch event when needed, and terminal event when needed. The
reference does not route this searched branch through `execute_play`.
`SessionCore`, not the player, still commits moves and owns event ordering.
Guard diagnostics are returned by the player but are not stored or displayed
by this CLI integration.

Run the seeded eleven-ply comparison before beginning defensive behavior.

### Milestone 2: reject verified losing candidates, not unknown candidates

#### 8. Add the bounded guard and its private injection seam

Implement `GuardConfig` and the resolution helper `choose` in the private
[guard module][guard-src]; re-export only its public data types. The production
call supplies `engine::prove_forced_win` as the probe callback. Tests supply a
closure to observe calls or return malformed proofs. An `impl FnMut` callback
keeps that testing seam local; it is not a new method on `Evaluator`.

`GuardConfig::baseline()` enables **five** candidate checks with a per-probe
`SearchBudget` of **1,000 nodes and ten plies**, all `[experiment]`. Disabled
configuration retains those work settings but turns checking off. These are
work bounds, not a strict response-time deadline: candidate construction,
board cloning, proof verification, and the preceding search are not timed by
that budget.

#### 9. Preserve the proposal and rank only the retries

After ordinary selection, pass the board, proposal, immutable root edges, and
guard config to `choose`. Disabled mode returns the proposal with zero checks.
Enabled mode with a zero cap also returns the proposal unchecked, but with
`BudgetExhausted`.

For enabled positive caps, construct a candidate iterator with
`std::iter::once(proposal)` followed by remaining root moves sorted by descending
visits, descending prior, then ascending move index. Filter out the proposal;
skip any already refuted candidate. Root edges include unvisited legal moves.
Do not resample, replace a temperature proposal just because another move ranks
higher, or consume more RNG during retries.

#### 10. Check a candidate on a clone, with the actual opponent

Before probing, clone the input board and play the candidate. Increment
`checked` for this hypothetical check. If it wins immediately for the original
side to move or produces a draw, return `Terminal` without probing.

Otherwise use `after.to_move()` as the opponent. Call the bounded engine TSS
prover [2] on the resulting board for that opponent. A returned proof refutes
the candidate only if its winner is the opponent, its first move exists and is
legal on that resulting board, and `engine::verify_line` accepts the line.
`Option::is_some_and` can express both the proof and first-move checks without
indexing an empty line. Short-circuit evaluation avoids verification of a
proof that already fails a prerequisite.

Record a valid refutation and continue. `None` or any rejected proof returns
the candidate as `Unrefuted`: the result is **UNKNOWN**, not evidence of safety.
The refutation is proof-certified; accepting a defense is not.

#### 11. Distinguish a candidate cap from exhausting legal moves

Check the cap before checking the next candidate. If previous checks have
all been refuted and the cap has been reached, return that next candidate
untested with `BudgetExhausted`. It is the highest-ranked remaining move, not
the already-refuted original proposal. Its safety is unknown.

If iteration instead exhausts **every legal candidate**, all have been
individually refuted. Assert that the number of refutations equals
`board.empty_moves().count()`, return the original legal proposal, and report
`AllRefuted`. This explicit fallback may play a known-refuted move; it keeps
the move interface usable when none escaped refutation. The diagnostic is not
a composed root-position loss proof and is not grounds for resignation.

#### 12. Enable baseline policy only at the CLI boundary

In the options construction in `SessionCore::execute_step`, map
`EvaluatorKind::Baseline` to `GuardConfig::baseline()` and
`EvaluatorKind::Tactics` to `GuardConfig::disabled()`. The reusable player's
default remains disabled and it never identifies an evaluator's concrete type.
A library caller can opt out by passing disabled options; this reference adds
no CLI guard-toggle command or flag.

Run the policy-mapping session test after the real-prover and private-helper
tests. Effective simulation, temperature, noise, and guard options change
without reconstructing the evaluator. The guard neither modifies the input
board nor the root edges, visits, priors, or value.

## TDD checklist

- [ ] Milestone 1A: terminal rejection, manual path equivalence including zero
  simulations and RNG state, proof-without-stats, and evaluator persistence.
- [ ] Milestone 1B: lifetime/drop tests, effective `SetSims` behavior,
  proof event behavior, and eleven-ply seeded CLI comparison.
- [ ] Pause: with checking disabled, no change to seeded moves or records.
- [ ] Milestone 2A: all twelve public guard tests, including
  `blocking_one_of_two_distinct_opponent_lines_is_refuted_in_favor_of_own_immediate_win`:
  exact independent horizontal/vertical threat sets, verified remaining-line
  proof, own-win retry, and unchanged statistics/RNG/board. Also cover either
  color, unknown budgets, one/zero caps, and all-legal-refuted fallback.
- [ ] Milestone 2B: deterministic retry order, malformed proofs, no-probe
  branches including a replay-built draw, and CLI evaluator-kind policy.
- [ ] Review each refactor only after its tests are green; do not repair
  failures by changing the expected move path or inventing proof statistics.

## Verification commands and evidence

Run these from the learner's `gomoku/` after implementation; from the sibling
reference's `gomoku/` they inspect the reference instead. Do not confuse a
reference result with source having landed on main.

```text
cargo check -p player
cargo test -p player
cargo test -p cli --lib
cargo test -p cli --test session
cargo test -p cli --test actor -- --skip baseline_ai_plays_proven_vcf_without_search_info --skip baseline_ai_falls_back_to_search_info_on_quiet_position
```

Fresh QA in the sibling reference's `gomoku/` reports the full gates green:

| Command | Result |
|---------|--------|
| `cargo test --release --locked --all-targets` | Exit 0; 574 passed, 0 failed, 6 ignored across the workspace; 42 s including compilation, 4 s cached. All ten actor tests passed, including both real-baseline cases previously skipped. |
| `cargo clippy --locked --all-targets -- -D warnings` | Exit 0; 9 s. |
| `cargo fmt --all -- --check` | Exit 0. |
| `git diff --check` | Exit 0. |

The current **21 player tests** comprise five private guard, four public
decision, and twelve public guard tests; the distinct-line regression is
additional to the older two-ended-line test. The six ignored tests are
preexisting measurements/proofs: two baseline acceptance, one CLI acceptance,
and three patterns differential tests. The earlier scoped/filtered commands
above are useful construction checkpoints, not the final QA scope. Reference
verification does not mean learner source has landed on main. Interactive
play is not claimed. Both [design][design] Mermaid fences now parse
cleanly under mermaid 12.3.0 with jsdom (`mermaid.parse`: sequenceDiagram →
`sequence`, stateDiagram-v2 → `stateDiagram`, no errors); syntactic parsing
only, no browser/render validation. These correctness results imply no measured guard strength
or latency.

## Done when

- The two milestones can be verified separately: extraction preserves the
  disabled path, then guard tests establish the new behavior.
- One evaluator is retained per AI seat across moves and `SetSims`; take,
  release, and reset follow the specified resource lifetime.
- Terminal inputs fail without evaluation/RNG; certified roots return a move
  without fabricated statistics, sampling, or guard checks.
- Ordinary decisions retain full real statistics even if the selected move
  changes after a refutation; events and records remain session-owned.
- Cap exhaustion chooses the highest-ranked untested alternative when one
  exists; only exhaustion of all legal refutations retains the original.
- Unknown outcomes are never described as safe, and `AllRefuted` is never
  advertised as a root loss certificate.
- Baseline checking is a CLI policy; library defaults remain disabled and
  the new crate has no Burn dependency.
- The unfiltered tests and repository gates above have actually passed on the
  implementation being accepted; record any remaining exclusions explicitly.

## References

- [Chapter 08 — The TSS eye][chapter08]: root certification before simulations;
  08b adds defensive checks after ordinary candidate selection, not at leaves.
- [The AI player: design paper][design]: ownership, diagrams, transitions,
  catalogs, and invariants.
- [Chapter 08b work plan][work-plan]: milestone and validation ledger.
- Implemented reference: [player manifest][player-manifest],
  [player library][player-lib], [guard module][guard-src],
  [decision tests][decision-tests], [guard tests][guard-tests].
- Implemented reference integration: [workspace manifest][workspace-manifest],
  [CLI manifest][cli-manifest], [session implementation and unit tests][session-src],
  [actor types][actor-src], [actor integration tests][actor-tests],
  [session integration tests][session-tests], [MCTS move policy][policy-src],
  [engine board][board-src], [engine prover and verifier][tss-src].
- [1] Silver et al. (2017), [Mastering the game of Go without human
  knowledge](https://doi.org/10.1038/nature24270): root noise and temperature
  selection. The post-search timing preserved here is a local implementation
  fact, not a claim about that paper's algorithm.
- [2] Allis (1994), [Searching for Solutions in Games and Artificial
  Intelligence](https://doi.org/10.26481/dis.19940923la): threat-space search.
  The guard budgets and acceptance policy here are local experiments.

[chapter08]: ../08-the-tss-eye/the-tss-eye.md
[design]: ./ai-player-design.md
[work-plan]: ./WORK-PLAN.md
[workspace-manifest]: ../../../../../rust-ml-ai-player-ref/gomoku/Cargo.toml
[player-manifest]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/player/Cargo.toml
[player-lib]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/player/src/lib.rs
[guard-src]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/player/src/guard.rs
[decision-tests]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/player/tests/decisions.rs
[guard-tests]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/player/tests/guard.rs
[cli-manifest]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/cli/Cargo.toml
[session-src]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/cli/src/session.rs
[actor-src]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/cli/src/actor.rs
[actor-tests]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/cli/tests/actor.rs
[session-tests]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/cli/tests/session.rs
[policy-src]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/mcts/src/policy.rs
[board-src]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/engine/src/board.rs
[tss-src]: ../../../../../rust-ml-ai-player-ref/gomoku/crates/engine/src/tss.rs
