> **Opt-in complete solution — read after attempting the chapter, or after being stuck for more than twenty minutes.**
>
> This paper quotes a verified reference build that is preserved as an
> **uncommitted detached worktree** beside the main repository:
> `/Users/wgiersche/workspace/Project-Ellie/rust-ml-unified08-ref`
> (base commit `2b6ec75`; the `player` crate and the CLI changes exist
> only as working-tree files there — no branch, no commit). The
> learner's own implementation on `main` is the reference for behavior;
> this paper is inspiration, not authority. Line numbers are the
> reference worktree's, not main's. No learner source is changed by
> this paper.

# Sub-chapter 08 — The AI player: complete solution

## Abstract

This paper is the full, annotated reference solution for baseline
sub-chapter 08, *the AI player*. One new crate, `player`, owns a
persistent evaluator per AI seat and answers move-decision requests.
Its `decide` pipeline runs, in order: a terminal check, an optional
own-side threat-space-search (TSS) proof probe with a four-part
admission predicate, one ordinary Monte Carlo tree search (MCTS),
optional post-search Dirichlet noise, temperature selection of a
proposal, an optional opponent-refutation guard, and honest search
statistics extracted from the real tree. The command-line interface
(CLI) session keeps one `AiPlayer` per AI seat across steps and routes
certified wins to the board without storing search statistics. The
`engine`, `mcts`, and `baseline` crates are byte-untouched; the retired
evaluator root hook (`RootEval`, `evaluate_root`, `SearchResult`) does
not exist in this design. The paper quotes every new source fragment
and every new test body with real line numbers, explains each fragment
where execution touches it, and closes with the necessary-versus-optional
analysis. Read it after the [chapter](the-ai-player.md); the
[implementation paper](the-ai-player-implementation.md) is the
spoiler-free layer, and the [design paper](the-ai-player-design.md)
explains why the design has this shape.

## Glossary

| Term | Meaning in this solution |
|------|--------------------------|
| AI seat | Black or White configured to produce moves automatically. |
| CLI | Command-line interface; its session owns history, events, configuration, and randomness. |
| MCTS | Monte Carlo tree search; simulations accumulate root-edge visits and values. |
| Evaluator | The `mcts::Evaluator` trait: one method, `evaluate`, producing a policy/value estimate. It has no proof hook in this design. |
| TSS | Threat-space search, the engine's bounded prover of forcing winning continuations (`engine::prove_forced_win`). |
| VCF | Victory by continuous four — a forcing sequence of threat moves; the win-in-3 fixture used throughout the tests. |
| Own probe | The player's pre-search call to `engine::prove_forced_win` for the side to move, configured by `OwnTssConfig`. |
| Admission | The predicate that accepts or rejects a claimed proof: correct winner, non-empty line, legal first move, verified line. |
| Certificate | An admitted proof; only its first move is ever played. |
| Proposal | The ordinary temperature-selected move, before defensive checking. |
| Guard | The optional post-selection opponent-refutation experiment (`guard::choose`), configured by `GuardConfig`. |
| Refuted | A candidate permits an opponent proof with the correct winner, legal first move, and verified line. |
| Unrefuted | No accepted opponent proof within the bounds: **unknown**, never certified safe. |
| Candidate cap | Maximum hypothetical candidate checks (`GuardConfig::max_candidates`); distinct from each probe's node/depth budget. |
| Budget | `engine::SearchBudget`: prover work caps in nodes and plies, **not** a wall-clock deadline. |
| Prior | Policy probability on a root edge, not its visit count. |
| RNG | Random-number generator; the session owns it and lends it to a decision. |
| Legacy packs | The retired sub-chapter 08 ("the TSS eye", root hook) and 08b ("the AI player" on top of the hook) document folders, kept on disk for history. |
| `[experiment]` | A local tuning choice, not a theorem, strength measurement, or latency guarantee. |

**Name disambiguation.** Three similarly named constructors exist and
must not be confused: `SearchInfo::from_search` (pre-existing CLI
constructor from an `mcts::SearchOutcome`, **unchanged**),
`SearchInfo::from_stats` (**new** CLI constructor from
`player::SearchStats`), and `SearchStats::from_searched` (**new**,
*private* player-crate extractor from a searched tree). The word
"probe" names two calls to the same engine function: the *own probe*
(before search, for the side to move) and the *guard probe* (after
selection, for the opponent on a cloned board).

## Provenance, starting point, and verification

All numbered source quotations are machine-extracted from
`rust-ml-unified08-ref/gomoku` (detached at `2b6ec75`, changes
uncommitted). The comparison starting point is main `2b6ec75`:
`engine`, `mcts`, and `baseline` are byte-identical to it
(`git status --porcelain` for those crates is empty in the reference
worktree), and the learner's main source at chapter 07 needs no
preparatory modification outside this chapter's own listed edits (the
artifact map below names them: the session and actor sources and the
two manifests). This paper is self-contained: it quotes every
new fragment and does not require the reference worktree to remain
available for copying. For the unchanged algorithms (the PUCT search,
noise, visit extraction, move selection, the engine prover and
verifier) it links the existing tutorial chapters instead of reprinting
them.

**Fresh reference gates** (run from `rust-ml-unified08-ref/gomoku`,
2026-11-06, recorded in the slice evidence):

| Command | Exit | Result |
|---------|------|--------|
| `cargo test --release --locked --all-targets` | 0 | **563 passed, 0 failed, 5 ignored** across 35 suites |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | zero warnings |
| `cargo fmt --all -- --check` | 0 | clean |
| `git diff --check` | 0 | clean |
| `rg 'RootEval\|evaluate_root\|SearchResult' crates/{mcts,baseline,cli,engine,player}` | 1 | no matches — the retired hook vocabulary is absent |
| `git status --porcelain crates/mcts crates/baseline crates/engine` | — | empty (byte-untouched) |

The five ignored tests are pre-existing measurement or full-proof
tests (three patterns differential tests, `quiet_whisper_ab_measured`,
`sims_vs_strength_measured`), not new skips. The test count is stated
fresh: older papers cite 574 from the retired hook design; that count
does not transfer because the hook-era tests are gone and the
player/CLI tests are new. No strength or latency re-measurement is
claimed for this redesign; the acceptance chapter's ladder numbers keep
their old provenance labels.

## Quick execution route (runtime order)

What actually happens when the AI makes a move, in the order it
happens. This is a call chain, not a list of implementation tasks.

1. `cli::SessionCore::execute_step` refuses to run while the Swap2
   ceremony is unfinished, the game is over, or a human occupies the
   seat; it resolves the seat's effective `AiConfig` (an active opening
   schedule forces temperature 1.0 and noise 0.25). All unchanged.
2. `execute_step` builds a `player::PlayerOptions`, mapping the seat's
   evaluator kind to the TSS policy: `Baseline` enables the own probe
   and the guard with baseline budgets; `Tactics` disables both.
3. `execute_step` borrows the seat's persistent `player::AiPlayer`
   (constructed when the seat was configured, reused ever since) and
   calls `decide(&board, &options, &mut self.rng)`.
4. `AiPlayer::decide` first checks `board.status()`: a finished game
   returns `Err(DecisionError::GameOver)` without calling the evaluator
   or touching the RNG.
5. If `options.own.enabled`, `decide` calls
   `engine::prove_forced_win(board, board.to_move(), budget)` once and
   runs the admission predicate `certified_own_move`. An admitted
   certificate returns its first move immediately: no evaluator call,
   no RNG draw, no search statistics, **no tree allocated** — the
   return happens before `mcts::search` is reached. A missing or
   rejected proof means *unknown*, and execution falls through to
   ordinary search.
6. `mcts::search` runs once with the persistent evaluator and returns
   the unchanged `SearchOutcome { tree, root }`.
7. If `noise_epsilon > 0.0`, `mcts::add_dirichlet_noise` re-weights the
   stored root priors and advances the shared RNG. Visit counts — the
   selection weights — are not touched.
8. `mcts::visit_distribution` plus `mcts::select_move` produce the
   proposal.
9. `guard::choose` (enabled guard only) checks the proposal first, then
   ranked alternatives, on cloned boards: an immediate own win or draw
   is `Terminal`; otherwise the opponent is probed and a verified
   opponent win refutes the candidate. An unrefuted candidate is
   *unknown*, never certified safe.
10. `SearchStats::from_searched` reads the real searched tree — also
    when the guard substituted the move.
11. Back in `execute_step`: a proof decision (`search: None`) goes
    straight to `execute_play` and stores **no new** `SearchInfo` at
    the fresh cursor (historical records elsewhere in the tree are
    untouched). A searched decision builds `SearchInfo::from_stats`,
    plays through `history.play`, inserts the record at the new cursor,
    and emits the move (and any branch/game-over) events.

Player lifecycle (stepped through in §3): players are constructed in
`SessionCore::new`, replaced together with their occupant by
`set_occupant` on release, take, and reset, and never rebuilt by
`set_sims` or navigation.

## Specification as tests

Thirty-six new tests pin the behavior. The roster below is the
executable specification, ordered along the execution route; every
body is quoted in full in the [Tests](#tests) part, grouped by file,
with shared fixture helpers quoted once per file and their line ranges
stated. Names are the contract; assertion details are at the quoted
bodies.

**Player decisions** — `crates/player/tests/decisions.rs` (9):

| # | Test | Pins |
|---|------|------|
| D1 | `terminal_board_returns_game_over_without_evaluation_or_rng` | Step 4: error before evaluation; zero RNG draws (ported from legacy 08b). |
| D2 | `ordinary_decisions_match_manual_search_stats_and_seeded_selection` | Steps 6–8, 10: player path equals the manual search→noise→select path for four option tuples; seeded RNG parity (ported). |
| D3 | `evaluator_instance_persists_across_two_decisions` | The evaluator object is reused across decisions (ported). |
| D4 | `white_own_proof_bypasses_search_and_certifies_absolute_color` | Step 5 with White to move on a legal mirrored fixture: the certificate names White, bypasses everything. |
| D5 | `own_tss_config_defaults_and_presets` | `OwnTssConfig`/`GuardConfig` presets and defaults. |
| D6 | `own_proof_bypasses_search_stats_and_rng` | Step 5: certified move, `search: None`, zero guard checks, zero RNG draws — with noise, temperature, and guard all deliberately enabled. |
| D7 | `disabled_own_probe_searches_vcf_position` | The opt-in gate is real: disabled probe on a proven position still searches. |
| D8 | `zero_budget_own_probe_falls_through_to_search` | An exhausted budget is unknown, not a loss; search runs. |
| D9 | `proven_decision_does_not_short_circuit_a_later_quiet_root` | A certified decision does not suppress a later, separate quiet search by the same player. |

**Admission predicate** — `crates/player/src/lib.rs` unit test (1):

| # | Test | Pins |
|---|------|------|
| U1 | `admission_rejects_wrong_winner_empty_line_illegal_first_move_and_unverified_line` | Each of the four admission rules rejects; a real engine proof admits; doctored lines fail `verify_line`. |

**Guard, public** — `crates/player/tests/guard.rs` (13; the twelve
ported legacy-08b tests all keep the own probe disabled so the guard is
observed in isolation; only G13 enables both):

| # | Test | Pins |
|---|------|------|
| G1 | `baseline_guard_is_opt_in_with_fixed_experiment_work_bounds` | Guard defaults disabled; baseline bounds are 5 candidates, 1,000 nodes / 10 plies (ported). |
| G2 | `real_proof_refutes_proposal_and_guard_finds_unvisited_block_without_mutating_board_or_stats` | Real engine proof refutes the proposal; the unvisited block is found; board and statistics untouched (ported). |
| G3 | `two_opponent_winning_threats_require_our_immediate_winning_alternative` | A proposal leaving two opponent wins is refuted; an immediate own win ends the check as `Terminal` (ported). |
| G4 | `blocking_one_of_two_distinct_opponent_lines_is_refuted_in_favor_of_own_immediate_win` | Blocking one of two *distinct* lines is still refuted; seeded RNG and statistics parity with the disabled guard (ported). |
| G5 | `white_player_guard_probes_actual_black_opponent` | The guard probes the actual opponent color, White seat probing Black (ported). |
| G6 | `zero_depth_probe_is_unknown_even_with_opponent_immediate_win` | A zero-depth probe cannot certify; candidate stays unknown (ported). |
| G7 | `zero_node_probe_is_unknown_even_with_opponent_immediate_win` | A zero-node probe cannot certify; candidate stays unknown (ported). |
| G8 | `cap_falls_back_to_unchecked_block_not_refuted_original` | An exhausted cap falls to the highest-ranked **unchecked** candidate, never back to a refuted one (ported). |
| G9 | `zero_candidate_cap_keeps_original_unchecked` | Cap zero checks nothing and keeps the proposal (ported). |
| G10 | `every_legal_move_really_refuted_reports_all_refuted_and_original_fallback` | `AllRefuted` with every legal move individually verified-refuted; original proposal retained — not a loss certificate (ported). |
| G11 | `own_immediate_win_is_terminal_not_opponent_probe_unknown` | Immediate own win short-circuits before any probe, even with a zero budget (ported). |
| G12 | `enabled_unknown_preserves_seeded_first_candidate_noise_stats_and_rng` | Enabled guard, unknown outcome: same move, same stats, same RNG stream as the disabled guard (ported). |
| G13 | `enabled_own_probe_precedes_guard_and_search` | Pipeline order: own certification beats both guard and search (new). |

**Guard, private** — `crates/player/src/guard.rs` unit tests (5,
ported; they use the `pub(crate)` probe-injection seam):

| # | Test | Pins |
|---|------|------|
| P1 | `alternatives_rank_visits_then_prior_then_index_without_repeats_and_include_unvisited` | Retry order: visits desc, prior desc, index asc; unvisited edges included; inputs unmutated. |
| P2 | `malformed_or_unverified_proofs_leave_candidate_unknown` | Five malformed proof shapes each leave the candidate unknown after exactly one probe. |
| P3 | `disabled_and_zero_cap_never_call_probe` | No probe calls when disabled or capped at zero (panicking probe). |
| P4 | `immediate_win_never_calls_opponent_probe` | Terminal fastpath before any probe. |
| P5 | `immediate_draw_never_calls_opponent_probe` | Same for a 224-ply replayed draw fixture (never `Board::from_position` for the draw). |

**CLI session** — `crates/cli/src/session.rs` unit tests (5):

| # | Test | Pins |
|---|------|------|
| S1 | `persistent_player_reuses_evaluator_across_steps_and_set_sims` | One evaluator serves repeated steps; `SetSims` mutates config without dropping the evaluator; `Take` drops it. |
| S2 | `persistent_players_follow_init_release_take_and_reset` | Player slots track occupant lifecycle exactly: init, release, take, reset. |
| S3 | `persistent_player_proof_plays_without_search_info_or_rng_draws` | A certified step through the session: certified move played, no record at the fresh cursor, no RNG draws. |
| S4 | `baseline_enables_own_tss_probe_but_tactics_searches` | The policy map: Baseline certifies without evaluation; Tactics really searches and stores the record (new). |
| S5 | `baseline_enables_defensive_guard_but_tactics_preserves_proposal` | The policy map for the guard: Baseline substitutes the block; Tactics keeps the proposal (ported). |

**CLI actor** — `crates/cli/tests/actor.rs` (3):

| # | Test | Pins |
|---|------|------|
| C1 | `persistent_players_match_seeded_legacy_search_through_opening_schedule` | Eleven seeded plies, two tactics seats: persistent players reproduce the pre-player manual path exactly — events, board, and full `SearchInfo` per ply. |
| C2 | `baseline_ai_plays_proven_vcf_without_search_info` | Real Burn baseline: proven VCF plays the forcing move and stores no record at the fresh cursor (carried over, now exercised through the player). |
| C3 | `baseline_ai_falls_back_to_search_info_on_quiet_position` | Real Burn baseline: quiet root searches and stores the record (carried over). |

## Retired root-hook tests and their successors

The legacy-08 root hook does not exist in this design, so its tests are
retired, not ported. This table maps each retired name to the
player-level successor that now pins the behavior (or states why none
is needed). It is a navigational aid, not a spec.

| Retired test (legacy 08 packs) | Successor in this chapter |
|--------------------------------|---------------------------|
| `default_evaluate_root_delegates_to_evaluate_once` (mcts) | None needed: the `Evaluator` trait has exactly one method again; D2 pins the ordinary path end to end. |
| `root_estimate_is_not_evaluated_twice` (mcts) | None needed: root evaluation lives entirely inside the unchanged `mcts::search`. |
| `terminal_root_does_not_call_evaluator` (mcts) | D1 (`terminal_board_returns_game_over_without_evaluation_or_rng`) — at the player level, before search exists. |
| `proven_root_returns_immediately_without_tree_or_leaf_eval` (mcts) | D6 (`own_proof_bypasses_search_stats_and_rng`) — stronger: no tree is even allocated. |
| `repeated_searches_invoke_root_hook_each_time` (mcts) | D9 (`proven_decision_does_not_short_circuit_a_later_quiet_root`). |
| `root_tss_budget_default_is_10k_30`, `with_tss_budget_overrides_default` (baseline) | D5 (`own_tss_config_defaults_and_presets`) — the budget moved to `OwnTssConfig`. |
| `evaluate_root_delegates_to_estimate_on_quiet_board` (baseline) | D7 (`disabled_own_probe_searches_vcf_position`) and D2. |
| `evaluate_root_finds_verified_vcf_proof` (baseline) | D6 plus U1 (admission). |
| `evaluate_root_falls_back_when_budget_is_exhausted` (baseline) | D8 (`zero_budget_own_probe_falls_through_to_search`). |
| `evaluate_does_not_run_tss` (baseline) | Structural: the evaluator interface contains no TSS call at all; the probe lives in the player, so there is nothing to forward or forget. |
| `repeated_evaluate_root_probes_each_root` (baseline) | D9. |
| `decorated_baseline_forwards_evaluate_root_and_bypasses_leaf_eval` (baseline acceptance) | Structural: with no hook, the decorator trap it guarded against cannot exist; S1/C1 pin evaluator reuse. |
| `root_proof_has_no_search_stats_leaf_calls_or_rng_consumption` (legacy 08b player test) | D6 — same behavior, now proven with the real engine prover instead of a hook-injected certificate. |

## Artifact map: new, ported, untouched

| Artifact | Status |
|----------|--------|
| `gomoku/Cargo.toml` workspace members | one-line edit (add `crates/player`) |
| `gomoku/crates/player/Cargo.toml` | new (deps: `engine`, `mcts`, `rand` — nothing else) |
| `gomoku/crates/player/src/lib.rs` | new (`AiPlayer`, `PlayerOptions`, `OwnTssConfig`, `Decision`, `DecisionError`, `certified_own_move`); `SearchStats`/`from_searched` ported verbatim from legacy 08b |
| `gomoku/crates/player/src/guard.rs` | ported verbatim from legacy 08b (module doc wording aside); `GuardOutcome::ProvenWin` re-documented as "certified own forced win", now produced by the own probe rather than an evaluator hook |
| `gomoku/crates/cli/src/session.rs` | edited: player slots, lifecycle routing, options/policy map, decide call, `from_stats` path |
| `gomoku/crates/cli/src/actor.rs` | edited: `from_stats` added; `from_search` byte-untouched |
| `gomoku/crates/cli/Cargo.toml` | one-line edit (add the `player` dependency) |
| `engine`, `mcts`, `baseline` crates | **untouched** — porcelain-empty in the reference worktree |
| `mcts::search`, `add_dirichlet_noise`, `visit_distribution`, `select_move`, `SearchOutcome`, `Evaluator` | unchanged; see the [mcts tutorial](../../mcts-tutorial/00-mcts-primer.md) chapters linked where execution reaches them |
| `engine::prove_forced_win`, `engine::verify_line`, `Proof`, `SearchBudget` | unchanged; see the [engine TSS chapter](../../13-engine-tutorial/08-threat-space-search.md) |
## 1. Workspace wiring

Before any behavior exists, the workspace must know the new crate.
Execution starts here only in the build sense: `cargo` resolves the
member list, then each crate's manifest.

`gomoku/Cargo.toml`, lines 4–7 — the members list gains exactly one
entry:

```text
   4 | [workspace]
   5 | resolver = "3"
   6 | members = ["crates/engine", "crates/cli", "crates/mcts", "crates/train", "crates/patterns",
   7 |     "crates/testkit", "crates/baseline", "crates/player"]
```

**COMMENT — the members line.**

- Why: a path dependency (`cli` depending on `player`) only resolves
  inside the workspace if the target is a member; without this entry,
  `cargo` rejects the build before any code is type-checked.
- Rust mechanics: resolver `"3"` is stated explicitly because this is
  a *virtual* workspace — without the line, Cargo would fall back to
  resolver `"1"` and warn (only edition-2024 packages default to
  `"3"`); the line is load-bearing, not a restated default. Adding a
  member does not touch the
  shared `[workspace.package]` metadata that `player` inherits below.
- Necessary versus optional: the member entry is necessary; its
  position at the end of the list is cosmetic.

`gomoku/crates/player/Cargo.toml`, lines 1–10 — the whole manifest of
the new crate:

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

**COMMENT — the player manifest.**

- Why three dependencies and no more: `engine` supplies `Board`,
  `Move`, `Color`, `Status`, and the TSS prover/verifier; `mcts`
  supplies `Evaluator`, `search`, and the tree readers; `rand` supplies
  the `Rng` bound for temperature sampling and noise. Everything else
  the pipeline needs already rides on one of those three. In
  particular there is no Burn dependency: the player speaks only the
  `Evaluator` trait, so the Burn-backed baseline stays behind the
  `cli` crate's factory (`make_evaluator`, §3) and this crate compiles
  with Burn nowhere in sight — the same dependency-island discipline
  the `engine` crate already follows.
- `version.workspace = true` (and edition/license likewise) inherit the
  workspace package table, keeping the eight workspace members on one
  version line; this is the established convention in every crate of this
  workspace, copied here for consistency, not necessity.
- Necessary versus optional: the three dependencies are necessary; the
  `workspace = true` inheritance is the house convention (a literal
  version would compile identically).

`gomoku/crates/cli/Cargo.toml`, lines 25–31 — the CLI gains the edge:

```text
  25 | [dependencies]
  26 | engine = { path = "../engine" }
  27 | mcts = { path = "../mcts" }
  28 | player = { path = "../player" }
  29 | thiserror.workspace = true
  30 | clap = { workspace = true, features = ["derive" ]}
  31 | rand = "0.10.3"
```

**COMMENT — the cli dependency line.** One line, placed with the other
path dependencies. The dependency direction is deliberate: `cli` depends
on `player`, never the reverse. The session (a UI concern) owns seats,
history, and RNG; the player (a UI-independent concern) owns
evaluators and decisions. A reverse edge would pull session types into
the decision layer and make the player untestable without the CLI.

`gomoku/crates/player/src/lib.rs`, lines 1–15 — the crate root's own
documentation, lint level, imports, and module wiring:

```text
   1 | //! Reusable AI move decisions, independent of any user interface.
   2 | //!
   3 | //! An [`AiPlayer`] retains its evaluator across decisions. Each decision
   4 | //! runs a fresh MCTS search; certified own forced wins bypass sampling
   5 | //! and carry no fabricated search statistics.
   6 | 
   7 | #![deny(missing_docs)]
   8 | 
   9 | use engine::{Board, SearchBudget, Status};
  10 | use mcts::{Evaluator, SearchConfig};
  11 | use rand::Rng;
  12 | 
  13 | mod guard;
  14 | pub use guard::{GuardConfig, GuardDiagnostics, GuardOutcome};
  15 | 
```

**COMMENT — the crate head.** The crate doc states the two facts a
reader needs before any type: the evaluator persists across decisions,
and certified wins bypass sampling *without fabricated statistics*.
`#![deny(missing_docs)]` turns undocumented public items into build
errors — the crate's public surface is small enough that full
documentation is cheap, and the deny keeps it that way. The import
list is the dependency list made concrete: `engine` for the board and
budget vocabulary, `mcts` for the evaluator and search configuration,
`rand` for the RNG bound. `mod guard;` is private; only the three
guard *types* are re-exported, so `choose` and `GuardDiagnostics::new`
stay crate-internal — the public can read diagnostics but cannot run
or fabricate the experiment.

## 2. `execute_step`: unchanged preconditions

Execution of an AI move starts in
`gomoku/crates/cli/src/session.rs`. Lines 417–428 are the pre-existing
entrance ceremony — quoted here because the new code slots in
immediately after it, and because two of its checks matter for
understanding what `decide` can assume:

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
```

**COMMENT — ceremony and game-over refusals (unchanged).**

- The Swap2 refusal (lines 418–424) is load-bearing for the player:
  ceremony stones are not game positions, and a decision computed now
  would be discarded when the ceremony resets the tree. Because of
  this check, `decide` never sees a ceremony board.
- The game-over check (lines 426–428) duplicates the terminal check
  inside `decide` (§6). The duplication is deliberate belt-and-braces:
  the CLI needs `CommandError::GameOver` for *any* caller, while the
  player needs `DecisionError::GameOver` for *any* embedding (a future
  non-CLI driver may lack this guard). Neither check is redundant for
  its own layer.

Lines 429–441 — the seat check and the effective configuration, also
unchanged:

```text
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
```

**COMMENT — seat check and opening schedule (unchanged).**

- The seat check (lines 430–435) guarantees that the `expect` on the
  player slot in §4 can only fire through an internal inconsistency,
  never through a human seat: a human seat returns early here, before
  the slot is borrowed.
- The effective-config block (lines 437–441) applies the opening
  schedule — during the first plies of an AI-versus-AI game from an
  empty root, temperature 1.0 and noise 0.25 are forced for variety.
  This is pre-existing behavior; the only change is that the resulting
  values now flow into `PlayerOptions` instead of a bare
  `SearchConfig` plus inline noise calls.

## 3. Where the player lives: slots and lifecycle

Step 3 of the execution route borrows `&mut self.black_player` (or the
white slot). Those fields exist from session construction onward, so
this section covers setup exactly where execution first depends on it.

`session.rs`, lines 265–273 — two new fields on `SessionCore`, next to
the occupants:

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
```

**COMMENT — the player slots.**

- Why slots beside the occupants: an `AiPlayer` owns a
  `Box<dyn Evaluator>`, and the Burn-backed baseline evaluator is
  expensive to construct (model record loading, backend setup). The
  pre-player CLI rebuilt the evaluator on every step; the slot makes
  the evaluator a per-seat resource that survives steps. That
  persistence is also what a stateful evaluator (a cache, a backend
  with warmup) needs to be useful at all.
- Why `Option`: a human seat has no player. `None` is the honest
  representation; a dummy player for human seats would waste a model
  load and invite stepping bugs.
- Why *not* inside `Occupant`: `Occupant` is clonable, serializable
  seat *configuration* (it is stored in `SessionInit`, cloned into
  `AiConfig`, compared in tests). A trait object is neither clonable
  nor serializable. The in-code comment at line 270 states exactly
  this. Keeping the two side by side — config in `Occupant`, runtime
  object in the slot — is what lets `reset` restore configuration while
  reconstructing runtime state.
- Rust mechanics: `Option<player::AiPlayer>` is a plain owned field;
  no `RefCell`, no `Rc`. All mutation flows through `&mut self`
  methods, so the borrow checker serializes access for free.

`session.rs`, lines 296–302 — construction in `SessionCore::new`:

```text
 296 |         Self {
 297 |             history: History::from_opening(&init.opening),
 298 |             black: init.black.clone(),
 299 |             white: init.white.clone(),
 300 |             black_player: Self::make_player(&init.black),
 301 |             white_player: Self::make_player(&init.white),
 302 |             default_config: init.default_config.clone(),
```

**COMMENT — construction at birth.** Players are built from the same
`init` occupants that configure the seats, in the same order, so a
fresh session is internally consistent: wherever `black` is
`Occupant::Ai(_)`, `black_player` is `Some`. The invariant "AI seat ⇔
`Some` player" is established here and maintained by exactly one
function, below.

`session.rs`, lines 784–794 — the factory and its unchanged sibling:

```text
 784 |         // Keep the same RNG; reset does not reseed.
 785 |     }
 786 | 
 787 |     fn make_player(occupant: &Occupant) -> Option<player::AiPlayer> {
 788 |         match occupant {
 789 |             Occupant::Ai(config) => Some(player::AiPlayer::new(Self::make_evaluator(
 790 |                 &config.evaluator,
 791 |             ))),
 792 |             Occupant::Human => None,
 793 |         }
 794 |     }
```

**COMMENT — `make_player` and `make_evaluator`.**

- `make_player` is a pure function of the occupant: AI config in,
  boxed player out; human in, `None` out. It is an associated
  function (`Self::make_player`), not a method — it needs no session
  state, and calling it from `new` (where `self` does not fully exist
  yet) would otherwise be impossible.
- `make_evaluator` is the pre-existing factory that maps
  `EvaluatorKind` to a boxed `dyn mcts::Evaluator` (the Burn baseline
  or the tactics evaluator); it is unchanged. The player crate never
  learns which kind it holds — that is the whole point of the trait
  object boundary.
- Rust mechanics: `Box<dyn mcts::Evaluator>` is moved straight into
  `AiPlayer::new`. The double indirection (`Option` → `AiPlayer` →
  `Box`) exists because each layer answers a different question: is
  there a player (Option), what is the decision owner (AiPlayer), what
  is the estimate source (the boxed trait object).

`session.rs`, lines 753–766 — the single replacement point:

```text
 753 |     fn set_occupant(&mut self, color: Color, occupant: Occupant) {
 754 |         let player = Self::make_player(&occupant);
 755 |         match color {
 756 |             Color::Black => {
 757 |                 self.black = occupant;
 758 |                 self.black_player = player;
 759 |             }
 760 |             Color::White => {
 761 |                 self.white = occupant;
 762 |                 self.white_player = player;
 763 |             }
 764 |         }
 765 |     }
 766 | 
```

**COMMENT — `set_occupant`.**

- Why replace both together: occupant and player are two halves of one
  seat. Replacing them in one function makes it impossible — by
  structure, not by discipline — to end up with an AI occupant whose
  player slot is `None`, or a human occupant with a live model behind
  it. Every lifecycle event (release a seat to the AI, take a seat
  back, reset the session) routes through this one function.
- Rust mechanics: assigning `self.black_player = player` drops the old
  `AiPlayer` (and with it the boxed evaluator, including any Burn
  model) at the assignment point. Test S1 observes this through a
  `Drop` counter.
- What deliberately does *not* go through here: `execute_set_sims`
  mutates the simulations field of the in-place `Occupant::Ai` config
  and touches no player, so changing the budget keeps the warmed-up
  evaluator. Navigation (`Back`, `Next`) likewise never rebuilds.

`session.rs`, lines 767–782 — reset goes through the same door:

```text
 767 |     fn reset(&mut self) {
 768 |         self.history = History::from_opening(&self.init.opening);
 769 |         self.set_occupant(Color::Black, self.init.black.clone());
 770 |         self.set_occupant(Color::White, self.init.white.clone());
 771 |         self.default_config = self.init.default_config.clone();
 772 |         self.black_sims = self.init.black_sims;
 773 |         self.white_sims = self.init.white_sims;
 774 |         self.search_info.clear();
 775 |         // `new` returns the session to its initial phase. A Swap2
 776 |         // session started in the ceremony, so it resets into the
 777 |         // ceremony.
 778 |         self.opening = if self.init.swap2 {
 779 |             Some(OpeningState::new())
 780 |         } else {
 781 |             None
 782 |         };
```

**COMMENT — `reset`.** Resetting reconstructs history and configuration
from `init`; by routing occupant restoration through `set_occupant`,
the player slots follow automatically, and the comment at the end
records the one thing reset must *not* do: reseed the RNG (seeded
sessions must stay reproducible across a reset). Before this change,
`reset` assigned the occupant fields directly; that direct assignment
would now leave stale players behind, which is why the detour through
`set_occupant` is necessary, not stylistic.

## 4. Options and the policy map

With the slot borrowed, `execute_step` builds the per-decision options.
`session.rs`, lines 442–457:

```text
 442 | 
 443 |         let options = player::PlayerOptions {
 444 |             simulations: effective_config.simulations,
 445 |             c_puct: effective_config.c_puct,
 446 |             temperature: effective_config.temperature,
 447 |             noise_epsilon: effective_config.noise_epsilon,
 448 |             noise_alpha: 0.3,
 449 |             own: match effective_config.evaluator {
 450 |                 EvaluatorKind::Baseline => player::OwnTssConfig::baseline(),
 451 |                 EvaluatorKind::Tactics => player::OwnTssConfig::disabled(),
 452 |             },
 453 |             guard: match effective_config.evaluator {
 454 |                 EvaluatorKind::Baseline => player::GuardConfig::baseline(),
 455 |                 EvaluatorKind::Tactics => player::GuardConfig::disabled(),
 456 |             },
 457 |         };
```

**COMMENT — the options literal and the policy map.**

- Why a struct literal with all seven fields: `PlayerOptions` is a
  plain data struct with no hidden defaults at the call site; the CLI
  wants to say everything explicitly. The alternative —
  `..PlayerOptions::default()` — would silently inherit future fields;
  spelling every field out means a new option fails to compile here
  until the CLI takes a stance on it. Exhaustiveness as a tripwire.
- The five search/selection fields map one-to-one from the effective
  config, so the CLI's pre-existing knobs (simulations, c_puct,
  temperature, noise) behave exactly as before. `noise_alpha` is the
  literal `0.3` — the same constant the pre-player CLI passed inline to
  `add_dirichlet_noise`; naming it in the options struct makes the
  constant visible and overridable without changing behavior.
