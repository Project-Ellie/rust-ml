> **Opt-in solution — read it only after attempting the chapter with the
> [implementation plan](diversity-and-grading-implementation.md), or after being
> stuck for more than twenty minutes.**
>
> This paper is inspiration, not authority over the learner's own code. The
> learner's implementation on `main` is the reference; the artifacts below
> exist so a stuck or finished learner can compare. No learner source is
> changed by this paper.

# Chapter 04 — Diversity and grading: solution

## Abstract

This paper is the annotated reference companion to
[chapter 04 of the selfplay-corpora tutorial](diversity-and-grading.md). It
quotes everything the chapter creates or changes, along the execution path of
one `selfplaygen` invocation: the opening-list error vocabulary
(`OpeningParseError`, `OpeningLoadError`), the `TournamentOpenings` pool with
its parser, loader, and `OpeningSource` implementation, the
`ExplorationSchedule::options_for_ply` behavior method and its per-ply
application inside the runner's move loop, the three doc-comment updates that
retire earlier slice wordings, the seeded generation loop with its Zobrist-walk
deduplication in `src/generate.rs`, the `selfplaygen` binary (usage text,
argument parser, run path, read-back check), the crate-root exports, all
fourteen chapter-04 test bodies (twelve default, two MANUAL), and the
chapter's verification evidence — the focused opening/schedule/dedup runs, the
binary unit tests, the help-text contract, the full `selfplay` suite at 27
passed and 4 ignored, the two new MANUAL tests plus chapter 01's MANUAL pair
in release mode, the two-process byte-identical capstone, and the standing
clippy and rustfmt gates. Every snippet carries an explanation, source line
numbers, and an exact provenance note: the nine files this chapter copies or
creates in full — `src/opening.rs`, `src/schedule.rs`, `src/record.rs`,
`src/generate.rs`, `src/bin/selfplaygen.rs`, `tests/openings.rs`,
`tests/schedule.rs`, `tests/dedup.rs`, and `tests/grading.rs` — are
byte-identical to the completed reference implementation, and the three that
are not — `src/game.rs`, `src/lib.rs`, and `tests/binary.rs` — are chapter-4
slice forms whose every difference from the completed reference is named
explicitly. (`tests/game.rs` and `tests/corpus.rs` are byte-identical to the
reference as well, but this chapter does not touch them, so they are not part
of its touched-file count.) All chapter gates ran green on the
chapter-4 slice on 2026-11-13; the precise evidence, including one fresh
observation about the grading probe's per-seed outcomes, is in the Provenance,
Verification, and Open observations sections. The intended reader is the
learner who has read the chapter paper and, ideally, worked the
[implementation plan](diversity-and-grading-implementation.md) first.

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
| **Swap2** | The project's opening protocol: one side places three stones, the other side chooses a color or places two more stones, and the first side then chooses a color. Executed through the engine's typestate. |
| **Opening offer** | The three-stone opening one side places in the Swap2 ceremony; in this chapter, one validated line of a tournament list. |
| **OpeningSource** | The runner's pluggable opening seam (chapter 01): one method, `opening(&mut self, rng: &mut StdRng) -> Opening`, sharing the game's random number generator (RNG) stream. |
| **Opening provenance** | `OpeningProvenance`: `Generated`, or `Listed { list, index }` — the stable identity of the source list and the zero-based offer position within it. Recorded in every game header. |
| **Exploration schedule** | `ExplorationSchedule`: the per-ply move-selection policy shared by both seats. Plies `0..plies` after the Swap2 ceremony sample moves at temperature τ with root Dirichlet noise; every later ply plays the argmax. |
| **Temperature (τ)** | The move-selection temperature: τ = 1 samples moves proportionally to visit counts; τ = 0 plays the argmax (Silver et al. 2017). |
| **Dirichlet noise (ε, α)** | Root noise mixed into the search priors during exploration plies: ε is the mix fraction, α the concentration parameter (Silver et al. 2017). |
| **Base seed / derived seed / accepted seed** | The seed a corpus slot starts from; the replacement `base_seed + attempt` used when a duplicate is rejected; and the seed — base or derived — that produced the accepted game and is recorded. |
| **Zobrist key** | A 64-bit hash of a board position, maintained incrementally by the engine; used here for game identity. |
| **Zobrist walk** | The ordered sequence of Zobrist keys of every position a game reaches, opening placements included. Equality of walks is this chapter's duplicate test. |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games. It never runs by default; it runs on explicit selection, in release mode. |
| **Threat-space search (TSS)** | A proof search over forced-win threat sequences (Allis 1994). Each seat's own-side probe runs before its search; a certified proof short-circuits the search. |
| **PUCT** | Predictor UCT (Rosin 2011), the selection rule of the Monte Carlo tree search both seats run. |
| **Claim labels** | The project's provenance labels (chapter 12 of the wiki): **[paper]** (stated in a cited paper), **[derived]** (computed from cited or measured values), **[experiment]** (a free parameter, to be settled by measurement). This paper also uses **[invention]**: a choice of this track, not a fact of the architecture chapters. |
| **Chapter-4 slice** | The chapter-4 subset of the completed reference: chapters 01–04 together, without the later chapters' additions. Defined and verified in the Provenance section. |
| **TDD** | Test-driven development: write the test, watch it fail (red), implement, watch it pass (green). |

## Provenance, the chapter-4 slice, and verification

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
established the provenance discipline this paper continues; chapters 02 and
03 applied it to the record format and the search semantics this chapter
leaves untouched.

One structural fact shapes everything below: the worktree holds the
**completed** crate — all seven chapters at once. Its
`gomoku/crates/selfplay/src/game.rs` already carries chapter 05's suffix
tracker (`SuffixTracker`, the `engine::Proof` import, `decision.own_proof`,
the `suffix.finish()` assembly, and two tracker unit tests); its `src/lib.rs`
declares and re-exports the later chapters' modules (`anchor`, `export`,
`mix`, `sample`, `sampler`); and its `tests/binary.rs` holds seven tests,
six of which belong to chapters 05 and 07. Quoting those regions verbatim
would leak later chapters and would contradict this chapter's approved
papers, which fix the chapter-4 form: the runner applies the schedule but
keeps `certified_suffix: None` and tracks nothing, the crate root exports
exactly the chapter-04 surface, and `tests/binary.rs` holds exactly one test
(see
[the implementation plan](diversity-and-grading-implementation.md), steps 1
through 10). This paper therefore distinguishes two kinds of quotation:

- **Reference-verbatim fragments.** Everything this chapter creates in full
  is stable in the completed reference: `src/opening.rs`, `src/schedule.rs`,
  `src/generate.rs`, `src/bin/selfplaygen.rs`, `src/record.rs`,
  `tests/openings.rs`, `tests/schedule.rs`, `tests/dedup.rs`, and
  `tests/grading.rs` are each byte-identical between the chapter-4 slice
  and the completed reference (whole-file scripted comparisons). The two
  earlier test targets this chapter does not touch — `tests/game.rs` and
  `tests/corpus.rs` — are likewise byte-identical between the slice and the
  completed reference, but they belong to earlier chapters and are not part
  of this chapter's created-in-full set. Two quoted
  fragments of `src/game.rs` — the `Game` struct's option fields and the
  `play_game` doc comment — are likewise byte-identical regions. These
  fragments are quoted exactly, and each carries a note naming the reference
  file and line range it was compared against. The comparisons were scripted,
  not eyeballed; every range reported as byte-identical was checked
  programmatically (trailing newline structure included).
- **Chapter-4 slice forms.** Three files hold the slice-form regions: the
  completed reference has already grown past chapter 04 in the `src/game.rs`
  import block, the runner's setup and move-loop regions, the `src/lib.rs`
  module and export lists, and the whole of `tests/binary.rs`. This paper
  quotes the chapter-4 form specified by the approved papers, and names
  exactly what the completed reference adds later. No later-chapter
  implementation is quoted.

### The slice was built and run

Quoting a slice the learner could not compile would violate the track's
first rule — a tutorial that does not run error-free is a defect. So the
chapter-4 slice was constructed and gate-verified on 2026-11-13 at
`/tmp/selfplay-ch4-verify` (a scratch path, not a repository artifact). The
starting point was not the reference worktree but an *untouched copy* of the
already-verified chapter-3 slice at `/tmp/selfplay-ch3-verify` (copied in
full, excluding its `target/` build directory) — so the chapter-4 slice
differs from chapter 03's verified state by exactly this chapter's edits,
and neither the chapter-3 scratch nor the reference worktree was modified:

1. `src/opening.rs`, `src/schedule.rs`, `src/generate.rs` (new),
   `src/bin/selfplaygen.rs` (new), `tests/openings.rs` (new),
   `tests/schedule.rs` (new), `tests/dedup.rs` (new), and
   `tests/grading.rs` (new) were copied from the completed reference. Each
   is byte-identical to the reference file (scripted whole-file
   comparison): chapters 05–07 never touch these files, so the completed
   reference carries exactly the chapter-4 form of each.
2. `src/game.rs` gained `ExplorationSchedule` in its crate import block, the
   two `Game` base-option field-doc updates, the `play_game` doc sentence,
   the per-game schedule binding, and the two-line move-loop application —
   and nothing else: no `Proof` import, no suffix tracker, no `own_proof`,
   `certified_suffix: None` kept, no test module.
3. `src/record.rs` gained the `SeatConfig.temperature` doc update — after
   which the whole file is byte-identical to the completed reference.
4. `src/lib.rs` gained the `generate` module declaration and the two new
   re-export groups (`generate`'s three names, `opening`'s three new names).
5. `tests/binary.rs` was written fresh as the chapter-4 slice form: one
   import and the one help test. The test body is byte-identical to the
   completed reference's lines 74–94; the reference file's other six tests
   and fixtures belong to chapters 05 and 07 and are absent.

Nothing else was touched: no player-crate change (in particular no
`own_proof` — the slice's `player` crate is chapter 03's), no record or
corpus mechanics, no manifest change, no anchor, export, mix, sample, or
sampler module anywhere. The slice's `engine`, `mcts`, `baseline`, and
`train` sources are byte-identical to the reference worktree's (scripted
whole-crate comparison); the `player` crate differs from the reference
worktree's only by chapter 05's `own_proof` retention, which cannot affect
move selection.

The chapter's verification commands were run from the copy's `gomoku/`, with
stdout and stderr saved to `/tmp/selfplay-ch4-verify-logs/` (a scratch path,
not a repository artifact):

```text
cargo test -p selfplay --test openings --test schedule --test dedup
cargo test -p selfplay --bin selfplaygen
cargo test -p selfplay --test binary selfplaygen_help
cargo test -p selfplay
cargo test -p selfplay --release --test schedule -- --ignored
cargo test -p selfplay --release --test game -- --ignored
cargo test -p selfplay --release --test grading -- --ignored --nocapture
# the two-process capstone (its full text is in the Verification section)
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

All commands passed on the slice; the measured evidence — per-target counts,
timings, the grading probe's per-seed outcomes, and the capstone's digests —
is tabled in [§12](#12-verification), and the honest observations in
[§13](#13-open-observations--reported-not-resolved). The slice fragments
quoted below are exactly the files those runs exercised.

### Reading rules

The section order is the execution order of one `selfplaygen` invocation,
followed by the exports, the full test surface, and the verification: the
opening lists are parsed and loaded before a game starts (sections 1–3); the
schedule shapes every ply through the runner's one changed region (sections
4–6); the seeded generation loop plays and deduplicates games (section 7);
the binary drives all of it, from argument parsing to the read-back check
(sections 8–9); the crate root publishes the vocabulary (section 10); the
fourteen tests pin every behavior (section 11); and the grading probe's
measured outcomes stand with the verification (sections 12–13). Within the
source sections the order is the file's own: errors before the pool, the
parser before the loader, the loader before the sampling implementation.

Every code fragment carries a right-aligned `NN |` line-number margin. For
fragments from files this chapter edits, the numbers are the edited
chapter-4 file's own line numbers — the same numbers the learner's files
will have after the chapter's edits land. The margin is the only annotation
added; the code text itself is quoted exactly, and each fragment's
provenance note states whether it is byte-identical to the completed
reference (with the reference's line range) or a chapter-4 slice form (with
the exact delta). Blank separators between lines appear without a margin; no
fragment skips a nonblank line inside its quoted range.

File links in this paper point into the learner's tree on `main`. Links to
files the track creates — for example
[`gomoku/crates/selfplay/src/generate.rs`](../../../../gomoku/crates/selfplay/src/generate.rs) —
resolve once chapters 01–04 land. The reference worktree is not linked: it
is temporary provenance, deleted once the tutorial is complete on `main`,
and everything needed from it is quoted here. The scratch copies and their
logs (`/tmp/selfplay-ch3-verify`, `/tmp/selfplay-ch4-verify`,
`/tmp/selfplay-ch4-verify-logs/`) are cited as evidence of this paper's own
verification; they are ephemeral and not part of the repository.

## 1. `src/opening.rs`: the error vocabulary

The chapter's first artifacts are the two error types, because everything
downstream names them: the parser fails with `OpeningParseError`, the loader
fails with `OpeningLoadError`, and the committed-list tests assert substrings
of their display text. Three imports join the file's header — the standard
library's `fs` and path types for the loader, and the `thiserror` derive the
chapter-01 vocabulary did not need (chapter 01's draws are infallible, so the
file had no error type). The engine and `rand` imports are chapter 01's,
unchanged.

**Source:** [`gomoku/crates/selfplay/src/opening.rs`](../../../../gomoku/crates/selfplay/src/opening.rs),
lines 1–9 after the edit. *Created by chapter 01; this chapter inserts lines
1–2 and 6.*

```rust
   1 | use std::fs;
   2 | use std::path::{Path, PathBuf};

   4 | use engine::{Board, Color, Move, Placing3, PlayError, Swap2};
   5 | use rand::{RngExt, rngs::StdRng};
   6 | use thiserror::Error;

   8 | /// Where an opening came from.
   9 | #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
