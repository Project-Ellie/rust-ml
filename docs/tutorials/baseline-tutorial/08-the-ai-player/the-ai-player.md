# Sub-chapter 08 — The AI player

## Abstract

This sub-chapter gives an AI seat a reusable move-deciding object that
orchestrates the whole decision in one place. The new `player` crate owns
one persistent evaluator per seat. Its `AiPlayer::decide` first asks the
engine for a certified forced win for the side to move, verifying the claim
itself before accepting it. When no proof is certified, it runs ordinary
Monte Carlo tree search (MCTS), applies optional root noise after the
search, selects a proposal by temperature, and then applies a bounded
candidate guard that can refuse moves the opponent can refute with a
verified winning proof. The `mcts` and `engine` crates stay byte-untouched:
there is no evaluator root hook. This is a learn-by-doing chapter:
implement the named executable tests before the corresponding production
behavior; no implementation bodies are supplied here.

**Superseded design note.** The retired chapter-08 design ("the TSS eye")
placed the root proof inside the evaluator seam as an `evaluate_root`
hook, and the provisional 08b chapter consumed that hook. Both are
replaced by this chapter's single player-level orchestration. The old
packs remain in the repository as legacy history; nothing in them is the
build target. The learner's main tree at sub-chapter-07 state is
authoritative: it contains no root hook, and none is added.

## Glossary

| Term | Meaning here |
|------|--------------|
| AI seat | A Black or White seat whose moves are produced automatically. |
| CLI | Command-line interface; its study session owns commands, history, and events. |
| MCTS | Monte Carlo tree search: repeated simulations accumulate root move visits and values. |
| Evaluator | An implementation of `mcts::Evaluator`, a trait with the single method `evaluate`, providing policy and value estimates for one position. |
| Persistent player | An `AiPlayer` retained across moves, holding one evaluator per AI seat. The MCTS tree is not retained. |
| Own probe | One bounded call to `engine::prove_forced_win` for the side to move at the start of a decision, before search. |
| TSS | Threat-space search: the engine's bounded search through forcing threats and replies. |
| VCF | Victory by continuous forcing: a sequence of threats the defender must answer until the attacker wins. |
| `Proof` | The engine's certified forced win: a winner and one forcing line. |
| Admission | The player's own re-check of a claimed proof: correct winner, non-empty line, legal first move, and a verified line. |
| Certified own move | The first move of an admitted own proof, played without search. |
| `SearchBudget` | Prover work caps (`max_nodes`, `max_depth`); not a wall-clock deadline and not a bound on verification work. |
| Work bounds | Candidate count and per-probe node/depth limits; `[experiment]` values, not measured optima. |
| Proposal | The ordinary move selected from root visits with the requested temperature, before candidate checking. |
| Prior | A root edge's policy probability; distinct from its visit count. |
| Temperature | The control used by `select_move` to choose the most-visited move or sample from visit-derived weights. |
| Dirichlet noise | Random perturbation of root priors; in this implementation it happens **after** simulations. |
| Candidate guard | A decision-local check of hypothetical moves for verified opponent winning proofs. |
| Refuted | A candidate after which the opponent has a proof passing winner, legal-first-move, and line verification checks. |
| Unrefuted | No accepted opponent proof was found within the work bounds; **unknown**, not safe. |
| `SearchStats` | Real statistics extracted from a searched tree; never fabricated for a proof. |
| `SearchInfo` | The CLI's search record: player statistics plus the effective seat configuration. |
| RNG | Random-number generator, owned by the session and borrowed by a decision. |
| Opening schedule | The session's first-ten-plies temperature/noise override for empty-root AI-versus-AI lines. |
| `[experiment]` | A project tuning choice, not a theorem or published strength/latency guarantee. |
| TDD | Test-driven development: write a failing behavioral test, make it pass, then improve structure without changing behavior. |

## Context

Sub-chapters 01 through 07 built the baseline evaluator: a policy readout
with a trust ladder, a saturating value readout, and an `[experiment]`
quiet-position whisper. Those readouts answer one question per call:
"what does this position look like right now?" They are fast enough to
call thousands of times per move, and the MCTS search calls them at every
leaf.

Two gaps remain at the decision level. First, Gomoku tactics are
sequences: a VCF win is a chain of forcing moves that the static maps
cannot see. The engine already has a soundness-first threat-space search
(TSS) prover [2], `engine::prove_forced_win`, whose `Some` is a certified
win and whose `None` means only "not proven within budget". Second, the CLI's step path
constructs a fresh evaluator for every move. The baseline evaluator
initializes a pattern network and a gravity kernel at construction;
rebuilding it per move is waste, and it makes the reusable question —
"which move should this evaluator play from this board?" — inseparable
from session mechanics.

This sub-chapter closes both gaps above the search crate, in a new
`player` crate. The retired design folded the root probe into the
evaluator seam (`Evaluator::evaluate_root` returning `RootEval`, and a
`SearchResult` enum from `mcts::search`). That design is superseded. On
main, `mcts::search` returns `SearchOutcome { tree, root }`, the
`Evaluator` trait has only `evaluate`, and the name
`SearchInfo::from_search` is kept as is. This chapter changes neither:
certification moves up into the player, which calls the engine prover
directly and verifies claims with `engine::verify_line`.

The verified reference for this chapter is the sibling worktree
`rust-ml-unified08-ref`, detached at main HEAD `2b6ec75`. Its source is
deliberately **uncommitted**: it is a reference to read and verify
against, not landed learner source. Links below point at that worktree;
paths are files to create or change in the learner's `gomoku/` workspace.
Main need not contain those artifacts yet.

