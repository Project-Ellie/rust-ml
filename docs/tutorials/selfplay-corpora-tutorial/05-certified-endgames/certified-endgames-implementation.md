# Chapter 05 — Certified endgames: implementation plan

## Abstract

This document is the implementation plan for
[chapter 05 of the selfplay-corpora tutorial](certified-endgames.md):
the hiding layer between the chapter paper and the solution, for the
learner who builds the code without reading the reference. It turns the
chapter's walkthrough into a numbered construction order along the
execution path of one certificate: the `player` crate's `Decision`
gains the `own_proof` field that retains the exact verified threat-space
search (TSS) proof at the certified bypass, the game runner gains a
private `SuffixTracker` that follows the retained line move for move,
the new `anchor` module gains `AnchorSeed`, the five-variant
`AnchorError`, and the `extract_anchor_seeds` pass with its five
checks, the `selfplayanchors` binary drives the pass over a corpus and
writes pretty JSON with create-new semantics, and the export audit,
regression, gates, and capstone close the chapter. Every artifact names
its file, its behavioral contract in prose, the helpers to reuse, and
the idioms to apply by name; method bodies never appear. The plan pins
the exact test names, fixtures, and assertions of the chapter's eight
new tests — all default-run, none MANUAL — plus the one strengthened
assertion in the `player` crate, and the per-target counts that sum to
the chapter's closing state of 35 passed and 4 ignored for `selfplay`
and 43 passed and 0 ignored for `player`. The intended reader has
completed chapter 04 (suite state: 27 passed, 4 ignored), has read the
chapter paper and, ideally, the
[design paper](certified-endgames-design.md), and wants low-level
guidance without solution leakage.

## Glossary

| Term | Definition |
|------|------------|
| **Threat-space search (TSS)** | A proof search over forced-win threat sequences (Allis 1994). The engine ships a bounded prover (`prove_forced_win`, `prove_shortest_forced_win`) and a verifier (`verify_line`); the prover may be incomplete, never unsound. |
| **Proof** | `engine::Proof`: a certified forced win — the winning side (`winner`) and one forcing sequence (`line`). The line alternates attacker and defender moves, starts with the attacker's first threat, and ends when the attacker completes five (overlines count) or creates an unblockable double threat. |
| **Attacker / defender** | The two roles inside a proof line. The attacker is the side with the forced win (`proof.winner`); the defender is its opponent. Attacker moves sit at even offsets of the line, defender replies at odd offsets. |
| **verify_line** | `engine::verify_line(board, proof) -> bool`: the soundness net. It replays the line from the board and, at every defender turn, enumerates every reply that addresses the threat; the attacker's continuation must still win against all of them. `true` only if the proof is airtight, and `false` also when the proof's winner is not the side to move. |
| **Own-side TSS probe** | The `AiPlayer::decide` step that runs `prove_shortest_forced_win` for the side to move before any search (baseline preset: 10 000 nodes / 30 plies). A certified proof short-circuits the decision: the line's first move is played and no search runs. |
| **own_proof** | The `Decision` field this chapter adds: an optional `engine::Proof`, present exactly when the own-side probe certified the played move. |
| **Certified suffix** | `CertifiedSuffix { from_ply, line }` (chapter 02): a terminal segment of a game in which every played move belongs to one machine-verified forced-win line. `from_ply` is a `u16`, the zero-based post-opening ply of the attacker's first certified move. |
| **SuffixTracker** | The runner's private detector, created in this chapter: it begins a pending candidate when a decision carries an own proof and observes every played move against the candidate's line; a departure discards the candidate, and a completed line becomes the game's certified suffix. |
| **Pending candidate** | The tracker's in-flight certificate: the private `PendingSuffix` value holding a `CertifiedSuffix` under construction plus `next`, the index of the line move expected next. |
| **Anchor seed** | `AnchorSeed { game_index, from_ply, line }`: replay instructions for one milestone-4 anchor position. Replay the source game's opening and its first `from_ply` post-opening moves; at that position `line[0]` is the forcing move and the verified value is +1 for the side to move (−1 for its opponent). |
| **Milestone 4** | The project's puzzle-solving milestone (chapter 12 of the wiki): an anchor set of (position, forcing move, ±1) triples whose positions come from real self-play games. This chapter produces its input. |
| **Swap2** | The project's opening protocol: one side places three stones, the other side chooses a color or places two more stones, and the first side then chooses a color. The placements are the ceremony, not plies. |
| **Post-opening ply** | One played move after the Swap2 ceremony, counted from zero. `from_ply`, the tracker, and the extraction errors count in these plies. |
| **Corpus** | A set of game records produced at one declared pairing, stored as shards plus a manifest (chapter 02). |
| **Soundness gate** | The project's rule (chapter 12 §12 of the wiki): every certificate consumed downstream must pass `verify_line`; 100 % must verify. |
| **VCF** | Victory by continuous forcing threats: a forced-win line in which every attacker move creates a threat the defender must answer. The tracker unit-test fixture is a VCF win-in-3. |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games. It never runs by default; it runs on explicit selection, in release mode. This chapter adds none. |
| **Claim labels** | The project's provenance labels (chapter 12 of the wiki): **[paper]** (stated in a cited paper), **[derived]** (computed from cited or measured values), **[experiment]** (a free parameter, to be settled by measurement). |
| **TDD** | Test-driven development: write the test, watch it fail (red), implement, watch it pass (green). |

## How to use this plan

Read [the chapter paper](certified-endgames.md) first. It carries the
intention, the specification as tests, and the execution-order
walkthrough; the [design paper](certified-endgames-design.md) carries
the reasoning, the tracker state machine, and the eight invariants.
This plan assumes both and adds only construction detail: which file,
which item, which contract, which test goes red then green. Work one
step at a time, in order — the order is the execution order of one
certificate, from the seat's decision where the proof is produced,
through the runner where the played line is tracked, to the extraction
pass and the binary that writes the seeds. Open
[certified-endgames-solution.md](certified-endgames-solution.md)
only if you are stuck for more than twenty minutes, or
after the chapter is complete.

## Context

The chapter-04 slice — the verified starting state — holds the
following. Selfplay paths are under
[`gomoku/crates/selfplay/`](../../../../gomoku/crates/selfplay/);
the crate does not exist on `main` until the learner lands it chapter
by chapter.

