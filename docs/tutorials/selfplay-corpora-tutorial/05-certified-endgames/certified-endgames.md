# Chapter 05 — Certified endgames

## Abstract

This chapter turns the certified endgames that self-play already
produces into a machine-checked anchor set for milestone 4. Each
seat's own-side threat-space search (TSS) probe has certified forced
wins since the baseline tutorial, but the proof vanished with the
decision that used it. The chapter threads that evidence through
three seams. First, the `player` crate's `Decision` gains
`own_proof: Option<engine::Proof>`: the certified own-proof bypass
retains the exact verified line that selected its move, while every
ordinary search path and every resignation carries `None`; move
selection does not change. Second, the game runner gains a
`SuffixTracker` that begins tracking when a decision carries a proof
and observes every played move; a game that follows a certified line
to completion records a `CertifiedSuffix` — the record slot chapter
02 created and left `None`. Third, `extract_anchor_seeds` replays
every certified record through the engine, re-verifies each stored
line with `engine::verify_line`, checks that the certified line is
the line actually played, and emits one `AnchorSeed` per certificate:
replay instructions from which the milestone-4 (position, forcing
move, ±1) triple derives. The `selfplayanchors` binary runs the pass
over a corpus and writes the seeds as JSON. The conclusions: every
emitted line has passed the soundness gate twice — once inside the
prover at decision time and once at extraction — so the anchor set
contains only machine-checked forced wins; a doctored or mismatched
certificate is a hard error that aborts the whole extraction, never a
silent skip; and a game without a certificate contributes no anchor
data and no negative label, because an unproven position is unknown,
not lost. The reference's measured corpus carried a certified suffix
in 47 of 48 games, and 47 of 47 certificates re-verified (100 %).
What this chapter deliberately does not do: it does not read corpora
back into training samples, mix sources, or run the acceptance gates
— chapters 06 and 07. The intended reader is working the
selfplay-corpora tutorial in order and has completed chapter 04.

## Glossary

| Term | Definition |
|------|------------|
| **Threat-space search (TSS)** | A proof search over forced-win threat sequences (Allis 1994). The engine ships a bounded prover (`prove_forced_win`, `prove_shortest_forced_win`) and a verifier (`verify_line`); the prover may be incomplete, never unsound. |
| **Proof** | `engine::Proof`: a certified forced win — the winning side (`winner`) and one forcing sequence (`line`). The line alternates attacker and defender moves, starts with the attacker's first threat, and ends when the attacker completes five (overlines count) or creates an unblockable double threat. |
| **Attacker / defender** | The two roles inside a proof line. The attacker is the side with the forced win (`proof.winner`); the defender is its opponent. Attacker moves sit at even offsets of the line, defender replies at odd offsets. |
| **verify_line** | `engine::verify_line(board, proof) -> bool`: the soundness net. It replays the line from the board and, at every defender turn, enumerates every reply that addresses the threat; the attacker's continuation must still win against all of them. `true` only if the proof is airtight. |
| **Certified suffix** | `CertifiedSuffix { from_ply: u16, line: Vec<Move> }`: a terminal segment of a game in which every played move belongs to one machine-verified forced-win line. `from_ply` is the zero-based post-opening ply of the attacker's first certified move. |
| **Own-side TSS probe** | The `AiPlayer::decide` step that runs `prove_shortest_forced_win` for the side to move before any search (baseline preset: 10 000 nodes / 30 plies). A certified proof short-circuits the decision: the line's first move is played and no search runs. |
| **own_proof** | The `Decision` field this chapter adds: `Option<engine::Proof>`, `Some` exactly when the own-side probe certified the played move. |
| **SuffixTracker** | The runner's private detector: it begins a pending certificate when a decision carries an own proof and observes every played move against the proof's line; a departure discards the pending certificate, and a completed line becomes the game's certified suffix. |
| **Anchor seed** | `AnchorSeed { game_index, from_ply, line }`: replay instructions for one milestone-4 anchor position. Replay the source game's opening and its first `from_ply` post-opening moves; at that position `line[0]` is the forcing move and the verified value is +1 for the side to move (−1 for its opponent). |
| **Soundness gate** | The project's rule (docs/12 §12): every certificate consumed downstream must pass `verify_line`; 100 % must verify. The prover may be incomplete, never unsound. |
| **VCF** | Victory by continuous forcing threats: a forced-win line in which every attacker move creates a threat the defender must answer. The chapter's unit-test fixture is a VCF win-in-3. |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games. This chapter adds none. |
| **Claim labels** | The project's provenance labels (chapter 12): **[paper]** (stated in a cited paper), **[derived]** (computed from cited or measured values), **[experiment]** (a free parameter, to be settled by measurement). |