## Intention

At the study table, a user steps an AI seat repeatedly, changes its
simulation count, explores variations, takes the seat back, or releases
it to the AI again. Those actions must keep their existing command
behavior while reusing the seat's evaluator. Taking a seat releases that
runtime resource; starting a new session position restores the initial
seats.

On top of that preserved behavior, a baseline seat gains two kinds of
tactical sight, both certified by the engine rather than estimated:

1. When the side to move has a forced win within the prover's work
   bounds, the seat plays the first move of the verified line directly,
   without running MCTS. When no proof is certified — including when the
   budget runs out — the seat searches normally. Unknown is not "no
   threat"; it is just unknown.
2. After ordinary move selection, the seat refuses a proposal when the
   engine verifies an opponent winning continuation after it and another
   candidate is available. A candidate without a verified opponent proof
   is unrefuted: unknown, not certified safe.

The study table must continue to show what the search actually measured.
A certified own win carries no invented visits or value. A guarded move
keeps the statistics of the real search that produced the original
proposal.

There are two working checkpoints:

1. **Own win and honest fallback, wired into the CLI.** The persistent
   player certifies own forced wins and searches everything else, with
   the guard disabled; the session holds one player per AI seat, so this
   checkpoint is already playable at the study table.
2. **Candidate guard.** Opt-in at the reusable API, then enabled by CLI
   policy for baseline seats only; refutations are certified,
   acceptances are not.

Neither checkpoint adds asynchronous execution, threads, an evaluator
hook, search-tree reuse, or changes to the engine or search crates.

## Artifacts and contracts

| Workspace file | Named artifact and contract |
|----------------|-----------------------------|
| [Cargo.toml][workspace-manifest] | Add `crates/player` to workspace members. |
| [crates/player/Cargo.toml][player-manifest] | Dependencies are `engine`, `mcts`, and `rand` (`0.10.3` in the reference); not Burn, `baseline`, `patterns`, or `cli`. |
| [crates/player/src/lib.rs][player-lib] | `AiPlayer` owns `Box<dyn Evaluator>`; `new` takes ownership, `decide` borrows a board, per-move options, and mutable RNG. One private admission helper, `certified_own_move`. Public items satisfy `#![deny(missing_docs)]`. |
| [crates/player/src/guard.rs][guard-src] | Private module: an implementation detail of the player crate, not a second architectural authority. It contains candidate resolution (`choose`, private to the crate) and defines the publicly re-exported `GuardConfig`, `GuardDiagnostics`, `GuardOutcome`. |
| [crates/player/tests/decisions.rs][decision-tests] | Public-interface decision, persistence, and own-probe tests. |
| [crates/player/tests/guard.rs][guard-tests] | Public-interface guard tests with the real engine prover. |
| [crates/cli/Cargo.toml][cli-manifest] | Add the path dependency on `player`. |
| [crates/cli/src/session.rs][session-src] | `SessionCore` retains optional Black/White players beside the occupants and owns their lifecycle, effective options, history, RNG, and events. |
| [crates/cli/src/actor.rs][actor-src] | `SearchInfo::from_stats` is new: it attaches the effective `AiConfig` to player statistics. `SearchInfo::from_search` stays unchanged for the manual comparison path. |
| [crates/cli/tests/actor.rs][actor-tests] | Seeded comparison with the pre-extraction move path, across the opening schedule boundary; two real-baseline acceptance tests. |

The public decision contract in the [player library][player-lib] is:

- `AiPlayer::new(evaluator: Box<dyn Evaluator>) -> Self`.
- `AiPlayer::decide(&mut self, board: &Board, options: &PlayerOptions,
  rng: &mut impl Rng) -> Result<Decision, DecisionError>`.
- `PlayerOptions`: `simulations`, `c_puct`, `temperature`,
  `noise_epsilon`, `noise_alpha`, `own: OwnTssConfig`,
  `guard: GuardConfig`. Defaults derive simulation count and exploration
  constant from `SearchConfig::default`; temperature and noise fraction
  are zero, noise concentration is `0.3`, and both probes are disabled.
- `OwnTssConfig`: `enabled` and `budget: SearchBudget`, with the
  `baseline()` preset (enabled; 10,000 nodes / 30 plies — the retired
  evaluator-side root budget, relocated to the player) and `disabled()`,
  which keeps the baseline budget bounds. `Default` is disabled.
- `Decision`: `mv`, `search: Option<SearchStats>`,
  `guard: GuardDiagnostics`. `search` is `None` exactly when a certified
  own win bypassed search.
- `SearchStats`: `side_to_move`, `visits`, `prior`, `value`; no proof
  flag and no manufactured proof distribution.
- `DecisionError::GameOver`: an already won or drawn board is rejected
  before evaluation or RNG consumption. This is not the own-proof result.
- `GuardConfig`: `enabled`, `max_candidates`, `budget: SearchBudget`.
  `baseline()` enables five candidate checks with 1,000 nodes / ten plies
  per probe. Both its default and `PlayerOptions::default().guard` are
  disabled.
- `GuardDiagnostics`: `proposal`, ordered `refuted` moves, `checked`,
  `outcome`. Outcomes are `Disabled`, `ProvenWin`, `Unrefuted`,
  `Terminal`, `BudgetExhausted`, and `AllRefuted`. `ProvenWin` now means
  "own TSS certified at decision time" — the player certified it, not an
  evaluator hook.