- [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs)
  (live on `main`) carries `Decision` with four public fields —
  `action`, `search`, `guard`, `resignation` — deriving `Debug`,
  `Clone`, and `PartialEq`, and `AiPlayer::decide` with three decision
  construction sites: the certified own-proof bypass (which currently
  discards the proof after the certified-own-move check keeps only the
  move), the heuristic resignation path, and the ordinary search path.
  `ResignReason` has carried serde's two derives since chapter 01, and
  the `player` manifest already lists `engine`, so this chapter's
  cross-crate change needs no manifest edit. The `player` suite stands at
  **43 passed, 0 ignored** — 17 `src` unit tests, 13
  `tests/decisions.rs`, 13 `tests/guard.rs`, no doc tests — and
  `own_proof_plays_the_shortest_certified_line` in
  [`tests/decisions.rs`](../../../../gomoku/crates/player/tests/decisions.rs)
  already holds the `shortest` proof value this chapter's strengthened
  assertion needs.
- `src/game.rs` (chapters 01–04) carries the runner
  `play_game_with_opening` / `play_game` with the chapter-04 schedule
  application, the `Game` record whose `certified_suffix` slot the
  runner initializes to `None` (chapter 02; the slot's doc states that
  absence means no anchor data, never a negative value label),
  `PlySearch` with the chapter-03 conversion, `PlyRecord`, `Outcome`,
  `WinReason`, and `GameError`. The move loop's per-ply order today is:
  decide, convert the search statistics, play, record. The file has no
  unit-test module: the library target currently runs zero tests.
- `src/record.rs` (chapter 02) carries `CertifiedSuffix` — `from_ply`
  (a `u16`) and `line` (a vector of `Move`), with serde's derives —
  whose doc names chapter 05 as the detection that populates it, and
  the `GameRecord.certified_suffix` slot that
  `GameRecord::from_game` copies verbatim, pinned by chapter 02's
  preservation test. **This chapter does not touch `src/record.rs`.**
- `src/corpus.rs` (chapter 02) carries `write_corpus` with its
  shard-capacity argument and overwrite protection, and `read_corpus`.
  `src/opening.rs` (chapters 01 and 04) carries `Opening` with
  `Opening::board`, the execution seam extraction replays through.
- `src/lib.rs` declares six modules (`corpus`, `game`, `generate`,
  `opening`, `record`, `schedule`) behind `#![deny(missing_docs)]` and
  re-exports the chapter-04 surface. Every public item this chapter
  adds needs doc comments; the deny turns a missing doc into a build
  error.
- `Cargo.toml` needs **no changes**: `selfplay` already depends on
  `engine`, `player`, `serde`, `serde_json`, and `thiserror`, and a
  binary in `src/bin/` needs no manifest entry. Verify this before
  writing anything; if a step seems to need a dependency, the step has
  drifted.
- The engine (consumed, unchanged) carries `Proof` with its `winner`
  and `line` fields, `prove_shortest_forced_win`, `verify_line`, and
  `Move`, a `u8` newtype that serializes as its board index
  (`row * 15 + col`) — the shape the seed file shows.
- Test targets at the chapter-04 state: `tests/game.rs` 5 passed / 2
  MANUAL-ignored; `tests/corpus.rs` 10/0; `tests/openings.rs` 5/0;
  `tests/schedule.rs` 1/1; `tests/dedup.rs` 1/0; the `selfplaygen`
  binary's unit tests 4/0; `tests/binary.rs` 1/0;
  `tests/grading.rs` 0/1 — **27 passed, 4 ignored** in sum.

Two scope boundaries keep the chapter honest. First, `tests/binary.rs`
will hold **seven** tests when the track is complete; this chapter adds
exactly two (`selfplayanchors_*`), and chapter 07 adds the four
`selfplayverify_*` tests. The `TestDir` and `certified_record` helpers
this chapter adds to that file are written for the two anchor tests;
nothing in them anticipates chapter 07. Second, no chapter-06-or-later
module, type, binary, or test is named anywhere in this chapter's code:
no sample, sampler, mix, or export module, no `selfplayexport` or
`selfplayverify` binary, no read-path target.

---

## Step 1 — the `player` touch: `Decision.own_proof` (red → green)

The chapter's only cross-crate change: one field, one strengthened
assertion, three construction sites, two doc updates. All edits land in
[`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs)
and
[`gomoku/crates/player/tests/decisions.rs`](../../../../gomoku/crates/player/tests/decisions.rs).

### 1.1 The strengthened test, red

In
[`tests/decisions.rs`](../../../../gomoku/crates/player/tests/decisions.rs),
strengthen the existing
`own_proof_plays_the_shortest_certified_line` — do not add a test. The
fixture is the file's non-minimal VCF board, on which the ordinary
prover and the shortest-line prover certify different first moves; the
test already binds the shortest proof as `shortest`, plays the decision
with the probe enabled, and asserts the played move is the shortest
line's first move and the search statistics are absent.

Insert one assertion between the action assertion and the
search-absence assertion: the decision's `own_proof` equals
`Some(shortest)` — the exact `Proof` value the probe returned, compared
with the equality assertion idiom. `Proof` derives `PartialEq` and
`Debug`, so the comparison needs nothing new. The red state is a
compile error: `Decision` has no `own_proof` field.

### 1.2 The field, green

In `src/lib.rs`, add one public field to `Decision`, placed directly
after `search` and before `guard`: `own_proof`, an optional
`engine::Proof`. The derive list does not change — `Proof` supports
`Debug`, `Clone`, and `PartialEq`. The doc comment is the reference
wording: "Exact verified own-side proof that selected this move, if the
TSS probe fired. Ordinary searches and resignations retain no own-side
proof."

### 1.3 The three construction sites

Pin the contract at all three sites; move selection changes nowhere.

1. **The certified own-proof bypass stores the proof.** The probe call
   currently chains the certified-own-move check so that only the move
   survives. Change the chain so the closure pairs the certified move
   with the proof — the map-pairing idiom on the check's result — bind
   the pair, and store the proof, wrapped in `Some`, in the returned
   decision. The certified-own-move helper itself is unchanged, and so
   are the action, the absent search statistics, and the guard
   diagnostics.
2. **The resignation path stores `None`.** A resignation asserts
   nothing about the board; it may not fabricate a proof-shaped hint.
3. **The ordinary search path stores `None`.** The PUCT search
   (predictor UCT; Rosin 2011) selected the move; a searched move
   asserts no forced win.

### 1.4 The `decide` doc update

The `AiPlayer::decide` doc comment describes the certified bypass as
returning "without consuming RNG, calling the evaluator, or
constructing search statistics." Append the reference sentence directly
after it: "The decision retains that exact proof in `own_proof` for
downstream suffix auditing."

Watch the gates: the strengthened test compiles and passes, and

```text
cargo test -p player
```

reports **43 passed, 0 ignored** — the count is unchanged, because the
chapter adds an assertion, not a test. If the count moves, the change
drifted.

---

## Step 2 — `src/game.rs`: the private `SuffixTracker` and the runner wiring (red → green)

One file:
[`src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs). It
gains a private detector, its three call sites in the move loop, the
`Game` assembly change, and a `#[cfg(test)]` module — the library
target's first unit tests. `Proof` joins the file's engine import
group, in alphabetical position; `CertifiedSuffix` is already imported
for the `Game` slot.

