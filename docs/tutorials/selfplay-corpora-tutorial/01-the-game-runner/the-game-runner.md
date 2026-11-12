# Chapter 01 — The game runner

## Abstract

This chapter creates the `selfplay` crate and its first inhabitant: a
synchronous, single-threaded game runner. Two seats of the hand-written
baseline player — one per color, each a persistent `player::AiPlayer`
owning a `baseline::BaselineEvaluator` — play one complete Swap2 game
against each other. The runner returns an in-memory `Game`: the opening
with its provenance, the ordered ply records, and the terminal outcome.
One seed fixes the entire game; the same seed and the same seat options
reproduce it exactly. The chapter's public surface is `play_game`,
`play_game_with_opening`, `Game`, `PlyRecord`, `PlySearch`, `Outcome`,
`WinReason`, `Opening`, `OpeningAnswer`, `OpeningProvenance`,
`GeneratedOpening`, and the `OpeningSource` trait. Nothing is written to
disk yet: recording arrives in chapter 02, the search-record semantics in
chapter 03, and curated openings plus the exploration schedule in
chapter 04. The intended reader is the learner who has completed the
engine, Monte Carlo Tree Search (MCTS), and baseline tutorials and has read the tutorial
[primer](../00-selfplay-primer.md).

## Glossary

| Term | Definition |
|------|------------|
| **Seat** | One of the two players of a generated game: a persistent `player::AiPlayer` running the baseline evaluator at a declared simulation budget. Every game pairs two seats. |
| **Simulation budget** | The number of Monte Carlo Tree Search (MCTS) simulations a seat runs per move decision; written baseline-X for a budget of X. |
| **Swap2** | The project's opening protocol: one side places three stones, the other side chooses a color or places two more stones, and the first side then chooses a color. |
| **Opening offer** | The three stones placed in the first phase of the Swap2 ceremony. |
| **Typestate** | A Rust pattern in which the phase of a protocol is a type parameter. The engine's `Swap2<State>` machine makes illegal opening sequences unrepresentable. |
| **Absolute colors** | The engine's storage convention: `Board` holds Black and White bitboards as-is, never a relative me/you view. Chapter 13 of the engine design locks this because Swap2 places colors non-alternatingly. |
| **Ply** | One move played after the opening. Ply indices in this chapter are zero-based and count post-opening moves only. |
| **Provenance** | The recorded origin of an opening: generated from the game's random stream, or drawn from an external list. |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games. It never runs by default; it runs on explicit selection, in release mode. |
| **Certified forced win** | A forced-win line produced by the engine's threat-space search (TSS) prover and checked by `engine::verify_line`. The prover may be incomplete, never unsound: budget exhaustion yields *unknown*, never a loss. |
| **Corpus** | A set of game records produced at one declared pairing, stored as shards plus a manifest. Chapters 02 and 04 build corpora; this chapter builds the single game. |
| **Dependency island** | The project's crate rule for `engine`: no deep-learning framework, no randomness, and no input/output inside it; other crates depend on `engine`, never the reverse. |
| **π (visit-distribution target)** | The normalized root visit-count distribution of the search at a ply; the policy training target (Silver et al. 2017). Chapter 03 owns it; this chapter only retains the raw counts. |

## Context

Three prerequisite tracks supply everything this chapter assembles.
Each summary states what the track delivered and why the runner needs
it.

- [Engine tutorial](../../13-engine-tutorial/README.md) — built `engine`,
  the rules crate: the bitboard `Board` with absolute colors, `Status`,
  legality, and the Swap2 typestate `Swap2<State>` with its
  `Placing3` → `FirstChoice` → `Placing2` → `FinalChoice` phases. The
  runner drives the opening ceremony and every move through this API,
  so legality is enforced by the same code that adjudicates the
  outcome.
