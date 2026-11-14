# Chapter 07 — Acceptance

## Abstract

This is the final chapter of the selfplay-corpora tutorial. Chapters
01–06 built the whole pipeline: two baseline seats play complete Swap2
games (the opening ceremony that fixes the initial stones and assigns
the colors), every game is recorded as a durable `GameRecord` with
per-ply search data and an optional certified endgame suffix, corpora
are generated at declared simulation-budget pairings with opening
lists and seed discipline, certificates are re-verified and extracted
as anchor seeds, and the read path replays records into training
samples. This chapter builds the last artifact — the `selfplayverify`
binary, which replays every recorded game through the engine, rejects
illegal moves and dishonest outcomes with named errors, and prints a
JSON quality report — and then takes the tutorial's five acceptance
gates to their recorded status and reports their measured evidence.
The conclusions, stated up
front. The verifier proves the *stored data contract* end to end —
every move legal, every outcome matching the engine, every statistic
derived from the records themselves — and it proves nothing about
playing strength; no strength claim appears anywhere in this chapter.
Four of the five gates carry measured evidence from the reference's
acceptance run: determinism (byte-identical shards across process
restarts), legality (48 of 48 games and 1 860 of 1 860 moves, every
certificate re-verified), the first corpus's quality report (about
1 400 games per hour at baseline-32), and the integration smoke
(1 860 exported samples trained for 50 `nettrain` steps, train loss
6.5483584 → 6.252371). The fifth gate — the grading gate that tests
whether simulation budget grades strength — is pre-registered in this
chapter as the design spec requires, but the reference has not
executed it at the registered scale: the larger of the two graded
measurements on record inverted the expectation (the budget-1 seat won
15 of 20 games against the budget-32 seat), and that observation
stands unresolved.
The intended reader is working the selfplay-corpora tutorial in order
and has completed chapter 06.

## Glossary

| Term | Definition |
|------|------------|
| **Acceptance gate** | One of the five repeatable checks of the design spec §5 — determinism, legality, grading, corpus, integration — that together decide whether the tutorial's corpus pipeline is accepted. |
| **Anchor seed** | `AnchorSeed { game_index, from_ply, line }`: a (position, forcing move, ±1) triple extracted from a certified suffix; the input format of the milestone-4 puzzle anchor set. |
| **Baseline-X** | The hand-written baseline evaluator behind the `mcts::Evaluator` seam, orchestrated by `player::AiPlayer`, running X search simulations per move decision. The simulation budget is the strength parameter. |
| **Certified suffix** | `CertifiedSuffix { from_ply, line }`: a terminal segment of a game in which every attacker move belongs to a machine-verified forced-win line produced by the engine's threat-space search (TSS) prover. |
| **Claim labels** | The project's provenance labels ([docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md)): **[measured]** marks numbers recorded in the reference's acceptance logs; **[experiment]** marks a free parameter or bar to be settled by measurement; **[derived]** marks a value computed from cited or measured values; **[invention]** marks a design choice of this track. |
| **Corpus** | A set of game records produced at one declared pairing, stored as shards plus a manifest. |
| **Game record** | `GameRecord`: the durable serialization of one complete game — a header (seed, seat pairing, evaluator identity, the Swap2 opening), the ordered post-opening `plies`, the `outcome`, and an optional certified suffix. Chapter 02 fixed the format. |
| **Holdout split** | The train/holdout partition of the `train` crate: a sample is held out iff its canonical position's Zobrist key modulo 100 is below 10. Content-based, so holdout metrics stay comparable across sources. |
| **Manifest** | The JSON file next to the shards: seed range, per-run configuration, game counts, and the pairing. Provenance, not data. |
| **MANUAL test** | A slow game-playing test marked `#[ignore]` with a MANUAL comment, run by explicit selection in release mode, never by the default gate. |
| **Opening offer / opening answer** | The three-stone opening one side places in the Swap2 ceremony, and the other side's reply (`OpeningAnswer::Take` a color, or `OpeningAnswer::AddTwo` with a final color choice). |
| **Outcome / Status** | `Outcome` is the record's terminal result (`Won { winner, reason }` with `WinReason::FiveOrMore` or `WinReason::Resignation(..)`, or `Draw`); `engine::Status` is the board's own terminal state (`Ongoing`, `Won(Color)`, `Draw`). The verifier compares the two. |
| **π (policy target)** | The normalized root visit-count distribution of the search at a ply; the policy training target (Silver et al. 2017). Computed from the recorded raw visit counts at read time. |
| **Pairing** | The declared simulation budgets of the two seats of a corpus: matched (X vs X) or graded (X vs Y). Recorded in every game header. |
| **Replay** | Rebuilding a position by playing a record's opening and its moves through a fresh `engine::Board`. The engine is the only position representation the verification path trusts. |
| **Shard** | A fixed-capacity file of length-delimited bincode records; the `train` crate's storage unit. |
| **Side to move** | The color whose turn it is at a position; `Board::to_move`. |
| **Swap2** | The opening protocol of this project: one side places three stones, the other chooses a color or adds two stones, and the first side then chooses a color. |
| **Threat-space search (TSS)** | A proof search over forced-win threat sequences (Allis 1994). The engine ships a bounded prover and a verifier; the prover may be incomplete, never unsound. |
| **verify_line** | `engine::verify_line(board, proof) -> bool`: the soundness net. It replays a certified line and, at every defender turn, enumerates every reply that addresses the threat; the attacker's continuation must win against all of them. Chapter 05 owns this gate. |
| **z (value target)** | The game outcome in {+1, 0, −1} from the perspective of the side to move at a ply; the value training target. Derived at read time. |
| **Zobrist key** | A 64-bit hash of a board position, maintained incrementally by the engine; used for identity, deduplication, and the holdout split. |

## Context

Six sources supply what this chapter combines. Each summary states
what the source delivered and why this chapter needs it.