For ordinary searches, visits include **every** root edge, even
zero-visit moves, sorted by descending visits then ascending move index.
Priors include all root edges after optional noise, sorted by descending
prior then ascending index. Value is the sum of root edge accumulated
values divided by total root visits, or zero when there are no visits.
Its perspective is the side to move on the searched board, not the side
to move after playing the decision.

## Specification as executable tests — write these first

The following names are executable targets in the linked reference test
files. In the learner's workspace, create each group before implementing
the behavior it specifies. Supply only enough declarations to compile,
run the named target to see the missing behavior fail, and then implement
it. Do not write the whole player before returning to these tests.

### Milestone 1: ordinary decisions through the public player

Create [crates/player/tests/decisions.rs][decision-tests]. These four
tests establish the data surface, the preserved selection path, and
evaluator persistence; their fixtures are small enough to isolate the
contracts.

| Test | Setup and required assertions |
|------|-------------------------------|
| `terminal_board_returns_game_over_without_evaluation_or_rng` | Replay a won board. `NeverEvaluate` panics if called. Expect `GameOver`; compare the next draw with an untouched equally seeded RNG: zero evaluator calls, zero RNG draws. |
| `ordinary_decisions_match_manual_search_stats_and_seeded_selection` | On a board containing the center stone, compare a player with a manual `search` → optional noise → visit selection path using separate equal-seed RNGs and uniform evaluators of value `0.6`. Cover `(simulations, temperature, noise fraction)` of `(17, 0, 0)`, `(19, 1, 0)`, `(23, 0.7, 0.25)`, `(0, 1, 0.25)` with exploration constant `1.7` and noise concentration `0.3`. Assert the same legal move, sorted visits and priors, root value, visit sum equal to simulations, and next RNG draw. |
| `evaluator_instance_persists_across_two_decisions` | `StatefulEvaluator` keeps a local call counter observed through shared test state (`Rc<RefCell<Vec<u32>>>`). With one simulation per decision, decide, play, then decide again. Observations must be `[1, 2, 3, 4]`, not a restarted counter. |
| `own_tss_config_defaults_and_presets` | `PlayerOptions::default().own` equals `OwnTssConfig::disabled()`; `baseline()` is enabled with 10,000 nodes / 30 plies; `disabled()` preserves that budget; `GuardConfig::default()` is disabled. |

`StdRng::seed_from_u64` and comparing the *next* draw test stream
consumption, not just coincidentally equal moves. Keep assertions on
public decisions and statistics.

### Milestone 2: the own proof probe

Stay in [crates/player/tests/decisions.rs][decision-tests], plus one unit
test beside the admission helper in [crates/player/src/lib.rs][player-lib].
All five decision-level fixtures use the win-in-3 VCF position from the
engine prover tests: replay `(7,4) (7,3) (7,5) (0,0) (7,6) (0,2) (6,6)
(0,4) (8,8) (0,6)`; Black to move forces a win starting with `(7,7)`.

| Test | Setup and required assertions |
|------|-------------------------------|
| `own_proof_bypasses_search_stats_and_rng` | VCF fixture, `NeverEvaluate`, own probe at `baseline()`, with noise, temperature, and guard deliberately enabled. (The guard's options field arrives in milestone 4; write the test without it and extend the options when the field lands.) Assert move `(7,7)`, `search.is_none()`, `ProvenWin` diagnostics with zero checks and no refutations, and an unchanged RNG stream: zero evaluator calls, zero RNG draws. |
| `white_own_proof_bypasses_search_and_certifies_absolute_color` | A legal alternating White-to-move mirror of the VCF fixture (six Black stones, five White). The real engine proof must name White as winner with first move `(7,7)`; the decision bypasses search and RNG. This pins absolute-color correctness: the certificate names a color, not "whoever was searched". |
| `disabled_own_probe_searches_vcf_position` | Same fixture, own probe disabled, counting evaluator. The decision really searches: evaluator called, real statistics, visit sum equal to simulations. The gate is real, not a finite-value substitute. |
| `zero_budget_own_probe_falls_through_to_search` | Own probe enabled with `max_nodes: 0`. The real engine returns `None`; unknown falls through to a real searched decision. An exhausted budget is unknown, never "no threat". |
| `proven_decision_does_not_short_circuit_a_later_quiet_root` | Counting evaluator. Decide on the VCF fixture: certified, zero evaluate calls. Then decide a separate quiet board with the same player: evaluator calls above zero, real statistics, and the move is not the stale certified move. This pins the dynamic half of "no stored proof"; the structural half is that the player holds no state besides the evaluator. |
| `admission_rejects_wrong_winner_empty_line_illegal_first_move_and_unverified_line` | Unit test in the library's own test module, using fabricated `Proof` values against the VCF fixture. Wrong winner, empty line, and illegal first move each reject; the genuine engine proof admits `(7,7)`; the same proof truncated or appended to fails `verify_line`. The player re-checks every claim; a false claim means ordinary search, not an error. |

### Milestone 3: CLI integration

Wire the session before the guard exists: checkpoint one ends playable.
Add four tests to the private test module in
[crates/cli/src/session.rs][session-src], where player slots can be
replaced by cheap evaluators, and the seeded comparison in
[crates/cli/tests/actor.rs][actor-tests]. The per-decision options carry
no guard field in this milestone, and the CLI policy maps only the own
probe; the guard's vocabulary types (`GuardConfig`, `GuardDiagnostics`,
`GuardOutcome`) exist from milestone 1 so decisions can carry
diagnostics, but no checking runs.