### 2.1 The tracker unit tests, red

Add a test module at the end of `src/game.rs`. Module imports: the
engine's `Board`; the `player` crate's `AiPlayer` and `OwnTssConfig`;
the seeded-RNG machinery (`StdRng` and the `SeedableRng` trait); and
the parent module's items. One shared fixture, `vcf_fixture()`: a fresh
board that plays ten stones in order — (7,4), (7,3), (7,5), (0,0),
(7,6), (0,2), (6,6), (0,4), (8,8), (0,6) — each constructed through
`Move::new` and expected to be legal. The result is Black to move with
a VCF win-in-3; the script is the same stone sequence as the engine's
own `vcf_win_in_three` test in `tss.rs`, so the prover's behavior on it
is already pinned upstream.

Write the two tests in this order, watching each go red (the red state
is a compile error — the tracker does not exist):

1. `followed_own_tss_proof_becomes_a_certified_suffix` — build the
   fixture mutably; build an `AiPlayer` on the baseline evaluator with
   the flexible device; decide on the fixture with the
   struct-update-syntax idiom over the default player options,
   overriding only the own-TSS probe configuration to
   `OwnTssConfig::baseline()`, on an RNG seeded at 8. Expect the
   decision's `own_proof` to be present, with the message "the VCF
   fixture has a proof", and clone its line as the expected line. Begin
   a fresh tracker at ply 0 with that proof. Then replay every move of
   the expected line through the real board — each play expected to
   succeed, with the message "verified proof moves replay legally" —
   observing each move into the tracker after the board accepts it.
   Finish, expect a certificate with the message "the complete line
   certifies", and assert its `from_ply` is 0 and its `line` equals the
   expected line.
2. `departing_from_the_proof_line_leaves_the_suffix_uncertified` —
   build the fixture (immutable this time); take the proof straight
   from `engine::prove_shortest_forced_win` on the fixture for the side
   to move at the baseline probe's budget, expected present with the
   same message. Begin a fresh tracker at ply 0 with that proof,
   observe one move off the line — (14, 14), an empty corner no VCF
   line from this fixture visits — and assert that finish yields `None`.
   A departed line is not a shortened certificate; it is no
   certificate.

Both tests run one TSS probe each on a ten-stone fixture and play no
game; they belong in the default suite and finish in well under a
second.

### 2.2 `PendingSuffix` and `SuffixTracker`, green

Place the tracker between the `Game` implementation block and the
`GameError` enum. Above the struct, write the retention comment the
reference carries, verbatim: "The player supplies the exact proof that
selected the move. Re-proving here could choose a different line and
would duplicate the probe's work." The comment lives above the tracker
because the tracker is the reason the proof crosses the crate boundary
at all.

Both types are **private** — no `pub`, not even crate-visible:

- `PendingSuffix`: the in-flight candidate. Two fields: `suffix`, the
  `CertifiedSuffix` under construction, and `next`, a `usize`, the
  index of the line move expected next. No derives are needed.
- `SuffixTracker`: two fields — `pending`, an optional `PendingSuffix`,
  and `certified`, an optional `CertifiedSuffix`, the last completed
  certificate. Derive `Default` — both fields start empty — and let
  the construction go through the derived default; there is no `new`.

Three methods, all on a mutable borrow except the last:

- `begin` consumes the current post-opening ply count (a `usize`) and
  the decision's `own_proof` **by value**, and returns nothing. With a
  proof, it starts — or wholesale replaces — the pending candidate: the
  candidate's `from_ply` is the ply count converted with the try-from
  idiom into `u16`, expected with the invariant message "a game has at
  most 225 plies"; the candidate's `line` is the proof's line, moved
  out of the proof; `next` starts at zero. Replacement is total: the
  latest own-side proof wins, which is why the `Game` field documents
  itself as the *latest* proof followed to completion. Without a proof,
  `begin` changes nothing — a pending candidate survives the defender's
  proof-less replies, because the defender's forced blocks are part of
  the line being tracked.
- `observe` consumes the played move after the engine accepted it, and
  returns nothing. With no candidate in flight it does nothing — moves
  after a completed or discarded certificate are not tracked. With a
  candidate, compare the move against the line's element at `next` with
  the get-and-compare idiom: a mismatch discards the candidate entirely
  (any earlier completed certificate is kept); a match advances `next`
  by one, and when `next` reaches the line's length the candidate
  promotes into the certified slot, replacing any earlier completed
  certificate — the take-and-map idiom leaves `pending` empty.
- `finish` consumes the tracker and returns the optional
  `CertifiedSuffix`: the last completed certificate, or none. An
  unfinished candidate yields nothing.

### 2.3 The runner wiring

In `play_game_with_opening`, three insertions and one replacement;
everything else in the function stays byte-identical:

1. In the setup, after the plies vector is created and before the
   schedule binding, bind the tracker through its derived `Default`.
2. In the loop's play arm, the per-ply order becomes **decide, begin,
   play, observe, record**: call `begin` with the current ply count and
   the decision's `own_proof` — this consumes the field out of the
   decision — immediately after the decision and before the chapter-03
   search-statistics conversion; the engine's legality gate and the
   `observe` call follow the accepted move, in that order; the ply
   record is appended last. The resignation arm is untouched: no begin,
   no observe.
