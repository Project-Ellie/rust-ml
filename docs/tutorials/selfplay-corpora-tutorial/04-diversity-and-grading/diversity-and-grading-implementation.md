# Chapter 04 — Diversity and grading: implementation plan

## Abstract

This document is the implementation plan for
[chapter 04 of the selfplay-corpora tutorial](diversity-and-grading.md):
the hiding layer between the chapter paper and the solution, for the
learner who builds the code without reading the reference. It turns the
chapter's walkthrough into a numbered construction order along the
execution path of one `selfplaygen` invocation: the opening-list error
vocabulary and the `TournamentOpenings` pool (parse, load, uniform
sampling with provenance), the `options_for_ply` behavior added to the
`ExplorationSchedule` type chapter 02 already created, the one-line-region
runner change that applies the schedule per ply, the seeded generation
loop with Zobrist-walk deduplication, the `selfplaygen` binary with its
parser, run path, and read-back check, the report-only grading probe,
and the export audit with the standing gates. Every artifact names its
file, its behavioral contract in prose, the helpers to reuse, and the
idioms to apply by name; method bodies never appear. The plan pins the
exact test names and fixtures of the chapter's fourteen tests — twelve
default, two MANUAL — and the per-target counts that sum to the
chapter's closing state of 27 passed and 4 ignored. The intended reader
has completed chapter 03 (suite state: 15 passed, 2 ignored), has read
the chapter paper and, ideally, the
[design paper](diversity-and-grading-design.md), and wants low-level
guidance without solution leakage.

## Glossary

| Term | Definition |
|------|------------|
| **Seat** | One of the two players of a generated game: a persistent `player::AiPlayer` running the baseline evaluator at a declared simulation budget (chapter 01). Every game pairs two seats. |
| **Simulation budget** | The number of Monte Carlo Tree Search (MCTS) simulations a seat runs per move decision; written baseline-X. |
| **Pairing** | The declared Black-vs-White seat configuration of a corpus — `SeatPairing` in the record header (chapter 02), `pairing` in the manifest. |
| **Corpus** | A set of game records produced at one declared pairing, stored as shards plus a manifest (chapter 02). |
| **Corpus slot** | One position in a run's game sequence. Each slot starts from one base seed and ends with exactly one accepted game. |
| **Shard** | A fixed-capacity file of length-delimited bincode records; the `train` crate's storage unit, reused unchanged. |
| **Manifest** | The JSON file next to the shards: the exact seed set, the pairing, game counts, shard names. |
| **Swap2** | The project's opening protocol: one side places three stones, the other side chooses a color or places two more stones, and the first side then chooses a color. |
| **Opening offer** | The three-stone opening one side places in the Swap2 ceremony. In this chapter, one validated line of a tournament list. |
| **OpeningSource** | The runner's pluggable opening seam (chapter 01): one method, `opening`, taking a mutable borrow of the source and of the game's `StdRng`, returning an `Opening`. |
| **Opening provenance** | `OpeningProvenance`: `Generated`, or `Listed` with a list identity and a zero-based index. Recorded in every game header. |
| **Exploration schedule** | `ExplorationSchedule`: the per-ply move-selection policy shared by both seats. Plies `0..plies` after the Swap2 ceremony sample at temperature τ with root Dirichlet noise; every later ply plays the argmax. |
| **Temperature (τ)** | The move-selection temperature: τ = 1 samples moves proportionally to visit counts; τ = 0 plays the argmax (Silver et al. 2017). |
| **Dirichlet noise (ε, α)** | Root noise mixed into the search priors during exploration plies: ε is the mix fraction, α the concentration parameter (Silver et al. 2017). |
| **Base seed / derived seed / accepted seed** | The seed a corpus slot starts from; the replacement `base_seed + attempt` used when a duplicate is rejected; and the seed — base or derived — that produced the accepted game and is recorded. |
| **Zobrist key** | A 64-bit hash of a board position, maintained incrementally by the engine; used here for game identity. |
| **Zobrist walk** | The ordered sequence of Zobrist keys of every position a game reaches, opening placements included. Equality of walks is this chapter's duplicate test. |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games. It never runs by default; it runs on explicit selection, in release mode. |
| **Threat-space search (TSS)** | A proof search over forced-win threat sequences (Allis 1994). Each seat's own-side probe runs before its search. |
| **PUCT** | Predictor UCT (Rosin 2011), the selection rule of the search both seats run. |
| **Claim labels** | The project's provenance labels (chapter 12 of the wiki): **[paper]** (stated in a cited paper), **[derived]** (computed from cited or measured values), **[experiment]** (a free parameter, to be settled by measurement). This chapter also uses **[invention]**: a choice of this track, not a fact of the architecture chapters. |
| **TDD** | Test-driven development: write the test, watch it fail (red), implement, watch it pass (green). |

## How to use this plan

Read [the chapter paper](diversity-and-grading.md) first. It carries
the intention, the specification as tests, and the execution-order
walkthrough; the [design paper](diversity-and-grading-design.md)
carries the reasoning and the invariants. This plan assumes both and
adds only construction detail: which file, which item, which contract,
which test goes red then green. Work one step at a time, in order —
the order is the execution order of one `selfplaygen` invocation, from
the opening lists loaded before a game starts, through the schedule
that shapes every ply, the seeded generation loop, and the binary, to
the grading probe that stands last. Open
[diversity-and-grading-solution.md](diversity-and-grading-solution.md)
only if you are stuck for more than twenty minutes, or
after the chapter is complete.

## Context

The chapter-03 slice — the verified starting state — holds the
following. All paths are under
[`gomoku/crates/selfplay/`](../../../../gomoku/crates/selfplay/);
the crate does not exist on `main` until the learner lands it chapter
by chapter.

- `src/opening.rs` (chapter 01) carries `OpeningProvenance`,
  `OpeningAnswer`, `Opening` with `new` and `board`, the
  `OpeningSource` trait, the stand-in `GeneratedOpening`, and two
  private helpers: one that draws unique cells from the central 7×7
  region (rows and columns 4 through 10 inclusive) excluding a given
  set, and one that draws a random color. The file imports the
  engine's `Board`, `Color`, `Move`, `Placing3`, `PlayError`, and
  `Swap2`, the `rand` extension trait and `StdRng`. It has no error
  vocabulary yet — the chapter-01 contract is that draws are
  infallible and malformed descriptions surface only at execution.
- `src/schedule.rs` (chapter 02) carries `EXPLORATION_PLIES = 10`,
  the `ExplorationSchedule` struct with its four public fields
  (`plies`, `temperature`, `noise_epsilon`, `noise_alpha`), the serde
  derives, and the `Default` implementation (a 10-ply window at
  temperature 1.0 with ε = 0.25, α = 0.3). It is record vocabulary
  only: no `options_for_ply`, no `player` import. Do not recreate any
  of it — this chapter adds one import line and one method to the
  existing file.