| Test | Required behavior |
|------|-------------------|
| `persistent_player_reuses_evaluator_across_steps_and_set_sims` | One-simulation step makes two evaluator calls. Go back, set Black to three simulations, step again: cumulative calls become six and visits/config show three. No evaluator drop on `SetSims`; taking the seat drops it once. |
| `persistent_players_follow_init_release_take_and_reset` | Initial AI seats have players; humans do not. Releasing White retains Black's evaluator. Setting both simulation counts retains both. `New` drops both injected evaluators and restores initial occupants and counts. Taking both clears both players; releasing both constructs both. |
| `persistent_player_proof_plays_without_search_info_or_rng_draws` | Replay the VCF script into a session, inject `NeverEvaluate` under the baseline kind. `Step` plays `(7,7)` with the normal AI move event, stores no search record at the new cursor, and consumes no RNG. Historical records elsewhere in the tree are unaffected. |
| `baseline_enables_own_tss_probe_but_tactics_searches` | The same counting evaluator under each kind on the VCF fixture: the Baseline policy certifies before search (zero evaluate calls, no search record); the Tactics policy really searches and stores the record. |
| `persistent_players_match_seeded_legacy_search_through_opening_schedule` | Compare eleven plies of two tactics seats against a manual old path that constructs a fresh evaluator each time and builds its records with the unchanged `SearchInfo::from_search`. Seed `42`, three simulations. Plies zero through nine use the opening schedule; the eleventh uses configured defaults. Assert events, board, and full `SearchInfo` per step. |

With these green, checkpoint one is playable: release a baseline seat
against yourself; on a VCF position it plays the certified move without
a search record, and everywhere else it searches exactly as before.

### Milestone 4: the candidate guard

Create [crates/player/tests/guard.rs][guard-tests]. Most fixtures use
zero simulations: ordinary temperature-zero selection then chooses the
lowest-index legal move, while a deliberately high prior makes an
alternative rank first after refutation. **Pitfall guard:** every guard
test keeps `own` disabled via `PlayerOptions::default()`; several
fixtures contain own immediate wins that an enabled own probe would
certify before the guard runs.

| Test | Required assertion |
|------|--------------------|
| `baseline_guard_is_opt_in_with_fixed_experiment_work_bounds` | Default player options disable the guard; `baseline()` enables five candidate checks with `1_000` nodes and ten plies per probe. |
| `real_proof_refutes_proposal_and_guard_finds_unvisited_block_without_mutating_board_or_stats` | White has a single winning endpoint at `(7,7)`; Black's initial `(0,0)` proposal permits a real verified opponent proof. With the guard, choose the unvisited block, record one refutation and two checks, and keep board, Zobrist key, and search statistics unchanged. Outcome is `Unrefuted`, not safe. |
| `two_opponent_winning_threats_require_our_immediate_winning_alternative` | Refute the initial proposal against White's two-ended four; accept Black's immediate winning alternative as `Terminal`. |
| `blocking_one_of_two_distinct_opponent_lines_is_refuted_in_favor_of_own_immediate_win` | Independently establish separate horizontal and vertical White wins. Blocking one still permits a real verified proof on the other; retry Black's own immediate win. Assert `Terminal`, ordered refutation, two checks, unchanged statistics, next RNG draw, board, and Zobrist key. |
| `white_player_guard_probes_actual_black_opponent` | Mirror the responsibility, not just the coordinates: White to move must probe Black after hypothetical play and select the block. |
| `zero_depth_probe_is_unknown_even_with_opponent_immediate_win` | A zero-depth prover budget leaves the proposal `Unrefuted` with one check and no recorded refutation, even though the fixture permits an opponent win. |
| `zero_node_probe_is_unknown_even_with_opponent_immediate_win` | The same unknown contract holds for zero nodes. |
| `cap_falls_back_to_unchecked_block_not_refuted_original` | With a one-candidate cap, refute `(0,0)`, choose the highest-ranked remaining **untested** move `(7,7)`, and report `BudgetExhausted` with one check. |
| `zero_candidate_cap_keeps_original_unchecked` | Keep the proposal with zero checks and `BudgetExhausted`. |
| `every_legal_move_really_refuted_reports_all_refuted_and_original_fallback` | Give the cap room for every legal move on a two-ended opponent-four fixture. Recheck each recorded refutation with the real prover and verifier. Only after all legal candidates are refuted, retain the original legal proposal and report `AllRefuted` — not a root loss certificate. |
| `own_immediate_win_is_terminal_not_opponent_probe_unknown` | Even with a zero prover budget, accept an immediate own win as `Terminal`, with one check and no opponent probe. |
| `enabled_unknown_preserves_seeded_first_candidate_noise_stats_and_rng` | With zero-node probes, compare enabled and disabled guards at zero and eleven simulations, temperature `0.7`, noise fraction `0.25`, seed `42`. The same proposal is retained, statistics and next RNG draw match, and the board is unchanged. |
| `enabled_own_probe_precedes_guard_and_search` | VCF fixture, own probe **and** guard at `baseline()`, `NeverEvaluate`. The own probe certifies first: `ProvenWin`, zero guard checks, no search, no evaluator call. This pins the pipeline order. |

For the single-threat fixture, Black stones are `(7,2)`, `(2,0)`,
`(2,2)`, `(2,4)`; White stones are `(7,3)` through `(7,6)`; Black moves
next. The `Prefer` test evaluator raises one policy logit and offers
nothing else; the tests exercise the guard, not the own probe.