```

*Provenance: byte-identical to the completed reference's `src/opening.rs`
lines 1–9 (whole-file identity holds for the entire file).*

`OpeningParseError` is a struct — one malformed line — carrying the three
things a repair needs: the list identity (the path string, stable into every
record header), the one-based physical line number, and the specific reason.
Its display format joins all three in that order, because the tests assert
all three as substrings of one message. `OpeningLoadError` is an enum in the
order a load meets its failures: `Empty` rejects an empty path set before any
file is touched; `Read` carries the path and the underlying I/O error as the
error source; `Parse` is a transparent wrap of `OpeningParseError` with the
from-conversion idiom, so a parse failure propagates unchanged and its
list-line-reason message reaches the caller verbatim — the piskvork
regression test asserts `line 41` and `outside` through this variant. The
enum derives `Debug` and the error derive only: the `io::Error` inside `Read`
forbids `Clone` and `PartialEq`.

**Source:** `gomoku/crates/selfplay/src/opening.rs`, lines 102–132 after the
edit. *Chapter 01's vocabulary ends at line 100; this chapter inserts the
block at lines 102–276, ahead of chapter 01's `GeneratedOpening`.*

```rust
 102 | /// A malformed tournament opening line, including its physical line number.
 103 | #[derive(Debug, Clone, PartialEq, Eq, Error)]
 104 | #[error("opening list '{list}' line {line}: {reason}")]
 105 | pub struct OpeningParseError {
 106 |     /// Stable identity supplied for the list.
 107 |     pub list: String,
 108 |     /// One-based physical line number in the source text.
 109 |     pub line: usize,
 110 |     /// Specific validation failure.
 111 |     pub reason: String,
 112 | }

 114 | /// Failure to load tournament opening lists from disk.
 115 | #[derive(Debug, Error)]
 116 | pub enum OpeningLoadError {
 117 |     /// No input paths were supplied.
 118 |     #[error("at least one tournament opening list is required")]
 119 |     Empty,
 120 |     /// An input file could not be read.
 121 |     #[error("could not read opening list '{}': {source}", path.display())]
 122 |     Read {
 123 |         /// Path that could not be read.
 124 |         path: PathBuf,
 125 |         /// Underlying filesystem error.
 126 |         #[source]
 127 |         source: std::io::Error,
 128 |     },
 129 |     /// An input file contained a malformed opening.
 130 |     #[error(transparent)]
 131 |     Parse(#[from] OpeningParseError),
 132 | }
```

*Provenance: byte-identical to the completed reference's `src/opening.rs`
lines 102–132.*

## 2. `src/opening.rs`: the offer pool and its parser

### 2.1 `ListedOffer` and `TournamentOpenings`

`ListedOffer` is private: one validated offer — its list identity, its
zero-based index within that list, and its first three stones in placement
order. Private, so no doc obligation beyond the derive line; the crate root
never names it. `TournamentOpenings` is the public pool: a vector of offers
behind a doc comment that states the format contract — every line holds at
least three distinct, on-board stones; longer lines are validated in full but
only their first three stones drive the Swap2 typestate, because the ceremony
consumes exactly three; the rest of a long line is the source tournament's
pre-set continuation, not this generator's to replay.

**Source:** `gomoku/crates/selfplay/src/opening.rs`, lines 134–150.

```rust
 134 | #[derive(Debug, Clone)]
 135 | struct ListedOffer {
 136 |     list: String,
 137 |     index: usize,
 138 |     initial: [Move; 3],
 139 | }

 141 | /// Tournament offers loaded from center-relative `x,y` opening lists.
 142 | ///
 143 | /// Every line must contain at least three distinct, on-board stones. Longer
 144 | /// lines are validated in full but only their first three stones drive the
 145 | /// Swap2 typestate; later stones belong to the source's full preset position
 146 | /// and are deliberately not replayed as post-opening moves.
 147 | #[derive(Debug, Clone)]
 148 | pub struct TournamentOpenings {
 149 |     offers: Vec<ListedOffer>,
 150 | }
```

*Provenance: byte-identical to the completed reference's `src/opening.rs`
lines 134–150.*

### 2.2 `TournamentOpenings::parse`

The parser works line by line, tracking the one-based physical line number,
with a `malformed` closure over the list identity and the current line number
so no reason string repeats the error format by hand. Per line, in execution
order: split on commas and trim every field; reject fewer than six fields or
an odd field count with one reason that covers both arities (the two
rejection tests assert different substrings of it); parse each coordinate as
a signed 16-bit integer, rejecting with the coordinate's axis and text on
failure; map center-relative to the board (row = 7 + y, column = 7 + x, so
`0,0` is row 7 column 7 and `-8,-6` would be column −1) and reject anything
outside 0..=14 or refused by `Move::new` as outside the 15×15 board; reject a
coordinate already present among this line's stones as a duplicate. Every
pair of the line is validated before the offer is recorded — the
full-validation rule — and the recorded offer keeps only the first three
stones, the slice-to-fixed-array conversion expected with a "length checked
above" note. The offer's index is the number of offers recorded so far, so
each list keeps its own zero-based indices when pools concatenate. After the
loop, an empty offer vector rejects at line 1: an empty file is malformed,
not an empty pool. The first malformed line fails the whole parse — fail
fast, no skipping — because an opening list is a curated artifact and a
corrupt line means the artifact is not what the invocation claimed.

**Source:** `gomoku/crates/selfplay/src/opening.rs`, lines 152–214 (the
inherent implementation block through the end of `parse`).

```rust
 152 | impl TournamentOpenings {
 153 |     /// Parse one ordered opening list.
 154 |     ///
 155 |     /// Coordinates are relative to the center: `0,0` maps to row 7, column 7,
 156 |     /// `x` is the column offset, and `y` is the row offset.
 157 |     ///
 158 |     /// # Errors
 159 |     /// Returns the first malformed line with its one-based line number.
 160 |     pub fn parse(list: impl Into<String>, text: &str) -> Result<Self, OpeningParseError> {
 161 |         let list = list.into();
 162 |         let mut offers = Vec::new();
 163 |         for (line_index, line) in text.lines().enumerate() {
 164 |             let line_number = line_index + 1;
 165 |             let malformed = |reason: String| OpeningParseError {
 166 |                 list: list.clone(),
 167 |                 line: line_number,
 168 |                 reason,
 169 |             };
 170 |             let fields: Vec<_> = line.split(',').map(str::trim).collect();
 171 |             if fields.len() < 6 || fields.len() % 2 != 0 {
 172 |                 return Err(malformed(
 173 |                     "expected at least three comma-separated x,y pairs".to_owned(),
 174 |                 ));
 175 |             }

 177 |             let mut stones = Vec::with_capacity(fields.len() / 2);
 178 |             for pair in fields.as_chunks::<2>().0 {
 179 |                 let x = pair[0].parse::<i16>().map_err(|error| {
 180 |                     malformed(format!("invalid x coordinate '{}': {error}", pair[0]))
 181 |                 })?;
 182 |                 let y = pair[1].parse::<i16>().map_err(|error| {
 183 |                     malformed(format!("invalid y coordinate '{}': {error}", pair[1]))
 184 |                 })?;
 185 |                 let row = 7 + y;
 186 |                 let col = 7 + x;
 187 |                 let mv = (0..15)
 188 |                     .contains(&row)
 189 |                     .then_some(())
 190 |                     .filter(|_| (0..15).contains(&col))
 191 |                     .and_then(|_| Move::new(row as u8, col as u8))
 192 |                     .ok_or_else(|| {
 193 |                         malformed(format!("coordinate {x},{y} is outside the 15 by 15 board"))
 194 |                     })?;
 195 |                 if stones.contains(&mv) {
 196 |                     return Err(malformed(format!("duplicate coordinate {x},{y}")));
 197 |                 }
 198 |                 stones.push(mv);
 199 |             }
 200 |             offers.push(ListedOffer {
 201 |                 list: list.clone(),
 202 |                 index: offers.len(),
 203 |                 initial: stones[..3].try_into().expect("length checked above"),
 204 |             });
 205 |         }
 206 |         if offers.is_empty() {
 207 |             return Err(OpeningParseError {
 208 |                 list,
 209 |                 line: 1,
 210 |                 reason: "opening list is empty".to_owned(),
 211 |             });
 212 |         }
 213 |         Ok(Self { offers })
 214 |     }
```

*Provenance: byte-identical to the completed reference's `src/opening.rs`
lines 152–214.*

### 2.3 `load`, `len`, and `is_empty`

`load` concatenates any number of lists into one pool. An empty slice fails
with `Empty` before any file is touched. Each path is read to a string — a
read failure becomes `Read` with the path and the I/O error — and the path's
display string is the list identity passed to `parse`, the stable identity
that later appears in every record's opening provenance. The parsed offers
extend one concatenated pool in path order; the first failure of any kind
fails the whole load. `len` and `is_empty` are the standard lint pair, both
must-use; `is_empty` is never true for a successfully parsed pool, because
`parse` rejects empty files.

**Source:** `gomoku/crates/selfplay/src/opening.rs`, lines 216–250 (through
the end of the inherent implementation block).

```rust
 216 |     /// Load and concatenate ordered opening lists into one uniform offer pool.
 217 |     ///
 218 |     /// Each list keeps its own zero-based offer indices for provenance.
 219 |     ///
 220 |     /// # Errors
 221 |     /// Returns an error for an empty path set, unreadable file, or malformed line.
 222 |     pub fn load<P: AsRef<Path>>(paths: &[P]) -> Result<Self, OpeningLoadError> {
 223 |         if paths.is_empty() {
 224 |             return Err(OpeningLoadError::Empty);
 225 |         }
 226 |         let mut offers = Vec::new();
 227 |         for path in paths {
 228 |             let path = path.as_ref();
 229 |             let text = fs::read_to_string(path).map_err(|source| OpeningLoadError::Read {
 230 |                 path: path.to_path_buf(),
 231 |                 source,
 232 |             })?;
 233 |             let list = path.to_string_lossy().into_owned();
 234 |             offers.extend(Self::parse(list, &text)?.offers);
 235 |         }
 236 |         Ok(Self { offers })
 237 |     }

 239 |     /// Number of offers available for uniform sampling.
 240 |     #[must_use]
 241 |     pub fn len(&self) -> usize {
 242 |         self.offers.len()
 243 |     }

 245 |     /// Whether no offers are loaded. Valid parsers never construct this state.
 246 |     #[must_use]
 247 |     pub fn is_empty(&self) -> bool {
 248 |         self.offers.is_empty()
 249 |     }
 250 | }
```

*Provenance: byte-identical to the completed reference's `src/opening.rs`
lines 216–250.*

## 3. `src/opening.rs`: the second `OpeningSource`

The chapter's second implementation of chapter 01's seam draws twice on the
game's single `StdRng` stream. First, one offer uniformly from the whole
concatenated pool — the ranged-random idiom over the pool's index range,
safe because the pool is non-empty by construction. Then the Swap2 answer,
exactly as `GeneratedOpening` draws it: a ranged draw over three outcomes —
take Black, take White, or add two — with the add-two arm reusing chapter
01's private `draw_unique` helper (two unique cells from the central 7×7
region, rows and columns 4..11, excluding the offer's stones) and chapter
01's private `random_color` helper for the final choice. The returned
`Opening` is assembled through `Opening::new` with
`OpeningProvenance::Listed { list, index }`.

The uniform draw and the three-way seeded answer are this chapter's
**[invention]** under chapter 14's authority —
[§3.2](../../../14-openings-and-supervised-curriculum.md) ranks the
tournament lists as the tier-1 offer source, and §3.3 prescribes perturbing
the answering side's argmax in self-play so games explore both colors of the
same offer; this track has no value model, so the perturbed argmax becomes a
seeded uniform protocol choice. The reference carries the honesty comment
where a future value-guided answer will replace it, and the fragment below
quotes it verbatim.

**Source:** `gomoku/crates/selfplay/src/opening.rs`, lines 252–276.

```rust
 252 | impl OpeningSource for TournamentOpenings {
 253 |     fn opening(&mut self, rng: &mut StdRng) -> Opening {
 254 |         let offer = &self.offers[rng.random_range(0..self.offers.len())];
 255 |         // Chapter 14 §3.3 perturbs a value-based argmax. This source has no
 256 |         // value model, so chapter 4 uses a seeded uniform protocol choice as
 257 |         // its exploration approximation, including both color assignments.
 258 |         let answer = match rng.random_range(0..3) {
 259 |             0 => OpeningAnswer::Take(Color::Black),
 260 |             1 => OpeningAnswer::Take(Color::White),
 261 |             _ => OpeningAnswer::AddTwo {
 262 |                 stones: draw_unique(rng, &offer.initial),
 263 |                 final_choice: random_color(rng),
 264 |             },
 265 |         };
 266 |         Opening::new(
 267 |             offer.initial,
 268 |             answer,
 269 |             OpeningProvenance::Listed {
 270 |                 list: offer.list.clone(),
 271 |                 index: offer.index,
 272 |             },
 273 |         )
 274 |     }
 275 | }

```

*Provenance: byte-identical to the completed reference's `src/opening.rs`
lines 252–276. Chapter 01's `GeneratedOpening` and the two private draw
helpers follow at lines 277–322, unchanged from chapter 01.*

## 4. `src/schedule.rs`: `options_for_ply`

Chapter 02 already created everything else in this file — `EXPLORATION_PLIES`,
the `ExplorationSchedule` struct with its four public fields and serde
derives, and the `Default` — so this chapter adds exactly one import line and
one method, and the implementation plan is explicit that recreating any of
the existing vocabulary is drift. The import line brings in the `player`
crate's `PlayerOptions`, the method's input and output type.

**Source:** [`gomoku/crates/selfplay/src/schedule.rs`](../../../../gomoku/crates/selfplay/src/schedule.rs),
lines 1–2 after the edit. *Created by chapter 02; this chapter inserts line
1.*

```rust
   1 | use player::PlayerOptions;
   2 | use serde::{Deserialize, Serialize};
```

*Provenance: byte-identical to the completed reference's `src/schedule.rs`
lines 1–2 (whole-file identity holds for the entire file).*

The method sits in its own inherent implementation block between the struct
and `Default`. It borrows the schedule, consumes the seat's base options **by
value**, and takes the zero-based post-opening ply index; it returns the
effective options, marked must-use, and it is pure and cannot fail, so no
`Result`. The contract is narrow on purpose: inside the window (ply strictly
below the schedule's `plies`), temperature, noise ε, and noise α are replaced
by the schedule's recorded values; outside the window, temperature becomes
0.0 (argmax selection) and ε becomes 0.0, which makes α inert — so the code
leaves α untouched there, a stated behavior the property test deliberately
does not pin. Every other field passes through exactly: the simulation
budget, the PUCT constant, the TSS probe, the guard, and the resignation
settings — named in the doc comment, because a schedule that quietly edited
the budget would corrupt the pairing, the corpus's declared identity.

**Source:** `gomoku/crates/selfplay/src/schedule.rs`, lines 28–46 after the
edit. *The constant (line 8), the struct (lines 16–26), and `Default` (lines
48–57) are chapter 02's, untouched.*

```rust
  28 | impl ExplorationSchedule {
  29 |     /// Apply this schedule to one post-opening ply.
  30 |     ///
  31 |     /// Only move-selection settings change. In particular, the seat's
  32 |     /// simulation budget, PUCT constant, TSS probe, guard, and resignation
  33 |     /// settings are preserved exactly.
  34 |     #[must_use]
  35 |     pub fn options_for_ply(&self, mut base: PlayerOptions, ply: usize) -> PlayerOptions {
  36 |         if ply < self.plies {
  37 |             base.temperature = self.temperature;
  38 |             base.noise_epsilon = self.noise_epsilon;
  39 |             base.noise_alpha = self.noise_alpha;
  40 |         } else {
  41 |             base.temperature = 0.0;
  42 |             base.noise_epsilon = 0.0;
  43 |         }
  44 |         base
  45 |     }
  46 | }
```

*Provenance: byte-identical to the completed reference's `src/schedule.rs`
lines 28–46.*

## 5. `src/game.rs`: the schedule enters the move loop (slice form)

The runner of chapters 01–03 changes in exactly one line region plus one
import, one binding — and nothing else. Anything beyond that is drift. The
two fragments of this section are **chapter-4 slice forms**: the completed
reference's `src/game.rs` additionally carries chapter 05's suffix tracking,
and the notes below name every difference exactly.

### 5.1 The import

`ExplorationSchedule` joins the crate import block in alphabetical position.
The block grows from three lines to four; the `engine` import above it is
chapter 03's, unchanged.

**Source:** [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs),
lines 1–10 after the edit.

```rust
   1 | use baseline::{BaselineEvaluator, Device};
   2 | use engine::{Color, Move, PlayError, Status};
   3 | use player::{AiPlayer, DecisionAction, DecisionError, PlayerOptions, ResignReason, SearchStats};
   4 | use rand::{SeedableRng, rngs::StdRng};
   5 | use thiserror::Error;

   7 | use crate::{
   8 |     CertifiedSuffix, ExplorationSchedule, GeneratedOpening, Opening, OpeningSource,
   9 |     record::RECORDED_PRIOR_TOP_K,
  10 | };
```

*Provenance: chapter-4 slice form. Every line except line 2 is byte-identical
to the completed reference's `src/game.rs` at the same line numbers (slice
lines 1 and 3–10 against reference lines 1 and 3–10, checked
programmatically). The reference's line 2 additionally names `engine::Proof`
— chapter 05's suffix tracking — which the slice does not have.*

### 5.2 The application

In `play_game_with_opening`'s setup, after the plies vector is created and
before the move loop, the default schedule is bound once per game. In the
move loop, between the seat selection and the decision call, the effective
options are the schedule applied to the moving seat's base options at the
current ply count — `plies.len()`, zero-based and post-opening, because the
Swap2 placements are the ceremony, not plies — and the decision call receives
a borrow of the effective options instead of the base options. The base
options are `Copy`, so the application consumes a copy; the returned `Game`
still carries the base options verbatim, so the record always shows what was
asked for. Everything else in the function — the single seeded stream, the
status-first loop structure, the seat selection by absolute color, chapter
03's `Option::map` conversion, the legality gate, the resignation arm, the
assembly with `certified_suffix: None` — stays byte-identical to chapter 03.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 156–163 after the
edit (the setup through the schedule binding) and lines 176–192 (the seat
selection through the end of the `Play` arm). *Created by chapter 01, extended by
chapters 02 and 03; this chapter inserts line 162 and replaces the decision
call's argument at lines 181–182.*

```rust
 156 |     let mut rng = StdRng::seed_from_u64(seed);
 157 |     let opening = opening_source.opening(&mut rng);
 158 |     let mut board = opening.board()?;
 159 |     let mut black = AiPlayer::new(Box::new(BaselineEvaluator::new(Device::flex())));
 160 |     let mut white = AiPlayer::new(Box::new(BaselineEvaluator::new(Device::flex())));
 161 |     let mut plies = Vec::new();
 162 |     let schedule = ExplorationSchedule::default();

```

```rust
 176 |         let side = board.to_move();
 177 |         let (seat, options) = match side {
 178 |             Color::Black => (&mut black, &black_options),
 179 |             Color::White => (&mut white, &white_options),
 180 |         };
 181 |         let effective_options = schedule.options_for_ply(*options, plies.len());
 182 |         let decision = seat.decide(&board, &effective_options, &mut rng)?;
 183 |         match decision.action {
 184 |             DecisionAction::Play(mv) => {
 185 |                 let search = decision.search.map(PlySearch::from);
 186 |                 board.play(mv).map_err(|source| GameError::Play {
 187 |                     ply: plies.len(),
 188 |                     mv,
 189 |                     source,
 190 |                 })?;
 191 |                 plies.push(PlyRecord { mv, search });
 192 |             }
```

*Provenance: chapter-4 slice forms. In the first fragment, slice lines 156–161
and 162–163 are byte-identical to the completed reference's lines 200–205 and
207–208 respectively; the reference inserts one line between them — line 206,
`let mut suffix = SuffixTracker::default();` (chapter 05) — which the slice
does not have. In the second fragment, slice lines 176–184, 185–190, and
191–192 are byte-identical to the reference's lines 221–229, 231–236, and
238–239; the
reference inserts `suffix.begin(plies.len(), decision.own_proof);` (reference
line 230) between the `Play` pattern and the map line, and `suffix.observe(mv);`
(reference line 237) between the legality gate and the ply push — both chapter
05, neither present in the slice. The chapter-4 lines themselves — slice line
162 and slice lines 181–182 — are byte-identical to the reference's lines 207
and 226–227.*

## 6. The three doc-comment updates

Three slice wordings that earlier chapters documented as deltas now become
the reference wordings, because the behavior they name exists as of section
5. No code changes; no test outcome changes. If a test moves, the refactor
drifted — fix the edit, never a test.

### 6.1 `Game.black_options` / `Game.white_options`: base, adjusted per ply

Chapter 01's field docs read "Base options for Black, preserved verbatim in
the record." (and White respectively) — true while nothing adjusted them.
The replacement keeps the honesty and adds the adjustment: the options are
still preserved verbatim in the returned game, but they are now *base*
options, adjusted per ply at decision time.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 76–83 after the edit
(the `Game` struct's head through the `white_options` field).

```rust
  76 | pub struct Game {
  77 |     /// Seed of the single RNG stream used for opening and move selection.
  78 |     pub seed: u64,
  79 |     /// Base options for Black; exploration adjusts selection settings per ply.
  80 |     pub black_options: PlayerOptions,
  81 |     /// Base options for White; exploration adjusts selection settings per ply.
  82 |     pub white_options: PlayerOptions,
  83 |     /// Full Swap2 description and its provenance.
```

*Provenance: byte-identical to the completed reference's `src/game.rs` lines
76–83.*

### 6.2 The `play_game` doc sentence

Chapter 01's solution paper recorded this sentence as the reference's and
marked it for this chapter. It is appended to the `play_game` doc comment
directly after the sentence about default options disabling resignation.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 122–133 after the
edit.

```rust
 122 | /// Play one game using a freshly generated seeded Swap2 opening.
 123 | ///
 124 | /// Each color owns a persistent [`AiPlayer`] with its own
 125 | /// [`BaselineEvaluator`] on [`Device::flex`]. There is no I/O, threading, or
 126 | /// GPU work. The supplied base options are preserved in the returned game;
 127 | /// `PlayerOptions::default()` disables resignation for ordinary corpus runs.
 128 | /// The chapter-4 exploration schedule adjusts only selection settings per ply.
 129 | ///
 130 | /// # Errors
 131 | /// Returns an error if the generated opening, a player decision, or a selected
 132 | /// move is rejected by the engine.
 133 | pub fn play_game(
```

*Provenance: byte-identical to the completed reference's `src/game.rs` lines
166–177.*

### 6.3 `SeatConfig.temperature`: the override stated precisely

Chapter 02's slice documented the field as the base visit-count sampling
temperature and named chapter 04 as the adjuster ("chapter 4's exploration
schedule adjusts it per ply") — true then, imprecise once the schedule's
behavior exists. The replacement states the rule precisely: the pairing's
exploration schedule overrides the base temperature on every ply, and the
override is zero after the opening window. Nothing else in
[`src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs) changes —
the conversion already records the base options and the default schedule, and
that is exactly right. After this two-line edit the whole file is
byte-identical to the completed reference's `src/record.rs` (scripted
whole-file comparison): chapter 03's solution paper had recorded this region
as the reference's one remaining difference from the chapter-3 slice, and
this edit retires it.

**Source:** `gomoku/crates/selfplay/src/record.rs`, lines 73–82 after the
edit. *Created by chapter 02; this chapter updates lines 77–78.*

```rust
  73 |     /// Number of MCTS simulations per decision.
  74 |     pub simulations: u32,
  75 |     /// PUCT exploration constant.
  76 |     pub c_puct: f32,
  77 |     /// Base visit-count sampling temperature; the pairing's exploration
  78 |     /// schedule overrides this on every ply (zero after the opening window).
  79 |     pub temperature: f32,
  80 |     /// Dirichlet noise mix fraction.
  81 |     pub noise_epsilon: f32,
  82 |     /// Dirichlet concentration parameter.
```

*Provenance: byte-identical to the completed reference's `src/record.rs`
lines 73–82.*

## 7. `src/generate.rs`: seeded generation with deduplication

The first of the chapter's two new files. The module's imports: the standard
library's `HashSet` for the per-run walk set; the engine's `Board` and `Move`
for the replay; the `player` crate's `PlayerOptions` for the wrapper's
signature; the `thiserror` derive; and, from the crate, `Game`, `GameError`,
`OpeningAnswer`, `OpeningSource`, and `play_game_with_opening`.

### 7.1 `GenerateError` and the retry bound

`MAX_ATTEMPTS_PER_GAME` is private — source behavior, not contract — and set
to 128: the maximum candidate games tried for one base seed before generation
reports failure. The bound exists so a pathological runner whose seeds do not
diversify its games fails loudly instead of spinning. `GenerateError` has two
variants in the order a run meets them: `Game`, a transparent wrap of chapter
01's `GameError` with the from-conversion idiom, so runner failures propagate
unchanged; and `DuplicateExhausted`, carrying the slot's base seed and the
attempt count, displayed as "could not generate a unique game for base seed
{base_seed} after {attempts} attempts".

**Source:** [`gomoku/crates/selfplay/src/generate.rs`](../../../../gomoku/crates/selfplay/src/generate.rs),
lines 1–26. *Created in this chapter.*

```rust
   1 | use std::collections::HashSet;

   3 | use engine::{Board, Move};
   4 | use player::PlayerOptions;
   5 | use thiserror::Error;

   7 | use crate::{Game, GameError, OpeningAnswer, OpeningSource, play_game_with_opening};

   9 | /// Maximum candidate games tried for one base seed before reporting failure.
  10 | const MAX_ATTEMPTS_PER_GAME: u64 = 128;

  12 | /// Failure while generating a deduplicated sequence of games.
  13 | #[derive(Debug, Error)]
  14 | pub enum GenerateError {
  15 |     /// The underlying game runner rejected a candidate.
  16 |     #[error(transparent)]
  17 |     Game(#[from] GameError),
  18 |     /// Every derived seed produced a move-list walk already seen in this run.
  19 |     #[error("could not generate a unique game for base seed {base_seed} after {attempts} attempts")]
  20 |     DuplicateExhausted {
  21 |         /// Original seed assigned to this corpus slot.
  22 |         base_seed: u64,
  23 |         /// Number of candidates attempted, including the base seed.
  24 |         attempts: u64,
  25 |     },
  26 | }
```

*Provenance: byte-identical to the completed reference's `src/generate.rs`
lines 1–26 (whole-file identity holds for the entire file).*

### 7.2 `generate_games`: the thin wrapper

The wrapper closes over the real runner call and forwards to the injectable
seam — the split mirrors chapter 01's `play_game` / `play_game_with_opening`
and the binary's own `run` / `run_with_opening_source` (section 9): every
stage of the pipeline has a seam where a test can substitute fabricated
behavior. Both option sets are `Copy`, so the closure captures them by copy;
the opening source is captured by its mutable borrow. The doc comment states
the derivation rule (attempt zero uses the base seed; attempt `n` uses
`base_seed.wrapping_add(n)`), the accepted-seed recording, and the error
contract.

**Source:** `gomoku/crates/selfplay/src/generate.rs`, lines 28–47.

```rust
  28 | /// Generate games for the supplied base seeds, deduplicated within this run.
  29 | ///
  30 | /// Attempt zero uses `base_seed`; a collision is rejected and attempt `n` uses
  31 | /// `base_seed.wrapping_add(n)`. The accepted game's actual seed is retained in
  32 | /// its record and manifest. Base seeds can be contiguous even when a derived
  33 | /// replacement seed enters the exact recorded seed set.
  34 | ///
  35 | /// # Errors
  36 | /// Returns a runner error, or an exhaustion error after 128 collisions for one
  37 | /// base seed. Deduplication covers full opening and post-opening move order.
  38 | pub fn generate_games(
  39 |     base_seeds: impl IntoIterator<Item = u64>,
  40 |     black_options: PlayerOptions,
  41 |     white_options: PlayerOptions,
  42 |     opening_source: &mut impl OpeningSource,
  43 | ) -> Result<Vec<Game>, GenerateError> {
  44 |     generate_games_with(base_seeds, |seed| {
  45 |         play_game_with_opening(seed, black_options, white_options, opening_source)
  46 |     })
  47 | }
```

*Provenance: byte-identical to the completed reference's `src/generate.rs`
lines 28–47.*

### 7.3 `generate_games_with`: the seam and the loop

The injectable seam is public because the dedup test and any in-process
caller drive it. Inside: the per-run walk set is created empty and dropped at
return — deduplication is scoped to the run, and no cross-corpus
deduplication exists. For each base seed, up to 128 candidates: attempt zero
plays at the base seed itself, attempt `n` at the wrapping-add derived seed.
The candidate is played through the injected runner (a runner error
propagates through the from-conversion), its Zobrist walk is computed, and
the walk is inserted into the set: a `true` insert accepts the game and moves
to the next slot; a `false` insert is the collision signal — the candidate is
discarded and the next attempt's seed derived. A slot that exhausts all 128
attempts fails with `DuplicateExhausted`, never a loop without end. The
accepted game's actual seed — base or derived — stays in the `Game`, flows
into the record header, and lands in the manifest's `seeds` list: the
recorded seed set is always the exact set that produced the corpus, and a
derived replacement seed in that set is expected behavior, visible in the
manifest, not an anomaly.

**Source:** `gomoku/crates/selfplay/src/generate.rs`, lines 49–86.

```rust
  49 | /// Generate and deduplicate games through an injectable seeded runner.
  50 | ///
  51 | /// This is the deterministic seam used by fabricated-game tests and by callers
  52 | /// with another in-process runner. Seed derivation and collision handling are
  53 | /// identical to [`generate_games`].
  54 | ///
  55 | /// # Errors
  56 | /// Propagates runner errors and reports repeated collisions instead of looping
  57 | /// forever when the runner's seeds do not diversify its move-list walks.
  58 | pub fn generate_games_with(
  59 |     base_seeds: impl IntoIterator<Item = u64>,
  60 |     mut play: impl FnMut(u64) -> Result<Game, GameError>,
  61 | ) -> Result<Vec<Game>, GenerateError> {
  62 |     let mut seen: HashSet<Vec<u64>> = HashSet::new();
  63 |     let mut games = Vec::new();

  65 |     for base_seed in base_seeds {
  66 |         let mut accepted = false;
  67 |         for attempt in 0..MAX_ATTEMPTS_PER_GAME {
  68 |             let seed = base_seed.wrapping_add(attempt);
  69 |             let game = play(seed)?;
  70 |             let walk = zobrist_walk(&game)?;
  71 |             if seen.insert(walk) {
  72 |                 games.push(game);
  73 |                 accepted = true;
  74 |                 break;
  75 |             }
  76 |         }
  77 |         if !accepted {
  78 |             return Err(GenerateError::DuplicateExhausted {
  79 |                 base_seed,
  80 |                 attempts: MAX_ATTEMPTS_PER_GAME,
  81 |             });
  82 |         }
  83 |     }

  85 |     Ok(games)
  86 | }
```

*Provenance: byte-identical to the completed reference's `src/generate.rs`
lines 49–86.*

### 7.4 The walk helpers

Two private free functions. `zobrist_walk` replays the game's full ordered
move list through a fresh `engine::Board` — the opening placements first (a
rejection maps to `GameError::Opening`), then every recorded ply in order (a
rejection maps to `GameError::Play` with the zero-based ply index, the move,
and the source) — and pushes the board's Zobrist key after **every** move,
pre-sizing the vector for the plies plus the at-most-five opening placements.
`opening_moves` returns the ordered opening placements: the three initial
stones, plus the two added stones when the answer is the add-two variant.
Three stones, or five — the ceremony, in placement order. The walk, not the
final position, is the identity: two games that reach the same end position
through different move orders are different games, and two games that share
only an opening prefix diverge in their walks.

**Source:** `gomoku/crates/selfplay/src/generate.rs`, lines 88–116 (to the
end of the file).

```rust
  88 | /// Hash every position reached by the full ordered move list, including Swap2
  89 | /// placements. Different prefixes and different placement orders cannot be
  90 | /// collapsed into a single final-position key.
  91 | fn zobrist_walk(game: &Game) -> Result<Vec<u64>, GameError> {
  92 |     let mut board = Board::new();
  93 |     let mut walk = Vec::with_capacity(game.plies.len() + 5);

  95 |     for mv in opening_moves(game) {
  96 |         board.play(mv).map_err(GameError::Opening)?;
  97 |         walk.push(board.zobrist());
  98 |     }
  99 |     for (ply, record) in game.plies.iter().enumerate() {
 100 |         board.play(record.mv).map_err(|source| GameError::Play {
 101 |             ply,
 102 |             mv: record.mv,
 103 |             source,
 104 |         })?;
 105 |         walk.push(board.zobrist());
 106 |     }
 107 |     Ok(walk)
 108 | }

 110 | fn opening_moves(game: &Game) -> Vec<Move> {
 111 |     let mut moves = game.opening.initial.to_vec();
 112 |     if let OpeningAnswer::AddTwo { stones, .. } = game.opening.answer {
 113 |         moves.extend(stones);
 114 |     }
 115 |     moves
 116 | }
```

*Provenance: byte-identical to the completed reference's `src/generate.rs`
lines 88–116.*

## 8. `src/bin/selfplaygen.rs`: declaration and parsing

The second new file, and the chapter's only binary. No manifest entry: Cargo
discovers the binary by its path. The file opens with a module-level doc
comment — one line naming the binary and one paragraph stating the process
rule — and imports the standard library's `env`, `PathBuf`, and `process`,
the `player` crate's `OwnTssConfig` and `PlayerOptions`, and six crate-root
names from `selfplay`: the binary speaks crate-root names only.

**Source:** [`gomoku/crates/selfplay/src/bin/selfplaygen.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplaygen.rs),
lines 1–13. *Created in this chapter.*

```rust
   1 | //! Strength-graded baseline self-play corpus generator.
   2 | //!
   3 | //! One invocation owns one contiguous base-seed range. Run additional
   4 | //! processes with disjoint `--seed-base` ranges for process-level parallelism.

   6 | use std::env;
   7 | use std::path::PathBuf;
   8 | use std::process;

  10 | use player::{OwnTssConfig, PlayerOptions};
  11 | use selfplay::{
  12 |     GameRecord, OpeningSource, TournamentOpenings, generate_games, read_corpus, write_corpus,
  13 | };
```

*Provenance: byte-identical to the completed reference's
`src/bin/selfplaygen.rs` lines 1–13 (whole-file identity holds for the
entire file).*

### 8.1 `USAGE` and `Args`

`USAGE` is a private string constant built with the line-continuation idiom.
It documents all seven flags and their syntax, states the five defaults (800,
800, 1, 0, 1000), the two parse rules (`--flag value` only, never
`--flag=value`; a repeated numeric flag takes its last value), the
contiguous-range statement, and the two operational rules: both seats use the
baseline own-TSS probe (10 000 nodes / 30 plies), and parallel generation is
separate processes over disjoint seed ranges. The help integration test
asserts all seven flag strings appear in this text. `Args` is a private
struct holding the parsed values; note the `own_tss` field — the probe
configuration is carried in the parsed arguments, not hard-coded in the run
path, so the binary's unit tests can disable it for speed.

**Source:** `gomoku/crates/selfplay/src/bin/selfplaygen.rs`, lines 15–24
(`USAGE`) and lines 26–36 (`Args`).

```rust
  15 | const USAGE: &str = "Usage: selfplaygen --out <DIR> --openings <PATH> [--openings <PATH> ...]\n\
  16 |                    [--black-sims <u32>] [--white-sims <u32>] [--games <usize>]\n\
  17 |                    [--seed-base <u64>] [--shard-capacity <usize>]\n\
  18 | Defaults: --black-sims 800 --white-sims 800 --games 1 --seed-base 0\n\
  19 |           --shard-capacity 1000\n\
  20 | Use `--flag value` only; `--flag=value` is not supported.\n\
  21 | Repeat --openings to combine lists. Numeric flags use the last value wins.\n\
  22 | One process generates the contiguous base-seed range seed-base..seed-base+games.\n\
  23 | Both seats use the baseline own-TSS probe (10,000 nodes / 30 plies).\n\
  24 | Use disjoint ranges in separate processes for parallel generation.";
```

```rust
  26 | #[derive(Debug)]
  27 | struct Args {
  28 |     black_sims: u32,
  29 |     white_sims: u32,
  30 |     games: usize,
  31 |     seed_base: u64,
  32 |     shard_capacity: usize,
  33 |     own_tss: OwnTssConfig,
  34 |     out_dir: PathBuf,
  35 |     openings: Vec<PathBuf>,
  36 | }
```

*Provenance: byte-identical to the completed reference's
`src/bin/selfplaygen.rs` lines 15–24 and 26–36.*

### 8.2 `parse_args` and `parse_value`

`parse_args` is a hand-rolled loop — no argument-parsing dependency: the flag
set is seven strings, the binary is a teaching artifact, and the parse rules
are exactly what the unit tests pin. The loop seeds the five numeric locals
with their defaults, the output directory as absent, and the opening paths
empty; per argument it matches the flag name — a numeric flag consumes the
next argument through `parse_value` and overwrites its local (last value
wins), `--out` consumes the next argument into the directory, `--openings`
consumes the next argument and appends (repeats accumulate in order), and any
other argument fails with "unknown flag". After the loop, validation runs in
a fixed order: zero games, zero shard capacity, an empty opening list, and
finally the absent output directory — `--out is required` is evaluated last,
and the first unit test's missing-`--out` case passes precisely because the
invocation supplies everything else. `parse_value` is the private generic
helper over any `FromStr` type whose error displays: a missing value fails
with "{flag} requires a value", a parse failure with "invalid {flag} value
'{value}': {error}". The assembled `Args` carries
`OwnTssConfig::baseline()` — the binary's declared default.

**Source:** `gomoku/crates/selfplay/src/bin/selfplaygen.rs`, lines 122–185.

```rust
 122 | fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
 123 |     let mut black_sims = 800;
 124 |     let mut white_sims = 800;
 125 |     let mut games = 1;
 126 |     let mut seed_base = 0;
 127 |     let mut shard_capacity = 1000;
 128 |     let mut out_dir = None;
 129 |     let mut openings = Vec::new();

 131 |     while let Some(arg) = args.next() {
 132 |         match arg.as_str() {
 133 |             "--black-sims" => black_sims = parse_value("--black-sims", args.next())?,
 134 |             "--white-sims" => white_sims = parse_value("--white-sims", args.next())?,
 135 |             "--games" => games = parse_value("--games", args.next())?,
 136 |             "--seed-base" => seed_base = parse_value("--seed-base", args.next())?,
 137 |             "--shard-capacity" => {
 138 |                 shard_capacity = parse_value("--shard-capacity", args.next())?;
 139 |             }
 140 |             "--out" => {
 141 |                 let value = args
 142 |                     .next()
 143 |                     .ok_or_else(|| "--out requires a value".to_owned())?;
 144 |                 out_dir = Some(PathBuf::from(value));
 145 |             }
 146 |             "--openings" => {
 147 |                 let value = args
 148 |                     .next()
 149 |                     .ok_or_else(|| "--openings requires a value".to_owned())?;
 150 |                 openings.push(PathBuf::from(value));
 151 |             }
 152 |             other => return Err(format!("unknown flag: {other}")),
 153 |         }
 154 |     }

 156 |     if games == 0 {
 157 |         return Err("--games must be greater than zero".to_owned());
 158 |     }
 159 |     if shard_capacity == 0 {
 160 |         return Err("--shard-capacity must be greater than zero".to_owned());
 161 |     }
 162 |     if openings.is_empty() {
 163 |         return Err("at least one --openings path is required".to_owned());
 164 |     }
 165 |     Ok(Args {
 166 |         black_sims,
 167 |         white_sims,
 168 |         games,
 169 |         seed_base,
 170 |         shard_capacity,
 171 |         own_tss: OwnTssConfig::baseline(),
 172 |         out_dir: out_dir.ok_or_else(|| "--out is required".to_owned())?,
 173 |         openings,
 174 |     })
 175 | }

 177 | fn parse_value<T: std::str::FromStr>(flag: &str, value: Option<String>) -> Result<T, String>
 178 | where
 179 |     T::Err: std::fmt::Display,
 180 | {
 181 |     let value = value.ok_or_else(|| format!("{flag} requires a value"))?;
 182 |     value
 183 |         .parse::<T>()
 184 |         .map_err(|error| format!("invalid {flag} value '{value}': {error}"))
 185 | }
```

*Provenance: byte-identical to the completed reference's
`src/bin/selfplaygen.rs` lines 122–185.*

## 9. `src/bin/selfplaygen.rs`: the run path and the read-back

### 9.1 `main` and `run`

`main` is the exit-code owner: if any raw process argument is `--help` or
`-h`, print the usage to stdout and return with exit code 0 — the check runs
before parsing, so `selfplaygen --help` works even alongside invalid flags;
a parse failure prints the message and the usage to stderr and exits with
code 2; a run failure prints "selfplaygen failed: {error}" and exits with
code 1. `run` loads the pool with `TournamentOpenings::load` — mapping a
failure to "failed to load openings: {error}" — and delegates to the seam.

**Source:** `gomoku/crates/selfplay/src/bin/selfplaygen.rs`, lines 38–60.

```rust
  38 | fn main() {
  39 |     if env::args().any(|arg| arg == "--help" || arg == "-h") {
  40 |         println!("{USAGE}");
  41 |         return;
  42 |     }
  43 |     let args = match parse_args(env::args().skip(1)) {
  44 |         Ok(args) => args,
  45 |         Err(message) => {
  46 |             eprintln!("{message}\n{USAGE}");
  47 |             process::exit(2);
  48 |         }
  49 |     };
  50 |     if let Err(error) = run(args) {
  51 |         eprintln!("selfplaygen failed: {error}");
  52 |         process::exit(1);
  53 |     }
  54 | }

  56 | fn run(args: Args) -> Result<(), String> {
  57 |     let mut openings = TournamentOpenings::load(&args.openings)
  58 |         .map_err(|error| format!("failed to load openings: {error}"))?;
  59 |     run_with_opening_source(&args, &mut openings)
  60 | }
```

*Provenance: byte-identical to the completed reference's
`src/bin/selfplaygen.rs` lines 38–60.*

### 9.2 `run_with_opening_source`: the seam and the read-back

The testable seam, in execution order. First, the seed range is validated
before any game is played: the game count converts to `u64`, and the last
base seed is the checked-add — an overflow fails the run with a named
message. Second, both seats' base options are built from
`PlayerOptions::default()` with the struct-update-syntax idiom, overriding
exactly two fields per seat: the declared simulation budget and the probe
configuration from the `Args`. Every other setting stays at its default —
which means the resignation heuristic and the defensive guard are **disabled**
in every corpus run: corpus games end on the board or on a full board, never
on a concession heuristic, and no guard ever substitutes a move. Third, the
pre-play stderr report names the declared range and the declared pairing.
Fourth, `generate_games` over the contiguous base seeds. Fifth, every
accepted game converts with chapter 02's `GameRecord::from_game` — nothing in
this step is new, which is the earlier chapters' discipline paying off.
Sixth, `write_corpus` streams the records into fixed-capacity shards with its
overwrite protection: a non-empty directory is refused before any file
changes. Seventh, the read-back: `read_corpus` must return records equal to
what was generated, and the manifest's pairing must preserve both declared
per-seat budgets; either mismatch fails the run with exit code 1. The
read-back turns a whole class of serialization defects into a loud failure at
generation time instead of a surprise at training time — and it is a check,
not a transaction: a mid-write failure can leave partial files behind.
Eighth, the closing stderr report: the manifest path, then the game count,
shard count, and the exact accepted seed list — the same list the manifest
persists.

**Source:** `gomoku/crates/selfplay/src/bin/selfplaygen.rs`, lines 62–120.

```rust
  62 | fn run_with_opening_source(
  63 |     args: &Args,
  64 |     opening_source: &mut impl OpeningSource,
  65 | ) -> Result<(), String> {
  66 |     let game_count = u64::try_from(args.games)
  67 |         .map_err(|_| "game count does not fit in the seed range".to_owned())?;
  68 |     let last_seed = args
  69 |         .seed_base
  70 |         .checked_add(game_count - 1)
  71 |         .ok_or_else(|| "seed-base plus game count overflows u64".to_owned())?;
  72 |     let black = PlayerOptions {
  73 |         simulations: args.black_sims,
  74 |         own: args.own_tss,
  75 |         ..PlayerOptions::default()
  76 |     };
  77 |     let white = PlayerOptions {
  78 |         simulations: args.white_sims,
  79 |         own: args.own_tss,
  80 |         ..PlayerOptions::default()
  81 |     };

  83 |     eprintln!(
  84 |         "Generating {} games: base seeds {}..={}, baseline-{} vs baseline-{}",
  85 |         args.games, args.seed_base, last_seed, args.black_sims, args.white_sims
  86 |     );
  87 |     let games = generate_games(
  88 |         (0..game_count).map(|offset| args.seed_base + offset),
  89 |         black,
  90 |         white,
  91 |         opening_source,
  92 |     )
  93 |     .map_err(|error| format!("generation failed: {error}"))?;
  94 |     let records: Vec<_> = games.iter().map(GameRecord::from_game).collect();
  95 |     let manifest = write_corpus(&records, &args.out_dir, args.shard_capacity)
  96 |         .map_err(|error| format!("failed to write corpus: {error}"))?;

  98 |     let read_back = read_corpus(&args.out_dir)
  99 |         .map_err(|error| format!("failed to read back corpus: {error}"))?;
 100 |     if read_back != records {
 101 |         return Err("read-back records differ from generated records".to_owned());
 102 |     }
 103 |     if manifest.pairing.black.simulations != args.black_sims
 104 |         || manifest.pairing.white.simulations != args.white_sims
 105 |     {
 106 |         return Err("manifest does not preserve the declared seat pairing".to_owned());
 107 |     }

 109 |     eprintln!(
 110 |         "Wrote manifest to {}",
 111 |         args.out_dir.join("manifest.json").display()
 112 |     );
 113 |     eprintln!(
 114 |         "Games: {}, shards: {}, accepted seeds: {:?}",
 115 |         manifest.game_count,
 116 |         manifest.shards.len(),
 117 |         manifest.seeds
 118 |     );
 119 |     Ok(())
 120 | }
```

*Provenance: byte-identical to the completed reference's
`src/bin/selfplaygen.rs` lines 62–120.*

## 10. `src/lib.rs`: the chapter-04 public surface (slice form)

The crate root's re-exports grew at the construction checkpoints, ahead of
the integration tests that needed them; this section is the audit. The
`generate` module joins the module list in alphabetical position, between
`game` and `opening`. Two re-export groups extend the surface: `generate`'s
three names, and `opening`'s three new names joining the chapter-01 group in
alphabetical position. From `schedule` nothing new is exported —
`options_for_ply` is a method on the already-exported `ExplorationSchedule`,
and methods need no re-export. Deliberate absences:
`MAX_ATTEMPTS_PER_GAME`, `zobrist_walk`, `opening_moves`, `ListedOffer`, and
chapter 01's draw helpers stay private; the binary's `USAGE`, `Args`,
`parse_args`, `parse_value`, `run`, and `run_with_opening_source` live in the
binary and are never exported; no anchor, export, mix, sample, or sampler
module exists. If any test reached for a path deeper than the crate root, the
test would be fixed to use these re-exports — none does.

**Source:** [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs),
lines 7–14 after the edit (the lint and the module list) and lines 16–32 (the
export list, to the end of the file). *Created by chapter 01, extended by
chapters 02 and 03; this chapter inserts line 11, lines 23–24, and the three
new opening names at lines 25–27.*

```rust
   7 | #![deny(missing_docs)]

   9 | mod corpus;
  10 | mod game;
  11 | mod generate;
  12 | mod opening;
  13 | mod record;
  14 | mod schedule;
```

```rust
  16 | pub use corpus::{
  17 |     CorpusWriteOptions, Manifest, OverwritePolicy, read_corpus, read_manifest, write_corpus,
  18 |     write_corpus_with_options,
  19 | };
  20 | pub use game::{
  21 |     Game, GameError, Outcome, PlyRecord, PlySearch, WinReason, play_game, play_game_with_opening,
  22 | };
  23 | pub use generate::{GenerateError, generate_games, generate_games_with};
  24 | pub use opening::{
  25 |     GeneratedOpening, Opening, OpeningAnswer, OpeningLoadError, OpeningParseError,
  26 |     OpeningProvenance, OpeningSource, TournamentOpenings,
  27 | };
  28 | pub use record::{
  29 |     CertifiedSuffix, GAME_RECORD_FORMAT_VERSION, GameHeader, GameRecord, GuardSettings,
  30 |     RECORDED_PRIOR_TOP_K, RecordedPly, SeatConfig, SeatPairing, TssProbeSettings,
  31 | };
  32 | pub use schedule::{EXPLORATION_PLIES, ExplorationSchedule};
```

*Provenance: chapter-4 slice forms. Every quoted line is byte-identical to a
completed-reference line, at shifted positions — checked programmatically:
slice lines 7–8 against reference lines 7–8; the module lines 9–14 against
reference lines 10, 12, 13, 15, 16, 19 in order; the export lines 16–19
against reference lines 22–25; lines 20–22 against 27–29; line 23 against 30;
lines 24–27 against 32–35; lines 28–31 against 36–39; and line 32 against 42.
The completed reference additionally declares `mod anchor;` (line 9),
`mod export;` (line 11), `mod mix;` (line 14), `mod sample;` (line 17), and
`mod sampler;` (line 18), and re-exports their vocabularies (reference lines
21, 26, 31, 40, 41) — all chapter 05 or later, all absent from the slice.*

## 11. The fourteen tests

The chapter specifies fourteen tests: twelve run by default, two are
MANUAL-ignored. They are quoted here in specification order — the opening
tests first (the lists are parsed before a game starts), then the schedule
pair, the dedup test, the binary's four unit tests, the help contract, and
the grading probe last. Each was written before its implementation (TDD):
the red state of each default test was a compile error against the not-yet-
existing name, and the MANUAL pair's red state is its absence — an honesty
note the implementation plan states for the scheduled game, because two
seeded games are equal with or without the schedule and the header records
the default schedule either way.

### 11.1 `tests/openings.rs`: the five opening tests

The file's imports: the standard-library path types; the engine's `Color`
and `Move`; the seeded-RNG machinery; and four crate-root names from
`selfplay` — the first three are chapter-01 exports, `TournamentOpenings` is
this chapter's. One helper: the `mv(row, col)` fixture constructor from the
earlier test files. The file carries no `TestDir` helper: every fixture is
text in a string or a committed list under `data/openings/`, reached from
`CARGO_MANIFEST_DIR` three directories up.

**Source:** [`gomoku/crates/selfplay/tests/openings.rs`](../../../../gomoku/crates/selfplay/tests/openings.rs),
lines 1–9. *Created in this chapter.*

```rust
   1 | use std::path::{Path, PathBuf};

   3 | use engine::{Color, Move};
   4 | use rand::{SeedableRng, rngs::StdRng};
   5 | use selfplay::{OpeningAnswer, OpeningProvenance, OpeningSource, TournamentOpenings};

   7 | fn mv(row: u8, col: u8) -> Move {
   8 |     Move::new(row, col).expect("test coordinate is on the board")
   9 | }
```

*Provenance: byte-identical to the completed reference's `tests/openings.rs`
lines 1–9 (whole-file identity holds for the entire file).*

The first test pins the parse mapping and the longer-line rule. Two text
fixtures, each parsed with the list identity `test-list`: a three-stone line
and a seven-stone line whose first three pairs are the same and whose
remaining pairs are valid, on-board, and distinct. For each, one opening is
sampled from the parsed pool with an RNG seeded at 7 — the pool holds one
offer, so the draw is forced. The assertions: the initial stones are the
center-relative mapping of the first three pairs; the provenance is `Listed`
with the list identity and index 0; and the opening executes through
`Opening::board` without error. The seven-stone fixture pins the rule that
longer lines are validated in full but only their first three stones drive
the ceremony.

**Source:** `gomoku/crates/selfplay/tests/openings.rs`, lines 11–33.

```rust
  11 | #[test]
  12 | fn three_stone_and_longer_lines_parse_as_swap2_offers() {
  13 |     for text in [
  14 |         "0,0, 1,0, -1,0\n",
  15 |         "0,0, 1,0, -1,0, 0,1, 0,-1, 2,2, -2,-2\n",
  16 |     ] {
  17 |         let mut openings = TournamentOpenings::parse("test-list", text)
  18 |             .expect("three or more stones form a valid opening line");
  19 |         let opening = openings.opening(&mut StdRng::seed_from_u64(7));

  21 |         assert_eq!(opening.initial, [mv(7, 7), mv(7, 8), mv(7, 6)]);
  22 |         assert_eq!(
  23 |             opening.provenance,
  24 |             OpeningProvenance::Listed {
  25 |                 list: "test-list".to_owned(),
  26 |                 index: 0,
  27 |             }
  28 |         );
  29 |         opening
  30 |             .board()
  31 |             .expect("the first three stones execute Swap2");
  32 |     }
  33 | }
```

*Provenance: byte-identical to the completed reference's `tests/openings.rs`
lines 11–33.*

The second test pins the rejection messages. Five malformed texts, each
parsed with the list identity `bad-list`, each expected to fail, with three
substring assertions against the display text — the list identity, the line
marker, the reason — and the message itself in the assertion context so a
failure prints it. The cases: a valid first line followed by a line whose
third pair repeats its own second (line 2, `duplicate` — duplicate detection
is within one line, never across lines); a two-pair line (line 1, `at least
three`); an off-board coordinate `8,0`, which maps to column 15, one past the
board (line 1, `outside`); a non-numeric x coordinate (line 1, `invalid x`);
and a five-field line, an odd field count (line 1, `x,y pairs`).

**Source:** `gomoku/crates/selfplay/tests/openings.rs`, lines 35–53.

```rust
  35 | #[test]
  36 | fn malformed_lines_are_rejected_with_physical_line_numbers() {
  37 |     let cases = [
  38 |         ("0,0, 1,0, -1,0\n0,0, 1,0, 1,0\n", "line 2", "duplicate"),
  39 |         ("0,0, 1,0\n", "line 1", "at least three"),
  40 |         ("0,0, 1,0, 8,0\n", "line 1", "outside"),
  41 |         ("0,0, nope,0, -1,0\n", "line 1", "invalid x"),
  42 |         ("0,0, 1,0, -1\n", "line 1", "x,y pairs"),
  43 |     ];

  45 |     for (text, line, reason) in cases {
  46 |         let error = TournamentOpenings::parse("bad-list", text)
  47 |             .expect_err("malformed opening line must be rejected");
  48 |         let message = error.to_string();
  49 |         assert!(message.contains("bad-list"), "{message}");
  50 |         assert!(message.contains(line), "{message}");
  51 |         assert!(message.contains(reason), "{message}");
  52 |     }
  53 | }
```

*Provenance: byte-identical to the completed reference's `tests/openings.rs`
lines 35–53.*

The third test pins seeded determinism and coverage. One three-offer text,
parsed twice into two identical pools, sampled 64 times from two RNGs seeded
identically at 91. Every pair of draws asserts equality — same pool, same
stream, same opening. Over the 64 draws, three trackers accumulate: all three
offers appear (matched against the expected center-relative mappings), all
three answer kinds appear, and both final colors appear among the add-two
answers; every sampled opening executes through `Opening::board`.

**Source:** `gomoku/crates/selfplay/tests/openings.rs`, lines 55–93.

```rust
  55 | #[test]
  56 | fn sampled_offers_come_from_the_loaded_list_with_seeded_determinism() {
  57 |     let text = "0,0, 1,0, -1,0\n0,0, 0,1, 0,-1\n1,1, 1,0, 1,-1\n";
  58 |     let expected = [
  59 |         [mv(7, 7), mv(7, 8), mv(7, 6)],
  60 |         [mv(7, 7), mv(8, 7), mv(6, 7)],
  61 |         [mv(8, 8), mv(7, 8), mv(6, 8)],
  62 |     ];
  63 |     let mut first = TournamentOpenings::parse("test-list", text).unwrap();
  64 |     let mut second = TournamentOpenings::parse("test-list", text).unwrap();
  65 |     let mut first_rng = StdRng::seed_from_u64(91);
  66 |     let mut second_rng = StdRng::seed_from_u64(91);

  68 |     let mut seen_offers = [false; 3];
  69 |     let mut seen_answers = [false; 3];
  70 |     let mut seen_final_choices = [false; 2];
  71 |     for _ in 0..64 {
  72 |         let a = first.opening(&mut first_rng);
  73 |         let b = second.opening(&mut second_rng);
  74 |         assert_eq!(a, b);
  75 |         let offer = expected
  76 |             .iter()
  77 |             .position(|initial| *initial == a.initial)
  78 |             .expect("sampled offer comes from the loaded list");
  79 |         seen_offers[offer] = true;
  80 |         match a.answer {
  81 |             OpeningAnswer::Take(Color::Black) => seen_answers[0] = true,
  82 |             OpeningAnswer::Take(Color::White) => seen_answers[1] = true,
  83 |             OpeningAnswer::AddTwo { final_choice, .. } => {
  84 |                 seen_answers[2] = true;
  85 |                 seen_final_choices[usize::from(final_choice == Color::White)] = true;
  86 |             }
  87 |         }
  88 |         a.board().expect("every sampled Swap2 answer is legal");
  89 |     }
  90 |     assert!(seen_offers.into_iter().all(|seen| seen));
  91 |     assert!(seen_answers.into_iter().all(|seen| seen));
  92 |     assert!(seen_final_choices.into_iter().all(|seen| seen));
  93 | }
```

*Provenance: byte-identical to the completed reference's `tests/openings.rs`
lines 55–93.*

The fourth test loads the three committed Gomocup lists —
[2024](../../../../data/openings/gomocup-2024-freestyle15.txt),
[2025](../../../../data/openings/gomocup-2025-freestyle15.txt),
[2026](../../../../data/openings/gomocup-2026-freestyle15.txt) —
and asserts the pool length is exactly 36 (each committed list holds 12
lines) and `is_empty` is false. A sampled offer's provenance is destructured
as `Listed` (a generated provenance here is a panic); the list identity must
equal one of the three paths as supplied, and the index is asserted only as
`< 41` — a deliberately loose bound: per-list zero-based indexing is pinned
by the parse fixtures of the first three tests, not by this assertion.

**Source:** `gomoku/crates/selfplay/tests/openings.rs`, lines 95–116.

```rust
  95 | #[test]
  96 | fn valid_committed_tournament_lists_load_into_one_uniform_offer_pool() {
  97 |     let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data/openings");
  98 |     let paths = [
  99 |         "gomocup-2024-freestyle15.txt",
 100 |         "gomocup-2025-freestyle15.txt",
 101 |         "gomocup-2026-freestyle15.txt",
 102 |     ]
 103 |     .map(|name| root.join(name));

 105 |     let mut openings = TournamentOpenings::load(&paths).expect("committed opening lists load");
 106 |     assert_eq!(openings.len(), 36);
 107 |     assert!(!openings.is_empty());

 109 |     let opening = openings.opening(&mut StdRng::seed_from_u64(123));
 110 |     let OpeningProvenance::Listed { list, index } = &opening.provenance else {
 111 |         panic!("tournament opening must retain list provenance")
 112 |     };
 113 |     assert!(paths.iter().any(|path| path.as_path() == Path::new(list)));
 114 |     assert!(*index < 41);
 115 |     opening.board().expect("committed offer executes Swap2");
 116 | }
```

*Provenance: byte-identical to the completed reference's `tests/openings.rs`
lines 95–116.*

The fifth test pins a real data defect as a regression guard: loading the
committed
[piskvork list](../../../../data/openings/piskvork-openings.txt)
alone must fail, and the error's display text must contain `line 41` and
`outside`. The file's line 41 holds the coordinate `-8,-6` — column −1 after
the center-relative mapping, off the 15×15 board. The open observations
(section 13) explain why the list stays rejected and the working pool stays
at 36 offers.

**Source:** `gomoku/crates/selfplay/tests/openings.rs`, lines 118–127 (to
the end of the file).

```rust
 118 | #[test]
 119 | fn malformed_committed_piskvork_line_is_rejected_with_its_line_number() {
 120 |     let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
 121 |         .join("../../../data/openings/piskvork-openings.txt");

 123 |     let error = TournamentOpenings::load(&[path])
 124 |         .expect_err("the off-board source coordinate must not be silently accepted");
 125 |     assert!(error.to_string().contains("line 41"), "{error}");
 126 |     assert!(error.to_string().contains("outside"), "{error}");
 127 | }
```

*Provenance: byte-identical to the completed reference's `tests/openings.rs`
lines 118–127.*

### 11.2 `tests/schedule.rs`: the property test and the MANUAL game

The imports: the `player` crate's `GuardConfig`, `OwnTssConfig`, and
`PlayerOptions` for the loud base fixture; and from the crate root
`EXPLORATION_PLIES`, `ExplorationSchedule`, plus `GameRecord` and
`play_game` for the MANUAL test.

**Source:** [`gomoku/crates/selfplay/tests/schedule.rs`](../../../../gomoku/crates/selfplay/tests/schedule.rs),
lines 1–2. *Created in this chapter.*

```rust
   1 | use player::{GuardConfig, OwnTssConfig, PlayerOptions};
   2 | use selfplay::{EXPLORATION_PLIES, ExplorationSchedule, GameRecord, play_game};
```

*Provenance: byte-identical to the completed reference's `tests/schedule.rs`
lines 1–2 (whole-file identity holds for the entire file).*

The property test builds a deliberately loud base `PlayerOptions`, every
field set explicitly with no struct-update syntax — so a future field added
to `PlayerOptions` fails this build loudly instead of passing through
untested: 777 simulations, c_puct 1.75, temperature 4.0, noise ε 0.8, noise
α 0.9, the baseline own-TSS probe, the baseline guard, resignation enabled.
The assertions, against the default schedule: `EXPLORATION_PLIES` equals 10,
so a silent window change fails loudly; inside the window (plies 0, 1, and
`EXPLORATION_PLIES - 1`), the effective options carry temperature 1.0, ε 0.25,
α 0.3 — and every other field preserved exactly; outside the window (plies
`EXPLORATION_PLIES`, `EXPLORATION_PLIES + 1`, and 100), temperature 0.0 and
ε 0.0 with the same five preservations asserted again. One deliberate gap,
stated in the chapter paper and held here: the code also leaves α untouched
outside the window (ε 0.0 makes α inert), but the test does not assert that —
α outside the window is stated source behavior, not a pinned contract. The
test plays no game and finishes in well under a second.

**Source:** `gomoku/crates/selfplay/tests/schedule.rs`, lines 4–41.

```rust
   4 | #[test]
   5 | fn first_ten_plies_explore_then_selection_is_deterministic_without_changing_budgets() {
   6 |     let base = PlayerOptions {
   7 |         simulations: 777,
   8 |         c_puct: 1.75,
   9 |         temperature: 4.0,
  10 |         noise_epsilon: 0.8,
  11 |         noise_alpha: 0.9,
  12 |         own: OwnTssConfig::baseline(),
  13 |         guard: GuardConfig::baseline(),
  14 |         resign: true,
  15 |     };
  16 |     let schedule = ExplorationSchedule::default();

  18 |     assert_eq!(EXPLORATION_PLIES, 10);
  19 |     for ply in [0, 1, EXPLORATION_PLIES - 1] {
  20 |         let effective = schedule.options_for_ply(base, ply);
  21 |         assert_eq!(effective.temperature, 1.0);
  22 |         assert_eq!(effective.noise_epsilon, 0.25);
  23 |         assert_eq!(effective.noise_alpha, 0.3);
  24 |         assert_eq!(effective.simulations, 777);
  25 |         assert_eq!(effective.c_puct, 1.75);
  26 |         assert_eq!(effective.own, base.own);
  27 |         assert_eq!(effective.guard, base.guard);
  28 |         assert!(effective.resign);
  29 |     }

  31 |     for ply in [EXPLORATION_PLIES, EXPLORATION_PLIES + 1, 100] {
  32 |         let effective = schedule.options_for_ply(base, ply);
  33 |         assert_eq!(effective.temperature, 0.0);
  34 |         assert_eq!(effective.noise_epsilon, 0.0);
  35 |         assert_eq!(effective.simulations, 777);
  36 |         assert_eq!(effective.c_puct, 1.75);
  37 |         assert_eq!(effective.own, base.own);
  38 |         assert_eq!(effective.guard, base.guard);
  39 |         assert!(effective.resign);
  40 |     }
  41 | }
```

*Provenance: byte-identical to the completed reference's `tests/schedule.rs`
lines 4–41.*

The MANUAL test carries its comment explaining why it is ignored: two full
games are unnecessary for the isolated schedule check and can run for minutes
in debug; the binary's budget-1 fixture checks recording. It builds Black at
2 simulations and White at 1 through the struct-update-syntax idiom, plays
two games at seed 29 through `play_game`, converts both with
`GameRecord::from_game`, and asserts: the two records are equal; the header's
recorded exploration equals the default schedule; and the recorded per-seat
budgets are 2 and 1. What it pins is reproducibility and recording *with the
schedule active*, not the application line itself — no test in this chapter
isolates that one line; the application is watched indirectly, by this test
and by chapter 01's MANUAL pair as the move-loop regression (section 12).

**Source:** `gomoku/crates/selfplay/tests/schedule.rs`, lines 43–69 (to the
end of the file).

```rust
  43 | #[test]
  44 | // MANUAL: two full games are unnecessary for the isolated schedule check and
  45 | // can run for minutes in debug. The binary's budget-1 fixture checks recording.
  46 | #[ignore]
  47 | fn scheduled_game_is_reproducible_and_records_its_schedule_and_pairing() {
  48 |     let black = PlayerOptions {
  49 |         simulations: 2,
  50 |         ..PlayerOptions::default()
  51 |     };
  52 |     let white = PlayerOptions {
  53 |         simulations: 1,
  54 |         ..PlayerOptions::default()
  55 |     };

  57 |     let first =
  58 |         GameRecord::from_game(&play_game(29, black, white).expect("scheduled game completes"));
  59 |     let second =
  60 |         GameRecord::from_game(&play_game(29, black, white).expect("replayed game completes"));

  62 |     assert_eq!(first, second);
  63 |     assert_eq!(
  64 |         first.header.config.exploration,
  65 |         ExplorationSchedule::default()
  66 |     );
  67 |     assert_eq!(first.header.config.black.simulations, 2);
  68 |     assert_eq!(first.header.config.white.simulations, 1);
  69 | }
```

*Provenance: byte-identical to the completed reference's `tests/schedule.rs`
lines 43–69.*

### 11.3 `tests/dedup.rs`: the forced collision

The chapter's only dedup test is deliberately fabricated: the contract under
test is the seed arithmetic and the rejection, not the probability that two
real games collide. The `fabricated_game` helper builds a `Game` by hand from
a seed: the opening's three initial stones sit on the row given by the seed
modulo 15, in columns 5, 6, and 7; the answer is `Take(Black)`; the
provenance is `Generated`; both option sets are the defaults; the ply list is
empty; the outcome is a draw; the certified-suffix slot is `None`. No real
game is played anywhere in this target.

**Source:** [`gomoku/crates/selfplay/tests/dedup.rs`](../../../../gomoku/crates/selfplay/tests/dedup.rs),
lines 1–24. *Created in this chapter.*

```rust
   1 | use engine::{Color, Move};
   2 | use player::PlayerOptions;
   3 | use selfplay::{Game, Opening, OpeningAnswer, OpeningProvenance, Outcome, generate_games_with};

   5 | fn mv(row: u8, col: u8) -> Move {
   6 |     Move::new(row, col).expect("test coordinate is on the board")
   7 | }

   9 | fn fabricated_game(seed: u64) -> Game {
  10 |     let row = (seed % 15) as u8;
  11 |     Game {
  12 |         seed,
  13 |         black_options: PlayerOptions::default(),
  14 |         white_options: PlayerOptions::default(),
  15 |         opening: Opening::new(
  16 |             [mv(row, 5), mv(row, 6), mv(row, 7)],
  17 |             OpeningAnswer::Take(Color::Black),
  18 |             OpeningProvenance::Generated,
  19 |         ),
  20 |         plies: Vec::new(),
  21 |         outcome: Outcome::Draw,
  22 |         certified_suffix: None,
  23 |     }
  24 | }
```

*Provenance: byte-identical to the completed reference's `tests/dedup.rs`
lines 1–24 (whole-file identity holds for the entire file).*

The test calls `generate_games_with` with the base seeds 7 and 7 and a
closure that fabricates its game from whatever seed it receives. The first
slot's game (seed 7, row 7) is accepted. The second slot's attempt zero plays
seed 7 again — the same row, the same three-stone walk — collides, and is
rejected; attempt one plays the derived seed 8 (row 8) and is accepted. The
assertions: exactly two games; the first game's seed is 7; the second game's
seed is 8, with the assertion message stating the rule; and the two games'
openings differ.

**Source:** `gomoku/crates/selfplay/tests/dedup.rs`, lines 26–35 (to the end
of the file).

```rust
  26 | #[test]
  27 | fn forced_collision_is_rejected_and_rerolled_with_seed_plus_attempt() {
  28 |     let games = generate_games_with([7, 7], |seed| Ok(fabricated_game(seed)))
  29 |         .expect("the second base seed collides once and then succeeds");

  31 |     assert_eq!(games.len(), 2);
  32 |     assert_eq!(games[0].seed, 7);
  33 |     assert_eq!(games[1].seed, 8, "derived seed = base seed 7 + attempt 1");
  34 |     assert_ne!(games[0].opening, games[1].opening);
  35 | }
```

*Provenance: byte-identical to the completed reference's `tests/dedup.rs`
lines 26–35.*

### 11.4 The binary's four unit tests

A `#[cfg(test)]` module at the end of `src/bin/selfplaygen.rs` holds the four
unit tests and their two fixtures. `TestDir` is the same discipline as the
earlier test targets: a unique directory under the system temporary directory
(the binary's name, the process id, a nanosecond timestamp, and an atomic
sequence counter), removed on `Drop`. `FixedOpening` is a test-local wrapper
around one `Opening`, implementing `OpeningSource` by returning a clone of
the stored opening and ignoring the RNG — the seam's point: the run path is
testable without a tournament list. The module also has its own `mv` helper.

**Source:** `gomoku/crates/selfplay/src/bin/selfplaygen.rs`, lines 187–238
(the test module through its fixtures).

```rust
 187 | #[cfg(test)]
 188 | mod tests {
 189 |     use std::path::{Path, PathBuf};
 190 |     use std::sync::atomic::{AtomicU64, Ordering};
 191 |     use std::time::{SystemTime, UNIX_EPOCH};

 193 |     use engine::{Color, Move};
 194 |     use rand::rngs::StdRng;
 195 |     use selfplay::{Opening, OpeningAnswer, OpeningProvenance, OpeningSource, read_manifest};

 197 |     use super::*;

 199 |     static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

 201 |     struct TestDir(PathBuf);

 203 |     impl TestDir {
 204 |         fn new() -> Self {
 205 |             let nanos = SystemTime::now()
 206 |                 .duration_since(UNIX_EPOCH)
 207 |                 .expect("system clock is after the Unix epoch")
 208 |                 .as_nanos();
 209 |             let sequence = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
 210 |             let path = std::env::temp_dir().join(format!(
 211 |                 "selfplaygen-test-{}-{nanos}-{sequence}",
 212 |                 std::process::id()
 213 |             ));
 214 |             Self(path)
 215 |         }

 217 |         fn path(&self) -> &Path {
 218 |             &self.0
 219 |         }
 220 |     }

 222 |     impl Drop for TestDir {
 223 |         fn drop(&mut self) {
 224 |             let _ = std::fs::remove_dir_all(&self.0);
 225 |         }
 226 |     }

 228 |     struct FixedOpening(Opening);

 230 |     impl OpeningSource for FixedOpening {
 231 |         fn opening(&mut self, _rng: &mut StdRng) -> Opening {
 232 |             self.0.clone()
 233 |         }
 234 |     }

 236 |     fn mv(row: u8, col: u8) -> Move {
 237 |         Move::new(row, col).expect("test coordinate is on the board")
 238 |     }
```

*Provenance: byte-identical to the completed reference's
`src/bin/selfplaygen.rs` lines 187–238.*

The first unit test pins the declared default: parsing the minimal invocation
(`--out`, one `--openings`) yields the baseline own-TSS probe configuration —
enabled, 10 000 nodes, 30 plies.

**Source:** `gomoku/crates/selfplay/src/bin/selfplaygen.rs`, lines 240–250.

```rust
 240 |     #[test]
 241 |     fn generation_defaults_to_the_baseline_own_tss_probe() {
 242 |         let args = parse_args(
 243 |             ["--out", "corpus", "--openings", "list.txt"]
 244 |                 .into_iter()
 245 |                 .map(str::to_owned),
 246 |         )
 247 |         .expect("required arguments parse");

 249 |         assert_eq!(args.own_tss, player::OwnTssConfig::baseline());
 250 |     }
```

*Provenance: byte-identical to the completed reference's
`src/bin/selfplaygen.rs` lines 240–250.*

The second pins the full flag vocabulary and the two parse rules: every flag
supplied; `--games` twice (2, then 1) — the parsed count is 1, last value
wins; two `--openings` flags accumulate in order (first.txt, then
second.txt).

**Source:** `gomoku/crates/selfplay/src/bin/selfplaygen.rs`, lines 252–288.

```rust
 252 |     #[test]
 253 |     fn parses_all_flags_with_last_numeric_value_winning() {
 254 |         let args = [
 255 |             "--black-sims",
 256 |             "300",
 257 |             "--white-sims",
 258 |             "200",
 259 |             "--games",
 260 |             "2",
 261 |             "--games",
 262 |             "1",
 263 |             "--seed-base",
 264 |             "41",
 265 |             "--shard-capacity",
 266 |             "7",
 267 |             "--out",
 268 |             "corpus",
 269 |             "--openings",
 270 |             "first.txt",
 271 |             "--openings",
 272 |             "second.txt",
 273 |         ]
 274 |         .into_iter()
 275 |         .map(str::to_owned);

 277 |         let args = parse_args(args).expect("complete arguments parse");
 278 |         assert_eq!(args.black_sims, 300);
 279 |         assert_eq!(args.white_sims, 200);
 280 |         assert_eq!(args.games, 1);
 281 |         assert_eq!(args.seed_base, 41);
 282 |         assert_eq!(args.shard_capacity, 7);
 283 |         assert_eq!(args.out_dir, PathBuf::from("corpus"));
 284 |         assert_eq!(
 285 |             args.openings,
 286 |             [PathBuf::from("first.txt"), PathBuf::from("second.txt")]
 287 |         );
 288 |     }
```

*Provenance: byte-identical to the completed reference's
`src/bin/selfplaygen.rs` lines 252–288.*

The third pins the rejections: an invocation without `--out` fails with a
message containing `--out`; `--games 0` fails with "greater than zero".

**Source:** `gomoku/crates/selfplay/src/bin/selfplaygen.rs`, lines 290–303.

```rust
 290 |     #[test]
 291 |     fn rejects_missing_required_flags_and_zero_counts() {
 292 |         let missing_out = parse_args(["--openings".to_owned(), "list.txt".to_owned()].into_iter())
 293 |             .expect_err("--out is required");
 294 |         assert!(missing_out.contains("--out"));

 296 |         let zero_games = parse_args(
 297 |             ["--games", "0", "--out", "corpus", "--openings", "list.txt"]
 298 |                 .into_iter()
 299 |                 .map(str::to_owned),
 300 |         )
 301 |         .expect_err("zero games are invalid");
 302 |         assert!(zero_games.contains("greater than zero"));
 303 |     }
```

*Provenance: byte-identical to the completed reference's
`src/bin/selfplaygen.rs` lines 290–303.*

The fourth drives the run path through its seam. The `Args` are built
directly: both budgets 1, one game, seed base 11, shard capacity 1, the probe
**disabled** (the game plays in seconds; the baseline probe would make a
default-suite test slow — this fixture, not a full-budget game, is what makes
a default run affordable), the temporary output directory, and one opening
path that goes unused because the source is injected. The fixture opening
carries three fixed initial stones, an add-two answer with two fixed stones
and final color White, and `Listed` provenance. After the run, the output is
read back through the crate-root `read_manifest` and `read_corpus`, and the
assertions pin the manifest (one game, accepted seeds exactly `[11]`, shards
exactly `["shard-000.bin"]`) and the record (header seed 11, per-seat budgets
1 and 1, the fixture opening verbatim, at least one ply).

**Source:** `gomoku/crates/selfplay/src/bin/selfplaygen.rs`, lines 305–345
(to the end of the file).

```rust
 305 |     #[test]
 306 |     fn one_budget_one_game_writes_a_corpus_that_the_reader_round_trips() {
 307 |         let out = TestDir::new();
 308 |         let args = Args {
 309 |             black_sims: 1,
 310 |             white_sims: 1,
 311 |             games: 1,
 312 |             seed_base: 11,
 313 |             shard_capacity: 1,
 314 |             own_tss: OwnTssConfig::disabled(),
 315 |             out_dir: out.path().to_path_buf(),
 316 |             openings: vec![PathBuf::from("unused-fixed-opening.txt")],
 317 |         };
 318 |         let opening = Opening::new(
 319 |             [mv(5, 8), mv(5, 10), mv(7, 9)],
 320 |             OpeningAnswer::AddTwo {
 321 |                 stones: [mv(9, 8), mv(9, 10)],
 322 |                 final_choice: Color::White,
 323 |             },
 324 |             OpeningProvenance::Listed {
 325 |                 list: "fixed-test".to_owned(),
 326 |                 index: 0,
 327 |             },
 328 |         );
 329 |         let mut source = FixedOpening(opening.clone());

 331 |         run_with_opening_source(&args, &mut source).expect("one-game corpus generation succeeds");

 333 |         let manifest = read_manifest(out.path()).expect("manifest reads");
 334 |         let records = read_corpus(out.path()).expect("chapter-2 reader reads the shard");
 335 |         assert_eq!(manifest.game_count, 1);
 336 |         assert_eq!(manifest.seeds, [11]);
 337 |         assert_eq!(manifest.shards, ["shard-000.bin"]);
 338 |         assert_eq!(records.len(), 1);
 339 |         assert_eq!(records[0].header.seed, 11);
 340 |         assert_eq!(records[0].header.config.black.simulations, 1);
 341 |         assert_eq!(records[0].header.config.white.simulations, 1);
 342 |         assert_eq!(records[0].header.opening, opening);
 343 |         assert!(!records[0].plies.is_empty());
 344 |     }
 345 | }
```

*Provenance: byte-identical to the completed reference's
`src/bin/selfplaygen.rs` lines 305–345.*

### 11.5 `tests/binary.rs`: the help contract (slice form)

The boundary, restated: this file will hold **seven** tests when the track is
complete; this chapter creates it with exactly one —
`selfplaygen_help_documents_corpus_and_grading_flags` — and exactly one
import, the standard library's process `Command`. No `TestDir`, no corpus
fixtures, no other binaries: the two `selfplayanchors_*` tests belong to
chapter 05 and the four `selfplayverify_*` tests to chapter 07, and nothing
in this file anticipates them. The test runs the compiled binary — the path
from Cargo's compile-time binary environment variable for `selfplaygen` —
with `--help`, expects the spawn to succeed, asserts the exit status is
successful, decodes stdout as UTF-8, and asserts all seven flag strings
appear.

**Source:** [`gomoku/crates/selfplay/tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs),
lines 1–23 — the complete chapter-4 file. *Created in this chapter.*

```rust
   1 | use std::process::Command;

   3 | #[test]
   4 | fn selfplaygen_help_documents_corpus_and_grading_flags() {
   5 |     let output = Command::new(env!("CARGO_BIN_EXE_selfplaygen"))
   6 |         .arg("--help")
   7 |         .output()
   8 |         .expect("selfplaygen --help runs");

  10 |     assert!(output.status.success());
  11 |     let stdout = String::from_utf8(output.stdout).expect("help is UTF-8");
  12 |     for flag in [
  13 |         "--black-sims",
  14 |         "--white-sims",
  15 |         "--games",
  16 |         "--seed-base",
  17 |         "--shard-capacity",
  18 |         "--out",
  19 |         "--openings",
  20 |     ] {
  21 |         assert!(stdout.contains(flag), "help omitted {flag}:\n{stdout}");
  22 |     }
  23 | }
```

*Provenance: chapter-4 slice form. The import line is byte-identical to the
completed reference's `tests/binary.rs` line 2, and the test (slice lines
3–23) is byte-identical to the reference's lines 74–94, checked
programmatically. The completed reference's file additionally carries its
`TestDir`, `mv`, and `certified_record` fixtures and the six later-chapter
tests (reference lines 1, 3–72, and 96 onward) — all chapter 05 or 07, all
absent from the slice.*

### 11.6 `tests/grading.rs`: the report-only probe

The file's one helper, `options(simulations)`, is the struct-update-syntax
idiom over the default options — which keeps the probe disabled, the guard
disabled, and resignation disabled, exactly as a corpus run configures a
seat.

**Source:** [`gomoku/crates/selfplay/tests/grading.rs`](../../../../gomoku/crates/selfplay/tests/grading.rs),
lines 1–10. *Created in this chapter.*

```rust
   1 | use engine::Color;
   2 | use player::PlayerOptions;
   3 | use selfplay::{Outcome, play_game};

   5 | fn options(simulations: u32) -> PlayerOptions {
   6 |     PlayerOptions {
   7 |         simulations,
   8 |         ..PlayerOptions::default()
   9 |     }
  10 | }
```

*Provenance: byte-identical to the completed reference's `tests/grading.rs`
lines 1–10 (whole-file identity holds for the entire file).*

The test is MANUAL-ignored, and its two comments pin the design: four full
games run for minutes in debug (the reference's own comment ends with the
selection command; this track's convention — the tutorial README and primer
§7.3 — adds the `--release` qualifier, and the four budget-16 games need
it), and the seeds are pinned measurements, so a cheap sanity check does not
become a flaky Elo estimate. (Elo: the relative-strength rating named after
Arpad Elo.) Run the probe with the verification section's command — `cargo
test -p selfplay --release --test grading -- --ignored --nocapture` (§12) —
not with the comment's own selection line: the comment is quoted byte-exact
from the reference, so its selection line (`cargo test -p selfplay --test
grading -- --ignored`) keeps its original, unqualified form even though
these four games need `--release`. The behavioral contract: for each of the
pinned seeds
`[11, 1, 7, 18]`, play one game with Black at 16 simulations and White at 1;
count a Black win as a stronger-seat win, a White win as a weaker-seat win,
and a draw as neither; print one stderr line per seed with the ply count and
the outcome, so the report is visible on `--nocapture`. The assertion is only
a direction: the stronger seat wins strictly more often than the weaker seat.
No margin, no rate — this is a smoke signal, not a measurement; the design
spec's pre-registered grading gate is chapter 07's work. Section 13 reports
what the probe actually did on the slice, including one outcome the
reference's pinned comment does not describe.

**Source:** `gomoku/crates/selfplay/tests/grading.rs`, lines 12–47 (to the
end of the file).

```rust
  12 | #[test]
  13 | // MANUAL: four full games — minutes in debug. Run explicitly with
  14 | // `cargo test -p selfplay --test grading -- --ignored`.
  15 | #[ignore]
  16 | fn higher_budget_seat_wins_more_often_on_measured_seeds() {
  17 |     // Measured fixed seeds: four short games, all won by baseline-16.
  18 |     // Pinning avoids turning a cheap sanity check into a flaky Elo estimate.
  19 |     const SEEDS: [u64; 4] = [11, 1, 7, 18];

  21 |     let mut stronger_wins = 0;
  22 |     let mut weaker_wins = 0;
  23 |     for seed in SEEDS {
  24 |         let game = play_game(seed, options(16), options(1)).expect("graded game completes");
  25 |         match game.outcome {
  26 |             Outcome::Won {
  27 |                 winner: Color::Black,
  28 |                 ..
  29 |             } => stronger_wins += 1,
  30 |             Outcome::Won {
  31 |                 winner: Color::White,
  32 |                 ..
  33 |             } => weaker_wins += 1,
  34 |             Outcome::Draw => {}
  35 |         }
  36 |         eprintln!(
  37 |             "grading seed {seed}: {} plies, {:?}",
  38 |             game.plies.len(),
  39 |             game.outcome
  40 |         );
  41 |     }

  43 |     assert!(
  44 |         stronger_wins > weaker_wins,
  45 |         "stronger seat won {stronger_wins}, weaker seat won {weaker_wins}"
  46 |     );
  47 | }
```

*Provenance: byte-identical to the completed reference's `tests/grading.rs`
lines 12–47.*

## 12. Verification

The chapter's commands, run from `gomoku/` — the same command set the
approved plan's step 10 names, listed here in the order the slice's
verification run executed and logged them, with the capstone at the end:

```text
cargo test -p selfplay --test openings --test schedule --test dedup
cargo test -p selfplay --bin selfplaygen
cargo test -p selfplay --test binary selfplaygen_help
cargo test -p selfplay
cargo test -p selfplay --release --test schedule -- --ignored
cargo test -p selfplay --release --test game -- --ignored
cargo test -p selfplay --release --test grading -- --ignored --nocapture
cargo build --release -p selfplay && <the two-process capstone below>
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the fast focused check: the parser, schedule-property,
and dedup targets. The second runs the binary's own unit tests — including
the budget-1 round trip, the chapter's one default-run game. The third is the
help-text contract selected by name, which leaves the six later-chapter
binary tests unrun — they do not exist yet. The fourth is the fast gate;
cargo prints one report per test target, never one aggregate line, and the
chapter's closing state is the sum of the per-target reports. The three
`--release` commands are the MANUAL selections, and the qualifier is not
optional for any of them: all three play full games. The schedule selection
watches this chapter's scheduled game; the grading selection is the
report-only probe, and it carries `--nocapture` so the probe's per-seed
report is visible in the log (section 13.3) — test output capture would
otherwise hide it; the game-target selection is the regression with teeth —
chapter 01's MANUAL pair runs the move loop this chapter rewired, so the
budget-2-versus-1 visit-sum test and the budget-8 smoke test re-prove the
loop with the schedule active. The last two commands are the project's
standing quality gates. `cargo test -p train` and `cargo test -p player` are
not required: this chapter touches no crate but `selfplay`.

Evidence actually measured for this paper (2026-11-13, on the chapter-4 slice
constructed in the scratch copy described under Provenance — not a re-run of
the completed reference, whose gate evidence the
[hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md)
records; logs at `/tmp/selfplay-ch4-verify-logs/`, scratch-local and
ephemeral):

| Command | Result on the chapter-4 slice |
|---------|-------------------------------|
| `cargo test -p selfplay --test openings --test schedule --test dedup` | openings 5 passed, 0 failed, 0 ignored (0.00 s); schedule 1 passed, 0 failed, 1 ignored (0.00 s); dedup 1 passed, 0 failed, 0 ignored (0.00 s) — all after one debug build |
| `cargo test -p selfplay --bin selfplaygen` | 4 passed, 0 failed, 0 ignored (6.24 s — the budget-1 round trip dominates) |
| `cargo test -p selfplay --test binary selfplaygen_help` | 1 passed, 0 failed (0.26 s) |
| `cargo test -p selfplay` | lib target: 0 tests; `selfplaygen` unit target: 4 passed, 0 ignored (6.23 s); binary target: 1 passed; corpus target: 10 passed, 0 ignored (0.01 s); dedup target: 1 passed; game target: 5 passed, 2 ignored (12.47 s, debug); grading target: 0 passed, 1 ignored; openings target: 5 passed; schedule target: 1 passed, 1 ignored; doc-tests: 0 — **27 passed, 4 ignored** in total |
| `cargo test -p selfplay --release --test schedule -- --ignored` | 1 passed, 0 failed, 1 filtered out (1.96 s) — `scheduled_game_is_reproducible_and_records_its_schedule_and_pairing` green |
| `cargo test -p selfplay --release --test game -- --ignored` | 2 passed, 0 failed, 5 filtered out (0.80 s) — chapter 01's MANUAL pair green through the rewired move loop |
| `cargo test -p selfplay --release --test grading -- --ignored --nocapture` | 1 passed, 0 failed (7.12 s) — per-seed report in section 13.3 |
| capstone (below) | both `cmp` calls exit silently; shard SHA-256 `08d358e1…6539`, manifest SHA-256 `54024c26…ac4e`, identical across the two processes |
| `cargo clippy --all-targets -- -D warnings` | clean, workspace-wide |
| `cargo fmt --all -- --check` | no changes |

Two count notes keep the evidence precise, and they double as the
distinction between this paper's slice evidence and the completed
reference's hand-off evidence. First, the completed reference's full suite is
larger — 50 passed, 4 ignored — entirely because chapters 05–07 add their own
targets plus chapter 05's two `game.rs` unit tests; the four ignored tests
are exactly this chapter's end state (chapter 01's pair, the schedule game,
the grading probe). Second, the corpus target shows three `test result`
lines in the full-suite log: its cross-process determinism test re-executes
the test binary twice as a child process, and the children's one-test reports
appear interleaved in the parent log because the children inherit its stdio —
chapter 02's mechanism, not this chapter's, and unchanged.

### The capstone: two processes, one byte-identical corpus

Generate a tiny corpus twice and prove the seed discipline byte-exactly. The
output directories are temporary — `mktemp -d` keeps the run out of the
repository's `data/` tree, now and for every future generation exercise; main
`data/` is never a generation target:

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

On the slice (release build, 2026-11-13), both invocations reported
`Generating 2 games: base seeds 424242..=424243, baseline-8 vs baseline-8`
before play and `Games: 2, shards: 1, accepted seeds: [424242, 424243]` after
the write; both `cmp` calls exited silently; the shard's SHA-256 was
`08d358e1f8485e9bb9a47cc60600e54eb40a2263b26ea4125f58979bdd216539` in both
processes and the manifest's
`54024c26a0f4c2bbe2fbb8a33fa86471034f965b4c734401470acf018225ac4e`. The
manifest records the declared pairing (baseline-8 vs baseline-8), the default
exploration schedule, the baseline own-TSS probe on both seats, and the guard
and resignation disabled — exactly the corpus configuration of section 9.2.
The capstone directories are retained at `/tmp/selfplaygen-capstone-a.MISAuv`
and `/tmp/selfplaygen-capstone-b.lgZxgn` (scratch-local, ephemeral). Three
properties land in one exercise: the default shard capacity 1000 writes a
single `shard-000.bin`; the stderr lines report the declared range and the
exact accepted seed list; and two invocations, two processes, one seed set
produce byte-identical shards and manifest — the project's determinism gate
rehearsed at two games instead of the reference's measured three. Two
honesty notes: the capstone is determinism evidence only — two seeded
processes agree with or without the schedule, so it says nothing about
whether the application line of section 5 exists; and the reference's
measured evidence (byte-identical three-game corpora at the same seed base,
SHA-256 `4388d84e…ae52`, recorded in the hand-off) is reference acceptance
evidence, distinct from this paper's fresh slice evidence above.

## 13. Open observations — reported, not resolved

Three observations from building and measuring the reference belong to this
chapter's mechanics, stated in the chapter paper and held here. Each is
reported with the slice's fresh evidence where the slice exercises it. None
is resolved; none is hidden; no strength conclusion is drawn anywhere in this
chapter.

### 13.1 The piskvork list is rejected — and the working pool is 36 offers

The committed
[piskvork-openings.txt](../../../../data/openings/piskvork-openings.txt)
holds 41 lines, and its line 41 contains the coordinate `-8,-6` — column −1
after the center-relative mapping, off the 15×15 board (verified directly:
line 41 reads `-8,-6, -7,-6, -8,-5, -6,-4, -6,-5`). The parser rejects the
file with `line 41` and `outside` in the message, and the regression test
`malformed_committed_piskvork_line_is_rejected_with_its_line_number` pins the
behavior — fresh slice evidence: the focused openings run (section 12) passed
with this test green. The rejection is correct, but its operational
consequence was measured in the reference's acceptance run: a generation
command that included the piskvork path among its `--openings` flags failed
fast in 0.16 s and produced no games, because `load` rejects the whole call
on the first malformed line (reference acceptance evidence, per the
hand-off). The working pool is therefore the three committed Gomocup
freestyle lists, 36 offers in total — the slice's fresh evidence agrees:
`valid_committed_tournament_lists_load_into_one_uniform_offer_pool` asserts
exactly 36. The reference's measured first corpus (48 games, chapter 07's
evidence) saw 28 of those 36 offers with a maximum single-offer share of
8.33 %. Whether and how the piskvork list joins the pool — corrected,
filtered, or replaced — is deliberately left open; the
[data/openings README](../../../../data/openings/README.md) documents the
file's source.

### 13.2 Debug builds are too slow for game-playing verification

The reference measured roughly 25 s for an 8-simulation game in a debug
build; the consequence is procedural — every game-playing verification runs
with `--release`, and the slow game-playing tests are `#[ignore]`d with a
MANUAL comment and run by explicit selection. This chapter follows the
discipline exactly, and the slice's fresh evidence shows the shape of it: the
default suite's slowest target is the game target at 12.47 s in debug
(chapter 01 and 03 games at budgets of at most 2), the binary's budget-1
round trip costs 6.24 s in debug, and the three MANUAL selections play their
games in release — 1.96 s for the two budget-2-versus-1 scheduled games,
0.80 s for chapter 01's pair, 7.12 s for the four budget-16-versus-1 grading
games. The opening tests parse text and load the committed lists, the
schedule property test plays no game, and the dedup test fabricates its
games — this chapter adds nothing slow to the default suite.

### 13.3 The grading probe: a pass, an unpinned per-seed detail, and the unresolved inversion

Keep two things distinct: this chapter's MANUAL test is a four-game smoke
signal with a direction assertion; the inverted 20-game measurement is the
reference's report-only observation. They are not the same run, and neither
licenses a strength claim.

**The MANUAL test on the slice.** The pinned comment in `tests/grading.rs`
records the reference's measurement: "four short games, all won by
baseline-16." On the chapter-4 slice (release, 2026-11-13, run twice with
identical outcomes — logs 07 and 07b agree seed for seed, as the play path is
deterministic), the per-seed report was:

```text
grading seed 11: 13 plies, Won { winner: White, reason: FiveOrMore }
grading seed 1: 220 plies, Draw
grading seed 7: 92 plies, Won { winner: Black, reason: FiveOrMore }
grading seed 18: 102 plies, Won { winner: Black, reason: FiveOrMore }
```

That is two stronger-seat wins (seeds 7 and 18), one weaker-seat win (seed
11, won by the budget-1 White seat in 13 plies), and one draw. The direction
assertion — the stronger seat wins strictly more often than the weaker seat,
2 > 1 — passes, and the test is green. But the per-seed outcomes do not
reproduce the reference comment's "all won by baseline-16": on this slice the
budget-1 seat won one of the four pinned games outright. The play path cannot
explain the difference by construction — the slice's `engine`, `mcts`,
`baseline`, and `train` sources are byte-identical to the reference
worktree's, and its `player` crate differs only by chapter 05's `own_proof`
retention, which does not touch selection — so the difference is between the
reference's measurement context and this slice's, not between the slice and
the reference code. The paper reports this without resolving it: the
comment's claim is the reference's measurement, the outcomes above are the
slice's fresh measurement, and the test's direction gate passes under both.
If anything, the slice's run reinforces the chapter's caution: at four games,
a budget-16-versus-1 pairing can lose a game to the budget-1 seat.

