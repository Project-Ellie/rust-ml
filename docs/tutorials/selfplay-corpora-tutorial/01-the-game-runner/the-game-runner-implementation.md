# Chapter 01 — The game runner: implementation plan

## Abstract

This document is the implementation plan for
[chapter 01 of the selfplay-corpora tutorial](the-game-runner.md): the
hiding layer between the chapter paper and the solution, for the
learner who builds the code without reading the reference. It turns
the chapter's walkthrough into a numbered construction order along the
execution path of one game: workspace registration, the crate
manifest, one deliberate touch to the `player` crate, the opening
vocabulary, the Swap2 ceremony, the outcome and ply vocabulary, the
error type, the runner itself, and the five tests of the chapter's
specification — three running by default, two MANUAL
(game-playing, `#[ignore]`d) tests. Every artifact names its file, its
behavioral contract in prose, the helpers to reuse, and the idioms to
apply by name; method bodies never appear. The intended reader is the
learner who has read the chapter paper and, ideally, the
[design paper](the-game-runner-design.md), and wants low-level
guidance without solution leakage.

## Glossary

| Term | Definition |
|------|------------|
| **Seat** | One of the two players of a generated game: a persistent `player::AiPlayer` running the baseline evaluator at a declared simulation budget. Every game pairs two seats. |
| **Simulation budget** | The number of Monte Carlo Tree Search (MCTS) simulations a seat runs per move decision; written baseline-X for a budget of X. |
| **Swap2** | The project's opening protocol: one side places three stones, the other side chooses a color or places two more stones, and the first side then chooses a color. |
| **Opening offer** | The three stones placed in the first phase of the Swap2 ceremony. |
| **Typestate** | A Rust pattern in which the phase of a protocol is a type parameter. The engine's `Swap2<State>` machine makes illegal opening sequences unrepresentable. |
| **Absolute colors** | The engine's storage convention: `Board` holds Black and White bitboards as-is, never a relative me/you view. |
| **Ply** | One move played after the opening. Ply indices in this chapter are zero-based and count post-opening moves only. |
| **Provenance** | The recorded origin of an opening: generated from the game's random stream, or drawn from an external list. |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games. It never runs by default; it runs on explicit selection, in release mode. |
| **Certified forced-win proof** | A forced-win line produced by the engine's threat-space search (TSS) prover and machine-verified by `engine::verify_line`. The prover may be incomplete, never unsound: *unknown* is not a loss and not a proof. |
| **π (visit-distribution target)** | The normalized root visit-count distribution of the deciding search at a ply; the policy training target (Silver et al. 2017). Chapter 03 owns it; this chapter only retains the raw counts. |
| **TDD** | Test-driven development: write the test, watch it fail (red), implement, watch it pass (green). |
| **RNG** | Random number generator; here the `rand` crate's `StdRng`. |

## How to use this plan

Read [the chapter paper](the-game-runner.md) first. It carries the
intention, the specification as tests, and the execution-order
walkthrough; the [design paper](the-game-runner-design.md) carries the
reasoning. This plan assumes both and adds only construction detail:
which file, which item, which contract, which test goes red then
green. Work one step at a time, in order — the order is the execution
order of one game. Open
[the-game-runner-solution.md](the-game-runner-solution.md)
only if you are stuck for more than twenty minutes, or after the
chapter is complete.

## Context

Three existing crates supply everything this chapter assembles; the
chapter paper's Context section summarizes each in full. In one
paragraph: [`engine`](../../../../gomoku/crates/engine/src/board.rs)
provides the bitboard `Board` in absolute colors, `Status`, legality,
and the Swap2 typestate `Swap2<State>`;
[`player`](../../../../gomoku/crates/player/src/lib.rs) provides
`AiPlayer`, whose `decide` returns a `Decision` with a
`DecisionAction` of `Play(mv)` or `Resign(reason)` plus optional
`SearchStats`; and
[`baseline`](../../../../gomoku/crates/baseline/src/lib.rs) provides
the hand-written `BaselineEvaluator` and re-exports `Device`. The work
is to create the `selfplay` crate with a synchronous, single-threaded
runner that plays one complete game between two seats and returns an
inspectable, reproducible in-memory `Game`.