- [MCTS tutorial](../../mcts-tutorial/README.md) — built `mcts`, the PUCT
  (predictor UCT; Rosin 2011) search behind an `Evaluator` trait seam.
  Both seats think with this search; the runner never touches the tree
  directly.
- [Baseline tutorial](../../baseline-tutorial/README.md) — built
  `baseline`, the hand-written evaluator, and `player`, whose
  `AiPlayer` couples one persistent evaluator to per-decision
  `PlayerOptions`: simulation budget, temperature and noise, an
  own-side threat-space search (TSS; Allis 1994) probe, a guard, and an opt-in
  resignation heuristic.
  `AiPlayer::decide` returns a `Decision` whose `DecisionAction` is
  either `Play(mv)` or `Resign(reason)`, with optional `SearchStats`.
  Two `AiPlayer` instances are the seats of every generated game.

The [tutorial README](../README.md) fixes the conventions of this
track — four papers per chapter, test-driven development (TDD) rhythm,
release mode for game-playing verification — and the
[primer](../00-selfplay-primer.md) pins why the generator exists and
what the record format becomes. The
[design spec](../../../specs/2026-11-11-selfplay-corpora-design.md)
remains the design authority; its decision S6 fixes this chapter's
shape: synchronous, single-threaded, one seed per game, no I/O, no
channels, no GPU.

## Intention

You can ask the `selfplay` crate for one complete game between two
baseline seats and get back something you can inspect: which opening
was played and where it came from, every move in order, what the
search saw when it chose each move, and how the game ended. Given the
same seed and the same seat options, you get the same game back —
byte for byte, move for move. You can replay the recorded moves
through the engine and watch them land legally on the recorded
outcome. That inspectable, reproducible game is the raw material every
later chapter consumes: chapter 02 persists it, chapter 03 refines the
search data inside it, chapter 04 produces games that differ from each
other on purpose.

## Specification as tests

The tests live in
[`gomoku/crates/selfplay/tests/game.rs`](../../../../gomoku/crates/selfplay/tests/game.rs).
The list below is deliberately ordered along the execution path of the
runner — the opening ceremony first, then the move loop, then terminal
accounting, then determinism and inspection. It is not the reference
file's lexical order, which leads with the chapter-03 `PlySearch`
tests. The TDD checklist at the end of the chapter repeats this
presentation order because it is the intended red/green sequence for
working the chapter. This chapter specifies **five** tests. Three run by
default; two are MANUAL game-playing tests marked `#[ignore]`. The
test helpers are a local `options(simulations)` that builds
`PlayerOptions` with a chosen budget, and a `FixedOpening` struct that
implements `OpeningSource` by returning one stored `Opening`.

One count note, stated now so the numbers stay honest: the completed
reference's `tests/game.rs` holds seven tests, not five. The two
others — `ply_search_preserves_the_side_to_move_root_value_sign` and
`ply_search_truncates_prior_to_top_16_without_reordering` — specify
the `PlySearch` conversion from `player::SearchStats`. They belong to
chapter 03's specification and are listed there.

### The opening source and the Swap2 ceremony

- `injectable_opening_executes_the_add_two_swap2_branch` — a
  `FixedOpening` carries a five-stone opening: initial stones
  (5, 8), (5, 10), (7, 9) and answer `OpeningAnswer::AddTwo` with
  stones (9, 8), (9, 10) and final choice White, under provenance
  `OpeningProvenance::Listed { list: "fixed-test", index: 11 }`.
  Executing `opening.board()` completes the ceremony: 220 cells
  remain empty and White is to move. Playing a game from this source
  with seed 11 and budget 1 per seat completes; the returned `Game`
  reports the injected opening verbatim (`game.opening` equals the
  injected value, provenance included) and records exactly 10 plies.

This one test specifies three properties at once: an opening source
is injectable (the runner does not hard-code where openings come
from), the `AddTwo` branch of the ceremony executes through the
engine's typestate, and the opening — including its provenance —
round-trips into the returned `Game`.