3. In the returned `Game`, the `certified_suffix` slot is the
   tracker's `finish` yield, replacing the `None` the runner has
   written since chapter 02.

One doc update rides along: the `Game.certified_suffix` field doc —
which since chapter 02 states only the absence semantics — becomes the
reference wording: "Latest own-side proof whose line was followed to
completion during play." The absence semantics stay where chapter 02
put them: on the record's slot in `src/record.rs`, which this chapter
does not touch.

Watch both tracker tests compile and pass:

```text
cargo test -p selfplay --lib
```

reports exactly **2 passed, 0 ignored**. Then run the full default
suite: every chapter-04 target stays green unchanged, because the
tracker only reads what the decision already carried.

---

## Step 3 — `src/anchor.rs` and `tests/anchors.rs`: extraction and the soundness gate (red → green)

Create two files:
[`src/anchor.rs`](../../../../gomoku/crates/selfplay/src/anchor.rs) and
[`tests/anchors.rs`](../../../../gomoku/crates/selfplay/tests/anchors.rs).
Declare the `anchor` module in `src/lib.rs`, first in the module list
(alphabetical position, before `corpus`), and add the re-export group —
`AnchorError`, `AnchorSeed`, `extract_anchor_seeds` — before the corpus
group. The re-exports land **before** the integration tests are watched
green: the tests import crate-root names only.

### 3.1 The four extraction tests, red

The test file's imports: the engine's `Color` and `Move`; the `player`
crate's `PlayerOptions`; and, from the crate root, `AnchorError`,
`AnchorSeed`, `CertifiedSuffix`, `Game`, `GameRecord`, `Opening`,
`OpeningAnswer`, `OpeningProvenance`, `Outcome`, `PlyRecord`,
`WinReason`, and `extract_anchor_seeds`. The red state is a compile
error — the module does not exist.

Two helpers. The `mv(row, col)` fixture constructor from the other test
targets, validating through `Move::new`. And `fabricated_record()`,
which builds a `Game` by hand — no game is played anywhere in this
target — and converts it with chapter 02's `GameRecord::from_game`:

- seed 17; both option sets the defaults;
- the opening's three initial stones at (5,5), (10,10), and (5,6); the
  answer is the take-Black variant, so White moves first after the
  ceremony; the provenance is `Generated`;
- six post-opening plies, each carrying no search metadata: (10,11),
  (5,7), (10,12), (5,8), (10,13), (5,9) — White builds four on row 10
  without finishing, while Black extends the row-5 pair from the
  opening to an open four at columns 5 through 8 and then completes
  five at (5,9);
- the outcome is a Black win, reason five-or-more;
- the certified-suffix slot holds `from_ply` 5 and the one-move line
  [(5,9)].

The fixture's load-bearing property: both ends of Black's open four win
immediately, so (5,4) is a *verified* alternative line — the tests can
separate "the line is sound" from "the line was played".

The four tests, in the specification's order:

1. `certified_suffix_emits_replayable_anchor_seed` — extract over the
   one-record corpus, expecting success with the message "the
   certificate verifies"; assert equality against exactly one seed:
   `game_index` 0, `from_ply` 5, `line` [(5,9)].
2. `missing_certificate_emits_no_anchor_data` — the fabricated record
   with its certified-suffix slot set to `None`; extraction expects
   success with the message "missing certificates are not errors", and
   the seed vector is empty. Absence is not a failure.
3. `verified_alternative_line_that_was_not_played_is_rejected` — the
   fabricated record with the certificate's line replaced by [(5,4)],
   the other winning end, carrying the comment "Both ends of Black's
   open four win immediately, but the game played 5,9."; extraction
   expects failure with the message "a certificate must describe the
   line actually played"; assert with the match idiom that the error is
   `AnchorError::SuffixNotPlayed` with `game_index` 0 and `from_ply` 5.
   `verify_line` passes on this line — soundness is not enough.
4. `doctored_suffix_line_is_a_hard_error` — the certificate's line
   replaced by [(0,0)], which wins nothing; extraction expects failure
   with the message "tampering must abort extraction"; assert with the
   match idiom that the error is `AnchorError::VerificationFailed` with
   `game_index` 0 and `from_ply` 5. Tampering aborts extraction; it is
   never skipped.

One deliberate gap, stated in the chapter paper and held here: every
fixture ends exactly when its certified line completes, so no test
plays a game that continues past the line. Accepting a certified line
as a *proper* prefix of the played suffix — trailing moves after the
line completes — is prose-specified, source-verified behavior of the
reference implementation, pinned by no test. **No ninth test fills that
gap in this chapter.**

### 3.2 `AnchorSeed`, green

A public struct in `src/anchor.rs`, deriving `Debug`, `Clone`,
`PartialEq`, `Eq`, and serde's `Serialize` and `Deserialize`. The doc
comment is the reference's contract: replay instructions for one
milestone-4 anchor position — replay the source game's opening and its
first `from_ply` post-opening moves; at that position the line's first
move is the forcing move and the verified value is +1 for the side to
move (−1 for its opponent); storing derivation information rather than
planes keeps the seed independent of plane encoding. Three public
fields, each documented: `game_index` (a `usize`, the zero-based index
of the source game in corpus order), `from_ply` (a `u16`, the
zero-based post-opening ply at which the certified position occurs),
and `line` (a vector of `Move`, the complete verified forced-win line
whose first move is the forcing move).

### 3.3 `AnchorError`, green

A public error enum deriving `Debug` and the `thiserror` error derive —
no `Clone`, no `PartialEq`: the tests match on variants with the match
idiom, so equality is not part of the contract. Five variants, in the
reference's declaration order — the vocabulary order, not the check
order; the check order is §3.4's contract — each with a doc comment and
documented fields:

1. `Opening` — the source game's Swap2 opening was invalid. Fields:
   `game_index` (a `usize`) and `source` (the engine's `PlayError`,
   marked as the error source). Display: "game {game_index}: opening
   replay failed: {source}".
2. `Replay` — a source move before the certified position was illegal.
   Fields: `game_index`, `ply` (a `usize`, the zero-based post-opening
   ply), `mv` (the rejected `Move`), and `source` (the `PlayError`).
   Display: "game {game_index}: move {mv:?} at ply {ply} was rejected:
   {source}" — the move renders with its debug format.