- `src/game.rs` (chapters 01–03) carries the runner
  `play_game_with_opening` / `play_game`, the `Game` record with its
  `certified_suffix` slot (initialized to `None`; the tracker is
  chapter 05's), `PlySearch` with the chapter-03
  `From<player::SearchStats>` conversion, `PlyRecord`, `Outcome`,
  `WinReason`, and `GameError`. The move loop passes the seat's base
  options directly to `decide`; chapter 04 inserts the schedule at
  exactly that point. Three doc wordings are known slice deltas that
  this chapter retires (step 5): the `Game.black_options` and
  `Game.white_options` field docs read "preserved verbatim in the
  record" because nothing adjusts them yet; the `play_game` doc lacks
  the schedule sentence; and `src/record.rs` documents
  `SeatConfig.temperature` as "chapter 4's exploration schedule
  adjusts it per ply" — true now, imprecise once the schedule's
  behavior exists.
- `src/record.rs` and `src/corpus.rs` (chapter 02) carry
  `GameRecord::from_game` (whose header assembles the `SeatPairing`
  from both seats' base options plus the schedule's `Default`), the
  `Manifest` with its `seeds` field, `write_corpus` with overwrite
  protection, and the readers `read_corpus` / `read_manifest`.
  Chapter 03 added `RECORDED_PRIOR_TOP_K`. This chapter touches none
  of the record or corpus mechanics — it fills `Manifest.seeds` with
  the accepted seeds of a generated run and makes the recorded
  schedule behavioral, through the call sites alone.
- `src/lib.rs` declares the five modules (`corpus`, `game`,
  `opening`, `record`, `schedule`) behind `#![deny(missing_docs)]`
  and re-exports the chapter-03 surface.
- `Cargo.toml` needs **no changes**: `selfplay` already depends on
  `baseline`, `engine`, `player`, `rand`, `thiserror`, `serde`,
  `serde_json`, and `train`, and a binary in `src/bin/` needs no
  manifest entry. Verify this before writing anything; if a step
  seems to need a dependency, the step has drifted.
- Test targets at the chapter-03 state: `tests/game.rs` 5 passed / 2
  MANUAL-ignored; `tests/corpus.rs` 10 passed / 0 ignored.

Two scope boundaries keep the chapter honest. First, the new
`tests/binary.rs` will hold **seven** tests when the track is
complete; this chapter creates the file with exactly one
(`selfplaygen_help_documents_corpus_and_grading_flags`). The two
`selfplayanchors_*` tests belong to chapter 05 and the four
`selfplayverify_*` tests to chapter 07; none of their helpers,
fixtures, or imports appears in this chapter's file. Second, no
chapter-05-or-later module, type, or test is named anywhere in this
chapter's code: no anchor, export, sample, sampler, or mix module, and
no suffix tracker in the runner.

---

## Step 1 — `tests/openings.rs`: the five opening tests (red)

Create
[`gomoku/crates/selfplay/tests/openings.rs`](../../../../gomoku/crates/selfplay/tests/openings.rs).
Imports: the standard-library path types; the engine's `Color` and
`Move`; the seeded-RNG machinery (`StdRng` and the `SeedableRng`
trait); and, from the `selfplay` crate root, `OpeningAnswer`,
`OpeningProvenance`, `OpeningSource`, and `TournamentOpenings` — the
first three are chapter-01 exports, the fourth does not exist yet, so
the red state is a compile error. One helper: the `mv(row, col)`
fixture constructor from the earlier test files, which validates a
test coordinate through `Move::new` and expects success. The file
carries no `TestDir` helper: every fixture is text in a string or a
committed list under `data/openings/`, reached from
`CARGO_MANIFEST_DIR` three directories up and into `data/openings`.

The five tests, in the specification's order:

### 1.1 `three_stone_and_longer_lines_parse_as_swap2_offers`

Two text fixtures, each parsed with the list identity `test-list`: a
three-stone line (`0,0, 1,0, -1,0`) and a seven-stone line whose
first three pairs are the same and whose remaining pairs are valid,
on-board, and distinct (`0,1, 0,-1, 2,2, -2,-2`). For each: sample
one opening from the parsed pool with an RNG seeded at 7 (the pool
holds one offer, so the draw is forced). Assert the opening's initial
stones are the center-relative mapping of the first three pairs —
`0,0` → row 7 column 7, `1,0` → row 7 column 8, `-1,0` → row 7
column 6 — that the provenance is `Listed` with list identity
`test-list` and index 0, and that the opening executes through
`Opening::board` without error. The seven-stone fixture pins the
rule: longer lines are validated in full but only their first three
stones drive the ceremony.

### 1.2 `malformed_lines_are_rejected_with_physical_line_numbers`

Five malformed texts, each parsed with the list identity `bad-list`,
each expected to fail, with three substring assertions against the
error's display text — the list identity, the line marker, the
reason:

1. a valid first line (`0,0, 1,0, -1,0`), then a second line
   (`0,0, 1,0, 1,0`) whose third pair repeats its own second pair →
   `line 2`, `duplicate` — duplicate detection is within one line,
   never across lines;
2. a two-pair line → `line 1`, `at least three`;
3. a three-pair line whose third coordinate is `8,0` (column offset 8
   maps to column 15, one past the board) → `line 1`, `outside`;
4. a three-pair line with a non-numeric x coordinate → `line 1`,
   `invalid x`;
5. a five-field line (an odd field count) → `line 1`, `x,y pairs`.

One loop over the five cases carries the three assertions, with the
message itself in the assertion context so a failure prints it.

### 1.3 `sampled_offers_come_from_the_loaded_list_with_seeded_determinism`

One three-offer text (the three-stone offers `0,0, 1,0, -1,0`;
`0,0, 0,1, 0,-1`; and `1,1, 1,0, 1,-1`), parsed twice into two
identical pools, sampled 64 times from two RNGs seeded identically at
91. Every pair of draws asserts equality — same pool, same stream,
same opening. Over the 64 draws, three boolean trackers accumulate:
all three offers appear (matched against the expected center-relative
mappings), all three answer kinds appear (take Black, take White, add
two), both final colors appear among the add-two answers, and every
sampled opening executes through `Opening::board`. The test ends with
the three all-trackers-true assertions.

### 1.4 `valid_committed_tournament_lists_load_into_one_uniform_offer_pool`

Load the three committed Gomocup lists —
[2024](../../../../data/openings/gomocup-2024-freestyle15.txt),
[2025](../../../../data/openings/gomocup-2025-freestyle15.txt),
[2026](../../../../data/openings/gomocup-2026-freestyle15.txt) —
through `TournamentOpenings::load`. Assert the pool length is exactly
36 (each committed list holds 12 lines) and `is_empty` is false.
Sample one opening with an RNG seeded at 123; destructure its
provenance as `Listed` (a generated provenance here is a panic); assert
the list identity equals one of the three paths as supplied and the
index is below 41 — a deliberately loose bound: per-list zero-based
indexing is pinned by the parse fixtures of tests 1.1–1.3, not by
this assertion — and that the opening executes.

### 1.5 `malformed_committed_piskvork_line_is_rejected_with_its_line_number`

Load the committed
[piskvork list](../../../../data/openings/piskvork-openings.txt)
alone and require failure: the error's display text contains
`line 41` and `outside`. The file's line 41 holds the coordinate
`-8,-6` — column −1 after the center-relative mapping, off the 15×15
board. This test pins a real data defect as a regression guard; the
chapter paper's open observations explain why the list stays
rejected and the working pool stays at 36 offers.

Watch all five fail to compile: `TournamentOpenings` does not exist.

---

## Step 2 — `src/opening.rs`: errors, pool, and the second source (green)

All items land in the existing
[`src/opening.rs`](../../../../gomoku/crates/selfplay/src/opening.rs),
after chapter 01's vocabulary. Three imports join the file's header:
the standard library's `fs`, the path types `Path` and `PathBuf`, and
the `thiserror` derive.

### 2.1 `OpeningParseError`

A public error **struct** — one malformed line — with three public
fields: `list` (a `String`, the stable list identity), `line` (a
`usize`, the one-based physical line number), and `reason` (a
`String`, the specific validation failure). Derive `Debug`, `Clone`,
`PartialEq`, `Eq`, and the `thiserror` error derive; the display
format is `opening list '{list}' line {line}: {reason}` — list, line,
reason, in that order, because the tests assert all three as
substrings of one message.

### 2.2 `OpeningLoadError`

A public error **enum** with three variants, in the order a load
meets them: `Empty` — no input paths were supplied, display "at least
one tournament opening list is required"; `Read` — an input file
could not be read, carrying the `PathBuf` and the underlying
`std::io::Error` marked as the error source, with a display that
names the path and the source; and `Parse` — a transparent wrap of
`OpeningParseError` with the from-conversion idiom, so a parse
failure propagates unchanged and its list-line-reason message reaches
the caller verbatim (test 1.5 asserts `line 41` and `outside`
through this variant). Derive `Debug` and the error derive; no
`Clone` or `PartialEq` — the `io::Error` forbids them.

### 2.3 `ListedOffer` and `TournamentOpenings`

`ListedOffer` is a **private** struct: one validated offer — its list
identity (`String`), its zero-based index within that list, and its
first three stones in placement order. Derive `Debug` and `Clone`;
private, so no doc obligation beyond a plain comment.

`TournamentOpenings` is the public pool: a struct holding a vector of
`ListedOffer`, deriving `Debug` and `Clone`. Its doc comment states
the format contract: every line must contain at least three distinct,
on-board stones; longer lines are validated in full but only their
first three stones drive the Swap2 typestate, because the ceremony
consumes exactly three — the rest of a long line is the source
tournament's pre-set continuation, not this generator's to replay.

### 2.4 `TournamentOpenings::parse`

A public associated function accepting a list identity (anything
convertible into a `String`) and the list's text (a string slice),
returning a `Result` of the pool or an `OpeningParseError`, marked
with a `# Errors` doc section. Behavioral contract, in execution
order:

1. Iterate the text's lines with their zero-based indices; the
   physical line number is the index plus one. A small closure over
   the list identity and the current line number builds each
   rejection, so no reason string repeats the format by hand.
2. Split the line on commas and trim every field. Fewer than six
   fields, or an odd field count, rejects with the reason "expected
   at least three comma-separated x,y pairs" — one reason covers both
   arities, and the two tests (1.2 cases 2 and 5) assert different
   substrings of it.
3. Per consecutive pair of fields: parse each coordinate as a signed
   16-bit integer; a failure rejects with "invalid x coordinate
   '{text}': {error}" (or y, respectively). Map center-relative to
   the board: the row is 7 plus y, the column is 7 plus x — x is the
   column offset, y the row offset, so `0,0` is row 7 column 7 and
   `-8,-6` would be column −1. Both coordinates must fall in the
   range 0 to 14 inclusive and survive `Move::new`; otherwise reject
   with "coordinate {x},{y} is outside the 15 by 15 board". A
   coordinate already present among this line's stones rejects with
   "duplicate coordinate {x},{y}"; otherwise append.
4. Every pair of the line is validated before the offer is recorded —
   the full-validation rule — and the recorded offer keeps only the
   first three stones (the slice-to-fixed-array conversion idiom,
   expected with a "length checked above" note, because step 2
   guaranteed at least three pairs). The offer's index is the number
   of offers recorded so far — zero-based within this list, so each
   list keeps its own indices when pools concatenate.
5. After the loop, an empty offer vector rejects at line 1 with the
   reason "opening list is empty" — an empty file is malformed, not
   an empty pool.

The first malformed line fails the whole parse: fail fast, no
skipping. An opening list is a curated artifact; a corrupt line means
the artifact is not what the invocation claimed.

### 2.5 `TournamentOpenings::load`

A public associated function accepting a slice of path-like items
(generic over `AsRef<Path>`) and returning a `Result` of the pool or
an `OpeningLoadError`, with a `# Errors` doc section. Behavioral
contract: an empty slice fails with `Empty` before any file is
touched. Each path is read to a string; a read failure becomes `Read`
with the path and the I/O error. The path's display string is the
list identity passed to `parse` — the stable identity that later
appears in every record's opening provenance — and the parsed offers
extend one concatenated pool in path order. The first failure of any
kind fails the whole load.

### 2.6 `len` and `is_empty`

Two public accessors on a borrow, both must-use: `len` returns the
offer count; `is_empty` reports emptiness and is never true for a
successfully parsed pool — the pair exists for the standard lint, and
test 1.4 exercises both.

### 2.7 `impl OpeningSource for TournamentOpenings`

The chapter's second implementation of chapter 01's seam, placed
after the inherent implementation block. Behavioral contract:

1. Draw one offer uniformly from the whole pool: the ranged-random
   idiom on the game's `StdRng` over the pool's index range. The pool
   is guaranteed non-empty by construction (step 2.4's empty-file
   rejection), so indexing the drawn position cannot fail.
2. Draw the Swap2 answer on the same stream, exactly as
   `GeneratedOpening` does: a ranged draw over three outcomes — take
   Black, take White, or add two. The add-two arm reuses chapter 01's
   private unique-cell helper with the offer's three stones as the
   exclusion set (two unique cells from the central 7×7 region) and
   chapter 01's private random-color helper for the final choice.
3. Assemble through `Opening::new` with the offer's three stones, the
   drawn answer, and `OpeningProvenance::Listed` carrying a clone of
   the offer's list identity and its index.

Above the answer draw, write the honesty comment the reference
carries, verbatim: "Chapter 14 §3.3 perturbs a value-based argmax.
This source has no value model, so chapter 4 uses a seeded uniform
protocol choice as its exploration approximation, including both
color assignments." (The **[invention]** label this plan attaches to
that choice is plan annotation, not part of the reference comment.)
The comment lives where a future value-guided answer will replace it.