**The 20-game inversion (reference acceptance evidence, unresolved).** The
reference's acceptance measurements include a report-only run of 20 games at
baseline-32 (Black) versus baseline-1 (White), generated with this chapter's
binary at seeds 200000–200019. The budget-1 seat won 15 of 20 — the opposite
of the grading expectation that more simulations win more often. The caveats
are real and stated: the sample is 20 games, far too small to separate budget
effects from color, openings, or tactical quirks; the exploration schedule
handicaps the stronger seat, because ten plies of temperature-1 sampling with
root noise perturb exactly the seat whose search would otherwise exploit the
weak opponent; and the run used fixed colors — the stronger seat always
played Black, because the binary declares budgets per color and no
color-swapping exists yet. None of these caveats resolves the observation,
and this chapter offers no resolution and no strength claim: the probe is
evidence that the ladder's load-bearing assumption needs the real gate, which
chapter 07 pre-registers and runs. This paper adds no fresh 20-game
measurement; the observation stands as the reference recorded it.

## References

**This chapter:**

- [diversity-and-grading.md](diversity-and-grading.md) — the chapter paper:
  intention, specification as tests, execution-order walkthrough, open
  observations.
- [diversity-and-grading-design.md](diversity-and-grading-design.md) — the
  conceptual story: diversity inside a corpus versus grading across corpora,
  the seed discipline, the state machine, the nine design invariants.
