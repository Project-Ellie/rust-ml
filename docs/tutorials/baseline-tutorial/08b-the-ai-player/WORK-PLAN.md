# Sub-chapter 08b — The AI player: work plan and evidence ledger

## Abstract

This chapter has two independently verifiable milestones: extract a reusable,
persistent AI player without changing ordinary decisions, then add an optional
bounded opponent-proof guard after move selection. Both are implemented in
the sibling reference worktree. This ledger records the implemented contracts,
corrects the earlier draft's fallback and statistics errors, and records
fresh full-workspace gate evidence alongside completed mechanical diagram
parsing; interactive play remains unclaimed. It does not claim that reference source
has landed on main.

## Glossary

| Term | Meaning |
|------|---------|
| Reference | Implemented worktree `rust-ml-ai-player-ref` beside the main repository. |
| MCTS | Monte Carlo tree search, producing root visits, priors, and values. |
| CLI | Command-line interface and its study-session command/event model. |
| Proposal | Original temperature-selected move before defensive checking. |
| Refuted | A candidate permits an opponent winning proof accepted by eligibility checks and line verification. |
| Unrefuted | Unknown within the bounded probe, not certified safe. |
| RNG | Session-owned random-number generator borrowed by a decision. |
| `[experiment]` | Local tuning settings without a strength or elapsed-time guarantee. |

## Scope and authority

The [tutorial][tutorial] and [design paper][design] are grounded in the
implemented [player library][player], [guard module][guard], and
[CLI integration][session]. Chapter 08's evaluator root hook is a prerequisite:
`Evaluator::evaluate_root`, `RootEval`, and `SearchResult` already distinguish
certified root moves from estimates and searched trees. Chapter 08b consumes
that seam without adding hooks or modifying engine behavior.

All four chapter papers are authored: [tutorial][tutorial], [design][design],
[construction][implementation], and [complete opt-in solution][solution],
plus this ledger. Fresh full reference gates are verified below. The final
bounded amendments reconcile the test inventory and evidence and narrow one
reference `AiPlayer::decide` panic doc-comment line, with no line-count or
behavior change and no new parameter validation. The solution's matching
numbered quote is updated. Main learner source, navigation, README, WARM-UP,
and other papers are outside this slice. No commits or pushes were made.

## Milestone 1 — persistent player, guard disabled

**Reference implementation: present. Learner implementation: follow the
executable tests before each production slice.**

### Artifacts

- [Workspace manifest][workspace-manifest]: register `crates/player`.
- [Player manifest][player-manifest]: depend on `engine`, `mcts`, and `rand`
  only; no Burn, baseline, patterns, CLI, or dependency cycle.
- [Player library][player]: `AiPlayer`, `PlayerOptions`, `Decision`,
  `SearchStats`, and `DecisionError`; public documentation is required.
- [Decision tests][decisions]: terminal rejection, manual seeded-path
  equivalence, proof-without-stats, and persistent evaluator observations.
- [CLI manifest][cli-manifest]: add player dependency.
- [CLI session][session]: optional per-seat runtime players, lifecycle helpers,
  effective options, and decision routing.
- [Actor types][actor]: `SearchInfo::from_stats` attaches effective config;
  existing `from_searched` remains available.
- [Actor integration tests][actor-tests]: eleven-ply seeded comparison across
  the opening schedule boundary.

### Contracts to preserve

1. `AiPlayer::new` owns `Box<dyn mcts::Evaluator>`. `decide` borrows the board,
   per-move `PlayerOptions`, and mutable RNG; it does not commit a move.
2. A terminal input returns `DecisionError::GameOver` before evaluator or RNG
   use. A certified root win instead returns a successful `Decision` with
   `search: None`, `ProvenWin` diagnostics, and zero candidate checks.
3. Ordinary order remains search, optional **post-search prior noise**, visit
   extraction, temperature selection. Noise does not change current visits or
   accumulated values, although its RNG consumption can change a seeded draw.
4. `SearchStats` contains all root visits, all post-noise priors, searched-side
   perspective, and actual root value; zero visits give zero value. It has no
   `proven` flag or invented proof distribution.
5. Initial AI seats get players through `make_player`. `set_occupant` replaces
   configuration and player together for release/take/reset; humans have none.
   `execute_new` calls reset, restoring initial seats and retaining the RNG.
