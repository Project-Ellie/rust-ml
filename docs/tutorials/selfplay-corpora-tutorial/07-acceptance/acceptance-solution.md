> **Opt-in solution — read it only after attempting the chapter with the
> [implementation plan](acceptance-implementation.md), or after being
> stuck for more than twenty minutes.**
>
> This paper is inspiration, not authority over the learner's own code. The
> learner's implementation on `main` is the reference; the artifacts below
> exist so a stuck or finished learner can compare. No learner source is
> changed by this paper.

# Chapter 07 — Acceptance: solution

## Abstract

This paper is the annotated reference companion to
[chapter 07 of the selfplay-corpora tutorial](acceptance.md), the final
chapter of the track. It quotes, in full and along the execution path of
one verification run, the only two artifacts the chapter creates: the new
binary
[`src/bin/selfplayverify.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayverify.rs)
(188 lines — the one-flag argument parser, the corpus read and empty-corpus
rejection, the per-game replay through `engine::Board`, the three-rule
outcome adjudication including the resignation rule, and the deterministic
JSON quality report) and the final form of
[`tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs)
(232 lines — the three chapter-05 binary tests unchanged, plus the four
`selfplayverify_*` specification tests). Both files are byte-identical to
the completed reference implementation; `src/lib.rs`, `Cargo.toml`, and
every other crate file are unchanged from the verified chapter-6 slice, and
because this is the final chapter, the verified slice *is* the completed
reference crate — a scripted whole-crate comparison closes with zero
differences, so no slice-versus-completed ambiguity exists and no
later-chapter code is anywhere in the picture. The paper closes with the
verification evidence, separated into what this session measured fresh on
the slice — the focused binary target at 7 passed (after one honest
collision with the known `TestDir` race, rerun green without touching
source), the debug and release default gates at 50 passed / 4 ignored
across 15 suites (52 across 17 raw result lines), the workspace gate at 852
passed / 11 ignored across 63 unique suites (854 across 65 raw lines, after
two further race collisions and a final green rerun), clippy and rustfmt
clean, a fresh determinism gate whose shard and manifest SHA-256 hashes
reproduce the reference's 2026-11-11 values exactly, a read-only
re-verification of the reference's 48-game corpus whose JSON report is
byte-identical to the historical one (48 games, 1 860 plies, 47 certified
suffixes) and whose anchor extraction wrote 47 verified seeds, and the full
capstone with its four boundary experiments — and what is quoted as
historical, attributed, and not re-run: the throughput figure, the export
and `nettrain` smoke, and the grading observations. The grading gate
itself remains as the chapter leaves it: pre-registered, not executed, in
this session as in the reference. The intended reader is the learner who
has read the chapter paper and, ideally, worked the
[implementation plan](acceptance-implementation.md) first.

## Glossary

| Term | Definition |
|------|------------|
| **Game record** | `GameRecord`: the durable serialization of one complete game — a header (seed, seat pairing, evaluator identity, the Swap2 opening), the ordered post-opening `plies`, the `outcome`, and an optional certified suffix. Chapter 02 fixed the format. |
| **Corpus** | A set of game records produced at one declared pairing, stored as shards plus a manifest. |
| **Shard** | A fixed-capacity file of length-delimited bincode records (`shard-*.bin`); the storage unit `read_corpus` lists, sorts, and decodes. |
| **Manifest** | The JSON file next to the shards: seed range, per-run configuration, game counts, and the pairing. Provenance, not data — and the verifier never opens it. |
| **Replay** | Rebuilding a position by playing a record's opening and its moves through a fresh `engine::Board`. The engine is the only position representation the verification path trusts. |
| **Outcome / Status** | `Outcome` is the record's terminal claim (`Won { winner, reason }` with `WinReason::FiveOrMore` or `WinReason::Resignation(..)`, or `Draw`); `engine::Status` is the replayed board's own terminal state (`Ongoing`, `Won(Color)`, `Draw`). The verifier adjudicates the claim against the board. |
| **Swap2** | The opening protocol of this project: one side places three stones, the other chooses a color or adds two stones, and the first side then chooses a color. The placements are the ceremony, not plies. |
| **Opening offer / opening answer** | The three-stone opening one side places (`OpeningProvenance::Generated` or `Listed { list, index }`), and the other side's reply (`OpeningAnswer::Take` a color, or `AddTwo` with a final color choice). The report counts both. |
| **Certified suffix** | `CertifiedSuffix { from_ply, line }`: a terminal segment of a game in which every attacker move belongs to a machine-verified forced-win line produced by the engine's threat-space search (TSS) prover. The verifier counts these; re-verification is `selfplayanchors`' pass. |
| **Anchor seed** | `AnchorSeed { game_index, from_ply, line }`: the extraction of a certified suffix; the input format of the milestone-4 puzzle anchor set. |
| **verify_line** | `engine::verify_line(board, proof) -> bool`: the soundness net that re-verifies a certified line against every defender reply. Chapter 05 owns this gate; this chapter's binary never calls it. |
| **π / z** | The policy and value training targets, computed from the recorded visit counts and the recorded outcome at read time. Chapter 06 owns both contracts; the verifier inspects neither. |
| **Acceptance gate** | One of the five repeatable checks of the design spec §5 — determinism, legality, grading, corpus, integration — that together decide whether the tutorial's corpus pipeline is accepted. |
| **MANUAL test** | A slow game-playing test marked `#[ignore]` with a MANUAL comment, run by explicit selection in release mode, never by the default gate. This chapter adds none and runs none. |
| **Deduplicated vs raw counts** | Cargo prints one `test result` line per test-target process; the corpus target's cross-process determinism test re-runs its own binary in two child processes whose result lines appear in the parent log. Deduplicated counts sum the unique suites; raw counts sum every printed line. The two methods are never mixed. |
| **Final slice** | The chapter-7 subset of the completed reference: chapters 01–07 together. Because chapter 07 is the final chapter, the final slice equals the completed reference crate, verified by scripted whole-crate comparison. |
| **Claim labels** | The project's provenance labels: **[experiment]**, **[derived]**, **[invention]**, and **[measured]** (a number recorded in the reference's acceptance logs or in this paper's own fresh runs, always attributed). |
| **TDD** | Test-driven development: write the test, watch it fail (red), implement, watch it pass (green). |

## Context

Seven sources supply what this paper combines. Each summary states what
the source delivered and why this paper needs it.

- [Chapter 07 — Acceptance](acceptance.md) — the chapter paper: the
  specification as tests, the execution-order walkthrough, the five
  acceptance gates with their measured evidence, and the open
  observations. This paper quotes the artifacts the chapter specifies and
  reports the gates it ran.
- [acceptance-design.md](acceptance-design.md) — the conceptual story:
  acceptance proves the stored data contract, not the player; one flag,
  unconditional JSON, and why the verifier reads the shards and never the
  manifest.
