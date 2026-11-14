# Chapter 07 — Acceptance: implementation plan

## Abstract

This document is the implementation plan for
[chapter 07 of the selfplay-corpora tutorial](acceptance.md): the
hiding layer between the chapter paper and the solution, for the
learner who builds the code without reading the reference. It turns
the chapter's walkthrough into a numbered construction order along the
execution path of one `selfplayverify` invocation from argument to
exit status: the four `selfplayverify_*` specification tests land in
the existing `tests/binary.rs` first, watched red against a stub
binary; then the new `src/bin/selfplayverify.rs` grows in execution
order — `parse_corpus` and the corpus read with its empty-corpus
rejection, `verify_record` with its opening and per-ply replay, the
three-rule outcome comparison, and `Summary` with `OutcomeCounts` and
the unconditional pretty-JSON report — one test going green per step.
Every artifact names its file, its behavioral contract in prose, the
helpers to reuse, and the idioms to apply by name; method bodies never
appear. The plan pins the exact test names, fixtures, error
substrings, exit codes, and the nine JSON assertions of the report
test, and the per-target counts that sum to the track's closing state
of 50 passed and 4 ignored for `selfplay`. Two files change and
nothing else: `src/bin/selfplayverify.rs` is created, and
`tests/binary.rs` gains exactly four tests. No library module, no
`src/lib.rs` edit, no manifest change, no unit tests on the binary.
The intended reader has completed chapter 06 (suite state: 46 passed,
4 ignored), has read the chapter paper and, ideally, the
[design paper](acceptance-design.md), and wants low-level guidance
without solution leakage.

## Glossary

| Term | Definition |
|------|------------|
| **Acceptance gate** | One of the five repeatable checks of the design spec §5 — determinism, legality, grading, corpus, integration — that together decide whether the tutorial's corpus pipeline is accepted. |
| **Anchor seed** | `AnchorSeed { game_index, from_ply, line }`: a (position, forcing move, ±1) triple extracted from a certified suffix; the input format of the milestone-4 puzzle anchor set. |
| **Certified suffix** | `CertifiedSuffix { from_ply, line }`: a terminal segment of a game in which every attacker move belongs to a machine-verified forced-win line produced by the engine's threat-space search (TSS) prover. |
| **Claim labels** | The project's provenance labels, from three sources: the [architecture document](../../../12-gomoku-architecture.md)'s head defines **[experiment]** (a free parameter or bar to be settled by measurement) and **[derived]** (a value computed from cited or measured values); the [design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) introduces **[invention]**; and **[measured]** — a number recorded in the reference's acceptance logs — is the convention of the tutorial [hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) and the reference's acceptance evidence. |
| **Corpus** | A set of game records produced at one declared pairing, stored as shard files plus a manifest in one directory. |
| **Deduplicated count** | A test count taken per target: each suite contributes its own tests once. Contrasts with the raw result-line count; the two differ because one test re-runs its own binary in child processes. |
| **Game record** | `GameRecord`: the durable serialization of one complete game — a header (seed, seat pairing, evaluator identity, the Swap2 opening), the ordered post-opening `plies`, the `outcome`, and an optional certified suffix. Chapter 02 fixed the format. |
| **MANUAL test** | A slow game-playing test marked `#[ignore]` with a MANUAL comment, run by explicit selection in release mode, never by the default gate. This chapter adds none. |
| **Manifest** | The corpus writer's JSON provenance, serialized as `manifest.json`: format version, seed set, pairing, game count, shard list, and workspace version. The shards are the data; the manifest is the provenance — and this chapter's verifier never opens it. |
| **Opening offer / opening answer** | The three-stone opening one side places in the Swap2 ceremony, and the other side's reply (`OpeningAnswer::Take` a color, or `OpeningAnswer::AddTwo` with a final color choice). |
| **Outcome / Status** | `Outcome` is the record's terminal claim (`Won { winner, reason }` with `WinReason::FiveOrMore` or `WinReason::Resignation(..)`, or `Draw`); `engine::Status` is the replayed board's own terminal state (`Ongoing`, `Won(Color)`, `Draw`). The verifier compares the two. |
| **Pairing** | The declared simulation budgets of the two seats of a corpus: matched (X vs X) or graded (X vs Y). Recorded in every game header. |
| **π (policy target)** | The normalized root visit-count distribution of the search at a ply; the policy training target (Silver et al. 2017). Computed from the recorded raw visit counts at read time. |
| **Post-opening ply** | One played move after the Swap2 ceremony, counted from zero. Every `ply` index in this chapter counts in these plies; the ceremony placements are not plies. |
| **Raw result-line count** | A test count taken by summing every `test result:` line in a log, including the lines printed by child processes. Contrasts with the deduplicated count. |
| **Replay** | Rebuilding a position by playing a record's opening and its moves through a fresh `engine::Board`. The engine is the only position representation the verification path trusts. |
| **Shard** | A fixed-capacity file of length-delimited bincode records, named `shard-NNN.bin`; the `train` crate's storage unit. |
| **Swap2** | The opening protocol of this project: one side places three stones, the other chooses a color or adds two stones, and the first side then chooses a color. |
| **TDD** | Test-driven development: write the test, watch it fail (red), implement, watch it pass (green). |
| **The reference** | The reference implementation of this tutorial track: built test-first, gate-verified, and acceptance-measured before the tutorial was written. It lives only in a read-only worktree beside this repository; the learner's own implementation on `main` is the reference in the didactic sense. |
| **Threat-space search (TSS)** | A proof search over forced-win threat sequences (Allis 1994). The engine ships a bounded prover and a verifier; the prover may be incomplete, never unsound. |
| **verify_line** | `engine::verify_line(board, proof) -> bool`: the soundness net. It replays a certified line and, at every defender turn, enumerates every reply that addresses the threat; the attacker's continuation must win against all of them. Chapter 05 owns this gate. |
| **z (value target)** | The game outcome in {+1, 0, −1} from the perspective of the side to move at a ply; the value training target. Derived at read time. |

## How to use this plan

Read [the chapter paper](acceptance.md) first. It carries the
intention, the specification as tests, the execution-order
walkthrough, and the five acceptance gates with their measured
evidence; the [design paper](acceptance-design.md) carries the
reasoning and the ten invariants. This plan assumes both and adds only
construction detail: which file, which item, which contract, which
test goes red then green. Work one step at a time, in order — the
order is the execution order of one `selfplayverify` invocation:
argument parsing first, then the corpus read, then the per-game
replay, then the outcome adjudication, then the report. There is
exactly one new production file, so there is no dependency order to
manage; each step grows the same binary and watches exactly one more
test go green. Open
[acceptance-solution.md](acceptance-solution.md) only if
you are stuck for more than twenty minutes, or after the chapter is
complete.

## Context