### Legal play through the runner

- `recorded_moves_replay_legally_to_the_recorded_outcome` — play one
  generated game (seed 11, budget 1 per seat), rebuild the board from
  `game.opening.board()`, then replay every recorded move. Every move
  passes `board.is_legal` before `board.play` accepts it. After the
  last ply the engine's `Status` is terminal — `Won(winner)` maps to
  the recorded `Outcome::Won` with reason `WinReason::FiveOrMore`,
  `Draw` maps to `Outcome::Draw` — and equals the recorded outcome. A
  still-`Ongoing` board after the last recorded ply fails the test.

This is the legality gate in miniature: the record the runner
produces must replay through the engine without error, and the
recorded outcome must be the engine's own verdict.

### Terminal outcomes

No dedicated chapter-01 test forces a resignation or a draw; the
types pin the semantics and the tests above cover the mapping. The
specification, as implemented:

- `Outcome` has exactly two variants: `Won { winner, reason }` and
  `Draw`. There is no *unknown* outcome and no adjudicated loss —
  every returned game ended on the board or by concession.
- `WinReason` is `FiveOrMore` (the winner completed a line of at
  least five stones — a line longer than five also wins) or
  `Resignation(ResignReason)`. The `ResignReason` vocabulary belongs
  to the `player` crate: resignation is a concession backed by
  verified opponent refutations, not an engine ruling.
- Resignation is reachable only when a seat's options enable it.
  `PlayerOptions::default()` sets `resign: false`, and the runner's
  documentation reserves default options for ordinary corpus runs —
  so this chapter's games end on the board. The MANUAL smoke test
  below asserts the returned `Game` preserves the supplied options
  with resignation still disabled.

### Determinism and inspection

- `same_seed_and_seat_options_reproduce_entire_game` — two calls of
  `play_game(11, options(1), options(1))` return structurally equal
  games: `assert_eq!(first, second)` passes on the whole `Game` (the
  type derives `PartialEq`), the move lists from `game.moves()` are
  identical, and the outcomes are identical.

Determinism is a property of the whole record, not just the move
list: opening, plies, search metadata, and outcome all agree.

### MANUAL game-playing tests

Two tests play slower games and stay `#[ignore]`d with MANUAL
comments, so default test runs stay fast. Run them deliberately, in
release mode (see Verification commands).

- `tiny_budget_game_completes_and_records_search_metadata` — budget 8
  per seat. The game completes; the ply list is non-empty and holds
  at most 222 entries (a full board minus the three stones of a
  three-stone opening); `game.moves().count()` equals the ply count;
  at least one ply carries search metadata; every recorded root value
  is finite; both recorded option sets keep resignation disabled. The
  test prints the ply count, the outcome, and the elapsed time. Its
  MANUAL comment warns that this game can run for minutes in a debug
  build.
- `tiny_budget_game_records_raw_visits_for_every_searched_ply` —
  budget 2 for Black, budget 1 for White. For every ply that carries
  search metadata: the visit list is non-empty, every count is
  positive, and the counts sum to the deciding seat's simulation
  budget — 2 on Black's plies, 1 on White's. At least one searched
  ply exists. This is the runner-level contract for the raw material
  of the policy target π — the normalized root visit distribution of
  the deciding search; chapter 03 owns what the record does with it.

## Walkthrough

The order is the execution order of one game. Each step names the
artifact at the moment execution first touches it.

### 1. Two entry points, one implementation

`play_game(seed, black_options, white_options)` is the convenience
entry: it delegates to
`play_game_with_opening(seed, black_options, white_options, opening_source)`
with a fresh `GeneratedOpening`. Everything below happens inside
`play_game_with_opening`, which returns `Result<Game, GameError>`.
`GameError` names the three ways a run can fail: `Opening` when the
engine rejects the ceremony, `Decision` when a seat is asked to
decide on a terminal board, and `Play { ply, mv, source }` when the
engine rejects a selected move — with `ply` the zero-based
post-opening index.