- [Chapter 02 — recording games](../02-recording-games/recording-games.md) —
  built the durable format the verifier reads: `GameRecord`,
  `GameHeader` with the Swap2 `Opening` and its `OpeningProvenance`,
  `RecordedPly`, and `read_corpus`, which lists a corpus directory's
  `shard-*.bin` files and decodes every record into a
  `Vec<GameRecord>`. Reading the manifest is a separate API
  (`read_manifest`) that the verifier never calls. The verifier
  consumes exactly what the writer stored; no format change occurs
  here.
- [Chapter 03 — recording the search](../03-recording-the-search/recording-the-search.md) —
  fixed the meaning of the recorded search data: sparse raw visit
  counts, sparse top-16 priors for audit, root-value estimates, and
  the honesty rule that a move played from a certified TSS proof
  carries no search record. The verifier leaves all of it untouched;
  this chapter's context is why those fields exist.
- [Chapter 04 — diversity and grading](../04-diversity-and-grading/diversity-and-grading.md) —
  built the `selfplaygen` generator whose corpora this chapter
  verifies: tournament opening lists, the exploration schedule, seed
  discipline with deduplication, and the release-mode discipline for
  everything that plays games. Its three open observations — the
  grading probe that inverted the expectation, the rejected piskvork
  opening list, debug-mode slowness — are this chapter's inheritance.
- [Chapter 05 — certified endgames](../05-certified-endgames/certified-endgames.md) —
  owns the certificate gate: `extract_anchor_seeds` re-verifies every
  certified suffix with `engine::verify_line` (100 % must verify; the
  prover may be incomplete, never unsound), and the `selfplayanchors`
  binary runs the pass. The verifier of this chapter *counts*
  certified suffixes; the re-verification stays chapter 05's artifact,
  and the acceptance legality gate runs both binaries.
- [Chapter 06 — reading back](../06-reading-back/reading-back.md) —
  built the read path whose contracts the verifier does not re-check:
  π is the normalized recorded visit histogram, z is the outcome from
  the side to move's perspective, and a corrupt nonempty-but-zero
  histogram is a named error. Its `selfplayexport` binary and the
  `nettrain` smoke are the integration gate of this chapter.
- [Net tutorial](../../net-tutorial/README.md) — built `net` and
  `nettrain`, the integration gate's consumer: `nettrain --data`
  reads exported sample shards unmodified.

The [design spec](../../../specs/2026-11-11-selfplay-corpora-design.md)
§5 fixes the five acceptance gates; the
[primer](../00-selfplay-primer.md) §6 carries the measured evidence
and §7 the open questions. The
[hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md)
lists the measured headline numbers the chapters quote. The reference
worktree's `acceptance-evidence.md` and its `acceptance-logs/`
directory are the raw measurement record; both are temporary
provenance, deleted with the worktree once the tutorial is complete on
`main`, so this chapter quotes what must survive.

## Intention

You can prove that a corpus on disk is what it claims to be. Given a
corpus directory, one command —
`selfplayverify --corpus <DIR>` — replays every recorded opening and
every recorded move through the engine, stops with a named error on
the first illegal move or the first outcome the engine disagrees with,
and on success prints a JSON quality report: how many games, how many
plies, how long the games ran, which openings appeared, how the Swap2
answers distributed, and how the outcomes balanced. A corrupt corpus
never produces a partial report, and a healthy corpus always produces
one, byte-identical across runs. Around the binary, you can state the
tutorial's acceptance status per gate: which gates carry measured
evidence and what the numbers are, which gate is pre-registered but
not yet executed, and which observations stand unresolved. What you
deliberately cannot do after this chapter: claim anything about
playing strength — the gates prove the data contract, and the one
measurement that touched strength inverted the expectation and remains
open.

## Specification as tests

