> **Opt-in solution — read it only after attempting the chapter with the
> [implementation plan](certified-endgames-implementation.md), or after being
> stuck for more than twenty minutes.**
>
> This paper is inspiration, not authority over the learner's own code. The
> learner's implementation on `main` is the reference; the artifacts below
> exist so a stuck or finished learner can compare. No learner source is
> changed by this paper.

# Chapter 05 — Certified endgames: solution

## Abstract

This paper is the annotated reference companion to
[chapter 05 of the selfplay-corpora tutorial](certified-endgames.md). It
quotes everything the chapter creates or changes, along the execution path
of one certificate: the `player` crate's `Decision.own_proof` field with its
doc-comment wording, the three construction sites that pin the retention
contract, the `decide` doc update, and the one strengthened assertion in
`tests/decisions.rs`; the game runner's `engine::Proof` import, the
`Game.certified_suffix` slot doc, the private `SuffixTracker` and
`PendingSuffix` types with their begin/observe/finish protocol, the three
wiring regions in the move loop, and the two tracker unit tests; the new
`src/anchor.rs` with `AnchorSeed`, the five-variant `AnchorError`, and the
five checks of `extract_anchor_seeds`; the new `selfplayanchors` binary
(usage text, argument parser, run path, exit codes); the four extraction
tests in `tests/anchors.rs`; the two binary tests plus their `TestDir`, `mv`,
and `certified_record` helpers in `tests/binary.rs`; and the crate-root
exports in `src/lib.rs`. The paper closes with the chapter's verification
evidence: the focused anchor, library, and binary runs, the full `selfplay`
suite at 35 passed and 4 ignored, the `player` suite at 43 passed and 0
ignored with the strengthened assertion green, chapter 01's MANUAL pair in
release mode, the release capstone (three fresh games at seed base 555, three
verified anchor seeds, and the create-new rerun failing with exit code 1),
and the standing clippy and rustfmt gates. Every snippet carries an
explanation, source line numbers, and an exact provenance note: the six
files this chapter copies or creates in full —
[`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs),
[`gomoku/crates/player/tests/decisions.rs`](../../../../gomoku/crates/player/tests/decisions.rs),
`src/game.rs`, `src/anchor.rs`, `src/bin/selfplayanchors.rs`, and
`tests/anchors.rs` — are byte-identical to the completed reference
implementation, and the two that are not — `src/lib.rs` and
`tests/binary.rs` — are chapter-5 slice forms whose every difference from
the completed reference is named explicitly. All chapter gates ran green on
the chapter-5 slice on 2026-11-13; the precise evidence, including the
soundness boundary's fresh-versus-reference split, is in the Provenance,
Verification, and Soundness evidence sections. The intended reader is the
learner who has read the chapter paper and, ideally, worked the
[implementation plan](certified-endgames-implementation.md) first.

## Glossary

| Term | Definition |
|------|------------|
| **Threat-space search (TSS)** | A proof search over forced-win threat sequences (Allis 1994). The engine ships a bounded prover (`prove_forced_win`, `prove_shortest_forced_win`) and a verifier (`verify_line`); the prover may be incomplete, never unsound **[paper]**. |
| **Proof** | `engine::Proof`: a certified forced win — the winning side (`winner`) and one forcing sequence (`line`). The line alternates attacker and defender moves, starts with the attacker's first threat, and ends when the attacker completes five (overlines count) or creates an unblockable double threat. |
| **Attacker / defender** | The two roles inside a proof line. The attacker is the side with the forced win (`proof.winner`); the defender is its opponent. Attacker moves sit at even offsets of the line, defender replies at odd offsets. |
| **verify_line** | `engine::verify_line(board, proof) -> bool`: the soundness net. It replays the line from the board and, at every defender turn, enumerates every reply that addresses the threat; the attacker's continuation must still win against all of them. `true` only if the proof is airtight, and `false` also when the proof's winner is not the side to move. |
| **Own-side TSS probe** | The `AiPlayer::decide` step that runs `prove_shortest_forced_win` for the side to move before any search (baseline preset: 10 000 nodes / 30 plies). A certified proof short-circuits the decision: the line's first move is played and no search runs. |
| **own_proof** | The `Decision` field this chapter adds: `Option<engine::Proof>`, `Some` exactly when the own-side probe certified the played move. |
| **Certified suffix** | `CertifiedSuffix { from_ply: u16, line: Vec<Move> }` (chapter 02): a terminal segment of a game in which every played move belongs to one machine-verified forced-win line. `from_ply` is the zero-based post-opening ply of the attacker's first certified move. |
| **SuffixTracker** | The runner's private detector, created in this chapter: it begins a pending candidate when a decision carries an own proof and observes every played move against the candidate's line; a departure discards the candidate, and a completed line becomes the game's certified suffix. |
| **Pending candidate** | The tracker's in-flight certificate: the private `PendingSuffix` value holding a `CertifiedSuffix` under construction plus `next`, the index of the line move expected next. |
| **Anchor seed** | `AnchorSeed { game_index, from_ply, line }`: replay instructions for one milestone-4 anchor position. Replay the source game's opening and its first `from_ply` post-opening moves; at that position `line[0]` is the forcing move and the verified value is +1 for the side to move (−1 for its opponent). |
| **Milestone 4** | The project's puzzle-solving milestone (chapter 12 of the wiki): an anchor set of (position, forcing move, ±1) triples whose positions come from real self-play games. This chapter produces its input. |
| **Swap2** | The project's opening protocol: one side places three stones, the other side chooses a color or places two more stones, and the first side then chooses a color. The placements are the ceremony, not plies. |
| **Post-opening ply** | One played move after the Swap2 ceremony, counted from zero. `from_ply`, the tracker, and the extraction errors count in these plies. |
| **Corpus** | A set of game records produced at one declared pairing, stored as shards plus a manifest (chapter 02). |
| **Soundness gate** | The project's rule (chapter 12 §12 of the wiki): every certificate consumed downstream must pass `verify_line`; 100 % must verify. |
| **VCF** | Victory by continuous forcing threats: a forced-win line in which every attacker move creates a threat the defender must answer. The tracker unit-test fixture is a VCF win-in-3. |
| **PUCT** | Predictor UCT (Rosin 2011), the selection rule of the Monte Carlo tree search both seats run when no probe certifies. |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games. It never runs by default; it runs on explicit selection, in release mode. This chapter adds none. |
| **Chapter-5 slice** | The chapter-5 subset of the completed reference: chapters 01–05 together, without the later chapters' additions. Defined and verified in the Provenance section. |
| **TDD** | Test-driven development: write the test, watch it fail (red), implement, watch it pass (green). |
| **Claim labels** | The project's provenance labels (chapter 12): **[paper]** (stated in a cited paper), **[derived]** (computed from cited or measured values), **[experiment]** (a free parameter, to be settled by measurement). |

## Provenance, the chapter-5 slice, and verification

### The reference and its completed-tree form

The reference implementation lives in a separate, read-only worktree beside
this repository (`rust-ml-selfplay-ref`, branch `selfplay-ref-v2`, committed
base `9028aa8` on `main`, all reference changes uncommitted). It was built
test-first and gate-verified before this tutorial was written; the
[hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md)
records its measured evidence (workspace gates green; `cargo test -p
selfplay` reporting 50 passed, 4 ignored across all test targets). The
worktree serves quotation only: never point a manifest at it, never build the
learner's crate from it, never commit there. Chapter 01's
[solution paper](../01-the-game-runner/the-game-runner-solution.md)
established the provenance discipline this paper continues; chapters 02–04
applied it to the record format, the search semantics, and the generation
pipeline this chapter leaves untouched.

One structural fact shapes everything below: the worktree holds the
**completed** crate — all seven chapters at once. Its `src/lib.rs` declares
and re-exports the chapter-06 modules (`export`, `mix`, `sample`, `sampler`)
alongside this chapter's `anchor`; its `src/bin/` carries the chapter-06 and
chapter-07 binaries (`selfplayexport`, `selfplayverify`) beside
`selfplaygen` and `selfplayanchors`; and its `tests/binary.rs` holds seven
tests, four of which (`selfplayverify_*`) belong to chapter 07. Quoting
those regions verbatim would leak later chapters and would contradict this
chapter's approved papers, which fix the chapter-5 form: the crate root
exports exactly the chapter-05 surface, and `tests/binary.rs` holds exactly
three tests (see
[the implementation plan](certified-endgames-implementation.md), steps 1
through 5). This paper therefore distinguishes two kinds of quotation:

- **Reference-verbatim fragments.** Everything this chapter touches that is
  stable in the completed reference is byte-identical between the chapter-5
  slice and the reference: the `player` crate's
  `src/lib.rs` and `tests/decisions.rs` (the reference's player changes are
  exactly this chapter's `own_proof` retention; chapters 06–07 never touch
  the crate), and the selfplay crate's `src/game.rs`, `src/anchor.rs`,
  `src/bin/selfplayanchors.rs`, and `tests/anchors.rs` (chapters 06–07 never
  touch these files either). The comparisons were scripted, not eyeballed:
  whole-file byte comparisons, logged in
  `/tmp/selfplay-ch5-verify-logs/10-provenance-diff.log` (six `IDENTICAL`
  verdicts). These fragments are quoted exactly, and each carries a note
  naming the reference file and line range it was compared against. Because
  the whole files are identical, the quoted line numbers are simultaneously
  the slice's and the reference's.
- **Chapter-5 slice forms.** Two files hold slice-form regions:
  `src/lib.rs`, where the completed reference has already grown past chapter
  05 in the module and export lists, and `tests/binary.rs`, where the
  reference appends chapter 07's four tests. This paper quotes the chapter-5
  form specified by the approved papers, and names exactly what the
  completed reference adds later. No later-chapter implementation is quoted.

### The slice was built and run

Quoting a slice the learner could not compile would violate the track's
first rule — a tutorial that does not run error-free is a defect. So the
chapter-5 slice was constructed and gate-verified on 2026-11-13 at
`/tmp/selfplay-ch5-verify` (a scratch path, not a repository artifact). The
starting point was not the reference worktree but an *untouched copy* of the
already-verified chapter-4 slice at `/tmp/selfplay-ch4-verify` (copied in
full, build directory included, so the verification runs are incremental) —
so the chapter-5 slice differs from chapter 04's verified state by exactly
this chapter's edits, and neither the chapter-4 scratch nor the reference
worktree was modified:

1. `src/game.rs`, `src/anchor.rs` (new), `src/bin/selfplayanchors.rs` (new),
   and `tests/anchors.rs` (new) were copied from the completed reference.
   Each is byte-identical to the reference file (scripted whole-file
   comparison): chapters 06–07 never touch these files, so the completed
   reference carries exactly the chapter-5 form of each. For `src/game.rs`
   this is the *final* form of the file: the whole-file difference against
   the chapter-4 slice is exactly this chapter's delta — the `Proof` import,
   the slot-doc wording, the tracker block, the three wiring regions, and
   the test module — with no later-chapter content.
2. `gomoku/crates/player/src/lib.rs` and
   `gomoku/crates/player/tests/decisions.rs` were copied from the completed
   reference. The whole-file difference against the chapter-4 slice is
   exactly this chapter's retention change: the `own_proof` field with its
   doc, the `decide` doc sentence, the paired map on the certified-own-move
   check, the three construction sites, and the one strengthened assertion.
   Chapter 01's serde derives on `ResignReason` and everything else in both
   files are untouched.
3. `tests/binary.rs` was written as the chapter-5 slice form: the completed
   reference's lines 1–142 — the chapter-04 help test, this chapter's three
   helpers, and this chapter's two `selfplayanchors_*` tests — with the
   reference's four `selfplayverify_*` tests (reference lines 144–232)
   dropped. The slice is byte-identical to the reference's lines 1–142
   (scripted prefix comparison).
4. `src/lib.rs` was written as the chapter-5 slice form: the chapter-4 file
   plus the `anchor` module declaration (first in the module list) and the
   `anchor` re-export group (first in the export list) — that is, the
   completed reference's file minus the four chapter-06 module declarations
   (`export`, `mix`, `sample`, `sampler`) and their four re-export lines.

Nothing else was touched: no manifest change (`selfplay` already depends on
`engine`, `player`, `serde`, `serde_json`, and `thiserror`; the `player`
manifest already lists `engine`), no record or corpus mechanics
(`src/record.rs` keeps the slot chapter 02 created; this chapter fills it
through the runner alone), and no chapter-06-or-later module, binary, or
test anywhere. Scripted whole-file comparisons against the chapter-4 scratch
confirm that every earlier-chapter file — `src/corpus.rs`, `src/record.rs`,
`src/opening.rs`, `src/schedule.rs`, `src/generate.rs`,
`src/bin/selfplaygen.rs`, `Cargo.toml`, and the six untouched test targets —
is unchanged, and whole-crate comparisons confirm the slice's `engine`,
`mcts`, `baseline`, `train`, `net`, `patterns`, `testkit`, and `cli` sources
are byte-identical to the reference worktree's (all logged in
`10-provenance-diff.log`).

The chapter's verification commands were run from the copy's `gomoku/`, with
stdout and stderr saved to `/tmp/selfplay-ch5-verify-logs/` (a scratch path,
not a repository artifact):

```text
cargo test -p selfplay --test anchors
cargo test -p selfplay --lib
cargo test -p selfplay --test binary selfplayanchors
cargo test -p selfplay
cargo test -p player
cargo test -p selfplay --release --test game -- --ignored
cargo build --release -p selfplay && <the capstone below>
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

All commands passed on the slice; the measured evidence — per-target counts,
timings, and the capstone's seed file — is tabled in
[§15](#15-verification), and the soundness evidence, split into fresh slice
measurements and quoted reference acceptance measurements, is in
[§16](#16-soundness-evidence-fresh-slice-versus-reference-acceptance). The
slice fragments quoted below are exactly the files those runs exercised.

### Reading rules

The section order is the execution order of one certificate, matching the
chapter paper's walkthrough: the proof is retained where it is produced —
the `player` crate's decision (sections 1–3) — tracked where the moves are
played — the runner's `SuffixTracker` and its unit tests (sections 4–7) —
re-verified where the corpus is read — `src/anchor.rs` and its four tests
(sections 8–10 and 12) — and driven by the binary last (sections 11 and 13),
with the crate-root exports (section 14) and the verification (sections
15–16) closing the chapter. Within each file the order is the file's own.

Every code fragment carries a right-aligned `NN |` line-number margin. For
fragments from files this chapter edits, the numbers are the edited
chapter-5 file's own line numbers — the same numbers the learner's files
will have after the chapter's edits land. (For the six reference-verbatim
files the numbers are also the completed reference's, because the whole
files are identical; for the two slice-form files they are the slice's.)
The margin is the only annotation added; the code text itself is quoted
exactly, and each fragment's provenance note states whether it is
byte-identical to the completed reference (with the reference's line range)
or a chapter-5 slice form (with the exact delta). Blank separators between
lines appear without a margin; no fragment skips a nonblank line inside its
quoted range.

File links in this paper point into the learner's tree on `main`. Links to
files the track creates — for example
[`gomoku/crates/selfplay/src/anchor.rs`](../../../../gomoku/crates/selfplay/src/anchor.rs) —
resolve once chapters 01–05 land. The
[`gomoku/crates/player/`](../../../../gomoku/crates/player/) links resolve
today; the `own_proof` field they will show lands with this chapter. The
reference worktree is not linked: it is temporary provenance, deleted once
the tutorial is complete on `main`, and everything needed from it is quoted
here. The scratch copies and their logs (`/tmp/selfplay-ch4-verify`,
`/tmp/selfplay-ch5-verify`, `/tmp/selfplay-ch5-verify-logs/`) are cited as
evidence of this paper's own verification; they are ephemeral and not part
of the repository.

## 1. `player/src/lib.rs`: the `Decision.own_proof` field

The chapter's only cross-crate change is one field, and it lands where the
chapter paper's walkthrough starts: the seat's decision. `Decision` gains
`own_proof: Option<engine::Proof>`, placed directly after `search` and
before `guard`. The derive list does not change — `engine::Proof` supports
`Debug`, `Clone`, and `PartialEq`, so the strengthened assertion of section
3 can compare the retained proof for equality. The doc comment is the
retention rule in two sentences: the field holds the exact verified own-side
proof that selected the move when the threat-space search (TSS) probe fired,
and ordinary searches and resignations retain no own-side proof.

**Source:** [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs),
lines 150–166 after the edit (the `Decision` struct, whole).

```rust
 150 | /// One player action and statistics from the real search, when it ran.
 151 | #[derive(Debug, Clone, PartialEq)]
 152 | pub struct Decision {
 153 |     /// Play or concede; no sentinel move represents resignation.
 154 |     pub action: DecisionAction,
 155 |     /// Real search stats; absent only for the certified own-proof bypass.
 156 |     pub search: Option<SearchStats>,
 157 |     /// Exact verified own-side proof that selected this move, if the TSS probe
 158 |     /// fired. Ordinary searches and resignations retain no own-side proof.
 159 |     pub own_proof: Option<engine::Proof>,
 160 |     /// Selection diagnostics; absent when no proposal was sampled.
 161 |     pub guard: Option<GuardDiagnostics>,
 162 |     /// Original evidence for a new resignation; absent for every play,
 163 |     /// including own-proof bypasses. Legacy resignations without retained
 164 |     /// evidence cannot reconstruct this field by rerunning the prover.
 165 |     pub resignation: Option<ResignationEvidence>,
 166 | }
```

*Provenance: byte-identical to the completed reference's
`gomoku/crates/player/src/lib.rs` lines 150–166 (whole-file identity holds
for the entire file; the whole-file difference against the chapter-4 slice
is exactly this chapter's retention change).*

The field's position between `search` and `guard` is not arbitrary grouping:
`search` is the field whose absence marks the certified bypass ("absent only
for the certified own-proof bypass"), and `own_proof` is the field whose
presence marks it — the bypass's two evidence slots sit side by side. Note
also that `resignation`'s doc already anticipates the bypass:
a resignation's evidence is absent for every play, "including own-proof
bypasses" — chapter 01 wrote that line, and this chapter's field finally
gives the bypass its own evidence slot.

## 2. `player/src/lib.rs`: the three construction sites and the `decide` doc

Move selection changes nowhere in this section; every edit is evidence
retention. The section walks the three `Decision` construction sites in the
order `decide` meets them, then the doc sentence that states the retention.

### 2.1 The certified own-proof bypass stores the proof

Before this chapter, the probe call chained the certified-own-move check so
that only the move survived: the `Proof` value the prover returned was
dropped at the `and_then`. The change pairs the certified move with the
proof inside the closure — the map-pairing idiom — so the pair binds as
`(mv, proof)`, and the returned decision stores the proof, wrapped in
`Some`. The `certified_own_move` helper itself is unchanged, and so are the
action (the line's first move), the absent search statistics, and the guard
diagnostics (`GuardOutcome::ProvenWin` — the bypass already reported itself
to the guard channel; the chapter adds the proof, not a new signal).

**Source:** `gomoku/crates/player/src/lib.rs`, lines 292–306 after the edit
(the probe call and the bypass construction, inside `decide`).

```rust
 292 |         if options.own.enabled {
 293 |             // One direct engine probe per decision, never cached: AiPlayer
 294 |             // holds no state besides the evaluator.
 295 |             if let Some((mv, proof)) =
 296 |                 engine::prove_shortest_forced_win(board, board.to_move(), options.own.budget)
 297 |                     .and_then(|proof| certified_own_move(board, &proof).map(|mv| (mv, proof)))
 298 |             {
 299 |                 return Ok(Decision {
 300 |                     action: DecisionAction::Play(mv),
 301 |                     search: None,
 302 |                     own_proof: Some(proof),
 303 |                     guard: Some(GuardDiagnostics::new(mv, GuardOutcome::ProvenWin)),
 304 |                     resignation: None,
 305 |                 });
 306 |             }
```

*Provenance: byte-identical to the completed reference's
`gomoku/crates/player/src/lib.rs` lines 292–306.*

Why retain the proof instead of re-proving in the runner? The source comment
above the runner's tracker (section 5) states the reasoning: the player
supplies the exact proof that selected the move, and re-proving could choose
a different line — the prover searches anew and its shortest-line preference
is not a promise to repeat — while duplicating the probe's work. Retention
makes the recorded certificate the same object as the evidence that drove
the decision.

### 2.2 The resignation path stores `None`

The heuristic resignation path runs after the ordinary PUCT search (predictor
UCT; Rosin 2011) has been built, and it already carries real search
statistics. It gains exactly one line: `own_proof: None`. A resignation
asserts nothing about the board beyond the verified opponent threat that
motivated it; it may not fabricate a proof-shaped hint about the resigning
side's own chances.

**Source:** `gomoku/crates/player/src/lib.rs`, lines 314–330 after the edit
(the resignation arm's construction).

```rust
 314 |         if options.resign
 315 |             && let Some(candidates) =
 316 |                 resign::consider(board, tree.node(root).edges(), engine::prove_forced_win)
 317 |         {
 318 |             return Ok(Decision {
 319 |                 action: DecisionAction::Resign(ResignReason::VerifiedOpponentVcf {
 320 |                     candidates: candidates.len(),
 321 |                 }),
 322 |                 search: Some(SearchStats::from_searched(&tree, root, board)),
 323 |                 own_proof: None,
 324 |                 guard: None,
 325 |                 resignation: Some(ResignationEvidence {
 326 |                     options: *options,
 327 |                     candidates,
 328 |                 }),
 329 |             });
 330 |         }
```

*Provenance: byte-identical to the completed reference's
`gomoku/crates/player/src/lib.rs` lines 314–330.*

### 2.3 The ordinary search path stores `None`

The final construction — the searched, guard-checked play — gains the same
one line. A searched move asserts no forced win, so `own_proof` is `None`.
The selection above it (temperature over the root edges, then the guard) is
chapter 01–04 behavior, byte-identical to what it was before this chapter.

**Source:** `gomoku/crates/player/src/lib.rs`, lines 343–357 after the edit
(the guard choice and the final construction).

```rust
 343 |         let (mv, guard) = guard::choose(
 344 |             board,
 345 |             proposal,
 346 |             tree.node(root).edges(),
 347 |             options.guard,
 348 |             engine::prove_forced_win,
 349 |         );
 350 |         Ok(Decision {
 351 |             action: DecisionAction::Play(mv),
 352 |             search: Some(SearchStats::from_searched(&tree, root, board)),
 353 |             own_proof: None,
 354 |             guard: Some(guard),
 355 |             resignation: None,
 356 |         })
 357 |     }
```

*Provenance: byte-identical to the completed reference's
`gomoku/crates/player/src/lib.rs` lines 343–357.*

### 2.4 The `decide` doc update

The doc comment's bypass sentence described the return as happening "without
consuming RNG, calling the evaluator, or constructing search statistics."
One sentence is appended directly after it, stating where the proof goes.
The rest of the contract — the order of search, resignation, noise, and
temperature selection — is unchanged prose about unchanged behavior.

**Source:** `gomoku/crates/player/src/lib.rs`, lines 255–263 after the edit
(the head of the `decide` doc comment).

```rust
 255 |     /// Decide whether to play or concede without modifying `board`.
 256 |     ///
 257 |     /// A certified own forced win plays the first move of the shortest verified
 258 |     /// line found within budget and returns immediately without consuming RNG,
 259 |     /// calling the evaluator, or constructing search statistics. The decision
 260 |     /// retains that exact proof in `own_proof` for downstream suffix auditing.
 261 |     /// Otherwise the order is search, optional heuristic resignation, optional root
 262 |     /// noise, then temperature selection over the root edges (zero
 263 |     /// temperature breaks visit ties by prior). Noise is deliberately applied
```

*Provenance: byte-identical to the completed reference's
`gomoku/crates/player/src/lib.rs` lines 255–263.*

Because the bypass already consumed no search, no evaluator call, and no
randomness, adding the retained proof to the decision changes no selection
and no random stream: the chapter-05 difference against the chapter-4 slice
is exactly the lines quoted in sections 1–3 (scripted whole-file comparison;
every other line of the 520-line file is untouched), so the determinism
story of chapters 01 and 04 — one seed, one stream, byte-identical
regeneration — is untouched by construction. The fresh evidence agrees: the
chapter-01 MANUAL pair still passes in release through the rewired loop
(§15), and the capstone's three fresh games certify exactly as the
reference's run of the same command did (§15).

## 3. `player/tests/decisions.rs`: the strengthened assertion

No test is added to the `player` crate; the chapter strengthens the existing
`own_proof_plays_the_shortest_certified_line`. The fixture is the file's
non-minimal VCF board — a 22-stone script on which the ordinary prover and
the shortest-line prover certify *different* first moves, which is what
makes the test's two prover calls distinguishable evidence:

**Source:** [`gomoku/crates/player/tests/decisions.rs`](../../../../gomoku/crates/player/tests/decisions.rs),
lines 346–375 after the edit (the fixture, unchanged by this chapter).

```rust
 346 | fn nonminimal_vcf_fixture() -> Board {
 347 |     let mut board = Board::new();
 348 |     for (row, col) in [
 349 |         (7, 4),
 350 |         (0, 0),
 351 |         (7, 5),
 352 |         (0, 2),
 353 |         (7, 6),
 354 |         (0, 4),
 355 |         (6, 6),
 356 |         (0, 6),
 357 |         (8, 8),
 358 |         (0, 8),
 359 |         (8, 11),
 360 |         (7, 11),
 361 |         (9, 11),
 362 |         (11, 7),
 363 |         (10, 11),
 364 |         (14, 0),
 365 |         (11, 8),
 366 |         (14, 2),
 367 |         (11, 9),
 368 |         (14, 4),
 369 |         (11, 10),
 370 |         (14, 6),
 371 |     ] {
 372 |         board.play(Move::new(row, col).unwrap()).unwrap();
 373 |     }
 374 |     board
 375 | }
```

*Provenance: byte-identical to the completed reference's
`gomoku/crates/player/tests/decisions.rs` lines 346–375 (whole-file identity
holds for the entire file; the whole-file difference against the chapter-4
slice is exactly the one added assertion at line 403).*

The test binds both proofs, pins that the shortest is strictly shorter and
starts elsewhere, then decides with the probe enabled. This chapter inserts
exactly one assertion — line 403 — between the action assertion and the
search-absence assertion: the decision's `own_proof` equals
`Some(shortest)`, the exact `Proof` value the probe returned. The red-green cycle of test-driven
development (TDD) runs on the strengthened test: the TDD red state
is a compile error (`Decision` has no `own_proof` field until section 1
lands); the green state leaves the suite count unchanged at 43 passed,
0 ignored, because the chapter adds an assertion, not a test.

**Source:** `gomoku/crates/player/tests/decisions.rs`, lines 377–405 after
the edit (the whole strengthened test).

```rust
 377 | #[test]
 378 | fn own_proof_plays_the_shortest_certified_line() {
 379 |     let board = nonminimal_vcf_fixture();
 380 |     let budget = engine::SearchBudget {
 381 |         max_nodes: 50_000,
 382 |         max_depth: 9,
 383 |     };
 384 |     let ordinary = engine::prove_forced_win(&board, board.to_move(), budget).unwrap();
 385 |     let shortest = engine::prove_shortest_forced_win(&board, board.to_move(), budget).unwrap();
 386 |     assert!(shortest.line.len() < ordinary.line.len());
 387 |     assert_ne!(shortest.line[0], ordinary.line[0]);

 389 |     let mut player = AiPlayer::new(Box::new(NeverEvaluate));
 390 |     let options = PlayerOptions {
 391 |         own: OwnTssConfig {
 392 |             enabled: true,
 393 |             budget,
 394 |         },
 395 |         ..PlayerOptions::default()
 396 |     };

 398 |     let decision = player
 399 |         .decide(&board, &options, &mut StdRng::seed_from_u64(8))
 400 |         .unwrap();

 402 |     assert_eq!(decision.action, DecisionAction::Play(shortest.line[0]));
 403 |     assert_eq!(decision.own_proof, Some(shortest));
 404 |     assert!(decision.search.is_none());
 405 | }
```

*Provenance: byte-identical to the completed reference's
`gomoku/crates/player/tests/decisions.rs` lines 377–405.*

## 4. `src/game.rs`: the import and the slot doc

Back in the selfplay crate, the runner's file changes in four regions plus
a test module. The first is one name in the import block: `Proof` joins the
engine import in alphabetical position. The second is the
`Game.certified_suffix` field doc, which since chapter 02 stated only the
absence semantics ("`None` until chapter 5's detection populates it") and
now states what the slot holds once the runner fills it. The absence
semantics themselves stay where chapter 02 put them: on the record's slot
in `src/record.rs`, which this chapter does not touch.

**Source:** [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs),
lines 1–5 after the edit.

```rust
   1 | use baseline::{BaselineEvaluator, Device};
   2 | use engine::{Color, Move, PlayError, Proof, Status};
   3 | use player::{AiPlayer, DecisionAction, DecisionError, PlayerOptions, ResignReason, SearchStats};
   4 | use rand::{SeedableRng, rngs::StdRng};
   5 | use thiserror::Error;
```

*Provenance: byte-identical to the completed reference's
`src/game.rs` lines 1–5 (whole-file identity holds for the entire file).*

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 85–91 after the edit
(the tail of the `Game` struct). *Created by chapter 01, extended by chapters
02 and 03; this chapter replaces the field doc at line 89.*

```rust
  85 |     /// Played moves in order, with their available search metadata.
  86 |     pub plies: Vec<PlyRecord>,
  87 |     /// Terminal winner or draw.
  88 |     pub outcome: Outcome,
  89 |     /// Latest own-side proof whose line was followed to completion during play.
  90 |     pub certified_suffix: Option<CertifiedSuffix>,
  91 | }
```

*Provenance: byte-identical to the completed reference's
`src/game.rs` lines 85–91.*

The doc's "latest" is load-bearing, and section 5's state machine is why: a
new own-side proof replaces any pending candidate, and a completed candidate
replaces any earlier certificate, so what the slot holds at the end of the
game is the *latest* proof whose line was followed to completion.

## 5. `src/game.rs`: `PendingSuffix` and `SuffixTracker`

The detector is placed between the `Game` implementation block and the
`GameError` enum, and both types are private — no `pub`, not even
crate-visible — because the tracker is runner mechanics, not contract. Above
the struct sits the retention comment the reference carries, verbatim; the
comment lives here because the tracker is the reason the proof crosses the
crate boundary at all. `PendingSuffix` is the in-flight candidate: a
`CertifiedSuffix` under construction plus `next`, the index of the line move
expected next. `SuffixTracker` holds the optional candidate and the optional
last-completed certificate, derives `Default` (both fields start empty;
there is no `new`), and implements the three-call protocol:

- `begin` consumes the current post-opening ply count and the decision's
  `own_proof` **by value**. With a proof, it starts — or wholesale replaces
  — the pending candidate: `from_ply` is the ply count converted with the
  try-from idiom into `u16`, expected with the invariant message "a game has
  at most 225 plies" (the 15×15 board is the bound); the candidate's `line`
  is the proof's line, moved out of the proof; `next` starts at zero.
  Without a proof, `begin` changes nothing — a pending candidate survives
  the defender's proof-less replies, because the defender's forced blocks
  are part of the line being tracked.
- `observe` consumes the played move after the engine accepted it. With no
  candidate in flight it does nothing. With a candidate, the get-and-compare
  idiom compares the move against the line's element at `next`: a mismatch
  discards the candidate entirely (any earlier completed certificate is
  kept); a match advances `next`, and when `next` reaches the line's length
  the candidate promotes into the certified slot — the take-and-map idiom
  leaves `pending` empty — replacing any earlier completed certificate.
- `finish` consumes the tracker and returns the last completed certificate,
  or none. An unfinished candidate yields nothing.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 100–143 after the
edit (the comment, both types, and the implementation block). *Created in
this chapter.*

```rust
 100 | // The player supplies the exact proof that selected the move. Re-proving here
 101 | // could choose a different line and would duplicate the probe's work.
 102 | #[derive(Default)]
 103 | struct SuffixTracker {
 104 |     pending: Option<PendingSuffix>,
 105 |     certified: Option<CertifiedSuffix>,
 106 | }

 108 | struct PendingSuffix {
 109 |     suffix: CertifiedSuffix,
 110 |     next: usize,
 111 | }

 113 | impl SuffixTracker {
 114 |     fn begin(&mut self, from_ply: usize, proof: Option<Proof>) {
 115 |         if let Some(proof) = proof {
 116 |             self.pending = Some(PendingSuffix {
 117 |                 suffix: CertifiedSuffix {
 118 |                     from_ply: u16::try_from(from_ply).expect("a game has at most 225 plies"),
 119 |                     line: proof.line,
 120 |                 },
 121 |                 next: 0,
 122 |             });
 123 |         }
 124 |     }

 126 |     fn observe(&mut self, mv: Move) {
 127 |         let Some(pending) = &mut self.pending else {
 128 |             return;
 129 |         };
 130 |         if pending.suffix.line.get(pending.next) != Some(&mv) {
 131 |             self.pending = None;
 132 |             return;
 133 |         }
 134 |         pending.next += 1;
 135 |         if pending.next == pending.suffix.line.len() {
 136 |             self.certified = self.pending.take().map(|pending| pending.suffix);
 137 |         }
 138 |     }

 140 |     fn finish(self) -> Option<CertifiedSuffix> {
 141 |         self.certified
 142 |     }
 143 | }
```

*Provenance: byte-identical to the completed reference's
`src/game.rs` lines 100–143.*

One discipline deserves a plain statement, because it is the design's
honesty made executable: the tracker follows the exact recorded line, move
for move, attacker threats and defender replies alike. A defender who
answers a gapped four with the alternative block — a reply `verify_line`
has proven the line still beats — departs from the recorded main line, the
pending candidate is discarded, and the win simply goes unrecorded unless a
later probe certifies a fresh line. Unrecorded is not an error: the game
lost an anchor candidate, nothing else. The machine certifies the recorded
line, not the winning position.

## 6. `src/game.rs`: the runner wiring

Three insertions and one replacement in `play_game_with_opening`; everything
else in the function stays byte-identical to chapter 04.

### 6.1 The setup binding

The tracker is bound through its derived `Default` after the plies vector is
created and before the chapter-04 schedule binding — before the loop,
because the candidate state must outlive every ply.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 200–208 after the
edit (the runner setup). *Created by chapter 01, extended by chapters 02–04;
this chapter inserts line 206.*

```rust
 200 |     let mut rng = StdRng::seed_from_u64(seed);
 201 |     let opening = opening_source.opening(&mut rng);
 202 |     let mut board = opening.board()?;
 203 |     let mut black = AiPlayer::new(Box::new(BaselineEvaluator::new(Device::flex())));
 204 |     let mut white = AiPlayer::new(Box::new(BaselineEvaluator::new(Device::flex())));
 205 |     let mut plies = Vec::new();
 206 |     let mut suffix = SuffixTracker::default();
 207 |     let schedule = ExplorationSchedule::default();

```

*Provenance: byte-identical to the completed reference's
`src/game.rs` lines 200–208.*

### 6.2 The per-ply order: decide, begin, play, observe, record

In the loop's play arm, `begin` runs immediately after the decision and
before the chapter-03 search-statistics conversion, consuming the `own_proof`
field out of the decision with the current ply count — `plies.len()`,
zero-based and post-opening, because the Swap2 placements are the ceremony,
not plies. The engine's legality gate and the `observe` call follow the
accepted move, in that order; the ply record is appended last. The
resignation arm is untouched: no begin, no observe — a resignation carries
no proof by construction (section 2.2), and no move is played to observe.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 226–247 after the
edit (the schedule application through the end of the match). *Chapter 04's
lines are byte-identical here; this chapter inserts lines 230 and 237.*

```rust
 226 |         let effective_options = schedule.options_for_ply(*options, plies.len());
 227 |         let decision = seat.decide(&board, &effective_options, &mut rng)?;
 228 |         match decision.action {
 229 |             DecisionAction::Play(mv) => {
 230 |                 suffix.begin(plies.len(), decision.own_proof);
 231 |                 let search = decision.search.map(PlySearch::from);
 232 |                 board.play(mv).map_err(|source| GameError::Play {
 233 |                     ply: plies.len(),
 234 |                     mv,
 235 |                     source,
 236 |                 })?;
 237 |                 suffix.observe(mv);
 238 |                 plies.push(PlyRecord { mv, search });
 239 |             }
 240 |             DecisionAction::Resign(reason) => {
 241 |                 break Outcome::Won {
 242 |                     winner: side.other(),
 243 |                     reason: WinReason::Resignation(reason),
 244 |                 };
 245 |             }
 246 |         }
 247 |     };
```

*Provenance: byte-identical to the completed reference's
`src/game.rs` lines 226–247.*

### 6.3 The assembly: `finish` into the slot

The returned `Game` carries the tracker's `finish` yield in the slot where
the runner has written `None` since chapter 02. From there the durable path
is pure chapter 02: `GameRecord::from_game` copies the slot verbatim into
the record, and the shard writer persists it — no record field changes
shape, and chapter 02's preservation test now guards a slot the runner
actually fills.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 249–258 after the
edit (the `Game` assembly). *This chapter replaces the slot's value at line
256.*

```rust
 249 |     Ok(Game {
 250 |         seed,
 251 |         black_options,
 252 |         white_options,
 253 |         opening,
 254 |         plies,
 255 |         outcome,
 256 |         certified_suffix: suffix.finish(),
 257 |     })
 258 | }
```

*Provenance: byte-identical to the completed reference's
`src/game.rs` lines 249–258.*

## 7. `src/game.rs`: the two unit tests

The file's first unit tests — the library target ran zero tests before this
chapter — live in a `#[cfg(test)]` module at the end of the file. Both tests
run one TSS probe each on a ten-stone fixture and play no game; they belong
in the default suite and finish in well under a second.

### 7.1 The shared VCF fixture

The module imports the engine's `Board`, the `player` crate's `AiPlayer` and
`OwnTssConfig`, and the seeded-RNG machinery, plus the parent module's
items. The fixture plays ten stones in order — (7,4), (7,3), (7,5), (0,0),
(7,6), (0,2), (6,6), (0,4), (8,8), (0,6) — leaving Black to move with a VCF
win-in-3: Black's (7,7) completes a closed four on row 7, White must block
at (7,8), and Black's (5,5) completes a diagonal open four — the
unblockable double threat that ends the proof. The
script is the same stone sequence as the engine's own `vcf_win_in_three`
test in
[`gomoku/crates/engine/src/tss.rs`](../../../../gomoku/crates/engine/src/tss.rs)
(the engine test's comment spells out the same two threats), so the prover's
behavior on the fixture is pinned upstream — the coordinate lists were
compared programmatically and are identical.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 260–285 after the
edit (the module header, imports, and fixture). *Created in this chapter.*

```rust
 260 | #[cfg(test)]
 261 | mod tests {
 262 |     use engine::Board;
 263 |     use player::{AiPlayer, OwnTssConfig};
 264 |     use rand::{SeedableRng, rngs::StdRng};

 266 |     use super::*;

 268 |     fn vcf_fixture() -> Board {
 269 |         let mut board = Board::new();
 270 |         for (row, col) in [
 271 |             (7, 4),
 272 |             (7, 3),
 273 |             (7, 5),
 274 |             (0, 0),
 275 |             (7, 6),
 276 |             (0, 2),
 277 |             (6, 6),
 278 |             (0, 4),
 279 |             (8, 8),
 280 |             (0, 6),
 281 |         ] {
 282 |             board.play(Move::new(row, col).unwrap()).unwrap();
 283 |         }
 284 |         board
 285 |     }
```

*Provenance: byte-identical to the completed reference's
`src/game.rs` lines 260–285.*

### 7.2 `followed_own_tss_proof_becomes_a_certified_suffix`

The first test takes its proof from a real decision: an `AiPlayer` on the
baseline evaluator decides on the fixture with the struct-update-syntax
idiom over the default player options, overriding only the own-TSS probe
configuration to `OwnTssConfig::baseline()`, on an RNG seeded at 8. The
decision's `own_proof` is expected present — "the VCF fixture has a proof" —
and its line is cloned as the expected line. A fresh tracker begins at ply 0
with that proof; then every move of the expected line is replayed through
the real board — each play expected to succeed, "verified proof moves replay
legally", because the proof was verified inside the prover — with an
`observe` after each acceptance. `finish` yields a certificate whose
`from_ply` is 0 and whose line equals the proof's line.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 287–314 after the
edit. *Created in this chapter.*

```rust
 287 |     #[test]
 288 |     fn followed_own_tss_proof_becomes_a_certified_suffix() {
 289 |         let mut board = vcf_fixture();
 290 |         let mut player = AiPlayer::new(Box::new(BaselineEvaluator::new(Device::flex())));
 291 |         let decision = player
 292 |             .decide(
 293 |                 &board,
 294 |                 &PlayerOptions {
 295 |                     own: OwnTssConfig::baseline(),
 296 |                     ..PlayerOptions::default()
 297 |                 },
 298 |                 &mut StdRng::seed_from_u64(8),
 299 |             )
 300 |             .unwrap();
 301 |         let proof = decision.own_proof.expect("the VCF fixture has a proof");
 302 |         let expected_line = proof.line.clone();
 303 |         let mut tracker = SuffixTracker::default();
 304 |         tracker.begin(0, Some(proof));

 306 |         for mv in expected_line.iter().copied() {
 307 |             board.play(mv).expect("verified proof moves replay legally");
 308 |             tracker.observe(mv);
 309 |         }

 311 |         let suffix = tracker.finish().expect("the complete line certifies");
 312 |         assert_eq!(suffix.from_ply, 0);
 313 |         assert_eq!(suffix.line, expected_line);
 314 |     }
```

*Provenance: byte-identical to the completed reference's
`src/game.rs` lines 287–314.*

### 7.3 `departing_from_the_proof_line_leaves_the_suffix_uncertified`

The second test takes its proof straight from
`engine::prove_shortest_forced_win` on the fixture at the baseline probe's
budget — no player, no RNG — begins a fresh tracker at ply 0, and observes
one move off the line: (14, 14), an empty corner no VCF line from this
fixture visits. `finish` yields `None`. A departed line is not a shortened
certificate; it is no certificate.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 316–332 after the
edit (to the end of the file). *Created in this chapter.*

```rust
 316 |     #[test]
 317 |     fn departing_from_the_proof_line_leaves_the_suffix_uncertified() {
 318 |         let board = vcf_fixture();
 319 |         let proof = engine::prove_shortest_forced_win(
 320 |             &board,
 321 |             board.to_move(),
 322 |             OwnTssConfig::baseline().budget,
 323 |         )
 324 |         .expect("the VCF fixture has a proof");
 325 |         let mut tracker = SuffixTracker::default();
 326 |         tracker.begin(0, Some(proof));

 328 |         tracker.observe(Move::new(14, 14).unwrap());

 330 |         assert_eq!(tracker.finish(), None);
 331 |     }
 332 | }
```

*Provenance: byte-identical to the completed reference's
`src/game.rs` lines 316–332.*

## 8. `src/anchor.rs`: `AnchorSeed`

The first of the chapter's two new files. The imports state the
dependencies: the engine's `Move`, `PlayError`, `Proof`, and `verify_line`;
serde's two derive macros; the `thiserror` derive; and the crate's
`GameRecord`. `AnchorSeed` is the extraction output: replay instructions for
one milestone-4 anchor position, deriving `Debug`, `Clone`, `PartialEq`,
`Eq`, and serde's `Serialize` and `Deserialize` — equality because the tests
compare seed vectors, serde because the binary writes the seeds as JSON. The
doc comment states the contract: replay the source game's opening and its
first `from_ply` post-opening moves, and at that position `line[0]` is the
forcing move and the verified value is +1 for the side to move (−1 for its
opponent); storing derivation information rather than planes keeps the seed
independent of plane encoding. The three fields are the derivation:
`game_index` (which record), `from_ply` (where in the game), and `line` (the
complete verified forced-win line whose first move is the forcing move).

**Source:** [`gomoku/crates/selfplay/src/anchor.rs`](../../../../gomoku/crates/selfplay/src/anchor.rs),
lines 1–21. *Created in this chapter.*

```rust
   1 | use engine::{Move, PlayError, Proof, verify_line};
   2 | use serde::{Deserialize, Serialize};
   3 | use thiserror::Error;

   5 | use crate::GameRecord;

   7 | /// Replay instructions for one milestone-4 anchor position.
   8 | ///
   9 | /// Replay the source game's opening and its first `from_ply` post-opening
  10 | /// moves. At that position, `line[0]` is the forcing move and the verified
  11 | /// value is +1 for the side to move (−1 for its opponent). Storing derivation
  12 | /// information rather than planes keeps the seed independent of plane encoding.
  13 | #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
  14 | pub struct AnchorSeed {
  15 |     /// Zero-based index of the source game in corpus order.
  16 |     pub game_index: usize,
  17 |     /// Zero-based post-opening ply at which the certified position occurs.
  18 |     pub from_ply: u16,
  19 |     /// Complete verified forced-win line; its first move is the forcing move.
  20 |     pub line: Vec<Move>,
  21 | }
```

*Provenance: byte-identical to the completed reference's `src/anchor.rs`
lines 1–21 (whole-file identity holds for the entire file).*

## 9. `src/anchor.rs`: `AnchorError`

The error enum derives `Debug` and the `thiserror` error derive only — no
`Clone`, no `PartialEq`: the tests match on variants with the match idiom,
so equality is not part of the contract. Five variants, in the reference's
declaration order — the vocabulary order, not the check order (the check
order is section 10's contract): `Opening` (the record's Swap2 opening was
invalid, carrying the game index and the engine's `PlayError` as the error
source), `Replay` (a pre-certificate move was illegal, adding the zero-based
post-opening ply and the rejected move, which renders with its debug format
in the display), `SuffixOutOfBounds` (the certificate names no played
position, carrying the invalid start and the record's ply count),
`SuffixNotPlayed` (the stored line is not a prefix of the moves actually
played), and `VerificationFailed` (the engine rejected the stored
certificate). Every display string names the game index first, so a failed
extraction points at its record.

**Source:** `gomoku/crates/selfplay/src/anchor.rs`, lines 23–72. *Created in
this chapter.*

```rust
  23 | /// A corpus certificate could not be replayed or verified.
  24 | #[derive(Debug, Error)]
  25 | pub enum AnchorError {
  26 |     /// The source game's Swap2 opening was invalid.
  27 |     #[error("game {game_index}: opening replay failed: {source}")]
  28 |     Opening {
  29 |         /// Zero-based corpus game index.
  30 |         game_index: usize,
  31 |         /// Engine rejection.
  32 |         source: PlayError,
  33 |     },
  34 |     /// A source move before the certified position was illegal.
  35 |     #[error("game {game_index}: move {mv:?} at ply {ply} was rejected: {source}")]
  36 |     Replay {
  37 |         /// Zero-based corpus game index.
  38 |         game_index: usize,
  39 |         /// Zero-based post-opening ply.
  40 |         ply: usize,
  41 |         /// Rejected move.
  42 |         mv: Move,
  43 |         /// Engine rejection.
  44 |         source: PlayError,
  45 |     },
  46 |     /// The certificate does not identify a played position.
  47 |     #[error("game {game_index}: certified from_ply {from_ply} is outside {} plies", .ply_count)]
  48 |     SuffixOutOfBounds {
  49 |         /// Zero-based corpus game index.
  50 |         game_index: usize,
  51 |         /// Invalid certificate start.
  52 |         from_ply: u16,
  53 |         /// Number of post-opening moves available for replay.
  54 |         ply_count: usize,
  55 |     },
  56 |     /// The stored proof is not a prefix of the moves actually played.
  57 |     #[error("game {game_index}: certified line from ply {from_ply} was not played")]
  58 |     SuffixNotPlayed {
  59 |         /// Zero-based corpus game index.
  60 |         game_index: usize,
  61 |         /// Certificate start supplied by the record.
  62 |         from_ply: u16,
  63 |     },
  64 |     /// The engine rejected a stored certificate.
  65 |     #[error("game {game_index}: certified line from ply {from_ply} failed verify_line")]
  66 |     VerificationFailed {
  67 |         /// Zero-based corpus game index.
  68 |         game_index: usize,
  69 |         /// Certificate start supplied by the record.
  70 |         from_ply: u16,
  71 |     },
  72 | }
```

*Provenance: byte-identical to the completed reference's `src/anchor.rs`
lines 23–72.*

## 10. `src/anchor.rs`: `extract_anchor_seeds`

The extraction pass is a public free function — no receiver — borrowing a
slice of `GameRecord` and returning a `Result` of the seed vector or an
`AnchorError`. It is pure: the binary supplies the records, and the pass
trusts nothing it has not re-checked. The doc comment states the honesty
contract — every stored certificate is a positive claim that must pass
`engine::verify_line`, one failure aborts the entire extraction, and records
without certificates contribute no seeds and, crucially, no negative
labels — and the `# Errors` section names the failure kinds. That last
sentence is about kinds, not all five variants: the reference comment's
"opening, replay, bounds, or line-verification failure" lists four failure
kinds for the five-variant `AnchorError`; "line-verification" covers only
`VerificationFailed`, and the fifth variant, `SuffixNotPlayed`, is the
separate played-prefix check covered in §10.2 — verification proves the
line *wins*; the prefix check proves the line *happened*.

### 10.1 Skip and bounds

Per record — its zero-based position in the slice is the `game_index` — the
first two checks are the cheap ones. A record without a certified suffix is
skipped with the let-else idiom and a `continue`: no seed, no error, no
negative label; absence is not a failure. Then the certificate's `from_ply`
must name a played position: converted to `usize` and compared against the
record's ply count, a start beyond the played moves fails with
`SuffixOutOfBounds`, carrying the game index, the invalid start, and the ply
count.

**Source:** `gomoku/crates/selfplay/src/anchor.rs`, lines 74–95. *Created in
this chapter.*

```rust
  74 | /// Re-verify every certified suffix and emit its replayable anchor seed.
  75 | ///
  76 | /// Every stored certificate is a positive claim and must pass
  77 | /// [`engine::verify_line`]; one failure aborts the entire extraction. Records
  78 | /// without certificates contribute no seeds and, crucially, no negative labels.
  79 | ///
  80 | /// # Errors
  81 | /// Returns the first opening, replay, bounds, or line-verification failure.
  82 | pub fn extract_anchor_seeds(records: &[GameRecord]) -> Result<Vec<AnchorSeed>, AnchorError> {
  83 |     let mut seeds = Vec::new();
  84 |     for (game_index, record) in records.iter().enumerate() {
  85 |         let Some(suffix) = &record.certified_suffix else {
  86 |             continue;
  87 |         };
  88 |         let from_ply = usize::from(suffix.from_ply);
  89 |         if from_ply >= record.plies.len() {
  90 |             return Err(AnchorError::SuffixOutOfBounds {
  91 |                 game_index,
  92 |                 from_ply: suffix.from_ply,
  93 |                 ply_count: record.plies.len(),
  94 |             });
  95 |         }
```

*Provenance: byte-identical to the completed reference's `src/anchor.rs`
lines 74–95.*

### 10.2 Replay, verify, match, emit

The remaining three checks rebuild the claim from primary evidence. The
record header's opening executes through `Opening::board`, a rejection
mapped to `Opening`; the first `from_ply` recorded moves replay through the
fresh engine board, the first rejection mapped to `Replay` with the
zero-based ply, the move, and the source. Extraction now holds the certified
position itself, rebuilt from the record's own opening and moves. Then the
soundness gate, applied a second time: a `Proof` is reconstructed with the
replayed board's side to move as the winner and the stored line cloned as
the line, and `engine::verify_line` must pass — a `false` fails with
`VerificationFailed`. The prover verified at decision time; extraction
verifies again at read time, against a board the record itself
reconstructed. Finally the played-prefix check: the record's moves from
`from_ply` onward, taken as many as the line is long, must equal the stored
line under the iterator equality idiom; a mismatch — or fewer remaining
moves than the line's length, which the equality also rejects — fails with
`SuffixNotPlayed`. Verification proves the line *wins*; this check proves
the line *happened*. Only then does the pass push the seed — game index,
`from_ply`, and the stored line cloned — and the first error of any kind
returns immediately: no partial seed set exists.

**Source:** `gomoku/crates/selfplay/src/anchor.rs`, lines 97–139 (to the end
of the file). *Created in this chapter.*

```rust
  97 |         let mut board = record
  98 |             .header
  99 |             .opening
 100 |             .board()
 101 |             .map_err(|source| AnchorError::Opening { game_index, source })?;
 102 |         for (ply, recorded) in record.plies[..from_ply].iter().enumerate() {
 103 |             board
 104 |                 .play(recorded.mv)
 105 |                 .map_err(|source| AnchorError::Replay {
 106 |                     game_index,
 107 |                     ply,
 108 |                     mv: recorded.mv,
 109 |                     source,
 110 |                 })?;
 111 |         }
 112 |         let proof = Proof {
 113 |             winner: board.to_move(),
 114 |             line: suffix.line.clone(),
 115 |         };
 116 |         if !verify_line(&board, &proof) {
 117 |             return Err(AnchorError::VerificationFailed {
 118 |                 game_index,
 119 |                 from_ply: suffix.from_ply,
 120 |             });
 121 |         }
 122 |         let played_line = record.plies[from_ply..]
 123 |             .iter()
 124 |             .map(|ply| ply.mv)
 125 |             .take(suffix.line.len());
 126 |         if !played_line.eq(suffix.line.iter().copied()) {
 127 |             return Err(AnchorError::SuffixNotPlayed {
 128 |                 game_index,
 129 |                 from_ply: suffix.from_ply,
 130 |             });
 131 |         }
 132 |         seeds.push(AnchorSeed {
 133 |             game_index,
 134 |             from_ply: suffix.from_ply,
 135 |             line: suffix.line.clone(),
 136 |         });
 137 |     }
 138 |     Ok(seeds)
 139 | }
```

*Provenance: byte-identical to the completed reference's `src/anchor.rs`
lines 97–139.*

The prefix check is a prefix check, not a whole-suffix match, and that is
deliberate: a proof can end in an unblockable double threat with moves still
to play, so a game may legally continue after its certified line completes,
and the trailing moves do not invalidate the certificate. One honesty note,
stated in the chapter paper and held here: every chapter fixture ends
exactly when its certified line completes, so the proper-prefix case —
trailing moves after the line — is prose-specified, source-verified behavior
of the reference implementation that no chapter test pins. The behavior is
visible in the source above (the `take` limits the comparison to the line's
length; nothing compares the remainder), and no ninth test fills that gap in
this chapter.

## 11. `src/bin/selfplayanchors.rs`: the extraction driver

The second new file is a thin driver — hand-rolled argument parsing like
`selfplaygen`, no dependency, no manifest entry (Cargo discovers the binary
by its path), and no `#[cfg(test)]` module: unlike `selfplaygen`, its whole
specification lives in `tests/binary.rs` (section 13).

### 11.1 Declaration, `USAGE`, and `Args`

The module-level doc comment names the binary in one line. The imports are
the standard library's `File`, `PathBuf`, `env`, and `process`, plus the two
crate-root functions the run path needs — the binary speaks crate-root names
only. `USAGE` states the contract out loud: the binary re-reads a selfplay
corpus, requires every certified suffix to pass engine `verify_line` and
match the played line, writes replayable anchor seeds, emits no anchor data
and no negative labels for games without certificates, and refuses to
overwrite an existing output file. The name `verify_line` appears verbatim —
the help test asserts it. `Args` is the two parsed paths, deriving `Debug`,
`PartialEq`, and `Eq`.

**Source:** [`gomoku/crates/selfplay/src/bin/selfplayanchors.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayanchors.rs),
lines 1–19. *Created in this chapter.*

```rust
   1 | //! Re-verify certified self-play suffixes and emit milestone-4 anchor seeds.

   3 | use std::fs::File;
   4 | use std::path::PathBuf;
   5 | use std::{env, process};

   7 | use selfplay::{extract_anchor_seeds, read_corpus};

   9 | const USAGE: &str = "Usage: selfplayanchors --corpus <DIR> --out <JSON>\n\
  10 | Re-read a selfplay corpus, require every certified suffix to pass engine\n\
  11 | verify_line and match the played line, then write replayable anchor seeds.\n\
  12 | Games without certificates emit no anchor data and no negative labels.\n\
  13 | The output file must not already exist.";

  15 | #[derive(Debug, PartialEq, Eq)]
  16 | struct Args {
  17 |     corpus: PathBuf,
  18 |     out: PathBuf,
  19 | }
```

*Provenance: byte-identical to the completed reference's
`src/bin/selfplayanchors.rs` lines 1–19 (whole-file identity holds for the
entire file).*

### 11.2 `main` and `run`: the three-step path and the exit codes

`main` owns the exit codes. If any raw process argument is `--help` or `-h`,
the usage prints to stdout and the process returns — exit code 0; the check
runs before parsing, so `--help` works even alongside invalid flags.
Otherwise the arguments after the program name parse; a parse failure prints
the message and the usage to stderr and exits with code 2; a run failure
prints `selfplayanchors failed: …` and exits with code 1; success falls off
the end. `run` is the three steps in execution order: `read_corpus` loads
the records (a failure maps to "failed to read corpus: …");
`extract_anchor_seeds` runs the pass (a failure maps to "certificate
extraction failed: …", the `AnchorError`'s display carrying the game index
and the detail); and the output file opens through the file-options builder
with write access and **create-new** semantics — an existing file fails the
open, and the failure maps to "failed to create '{path}': …" with the path
displayed — before the seeds serialize through `serde_json`'s pretty writer.
The create-new rule is the same overwrite discipline the corpus writer
keeps: an extraction never replaces a seed set by accident. On success the
count goes to stderr: "Wrote {n} verified anchor seeds".

**Source:** `gomoku/crates/selfplay/src/bin/selfplayanchors.rs`, lines 21–53.
*Created in this chapter.*

```rust
  21 | fn main() {
  22 |     if env::args().any(|arg| arg == "--help" || arg == "-h") {
  23 |         println!("{USAGE}");
  24 |         return;
  25 |     }
  26 |     let args = match parse_args(env::args().skip(1)) {
  27 |         Ok(args) => args,
  28 |         Err(message) => {
  29 |             eprintln!("{message}\n{USAGE}");
  30 |             process::exit(2);
  31 |         }
  32 |     };
  33 |     if let Err(error) = run(args) {
  34 |         eprintln!("selfplayanchors failed: {error}");
  35 |         process::exit(1);
  36 |     }
  37 | }

  39 | fn run(args: Args) -> Result<(), String> {
  40 |     let records =
  41 |         read_corpus(&args.corpus).map_err(|error| format!("failed to read corpus: {error}"))?;
  42 |     let seeds = extract_anchor_seeds(&records)
  43 |         .map_err(|error| format!("certificate extraction failed: {error}"))?;
  44 |     let output = File::options()
  45 |         .write(true)
  46 |         .create_new(true)
  47 |         .open(&args.out)
  48 |         .map_err(|error| format!("failed to create '{}': {error}", args.out.display()))?;
  49 |     serde_json::to_writer_pretty(output, &seeds)
  50 |         .map_err(|error| format!("failed to write anchor JSON: {error}"))?;
  51 |     eprintln!("Wrote {} verified anchor seeds", seeds.len());
  52 |     Ok(())
  53 | }
```

*Provenance: byte-identical to the completed reference's
`src/bin/selfplayanchors.rs` lines 21–53.*

### 11.3 `parse_args`: two flags, both required

The parser consumes the argument iterator as owned strings and returns a
`Result` of `Args` or a `String` message — no error type, because the binary
reports strings. Both slots start empty; per argument, the following
argument is consumed as the flag's value *first* — a missing value fails
with "{arg} requires a value" — and only then does the flag name match:
`--corpus` fills the corpus slot, `--out` fills the output slot, anything
else fails with "unknown flag: {arg}". The order this pins is observable: a
trailing unknown flag with no following argument reports the missing value,
not the unknown flag. After the loop, an empty corpus slot fails with
"--corpus is required" and an empty output slot with "--out is required".

**Source:** `gomoku/crates/selfplay/src/bin/selfplayanchors.rs`, lines 55–72
(to the end of the file). *Created in this chapter.*

```rust
  55 | fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
  56 |     let mut corpus = None;
  57 |     let mut out = None;
  58 |     while let Some(arg) = args.next() {
  59 |         let value = args
  60 |             .next()
  61 |             .ok_or_else(|| format!("{arg} requires a value"))?;
  62 |         match arg.as_str() {
  63 |             "--corpus" => corpus = Some(PathBuf::from(value)),
  64 |             "--out" => out = Some(PathBuf::from(value)),
  65 |             other => return Err(format!("unknown flag: {other}")),
  66 |         }
  67 |     }
  68 |     Ok(Args {
  69 |         corpus: corpus.ok_or_else(|| "--corpus is required".to_owned())?,
  70 |         out: out.ok_or_else(|| "--out is required".to_owned())?,
  71 |     })
  72 | }
```

*Provenance: byte-identical to the completed reference's
`src/bin/selfplayanchors.rs` lines 55–72.*

## 12. `tests/anchors.rs`: the extraction specification

The new test target fabricates its records — no game is played anywhere in
the file — and pins all four extraction behaviors. The imports speak
crate-root names only, which is why section 14's re-exports land before
these tests are watched green.

### 12.1 The `mv` helper and the fabricated record

`mv(row, col)` is the shared fixture constructor, validating through
`Move::new`. `fabricated_record()` builds a `Game` by hand and converts it
with chapter 02's `GameRecord::from_game`: seed 17; both option sets the
defaults; an opening whose three initial stones sit at (5,5), (10,10), and
(5,6) with the take-Black answer — so White moves first after the ceremony —
and `Generated` provenance; six post-opening plies, each carrying no search
metadata — (10,11), (5,7), (10,12), (5,8), (10,13), (5,9), in which White
builds four on row 10 without finishing while Black extends the row-5 pair
from the opening to an open four at columns 5 through 8 and then completes
five at (5,9); a Black five-or-more win; and the certificate `from_ply: 5,
line: [(5,9)]`. The fixture's load-bearing property: both ends of Black's
open four win immediately, so (5,4) is a *verified* alternative line — the
tests can separate "the line is sound" from "the line was played".

**Source:** [`gomoku/crates/selfplay/tests/anchors.rs`](../../../../gomoku/crates/selfplay/tests/anchors.rs),
lines 1–45. *Created in this chapter.*

```rust
   1 | use engine::{Color, Move};
   2 | use player::PlayerOptions;
   3 | use selfplay::{
   4 |     AnchorError, AnchorSeed, CertifiedSuffix, Game, GameRecord, Opening, OpeningAnswer,
   5 |     OpeningProvenance, Outcome, PlyRecord, WinReason, extract_anchor_seeds,
   6 | };

   8 | fn mv(row: u8, col: u8) -> Move {
   9 |     Move::new(row, col).expect("test coordinate is on the board")
  10 | }

  12 | fn fabricated_record() -> GameRecord {
  13 |     let opening = Opening::new(
  14 |         [mv(5, 5), mv(10, 10), mv(5, 6)],
  15 |         OpeningAnswer::Take(Color::Black),
  16 |         OpeningProvenance::Generated,
  17 |     );
  18 |     let moves = [
  19 |         mv(10, 11),
  20 |         mv(5, 7),
  21 |         mv(10, 12),
  22 |         mv(5, 8),
  23 |         mv(10, 13),
  24 |         mv(5, 9),
  25 |     ];
  26 |     let game = Game {
  27 |         seed: 17,
  28 |         black_options: PlayerOptions::default(),
  29 |         white_options: PlayerOptions::default(),
  30 |         opening,
  31 |         plies: moves
  32 |             .into_iter()
  33 |             .map(|mv| PlyRecord { mv, search: None })
  34 |             .collect(),
  35 |         outcome: Outcome::Won {
  36 |             winner: Color::Black,
  37 |             reason: WinReason::FiveOrMore,
  38 |         },
  39 |         certified_suffix: Some(CertifiedSuffix {
  40 |             from_ply: 5,
  41 |             line: vec![mv(5, 9)],
  42 |         }),
  43 |     };
  44 |     GameRecord::from_game(&game)
  45 | }
```

*Provenance: byte-identical to the completed reference's `tests/anchors.rs`
lines 1–45 (whole-file identity holds for the entire file).*

### 12.2 `certified_suffix_emits_replayable_anchor_seed`

The positive case: extraction over the one-record corpus expects success —
"the certificate verifies" — and asserts equality against exactly one seed:
`game_index` 0, `from_ply` 5, `line` [(5,9)]. This is the round trip the
anchor set exists for: the certificate in the record re-verified against the
replayed position, matched against the played line, and emitted as replay
instructions.

**Source:** `gomoku/crates/selfplay/tests/anchors.rs`, lines 47–59. *Created
in this chapter.*

```rust
  47 | #[test]
  48 | fn certified_suffix_emits_replayable_anchor_seed() {
  49 |     let seeds = extract_anchor_seeds(&[fabricated_record()]).expect("the certificate verifies");

  51 |     assert_eq!(
  52 |         seeds,
  53 |         vec![AnchorSeed {
  54 |             game_index: 0,
  55 |             from_ply: 5,
  56 |             line: vec![mv(5, 9)],
  57 |         }]
  58 |     );
  59 | }
```

*Provenance: byte-identical to the completed reference's `tests/anchors.rs`
lines 47–59.*

### 12.3 `missing_certificate_emits_no_anchor_data`

The absence case: the same record with its certified-suffix slot set to
`None`. Extraction expects success — "missing certificates are not errors" —
and the seed vector is empty. Absence is not a failure, and it carries no
negative label.

**Source:** `gomoku/crates/selfplay/tests/anchors.rs`, lines 61–69. *Created
in this chapter.*

```rust
  61 | #[test]
  62 | fn missing_certificate_emits_no_anchor_data() {
  63 |     let mut record = fabricated_record();
  64 |     record.certified_suffix = None;

  66 |     let seeds = extract_anchor_seeds(&[record]).expect("missing certificates are not errors");

  68 |     assert!(seeds.is_empty());
  69 | }
```

*Provenance: byte-identical to the completed reference's `tests/anchors.rs`
lines 61–69.*

### 12.4 `verified_alternative_line_that_was_not_played_is_rejected`

The first negative case separates soundness from history. The certificate's
line is replaced with [(5,4)] — the other end of the open four, which
`verify_line` passes, as the fixture's comment states — but the game played
(5,9). Extraction expects failure — "a certificate must describe the line
actually played" — and the match idiom pins the error as
`AnchorError::SuffixNotPlayed` with `game_index` 0 and `from_ply` 5. A
certificate must describe the line actually played; soundness is not enough.

**Source:** `gomoku/crates/selfplay/tests/anchors.rs`, lines 71–87. *Created
in this chapter.*

```rust
  71 | #[test]
  72 | fn verified_alternative_line_that_was_not_played_is_rejected() {
  73 |     let mut record = fabricated_record();
  74 |     // Both ends of Black's open four win immediately, but the game played 5,9.
  75 |     record.certified_suffix.as_mut().unwrap().line = vec![mv(5, 4)];

  77 |     let error = extract_anchor_seeds(&[record])
  78 |         .expect_err("a certificate must describe the line actually played");

  80 |     assert!(matches!(
  81 |         error,
  82 |         AnchorError::SuffixNotPlayed {
  83 |             game_index: 0,
  84 |             from_ply: 5,
  85 |         }
  86 |     ));
  87 | }
```

*Provenance: byte-identical to the completed reference's `tests/anchors.rs`
lines 71–87.*

### 12.5 `doctored_suffix_line_is_a_hard_error`

The second negative case is tampering: the certificate's line is replaced
with [(0,0)], which wins nothing. Extraction expects failure — "tampering
must abort extraction" — and the error is `AnchorError::VerificationFailed`
with `game_index` 0 and `from_ply` 5. Tampering aborts extraction; it is
never skipped, because skipping would turn tampering into silent data loss
and aborting turns it into an investigation.

**Source:** `gomoku/crates/selfplay/tests/anchors.rs`, lines 89–103 (to the
end of the file). *Created in this chapter.*

```rust
  89 | #[test]
  90 | fn doctored_suffix_line_is_a_hard_error() {
  91 |     let mut record = fabricated_record();
  92 |     record.certified_suffix.as_mut().unwrap().line = vec![mv(0, 0)];

  94 |     let error = extract_anchor_seeds(&[record]).expect_err("tampering must abort extraction");

  96 |     assert!(matches!(
  97 |         error,
  98 |         AnchorError::VerificationFailed {
  99 |             game_index: 0,
 100 |             from_ply: 5,
 101 |         }
 102 |     ));
 103 | }
```

*Provenance: byte-identical to the completed reference's `tests/anchors.rs`
lines 89–103.*

## 13. `tests/binary.rs`: the two binary tests (slice form)

The boundary, restated: this file will hold **seven** tests when the track
is complete; chapter 04 created it with one
(`selfplaygen_help_documents_corpus_and_grading_flags`, untouched and not
re-quoted here — chapter 04's solution paper quotes it), this chapter adds
the helpers and exactly two tests, and chapter 07 adds the four
`selfplayverify_*` tests. The fragments below are **chapter-5 slice forms**:
the slice file is the completed reference's lines 1–142, and the provenance
notes name exactly what the reference appends later.

### 13.1 The helpers: `TestDir`, `mv`, `certified_record`

The imports grow to the standard library's path types and time sources, the
engine's `Color` and `Move`, the `player` crate's `PlayerOptions`, and the
crate-root names the fixture and the tests need. `TestDir` is the
temporary-directory discipline: a unique directory under the system
temporary directory whose name joins the prefix `selfplayanchors-test`, the
process id, and a nanosecond timestamp; a `path` accessor; and removal on
`Drop`, ignoring errors — the directory cleans itself up when the test ends.
`certified_record()` fabricates the same game as section 12's fixture —
seed 17, the same opening, the same six plies, the same outcome, the same
certificate — converted with `GameRecord::from_game`. The duplication across
test targets is deliberate: integration targets cannot share helpers.
Nothing in the helpers anticipates chapter 07.

**Source:** [`gomoku/crates/selfplay/tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs),
lines 1–39 and 41–72 after the edit. *Created by chapter 04; this chapter
rewrites the import block and inserts the helpers ahead of chapter 04's
test.*

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
```

```rust
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

*Provenance: chapter-5 slice form. Both fragments are byte-identical to the
completed reference's `tests/binary.rs` at the same line numbers (slice
lines 1–39 and 41–72 against reference lines 1–39 and 41–72; the slice's
lines 1–142 equal the reference's lines 1–142 by scripted prefix
comparison). Chapter 04's help test sits between the fragments at lines
74–94, byte-identical to chapter 04's form. The completed reference appends
its four `selfplayverify_*` tests at lines 144–232 — chapter 07, absent
from the slice.*

### 13.2 `selfplayanchors_help_documents_corpus_extraction`

The help contract: run the compiled binary — its path from Cargo's
compile-time binary environment variable for `selfplayanchors` — with
`--help`; expect the spawn to succeed, assert the exit status is successful,
decode stdout as UTF-8, assert the flag strings `--corpus` and `--out`
appear (one loop, the flag named in the failure message), and assert the
text contains `verify_line`, so the soundness gate is visible in the
interface.

**Source:** `gomoku/crates/selfplay/tests/binary.rs`, lines 96–109 after the
edit. *Created in this chapter.*

```rust
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
```

*Provenance: chapter-5 slice form, byte-identical to the completed
reference's `tests/binary.rs` lines 96–109 at the same line numbers.*

### 13.3 `selfplayanchors_reads_a_corpus_and_writes_json_anchor_seeds`

The round trip: under a fresh `TestDir`, `write_corpus` writes the one
certified record into a corpus subdirectory at shard capacity 1 — "fabricated
corpus writes"; the binary runs with `--corpus` and `--out` naming a JSON
path inside the temporary directory; the exit status asserts successful with
the process's stderr in the failure message; and the output file — "anchor
JSON was written" — deserializes as a vector of `AnchorSeed` through
`serde_json` — "anchor output is valid JSON" — equal to exactly one seed:
`game_index` 0, `from_ply` 5, `line` [(5,9)]. The temporary directory cleans
itself up on drop.

**Source:** `gomoku/crates/selfplay/tests/binary.rs`, lines 111–142 after
the edit (to the end of the slice file). *Created in this chapter.*

```rust
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

*Provenance: chapter-5 slice form, byte-identical to the completed
reference's `tests/binary.rs` lines 111–142 at the same line numbers. The
completed reference continues at line 144 with chapter 07's four
`selfplayverify_*` tests; the slice ends here.*

## 14. `src/lib.rs`: the chapter-05 public surface (slice form)

The crate root grows by one module declaration and one re-export group, both
in first position: `anchor` sorts before `corpus` alphabetically. The
fragments are **chapter-5 slice forms** — the completed reference's
`src/lib.rs` additionally declares the chapter-06 modules and re-exports
their vocabulary, and the provenance notes name every differing line.

**Source:** [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs),
lines 7–15 after the edit (the lint and the module list). *This chapter
inserts line 9.*

```rust
   7 | #![deny(missing_docs)]

   9 | mod anchor;
  10 | mod corpus;
  11 | mod game;
  12 | mod generate;
  13 | mod opening;
  14 | mod record;
  15 | mod schedule;
```

*Provenance: chapter-5 slice form. Slice lines 7–10 are byte-identical to
the completed reference's `src/lib.rs` lines 7–10; slice line 11 (`mod
game;`) is the reference's line 12, slice line 12 the reference's line 13,
slice line 13 the reference's line 15, slice line 14 the reference's line
16, and slice line 15 the reference's line 19 — the reference interleaves
`mod export;` (line 11), `mod mix;` (line 14), and `mod sample;` / `mod
sampler;` (lines 17–18), all chapter 06, all absent from the slice.*

**Source:** `gomoku/crates/selfplay/src/lib.rs`, lines 17–34 after the edit
(the re-export list). *This chapter inserts line 17.*

```rust
  17 | pub use anchor::{AnchorError, AnchorSeed, extract_anchor_seeds};
  18 | pub use corpus::{
  19 |     CorpusWriteOptions, Manifest, OverwritePolicy, read_corpus, read_manifest, write_corpus,
  20 |     write_corpus_with_options,
  21 | };
  22 | pub use game::{
  23 |     Game, GameError, Outcome, PlyRecord, PlySearch, WinReason, play_game, play_game_with_opening,
  24 | };
  25 | pub use generate::{GenerateError, generate_games, generate_games_with};
  26 | pub use opening::{
  27 |     GeneratedOpening, Opening, OpeningAnswer, OpeningLoadError, OpeningParseError,
  28 |     OpeningProvenance, OpeningSource, TournamentOpenings,
  29 | };
  30 | pub use record::{
  31 |     CertifiedSuffix, GAME_RECORD_FORMAT_VERSION, GameHeader, GameRecord, GuardSettings,
  32 |     RECORDED_PRIOR_TOP_K, RecordedPly, SeatConfig, SeatPairing, TssProbeSettings,
  33 | };
  34 | pub use schedule::{EXPLORATION_PLIES, ExplorationSchedule};
```

*Provenance: chapter-5 slice form. Slice line 17 is byte-identical to the
reference's line 21; slice lines 18–21 to the reference's lines 22–25; slice
lines 22–24 to the reference's lines 27–29; slice line 25 to the reference's
line 30; slice lines 26–29 to the reference's lines 32–35; slice lines 30–33
to the reference's lines 36–39; and slice line 34 to the reference's line
42. The reference interleaves the chapter-06 re-export groups for `export`
(line 26), `mix` (line 31), and `sample` / `sampler` (lines 40–41), all
absent from the slice.*

The export audit, stated once: the crate root re-exports exactly the
chapter-05 surface alongside the chapter-04 surface — from `anchor`, the
three names `AnchorError`, `AnchorSeed`, `extract_anchor_seeds`; from
`game`, nothing new (`SuffixTracker` and `PendingSuffix` stay private, not
even crate-visible: the tracker is runner mechanics, not contract); from
`player`, nothing (`own_proof` is a field on the already public `Decision`,
and fields need no re-export). Deliberate absences: the binary's `USAGE`,
`Args`, `parse_args`, `run`, and `main` live in the binary and are never
exported; no sample, sampler, mix, or export module exists; no
`selfplayexport` or `selfplayverify` binary exists; no chapter-06-or-later
name appears anywhere in this chapter's code.

## 15. Verification

The chapter's commands, run from `gomoku/` — the same command set the
approved plan's step 5.3 names, listed here in the order the slice's
verification run executed and logged them, with the release build folded
into the capstone:

```text
cargo test -p selfplay --test anchors
cargo test -p selfplay --lib
cargo test -p selfplay --test binary selfplayanchors
cargo test -p selfplay
cargo test -p player
cargo test -p selfplay --release --test game -- --ignored
cargo build --release -p selfplay && <the capstone below>
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first three commands are the focused selections — the anchor tests, the
library's new unit pair, and the binary's two tests selected by name, which
leaves the later chapters' binary tests unrun because they do not exist yet.
The fourth is the fast gate; cargo prints one report per test target, never
one aggregate line, and the chapter's closing state is the sum of the
per-target reports. The fifth is the cross-crate regression: `Decision`
gained a field, so the `player` suite joins this chapter's gates. The sixth
is the chapter's only MANUAL invocation — chapter 01's MANUAL pair plays
full games through the rewired move loop, and the `--release` qualifier is
not optional. The last two are the project's standing quality gates.
`cargo test -p train` is not required: the record format and the storage
mechanics are untouched.

Evidence actually measured for this paper (2026-11-13, on the chapter-5
slice constructed in the scratch copy described under Provenance — not a
re-run of the completed reference, whose gate evidence the
[hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md)
records; logs at `/tmp/selfplay-ch5-verify-logs/`, scratch-local and
ephemeral):

| Command | Result on the chapter-5 slice |
|---------|-------------------------------|
| `cargo test -p selfplay --test anchors` | 4 passed, 0 failed, 0 ignored (0.00 s) — all four extraction tests green |
| `cargo test -p selfplay --lib` | 2 passed, 0 failed, 0 ignored (0.05 s) — both tracker unit tests green; the library target's first tests |
| `cargo test -p selfplay --test binary selfplayanchors` | 2 passed, 0 failed, 1 filtered out (0.17 s) — chapter 04's help test is the filtered one |
| `cargo test -p selfplay` | lib target: 2 passed, 0 ignored (0.05 s); `selfplayanchors` binary unit target: 0 tests; `selfplaygen` binary unit target: 4 passed, 0 ignored (6.21 s); anchors target: 4 passed (0.00 s); binary target: 3 passed (0.27 s); corpus target: 10 passed, 0 ignored (0.01 s); dedup target: 1 passed; game target: 5 passed, 2 ignored (12.48 s, debug); grading target: 0 passed, 1 ignored; openings target: 5 passed; schedule target: 1 passed, 1 ignored; doc-tests: 0 — **35 passed, 4 ignored** in total |
| `cargo test -p player` | src unit tests: 17 passed; `tests/decisions.rs`: 13 passed (0.25 s — the strengthened `own_proof_plays_the_shortest_certified_line` among them); `tests/guard.rs`: 13 passed; doc-tests: 0 — **43 passed, 0 ignored** in total, the count unchanged from chapter 04 |
| `cargo test -p selfplay --release --test game -- --ignored` | 2 passed, 0 failed, 5 filtered out (0.82 s) — chapter 01's MANUAL pair green through the rewired move loop |
| capstone (below) | `Wrote 3 verified anchor seeds`; the seed file inspected; the rerun against the existing `--out` failed with `File exists (os error 17)` and exit code 1 |
| `cargo clippy --all-targets -- -D warnings` | clean, workspace-wide (11.63 s) |
| `cargo fmt --all -- --check` | no changes |

Three count notes keep the evidence precise. First, the chapter-05 closing
state is the sum of the per-target reports: chapter 04's targets keep their
state (game 5 passed / 2 MANUAL-ignored, corpus 10/0, openings 5/0, schedule
1/1, dedup 1/0, the `selfplaygen` binary's unit tests 4/0, grading 0/1), and
this chapter adds the library target's unit pair (2/0), the anchors target
(4/0), and the binary target's two new tests (3/0 including chapter 04's
one) — 35 passed, 4 ignored, the four ignored tests exactly chapter 04's
set. Second, the corpus target again shows three `test result` lines in the
full-suite log: its cross-process determinism test re-executes the test
binary twice as a child process, and the children's one-test reports (1
passed, 9 filtered out each) appear interleaved in the parent log — chapter
02's mechanism, unchanged. Third, the completed reference's full suite is
larger — 50 passed, 4 ignored, the hand-off's recorded number — entirely
because chapters 06 and 07 add their own targets (`tests/read_path.rs` and
the four `selfplayverify_*` binary tests); that number describes the
completed reference, not this chapter's expectation.

### The capstone: three fresh games, three verified seeds

Extract anchors from a tiny generated corpus and watch the boundary work end
to end. The output directory is temporary — `mktemp -d` keeps the run out of
the repository's `data/` tree, now and for every future generation exercise;
main `data/` is never a generation target. From `gomoku/`, in release (the
generation plays real games):

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

target/release/selfplayanchors \
  --corpus "$dir/corpus" --out "$dir/anchors.json"
```

On the slice (release build, 2026-11-13, temporary directory
`/tmp/selfplayanchors-capstone.3ItQII`, retained and scratch-local), the
generator reported `Games: 3, shards: 1, accepted seeds: [555, 556, 557]`;
the extraction exited 0 and reported `Wrote 3 verified anchor seeds`; and
the seed file was:

```json
[
  {
    "game_index": 0,
    "from_ply": 47,
    "line": [
      71
    ]
  },
  {
    "game_index": 1,
    "from_ply": 29,
    "line": [
      109
    ]
  },
  {
    "game_index": 2,
    "from_ply": 65,
    "line": [
      187
    ]
  }
]
```

The rerun against the same `--out` failed as designed: stderr carried
`selfplayanchors failed: failed to create
'/tmp/selfplayanchors-capstone.3ItQII/anchors.json': File exists (os error
17)` and the exit code was 1 — the output file must not exist, so an
extraction never overwrites a seed set by accident.

Three properties land in one exercise, and three honesty notes keep them in
proportion. The format shows end to end: `game_index` counts the corpus
order, `from_ply` counts post-opening plies, and `line` holds board indices
— a `Move` serializes as its index (`row * 15 + col`), so `71` is row 4,
column 11; the file is pretty-printed and ends without a trailing newline.
The count agrees with the reference's reported output of exactly this
command pair — the chapter paper quotes "Wrote 3 verified anchor seeds", one
per game, each certified line one ply long, and the fresh run's game-0 seed
carries the same line `[71]` the chapter's example explains. And the count
is the measured output of this seeded pipeline on this slice, not a
guarantee about generation: whether every game certifies is a property of
the games, not of the extractor, and a different `--seed-base` may certify
fewer or later — a game without a certificate writes no line and no apology.

## 16. Soundness evidence: fresh slice versus reference acceptance

The chapter's soundness story has two evidence tiers, and this paper keeps
them separate.

**Fresh slice evidence (this paper's runs, 2026-11-13).** The eight new
tests pass on the slice: the four extraction tests pin the positive round
trip, the absent-certificate skip, the verified-but-not-played rejection,
and the doctored-line abort; the two tracker unit tests pin the
follow-to-completion and departure rules on a fixture whose prover behavior
is pinned upstream by the engine's own `vcf_win_in_three` test; and the two
binary tests pin the help contract and the corpus-to-JSON round trip. The
strengthened `player` assertion pins the retention at its source. The
capstone adds a behavioral datum: three freshly generated games produced
three certificates, and every one passed extraction's second verification.
The chapter-01 MANUAL pair in release re-proves the rewired move loop end to
end. And the provenance comparison grounds the no-behavior-change claim in
the diff itself: the whole-file difference between the chapter-4 slice's
`player` crate and this chapter's is exactly the retention change quoted in
sections 1–3 — one field, one doc sentence, the paired map, three
construction lines, one assertion — so no selection, evaluator, or
random-stream consumption changes anywhere. These are measurements of one
low-budget slice, stated as such; the 3-of-3 capstone count is not a
guarantee about other seeds or budgets.

**Reference acceptance evidence (quoted, attributed, not re-run).** The
reference's measured acceptance run — `acceptance-evidence.md` in the
reference worktree, chapter 07's measurement record — reports on its 48-game
baseline-32-versus-baseline-32 corpus (seeds 100000–100047): **47 of 48
games carried a certified suffix** (the sole exception was the 222-ply draw;
absence of a certificate emitted no negative label), **47 of 47 certificates
re-verified through `selfplayanchors` / `verify_line` (100 %)**, the 47
certificates came from 47 distinct games, every certified line in that
corpus was one ply long (the baseline probe finds the final forcing move
late and short), and the extraction pass took 0.14 s, reporting `Wrote 47
verified anchor seeds`. The chapter paper identifies the exception record by
record in the shipped corpus: game index 4 (seed 100004) is the only draw,
the only 222-ply game, and the only record whose `certified_suffix` is
`None`. These numbers describe one low-budget corpus; they are the design
spec's soundness gate measured once, and chapter 07 owns the corpus-level
gates that re-measure new corpora. Neither tier licenses a general claim:
the fresh 3-of-3 and the reference 47-of-48 are reports of specific corpora,
not promises about future ones.

One behavior stands outside both tiers by design: accepting a certified line
as a *proper* prefix of the played suffix — a game that continues after its
certified line completes — is prose-specified, source-verified behavior of
the reference implementation (section 10.2 points at the `take` that limits
the comparison), and no chapter test pins it. The chapter papers state this
gap explicitly rather than papering over it with a ninth test, and this
paper holds the same line: no ninth test exists in the slice, and none is
quoted.

## References

**This chapter:**

- [certified-endgames.md](certified-endgames.md) — the chapter paper:
  intention, specification as tests, execution-order walkthrough, the
  soundness boundary.
- [certified-endgames-design.md](certified-endgames-design.md) — the
  conceptual story: the three seams, the tracker state machine, the eight
  design invariants.
- [certified-endgames-implementation.md](certified-endgames-implementation.md) —
  the hiding layer this paper quotes the construction contracts of; its
  steps 1–5 fix the chapter-5 slice forms and the test fixtures.

**This track:**

- [Tutorial README](../README.md) — conventions, gates, the chapter map, and
  the reference-provenance rule this paper's Provenance section applies.
- [Self-play primer](../00-selfplay-primer.md) — §4.5 states this chapter's
  contract; the glossary defines the certified suffix and the anchor seed.
- [Chapter 01 — the game runner](../01-the-game-runner/the-game-runner.md)
  and its [solution](../01-the-game-runner/the-game-runner-solution.md) —
  the move loop this chapter wires into, the single-RNG discipline the
  retention respects, the provenance discipline this paper continues, and
  the MANUAL pair that re-proves the rewired loop.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md)
  and its [solution](../02-recording-games/recording-games-solution.md) —
  the `CertifiedSuffix` type, the `Game` and `GameRecord` slots, the
  preservation test this chapter activates, and the corpus mechanics
  (`write_corpus`, `read_corpus`) the tests and the binary reuse.
- [Chapter 03 — recording the search](../03-recording-the-search/recording-the-search.md)
  and its [solution](../03-recording-the-search/recording-the-search-solution.md) —
  the honesty rule for certified-proof moves (`search: None`) and the
  search-statistics conversion that this chapter's `begin` call precedes.
- [Chapter 04 — diversity and grading](../04-diversity-and-grading/diversity-and-grading.md)
  and its [solution](../04-diversity-and-grading/diversity-and-grading-solution.md) —
  the generator whose baseline-probe seats produce the certificates, the
  MANUAL discipline this chapter leaves unchanged, and the chapter-4 scratch
  (`/tmp/selfplay-ch4-verify`) this chapter's slice was copied from.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  §3.4 the certified-endgame contract and the no-negative-labels rule; §5
  the acceptance gates chapter 07 runs.
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  the reference-implementation layout and the measured headline numbers this
  paper quotes with attribution.
- [docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) —
  §12 the soundness gate.
- [docs/13-engine-design.md](../../../13-engine-design.md) — the
  soundness-over-completeness decision behind the bounded prover.

**Prerequisite tracks:**

- [Baseline tutorial](../../baseline-tutorial/README.md) —
  `AiPlayer::decide`, `PlayerOptions`, `OwnTssConfig::baseline()` (10 000
  nodes / 30 plies), and the certified own-proof bypass that now retains its
  proof.
- [Engine tutorial](../../13-engine-tutorial/README.md) — the TSS prover and
  `verify_line`, the budget-honesty rule, and the `vcf_win_in_three` fixture
  script the tracker tests reuse.

**Source (live on `main`; touched by this chapter):**

- [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs) —
  `Decision` gains `own_proof`; the own-proof bypass stores `Some(proof)`,
  the resignation and search paths store `None`; the `decide` doc gains the
  retention sentence (sections 1–2).
- [`gomoku/crates/player/tests/decisions.rs`](../../../../gomoku/crates/player/tests/decisions.rs) —
  `own_proof_plays_the_shortest_certified_line` gains the retention
  assertion (section 3).

**Source (created by chapters 01–05; the links resolve once the chapters
land on `main`):**

- [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs) —
  gains the `Proof` import, the slot-doc wording, the `SuffixTracker` and
  `PendingSuffix` types, the three wiring regions, and the two unit tests
  (sections 4–7). Touched in this chapter.
- [`gomoku/crates/selfplay/src/anchor.rs`](../../../../gomoku/crates/selfplay/src/anchor.rs) —
  `AnchorSeed`, the five-variant `AnchorError`, and `extract_anchor_seeds`
  (sections 8–10). Created in this chapter.
- [`gomoku/crates/selfplay/src/bin/selfplayanchors.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayanchors.rs) —
  the extraction binary (section 11). Created in this chapter.
- [`gomoku/crates/selfplay/tests/anchors.rs`](../../../../gomoku/crates/selfplay/tests/anchors.rs) —
  the extraction specification, four tests (section 12). Created in this
  chapter.
- [`gomoku/crates/selfplay/tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs) —
  gains the three helpers and this chapter's two binary tests (section 13).
  Touched in this chapter.
- [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs) —
  declares the `anchor` module and re-exports the new vocabulary (section
  14). Touched in this chapter.

**Source (unchanged in this chapter):**

- [`gomoku/crates/selfplay/src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs)
  and `src/corpus.rs` mechanics — the `CertifiedSuffix` slot (chapter 02's,
  filled through the runner alone), `GameRecord::from_game`, `write_corpus`,
  `read_corpus`; consumed through call sites only.
- [`gomoku/crates/engine/src/tss.rs`](../../../../gomoku/crates/engine/src/tss.rs) —
  `Proof`, `prove_shortest_forced_win`, `verify_line`, and the
  `vcf_win_in_three` fixture script; consumed, not touched.
- [`gomoku/crates/selfplay/Cargo.toml`](../../../../gomoku/crates/selfplay/Cargo.toml)
  and [`gomoku/crates/player/Cargo.toml`](../../../../gomoku/crates/player/Cargo.toml) —
  no manifest changes; `selfplay` already depends on `engine`, `player`,
  `serde`, `serde_json`, and `thiserror`, and `player` already lists
  `engine`.

**Reference provenance:** the completed reference implementation lives in
the read-only sibling worktree `rust-ml-selfplay-ref` (branch
`selfplay-ref-v2`, base `9028aa8`, changes uncommitted); it is quoted and
compared against, never linked, and it is deleted once the tutorial is
complete on `main`. The chapter-5 slice in this paper was constructed per
the implementation plan in an untouched copy of the verified chapter-4
scratch (`/tmp/selfplay-ch4-verify` → `/tmp/selfplay-ch5-verify`) and
gate-verified there on 2026-11-13 (logs at
`/tmp/selfplay-ch5-verify-logs/`, including the scripted provenance
comparisons in `10-provenance-diff.log`); the reference worktree, the
chapter-4 scratch, and the earlier scratch copies (`/tmp/selfplay-ch1-verify`,
`/tmp/selfplay-ch2-verify`, `/tmp/selfplay-ch3-verify`) were not modified.
The capstone corpus and seed file live at
`/tmp/selfplayanchors-capstone.3ItQII/` (scratch-local, ephemeral). The
quoted acceptance measurements are the reference worktree's
`acceptance-evidence.md`, chapter 07's measurement record, cited here — not
re-run — exactly as the chapter paper cites them.

**External sources:**

- Allis, L. V. (1994). *Searching for Solutions in Games and Artificial
  Intelligence*. PhD thesis, Maastricht University.
  <https://doi.org/10.26481/dis.19940923la> — threat-space search, the
  prover and verifier behind every certificate this chapter handles.
- Rosin, C. D. (2011). *Multi-armed bandits with episode context* (PUCT).
  <https://doi.org/10.1007/s10472-011-9258-6> — the selection rule of the
  search the probe short-circuits and the retention leaves untouched.