### 2. One seed, one random stream

The runner's first act is to build one `StdRng` (the `rand` crate's
standard random number generator) from the seed. That
single stream feeds the opening draw and both seats' move selection —
the seed is the complete identity of the game. This is the seed
discipline the whole track relies on: chapter 02 stores the seed in
the record header, and the determinism test above is the in-memory
form of the project's byte-exact regeneration rule. It works because
everything downstream of the stream is deterministic: the baseline
evaluator has no hidden state, and the search consumes randomness
only through this stream.

### 3. The opening source draws a description

`OpeningSource` is a one-method trait: given the game's `StdRng`,
produce an `Opening`. The draw cannot fail — the method returns
`Opening`, not `Result`. Error behavior is therefore exact: a
malformed description surfaces only at execution (step 4), and a
list-backed source — chapter 04's artifact — validates at load time
instead.

An `Opening` is a complete Swap2 description: `initial`, the three
offer stones in placement order; `answer`, an `OpeningAnswer`; and
`provenance`, an `OpeningProvenance`. `OpeningAnswer` is either
`Take(color)` — finish after three stones by taking this color — or
`AddTwo { stones, final_choice }` — add one White and one Black stone
in placement order, then take a color. `OpeningProvenance` is
`Generated` or `Listed { list, index }`; this chapter produces only
`Generated`, and the record keeps the distinction so chapter 04's
list-drawn openings never masquerade as generated ones.

`GeneratedOpening` is this chapter's source: a unit struct that draws
three unique cells from the central 7×7 region (rows and columns 4
through 10) and then answers uniformly — take Black, take White, or
add two more unique central stones and take a random color. The
central region is deliberate: nearby stones produce representative
tactical games instead of scattered full-board play. It is the
stand-in for curated openings until chapter 04 loads the tournament
lists.

### 4. The ceremony executes through the typestate

`Opening::board()` replays the description through the engine's
`Swap2<State>` machine: `Swap2::<Placing3>::new()`, three `place`
calls with explicit colors — Black, White, Black — then `finish()`
into `FirstChoice`. There the answer branches: `take_black()` or
`take_white()` return the playing `Board` directly; `place_two_more()`
continues to `Placing2`, where White and then Black are placed and
`finish()` yields `FinalChoice`, whose `take_black()`/`take_white()`
return the `Board`. Illegal ceremonies are unrepresentable — the
typestate has no method for the wrong order — and the only execution
error is `PlayError::Occupied`, raised when a description reuses a
cell. The runner wraps it as `GameError::Opening`.

The explicit colors are not decoration. The engine stores absolute
colors precisely because Swap2 places colors non-alternatingly
([engine design](../../../13-engine-design.md), decision 5), so an
opening description names real colors, never relative sides. Both
branches of the ceremony leave White to move; the ceremony test
asserts exactly that for the five-stone branch.

### 5. Two seats, bound to absolute colors

The runner builds two persistent seats, one per color. Each seat is
an `AiPlayer` constructed once, before the loop, around a boxed
`BaselineEvaluator` whose backend is selected with `Device::flex()`;
the seat then owns that evaluator for the whole game while every
decision runs a fresh search — that is the `player` crate's
contract. `Device::flex()` is the baseline evaluator's backend
selector; it is the only value from the Burn deep-learning framework
that the runner touches, and
`engine` itself stays a dependency island far below.

Seat identity *is* the absolute color. The seat configured by
`black_options` plays Black in every game; `board.to_move()` — not
turn order, not the previous game — selects which seat decides. The
Swap2 answers come from the seeded random stream rather than from the
seats, so color choice is opening diversity, not seat negotiation,
and nothing alternates seat colors between games. When chapter 04
introduces graded pairings, the stronger seat keeps a fixed color
across a corpus — a property the later chapters discuss on its own
merits.