- The two `match` blocks are the *policy map*: the seat's evaluator
  kind decides the TSS posture. `Baseline` — the strong, slow,
  Burn-backed player — gets both probes at baseline budgets; `Tactics`
  — the cheap built-in — gets neither. Why coupled to the evaluator
  kind: the probes exist to compensate for what a weak or expensive
  search misses (tactics the network cannot see, blunders the search
  cannot rank); the tactics evaluator already hand-weaves tactics into
  its policy, so the probes would duplicate its strength. This mapping
  is the [experiment] knob; tests S4 and S5 pin it from both sides.
- Rust mechanics: `effective_config.evaluator` is matched by value
  twice; `EvaluatorKind` is `Copy`, so the first match does not move it
  away from the second. Two small matches instead of one combined
  `(own, guard)` match are a readability choice — each field's rule
  reads independently.

The options type itself lives in the player crate.
`gomoku/crates/player/src/lib.rs`, lines 60–76:

```text
  60 | #[derive(Debug, Clone, Copy, PartialEq)]
  61 | pub struct PlayerOptions {
  62 |     /// Number of MCTS simulations.
  63 |     pub simulations: u32,
  64 |     /// PUCT exploration constant.
  65 |     pub c_puct: f32,
  66 |     /// Visit-count sampling temperature; zero selects the most-visited move.
  67 |     pub temperature: f32,
  68 |     /// Dirichlet noise mix fraction; zero disables noise and its RNG draws.
  69 |     pub noise_epsilon: f32,
  70 |     /// Positive Dirichlet concentration parameter when noise is enabled.
  71 |     pub noise_alpha: f32,
  72 |     /// Own-side TSS probe before search; disabled by default.
  73 |     pub own: OwnTssConfig,
  74 |     /// Optional [experiment] defensive refutation guard; disabled by default.
  75 |     pub guard: GuardConfig,
  76 | }
```

**COMMENT — `PlayerOptions`.**

- Why one struct for one decision: every input to `decide` that varies
  per call travels in this value, so `AiPlayer` itself holds no
  configuration. That is what makes the pipeline auditable: a decision
  is a pure function of `(board, options, rng)` plus the persistent
  evaluator. It also means a caller can vary options per move
  (the opening schedule does exactly this) without reconstructing the
  player.
- Why `Copy`: the struct is seven small fields; `Copy` lets tests and
  callers derive variants (`PlayerOptions { own: .., ..options }`)
  without clones. `Eq` is impossible — the struct contains `f32`
  fields, and floats have no `Eq` (NaN breaks reflexivity); `PartialEq`
  is all the tests need.
- `own` and `guard` are disabled in the default (lines 73–74): both
  probes are opt-in experiments, and the default decision is exactly
  the pre-chapter behavior. Test D5 pins this.

`lib.rs`, lines 78–91 — the default derives its search numbers from the
mcts crate's own default, not from copied literals:

```text
  78 | impl Default for PlayerOptions {
  79 |     fn default() -> Self {
  80 |         let search = SearchConfig::default();
  81 |         Self {
  82 |             simulations: search.simulations,
  83 |             c_puct: search.c_puct,
  84 |             temperature: 0.0,
  85 |             noise_epsilon: 0.0,
  86 |             noise_alpha: 0.3,
  87 |             own: OwnTssConfig::disabled(),
  88 |             guard: GuardConfig::disabled(),
  89 |         }
  90 |     }
  91 | }
```

**COMMENT — the default.** `SearchConfig::default()` (400 simulations,
c_puct 1.5) is read through so the player cannot drift from the mcts
crate's baseline. Temperature 0 selects the most-visited move; noise
epsilon 0 disables noise *and its RNG draws* (the doc comment states
this because the seeded tests depend on it); alpha 0.3 matches the
CLI's constant for when noise is enabled.

`lib.rs`, lines 17–57 — the own-probe configuration is new in this
chapter (the guard's `GuardConfig` arrives with the guard, §11):

```text
  17 | ///
  18 | /// When enabled, [`AiPlayer::decide`] asks the engine for a certified forced
  19 | /// win for the side to move before searching. The budget bounds prover work;
  20 | /// exhausting it means unknown, never loss, and ordinary search runs.
  21 | #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  22 | pub struct OwnTssConfig {
  23 |     /// Whether to run the own-side proof probe before search.
  24 |     pub enabled: bool,
  25 |     /// Prover work caps, not a wall-clock deadline.
  26 |     pub budget: SearchBudget,
  27 | }
```

```text
  29 | impl OwnTssConfig {
  30 |     /// Baseline preset: enabled, 10,000 nodes / 30 plies (the retired
  31 |     /// chapter-08 `ROOT_TSS_BUDGET` provenance, relocated to the player).
  32 |     #[must_use]
  33 |     pub const fn baseline() -> Self {
  34 |         Self {
  35 |             enabled: true,
  36 |             budget: SearchBudget {
  37 |                 max_nodes: 10_000,
  38 |                 max_depth: 30,
  39 |             },
  40 |         }
  41 |     }
  42 | 
  43 |     /// Disabled preset keeping the baseline budget bounds.
  44 |     #[must_use]
  45 |     pub const fn disabled() -> Self {
  46 |         Self {
  47 |             enabled: false,
  48 |             ..Self::baseline()
  49 |         }
  50 |     }
  51 | }
```

```text
  53 | impl Default for OwnTssConfig {
  54 |     fn default() -> Self {
  55 |         Self::disabled()
  56 |     }
  57 | }
```

**COMMENT — `OwnTssConfig`.**

- Why a separate struct instead of two loose fields on
  `PlayerOptions`: the probe is a unit — an on/off switch plus the
  work bounds that make "on" safe. Packaging them means the presets
  travel together and the default can say "disabled, but here are the
  bounds you would get if you enabled it".
- Why the budget moved here: in the legacy hook design the budget was
  a constant inside the baseline evaluator (`ROOT_TSS_BUDGET`, 10,000
  nodes / 30 plies). Certification authority now sits in the player, so
  the budget follows it — same numbers, new owner, recorded as the
  provenance comment on `baseline()` states. The numbers themselves are
  the [experiment] values from the engine chapter's prover tuning.
- Rust mechanics: `const fn` presets allow use in const contexts and
  cost nothing at runtime; `..Self::baseline()` in `disabled()`
  struct-update-copies the budget and flips only `enabled`, which is
  what "disabled preset keeping the baseline budget bounds" means —
  a later `enabled: true` flip needs no second edit. `Default` is
  implemented by hand (not `derive`) because the desired default is
  `disabled()`, not the field-wise zero value; `enabled: false` would
  coincide, but the budget fields would zero out, and a zero budget
  enabled later by accident would mean "probe with no work allowed" —
  always unknown. The hand implementation documents the choice.
- Necessary versus optional: the enabled flag and budget are
  necessary; the two named presets and the `const` qualifier are
  ergonomics.

## 5. The decision and its error

`execute_step` calls `decide` and matches on its result. The return
types first, then the call.

`lib.rs`, lines 93–102 — what a decision carries:

```text
  93 | /// A chosen move and, when a search ran, its real statistics.
  94 | #[derive(Debug, Clone, PartialEq)]
  95 | pub struct Decision {
  96 |     /// Move to play.
  97 |     pub mv: engine::Move,
  98 |     /// Search statistics, or `None` when a certified own win bypassed search.
  99 |     pub search: Option<SearchStats>,
 100 |     /// Defensive experiment diagnostics, independent of search statistics.
 101 |     pub guard: GuardDiagnostics,
 102 | }
```

**COMMENT — `Decision`.**

- Why `Option<SearchStats>`: a certified own win never ran a search, so
  there are no statistics to report. The alternatives were a fabricated
  zero-row `SearchStats` (dishonest — it would record a search that
  never happened) or a separate enum (heavier, and the CLI's two arms
  already express the split). `None` is the honest "no search ran".
- `guard` is always present, even when disabled: diagnostics are cheap
  and make every decision self-describing — a disabled guard says
  `Disabled` with the proposal recorded, rather than vanishing and
  leaving the reader to infer why no check happened.
- Rust mechanics: `PartialEq` is derived so tests can compare whole
  decisions; `Eq` is impossible (`SearchStats` contains `f32`).

`lib.rs`, lines 145–160 — the failure mode:

```text
 145 | /// Failure to decide a move.
 146 | #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 147 | pub enum DecisionError {
 148 |     /// The board is already won or drawn.
 149 |     GameOver,
 150 | }
 151 | 
 152 | impl std::fmt::Display for DecisionError {
 153 |     fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
 154 |         match self {
 155 |             Self::GameOver => formatter.write_str("the game is over"),
 156 |         }
 157 |     }
 158 | }
 159 | 
 160 | impl std::error::Error for DecisionError {}
```

**COMMENT — `DecisionError`.** One variant, by design: every
non-terminal path either produces a move or panics on a violated
internal invariant (the `# Panics` doc on `decide` lists the
noise/selection panics — Gamma rejection and the empty move
distribution; the guard's internal-invariant panics are documented in
§11), so
"the game is over" is the only recoverable failure. The hand-written
`Display` and `std::error::Error` impls — instead of a `thiserror`
derive — keep the crate's dependency list at exactly the three
manifest entries (§1). This is the deliberate exception: the engine
crate's error types use `thiserror` derives (`thiserror` is a shared
workspace dependency there), while `player` keeps its manifest at
three entries by writing the two impls by hand.

`lib.rs`, lines 163–190 — the player itself:

```text
 163 | pub struct AiPlayer {
 164 |     evaluator: Box<dyn Evaluator>,
 165 | }
```

```text
 183 | impl AiPlayer {
 184 |     /// Own an evaluator and reuse it for every subsequent decision.
 185 |     #[must_use]
 186 |     pub fn new(evaluator: Box<dyn Evaluator>) -> Self {
 187 |         Self { evaluator }
 188 |     }
 189 | 
 190 |     /// Decide a move without modifying `board`.
```

**COMMENT — `AiPlayer` and `new`.**

- The struct has exactly one field. This is the structural fact behind
  the "no caching" property: there is no field in which a proof, a
  board hash, or a tree could persist between decisions. The companion
  dynamic evidence is test D9 (§Tests).
- `Box<dyn Evaluator>` — a trait object, not a generic
  `AiPlayer<E: Evaluator>`: the CLI stores players for two different
  evaluator kinds in the same `Option<player::AiPlayer>` slot type
  (§3), which a generic parameter would make impossible without an
  enum of players. The dynamic dispatch cost is one indirection per
  evaluation call, negligible against the evaluation itself.
- `decide` takes `&mut self` (§6) because `Evaluator::evaluate` takes
  `&mut self` — the trait deliberately permits stateful evaluators
  (caches, backends), and the player must be able to offer that
  exclusive borrow. The player itself mutates nothing else; it owns no
  other state to mutate.