## Context

Five sources supply what this chapter combines. Each summary states
what the source delivered and why this chapter needs it.

- [Chapter 01 — the game runner](../01-the-game-runner/the-game-runner.md) —
  built the move loop this chapter instruments: one `StdRng` stream
  per game, two persistent `AiPlayer` seats, and one
  `seat.decide(&board, &effective_options, &mut rng)` call per ply
  whose `Decision` yields the played move. The loop is where the
  proof evidence surfaces, and the loop is where this chapter
  captures it.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md) —
  created the `CertifiedSuffix` type, the `Game.certified_suffix`
  slot (set to `None` by the runner until now), and the matching
  `GameRecord.certified_suffix` slot that `GameRecord::from_game`
  copies verbatim, with `game_record_preserves_a_certified_suffix`
  already pinning the copy. This chapter fills the slot; the record
  format does not change.
- [Chapter 03 — recording the search](../03-recording-the-search/recording-the-search.md) —
  fixed the honesty rule for certified moves: a move played from a
  certified own-side proof carries `search: None`, because no search
  ran. That rule is why the proof itself is the only evidence such a
  ply can offer — and why retaining it matters.
- [Chapter 04 — diversity and grading](../04-diversity-and-grading/diversity-and-grading.md) —
  landed the corpus generator: `selfplaygen` configures both seats
  with the baseline own-side TSS probe, so every generated game runs
  the prover whose output this chapter collects. The measured corpus
  this chapter quotes came from that binary.
- [Baseline tutorial](../../baseline-tutorial/README.md) and
  [engine tutorial](../../13-engine-tutorial/README.md) — the two
  halves of the certificate. The baseline tutorial built
  `AiPlayer::decide` and its own-side probe; the engine tutorial
  built the TSS prover and `verify_line`, including the rule that a
  `None` from the prover means "not proven within budget" and says
  nothing about the true value of the position.

The [primer](../00-selfplay-primer.md) §4.5 states this chapter's
contract: when the probe fires and the forced line is then played
out, the record marks the ply from which the line is certified; an
extraction pass re-verifies every certified line with `verify_line`
and emits anchor seeds. The
[design spec](../../../specs/2026-11-11-selfplay-corpora-design.md)
§3.4 is the design authority: certified suffixes feed the milestone-4
anchor set as on-distribution puzzle roots, and absence of a
certificate is "no anchor data", not a negative label.

## Intention

You can turn finished self-play games into a milestone-4 anchor set
you can trust without trusting the generator. One command —
`selfplayanchors --corpus <DIR> --out <JSON>` — re-reads a corpus,
and every certificate inside it must survive a fresh, independent
replay: the opening executes, the pre-certificate moves are legal,
the stored line passes `engine::verify_line` from the replayed
position, and the line is the line the game actually played. Each
surviving certificate becomes one anchor seed: replay the game to
`from_ply`, and the position there is a puzzle root whose forcing
move is `line[0]` and whose verified value is +1 for the side to
move. The trust comes from the boundary, not from the pipeline that
produced the records: a tampered record does not get skipped, it
aborts the run with a named error. And the set is honest about its
own coverage — a game whose endgame no probe certified contributes
nothing, and nothing is what unknown means.

## Specification as tests