### 6. The move loop

Each iteration checks `board.status()` **first**. `Status::Won` ends
the game as `Outcome::Won` with reason `WinReason::FiveOrMore`;
`Status::Draw` ends it as `Outcome::Draw`. Only on `Status::Ongoing`
does the runner ask a seat to decide — so `DecisionError::GameOver`,
surfaced as `GameError::Decision`, is the defensive seam a correct
loop never hits.

The deciding seat receives the board, its options, and the shared
random stream: `seat.decide(&board, &options, &mut rng)`. The base
options pass through unchanged at each ply; chapter 04 inserts the
exploration schedule at exactly this point, and the returned `Game`
preserves the base options verbatim so the record always shows what
was asked for. A `Decision` carries an action plus what the decision
rested on. When the seat's own-side TSS probe is enabled and finds a
certified forced win, the decision plays the first move of the
verified line immediately — no search runs, and the decision carries
no search statistics. When the probe finds nothing, the result is
*unknown*, and unknown means ordinary search — never a resignation,
never a loss.

### 7. Playing and recording one ply

A `DecisionAction::Play(mv)` is proposed to the engine:
`board.play(mv)`. The engine remains the legality gate even though
the seat proposed the move; a rejection is `GameError::Play`. The
accepted move is appended as a `PlyRecord`: `mv`, the played move,
and `search`, an `Option<PlySearch>`.

`PlySearch` is this chapter's window into the search — and chapter
03's subject. Its fields are `side_to_move`, `visits` (raw counts per
root child), `prior`, and `root_value`. This chapter requires only
two properties: the field is present when search happened and absent
for a certified-proof move, because fabricating statistics for a move
no search produced would be dishonest telemetry; and the raw visit
counts sum to the deciding seat's budget, as the MANUAL visit-sum
test pins. That same test —
`tiny_budget_game_records_raw_visits_for_every_searched_ply` — pins
every recorded count as positive, so this chapter's runner already
drops zero-count visit entries: an unvisited root child carries no
information and contributes nothing to the sum. What chapter 03 owns
is everything built on that sparsity: the `From<SearchStats>`
conversion, the sparse record's read-time and normalization
semantics, the top-k prior truncation, and the sign-convention tests
(`ply_search_preserves_the_side_to_move_root_value_sign` and
`ply_search_truncates_prior_to_top_16_without_reordering`).

A `DecisionAction::Resign(reason)` ends the game instead:
`Outcome::Won` with the winner `side.other()` — the *other* color
wins when a seat concedes — and reason `WinReason::Resignation`. With
default options the arm is unreachable, because the heuristic is
disabled and because it only fires on verified opponent refutations:
an unrefuted candidate is unknown, and unknown cancels resignation.

### 8. The returned `Game`

The loop exits with an `Outcome`, and the runner assembles the
`Game`: the seed, both supplied option sets, the opening with its
provenance, the ordered plies, and the outcome. Nothing is written to
disk — persistence is chapter 02 — and nothing is thrown away that
chapter 02 needs. `Game` derives `Debug` for inspection,
`Clone`, and `PartialEq` for whole-game comparison, which is what makes the
determinism test a one-liner. `game.moves()` iterates the played
moves in order, excluding the opening stones, with an exact size; the
opening stones live in `game.opening`. Termination is bounded by the
board: at most 222 plies after a three-stone opening, 220 after a
five-stone one, and a full board without a completed line is the
draw. Chapter 05 later teaches this same loop to notice when a
certified forced-win line was followed to the end; the hook is not
this chapter's work.

## What this chapter defers

- Recording `Game` values to shards and a manifest — chapter 02.
- The rest of the `PlySearch` semantics — the `From<SearchStats>`
  conversion, the sparse record's read-time and normalization
  semantics, the top-k prior truncation, and the sign-convention
  tests — chapter 03. This chapter's runner already drops zero-count
  visit entries, because the MANUAL
  `tiny_budget_game_records_raw_visits_for_every_searched_ply` test
  pins positive counts; only the semantics built on that sparsity are
  deferred.