Then write five private tests beside `choose` in
[crates/player/src/guard.rs][guard-src]. Its injected probe callback
allows precise observations that an external test cannot obtain:

| Test | Required assertion |
|------|--------------------|
| `alternatives_rank_visits_then_prior_then_index_without_repeats_and_include_unvisited` | Reverse edge creation order. Check proposal first, then descending visits, descending prior, ascending index; each once. Five refutations must lead to the next ranked, unvisited, untested move. Assert correct opponent and budget forwarding; board and edges stay equal. |
| `malformed_or_unverified_proofs_leave_candidate_unknown` | Wrong winner, empty line, occupied/illegal first move, and an unverified line cannot refute the proposal. Expect one probe and `Unrefuted`. |
| `disabled_and_zero_cap_never_call_probe` | Inject a callback that panics. Disabled guard and enabled zero cap must return the original proposal without calling it, with their distinct outcomes. |
| `immediate_win_never_calls_opponent_probe` | An immediate own win returns `Terminal` after one hypothetical check without invoking the callback. |
| `immediate_draw_never_calls_opponent_probe` | Replay a valid 224-ply non-winning board, accept its last move as `Terminal`, and never invoke the callback. Keep the input board unchanged. |

The draw fixture replays stones rather than importing them with
`Board::from_position`: imported positions do not populate the move
history used by subsequent `Board::play` draw bookkeeping.

With the guard green through the public player, finish the CLI wiring:
one session test pins the guard policy mapping, and two real-Burn actor
tests validate both directions end to end.

| Test | Required behavior |
|------|-------------------|
| `baseline_enables_defensive_guard_but_tactics_preserves_proposal` | Inject a fixed block-preferring evaluator under each kind on the single-threat fixture. Baseline policy selects the unrefuted block; tactics policy retains the proposal. Both retain real zero-visit search information. This tests CLI policy mapping without running the Burn baseline. |
| `baseline_ai_plays_proven_vcf_without_search_info` | Real Burn baseline on the VCF fixture: the certified move is played, no search record is stored. Release-mode, full-QA scope. |
| `baseline_ai_falls_back_to_search_info_on_quiet_position` | Real Burn baseline on a quiet position: ordinary search stores its record. Release-mode, full-QA scope. |

Checkpoint two is playable as well: on the single-threat geometry a
baseline seat blocks instead of playing the naive corner proposal, while
a tactics seat keeps its proposal.

## Walkthrough — one step in execution order

The walkthrough follows one `step` command through the running program,
from the session down into the player and back. The build order is the
reverse: construct and test the player crate standalone, wire the
session as soon as the player decides moves (milestones 1–3, ending in
the playable checkpoint one), and add the candidate guard last
(milestone 4, checkpoint two).

### 1. The step begins in the session

`SessionCore::execute_step` in [crates/cli/src/session.rs][session-src]
keeps its existing checks in order: reject an opening ceremony in
progress, reject a finished board, and require the side-to-move seat to
be AI. It clones that seat's `AiConfig` into an effective configuration.
The existing opening schedule may replace temperature with `1.0` and
noise fraction with `0.25` for the first ten plies of an eligible
empty-root AI-versus-AI line; explicit selection settings disable it.

The effective settings become `PlayerOptions`. This is also where the CLI
attaches its probe policy: evaluator kind `Baseline` maps to
`OwnTssConfig::baseline()` and `GuardConfig::baseline()`; kind `Tactics`
maps to both disabled. The mapping lives here, not in the player: the
reusable crate never inspects the evaluator's concrete type, and a
library caller opts out by passing disabled options.

To decide, the session must own a player for that seat. Add
`black_player` and `white_player` slots — `Option<AiPlayer>` beside the
serializable `Occupant` values, never inside `Occupant` or `AiConfig`:
those configuration types keep their clone, equality, and serialization
behavior, and a boxed evaluator is runtime state, not a configuration
value. `make_player(&Occupant)` maps an AI occupant to a newly owned
evaluator through the existing `make_evaluator`, and a human occupant to
`None`. `set_occupant` becomes the single replacement operation:
construction creates initial players, releasing a seat constructs its
player, taking a seat drops it, and `reset` replaces both from the
initial occupants while keeping the same RNG stream. `execute_set_sims`
and history or puzzle navigation change configuration, not player
identity.

The step borrows the seat's player mutably and passes `&mut self.rng`.
These borrows are design choices, each visible in the contract: the
player is `&mut self` because one retained evaluator serves every
decision of the seat and may keep mutable state; the board is `&Board`
because the player never commits session moves; the RNG is borrowed
because the session owns one seeded stream and reproducibility spans the
whole session, not one decision. A missing player after the AI-occupant
check is an internal invariant violation, so the reference uses `expect`,
not a user-facing error.

### 2. The player rejects a finished board

At the start of `AiPlayer::decide` in [crates/player/src/lib.rs][player-lib],
inspect `board.status()`. A won or drawn input returns
`DecisionError::GameOver` before anything else: zero evaluator calls,
zero RNG draws. The CLI already checks this, but a reusable API cannot
rely on its caller being the CLI. The error maps back to the existing
`CommandError::GameOver` at the session boundary.

### 3. The own probe asks the engine for a certified win

For an ongoing board with `options.own.enabled`, the player calls
`engine::prove_forced_win(board, board.to_move(), options.own.budget)`
once. The probe works directly on the session's existing `Board`, in
absolute colors. It never touches the evaluator's side-to-move-relative
planes, so there is nothing to reconstruct: the board is already the
prover's native input.