- [acceptance-implementation.md](acceptance-implementation.md) — the hiding
  layer: the six-step red-to-green construction plan whose step boundaries
  this paper's section order follows.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md)
  and its [solution](../02-recording-games/recording-games-solution.md) —
  the durable format the verifier consumes: `GameRecord`, `read_corpus`,
  `write_corpus` (the tests' corpus writer), and the cross-process
  determinism test whose child processes explain the raw-line counting
  caveat.
- [Chapter 05 — certified endgames](../05-certified-endgames/certified-endgames.md)
  and its
  [solution](../05-certified-endgames/certified-endgames-solution.md) —
  the `TestDir`, `mv`, and `certified_record` fixture helpers the four new
  tests reuse unchanged, and the `selfplayanchors` binary that owns
  certificate re-verification.
- [Chapter 06 — reading back](../06-reading-back/reading-back.md) and its
  [solution](../06-reading-back/reading-back-solution.md) — the chapter-6
  slice this paper's slice was copied from, and the provenance discipline
  this paper continues.
- The completed reference implementation in the read-only worktree
  `rust-ml-selfplay-ref` (branch `selfplay-ref-v2`) — the quotation source.
  Its `acceptance-evidence.md` and `acceptance-logs/` are the raw
  measurement record behind every historical number quoted here.

The [design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) §5
fixes the five acceptance gates; the
[hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md)
records the reference's measured headline numbers.

## Intention

After this paper you can explain the finished verifier end to end: how one
required flag becomes a corpus path, how `read_corpus` turns shards into
records, why an empty corpus is an error rather than an empty report, how
every opening and every recorded move is replayed through a real
`engine::Board`, how the three outcome shapes are adjudicated against the
replayed board — including the resignation rule, whose terminal board is
still open with the loser to move — and how the summary's every field is
derived from the records themselves, never from the manifest. You can read
the four specification tests and say exactly which three share the
`certified_record` fixture and which one fabricates nothing but a
directory. And you can reproduce the evidence: the focused and full gates
with their deduplicated counts, the fresh determinism run whose hashes
match the reference's 2026-11-11 values, the read-only re-verification of
the 48-game corpus, and the capstone with its four boundary experiments.
What this paper deliberately does not give you: a strength claim of any
kind, a manifest-versus-shard cross-check (an open hardening gap, recorded
in chapter 02 and the chapter paper, implemented nowhere), and an execution
of the pre-registered grading gate.

## Provenance and the final slice

### The reference and the completed tree

The reference implementation lives in a separate, read-only worktree beside
this repository (`rust-ml-selfplay-ref`, branch `selfplay-ref-v2`,
committed base `9028aa8` on `main`, the selfplay crate uncommitted). It was
built test-first and gate-verified before this tutorial was written. The
worktree serves quotation only: never point a manifest at it, never build
the learner's crate from it, never commit there. Chapter 01's
[solution paper](../01-the-game-runner/the-game-runner-solution.md)
established the provenance discipline this paper continues; chapter 06's
[solution paper](../06-reading-back/reading-back-solution.md) applied it to
the chapter-6 slice this chapter's slice was copied from.

Chapter 07 is the final chapter, and that simplifies the provenance story
that earlier chapters had to tell carefully. There is no later chapter
whose code could leak into the slice: the tutorial's chapter map ends at
this chapter, and the completed reference's `selfplay` crate contains
exactly the chapters 01–06 artifacts plus this chapter's two files — a
scripted whole-crate recursive comparison between the verified slice and
the reference (`diff -rq` over
`gomoku/crates/selfplay/`) reported no differences. What this chapter adds
is therefore not a *form* of a file but the whole story:

| File | Form | Evidence |
|------|------|----------|
| `src/bin/selfplayverify.rs` (188 lines) | **created** in this chapter, reference-verbatim | whole-file byte comparison: IDENTICAL; SHA-256 `4e468bf572c887b31b494214e542a9751bc962b458408a02e023a823383d66dc` in both trees |
| `tests/binary.rs` (232 lines) | **grows** from the chapter-6 form (142 lines, 3 tests) to the final form (7 tests), reference-verbatim | whole-file byte comparison: IDENTICAL; SHA-256 `588707e9ecba1301701aa44dfe6f391eddd26d40e5cd8f48564df7d4fdd6f826` in both trees; the chapter-6 form differs from the final form by exactly the appended region of reference lines 143–232 (a blank separator plus the four `selfplayverify_*` tests, whose code spans lines 144–232) |
| the 24 unchanged crate files (the twelve src-root modules, the three earlier binaries, the eight other test targets, and `Cargo.toml`) | unchanged | scripted whole-file byte comparisons against the chapter-6 slice: 24 `UNCHANGED` verdicts in total — one per file — logged in `/tmp/selfplay-ch7-verify-logs/provenance-diff.log` |

The comparisons were scripted, not eyeballed: whole-file byte comparisons
(`cmp`), plus the recursive whole-crate comparison for the
completed-tree claim. The chapter adds a binary, not a module, so
`src/lib.rs` is among the untouched twelve, and `Cargo.toml` needs no
manifest entry for a `src/bin/` binary — the crate's existing
`serde`/`serde_json` dependencies cover the report. Because the slice
and the reference are byte-identical across the whole crate, the quoted
line numbers below are simultaneously the slice's, the reference's, and
the numbers the learner's files will have once the chapter's two
artifacts land exactly as specified. No file outside
`gomoku/crates/selfplay/` is part of this chapter, and none was
touched: the reference worktree's other modified crates (`player`,
`train`) predate this chapter — they are earlier tracks' uncommitted
reference material — and neither they nor the learner's tree were read
or written by this chapter's verification beyond the read-only corpus
inputs named in the Verification section.

### The slice was built and run

Quoting a slice the learner could not compile would violate the track's
first rule — a tutorial that does not run error-free is a defect. So the
final slice was constructed and gate-verified on 2026-11-14 at
`/tmp/selfplay-ch7-verify` (a scratch path, not a repository artifact). The
starting point was not the reference worktree but an *untouched copy* of
the already-verified chapter-6 slice at `/tmp/selfplay-ch6-verify` (copied
in full, build directory included, so the verification runs are
incremental). No `/tmp/selfplay-ch7-verify` existed beforehand, so the copy
needed no `-2` suffix and nothing was overwritten; neither the chapter-6
scratch nor the reference worktree was modified. Onto that copy exactly
two files were placed, both copied from the read-only reference:
`src/bin/selfplayverify.rs` (new) and `tests/binary.rs` (final form).
Nothing else was touched — no library module, no manifest, no other test
target — and the scripted comparisons above prove it.

The chapter's verification commands were run from the slice's `gomoku/`
(with the corpus-level commands run from the slice root where the recorded
path spellings require it), with stdout and stderr saved to
`/tmp/selfplay-ch7-verify-logs/` (a scratch path, not a repository
artifact):

```text
cargo test -p selfplay --test binary
cargo test -p selfplay --test binary selfplayverify
cargo test -p selfplay
cargo test -p selfplay --release
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build --release -p selfplay && <the determinism gate, the 48-game
corpus re-verification, and the capstone below>
```