- `#[must_use]` on `new`: constructing a player and dropping it
  silently would waste a model load; the attribute turns that mistake
  into a warning. Optional ergonomics, zero runtime effect.

## 6. `decide`, step 1: the terminal check

`lib.rs`, lines 191–217 — the contract documentation and the first
thing the function does:

```text
 191 |     ///
 192 |     /// A certified own forced win returns immediately without consuming RNG,
 193 |     /// calling the evaluator, or constructing search statistics. Otherwise
 194 |     /// the order is search, optional root noise, visit extraction, then
 195 |     /// temperature selection. Noise is deliberately applied after search to
 196 |     /// preserve the existing CLI behavior. An enabled guard then checks the
 197 |     /// selected proposal and ranked alternatives on cloned boards; it may
 198 |     /// substitute the returned move, but the search statistics always describe
 199 |     /// the real proposal search, never the guard's hypothetical probes. An
 200 |     /// unrefuted candidate is unknown, not certified safe.
 201 |     ///
 202 |     /// # Errors
 203 |     /// Returns [`DecisionError::GameOver`] for a won or drawn board, without
 204 |     /// calling the evaluator or consuming RNG.
 205 |     ///
 206 |     /// # Panics
 207 |     /// Panics if Gamma rejects enabled noise's alpha, or if MCTS returns an
 208 |     /// empty move distribution for an ongoing board.
```

```text
 209 |     pub fn decide(
 210 |         &mut self,
 211 |         board: &Board,
 212 |         options: &PlayerOptions,
 213 |         rng: &mut impl Rng,
 214 |     ) -> Result<Decision, DecisionError> {
 215 |         if board.status() != Status::Ongoing {
 216 |             return Err(DecisionError::GameOver);
 217 |         }
```

**COMMENT — signature and terminal guard.**

- Why the check is here and not only in the CLI: `decide` promises a
  legal move or a `GameOver` error; a finished board has no legal
  moves, so the promise cannot be kept and must be refused. Keeping
  the refusal inside the player makes the guarantee caller-independent
  — the CLI's own earlier check (§2) protects the CLI, this one
  protects the abstraction.
- Why before everything: the own probe, the evaluator, and the RNG
  must all be able to assume an ongoing game. Test D1 pins the
  observable part: a panicking evaluator never fires, and the RNG
  stream is untouched (compared against a same-seed untouched stream).
- Rust mechanics: `rng: &mut impl Rng` is a generic parameter with a
  trait bound, monomorphized per caller; an alternative
  `&mut dyn rand::Rng` would also work but adds a vtable for no
  benefit here — the function is called from few sites. (`Rng` is the
  dyn-safe trait in the pinned rand 0.10; the old `RngCore` name is a
  deprecated stub that this workspace's `-D warnings` gate would
  reject.) `impl Rng` in
  argument position is the anonymous-generic spelling; it keeps the
  signature short while staying object-agnostic, so tests can pass a
  seeded `StdRng` and the CLI passes its own session RNG.
- `board: &Board` is a shared borrow: deciding must not modify the
  session board. Every board mutation in the pipeline happens on
  clones (§11) or inside `mcts::search` on its own tree.

## 7. `decide`, step 2: the own TSS probe and admission

This is the chapter's new capability. The legacy design put the proof
inside the evaluator behind a hook; here the player itself is the
certifier, calling the engine directly before any search exists.

The two engine functions the player relies on are unchanged from the
engine tutorial (`crates/engine/src/tss.rs`, lines 16–55 — quoted
because the admission predicate below is defined against exactly these
signatures):

```text
  16 | #[derive(Debug, Clone, PartialEq, Eq)]
  17 | pub struct Proof {
  18 |     /// The side that can force a win.
  19 |     pub winner: Color,
  20 |     /// The forcing sequence, starting with the attacker's first threat.
  21 |     pub line: Vec<Move>,
  22 | }
  23 | 
  24 | /// Hard caps. Exhausting the budget means "no proof" — never "loss".
  25 | #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  26 | pub struct SearchBudget {
  27 |     /// Maximum threat-space nodes to explore.
  28 |     pub max_nodes: u32,
  29 |     /// Maximum depth (plies) to search.
  30 |     pub max_depth: u8,
  31 | }
  32 | 
  33 | /// `Some` = certified forced win for `side` from `b`.
  34 | /// `None` = not proven within budget; says NOTHING about the true value.
  35 | pub fn prove_forced_win(b: &Board, side: Color, budget: SearchBudget) -> Option<Proof> {
  36 |     if b.status() != Status::Ongoing || b.to_move() != side {
  37 |         return None;
  38 |     }
  39 |     let mut b = b.clone();
  40 |     let mut search = Search::new(budget);
  41 |     search
  42 |         .solve(&mut b, side, budget.max_depth)
  43 |         .map(|line| Proof { winner: side, line })
  44 | }
  45 | 
  46 | /// The soundness net. Replays `proof.line` from `b`: every move legal,
  47 | /// alternating, forcing; at each defender turn, EVERY alternative that
  48 | /// addresses the threat is enumerated and the attacker's continuation
  49 | /// must still win (gapped fours give the defender two blocks). `true`
  50 | /// only if the proof is airtight.
  51 | pub fn verify_line(b: &Board, proof: &Proof) -> bool {
  52 |     if b.to_move() != proof.winner {
  53 |         return false;
  54 |     }
  55 |     verify_rec(b.clone(), proof.winner, &proof.line)
```

**COMMENT — the engine contract being consumed.**

- `prove_forced_win` already refuses non-ongoing boards and wrong-side
  requests (line 36) and already returns `None` on budget exhaustion —
  documented as "says NOTHING about the true value" (line 34). The
  player's policy "unknown, never loss" is inherited from the engine,
  not invented here.
- `verify_line` (lines 46–55) is the soundness net: it replays the
  line, enumerates every defender alternative at each defensive turn,
  and requires the attacker's continuation to still win. It also
  re-checks `to_move == winner` (line 52). The admission predicate
  below *adds* checks that look redundant against this verifier; they
  are defensive overlap, deliberate, and cheap (see the A9 discussion).
  Verification has no budget knob — it replays one concrete line, so
  its work is bounded by the line length, not by a search.
- Budgets are work caps — node and depth counters inside the prover —
  not wall-clock deadlines; nothing here races a timer.

`lib.rs`, lines 167–181 — the admission predicate, a private free
function placed directly above the `impl` block that uses it:

```text
 167 | /// Admit a claimed own forced win, or reject it as unknown.
 168 | ///
 169 | /// The player is the certifier: the proof must name the side to move, start
 170 | /// with a legal move on this board, and survive the engine's line verifier.
 171 | /// Anything less is unknown, and ordinary search runs instead.
 172 | fn certified_own_move(board: &Board, proof: &engine::Proof) -> Option<engine::Move> {
 173 |     if proof.winner != board.to_move() {
 174 |         return None;
 175 |     }
 176 |     let first = *proof.line.first()?;
 177 |     if !board.is_legal(first) {
 178 |         return None;
 179 |     }
 180 |     engine::verify_line(board, proof).then_some(first)
 181 | }
```

**COMMENT — `certified_own_move`.**

- Why needed: `prove_forced_win` returns a *claimed* proof. Between
  the claim and the played move sit four failure modes — the proof
  names the wrong winner, the line is empty (no first move to play),
  the first move is not legal on this board, or the line does not
  survive verification. Each is checked explicitly so that a
  malformed or stale proof degrades to "unknown → ordinary search"
  instead of a wrong stone on the board. The checks run against the
  real board at decision time, so a certified decision on an ongoing
  board plays the first move of a verified line and nothing else —
  the game itself is not finished; only the forcing sequence is
  certified.
- Why a free function, not a method: it needs no player state — only
  the board and the proof — so a method would imply access it does
  not have. Why private: it is an implementation detail of `decide`;
  its unit test (U1) lives in the same module's `#[cfg(test)]` block,
  so no `pub` or `pub(crate)` widening was needed to test it.
- Rust mechanics, line by line: `proof.winner != board.to_move()`
  compares `Color` (`Copy`, `Eq`) — absolute colors, so a White proof
  on a Black-to-move board is rejected even though the engine already
  refuses to produce one. `*proof.line.first()?` combines two checks:
  `first()` returns `Option<&Move>`, the `?` propagates `None` (empty
  line → rejection), and the `*` copies the move out of the reference
  (`Move` is `Copy`). `board.is_legal(first)` re-checks occupancy
  against the live board. `engine::verify_line(board, proof)
  .then_some(first)` — `bool::then_some` evaluates to
  `Some(first)` only on `true`; the value is a copy, so no closure
  laziness (`then`) is needed. The function returns
  `Option<Move>`: `Some` is the certificate *plus* the move to play,
  in one value.
- Necessary versus optional: the winner check and the `verify_line`
  call are the substantive soundness checks; the empty-line and
  legality checks are defensive overlap (the current verifier rejects
  both) that cost one branch each and guard against future prover
  changes. The function's exact shape (free function, `then_some`) is
  a choice; the four rejections are the necessity.

`lib.rs`, lines 218–230 — the probe call site in `decide`:

```text
 218 |         if options.own.enabled {
 219 |             // One direct engine probe per decision, never cached: AiPlayer
 220 |             // holds no state besides the evaluator.
 221 |             if let Some(mv) = engine::prove_forced_win(board, board.to_move(), options.own.budget)
 222 |                 .and_then(|proof| certified_own_move(board, &proof))
 223 |             {
 224 |                 return Ok(Decision {
 225 |                     mv,
 226 |                     search: None,
 227 |                     guard: GuardDiagnostics::new(mv, GuardOutcome::ProvenWin),
 228 |                 });
 229 |             }
 230 |         }
```

**COMMENT — the probe call site.**

- Why here, before search: a certified forced win makes the search's
  answer irrelevant — no visit distribution outranks a verified forced
  sequence. Running the probe after the search would spend the whole
  simulation budget to learn nothing. Running it inside the evaluator
  (the legacy design) forced every evaluator implementor to confront
  certification; running it here, the trait stays one method wide.
- No injection seam, deliberately: the prover is called directly, once,
  at this single site. The in-code comment records the structural
  consequence — `AiPlayer` holds no state besides the evaluator, so
  there is nowhere a proof could be cached, and "the probe runs once
  per decision" is a code-review fact rather than a tested count.
  The dynamic evidence is the fixture pair: D6 proves the probe fires
  and search is skipped (a panicking evaluator survives); D9 proves a
  *later, separate* quiet board is really searched — the certified
  move is not even legal there, so no carried-over result could
  produce the observed decision. An injectable prover would let tests
  count calls directly but would add a production seam whose only
  purpose is testing; the design accepts the fixture-plus-structure
  evidence instead (stated as a limitation in the design paper).
- What the early return skips: the evaluator (never called), the RNG
  (never drawn), the guard (zero checks), statistics (`search: None`),
  and — unlike the legacy hook, where `mcts::search` allocated a
  one-root-node tree and dropped it on the proven path — here the
  return happens *before* `mcts::search` is reached, so **no tree is
  allocated at all** on the proof path. Verified against the source:
  the `mcts::search` call is line 236, strictly after this return.
- `GuardDiagnostics::new(mv, GuardOutcome::ProvenWin)` reuses the guard
  vocabulary for the bypass: the outcome reads "certified own forced
  win" — certified *at decision time by the player*, the hook-era
  meaning ("the evaluator certified at the root") is retired with the
  hook.
- Rust mechanics: `.and_then(|proof| certified_own_move(board, &proof))`
  chains the two `Option` stages — prover, then admission — into one;
  the closure borrows `proof` so the (potentially long) line vector is
  dropped uncloned on rejection. `if let Some(mv)` binds the copied
  move; the enclosing `if options.own.enabled` keeps the prover from
  running at all when the experiment is off, which is what makes the
  feature free for the Tactics seat.
## 8. `decide`, step 3: ordinary search — unchanged

`lib.rs`, lines 231–236:

```text
 231 |         let config = SearchConfig {
 232 |             simulations: options.simulations,
 233 |             c_puct: options.c_puct,
 234 |         };
 235 |         let mcts::SearchOutcome { mut tree, root } =
 236 |             mcts::search(board, &mut *self.evaluator, &config);
```

**COMMENT — the one search.**

- Everything in this fragment is pre-existing mcts behavior, invoked
  unchanged: `mcts::search` builds the tree, expands the root, runs the
  PUCT simulation loop, and returns `SearchOutcome { tree, root }`.
  The full algorithm is documented in the mcts tutorial
  ([the simulation loop](../../mcts-tutorial/07-the-simulation-loop.md)
  and [from tree to move](../../mcts-tutorial/08-from-tree-to-move.md))
  and is deliberately not reprinted here; the `SearchOutcome` type and
  the one-method `Evaluator` trait are byte-identical to chapter 07.
- What *is* new is only the caller: the search now runs inside
  `decide`, after the own probe, instead of directly inside
  `execute_step`. The `SearchConfig` is rebuilt from the two options
  fields because `mcts::SearchConfig` intentionally knows nothing about
  temperature, noise, or probes — those are selection-time and
  decision-time concerns, and the crate boundary keeps them out of
  search.
- Rust mechanics: `&mut *self.evaluator` dereferences the `Box` and
  reborrows the trait object mutably for the duration of the call;
  `mut tree` in the destructure declares the binding mutable because
  the noise step below mutates it — the only reason for the `mut`.

## 9. `decide`, step 4: post-search noise — unchanged semantics

`lib.rs`, lines 237–245:

```text
 237 |         if options.noise_epsilon > 0.0 {
 238 |             mcts::add_dirichlet_noise(
 239 |                 &mut tree,
 240 |                 root,
 241 |                 options.noise_epsilon,
 242 |                 options.noise_alpha,
 243 |                 rng,
 244 |             );
 245 |         }
```

**COMMENT — noise after search.**

- Why after the search: this preserves the CLI's established behavior
  exactly (the pre-player CLI applied noise to the finished tree in the
  same position). Root Dirichlet noise is an exploration device; here
  it runs once per *decision*, not per simulation.
- What noise does **not** change: the visit counts. `select_move`
  below reads only visits, so noise never alters the move-selection
  weights of this decision — no visit "repair" or redistribution
  happens or is needed.
- What noise **does** change: (a) the stored root-edge priors, which
  `SearchStats::from_searched` (§12) reads into `stats.prior`, so the
  recorded policy shows the noised priors; (b) the shared RNG, which
  the sampler advances, so the temperature draw below — and every later
  session draw — sees a different state than a no-noise run. The
  `noise_epsilon > 0.0` gate is what makes a disabled-noise decision
  consume zero RNG draws; tests D2 and D6 both depend on that fact.
- Rust mechanics: `rng` is the `&mut impl Rng` parameter passed
  straight through — the player owns no RNG of its own; the session
  lends its stream, which is what makes seeded sessions reproducible
  end to end.

## 10. `decide`, step 5: the proposal

`lib.rs`, lines 246–248:

```text
 246 |         let visits = mcts::visit_distribution(&tree, root);
 247 |         let proposal = mcts::select_move(&visits, options.temperature, rng)
 248 |             .expect("AI must select a move from a non-empty distribution");
```

**COMMENT — selection.** Unchanged mcts machinery
([from tree to move](../../mcts-tutorial/08-from-tree-to-move.md)):
`visit_distribution` reads the root edges' visit counts; `select_move`
samples proportionally at the configured temperature and is argmax at
temperature 0. The `expect` rests on an invariant, not luck: an ongoing
root is always expanded, so the distribution is non-empty. The result
is named `proposal` — not `mv` — because the guard may still overrule
it; the rename is the only new thing in this step, and it is the honest
name for a value that is no longer final.

## 11. `decide`, step 6: the opponent-refutation guard

The guard is the bounded defensive experiment ported from legacy 08b.
Its call site comes first — `lib.rs`, lines 249–255:

```text
 249 |         let (mv, guard) = guard::choose(
 250 |             board,
 251 |             proposal,
 252 |             tree.node(root).edges(),
 253 |             options.guard,
 254 |             engine::prove_forced_win,
 255 |         );
```

**COMMENT — the call site.**

- Why the guard sees `tree.node(root).edges()`: the candidates are the
  root edges — every legal move, ranked by the search's own visits and
  priors. The edges are passed as a shared slice; the guard clones
  boards, never the tree, and its diagnostics never feed back into
  visits or priors.
- `engine::prove_forced_win` is passed as a function value — the real
  prover, not a seam the player uses. The parameter type on `choose`
  is an `impl FnMut` closure (below) that exists so the guard's private
  unit tests can inject panicking or malformed provers. Production
  always passes the engine function; this is a test seam for the
  guard's bookkeeping, not an evaluator hook and not a policy.
- Why after selection, not before: the guard refines *one* concrete
  proposal with *ranked* alternatives; ranking needs the finished
  search. Checking before search would probe 225 candidates with no
  ranking at all.

The guard module, `gomoku/crates/player/src/guard.rs`, is a verbatim
port from legacy 08b (module documentation wording aside) — the
semantics were already locked there and are re-verified here against
the current engine. Lines 1–8 state the module's one rule:

```text
   1 | //! Bounded defensive refutation experiment (`choose`) and its data types.
   2 | //!
   3 | //! After ordinary move selection, an enabled guard probes the opponent on
   4 | //! cloned boards. A missing or unverified proof means unknown, never safe;
   5 | //! the chosen move and the search statistics stay separate concerns.
   6 | 
   7 | use engine::{Board, Color, Move, Proof, SearchBudget, Status};
   8 | use mcts::Edge;
```

Lines 10–53 — configuration, with the same preset idiom as
`OwnTssConfig`:

```text
  10 | /// Bounded defensive refutation experiment, not a wall-clock deadline.
  11 | ///
  12 | /// Disabled by default. A missing proof means unknown, never safety. The
  13 | /// candidate cap bounds hypothetical checks; each probe has its own node/depth
  14 | /// budget. Proof verification is also required and is not a timed operation.
  15 | #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  16 | pub struct GuardConfig {
  17 |     /// Whether to inspect ordinary search candidates after selection.
  18 |     pub enabled: bool,
  19 |     /// Maximum candidate checks, including the original proposal. Zero skips all.
  20 |     pub max_candidates: usize,
  21 |     /// Per-candidate opponent prover work caps, not a wall-clock deadline.
  22 |     pub budget: SearchBudget,
  23 | }
  24 | 
```

```text
  25 | impl GuardConfig {
  26 |     /// Preserve ordinary move selection without checking any candidate.
  27 |     #[must_use]
  28 |     pub const fn disabled() -> Self {
  29 |         Self {
  30 |             enabled: false,
  31 |             ..Self::baseline()
  32 |         }
  33 |     }
  34 | 
  35 |     /// [experiment] Baseline defaults: five candidates, 1,000 nodes / 10 plies
  36 |     /// per opponent probe. These are work bounds, not a response-time guarantee.
  37 |     #[must_use]
  38 |     pub const fn baseline() -> Self {
  39 |         Self {
  40 |             enabled: true,
  41 |             max_candidates: 5,
  42 |             budget: SearchBudget {
  43 |                 max_nodes: 1_000,
  44 |                 max_depth: 10,
  45 |             },
  46 |         }
  47 |     }
  48 | }
  49 | 
  50 | impl Default for GuardConfig {
  51 |     fn default() -> Self {
  52 |         Self::disabled()
  53 |     }
```

**COMMENT — `GuardConfig`.**

- Two different bounds, on purpose: `max_candidates` caps how many
  hypothetical moves are checked at all; `budget` caps how hard each
  opponent probe works. A candidate cap of 5 with a 1,000-node probe
  bounds the experiment at "5 × a small proof search" — predictable,
  budgeted work, still not a wall-clock deadline (the doc comments say
  so twice, because an unresponsive UI was the failure this experiment
  was born from).
- `disabled()` keeps the baseline numbers, same idiom as
  `OwnTssConfig::disabled()`; the default is disabled (test G1 pins
  both presets).
- Necessary versus optional: `enabled` and the two bounds are the
  experiment's definition; `baseline()`/`disabled()` presets and the
  `const` qualifier are ergonomics.

Lines 55–97 — the outcome vocabulary and the diagnostics record:

```text
  55 | 
  56 | /// What the defensive experiment established (or deliberately left unknown).
  57 | #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  58 | pub enum GuardOutcome {
  59 |     /// Guard disabled; the original candidate is unchecked.
  60 |     Disabled,
  61 |     /// Certified own forced win bypassed both ordinary selection and this guard.
  62 |     ProvenWin,
  63 |     /// No verified opponent win found within the probe budget: unknown, not safe.
  64 |     Unrefuted,
  65 |     /// The checked candidate immediately won for us or drew the game.
  66 |     Terminal,
  67 |     /// Candidate cap reached; selected move is untested, not certified safe.
  68 |     BudgetExhausted,
  69 |     /// Every legal candidate was individually refuted; original move retained.
  70 |     /// This is not a proof that the root position is lost.
  71 |     AllRefuted,
  72 | }
  73 | 
  74 | /// Decision-local diagnostics; these never alter MCTS visits or priors.
```

```text
  76 | pub struct GuardDiagnostics {
  77 |     /// Original ordinary selection (or the certified own winning move).
  78 |     pub proposal: Move,
  79 |     /// Candidates refuted by verified opponent proofs, in check order.
  80 |     pub refuted: Vec<Move>,
  81 |     /// Hypothetical candidates checked, including immediate wins/draws.
  82 |     pub checked: usize,
  83 |     /// Result of the defensive experiment, not a position-value judgement.
  84 |     pub outcome: GuardOutcome,
  85 | }
  86 | 
  87 | impl GuardDiagnostics {
  88 |     pub(crate) fn new(proposal: Move, outcome: GuardOutcome) -> Self {
  89 |         Self {
  90 |             proposal,
  91 |             refuted: Vec::new(),
  92 |             checked: 0,
  93 |             outcome,
  94 |         }
  95 |     }
  96 | }
  97 | 
```

**COMMENT — `GuardOutcome` and `GuardDiagnostics`.**