### 2.8 `lib.rs` checkpoint

Add the three new names to the existing `opening` re-export group in
[`src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs), in
alphabetical position: `OpeningLoadError`, `OpeningParseError`,
`TournamentOpenings`. The step-1 tests import from the crate root
only; the modules stay private.

Watch the five opening tests compile and pass: parser, pool,
sampling, and both committed-list guards green. The full default
suite stays at the chapter-03 state plus these five: game 5/2,
corpus 10/0, openings 5/0.

---

## Step 3 — `src/schedule.rs`: `options_for_ply` (red → green)

### 3.1 The property test, red

Create
[`tests/schedule.rs`](../../../../gomoku/crates/selfplay/tests/schedule.rs)
with the first of its two tests,
`first_ten_plies_explore_then_selection_is_deterministic_without_changing_budgets`.
Imports: the `player` crate's `GuardConfig`, `OwnTssConfig`, and
`PlayerOptions`; and from the crate root `EXPLORATION_PLIES`,
`ExplorationSchedule`, plus `GameRecord` and `play_game` for the
second test (step 4). The red state is a compile error:
`options_for_ply` does not exist.

The fixture is a deliberately loud base `PlayerOptions`, every field
set explicitly with no struct-update syntax — so a future field added
to `PlayerOptions` fails this build loudly instead of passing through
untested: 777 simulations, c_puct 1.75, temperature 4.0, noise
ε 0.8, noise α 0.9, the baseline own-TSS probe, the baseline guard,
resignation enabled. The loud values make any quiet edit visible.

The assertions, against `ExplorationSchedule::default()`:

- `EXPLORATION_PLIES` equals 10 — a silent window change fails loudly.
- Inside the window, at plies 0, 1, and `EXPLORATION_PLIES - 1`, the
  effective options carry temperature 1.0, ε 0.25, α 0.3 — and every
  other field preserved exactly: 777 simulations, c_puct 1.75, the
  probe equal to the base probe, the guard equal to the base guard,
  resignation still enabled.
- Outside the window, at plies `EXPLORATION_PLIES`,
  `EXPLORATION_PLIES + 1`, and 100, the effective options carry
  temperature 0.0 and ε 0.0, with the same five preservations
  asserted again.

One deliberate gap, stated in the chapter paper and held here: the
code also leaves α untouched outside the window (ε 0.0 makes α
inert), but the test does not assert that — α outside the window is
stated source behavior, not a pinned contract.

### 3.2 The method, green

In
[`src/schedule.rs`](../../../../gomoku/crates/selfplay/src/schedule.rs),
add one import line at the top — the `player` crate's
`PlayerOptions` — and one inherent implementation block on the
existing `ExplorationSchedule`, placed between the struct and the
`Default` implementation. Do not touch the constant, the struct, its
derives, the field docs, or `Default`.

`options_for_ply` borrows the schedule, consumes the seat's base
`PlayerOptions` **by value**, and takes the zero-based post-opening
ply index; it returns the effective `PlayerOptions`, marked must-use.
It is pure and cannot fail, so no `Result`. Behavioral contract:

- Inside the window (ply strictly below the schedule's `plies`):
  replace temperature, noise ε, and noise α with the schedule's
  recorded values.
- Outside the window: set temperature to 0.0 and noise ε to 0.0;
  leave α untouched — ε 0.0 makes it inert.
- Every other field passes through exactly: simulations, c_puct,
  probe, guard, resignation. The contract is narrow on purpose — only
  move-selection settings change, because a schedule that quietly
  edited the budget would corrupt the pairing, the corpus's declared
  identity.

The doc comment says exactly that: the method applies the schedule to
one post-opening ply, and names the preserved fields — the simulation
budget, the PUCT constant, the TSS probe, the guard, and the
resignation settings.

Watch the property test pass. It plays no game and finishes in well
under a second.

---

## Step 4 — `src/game.rs`: the schedule enters the move loop

One file, one import, two lines of behavior, one MANUAL test. The
runner of chapters 01–03 changes in exactly one line region; anything
beyond it is drift.

### 4.1 The MANUAL test, red

Add the second test to `tests/schedule.rs`:
`scheduled_game_is_reproducible_and_records_its_schedule_and_pairing`,
marked `#[ignore]` with this MANUAL comment: two full games are
unnecessary for the isolated schedule check and can run for minutes
in debug; the binary's budget-1 fixture checks recording. The test
builds Black at 2 simulations and White at 1 simulation through the
struct-update-syntax idiom over the default options, plays two games
at seed 29 through `play_game`, converts both with
`GameRecord::from_game`, and asserts: the two records are equal; the
header's recorded exploration equals the default schedule; and the
recorded per-seat budgets are 2 and 1.

An honesty note about the red state: this test cannot fail on the
missing runner application alone, because two seeded games are equal
with or without the schedule and the header records the default
schedule either way. It is written before the runner change and
watched green after it — its red state is its absence — and what it
pins is reproducibility and recording *with the schedule active*, not
the application line itself. No test in this chapter isolates that
one line: the application is watched only indirectly — by this
MANUAL test and by the chapter-01 MANUAL pair's regression run
(step 10), which re-prove the rewired move loop with the schedule
active. The capstone's two-process byte identity is determinism
evidence only: two seeded processes agree with or without the
schedule, so it says nothing about whether the application line
exists.

### 4.2 The runner change, green

In
[`src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs):

1. `ExplorationSchedule` joins the crate's import block, in
   alphabetical position.
2. In `play_game_with_opening`'s setup, after the plies vector is
   created and before the move loop, bind the default exploration
   schedule once per game.
3. In the move loop, between the seat selection and the decision
   call, insert the application: the effective options are the
   schedule applied to the moving seat's base options at the current
   ply count — `plies.len()`, zero-based, post-opening, because the
   Swap2 placements are the ceremony, not plies — and the decision
   call receives a borrow of the effective options instead of the
   base options. The base options are `Copy`, so the application
   consumes a copy; the returned `Game` still carries the base
   options verbatim, so the record always shows what was asked for.

Everything else in the function — the single seeded stream, the
status-first loop structure, the seat selection by absolute color,
the chapter-03 conversion, the legality gate, the resignation arm,
the assembly — stays byte-identical.

Run the focused release command once and watch the MANUAL test pass:

```text
cargo test -p selfplay --release --test schedule -- --ignored
```

The `--release` qualifier is not optional: two full games at budgets
2 and 1 run for minutes in a debug build.

---

## Step 5 — the doc-comment ownership updates

Three slice wordings that earlier chapters documented as deltas now
become the reference wordings, because the behavior they name exists
as of step 4. No code changes; no test outcome changes. If a test
moves, the refactor drifted — fix the edit, never a test.

### 5.1 `src/game.rs`: the `Game` base-options field docs

The `black_options` and `white_options` field docs read "Base options
for Black, preserved verbatim in the record." (and White
respectively) — true while nothing adjusted them. Replace each with
the reference wording: "Base options for Black; exploration adjusts
selection settings per ply." The options are still preserved verbatim
in the returned game; what changes is that they are now *base*
options, adjusted per ply at decision time.

### 5.2 `src/game.rs`: the `play_game` doc

Chapter 01's solution paper recorded this sentence as the reference's
and marked it for this chapter. Append it to the `play_game` doc
comment, directly after the sentence about default options disabling
resignation: "The chapter-4 exploration schedule adjusts only
selection settings per ply."

### 5.3 `src/record.rs`: the `SeatConfig.temperature` doc

Chapter 02's slice documents the field as the base visit-count
sampling temperature and names chapter 04 as the adjuster. Replace
the adjustment clause with the reference wording, which states the
rule precisely now that the behavior exists: the pairing's
exploration schedule overrides this on every ply (zero after the
opening window). Nothing else in `record.rs` changes — the
conversion already records the base options and the default schedule,
and that is exactly right.

---

## Step 6 — `src/generate.rs` and `tests/dedup.rs`: seeded generation with dedup

Create two files:
[`src/generate.rs`](../../../../gomoku/crates/selfplay/src/generate.rs)
and
[`tests/dedup.rs`](../../../../gomoku/crates/selfplay/tests/dedup.rs).
Declare the `generate` module in `lib.rs`, in alphabetical position
between `game` and `opening`.

### 6.1 The dedup test, red

The test file's imports: the engine's `Color` and `Move`; the
`player` crate's `PlayerOptions`; and from the crate root `Game`,
`Opening`, `OpeningAnswer`, `OpeningProvenance`, `Outcome`, and
`generate_games_with`. The red state is a compile error — the module
does not exist.

Two helpers. The `mv(row, col)` fixture constructor, as in the other
test files. And `fabricated_game`, which builds a `Game` by hand from
a seed: the opening's three initial stones sit on the row given by
the seed modulo 15, in columns 5, 6, and 7; the answer is
`Take(Black)`; the provenance is `Generated`; both option sets are
the defaults; the ply list is empty; the outcome is a draw; the
certified-suffix slot is empty. No real game is played anywhere in
this target — the contract under test is the seed arithmetic and the
rejection, not the probability that two real games collide.

The test,
`forced_collision_is_rejected_and_rerolled_with_seed_plus_attempt`:
call `generate_games_with` with the base seeds 7 and 7 and a closure
that fabricates its game from whatever seed it receives. The first
slot's game (seed 7, row 7) is accepted. The second slot's attempt
zero plays seed 7 again — the same row, the same three-stone walk —
collides, and is rejected; attempt one plays the derived seed 8
(row 8) and is accepted. Assert: exactly two games; the first game's
seed is 7; the second game's seed is 8, with the assertion message
stating the rule (derived seed = base seed 7 + attempt 1); and the
two games' openings differ.

### 6.2 `GenerateError`, green

A public error enum in `src/generate.rs`, deriving `Debug` and the
`thiserror` derive, with two variants in the order a run meets them:
`Game`, a transparent wrap of chapter 01's `GameError` with the
from-conversion idiom — runner failures propagate unchanged; and
`DuplicateExhausted`, carrying the slot's base seed and the number of
attempts (both `u64`), displayed as "could not generate a unique game
for base seed {base_seed} after {attempts} attempts". Each variant
carries its doc comment.

The module's imports: the standard library's `HashSet`; the engine's
`Board` and `Move`; the `player` crate's `PlayerOptions`; the
`thiserror` derive; and, from the crate, `Game`, `GameError`,
`OpeningAnswer`, `OpeningSource`, and `play_game_with_opening`.

### 6.3 The private walk helpers

Two **private** free functions, no doc obligation beyond plain
comments:

- `opening_moves` borrows a `Game` and returns the ordered opening
  placements as a vector of `Move`: the three initial stones, plus
  the two added stones when the answer is the add-two variant. Three
  stones, or five — the ceremony, in placement order.
- `zobrist_walk` borrows a `Game` and returns a `Result` of the
  `Vec<u64>` walk or a `GameError`. It replays the game's full
  ordered move list through a fresh `engine::Board`: the opening
  placements first (a rejection maps to `GameError::Opening`), then
  every recorded ply in order (a rejection maps to `GameError::Play`
  with the zero-based ply index, the move, and the source). After
  **every** move it pushes the board's Zobrist key. Pre-size the
  vector for the plies plus the at-most-five opening placements.

The walk, not the final position, is the identity: two games that
reach the same end position through different move orders are
different games, and two games that share only an opening prefix
diverge in their walks.

### 6.4 `MAX_ATTEMPTS_PER_GAME`

A **private** `u64` constant, the value 128: the maximum candidate
games tried for one base seed before generation reports failure. The
bound exists so a pathological runner whose seeds do not diversify
its games fails loudly instead of spinning.

### 6.5 `generate_games_with`

The injectable seam — public, because the dedup test and any
in-process caller drive it. It accepts a base-seed iterator (anything
into an iterator of `u64`) and an injected runner — anything
implementing `FnMut(u64) -> Result<Game, GameError>` — and returns a
`Result` of the accepted games (in slot order) or a `GenerateError`.
Behavioral contract:

1. Create the per-run walk set: an empty `HashSet` of walks. It is
   built fresh per invocation and dropped at return — deduplication
   is scoped to the run, and no cross-corpus deduplication exists.
2. For each base seed, try up to `MAX_ATTEMPTS_PER_GAME` candidates.
   Attempt zero uses the base seed itself; attempt n uses the
   wrapping-add idiom, `base_seed.wrapping_add(n)` — the derivation
   rule the test pins.
3. Play the candidate through the injected runner, propagating a
   runner error with `?` (the from-conversion wraps it in
   `Game`). Compute the candidate's walk. Insert the walk into the
   set: a `true` insert means the walk is new — accept the game,
   retain it in slot order, and move to the next slot; a `false`
   insert is the collision signal — discard the candidate and derive
   the next attempt's seed.
4. If a slot exhausts all 128 attempts without an accept, fail with
   `DuplicateExhausted`, naming the base seed and the attempt count —
   never loop forever.

The accepted game's actual seed — base or derived — stays in the
`Game`, flows into the record header, and lands in the manifest's
`seeds` list: the recorded seed set is always the exact set that
produced the corpus, and a derived replacement seed in that set is
expected behavior, visible in the manifest, not an anomaly.

The doc comment carries the `# Errors` section and states the seam's
purpose: the deterministic entry for fabricated-game tests and for
callers with another in-process runner; seed derivation and collision
handling identical to the wrapper below.

### 6.6 `generate_games`

The thin public wrapper: accepts a base-seed iterator, both seats'
base `PlayerOptions`, and a mutable borrow of `impl OpeningSource`,
and returns the same result type. It delegates to
`generate_games_with`, closing over the real runner call —
`play_game_with_opening` at the candidate seed with the two option
sets (both `Copy`) and the borrowed opening source. The split mirrors
chapter 01's `play_game` / `play_game_with_opening` and the binary's
own `run` / `run_with_opening_source` (step 8): every stage of the
pipeline has a seam where a test can substitute fabricated behavior.
The doc comment states the derivation rule, the accepted-seed
recording, and the `# Errors` contract (a runner error, or the
exhaustion error after 128 collisions), and that deduplication covers
the full opening and post-opening move order.

### 6.7 `lib.rs` checkpoint

Add the re-export group, in alphabetical position:
`GenerateError`, `generate_games`, `generate_games_with`. The
constant, both walk helpers, and the opening-moves helper stay
private — they are source behavior, not contract.

Watch the dedup test compile and pass. The default suite now reports:
game 5/2, corpus 10/0, openings 5/0, dedup 1/0.

---

## Step 7 — `src/bin/selfplaygen.rs`: the argument parser (red → green)

Create
[`src/bin/selfplaygen.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplaygen.rs).
No manifest entry: Cargo discovers the binary by its path. The file
opens with a module-level doc comment — one line naming the binary
(strength-graded baseline self-play corpus generator) and one
paragraph stating the process rule: one invocation owns one
contiguous base-seed range, and additional processes with disjoint
`--seed-base` ranges are the parallelism.

Imports: the standard library's `env`, `PathBuf`, and `process`; the
`player` crate's `OwnTssConfig` and `PlayerOptions`; and, from the
`selfplay` crate root, `GameRecord`, `OpeningSource`,
`TournamentOpenings`, `generate_games`, `read_corpus`, and
`write_corpus` — the binary speaks crate-root names only.

### 7.1 `USAGE`

A private string constant: the help text. It documents the seven
flags and their syntax — `--out <DIR>` and `--openings <PATH>`
required, `--openings` repeatable; the five numeric flags
`--black-sims`, `--white-sims`, `--games`, `--seed-base`,
`--shard-capacity` — and it states the five defaults (800, 800, 1, 0,
1000), the two parse rules (`--flag value` only, never
`--flag=value`; a repeated numeric flag takes its last value), the
contiguous-range statement (one process generates the base seeds
seed-base through seed-base + games − 1), and the two operational
rules: both seats use the baseline own-TSS probe (10 000 nodes /
30 plies), and parallel generation is separate processes over
disjoint seed ranges. The help integration test (step 8.3) asserts
all seven flag strings appear in this text.

### 7.2 `Args`

A private struct, deriving `Debug`, holding the parsed values: the
two simulation budgets (`u32`), the game count (`usize`), the seed
base (`u64`), the shard capacity (`usize`), the own-TSS probe
configuration (an `OwnTssConfig`), the output directory (a
`PathBuf`), and the ordered opening paths (a vector of `PathBuf`).
The probe configuration is carried here — not hard-coded in the run
path — so the binary's unit tests can disable it for speed.

### 7.3 The three parser tests, red

A `#[cfg(test)]` module at the end of the binary file holds the four
unit tests; the first three pin the parser and go red now (the red
state is a compile error — `parse_args` does not exist):

- `generation_defaults_to_the_baseline_own_tss_probe` — parsing the
  minimal invocation (`--out corpus`, one `--openings` path) yields a
  probe configuration equal to `OwnTssConfig::baseline()`: enabled,
  10 000 nodes, 30 plies.
- `parses_all_flags_with_last_numeric_value_winning` — every flag
  supplied: budgets 300 and 200; `--games` twice (2, then 1) — the
  parsed count is 1, last value wins; seed base 41; shard capacity 7;
  the output directory; and two `--openings` flags asserting
  accumulation in order (first.txt, then second.txt).
- `rejects_missing_required_flags_and_zero_counts` — an invocation
  without `--out` fails with a message containing `--out`; an
  invocation with `--games 0` fails with a message containing
  "greater than zero".

### 7.4 `parse_args` and `parse_value`, green

`parse_args` is a private function consuming the argument iterator
(strings) and returning a `Result` of `Args` or a `String` message —
no error type, because the binary reports strings. It is a
hand-rolled loop — no argument-parsing dependency: the flag set is
seven strings, and the parse rules are exactly what the unit tests
pin. Behavioral contract:

1. Seed the five numeric locals with their defaults (800, 800, 1, 0,
   1000), the output directory as absent, and the opening paths
   empty.
2. Per argument, match the flag name. A numeric flag consumes the
   next argument as its value through `parse_value` and overwrites
   its local — last value wins. `--out` consumes the next argument
   into the directory; a missing value fails with "--out requires a
   value". `--openings` consumes the next argument and appends —
   repeats accumulate in order; a missing value fails likewise. Any
   other argument fails with "unknown flag: {arg}".
3. After the loop, validate in this order: a zero game count fails
   with "--games must be greater than zero"; a zero shard capacity
   fails with "--shard-capacity must be greater than zero"; an empty
   opening list fails with "at least one --openings path is
   required"; an absent output directory fails with "--out is
   required".
4. Assemble the `Args`; the probe configuration is
   `OwnTssConfig::baseline()` — the binary's declared default, pinned
   by the first unit test.

`parse_value` is a private generic helper over any `FromStr` type
whose error displays: it takes the flag name and the optional next
argument; a missing value fails with "{flag} requires a value"; a
parse failure fails with "invalid {flag} value '{value}': {error}".

Watch the three parser tests compile and pass.

---

## Step 8 — `src/bin/selfplaygen.rs`: the run path and the read-back (red → green)

### 8.1 The round-trip unit test, red

The fourth unit test,
`one_budget_one_game_writes_a_corpus_that_the_reader_round_trips`,
drives the run path through its seam. It needs two test fixtures,
added to the test module:

- `TestDir` — the same discipline as the earlier test targets: a
  unique directory under the system temporary directory (the binary's
  name, the process id, a nanosecond timestamp, and an atomic
  sequence counter), removed on `Drop`. Add the atomic and
  time-source imports the helper needs.
- `FixedOpening` — a test-local wrapper around one `Opening`,
  implementing `OpeningSource` by returning a clone of the stored
  opening and ignoring the RNG. This is the seam's point: the run
  path is testable without a tournament list. The fixture opening
  carries three fixed initial stones (row 5 column 8, row 5 column
  10, row 7 column 9), an add-two answer with two fixed stones
  (row 9 column 8, row 9 column 10) and final color White, and
  `Listed` provenance with list identity `fixed-test` and index 0.

The test builds `Args` directly: both budgets 1, one game, seed base
11, shard capacity 1, the probe **disabled** (the game plays in
seconds; the baseline probe would make a default-suite test slow),
the temporary output directory, and one opening path that goes unused
because the source is injected. It calls the seam
`run_with_opening_source` and expects success, then reads the output
back through the crate-root `read_manifest` and `read_corpus` and
asserts: the manifest's game count is 1; its seeds are exactly
`[11]`; its shards are exactly `["shard-000.bin"]`; the read-back
holds exactly one record; that record's header seed is 11; its Black
and White budgets are 1 and 1; its opening equals the fixture
verbatim; and it carries at least one ply. The red state is a compile
error — the run path does not exist.

### 8.2 `main`, `run`, and `run_with_opening_source`, green

Three private functions, in the call order:

- `main` — the exit-code owner. If any raw process argument is
  `--help` or `-h`, print the usage to stdout and return (exit code
  0) — the check runs before parsing, so `selfplaygen --help` works
  even alongside invalid flags. Otherwise parse; on a parse failure,
  print the message and the usage to stderr and exit with code 2. On
  a run failure, print "selfplaygen failed: {error}" to stderr and
  exit with code 1.
- `run` — accepts the parsed `Args` and returns a `Result` of unit or
  `String`. It loads the pool with `TournamentOpenings::load` —
  mapping a failure to "failed to load openings: {error}" — and
  delegates to the seam.
- `run_with_opening_source` — the testable seam: borrows the `Args`
  and takes a mutable borrow of `impl OpeningSource`; returns the
  same result type. Behavioral contract, in execution order:

  1. Convert the game count to `u64` ("game count does not fit in the
     seed range" on failure) and compute the last base seed with the
     checked-add idiom — "seed-base plus game count overflows u64" on
     overflow. The range is validated before any game is played.
  2. Build both seats' base options from `PlayerOptions::default()`
     with the struct-update-syntax idiom, overriding exactly two
     fields per seat: the declared simulation budget and the own-TSS
     probe configuration from the `Args`. Every other setting stays
     at its default — which means the resignation heuristic and the
     defensive guard are **disabled** in every corpus run: corpus
     games end on the board or on a full board, never on a concession
     heuristic, and no guard ever substitutes a move.
  3. Report on stderr before play begins: "Generating {games} games:
     base seeds {seed_base}..={last_seed}, baseline-{black} vs
     baseline-{white}" — the declared range and the declared pairing.
  4. Generate: `generate_games` over the contiguous base seeds — the
     range of offsets zero to the game count, each added to the seed
     base — with the two option sets and the opening source; a
     failure maps to "generation failed: {error}".
  5. Convert every accepted game with chapter 02's
     `GameRecord::from_game` — the header takes the game's actual
     seed, the pairing from both base options plus the default
     schedule, the evaluator identity `baseline@<workspace version>`,
     and the opening with its provenance. Nothing in this step is
     new, which is the earlier chapters' discipline paying off.
  6. Write with `write_corpus` at the declared output directory and
     shard capacity — chapter 02's mechanics, with its overwrite
     protection: a non-empty directory is refused before any file
     changes. A failure maps to "failed to write corpus: {error}".
  7. Read the corpus back with `read_corpus` ("failed to read back
     corpus: {error}" on failure) and compare: the read-back records
     must equal the generated records, and the manifest's pairing
     must preserve both declared per-seat budgets. Either mismatch
     fails the run — "read-back records differ from generated
     records", or "manifest does not preserve the declared seat
     pairing". The read-back turns serialization defects into a loud
     exit code 1 at generation time instead of a surprise at training
     time; it is a check, not a transaction.
  8. Report on stderr after the write: the manifest path ("Wrote
     manifest to …"), then the summary line — "Games: {n}, shards:
     {m}, accepted seeds: {seeds}" — the same list the manifest
     persists.

Watch the round-trip unit test compile and pass — it plays one
budget-1-versus-1 game with the probe disabled, seconds in a debug
build, which is what makes a default-run game affordable.

### 8.3 `tests/binary.rs`: the help contract, red → green

Create
[`tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs)
with exactly one test,
`selfplaygen_help_documents_corpus_and_grading_flags`, and exactly
one import: the standard library's process `Command`. The test runs
the compiled binary — the path from Cargo's compile-time binary
environment variable for `selfplaygen` — with `--help`, expects the
spawn to succeed, asserts the exit status is successful, decodes
stdout as UTF-8, and asserts all seven flag strings appear:
`--black-sims`, `--white-sims`, `--games`, `--seed-base`,
`--shard-capacity`, `--out`, `--openings`.

The boundary, restated: this file will hold seven tests when the
track is complete. This chapter adds one. No `TestDir`, no corpus
fixtures, no other binaries — the six later tests do not exist yet,
and nothing in this file anticipates them.

The red state is trivially green-adjacent — the test compiles once
the binary exists — so run it focused and watch it pass:

```text
cargo test -p selfplay --test binary selfplaygen_help
```

The default suite now reports: game 5/2, corpus 10/0, openings 5/0,
schedule 1/1 (the MANUAL test is registered, not run), dedup 1/0,
binary 1/0, the `selfplaygen` binary unit tests 4/0.

---

## Step 9 — `tests/grading.rs`: the report-only probe (MANUAL)

Create
[`tests/grading.rs`](../../../../gomoku/crates/selfplay/tests/grading.rs).
Imports: the engine's `Color`; the `player` crate's `PlayerOptions`;
and from the crate root `Outcome` and `play_game`. One helper:
`options(simulations)`, the struct-update-syntax idiom over the
default options — which keeps the probe disabled, the guard disabled,
and resignation disabled, exactly as a corpus run configures a seat.

One test,
`higher_budget_seat_wins_more_often_on_measured_seeds`, marked
`#[ignore]` with this MANUAL comment: four full games — minutes in
debug; run explicitly with the grading target selected and
`--ignored`. (The reference's own comment ends there; this track's
convention — the tutorial README and primer §7.3 — adds the
`--release` qualifier, and the four budget-16 games need it. State
the convention in the verification commands, and keep the comment's
selection line as the reference has it.) A second comment pins the
design: the seeds are measured fixed seeds — four short games, all
won by baseline-16 in the reference — and pinning avoids turning a
cheap sanity check into a flaky Elo estimate. (Elo: the
relative-strength rating named after Arpad Elo.)

Behavioral contract: a constant seed array `[11, 1, 7, 18]`; for each
seed, play one game with Black at 16 simulations and White at 1
simulation; count a Black win as a stronger-seat win, a White win as
a weaker-seat win, and a draw as neither; print one stderr line per
seed with the ply count and the outcome, so the report is visible on
`--nocapture`. The assertion is only a direction: the stronger seat
wins strictly more often than the weaker seat. No margin, no rate —
this is a smoke signal, not a measurement. The design spec's
pre-registered grading gate is chapter 07's work, and the reference's
report-only 20-game probe inversion (the budget-1 seat won 15 of 20
at 32-versus-1) is discussed in the chapter paper's open
observations, unresolved.

Run it once, on explicit selection, in release:

```text
cargo test -p selfplay --release --test grading -- --ignored
```

---

## Step 10 — `src/lib.rs`: audit the public surface, then regression, gates, capstone

The crate root's re-exports grew at the checkpoints of steps 2, 6,
and 8 — before the integration tests that needed them. This step is
the final audit, not the first appearance of exports: the crate root
re-exports exactly the chapter-04 surface and nothing more,
alongside the chapter-03 surface. The chapter's additions:

- From `opening`: `OpeningLoadError`, `OpeningParseError`,
  `TournamentOpenings` — joining the chapter-01 opening names.
- From `generate` (new module, declared between `game` and
  `opening`): `GenerateError`, `generate_games`, `generate_games_with`.
- From `schedule`: nothing new — `options_for_ply` is a method on the
  already-exported `ExplorationSchedule`; methods need no re-export.

Deliberate absences: `MAX_ATTEMPTS_PER_GAME`, `zobrist_walk`,
`opening_moves`, `ListedOffer`, and chapter 01's draw helpers stay
private; the binary's `USAGE`, `Args`, `parse_args`, `parse_value`,
`run`, and `run_with_opening_source` live in the binary and are never
exported; no anchor, export, mix, sample, or sampler module exists.
If any test reached for a path deeper than the crate root, fix the
test to use these re-exports.

Then the regression, the gates, and the capstone. From `gomoku/`:

```text
cargo test -p selfplay
cargo test -p selfplay --test openings --test schedule --test dedup
cargo test -p selfplay --bin selfplaygen
cargo test -p selfplay --test binary selfplaygen_help
cargo test -p selfplay --release --test schedule -- --ignored
cargo test -p selfplay --release --test game -- --ignored
cargo test -p selfplay --release --test grading -- --ignored
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the fast gate, and its report needs a per-target
reading: cargo prints one report per test target, never one aggregate
line. The chapter's closing state is the sum — the game target 5
passed / 2 ignored (chapter 03's state, unchanged), the corpus target
10 passed / 0 ignored (unchanged), openings 5/0, schedule 1/1, dedup
1/0, binary 1/0, the `selfplaygen` binary's own unit tests 4/0, and
grading 0/1 — **27 passed, 4 ignored** in total. The completed
reference's full suite is larger (50 passed, 4 ignored) only because
chapters 05–07 add their own tests; the four ignored tests of the
completed reference are exactly the four this track has after this
chapter — chapter 01's pair, this chapter's schedule game, and this
chapter's grading probe.

The focused targets re-run the chapter's default surface: the parser,
schedule-property, and dedup targets; the binary's unit tests; and
the help contract selected by name, which leaves the six
later-chapter binary tests unrun — they do not exist yet. The three
`--release` commands are the MANUAL selections, and the qualifier is
not optional for any of them: all three play full games. The schedule
selection watches this chapter's scheduled game; the grading
selection is the report-only probe. The game-target selection is the
regression with teeth: chapter 01's MANUAL pair runs the move loop
this chapter rewired, so the budget-2-versus-1 visit-sum test and the
budget-8 smoke test re-prove the loop with the schedule active.

`cargo test -p train` and `cargo test -p player` are not required:
this chapter touches no crate but `selfplay`. The last two commands
are the project's standing quality gates; both must be green with no
changes.

### The capstone: two processes, one byte-identical corpus

Generate a tiny corpus twice and prove the seed discipline
byte-exactly. The output directories are temporary — `mktemp -d`
keeps the run out of the repository's `data/` tree, now and for every
future generation exercise; main `data/` is never a generation
target:

```text
cargo build --release -p selfplay

dir_a=$(mktemp -d)
dir_b=$(mktemp -d)

target/release/selfplaygen \
  --out "$dir_a/corpus" \
  --openings ../data/openings/gomocup-2024-freestyle15.txt \
  --openings ../data/openings/gomocup-2025-freestyle15.txt \
  --openings ../data/openings/gomocup-2026-freestyle15.txt \
  --black-sims 8 --white-sims 8 --games 2 --seed-base 424242

target/release/selfplaygen \
  --out "$dir_b/corpus" \
  --openings ../data/openings/gomocup-2024-freestyle15.txt \
  --openings ../data/openings/gomocup-2025-freestyle15.txt \
  --openings ../data/openings/gomocup-2026-freestyle15.txt \
  --black-sims 8 --white-sims 8 --games 2 --seed-base 424242

cmp "$dir_a/corpus/shard-000.bin" "$dir_b/corpus/shard-000.bin"
cmp "$dir_a/corpus/manifest.json" "$dir_b/corpus/manifest.json"
cat "$dir_a/corpus/manifest.json"
```

Three properties land in one exercise. The default shard capacity
1000 writes a single `shard-000.bin`. The stderr lines report the
declared range (`base seeds 424242..=424243, baseline-8 vs
baseline-8`) before play and the exact accepted seed list after the
write. And both `cmp` calls exit silently: two invocations, two
processes, one seed set, byte-identical shards and manifest — the
project's determinism gate rehearsed at two games instead of the
reference's measured three. The reference measured 2.79 s for three
games at this budget in release; this pair costs less. In debug, the
same two games would cost roughly a minute — the last practical
demonstration of why every game-playing command in this track carries
`--release`.

---

## Done when

- `cargo test -p selfplay` reports **27 passed, 4 ignored** as the
  sum of its per-target reports: game 5 passed / 2 MANUAL-ignored
  (unchanged from chapter 03), corpus 10 passed / 0 ignored
  (unchanged), openings 5/0, schedule 1/1, dedup 1/0, binary 1/0, the
  `selfplaygen` binary's unit tests 4/0, and grading 0/1.
- All three MANUAL selections pass in release: this chapter's
  `scheduled_game_is_reproducible_and_records_its_schedule_and_pairing`
  and `higher_budget_seat_wins_more_often_on_measured_seeds`, and
  chapter 01's pair as the move-loop regression.
- `cargo clippy --all-targets -- -D warnings` is green.
- `cargo fmt --all -- --check` makes no changes.
- The capstone's two `cmp` calls exit silently — byte-identical
  shards and manifest across two release processes — and the run
  wrote only to temporary directories.
- The commit message is
  `feat(selfplay): diversity and grading — selfplay-corpora chapter 04`.

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `selfplay/tests/openings.rs` | five tests, `mv` helper only | parse mappings and provenance; five rejections with list, line, reason; 64-draw seeded equality with full coverage; the 36-offer pool from the committed Gomocup lists; the piskvork line-41 rejection |
| `selfplay/src/opening.rs` | `OpeningParseError` | struct: list, one-based line, reason; display joins all three |
| `selfplay/src/opening.rs` | `OpeningLoadError` | `Empty` before any file; `Read` with path and source; `Parse` transparent |
| `selfplay/src/opening.rs` | `TournamentOpenings::{parse, load, len, is_empty}` | even field count, at least three pairs; signed coordinates, center-relative mapping (row = 7 + y, col = 7 + x, range 0..=14); duplicate rejection; full validation, first three stones offered; per-list zero-based indices; fail fast |
| `selfplay/src/opening.rs` | `impl OpeningSource for TournamentOpenings` | uniform offer draw on the game's stream; three-way answer draw reusing chapter 01's helpers; `Listed` provenance; the chapter-14 §3.3 honesty comment |
| `selfplay/src/schedule.rs` | `options_for_ply` added to the existing type | one import, one method; the constant, struct, derives, and `Default` untouched; inside the window τ/ε/α from the schedule, outside τ = 0 and ε = 0 with α untouched; every other field preserved; must-use, pure |
| `selfplay/tests/schedule.rs` | property test + MANUAL game | loud base options, every field explicit; `EXPLORATION_PLIES == 10`; window and post-window assertions; MANUAL seed 29, budgets 2 and 1, equal records, default schedule recorded |
| `selfplay/src/game.rs` | schedule application | one import, one binding in setup, one inserted line region in the loop; the decision call takes the effective options; everything else byte-identical |
| `selfplay/src/game.rs`, `src/record.rs` | three doc updates | `Game` base-options field docs, the `play_game` schedule sentence, `SeatConfig.temperature` — the reference wordings, no code change |
| `selfplay/src/generate.rs` | `GenerateError`, `generate_games`, `generate_games_with` | transparent `Game` wrap; `DuplicateExhausted` with base seed and attempt count; attempt 0 = base seed, attempt n = wrapping-add; at most 128 candidates; per-run `HashSet` of full walks, opening placements included; accepted seed recorded |
| `selfplay/src/generate.rs` | `MAX_ATTEMPTS_PER_GAME`, `zobrist_walk`, `opening_moves` | private; the walk replays through a fresh board with a key after every move; walk errors reuse `GameError::Opening` and `GameError::Play` |
| `selfplay/tests/dedup.rs` | `forced_collision_is_rejected_and_rerolled_with_seed_plus_attempt` | fabricated games on row seed mod 15; base seeds 7 and 7; accepted seeds exactly 7 and 8; openings differ; no real game played |
| `selfplay/src/bin/selfplaygen.rs` | `USAGE`, `Args`, `parse_args`, `parse_value` | seven flags; defaults 800/800/1/0/1000; last numeric value wins; `--openings` accumulates; zero counts, missing values, unknown flags, missing `--out` rejected; the baseline probe is the parsed default |
| `selfplay/src/bin/selfplaygen.rs` | `main`, `run`, `run_with_opening_source` | help → exit 0; parse failure → message + usage, exit 2; run failure → exit 1; overflow-checked contiguous range; seats from defaults with only budget and probe overridden (resign and guard stay disabled); generate, convert, protected write, read-back equality and pairing check; stderr reports before and after |
| `selfplay/src/bin/selfplaygen.rs` (unit) | four tests | baseline-probe default; all flags with last-wins; missing `--out` and zero games; the budget-1 round trip with injected fixed opening, probe disabled, seeds exactly `[11]`, one shard |
| `selfplay/tests/binary.rs` | `selfplaygen_help_documents_corpus_and_grading_flags` | the chapter's one test in a file that will hold seven; all seven flags in the help text |
| `selfplay/tests/grading.rs` | `higher_budget_seat_wins_more_often_on_measured_seeds` | MANUAL; seeds `[11, 1, 7, 18]`; 16 versus 1; direction only; per-seed stderr report |
| `selfplay/src/lib.rs` | incremental re-exports, audited in step 10 | the `generate` module and its three names, the three opening names; everything else stays private; no chapter-05+ module |
| untouched | `Cargo.toml`, `record.rs`/`corpus.rs` mechanics, `train`, `player`, `engine`, `mcts`, `baseline` | no manifest changes; the record side fills through call sites only |

## References

**This chapter:**

- [diversity-and-grading.md](diversity-and-grading.md) — the chapter
  paper: intention, specification as tests, execution-order
  walkthrough, open observations.
- [diversity-and-grading-design.md](diversity-and-grading-design.md) —
  the conceptual story, the state machine, and the nine design
  invariants this plan builds under.
- [diversity-and-grading-solution.md](diversity-and-grading-solution.md)
  — the annotated reference snippets. Open only after
  twenty stuck minutes, or afterwards to compare.

**This track:**

- [Tutorial README](../README.md) — conventions, gates, the chapter
  map, and the reference provenance.
- [Self-play primer](../00-selfplay-primer.md) — §4.4 fixes the five
  diversity mechanics; §6 holds the measured acceptance evidence; §7
  states the three open observations, including §7.3's release-mode
  convention.
- [Chapter 01 — the game runner](../01-the-game-runner/the-game-runner.md),
  its [implementation plan](../01-the-game-runner/the-game-runner-implementation.md),
  and its [solution](../01-the-game-runner/the-game-runner-solution.md)
  — the `OpeningSource` seam, `GeneratedOpening` and its private draw
  helpers, the single-RNG discipline, the move loop this chapter
  schedules, and the recorded `play_game` doc delta this chapter
  retires.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md),
  its [implementation plan](../02-recording-games/recording-games-implementation.md),
  and its [solution](../02-recording-games/recording-games-solution.md)
  — the `ExplorationSchedule` vocabulary this chapter must not
  recreate, `SeatPairing.exploration`, `Manifest.seeds`,
  `GameRecord::from_game`, the writer and readers, and the
  `SeatConfig.temperature` slice wording this chapter retires.
- [Chapter 03 — recording the search](../03-recording-the-search/recording-the-search.md),
  its [implementation plan](../03-recording-the-search/recording-the-search-implementation.md),
  and its [solution](../03-recording-the-search/recording-the-search-solution.md)
  — the runner's current line region (the chapter-03 `map` call) that
  step 4 extends, and the honesty rule for certified-proof moves that
  the schedule never disturbs.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  §3.1 the runner shape, §4 the five diversity mechanics, §5 the
  acceptance gates; decisions S5 (grading as primary axis) and S6
  (synchronous, one seed base per process).
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  this chapter's public surface, the measured headline numbers, and
  the open questions.
- [docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) —
  the claim-label convention (**[paper]** / **[derived]** /
  **[experiment]**).
- [docs/14-openings-and-supervised-curriculum.md](../../../14-openings-and-supervised-curriculum.md) —
  §3.2 the tier-1 tournament lists, §3.3 the perturbed answering
  argmax this chapter approximates with a seeded uniform choice
  **[invention]**.
- [docs/13-engine-design.md](../../../13-engine-design.md) — the
  locked absolute-color decision the opening vocabulary carries.

**Prerequisite tracks:**

- [Baseline tutorial](../../baseline-tutorial/README.md) —
  `AiPlayer::decide`, `PlayerOptions`, `OwnTssConfig::baseline()`
  (10 000 nodes / 30 plies), and the guard.
- [MCTS tutorial](../../mcts-tutorial/README.md) — the PUCT search
  whose root carries the Dirichlet noise.
- [Engine tutorial](../../13-engine-tutorial/README.md) — the Swap2
  typestate, `Move::new`, and the Zobrist keys this chapter walks.
- [Datagen tutorial](../../datagen-tutorial/README.md) — the `train`
  crate's shard and manifest storage discipline.
- [gomoku/crates/cli/README.md](../../../../gomoku/crates/cli/README.md) —
  the AI-vs-AI exploration practice the schedule mirrors.
- [data/openings/README.md](../../../../data/openings/README.md) —
  the committed opening lists and their sources.

**Source (created by this chapter; not yet present on `main`):**

- `gomoku/crates/selfplay/src/generate.rs` — `GenerateError`,
  `generate_games`, `generate_games_with`; private
  `MAX_ATTEMPTS_PER_GAME`, `zobrist_walk`, `opening_moves`.
- `gomoku/crates/selfplay/src/bin/selfplaygen.rs` — the corpus
  generator binary and its four unit tests.
- `gomoku/crates/selfplay/tests/openings.rs`,
  `tests/schedule.rs`, `tests/dedup.rs`, `tests/binary.rs`,
  `tests/grading.rs` — the chapter's specification.

**Source (created by earlier chapters, touched by this chapter; not
yet present on `main`):**

- `gomoku/crates/selfplay/src/opening.rs` — gains
  `OpeningParseError`, `OpeningLoadError`, `TournamentOpenings` with
  `parse`, `load`, `len`, `is_empty`, and the `OpeningSource`
  implementation.
- `gomoku/crates/selfplay/src/schedule.rs` — gains the
  `PlayerOptions` import and `ExplorationSchedule::options_for_ply`;
  nothing else changes.
- `gomoku/crates/selfplay/src/game.rs` — the move loop applies the
  schedule per ply; three doc comments move to the reference
  wordings.
- `gomoku/crates/selfplay/src/record.rs` — the
  `SeatConfig.temperature` doc comment moves to the reference
  wording; no code change.
- `gomoku/crates/selfplay/src/lib.rs` — declares the `generate`
  module and re-exports the new vocabulary.

**Source (unchanged in this chapter):**

- `gomoku/crates/selfplay/src/record.rs` and `src/corpus.rs`
  mechanics — `GameRecord::from_game`, `Manifest.seeds`,
  `write_corpus`, `read_corpus`, `read_manifest`; consumed through
  call sites only.
- `gomoku/crates/selfplay/tests/game.rs` and `tests/corpus.rs` — the
  earlier chapters' specifications; regression evidence.
- [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs) —
  `PlayerOptions`, `OwnTssConfig`; the options the schedule adjusts
  and the probe the binary declares. Not touched.
- [`gomoku/crates/selfplay/Cargo.toml`](../../../../gomoku/crates/selfplay/Cargo.toml) —
  no manifest changes.

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). https://doi.org/10.1038/nature24270 —
  temperature sampling over visit counts and root Dirichlet noise,
  the exploration mechanisms the schedule applies.
- Rosin, C. D. (2011). *Multi-armed bandits with episode context*
  (PUCT). https://doi.org/10.1007/s10472-011-9258-6 — the selection
  rule of the search the schedule leaves untouched.
- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://doi.org/10.26481/dis.19940923la — threat-space search, the
  seat probe whose configuration the binary declares.