All gates closed green on the slice; the measured evidence — per-target
counts, the three `TestDir` race collisions and their green reruns, the
determinism hashes, the 48-game re-verification, and the capstone with its
boundary experiments — is tabled in [§9](#9-verification). No ignored test
was run: the four MANUAL tests of chapter 04 execute only by explicit
selection, and this session selected none. The pre-registered grading gate
was likewise not executed — it is the chapter's registered follow-up work,
not this paper's. The slice fragments quoted below are exactly the files
those runs exercised.

### Reading rules

The section order is the execution order of one verification run from
argument to exit status, matching the chapter paper's walkthrough: the
entry point and the run shape first (section 1), then the argument parser
(section 2), then the per-game replay and outcome adjudication (section 3),
then the report types (section 4) and the report derivation (section 5),
then the test file — the shared fixture (section 6), the three earlier
tests (section 7), and the four new specification tests (section 8) — and
finally the verification (section 9), the done-when (section 10), and the
troubleshooting (section 11). Within the binary the order is the execution
path, not the file's declaration order; the test file is quoted in the
file's own order.

Every code fragment carries a right-aligned `NNN |` line-number margin. The
numbers are the file's own line numbers — simultaneously the slice's, the
completed reference's, and the learner's once the chapter lands. The margin
is the only annotation added; the code text itself is quoted exactly, and
each fragment's provenance note states its byte-identity evidence. Blank
separators between lines appear without a margin; no fragment skips a
nonblank line inside its quoted range, and both files this chapter touches
are quoted in full across their fragments (`selfplayverify.rs` lines 1–188
across sections 1–5; `tests/binary.rs` lines 1–232 across sections 6–8).

File links in this paper point into the learner's tree on `main`. The
chapter-01 links
([`src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs),
[`src/opening.rs`](../../../../gomoku/crates/selfplay/src/opening.rs)) and
the engine link resolve today; links to files the later chapters create —
for example
[`gomoku/crates/selfplay/src/bin/selfplayverify.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayverify.rs),
[`src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs),
[`src/corpus.rs`](../../../../gomoku/crates/selfplay/src/corpus.rs), and
[`tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs) —
resolve once chapters 01–07 land. The reference worktree is not linked: it
is temporary provenance, deleted once the tutorial is complete on `main`,
and everything needed from it is quoted here. The scratch copies and their
logs (`/tmp/selfplay-ch6-verify`, `/tmp/selfplay-ch7-verify`,
`/tmp/selfplay-ch7-verify-logs/`) are cited as evidence of this paper's own
verification; they are ephemeral and not part of the repository.

## 1. `src/bin/selfplayverify.rs`: the entry point and the run shape

The chapter's only new source file, and the track's thinnest binary: one
required flag, no `--help` special case, no options of any other kind. The
module comment states the whole contract in one line. The imports state the
consumed surface: `BTreeMap` (every map in the report is sorted, which is
what makes the report byte-deterministic), `engine::{Color, Status}` (the
adjudicator's vocabulary), the `selfplay` crate's record vocabulary and
`read_corpus`, and `serde::Serialize` for the report.

`main` is the five-stage pipeline the chapter's walkthrough describes, and
its exit-code discipline is the binary's contract: **2** for command-line
misuse (the parse error and the one-line usage go to stderr), **1** for a
corpus failure (read error, empty corpus, or the first replay or outcome
failure, each with a named error on stderr), **0** for a verified corpus
with the JSON report on stdout. Note the all-or-nothing shape: the per-game
loop stops at the first failing record, so a corrupt corpus never produces
even a partial report; and the summary is computed only after every record
verified, which is what entitles `verified_plies` to its name.

**Source:** [`gomoku/crates/selfplay/src/bin/selfplayverify.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayverify.rs),
lines 1–42 (the module comment, imports, `main`, and the trailing separator). *Created in this
chapter; byte-identical to the completed reference (SHA-256
`4e468bf5…66dc`).*

```rust
   1 | //! Replay self-play records and reject illegal moves or inconsistent outcomes.

   3 | use std::collections::BTreeMap;
   4 | use std::path::PathBuf;
   5 | use std::{env, process};

   7 | use engine::{Color, Status};
   8 | use selfplay::{GameRecord, OpeningAnswer, OpeningProvenance, Outcome, WinReason, read_corpus};
   9 | use serde::Serialize;

  11 | fn main() {
  12 |     let corpus = match parse_corpus(env::args().skip(1)) {
  13 |         Ok(corpus) => corpus,
  14 |         Err(message) => {
  15 |             eprintln!("{message}\nUsage: selfplayverify --corpus <DIR>");
  16 |             process::exit(2);
  17 |         }
  18 |     };
  19 |     let records = read_corpus(&corpus).unwrap_or_else(|error| {
  20 |         eprintln!("selfplayverify failed: could not read corpus: {error}");
  21 |         process::exit(1);
  22 |     });
  23 |     if records.is_empty() {
  24 |         eprintln!("selfplayverify failed: corpus contains no game records");
  25 |         process::exit(1);
  26 |     }
  27 |     for (game_index, record) in records.iter().enumerate() {
  28 |         if let Err(error) = verify_record(game_index, record) {
  29 |             eprintln!("selfplayverify failed: {error}");
  30 |             process::exit(1);
  31 |         }
  32 |     }
  33 |     let summary = Summary::from_records(&records);
  34 |     match serde_json::to_string_pretty(&summary) {
  35 |         Ok(json) => println!("{json}"),
  36 |         Err(error) => {
  37 |             eprintln!("selfplayverify failed: could not serialize summary: {error}");
  38 |             process::exit(1);
  39 |         }
  40 |     }
  41 | }

```

## 2. `parse_corpus`: one flag, three errors

The parser accepts exactly `--corpus <DIR>` as two separate arguments and
nothing else. Its three failures are distinct messages: an unrecognized
argument is `unknown flag: <FLAG>` (the capstone demonstrates this with
`--json`, exit code 2 — the report needs no opt-in flag because JSON is all
the binary prints), a trailing `--corpus` without a value is `--corpus
requires a value`, and no arguments at all is `--corpus is required`. Every
parse failure lands in `main`'s first branch: message, usage line, exit 2.
Hand-rolled over `env::args().skip(1)`, no dependency, in the style of the
earlier binaries — and deliberately thinner than they are: no `--help`, no
`--limit` (limiting work is a generation-time decision, `--games`), no
`--anchors` (certificates are `selfplayanchors`' pass).

**Source:** `src/bin/selfplayverify.rs`, lines 139–154. *Created in this
chapter; byte-identical to the completed reference.*

```rust
 139 | fn parse_corpus(mut args: impl Iterator<Item = String>) -> Result<PathBuf, String> {
 140 |     let mut corpus = None;
 141 |     while let Some(arg) = args.next() {
 142 |         match arg.as_str() {
 143 |             "--corpus" => {
 144 |                 corpus = Some(PathBuf::from(
 145 |                     args.next()
 146 |                         .ok_or_else(|| "--corpus requires a value".to_owned())?,
 147 |                 ));
 148 |             }
 149 |             other => return Err(format!("unknown flag: {other}")),
 150 |         }
 151 |     }
 152 |     corpus.ok_or_else(|| "--corpus is required".to_owned())
 153 | }

```

## 3. `verify_record`: replay, then adjudication

The per-game check rebuilds each record from primary evidence and then
adjudicates its outcome claim. Two stages:

**Replay.** `record.header.opening.board()` replays the Swap2 ceremony into
a fresh `engine::Board`; a rejection is `game <N>: opening replay failed:
…`. Then every recorded move is played in order with `board.play`, and the
first rejection stops the whole run: `game <N>: move <MOVE> at ply <PLY>
was rejected: …`, where the trailing text is the engine's own `PlayError`
message (the specification test pins `Cell is already occupied.`).
`game_index` is the record's position in corpus order — the same indexing
chapter 05's `AnchorSeed`s use — and `ply` counts post-opening plies.

**Adjudication.** After the last ply, the board's own `status()` decides
whether the recorded `outcome` is honest. Three rules, one per shape:

- `Won { winner, reason: FiveOrMore }` requires `Status::Won(winner)` — the
  engine must see the same five-or-more.
- `Won { winner, reason: Resignation(..) }` requires `Status::Ongoing`
  *and* `board.to_move() == winner.other()` — a resignation truncates the
  game with the loser to move, so the replayed board must still be open
  with the losing side's turn pending. The resignation reason itself is a
  decision record, not a board state, and is not replayed.
- `Draw` requires `Status::Draw` — the board full, no winner.

Any disagreement is `game <N>: final status <STATUS> does not match
recorded outcome <OUTCOME>`. This is the check the whole format depends on:
z, the value target, is derived from the recorded outcome at read time, so
an outcome the engine contradicts would poison every value target derived
from it.

**Source:** `src/bin/selfplayverify.rs`, lines 155–188. *Created in this
chapter; byte-identical to the completed reference.*

```rust
 155 | fn verify_record(game_index: usize, record: &GameRecord) -> Result<(), String> {
 156 |     let mut board = record
 157 |         .header
 158 |         .opening
 159 |         .board()
 160 |         .map_err(|error| format!("game {game_index}: opening replay failed: {error}"))?;
 161 |     for (ply, recorded) in record.plies.iter().enumerate() {
 162 |         board.play(recorded.mv).map_err(|error| {
 163 |             format!(
 164 |                 "game {game_index}: move {:?} at ply {ply} was rejected: {error}",
 165 |                 recorded.mv
 166 |             )
 167 |         })?;
 168 |     }
 169 |     let status = board.status();
 170 |     let matches = match record.outcome {
 171 |         Outcome::Won {
 172 |             winner,
 173 |             reason: WinReason::FiveOrMore,
 174 |         } => status == Status::Won(winner),
 175 |         Outcome::Won {
 176 |             winner,
 177 |             reason: WinReason::Resignation(_),
 178 |         } => status == Status::Ongoing && board.to_move() == winner.other(),
 179 |         Outcome::Draw => status == Status::Draw,
 180 |     };
 181 |     if !matches {
 182 |         return Err(format!(
 183 |             "game {game_index}: final status {status:?} does not match recorded outcome {:?}",
 184 |             record.outcome
 185 |         ));
 186 |     }
 187 |     Ok(())
 188 | }
```

## 4. `Summary` and `OutcomeCounts`: the report's schema

The report is two `Serialize` structs. `Summary` carries the twelve fields
in the report's order; `OutcomeCounts` carries the three-way outcome
balance. Two representation choices do quiet work: every map is a
`BTreeMap`, so iteration is sorted and the serialized JSON is deterministic
across runs; and the maps' keys are the histogram's ply counts (which
serialize as JSON object keys, hence strings — the test pins `"6"`, not
`6`) and the opening counters' string keys.

**Source:** `src/bin/selfplayverify.rs`, lines 43–65. *Created in this
chapter; byte-identical to the completed reference.*

```rust
  43 | #[derive(Serialize)]
  44 | struct Summary {
  45 |     game_count: usize,
  46 |     verified_plies: usize,
  47 |     certified_suffixes: usize,
  48 |     games_ending_within_16_plies: usize,
  49 |     min_plies: usize,
  50 |     max_plies: usize,
  51 |     mean_plies: f64,
  52 |     median_plies: f64,
  53 |     length_histogram_plies: BTreeMap<usize, usize>,
  54 |     opening_offers: BTreeMap<String, usize>,
  55 |     opening_answers: BTreeMap<String, usize>,
  56 |     outcomes: OutcomeCounts,
  57 | }

  59 | #[derive(Serialize)]
  60 | struct OutcomeCounts {
  61 |     black: usize,
  62 |     white: usize,
  63 |     draw: usize,
  64 | }

```

## 5. `Summary::from_records`: every number from the records

The derivation runs only after every record has verified. Lengths are
collected and sorted once, which gives `min_plies` and `max_plies` as the
first and last elements and the median as the middle element — or, for an
even count, the mean of the two middle values, so a median can end in `.5`
(the 48-game corpus's median is 27.0; a 50-game corpus could report 27.5).
`verified_plies` is the total of post-opening plies across all records;
`games_ending_within_16_plies` counts the short tactical tail. The
per-record loop then accumulates the exact length frequencies (not bins),
the opening offer counters, the answer counters, and the outcome balance.

Two keying decisions deserve attention. First, `opening_offers` keys are
provenance strings: `Generated` for a generated opening, or `<list>#<index>`
for a listed one, where `<list>` is the opening list's path **as recorded
in the header at generation time** — the literal string the `--openings`
flag carried. That is why the capstone's report shows keys beginning
`../data/openings/…`: the generation command was run from `gomoku/` with
that relative spelling. Verify the same corpus from any working directory
and the keys do not change; they are data, not invocation artifacts.
Second, `opening_answers` collapses the answer enum into four shape strings
(`take_black`, `take_white`, `add_two_take_black`, `add_two_take_white`) —
the placements themselves stay in the records; the report counts shapes.

`certified_suffixes` counts records that *carry* a suffix — presence, not
re-verification. The `verify_line` re-verification is chapter 05's
`selfplayanchors` pass; the acceptance legality gate runs both binaries,
and this paper's Verification section does exactly that against the
48-game corpus.

**Source:** `src/bin/selfplayverify.rs`, lines 66–138. *Created in this
chapter; byte-identical to the completed reference.*

```rust
  66 | impl Summary {
  67 |     fn from_records(records: &[GameRecord]) -> Self {
  68 |         let mut lengths: Vec<_> = records.iter().map(|record| record.plies.len()).collect();
  69 |         lengths.sort_unstable();
  70 |         let verified_plies = lengths.iter().sum();
  71 |         let mean_plies = verified_plies as f64 / lengths.len() as f64;
  72 |         let median_plies = if lengths.len() % 2 == 0 {
  73 |             (lengths[lengths.len() / 2 - 1] + lengths[lengths.len() / 2]) as f64 / 2.0
  74 |         } else {
  75 |             lengths[lengths.len() / 2] as f64
  76 |         };
  77 |         let mut length_histogram_plies = BTreeMap::new();
  78 |         let mut opening_offers = BTreeMap::new();
  79 |         let mut opening_answers = BTreeMap::new();
  80 |         let mut outcomes = OutcomeCounts {
  81 |             black: 0,
  82 |             white: 0,
  83 |             draw: 0,
  84 |         };
  85 |         for record in records {
  86 |             *length_histogram_plies
  87 |                 .entry(record.plies.len())
  88 |                 .or_insert(0) += 1;
  89 |             let offer = match &record.header.opening.provenance {
  90 |                 OpeningProvenance::Generated => "Generated".to_owned(),
  91 |                 OpeningProvenance::Listed { list, index } => format!("{list}#{index}"),
  92 |             };
  93 |             *opening_offers.entry(offer).or_insert(0) += 1;
  94 |             let answer = match record.header.opening.answer {
  95 |                 OpeningAnswer::Take(Color::Black) => "take_black",
  96 |                 OpeningAnswer::Take(Color::White) => "take_white",
  97 |                 OpeningAnswer::AddTwo {
  98 |                     final_choice: Color::Black,
  99 |                     ..
 100 |                 } => "add_two_take_black",
 101 |                 OpeningAnswer::AddTwo {
 102 |                     final_choice: Color::White,
 103 |                     ..
 104 |                 } => "add_two_take_white",
 105 |             };
 106 |             *opening_answers.entry(answer.to_owned()).or_insert(0) += 1;
 107 |             match record.outcome {
 108 |                 Outcome::Won {
 109 |                     winner: Color::Black,
 110 |                     ..
 111 |                 } => outcomes.black += 1,
 112 |                 Outcome::Won {
 113 |                     winner: Color::White,
 114 |                     ..
 115 |                 } => outcomes.white += 1,
 116 |                 Outcome::Draw => outcomes.draw += 1,
 117 |             }
 118 |         }
 119 |         Self {
 120 |             game_count: records.len(),
 121 |             verified_plies,
 122 |             certified_suffixes: records
 123 |                 .iter()
 124 |                 .filter(|record| record.certified_suffix.is_some())
 125 |                 .count(),
 126 |             games_ending_within_16_plies: lengths.iter().filter(|&&length| length <= 16).count(),
 127 |             min_plies: lengths.first().copied().unwrap_or(0),
 128 |             max_plies: lengths.last().copied().unwrap_or(0),
 129 |             mean_plies,
 130 |             median_plies,
 131 |             length_histogram_plies,
 132 |             opening_offers,
 133 |             opening_answers,
 134 |             outcomes,
 135 |         }
 136 |     }
 137 | }

```

That completes the binary: lines 1–188 quoted in full across sections 1–5,
in execution order. The binary has no unit tests — the `selfplayverify`
test target reports zero tests of its own — because the four integration
tests of section 8 are its whole specification.

## 6. `tests/binary.rs`: the shared fixture

The final form of the test target chapter 05 created. The fixture half is
untouched chapter-05 material, quoted here because the four new tests build
on it: the imports (`CARGO_BIN_EXE`-driven `Command`, the `engine` and
`selfplay` vocabulary, `PlayerOptions` for the default seats); `TestDir`,
the temporary-directory helper whose `Drop` deletes the directory; `mv`,
the coordinate shorthand; and `certified_record`, the fabricated one-game
corpus content.

The fixture record is worth reading once, carefully. The opening places
`(5, 5)`, `(10, 10)`, `(5, 6)` with the answer "take Black" and generated
provenance. The six post-opening plies are `(10, 11)`, `(5, 7)`,
`(10, 12)`, `(5, 8)`, `(10, 13)`, `(5, 9)`: White builds on row 10, Black
on row 5, and Black's last move completes the five `(5, 5)`–`(5, 9)` — so
the honest outcome is a Black win by five-or-more, and the certified suffix
from ply 5 carries the one-move winning line `[(5, 9)]`. Every ply is
unsearched (`search: None`): the verifier never looks at search data, so
the fixture needs none. The record is built as a `Game` with default seat
options and converted with `GameRecord::from_game` — the same conversion
the real runner uses.

One honest note belongs with `TestDir`, and it is developed in the
Verification section: the directory name is
`selfplayanchors-test-<pid>-<nanos>` (the prefix is historical — chapter
05's anchor tests introduced the helper), parallel tests share the process
id, and this session observed three collisions. The helper is quoted here
exactly as it stands; no fix has landed, and the hardening, if it ever
lands, is a fixture change — never a verifier change.

**Source:** [`gomoku/crates/selfplay/tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs),
lines 1–73 (imports, `TestDir`, `mv`, `certified_record`, and the trailing separator). *Unchanged from
the chapter-6 slice; the final file is byte-identical to the completed
reference (SHA-256 `588707e9…f826`).*

```rust
   1 | use std::path::{Path, PathBuf};
   2 | use std::process::Command;
   3 | use std::time::{SystemTime, UNIX_EPOCH};

   5 | use engine::{Color, Move};
   6 | use player::PlayerOptions;
   7 | use selfplay::{
   8 |     AnchorSeed, CertifiedSuffix, Game, GameRecord, Opening, OpeningAnswer, OpeningProvenance,
   9 |     Outcome, PlyRecord, WinReason, write_corpus,
  10 | };

  12 | struct TestDir(PathBuf);

  14 | impl TestDir {
  15 |     fn new() -> Self {
  16 |         let nanos = SystemTime::now()
  17 |             .duration_since(UNIX_EPOCH)
  18 |             .expect("system clock is after the Unix epoch")
  19 |             .as_nanos();
  20 |         Self(std::env::temp_dir().join(format!(
  21 |             "selfplayanchors-test-{}-{nanos}",
  22 |             std::process::id()
  23 |         )))
  24 |     }

  26 |     fn path(&self) -> &Path {
  27 |         &self.0
  28 |     }
  29 | }

  31 | impl Drop for TestDir {
  32 |     fn drop(&mut self) {
  33 |         let _ = std::fs::remove_dir_all(&self.0);
  34 |     }
  35 | }

  37 | fn mv(row: u8, col: u8) -> Move {
  38 |     Move::new(row, col).expect("test coordinate is on the board")
  39 | }

  41 | fn certified_record() -> GameRecord {
  42 |     let moves = [
  43 |         mv(10, 11),
  44 |         mv(5, 7),
  45 |         mv(10, 12),
  46 |         mv(5, 8),
  47 |         mv(10, 13),
  48 |         mv(5, 9),
  49 |     ];
  50 |     GameRecord::from_game(&Game {
  51 |         seed: 17,
  52 |         black_options: PlayerOptions::default(),
  53 |         white_options: PlayerOptions::default(),
  54 |         opening: Opening::new(
  55 |             [mv(5, 5), mv(10, 10), mv(5, 6)],
  56 |             OpeningAnswer::Take(Color::Black),
  57 |             OpeningProvenance::Generated,
  58 |         ),
  59 |         plies: moves
  60 |             .into_iter()
  61 |             .map(|mv| PlyRecord { mv, search: None })
  62 |             .collect(),
  63 |         outcome: Outcome::Won {
  64 |             winner: Color::Black,
  65 |             reason: WinReason::FiveOrMore,
  66 |         },
  67 |         certified_suffix: Some(CertifiedSuffix {
  68 |             from_ply: 5,
  69 |             line: vec![mv(5, 9)],
  70 |         }),
  71 |     })
  72 | }

```

## 7. `tests/binary.rs`: the three earlier tests, unchanged

Chapter 05's three binary tests, quoted for completeness: the `selfplaygen`
help contract (the corpus and grading flags must appear in `--help`), the
`selfplayanchors` help contract (`--corpus`, `--out`, and the `verify_line`
mention), and the anchor round trip (write the fixture corpus, run the
extractor, read back exactly one `AnchorSeed` with the fixture's
`game_index: 0`, `from_ply: 5`, `line: [(5, 9)]`). This chapter changes not
one byte of them; they are quoted because the paper quotes the final file
in full, and because their presence is what "grows from three tests to
seven" means.

**Source:** `tests/binary.rs`, lines 74–143. *Unchanged from the chapter-6
slice; byte-identical to the completed reference.*

```rust
  74 | #[test]
  75 | fn selfplaygen_help_documents_corpus_and_grading_flags() {
  76 |     let output = Command::new(env!("CARGO_BIN_EXE_selfplaygen"))
  77 |         .arg("--help")
  78 |         .output()
  79 |         .expect("selfplaygen --help runs");

  81 |     assert!(output.status.success());
  82 |     let stdout = String::from_utf8(output.stdout).expect("help is UTF-8");
  83 |     for flag in [
  84 |         "--black-sims",
  85 |         "--white-sims",
  86 |         "--games",
  87 |         "--seed-base",
  88 |         "--shard-capacity",
  89 |         "--out",
  90 |         "--openings",
  91 |     ] {
  92 |         assert!(stdout.contains(flag), "help omitted {flag}:\n{stdout}");
  93 |     }
  94 | }

  96 | #[test]
  97 | fn selfplayanchors_help_documents_corpus_extraction() {
  98 |     let output = Command::new(env!("CARGO_BIN_EXE_selfplayanchors"))
  99 |         .arg("--help")
 100 |         .output()
 101 |         .expect("selfplayanchors --help runs");

 103 |     assert!(output.status.success());
 104 |     let stdout = String::from_utf8(output.stdout).expect("help is UTF-8");
 105 |     for flag in ["--corpus", "--out"] {
 106 |         assert!(stdout.contains(flag), "help omitted {flag}:\n{stdout}");
 107 |     }
 108 |     assert!(stdout.contains("verify_line"));
 109 | }

 111 | #[test]
 112 | fn selfplayanchors_reads_a_corpus_and_writes_json_anchor_seeds() {
 113 |     let root = TestDir::new();
 114 |     let corpus = root.path().join("corpus");
 115 |     let out = root.path().join("anchors.json");
 116 |     write_corpus(&[certified_record()], &corpus, 1).expect("fabricated corpus writes");

 118 |     let output = Command::new(env!("CARGO_BIN_EXE_selfplayanchors"))
 119 |         .arg("--corpus")
 120 |         .arg(&corpus)
 121 |         .arg("--out")
 122 |         .arg(&out)
 123 |         .output()
 124 |         .expect("selfplayanchors runs");

 126 |     assert!(
 127 |         output.status.success(),
 128 |         "stderr: {}",
 129 |         String::from_utf8_lossy(&output.stderr)
 130 |     );
 131 |     let seeds: Vec<AnchorSeed> =
 132 |         serde_json::from_slice(&std::fs::read(&out).expect("anchor JSON was written"))
 133 |             .expect("anchor output is valid JSON");
 134 |     assert_eq!(
 135 |         seeds,
 136 |         vec![AnchorSeed {
 137 |             game_index: 0,
 138 |             from_ply: 5,
 139 |             line: vec![mv(5, 9)],
 140 |         }]
 141 |     );
 142 | }

```

## 8. `tests/binary.rs`: the four `selfplayverify_*` tests

The chapter's whole specification, appended at reference lines 143–232 (the
blank separator at 143, then the four test functions at 144–232). The
file order is the order the tests were landed; the chapter paper orders
them along the execution path instead (empty corpus, illegal move, outcome
disagreement, report). Both orders are quoted here as they stand in the
file. Exactly three of the four tests use `certified_record()`; the
empty-corpus test fabricates no record at all — it creates a directory and
nothing else, because an empty report would be indistinguishable from a
typo'd path that happened to be a directory.

- `selfplayverify_rejects_a_recorded_outcome_that_disagrees_with_the_engine`
  (lines 144–165) doctors the fixture's outcome to a *White* win while
  leaving the moves — and Black's five — unchanged. The process must fail,
  and stderr must contain `final status` and `game 0`.
- `selfplayverify_reports_replay_and_corpus_statistics_as_json` (lines
  167–195) runs the unmodified fixture corpus and pins the report schema
  field by field: `game_count` 1, `verified_plies` 6, `certified_suffixes`
  1, outcomes `{black: 1, white: 0, draw: 0}`, the histogram `{"6": 1}`
  (a map key, hence a string), `games_ending_within_16_plies` 1, and
  `opening_offers` `{"Generated": 1}`. The exit status is success and
  stdout parses as JSON — the success path is a machine-readable report,
  not prose.
- `selfplayverify_rejects_an_illegal_recorded_move` (lines 197–215) doctors
  ply 1 to `(5, 5)`, a cell the opening already occupies. The process must
  fail, and stderr must contain `ply 1` and `occupied` — the verifier names
  the position, the engine's `PlayError` supplies the reason.
- `selfplayverify_rejects_a_corpus_without_game_records` (lines 217–232)
  points `--corpus` at an empty directory. The process must fail with `no
  game records` on stderr.

All four run by default; none is ignored; no test plays a game — each
drives the compiled binary through `env!("CARGO_BIN_EXE_selfplayverify")`
against fabricated corpora, so the fast gate stays fast (the slice runs the
whole target in 0.15 s in debug).

**Source:** `tests/binary.rs`, lines 144–232. *Created in this chapter;
byte-identical to the completed reference.*

```rust
 144 | #[test]
 145 | fn selfplayverify_rejects_a_recorded_outcome_that_disagrees_with_the_engine() {
 146 |     let root = TestDir::new();
 147 |     let corpus = root.path().join("corpus");
 148 |     let mut record = certified_record();
 149 |     record.outcome = Outcome::Won {
 150 |         winner: Color::White,
 151 |         reason: WinReason::FiveOrMore,
 152 |     };
 153 |     write_corpus(&[record], &corpus, 1).expect("fabricated corpus writes");

 155 |     let output = Command::new(env!("CARGO_BIN_EXE_selfplayverify"))
 156 |         .arg("--corpus")
 157 |         .arg(&corpus)
 158 |         .output()
 159 |         .expect("selfplayverify runs");

 161 |     assert!(!output.status.success());
 162 |     let stderr = String::from_utf8_lossy(&output.stderr);
 163 |     assert!(stderr.contains("final status"), "stderr: {stderr}");
 164 |     assert!(stderr.contains("game 0"), "stderr: {stderr}");
 165 | }

 167 | #[test]
 168 | fn selfplayverify_reports_replay_and_corpus_statistics_as_json() {
 169 |     let root = TestDir::new();
 170 |     let corpus = root.path().join("corpus");
 171 |     write_corpus(&[certified_record()], &corpus, 1).expect("fabricated corpus writes");

 173 |     let output = Command::new(env!("CARGO_BIN_EXE_selfplayverify"))
 174 |         .arg("--corpus")
 175 |         .arg(&corpus)
 176 |         .output()
 177 |         .expect("selfplayverify runs");

 179 |     assert!(
 180 |         output.status.success(),
 181 |         "stderr: {}",
 182 |         String::from_utf8_lossy(&output.stderr)
 183 |     );
 184 |     let summary: serde_json::Value =
 185 |         serde_json::from_slice(&output.stdout).expect("summary is JSON");
 186 |     assert_eq!(summary["game_count"], 1);
 187 |     assert_eq!(summary["verified_plies"], 6);
 188 |     assert_eq!(summary["certified_suffixes"], 1);
 189 |     assert_eq!(summary["outcomes"]["black"], 1);
 190 |     assert_eq!(summary["outcomes"]["white"], 0);
 191 |     assert_eq!(summary["outcomes"]["draw"], 0);
 192 |     assert_eq!(summary["length_histogram_plies"]["6"], 1);
 193 |     assert_eq!(summary["games_ending_within_16_plies"], 1);
 194 |     assert_eq!(summary["opening_offers"]["Generated"], 1);
 195 | }

 197 | #[test]
 198 | fn selfplayverify_rejects_an_illegal_recorded_move() {
 199 |     let root = TestDir::new();
 200 |     let corpus = root.path().join("corpus");
 201 |     let mut record = certified_record();
 202 |     record.plies[1].mv = mv(5, 5);
 203 |     write_corpus(&[record], &corpus, 1).expect("fabricated corpus writes");

 205 |     let output = Command::new(env!("CARGO_BIN_EXE_selfplayverify"))
 206 |         .arg("--corpus")
 207 |         .arg(&corpus)
 208 |         .output()
 209 |         .expect("selfplayverify runs");

 211 |     assert!(!output.status.success());
 212 |     let stderr = String::from_utf8_lossy(&output.stderr);
 213 |     assert!(stderr.contains("ply 1"), "stderr: {stderr}");
 214 |     assert!(stderr.contains("occupied"), "stderr: {stderr}");
 215 | }

 217 | #[test]
 218 | fn selfplayverify_rejects_a_corpus_without_game_records() {
 219 |     let root = TestDir::new();
 220 |     let corpus = root.path().join("corpus");
 221 |     std::fs::create_dir_all(&corpus).expect("empty corpus directory exists");

 223 |     let output = Command::new(env!("CARGO_BIN_EXE_selfplayverify"))
 224 |         .arg("--corpus")
 225 |         .arg(&corpus)
 226 |         .output()
 227 |         .expect("selfplayverify runs");

 229 |     assert!(!output.status.success());
 230 |     let stderr = String::from_utf8_lossy(&output.stderr);
 231 |     assert!(stderr.contains("no game records"), "stderr: {stderr}");
 232 | }
```

## 9. Verification

The chapter paper's verification set, plus the release default gate (the
game-playing discipline of chapter 04), the workspace gate, the standing
quality gates, a fresh determinism gate, a read-only re-verification of the
reference's 48-game corpus, and the capstone with its four boundary
experiments. Everything in this section was measured fresh on the final
slice on 2026-11-14, from the slice's `gomoku/` unless a command line says
otherwise, with logs at `/tmp/selfplay-ch7-verify-logs/` (scratch-local and
ephemeral) — except the closing subsection, which quotes the reference's
historical evidence, attributed and not re-run.

### Gates

| Command | Result on the final slice |
|---------|---------------------------|
| `cargo test -p selfplay --test binary` | first run: **1 failed, 6 passed** — the known `TestDir` race, logged honestly (below); rerun without touching source: **7 passed, 0 failed, 0 ignored** (0.15 s) |
| `cargo test -p selfplay --test binary selfplayverify` | **4 passed**, 0 failed, 3 filtered out (0.05 s) — the chapter's four tests by name |
| `cargo test -p selfplay` (debug) | **50 passed, 0 failed, 4 ignored across 15 suites** (deduplicated); the raw log shows 52 passed across 17 `test result` lines |
| `cargo test -p selfplay --release` | **50 passed, 0 failed, 4 ignored** (deduplicated; raw 52 across 17); the binary target itself: 7 passed (0.17 s); the `selfplayverify` unit target: 0 tests, as specified |
| `cargo test --workspace` (debug) | first two runs: the `TestDir` race again (below), cargo aborting at the failing target; third run, without touching source: **852 passed, 0 failed, 11 ignored across 63 unique suites** (deduplicated); raw 854 passed across 65 `test result` lines |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0, no warnings |
| `cargo fmt --all -- --check` | exit 0, no changes |

The per-target closing state of the crate, identical in debug and release,
matches the chapter paper's table exactly: `src/lib.rs` unit 2/0,
`selfplaygen` unit 4/0, the three other binary unit targets (including the
new `selfplayverify`) 0/0 each — **the `src` unit tests total 6/0** — and
the integration targets anchors 4/0, binary 7/0, corpus 10/0, dedup 1/0,
game 5/2, grading 0/1, openings 5/0, read_path 11/0, schedule 1/1,
doc-tests 0/0. The four ignored tests are exactly chapter 04's MANUAL set —
the two game-playing probes in `tests/game.rs`, the scheduled-game
reproducibility test in `tests/schedule.rs`, and the grading probe in
`tests/grading.rs` — and this chapter adds none. The deduplicated
per-target count is the contract; the raw lines are higher by exactly two
in the crate gate (52 across 17) and two in the workspace gate (854 across
65) because the corpus target's cross-process determinism test re-executes
its binary in two child processes whose one-test reports (`1 passed, 9
filtered out`, each) print in the parent log. Count per target or count raw
lines — never mix the two methods.

### The `TestDir` race: three fresh collisions, observation and hypothesis

The chapter paper carries an open observation: one of the reference's
repeated binary-target runs failed with the paired symptoms of two tests
sharing one temporary directory. This session observed the race **three
more times**, and preserves the evidence rather than fixing the fixture:

1. The first focused `cargo test -p selfplay --test binary` run:
   `selfplayverify_rejects_a_corpus_without_game_records` failed — its
   stderr showed `game 0: move Move(80) at ply 1 was rejected: Cell is
   already occupied.`, which is the *illegal-move* test's corpus content,
   not the empty directory the test created
   (`/tmp/selfplay-ch7-verify-logs/test-binary-debug.log`). Rerun: green,
   7/7.
2. The first `cargo test --workspace` run:
   `selfplayverify_rejects_an_illegal_recorded_move` failed with `File
   exists (os error 17)` while writing its corpus, and
   `selfplayverify_rejects_a_recorded_outcome_that_disagrees_with_the_engine`
   failed because the verifier reported `corpus contains no game records`
   over a directory the sibling test's `Drop` had just emptied
   (`test-workspace-debug.log`). Cargo aborted the remaining targets at the
   failure, so that log's partial counts (42 result lines) are not a gate
   result and are quoted as nothing but race evidence.
3. The immediate workspace rerun collided again with the roles swapped —
   the outcome test hit `File exists`, the illegal-move test hit `No such
   file or directory` (`test-workspace-debug-rerun1.log`). The next rerun
   was green end to end and is the gate result above.

The mechanism is still a hypothesis, stated in the chapter and unchanged
here: parallel tests share the process id, so two tests that draw the same
clock nanoseconds share one directory, and one test's `Drop` then deletes
the other's directory mid-run. One new supporting measurement, honestly
labeled as such: a probe of this machine's system clock (Python's
`time_ns`, 100 000 consecutive samples) returned microsecond-quantized
values — consecutive reads usually returned identical stamps (97 073 of
99 999 consecutive pairs identical), nonzero deltas were multiples of
1 000 ns, the smallest 1 000 ns — so `SystemTime::now().as_nanos()` on
this machine has a collision window of at least one microsecond, which
makes same-instant `TestDir` names plausible for tests whose threads
start together. The probe describes the clock, not the failing runs —
no failed run was instrumented — so the hypothesis stands as a
hypothesis, consistent with all three observed pairings. No source was
modified; every count this paper quotes comes from
a green run; and the documented hardening (a per-test label in the
directory name, as `tests/corpus.rs`'s `TestDir` already has) remains a
fixture change that has not landed.

### The determinism gate, fresh — and equal to history

From the slice root (so the opening-list path spellings match the
historical gate's `data/openings/…` arguments), two separate release
invocations of the historical gate's exact command, differing only in the
output directory:

```text
gomoku/target/release/selfplaygen \
  --out /tmp/selfplay-ch7-verify-logs/determinism-{a,b} \
  --openings data/openings/gomocup-2024-freestyle15.txt \
  --openings data/openings/gomocup-2025-freestyle15.txt \
  --openings data/openings/gomocup-2026-freestyle15.txt \
  --black-sims 8 --white-sims 8 --games 3 \
  --seed-base 424242 --shard-capacity 1000
```

Both runs accepted seeds `[424242, 424243, 424244]` and wrote one shard
each; wall times 2.98 s and 2.80 s. The shards compare byte-identical
(`cmp` exit 0), and so do the manifests. The measured SHA-256 values:

| File | SHA-256 (both runs) | Historical 2026-11-11 value |
|------|---------------------|-----------------------------|
| `shard-000.bin` | `4388d84e4ce5f3c2a03017adc49df11d5aabba841c60413b1e9120a70812ae52` | **identical** |
| `manifest.json` | `e3c7c463e0cc8dcb6576823158a743c4976242ce692757bae385a425c9c466c6` | **identical** |

Because the fresh gate used the same path spellings, seed base, pairing,
and shard capacity as the reference's 2026-11-11 run, the equality of
hashes is a cross-session, cross-build reproduction of the historical gate,
not just a same-afternoon rerun. (The runs' stdout differs between A and B
only in the `--out` path line, as expected; the recorded provenance embeds
the opening-list paths, not the output path.)

### The 48-game corpus, re-verified read-only

Against the reference worktree's first corpus
(`data/selfplay/v0-b32/`), with the four corpus files' SHA-256 hashes taken
before and after and no writes anywhere in the reference tree:

- The slice's release `selfplayverify --corpus …/v0-b32` exited 0 and
  printed the quality report: **48 games, 1 860 verified plies, 47
  certified suffixes**, lengths min 7 / median 27.0 / mean 38.75 / max 222,
  10 games ending within 16 plies. The fresh report is **byte-identical**
  to the historical `acceptance-logs/v0-b32-verified.json` — the report
  contains no invocation paths, so the comparison is exact rather than
  approximate. Fresh wall time below the timer's 0.01 s display resolution
  (`real 0.00` in `/tmp/selfplay-ch7-verify-logs/v0-b32-verify-time.log`,
  warm cache, release); the historical 0.19 s is quoted for the record —
  content was compared, never timing.
- The slice's release `selfplayanchors --corpus …/v0-b32 --out
  /tmp/selfplay-ch7-verify-logs/v0-b32-anchors-rerun.json` reported
  **`Wrote 47 verified anchor seeds`** on stderr — 47 of 47 certificates
  re-verified by `engine::verify_line`. The binary reports on stderr, so
  the run was re-captured with both streams into
  `/tmp/selfplay-ch7-verify-logs/v0-b32-anchors-rerun.log` (the earlier
  stdout-only capture is empty for that reason); the 47-seed JSON is
  preserved as `v0-b32-anchors-rerun.json` and compares byte-identical
  (`cmp` exit 0) to the reference's `data/selfplay/v0-b32-anchors.json`.
- The before/after hash lists of the corpus (manifest plus three shards)
  are identical, and the reference worktree's `git status` is unchanged by
  these runs: the inputs were read, never written.

### The capstone, fresh

The chapter paper's capstone, run from the slice's `gomoku/` in release
with every output inside a `mktemp -d` directory. The slice carries the
committed opening lists at `../data/openings/` relative to `gomoku/`, so
the quoted command — and its quoted expected `opening_offers` keys — needed
no adaptation:

```text
target/release/selfplaygen \
  --out "$dir/corpus" \
  --openings ../data/openings/gomocup-2024-freestyle15.txt \
  --black-sims 8 --white-sims 8 --games 3 --seed-base 777
target/release/selfplayverify --corpus "$dir/corpus"
target/release/selfplayanchors --corpus "$dir/corpus" --out "$dir/anchors.json"
```

The generation accepted seeds `[777, 778, 779]` and wrote one shard. The
verifier's JSON matched the chapter paper's quoted report **exactly** —
`game_count` 3, `verified_plies` 114, `certified_suffixes` 3,
`games_ending_within_16_plies` 1, lengths min 13 / median 18.0 / mean 38.0
/ max 83 with histogram `{13: 1, 18: 1, 83: 1}`, `opening_offers` keyed by
the literal `../data/openings/gomocup-2024-freestyle15.txt#0`, `#5`, `#6`,
`opening_answers` `{take_white: 3}`, outcomes `{black: 1, white: 2, draw:
0}`. The extractor reported `Wrote 3 verified anchor seeds`, one per game,
each line one ply long; the fresh `anchors.json` even reproduces the
chapter's worked example — game 0's seed is `{game_index: 0, from_ply: 17,
line: [139]}`, and 139 is row 9, column 4 (9·15 + 4). The extraction was
re-run against the preserved capstone corpus with both streams captured
into `/tmp/selfplay-ch7-verify-logs/capstone-anchors-rerun.log` (the
earlier stdout-only capture is empty because the report goes to stderr);
the rerun's 3-seed JSON, preserved as `capstone-anchors-rerun.json`,
compares byte-identical (`cmp` exit 0) to the capstone's original
`anchors.json`. The four boundary
experiments, all green:

| Experiment | Result |
|------------|--------|
| `--corpus` on an empty directory | `selfplayverify failed: corpus contains no game records`, exit **1** |
| unknown flag `--json` | `unknown flag: --json` plus the one-line usage, exit **2** |
| `selfplayanchors` with an existing `--out` | `failed to create '…/anchors.json': File exists (os error 17)`, exit **1** — extractions never overwrite |
| verifier run twice into two files | `cmp` exit 0 — **byte-identical JSON**, and identical to the first capstone verification as well |

No `nettrain` rerun: the integration smoke is historical evidence, quoted
below, and re-running it is not this chapter's verification work.

### Historical evidence, attributed and not re-run

These numbers are the reference's acceptance run of 2026-11-11, recorded in
the reference worktree's `acceptance-evidence.md` and `acceptance-logs/`
and quoted in the chapter paper's gates section. This session did not
re-measure them; they are separated from the fresh evidence above
deliberately.

- **Throughput (gate 4):** the 48-game baseline-32 corpus generated in
  122.68 s — 1 408.54 games/hour, mean 2.56 s per game — three shards, 200
  KiB, all 48 base seeds accepted with no derived replacements.
- **Integration (gate 5):** `selfplayexport` materialized 1 860 samples
  from the 48 games into 4 shards in 0.18 s; the unmodified `nettrain`
  trained the tiny configuration for 50 of 50 steps in 0.47 s, train loss
  6.5483584 at step 1 to 6.252371 at step 50 (holdout 6.1449027; split
  1 665 train / 195 holdout). Smoke evidence only; no model-quality or
  strength claim.
- **Grading observations (gate 3):** the report-only 20-game probe at
  baseline-32 vs baseline-1 inverted the expectation (the budget-1 seat won
  15 of 20), and the chapter-04 slice's fresh run of the four pinned
  grading seeds produced per-seed outcomes the reference's pinned comment
  does not describe. Both stand unresolved; the pre-registered 50-game
  baseline-800 vs baseline-200 protocol is **registered, not executed** —
  in the reference and in this session alike. This paper ran no grading
  probe and no ignored test.

## 10. Done when

Every item of the chapter paper's done-when, mapped to this paper's
evidence:

- `cargo test -p selfplay --release` reports **50 passed, 4 ignored** —
  measured (§9, gates table); the deduplicated per-target count is the
  contract, the raw log's 52 across 17 is explained, and the two counting
  methods are never mixed. `tests/binary.rs` closes at **7/0** with exactly
  the four `selfplayverify_*` tests; the three earlier tests are unchanged
  (§7's quotation is byte-checked).
- The **`src` unit tests total 6/0** — two in the library target, four in
  the `selfplaygen` binary target — and the new `selfplayverify` unit
  target contributes **zero** tests, as specified (§9's release log shows
  its `0 passed` line).
- **No ignored test was added and none was run**: the ignored set is
  exactly chapter 04's four MANUAL tests, and this session executed no
  `-- --ignored` selection and no grading probe.
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt
  --all -- --check` are green on the slice (§9).
- The acceptance gates are documented in the chapter paper with their
  measured evidence attributed to the reference's logs; the grading gate is
  pre-registered with its non-execution stated, and this paper's
  Verification section keeps fresh and historical evidence separated.
- The commit message is `feat(selfplay): acceptance — selfplay-corpora
  chapter 07`.

## 11. Troubleshooting and pitfalls

Symptoms first, causes second, in the order a learner meets them.

- **A binary test fails with `File exists`, `No such file or directory`, or
  another test's corpus content.** The known `TestDir` race (§9): two
  parallel tests drew the same `<pid>-<nanos>` directory name. Rerun the
  gate; do not patch the verifier, and do not silently "fix" the fixture
  mid-chapter — the documented hardening is a per-test label in the
  directory name, and it is deliberately not part of this chapter.
- **`corpus contains no game records` on a corpus you believe is
  populated.** Check the path first — an empty report would be
  indistinguishable from a typo'd directory, which is exactly why the empty
  corpus is an error (exit 1), not a report. Then check that the directory
  holds `shard-*.bin` files: `read_corpus` lists shards, and a directory of
  anything else reads as empty.
- **`unknown flag: --corpus=/path`.** Values are separate arguments only;
  write `--corpus /path`. The same holds for `--flag=value` forms
  generally — the parser pattern-matches whole arguments.
- **Expecting `--help` or `--json`.** Neither exists, by design: misuse is
  exit 2 with the one-line usage, and the report is always JSON on stdout.
  Redirect stdout to a file; do not look for a format flag.
- **A `median_plies` ending in `.5`.** Working as designed: an even game
  count averages the two middle lengths.
- **`length_histogram_plies` keys are strings in JSON.** The histogram is a
  map; JSON object keys are strings. Consume it as a map, not an array, and
  do not "fix" the report into bins — the frequencies are exact by design.
- **`opening_offers` keys that look like someone else's paths.** The keys
  embed the opening-list path *as spelled at generation time* and travel
  inside the records. A corpus generated with `../data/openings/…` keeps
  those keys no matter where you verify it from. Compare reports across
  corpora only after normalizing for that spelling.
- **A resignation outcome rejected.** The resignation rule requires
  `Status::Ongoing` with the *loser* to move — not `Status::Won`. If a
  fabricated resignation record fails, check whose turn the replayed board
  says it is; the reason string inside `Resignation(..)` is a decision
  record and is never replayed.
- **Counting 52 where the chapter says 50.** You summed raw `test result`
  lines. The corpus target's two child processes print their own reports;
  count per target (50/4) or count raw lines (52/17), never mix. The same
  caveat applies to the workspace totals (852/63 vs 854/65).
- **Adding the manifest cross-check to make the verifier "complete."** The
  gap is real and recorded — the verifier never opens `manifest.json`, so a
  manifest that disagrees with its shards passes unnoticed — but it is an
  *open* hardening item, not a defect this chapter papers over. If you
  implement it, do it as new work with its own tests, and expect the
  chapter's quoted byte-identical behaviors to change.

## References

**This chapter:**

- [acceptance.md](acceptance.md) — the chapter paper: intention,
  specification as tests, the execution-order walkthrough, the five
  acceptance gates with measured evidence, and the open observations.
- [acceptance-design.md](acceptance-design.md) — the conceptual story:
  acceptance proves the stored data contract, not the player; one flag,
  unconditional JSON, and the manifest boundary.
- [acceptance-implementation.md](acceptance-implementation.md) — the hiding
  layer whose six-step construction plan this paper's section order
  follows.

**This track:**

- [Tutorial README](../README.md) — conventions, gates, the chapter map,
  and the reference-provenance rule this paper's Provenance section
  applies.
- [Self-play primer](../00-selfplay-primer.md) — §6 the five acceptance
  gates with the measured headline numbers; §7 the open questions; §8 the
  invariants.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md)
  and its [solution](../02-recording-games/recording-games-solution.md) —
  the `GameRecord` format, `read_corpus` / `write_corpus`, and the
  cross-process determinism test behind the raw-line counting caveat.
- [Chapter 04 — diversity and grading](../04-diversity-and-grading/diversity-and-grading.md)
  and its
  [solution](../04-diversity-and-grading/diversity-and-grading-solution.md) —
  `selfplaygen`, the opening lists, the seed discipline, the release-mode
  discipline, and the four MANUAL tests this chapter neither adds to nor
  runs.
- [Chapter 05 — certified endgames](../05-certified-endgames/certified-endgames.md)
  and its
  [solution](../05-certified-endgames/certified-endgames-solution.md) —
  the `TestDir` / `mv` / `certified_record` fixture, the `verify_line`
  soundness gate, and the `selfplayanchors` binary the legality gate runs
  beside the verifier.
- [Chapter 06 — reading back](../06-reading-back/reading-back.md) and its
  [solution](../06-reading-back/reading-back-solution.md) — the π/z
  contracts the verifier does not re-check, and the chapter-6 slice this
  paper's slice was copied from.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) — §5
  the five acceptance gates; S7 the retired mock-strength probe.
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  the reference-implementation layout and the measured headline numbers
  quoted here with attribution.
- [docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) —
  the document head's claim-label convention; §12 the soundness gate and
  the determinism policy.
- [docs/14-openings-and-supervised-curriculum.md](../../../14-openings-and-supervised-curriculum.md) —
  §3 the Swap2 procedure and the tournament opening lists; decision D6 the
  training mixture the verified corpora feed.

**Prerequisite tracks:**

- [Engine tutorial](../../13-engine-tutorial/README.md) — the bitboard
  `Board`, `Status`, `PlayError`, and `verify_line`: the adjudicator every
  check in this chapter trusts.
- [Net tutorial](../../net-tutorial/README.md) — `nettrain`, the historical
  integration smoke's consumer.
- [Datagen tutorial](../../datagen-tutorial/README.md) — the shard and
  manifest storage discipline the corpora inherit.

**Source:**

- [`gomoku/crates/selfplay/src/bin/selfplayverify.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayverify.rs) —
  new in this chapter; quoted in full in sections 1–5.
- [`gomoku/crates/selfplay/tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs) —
  the four `selfplayverify_*` specification tests beside the three earlier
  binary tests; quoted in full in sections 6–8.
- [`gomoku/crates/selfplay/src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs),
  [`gomoku/crates/selfplay/src/opening.rs`](../../../../gomoku/crates/selfplay/src/opening.rs),
  [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs),
  [`gomoku/crates/selfplay/src/corpus.rs`](../../../../gomoku/crates/selfplay/src/corpus.rs) —
  the consumed vocabulary. Not touched in this chapter.
- [`gomoku/crates/engine/src/board.rs`](../../../../gomoku/crates/engine/src/board.rs) —
  `Board::play`, `Board::status`, `Board::to_move`, `Status`, `PlayError`.
  Not touched in this chapter.

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). https://doi.org/10.1038/nature24270 — the
  (s, π, z) record format whose stored-data contract the verifier checks,
  and the root Dirichlet noise the grading caveats discuss.
- Allis, L. V. (1994). *Searching for Solutions in Games and Artificial
  Intelligence*. PhD thesis, Maastricht University.
  https://doi.org/10.26481/dis.19940923la — threat-space search, the prover
  whose certificates `selfplayanchors` re-verifies beside the verifier.