Two scope guards keep the chapter slice honest. The reference test
file grows to seven tests in later chapters; this chapter writes
exactly five — three default, two MANUAL. The two further default game
tests (`ply_search_preserves_the_side_to_move_root_value_sign` and
`ply_search_truncates_prior_to_top_16_without_reordering`) specify the
`PlySearch` conversion from `player::SearchStats` and belong to
chapter 03; do not write them here. And `PlySearch` itself is a slot
in this chapter: the struct exists and the runner fills it, but the
`From<SearchStats>` conversion, the recorded top-k prior truncation,
and the sign-convention tests are chapter 03's work. No certified
suffix, no corpus, no tournament openings, no exploration schedule, no
binary appears anywhere in this chapter.

---

## Step 0 — `gomoku/Cargo.toml`: register the workspace member

In the [workspace manifest](../../../../gomoku/Cargo.toml), append
`"crates/selfplay"` to the `workspace.members` array, after
`"crates/net"`. Keep the existing members in their current order.

Tests that go red then green: none; the workspace simply has no crate
to build yet.

---

## Step 1 — `gomoku/crates/selfplay/Cargo.toml`: the crate manifest

Create `gomoku/crates/selfplay/Cargo.toml`.

### 1.1 Package metadata

Use `name = "selfplay"` and the description "Synchronous baseline
self-play game runner." Pull `version`, `edition`, and `license` from
the workspace.

### 1.2 Dependencies

Add `engine`, `baseline`, and `player` as path dependencies. Add `rand`
at version `"0.10.3"` — the same version `player` and `baseline`
already use, so the whole workspace agrees on one RNG API. Add `serde`
and `thiserror` from the workspace dependency table.

Two deliberate absences. `serde_json` and the `train` path dependency
wait for chapter 02, where the corpus writer first uses them; `serde`
is here now only because the opening and outcome enums derive
serialization in this chapter (step 4 and step 7), which keeps chapter
02 out of the vocabulary types. And Burn never appears: the backend
selector `Device` reaches the runner through the `baseline` crate's
re-export, so the runner touches exactly one Burn value
(`Device::flex()`) without naming the framework, and `engine` stays a
dependency island — no deep-learning framework, no randomness, no
input/output (I/O) inside it, and no dependency from `engine` back to
any consumer.

---

## Step 2 — the `player` touch: serde for `ResignReason`

This is the chapter's only cross-crate change, and it exists for one
reason: step 7's `WinReason::Resignation` embeds
`player::ResignReason`, and `WinReason` derives serde serialization —
so the embedded type must serialize too.

### 2.1 The manifest

In [`gomoku/crates/player/Cargo.toml`](../../../../gomoku/crates/player/Cargo.toml),
add the workspace `serde` to the dependency table.

### 2.2 The derive

In [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs),
extend the existing derive list on `ResignReason` with serde's
`Serialize` and `Deserialize`. Change nothing else in the crate: no
other type, no behavior, no test.

### 2.3 Gate

Run `cargo test -p player` from `gomoku/`. It must stay exactly as
green as before the touch — that is the proof nothing else changed.

---

## Step 3 — `gomoku/crates/selfplay/src/lib.rs`: the skeleton

Create `gomoku/crates/selfplay/src/lib.rs` with three things:

1. A crate-level doc comment: the crate is synchronous,
   single-threaded baseline self-play; one seeded game runs a complete
   Swap2 ceremony and then alternates two persistent baseline seats;
   the result lives entirely in memory (opening provenance, per-seat
   options, ordered plies, search metadata, outcome).
2. `#![deny(missing_docs)]`, so every public item — including every
   public field and every enum variant — needs a doc comment from the
   moment it exists.
3. Nothing else yet. Modules are declared as their files appear
   (steps 4 and 7) and stay private — no `pub mod`. The crate-root
   `pub use` re-exports, however, are not one late step: each
   re-export lands as the integration tests first need it. Step 9's
   test file imports every name it uses from the crate root, so the
   opening and runner re-exports must exist before step 10 can turn
   that test green (the checkpoint lives in step 10); step 14 is the
   final audit of exactly the public surface, not the first time
   exports appear.

Compile check: `cargo check -p selfplay` from `gomoku/`.

---

## Step 4 — `src/opening.rs`: the opening vocabulary

Create `gomoku/crates/selfplay/src/opening.rs` and declare the module
in `lib.rs`. This step writes description types only — nothing
executes yet.

### 4.1 `OpeningProvenance`