3. `SuffixOutOfBounds` — the certificate does not identify a played
   position. Fields: `game_index`, `from_ply` (the `u16` from the
   certificate), and `ply_count` (a `usize`, the number of post-opening
   moves available for replay). Display: "game {game_index}: certified
   from_ply {from_ply} is outside {ply_count} plies".
4. `SuffixNotPlayed` — the stored line is not a prefix of the moves
   actually played. Fields: `game_index` and `from_ply`. Display:
   "game {game_index}: certified line from ply {from_ply} was not
   played".
5. `VerificationFailed` — the engine rejected a stored certificate.
   Fields: `game_index` and `from_ply`. Display: "game {game_index}:
   certified line from ply {from_ply} failed verify_line".

The module's imports: the engine's `Move`, `PlayError`, `Proof`, and
`verify_line`; serde's two derive macros; the `thiserror` derive; and
the crate's `GameRecord`.

### 3.4 `extract_anchor_seeds`, green

A public free function — no receiver — borrowing a slice of
`GameRecord` and returning a `Result` of the seed vector or an
`AnchorError`. The doc comment states the honesty contract: every
stored certificate is a positive claim and must pass
`engine::verify_line`; one failure aborts the entire extraction;
records without certificates contribute no seeds and, crucially, no
negative labels. The reference's `# Errors` section reads "Returns
the first opening, replay, bounds, or line-verification failure." That
comment names the failure kinds — it is not a list of all five
variants in check order, and it does not mention the played-prefix
failure at all: "line-verification" covers only `VerificationFailed`,
because `verify_line` proves the line *wins*, never that it was
played. The full behavioral contract is the checklist below: the
errors actually occur in the order `SuffixOutOfBounds` → `Opening` →
`Replay` → `VerificationFailed` → `SuffixNotPlayed`, and the first
error aborts the extraction. `SuffixNotPlayed` is the separate final
played-prefix failure of check 5 — not something `verify_line` can
prove, and not a category hidden in the doc wording.

Behavioral contract, per record, in this exact order — the record's
zero-based position in the slice is its `game_index`:

1. **Skip.** A record without a certified suffix contributes nothing:
   no seed, no error, no negative label. The let-else idiom with a
   continue keeps the happy path flat.
2. **Bounds.** Convert the certificate's `from_ply` to `usize`. If it
   is greater than or equal to the record's ply count, fail with
   `SuffixOutOfBounds`, carrying the game index, the invalid start, and
   the ply count. A certificate must name a played position.
3. **Replay.** Execute the record header's opening through
   `Opening::board`, mapping a rejection to `Opening`; then replay the
   first `from_ply` recorded moves, in order, through the fresh engine
   board, mapping the first rejection to `Replay` with the zero-based
   ply index, the move, and the source. Extraction now holds the
   certified position itself, rebuilt from primary evidence.
4. **Verify.** Reconstruct a `Proof` whose winner is the replayed
   board's side to move and whose line is the stored line, cloned.
   Require `engine::verify_line` on the replayed board and that proof;
   a `false` fails with `VerificationFailed`. This is the soundness
   gate applied a second time: the prover verified at decision time,
   and extraction verifies again at read time, against a board the
   record itself reconstructed. The prover is soundness-first
   **[paper]** (Allis 1994); the gate keeps it honest at the boundary.