- [diversity-and-grading-implementation.md](diversity-and-grading-implementation.md) —
  the hiding layer this paper quotes the construction contracts of; its
  steps 1–10 fix the chapter-4 slice forms and the test fixtures.

**This track:**

- [Tutorial README](../README.md) — conventions, gates, the chapter map, and
  the reference-provenance rule this paper's Provenance section applies.
- [Self-play primer](../00-selfplay-primer.md) — §4.4 fixes the five
  diversity mechanics; §6 holds the measured acceptance evidence this paper
  quotes; §7 states the three open observations, including §7.3's
  release-mode convention; §8 pins the track invariants.
- [Chapter 01 — the game runner](../01-the-game-runner/the-game-runner.md)
  and its [solution](../01-the-game-runner/the-game-runner-solution.md) —
  the `OpeningSource` seam, `GeneratedOpening` and its private draw helpers
  (`draw_unique`, `random_color`), the single-RNG discipline, the move loop
  this chapter schedules, the `play_game` doc delta this chapter retires,
  and the MANUAL pair that re-proves the rewired loop.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md)
  and its [solution](../02-recording-games/recording-games-solution.md) —
  the `ExplorationSchedule` vocabulary this chapter must not recreate,
  `SeatPairing.exploration`, `Manifest.seeds`, `GameRecord::from_game`,
  `write_corpus` with overwrite protection, the readers, and the
  `SeatConfig.temperature` slice wording this chapter retires.