A public enum with two variants: `Generated` (stones and answers drawn
from the game's seeded RNG stream) and `Listed { list, index }` — an
opening selected from an external, ordered list, where `list` is a
`String` naming the list's stable identity and `index` is the
zero-based position within it. Derive `Debug`, `Clone`, `PartialEq`,
`Eq`, and serde's `Serialize` and `Deserialize`.

This chapter produces only `Generated`, but the distinction exists
from the start so chapter 04's list-drawn openings never masquerade as
generated ones.

### 4.2 `OpeningAnswer`

A public enum with two variants: `Take(Color)` — finish after three
stones by taking this color — and `AddTwo { stones, final_choice }`,
where `stones` is a fixed-size array of two `Move` values in placement
order (White first, then Black) and `final_choice` is the `Color`
taken after the five-stone offer. Derive the same set as
`OpeningProvenance`, plus `Copy` (the enum holds only `Copy` data).

The colors are absolute, never relative sides. Swap2 places colors
non-alternatingly — Black, White, Black, then possibly White, Black —
so a description could not name relative sides at all. This is the
engine design's locked absolute-colors decision applied to the
opening vocabulary.

### 4.3 `Opening`

A public struct with three public fields: `provenance`
(an `OpeningProvenance`), `initial` (a fixed-size array of three
`Move` values — the offer stones in placement order Black, White,
Black), and `answer` (an `OpeningAnswer`). Derive `Debug`, `Clone`,
`PartialEq`, `Eq`, and serde's `Serialize` and `Deserialize` — not
`Copy`, because the provenance owns a `String`.

### 4.4 `Opening::new`

A public `const fn` constructor that accepts the three offer stones,
the answer, and the provenance — in that parameter order, which is the
order the chapter's ceremony test calls it — and returns the
description without executing it. Mark it `#[must_use]`.

Tests that go red then green: none yet; compilation under
`#![deny(missing_docs)]` is the gate. Document every field and variant
as you write it.

---

## Step 5 — `src/opening.rs`: `Opening::board()` executes the ceremony

Add one method to `Opening`: it borrows the description and returns
`Result<Board, PlayError>` — the ceremony executed through the
engine's Swap2 typestate. Import `Swap2` and `Placing3` from `engine`
(defined in
[`gomoku/crates/engine/src/opening.rs`](../../../../gomoku/crates/engine/src/opening.rs)).

Behavioral contract, in execution order:

1. Create the ceremony machine in its first phase.
2. Place the three `initial` stones with explicit colors Black, White,
   Black. Each placement returns a `Result`; propagate rejections with
   the `?` operator.
3. `finish()` the offer phase into the first-choice phase, again with
   `?`.
4. Branch on the stored answer. `Take(Color::Black)` and
   `Take(Color::White)` end the ceremony immediately: the matching
   `take_black()` / `take_white()` call consumes the machine and
   returns the playing `Board`. `AddTwo` continues: `place_two_more()`
   moves the machine into the placing-two phase, where the two stored
   stones are placed with explicit colors White then Black (both with
   `?`), `finish()` yields the final-choice phase, and the stored
   `final_choice` selects between `take_black()` and `take_white()`.

The typestate idiom does the heavy lifting: each phase-transition
method consumes the machine by value and returns the next phase's
type, so an illegal ceremony order does not compile. The only
reachable execution error is `PlayError::Occupied`, raised when a
description reuses a cell. Both branches of the ceremony leave White
to move; the chapter's ceremony test asserts exactly that for the
five-stone branch. Write a `# Errors` doc section naming the
`Occupied` condition.

Tests that go red then green: the ceremony half of
`injectable_opening_executes_the_add_two_swap2_branch` (step 9).

---

## Step 6 — `src/opening.rs`: `OpeningSource` and `GeneratedOpening`

### 6.1 The `OpeningSource` trait

A public trait with one method: it mutably borrows the source and
mutably borrows the game's `StdRng`, and returns an `Opening` — not a
`Result`. The draw cannot fail; error behavior is therefore exact: a
malformed description surfaces only at execution (step 5's `Occupied`),
and chapter 04's list-backed source will validate at load time
instead. The doc comment states the stream contract: the runner
supplies the same seeded stream it later uses for move selection, so
one game has exactly one RNG stream and one identity.

### 6.2 `GeneratedOpening`

A public unit struct — this chapter's source, and the stand-in for
curated openings until chapter 04 loads the tournament lists. Derive
`Debug` and `Default`.

### 6.3 `impl OpeningSource for GeneratedOpening`

Behavioral contract:

1. Draw three unique cells from the central 7×7 region — rows and
   columns 4 through 10 inclusive. The central region is deliberate:
   nearby stones produce representative tactical games instead of
   scattered full-board play.
2. Draw the answer uniformly among three outcomes: take Black; take
   White; or draw two more unique central cells — distinct from the
   three offer stones and from each other — and take a random color,
   producing the `AddTwo` answer.
3. Assemble the `Opening` with `OpeningProvenance::Generated`.

Use two private helpers to keep the draw readable: one that draws a
fixed number of unique cells from the central region while excluding a
given set of already-taken cells, and one that draws a random color.
For the first, build the candidate list from the row/column ranges
excluding the taken cells, then select repeatedly by random index,
removing each selected candidate with the swap-remove idiom, and
collect into a fixed-size array with `std::array::from_fn`. For the
random draws, bring the `rand` crate's extension trait into scope and
use its ranged random method on `StdRng` (imported from `rand::rngs`);
`Move::new` (in
[`gomoku/crates/engine/src/moveset.rs`](../../../../gomoku/crates/engine/src/moveset.rs))
validates coordinates and can be `expect`ed here because the region is
on the board by construction.

Tests that go red then green: none of their own; the runner tests
exercise this source through `play_game`.

---

## Step 7 — `src/game.rs`: the outcome and ply vocabulary

Create `gomoku/crates/selfplay/src/game.rs` and declare the module in
`lib.rs`. This step writes records only — nothing plays yet.

### 7.1 `WinReason`

A public enum with two variants: `FiveOrMore` — the winner completed a
line of at least five stones (a longer line also wins) — and
`Resignation(ResignReason)` — a voluntary concession, embedding the
`player` crate's `ResignReason` (the type step 2 made serializable).
Derive `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, and serde's
`Serialize` and `Deserialize`.

### 7.2 `Outcome`

A public enum with exactly two variants: `Won { winner, reason }` —
`winner` is a `Color`, `reason` a `WinReason` — and `Draw`. Same
derive list as `WinReason`. There is no *unknown* outcome and no
adjudicated loss: every returned game ended on the board or by
concession, and the runner only ever maps the engine's terminal
`Status` or a seat's resignation into this type — it never invents
one.

### 7.3 `PlySearch` — the slot

A public struct with four public fields: `side_to_move` (the `Color`
whose perspective determines the root value's sign), `visits` (a
vector of `(Move, u32)` raw visit counts per visited root child),
`prior` (a vector of `(Move, f32)`), and `root_value` (an `f32`).
Derive `Debug`, `Clone`, and `PartialEq` only — serde arrives with the
record format in chapter 02.

Chapter-1-only handling, stated once and held: the struct is a slot.
The runner fills it from a decision's search statistics (step 10
specifies exactly how), but the `From<SearchStats>` conversion, the
recorded top-k prior truncation, and the sign-convention tests belong
to chapter 03. Do not write the conversion impl here.

### 7.4 `PlyRecord`

A public struct with two public fields: `mv` (the played `Move`) and
`search` (an `Option<PlySearch>`). Derive `Debug`, `Clone`, and
`PartialEq`. The honesty rule: the slot is present when a search
produced it and absent when a certified forced-win proof from the
seat's own-side threat-space search (TSS; Allis 1994) probe bypassed
search entirely — fabricating statistics for a move no search produced
would be dishonest telemetry.

### 7.5 `Game`

A public struct with six public fields, in this order: `seed` (a
`u64`), `black_options` and `white_options` (the two supplied
`PlayerOptions` sets, preserved verbatim), `opening` (the `Opening`
with its provenance), `plies` (a vector of `PlyRecord` in play order),
and `outcome` (the `Outcome`). Derive `Debug`, `Clone`, and
`PartialEq` — the whole-record equality is what makes the determinism
test a one-liner. There is deliberately no certified-suffix field and
no suffix tracking anywhere in this chapter; chapters 02 and 05 own
that record and its detection.

### 7.6 `Game::moves()`

One method on `Game`: it borrows the game and returns an exact-size
iterator over the played moves in order, excluding the opening stones
(which live in `game.opening`). Apply the iterator-`map` idiom over
the ply records, and name the return type with `impl
ExactSizeIterator` in return position so callers can count without
collecting.

Tests that go red then green: none yet; compilation is the gate.

---

## Step 8 — `src/game.rs`: `GameError`

A public enum deriving `Debug` and `thiserror::Error`, with three
variants naming the three ways a run can fail:

- `Opening` — the engine rejected the ceremony. Wraps `PlayError` with
  the thiserror `#[from]` idiom, so the `?` on `opening.board()`
  converts automatically. The message states that the Swap2 opening
  failed and embeds the source.
- `Decision` — a seat was asked to decide on a terminal board. Wraps
  `player::DecisionError`, again with `#[from]`. This variant is a
  defensive seam: the status-first loop of step 10 never asks a seat
  to decide on a terminal board, so a correct run never produces it.
- `Play { ply, mv, source }` — the engine rejected a selected move.
  `ply` is the zero-based post-opening ply index, `mv` the rejected
  `Move`, `source` the engine's `PlayError`. The message names all
  three.

Tests that go red then green: none directly; the runner's `?`
operators exercise the `#[from]` conversions.

---

## Step 9 — `tests/game.rs`: helpers and the injectable-opening test (red)

Create `gomoku/crates/selfplay/tests/game.rs`. Write the helpers and
the first test before the runner exists, and watch it fail — for a
brand-new function the red state is a compile error, which is expected
and correct.

### 9.1 Helpers

Three test-local helpers:

- `options(simulations)` — accepts a `u32` budget and returns a
  `PlayerOptions`. Apply the struct-update-syntax idiom over
  `PlayerOptions::default()`, setting only the simulation budget; the
  defaults keep resignation disabled, which this chapter's games rely
  on.
- `mv(row, col)` — accepts two `u8` coordinates and returns a `Move`,
  `expect`ing that the test coordinate is on the board.
- `FixedOpening` — a tuple struct wrapping one `Opening`, implementing
  `OpeningSource` by ignoring the supplied stream (underscore-binding
  idiom) and returning a clone of the stored opening. This is the
  injection seam made concrete: the runner does not hard-code where
  openings come from.

### 9.2 `injectable_opening_executes_the_add_two_swap2_branch`

Contract, in two halves:

1. Ceremony half. Build a fixed five-stone opening: initial stones
   (5, 8), (5, 10), (7, 9); answer `AddTwo` with stones (9, 8),
   (9, 10) and final choice White; provenance `Listed` with list
   `"fixed-test"` and index 11. Execute `opening.board()` and assert
   the ceremony completes with exactly 220 empty cells
   (`Board::empty_moves` counted) and White to move.
2. Game half. Play a game from a `FixedOpening` carrying a clone of
   that description — seed 11, budget 1 per seat — through
   `play_game_with_opening`. Assert the returned game reports the
   injected opening verbatim (the whole `opening` field equals the
   injected value, provenance included) and records exactly 10 plies.

One test, three specified properties: the opening source is
injectable, the `AddTwo` branch of the ceremony executes through the
engine's typestate, and the opening round-trips into the returned
`Game`.

Watch it fail to compile: `play_game_with_opening` does not exist.
That is the red state.

---

## Step 10 — `src/game.rs`: the runner (green)

Two public free functions; one implementation.

### 10.1 `play_game_with_opening`

Accepts the `u64` seed, the Black seat's `PlayerOptions`, the White
seat's `PlayerOptions`, and a mutable borrow of `impl OpeningSource`
(the trait-impl-in-argument-position idiom). Returns
`Result<Game, GameError>`. Behavioral contract, in execution order:

1. **One seed, one stream.** Build one `StdRng` from the seed with the
   `SeedableRng::seed_from_u64` idiom. That single stream feeds the
   opening draw and both seats' move selection for the whole game —
   the seed is the complete identity of the game, because everything
   downstream of the stream is deterministic: the baseline evaluator
   holds no hidden state, and the search consumes randomness only
   through this stream. The determinism claim carries one
   qualification: it holds for the same seed, the same seat options,
   and the same opening-source draw behavior. `play_game` fixes the
   source to `GeneratedOpening`, so for it the seed and the options
   alone suffice.
2. **Draw the opening.** Ask the source for an `Opening`, passing the
   shared stream. The draw cannot fail.
3. **Execute the ceremony.** `opening.board()` with `?`; step 8's
   `#[from]` makes a rejection a `GameError::Opening`.
4. **Build two persistent seats, one per absolute color.** Each seat
   is an `AiPlayer` constructed once, before the loop, around a boxed
   `BaselineEvaluator` whose backend is selected with `Device::flex()`
   (both names come from `baseline`). The seat owns its evaluator for
   the whole game while every decision runs a fresh search — that is
   the `player` crate's contract. Seat identity *is* the absolute
   color: the seat configured by the Black options plays Black in
   every game, and nothing alternates colors between games.
5. **The move loop, status first.** Each iteration checks
   `board.status()` before anything else. `Status::Won(winner)` ends
   the game as `Outcome::Won` with reason `WinReason::FiveOrMore`;
   `Status::Draw` ends it as `Outcome::Draw`. Only on
   `Status::Ongoing` does the loop continue — which is why
   `GameError::Decision` is defensive and unreachable here.
6. **Select the deciding seat by absolute color.** Read
   `board.to_move()` — not a turn counter, not the previous game —
   and select the matching seat and its base option set.
7. **Decide.** Call the seat's `decide` with the board, the base
   options, and the shared stream; `?` converts a `DecisionError` into
   `GameError::Decision`. The base options pass through unchanged at
   every ply; chapter 04 inserts its exploration schedule at exactly
   this point, and the returned `Game` preserves the base options
   verbatim so the record always shows what was asked for.
8. **Play and record.** On `DecisionAction::Play(mv)`: first convert
   the decision's optional search statistics into the `PlySearch` slot
   (contract below), then propose the move to `board.play(mv)`,
   mapping a rejection into `GameError::Play` whose ply index is the
   number of plies recorded so far — zero-based, post-opening. On
   acceptance, append the `PlyRecord`. The engine remains the legality
   gate even though the seat proposed the move.
9. **Resign.** On `DecisionAction::Resign(reason)`, end the game as
   `Outcome::Won` with winner `side.other()` — the *other* color wins
   when a seat concedes — and reason `WinReason::Resignation(reason)`.
   Under default options this arm is unreachable: the heuristic is
   disabled, and it only fires on verified opponent refutations.
10. **Assemble the `Game`.** The seed, both supplied option sets
    verbatim, the opening with its provenance, the ordered plies, the
    outcome. Nothing is written to disk.

The slot conversion, chapter-1 form: apply the `Option::map` idiom to
the decision's search statistics. When statistics are present, build a
`PlySearch` that copies the side to move, keeps only the
positive-count entries of the visit list (an unvisited root child
carries no information and contributes nothing to the sum — the MANUAL
visit-sum test asserts both positivity and the sum), carries the prior
list through untruncated (chapter 03 introduces the recorded top-k),
and copies the root value. When statistics are absent — a certified
own-side TSS proof played its first move without searching — the slot
stays absent.

Termination is bounded by the board: at most 222 plies after a
three-stone opening, at most 220 after a five-stone one, and a full
board without a completed line is the draw.

### 10.2 `play_game`

The convenience entry: accepts the seed and both option sets, returns
`Result<Game, GameError>`, and delegates to `play_game_with_opening`
with a mutable borrow of a fresh `GeneratedOpening` temporary. Its doc
comment states the default-options contract: `PlayerOptions::default()`
disables resignation, and default options are reserved for ordinary
corpus runs — so this chapter's games end on the board. Both functions
carry a `# Errors` doc section naming the three failure sources.

`lib.rs` checkpoint: the test cannot compile until the crate root
re-exports the names its import block lists, so add the `pub use`
statements now — from `opening`: `Opening`, `OpeningAnswer`,
`OpeningProvenance`, `OpeningSource`; from `game`: `Outcome`,
`WinReason`, `play_game`, `play_game_with_opening`. The modules
themselves stay private. Steps 11 to 13 add no new imports to the
test file, so no further re-exports are needed before step 14's
audit.

Watch `injectable_opening_executes_the_add_two_swap2_branch` go green:
the ceremony, the seats, the loop, and the record now run end to end.

---

## Step 11 — `tests/game.rs`: legal replay

Write `recorded_moves_replay_legally_to_the_recorded_outcome`.

Contract: play one generated game (seed 11, budget 1 per seat) through
`play_game`; rebuild the board from `game.opening.board()`; replay
every recorded move, asserting `board.is_legal` before each
`board.play` accepts it (the assertion message names the ply index).
After the last ply, map the engine's `Status` to an `Outcome` —
`Won(winner)` to `Outcome::Won` with `WinReason::FiveOrMore`, `Draw`
to `Outcome::Draw`, and a still-`Ongoing` board panics — and assert it
equals the recorded outcome.

This test should pass immediately: it pins a property the runner
already has by construction (the engine is the only legality gate). If
it goes red, the runner drifted from the contract — fix the runner,
never the test.

---

## Step 12 — `tests/game.rs`: determinism

Write `same_seed_and_seat_options_reproduce_entire_game`.

Contract: call `play_game` twice with seed 11 and budget 1 per seat.
Assert the two returned games are structurally equal as a whole (the
`PartialEq` derive makes this one assertion), that the move lists from
`game.moves()` are identical, and that the outcomes are identical.
Determinism is a property of the whole record — opening, plies, search
metadata, and outcome — not just the move list.

Same rule as step 11: this should pass immediately; a red result means
something in the runner consumes hidden state or randomness outside
the single seeded stream.

---

## Step 13 — `tests/game.rs`: the two MANUAL tests

Both tests play slower games. Mark each `#[ignore]` with a MANUAL
comment — the budget-8 smoke test's comment warns that the game can
run for minutes in a debug build — so default test runs stay fast.
Run them once, in release mode, at the end of this step (command in
step 15).

### 13.1 `tiny_budget_game_completes_and_records_search_metadata`

Budget 8 per seat, seed 11. Record the start instant; play through
`play_game`. Assert: the ply list is non-empty and holds at most 222
entries (a full board minus the three stones of a three-stone
opening); `game.moves().count()` equals the ply count; at least one
ply carries search metadata; every recorded root value is finite
(the `Option::is_none_or` idiom keeps this one expression); and both
recorded option sets keep resignation disabled. Print the ply count,
the outcome, and the elapsed time — the chapter's capstone line.

### 13.2 `tiny_budget_game_records_raw_visits_for_every_searched_ply`

Budget 2 for Black, budget 1 for White, seed 11. For every ply that
carries search metadata: the visit list is non-empty, every count is
positive, and the counts sum to the deciding seat's simulation
budget — selected by the recorded `side_to_move`, 2 on Black's plies
and 1 on White's. Assert at least one searched ply exists. This is the
runner-level contract for the raw material of the policy target π —
the normalized root visit-count distribution of the deciding search
(Silver et al. 2017); chapter 03 owns what the record does with it.

---

## Step 14 — `src/lib.rs`: audit the public surface

Both modules stayed private throughout, and the re-exports arrived
incrementally at the step-10 checkpoint. This step is the final
audit, not the first appearance of exports: the crate root re-exports
exactly the chapter's public surface and nothing more — every listed
item present, no unlisted item added:

- From `game`: `Game`, `GameError`, `Outcome`, `PlyRecord`,
  `PlySearch`, `WinReason`, `play_game`, `play_game_with_opening`.
- From `opening`: `GeneratedOpening`, `Opening`, `OpeningAnswer`,
  `OpeningProvenance`, `OpeningSource`.

If anything in the test file reached for a path deeper than the crate
root, fix the test to use these re-exports. `cargo test -p selfplay`
now reports 3 passed, 2 ignored for the game target — the chapter's
exact slice (see Done when for how the reference's count differs).

---

## Step 15 — Gates

From `gomoku/`:

```text
cargo test -p selfplay
cargo test -p player
cargo test -p selfplay --release --test game -- --ignored
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the fast check: the default tests play budget-1
games and finish in seconds even in a debug build. The second is the
regression gate for step 2's cross-crate touch. The third is the slow
game-playing verification, and both qualifiers matter: `--release` is
not optional (the MANUAL comment warns of minutes in a debug build),
and `--test game` focuses the run on this chapter's target — in the
completed reference a bare `-- --ignored` would also select later
chapters' MANUAL tests, which this chapter has no business running.
Naming the tests works too: `-- --ignored tiny_budget` selects exactly
the two MANUAL tests by their shared prefix. The last two are the
project's standing quality gates; both must be green with no changes.

---

## Done when

- The three default tests pass; `cargo test -p selfplay` reports
  3 passed, 2 ignored for the game target. (In the completed
  reference the same target reports 5 passed, 2 ignored; the two
  extra tests belong to chapter 03.)
- Both MANUAL tests pass under the release command above.
- `cargo test -p player` is green after the `ResignReason` derive.
- `cargo clippy --all-targets -- -D warnings` is green.
- `cargo fmt --all -- --check` makes no changes.
- The commit message is
  `feat(selfplay): the game runner — selfplay-corpora chapter 01`.

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `gomoku/Cargo.toml` | `crates/selfplay` in `workspace.members` | the workspace knows the new crate |
| `selfplay/Cargo.toml` | path deps on `engine`, `baseline`, `player`; `rand` 0.10.3; workspace `serde` and `thiserror` | no `serde_json`, no `train`, no Burn |
| `player/Cargo.toml` + `player/src/lib.rs` | serde on `ResignReason` | the only cross-crate change; `cargo test -p player` green |
| `selfplay/src/lib.rs` | crate doc, `#![deny(missing_docs)]`, incremental crate-root re-exports audited in step 14 | exactly the chapter's public surface |
| `selfplay/src/opening.rs` | `OpeningProvenance`, `OpeningAnswer`, `Opening`, `Opening::new` | description vocabulary with serde derives; absolute colors |
| `selfplay/src/opening.rs` | `Opening::board()` | the ceremony through the typestate; only `Occupied` can fail; White to move on both branches |
| `selfplay/src/opening.rs` | `OpeningSource`, `GeneratedOpening` | infallible draw from the shared stream; central 7×7 unique cells; uniform answer |
| `selfplay/src/game.rs` | `WinReason`, `Outcome` | two variants each; serde derives; no unknown outcome |
| `selfplay/src/game.rs` | `PlySearch`, `PlyRecord` | the slot, without the chapter-03 conversion |
| `selfplay/src/game.rs` | `Game`, `Game::moves()` | six fields; whole-record `PartialEq`; exact-size move iteration excluding the opening |
| `selfplay/src/game.rs` | `GameError` | three variants; `#[from]` on `Opening` and `Decision` |
| `selfplay/src/game.rs` | `play_game_with_opening` | one seeded stream; status-first loop; seats bound to absolute colors; engine is the legality gate; exact `Play` error with zero-based ply |
| `selfplay/src/game.rs` | `play_game` | delegation with a fresh `GeneratedOpening` |
| `selfplay/tests/game.rs` | `options`, `mv`, `FixedOpening` helpers | struct-update options; injectable source |
| `selfplay/tests/game.rs` | three default tests | ceremony injection, legal replay, determinism |
| `selfplay/tests/game.rs` | two MANUAL tests | `#[ignore]`d with MANUAL comments; release-run once |

## References

**This chapter:**

- [the-game-runner.md](the-game-runner.md) — the chapter paper:
  intention, specification as tests, execution-order walkthrough.
- [the-game-runner-design.md](the-game-runner-design.md) — the
  conceptual story and the design invariants this plan builds under.
- [the-game-runner-solution.md](the-game-runner-solution.md)
  — the annotated reference snippets. Open only after
  twenty stuck minutes, or afterwards to compare.

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
  [engine design](../../../13-engine-design.md) — the Swap2 typestate
  and the absolute-colors decision (decision 5).
- [MCTS tutorial](../../mcts-tutorial/README.md) — the search both
  seats think with, behind the evaluator seam.
- [Baseline tutorial](../../baseline-tutorial/README.md) — the seats:
  `AiPlayer`, `PlayerOptions`, the own-side TSS probe, and the honesty
  rule for certified moves.

**Source (live on `main`):**

- [`gomoku/crates/engine/src/opening.rs`](../../../../gomoku/crates/engine/src/opening.rs) —
  the `Swap2<State>` typestate: `Placing3`, `FirstChoice`, `Placing2`,
  `FinalChoice`.
- [`gomoku/crates/engine/src/board.rs`](../../../../gomoku/crates/engine/src/board.rs) —
  `Board`, `Color`, `Status`, `PlayError`.
- [`gomoku/crates/engine/src/moveset.rs`](../../../../gomoku/crates/engine/src/moveset.rs) —
  `Move` and its validating constructor.
- [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs) —
  `AiPlayer`, `Decision`, `DecisionAction`, `PlayerOptions`,
  `SearchStats`, `ResignReason`, `DecisionError`. Touched in this
  chapter: `ResignReason` gains serde serialization.
- [`gomoku/crates/baseline/src/lib.rs`](../../../../gomoku/crates/baseline/src/lib.rs) —
  `BaselineEvaluator` and the `Device` re-export.

**Source (created by this chapter; not yet present on `main`):**

- `gomoku/crates/selfplay/src/game.rs` — the runner, `Game`,
  `PlyRecord`, `PlySearch`, `Outcome`, `WinReason`, `GameError`.
- `gomoku/crates/selfplay/src/opening.rs` — `Opening`,
  `OpeningAnswer`, `OpeningProvenance`, `OpeningSource`,
  `GeneratedOpening`.
- `gomoku/crates/selfplay/src/lib.rs` — the crate's public surface.
- `gomoku/crates/selfplay/tests/game.rs` — the chapter's
  specification.

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). https://doi.org/10.1038/nature24270 —
  the visit-distribution policy target π whose raw material the
  runner retains.
- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://doi.org/10.26481/dis.19940923la — threat-space search, the
  seat's certified-forced-win probe.