6. Moves, navigation, and `execute_set_sims` retain evaluator instances. The
   next call receives changed simulation/selection settings through options.
7. Runtime players remain outside cloneable/serializable `Occupant` and
   `AiConfig`. The CLI constructs concrete evaluators; the player stays generic.
8. `SessionCore` retains opening, game-over, and seat checks, history, search
   record storage, and events. Proof decisions use `execute_play`; searched
   decisions preserve the existing history/event branch. A searched record
   attaches effective, not merely configured, selection settings.

### Test-first checkpoint

Implement the four [public decision tests][decisions], then the three session
unit tests named `persistent_player_reuses_evaluator_across_steps_and_set_sims`,
`persistent_players_follow_init_release_take_and_reset`, and
`persistent_player_proof_plays_without_search_info_or_rng_draws`. Finally run
`persistent_players_match_seeded_legacy_search_through_opening_schedule` in
[actor integration tests][actor-tests]. Pause here before enabling the guard.
These tests check lifecycle and preserved decisions, not baseline strength.

## Milestone 2 — defensive candidate guard

**Reference implementation: present. Reusable defaults: disabled. CLI policy:
enabled for baseline, disabled for tactics.**

### Artifacts and contracts

The private [guard module][guard] defines `GuardConfig`, `GuardDiagnostics`,
and `GuardOutcome`, re-exported from the player library. Its `choose` helper
accepts a probe callback for private tests; production supplies the engine
prover. It is not a new evaluator hook.

1. `GuardConfig::baseline()` enables `[experiment]` bounds of **five candidate
   checks, 1,000 nodes and ten plies per opponent probe**. Both default guard
   configuration and default player options disable checking. These bounds
   are not a strict elapsed-time deadline, including proof verification.
2. Retain the ordinary temperature proposal first. Retry only after a verified
   refutation; rank remaining moves by descending visits, descending prior,
   ascending move index. Include zero-visit legal moves and do not repeat or
   resample candidates.
3. Clone the board and play the candidate. Accept an immediate own win/draw
   without probing. Otherwise probe `after.to_move()`, the actual opponent.
   Refute only with correct winner, a present legal first move, and an accepted
   `verify_line`. `None` or rejected evidence means `Unrefuted`: UNKNOWN.
4. At the candidate cap, choose the highest-ranked remaining **untested** move
   with `BudgetExhausted`. A zero cap leaves the proposal unchecked. Do not
   revert to a known-refuted proposal merely because the cap was reached.
5. Only after individually refuting **every legal move** return the original
   legal proposal with `AllRefuted`. This fallback can choose a known-refuted
   move. The diagnostic is not a composed root-position loss proof.
6. Diagnostics are `proposal`, ordered `refuted`, `checked`, and `outcome`.
   Outcomes are `Disabled`, `ProvenWin`, `Unrefuted`, `Terminal`,
   `BudgetExhausted`, and `AllRefuted`. They do not alter search statistics.
7. The CLI chooses guard options from evaluator kind. The generic player does
   not depend on baseline or identify evaluator types. Library callers can
   disable the guard; no CLI toggle is introduced. The current CLI does not
   retain/display the returned guard diagnostics.

### Test-first checkpoint

Use the twelve [public guard tests][guard-tests] with real engine proofs,
then five private tests beside `choose` in the [guard module][guard] for
ordering, malformed proofs, disabled/zero caps, immediate wins, and draws.
The draw fixture is replay-built so ordinary engine move-history bookkeeping
is populated; this is not an engine change. The tutorial specifies exact test
names, setups, and assertions. In particular,
`blocking_one_of_two_distinct_opponent_lines_is_refuted_in_favor_of_own_immediate_win`
independently verifies horizontal/vertical threats and a remaining-line proof,
then asserts the own-win retry and unchanged statistics, RNG, and board.
Keep the older two-endpoints-of-one-line test too.

Finish with `baseline_enables_defensive_guard_but_tactics_preserves_proposal`
in the [session unit tests][session]. This test injects a cheap evaluator under
the configured kinds; it verifies policy mapping **without executing a real
Burn baseline evaluator**. Candidate refutation is proof-certified; an accepted
nonterminal defense is not.