- Six variants, each a statement about *what is known*, none about
  position value. The three that need care: `Unrefuted` means the probe
  found nothing within budget — **unknown, not safe** (the prover
  proves wins; it cannot prove their absence). `BudgetExhausted` means
  the returned move was never tested — also not safe. `AllRefuted`
  means every legal candidate was individually refuted — remarkable,
  but still **not a proof that the root is lost** (each probe was
  bounded; a deeper defense might exist behind any of them), so the
  original proposal is retained rather than resigning.
- `ProvenWin` is produced only by the own probe (§7), never by
  `choose`; the variant lives in this enum because a `Decision` carries
  one diagnostics value regardless of which mechanism certified it.
- `Eq` is derivable here (all fields are integers, moves, colors) —
  unlike `SearchStats`, which contains floats.
- `GuardDiagnostics::new` is `pub(crate)`: constructing diagnostics is
  the module's own business; the CLI and tests read the public fields
  but never build one. This is the visibility floor that still lets
  `decide` (same crate) certify.
- `refuted` is a `Vec<Move>` in check order, not a set — order is
  diagnostic information (G2 asserts the exact vector), and the
  duplicate-skip in the loop below is cheap at ≤ 5 entries.

Now `choose` itself, in execution order. Lines 98–114 — the signature
and the two early exits:

```text
  98 | // Private injection seam for bounded bookkeeping tests, never an evaluator hook.
  99 | // Expanded MCTS roots contain every legal edge, including unvisited edges.
 100 | pub(crate) fn choose(
 101 |     board: &Board,
 102 |     proposal: Move,
 103 |     edges: &[Edge],
 104 |     config: GuardConfig,
 105 |     mut probe: impl FnMut(&Board, Color, SearchBudget) -> Option<Proof>,
 106 | ) -> (Move, GuardDiagnostics) {
 107 |     let mut diagnostics = GuardDiagnostics::new(proposal, GuardOutcome::Disabled);
 108 |     if !config.enabled {
 109 |         return (proposal, diagnostics);
 110 |     }
 111 |     diagnostics.outcome = GuardOutcome::BudgetExhausted;
 112 |     if config.max_candidates == 0 {
 113 |         return (proposal, diagnostics);
 114 |     }
```

**COMMENT — signature and disabled paths.**

- `pub(crate)`: the CLI never calls `choose` directly — it goes through
  `decide`. The `mut probe: impl FnMut(&Board, Color, SearchBudget) ->
  Option<Proof>` parameter is the test seam named at the call site;
  `FnMut` because a test closure counts its own calls.
- Why `edges: &[Edge]` instead of a board: the caller already has the
  expanded root; re-deriving candidates from the board would discard
  the search's ranking, which is the whole input to the retry order.
  The comment at line 99 records the invariant the AllRefuted assertion
  at the end relies on.
- The two early exits both return the proposal: disabled means
  "unchecked" (`Disabled`), a zero cap means "nothing checkable"
  (`BudgetExhausted` — set optimistically at line 111 so both the zero
  cap and the loop's cap hit share one outcome). Test P3 pins that
  neither path probes.

Lines 115–130 — ranking and the candidate iterator:

```text
 115 |     let mut ranked: Vec<_> = edges.iter().collect();
 116 |     ranked.sort_by(|a, b| {
 117 |         b.n.cmp(&a.n)
 118 |             .then_with(|| {
 119 |                 b.prior
 120 |                     .partial_cmp(&a.prior)
 121 |                     .unwrap_or(std::cmp::Ordering::Equal)
 122 |             })
 123 |             .then_with(|| a.mv.index().cmp(&b.mv.index()))
 124 |     });
```

```text
 125 |     let candidates = std::iter::once(proposal).chain(
 126 |         ranked
 127 |             .into_iter()
 128 |             .map(|edge| edge.mv)
 129 |             .filter(|&mv| mv != proposal),
 130 |     );
```

**COMMENT — ranking and candidates.**

- The retry order is visits descending, then prior descending, then
  move index ascending — the search's own preferences, made total by a
  deterministic tiebreak. `partial_cmp(...).unwrap_or(Equal)` treats a
  NaN prior as a tie rather than panicking; NaN priors would indicate
  an evaluator bug elsewhere, and the guard refuses to amplify one
  into a panic. Test P1 pins the full order, including that creation
  order does not leak into ties (`edges.reverse()` in the test).
- `std::iter::once(proposal).chain(...)` puts the proposal first and
  appends every other edge in ranked order; the `filter` removes the
  proposal's duplicate from the ranked tail. The proposal goes first
  because it is the move the search actually chose — the guard's job
  is to save it if possible, not to shop for alternatives.

Lines 131–144 — the loop, the duplicate skip, the cap, and the clone:

```text
 131 |     for candidate in candidates {
 132 |         if diagnostics.refuted.contains(&candidate) {
 133 |             continue;
 134 |         }
 135 |         // All previously checked candidates were refuted. The next ranked
 136 |         // candidate is untested, so the cap must not send us back to a loser.
 137 |         if diagnostics.checked == config.max_candidates {
 138 |             return (candidate, diagnostics);
 139 |         }
 140 |         let mut after = board.clone();
 141 |         after
 142 |             .play(candidate)
 143 |             .expect("root candidates must be legal");
 144 |         diagnostics.checked += 1;
```

**COMMENT — cap before clone.**

- The cap check (lines 135–139) runs *before* the clone and play: when
  the budget is spent, the highest-ranked **untested** candidate is
  returned — never a known loser. That is the whole reason the check
  sits here rather than after the probe. The returned candidate is
  untested, and `BudgetExhausted` says so; the diagnostics do not dress
  it up as vetted.
- `diagnostics.refuted.contains(&candidate)` (line 132) is defensive:
  candidates are unique by construction, so it never fires today; it
  keeps the loop robust if the edge list ever gains duplicates.
  Harmless, and the reviewer flagged it as exactly that.
- `board.clone()` per candidate (line 140) is why the session board is
  never mutated — every hypothetical plays on its own copy, and the
  `expect` at line 143 rests on the line-99 invariant (root edges are
  legal moves). Test G2 asserts board equality *and* Zobrist equality
  after a guarded decision.

Lines 145–162 — the terminal fastpath and the probe with its
admission:

```text
 145 |         if matches!(after.status(), Status::Won(side) if side == board.to_move())
 146 |             || after.status() == Status::Draw
 147 |         {
 148 |             diagnostics.outcome = GuardOutcome::Terminal;
 149 |             return (candidate, diagnostics);
 150 |         }
```

```text
 151 |         let opponent = after.to_move();
 152 |         let refuted = probe(&after, opponent, config.budget).is_some_and(|proof| {
 153 |             proof.winner == opponent
 154 |                 && proof.line.first().is_some_and(|&mv| after.is_legal(mv))
 155 |                 && engine::verify_line(&after, &proof)
 156 |         });
 157 |         if !refuted {
 158 |             diagnostics.outcome = GuardOutcome::Unrefuted;
 159 |             return (candidate, diagnostics);
 160 |         }
 161 |         diagnostics.refuted.push(candidate);
 162 |     }
```

**COMMENT — terminal fastpath and opponent admission.**

- The terminal check (lines 145–150) runs before the probe: a
  candidate that wins the game or draws it needs no opponent proof —
  there is no opponent turn to refute. `matches!` with the guard
  `side == board.to_move()` restricts `Won` to *our* win; a candidate
  that ends the game in the opponent's favor cannot exist here (our own
  move cannot complete their five), but the guard states the intended
  reading directly. `checked` is incremented before this return, so
  terminal candidates count against the cap — they are checks, just
  cheap ones.
- The probe targets `after.to_move()` (line 151) — the actual opponent
  on the actual hypothetical board, not a color computed from the root.
  This is the absolute-color discipline: test G5 builds a White seat
  and pins that Black gets probed.
- The admission chain (lines 152–156) mirrors the own probe's
  predicate with the winner flipped to the opponent: right winner,
  non-empty line with a legal first move, verified line. `is_some_and`
  collapses "probe says nothing" (`None`) and "proof malformed" into
  the same outcome: not refuted. The asymmetry is the design's center:
  a refutation must be airtight to change the move, while *failing* to
  refute changes nothing and proves nothing — `Unrefuted`, unknown.
- `verify_line` has no budget: it replays one concrete line, work
  bounded by line length. Only the *search* for proofs is budgeted.

Lines 163–170 — the loop's only fall-through:

```text
 163 |     assert_eq!(
 164 |         diagnostics.refuted.len(),
 165 |         board.empty_moves().count(),
 166 |         "expanded root must include every legal candidate"
 167 |     );
 168 |     diagnostics.outcome = GuardOutcome::AllRefuted;
 169 |     (proposal, diagnostics)
 170 | }
```

**COMMENT — `AllRefuted`.** Reaching this line means every candidate
was checked and refuted, so the assertion compares the refuted count
against the number of legal moves at the root. It is a plain
`assert_eq!`, compiled in **all** profiles (not a `debug_assert!` that
release builds would strip): the invariant is cheap to check and its
violation would mean the mcts root-expansion contract broke, which is
worth a loud failure everywhere. The return is still the original
proposal — with every alternative individually refuted within budget,
no better move is known, and per the variant's documentation this is
not a loss certificate.

## 12. `decide`, step 7: honest statistics

`lib.rs`, lines 256–262 — the searched path returns:

```text
 256 |         Ok(Decision {
 257 |             mv,
 258 |             search: Some(SearchStats::from_searched(&tree, root, board)),
 259 |             guard,
 260 |         })
 261 |     }
 262 | }
```

**COMMENT — the return.** Note the order: `from_searched` runs on the
real tree *regardless of what the guard did to the move*. The
statistics always describe the proposal search, never the guard's
hypothetical probes (the `decide` rustdoc states this in one
sentence); tests G2, G4, and G12 assert `guarded.search ==
plain.search` under substitution. The `Decision` fields are filled from
three different sources — `mv` possibly from the guard, `search` from
the tree, `guard` from the diagnostics — and the honesty rule is that
each field names its true source.

`lib.rs`, lines 106–143 — the statistics type and its private
extractor, ported verbatim from legacy 08b:

```text
 106 | pub struct SearchStats {
 107 |     /// Player whose perspective determines the root value.
 108 |     pub side_to_move: engine::Color,
 109 |     /// All root moves and visit counts, descending by count, then move index.
 110 |     pub visits: Vec<(engine::Move, u32)>,
 111 |     /// All root priors after optional noise, descending by prior, then move index.
 112 |     pub prior: Vec<(engine::Move, f32)>,
 113 |     /// Root value from the side-to-move perspective: `sum(W) / sum(N)`.
 114 |     /// Zero when no simulations have visited an edge.
 115 |     pub value: f32,
 116 | }
```

```text
 118 | impl SearchStats {
 119 |     fn from_searched(tree: &mcts::Tree, root: mcts::NodeId, board: &Board) -> Self {
 120 |         let edges = tree.node(root).edges();
 121 |         let mut visits = mcts::visit_distribution(tree, root);
 122 |         visits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.index().cmp(&b.0.index())));
 123 |         let mut prior: Vec<_> = edges.iter().map(|edge| (edge.mv, edge.prior)).collect();
 124 |         prior.sort_by(|a, b| {
 125 |             b.1.partial_cmp(&a.1)
 126 |                 .unwrap_or(std::cmp::Ordering::Equal)
 127 |                 .then_with(|| a.0.index().cmp(&b.0.index()))
 128 |         });
 129 |         let total_visits: u32 = edges.iter().map(|edge| edge.n).sum();
 130 |         let total_value: f32 = edges.iter().map(|edge| edge.w).sum();
 131 |         let value = if total_visits == 0 {
 132 |             0.0
 133 |         } else {
 134 |             total_value / total_visits as f32
 135 |         };
 136 |         Self {
 137 |             side_to_move: board.to_move(),
 138 |             visits,
 139 |             prior,
 140 |             value,
 141 |         }
 142 |     }
 143 | }
```

**COMMENT — `SearchStats` and `from_searched`.**

- Why sorted copies: consumers (the CLI overlay, future datagen) read
  "best first" order without re-sorting; the move-index tiebreak makes
  the order total and reproducible. `prior` is read from the edges
  *after* noise, so it records the noised priors — the same numbers
  the pre-player CLI stored.
- The value is the visit-weighted mean `ΣW/ΣN` from the side-to-move
  perspective, with an explicit zero guard for a zero-simulation
  search (test D2's `(0, 1.0, 0.25)` tuple exercises exactly that
  path, where every root edge is unvisited).
- Why private (`fn`, no `pub`): the crate's public surface is the
  *decision*, not tree introspection. External callers receive a
  finished `SearchStats`; only `decide` builds one. The name
  `from_searched` is deliberately distinct from the CLI's
  `from_search`/`from_stats` pair (see the glossary disambiguation):
  three constructors, three different inputs, three names that say so.
- Rust mechanics: `b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal)` is
  the same NaN-tolerant tiebreak idiom as the guard's ranking; visits
  sort with `u32::cmp` because counts are total. The two local
  `Vec`s are owned and sorted in place; the `edges` borrow ends before
  the struct literal, so the tree can be dropped when `decide`
  returns.

## 13. Back in the session: proof path, searched path, events

`decide` has returned. `session.rs`, lines 458–471 — the call and the
proof-path early return:

```text
 458 |         let player = match side {
 459 |             Color::Black => &mut self.black_player,
 460 |             Color::White => &mut self.white_player,
 461 |         }
 462 |         .as_mut()
 463 |         .expect("AI seat must own a persistent player");
 464 |         let decision = player
 465 |             .decide(&board, &options, &mut self.rng)
 466 |             .map_err(|player::DecisionError::GameOver| CommandError::GameOver)?;
 467 |         let mv = decision.mv;
 468 |         let Some(stats) = decision.search else {
 469 |             // A certified own forced win bypasses search and its statistics.
 470 |             return self.execute_play(mv);
 471 |         };
```

**COMMENT — borrowing the slot and routing the proof path.**

- The `match side` over the two slots yields
  `&mut Option<AiPlayer>`; `.as_mut()` turns that into
  `Option<&mut AiPlayer>`, and the `expect` converts the
  impossible-by-invariant `None` into a panic with a message that
  names the invariant (§2's seat check makes it unreachable through
  the public command surface).
- `map_err` translates the player's error vocabulary into the CLI's:
  `DecisionError::GameOver` becomes `CommandError::GameOver`. The two
  enums stay separate because the player crate must not know CLI error
  types (dependency direction, §1).
- The `let Some(stats) = ... else` binding is the whole proof-path
  routing: a certified decision goes to `execute_play` — the same
  legality-checked play path a human move takes — and returns its
  events directly. Nothing is inserted into `self.search_info` on this
  path. Scoped precisely: there is no **new** record at the fresh
  cursor; a *historical* record stored at an earlier visit to the same
  node persists in the map untouched, and the tests assert
  `search_info().is_none()` only for fresh cursors (S3, C2). The
  history map is keyed by node id and never purged by the step paths;
  `reset` and puzzle navigation clear it (session.rs lines 774 and
  706).

`session.rs`, lines 472–478 — the searched path continues with one
constructor swapped:

```text
 472 |         let info = SearchInfo::from_stats(stats, &effective_config);
 473 | 
 474 |         let old_cursor = self.history.cursor();
 475 |         let play_outcome = self.history.play(mv)?;
 476 |         let new_cursor = self.history.cursor();
 477 |         self.search_info.insert(new_cursor, info);
 478 | 
```

**COMMENT — the record swap.**

- Line 472 is the only behavioral edit in this block: the record is
  built from the player's `SearchStats` instead of directly from the
  tree. Everything after is the unchanged pre-player tail.
- Why insert at the new cursor (line 477): the record describes the
  search that *produced* the move, and the natural place to look at it
  is the position after the move — the same convention as before this
  chapter.

Lines 479–499 — the event tail, byte-identical to the pre-player code:

```text
 479 |         let mut events = Vec::new();
 480 |         events.push(Event::Moved {
 481 |             mv,
 482 |             by: OccupantKind::Ai,
 483 |         });
 484 | 
 485 |         if matches!(play_outcome, PlayOutcome::Moved(_)) {
 486 |             let siblings = self.history.children(old_cursor);
 487 |             if siblings.len() > 1 {
 488 |                 events.push(Event::Branched { options: siblings });
 489 |             }
 490 |         }
 491 | 
 492 |         let new_board = self.history.board();
 493 |         if new_board.status() != Status::Ongoing {
 494 |             events.push(Event::GameOver {
 495 |                 result: new_board.status(),
 496 |             });
 497 |         }
 498 | 
 499 |         Ok(events)
```

**COMMENT — events (unchanged).** Cursor bookkeeping, legality-checked
`history.play`, `Moved`/`Branched`/`GameOver` assembly — kept exactly
so that event order and history semantics are preserved (test C1
proves this over eleven seeded plies). Note what is absent: no
noise call, no visit sampling, no `SearchInfo` construction — all of
that moved into `decide`, and this function now only commits the
result.

`gomoku/crates/cli/src/actor.rs`, lines 39–50 — the new constructor:

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
  50 | 
```

**COMMENT — `from_stats`.** A field-by-field move out of the player's
statistics, attaching the effective seat configuration (cloned, as
`SearchInfo` outlives the step). The pre-existing `from_search` at
line 57 — which takes an `mcts::SearchOutcome` and does its own tree
reads — is **byte-untouched**: session tests and the seeded regression
still exercise it on the manual path (C1 builds its expected values
through it). Two constructors side by side, each naming its input;
`from_stats` exists so `execute_step` never needs the player's tree,
which is dropped when `decide` returns.
## Tests

Every new test body is quoted in full below, grouped by file in the
same order execution reaches the behavior: player decisions, admission,
guard (public then private), CLI session, CLI actor. Shared fixture
helpers are quoted once per file with their line ranges stated; no
test name appears without its body.

### Player decisions — `crates/player/tests/decisions.rs`

Lines 1–8 are the imports; lines 9–33 define the two shared fixtures —
a one-move-from-victory board and a panicking evaluator:

```text
   1 | use std::cell::{Cell, RefCell};
   2 | use std::rc::Rc;
   3 | 
   4 | use engine::{Board, Move, Status};
   5 | use mcts::{EvalRequest, EvalResult, Evaluator, SearchConfig, UniformEvaluator};
   6 | use player::{AiPlayer, DecisionError, GuardConfig, GuardOutcome, OwnTssConfig, PlayerOptions};
   7 | use rand::{RngExt, SeedableRng, rngs::StdRng};
   8 | 
```

```text
   9 | fn winning_position() -> Board {
  10 |     let mut board = Board::new();
  11 |     for (row, col) in [
  12 |         (7, 3),
  13 |         (0, 0),
  14 |         (7, 4),
  15 |         (0, 2),
  16 |         (7, 5),
  17 |         (0, 4),
  18 |         (7, 6),
  19 |         (0, 6),
  20 |     ] {
  21 |         board.play(Move::new(row, col).unwrap()).unwrap();
  22 |     }
  23 |     board
  24 | }
  25 | 
  26 | struct NeverEvaluate;
  27 | 
  28 | impl Evaluator for NeverEvaluate {
  29 |     fn evaluate(&mut self, _: EvalRequest) -> EvalResult {
  30 |         panic!("terminal boards must not reach the evaluator");
  31 |     }
  32 | }
  33 | 
```

**COMMENT — shared fixtures.** `winning_position` leaves Black one move
from five, so playing (7, 7) finishes the game — the terminal fixture.
`NeverEvaluate` panics on any evaluation: with it behind a player, a
passing test *proves* the evaluator was never called, which is stronger
than counting calls (a count of zero could also mean a broken counter).
The `Rc<RefCell<...>>` and `Cell` patterns used below are single-threaded
test scaffolding — the evaluators must be shared with the test after
being moved into the player, and `Rc`/`RefCell` is the cheapest way; it
works because tests are single-threaded per instance and the player
never sends the evaluator across threads here.

D1 — the terminal refusal, lines 34–48:

```text
  34 | #[test]
  35 | fn terminal_board_returns_game_over_without_evaluation_or_rng() {
  36 |     let mut board = winning_position();
  37 |     board.play(Move::new(7, 7).unwrap()).unwrap();
  38 |     assert_ne!(board.status(), Status::Ongoing);
  39 |     let mut player = AiPlayer::new(Box::new(NeverEvaluate));
  40 |     let mut rng = StdRng::seed_from_u64(7);
  41 |     let mut untouched_rng = StdRng::seed_from_u64(7);
  42 | 
  43 |     let result = player.decide(&board, &PlayerOptions::default(), &mut rng);
  44 | 
  45 |     assert_eq!(result.unwrap_err(), DecisionError::GameOver);
  46 |     assert_eq!(rng.random::<u64>(), untouched_rng.random::<u64>());
  47 | }
  48 | 