5. **Match.** The played-prefix check: iterate the record's moves from
   `from_ply` onward, take as many as the line is long, and compare for
   equality against the stored line with the iterator equality idiom.
   A mismatch — or fewer remaining moves than the line's length, which
   the equality also rejects — fails with `SuffixNotPlayed`. This is a
   prefix check, not a whole-suffix match: a game may legally continue
   after its certified line completes, because a proof can end in an
   unblockable double threat with moves still to play (the
   prose-specified proper-prefix case of step 3.1's gap note).
   Verification proves the line *wins*; this check proves the line
   *happened*.

Only then push the seed: the game index, the certificate's `from_ply`,
and the stored line, cloned. The first error of any kind returns
immediately — no partial seed set exists.

Watch the four tests compile and pass:

```text
cargo test -p selfplay --test anchors
```

reports **4 passed, 0 ignored**.

---

## Step 4 — `src/bin/selfplayanchors.rs` and `tests/binary.rs`: the extraction driver (red → green)

Create
[`src/bin/selfplayanchors.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayanchors.rs).
No manifest entry: Cargo discovers the binary by its path. The file
opens with a module-level doc comment: one line naming the binary —
re-verify certified self-play suffixes and emit milestone-4 anchor
seeds. Imports: the standard library's `File`, `PathBuf`, `env`, and
`process`; and, from the `selfplay` crate root, `extract_anchor_seeds`
and `read_corpus` — the binary speaks crate-root names only.
`serde_json` is used by path; the crate already depends on it.

### 4.1 The two binary tests, red

Extend
[`tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs)
— chapter 04's file with exactly one test — with the helpers and the
two tests below. New imports: the standard library's path types
(`Path`, `PathBuf`) and time sources (`SystemTime`, `UNIX_EPOCH`); the
engine's `Color` and `Move`; the `player` crate's `PlayerOptions`; and,
from the crate root, `AnchorSeed`, `CertifiedSuffix`, `Game`,
`GameRecord`, `Opening`, `OpeningAnswer`, `OpeningProvenance`,
`Outcome`, `PlyRecord`, `WinReason`, and `write_corpus`. The red state
is a compile error — the binary does not exist.

Three helpers, added above the tests:

- `TestDir` — the temporary-directory discipline: a unique directory
  under the system temporary directory whose name joins the prefix
  `selfplayanchors-test`, the process id, and a nanosecond timestamp; a
  `path` accessor; removal on `Drop`, ignoring errors. The directory
  cleans itself up when the test ends.
- `mv(row, col)` — the shared fixture constructor.
- `certified_record()` — the same fabricated game as step 3.1's
  fixture (seed 17, the same opening, the same six plies, the same
  outcome, the same certificate), converted with
  `GameRecord::from_game`. The duplication across test targets is
  deliberate: integration targets cannot share helpers.

The two tests:

1. `selfplayanchors_help_documents_corpus_extraction` — run the
   compiled binary, its path from Cargo's compile-time binary
   environment variable for `selfplayanchors`, with `--help`; expect
   the spawn to succeed ("selfplayanchors --help runs"); assert the
   exit status is successful; decode stdout as UTF-8; assert the flag
   strings `--corpus` and `--out` appear (one loop, the flag named in
   the failure message); and assert the text contains `verify_line`, so
   the soundness gate is visible in the interface.
2. `selfplayanchors_reads_a_corpus_and_writes_json_anchor_seeds` —
   under a fresh `TestDir`, write the one certified record into a
   corpus subdirectory with `write_corpus` at shard capacity 1
   (expected to succeed, "fabricated corpus writes"); run the binary
   with `--corpus` and `--out` naming a JSON path inside the temporary
   directory; assert success with the process's stderr in the failure
   message; read the output file ("anchor JSON was written"),
   deserialize it as a vector of `AnchorSeed` through `serde_json`
   ("anchor output is valid JSON"), and assert equality against exactly
   one seed: `game_index` 0, `from_ply` 5, `line` [(5,9)].

### 4.2 `USAGE` and `Args`, green

`USAGE` is a private string constant. Its first line is "Usage:
selfplayanchors --corpus <DIR> --out <JSON>"; the remaining lines state
the contract out loud: the binary re-reads a selfplay corpus, requires
every certified suffix to pass engine `verify_line` and match the
played line, and writes replayable anchor seeds; games without
certificates emit no anchor data and no negative labels; the output
file must not already exist. The name `verify_line` must appear
verbatim — the help test asserts it.

`Args` is a private struct deriving `Debug`, `PartialEq`, and `Eq`,
holding the two parsed paths: `corpus` and `out`, both `PathBuf`.

### 4.3 `parse_args`, green

A private function consuming the argument iterator (owned strings) and
returning a `Result` of `Args` or a `String` message — no error type,
because the binary reports strings. Hand-rolled, like `selfplaygen`'s:
no dependency. Behavioral contract:

1. Both slots start empty.
2. Per argument, first consume the following argument as the flag's
   value; a missing value fails with "{arg} requires a value". Then
   match the flag name: `--corpus` fills the corpus slot, `--out`
   fills the output slot, anything else fails with "unknown flag:
   {arg}". Note the order this pins: a trailing unknown flag with no
   following argument reports the missing value, not the unknown flag.
3. After the loop, an empty corpus slot fails with "--corpus is
   required" and an empty output slot with "--out is required".

The vocabulary is two flags, both required, values as separate
arguments only.

### 4.4 `run` and `main`, green

`run` is a private function consuming the `Args` and returning a
`Result` of unit or `String`. Three steps, in execution order:

1. Read the corpus with chapter 02's `read_corpus`; a failure maps to
   "failed to read corpus: {error}".
2. Run `extract_anchor_seeds` over the records; a failure maps to
   "certificate extraction failed: {error}" — the `AnchorError`'s
   display carries the game index and the detail.
3. Open the output file through the file-options builder with write
   access and **create-new** semantics — an existing file fails the
   open, and the failure maps to "failed to create '{path}': {error}"
   with the path displayed — then serialize the seeds into it with
   `serde_json`'s pretty writer; a serialization failure maps to
   "failed to write anchor JSON: {error}". The create-new rule is the
   same overwrite discipline the corpus writer keeps: an extraction
   never replaces a seed set by accident. On success, report the count
   on stderr: "Wrote {n} verified anchor seeds".

`main` owns the exit codes. If any raw process argument is `--help` or
`-h`, print the usage to stdout and return — exit code 0; the check
runs before parsing, so `--help` works even alongside invalid flags.
Otherwise parse the arguments after the program name; on a parse
failure, print the message and the usage to stderr and exit with code
2. On a run failure, print "selfplayanchors failed: {error}" to stderr
and exit with code 1. Success falls off the end — exit code 0.

The binary carries no `#[cfg(test)]` module: unlike `selfplaygen`, its
whole specification lives in `tests/binary.rs`.

Watch both tests compile and pass, selected by name so chapter 04's
`selfplaygen` test is not the point:

```text
cargo test -p selfplay --test binary selfplayanchors
```

reports **2 passed**. The binary target now stands at 3 passed, 0
ignored, of the seven it will hold when the track is complete.

---

## Step 5 — export audit, regression, gates, capstone

### 5.1 The export audit

This step is the final audit, not the first appearance of exports — the
`anchor` re-export group landed in step 3, before the integration tests
that needed it. The crate root now re-exports exactly the chapter-05
surface alongside the chapter-04 surface:

- From `anchor` (new module, declared first): `AnchorError`,
  `AnchorSeed`, `extract_anchor_seeds`.
- From `game`: nothing new — `SuffixTracker` and `PendingSuffix` stay
  **private**, not even crate-visible; the tracker is runner
  mechanics, not contract. `Game` was already exported; its
  `certified_suffix` slot is a field, not a new name.
- From `player`: nothing — `own_proof` is a field on the already
  public `Decision`; fields need no re-export.

Deliberate absences: the binary's `USAGE`, `Args`, `parse_args`, `run`,
and `main` live in the binary and are never exported; no sample,
sampler, mix, or export module exists; no `selfplayexport` or
`selfplayverify` binary exists. If any test reached for a path deeper
than the crate root, fix the test to use these re-exports.

### 5.2 Regression

The chapter-04 targets stay green unchanged — game 5/2, corpus 10/0,
openings 5/0, schedule 1/1, dedup 1/0, the `selfplaygen` binary's unit
tests 4/0, grading 0/1 — because the tracker only reads what the
decision already carried, and the record format is untouched.
`cargo test -p train` is not required: the storage mechanics are
untouched. The move loop gained two calls, so the chapter's TDD
checklist also re-proves chapter 01's MANUAL pair in release — run it
once:

```text
cargo test -p selfplay --release --test game -- --ignored
```

The `--release` qualifier is not optional: both tests play full games.
This is the chapter's only MANUAL invocation; the chapter itself adds
no MANUAL test.

### 5.3 The standing gates

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

The first command is the fast gate, and its report needs a per-target
reading: cargo prints one report per test target, never one aggregate
line. The chapter's closing state is the sum — game 5 passed / 2
ignored, corpus 10/0, openings 5/0, schedule 1/1, dedup 1/0, the
`selfplaygen` binary's unit tests 4/0, grading 0/1, the library
target's new unit pair 2/0, anchors 4/0, binary 3/0 — **35 passed, 4
ignored** in total. The four ignored tests are exactly chapter 04's
set; this chapter adds none. The completed reference's full suite is
larger (50 passed, 4 ignored) only because chapters 06 and 07 add their
own tests; do not generalize that number into this chapter's
expectation. The focused selections re-run the chapter's surface in
isolation: the anchors target, the library's two unit tests, and the
binary's two tests by name, which leaves the later chapters' binary
tests unrun because they do not exist yet. `cargo test -p player` is
the cross-crate regression: **43 passed, 0 ignored**, with the
strengthened assertion green. The last two commands are the project's
standing quality gates; both must be green with no changes.

### 5.4 The capstone: one tiny corpus, one verified seed set

Extract anchors from a corpus you generate, and watch the boundary work
end to end. The output directory is temporary — `mktemp -d` keeps the
run out of the repository's `data/` tree. From `gomoku/`, in release
(the generation plays real games; in debug the same three games would
cost minutes):

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

The reference run of exactly this pair reported "Wrote 3 verified
anchor seeds", one per game, each certified line one ply long — at
these budgets the baseline probe certifies the final forcing move of
most games, late and short. Treat that count as the reference's
reported output of this seeded pipeline, not as a guarantee about
generation: whether every game certifies is a property of the games,
not of the extractor. The JSON shows the format end to end:
`game_index` counts the corpus order, `from_ply` counts post-opening
plies, and `line` holds board indices — a `Move` serializes as its
index (`row * 15 + col`), so `71` is row 4, column 11.

Then the boundary experiment — rerun the extraction against the same
output path:

```text
target/release/selfplayanchors \
  --corpus "$dir/corpus" --out "$dir/anchors.json"
```

The run fails: stderr carries "selfplayanchors failed: failed to create
'…': File exists (os error 17)" and the exit code is 1. The output file
must not exist, so an extraction never overwrites a seed set by
accident. A second optional experiment: re-run the generator into a
fresh directory with a different `--seed-base` and extract again —
whether the seed count moves is a property of the new games, and a game
without a certificate contributes no anchor seed and causes no error.

---

## Done when

- `cargo test -p selfplay` reports **35 passed, 4 ignored** as the sum
  of its per-target reports: game 5 passed / 2 MANUAL-ignored
  (unchanged), corpus 10/0 (unchanged), openings 5/0 (unchanged),
  schedule 1/1 (unchanged), dedup 1/0 (unchanged), the `selfplaygen`
  binary's unit tests 4/0 (unchanged), grading 0/1 (unchanged), the
  library target 2/0 (this chapter's tracker pair), anchors 4/0 (new),
  binary 3/0 (this chapter's two plus chapter 04's one).
- `cargo test -p player` reports **43 passed, 0 ignored**, with the
  strengthened `own_proof_plays_the_shortest_certified_line` green —
  the count unchanged, the assertion added.
- `cargo test -p train` is not required: the record format and the
  storage mechanics are untouched.
- No new MANUAL test exists; the four ignored tests are exactly chapter
  04's set.
- `cargo clippy --all-targets -- -D warnings` is green.
- `cargo fmt --all -- --check` makes no changes.
- The capstone extracted a seed set into a temporary directory, and the
  rerun against the existing output failed with the file-exists error
  and exit code 1.
- The commit message is
  `feat(selfplay): certified endgames — selfplay-corpora chapter 05`.

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `player/src/lib.rs` | `Decision.own_proof` | public optional `engine::Proof`, placed after `search`; derive list unchanged; doc states the retention rule |
| `player/src/lib.rs` | three construction sites | bypass stores the exact probe proof (map-pairing idiom, `certified_own_move` unchanged); resignation and ordinary search store `None`; no selection, evaluator, or RNG change |
| `player/src/lib.rs` | `decide` doc | the retention sentence appended after the bypass sentence |
| `player/tests/decisions.rs` | `own_proof_plays_the_shortest_certified_line` | strengthened, not added: the equality assertion against `Some(shortest)` sits between the action and search assertions; suite stays 43/0 |
| `selfplay/src/game.rs` | `SuffixTracker`, `PendingSuffix` | private, derived `Default`; the retention comment verbatim; begin replaces on a newer proof and is a no-op without one; observe advances, promotes, or discards, and is a no-op without a candidate; finish yields the last completed certificate; the `u16` conversion expects "a game has at most 225 plies" |
| `selfplay/src/game.rs` | runner wiring | tracker bound in setup; per-ply order decide → begin → play → observe → record; resignation arm untouched; the `Game` slot is the finish yield; the slot's doc is the reference wording; everything else byte-identical |
| `selfplay/src/game.rs` (unit) | the tracker pair | shared ten-stone VCF fixture (the engine test's script); decision-driven proof in test one, prover-driven in test two; departure at (14,14) yields `None`; the library target's first tests |
| `selfplay/src/anchor.rs` | `AnchorSeed` | three documented public fields (`game_index: usize`, `from_ply: u16`, `line`); `Debug`, `Clone`, `PartialEq`, `Eq`, and serde's two derives; the doc states the replay instructions and the plane-independence rule |
| `selfplay/src/anchor.rs` | `AnchorError` | five variants in the reference declaration order — `Opening`, `Replay`, `SuffixOutOfBounds`, `SuffixNotPlayed`, `VerificationFailed` — with the exact fields and displays; `Debug` + the error derive only |
| `selfplay/src/anchor.rs` | `extract_anchor_seeds` | slice in, `Result<Vec<AnchorSeed>, AnchorError>` out; skip → bounds → opening → prefix replay → `verify_line` with the replayed side to move as winner → played-prefix check; first error aborts; one seed per surviving certificate |
| `selfplay/tests/anchors.rs` | four tests | the shared fabricated record (open four on row 5, certificate at `from_ply` 5); the one-seed equality; the empty-vector skip; `SuffixNotPlayed` for the verified alternative [(5,4)]; `VerificationFailed` for the doctored [(0,0)]; no ninth test |
| `selfplay/src/bin/selfplayanchors.rs` | `USAGE`, `Args`, `parse_args` | both flags required, separate values only; value consumed before the flag match; the usage names both flags and `verify_line` and states the no-negative-labels and no-overwrite rules |
| `selfplay/src/bin/selfplayanchors.rs` | `run`, `main` | read → extract → create-new open → pretty JSON; stderr count "Wrote {n} verified anchor seeds"; help → 0, parse failure → message + usage → 2, run failure → "selfplayanchors failed: …" → 1; no unit-test module |
| `selfplay/tests/binary.rs` | two tests + `TestDir`, `mv`, `certified_record` | help asserts both flags and `verify_line`; the corpus round trip writes at shard capacity 1 and parses the pretty JSON into the one expected seed; the file will hold seven tests, this chapter adds two |
| `selfplay/src/lib.rs` | `anchor` first, three re-exports | exported before the integration tests run; the tracker stays private; no chapter-06+ module or binary named anywhere |
| untouched | `Cargo.toml`, `src/record.rs`, `src/corpus.rs`, `src/schedule.rs`, `src/opening.rs`, `src/generate.rs`, `train`, `engine`, `mcts`, `baseline`, `net` | no manifest changes; the record slot fills through the runner alone |

## References

**This chapter:**

- [certified-endgames.md](certified-endgames.md) — the chapter paper:
  intention, specification as tests, execution-order walkthrough, the
  soundness boundary.
- [certified-endgames-design.md](certified-endgames-design.md) — the
  conceptual story, the tracker state machine, and the eight design
  invariants this plan builds under.
- [certified-endgames-solution.md](certified-endgames-solution.md) —
  the annotated reference snippets. Open only after twenty stuck
  minutes, or afterwards to compare.

**This track:**

- [Tutorial README](../README.md) — conventions, gates, the chapter
  map, and the reference provenance.
- [Self-play primer](../00-selfplay-primer.md) — §4.5 states this
  chapter's contract; the glossary defines the certified suffix and the
  anchor seed.
- [Chapter 01 — the game runner](../01-the-game-runner/the-game-runner.md)
  and its [implementation plan](../01-the-game-runner/the-game-runner-implementation.md)
  — the move loop this chapter wires into, the single-RNG discipline
  the retention respects, and the serde addition to the `player`
  manifest that step 1 relies on.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md)
  and its [implementation plan](../02-recording-games/recording-games-implementation.md)
  — the `CertifiedSuffix` type, the `Game` and `GameRecord` slots, the
  preservation test this chapter activates, and the corpus mechanics
  (`write_corpus`, `read_corpus`) the tests and the binary reuse.
- [Chapter 03 — recording the search](../03-recording-the-search/recording-the-search.md)
  and its [implementation plan](../03-recording-the-search/recording-the-search-implementation.md)
  — the honesty rule for certified-proof moves (`search: None`) and
  the search-statistics conversion that step 2's wiring precedes.
- [Chapter 04 — diversity and grading](../04-diversity-and-grading/diversity-and-grading.md)
  and its [implementation plan](../04-diversity-and-grading/diversity-and-grading-implementation.md)
  — the generator whose baseline-probe seats produce the certificates,
  the schedule binding the tracker sits next to, and the MANUAL
  discipline this chapter leaves unchanged.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  §3.4 the certified-endgame contract and the no-negative-labels rule;
  §5 the acceptance gates chapter 07 runs.
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  this chapter's public surface and the measured headline numbers.
- [docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) —
  §12 the soundness gate; the claim-label convention (**[paper]** /
  **[derived]** / **[experiment]**).
- [docs/13-engine-design.md](../../../13-engine-design.md) — the
  soundness-over-completeness decision behind the bounded prover.

**Prerequisite tracks:**

- [Baseline tutorial](../../baseline-tutorial/README.md) —
  `AiPlayer::decide`, the own-side TSS probe, `OwnTssConfig::baseline()`
  (10 000 nodes / 30 plies), and the certified bypass that now retains
  its proof.
- [Engine tutorial](../../13-engine-tutorial/README.md) — the TSS
  prover and `verify_line`, the budget-honesty rule, and the
  `vcf_win_in_three` fixture script this chapter's tracker fixture
  shares.

**Source (live on `main`):**

- [`gomoku/crates/engine/src/tss.rs`](../../../../gomoku/crates/engine/src/tss.rs) —
  `Proof`, `prove_shortest_forced_win`, `verify_line`. Consumed,
  unchanged.
- [`gomoku/crates/engine/src/moveset.rs`](../../../../gomoku/crates/engine/src/moveset.rs) —
  `Move` and its index serialization, the shape the JSON seed file
  shows. Consumed, unchanged.
- [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs) —
  `Decision` gains `own_proof`; the bypass stores the proof, the other
  two paths store `None`. Touched in this chapter.
- [`gomoku/crates/player/tests/decisions.rs`](../../../../gomoku/crates/player/tests/decisions.rs) —
  one strengthened assertion. Touched in this chapter.

**Source (created by chapters 01–05; not yet present on `main`):**

- `gomoku/crates/selfplay/src/game.rs` — gains the private
  `SuffixTracker` and `PendingSuffix`, the three call sites in the move
  loop, the `Game` assembly change, the slot's doc update, and the two
  unit tests. Touched in this chapter.
- `gomoku/crates/selfplay/src/anchor.rs` — new: `AnchorSeed`, the
  five-variant `AnchorError`, and `extract_anchor_seeds`. Created in
  this chapter.
- `gomoku/crates/selfplay/src/bin/selfplayanchors.rs` — new: the
  extraction binary. Created in this chapter.
- `gomoku/crates/selfplay/src/lib.rs` — declares the `anchor` module
  and re-exports the new vocabulary. Touched in this chapter.
- `gomoku/crates/selfplay/src/record.rs` — the `CertifiedSuffix` slot,
  unchanged since chapter 02. Not touched in this chapter.
- `gomoku/crates/selfplay/tests/anchors.rs` — the extraction
  specification, four tests. Created in this chapter.
- `gomoku/crates/selfplay/tests/binary.rs` — gains the three helpers
  and this chapter's two binary tests. Touched in this chapter.
- [`gomoku/crates/selfplay/Cargo.toml`](../../../../gomoku/crates/selfplay/Cargo.toml) —
  no manifest changes.

**External sources:**

- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://doi.org/10.26481/dis.19940923la — threat-space search, the
  prover and verifier behind every certificate this chapter handles.
- Rosin, C. D. (2011). *Multi-armed bandits with episode context*
  (PUCT). https://doi.org/10.1007/s10472-011-9258-6 — the selection
  rule of the ordinary search path the retention leaves untouched.