## Corrections to the earlier draft

| Earlier claim | Implemented fact |
|---------------|------------------|
| Player has only one source file | Library plus private guard module; public integration tests are separate files. |
| Proof gets a visit, unit prior, value one, and `proven` flag | Proof decision has `search: None`; no synthetic statistics or proof flag. |
| Every tested candidate refuted implies original fallback | Cap chooses the next ranked untested candidate; original fallback occurs only when every legal candidate is refuted. |
| Guard receives a noisy visit distribution | Post-search noise modifies priors, not visits; it consumes RNG before temperature sampling and can affect retry prior ties. |
| `None` indicates a safe move | `None` is unknown under the bounds; acceptance is not a safety certificate. |
| All-refuted outcome proves the root lost | Individual refutations are diagnostics, not a composed root-position loss certificate. |
| CLI policy test demonstrates real baseline execution | It substitutes a cheap evaluator. Both real-baseline actor tests, historically skipped in the filtered run, now pass in full release QA. |
| Proposed test names alone establish completion | The tutorial uses actual implemented test names; fresh full-gate evidence is recorded below. |
| Every returned move follows `execute_play` | Proof path does; searched path retains explicit history commit, record storage, and event construction. |

## Verification ledger

Fresh QA from the reference's `gomoku/` records:

| Command | Result |
|---------|--------|
| `cargo test --release --locked --all-targets` | Exit 0; 574 passed, 0 failed, 6 ignored across the workspace; 42 s including compilation, 4 s cached. All ten actor tests passed, including both real-baseline cases historically skipped. |
| `cargo clippy --locked --all-targets -- -D warnings` | Exit 0; 9 s. |
| `cargo fmt --all -- --check` | Exit 0. |
| `git diff --check` | Exit 0. |

The current player inventory is **21 passed: 5 private guard + 4 public
decision + 12 public guard tests**. The six ignored tests are preexisting:
two baseline acceptance, one CLI acceptance, and three patterns differential
tests. The older 20-test inventory predates the additional distinct-line
regression. Earlier scoped library/session/filtered-actor results are
historical checkpoints, superseded for final QA by the full release suite.
Reference verification does not land implementation on main. After the
single-line reference doc-comment amendment,
`cargo test --release --locked -p player` passed all 21 tests again (5 + 4 + 12);
formatting and reference
`git diff --check` also exited 0.

Independent review findings are addressed by these bounded amendments:
explicit distinct-line specification/checklist, precise panic documentation
without new validation, consistent inventory, and fresh evidence in all five
papers. A fresh audit verified all 68 complete-solution source snippets and their
numbered ranges byte-exact against the reference, with complete nonblank
coverage of all four new player Rust files. Reference `lib.rs` remains 192
lines; only its line 143 doc comment was amended. Both design Mermaid fences now parse cleanly under mermaid
12.3.0 with jsdom (`mermaid.parse`: sequenceDiagram → `sequence`,
stateDiagram-v2 → `stateDiagram`, no errors); syntactic parsing only, no
browser/render validation. Interactive study-table smoke testing is not
claimed.
No measured guard strength or latency follows from correctness gates, and
no asynchronous work or tree reuse is planned. No commits or pushes were made.

## References

- [The AI player: tutorial][tutorial] — executable specification before
  implementation walkthrough, TDD checklist, and done-when criteria.
- [The AI player: design paper][design] — connected narrative, sequence and
  state diagrams, transition table, catalogs, and invariants.
- [The AI player: construction][implementation] — test-first implementation order.
- [The AI player: complete opt-in solution][solution] — self-contained, numbered source quotations.
- Implemented reference: [workspace manifest][workspace-manifest],
  [player manifest][player-manifest], [player library][player],
  [guard module][guard], [decision tests][decisions],
  [public guard tests][guard-tests], [CLI manifest][cli-manifest],
  [session implementation and unit tests][session], [actor types][actor],
  [actor integration tests][actor-tests], [session integration tests][session-tests].

[tutorial]: ./ai-player.md
[design]: ./ai-player-design.md
[implementation]: ./ai-player-implementation.md
[solution]: ./ai-player-solution.md
[workspace-manifest]: ../../../../../rust-ml-ai-player-ref/gomoku/Cargo.toml
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