```

**COMMENT — D1.** Arrange: a finished game (Black won). Act: `decide`.
Assert: `GameOver`, and — the real content — the RNG streams match:
`rng` drew nothing, proven by comparing the next `u64` against an
untouched same-seed stream. The panicking evaluator covers the
evaluation half. Assertion scope: this proves the *check ordering*
(terminal before evaluation and RNG), not that every terminal shape
behaves alike; one `Won` fixture stands in for the enum.

D2 — the ordinary path equals the manual path, lines 49–122:

```text
  49 | #[test]
  50 | fn ordinary_decisions_match_manual_search_stats_and_seeded_selection() {
  51 |     let mut board = Board::new();
  52 |     board.play(Move::new(7, 7).unwrap()).unwrap();
  53 |     for (simulations, temperature, noise_epsilon) in [
  54 |         (17, 0.0, 0.0),
  55 |         (19, 1.0, 0.0),
  56 |         (23, 0.7, 0.25),
  57 |         (0, 1.0, 0.25),
  58 |     ] {
  59 |         let options = PlayerOptions {
  60 |             simulations,
  61 |             c_puct: 1.7,
  62 |             temperature,
  63 |             noise_epsilon,
  64 |             noise_alpha: 0.3,
  65 |             ..PlayerOptions::default()
  66 |         };
  67 |         let mut manual_rng = StdRng::seed_from_u64(42);
  68 |         let mut player_rng = StdRng::seed_from_u64(42);
  69 |         let mut evaluator = UniformEvaluator::new(0.6);
  70 |         let mcts::SearchOutcome { mut tree, root } = mcts::search(
  71 |             &board,
  72 |             &mut evaluator,
  73 |             &SearchConfig {
  74 |                 simulations,
  75 |                 c_puct: options.c_puct,
  76 |             },
  77 |         );
  78 |         if noise_epsilon > 0.0 {
  79 |             mcts::add_dirichlet_noise(
  80 |                 &mut tree,
  81 |                 root,
  82 |                 noise_epsilon,
  83 |                 options.noise_alpha,
  84 |                 &mut manual_rng,
  85 |             );
  86 |         }
  87 |         let visits = mcts::visit_distribution(&tree, root);
  88 |         let expected_move = mcts::select_move(&visits, temperature, &mut manual_rng).unwrap();
  89 |         let mut expected_visits = visits;
  90 |         expected_visits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.index().cmp(&b.0.index())));
  91 |         let edges = tree.node(root).edges();
  92 |         let mut expected_prior: Vec<_> = edges.iter().map(|e| (e.mv, e.prior)).collect();
  93 |         expected_prior.sort_by(|a, b| {
  94 |             b.1.partial_cmp(&a.1)
  95 |                 .unwrap()
  96 |                 .then_with(|| a.0.index().cmp(&b.0.index()))
  97 |         });
  98 |         let expected_value = if simulations == 0 {
  99 |             0.0
 100 |         } else {
 101 |             edges.iter().map(|e| e.w).sum::<f32>() / simulations as f32
 102 |         };
 103 |         let mut player = AiPlayer::new(Box::new(UniformEvaluator::new(0.6)));
 104 | 
 105 |         let decision = player.decide(&board, &options, &mut player_rng).unwrap();
 106 | 
 107 |         assert_eq!(decision.mv, expected_move);
 108 |         assert!(board.is_legal(decision.mv));
 109 |         let stats = decision
 110 |             .search
 111 |             .expect("ordinary search has real statistics");
 112 |         assert_eq!(stats.side_to_move, board.to_move());
 113 |         assert_eq!(stats.visits, expected_visits);
 114 |         assert_eq!(
 115 |             stats.visits.iter().map(|(_, n)| n).sum::<u32>(),
 116 |             simulations
 117 |         );
 118 |         assert_eq!(stats.prior, expected_prior);
 119 |         assert_eq!(stats.value, expected_value);
 120 |         assert_eq!(player_rng.random::<u64>(), manual_rng.random::<u64>());
 121 |     }
 122 | }
```

**COMMENT — D2.** The most laborious test in the crate, and the one
that makes "unchanged behavior" precise. For each of four option
tuples — cold/hot temperature, with and without noise, including the
zero-simulation edge — it runs the pre-player recipe by hand
(`search` → `add_dirichlet_noise` → `visit_distribution` →
`select_move` with a paired seed-42 RNG) and then lets the player
decide the same board with the same seed. Asserts: same move (legal),
same sorted visits, visit sum equal to the simulation count, same
sorted (noised) priors, same value, and identical next RNG draws.
The tuple `(0, 1.0, 0.25)` pins the zero-simulation value convention
(0.0, not NaN). Scope: this is equivalence of *pipelines*, proven on
one board per tuple; it does not re-prove mcts internals, which the
mcts acceptance suite owns. `UniformEvaluator::new(0.6)` supplies a
constant value so the two runs differ only in who orchestrates, never
in what is evaluated.

D3 — evaluator persistence, with its stateful double at lines 124–139
and the test at 140–162:

```text
 124 | struct StatefulEvaluator {
 125 |     calls: u32,
 126 |     observed: Rc<RefCell<Vec<u32>>>,
 127 | }
 128 | 
 129 | impl Evaluator for StatefulEvaluator {
 130 |     fn evaluate(&mut self, _: EvalRequest) -> EvalResult {
 131 |         self.calls += 1;
 132 |         self.observed.borrow_mut().push(self.calls);
 133 |         EvalResult {
 134 |             policy: [0.0; 225],
 135 |             value: 0.1,
 136 |         }
 137 |     }
 138 | }
 139 | 
```

```text
 140 | #[test]
 141 | fn evaluator_instance_persists_across_two_decisions() {
 142 |     let observed = Rc::new(RefCell::new(Vec::new()));
 143 |     let mut player = AiPlayer::new(Box::new(StatefulEvaluator {
 144 |         calls: 0,
 145 |         observed: Rc::clone(&observed),
 146 |     }));
 147 |     let options = PlayerOptions {
 148 |         simulations: 1,
 149 |         ..PlayerOptions::default()
 150 |     };
 151 |     let mut board = Board::new();
 152 |     let mut rng = StdRng::seed_from_u64(9);
 153 | 
 154 |     let first = player.decide(&board, &options, &mut rng).unwrap();
 155 |     assert_eq!(*observed.borrow(), [1, 2]);
 156 |     board.play(first.mv).unwrap();
 157 |     let second = player.decide(&board, &options, &mut rng).unwrap();
 158 | 
 159 |     assert!(board.is_legal(second.mv));
 160 |     assert_eq!(*observed.borrow(), [1, 2, 3, 4]);
 161 | }
 162 | 
```

**COMMENT — D3.** The double records its own call count into a shared
vector. Two decisions on two boards must produce `[1, 2, 3, 4]` — one
simulation per decision expands root plus one leaf, hence two
evaluations each. A fresh evaluator per decision would read
`[1, 2]` twice. Scope: object identity across decisions (the evaluator
is *the same instance*), not any quality property of persistence.
`RefCell` borrow rules are exercised at runtime; a borrow across the
decide call would panic, which is itself informative.

D4 — absolute color with White to move, lines 163–208:

```text
 163 | #[test]
 164 | fn white_own_proof_bypasses_search_and_certifies_absolute_color() {
 165 |     // White-to-move mirror of the Black VCF fixture: the same attacking
 166 |     // shape with colors swapped, plus one inert Black stone at (14, 14) so
 167 |     // Black has exactly one more stone — a legal alternating position with
 168 |     // White to move. Pins absolute-color correctness of the own probe: the
 169 |     // certificate must name White, not just "the side that was searched".
 170 |     let mut board = Board::new();
 171 |     for (row, col) in [
 172 |         (7, 3),
 173 |         (7, 4),
 174 |         (0, 0),
 175 |         (7, 5),
 176 |         (0, 2),
 177 |         (7, 6),
 178 |         (0, 4),
 179 |         (6, 6),
 180 |         (0, 6),
 181 |         (8, 8),
 182 |         (14, 14),
 183 |     ] {
 184 |         board.play(Move::new(row, col).unwrap()).unwrap();
 185 |     }
 186 |     assert_eq!(board.to_move(), engine::Color::White);
 187 |     let proof = engine::prove_forced_win(&board, board.to_move(), OwnTssConfig::baseline().budget)
 188 |         .expect("White has the mirrored VCF");
 189 |     assert_eq!(proof.winner, engine::Color::White);
 190 |     assert_eq!(proof.line[0], Move::new(7, 7).unwrap());
 191 | 
 192 |     let mut player = AiPlayer::new(Box::new(NeverEvaluate));
 193 |     let options = PlayerOptions {
 194 |         simulations: 100,
 195 |         own: OwnTssConfig::baseline(),
 196 |         ..PlayerOptions::default()
 197 |     };
 198 |     let mut rng = StdRng::seed_from_u64(19);
 199 |     let mut untouched_rng = StdRng::seed_from_u64(19);
 200 | 
 201 |     let decision = player.decide(&board, &options, &mut rng).unwrap();
 202 | 
 203 |     assert_eq!(decision.mv, Move::new(7, 7).unwrap());
 204 |     assert!(decision.search.is_none());
 205 |     assert_eq!(decision.guard.outcome, GuardOutcome::ProvenWin);
 206 |     assert_eq!(rng.random::<u64>(), untouched_rng.random::<u64>());
 207 | }
 208 | 
```

**COMMENT — D4.** The mirror fixture is a *legal alternating* position
built move by move: six Black stones, five White stones, White to move
(the inert Black stone at (14, 14) makes the counts legal). The test
first checks the fixture against the real engine — proof winner is
White, first move (7, 7) — so the assertions below rest on a verified
certificate, not on hope. Then a panicking-evaluator player with the
own probe enabled decides: certified move, no statistics, `ProvenWin`,
zero RNG draws. Why this test exists when D6 exists: the probe reads
`board.to_move()` at one instant, and a color-symmetric bug (certifying
"the side that was searched" rather than the actual side) passes every
Black-to-move test. Scope: one mirrored shape; the prover's own suite
covers shape generality.

D5 — configuration presets, lines 209–233:

```text
 209 | #[test]
 210 | fn own_tss_config_defaults_and_presets() {
 211 |     assert_eq!(PlayerOptions::default().own, OwnTssConfig::disabled());
 212 |     assert_eq!(
 213 |         OwnTssConfig::baseline(),
 214 |         OwnTssConfig {
 215 |             enabled: true,
 216 |             budget: engine::SearchBudget {
 217 |                 max_nodes: 10_000,
 218 |                 max_depth: 30,
 219 |             },
 220 |         }
 221 |     );
 222 |     assert_eq!(
 223 |         OwnTssConfig::disabled(),
 224 |         OwnTssConfig {
 225 |             enabled: false,
 226 |             ..OwnTssConfig::baseline()
 227 |         }
 228 |     );
 229 |     assert_eq!(OwnTssConfig::default(), OwnTssConfig::disabled());
 230 |     assert_eq!(GuardConfig::default(), GuardConfig::disabled());
 231 |     assert!(!GuardConfig::default().enabled);
 232 | }
 233 | 
```

**COMMENT — D5.** Pure value assertions: the default options disable
the own probe; `baseline()` is enabled at 10,000 nodes / 30 plies (the
provenance numbers, now pinned at their new owner); `disabled()` keeps
those bounds; `Default` agrees; the guard's default is disabled.
No behavior is executed — this test is the tripwire that fails if a
default ever silently changes.

D6 — the certified bypass. The shared VCF fixture (lines 234–254),
then the test (255–281):

```text
 234 | /// The win-in-3 VCF fixture from the engine prover tests: Black to move
 235 | /// forces a win starting with (7, 7).
 236 | fn vcf_fixture() -> Board {
 237 |     let mut board = Board::new();
 238 |     for (row, col) in [
 239 |         (7, 4),
 240 |         (7, 3),
 241 |         (7, 5),
 242 |         (0, 0),
 243 |         (7, 6),
 244 |         (0, 2),
 245 |         (6, 6),
 246 |         (0, 4),
 247 |         (8, 8),
 248 |         (0, 6),
 249 |     ] {
 250 |         board.play(Move::new(row, col).unwrap()).unwrap();
 251 |     }
 252 |     board
 253 | }
 254 | 
```

```text
 255 | #[test]
 256 | fn own_proof_bypasses_search_stats_and_rng() {
 257 |     let board = vcf_fixture();
 258 |     let certified = Move::new(7, 7).unwrap();
 259 |     let mut player = AiPlayer::new(Box::new(NeverEvaluate));
 260 |     let options = PlayerOptions {
 261 |         simulations: 100,
 262 |         temperature: 1.0,
 263 |         noise_epsilon: 0.25,
 264 |         own: OwnTssConfig::baseline(),
 265 |         guard: GuardConfig::baseline(),
 266 |         ..PlayerOptions::default()
 267 |     };
 268 |     let mut rng = StdRng::seed_from_u64(8);
 269 |     let mut untouched_rng = StdRng::seed_from_u64(8);
 270 | 
 271 |     let decision = player.decide(&board, &options, &mut rng).unwrap();
 272 | 
 273 |     assert_eq!(decision.mv, certified);
 274 |     assert!(decision.search.is_none());
 275 |     assert_eq!(decision.guard.outcome, GuardOutcome::ProvenWin);
 276 |     assert_eq!(decision.guard.proposal, certified);
 277 |     assert_eq!(decision.guard.checked, 0);
 278 |     assert!(decision.guard.refuted.is_empty());
 279 |     assert_eq!(rng.random::<u64>(), untouched_rng.random::<u64>());
 280 | }
 281 | 
```

**COMMENT — the fixture and D6.** The fixture is the win-in-3 VCF
script from the engine prover's own tests: ten replayed plies, Black
to move, forced win starting (7, 7). D6 deliberately enables
*everything* — 100 simulations, temperature 1.0, noise 0.25, and the
guard — so that the bypass is observed against maximal interference:
certified move returned, `search` is `None`, guard reports `ProvenWin`
with the proposal equal to the certified move and **zero** checks,
refuted list empty, RNG untouched, and the panicking evaluator never
fired. Assertion scope: the ordering "probe precedes search, noise,
selection, guard" — the single strongest behavioral pin of this
chapter. It replaces the legacy hook test
`root_proof_has_no_search_stats_leaf_calls_or_rng_consumption`, which
injected a fabricated certificate through the hook; here the
certificate comes from the real engine prover.

D7 and D8 need a counting double (lines 282–292):

```text
 282 | struct CountingEvaluator {
 283 |     calls: Rc<Cell<u32>>,
 284 | }
 285 | 
 286 | impl Evaluator for CountingEvaluator {
 287 |     fn evaluate(&mut self, request: EvalRequest) -> EvalResult {
 288 |         self.calls.set(self.calls.get() + 1);
 289 |         UniformEvaluator::new(0.0).evaluate(request)
 290 |     }
 291 | }
 292 | 
```

**COMMENT — `CountingEvaluator`.** Wraps a `UniformEvaluator` and
counts calls through a shared `Cell<u32>` (interior mutability, so the
count can be shared with the test without a mutable borrow of the
evaluator). Delegating to a real uniform evaluation keeps the wrapped
behavior honest — the counter changes nothing the search sees.

D7 — the gate is real, lines 293–317:

```text
 293 | #[test]
 294 | fn disabled_own_probe_searches_vcf_position() {
 295 |     let board = vcf_fixture();
 296 |     let calls = Rc::new(Cell::new(0));
 297 |     let mut player = AiPlayer::new(Box::new(CountingEvaluator {
 298 |         calls: Rc::clone(&calls),
 299 |     }));
 300 |     let options = PlayerOptions {
 301 |         simulations: 9,
 302 |         own: OwnTssConfig::disabled(),
 303 |         ..PlayerOptions::default()
 304 |     };
 305 |     let mut rng = StdRng::seed_from_u64(11);
 306 | 
 307 |     let decision = player.decide(&board, &options, &mut rng).unwrap();
 308 | 
 309 |     assert!(board.is_legal(decision.mv));
 310 |     assert!(calls.get() > 0, "disabled probe must really search");
 311 |     let stats = decision
 312 |         .search
 313 |         .expect("ordinary search has real statistics");
 314 |     assert_eq!(stats.visits.iter().map(|(_, n)| n).sum::<u32>(), 9);
 315 |     assert_eq!(decision.guard.outcome, GuardOutcome::Disabled);
 316 | }
 317 | 
```

**COMMENT — D7.** Same proven position, probe disabled: the decision
must *search* — evaluator called, nine visits recorded across the root
edges, guard untouched (`Disabled`). This pins that `enabled: false`
is an actual gate and the certified shortcut is not a default
behavior. Scope: it does not assert which move is chosen (the uniform
search's choice is the mcts crate's business), only that a real search
happened and its statistics are honest.

D8 — exhausted budget is unknown, lines 318–348:

```text
 318 | #[test]
 319 | fn zero_budget_own_probe_falls_through_to_search() {
 320 |     let board = vcf_fixture();
 321 |     let calls = Rc::new(Cell::new(0));
 322 |     let mut player = AiPlayer::new(Box::new(CountingEvaluator {
 323 |         calls: Rc::clone(&calls),
 324 |     }));
 325 |     let options = PlayerOptions {
 326 |         simulations: 5,
 327 |         own: OwnTssConfig {
 328 |             enabled: true,
 329 |             budget: engine::SearchBudget {
 330 |                 max_nodes: 0,
 331 |                 max_depth: 30,
 332 |             },
 333 |         },
 334 |         ..PlayerOptions::default()
 335 |     };
 336 |     let mut rng = StdRng::seed_from_u64(13);
 337 | 
 338 |     let decision = player.decide(&board, &options, &mut rng).unwrap();
 339 | 
 340 |     assert!(board.is_legal(decision.mv));
 341 |     assert!(
 342 |         calls.get() > 0,
 343 |         "an exhausted probe is unknown, so search runs"
 344 |     );
 345 |     assert!(decision.search.is_some());
 346 |     assert_eq!(decision.guard.outcome, GuardOutcome::Disabled);
 347 | }
 348 | 
```

**COMMENT — D8.** Probe enabled with `max_nodes: 0`: the real engine
prover can expand nothing and returns `None`; `decide` must fall
through to search. The assertion pair — evaluator called, statistics
present — pins that "no proof" never means "proven loss" and never
disables the ordinary path. The same fall-through covers admission
rejections (U1 exercises those at the predicate level).

D9 — no short-circuit of a later quiet root, lines 349–392:

```text
 349 | #[test]
 350 | fn proven_decision_does_not_short_circuit_a_later_quiet_root() {
 351 |     // Structural invariant (code-review-verifiable): AiPlayer holds no state
 352 |     // besides the evaluator, so there is nothing that could cache a proof.
 353 |     // This test pins the dynamic half: after a proven decision on the VCF
 354 |     // fixture, a separate quiet board is really searched by the same player.
 355 |     let vcf = vcf_fixture();
 356 |     let certified = Move::new(7, 7).unwrap();
 357 |     let calls = Rc::new(Cell::new(0));
 358 |     let mut player = AiPlayer::new(Box::new(CountingEvaluator {
 359 |         calls: Rc::clone(&calls),
 360 |     }));
 361 |     let options = PlayerOptions {
 362 |         simulations: 5,
 363 |         own: OwnTssConfig::baseline(),
 364 |         ..PlayerOptions::default()
 365 |     };
 366 |     let mut rng = StdRng::seed_from_u64(17);
 367 | 
 368 |     let proven = player.decide(&vcf, &options, &mut rng).unwrap();
 369 |     assert_eq!(proven.mv, certified);
 370 |     assert!(proven.search.is_none());
 371 |     assert_eq!(calls.get(), 0, "the proof path must not evaluate");
 372 | 
 373 |     // A separate, known-quiet board: one Black stone at (7, 7), White to
 374 |     // move. No forced win exists within any budget, and the certified move
 375 |     // from the previous decision is not even legal here.
 376 |     let mut quiet = Board::new();
 377 |     quiet.play(certified).unwrap();
 378 |     assert!(!quiet.is_legal(certified));
 379 | 
 380 |     let searched = player.decide(&quiet, &options, &mut rng).unwrap();
 381 | 
 382 |     assert!(calls.get() > 0, "the quiet root must really be searched");
 383 |     let stats = searched.search.expect("the quiet root has real statistics");
 384 |     assert_eq!(stats.side_to_move, quiet.to_move());
 385 |     assert_eq!(stats.visits.iter().map(|(_, n)| n).sum::<u32>(), 5);
 386 |     assert!(quiet.is_legal(searched.mv));
 387 |     assert_ne!(
 388 |         searched.mv, certified,
 389 |         "no stale proof move from the earlier decision"
 390 |     );
 391 |     assert_eq!(searched.guard.outcome, GuardOutcome::Disabled);
 392 | }