The chapter-06 slice — the verified starting state — holds the
following. Selfplay paths are under
[`gomoku/crates/selfplay/`](../../../../gomoku/crates/selfplay/);
on `main` the crate lands chapter by chapter as the track is worked,
so the files below exist once the learner's chapters 01–06 have
landed.

- [`src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs)
  (chapters 02–03) carries `GameRecord` with its public `header`,
  `plies`, `outcome`, and `certified_suffix` fields; `GameHeader` with
  the `opening` slot; `RecordedPly` with its public `mv`; and
  `GameRecord::from_game`, the conversion the runner and the test
  fixture share. **This chapter does not touch `src/record.rs`.**
- [`src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs)
  (chapters 01–05) carries `Game` (seed, per-seat options, opening,
  plies, outcome, certified suffix), `PlyRecord { mv, search }` with
  `search: Option<PlySearch>`, `Outcome`, and `WinReason`. The fixture
  fabricates a `Game` by hand and converts it with
  `GameRecord::from_game`, so no test plays a game.
- [`src/opening.rs`](../../../../gomoku/crates/selfplay/src/opening.rs)
  (chapters 01 and 04) carries `Opening::new` — the constant
  constructor from the three initial cells, an `OpeningAnswer`, and an
  `OpeningProvenance` — and `Opening::board`, which executes the
  ceremony through the engine's Swap2 typestate into a fresh
  `Result<Board, PlayError>`. Replay starts here.
- [`src/corpus.rs`](../../../../gomoku/crates/selfplay/src/corpus.rs)
  (chapter 02) carries `write_corpus`, which the tests use to
  fabricate corpora, and `read_corpus`, the binary's only input path:
  it lists a corpus directory's `shard-*.bin` files, sorts them, and
  decodes every record into a `Vec<GameRecord>` in record order. It
  never opens `manifest.json`; `read_manifest` is a separate API, and
  this chapter's binary does not call it — the manifest-versus-shard
  cross-check is implemented nowhere in the tutorial and stands as an
  open hardening gap, recorded in chapter 02's corrected honesty
  boundary and in the chapter paper's walkthrough §2. Do not add the
  call, and do not let any sentence in code or documentation promise
  the check.