The tests live in three targets under
[`gomoku/crates/selfplay/`](../../../../gomoku/crates/selfplay/) —
the unit tests inside
[`src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs), the
new target
[`tests/anchors.rs`](../../../../gomoku/crates/selfplay/tests/anchors.rs),
and
[`tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs)
— plus one strengthened assertion in
[`gomoku/crates/player/tests/decisions.rs`](../../../../gomoku/crates/player/tests/decisions.rs).
The list below is ordered along the execution path of the feature:
the proof is retained where it is produced (the seat's decision),
tracked where the moves are played (the runner), re-verified where
the corpus is read (extraction), and driven by the binary last.

This chapter specifies **eight** new tests in the `selfplay` crate:
**all eight run by default, none is ignored**. No game-playing test
is added — the anchor tests fabricate their records, and the tracker
unit tests run one TSS probe each on a small fixture — so the fast
gate stays fast and the release-only MANUAL discipline of chapter 04
is untouched. The per-target counts:

| Target | Default | Ignored | Chapter-05 tests |
|---|---:|---:|---:|
| `tests/anchors.rs` | 4 | 0 | all four (new target) |
| `src/game.rs` (unit) | 2 | 0 | both |
| `tests/binary.rs` | 2 | 0 | two of seven |
| `player` crate (cross-crate) | 0 new | 0 | one strengthened assertion |

Two count notes, continued from chapter 04. First,
`tests/binary.rs` will hold **seven** tests when the track is
complete; chapter 04 added one, this chapter adds exactly two, and
the four `selfplayverify_*` tests of chapter 07 complete the target.
Second, chapter 04 ended with the suite at **27 passed, 4 ignored**;
this chapter's eight default tests bring the chapter-05 state to
**35 passed, 4 ignored**. The completed reference's full suite is 50
passed, 4 ignored; the remaining 15 default tests belong to chapters
06 (`tests/read_path.rs`, 11) and 07 (the four binary tests above).

The `player` crate changes hands across the crate boundary this
chapter: `Decision` gains a field, so `cargo test -p player` joins
this chapter's gates. The reference change adds no test there — it
strengthens the existing
`own_proof_plays_the_shortest_certified_line` in
`tests/decisions.rs` with one assertion,
`assert_eq!(decision.own_proof, Some(shortest));` — and the player
suite stays at **43 passed, 0 ignored** (17 `src` unit tests, 13
`tests/decisions.rs`, 13 `tests/guard.rs`, no doc tests).

### Own-proof retention in the player

- `own_proof_plays_the_shortest_certified_line`
  (`tests/decisions.rs`, existing, strengthened) — on the fixture
  whose own-side probe finds a certified shortest line, the decision
  plays the line's first move, carries no search statistics, and now
  must also carry the proof itself:
  `assert_eq!(decision.own_proof, Some(shortest));`. The retention is
  part of the bypass contract, not an optional extra.

### The suffix tracker

Both tests live in the unit-test module of
[`src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs) and
share one fixture: a ten-stone board, Black to move, with a VCF
win-in-3, built stone for stone from the same script as the
`vcf_win_in_three` test in the engine's `tss.rs`.

- `followed_own_tss_proof_becomes_a_certified_suffix` — an
  `AiPlayer` decides on the fixture with the baseline own-side probe
  enabled; the decision's `own_proof` is `Some`. A tracker begins at
  ply 0 with that proof, the test replays every move of the expected
  line through a real board (every replay is legal — the proof was
  verified), observing each move, and `finish` yields
  `CertifiedSuffix { from_ply: 0, line }` equal to the proof's line.
- `departing_from_the_proof_line_leaves_the_suffix_uncertified` —
  the same fixture, the proof taken straight from
  `engine::prove_shortest_forced_win`; the tracker begins at ply 0,
  observes one move off the line (`(14, 14)`), and `finish` yields
  `None`. A departed line is not a shortened certificate; it is no
  certificate.

### Extraction and the soundness gate

All four tests share one fabricated record: a hand-built `Game` (no
game is played) whose six post-opening plies give Black an open four
on row 5 — stones at columns 5 through 8 — and end with Black
playing `(5, 9)` to complete five; its `certified_suffix` is
`from_ply: 5, line: [(5,9)]`. `GameRecord::from_game` converts it.
The fixture matters: both ends of the open four win immediately, so
`(5, 4)` is a *verified* alternative line — the tests can separate
"the line is sound" from "the line was played".

- `certified_suffix_emits_replayable_anchor_seed` — extraction over
  the one-record corpus yields exactly
  `[AnchorSeed { game_index: 0, from_ply: 5, line: [(5,9)] }]`.
- `missing_certificate_emits_no_anchor_data` — the same record with
  `certified_suffix: None` yields an empty seed vector and no error.
  Absence is not a failure.
- `verified_alternative_line_that_was_not_played_is_rejected` — the
  certificate's line replaced with `[(5,4)]`, the other winning end:
  `verify_line` passes (the line is sound), but the game played
  `(5, 9)`, so extraction fails with
  `AnchorError::SuffixNotPlayed { game_index: 0, from_ply: 5 }`. A
  certificate must describe the line actually played.
- `doctored_suffix_line_is_a_hard_error` — the certificate's line
  replaced with `[(0,0)]`, which wins nothing: extraction returns
  `AnchorError::VerificationFailed { game_index: 0, from_ply: 5 }`.
  Tampering aborts extraction; it is never skipped.

One behavior is deliberately not pinned by these eight tests: every
fixture ends exactly when its certified line completes, so no test
plays a game that continues past the line. Accepting a certified
line as a *proper* prefix of the played suffix — trailing moves
after the line completes, the case walkthrough step 5 describes — is
prose-specified, source-verified behavior of the reference
implementation, not behavior any chapter test checks. No ninth test
fills that gap in this chapter.

### The `selfplayanchors` binary

Both tests live in
[`tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs)
and drive the compiled binary through
`env!("CARGO_BIN_EXE_selfplayanchors")`.

- `selfplayanchors_help_documents_corpus_extraction` — `--help`
  exits successfully; the usage text documents `--corpus` and
  `--out` and names `verify_line`, so the soundness gate is visible
  in the interface.
- `selfplayanchors_reads_a_corpus_and_writes_json_anchor_seeds` —
  `write_corpus` writes the fabricated certified record into a
  temporary corpus (shard capacity 1); the binary runs with
  `--corpus` and `--out`, exits successfully, and the output file
  parses as `Vec<AnchorSeed>` equal to the one expected seed. The
  temporary directory cleans itself up on drop.

## Walkthrough

The order is the execution order of one certified endgame, from the
seat's decision to the JSON file on disk. Two source files are new —
[`src/anchor.rs`](../../../../gomoku/crates/selfplay/src/anchor.rs)
and
[`src/bin/selfplayanchors.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayanchors.rs)
— and three existing files grow:
[`src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs)
gains the `SuffixTracker` and its three wiring regions: two calls
inside each played-ply iteration of the move loop — `begin` before
the engine accepts the move, `observe` after — plus `finish` called
once, after the loop, in the returned `Game` assembly,
[`src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs)
re-exports the new vocabulary, and
[`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs)
gains the `Decision.own_proof` field. `src/record.rs` is not
touched: the slot has waited there since chapter 02.

### 1. The seat's own probe certifies a forced win

Nothing in this step is new — it is the baseline tutorial's decision
pipeline, restated because everything downstream hangs on its order.
When the seat's options enable the own-side TSS probe — in corpus
runs they always do, because `selfplaygen` configures both seats
with `OwnTssConfig::baseline()` (10 000 nodes, 30 plies) —
`AiPlayer::decide` calls
`engine::prove_shortest_forced_win(board, board.to_move(), budget)`
before any search. The prover is soundness-first **[paper]** (Allis
1994 for threat-space search): it explores threat sequences within
budget, and every candidate line it returns has already passed
`verify_line` inside the prover — the initial proof and each
shorter retry alike. A `Some` is a machine-checked forced win; the
decision plays the line's first move immediately and consumes no
search, no evaluator call, and no randomness. A `None` is a budget
report, not a value judgment: the position is unproven, and the
ordinary PUCT (predictor UCT; Rosin 2011) search takes over.

### 2. The decision retains the exact proof

This chapter's only cross-crate change is one field. `Decision`
gains `own_proof: Option<engine::Proof>`. The three construction
sites pin the contract: the certified own-proof bypass stores
`Some(proof)` — the very `Proof` value the probe returned — and the
resignation path and the ordinary search path store `None`. Move
selection does not change anywhere; the field is evidence retention,
not behavior.

Why retain the proof instead of re-proving in the runner? The source
comment in `src/game.rs` states the reasoning: the player supplies
the exact proof that selected the move, and re-proving could choose
a different line — the prover searches anew and its shortest-line
preference is not a promise to repeat — while duplicating the
probe's work. Retention makes the recorded certificate the same
object as the evidence that drove the decision. The ordinary paths
carrying `None` is equally load-bearing: a searched move asserts no
forced win, and a resignation asserts nothing about the board at
all, so neither may fabricate a proof-shaped hint.

### 3. The runner tracks the line

Back in chapter 01's move loop, a private `SuffixTracker` rides
alongside the board. The loop's per-ply order becomes: decide,
`begin`, play, `observe`, record. `begin(plies.len(),
decision.own_proof)` runs on every played decision and starts a
pending certificate only when the decision carries a proof —
recording the current ply count as `from_ply` (a `u16`; the
conversion's `expect` states the invariant: a game has at most 225
plies) and the proof's line as the candidate. A decision without a
proof changes nothing: a pending certificate survives the defender's
proof-less replies, because the defender's forced blocks are part of
the line being tracked. A new proof replaces any pending
certificate — the latest own-side proof wins, which is why the
`Game` field documents itself as the *latest* proof followed to
completion.

After the engine accepts the move, `observe(mv)` compares it against
the pending line's next expected move. A match advances; completing
the whole line promotes the pending certificate to the game's
certified suffix. A mismatch discards the pending certificate
entirely. Note the discipline this enforces: the tracker follows the
exact recorded line, move for move, attacker threats and defender
replies alike. A defender who answers a gapped four with the
alternative block — a reply `verify_line` has proven the line still
beats — departs from the recorded main line, the pending certificate
is discarded, and the win simply goes unrecorded unless a later
probe certifies a fresh line. Unrecorded is not an error: the game
lost an anchor candidate, nothing else.

### 4. The game and the record carry the suffix

When the loop breaks — five completed, board full, or a resignation
— `finish` yields the last completed certificate, and the returned
`Game` carries it in the slot chapter 02 created. From there the
durable path is pure chapter 02: `GameRecord::from_game` copies the
slot verbatim into the record, and the shard writer persists it. The
certified suffix is the one field of the record that no later
chapter reinterprets: it is already final when the game ends.

Semantics, stated exactly once. `from_ply` counts post-opening plies
from zero — the Swap2 placements are the ceremony, not plies — and
identifies the position *before* the attacker's first certified
move. `line` is the complete verified forced-win line from that
position: `line[0]` is the attacker's forcing move at `from_ply`,
`line[1]` the defender's reply, and so on, ending when the attacker
completes five (overlines count) or holds an unblockable double
threat. The record stores both sides' moves because the claim is
about the game that happened, not about the attacker's intentions.

### 5. Extraction: replay, verify, match

`extract_anchor_seeds(records)` is a pure pass over in-memory
records — the binary supplies them — and it trusts nothing it has
not re-checked. Per record, in order:

1. A record without a certified suffix is skipped. No error, no
   seed, no negative label.
2. `from_ply` must name a played position: `from_ply` beyond the
   record's plies fails with `AnchorError::SuffixOutOfBounds`.
3. The opening executes through the Swap2 description in the record
   header (`AnchorError::Opening` on rejection), and the first
   `from_ply` recorded moves replay through a fresh engine board
   (`AnchorError::Replay { ply, mv, .. }` on the first illegal
   move). Extraction now holds the certified position itself,
   rebuilt from primary evidence.
4. The stored line, paired with the replayed side to move as the
   `Proof`'s winner, must pass `engine::verify_line`
   (`AnchorError::VerificationFailed`). This is the soundness gate,
   applied a second time: the prover verified at decision time, and
   extraction verifies again at read time, against a board the
   record itself reconstructed.
5. The first `line.len()` played moves from `from_ply` must equal
   the certified line — a prefix check, not a whole-suffix match
   (`AnchorError::SuffixNotPlayed`). A game may legally continue
   after the line completes: a proof can end in an unblockable
   double threat with moves still to play. If fewer than
   `line.len()` moves remain after `from_ply`, the prefix cannot
   match and the same error fires. Verification proves the line
   *wins*; this check proves the line *happened*. The anchor set
   wants both: a puzzle root whose recorded continuation is the
   sound one.

One failure aborts the entire extraction — the first error returns,
and no partial seed set exists. Only then does the pass emit, per
surviving certificate, an `AnchorSeed { game_index, from_ply, line
}`. The seed stores derivation instructions, not planes: replay the
game's opening and its first `from_ply` plies, and at that position
`line[0]` is the forcing move and the verified value is +1 for the
side to move (−1 for its opponent) — the milestone-4 (position,
forcing move, ±1) triple, computed rather than duplicated. The doc
comment on `AnchorSeed` states why: storing derivation information
keeps the seed independent of plane encoding.

### 6. The binary: read a corpus, write the seeds

`selfplayanchors` is a thin driver, hand-rolled argument parsing
like `selfplaygen` — no dependency. The vocabulary is two flags,
both required, values as separate arguments only: `--corpus <DIR>`
and `--out <JSON>`. `--help` (or `-h`) prints the usage and exits
successfully; the usage text states the contract — every certified
suffix must pass `verify_line` and match the played line, games
without certificates emit no anchor data and no negative labels, and
the output file must not already exist. A parse failure — a missing
flag, a flag without a value, an unknown flag — prints the message
and the usage and exits with code 2.

The run path is three steps. `read_corpus` loads the records
(chapter 02's mechanics); a read failure reports
`failed to read corpus: …`. `extract_anchor_seeds` runs the pass; a
certificate failure reports `certificate extraction failed: …` with
the `AnchorError`'s game index and detail. The output file opens
with create-new semantics — an existing file fails with
`failed to create '<path>': File exists (os error 17)`, the same
overwrite discipline the corpus writer keeps — and the seeds
serialize with `serde_json::to_writer_pretty`. Any run failure
prints `selfplayanchors failed: …` and exits with code 1. On success
the count goes to stderr: `Wrote 47 verified anchor seeds`.

The output is a pretty-printed JSON array of seeds. A `Move`
serializes as its board index (`row * 15 + col`, a `u8`), so the
anchor at game 0 of the measured corpus — certified at post-opening
ply 23, the forcing move at row 11, column 4 (11·15 + 4 = 169) —
looks like this:

```json
[
  {
    "game_index": 0,
    "from_ply": 23,
    "line": [
      169
    ]
  }
]
```

## The soundness boundary

The chapter's honesty rules, in one place.

- **Every emitted line re-passed `verify_line`.** The gate of
  docs/12 §12 is unchanged: 100 % of consumed certificates verify.
  The extraction pass checks each line against a position it
  replayed from the record's own opening and moves, so a corrupted
  header, an illegal prefix move, or a doctored line fails loudly —
  the first failure aborts the run, and nothing downstream ever sees
  a partial or unverified seed set.
- **Doctored records fail; they are never silently skipped.** The
  two negative extraction tests pin both failure modes: a sound line
  that was not played (`SuffixNotPlayed`) and a line that is not
  sound (`VerificationFailed`). Skipping would turn tampering into
  data loss; aborting turns it into an investigation.
- **Not every game has a suffix.** Quiet endings, draws, and
  endgames beyond probe depth certify nothing. The reference's
  measured corpus (48 games, baseline-32 vs baseline-32) carried a
  certified suffix in 47 of 48 games; the sole exception was the
  222-ply draw. The identification is stated directly in the
  reference's measured acceptance evidence (`acceptance-evidence.md`
  in the reference repository) and is confirmed record by record in
  the shipped corpus: game index 4 (seed 100004) is the only draw,
  the only 222-ply game, and the only record whose
  `certified_suffix` is `None`.
- **Absence of a proof is unknown, not loss.** The prover is
  bounded; `None` means "not proven within budget" and says nothing
  about the true value. Records without certificates therefore
  contribute no anchor data and no negative labels — the anchor set
  is a set of positives, and the design spec §3.4 fixes this
  reading.

The measured evidence belongs to the reference's acceptance run,
quoted here, not re-run by the learner: on the 48-game corpus,
`selfplayanchors` reported `Wrote 47 verified anchor seeds`; 47 of
47 certified suffixes verified (100 %), the 47 certificates came
from 47 distinct games, every certified line in that corpus was one
ply long (the baseline probe finds the final forcing move late and
short), and the extraction pass took 0.14 s. These numbers describe
one low-budget corpus, not a promise about future ones; chapter 07
owns the corpus-level gates.

## What this chapter defers

- Replay-to-sample derivation, the holdout split, D4 augmentation at
  sample time, weighted source mixing, and the `selfplayexport`
  binary — chapter 06. The anchor seeds this chapter emits are
  milestone-4 input, not training samples.
- Corpus verification (`selfplayverify`), the determinism, legality,
  grading, corpus, and integration gates, and the first corpus's
  quality report — chapter 07. The numbers quoted above are that
  chapter's acceptance evidence, cited early because they are this
  chapter's soundness story.

## TDD checklist

Work one artifact at a time, red to green. No manifest changes:
`selfplay` already depends on `engine`, `player`, `serde`,
`serde_json`, and `thiserror`, and a binary in `src/bin/` needs no
manifest entry. The `player` change does touch that crate's public
type, so its gate joins this chapter.

1. The `player` touch in
   [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs):
   `Decision` gains `own_proof: Option<engine::Proof>`; the
   own-proof bypass stores `Some(proof)`, the resignation and search
   paths store `None`. Red: strengthen
   `own_proof_plays_the_shortest_certified_line` in
   [`tests/decisions.rs`](../../../../gomoku/crates/player/tests/decisions.rs)
   with `assert_eq!(decision.own_proof, Some(shortest));` — it fails
   to compile until the field exists. Green across
   `cargo test -p player`, 43 passed, 0 ignored, count unchanged.
2. The `SuffixTracker` in
   [`src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs):
   a private tracker with `begin`, `observe`, and `finish`, wired
   into the move loop between the decision and the ply record —
   `begin` on every played decision with the decision's `own_proof`,
   `observe` after the engine accepts the move, `finish` into the
   `Game` slot. Red in two steps:
   `followed_own_tss_proof_becomes_a_certified_suffix` first, then
   `departing_from_the_proof_line_leaves_the_suffix_uncertified`.
   Both go green with the one tracker implementation.
3. The extraction module
   [`src/anchor.rs`](../../../../gomoku/crates/selfplay/src/anchor.rs):
   `AnchorSeed`, the five-variant `AnchorError`, and
   `extract_anchor_seeds` with its five checks in the walkthrough's
   order. Red: the four `tests/anchors.rs` tests in the
   specification's order — the fabricated record first, then the
   missing-certificate, alternative-line, and doctored-line cases.
   Green against all four. Re-export the new vocabulary from
   [`src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs).
4. The binary
   [`src/bin/selfplayanchors.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayanchors.rs):
   the usage text, `parse_args`, and the three-step run path with
   create-new output. Red: the two `tests/binary.rs` tests. Green in
   both.
5. Regression: the game, corpus, openings, schedule, dedup, and
   existing binary targets stay green unchanged — the tracker only
   reads what the decision already carried — and chapter 01's MANUAL
   pair still passes in release, because the loop gained two calls.

## Capstone

Extract anchors from a tiny corpus you generate, and watch the
boundary work end to end. From `gomoku/`, in release (the generation
plays real games):

```text
cargo build --release -p selfplay

dir=$(mktemp -d)

target/release/selfplaygen \
  --out "$dir/corpus" \
  --openings ../data/openings/gomocup-2024-freestyle15.txt \
  --black-sims 8 --white-sims 8 --games 3 --seed-base 555

target/release/selfplayanchors \
  --corpus "$dir/corpus" --out "$dir/anchors.json"

cat "$dir/anchors.json"
```

The reference run of exactly this pair reported
`Wrote 3 verified anchor seeds`, one per game — at these budgets the
baseline probe certifies the final forcing move of most games, and
each certificate was one ply long. The JSON shows the format end to
end: `game_index` counts the corpus order, `from_ply` counts
post-opening plies, and `line` holds board indices, so `71` is row
4, column 11 (4·15 + 11). Two boundary experiments complete the
picture. Rerun the extraction with the same `--out`: the run fails
with `failed to create '…': File exists (os error 17)` — the output
file must not exist, so an extraction never overwrites a seed set by
accident. And re-run the generator with a different `--seed-base`
and compare counts: whether every game certifies is a property of
the games, not of the extractor, and a game without a certificate
writes no line and no apology.

## Done when

- `cargo test -p selfplay` reports **35 passed, 4 ignored**: the
  chapter-04 targets keep their state (game 5 passed / 2
  MANUAL-ignored, corpus 10/0, openings 5/0, schedule 1/1, dedup
  1/0, the `selfplaygen` binary's unit tests 4/0, grading 0/1), and
  this chapter adds `tests/anchors.rs` 4/0, the `src/game.rs` unit
  pair 2/0, and `tests/binary.rs` at 3/0. The four ignored tests are
  exactly chapter 04's set; this chapter adds none.
- `cargo test -p player` reports **43 passed, 0 ignored**, with the
  strengthened `own_proof_plays_the_shortest_certified_line` green —
  the cross-crate field is pinned at its source.
- `cargo test -p train` is not required: the record format and the
  storage mechanics are untouched.
- No new MANUAL test exists; the release-only verification
  discipline of chapter 04 is unchanged.
- `cargo clippy --all-targets -- -D warnings` is green.
- `cargo fmt --all -- --check` makes no changes.
- The commit message is
  `feat(selfplay): certified endgames — selfplay-corpora chapter 05`.

## Verification commands

From `gomoku/`:

```text
cargo test -p selfplay
cargo test -p selfplay --test anchors
cargo test -p selfplay --lib
cargo test -p selfplay --test binary selfplayanchors
cargo test -p player
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the fast gate: this chapter adds nothing slow
to it. The anchor tests replay fabricated records through the
engine; the tracker unit tests run one TSS probe each on a ten-stone
fixture; the binary tests drive the compiled binary over a one-game
fabricated corpus. The focused selections — the `anchors` target,
the library's unit tests, and the binary's two tests selected by
name — are this chapter's surface in isolation; the name filter
leaves the later chapters' binary tests unrun because they do not
exist yet. `cargo test -p player` is the cross-crate regression. No
`--release -- --ignored` run is chapter-05 work: game-playing
verification stays MANUAL and release-only, exactly as chapter 04
left it.

## References

**This track:**

- [Tutorial README](../README.md) — conventions, gates, the chapter
  map, and the reference provenance.
- [Self-play primer](../00-selfplay-primer.md) — §4.5 states this
  chapter's contract; the glossary defines the certified suffix and
  the anchor seed.
- [Chapter 01 — the game runner](../01-the-game-runner/the-game-runner.md) —
  the move loop and the `Decision` seam this chapter instruments.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md) —
  the `CertifiedSuffix` type, the `Game` and `GameRecord` slots, and
  the preservation test this chapter activates.
- [Chapter 03 — recording the search](../03-recording-the-search/recording-the-search.md) —
  the honesty rule: a certified-proof move carries no search record.
- [Chapter 04 — diversity and grading](../04-diversity-and-grading/diversity-and-grading.md) —
  the generator whose baseline-probe seats produce the certificates,
  and the MANUAL discipline this chapter leaves unchanged.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  §3.4 the certified-endgame contract and the no-negative-labels
  rule; §5 the acceptance gates chapter 07 runs.
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  this chapter's public surface and the measured headline numbers.
- [docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) —
  §12 the soundness gate; the claim-label convention.

**Prerequisite tracks:**

- [Baseline tutorial](../../baseline-tutorial/README.md) —
  `AiPlayer::decide`, the own-side TSS probe, and the certified
  bypass that now retains its proof.
- [Engine tutorial](../../13-engine-tutorial/README.md) — the TSS
  prover and `verify_line`, and the budget-honesty rule.

**Source:**

- [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs) —
  `Decision` gains `own_proof`. Touched in this chapter.
- [`gomoku/crates/player/tests/decisions.rs`](../../../../gomoku/crates/player/tests/decisions.rs) —
  one strengthened assertion. Touched in this chapter.
- [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs) —
  gains the `SuffixTracker`, its three wiring regions (the per-ply
  `begin` and `observe` in the move loop, the one `finish` in the
  returned `Game`), and the two unit tests. Touched in this chapter.
- [`gomoku/crates/selfplay/src/anchor.rs`](../../../../gomoku/crates/selfplay/src/anchor.rs) —
  new: `AnchorSeed`, `AnchorError`, `extract_anchor_seeds`. Created
  in this chapter.
- [`gomoku/crates/selfplay/src/bin/selfplayanchors.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayanchors.rs) —
  new: the extraction binary. Created in this chapter.
- [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs) —
  re-exports the new vocabulary. Touched in this chapter.
- [`gomoku/crates/selfplay/src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs) —
  the `CertifiedSuffix` slot, unchanged since chapter 02. Not
  touched in this chapter.
- [`gomoku/crates/selfplay/tests/anchors.rs`](../../../../gomoku/crates/selfplay/tests/anchors.rs) —
  the extraction specification. Created in this chapter.
- [`gomoku/crates/selfplay/tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs) —
  gains this chapter's two binary tests. Touched in this chapter.

**Companion papers:**

- [certified-endgames-design.md](certified-endgames-design.md) — the
  conceptual story of the certificate's path from decision to anchor
  seed.
- [certified-endgames-implementation.md](certified-endgames-implementation.md)
  — the hiding layer: low-level construction instructions, signatures
  in prose, method bodies never.
- [certified-endgames-solution.md](certified-endgames-solution.md) —
  the annotated reference snippets.

**External sources:**

- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://doi.org/10.26481/dis.19940923la — threat-space search, the
  prover and verifier behind every certificate this chapter handles.
- Rosin, C. D. (2011). *Multi-armed bandits with episode context*
  (PUCT). https://doi.org/10.1007/s10472-011-9258-6 — the selection
  rule of the search the probe short-circuits.