```

**COMMENT — D9.** The "no caching" evidence policy made executable.
Decide the VCF fixture (certified; zero evaluations — the counting
double asserts the proof path really skipped the evaluator), then play
into a *separate, known-quiet* board: one Black stone at (7, 7), White
to move, where the previously certified move is not even legal. The
second decision must search (calls > 0), must carry real statistics
(five visits, White's perspective), must be legal, and must not equal
the stale certified move. What this proves: a certified decision does
not suppress or contaminate a later search by the same player. What it
does not prove (and the in-code comment says so): call-count discipline
of the prover itself — that is structural (single call site, no state
fields), accepted as code-review-verifiable evidence rather than
dynamic counting, because the own probe has no injection seam by
design.

### Admission — `crates/player/src/lib.rs` unit test

The private predicate is tested from the same module (no widened
visibility). Setup at lines 264–290 (module header plus its own copy
of the VCF fixture), the test at 291–331:

```text
 264 | #[cfg(test)]
 265 | mod tests {
 266 |     use super::*;
 267 |     use engine::{Color, Move, Proof};
 268 | 
 269 |     /// The win-in-3 VCF fixture from the engine prover tests: Black to move
 270 |     /// forces a win starting with (7, 7).
 271 |     fn vcf_fixture() -> Board {
 272 |         let mut board = Board::new();
 273 |         for (row, col) in [
 274 |             (7, 4),
 275 |             (7, 3),
 276 |             (7, 5),
 277 |             (0, 0),
 278 |             (7, 6),
 279 |             (0, 2),
 280 |             (6, 6),
 281 |             (0, 4),
 282 |             (8, 8),
 283 |             (0, 6),
 284 |         ] {
 285 |             board.play(Move::new(row, col).unwrap()).unwrap();
 286 |         }
 287 |         board
 288 |     }
 289 | 
 290 |     #[test]
```

```text
 291 |     fn admission_rejects_wrong_winner_empty_line_illegal_first_move_and_unverified_line() {
 292 |         let board = vcf_fixture();
 293 |         let real =
 294 |             engine::prove_forced_win(&board, board.to_move(), OwnTssConfig::baseline().budget)
 295 |                 .expect("the engine proves this fixture within the baseline budget");
 296 | 
 297 |         // A genuine, verified proof admits its first move.
 298 |         assert_eq!(
 299 |             certified_own_move(&board, &real),
 300 |             Some(Move::new(7, 7).unwrap())
 301 |         );
 302 | 
 303 |         // Wrong winner: the opponent cannot own our certificate.
 304 |         let wrong_winner = Proof {
 305 |             winner: Color::White,
 306 |             line: real.line.clone(),
 307 |         };
 308 |         assert_eq!(certified_own_move(&board, &wrong_winner), None);
 309 | 
 310 |         // Empty line: no move to play, nothing to admit.
 311 |         let empty = Proof {
 312 |             winner: Color::Black,
 313 |             line: Vec::new(),
 314 |         };
 315 |         assert_eq!(certified_own_move(&board, &empty), None);
 316 | 
 317 |         // Illegal first move: (0, 0) is already occupied in the fixture.
 318 |         let illegal_first = Proof {
 319 |             winner: Color::Black,
 320 |             line: vec![Move::new(0, 0).unwrap()],
 321 |         };
 322 |         assert_eq!(certified_own_move(&board, &illegal_first), None);
 323 | 
 324 |         // Unverified lines: the real proof, truncated or appended to.
 325 |         let mut truncated = real.clone();
 326 |         truncated.line.pop();
 327 |         assert_eq!(certified_own_move(&board, &truncated), None);
 328 |         let mut appended = real.clone();
 329 |         appended.line.push(Move::new(14, 14).unwrap());
 330 |         assert_eq!(certified_own_move(&board, &appended), None);
 331 |     }
```

**COMMENT — U1.** Arrange/act/assert in one sweep: a *real* engine
proof on the fixture admits its first move (positive control — without
it, four passing rejections could hide a predicate that rejects
everything); then four fabricated `Proof` values isolate the four
rules — wrong winner, empty line, illegal first move ((0, 0) is
occupied in the fixture), and unverified lines. The last rule is
exercised twice with the *real* proof doctored (truncated, appended),
which pins that `verify_line` is actually consulted: both mutations
keep the correct winner and a legal first move, so only the verifier
can reject them. Fabrication is possible because `Proof`'s fields are
public — the engine exports the record, the player decides whether to
trust it. Scope: predicate-level coverage of the rejection matrix; the
end-to-end admission is D6/D4.

### Guard, public — `crates/player/tests/guard.rs`

Lines 1–45 define the shared fixtures: a `mv` shorthand, a
`Board::from_position` helper, the single-threat position (White's
four on row 7 has one winning endpoint at (7, 7); Black must block),
a `Prefer` evaluator that forces one policy peak, and the options
helper whose comment records the isolation rule — **the own probe
stays disabled in every guard test** (via `PlayerOptions::default`),
because several fixtures contain own immediate wins that an enabled
probe would certify before the guard runs:

```text
   1 | use engine::{Board, Color, Move, SearchBudget, Status};
   2 | use mcts::{EvalRequest, EvalResult, Evaluator, UniformEvaluator};
   3 | use player::{AiPlayer, GuardConfig, GuardOutcome, OwnTssConfig, PlayerOptions};
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
  26 | 
  27 | struct Prefer(Move);
  28 | impl Evaluator for Prefer {
  29 |     fn evaluate(&mut self, _: EvalRequest) -> EvalResult {
  30 |         let mut policy = [0.0; 225];
  31 |         policy[self.0.index()] = 10.0;
  32 |         EvalResult { policy, value: 0.0 }
  33 |     }
  34 | }
  35 | 
  36 | // Guard fixtures may contain own immediate wins; the own probe stays
  37 | // disabled (via `PlayerOptions::default`) so these tests isolate the guard.
  38 | fn options(guard: GuardConfig) -> PlayerOptions {
  39 |     PlayerOptions {
  40 |         simulations: 0,
  41 |         guard,
  42 |         ..PlayerOptions::default()
  43 |     }
  44 | }
  45 | 
```

**COMMENT — guard fixtures.** `simulations: 0` in the options helper is
deliberate: with zero simulations the search expands the root and runs
no loop, so every root edge is unvisited and the proposal is decided by
priors alone — which is what `Prefer` controls, making the proposal
deterministic without a single simulation. The guard tests therefore
observe guard logic, not search dynamics.

G1 — opt-in and bounds, lines 46–60:

```text
  46 | #[test]
  47 | fn baseline_guard_is_opt_in_with_fixed_experiment_work_bounds() {
  48 |     assert_eq!(PlayerOptions::default().guard, GuardConfig::disabled());
  49 |     let baseline = GuardConfig::baseline();
  50 |     assert!(baseline.enabled);
  51 |     assert_eq!(baseline.max_candidates, 5);
  52 |     assert_eq!(
  53 |         baseline.budget,
  54 |         SearchBudget {
  55 |             max_nodes: 1_000,
  56 |             max_depth: 10
  57 |         }
  58 |     );
  59 | }
  60 | 
```

**COMMENT — G1.** The configuration tripwire: default disabled;
baseline enabled with five candidates and 1,000 nodes / 10 plies per
probe. Value-level, like D5.

G2 — the central refutation story, lines 61–103:

```text
  61 | #[test]
  62 | fn real_proof_refutes_proposal_and_guard_finds_unvisited_block_without_mutating_board_or_stats() {
  63 |     let board = single_threat();
  64 |     let original = board.clone();
  65 |     let block = mv(7, 7);
  66 |     let mut disabled = AiPlayer::new(Box::new(Prefer(block)));
  67 |     let mut enabled = AiPlayer::new(Box::new(Prefer(block)));
  68 |     let initial = disabled
  69 |         .decide(
  70 |             &board,
  71 |             &options(GuardConfig::disabled()),
  72 |             &mut StdRng::seed_from_u64(1),
  73 |         )
  74 |         .unwrap();
  75 |     assert_eq!(initial.mv, mv(0, 0));
  76 |     let mut hypothetical = board.clone();
  77 |     hypothetical.play(initial.mv).unwrap();
  78 |     let opponent = hypothetical.to_move();
  79 |     let proof =
  80 |         engine::prove_forced_win(&hypothetical, opponent, GuardConfig::baseline().budget).unwrap();
  81 |     assert_eq!(proof.winner, Color::White);
  82 |     assert!(!proof.line.is_empty());
  83 |     assert!(hypothetical.is_legal(proof.line[0]));
  84 |     assert!(engine::verify_line(&hypothetical, &proof));
  85 | 
  86 |     let guarded = enabled
  87 |         .decide(
  88 |             &board,
  89 |             &options(GuardConfig::baseline()),
  90 |             &mut StdRng::seed_from_u64(1),
  91 |         )
  92 |         .unwrap();
  93 |     assert_eq!(guarded.mv, block);
  94 |     assert_eq!(guarded.guard.proposal, initial.mv);
  95 |     assert_eq!(guarded.guard.refuted, vec![initial.mv]);
  96 |     assert_eq!(guarded.guard.checked, 2);
  97 |     assert_eq!(guarded.guard.outcome, GuardOutcome::Unrefuted);
  98 |     assert_eq!(guarded.search, initial.search);
  99 |     assert_eq!(board, original);
 100 |     assert_eq!(board.zobrist(), original.zobrist());
 101 |     assert_eq!(initial.guard.outcome, GuardOutcome::Disabled);
 102 | }
 103 | 
```

**COMMENT — G2.** Two seeded-identical decisions on the single-threat
fixture, guard off then on. The disabled decision proposes (0, 0):
with zero simulations every visit count is zero, `select_move` at
temperature 0 takes argmax over visits, and the tie breaks to the
lowest move index. The `Prefer` peak on the block (7, 7) raises only
its *prior* — which `select_move` never reads — so the peak's real
role appears once the guard runs: it makes the block the
highest-ranked retry candidate. The test then verifies the threat manually
against the real engine — winner White, non-empty line, legal start,
verified line — so the guard's later verdict rests on an independently
established fact. Guarded: the proposal is refuted, the unvisited
block (7, 7) is found and returned, `checked == 2` (proposal plus one
retry), outcome `Unrefuted` (the block is *unknown*, not certified
safe), and the search statistics are identical to the disabled run —
the guard substituted the move without touching the record. Board
equality *and* Zobrist equality close the no-mutation pin. Scope: one
fixture; the ranking and cap machinery behind it is P1's domain.

G3 — two threats need an immediate win, lines 104–127:

```text
 104 | #[test]
 105 | fn two_opponent_winning_threats_require_our_immediate_winning_alternative() {
 106 |     let board = position(
 107 |         &[(7, 2), (7, 3), (7, 4), (7, 5)],
 108 |         &[(9, 3), (9, 4), (9, 5), (9, 6)],
 109 |     );
 110 |     let winning = mv(7, 6);
 111 |     let mut after_bad = board.clone();
 112 |     after_bad.play(mv(0, 0)).unwrap();
 113 |     assert_eq!(engine::immediate_wins(&after_bad, Color::White).len(), 2);
 114 |     let decision = AiPlayer::new(Box::new(Prefer(winning)))
 115 |         .decide(
 116 |             &board,
 117 |             &options(GuardConfig::baseline()),
 118 |             &mut StdRng::seed_from_u64(1),
 119 |         )
 120 |         .unwrap();
 121 |     assert_eq!(decision.guard.proposal, mv(0, 0));
 122 |     assert_eq!(decision.guard.refuted, vec![mv(0, 0)]);
 123 |     assert_eq!(decision.mv, winning);
 124 |     assert_eq!(decision.guard.outcome, GuardOutcome::Terminal);
 125 |     assert_eq!(decision.guard.checked, 2);
 126 | }
 127 | 
```

**COMMENT — G3.** After a bad Black move, White has *two* immediate
wins (asserted through `engine::immediate_wins` on the hypothetical),
so any single block is refutable. The guard refutes the proposal and
lands on Black's own immediate win (7, 6) — returned as `Terminal`
via the fastpath, never probed. Pins the interplay of the refutation
loop and the terminal fastpath in one decision.

G4 — distinct lines, lines 128–212:

```text
 128 | #[test]
 129 | fn blocking_one_of_two_distinct_opponent_lines_is_refuted_in_favor_of_own_immediate_win() {
 130 |     // White's horizontal b1-e1 needs a1 (f1 is Black); its vertical
 131 |     // k2-k5 needs k1 (k6 is Black). Blocking one does not cover the other.
 132 |     let board = position(
 133 |         &[
 134 |             (0, 5),
 135 |             (5, 10),
 136 |             (13, 5),
 137 |             (13, 7),
 138 |             (9, 1),
 139 |             (9, 2),
 140 |             (9, 3),
 141 |             (9, 4),
 142 |         ],
 143 |         &[
 144 |             (0, 1),
 145 |             (0, 2),
 146 |             (0, 3),
 147 |             (0, 4),
 148 |             (1, 10),
 149 |             (2, 10),
 150 |             (3, 10),
 151 |             (4, 10),
 152 |         ],
 153 |     );
 154 |     let original = board.clone();
 155 |     let proposal = mv(0, 0); // a1: lowest-index legal move at zero visits.
 156 |     let remaining_win = mv(0, 10); // k1: the distinct vertical line.
 157 |     let winning = mv(9, 5); // f10: Black completes b10-f10.
 158 |     let guard = GuardConfig {
 159 |         enabled: true,
 160 |         max_candidates: 2,
 161 |         budget: SearchBudget {
 162 |             max_nodes: 1_000,
 163 |             max_depth: 10,
 164 |         },
 165 |     };
 166 |     assert_eq!(
 167 |         engine::immediate_wins(&board, Color::White)
 168 |             .iter()
 169 |             .collect::<Vec<_>>(),
 170 |         vec![proposal, remaining_win]
 171 |     );
 172 |     let mut after_block = board.clone();
 173 |     after_block.play(proposal).unwrap();
 174 |     assert_eq!(
 175 |         engine::immediate_wins(&after_block, Color::White)
 176 |             .iter()
 177 |             .collect::<Vec<_>>(),
 178 |         vec![remaining_win]
 179 |     );
 180 |     let proof =
 181 |         engine::prove_forced_win(&after_block, after_block.to_move(), guard.budget).unwrap();
 182 |     assert_eq!(proof.winner, Color::White);
 183 |     assert_eq!(proof.line, vec![remaining_win]);
 184 |     assert!(engine::verify_line(&after_block, &proof));
 185 | 
 186 |     let mut plain_rng = StdRng::seed_from_u64(1);
 187 |     let mut guarded_rng = StdRng::seed_from_u64(1);
 188 |     let plain = AiPlayer::new(Box::new(Prefer(winning)))
 189 |         .decide(&board, &options(GuardConfig::disabled()), &mut plain_rng)
 190 |         .unwrap();
 191 |     let guarded = AiPlayer::new(Box::new(Prefer(winning)))
 192 |         .decide(&board, &options(guard), &mut guarded_rng)
 193 |         .unwrap();
 194 |     assert_eq!(plain.mv, proposal);
 195 |     assert_eq!(plain.guard.outcome, GuardOutcome::Disabled);
 196 |     let stats = plain.search.as_ref().unwrap();
 197 |     assert!(stats.visits.iter().all(|&(_, visits)| visits == 0));
 198 |     assert_eq!(stats.prior[0].0, winning);
 199 |     assert_eq!(guarded.guard.proposal, proposal);
 200 |     assert_eq!(guarded.guard.refuted, vec![proposal]);
 201 |     assert_eq!(guarded.guard.checked, 2);
 202 |     assert_eq!(guarded.guard.outcome, GuardOutcome::Terminal);
 203 |     assert_eq!(guarded.mv, winning);
 204 |     assert_eq!(guarded.search, plain.search);
 205 |     assert_eq!(plain_rng.random::<u64>(), guarded_rng.random::<u64>());
 206 |     let mut after_win = board.clone();
 207 |     after_win.play(guarded.mv).unwrap();
 208 |     assert_eq!(after_win.status(), Status::Won(Color::Black));
 209 |     assert_eq!(board, original);
 210 |     assert_eq!(board.zobrist(), original.zobrist());
 211 | }
 212 | 
```

**COMMENT — G4.** The deepest public guard test. White owns two
*distinct* winning lines (horizontal needing a1, vertical needing k1);
blocking one leaves the other. The test establishes the geometry with
direct engine assertions first — two immediate wins at the root, one
remaining after the block, a real proof of the remainder, verified —
then runs plain and guarded decisions with paired seeds. Asserts:
proposal is the block (lowest-index legal move at zero visits), the
block is refuted, Black's own immediate win f10 is found via the
terminal fastpath, statistics identical between runs, RNG streams
identical (the guard consumes no randomness), the winning move really
wins on a replayed clone, and the board is unmutated. Note the cap of
2: enough for proposal plus one retry, keeping the experiment small by
construction.

G5 — the guard probes the true opponent, lines 213–235:

```text
 213 | #[test]
 214 | fn white_player_guard_probes_actual_black_opponent() {
 215 |     let board = Board::from_position(
 216 |         &[mv(7, 3), mv(7, 4), mv(7, 5), mv(7, 6), mv(14, 14)],
 217 |         &[mv(7, 2), mv(2, 0), mv(2, 2), mv(2, 4)],
 218 |         Color::White,
 219 |     )
 220 |     .unwrap();
 221 |     let block = mv(7, 7);
 222 |     let original = board.clone();
 223 |     let decision = AiPlayer::new(Box::new(Prefer(block)))
 224 |         .decide(
 225 |             &board,
 226 |             &options(GuardConfig::baseline()),
 227 |             &mut StdRng::seed_from_u64(1),
 228 |         )
 229 |         .unwrap();
 230 |     assert_eq!(decision.guard.refuted, vec![mv(0, 0)]);
 231 |     assert_eq!(decision.mv, block);
 232 |     assert_eq!(decision.guard.outcome, GuardOutcome::Unrefuted);
 233 |     assert_eq!(board, original);
 234 | }
 235 | 
```

**COMMENT — G5.** A White-seat fixture (five Black stones, four White,
White to move — legal alternation) where Black has the threat. The
guard must probe *Black*, and the `Unrefuted` block (7, 7) returned
for White proves the probe targeted `after.to_move()` rather than a
root-relative color. The color-twin of D4, one pipeline stage later.

G6 and G7 — starved probes stay unknown, lines 236–273:

```text
 236 | #[test]
 237 | fn zero_depth_probe_is_unknown_even_with_opponent_immediate_win() {
 238 |     let board = single_threat();
 239 |     let config = GuardConfig {
 240 |         budget: SearchBudget {
 241 |             max_nodes: 1_000,
 242 |             max_depth: 0,
 243 |         },
 244 |         ..GuardConfig::baseline()
 245 |     };
 246 |     let decision = AiPlayer::new(Box::new(Prefer(mv(7, 7))))
 247 |         .decide(&board, &options(config), &mut StdRng::seed_from_u64(1))
 248 |         .unwrap();
 249 |     assert_eq!(decision.mv, mv(0, 0));
 250 |     assert_eq!(decision.guard.outcome, GuardOutcome::Unrefuted);
 251 |     assert_eq!(decision.guard.checked, 1);
 252 |     assert!(decision.guard.refuted.is_empty());
 253 | }
 254 | 
```

```text
 255 | #[test]
 256 | fn zero_node_probe_is_unknown_even_with_opponent_immediate_win() {
 257 |     let board = single_threat();
 258 |     let config = GuardConfig {
 259 |         budget: SearchBudget {
 260 |             max_nodes: 0,
 261 |             max_depth: 10,
 262 |         },
 263 |         ..GuardConfig::baseline()
 264 |     };
 265 |     let decision = AiPlayer::new(Box::new(UniformEvaluator::new(0.0)))
 266 |         .decide(&board, &options(config), &mut StdRng::seed_from_u64(1))
 267 |         .unwrap();
 268 |     assert_eq!(decision.mv, mv(0, 0));
 269 |     assert_eq!(decision.guard.outcome, GuardOutcome::Unrefuted);
 270 |     assert_eq!(decision.guard.checked, 1);
 271 |     assert!(decision.guard.refuted.is_empty());
 272 | }
 273 | 
```

**COMMENT — G6/G7.** Zero depth (G6) or zero nodes (G7) starves the
opponent probe even though the opponent *does* have an immediate win
in the fixture: the candidate comes back `Unrefuted` — unknown — with
exactly one check and an empty refuted list, and the move stays the
proposal. The pair pins that "no proof found" is never read as "no
proof exists", for both dimensions of the budget.

G8 and G9 — the cap, lines 274–306:

```text
 274 | #[test]
 275 | fn cap_falls_back_to_unchecked_block_not_refuted_original() {
 276 |     let board = single_threat();
 277 |     let block = mv(7, 7);
 278 |     let guard = GuardConfig {
 279 |         max_candidates: 1,
 280 |         ..GuardConfig::baseline()
 281 |     };
 282 |     let decision = AiPlayer::new(Box::new(Prefer(block)))
 283 |         .decide(&board, &options(guard), &mut StdRng::seed_from_u64(1))
 284 |         .unwrap();
 285 |     assert_eq!(decision.mv, block);
 286 |     assert_eq!(decision.guard.outcome, GuardOutcome::BudgetExhausted);
 287 |     assert_eq!(decision.guard.checked, 1);
 288 |     assert_eq!(decision.guard.refuted, vec![mv(0, 0)]);
 289 | }
 290 | 
```

```text
 291 | #[test]
 292 | fn zero_candidate_cap_keeps_original_unchecked() {
 293 |     let board = single_threat();
 294 |     let guard = GuardConfig {
 295 |         max_candidates: 0,
 296 |         ..GuardConfig::baseline()
 297 |     };
 298 |     let decision = AiPlayer::new(Box::new(Prefer(mv(7, 7))))
 299 |         .decide(&board, &options(guard), &mut StdRng::seed_from_u64(1))
 300 |         .unwrap();
 301 |     assert_eq!(decision.mv, mv(0, 0));
 302 |     assert_eq!(decision.guard.checked, 0);
 303 |     assert!(decision.guard.refuted.is_empty());
 304 |     assert_eq!(decision.guard.outcome, GuardOutcome::BudgetExhausted);
 305 | }
 306 | 
```

**COMMENT — G8/G9.** G8 (cap 1): after the proposal is refuted the
budget is spent, so the highest-ranked *unchecked* candidate is
returned — the block, which happens to be the right move, but the
outcome honestly reports `BudgetExhausted` (untested, not vetted).
G9 (cap 0): nothing is checked, the proposal stands, same outcome.
Together they pin both cap exits and the "never back to a known loser"
rule from §11.

G10 — every legal move refuted, lines 307–340:

```text
 307 | #[test]
 308 | fn every_legal_move_really_refuted_reports_all_refuted_and_original_fallback() {
 309 |     // Two independent winning endpoints: neither can be covered with one move.
 310 |     let board = position(
 311 |         &[(2, 0), (2, 2), (2, 4), (2, 6)],
 312 |         &[(7, 3), (7, 4), (7, 5), (7, 6)],
 313 |     );
 314 |     let original = board.clone();
 315 |     let guard = GuardConfig {
 316 |         max_candidates: board.empty_moves().count(),
 317 |         budget: SearchBudget {
 318 |             max_nodes: 1,
 319 |             max_depth: 1,
 320 |         },
 321 |         ..GuardConfig::baseline()
 322 |     };
 323 |     let decision = AiPlayer::new(Box::new(UniformEvaluator::new(0.0)))
 324 |         .decide(&board, &options(guard), &mut StdRng::seed_from_u64(1))
 325 |         .unwrap();
 326 |     assert_eq!(decision.guard.outcome, GuardOutcome::AllRefuted);
 327 |     assert_eq!(decision.guard.checked, board.empty_moves().count());
 328 |     assert_eq!(decision.guard.refuted.len(), board.empty_moves().count());
 329 |     for candidate in board.empty_moves() {
 330 |         assert!(decision.guard.refuted.contains(&candidate));
 331 |         let mut after = board.clone();
 332 |         after.play(candidate).unwrap();
 333 |         let proof = engine::prove_forced_win(&after, after.to_move(), guard.budget).unwrap();
 334 |         assert!(engine::verify_line(&after, &proof));
 335 |     }
 336 |     assert_eq!(decision.mv, decision.guard.proposal);
 337 |     assert!(board.is_legal(decision.mv));
 338 |     assert_eq!(board, original);
 339 | }
 340 | 
```

**COMMENT — G10.** The two-ended-four fixture gives White two
independent winning endpoints, so *every* Black move is refutable
within a tiny (1 node / 1 ply — enough for immediate-win lines)
budget; the cap is set to the full legal-move count. Asserts:
`AllRefuted`, checked and refuted counts equal the empty-square count,
and then — the expensive part — the test re-proves and re-verifies
every single candidate against the engine independently, so the
`AllRefuted` report is checked move by move against ground truth. The
decision retains the original proposal: not a resignation, not a loss
certificate. Board unmutated.

G11 — immediate win beats a zero-budget probe, lines 341–365:

```text
 341 | #[test]
 342 | fn own_immediate_win_is_terminal_not_opponent_probe_unknown() {
 343 |     let board = position(
 344 |         &[(0, 1), (0, 2), (0, 3), (0, 4)],
 345 |         &[(2, 0), (2, 2), (2, 4), (2, 6)],
 346 |     );
 347 |     let guard = GuardConfig {
 348 |         budget: SearchBudget {
 349 |             max_nodes: 0,
 350 |             max_depth: 0,
 351 |         },
 352 |         ..GuardConfig::baseline()
 353 |     };
 354 |     let decision = AiPlayer::new(Box::new(UniformEvaluator::new(0.0)))
 355 |         .decide(&board, &options(guard), &mut StdRng::seed_from_u64(1))
 356 |         .unwrap();
 357 |     assert_eq!(decision.mv, mv(0, 0));
 358 |     assert_eq!(decision.guard.outcome, GuardOutcome::Terminal);
 359 |     assert_eq!(decision.guard.checked, 1);
 360 |     assert!(decision.guard.refuted.is_empty());
 361 |     let mut after = board.clone();
 362 |     after.play(decision.mv).unwrap();
 363 |     assert_eq!(after.status(), Status::Won(Color::Black));
 364 | }
 365 | 
```

**COMMENT — G11.** The terminal fastpath precedes the probe, so even a
completely starved budget (0 nodes, 0 plies) cannot turn an immediate
own win into `Unrefuted`: the outcome is `Terminal` with one check and
no probe call. The replayed clone confirms the win is real.

G12 — unknown outcome changes nothing, lines 366–401:

```text
 366 | #[test]
 367 | fn enabled_unknown_preserves_seeded_first_candidate_noise_stats_and_rng() {
 368 |     let board = Board::new();
 369 |     let original = board.clone();
 370 |     for simulations in [0, 11] {
 371 |         let mut plain_options = options(GuardConfig::disabled());
 372 |         plain_options.simulations = simulations;
 373 |         plain_options.temperature = 0.7;
 374 |         plain_options.noise_epsilon = 0.25;
 375 |         let guarded_options = PlayerOptions {
 376 |             guard: GuardConfig {
 377 |                 budget: SearchBudget {
 378 |                     max_nodes: 0,
 379 |                     max_depth: 10,
 380 |                 },
 381 |                 ..GuardConfig::baseline()
 382 |             },
 383 |             ..plain_options
 384 |         };
 385 |         let mut plain_rng = StdRng::seed_from_u64(42);
 386 |         let mut guarded_rng = StdRng::seed_from_u64(42);
 387 |         let plain = AiPlayer::new(Box::new(UniformEvaluator::new(0.3)))
 388 |             .decide(&board, &plain_options, &mut plain_rng)
 389 |             .unwrap();
 390 |         let guarded = AiPlayer::new(Box::new(UniformEvaluator::new(0.3)))
 391 |             .decide(&board, &guarded_options, &mut guarded_rng)
 392 |             .unwrap();
 393 |         assert_eq!(guarded.mv, plain.mv);
 394 |         assert_eq!(guarded.guard.proposal, plain.mv);
 395 |         assert_eq!(guarded.guard.outcome, GuardOutcome::Unrefuted);
 396 |         assert_eq!(guarded.search, plain.search);
 397 |         assert_eq!(plain_rng.random::<u64>(), guarded_rng.random::<u64>());
 398 |         assert_eq!(board, original);
 399 |     }
 400 | }
 401 | 
```

**COMMENT — G12.** On an empty board with a starved probe, plain and
guarded runs (two simulation counts, temperature and noise enabled)
must agree on everything: move, proposal, statistics, RNG stream,
board. This is the "guard is inert when it learns nothing" pin — the
experiment may refuse to certify, but it may not perturb the decision
it failed to refute.

G13 — pipeline precedence. A local panicking evaluator and the VCF
fixture (lines 402–429), then the test (430–452):

```text
 402 | struct NeverEvaluate;
 403 | impl Evaluator for NeverEvaluate {
 404 |     fn evaluate(&mut self, _: EvalRequest) -> EvalResult {
 405 |         panic!("a certified own win must bypass search and the guard");
 406 |     }
 407 | }
 408 | 
 409 | /// The win-in-3 VCF fixture from the engine prover tests: Black to move
 410 | /// forces a win starting with (7, 7).
 411 | fn vcf_fixture() -> Board {
 412 |     let mut board = Board::new();
 413 |     for (row, col) in [
 414 |         (7, 4),
 415 |         (7, 3),
 416 |         (7, 5),
 417 |         (0, 0),
 418 |         (7, 6),
 419 |         (0, 2),
 420 |         (6, 6),
 421 |         (0, 4),
 422 |         (8, 8),
 423 |         (0, 6),
 424 |     ] {
 425 |         board.play(Move::new(row, col).unwrap()).unwrap();
 426 |     }
 427 |     board
 428 | }
 429 | 
```

```text
 430 | #[test]
 431 | fn enabled_own_probe_precedes_guard_and_search() {
 432 |     // With both probes enabled, the own probe certifies first: no search,
 433 |     // no guard candidate checks, no evaluator call.
 434 |     let board = vcf_fixture();
 435 |     let certified = mv(7, 7);
 436 |     let mut player = AiPlayer::new(Box::new(NeverEvaluate));
 437 |     let options = PlayerOptions {
 438 |         simulations: 100,
 439 |         own: OwnTssConfig::baseline(),
 440 |         guard: GuardConfig::baseline(),
 441 |         ..PlayerOptions::default()
 442 |     };
 443 |     let mut rng = StdRng::seed_from_u64(3);
 444 | 
 445 |     let decision = player.decide(&board, &options, &mut rng).unwrap();
 446 | 
 447 |     assert_eq!(decision.mv, certified);
 448 |     assert!(decision.search.is_none());
 449 |     assert_eq!(decision.guard.outcome, GuardOutcome::ProvenWin);
 450 |     assert_eq!(decision.guard.checked, 0);
 451 |     assert!(decision.guard.refuted.is_empty());
 452 | }
```

**COMMENT — G13.** Both probes enabled at baseline budgets: the own
probe certifies first — certified move, no statistics, `ProvenWin`,
zero guard checks — with a panicking evaluator proving search never
ran. This is the only guard-file test with the own probe enabled, and
it exists to pin the pipeline order itself (probe → search → guard),
which all the isolation comments depend on.

### Guard, private — `crates/player/src/guard.rs` unit tests

These five use the `pub(crate)` injection seam to feed `choose`
fabricated provers. Helpers at lines 173–200 (`mv`, the
`two_ended_four` fixture — here `Board::from_position` is fine because
only legality, not history, matters — and an `edges` builder that
synthesizes zero-visit root edges for every empty square):

```text
 173 | mod tests {
 174 |     use super::*;
 175 | 
 176 |     fn mv(row: u8, col: u8) -> Move {
 177 |         Move::new(row, col).unwrap()
 178 |     }
 179 | 
 180 |     fn two_ended_four() -> Board {
 181 |         Board::from_position(
 182 |             &[mv(2, 0), mv(2, 2), mv(2, 4), mv(2, 6)],
 183 |             &[mv(7, 3), mv(7, 4), mv(7, 5), mv(7, 6)],
 184 |             Color::Black,
 185 |         )
 186 |         .unwrap()
 187 |     }
 188 | 
 189 |     fn edges(board: &Board) -> Vec<Edge> {
 190 |         board
 191 |             .empty_moves()
 192 |             .map(|mv| Edge {
 193 |                 mv,
 194 |                 prior: 0.0,
 195 |                 n: 0,
 196 |                 w: 0.0,
 197 |                 child: None,
 198 |             })
 199 |             .collect()
 200 |     }
```

P1 — ranking, lines 202–246:

```text
 202 |     #[test]
 203 |     fn alternatives_rank_visits_then_prior_then_index_without_repeats_and_include_unvisited() {
 204 |         let board = two_ended_four();
 205 |         let original = board.clone();
 206 |         let proposal = mv(0, 4);
 207 |         let mut edges = edges(&board);
 208 |         for (candidate, n, prior) in [
 209 |             (proposal, 100, 1.0),
 210 |             (mv(0, 3), 4, 0.1),
 211 |             (mv(0, 2), 3, 0.9),
 212 |             (mv(0, 1), 3, 0.9),
 213 |             (mv(0, 0), 3, 0.5),
 214 |             (mv(0, 5), 0, 0.9),
 215 |         ] {
 216 |             let edge = edges.iter_mut().find(|e| e.mv == candidate).unwrap();
 217 |             edge.n = n;
 218 |             edge.prior = prior;
 219 |         }
 220 |         edges.reverse(); // Creation order must not leak into tie-breaking.
 221 |         let before = edges.clone();
 222 |         let mut seen = Vec::new();
 223 |         let config = GuardConfig {
 224 |             budget: SearchBudget {
 225 |                 max_nodes: 1,
 226 |                 max_depth: 1,
 227 |             },
 228 |             ..GuardConfig::baseline()
 229 |         };
 230 |         let (chosen, diagnostics) =
 231 |             choose(&board, proposal, &edges, config, |after, side, budget| {
 232 |                 seen.push(*after.moves().last().unwrap());
 233 |                 assert_eq!(side, after.to_move());
 234 |                 assert_eq!(side, Color::White);
 235 |                 assert_eq!(budget, config.budget);
 236 |                 engine::prove_forced_win(after, side, budget)
 237 |             });
 238 |         assert_eq!(seen, [proposal, mv(0, 3), mv(0, 1), mv(0, 2), mv(0, 0)]);
 239 |         assert_eq!(diagnostics.refuted, seen);
 240 |         assert_eq!(diagnostics.checked, 5);
 241 |         assert_eq!(diagnostics.outcome, GuardOutcome::BudgetExhausted);
 242 |         assert_eq!(chosen, mv(0, 5)); // Unvisited, untested, not a known loser.
 243 |         assert_eq!(edges, before);
 244 |         assert_eq!(board, original);
 245 |     }
 246 | 
```

**COMMENT — P1.** Hand-set visits/priors on synthesized edges,
deliberately reversed in creation order to expose tie-breaking leaks,
a 1-node/1-ply budget so every probe refutes, and a recording closure
that also asserts the probe receives the right side and budget.
Asserts the exact check order (proposal, then visits desc, prior desc,
index asc), that the cap-exhausted fallback is the unvisited untested
(0, 5) — not a known loser — and that inputs are unmutated. The
closure delegates to the real engine prover after recording, so the
refutations are genuine.

P2 — malformed proofs, lines 247–292:

```text
 247 |     #[test]
 248 |     fn malformed_or_unverified_proofs_leave_candidate_unknown() {
 249 |         let board = Board::from_position(&[mv(7, 3)], &[mv(7, 4)], Color::Black).unwrap();
 250 |         let proposal = mv(0, 0);
 251 |         let edges = edges(&board);
 252 |         for proof in [
 253 |             Proof {
 254 |                 winner: Color::Black,
 255 |                 line: vec![mv(7, 7)],
 256 |             },
 257 |             Proof {
 258 |                 winner: Color::White,
 259 |                 line: vec![],
 260 |             },
 261 |             Proof {
 262 |                 winner: Color::White,
 263 |                 line: vec![mv(7, 3)],
 264 |             },
 265 |             Proof {
 266 |                 winner: Color::White,
 267 |                 line: vec![proposal],
 268 |             },
 269 |             Proof {
 270 |                 winner: Color::White,
 271 |                 line: vec![mv(1, 0)],
 272 |             },
 273 |         ] {
 274 |             let mut calls = 0;
 275 |             let (chosen, diagnostics) = choose(
 276 |                 &board,
 277 |                 proposal,
 278 |                 &edges,
 279 |                 GuardConfig::baseline(),
 280 |                 |_, _, _| {
 281 |                     calls += 1;
 282 |                     Some(proof.clone())
 283 |                 },
 284 |             );
 285 |             assert_eq!(chosen, proposal);
 286 |             assert_eq!(diagnostics.outcome, GuardOutcome::Unrefuted);
 287 |             assert_eq!(diagnostics.checked, 1);
 288 |             assert!(diagnostics.refuted.is_empty());
 289 |             assert_eq!(calls, 1);
 290 |         }
 291 |     }
 292 | 
```

**COMMENT — P2.** Five fabricated proof shapes — wrong winner, empty
line, occupied first move, the candidate itself as "opponent" first
move, an irrelevant legal move — each returned by the injected prover,
each leaving the candidate `Unrefuted` after exactly one call. This is
the guard-side admission matrix; U1 covers the own-probe side. Scope:
with a fabricated prover, `verify_line` rejection is exercised only
where the fabricated line happens to fail verification — the
real-verifier interplay is covered end-to-end by G2 and G4.

P3 — no probe when disabled or zero-capped, lines 293–317:

```text
 293 |     #[test]
 294 |     fn disabled_and_zero_cap_never_call_probe() {
 295 |         let board = two_ended_four();
 296 |         let proposal = mv(0, 0);
 297 |         let edges = edges(&board);
 298 |         for (config, outcome) in [
 299 |             (GuardConfig::disabled(), GuardOutcome::Disabled),
 300 |             (
 301 |                 GuardConfig {
 302 |                     max_candidates: 0,
 303 |                     ..GuardConfig::baseline()
 304 |                 },
 305 |                 GuardOutcome::BudgetExhausted,
 306 |             ),
 307 |         ] {
 308 |             let (chosen, diagnostics) = choose(&board, proposal, &edges, config, |_, _, _| {
 309 |                 panic!("no probe allowed")
 310 |             });
 311 |             assert_eq!(chosen, proposal);
 312 |             assert_eq!(diagnostics.outcome, outcome);
 313 |             assert_eq!(diagnostics.checked, 0);
 314 |             assert!(diagnostics.refuted.is_empty());
 315 |         }
 316 |     }
 317 | 
```

**COMMENT — P3.** A panicking injected probe under both early exits;
passing means the exits precede any probe call. Also pins the two
different outcomes (`Disabled` versus `BudgetExhausted`) with zero
checks in both.

P4 — immediate win, no probe, lines 318–338:

```text
 318 |     #[test]
 319 |     fn immediate_win_never_calls_opponent_probe() {
 320 |         let board = Board::from_position(
 321 |             &[mv(0, 1), mv(0, 2), mv(0, 3), mv(0, 4)],
 322 |             &[mv(2, 0), mv(2, 2), mv(2, 4), mv(2, 6)],
 323 |             Color::Black,
 324 |         )
 325 |         .unwrap();
 326 |         let proposal = mv(0, 0);
 327 |         let (chosen, diagnostics) = choose(
 328 |             &board,
 329 |             proposal,
 330 |             &edges(&board),
 331 |             GuardConfig::baseline(),
 332 |             |_, _, _| panic!("terminal candidate must not be probed"),
 333 |         );
 334 |         assert_eq!(chosen, proposal);
 335 |         assert_eq!(diagnostics.outcome, GuardOutcome::Terminal);
 336 |         assert_eq!(diagnostics.checked, 1);
 337 |     }
 338 | 
```

**COMMENT — P4.** A four-in-a-row fixture where the proposal completes
five: `Terminal`, one check, panicking probe never called.

P5 — replayed draw, lines 339–381:

```text
 339 |     #[test]
 340 |     fn immediate_draw_never_calls_opponent_probe() {
 341 |         let proposal = mv(0, 0);
 342 |         let mut black = Vec::new();
 343 |         let mut white = Vec::new();
 344 |         for r in 0..15 {
 345 |             for c in 0..15 {
 346 |                 let stone = mv(r, c);
 347 |                 if stone == proposal {
 348 |                     continue;
 349 |                 }
 350 |                 // Repeating pairs avoid five on rows, columns and diagonals.
 351 |                 if (r + 2 * c) % 4 < 2 {
 352 |                     black.push(stone);
 353 |                 } else {
 354 |                     white.push(stone);
 355 |                 }
 356 |             }
 357 |         }
 358 |         // Replay stones so the engine also has the full pre-draw move history.
 359 |         let mut board = Board::new();
 360 |         for (black, white) in black.into_iter().zip(white) {
 361 |             board.play(black).unwrap();
 362 |             board.play(white).unwrap();
 363 |         }
 364 |         assert_eq!(board.status(), Status::Ongoing);
 365 |         let original = board.clone();
 366 |         let (chosen, diagnostics) = choose(
 367 |             &board,
 368 |             proposal,
 369 |             &edges(&board),
 370 |             GuardConfig::baseline(),
 371 |             |_, _, _| panic!("draw must not be probed"),
 372 |         );
 373 |         assert_eq!(chosen, proposal);
 374 |         assert_eq!(diagnostics.outcome, GuardOutcome::Terminal);
 375 |         assert_eq!(diagnostics.checked, 1);
 376 |         assert!(diagnostics.refuted.is_empty());
 377 |         assert_eq!(board, original);
 378 |         let mut after = board.clone();
 379 |         after.play(chosen).unwrap();
 380 |         assert_eq!(after.status(), Status::Draw);
 381 |     }
```

**COMMENT — P5.** The 224-stone draw fixture is **replayed** move by
move (never `Board::from_position`): the repeating-pair pattern avoids
fives on every line, and replaying gives the engine the full move
history a real game would have. Asserts `Terminal` with no probe, an
unmutated board, and — as a positive control — that playing the
candidate really does draw. Scope: the draw arm of the fastpath; it
also quietly proves `choose` handles a 224-ply replayed board whose
candidate list is the single remaining move without straining.
### CLI session — `crates/cli/src/session.rs` unit tests

These five live in the session's private test module, where the player
slots are directly injectable and the session RNG is observable. The
shared double at lines 864–881 counts evaluations *and* drops, wrapping
a real `TacticsEvaluator` so wrapped behavior stays honest:

```text
 864 |     struct CountingEvaluator {
 865 |         calls: Rc<Cell<u32>>,
 866 |         drops: Rc<Cell<u32>>,
 867 |         inner: mcts::TacticsEvaluator,
 868 |     }
 869 | 
 870 |     impl mcts::Evaluator for CountingEvaluator {
 871 |         fn evaluate(&mut self, request: mcts::EvalRequest) -> mcts::EvalResult {
 872 |             self.calls.set(self.calls.get() + 1);
 873 |             mcts::Evaluator::evaluate(&mut self.inner, request)
 874 |         }
 875 |     }
 876 | 
 877 |     impl Drop for CountingEvaluator {
 878 |         fn drop(&mut self) {
 879 |             self.drops.set(self.drops.get() + 1);
 880 |         }
 881 |     }
```

**COMMENT — `CountingEvaluator` with `Drop`.** Two `Rc<Cell<u32>>`
counters shared with the test: `calls` proves reuse, `drops` proves
replacement. The `Drop` impl is what turns "the old evaluator was
released" from an assumption into an observation. `Cell` (not
`RefCell`) suffices because the payload is `Copy`; both patterns are
test-only conveniences, not production requirements.

S1 — reuse across steps and across `SetSims`, lines 883–919:

```text
 883 |     #[test]
 884 |     fn persistent_player_reuses_evaluator_across_steps_and_set_sims() {
 885 |         let config = AiConfig {
 886 |             simulations: 1,
 887 |             ..AiConfig::default()
 888 |         };
 889 |         let mut core = SessionCore::new(SessionInit::versus_ai(Color::White, config));
 890 |         let calls = Rc::new(Cell::new(0));
 891 |         let drops = Rc::new(Cell::new(0));
 892 |         core.black_player = Some(player::AiPlayer::new(Box::new(CountingEvaluator {
 893 |             calls: calls.clone(),
 894 |             drops: drops.clone(),
 895 |             inner: mcts::TacticsEvaluator::new(),
 896 |         })));
 897 | 
 898 |         core.execute(Command::Step).unwrap();
 899 |         assert_eq!(calls.get(), 2, "root plus one simulation");
 900 |         core.execute(Command::Back { n: 1 }).unwrap();
 901 |         core.execute(Command::SetSims {
 902 |             seat: SeatSel::Black,
 903 |             n: 3,
 904 |         })
 905 |         .unwrap();
 906 |         assert_eq!(drops.get(), 0, "SetSims must retain the evaluator");
 907 |         core.execute(Command::Step).unwrap();
 908 |         assert_eq!(calls.get(), 6, "same evaluator: root plus three more sims");
 909 |         let info = core.search_info().unwrap();
 910 |         assert_eq!(info.config.simulations, 3);
 911 |         assert_eq!(info.visits.iter().map(|(_, n)| n).sum::<u32>(), 3);
 912 | 
 913 |         core.execute(Command::Take {
 914 |             seat: SeatSel::Black,
 915 |         })
 916 |         .unwrap();
 917 |         assert_eq!(drops.get(), 1, "taking a seat drops its evaluator");
 918 |         assert!(core.black_player.is_none());
 919 |     }
```

**COMMENT — S1.** One injected evaluator, then: step (2 calls at 1
simulation: root plus one), navigate back, raise the budget to 3,
step again — 6 total, so the *same* evaluator kept evaluating, and
`drops == 0` proves `SetSims` did not rebuild anything. The stored
`SearchInfo` confirms the new budget took effect (config says 3,
visits sum to 3). Finally `Take` drops the evaluator and empties the
slot. Scope: persistence and lifecycle of the *object*; the seeded
equivalence of what it computes is C1's job.

S2 — the full lifecycle matrix, lines 921–980:

```text
 921 |     #[test]
 922 |     fn persistent_players_follow_init_release_take_and_reset() {
 923 |         let config = AiConfig {
 924 |             simulations: 1,
 925 |             ..AiConfig::default()
 926 |         };
 927 |         let init = SessionInit::versus_ai(Color::White, config);
 928 |         let mut core = SessionCore::new(init.clone());
 929 |         assert!(core.black_player.is_some());
 930 |         assert!(core.white_player.is_none());
 931 | 
 932 |         let calls = Rc::new(Cell::new(0));
 933 |         let drops = Rc::new(Cell::new(0));
 934 |         core.black_player = Some(player::AiPlayer::new(Box::new(CountingEvaluator {
 935 |             calls: calls.clone(),
 936 |             drops: drops.clone(),
 937 |             inner: mcts::TacticsEvaluator::new(),
 938 |         })));
 939 |         core.execute(Command::Release {
 940 |             seat: SeatSel::White,
 941 |         })
 942 |         .unwrap();
 943 |         assert_eq!(drops.get(), 0, "releasing White must retain Black's player");
 944 |         assert!(core.white_player.is_some());
 945 |         core.white_player = Some(player::AiPlayer::new(Box::new(CountingEvaluator {
 946 |             calls,
 947 |             drops: drops.clone(),
 948 |             inner: mcts::TacticsEvaluator::new(),
 949 |         })));
 950 |         core.execute(Command::SetSims {
 951 |             seat: SeatSel::Both,
 952 |             n: 3,
 953 |         })
 954 |         .unwrap();
 955 |         assert_eq!(drops.get(), 0);
 956 | 
 957 |         assert_eq!(core.execute(Command::New).unwrap(), vec![Event::Reset]);
 958 |         assert_eq!(drops.get(), 2, "reset discards both old evaluators");
 959 |         assert!(core.black_player.is_some());
 960 |         assert!(core.white_player.is_none());
 961 |         assert_eq!(core.black(), &init.black);
 962 |         assert_eq!(core.white(), &init.white);
 963 |         core.execute(Command::Step).unwrap();
 964 |         assert_eq!(core.search_info().unwrap().config.simulations, 1);
 965 | 
 966 |         core.execute(Command::Take {
 967 |             seat: SeatSel::Both,
 968 |         })
 969 |         .unwrap();
 970 |         assert!(core.black_player.is_none());
 971 |         assert!(core.white_player.is_none());
 972 |         core.execute(Command::Release {
 973 |             seat: SeatSel::Both,
 974 |         })
 975 |         .unwrap();
 976 |         assert!(core.black_player.is_some());
 977 |         assert!(core.white_player.is_some());
 978 |         core.execute(Command::Step).unwrap();
 979 |         assert_eq!(core.search_info().unwrap().config.simulations, 1);
 980 |     }
```

**COMMENT — S2.** Init gives Black a player and White none; releasing
White adds one without touching Black's (`drops == 0`); `SetSims` on
both drops nothing; `New` (reset) drops both injected doubles and
restores the `init` occupants with fresh players; `Take`/`Release` on
both seats empty and refill the slots together. Every transition routes
through `set_occupant`, which is why the occupant/player pairs can
never desynchronize — this test is the dynamic proof of the §3
invariant.

S3 — a certified step through the session. The panicking double and
the replayed VCF script helper (lines 982–997), then the test
(999–1030):

```text
 982 |     struct NeverEvaluate;
 983 | 
 984 |     impl mcts::Evaluator for NeverEvaluate {
 985 |         fn evaluate(&mut self, _: mcts::EvalRequest) -> mcts::EvalResult {
 986 |             panic!("a certified own win must bypass evaluation");
 987 |         }
 988 |     }
 989 | 
 990 |     // Play the slice-A win-in-3 VCF script into a replayed session position:
 991 |     // Black to move forces a win starting with (7, 7).
 992 |     fn play_vcf_script(core: &mut SessionCore) {
 993 |         play_some(
 994 |             core,
 995 |             &[7, 4, 7, 3, 7, 5, 0, 0, 7, 6, 0, 2, 6, 6, 0, 4, 8, 8, 0, 6],
 996 |         );
 997 |     }
```

```text
 999 |     #[test]
1000 |     fn persistent_player_proof_plays_without_search_info_or_rng_draws() {
1001 |         use rand::RngExt;
1002 | 
1003 |         let config = AiConfig {
1004 |             evaluator: EvaluatorKind::Baseline,
1005 |             temperature: 1.0,
1006 |             noise_epsilon: 0.25,
1007 |             explicit_selection: true,
1008 |             ..AiConfig::default()
1009 |         };
1010 |         let mut core = SessionCore::new(SessionInit {
1011 |             seed: Some(42),
1012 |             ..SessionInit::new_hotseat()
1013 |         });
1014 |         play_vcf_script(&mut core);
1015 |         core.black = Occupant::Ai(config);
1016 |         core.black_player = Some(player::AiPlayer::new(Box::new(NeverEvaluate)));
1017 |         let mut untouched_rng = StdRng::seed_from_u64(42);
1018 | 
1019 |         assert_eq!(
1020 |             core.execute(Command::Step).unwrap(),
1021 |             vec![Event::Moved {
1022 |                 mv: mv(7, 7),
1023 |                 by: OccupantKind::Ai
1024 |             }]
1025 |         );
1026 |         // The proof path stores no record at the fresh cursor; historical
1027 |         // records elsewhere in the tree are unaffected either way.
1028 |         assert!(core.search_info().is_none());
1029 |         assert_eq!(core.rng.random::<u64>(), untouched_rng.random::<u64>());
1030 |     }
```

**COMMENT — S3.** The session-level bypass: the VCF position is built
by *replayed session plays* (so the session history genuinely contains
the position), the Baseline-mapped seat is given a panicking
evaluator, and the configuration deliberately enables temperature and
noise. Step emits exactly one event — the certified move (7, 7) — the
panicking evaluator proves no search ran, `search_info().is_none()`
records the scoped claim (no record **at the fresh cursor**; the
in-code comment repeats that historical records elsewhere are neither
asserted nor disturbed), and the RNG comparison against an untouched
seed-42 stream proves zero draws — noise and temperature never
executed. Note the injection pattern: the test assigns `core.black`
and `core.black_player` directly, which is exactly the desync
`set_occupant` prevents in production — acceptable here because the
test module owns the invariant and wants a `NeverEvaluate` the factory
cannot build.

S4 — the own-probe policy map, lines 1032–1077:

```text
1032 |     #[test]
1033 |     fn baseline_enables_own_tss_probe_but_tactics_searches() {
1034 |         // Same injected evaluator under both kinds on the VCF fixture: the
1035 |         // Baseline policy certifies before search; the Tactics policy really
1036 |         // searches and stores the record.
1037 |         for (kind, expect_proof) in [
1038 |             (EvaluatorKind::Baseline, true),
1039 |             (EvaluatorKind::Tactics, false),
1040 |         ] {
1041 |             let config = AiConfig {
1042 |                 evaluator: kind,
1043 |                 simulations: 5,
1044 |                 explicit_selection: true,
1045 |                 ..AiConfig::default()
1046 |             };
1047 |             let mut core = SessionCore::new(SessionInit {
1048 |                 seed: Some(42),
1049 |                 ..SessionInit::new_hotseat()
1050 |             });
1051 |             play_vcf_script(&mut core);
1052 |             let calls = Rc::new(Cell::new(0));
1053 |             core.black = Occupant::Ai(config);
1054 |             core.black_player = Some(player::AiPlayer::new(Box::new(CountingEvaluator {
1055 |                 calls: calls.clone(),
1056 |                 drops: Rc::new(Cell::new(0)),
1057 |                 inner: mcts::TacticsEvaluator::new(),
1058 |             })));
1059 | 
1060 |             let events = core.execute(Command::Step).unwrap();
1061 | 
1062 |             if expect_proof {
1063 |                 assert_eq!(calls.get(), 0, "baseline own probe precedes search");
1064 |                 assert!(events.contains(&Event::Moved {
1065 |                     mv: mv(7, 7),
1066 |                     by: OccupantKind::Ai
1067 |                 }));
1068 |                 assert!(core.search_info().is_none());
1069 |             } else {
1070 |                 assert!(calls.get() > 0, "tactics policy really searches");
1071 |                 let info = core.search_info().expect("searched step stores info");
1072 |                 assert_eq!(info.config.simulations, 5);
1073 |                 assert_eq!(info.visits.iter().map(|(_, n)| n).sum::<u32>(), 5);
1074 |             }
1075 |         }
1076 |     }
1077 | 
```

**COMMENT — S4.** The new policy-map pin: the *same* injected counting
evaluator under both evaluator kinds on the VCF fixture. Baseline:
zero evaluation calls, the certified move event, no record — the own
probe preceded search. Tactics: calls > 0 and a stored record whose
visits sum to the configured 5 — the tactics policy really searches.
The loop over `(kind, expect_proof)` runs both arms through identical
setup, so the only varying input is the policy map itself.

S5 — the guard policy map, lines 1078–1127:

```text
1078 |     #[test]
1079 |     fn baseline_enables_defensive_guard_but_tactics_preserves_proposal() {
1080 |         struct PreferBlock;
1081 |         impl mcts::Evaluator for PreferBlock {
1082 |             fn evaluate(&mut self, _: mcts::EvalRequest) -> mcts::EvalResult {
1083 |                 let mut policy = [0.0; 225];
1084 |                 policy[mv(7, 7).index()] = 10.0;
1085 |                 mcts::EvalResult { policy, value: 0.0 }
1086 |             }
1087 |         }
1088 |         let board = Board::from_position(
1089 |             &[mv(7, 2), mv(2, 0), mv(2, 2), mv(2, 4)],
1090 |             &[mv(7, 3), mv(7, 4), mv(7, 5), mv(7, 6)],
1091 |             Color::Black,
1092 |         )
1093 |         .unwrap();
1094 |         for (kind, expected) in [
1095 |             (EvaluatorKind::Baseline, mv(7, 7)),
1096 |             (EvaluatorKind::Tactics, mv(0, 0)),
1097 |         ] {
1098 |             // Initialize a cheap tactics seat, then inject a fixed evaluator.
1099 |             // This tests the CLI policy mapping without running Burn in debug.
1100 |             let config = AiConfig {
1101 |                 evaluator: EvaluatorKind::Tactics,
1102 |                 simulations: 0,
1103 |                 explicit_selection: true,
1104 |                 ..AiConfig::default()
1105 |             };
1106 |             let mut core = SessionCore::new(SessionInit {
1107 |                 opening: board.clone(),
1108 |                 seed: Some(42),
1109 |                 ..SessionInit::versus_ai(Color::White, config.clone())
1110 |             });
1111 |             core.black = Occupant::Ai(AiConfig {
1112 |                 evaluator: kind,
1113 |                 ..config
1114 |             });
1115 |             core.black_player = Some(player::AiPlayer::new(Box::new(PreferBlock)));
1116 |             let events = core.execute(Command::Step).unwrap();
1117 |             assert!(events.contains(&Event::Moved {
1118 |                 mv: expected,
1119 |                 by: OccupantKind::Ai
1120 |             }));
1121 |             assert_eq!(core.board().stone_at(expected), Some(Color::Black));
1122 |             let info = core.search_info().expect("guard keeps real search info");
1123 |             assert!(info.visits.iter().all(|(_, n)| *n == 0));
1124 |             assert_eq!(info.prior[0].0, mv(7, 7));
1125 |         }
1126 |     }
1127 | 
```

**COMMENT — S5.** Same shape, guard side: a `PreferBlock` evaluator
peaks the policy on the block (7, 7) of the single-threat fixture with
zero simulations, so the proposal is (0, 0) by index tie-break.
Baseline substitutes the block (guard enabled); Tactics keeps the
proposal (guard disabled). Both arms store real search info (visits
all zero, top prior (7, 7)) — the guard substitutes the move, never
the record. The comment in the body documents why the seat starts as
cheap tactics: the policy mapping is under test, not Burn's speed in a
debug build.

### CLI actor — `crates/cli/tests/actor.rs`

C1 — the seeded regression, lines 146–207:

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
 166 |         let mut outcome = mcts::search(
 167 |             &board,
 168 |             &mut evaluator,
 169 |             &mcts::SearchConfig {
 170 |                 simulations: effective.simulations,
 171 |                 c_puct: effective.c_puct,
 172 |             },
 173 |         );
 174 |         if effective.noise_epsilon > 0.0 {
 175 |             mcts::add_dirichlet_noise(
 176 |                 &mut outcome.tree,
 177 |                 outcome.root,
 178 |                 effective.noise_epsilon,
 179 |                 0.3,
 180 |                 &mut rng,
 181 |             );
 182 |         }
 183 |         let expected_info = SearchInfo::from_search(&outcome, &board, &effective);
 184 |         let expected_move = mcts::select_move(
 185 |             &mcts::visit_distribution(&outcome.tree, outcome.root),
 186 |             effective.temperature,
 187 |             &mut rng,
 188 |         )
 189 |         .unwrap();
 190 | 
 191 |         assert_eq!(
 192 |             core.execute(Command::Step).unwrap(),
 193 |             vec![Event::Moved {
 194 |                 mv: expected_move,
 195 |                 by: OccupantKind::Ai
 196 |             }],
 197 |             "same seeded move at ply {ply}"
 198 |         );
 199 |         assert_eq!(
 200 |             core.search_info(),
 201 |             Some(&expected_info),
 202 |             "stats at ply {ply}"
 203 |         );
 204 |         board.play(expected_move).unwrap();
 205 |         assert_eq!(core.board(), board);
 206 |     }
 207 | }
```

**COMMENT — C1.** The equivalence anchor for the whole refactor:
eleven plies, two tactics seats, seed 42, three simulations. Each ply
recomputes the expected result through the *pre-player manual path* —
fresh `TacticsEvaluator`, `mcts::search`, noise, and crucially the
untouched `SearchInfo::from_search` — with the opening schedule applied
by hand (temperature 1.0, noise 0.25 for plies < 10). Then it asserts
the session's event, its full stored `SearchInfo`, and board equality
per ply. Persistent players must reproduce a fresh-evaluator legacy
session bit for bit; the tactics evaluator is stateless across
instances, which is what makes the equivalence provable here. Scope:
the tactics policy (both probes disabled); the baseline policy's
equivalence is not claimed and not needed — the baseline seat's new
behavior is the point of the chapter.

C2 and C3 — the real-Burn baseline pair (full-QA scope; both ran in
the recorded 563-test gate), lines 289–385:

```text
 289 | #[test]
 290 | fn baseline_ai_plays_proven_vcf_without_search_info() {
 291 |     let mut board = Board::new();
 292 |     let script = [
 293 |         (7, 4),
 294 |         (7, 3),
 295 |         (7, 5),
 296 |         (0, 0),
 297 |         (7, 6),
 298 |         (0, 2),
 299 |         (6, 6),
 300 |         (0, 4),
 301 |         (8, 8),
 302 |         (0, 6),
 303 |     ];
 304 |     for &(r, c) in &script {
 305 |         board.play(mv(r, c)).unwrap();
 306 |     }
 307 |     assert_eq!(board.to_move(), Color::Black);
 308 | 
 309 |     let mut core = SessionCore::new(SessionInit {
 310 |         black: Occupant::Ai(AiConfig {
 311 |             evaluator: EvaluatorKind::Baseline,
 312 |             simulations: 50,
 313 |             c_puct: 1.5,
 314 |             temperature: 0.0,
 315 |             noise_epsilon: 0.0,
 316 |             explicit_selection: true,
 317 |         }),
 318 |         white: Occupant::Human,
 319 |         default_config: AiConfig::default(),
 320 |         black_sims: None,
 321 |         white_sims: None,
 322 |         seed: Some(1),
 323 |         opening: board,
 324 |         swap2: false,
 325 |         puzzle: None,
 326 |     });
 327 | 
 328 |     let events = core.execute(Command::Step).unwrap();
 329 |     let moved = events
 330 |         .iter()
 331 |         .find_map(|e| match e {
 332 |             Event::Moved { mv, .. } => Some(*mv),
 333 |             _ => None,
 334 |         })
 335 |         .expect("AI should move.");
 336 |     assert_eq!(
 337 |         moved,
 338 |         mv(7, 7),
 339 |         "proven VCF must play the forcing first move"
 340 |     );
 341 |     assert!(
 342 |         core.search_info().is_none(),
 343 |         "proven win must not store search info at the fresh cursor"
 344 |     );
 345 | }
```

```text
 347 | #[test]
 348 | fn baseline_ai_falls_back_to_search_info_on_quiet_position() {
 349 |     let mut core = SessionCore::new(SessionInit {
 350 |         black: Occupant::Ai(AiConfig {
 351 |             evaluator: EvaluatorKind::Baseline,
 352 |             simulations: 50,
 353 |             c_puct: 1.5,
 354 |             temperature: 0.0,
 355 |             noise_epsilon: 0.0,
 356 |             explicit_selection: true,
 357 |         }),
 358 |         white: Occupant::Human,
 359 |         default_config: AiConfig::default(),
 360 |         black_sims: None,
 361 |         white_sims: None,
 362 |         seed: Some(1),
 363 |         opening: Board::new(),
 364 |         swap2: false,
 365 |         puzzle: None,
 366 |     });
 367 | 
 368 |     let events = core.execute(Command::Step).unwrap();
 369 |     assert!(
 370 |         events.iter().any(|e| matches! {
 371 |             e, Event::Moved { by: OccupantKind::Ai, ..}
 372 |         }),
 373 |         "Step with an AI seat must emit Move{{by: Ai}}, got {events:?}"
 374 |     );
 375 | 
 376 |     let info = core
 377 |         .search_info()
 378 |         .expect("searched position must store info");
 379 |     assert_eq!(info.config.simulations, 50);
 380 |     assert_eq!(
 381 |         info.visits.iter().map(|(_, n)| n).sum::<u32>(),
 382 |         50,
 383 |         "total root visits must match simulations"
 384 |     );
 385 | }
```

**COMMENT — C2/C3.** Carried over from the legacy chapter and now
exercised entirely through the player. C2: a session opened on the VCF
fixture with a real baseline seat plays the forcing move (7, 7) and
stores no record at the fresh cursor — the certified path end to end
with the production evaluator in the slot. C3: the same seat on an
empty board searches and stores a consistent record (50 simulations,
visits summing to 50). Together they pin that the baseline policy
certifies when it can and searches honestly when it cannot. They are
slow (real model inference) and were part of the full release-mode QA
run, not the filtered development loop.

## Minimal necessary mechanism versus optional choices

**Essential behaviors — any correct implementation of this chapter has
them:**

- A persistent per-seat decision owner that reuses one evaluator across
  moves; the evaluator's `&mut self` contract makes the owner's
  `decide(&mut self, ...)` signature necessary.
- A terminal refusal before any evaluation or randomness, inside the
  player (not only in the caller).
- A pre-search own probe whose *admission* requires the correct winner,
  a non-empty line, a legal first move, and a passed `verify_line` —
  with every rejection degrading to ordinary search (unknown, never
  loss).
- A certified bypass that allocates nothing, evaluates nothing, draws
  no randomness, and fabricates no statistics (`search: None`).
- The ordinary path preserving the established order: search → noise →
  visit extraction → temperature selection, with noise after search and
  the RNG owned by the caller.
- Guard refutation decided only by verified opponent proofs on cloned
  boards, with unrefuted meaning unknown, cap fallback to an untested
  candidate, and no loss certificate when everything is refuted.
- Statistics that always describe the real proposal search, and a
  session that stores no new record on the certified path.
- Budgets as work caps, never wall-clock deadlines; verification
  unbudgeted by construction (it replays one line).

**Chosen encodings — the reference picks these; equivalents exist:**

- The type names and shapes: `PlayerOptions`/`OwnTssConfig`/
  `GuardConfig` structs with named `baseline()`/`disabled()` presets
  (bare fields plus constants would do), six `GuardOutcome` variants
  (fewer could encode the same information), `Decision` as a struct
  with `Option<SearchStats>` (an enum of certified/searched would too).
- `certified_own_move` as a private free function using
  `bool::then_some`; a method or an inline block would behave
  identically — the four rejection rules are the necessity, their
  packaging is not.
- The guard's `impl FnMut` probe seam (the engine function could be
  called directly, at the cost of the private tests' precision);
  `partial_cmp(...).unwrap_or(Equal)` NaN-tolerant tiebreaks; the
  `Rc<Cell>`/`Rc<RefCell>` counters in tests.
- The CLI policy map keyed on `EvaluatorKind` (a per-seat
  configuration flag would express the same policy).
- `const fn` presets, `#[must_use]`, hand-written `Display`/`Error`
  (a `thiserror` derive would need a fourth dependency).

**What this design does not promise:**

- It does not prove positions lost: not on probe exhaustion (unknown),
  not on `Unrefuted` (unknown), not on `AllRefuted` (each refutation
  was budget-bounded; the original proposal is retained).
- It does not bound wall-clock time; budgets bound prover work only.
- It does not count or cache own probes dynamically; "once per decide,
  never cached" is structural (one call site, one state field) plus
  fixture evidence (D6/D9/S3), by design — there is no injectable own
  prover.
- It does not make the certified move bypass legality: the CLI plays it
  through the same checked `execute_play`/`history.play` path as any
  move.
- It does not change the `engine`, `mcts`, or `baseline` crates, the
  `Evaluator` trait, or `SearchOutcome`; the retired hook vocabulary
  (`RootEval`, `evaluate_root`, `SearchResult`) does not exist here.
- It does not claim strength or latency differences versus earlier
  designs; no measurements were taken for this redesign.
- It does not make the TSS overlay (the CLI's `t` key) part of move
  selection; that display path calls the prover independently, as
  before.

## References

- [The chapter](the-ai-player.md) — intention, specification, walkthrough.
- [The design paper](the-ai-player-design.md) — why the design has this shape; the certification-authority invariants.
- [The implementation paper](the-ai-player-implementation.md) — the spoiler-free construction layer.
- [Baseline primer](../00-baseline-primer.md) — the tutorial's design authority.
- [Chapter 07: the quiet whisper](../07-the-quiet-whisper/the-quiet-whisper.md) — the learner's starting point; its hookless `main` source is the base of this chapter.
- [mcts tutorial: the evaluation seam](../../mcts-tutorial/04-the-evaluation-seam.md), [the simulation loop](../../mcts-tutorial/07-the-simulation-loop.md), [from tree to move](../../mcts-tutorial/08-from-tree-to-move.md) — the unchanged search, noise, and selection algorithms this chapter orchestrates.
- [Engine tutorial: threat-space search](../../13-engine-tutorial/08-threat-space-search.md) — `prove_forced_win`, `verify_line`, `Proof`, `SearchBudget`.
- Legacy packs (superseded, kept for history): [08 — the TSS eye](../08-the-tss-eye/the-tss-eye.md) (the retired root-hook design) and [08b — the AI player](../08b-the-ai-player/ai-player.md) (the source of the ported guard and statistics code).
- Reference build: `rust-ml-unified08-ref` worktree (detached at `2b6ec75`, uncommitted); gates recorded in the slice evidence files cited by the chapter.