- [Chapter 03 — recording the search](../03-recording-the-search/recording-the-search.md)
  and its [solution](../03-recording-the-search/recording-the-search-solution.md) —
  the `Option::map` conversion line the move loop retains, the honesty rule
  for certified-proof moves that the schedule never disturbs, and the
  chapter-3 scratch (`/tmp/selfplay-ch3-verify`) this chapter's slice was
  copied from.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  §3.1 the runner shape, §4 the five diversity mechanics, §5 the acceptance
  gates; decisions S5 (grading as primary axis) and S6 (synchronous, one
  seed per process).
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  the reference-implementation layout, the measured headline numbers (the
  three-game determinism run, the 48-game first corpus, the 20-game grading
  inversion), and the open questions.
- [docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) —
  defines the **[paper]** / **[derived]** / **[experiment]** claim-label
  convention.
- [docs/14-openings-and-supervised-curriculum.md](../../../14-openings-and-supervised-curriculum.md) —
  §3.2 the tier-1 tournament lists, §3.3 the perturbed answering argmax this
  chapter approximates with a seeded uniform choice **[invention]**, and the
  honesty comment quoted in section 3.
- [docs/13-engine-design.md](../../../13-engine-design.md) — the locked
  absolute-color decision the opening vocabulary carries.

**Prerequisite tracks:**