The player does not trust the prover's claim. The private admission
helper `certified_own_move` re-checks four conditions: the proof's
winner is the side to move, the line is non-empty, its first move is
legal on this board, and `engine::verify_line` accepts the line. Any failure rejects the
claim, and the decision falls through to ordinary search silently. A
rejected claim is unknown, exactly like `None` from the prover: not "no
threat exists", just nothing certified within the work bounds. The budget
caps prover nodes and plies; it is not a wall-clock deadline, and
`verify_line`'s work is separate from it.

An admitted proof returns immediately: the first move of the line,
`search: None`, and `GuardDiagnostics` with outcome `ProvenWin`, zero
checks, and no refutations. No evaluator call, no RNG draw, no guard
check, and no fabricated statistics. Note that the certified move is the
**start** of a forcing line: the VCF fixture certifies a win three plies
away, so the game continues after the move is played.

The probe runs once per `decide` and its result is never stored. This is
a structural property, not a measured one: there is a single
unconditional call site, and `AiPlayer` holds no state besides the
evaluator, so there is nothing that could cache a proof. The quiet-root
test in milestone 2 pins the dynamic consequence — a later quiet board is
really searched — which excludes a stale short-circuit; it does not count
probe calls.

### 4. Ordinary search runs once

Without a certified win, construct `SearchConfig` from `simulations` and
`c_puct` and call `mcts::search(board, &mut *self.evaluator, &config)`
once. The evaluator seam is unchanged: the `Evaluator` trait still has
only `evaluate`, and the search returns `SearchOutcome { tree, root }`.
The tree is decision-local; persistence means retaining the evaluator,
not the tree.

### 5. Noise perturbs root priors after search

When `noise_epsilon` is positive, call `mcts::add_dirichlet_noise` on the
root **after** the search. The mechanism is familiar from AlphaGo Zero
[1], but the timing here is a preserved local behavior, not that paper's
pre-search noise policy. Noise changes root **priors** after simulations;
it does not change existing visits or accumulated values, and therefore
does not change the visit-derived weights that temperature selection is
about to use. It does consume RNG before sampling, so enabling or
disabling it shifts a seeded stream and can change a seeded proposal. The
post-noise priors are also what the statistics record and what the
guard's retry ranking tie-breaks on.

### 6. Temperature selection proposes a move

`mcts::visit_distribution` extracts the root visits, and
`mcts::select_move` applies the requested temperature with the borrowed
RNG: temperature zero selects the largest visit count with lowest-index
tie-breaking; positive temperature samples visit-derived weights. This
move is the **proposal** — the ordinary selection the guard may still
refuse. An empty distribution for an ongoing board is an internal
invariant violation and panics.

### 7. Working checkpoint one: certified wins and honest fallback

With the guard vocabulary defined but checking not yet implemented,
checkpoint one is complete and playable in the CLI: terminal rejection,
certified own wins without search, the preserved ordinary path with real
statistics, and the session wiring — persistent player slots, lifecycle,
per-step options with `from_stats`, and the own-probe policy mapping.
The milestone-1, milestone-2, and milestone-3 suites pin this state.
Ordinary decisions report `Disabled` diagnostics; the data surface
(`GuardOutcome` variants and `GuardDiagnostics`) already exists so the
next checkpoint adds behavior, not types.

### 8. The candidate guard checks hypothetical moves

Checkpoint two implements `choose` in the private
[guard module][guard-src]. The module is an implementation detail of the
player crate: `AiPlayer` remains the single structure responsible for the
decision, and only the guard's data types are re-exported. `choose`
receives the board, the proposal, the root edges (read-only), the guard
configuration, and a probe callback. The production call supplies
`engine::prove_forced_win`; private tests supply closures to observe
calls or return malformed proofs. That `impl FnMut` seam is test
equipment, not an evaluator hook.

Disabled mode returns the proposal unchecked with outcome `Disabled`; an
enabled zero cap returns it unchecked with `BudgetExhausted`. With work
available, candidates are the proposal first, then the remaining root
moves ranked by descending visits, descending prior (incomparable values
treated as equal), and ascending move index — including unvisited moves.
Retries never resample and never consume RNG.

Each checked candidate is played on a clone of the board; the input board
is never mutated. Increment `checked` per hypothetical play. An immediate
own win or a draw is accepted as `Terminal` without probing. Otherwise
the probe runs for `after.to_move()` — the actual opponent on that
hypothetical board. A proof refutes the candidate only if its winner is
that opponent, its first move is legal on the resulting board, and
`engine::verify_line` accepts the line. Record a valid refutation and
continue. `None` or any rejected proof returns the candidate as
`Unrefuted`: unknown, never certified safe.

The cap is checked before playing the next candidate. If all previous
checks were refuted and the cap is reached, return the next ranked
candidate untested with `BudgetExhausted` — the highest-ranked unchecked
move, not the already-refuted proposal. Only when iteration exhausts
**every legal candidate** does the guard report `AllRefuted` and retain
the original legal proposal, asserting the refutation count equals the
legal-move count. That diagnostic is a list of individual refutations,
not a composed proof that the root position is lost, and no grounds for
resignation.

### 9. Working checkpoint two: refutation, not safety

The milestone-4 suites pin the guard: real-prover refutation through the
public player, deterministic retry order, malformed-proof handling, the
no-probe branches through the private seam, and the CLI guard-policy
mapping. The guard changes which
move is returned only after a verified refutation; it never changes root
edges, visits, priors, or values.