The tests live in the existing target
[`tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs)
under
[`gomoku/crates/selfplay/`](../../../../gomoku/crates/selfplay/). This
chapter specifies **exactly four** new tests, all named
`selfplayverify_*`: **all four run by default, none is ignored**. No
game-playing test is added — every test fabricates its records by hand
and drives the compiled binary through
`env!("CARGO_BIN_EXE_selfplayverify")` — so the fast gate stays fast
(the reference runs the whole target in about 0.2 s in debug) and the
release-only MANUAL discipline of chapter 04 is untouched. The binary
itself gains **no unit tests**: the four integration tests are its
whole specification, and the `selfplayverify` test target reports zero
tests of its own.

Three count notes, continued from chapter 06. First,
`tests/binary.rs` grows from the three tests chapter 05 left there
(the `selfplaygen` help contract, the `selfplayanchors` help contract,
and the anchor round trip) to **seven**; the three existing tests are
unchanged. Second, chapter 06 ended with the suite at **46 passed, 4
ignored**; this chapter's four default tests complete the track at
**50 passed, 4 ignored**. The four ignored tests are exactly chapter
04's set — the two game-playing probes in `tests/game.rs`, the
scheduled-game reproducibility test in `tests/schedule.rs`, and the
grading probe in `tests/grading.rs` — and no chapter-07 test joins
them. Third, the per-target final state of the suite is:

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

One raw-log caveat belongs with this table [measured]: the reference's
gate log shows 52 passes across 17 result lines, not 50 across 15,
because the corpus target's process helper re-runs the test binary in
two child processes and each child prints its own result line. Count
per target (deduplicated) or count raw result lines — never mix the
two methods. The same caveat applies to the workspace totals quoted in
the gates section below.

All four tests share one fabricated record, built by the
`certified_record()` helper that chapter 05's binary tests introduced
(alongside the `TestDir` temporary-directory helper and the `mv`
coordinate shorthand). No game is played. The fixture's Swap2 opening
places stones at `(5, 5)`, `(10, 10)`, `(5, 6)` with the answer "take
Black" and generated provenance; the six post-opening plies are
`(10, 11)`, `(5, 7)`, `(10, 12)`, `(5, 8)`, `(10, 13)`, `(5, 9)` —
White builds on row 10, Black on row 5, and Black's last move
completes the five `(5, 5)` through `(5, 9)` — and the outcome is a
Black win by five-or-more. A certified suffix from ply 5 carries the
one-move line `[(5, 9)]`. Every ply is unsearched (`search: None`):
the verifier never looks at search data, so the fixture needs none.
The fixture is built as a `Game` with default seat options and
converted with `GameRecord::from_game`, the same conversion the runner
uses.

The list below is ordered along the execution path of the future
program: the corpus-level rejection first (it fires before any game is
verified), then the per-ply replay rejection, then the terminal
outcome comparison, and the success report last.

### Rejecting an empty corpus

- `selfplayverify_rejects_a_corpus_without_game_records` — the test
  creates an empty directory and runs the binary with `--corpus`
  pointing at it. The process must fail, and stderr must contain `no
  game records`. A corpus with zero records is not an empty report; it
  is an error, because an empty report would be indistinguishable from
  a typo'd path that happened to be a directory.

### Rejecting an illegal recorded move

- `selfplayverify_rejects_an_illegal_recorded_move` — the fixture's
  ply 1 is doctored to `(5, 5)`, a cell the opening already occupies.
  The process must fail, and stderr must contain `ply 1` and
  `occupied`. The replay is through a real `engine::Board`, so the
  engine's own `PlayError` supplies the reason (`Cell is already
  occupied.`), and the verifier's message names the corpus position of
  the game and the ply index of the rejected move: `game 0`, `ply 1`.

### Rejecting an outcome that disagrees with the engine

- `selfplayverify_rejects_a_recorded_outcome_that_disagrees_with_the_engine` —
  the fixture's outcome is doctored to a *White* win by five-or-more;
  the moves are unchanged, so the replay still ends in Black's five.
  The process must fail, and stderr must contain `final status` and
  `game 0`. The recorded outcome is a claim about the terminal
  position; the engine replays the position and adjudicates the claim.

### Reporting replay and corpus statistics as JSON

- `selfplayverify_reports_replay_and_corpus_statistics_as_json` — the
  one-record corpus verifies cleanly, the exit status is success, and
  stdout parses as JSON. The assertions pin the report's schema field
  by field: `game_count` is `1`; `verified_plies` is `6` (the six
  post-opening plies, all replayed); `certified_suffixes` is `1` (the
  fixture carries one); `outcomes` is `{black: 1, white: 0, draw: 0}`;
  `length_histogram_plies` is `{"6": 1}` (one game of six plies — the
  histogram is a map, so JSON object keys are strings);
  `games_ending_within_16_plies` is `1` (six is at most sixteen);
  `opening_offers` is `{"Generated": 1}` (the fixture's provenance).
  The success path is a machine-readable report, not a prose summary,
  because corpus quality is something later tooling and documents
  quote.

### What the fabricated tests cannot pin

The four tests pin the failure contract and the report schema on one
hand-built six-ply record whose plies carry no search data. What they
cannot pin is everything that only a real corpus exercises: records
produced by the actual runner, whose plies carry real visit
histograms, priors, and root values that must ride through storage
untouched; real tournament-opening provenance keys in the report; real
certified suffixes in the count; outcomes of every shape — including
the 222-ply draw — that the engine must confirm at scale; and
statistics over a real distribution of game lengths. That is the
capstone's and the acceptance gates' work below: the reference's
measured run verified 48 real games and 1 860 real moves with zero
failures, and this chapter's capstone re-runs the same checks on a
corpus the learner generates.

## Walkthrough

The order is the execution order of `selfplayverify` from argument to
exit status. Exactly one source file is new —
[`src/bin/selfplayverify.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayverify.rs).
No library module is added, and
[`src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs) is
untouched: the binary consumes only existing vocabulary —
`read_corpus`, `GameRecord`, `OpeningProvenance`, `OpeningAnswer`,
`Outcome`, `WinReason` from the `selfplay` crate, and `Color`,
`Status`, and the `Board` replay methods from `engine`. No manifest
change is needed: a binary in `src/bin/` needs no entry, and the
crate's existing `serde`/`serde_json` dependencies cover the report.
The binary's private pieces, named here at first use, are
`parse_corpus` (the argument parser), `verify_record` (the per-game
check), and `Summary` with `OutcomeCounts` and
`Summary::from_records` (the report).

### 1. Parsing the command line

The vocabulary is exactly one required flag: `--corpus <DIR>`. The
binary is deliberately thinner than its siblings — `selfplaygen`,
`selfplayanchors`, `selfplayexport` all carry `--help` text and
several flags; `selfplayverify` does not. There is no `--help`
special case, no `--json` (the report is always JSON), no `--anchors`
(certificates are `selfplayanchors`' pass, chapter 05), and no
`--limit` (the verifier always reads the whole corpus; limiting work
is a generation-time decision, `--games`, not a verification-time
one). Any other flag fails with `unknown flag: <FLAG>`; a `--corpus`
without a value fails with `--corpus requires a value`; no flag at
all fails with `--corpus is required`. Every parse failure prints the
message followed by the one-line usage `Usage: selfplayverify
--corpus <DIR>` to stderr and exits with code 2. Hand-rolled parsing,
no dependency, in the style of the earlier binaries.

### 2. Reading the corpus

`read_corpus(&corpus)` lists the directory's `shard-*.bin` files,
sorts them, and decodes every record into a `Vec<GameRecord>` —
chapter 02's mechanics, unchanged. It never opens `manifest.json`:
`read_manifest` is a separate API, and `selfplayverify` does not
call it. A read failure prints `selfplayverify failed: could not
read corpus: …` and
exits with code 1. An empty record vector prints `selfplayverify
failed: corpus contains no game records` and exits with code 1 — the
first specification test. Note what the verifier reads its statistics
from: the **records**, not the manifest. The manifest is the writer's
provenance; every number in the report is derived from the decoded
games themselves, so the report cannot agree with a manifest that the
data contradicts.

One honesty boundary belongs here, stated plainly. The completed
reference performs **no manifest-versus-shard cross-check**: because
the verifier never opens `manifest.json`, a manifest that disagrees
with its shards — a wrong game count, a wrong shard list, a forged
pairing — passes verification unnoticed, and the report, derived
entirely from the records, simply describes the data that is there.
Chapter 02's honesty boundary deferred this cross-check to the
acceptance chapter alongside the replay checks; that deferral was
wrong about this one item, and this chapter corrects it. What the
completed reference does implement: the move-legality and
outcome-agreement replay of steps 3 and 4 below. What stays where
earlier chapters put it: certified-suffix re-verification remains
`selfplayanchors`' pass (chapter 05), and the π/z contracts remain
the read path's (chapter 06). The manifest cross-check is implemented
nowhere in the tutorial; it stands as an open hardening gap, recorded
here and in chapter 02 rather than papered over.

### 3. One record at a time: replay

`verify_record(game_index, record)` rebuilds each game from primary
evidence. `record.header.opening.board()` replays the Swap2 ceremony
into a fresh `engine::Board` (a rejection is `game <N>: opening
replay failed: …`), and then every recorded move is played in order:
`board.play(recorded.mv)`. The first rejection stops the run with
`game <N>: move <MOVE> at ply <PLY> was rejected: …` — the error the
second specification test pins, with the engine's `PlayError` text
(`Cell is already occupied.`) carrying the reason. `game_index` is
the record's position in corpus order — the same indexing the
`AnchorSeed`s of chapter 05 use — and `ply` counts post-opening plies.
Failure handling is all-or-nothing: the run stops at the first failing
game, and no partial report exists.

### 4. The recorded outcome must match the engine

After the last ply, the board's own `status()` adjudicates the
recorded `outcome`. Three rules, one per outcome shape:

- `Won { winner, reason: FiveOrMore }` requires
  `status == Status::Won(winner)` — the engine must see the same
  five-or-more on the board.
- `Won { winner, reason: Resignation(..) }` requires
  `status == Status::Ongoing` and `board.to_move() ==
  winner.other()` — a resignation truncates the game with the *loser*
  to move, so the replayed board must still be open with the losing
  side's turn pending. The resignation reason itself is not replayed;
  it is a decision record, not a board state.
- `Draw` requires `status == Status::Draw` — the board full, no
  winner.

Any disagreement fails with `game <N>: final status <STATUS> does not
match recorded outcome <OUTCOME>` — the message the third
specification test pins. This is the honesty check the whole format
depends on: z, the value target, is *derived* from the recorded
outcome at read time (chapter 06), so a recorded outcome the engine
contradicts would poison every value target derived from it.

### 5. The summary: fields and semantics

When every record verifies, `Summary::from_records` derives the
report. The fields, in the report's order:

- `game_count` — the number of records.
- `verified_plies` — the total number of post-opening plies across all
  records. Because the summary is only computed after every record
  verified, every counted ply is a verified ply; on the 48-game corpus
  this is where the "1 860 of 1 860 moves legal" figure comes from.
  The opening placements themselves are replayed (step 3) but not
  counted here — lengths are post-opening lengths.
- `certified_suffixes` — the number of records that *carry* a
  certified suffix. Presence is counted, not re-verified: the
  `verify_line` re-verification is `selfplayanchors`' pass (chapter
  05), and the legality gate below runs both binaries.
- `games_ending_within_16_plies` — the count of games with at most 16
  post-opening plies; the tactical short tail the corpus gate reports
  on.
- `min_plies`, `max_plies`, `mean_plies`, `median_plies` — the length
  summary over post-opening plies. The median of an even count is the
  mean of the two middle values, so it can end in `.5`.
- `length_histogram_plies` — the exact length frequencies as a map
  from ply count to game count (a `BTreeMap`, so keys are sorted; in
  JSON they are object keys, hence strings). Exact frequencies, not
  bins.
- `opening_offers` — one counter per opening offer, keyed by
  provenance: `Generated` for a generated opening, or
  `<list>#<index>` for a listed one, where `<list>` is the opening
  list's path as recorded in the header (for example
  `data/openings/gomocup-2024-freestyle15.txt#4`). This is where
  opening diversity is read off.