- [Baseline tutorial](../../baseline-tutorial/README.md) —
  `AiPlayer::decide`, `PlayerOptions`, `OwnTssConfig::baseline()` (10 000
  nodes / 30 plies), and the guard the corpus configuration leaves disabled.
- [MCTS tutorial](../../mcts-tutorial/README.md) — the PUCT search whose
  root carries the Dirichlet noise during exploration plies.
- [Engine tutorial](../../13-engine-tutorial/README.md) — the Swap2
  typestate, `Move::new`, and the Zobrist keys this chapter walks.
- [Datagen tutorial](../../datagen-tutorial/README.md) — the `train` crate's
  shard and manifest storage discipline.
- [gomoku/crates/cli/README.md](../../../../gomoku/crates/cli/README.md) —
  the AI-vs-AI exploration practice the schedule mirrors.
- [data/openings/README.md](../../../../data/openings/README.md) — the
  committed opening lists and their sources.

**Source (created by this chapter; the links resolve once the chapter lands
on `main`):**

- [`gomoku/crates/selfplay/src/generate.rs`](../../../../gomoku/crates/selfplay/src/generate.rs) —
  `GenerateError`, `generate_games`, `generate_games_with`; private
  `MAX_ATTEMPTS_PER_GAME`, `zobrist_walk`, `opening_moves` (section 7).