### 10. Statistics describe the real search

On every searched decision, the private `SearchStats::from_searched`
extracts the statistics from the real tree: all root edges, visits sorted
by descending count then ascending index, priors (after optional noise)
sorted by descending probability then ascending index, value as the sum
of accumulated values over total visits with an explicit zero-visit
branch, and the searched board's side-to-move perspective. This runs
regardless of what the guard decided: when the guard substitutes a
different move, the statistics still describe the original proposal's
search. The record is a measurement, not an endorsement of the returned
move.

### 11. The session commits and notifies

Back in `execute_step`, a decision with `search: None` — a certified own
win — goes through the existing `execute_play` path: the move is
committed and the normal events are produced, and **no** search record is
stored at the fresh cursor. Records stored earlier at other cursors are
historical information and stay untouched.

A searched decision is wrapped by the new `SearchInfo::from_stats` in
[crates/cli/src/actor.rs][actor-src], which attaches the **effective**
configuration to the player's statistics. The existing branch is
preserved: `history.play`, record storage at the new cursor, the AI move
event, a branch event when the move created siblings, and a terminal
event when the game ended. `SearchInfo::from_search` is unchanged; the
seeded eleven-ply comparison test uses it to reproduce the pre-extraction
manual path. Guard diagnostics are returned by the player but are neither
stored nor rendered by this CLI integration.

## TDD checklist

Work one test at a time, red to green.

- [ ] Milestone 1: terminal rejection, manual-path equivalence including
  zero simulations and RNG state, evaluator persistence, and the
  configuration presets.
- [ ] Milestone 2: own-proof bypass for both colors, disabled-gate and
  zero-budget fall-through, the quiet-root no-stale-proof test, and the
  four admission rejections plus the genuine-proof acceptance.
- [ ] Milestone 3: the four session tests (persistence, lifecycle, proof
  step, own-probe policy mapping), then the eleven-ply seeded
  comparison, all red to green against the CLI wiring with no guard in
  the options yet.
- [ ] **Checkpoint one gate:** `cargo test -p player` and
  `cargo test -p cli` green with the guard absent; the session keeps one
  player per AI seat, certified wins bypass search, everything else is
  honestly searched, and the CLI is playable.
- [ ] Milestone 4: all twelve ported public guard tests (with `own`
  disabled), the precedence test, and the five private seam tests
  including the replay-built draw; then the guard policy-mapping session
  test and the two real-baseline actor validations.
- [ ] **Checkpoint two gate:** `cargo test -p player` green with the
  guard enabled; verified refutations move the choice, unknowns keep it,
  and the baseline CLI policy substitutes the block.
- [ ] Review each refactor only after its tests are green; do not repair
  failures by changing the expected move path or inventing proof
  statistics.

## Verification commands and evidence

Run these from the learner's `gomoku/` after implementation; from the
sibling reference's `gomoku/` they inspect the reference instead. Do not
confuse a reference result with source having landed on main.

```text
cargo test -p player
cargo test -p mcts
cargo test -p cli --lib
cargo test -p cli --test session
cargo test -p cli --test actor -- --skip baseline_ai_plays_proven_vcf_without_search_info --skip baseline_ai_falls_back_to_search_info_on_quiet_position
```

The two skipped tests use the real Burn baseline and belong to release
full-QA scope. The untouched-proof gates matter as much as the tests:

```text
git status --porcelain gomoku/crates/mcts gomoku/crates/baseline gomoku/crates/engine
rg 'RootEval|evaluate_root|SearchResult' gomoku/crates/mcts gomoku/crates/baseline gomoku/crates/cli gomoku/crates/engine
```

Both must be empty: the search, baseline, and engine crates are
byte-untouched, and no root-hook vocabulary exists anywhere.

Fresh QA in the sibling reference's `gomoku/` (worktree
`rust-ml-unified08-ref`, detached at main HEAD `2b6ec75`, source
uncommitted, recorded 2026-11-06) reports the full gates green:

| Command | Result |
|---------|--------|
| `cargo test --release --locked --all-targets` | Exit 0; **563 passed, 0 failed, 5 ignored**. The ignored five are pre-existing measurement and full-proof tests (three patterns differentials, the quiet-whisper A/B, sims-versus-strength). |
| `cargo clippy --locked --all-targets -- -D warnings` | Exit 0. |
| `cargo fmt --all -- --check` | Exit 0. |
| `git diff --check` | Exit 0. |

The new tests number 36: six library unit tests (one admission, five
private guard), nine public decision tests, thirteen public guard tests,
five CLI session tests, and three CLI actor tests. Reference verification
does not mean learner source has landed on main.

**Historical provenance, kept distinct.** The legacy 08b paper quotes a
574-test workspace run. That count belongs to the retired hook-era
reference and does not transfer; 563/0/5 is this chapter's fresh number.
Likewise, the strength-ladder measurements in
[sub-chapter 09][acceptance] were recorded against earlier proof paths.
This chapter changes the proof path again and claims **no** re-measurement:
the ladder numbers keep their old provenance labels, and re-running them
is future work, not part of this milestone.

## Capstone

Take the defender seat and watch the baseline seat execute a forcing line
without searching:

```text
cargo run -q -p cli --bin gomoku -- play --puzzle ../data/puzzles/kaibao-vcf-676.json --index 0 --ai baseline --sims 50
```

The puzzle attacker is to move; you sit opposite the AI. Step once. The
baseline seat's own probe certifies the forced win and the seat plays the
first move of the verified line directly — no search record appears for
that move. Step a few more times and watch the forcing line unfold.