- Tournament opening lists, the exploration schedule, seeded
  generation with deduplication, the generator binary — chapter 04.
- The `CertifiedSuffix` record type and its slot in `GameRecord` —
  chapter 02, with the record format. Teaching the runner to notice
  when a certified forced-win line was followed to the end, and
  extracting anchors from the recorded suffixes — chapter 05.
- Replay, sampling, and export — chapter 06.
- Acceptance gates and corpus statistics — chapter 07.

## TDD checklist

Work one artifact at a time, red to green, in
[`gomoku/crates/selfplay/`](../../../../gomoku/crates/selfplay/).
Register the crate in the
[workspace manifest](../../../../gomoku/Cargo.toml) first; the crate
manifest depends on `engine`, `baseline`, and `player` by path, plus
`rand`, `thiserror`, and the workspace `serde`, and `src/lib.rs`
carries `#![deny(missing_docs)]`. Derive serde serialization on the
opening and outcome enums now — chapter 02 serializes them, and
declaring it now keeps chapter 02 out of the vocabulary types. That
derive is why `serde` is in the manifest from the start; `serde_json`
and the `train` path dependency wait for chapter 02, where the corpus
writer first uses them.

Deriving on the outcome enums pulls one type across a crate boundary:
`WinReason::Resignation` embeds `player::ResignReason`, so the
`player` crate needs the same treatment. Add the workspace `serde`
to the `player` manifest and derive `Serialize` and `Deserialize` on
`ResignReason` — a small, deliberate touch to a finished crate, and
the only one this chapter makes. Nothing else in `player` changes,
and `cargo test -p player` must stay green to prove it.

1. The opening vocabulary — `OpeningProvenance`, `OpeningAnswer`,
   `Opening`, and `Opening::board()` through the engine typestate.
2. The `OpeningSource` trait and `GeneratedOpening`.
3. The outcome vocabulary — `WinReason`, `Outcome` — and the ply
   vocabulary — `PlySearch`, `PlyRecord`, `Game`, `Game::moves()`.
4. `GameError`, then `play_game_with_opening`, then the `play_game`
   convenience wrapper.
5. `injectable_opening_executes_the_add_two_swap2_branch` — red, then
   green. This is the moment the ceremony and the loop first run end
   to end.
6. `recorded_moves_replay_legally_to_the_recorded_outcome`.
7. `same_seed_and_seat_options_reproduce_entire_game`.
8. `tiny_budget_game_completes_and_records_search_metadata` and
   `tiny_budget_game_records_raw_visits_for_every_searched_ply`,
   written `#[ignore]`d with their MANUAL comments, then run once in
   release mode.

## Capstone

Play one real game and watch the accounting:

```text
cargo test -p selfplay --release --test game -- --ignored tiny_budget_game_completes_and_records_search_metadata --nocapture
```

The test prints one line: the ply count, the outcome, and the elapsed
time. That line is a complete baseline-versus-baseline game — Swap2
ceremony, legal play, terminal verdict — produced entirely by code
this chapter owns, reproducible to the move by its seed.

## Done when

- The three default tests pass; `cargo test -p selfplay` reports
  3 passed, 2 ignored for the game target. (In the completed
  reference the same target reports 5 passed, 2 ignored; the two
  extra tests belong to chapter 03.)
- Both MANUAL tests pass under the release command below.
- `cargo test -p player` is green after the `ResignReason` derive.
- `cargo clippy --all-targets -- -D warnings` is green.
- `cargo fmt --all -- --check` makes no changes.
- The commit message is
  `feat(selfplay): the game runner — selfplay-corpora chapter 01`.

## Verification commands

From `gomoku/`:

```text
cargo test -p selfplay
cargo test -p selfplay --release --test game -- --ignored
cargo test -p player
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the fast check: the default tests play budget-1
games and finish in seconds even in a debug build. The second command
is the slow game-playing verification, and both of its qualifiers
matter. `--release` is not optional: the MANUAL comment on the
budget-8 smoke test warns of minutes in a debug build. `--test game`
focuses the run on this chapter's target — in the completed reference
a bare `-- --ignored` would also select the MANUAL tests of later
chapters (the schedule and grading targets), which this chapter has
no business running. Naming the tests works too:
`-- --ignored tiny_budget` selects exactly the two MANUAL tests by
their shared prefix. The `player` command guards the chapter's one
cross-crate touch: the serde derives on `ResignReason`.

## References

**This track:**

- [Tutorial README](../README.md) — conventions, gates, and the
  chapter map.
- [Self-play primer](../00-selfplay-primer.md) — why strength-graded
  self-play data; the invariants this chapter starts from.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  decision S6 (the runner's shape) and the record format this
  chapter's `Game` grows into.
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  the per-chapter public surface and the measured evidence.

**Prerequisite tracks:**

- [Engine tutorial](../../13-engine-tutorial/README.md) and
  [engine design](../../../13-engine-design.md) — the Swap2 typestate and
  the absolute-colors decision (decision 5).
- [MCTS tutorial](../../mcts-tutorial/README.md) — the PUCT search behind
  the `Evaluator` seam.
- [Baseline tutorial](../../baseline-tutorial/README.md) — the seats:
  `AiPlayer`, `PlayerOptions`, the own-side TSS probe, and the
  honesty rule for certified moves.

**Source:**

- [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs) —
  the runner, `Game`, `PlyRecord`, `PlySearch`, `Outcome`,
  `WinReason`, `GameError`. Created in this chapter.
- [`gomoku/crates/selfplay/src/opening.rs`](../../../../gomoku/crates/selfplay/src/opening.rs) —
  `Opening`, `OpeningAnswer`, `OpeningProvenance`, `OpeningSource`,
  `GeneratedOpening`. Created in this chapter.
- [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs) —
  the crate's public surface. Created in this chapter.
- [`gomoku/crates/selfplay/tests/game.rs`](../../../../gomoku/crates/selfplay/tests/game.rs) —
  this chapter's specification. Created in this chapter.
- [`gomoku/crates/engine/src/opening.rs`](../../../../gomoku/crates/engine/src/opening.rs) —
  the `Swap2<State>` typestate.
- [`gomoku/crates/engine/src/board.rs`](../../../../gomoku/crates/engine/src/board.rs) —
  `Board`, `Color`, `Status`, `PlayError`.
- [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs) —
  `AiPlayer`, `Decision`, `DecisionAction`, `PlayerOptions`,
  `SearchStats`, `ResignReason`, `DecisionError`. Touched in this
  chapter: `ResignReason` gains serde serialization.
- [`gomoku/crates/baseline/src/lib.rs`](../../../../gomoku/crates/baseline/src/lib.rs) —
  `BaselineEvaluator` and the `Device` re-export.

**Companion papers:**

- [the-game-runner-design.md](the-game-runner-design.md) —
  the conceptual story of the runner.
- [the-game-runner-implementation.md](the-game-runner-implementation.md) —
  the hiding layer: low-level construction instructions.
- [the-game-runner-solution.md](the-game-runner-solution.md) —
  the annotated reference snippets.

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). https://doi.org/10.1038/nature24270 —
  the visit-distribution policy target π whose raw material the
  runner retains.
- Rosin, C. D. (2011). *Multi-armed bandits with episode context*
  (PUCT). https://doi.org/10.1007/s10472-011-9258-6 — the selection
  rule of the search both seats run.
- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://doi.org/10.26481/dis.19940923la — threat-space search, the
  seat's certified-forced-win probe.