- [`src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs)
  declares eleven modules behind `#![deny(missing_docs)]` and
  re-exports the chapter-06 surface. **This chapter does not touch
  `src/lib.rs`**: the binary is a private consumer of the existing
  crate-root vocabulary — `GameRecord`, `OpeningAnswer`,
  `OpeningProvenance`, `Outcome`, `WinReason`, and `read_corpus` — and
  adds nothing to the public surface.
- [`Cargo.toml`](../../../../gomoku/crates/selfplay/Cargo.toml) needs
  **no changes**: by chapter 06 the crate already depends on
  `baseline`, `engine`, `player`, `rand`, `serde`, `serde_json`,
  `thiserror`, and `train`; the binary needs only `engine`, the crate
  itself, and `serde`/`serde_json` for the report, and a binary in
  `src/bin/` needs no manifest entry. Verify this before writing
  anything; if a step seems to need a dependency, the step has
  drifted.
- The engine (consumed, unchanged) carries `Board` with `play`,
  `status`, and `to_move`; `Status` (`Ongoing`, `Won(Color)`, `Draw`);
  `Color` with `Color::other`; `Move`; and `PlayError`, whose display
  text carries the rejection reason into the verifier's error messages
  — the occupied-cell case displays as `Cell is already occupied.`
- [`tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs)
  holds the three tests chapter 05 left there —
  `selfplaygen_help_documents_corpus_and_grading_flags`,
  `selfplayanchors_help_documents_corpus_extraction`, and
  `selfplayanchors_reads_a_corpus_and_writes_json_anchor_seeds` — and,
  above them, the three shared helpers this chapter reuses
  **unchanged**: `TestDir` (a temporary directory under the system
  temp dir named `selfplayanchors-test-<pid>-<nanos>`, with a `path`
  accessor and removal on `Drop`), `mv(row, col)` (the coordinate
  shorthand, validating through `Move::new`), and `certified_record()`
  (the one fabricated record three of this chapter's tests share; see
  step 1). The target's imports already cover everything the four new
  tests need: `Command`, `serde_json`, `std::fs`, and the crate-root
  vocabulary including `Color`, `Outcome`, `WinReason`, and
  `write_corpus`. No new import is required.
- Test targets at the chapter-06 state: `tests/game.rs` 5 passed / 2
  MANUAL-ignored; `tests/corpus.rs` 10/0; `tests/openings.rs` 5/0;
  `tests/schedule.rs` 1/1; `tests/dedup.rs` 1/0; `tests/grading.rs`
  0/1; `tests/anchors.rs` 4/0; `tests/binary.rs` 3/0;
  `tests/read_path.rs` 11/0; the `src` unit tests 6/0 (the library
  target's two tests in `game.rs` plus the `selfplaygen` binary's
  four) — **46 passed, 4 ignored** in sum, across 14 test suites.

Four scope boundaries keep the chapter honest. First, exactly two
files change: `src/bin/selfplayverify.rs` is created, and
`tests/binary.rs` gains exactly four tests. Everything else is
untouched: `src/lib.rs` and every library module; `Cargo.toml`; the
three existing `tests/binary.rs` tests and the `TestDir`, `mv`, and
`certified_record` helpers; every other test target; and the `engine`,
`train`, `player`, and `net` crates. Second, the new binary carries
**no** `#[cfg(test)]` module — like `selfplayanchors` and
`selfplayexport` before it, its whole specification lives in the
integration target — so the `src` unit-test count stays at 6 and the
`selfplayverify` test target reports zero tests of its own. Third, no
test in this chapter plays a game: each fabricates its records by hand
and drives the compiled binary through
`env!("CARGO_BIN_EXE_selfplayverify")`, so the fast gate stays fast
(the reference runs the whole target in about 0.2 s in debug) and the
release-only MANUAL discipline of chapter 04 is untouched — the four
ignored tests remain exactly chapter 04's set (the two game-playing
probes in `tests/game.rs`, the scheduled-game reproducibility test in
`tests/schedule.rs`, and the grading probe in `tests/grading.rs`).
Fourth, `cargo test -p train`, `cargo test -p player`, and
`cargo test -p net` are not chapter-07 gates: those crates are
untouched.

---

## Step 1 — `tests/binary.rs`: the four `selfplayverify_*` tests (red)

### 1.1 The shared fixture, as it already exists

No fixture work is chapter-07 work. The three record-based tests share
the `certified_record()` helper chapter 05 introduced; read it before
writing the tests, because every assertion below is a consequence of
its shape. The fixture's Swap2 opening places stones at `(5, 5)`,
`(10, 10)`, and `(5, 6)` through `Opening::new`, with the answer
`OpeningAnswer::Take(Color::Black)` — so White moves first after the
ceremony — and `OpeningProvenance::Generated`. The six post-opening
plies are `(10, 11)`, `(5, 7)`, `(10, 12)`, `(5, 8)`, `(10, 13)`,
`(5, 9)`: White builds on row 10, Black on row 5, and Black's last
move completes the five `(5, 5)` through `(5, 9)`. The outcome is a
Black win by five-or-more. A certified suffix from ply 5 carries the
one-move line `[(5, 9)]`. Every ply is unsearched (`search: None`) —
the verifier never looks at search data, so the fixture needs none.
The empty-corpus test creates no record at all.

### 1.2 The four tests, in teaching order

Append the four tests to
[`tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs),
ordered along the execution path of the future program: the
corpus-level rejection first (it fires before any game is verified),
then the per-ply replay rejection, then the terminal outcome
comparison, and the success report last. Each test builds its own
`TestDir`, writes its corpus with `write_corpus(&[...], &corpus, 1)`
expected with the message "fabricated corpus writes" (the record-based
tests), spawns the compiled binary through
`Command::new(env!("CARGO_BIN_EXE_selfplayverify"))` with `--corpus`
pointed at the corpus directory, expected with "selfplayverify runs",
and asserts on the exit status and the captured streams. All four run
by default; none is ignored.

1. `selfplayverify_rejects_a_corpus_without_game_records` — create an
   empty directory under the `TestDir` with `std::fs::create_dir_all`,
   expected with "empty corpus directory exists"; no record, no
   `write_corpus` call. Run the binary against it. Assert the exit
   status is **not** success, and assert stderr contains the substring
   `no game records`, with the captured stderr in the failure message.
   A corpus with zero records is an error, never an empty report: an
   empty report would be indistinguishable from a mistyped path that
   happened to be a directory.
2. `selfplayverify_rejects_an_illegal_recorded_move` — take
   `certified_record()` mutably and doctor ply index 1: replace its
   `mv` with `mv(5, 5)`, a cell the opening already occupies. Write
   the one-record corpus and run the binary. Assert the exit status is
   not success, and assert stderr contains both `ply 1` and
   `occupied`, each with the captured stderr in the failure message.
   The replay runs through a real `engine::Board`, so the engine's own
   `PlayError` supplies the reason (`Cell is already occupied.`), and
   the verifier's message names the corpus position of the game and
   the ply index of the rejected move — the full production message
   has the shape `game 0: move <MOVE> at ply 1 was rejected: …`; the
   test pins the two substrings, not the rendering of the move.
3. `selfplayverify_rejects_a_recorded_outcome_that_disagrees_with_the_engine` —
   take `certified_record()` mutably and doctor only the outcome: a
   *White* win by five-or-more. The moves are unchanged, so the replay
   still ends in Black's five. Write the corpus and run the binary.
   Assert the exit status is not success, and assert stderr contains
   both `final status` and `game 0`. The recorded outcome is a claim
   about the terminal position; the engine replays the position and
   adjudicates the claim.
4. `selfplayverify_reports_replay_and_corpus_statistics_as_json` —
   write the unmodified `certified_record()` as a one-record corpus
   and run the binary. Assert the exit status **is** success, with the
   captured stderr in the failure message. Parse stdout as a
   `serde_json::Value` through `serde_json::from_slice`, expected with
   "summary is JSON". Then pin the report's schema field by field with
   exactly nine equality assertions against the `Value`:

   - `summary["game_count"]` is `1`;
   - `summary["verified_plies"]` is `6` (the six post-opening plies,
     all replayed);
   - `summary["certified_suffixes"]` is `1` (the fixture carries one);
   - `summary["outcomes"]["black"]` is `1`;
   - `summary["outcomes"]["white"]` is `0`;
   - `summary["outcomes"]["draw"]` is `0`;
   - `summary["length_histogram_plies"]["6"]` is `1` (one game of six
     plies — the histogram is a map, so JSON object keys are strings);
   - `summary["games_ending_within_16_plies"]` is `1` (six is at most
     sixteen);
   - `summary["opening_offers"]["Generated"]` is `1` (the fixture's
     provenance).

   The success path is a machine-readable report, not a prose summary,
   because corpus quality is something later tooling and documents
   quote.

No fifth test. Resist pinning the parse-failure exit codes, the
opening-replay failure, or the resignation rule as unit tests: the
command-line contract is demonstrated by the capstone's boundary
experiments, and the four fabricated-record tests are the binary's
whole specification.

### 1.3 The red scaffold: a stub binary

The red state is a compile error, and it deserves one sentence of
explanation because it is the same mechanism chapter 06 used. The
tests obtain the binary's path from Cargo's compile-time binary
environment variable `env!("CARGO_BIN_EXE_selfplayverify")`; the
`env!` macro resolves at compile time, and Cargo sets the variable for
integration tests only when the binary target exists. Until
[`src/bin/selfplayverify.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayverify.rs)
exists, `tests/binary.rs` does not compile at all — the three existing
tests cannot run either. Turn the compile error into four honest test
failures with the smallest possible scaffold: create the file with an
empty `main` and nothing else. The stub binary starts, prints nothing,
and exits 0, so: the three rejection tests fail on their not-success
assertions, the report test fails parsing the empty stdout, and the
three chapter-05 tests stay green.

```text
cargo test -p selfplay --test binary
```

reports **3 passed, 4 failed** — the four `selfplayverify_*` tests red
for the right reason.

---

## Step 2 — the binary's front half: `parse_corpus`, the corpus read, and the empty-corpus rejection (red → green)

Grow
[`src/bin/selfplayverify.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayverify.rs)
from the stub. Open the file with a one-line module doc comment naming
the binary's job: replay self-play records and reject illegal moves or
inconsistent outcomes. Imports, all used by this step or the later
ones: the standard library's `BTreeMap`, `PathBuf`, `env`, and
`process`; the engine's `Color` and `Status`; the crate root's
`GameRecord`, `OpeningAnswer`, `OpeningProvenance`, `Outcome`,
`WinReason`, and `read_corpus`; and serde's `Serialize` derive. The
binary speaks crate-root names only; no path deeper than `selfplay::`
or `engine::` appears.

### 2.1 `parse_corpus`

A private function, `parse_corpus(args) -> Result<PathBuf, String>`,
generic over the argument iterator.

Hand-rolled parsing, no dependency, in the style of the earlier
binaries — but thinner, because the vocabulary is exactly one required
flag: `--corpus <DIR>`. Behavioral contract:

1. Walk the argument iterator. `--corpus` consumes the following
   argument as the path value; a `--corpus` without a following
   argument fails with exactly `--corpus requires a value`.
2. Any other argument fails with exactly `unknown flag: <FLAG>`. There
   is deliberately no `--help` special case (the whole interface is
   the one flag, and every parse error already prints the usage line),
   no `--json` (the report is always JSON; an opt-in flag would create
   a second output mode), no `--anchors` (certificate re-verification
   is `selfplayanchors`' pass, chapter 05), and no `--limit` (a
   verified prefix is not a verified corpus; limiting work is
   `selfplaygen --games`' decision). Passing `--help` therefore fails
   like any other unknown flag.
3. An exhausted iterator with no corpus seen fails with exactly
   `--corpus is required`.

### 2.2 `main`: the parse, read, and empty-corpus branches

`main` owns the exit codes from the start; later steps insert their
stages between these branches. Behavioral contract, in execution
order:

1. Parse the process arguments after the program name through
   `parse_corpus`. On failure, print the message followed by the
   one-line usage `Usage: selfplayverify --corpus <DIR>` to **stderr**
   and exit with code **2**.
2. Read the corpus with chapter 02's `read_corpus` — shard files only,
   sorted, decoded in record order; the manifest is never opened. A
   read failure prints `selfplayverify failed: could not read corpus:
   {error}` to stderr and exits with code **1**.
3. An empty record vector prints `selfplayverify failed: corpus
   contains no game records` to stderr and exits with code **1** —
   step 1's first test pins the substring. Every number the report
   will eventually show is derived from these decoded records, so the
   report describes the data that is there, never the provenance the
   writer claimed; do not read the manifest here or anywhere else in
   the binary.

Watch the first test go green:

```text
cargo test -p selfplay --test binary
```

reports **4 passed, 3 failed** — the empty-corpus test passes; the two
rejection tests still fail because nothing is replayed yet, and the
report test still fails because nothing is printed.

---

## Step 3 — `verify_record`: the opening and per-ply replay (red → green)

A private function, `verify_record(game_index, record) ->
Result<(), String>`, taking the record's corpus-order position and a
borrowed `&GameRecord`.

Behavioral contract, in execution order:

1. **Replay the opening.** `record.header.opening.board()` executes
   the Swap2 ceremony into a fresh `engine::Board` — chapter 01's
   typestate path, consumed unchanged. A rejection maps to an error of
   the shape `game <N>: opening replay failed: {error}`, where the
   engine's `PlayError` display carries the reason.
2. **Replay every recorded move.** Iterate `record.plies` with
   `enumerate` and play each `recorded.mv` through the board. The
   first rejection stops the check with an error of the shape
   `game <N>: move <MOVE> at ply <PLY> was rejected: {error}` — the
   move rendered with its debug format, the engine's `PlayError` text
   carrying the reason. Two indexing decisions are part of the
   contract: `game_index` is the record's position in corpus order —
   the same indexing chapter 05's `AnchorSeed`s use, so an error and
   an anchor seed cross-reference without a translation table — and
   `ply` is the zero-based post-opening ply index, matching the
   record's own `plies` vector, so the index in the error is the index
   in the data.
3. Return `Ok(())` when every move replayed. (Step 4 inserts the
   outcome comparison before this return.)

Wire the check into `main` between the empty-corpus rejection and the
(still missing) report: iterate the records with `enumerate`, call
`verify_record`, and on the first error print `selfplayverify failed:
{error}` to stderr and exit with code **1**. Failure handling is
all-or-nothing: the run stops at the first failing game, and no
partial report exists — a corpus of ten thousand legal games and one
illegal move is a rejected corpus, not a 99.99 % report.

Watch the second test go green:

```text
cargo test -p selfplay --test binary
```

reports **5 passed, 2 failed** — the illegal-move test passes (the
fixture's doctored ply 1 hits the occupied opening cell); the outcome
test still fails because no comparison runs yet, and the report test
still fails.

---

## Step 4 — the outcome comparison (red → green)

Extend `verify_record` after the replay loop: the replayed board's own
`status()` adjudicates the recorded `outcome`. Three rules, one per
outcome shape:

- `Won { winner, reason: WinReason::FiveOrMore }` requires
  `status == Status::Won(winner)` — the engine must see the same
  five-or-more on the board.
- `Won { winner, reason: WinReason::Resignation(..) }` requires
  `status == Status::Ongoing` **and** `board.to_move() ==
  winner.other()` — a resignation truncates the game with the *loser*
  to move, so the replayed board must still be open with the losing
  side's turn pending. The resignation reason itself is not replayed;
  it is a decision record, not a board state. This is the subtle rule:
  it pins a non-terminal board state and a side to move, where the
  other two pin terminal ones.
- `Draw` requires `status == Status::Draw` — the board full, no
  winner.

Any disagreement returns an error of the shape `game <N>: final status
<STATUS> does not match recorded outcome <OUTCOME>`, both rendered
with their debug formats — the message step 1's third test pins by its
`final status` and `game 0` substrings. This is the honesty check the
whole format depends on: z, the value target, is *derived* from the
recorded outcome at read time (chapter 06), so a recorded outcome the
engine contradicts would poison every value target derived from it.

Watch the third test go green:

```text
cargo test -p selfplay --test binary
```

reports **6 passed, 1 failed** — the outcome test passes (the doctored
White-win claim meets the engine's Black five); only the report test
remains red.

---

## Step 5 — `Summary`, `OutcomeCounts`, and the report (red → green)

### 5.1 The two report types

Both private, both deriving serde's `Serialize` and nothing else —
the report is written, never read back by this binary, so no
`Deserialize`, and no `Debug`: the JSON on stdout is the only
rendering.

`OutcomeCounts` holds three `usize` fields: `black`, `white`, `draw`.

`Summary` holds exactly twelve fields, in this declaration order —
serialization follows declaration order, and the chapter paper's
example report shows it:

```text
game_count: usize
verified_plies: usize
certified_suffixes: usize
games_ending_within_16_plies: usize
min_plies: usize
max_plies: usize
mean_plies: f64
median_plies: f64
length_histogram_plies: BTreeMap<usize, usize>
opening_offers: BTreeMap<String, usize>
opening_answers: BTreeMap<String, usize>
outcomes: OutcomeCounts
```

Every map is a `BTreeMap`: iteration order is key order, so
serialization order is fixed — one of the three decisions (fixed
input, sorted maps, literal path keys) that make the report a
deterministic function of the corpus. In JSON the map keys become
object keys, hence strings, which is why the test indexes
`length_histogram_plies` with `"6"`, and why lexicographic string
order, not numeric order, sorts the opening-offer keys.

### 5.2 `Summary::from_records`

A private associated function, `Summary::from_records(records) ->
Self`, taking the verified records as a borrowed `&[GameRecord]`
slice.

Behavioral contract. The derivation happens only after `main`'s
all-or-nothing loop has accepted every record, so the report's counts
and its adjective agree: every counted ply is a verified ply.

1. **Lengths.** Collect each record's post-opening ply count into a
   vector and sort it. `verified_plies` is the sum. `min_plies` and
   `max_plies` are the sorted extremes. `mean_plies` is the sum over
   the count, in `f64`. `median_plies`: for an odd count, the middle
   value; for an even count, the mean of the two middle values, in
   `f64` — so a median can end in `.5`. `games_ending_within_16_plies`
   counts the lengths at most 16 — the tactical short tail the corpus
   gate reports on. The opening placements themselves were replayed in
   step 3 but are counted nowhere here: lengths are post-opening
   lengths.
2. **The histogram.** `length_histogram_plies` counts exact length
   frequencies with the entry-or-insert idiom — exact frequencies, not
   bins: a bin boundary would be an opinion, and the corpus gate's job
   is to report, not to interpret.
3. **The offer keys.** One counter per opening offer, keyed by the
   header's provenance: `Generated` for a generated opening, or
   `<list>#<index>` for a listed one, where `<list>` is the opening
   list's path **as recorded in the header** — the literal string the
   generation command's `--openings` flag carried, never normalized.
   Normalizing would make differently-spelled runs comparable at the
   cost of inventing a canonicalization rule; the design keeps the
   bytes and accepts the spellings.
4. **The answer keys.** One counter per Swap2 answer shape, exactly
   four possible strings: `take_black`, `take_white`,
   `add_two_take_black`, `add_two_take_white` — the `AddTwo` variants
   key on their final color choice.
5. **The outcomes.** One increment of `OutcomeCounts` per record, by
   the winner's color or the draw. Every counted outcome passed step
   4's engine comparison first.
6. **The suffix count.** `certified_suffixes` counts the records that
   *carry* a certified suffix — presence, not validity. The
   `verify_line` re-verification is `selfplayanchors`' pass (chapter
   05), and the acceptance legality gate runs both binaries; a count
   here is never a proof, and nothing in this binary calls
   `verify_line`.

### 5.3 The report branch in `main`

After the verification loop, build the summary from the records and
serialize it with `serde_json`'s pretty printer to a string, then
print it to **stdout**; success falls off the end — exit code **0**. A
serialization failure prints `selfplayverify failed: could not
serialize summary: {error}` to stderr and exits with code **1**. The
exit-code discipline is now complete, and it is the binary's contract
in one table: 0 means verified with a report on stdout; 1 means a
read, empty-corpus, replay, outcome, or serialization failure with a
named error on stderr; 2 means command-line misuse with the usage line
on stderr. There is exactly one way to run the binary, so a green exit
status always means the same thing.

The binary carries **no** `#[cfg(test)]` module: the four integration
tests are its whole specification, the `selfplayverify` test target
reports zero tests of its own, and the `src` unit-test count stays at
6.

Watch the fourth test go green:

```text
cargo test -p selfplay --test binary
```

reports **7 passed, 0 failed** — the chapter's complete new surface,
in about 0.2 s in debug [measured]: no test plays a game, and every
test drives the compiled binary over a fabricated one-record corpus.

---

## Step 6 — audit, regression, standing gates, capstone

### 6.1 The audit

Read the two changed files top to bottom against this list.
`src/bin/selfplayverify.rs` holds exactly the module doc comment, the
imports, `main`, `Summary`, `OutcomeCounts`, `Summary::from_records`,
`parse_corpus`, and `verify_record` — every item private, nothing
re-exported, nothing `pub`. `tests/binary.rs` holds the three
chapter-05 tests unchanged, the three helpers unchanged, and exactly
the four `selfplayverify_*` tests of step 1. Nothing else in the
workspace changed: `git status` names exactly two files under
`gomoku/crates/selfplay/`. Deliberate absences, each a decision from
the design paper's rejected-alternatives table: no `--help`, no
`--json`, no `--anchors`, no `--limit`; no `read_manifest` call; no
`verify_line` call; no inspection of recorded search data (the visit
histograms, sparse priors, and root values are audit and training
material, not legality material — the π (policy target) contract
lives in the read path, where chapter 06's tests pin the named
`SampleError::ZeroVisitTotal` failure, and the export exercises it at
corpus scale); no strength claim anywhere.

### 6.2 Regression

The chapter-06 targets stay green unchanged — game 5/2, corpus 10/0,
openings 5/0, schedule 1/1, dedup 1/0, grading 0/1, anchors 4/0,
read_path 11/0, the `src` unit tests 6/0 — because the chapter only
reads what earlier chapters wrote: the record format, the corpus
mechanics, the opening ceremony, and the engine are untouched, and no
earlier test imports the new vocabulary (there is none to import — the
binary exports nothing). `cargo test -p train`, `cargo test -p
player`, and `cargo test -p net` are not required: all three crates
are untouched. No new MANUAL test exists, and the four ignored tests
remain exactly chapter 04's set; they are not run by the default gate,
and this chapter adds none.

### 6.3 The standing gates

From `gomoku/`:

```text
cargo test -p selfplay --release
cargo test -p selfplay --test binary
cargo test -p selfplay --test binary selfplayverify
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the final default gate of the track, run in
release per the game-playing discipline of chapter 04 — the default
suite plays its small budget-1 and budget-2 games, and release keeps
the gate at seconds. Its report needs a per-target reading: cargo
prints one report per test target, never one aggregate line. The
track's closing state is the sum, per target:

| Target | Passed | Ignored |
|---|---:|---:|
| `src/lib.rs` (unit — the lib target's two tests, both in `src/game.rs`) | 2 | 0 |
| `src/bin/selfplaygen.rs` (unit) | 4 | 0 |
| `src/bin/selfplayanchors.rs` (unit) | 0 | 0 |
| `src/bin/selfplayexport.rs` (unit) | 0 | 0 |
| `src/bin/selfplayverify.rs` (unit) | 0 | 0 |
| `tests/anchors.rs` | 4 | 0 |
| `tests/binary.rs` | 7 | 0 |
| `tests/corpus.rs` | 10 | 0 |
| `tests/dedup.rs` | 1 | 0 |
| `tests/game.rs` | 5 | 2 |
| `tests/grading.rs` | 0 | 1 |
| `tests/openings.rs` | 5 | 0 |
| `tests/read_path.rs` | 11 | 0 |
| `tests/schedule.rs` | 1 | 1 |
| doc-tests | 0 | 0 |
| **Total (15 suites)** | **50** | **4** |

The chapter-06 state was 46 passed, 4 ignored; this chapter adds
exactly the four `selfplayverify_*` tests and nothing else. The
reference's historical gate log ran the same default suite in debug
(19.73 s, 50 passed / 4 ignored); the deduplicated counts are
identical in either mode.

One raw-log caveat belongs with this table [measured]: the reference's
gate log shows **52 passes across 17 result lines**, not 50 across 15,
because chapter 02's cross-process determinism test in
`tests/corpus.rs` re-runs the compiled test binary in two child
processes and each child prints its own `test result:` line. Count per
target (deduplicated) or count raw result lines — never mix the two
methods.

If the workspace-wide gate is wanted, `cargo test --workspace` is the
reference's closing command: **852 passed, 11 ignored across 63 unique
suites**, deduplicated [measured]; the raw log shows 854 passes across
65 result lines — the same corpus child-process caveat, and the two
counting methods are never mixed in one comparison. The workspace gate
also runs `cargo clippy --workspace --all-targets -- -D warnings` and
`cargo fmt --all` green. The last two standing commands above are the
project's per-crate quality gates; both must be green with no changes.

### 6.4 The capstone: verify a corpus you generate

Verify a corpus you generate, extract its anchors, and watch the
boundaries hold — everything read-only against committed data, with
every output inside a temporary directory. From `gomoku/`, in release
(the generation plays real games):

```text
cargo build --release -p selfplay

dir=$(mktemp -d)

target/release/selfplaygen \
  --out "$dir/corpus" \
  --openings ../data/openings/gomocup-2024-freestyle15.txt \
  --black-sims 8 --white-sims 8 --games 3 --seed-base 777

target/release/selfplayverify --corpus "$dir/corpus"

target/release/selfplayanchors \
  --corpus "$dir/corpus" --out "$dir/anchors.json"

cat "$dir/anchors.json"
```

The generation reports its three accepted seeds; the verifier prints
the JSON quality report to stdout. A fresh run of exactly this pair
against the reference [measured 2026-11-14] produced:

```json
{
  "game_count": 3,
  "verified_plies": 114,
  "certified_suffixes": 3,
  "games_ending_within_16_plies": 1,
  "min_plies": 13,
  "max_plies": 83,
  "mean_plies": 38.0,
  "median_plies": 18.0,
  "length_histogram_plies": {
    "13": 1,
    "18": 1,
    "83": 1
  },
  "opening_offers": {
    "../data/openings/gomocup-2024-freestyle15.txt#0": 1,
    "../data/openings/gomocup-2024-freestyle15.txt#5": 1,
    "../data/openings/gomocup-2024-freestyle15.txt#6": 1
  },
  "opening_answers": {
    "take_white": 3
  },
  "outcomes": {
    "black": 1,
    "white": 2,
    "draw": 0
  }
}
```

and the extractor reported `Wrote 3 verified anchor seeds` — one per
game, each line one ply long — with the JSON showing the format end to
end: `game_index` counts the corpus order, `from_ply` counts
post-opening plies, and `line` holds board indices, so `139` is row 9,
column 4 (9·15 + 4). The run is deterministic: the same commands with
the same seed base reproduce these numbers exactly, and a different
seed base can change them. Note the `opening_offers` keys: the
provenance preserves the literal path the generation command was
given — here `../data/openings/gomocup-2024-freestyle15.txt`, because
that is the string the `--openings` flag carried.

Four boundary experiments complete the picture:

1. Verify an empty directory (`mkdir "$dir/empty"`, then point
   `--corpus` at it): the run fails with `corpus contains no game
   records` and exit code **1** — an empty corpus is an error, never
   an empty report.
2. Pass a flag the binary does not know, for example `--json`: the run
   fails with `unknown flag: --json`, prints the one-line usage, and
   exits with code **2** — the report needs no opt-in flag, because
   JSON is all it prints.
3. Re-run the extractor with the same `--out`: the run fails with
   `failed to create '…': File exists (os error 17)` — an extraction
   never overwrites a seed set by accident.
4. Run the verifier twice into two files and compare them: the report
   is byte-identical, because the corpus is fixed input and every map
   in it is sorted.

None of these commands writes outside `$dir`; the verifier never
writes at all.

Two pieces of neighboring evidence stay separate from this capstone,
and the capstone does not re-run them. The historical 48-game
acceptance run [measured 2026-11-11, the reference's
`acceptance-logs/`] verified the first corpus — 48 of 48 games, 1 860
of 1 860 post-opening plies legal, 47 of 47 certificates re-verified
by the anchor pass — and the integration smoke trained the unmodified
`nettrain` for 50 steps on the 1 860 exported samples (train loss
6.5483584 → 6.252371); the chapter paper's gates section quotes both
with their provenance. And the pre-registered grading gate — 50 games
at baseline-800 vs baseline-200, in two color-swapped runs of 25, bar
at least 30 higher-budget wins — remains **registered, not executed**:
executing it is deliberate follow-up work after the inverted 20-game
probe has been explored, and no step of this chapter runs it.

---

## Pitfalls

Each pitfall below is a defect an independent implementation is likely
to produce; each names the test, capstone experiment, or gate that
catches it. Skim this list before starting step 1, and again before
the gates.

1. **A fifth test.** The chapter specifies exactly four
   `selfplayverify_*` tests, all default-run, none MANUAL. Resist
   pinning extras — no help-text test (there is no `--help`), no
   parse-failure exit-code test (the capstone's boundary experiments
   demonstrate the codes), no unit tests on the binary.
2. **Growing the interface.** No `--help`, no `--json`, no
   `--anchors`, no `--limit`. Any flag beyond `--corpus` is scope
   creep the design rejected deliberately; the capstone's `--json`
   experiment pins the rejection (`unknown flag: --json`, exit 2).
3. **Reading the manifest.** `read_corpus` is the binary's only input
   path; `read_manifest` exists and is never called. A
   manifest-versus-shard cross-check is implemented nowhere in the
   tutorial — it is an open hardening gap recorded in chapters 02 and
   07, not a check to add quietly here.
4. **Re-verifying certificates in the verifier.** `certified_suffixes`
   is a presence count. `verify_line` belongs to `selfplayanchors`;
   the legality gate composes the two binaries, it does not merge
   them.
5. **A partial report.** Failure is all-or-nothing: the first failing
   game exits before the summary exists. A summary printed alongside
   a failure — or a "failed games" counter — breaks the contract that
   a green exit always means the same thing.
6. **The wrong side to move in the resignation rule.** The rule pins
   `board.to_move() == winner.other()` on a still-`Ongoing` board —
   the *loser's* turn pending. Taking the side from the winner, or
   requiring a terminal status, inverts the check.
7. **Counting the opening placements.** Lengths are post-opening
   lengths; the Swap2 placements are replayed but counted in neither
   `verified_plies` nor the histogram. The fixture's six plies pin the
   convention (`verified_plies` is 6, not 9).
8. **A `HashMap` in the report.** Key order would vary run to run and
   the capstone's byte-identity experiment would fail. Every map is a
   `BTreeMap`.
9. **Normalizing the opening-offer keys.** The keys are the literal
   recorded path strings (`<list>#<index>`), not canonicalized paths;
   the capstone's `../data/...#0` keys demonstrate the convention.
10. **Numeric histogram keys in the assertions.** JSON object keys are
    strings; the test indexes `length_histogram_plies` with `"6"`.
11. **An integer median.** The median of an even count is the mean of
    the two middle values in `f64`, so it can end in `.5`.
12. **Deriving more than `Serialize`.** The report types need no
    `Deserialize` and no `Debug`; stdout is the only rendering.
13. **New imports in `tests/binary.rs`.** The target already imports
    everything the four tests need — `Command`, `serde_json`,
    `std::fs`, `Color`, `Outcome`, `WinReason`, `write_corpus`, and
    the helpers. A missing name means the test drifted from the
    existing vocabulary, not that the import list is short.
14. **Hardening `TestDir` silently.** One flake was observed while
    verifying this chapter [measured 2026-11-14]: in one of several
    repeated runs of `cargo test -p selfplay --test binary` against
    the reference, two tests failed together — one with `File exists`
    while writing its corpus, another with `No such file or directory`
    while reading its corpus back — and every re-run was green. The
    failures were not instrumented, so the mechanism is not
    established fact; the explanation consistent with both errors
    lives in the fixture, not the verifier: `TestDir` names its
    directory `selfplayanchors-test-<pid>-<nanos>`, parallel tests
    share the process id, and two tests drawing the same clock
    nanoseconds would share one directory, so one test's `Drop` would
    delete the other's directory mid-run. No fix has landed, and this
    chapter does not land one: the helpers stay as the source has
    them. If the flake ever bites during your gate, the hardening is a
    per-test label in the directory name — the shape
    `tests/corpus.rs`'s labeled `TestDir` already shows — a fixture
    change, never a verifier change, and never a silent one.
15. **Mixing the counting methods.** Deduplicated per-target counts
    (50/4 across 15 suites) and raw result-line counts (52 across 17,
    because of the corpus target's two child processes) are both true;
    comparing one run's raw count against another run's deduplicated
    count manufactures a discrepancy out of a counting convention.
16. **Running the MANUAL set.** The four ignored tests are exactly
    chapter 04's; they execute only by explicit selection
    (`-- --ignored`) in release, as deliberate measurements. The
    default gate never runs them, and this chapter's done-when quotes
    them as ignored, not passed.

---

## Done when

- `cargo test -p selfplay --release` reports **50 passed, 4 ignored**
  as the sum of its per-target reports: the chapter-06 targets keep
  their state (the `src` unit tests 6/0 — two in `game.rs`, four in
  `selfplaygen.rs`; corpus 10/0; openings 5/0; schedule 1/1; dedup
  1/0; grading 0/1; anchors 4/0; read_path 11/0), and this chapter
  brings `tests/binary.rs` to **7/0** with exactly the four
  `selfplayverify_*` tests. The deduplicated per-target count is the
  contract; a raw log will show two extra corpus child-process result
  lines (52 across 17), and the two counting methods are never mixed.
- `cargo test -p selfplay --test binary selfplayverify` selects and
  passes exactly this chapter's four tests.
- `tests/binary.rs`'s three earlier tests and its `TestDir`, `mv`, and
  `certified_record` helpers are unchanged, and the new binary target
  contributes zero unit tests — the `src` unit tests stay at 6.
- Exactly two files changed: `src/bin/selfplayverify.rs` created,
  `tests/binary.rs` extended. `src/lib.rs`, every library module,
  `Cargo.toml`, and every other crate are untouched.
- No new MANUAL test exists, and nothing in this chapter's tests plays
  a game: every test fabricates its records and drives the compiled
  binary (about 0.2 s in debug for the whole target [measured]). The
  four ignored tests are exactly chapter 04's set, still ignored.
- `cargo test -p train`, `cargo test -p player`, and `cargo test -p
  net` are not required: all three crates are untouched.
- `cargo clippy --all-targets -- -D warnings` is green.
- `cargo fmt --all -- --check` makes no changes.
- The capstone ran end to end inside a `mktemp` directory: the
  three-game corpus generated, the verifier's JSON report matched the
  measured reference run (114 verified plies, 3 certified suffixes,
  the literal `../data/...` offer keys), the extractor wrote 3
  verified anchor seeds, and the four boundary experiments behaved as
  specified — empty directory refused with exit 1, `--json` rejected
  with exit 2, the anchor re-run refused with `File exists`, and two
  verifier runs byte-identical.
- The chapter paper's acceptance gates stand as documented: gates 1
  (determinism), 2 (legality), 4 (corpus), and 5 (integration) carry
  the reference's measured evidence with its provenance, and gate 3
  (grading) remains pre-registered and **not executed** — this chapter
  does not run it.
- The commit message is
  `feat(selfplay): acceptance — selfplay-corpora chapter 07`.

## Verification commands

From `gomoku/`:

```text
cargo test -p selfplay --release
cargo test -p selfplay --test binary
cargo test -p selfplay --test binary selfplayverify
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the final default gate of the track, in release
per chapter 04's game-playing discipline; the reference's historical
gate log ran the same default suite in debug with identical
deduplicated counts (50 passed / 4 ignored). The second command runs
the seven binary tests in isolation; the third selects this chapter's
four by name. If the workspace-wide gate is wanted, `cargo test
--workspace` is the reference's closing command (852 passed / 11
ignored across 63 unique suites, deduplicated; the raw log shows 854
across 65 result lines — same corpus child-process caveat, never mix
the methods). The capstone above is the fresh end-to-end evidence:
generate, verify, extract, all inside a temporary directory.

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `selfplay/tests/binary.rs` | four `selfplayverify_*` tests | the exact names, fixtures, and assertions of §1.2 — teaching order: empty corpus, illegal move, outcome disagreement, JSON report; the three record-based tests share `certified_record()`; the empty-corpus test creates no record; all default-run; no fifth test; no new imports |
| `selfplay/tests/binary.rs` | existing content | the three chapter-05 tests and the `TestDir`, `mv`, `certified_record` helpers byte-unchanged |
| `selfplay/src/bin/selfplayverify.rs` | `parse_corpus` | one flag `--corpus <DIR>`; `unknown flag: <FLAG>`, `--corpus requires a value`, `--corpus is required`; no `--help`/`--json`/`--anchors`/`--limit`; hand-rolled, no dependency |
| `selfplay/src/bin/selfplayverify.rs` | `verify_record` | opening replay through `Opening::board`; per-ply `board.play` with corpus-order `game_index` and zero-based post-opening `ply` in the errors; the three outcome rules, resignation pinning an `Ongoing` board with the loser to move; all-or-nothing, no partial report |
| `selfplay/src/bin/selfplayverify.rs` | `Summary`, `OutcomeCounts` | the exact twelve-field and three-field sets, in report order; `Serialize` only; every map a `BTreeMap`; literal `<list>#<index>` offer keys; the four answer-key strings; even-count median in `f64`; suffixes counted, never re-verified |
| `selfplay/src/bin/selfplayverify.rs` | `main` | parse → read → empty check → verify loop → pretty JSON to stdout; exit 0/1/2 per the contract table; the manifest never opened; no `#[cfg(test)]` module |
| untouched | `src/lib.rs`, `Cargo.toml`, all library modules, all other test targets, `engine` / `train` / `player` / `net` | no exports, no dependencies, no module, no unit tests; `git status` names exactly two files |

## References

**This chapter:**

- [acceptance.md](acceptance.md) — the chapter paper: intention,
  specification as tests, the execution-order walkthrough, the five
  gates with their measured evidence, the open observations, and the
  capstone.
- [acceptance-design.md](acceptance-design.md) — the conceptual story:
  the records as evidence and the engine as adjudicator, the
  count-versus-re-verify split, the manifest gap, and the determinism
  of the report.
- [acceptance-solution.md](acceptance-solution.md) — the
  annotated reference snippets. Open only after twenty stuck minutes,
  or afterwards to compare.

**This track:**

- [Tutorial README](../README.md) — conventions, gates, the chapter
  map, and the reference provenance.
- [Self-play primer](../00-selfplay-primer.md) — §6 the five
  acceptance gates with the measured headline numbers; §7 the open
  questions; §8 the invariants this chapter's binary enforces.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md) —
  the `GameRecord` format, the shard-only `read_corpus`, the separate
  `read_manifest`, the corrected honesty boundary with the open
  manifest cross-check, and the cross-process determinism test whose
  child processes explain the two counting methods.
- [Chapter 04 — diversity and grading](../04-diversity-and-grading/diversity-and-grading.md) —
  `selfplaygen`, the release-mode and MANUAL-test disciplines, and the
  grading probe whose inversion keeps gate 3 registered-but-unexecuted.
- [Chapter 05 — certified endgames](../05-certified-endgames/certified-endgames.md) —
  the certified suffix, `verify_line`, `extract_anchor_seeds`, the
  `selfplayanchors` pass the legality gate runs beside the verifier,
  and the `TestDir` / `mv` / `certified_record` helpers this chapter
  reuses.
- [Chapter 06 — reading back](../06-reading-back/reading-back.md) and
  its [implementation plan](../06-reading-back/reading-back-implementation.md) —
  the π/z contracts the verifier does not re-check, the export seam
  that exercises the π contract at scale, and the compile-time binary
  variable mechanism the red scaffold relies on.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  §5 the five acceptance gates.
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  the measured acceptance evidence the gates quote and the open
  questions carried honestly.
- [docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) —
  the document head's claim-label convention; §12 the determinism
  policy.
- [docs/14-openings-and-supervised-curriculum.md](../../../14-openings-and-supervised-curriculum.md) —
  §3 the Swap2 procedure and the tournament opening lists whose
  literal paths key the report's `opening_offers` map.

**Prerequisite tracks:**

- [Engine tutorial](../../13-engine-tutorial/README.md) — the
  bitboard `Board`, `Status`, `PlayError`, and `verify_line`: the
  adjudicator every check in this chapter trusts.
- [Datagen tutorial](../../datagen-tutorial/README.md) — the shard
  and manifest storage discipline the corpora inherit.
- [Net tutorial](../../net-tutorial/README.md) — `nettrain`, the
  integration gate's consumer, and the acceptance-chapter precedent
  for measured-evidence discipline.

**Source (live on `main`, consumed unchanged):**

- [`gomoku/crates/engine/src/board.rs`](../../../../gomoku/crates/engine/src/board.rs) —
  `Board::play`, `Board::status`, `Board::to_move`, `Status`,
  `PlayError`: the adjudicator's surface. Not touched in this chapter.

**Source (lands on `main` as the track is worked; until then it
lives only in the read-only reference worktree):** chapter 01's
`src/game.rs` and `src/opening.rs` have landed on `main` with the
learner-worked game runner; the chapter 02–07 files below, including
this chapter's binary, land as those chapters are worked.

- `gomoku/crates/selfplay/src/bin/selfplayverify.rs` — new: the
  verifier binary (`main`, `Summary`, `OutcomeCounts`,
  `Summary::from_records`, `parse_corpus`, `verify_record`). Created
  in this chapter.
- `gomoku/crates/selfplay/tests/binary.rs` — the four
  `selfplayverify_*` specification tests beside the three existing
  binary tests and the shared helpers. Touched in this chapter.
- `gomoku/crates/selfplay/src/record.rs`,
  `gomoku/crates/selfplay/src/opening.rs`,
  `gomoku/crates/selfplay/src/game.rs`,
  `gomoku/crates/selfplay/src/corpus.rs` — the consumed vocabulary:
  `GameRecord` / `GameHeader` / `RecordedPly` / `CertifiedSuffix`,
  `Opening` / `OpeningProvenance` / `OpeningAnswer` / `Opening::board`,
  `Outcome` / `WinReason`, `read_corpus` / `read_manifest`,
  `GameRecord::from_game`. Not touched in this chapter.
- `gomoku/crates/selfplay/src/anchor.rs` and
  `gomoku/crates/selfplay/src/bin/selfplayanchors.rs` — the
  certificate pass the legality gate runs beside the verifier. Not
  touched in this chapter.
- `gomoku/crates/selfplay/src/export.rs` and
  `gomoku/crates/selfplay/src/bin/selfplayexport.rs` — the export pass
  whose validate-every-position discipline exercises the π contract at
  corpus scale. Not touched in this chapter.
- `gomoku/crates/selfplay/tests/corpus.rs` — the cross-process
  determinism test with its two child-process result lines, and the
  labeled `TestDir` the flake observation contrasts with. Not touched
  in this chapter.
- `gomoku/crates/selfplay/tests/grading.rs` — the pinned grading
  probe, one of the four ignored MANUAL tests. Not touched in this
  chapter.
- `gomoku/crates/train/src/storage.rs` — the shared shard mechanics
  `read_corpus` decodes through. Chapter 02's extraction; not touched
  in this chapter.

**Scratch measurement provenance (temporary, deleted with the
worktree):**

- `acceptance-evidence.md` and `acceptance-logs/` in the reference
  worktree (`rust-ml-selfplay-ref`, branch `selfplay-ref-v2`) — the
  raw measurement record behind every [measured] label in this plan;
  the load-bearing numbers are preserved in the hand-off and quoted in
  the chapter paper.

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). <https://doi.org/10.1038/nature24270> —
  the (s, π, z) record format whose stored-data contract this chapter
  verifies.
- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  <https://doi.org/10.26481/dis.19940923la> — threat-space search, the
  prover whose certificates the legality gate re-verifies beside this
  chapter's verifier.