- [`gomoku/crates/selfplay/src/bin/selfplaygen.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplaygen.rs) —
  the corpus generator binary and its four unit tests (sections 8, 9, 11.4).
- [`gomoku/crates/selfplay/tests/openings.rs`](../../../../gomoku/crates/selfplay/tests/openings.rs),
  [`tests/schedule.rs`](../../../../gomoku/crates/selfplay/tests/schedule.rs),
  [`tests/dedup.rs`](../../../../gomoku/crates/selfplay/tests/dedup.rs),
  [`tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs),
  [`tests/grading.rs`](../../../../gomoku/crates/selfplay/tests/grading.rs) —
  this chapter's specification (section 11).

**Source (created by earlier chapters, touched by this chapter):**

- [`gomoku/crates/selfplay/src/opening.rs`](../../../../gomoku/crates/selfplay/src/opening.rs) —
  gains `OpeningParseError`, `OpeningLoadError`, `TournamentOpenings` with
  `parse`, `load`, `len`, `is_empty`, and the `OpeningSource` implementation
  (sections 1–3).
- [`gomoku/crates/selfplay/src/schedule.rs`](../../../../gomoku/crates/selfplay/src/schedule.rs) —
  gains the `PlayerOptions` import and `ExplorationSchedule::options_for_ply`
  (section 4); nothing else changes.
- [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs) —
  the move loop applies the schedule per ply (section 5); the two doc-comment
  updates (sections 6.1, 6.2).
- [`gomoku/crates/selfplay/src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs) —
  the `SeatConfig.temperature` doc comment moves to the reference wording
  (section 6.3); no code change.
- [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs) —
  declares the `generate` module and re-exports the new vocabulary (section
  10).

**Source (unchanged in this chapter):**

- [`gomoku/crates/selfplay/src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs)
  and `src/corpus.rs` mechanics — `GameRecord::from_game`, `Manifest.seeds`,
  `write_corpus`, `read_corpus`, `read_manifest`; consumed through call sites
  only.
- [`gomoku/crates/selfplay/tests/game.rs`](../../../../gomoku/crates/selfplay/tests/game.rs)
  and [`tests/corpus.rs`](../../../../gomoku/crates/selfplay/tests/corpus.rs) —
  the earlier chapters' specifications, byte-identical to the completed
  reference (scripted whole-file comparison); regression evidence.
- [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs) —
  `PlayerOptions`, `OwnTssConfig`, `GuardConfig`; the options the schedule
  adjusts and the probe the binary declares. Not touched.
- [`gomoku/crates/selfplay/Cargo.toml`](../../../../gomoku/crates/selfplay/Cargo.toml) —
  no manifest changes.

**Reference provenance:** the completed reference implementation lives in the
read-only sibling worktree `rust-ml-selfplay-ref` (branch `selfplay-ref-v2`,
base `9028aa8`, changes uncommitted); it is quoted and compared against,
never linked, and it is deleted once the tutorial is complete on `main`. The
chapter-4 slice in this paper was constructed per the implementation plan in
an untouched copy of the verified chapter-3 scratch
(`/tmp/selfplay-ch3-verify` → `/tmp/selfplay-ch4-verify`) and gate-verified
there on 2026-11-13 (logs at `/tmp/selfplay-ch4-verify-logs/`); the reference
worktree, the chapter-3 scratch, and the earlier scratch copies
(`/tmp/selfplay-ch1-verify`, `/tmp/selfplay-ch2-verify`) were not modified.
The capstone corpora live at `/tmp/selfplaygen-capstone-a.MISAuv/corpus` and
`/tmp/selfplaygen-capstone-b.lgZxgn/corpus` (scratch-local, ephemeral).

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). <https://doi.org/10.1038/nature24270> —
  temperature sampling over visit counts and root Dirichlet noise, the
  exploration mechanisms the schedule applies.
- Rosin, C. D. (2011). *Multi-armed bandits with episode context* (PUCT).
  <https://doi.org/10.1007/s10472-011-9258-6> — the selection rule of the
  search the schedule leaves untouched.
- Allis, L. V. (1994). *Searching for Solutions in Games and Artificial
  Intelligence*. PhD thesis, Maastricht University.
  <https://doi.org/10.26481/dis.19940923la> — threat-space search, the seat
  probe the binary declares on both seats.