- `opening_answers` — one counter per Swap2 answer shape:
  `take_black`, `take_white`, `add_two_take_black`,
  `add_two_take_white`.
- `outcomes` — `{black, white, draw}`: the recorded outcome balance.
  Every counted outcome passed step 4's engine comparison first.

### 6. Report and exit status

On success the summary is serialized with `serde_json`'s pretty
printer and printed to **stdout**; the process exits with code 0. A
serialization failure would print `selfplayverify failed: could not
serialize summary: …` and exit 1. The exit-code discipline is the
binary's contract in one table: 0 means verified with a report on
stdout; 1 means a read, empty-corpus, replay, or outcome failure with
a named error on stderr; 2 means command-line misuse with the usage
line on stderr. Because the corpus is fixed input and every map in
the report is a `BTreeMap`, the report itself is deterministic: two
runs over the same corpus print byte-identical JSON [measured — this
chapter's capstone demonstrates it].

### What the verifier deliberately does not check

Three absences are design, not oversight. First, certified suffixes
are counted, not re-verified: re-verification with
`engine::verify_line` is chapter 05's `selfplayanchors` pass, and
splitting the two keeps each binary single-purpose — the acceptance
legality gate runs both. Second, the recorded search data — the visit
histograms, the sparse priors, the root values — is never inspected:
it is audit and training material, not legality material. The π
contract (a nonempty histogram must have positive total mass; an empty
one means no search ran) lives in the read path, where chapter 06's
tests pin it — a corrupt histogram fails with the named
`SampleError::ZeroVisitTotal` — and the integration gate exercises it
end to end: the export replayed all 1 860 plies of the first corpus
into samples, so a contract violation anywhere in the corpus would
have failed the export. Third, there is no strength claim anywhere:
the verifier proves that the stored data is what it claims to be, and
the quality report describes the corpus. Whether the *player* is
strong is the grading gate's question, and that gate's measurement is
the unresolved observation of this chapter.

## The acceptance gates

The design spec §5 fixes five gates. Each subsection below states the
gate, its status, and its evidence. The measured numbers are the
reference's acceptance run of 2026-11-11, recorded in the reference
worktree's `acceptance-evidence.md` and `acceptance-logs/` — raw
measurement records, temporary provenance. The learner's fresh
commands are this chapter's verification section and capstone; the
two are never mixed.

### Gate 1 — determinism [measured]

Same seed, byte-identical game record, across process restarts. Two
separate release invocations of `selfplaygen` (3 games, baseline-8 vs
baseline-8, seed base 424242, the three committed Gomocup freestyle
lists) produced byte-identical shards — SHA-256
`4388d84e4ce5f3c2a03017adc49df11d5aabba841c60413b1e9120a70812ae52`
for both `shard-000.bin` files — and byte-identical manifests; both
`cmp` comparisons exited 0. Both runs accepted seeds 424242–424244 and
wrote one shard each; wall times 2.79 s and 2.80 s. The gate also has
a permanent default-suite anchor:
`identical_seeds_produce_byte_identical_corpora_across_process_restarts`
in `tests/corpus.rs` (chapter 02) runs by default. **Status: met.**

### Gate 2 — legality and certificates [measured]

Every recorded game replays through the engine without error; every
certified suffix verifies, 100 %. On the 48-game first corpus
(`data/selfplay/v0-b32/` in the reference worktree), the release
verifier ran in 0.19 s and reported `game_count: 48`,
`verified_plies: 1860` — 48 of 48 games and 1 860 of 1 860
post-opening moves legal — and every recorded outcome matched the
replayed engine status. The certificate pass ran separately:
`selfplayanchors --corpus data/selfplay/v0-b32 --out
data/selfplay/v0-b32-anchors.json` re-verified every certified line
with `engine::verify_line` and reported `Wrote 47 verified anchor
seeds` in 0.14 s — 47 certificates from 47 distinct games, 47 of 47
verified (100 %). Every certified line in this corpus was one ply
long. The sole uncertified game was the 222-ply draw; its absence of a
certificate emitted no anchor data and no negative label — chapter
05's honesty rule. The graded probe corpus (below) replayed cleanly
too: 20 of 20 games, 729 post-opening plies, 20 certified suffixes
present. **Status: met.**

### Gate 3 — grading [pre-registered here; not yet executed]

The gate validates the ladder's load-bearing assumption — that the
simulation budget grades strength — which the later promotion
criterion ("the candidate beats the baseline at an equal simulation
budget") depends on. The design spec leaves the bar to this chapter
[experiment]; the pre-registration is:

- **Pairing:** baseline-800 vs baseline-200.
- **Games:** 50, in two `selfplaygen` runs of 25. The second run swaps
  the budgets across the colors (`--black-sims 200 --white-sims 800`
  after `--black-sims 800 --white-sims 200`), because the binary
  declares budgets per color and no color-swapping exists — the swap
  removes the fixed-color confound of the probe below.
- **Seeds:** one declared seed base per run; the manifests record the
  accepted seeds.
- **Mode:** release; the exploration schedule is unchanged, because it
  is part of the matched conditions.
- **Bar** [experiment]: the higher-budget seat wins at least 30 of the
  50 games; draws count as won by neither side. The number is a
  starting bar, not a research result — a 50-game sample cannot
  separate small strength differences, and the bar exists to detect a
  large effect or its absence.
- **Cost** [derived]: at the measured 2.56 s per baseline-32 game and
  roughly linear scaling in the budget, a 800-vs-200 game costs on the
  order of half a minute single-threaded; 50 games are tens of minutes
  on one process, minutes across a dozen parallel processes (one seed
  base per invocation, per chapter 04's process-parallel discipline).

**Status: registered, not executed.** The reference's only graded
measurements are the report-only probe and the chapter-04 smoke, and
the probe *inverted* the expectation: 20 games at baseline-32 (Black)
vs baseline-1 (White), seeds 200000–200019, generated in 40.82 s —
the budget-1 seat won 15 of 20 (75 %), no draws. The caveats are real
and stated: the sample is small; the exploration schedule (temperature
1 with root Dirichlet noise for the first 10 plies) handicaps the
stronger seat, because noise perturbs exactly the seat whose search
would otherwise exploit the weak opponent; and the probe used fixed
colors. None of these caveats resolves the observation, and this
chapter offers no resolution and no strength conclusion. The chapter-04
slice added a second, smaller anomaly: the reference's pinned comment
in `tests/grading.rs` records "four short games, all won by
baseline-16", while the slice's fresh release run of the same seeds
measured two stronger-seat wins, one weaker-seat win, and one draw —
the direction assertion passed, but the per-seed detail did not
reproduce the comment. That divergence is likewise reported, not
resolved. A registered gate whose premise is under suspicion is run
deliberately, not silently: executing the 50-game protocol above is
the owner's follow-up work, and whatever it measures is a finding
about the ladder premise, not a defect of this tutorial.

### Gate 4 — the first corpus [measured]

Build the first corpus at a declared pairing and report the numbers.
The measured corpus: 48 games, baseline-32 vs baseline-32, seeds
100000–100047, the three committed Gomocup freestyle lists, shard
capacity 16, one release process. Wall time 122.68 s — **1 408.54
games/hour**, mean 2.56 s per game — writing three shards, 200 KiB on
disk. All 48 base seeds were accepted with no derived replacement
seeds, from which one fact is *inferred*, accurately labeled: the
Zobrist-walk deduplication of chapter 04 never fired on this corpus,
because a rejected duplicate would have been re-rolled under a derived
seed and the manifest records the accepted seed set.

The quality report, from the verifier's JSON
(`acceptance-logs/v0-b32-verified.json`):

- **Outcomes:** Black 23 (47.92 %), White 24 (50.00 %), draw 1
  (2.08 %). No outcome-balance bar was pre-registered; the first
  corpus establishes the baseline.
- **Game length** (post-opening plies; the 3-or-5 Swap2 placements are
  not included): min 7, median 27.0, mean 38.75, max 222 — the longest
  game was the only draw. 10 of 48 games (20.83 %) ended within 16
  plies: a clear short tactical tail, but 38 of 48 (79.17 %) lasted
  longer, so this small corpus does not support saying that *most*
  games end inside the ~16-ply prediction. The exact frequencies:

  | Plies | Games | Plies | Games | Plies | Games |
  |---:|---:|---:|---:|---:|---:|
  | 7 | 2 | 24 | 3 | 50 | 1 |
  | 8 | 1 | 25 | 2 | 54 | 1 |
  | 9 | 1 | 26 | 2 | 56 | 1 |
  | 10 | 1 | 28 | 1 | 60 | 1 |
  | 12 | 2 | 29 | 2 | 61 | 2 |
  | 14 | 1 | 33 | 1 | 75 | 1 |
  | 15 | 1 | 35 | 3 | 82 | 1 |
  | 16 | 1 | 36 | 2 | 87 | 1 |
  | 18 | 1 | 37 | 1 | 102 | 2 |
  | 19 | 2 | 42 | 1 | 222 | 1 |
  | 21 | 3 | 47 | 1 | | |
  | 23 | 1 | | | | |

- **Opening diversity:** 28 of the 36 offers in the pool appeared; the
  most frequent single offer appeared 4 of 48 times (8.33 %). List
  totals: 2024: 23, 2025: 15, 2026: 10 games. Full offer-index
  counts, including zeros:

  ```text
  2024: #0=1 #1=3 #2=1 #3=1 #4=4 #5=1 #6=3 #7=1 #8=3 #9=1 #10=2 #11=2
  2025: #0=1 #1=0 #2=1 #3=1 #4=1 #5=2 #6=2 #7=2 #8=1 #9=2 #10=1 #11=1
  2026: #0=0 #1=0 #2=0 #3=4 #4=0 #5=0 #6=0 #7=1 #8=2 #9=1 #10=0 #11=2
  ```

- **Swap2 answers:** take Black 20, take White 17, add two with final
  choice Black 6, add two with final choice White 5.
- **Certified suffixes:** 47 of 48 games carry one (97.92 %), all
  re-verified under gate 2.

**Status: met** (the gate is a report; no bars beyond generation and
verification were pre-registered).

### Gate 5 — integration [measured]

`nettrain` reads a game-record shard set and trains on it. Measured as
a smoke test: `selfplayexport --corpus data/selfplay/v0-b32 --out
data/selfplay/v0-b32-export --shard-capacity 512` materialized **1 860
samples from the 48 games into 4 shards** in 0.18 s, and the
exporter's own read-back through `train::dataset` matched the
manifest's count. Then the unmodified `nettrain` trained the tiny
configuration (8 channels, 1 residual block, batch size 16) on the
export for 50 of 50 steps in 0.47 s and wrote a checkpoint. The
dataset split into 1 665 train and 195 holdout samples — the Zobrist
rule over the canonical export, 10.5 % held out. Train loss moved from
**6.5483584 at step 1 to 6.252371 at step 50** (`metrics.jsonl`;
step-50 holdout loss 6.1449027). This proves the net consumed
game-derived sample shards end to end. The loss values are smoke-run
evidence only, not a model-quality assessment, and not a
playing-strength claim of any kind. **Status: met, as a smoke.**

### The final project gates [measured]

The reference's closing gate run (2026-11-11, from `gomoku/`):

| Gate | Result | Wall time |
|---|---|---:|
| `cargo test -p selfplay` | 50 passed, 0 failed, 4 ignored, across 15 top-level test/doc suites | 19.73 s (debug) |
| `cargo test --workspace` | 852 passed, 0 failed, 11 ignored, across 63 unique suites | 56.36 s |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0, no warnings | 0.38 s |
| `cargo fmt --all` | exit 0 | — |

Counting discipline, stated once and never mixed: per-target
(deduplicated) counts are 50/4 for the crate and 852/11 across 63
suites for the workspace; raw result-line counts are 52 across 17
lines for the crate and 854 across 65 lines for the workspace, the two
extra lines being the corpus target's child-process reports. This
chapter quotes deduplicated counts everywhere except this sentence.

## Open observations — reported, not resolved

Three observations stand at the end of the track. None is resolved
here; none is hidden.

### The grading inversion stands

Gate 3 carries the details. In short: the report-only 20-game probe
had the budget-1 seat beat the budget-32 seat 15–5; the chapter-04
slice's fresh run of the four pinned grading seeds produced per-seed
outcomes the reference's comment does not describe; neither is a
strength conclusion, and the pre-registered 50-game protocol is the
deliberate next measurement. The tutorial's answer to an inverted
expectation is a registered gate and an honest report, not an
explanation.

### The piskvork list stays rejected

The reference's first acceptance-generation command included all four
committed opening lists and produced no games: it failed fast in
0.16 s, because the opening loader rejects
[`data/openings/piskvork-openings.txt`](../../../../data/openings/piskvork-openings.txt)
on line 41 — the coordinate `-8,-6` is outside the 15×15 board. The
rejection is correct parser behavior (chapter 04's malformed-line
tests pin it); the operational consequence is that the working pool is
the three committed Gomocup freestyle lists, 36 offers, which produced
every measured number in this chapter. Whether and how the piskvork
list joins the pool — corrected, filtered, or replaced — remains open,
exactly as chapter 04 left it.

### A fixture flake, observed while verifying this chapter

One fresh observation belongs to this chapter's own verification
[measured 2026-11-14, this writing session]. In one of several
repeated runs of `cargo test -p selfplay --test binary` against the
reference, two tests failed together: one with `File exists` while
writing its corpus, another with `No such file or directory` while
reading its corpus back; every re-run was green. The failures were not
instrumented, so the mechanism is not established fact; the
explanation consistent with both errors is in the test fixture, not
the verifier: `tests/binary.rs`'s `TestDir`
names its directory `selfplayanchors-test-<pid>-<nanos>` (the prefix
is historical — chapter 05's anchor tests introduced the helper), and
parallel tests share the process id, so two tests that draw the same
clock nanoseconds would share one directory — one test's `Drop` would
then delete the other's directory mid-run, a plausible collision that
matches the observed pair of errors. (`tests/corpus.rs`'s `TestDir`
takes a per-test label and is not affected.) The counts this chapter
quotes come from green runs. If the flake ever bites during the
learner's gate, the hardening is a per-test label in the directory
name — a fixture change, never a verifier change; no such fix has
landed.

## What this chapter defers

- **Executing the pre-registered grading gate** — deliberate follow-up
  work, after the inversion has been explored; the registration above
  is the chapter's deliverable, not the run.
- **Color-swap support in `selfplaygen`** — budgets are declared per
  color; the registered gate works around it with two swapped runs. A
  generator flag is future work, deliberately not built here.
- **The piskvork opening list** — the Gomocup lists are the working
  pool; extending the pool is later work (chapter 04's observation
  stands).
- **Consuming the anchor seeds and the record format** — the
  milestone-4 anchor set and the milestone-5 replay buffer and
  self-play service are other milestones' work; this track delivered
  their input formats, verified.
- **Any playing-strength conclusion** — the first strength signal on a
  trained artifact comes from candidate-vs-baseline ladder matches in
  the AlphaZero loop, starting low (design spec S7).

## TDD checklist

Work one artifact at a time, red to green, in
[`gomoku/crates/selfplay/`](../../../../gomoku/crates/selfplay/). No
manifest changes: the crate already depends on `engine`, `serde`, and
`serde_json`, and a binary in `src/bin/` needs no manifest entry. No
other crate is touched; no library module is added.

1. The tests
   [`tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs):
   the four `selfplayverify_*` tests in the specification's order —
   empty corpus, illegal move, outcome disagreement, JSON report —
   reusing chapter 05's `TestDir`, `mv`, and `certified_record`
   helpers unchanged. Red: until
   [`src/bin/selfplayverify.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayverify.rs)
   exists, the target does not even compile
   (`env!("CARGO_BIN_EXE_selfplayverify")` has no binary to name); a
   stub binary with an empty `main` turns the compile error into four
   honest test failures.
2. The binary's front half: `parse_corpus` with its three parse
   errors and the usage line (exit 2), the `read_corpus` call with its
   read error, and the empty-corpus rejection (both exit 1). Green:
   `selfplayverify_rejects_a_corpus_without_game_records`.
3. `verify_record`: the opening replay and the per-ply replay with
   their named errors. Green:
   `selfplayverify_rejects_an_illegal_recorded_move`.
4. The outcome comparison: the three shape rules, including the
   resignation rule (open board, loser to move). Green:
   `selfplayverify_rejects_a_recorded_outcome_that_disagrees_with_the_engine`.
5. `Summary`, `OutcomeCounts`, and `Summary::from_records` with the
   exact field set of the walkthrough, serialized pretty to stdout.
   Green:
   `selfplayverify_reports_replay_and_corpus_statistics_as_json`.
6. Regression: every other target stays green unchanged — the chapter
   only reads what earlier chapters wrote — the ignored set stays at
   exactly chapter 04's four MANUAL tests, and the new binary
   contributes zero unit tests.

## Capstone

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

Four boundary experiments complete the picture. Verify an empty
directory (`mkdir "$dir/empty"`, then point `--corpus` at it): the run
fails with `corpus contains no game records` and exit code 1 — an
empty corpus is an error, never an empty report. Pass a flag the
binary does not know, for example `--json`: the run fails with
`unknown flag: --json`, prints the one-line usage, and exits with
code 2 — the report needs no opt-in flag, because JSON is all it
prints. Re-run the extractor with the same `--out`: the run fails with
`failed to create '…': File exists (os error 17)` — an extraction
never overwrites a seed set by accident. And run the verifier twice
into two files and compare them: the report is byte-identical, because
the corpus is fixed input and every map in it is sorted. None of these
commands writes outside `$dir`; the verifier never writes at all.

## Done when

- `cargo test -p selfplay --release` reports **50 passed, 4 ignored**:
  the chapter-06 targets keep their state (the `src` unit tests 6/0 —
  two in `game.rs`, four in `selfplaygen.rs` — corpus 10/0, openings
  5/0, schedule 1/1, dedup 1/0, grading 0/1, anchors 4/0, read_path
  11/0), and this chapter brings `tests/binary.rs` to 7/0 with exactly
  the four `selfplayverify_*` tests. The deduplicated per-target count
  is the contract; a raw log will show two extra corpus child-process
  result lines (52 across 17), and the two counting methods are never
  mixed. The four ignored tests are exactly chapter 04's MANUAL set;
  they are not run by the default gate, and this chapter adds none.
- `tests/binary.rs`'s three earlier tests are unchanged, and the new
  binary target contributes zero unit tests.
- No new MANUAL test exists, and nothing in this chapter's tests plays
  a game: every test fabricates its records and drives the compiled
  binary (the reference runs the target in about 0.2 s in debug). The
  release-only, game-playing verification discipline of chapter 04 is
  unchanged.
- `cargo test -p train` and `cargo test -p player` are not required:
  both crates are untouched.
- `cargo clippy --all-targets -- -D warnings` is green.
- `cargo fmt --all -- --check` makes no changes.
- The acceptance gates are documented in this chapter with their
  measured evidence attributed to the reference's logs, and the
  grading gate is pre-registered with its non-execution stated.
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

The first command is the final default gate of the track, run in
release per the game-playing discipline of chapter 04 — the default
suite plays its small budget-1 and budget-2 games, and release keeps
the gate at seconds. The reference's historical gate log ran the same
default suite in debug (19.73 s, 50 passed / 4 ignored); the
deduplicated counts are identical in either mode. The four ignored
MANUAL tests are not run by this gate and are not silently run
anywhere: they execute only by explicit selection (`-- --ignored`),
release-only, as deliberate measurements. The second command runs the
seven binary tests in isolation; the third selects this chapter's four
by name. If the workspace-wide gate is wanted, `cargo test
--workspace` is the reference's closing command (852 passed / 11
ignored across 63 unique suites, deduplicated; the raw log shows 854
across 65 result lines — same corpus child-process caveat, never mix
the methods). The capstone above is the fresh end-to-end evidence:
generate, verify, extract, all inside a temporary directory.

## References

**This track:**

- [Tutorial README](../README.md) — conventions, gates, the chapter
  map, and the reference provenance.
- [Self-play primer](../00-selfplay-primer.md) — §6 the five
  acceptance gates with the measured headline numbers; §7 the open
  questions; §8 the invariants this chapter's gates enforce.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md) —
  the `GameRecord` format, `read_corpus`, and the cross-process
  determinism test that anchors gate 1.
- [Chapter 03 — recording the search](../03-recording-the-search/recording-the-search.md) —
  what the recorded search data means, and why the verifier leaves it
  untouched.
- [Chapter 04 — diversity and grading](../04-diversity-and-grading/diversity-and-grading.md) —
  `selfplaygen`, the opening lists, the exploration schedule, seed
  discipline and dedup, the release-mode discipline, and the grading
  smoke whose anomalies this chapter inherits.
- [Chapter 05 — certified endgames](../05-certified-endgames/certified-endgames.md) —
  the certified suffix, the `verify_line` soundness gate,
  `extract_anchor_seeds`, and the `selfplayanchors` binary this
  chapter's legality gate runs beside the verifier.
- [Chapter 06 — reading back](../06-reading-back/reading-back.md) —
  the π and z contracts the verifier does not re-check, the
  `selfplayexport` seam, and the integration smoke this chapter
  records as gate 5.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  §5 the five acceptance gates; S7 the retired mock-strength probe.
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  the measured acceptance evidence the gates quote and the open
  questions carried honestly.
- [docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) —
  §12 the soundness gate, the determinism policy, and the claim-label
  convention this chapter's glossary defines.
- [docs/14-openings-and-supervised-curriculum.md](../../../14-openings-and-supervised-curriculum.md) —
  §3 the Swap2 procedure and the tournament opening lists; decision D6
  the training mixture the verified corpora feed.
- `acceptance-evidence.md` and `acceptance-logs/` in the reference
  worktree (`rust-ml-selfplay-ref`, branch `selfplay-ref-v2`) — the
  raw measurement record behind every [measured] label in this
  chapter; temporary provenance, deleted with the worktree once the
  tutorial is complete on `main`.

**Prerequisite tracks:**

- [Engine tutorial](../../13-engine-tutorial/README.md) — the
  bitboard `Board`, `Status`, `PlayError`, and `verify_line`: the
  adjudicator every check in this chapter trusts.
- [Net tutorial](../../net-tutorial/README.md) — `nettrain`, the
  integration gate's consumer, and the acceptance-chapter precedent
  for measured-evidence discipline.
- [Datagen tutorial](../../datagen-tutorial/README.md) — the shard
  and manifest storage discipline the corpora inherit.

**Source:**

- [`gomoku/crates/selfplay/src/bin/selfplayverify.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayverify.rs) —
  new: the verifier binary. Created in this chapter.
- [`gomoku/crates/selfplay/tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs) —
  the four `selfplayverify_*` specification tests, beside the three
  existing binary tests. Touched in this chapter.
- [`gomoku/crates/selfplay/src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs),
  [`gomoku/crates/selfplay/src/opening.rs`](../../../../gomoku/crates/selfplay/src/opening.rs),
  [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs),
  [`gomoku/crates/selfplay/src/corpus.rs`](../../../../gomoku/crates/selfplay/src/corpus.rs) —
  the consumed vocabulary: `GameRecord` / `GameHeader` /
  `CertifiedSuffix`, `Opening::board` / `OpeningProvenance` /
  `OpeningAnswer`, `Outcome` / `WinReason`, `read_corpus`. Not touched
  in this chapter.
- [`gomoku/crates/selfplay/src/anchor.rs`](../../../../gomoku/crates/selfplay/src/anchor.rs)
  and
  [`gomoku/crates/selfplay/src/bin/selfplayanchors.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayanchors.rs) —
  the certificate pass the legality gate runs beside the verifier. Not
  touched in this chapter.
- [`gomoku/crates/engine/src/board.rs`](../../../../gomoku/crates/engine/src/board.rs) —
  `Board::play`, `Board::status`, `Board::to_move`, `Status`,
  `PlayError`. Not touched in this chapter.

**Companion papers:**

- [acceptance-design.md](acceptance-design.md) *(planned)* — the
  conceptual story: acceptance proves the stored data contract, not
  the player.
- [acceptance-implementation.md](acceptance-implementation.md)
  *(planned)* — the hiding layer: low-level construction instructions,
  signatures in prose, method bodies never.
- [acceptance-solution.md](acceptance-solution.md) *(planned)* — the
  annotated reference snippets.

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). https://doi.org/10.1038/nature24270 —
  the (s, π, z) record format whose stored-data contract this chapter
  verifies, and the root Dirichlet noise of the exploration schedule
  the grading caveats discuss.
- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://doi.org/10.26481/dis.19940923la — threat-space search, the
  prover whose certificates the legality gate re-verifies.