Then try an index deeper in the same pack:

```text
cargo run -q -p cli --bin gomoku -- play --puzzle ../data/puzzles/kaibao-vcf-676.json --index 12 --ai baseline --sims 50
```

Some positions solve in one move; some need a short sequence; some
positions yield no proof within the bounds, and the seat falls back to
ordinary search with its usual record. The difference between the two
behaviors is the own probe opening and closing.

## Done when

- Both checkpoints verify separately, each playable in the CLI: with
  the guard absent or disabled, certified own wins bypass search and
  ordinary decisions match the manual path; with the guard enabled,
  verified refutations move the choice and unknowns keep it.
- One evaluator is retained per AI seat across moves and `SetSims`;
  take, release, and reset follow the specified resource lifetime.
- Terminal inputs and certified own wins consume no evaluator calls and
  no RNG draws; proof decisions carry no fabricated statistics and store
  no search record at the fresh cursor.
- Ordinary decisions retain full real statistics even when the guard
  substitutes the move; the statistics describe the original proposal's
  search.
- An exhausted prover budget is treated as unknown and falls through to
  search; unrefuted candidates are never described as safe; `AllRefuted`
  is never advertised as a root loss certificate; cap exhaustion chooses
  the highest-ranked untested alternative.
- Probe policy is a CLI mapping per evaluator kind; library defaults keep
  both probes disabled, and the new crate has no Burn dependency.
- The `mcts`, `baseline`, and `engine` crates are byte-untouched and the
  root-hook grep gate is empty.
- The unfiltered tests and repository gates above have actually passed on
  the implementation being accepted; record any remaining exclusions
  explicitly.
- The worktree is committed with:
  `feat(baseline): the AI player — baseline sub-chapter 08`.

## References

- [The AI player: design paper][design] — the conceptual narrative,
  diagrams, catalogs, and invariants for this sub-chapter.
- [The AI player: implementation plan][implementation] — the per-artifact
  construction contracts, for learners who do not read the solution.
- [The AI player: solution][solution] — the opt-in, fully annotated
  reference walkthrough.
- [Baseline primer][primer] — the tutorial's design authority; its
  player-level probe section matches this chapter.
- [Sub-chapter 07 — The quiet whisper][chapter07] — the previous
  sub-chapter; the evaluator this chapter retains per seat.
- [Sub-chapter 09 — Acceptance][acceptance] — where measured strength
  numbers live; this chapter claims no re-measurement.
- Legacy history, superseded: [08 — The TSS eye][legacy-08] and
  [08b — The AI player][legacy-08b]. The hook design and the split
  chapter are kept for the record only.
- Verified reference (uncommitted, detached at `2b6ec75`):
  [workspace manifest][workspace-manifest],
  [player manifest][player-manifest], [player library][player-lib],
  [guard module][guard-src], [decision tests][decision-tests],
  [guard tests][guard-tests], [CLI manifest][cli-manifest],
  [CLI session][session-src], [actor types][actor-src],
  [actor integration tests][actor-tests].
- Main source (authoritative, pre-change): [engine prover and
  verifier][tss-main], [MCTS move policy][policy-main],
  [evaluator seam][eval-main].
- [1] Silver et al. (2017), [Mastering the game of Go without human
  knowledge](https://doi.org/10.1038/nature24270): root noise and
  temperature selection. The post-search noise timing preserved here is a
  local implementation fact, not a claim about that paper's algorithm.
- [2] Allis (1994), [Searching for Solutions in Games and Artificial
  Intelligence](https://doi.org/10.26481/dis.19940923la): threat-space
  search. The probe budgets and acceptance policy here are local
  experiments.

[design]: ./the-ai-player-design.md
[implementation]: ./the-ai-player-implementation.md
[solution]: ./the-ai-player-solution.md
[primer]: ../00-baseline-primer.md
[chapter07]: ../07-the-quiet-whisper/the-quiet-whisper.md
[acceptance]: ../09-acceptance/acceptance.md
[legacy-08]: ../08-the-tss-eye/the-tss-eye.md
[legacy-08b]: ../08b-the-ai-player/ai-player.md
[workspace-manifest]: ../../../../../rust-ml-unified08-ref/gomoku/Cargo.toml
[player-manifest]: ../../../../../rust-ml-unified08-ref/gomoku/crates/player/Cargo.toml
[player-lib]: ../../../../../rust-ml-unified08-ref/gomoku/crates/player/src/lib.rs
[guard-src]: ../../../../../rust-ml-unified08-ref/gomoku/crates/player/src/guard.rs
[decision-tests]: ../../../../../rust-ml-unified08-ref/gomoku/crates/player/tests/decisions.rs
[guard-tests]: ../../../../../rust-ml-unified08-ref/gomoku/crates/player/tests/guard.rs
[cli-manifest]: ../../../../../rust-ml-unified08-ref/gomoku/crates/cli/Cargo.toml
[session-src]: ../../../../../rust-ml-unified08-ref/gomoku/crates/cli/src/session.rs
[actor-src]: ../../../../../rust-ml-unified08-ref/gomoku/crates/cli/src/actor.rs
[actor-tests]: ../../../../../rust-ml-unified08-ref/gomoku/crates/cli/tests/actor.rs
[tss-main]: ../../../../gomoku/crates/engine/src/tss.rs
[policy-main]: ../../../../gomoku/crates/mcts/src/policy.rs
[eval-main]: ../../../../gomoku/crates/mcts/src/eval.rs
