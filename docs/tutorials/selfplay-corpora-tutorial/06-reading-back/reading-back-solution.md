> **Opt-in solution — read it only after attempting the chapter with the
> [implementation plan](reading-back-implementation.md), or after being
> stuck for more than twenty minutes.**
>
> This paper is inspiration, not authority over the learner's own code. The
> learner's implementation on `main` is the reference; the artifacts below
> exist so a stuck or finished learner can compare. No learner source is
> changed by this paper.

# Chapter 06 — Reading back: solution

## Abstract

This paper is the annotated reference companion to
[chapter 06 of the selfplay-corpora tutorial](reading-back.md). It quotes
everything the chapter creates, along the execution path of one position
travelling from a stored record to a training batch: the new
`src/sample.rs` with the four-variant `SampleError`, `replay_sample`,
`augment_sample`, `position_is_holdout`, and the private `replay_board` and
`stones` helpers; the new `src/sampler.rs` with `SamplerError` and the
seeded, infinite `CorpusSampler`; the new `src/mix.rs` with `MixError`,
`WeightedSource`, and the weighted round-robin `SourceMixer`; the new
`src/export.rs` with `SAMPLE_EXPORT_FORMAT_VERSION`, `ExportManifest`, the
four-variant `ExportError`, and the validate-before-write `export_corpus`;
the new `src/bin/selfplayexport.rs` binary (usage text, argument parser,
the four-step run path with its read-back count check, and the exit codes);
the crate-root module declarations and re-exports in `src/lib.rs`; and the
eleven-test specification in `tests/read_path.rs`, quoted in full. The
paper closes with the chapter's verification evidence: the focused
read-path run at 11 passed, the full `selfplay` suite at 46 passed and 4
ignored, the untouched `train` and `player` suites green as regression
evidence, the workspace gate, the standing clippy and rustfmt gates, and a
fresh debug capstone that exported the 48-game baseline-32 corpus — 1 860
samples into 4 shards — read it back through the real `train` dataset
reader inside the binary, and refused a second export into the same
directory with exit code 1. The reference's integration smoke (the same
1 860-sample export followed by 50 `nettrain` steps, train loss 6.5483584
at step 1 to 6.252371 at step 50) is quoted as historical acceptance
evidence, clearly separated and not re-run. Every snippet carries an
explanation, source line numbers, and an exact provenance note: the six
files this chapter creates in full —
`src/sample.rs`, `src/sampler.rs`, `src/mix.rs`, `src/export.rs`,
`src/bin/selfplayexport.rs`, and `tests/read_path.rs` — are byte-identical
to the completed reference implementation, and `src/lib.rs` is the
chapter-6 slice form whose every difference from the chapter-5 slice is
named explicitly (and which a scripted whole-file comparison shows to be
byte-identical to the completed reference's `src/lib.rs`, because chapter
07 adds no modules). All chapter gates ran green on the chapter-6 slice on
2026-11-14; the precise evidence is in the Provenance and Verification
sections. The intended reader is the learner who has read the chapter
paper and, ideally, worked the
[implementation plan](reading-back-implementation.md) first.

## Glossary

| Term | Definition |
|------|------------|
| **Game record** | `GameRecord`: the durable serialization of one complete game — a header (seed, seat pairing, evaluator identity, the Swap2 opening), the ordered post-opening `plies`, the `outcome`, and an optional certified suffix. Chapter 02 fixed the format. |
| **Recorded ply** | `RecordedPly { mv, visits, prior, root_value }`: one played post-opening move and its durable search data. `visits` holds sparse raw root visit counts, descending; `prior` holds up to 16 root priors for audit; both are empty when no search ran, and `root_value` is then `None`. |
| **Swap2** | The project's opening protocol: one side places three stones, the other side chooses a color or places two more stones, and the first side then chooses a color. The placements are the ceremony, not plies. |
| **Post-opening ply** | One played move after the Swap2 ceremony, counted from zero. `replay_sample`, `position_is_holdout`, and the export loop count in these plies; the sample for a ply describes the position immediately before that ply is played. |
| **Sample** | `train::Sample`: the labelled training record — `black` and `white` stone lists in absolute color, `to_move`, a sparse `policy` of `(move, probability)` pairs, and a `value` target from the side-to-move perspective. It stores stones, never planes. |
| **π (policy target)** | The normalized root visit-count distribution of the search at a ply; the policy training target (Silver et al. 2017). Computed at read time from the recorded raw counts. |
| **z (value target)** | The game outcome in {+1, 0, −1} from the perspective of the side to move at the sampled ply; the value training target. |
| **Prior** | The evaluator's policy probabilities over root moves before search, recorded sparsely (top 16) for audit. Audit data, not a training target: the read path never turns priors into a policy target. |
| **Root value** | The evaluator's value estimate at the root of a ply's search, recorded for audit. Never a training target: z comes from the outcome, not the interim guess. |
| **Canonical orientation** | The position exactly as played, with no D4 transform applied. Records and exported shards are canonical; augmentation happens per sample, never per batch file. |
| **D4** | The dihedral group of the square: the 8 rotations and reflections of the board, `engine::Transform::ALL`. |
| **Zobrist key** | A 64-bit hash of a board position, maintained incrementally by the engine (`Board::zobrist`). Orientation-specific: a transformed position hashes differently. |
| **Holdout split** | The train/holdout partition of the `train` crate: a sample is held out iff `zobrist(board) % 100 < HOLDOUT_PERCENTAGE` (= 10). Content-based, so collection order cannot leak correlated positions into both sides. |
| **CorpusSampler** | The seeded, infinite iterator of augmented training samples over a corpus: each draw picks a random game, a random ply, and a random D4 transform, and replays on demand. |
| **SourceMixer / WeightedSource** | The seeded weighted round-robin over sample iterators: each `WeightedSource` contributes `weight` consecutive items per turn, and the seed picks the initial source. |
| **Shard** | A fixed-capacity file of length-delimited bincode records; the `train` crate's storage unit, shared by synthetic datasets, game corpora, and sample exports. |
| **ExportManifest** | The JSON provenance of an exported sample dataset: format version, game count, sample count, shard file names in sample order, and the workspace version. |
| **TSS bypass** | The certified own-side threat-space search (TSS; Allis 1994) proof that short-circuits a decision: the line's first move is played and no search runs. Such a ply records empty `visits` and `prior` and `root_value: None` (chapter 03's honesty rule). |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games. It never runs by default; it runs on explicit selection, in release mode. This chapter adds none. |
| **Chapter-6 slice** | The chapter-6 subset of the completed reference: chapters 01–06 together, without chapter 07's additions (`selfplayverify` and its four binary tests). Defined and verified in the Provenance section. |
| **TDD** | Test-driven development: write the test, watch it fail (red), implement, watch it pass (green). |

## Provenance, the chapter-6 slice, and verification

### The reference and its completed-tree form

The reference implementation lives in a separate, read-only worktree beside
this repository (`rust-ml-selfplay-ref`, branch `selfplay-ref-v2`, committed
base `9028aa8` on `main`, all reference changes uncommitted). It was built
test-first and gate-verified before this tutorial was written; the
[hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md)
records its measured evidence. The worktree serves quotation only: never
point a manifest at it, never build the learner's crate from it, never
commit there. Chapter 01's
[solution paper](../01-the-game-runner/the-game-runner-solution.md)
established the provenance discipline this paper continues; chapter 05's
[solution paper](../05-certified-endgames/certified-endgames-solution.md)
applied it to the chapter-5 slice this chapter's slice was copied from.

One structural fact shapes everything below: the worktree holds the
**completed** crate — all seven chapters at once. Its `src/bin/` carries
the chapter-07 binary `selfplayverify` beside this chapter's
`selfplayexport`, and its `tests/binary.rs` holds seven tests, four of
which (`selfplayverify_*`, reference lines 144–232) belong to chapter 07.
Quoting those artifacts would leak the next chapter and would contradict
this chapter's approved papers, which fix the chapter-6 form:
`tests/binary.rs` stays at exactly three tests, and no
`selfplayverify`-shaped code exists anywhere in the slice (see
[the implementation plan](reading-back-implementation.md)). This paper
therefore distinguishes two kinds of quotation:

| File | Form | Evidence |
|------|------|----------|
| `src/sample.rs` | reference-verbatim | whole-file byte comparison: IDENTICAL |
| `src/sampler.rs` | reference-verbatim | whole-file byte comparison: IDENTICAL |
| `src/mix.rs` | reference-verbatim | whole-file byte comparison: IDENTICAL |
| `src/export.rs` | reference-verbatim | whole-file byte comparison: IDENTICAL |
| `src/bin/selfplayexport.rs` | reference-verbatim | whole-file byte comparison: IDENTICAL |
| `tests/read_path.rs` | reference-verbatim | whole-file byte comparison: IDENTICAL |
| `src/lib.rs` | **chapter-6 slice form** | the chapter-5 file plus exactly this chapter's four module declarations and four re-export lines; scripted whole-file comparison shows the result is *also* byte-identical to the completed reference's `src/lib.rs`, because chapter 07 adds no modules and changes no exports |

The six verbatim files are stable in the completed reference: chapter 07
adds its own binary and its own tests and never touches them (a scripted
content scan of all seven found no `verify`-shaped code). The comparisons
were scripted, not eyeballed: whole-file byte comparisons, logged in
`/tmp/selfplay-ch6-verify-logs/14-provenance-diff.log` (seven `IDENTICAL`
verdicts). Because the whole files are identical, the quoted line numbers
are simultaneously the slice's and the reference's. For `src/lib.rs` the
numbers are the slice's; the provenance note on that section names the
exact delta against the chapter-5 slice. No chapter-07 implementation is
quoted anywhere in this paper.

### The slice was built and run

Quoting a slice the learner could not compile would violate the track's
first rule — a tutorial that does not run error-free is a defect. So the
chapter-6 slice was constructed and gate-verified on 2026-11-14 at
`/tmp/selfplay-ch6-verify` (a scratch path, not a repository artifact). The
starting point was not the reference worktree but an *untouched copy* of
the already-verified chapter-5 slice at `/tmp/selfplay-ch5-verify` (copied
in full, build directory included, so the verification runs are
incremental) — so the chapter-6 slice differs from chapter 05's verified
state by exactly this chapter's additions, and neither the chapter-5
scratch nor the reference worktree was modified:

1. `src/sample.rs`, `src/sampler.rs`, `src/mix.rs`, `src/export.rs`,
   `src/bin/selfplayexport.rs`, and `tests/read_path.rs` were copied from
   the completed reference. Each is byte-identical to the reference file
   (scripted whole-file comparison): chapter 07 never touches these files,
   so the completed reference carries exactly the chapter-6 form of each.
2. `src/lib.rs` was written as the chapter-6 slice form: the chapter-5
   file plus the `export`, `mix`, `sample`, and `sampler` module
   declarations (in alphabetical position) and their four re-export lines
   (each in its module's alphabetical group position). A scripted
   whole-file comparison then confirmed the slice form is byte-identical
   to the completed reference's `src/lib.rs` — chapter 07's verifier is a
   binary and four test functions, so the crate root never changes again.

Nothing else was touched: no manifest change (`selfplay` already depends on
`engine`, `train`, `player`, `rand`, `serde`, `serde_json`, and
`thiserror`; a binary in `src/bin/` needs no manifest entry), no change to
any chapters 01–05 file, and no chapter-07 module, binary, or test
anywhere. Scripted whole-file comparisons against the chapter-5 scratch
confirm that every earlier-chapter file — `src/game.rs`, `src/anchor.rs`,
`src/corpus.rs`, `src/generate.rs`, `src/opening.rs`, `src/record.rs`,
`src/schedule.rs`, `src/bin/selfplaygen.rs`, `src/bin/selfplayanchors.rs`,
`Cargo.toml`, and the eight untouched test targets including
`tests/binary.rs` — is unchanged (all logged in
`14-provenance-diff.log`, eighteen `UNCHANGED` verdicts), and the slice
contains no `selfplayverify` binary and no `selfplayverify_*` test.

The chapter's verification commands were run from the copy's `gomoku/`,
with stdout and stderr saved to `/tmp/selfplay-ch6-verify-logs/` (a scratch
path, not a repository artifact):

```text
cargo test -p selfplay --test read_path
cargo test -p selfplay
cargo test -p train
cargo test -p player
cargo test --workspace
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build -p selfplay && <the capstone below>
```

All commands passed on the slice; the measured evidence — per-target
counts, timings, and the capstone's export, refusal, and read-only-input
hash checks — is tabled in [§12](#12-verification), and the reference's
integration smoke is quoted there as historical evidence, attributed and
not re-run. The slice fragments quoted below are exactly the files those
runs exercised.

### Reading rules

The section order is the execution order of one position travelling from a
stored record to a training batch, matching the chapter paper's
walkthrough: replay first (the error vocabulary, then `replay_sample`,
then augmentation, then the holdout partition — sections 1–5), then the
sampling and mixing streams (sections 6–7), then the batch export and its
binary driver (sections 8–9), then the crate-root surface (section 10),
the eleven-test specification (section 11), the verification (section
12), and the troubleshooting (section 13). Within each file the order is the file's own, with one exception:
`src/sample.rs` follows the chapter's walkthrough — augmentation before
the holdout partition — rather than the file's declaration order.

Every code fragment carries a right-aligned `NN |` line-number margin. The
numbers are the chapter-6 file's own line numbers — the same numbers the
learner's files will have after the chapter's artifacts land. (For the six
reference-verbatim files the numbers are also the completed reference's,
because the whole files are identical; for the one slice-form file they are
the slice's.) The margin is the only annotation added; the code text itself
is quoted exactly, and each fragment's provenance note states whether it is
byte-identical to the completed reference or a chapter-6 slice form (with
the exact delta). Blank separators between lines appear without a margin;
no fragment skips a nonblank line inside its quoted range, and every file
this chapter creates is quoted in full across its fragments.

File links in this paper point into the learner's tree on `main`. Links to
files the track creates — for example
[`gomoku/crates/selfplay/src/sample.rs`](../../../../gomoku/crates/selfplay/src/sample.rs) —
resolve once chapters 01–06 land. The
[`gomoku/crates/train/`](../../../../gomoku/crates/train/) and
[`gomoku/crates/engine/`](../../../../gomoku/crates/engine/) links resolve
today; the `train` crate's `storage.rs` module they will show lands with
this track's chapter 02, which creates it by extraction. The reference worktree is not linked:
it is temporary provenance, deleted once the tutorial is complete on
`main`, and everything needed from it is quoted here. The scratch copies
and their logs (`/tmp/selfplay-ch5-verify`, `/tmp/selfplay-ch6-verify`,
`/tmp/selfplay-ch6-verify-logs/`) are cited as evidence of this paper's own
verification; they are ephemeral and not part of the repository.

## 1. `src/sample.rs`: the module and `SampleError`

The first of the chapter's five new source files. The module comment states
the placement decision: replay lives in `selfplay`, not `train`, because
replay owns the `GameRecord` and opening vocabulary — and the dependency
already points `selfplay → train` for storage, so `train` stays the shared
sample and shard-format crate without learning about self-play records. The
imports state the consumed surface: the engine's `Board`, `Color`, `Move`,
`PlayError`, and `Transform`; the `thiserror` derive; `train::Sample`; and
the crate's own `GameRecord` and `Outcome`.

`SampleError` has four variants, in the order the checks fire:
`PlyOutOfBounds` (the requested post-opening ply does not exist, carrying
the requested ply and the record's ply count), `ZeroVisitTotal` (a nonempty
visit histogram contains no actual visits), `Opening` (the record's Swap2
opening was rejected, carrying the engine's `PlayError` as the source), and
`Replay` (a recorded move was rejected during replay, carrying the
zero-based ply, the move, and the engine error). The enum derives `Debug`
and the `thiserror` error derive only — no `Clone`, no `PartialEq`:
`PlayError` does not support them, and the tests match on variants with the
match idiom, so equality is not part of the contract.

**Source:** [`gomoku/crates/selfplay/src/sample.rs`](../../../../gomoku/crates/selfplay/src/sample.rs),
lines 1–44 (the module comment, imports, and the error enum). *Created in
this chapter.*

```rust
   1 | //! Replay durable game records into canonical training samples.
   2 | //!
   3 | //! This module lives in `selfplay`, rather than `train`, because replay owns
   4 | //! the `GameRecord` and opening vocabulary. The dependency already points
   5 | //! `selfplay -> train` for storage, so `train` remains the shared sample and
   6 | //! shard-format crate without learning about self-play records.

   8 | use engine::{Board, Color, Move, PlayError, Transform};
   9 | use thiserror::Error;
  10 | use train::Sample;

  12 | use crate::{GameRecord, Outcome};

  14 | /// A game record could not be converted into a training position.
  15 | #[derive(Debug, Error)]
  16 | pub enum SampleError {
  17 |     /// The requested post-opening ply does not exist.
  18 |     #[error("ply {ply} is outside the record's {ply_count} plies")]
  19 |     PlyOutOfBounds {
  20 |         /// Requested zero-based ply.
  21 |         ply: usize,
  22 |         /// Number of post-opening plies in the record.
  23 |         ply_count: usize,
  24 |     },
  25 |     /// A nonempty visit histogram contains no actual visits.
  26 |     #[error("visit histogram at ply {ply} has zero total visits")]
  27 |     ZeroVisitTotal {
  28 |         /// Zero-based post-opening ply.
  29 |         ply: usize,
  30 |     },
  31 |     /// The record's Swap2 opening was invalid.
  32 |     #[error("opening replay failed: {0}")]
  33 |     Opening(#[source] PlayError),
  34 |     /// A recorded move was rejected during replay.
  35 |     #[error("move {mv:?} at ply {ply} was rejected: {source}")]
  36 |     Replay {
  37 |         /// Zero-based post-opening ply.
  38 |         ply: usize,
  39 |         /// Rejected move.
  40 |         mv: Move,
  41 |         /// Engine legality error.
  42 |         source: PlayError,
  43 |     },
  44 | }
```

*Provenance: byte-identical to the completed reference's `src/sample.rs`
lines 1–44 (whole-file identity holds for the entire file).*

The `ZeroVisitTotal` variant deserves one sentence of motivation before its
use site in section 2: an empty histogram is the honest no-search case
(chapters 03 and 05), but a *nonempty* histogram whose counts sum to zero
is a corrupt record — chapter 03's writer never produces one. Normalizing
it would divide by zero and hand the trainer a NaN policy that surfaces
three crates away as a training divergence; the named error fails at the
read path, at the ply, with the record in hand.

## 2. `src/sample.rs`: `replay_sample`

The chapter's center. `replay_sample(record, ply)` yields the sample for
the position *immediately before* `ply` is played. The board comes from the
private `replay_board` helper (section 5); everything else derives from the
record at the same ply, in three steps.

First, the policy target π. The recorded visit histogram is summed as
`u64` (the recorded counts are smaller integers; the sum promotes before
dividing), the nonempty-but-zero corruption is rejected, and then the two
honest cases split. An empty histogram means no search ran — the TSS
bypass — and the policy is the one-hot `[(played_move, 1.0)]`: the source
comment above the branch (lines 64–66) states the reading, deliberate
behavior cloning of a machine-certified move, with the value target
unchanged. A nonempty histogram normalizes each count to a probability
mass, in the recorded order, sparse.

Second, the value target z. The side to move comes from the replayed board
— never from ply parity, because Swap2 answers decide who moved first and
the board already knows — and the match maps the outcome to {+1, 0, −1}
from that side's perspective: the side to move won, +1.0; the opponent won,
−1.0; a draw, 0.0.

Third, the assembly. The `train::Sample` stores **stone lists, not
planes**: `black` and `white` are enumerated from the board by color, in
board order (section 5's `stones`), in absolute color — the same
absolute-colors discipline the engine tutorial locked. The plane encoding
is applied later, at batch time, inside `net`'s batcher
(`planes::assemble`): `engine::encode` emits the two 17×17 border-padded
planes (the side to move's stones and the opponent's, the border ring set
in the opponent plane), and `planes::assemble` builds the four model-input
planes by adding the one-hot `my_last` and `opp_last` history planes. Two
planes are the engine's, four are the net's, and the sample owns neither —
the deferral is safe because the engine's symmetry slice proved
`encode_commutes_with_every_transform`: transforming the stones and then
encoding yields the same planes as encoding and then permuting. So
augmentation lives on stone lists, the storage format stays in engine
vocabulary, and the border is added exactly once, by the consumer that
needs it.

**Source:** [`gomoku/crates/selfplay/src/sample.rs`](../../../../gomoku/crates/selfplay/src/sample.rs),
lines 46–89. *Created in this chapter.*

```rust
  46 | /// Replay a record to the position immediately before `ply` is played.
  47 | ///
  48 | /// The returned sample is in canonical orientation. Policy and value targets
  49 | /// are derived from the recorded search and outcome at this same position.
  50 | ///
  51 | /// # Errors
  52 | /// Returns an error for an out-of-bounds ply or an illegal opening/move.
  53 | pub fn replay_sample(record: &GameRecord, ply: usize) -> Result<Sample, SampleError> {
  54 |     let board = replay_board(record, ply)?;
  55 |     let recorded = &record.plies[ply];
  56 |     let total_visits = recorded
  57 |         .visits
  58 |         .iter()
  59 |         .map(|(_, count)| u64::from(*count))
  60 |         .sum::<u64>();
  61 |     if !recorded.visits.is_empty() && total_visits == 0 {
  62 |         return Err(SampleError::ZeroVisitTotal { ply });
  63 |     }
  64 |     // A certified TSS move bypasses search, so there is no visit histogram to
  65 |     // normalize. Its deterministic one-hot policy is deliberately behavior
  66 |     // cloning for that ply; the game-outcome value target is unchanged.
  67 |     let policy = if recorded.visits.is_empty() {
  68 |         vec![(recorded.mv, 1.0)]
  69 |     } else {
  70 |         recorded
  71 |             .visits
  72 |             .iter()
  73 |             .map(|(mv, count)| (*mv, u64::from(*count) as f32 / total_visits as f32))
  74 |             .collect()
  75 |     };
  76 |     let to_move = board.to_move();
  77 |     let value = match record.outcome {
  78 |         Outcome::Draw => 0.0,
  79 |         Outcome::Won { winner, .. } if winner == to_move => 1.0,
  80 |         Outcome::Won { .. } => -1.0,
  81 |     };
  82 |     Ok(Sample {
  83 |         black: stones(&board, Color::Black),
  84 |         white: stones(&board, Color::White),
  85 |         to_move,
  86 |         policy,
  87 |         value,
  88 |     })
  89 | }
```

*Provenance: byte-identical to the completed reference's `src/sample.rs`
lines 46–89.*

Two recorded fields deliberately do **not** become targets, and the
function's silence about them is the honesty contract. The sparse `prior`
(the top-16 root priors after noise) is audit data — a selfplay-corpora
format invention for inspecting what the evaluator believed before search —
and the read path never trains on it: π comes from visits, not priors,
because visits are what search actually did. The `root_value` estimate is
likewise audit data: z comes from the game outcome, the AlphaGo Zero
discipline (Silver et al. 2017), not from the recording seat's interim
guess. Both fields travel inside the record for chapter 07's statistics and
for later analysis; neither crosses into the sample.

## 3. `src/sample.rs`: `augment_sample`

One D4 transform, applied to the geometry and nothing else. The closure
maps a stone list through `transform.transform_move`; the policy's masses
keep their values while their moves transform; `to_move` and `value` are
copied untouched. The semantics are the group structure: rotations and
reflections of the square preserve wins, losses, draws, and turn order, so
the game facts of a sample are D4-invariant and only its geometry changes.
The `#[must_use]` marks the function as pure — it returns the augmented
sample rather than mutating — and the doc comment repeats the stones-not-
planes seam: the 17×17 border is still added later by `net`.

Because the stored record stays canonical, this one function is where the
design's eight-fold multiplicity lives: one stored position becomes eight
possible training views, and the multiplicity costs nothing on disk. The
sampler (section 6) draws the transform per sample; the export path
(section 8) never calls this function at all.

**Source:** [`gomoku/crates/selfplay/src/sample.rs`](../../../../gomoku/crates/selfplay/src/sample.rs),
lines 104–127. *Created in this chapter.*

```rust
 104 | /// Apply one D4 transform to a sample's absolute stones and sparse policy.
 105 | ///
 106 | /// The side to move and outcome are invariant. This operates on stone lists,
 107 | /// not encoded planes, so the 17×17 border is still added later by `net`.
 108 | #[must_use]
 109 | pub fn augment_sample(sample: &Sample, transform: Transform) -> Sample {
 110 |     let transform_moves = |moves: &[Move]| {
 111 |         moves
 112 |             .iter()
 113 |             .map(|&mv| transform.transform_move(mv))
 114 |             .collect()
 115 |     };
 116 |     Sample {
 117 |         black: transform_moves(&sample.black),
 118 |         white: transform_moves(&sample.white),
 119 |         to_move: sample.to_move,
 120 |         policy: sample
 121 |             .policy
 122 |             .iter()
 123 |             .map(|&(mv, probability)| (transform.transform_move(mv), probability))
 124 |             .collect(),
 125 |         value: sample.value,
 126 |     }
 127 | }
```

*Provenance: byte-identical to the completed reference's `src/sample.rs`
lines 104–127.*

## 4. `src/sample.rs`: `position_is_holdout`

The partition function is three lines of composition over section 2:
replay, then apply the synthetic datasets' rule unchanged through
`train::split::is_holdout` — rebuild the board, take its Zobrist key, held
out iff `zobrist % 100 < HOLDOUT_PERCENTAGE` (= 10). The doc comment states
the load-bearing discipline: the rule is evaluated on the **canonical**
position, before any augmentation, matching `nettrain`. Zobrist keys are
orientation-specific — a rotated board hashes differently — so hashing
after a random D4 transform would hash a different orientation and could
move a position between training and holdout from draw to draw. Content,
not orientation and not position-in-file, decides the split, which is what
keeps holdout metrics comparable across the synthetic and self-play
sources. The doc also names the direct route for callers that already own a
canonical `Sample`: call `train::split::is_holdout` themselves. (That
function treats a sample whose stones cannot rebuild a legal board as held
out — the conservative default keeps corrupt data out of training.)

**Source:** [`gomoku/crates/selfplay/src/sample.rs`](../../../../gomoku/crates/selfplay/src/sample.rs),
lines 91–102. *Created in this chapter.*

```rust
  91 | /// Partition a replayed position with the synthetic dataset's holdout rule.
  92 | ///
  93 | /// This deliberately evaluates the canonical position before augmentation,
  94 | /// matching `nettrain`: a random D4 orientation must not move a position
  95 | /// between training and holdout. Callers that already own a canonical
  96 | /// [`Sample`] can call [`train::split::is_holdout`] directly.
  97 | ///
  98 | /// # Errors
  99 | /// Returns the same replay errors as [`replay_sample`].
 100 | pub fn position_is_holdout(record: &GameRecord, ply: usize) -> Result<bool, SampleError> {
 101 |     replay_sample(record, ply).map(|sample| train::split::is_holdout(&sample))
 102 | }
```

*Provenance: byte-identical to the completed reference's `src/sample.rs`
lines 91–102.*

## 5. `src/sample.rs`: the replay internals

Two private helpers close the file. `replay_board` enforces the bounds
first — a `ply` at or beyond the record's ply count is
`SampleError::PlyOutOfBounds { ply, ply_count }` — then rebuilds the
post-ceremony board from the header's Swap2 description
(`record.header.opening.board()`, chapter 02's constructor, with a
rejection surfaced as `SampleError::Opening`) and plays the first `ply`
recorded moves through it. The first illegal move fails as
`SampleError::Replay { ply, mv, source }` with the loop index as the ply.
Replay is the only position reconstruction the read path trusts: reading
re-checks legality instead of believing the file.

`stones` enumerates one color's stones in board order — row-major over the
225 valid cells, `Move::new(index / 15, index % 15)`, the `expect` stating
the range invariant — filtered through `board.stone_at`. Board order is
what makes the sample's stone lists deterministic functions of the
position, and it is the order the specification's first test pins exactly.

**Source:** [`gomoku/crates/selfplay/src/sample.rs`](../../../../gomoku/crates/selfplay/src/sample.rs),
lines 129–160 (to the end of the file). *Created in this chapter.*

```rust
 129 | fn replay_board(record: &GameRecord, ply: usize) -> Result<Board, SampleError> {
 130 |     if ply >= record.plies.len() {
 131 |         return Err(SampleError::PlyOutOfBounds {
 132 |             ply,
 133 |             ply_count: record.plies.len(),
 134 |         });
 135 |     }
 136 |     let mut board = record
 137 |         .header
 138 |         .opening
 139 |         .board()
 140 |         .map_err(SampleError::Opening)?;
 141 |     for (index, recorded) in record.plies[..ply].iter().enumerate() {
 142 |         board
 143 |             .play(recorded.mv)
 144 |             .map_err(|source| SampleError::Replay {
 145 |                 ply: index,
 146 |                 mv: recorded.mv,
 147 |                 source,
 148 |             })?;
 149 |     }
 150 |     Ok(board)
 151 | }

 153 | fn stones(board: &Board, color: Color) -> Vec<Move> {
 154 |     (0_u8..225)
 155 |         .map(|index| {
 156 |             Move::new(index / 15, index % 15).expect("0..225 contains only valid board cells")
 157 |         })
 158 |         .filter(|&mv| board.stone_at(mv) == Some(color))
 159 |         .collect()
 160 | }
```

*Provenance: byte-identical to the completed reference's `src/sample.rs`
lines 129–160.*

## 6. `src/sampler.rs`: `SamplerError` and `CorpusSampler`

The second new file turns a corpus into a training stream. `SamplerError`
has one variant — `EmptyCorpus`, when no record has a post-opening ply to
sample — and derives `PartialEq` and `Eq` alongside the error derive,
because there is no `PlayError` inside to forbid equality and the
constructor's rejection is worth comparing directly.

`CorpusSampler` holds the records, the indices of the *sampleable* games
(those with at least one post-opening ply), and one `StdRng`. The struct's
doc comment is the split-discipline warning the chapter paper quotes: the
stream yields augmented samples for training, and consumers constructing a
split dataset must use direct replay plus `position_is_holdout` *before*
augmentation, or the canonical export path — applying the Zobrist rule
after D4 would hash a different orientation. The sampler is a
training-stream artifact, not a split-dataset artifact, and the type says
so on its own doc.

**Source:** [`gomoku/crates/selfplay/src/sampler.rs`](../../../../gomoku/crates/selfplay/src/sampler.rs),
lines 1–32 (the module comment, imports, error, and struct). *Created in
this chapter.*

```rust
   1 | //! Seeded random sampling over stored games.

   3 | use std::io;
   4 | use std::path::Path;

   6 | use engine::Transform;
   7 | use rand::{RngExt, SeedableRng, rngs::StdRng};
   8 | use thiserror::Error;
   9 | use train::Sample;

  11 | use crate::{GameRecord, SampleError, augment_sample, replay_sample};

  13 | /// A corpus contains no positions that can be sampled.
  14 | #[derive(Debug, Error, PartialEq, Eq)]
  15 | pub enum SamplerError {
  16 |     /// No records were supplied, or every record has zero post-opening plies.
  17 |     #[error("a corpus sampler needs at least one game with a post-opening ply")]
  18 |     EmptyCorpus,
  19 | }

  21 | /// An infinite seeded stream of random game, ply, and D4 draws.
  22 | ///
  23 | /// Each item replays its selected record on demand; positions are not
  24 | /// materialized in advance. The stream yields augmented samples for training.
  25 | /// Consumers constructing a split dataset should instead use direct replay +
  26 | /// [`crate::position_is_holdout`] before augmentation, or the canonical export
  27 | /// path; applying the Zobrist rule after D4 would hash a different orientation.
  28 | pub struct CorpusSampler {
  29 |     records: Vec<GameRecord>,
  30 |     sampleable_games: Vec<usize>,
  31 |     rng: StdRng,
  32 | }
```

*Provenance: byte-identical to the completed reference's `src/sampler.rs`
lines 1–32 (whole-file identity holds for the entire file).*

The two constructors split I/O from logic. `from_corpus` is a thin
composition: chapter 02's `read_corpus` loads the records, `new` validates,
and an empty corpus surfaces as `io::ErrorKind::InvalidData` so the
directory-backed constructor speaks `io::Result` like the reader it wraps.
`new` collects the sampleable-game indices and rejects an empty list up
front — before any draw — with `SamplerError::EmptyCorpus`; the RNG is
seeded once, from `seed`, and lives in the struct.

**Source:** [`gomoku/crates/selfplay/src/sampler.rs`](../../../../gomoku/crates/selfplay/src/sampler.rs),
lines 34–64. *Created in this chapter.*

```rust
  34 | impl CorpusSampler {
  35 |     /// Read a corpus directory and construct its seeded sample stream.
  36 |     ///
  37 |     /// # Errors
  38 |     /// Returns corpus I/O/decoding errors, or `InvalidData` if no game has a
  39 |     /// post-opening ply to sample.
  40 |     pub fn from_corpus(dir: &Path, seed: u64) -> io::Result<Self> {
  41 |         let records = crate::read_corpus(dir)?;
  42 |         Self::new(records, seed).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
  43 |     }

  45 |     /// Construct a sampler over in-memory records.
  46 |     ///
  47 |     /// # Errors
  48 |     /// Returns [`SamplerError::EmptyCorpus`] if there are no post-opening plies.
  49 |     pub fn new(records: Vec<GameRecord>, seed: u64) -> Result<Self, SamplerError> {
  50 |         let sampleable_games: Vec<_> = records
  51 |             .iter()
  52 |             .enumerate()
  53 |             .filter_map(|(index, record)| (!record.plies.is_empty()).then_some(index))
  54 |             .collect();
  55 |         if sampleable_games.is_empty() {
  56 |             return Err(SamplerError::EmptyCorpus);
  57 |         }
  58 |         Ok(Self {
  59 |             records,
  60 |             sampleable_games,
  61 |             rng: StdRng::seed_from_u64(seed),
  62 |         })
  63 |     }
  64 | }
```

*Provenance: byte-identical to the completed reference's `src/sampler.rs`
lines 34–64.*

The `Iterator` implementation is the three-draw discipline in five lines:
each `next` draws three uniform randoms from the one seeded `StdRng` — a
game slot among the sampleable games, a ply within that game, and a
transform from `Transform::ALL` — then replays on demand and augments.
Positions are never materialized in advance; the stream's memory is the
corpus itself. Items are `Result<Sample, SampleError>`, so replay errors
surface at draw time rather than being hidden by the iterator. And the
iterator never returns `None` for a validated corpus: the stream is
infinite by construction. Determinism is the contract — same records, same
seed, same stream — which is exactly what the specification's 64-draw
equality test pins.

**Source:** [`gomoku/crates/selfplay/src/sampler.rs`](../../../../gomoku/crates/selfplay/src/sampler.rs),
lines 66–77 (to the end of the file). *Created in this chapter.*

```rust
  66 | impl Iterator for CorpusSampler {
  67 |     type Item = Result<Sample, SampleError>;

  69 |     fn next(&mut self) -> Option<Self::Item> {
  70 |         let game_slot = self.rng.random_range(0..self.sampleable_games.len());
  71 |         let game_index = self.sampleable_games[game_slot];
  72 |         let record = &self.records[game_index];
  73 |         let ply = self.rng.random_range(0..record.plies.len());
  74 |         let transform = Transform::ALL[self.rng.random_range(0..Transform::ALL.len())];
  75 |         Some(replay_sample(record, ply).map(|sample| augment_sample(&sample, transform)))
  76 |     }
  77 | }
```

*Provenance: byte-identical to the completed reference's `src/sampler.rs`
lines 66–77.*

## 7. `src/mix.rs`: `MixError`, `WeightedSource`, and `SourceMixer`

The third new file is the design spec's decision S4 made concrete: the
sampler must mix sources, and the mixer is generic over the item type so it
combines plain synthetic `Sample` iterators and fallible corpus draws
(`Result<Sample, SampleError>` items) without hiding replay errors.
`MixError` derives equality (no `PlayError` inside) and has two variants:
`Empty` (no sources supplied) and `ZeroWeight { index }` (a source that
could never be drawn). `WeightedSource<T>` boxes one iterator with its
positive relative weight and an `exhausted` flag; its constructor does not
reject zero itself — the doc says why: zero is rejected by
`SourceMixer::new`, alongside the other sources, so the configuration error
is reported with the full source list in hand.

**Source:** [`gomoku/crates/selfplay/src/mix.rs`](../../../../gomoku/crates/selfplay/src/mix.rs),
lines 1–41 (the module comment, imports, `MixError`, and `WeightedSource`).
*Created in this chapter.*

```rust
   1 | //! Deterministic weighted round-robin mixing for heterogeneous sample streams.

   3 | use rand::{RngExt, SeedableRng, rngs::StdRng};
   4 | use thiserror::Error;

   6 | /// Invalid source-mixer configuration.
   7 | #[derive(Debug, Error, PartialEq, Eq)]
   8 | pub enum MixError {
   9 |     /// No sources were supplied.
  10 |     #[error("a source mixer needs at least one source")]
  11 |     Empty,
  12 |     /// One source had zero weight and could never be drawn.
  13 |     #[error("source {index} has zero weight")]
  14 |     ZeroWeight {
  15 |         /// Zero-based source index.
  16 |         index: usize,
  17 |     },
  18 | }

  20 | /// One iterator and its relative round-robin weight.
  21 | ///
  22 | /// The item type is generic, so a mixer can combine plain synthetic `Sample`
  23 | /// iterators and fallible corpus draws without hiding replay errors.
  24 | pub struct WeightedSource<T> {
  25 |     weight: u32,
  26 |     samples: Box<dyn Iterator<Item = T>>,
  27 |     exhausted: bool,
  28 | }

  30 | impl<T> WeightedSource<T> {
  31 |     /// Box a sample iterator with a positive relative weight.
  32 |     ///
  33 |     /// Zero is rejected by [`SourceMixer::new`], alongside the other sources.
  34 |     pub fn new(weight: u32, samples: impl Iterator<Item = T> + 'static) -> Self {
  35 |         Self {
  36 |             weight,
  37 |             samples: Box::new(samples),
  38 |             exhausted: false,
  39 |         }
  40 |     }
  41 | }
```

*Provenance: byte-identical to the completed reference's `src/mix.rs`
lines 1–41 (whole-file identity holds for the entire file).*

`SourceMixer<T>` holds the sources, a cursor, the count emitted in the
current turn, and the count of still-active sources. The constructor
validates — empty list, then the first zero weight by position — and then
takes its one piece of scheduling randomness: the seed picks the *initial*
source, a uniform draw from a freshly seeded `StdRng` that is then
discarded. That is the only dice the mixer rolls; the doc comment says why:
seeding the initial source makes the stream deterministic without
introducing a second random scheduling policy. `advance` is the cursor
step: modulo the source count, with the per-turn emission counter reset.

**Source:** [`gomoku/crates/selfplay/src/mix.rs`](../../../../gomoku/crates/selfplay/src/mix.rs),
lines 43–83. *Created in this chapter.*

```rust
  43 | /// Seeded weighted round-robin over any number of sample iterators.
  44 | ///
  45 | /// Each source contributes `weight` consecutive items per turn. The seed picks
  46 | /// the initial source, making the stream deterministic without introducing a
  47 | /// second random scheduling policy. Exhausted finite sources are skipped; the
  48 | /// mixer ends after every source is exhausted. Infinite sources such as
  49 | /// [`crate::CorpusSampler`] make the mixed stream infinite.
  50 | pub struct SourceMixer<T> {
  51 |     sources: Vec<WeightedSource<T>>,
  52 |     cursor: usize,
  53 |     emitted_in_turn: u32,
  54 |     active: usize,
  55 | }

  57 | impl<T> SourceMixer<T> {
  58 |     /// Build a mixer and seed its initial source.
  59 |     ///
  60 |     /// # Errors
  61 |     /// Rejects an empty source list or any zero-weight source.
  62 |     pub fn new(sources: Vec<WeightedSource<T>>, seed: u64) -> Result<Self, MixError> {
  63 |         if sources.is_empty() {
  64 |             return Err(MixError::Empty);
  65 |         }
  66 |         if let Some(source) = sources.iter().position(|source| source.weight == 0) {
  67 |             return Err(MixError::ZeroWeight { index: source });
  68 |         }
  69 |         let active = sources.len();
  70 |         let cursor = StdRng::seed_from_u64(seed).random_range(0..active);
  71 |         Ok(Self {
  72 |             sources,
  73 |             cursor,
  74 |             emitted_in_turn: 0,
  75 |             active,
  76 |         })
  77 |     }

  79 |     fn advance(&mut self) {
  80 |         self.cursor = (self.cursor + 1) % self.sources.len();
  81 |         self.emitted_in_turn = 0;
  82 |     }
  83 | }
```

*Provenance: byte-identical to the completed reference's `src/mix.rs`
lines 43–83.*

The `Iterator` implementation is the weighted round-robin discipline. The
loop ends when every source is exhausted (`active == 0`). Otherwise the
current source is polled: an item increments the per-turn count, and when
the count reaches the source's weight the cursor advances — each source
contributes exactly `weight` consecutive items per turn. An exhausted
finite source is flagged once (decrementing `active`) and skipped from then
on. Because the schedule is deterministic, the mix matches the weights
exactly over a full cycle — the specification's 1 000 / 3 000 split over
4 000 draws at weights 1 and 3 is an equality, not a confidence interval.
With an infinite source such as `CorpusSampler` in the mix, `active` never
reaches zero and the mixed stream is infinite.

**Source:** [`gomoku/crates/selfplay/src/mix.rs`](../../../../gomoku/crates/selfplay/src/mix.rs),
lines 85–112 (to the end of the file). *Created in this chapter.*

```rust
  85 | impl<T> Iterator for SourceMixer<T> {
  86 |     type Item = T;

  88 |     fn next(&mut self) -> Option<Self::Item> {
  89 |         loop {
  90 |             if self.active == 0 {
  91 |                 return None;
  92 |             }
  93 |             let item = self.sources[self.cursor].samples.next();
  94 |             match item {
  95 |                 Some(item) => {
  96 |                     self.emitted_in_turn += 1;
  97 |                     if self.emitted_in_turn == self.sources[self.cursor].weight {
  98 |                         self.advance();
  99 |                     }
 100 |                     return Some(item);
 101 |                 }
 102 |                 None => {
 103 |                     if !self.sources[self.cursor].exhausted {
 104 |                         self.sources[self.cursor].exhausted = true;
 105 |                         self.active -= 1;
 106 |                     }
 107 |                     self.advance();
 108 |                 }
 109 |             }
 110 |         }
 111 |     }
 112 | }
```

*Provenance: byte-identical to the completed reference's `src/mix.rs`
lines 85–112.*

## 8. `src/export.rs`: the manifest, the errors, and `export_corpus`

The fourth new file is the batch alternative to streaming. The module
comment is the seam in two sentences: `nettrain --data` reads
length-delimited sample shards and ignores manifest details, so no network
or training changes; and exported positions stay canonical, so `nettrain`
computes the Zobrist holdout split before its existing seeded per-sample
batch-time D4 transform. `SAMPLE_EXPORT_FORMAT_VERSION` is the manifest
layout version (1). `ExportManifest` derives `Debug`, `Clone`, `PartialEq`,
`Eq`, and serde's two derives — equality because the tests compare
manifests, serde because the export writes it as JSON — and carries the
five provenance fields: the format version, the source game count, the
sample count (one per post-opening ply), the shard file names in sample
order, and the workspace package version shared with the `train` reader so
format drift is visible.

`ExportError` has four variants in the order the checks fire:
`EmptyCorpus`, `ZeroShardCapacity`, `Sample { game_index, ply, source }`
(one source position failed replay, wrapping section 1's `SampleError` with
its corpus coordinates), and `Io` (a transparent filesystem, framing,
serialization, or manifest failure).

**Source:** [`gomoku/crates/selfplay/src/export.rs`](../../../../gomoku/crates/selfplay/src/export.rs),
lines 1–58 (the module comment, imports, version constant, manifest, and
error enum). *Created in this chapter.*

```rust
   1 | //! Export game records as an ordinary `train::Sample` dataset.
   2 | //!
   3 | //! `nettrain --data` reads length-delimited sample shards and ignores manifest
   4 | //! details, so this is the integration seam: no network or training changes.
   5 | //! Exported positions stay canonical. `nettrain` consequently computes the
   6 | //! Zobrist holdout split before its existing seeded batch-time D4 transform.

   8 | use std::fs::{self, File};
   9 | use std::io;
  10 | use std::path::Path;

  12 | use serde::{Deserialize, Serialize};
  13 | use thiserror::Error;

  15 | use crate::{GameRecord, SampleError, replay_sample};

  17 | /// Current sample-export manifest layout version.
  18 | pub const SAMPLE_EXPORT_FORMAT_VERSION: u16 = 1;

  20 | /// Provenance and shard inventory for an exported sample dataset.
  21 | #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
  22 | pub struct ExportManifest {
  23 |     /// Sample-export manifest layout version.
  24 |     pub format_version: u16,
  25 |     /// Number of source games.
  26 |     pub game_count: usize,
  27 |     /// Number of replayed samples; one per post-opening ply.
  28 |     pub sample_count: usize,
  29 |     /// Shard file names in sample order.
  30 |     pub shards: Vec<String>,
  31 |     /// Workspace package version shared with the `train` reader.
  32 |     pub workspace_version: String,
  33 | }

  35 | /// A corpus could not be exported as a sample dataset.
  36 | #[derive(Debug, Error)]
  37 | pub enum ExportError {
  38 |     /// No source records were supplied.
  39 |     #[error("cannot export an empty corpus")]
  40 |     EmptyCorpus,
  41 |     /// Shard capacity must be positive.
  42 |     #[error("shard capacity must be greater than zero")]
  43 |     ZeroShardCapacity,
  44 |     /// One source position failed replay or target derivation.
  45 |     #[error("game {game_index} ply {ply}: {source}")]
  46 |     Sample {
  47 |         /// Zero-based source game index.
  48 |         game_index: usize,
  49 |         /// Zero-based post-opening ply.
  50 |         ply: usize,
  51 |         /// Replay or target error.
  52 |         #[source]
  53 |         source: SampleError,
  54 |     },
  55 |     /// Filesystem, framing, serialization, or manifest I/O failed.
  56 |     #[error(transparent)]
  57 |     Io(#[from] io::Error),
  58 | }
```

*Provenance: byte-identical to the completed reference's `src/export.rs`
lines 1–58 (whole-file identity holds for the entire file).*

`export_corpus(records, out_dir, shard_capacity)` enforces its contract in
a fixed order, and the order is the design. Validation comes first: an
empty record list and a zero capacity fail before anything is computed.
Then the full replay pass — every position in the corpus, game by game, ply
by ply, in corpus order — and only after every position has replayed does
the function touch the output: a replay failure is `ExportError::Sample`
and no partial dataset exists. Then the overwrite discipline: a non-empty
output directory is refused outright with an `AlreadyExists` I/O error
("refusing to export samples into a non-empty directory"), mirroring the
corpus writer's rule that exports never overwrite by accident; a missing
directory is the expected case and is created. The samples are chunked by
`shard_capacity` into `shard-000.bin`, `shard-001.bin`, … through
`train::storage::write_shard` — the same length-delimited bincode framing
the synthetic datasets use — and the manifest is written pretty-printed
with create-new semantics (a manifest that somehow exists already is an
error, never an overwrite). The returned manifest is the same value the
file holds.

Note what is *not* exported: no D4 variants, no holdout labels. The shards
stay canonical, `nettrain` computes the Zobrist split on load, and its
batcher applies its own seeded D4 transform per sample within each batch.
Export order is fully determined by the corpus and the framing is fixed, so
the same corpus exports the same bytes — the determinism chapter 07's
byte-identity gates rely on.

**Source:** [`gomoku/crates/selfplay/src/export.rs`](../../../../gomoku/crates/selfplay/src/export.rs),
lines 60–130 (to the end of the file). *Created in this chapter.*

```rust
  60 | /// Export one canonical sample per post-opening ply.
  61 | ///
  62 | /// Shards use exactly the same bincode framing and `Sample` serialization as
  63 | /// [`train::dataset::read_dataset`]. The manifest carries game-export
  64 | /// provenance rather than synthetic tactical quotas. Any non-empty output
  65 | /// directory is refused before files are changed.
  66 | ///
  67 | /// # Errors
  68 | /// Returns an error for invalid input, replay failures, or output I/O. Replay
  69 | /// validation finishes before the output directory is modified.
  70 | pub fn export_corpus(
  71 |     records: &[GameRecord],
  72 |     out_dir: &Path,
  73 |     shard_capacity: usize,
  74 | ) -> Result<ExportManifest, ExportError> {
  75 |     if records.is_empty() {
  76 |         return Err(ExportError::EmptyCorpus);
  77 |     }
  78 |     if shard_capacity == 0 {
  79 |         return Err(ExportError::ZeroShardCapacity);
  80 |     }

  82 |     let mut samples = Vec::new();
  83 |     for (game_index, record) in records.iter().enumerate() {
  84 |         for ply in 0..record.plies.len() {
  85 |             samples.push(
  86 |                 replay_sample(record, ply).map_err(|source| ExportError::Sample {
  87 |                     game_index,
  88 |                     ply,
  89 |                     source,
  90 |                 })?,
  91 |             );
  92 |         }
  93 |     }

  95 |     match fs::read_dir(out_dir) {
  96 |         Ok(mut entries) => {
  97 |             if entries.next().transpose()?.is_some() {
  98 |                 return Err(io::Error::new(
  99 |                     io::ErrorKind::AlreadyExists,
 100 |                     "refusing to export samples into a non-empty directory",
 101 |                 )
 102 |                 .into());
 103 |             }
 104 |         }
 105 |         Err(error) if error.kind() == io::ErrorKind::NotFound => {}
 106 |         Err(error) => return Err(error.into()),
 107 |     }
 108 |     fs::create_dir_all(out_dir)?;

 110 |     let mut shards = Vec::new();
 111 |     for (shard_index, chunk) in samples.chunks(shard_capacity).enumerate() {
 112 |         let name = format!("shard-{shard_index:03}.bin");
 113 |         train::storage::write_shard(chunk, &out_dir.join(&name))?;
 114 |         shards.push(name);
 115 |     }
 116 |     let manifest = ExportManifest {
 117 |         format_version: SAMPLE_EXPORT_FORMAT_VERSION,
 118 |         game_count: records.len(),
 119 |         sample_count: samples.len(),
 120 |         shards,
 121 |         workspace_version: env!("CARGO_PKG_VERSION").to_owned(),
 122 |     };
 123 |     let manifest_file = File::options()
 124 |         .write(true)
 125 |         .create_new(true)
 126 |         .open(out_dir.join("manifest.json"))?;
 127 |     serde_json::to_writer_pretty(manifest_file, &manifest)
 128 |         .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
 129 |     Ok(manifest)
 130 | }
```

*Provenance: byte-identical to the completed reference's `src/export.rs`
lines 60–130.*

One quotation caveat the chapter paper also carries: its small manifest
example uses compact arrays for readability and pins the semantic schema,
not `serde_json`'s pretty output — `to_writer_pretty` expands the `shards`
array one element per line. §12 quotes the real pretty-printed manifest
from this paper's capstone run, byte-exact as written.

## 9. `src/bin/selfplayexport.rs`: the export driver

The fifth new file is a thin driver with hand-rolled argument parsing — no
dependency — in the style of `selfplaygen` and `selfplayanchors`. The
`USAGE` text is the exact five-line string the chapter paper pins, and
`main` implements the three exit paths: `--help` or `-h` anywhere prints
the usage and exits successfully; a parse failure prints the message and
the usage and exits with code 2; a run failure prints
`selfplayexport failed: …` and exits with code 1.

**Source:** [`gomoku/crates/selfplay/src/bin/selfplayexport.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayexport.rs),
lines 1–37 (the module comment, imports, usage text, `Args`, and `main`).
*Created in this chapter.*

```rust
   1 | //! Convert a self-play game corpus into a `nettrain --data` sample dataset.

   3 | use std::env;
   4 | use std::path::PathBuf;
   5 | use std::process;

   7 | use selfplay::{export_corpus, read_corpus};

   9 | const USAGE: &str = "Usage: selfplayexport --corpus <DIR> --out <DIR> [--shard-capacity <usize>]\n\
  10 | Default: --shard-capacity 4096\n\
  11 | Writes one canonical train::Sample per post-opening ply plus manifest.json.\n\
  12 | nettrain --data <DIR> reads the shards directly and applies seeded D4 at batch time.\n\
  13 | Use `--flag value` only; `--flag=value` is not supported.";

  15 | struct Args {
  16 |     corpus: PathBuf,
  17 |     out: PathBuf,
  18 |     shard_capacity: usize,
  19 | }

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
  34 |         eprintln!("selfplayexport failed: {error}");
  35 |         process::exit(1);
  36 |     }
  37 | }
```

*Provenance: byte-identical to the completed reference's
`src/bin/selfplayexport.rs` lines 1–37 (whole-file identity holds for the
entire file).*

The run path is four steps, and the fourth is the design's self-audit:
`read_corpus` loads the records (a read failure reports `failed to read
corpus: …`); `export_corpus` writes the dataset (a failure reports `failed
to export samples: …`); the binary then reads its own export back through
`train::dataset::read_dataset` — the very reader `nettrain` uses — and
fails if the read-back count differs from the manifest's `sample_count`.
The exporter verifies its own output through the consumer's reader before
reporting success, so the one stderr line —
`Exported 1860 samples from 48 games into 4 shards at <DIR>` — is a
checked claim, not a hope.

**Source:** [`gomoku/crates/selfplay/src/bin/selfplayexport.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayexport.rs),
lines 39–61 (`run`). *Created in this chapter.*

```rust
  39 | fn run(args: Args) -> Result<(), String> {
  40 |     let records =
  41 |         read_corpus(&args.corpus).map_err(|error| format!("failed to read corpus: {error}"))?;
  42 |     let manifest = export_corpus(&records, &args.out, args.shard_capacity)
  43 |         .map_err(|error| format!("failed to export samples: {error}"))?;
  44 |     let read_back = train::dataset::read_dataset(&args.out)
  45 |         .map_err(|error| format!("failed to read back exported samples: {error}"))?;
  46 |     if read_back.len() != manifest.sample_count {
  47 |         return Err(format!(
  48 |             "read-back sample count {} differs from manifest {}",
  49 |             read_back.len(),
  50 |             manifest.sample_count
  51 |         ));
  52 |     }
  53 |     eprintln!(
  54 |         "Exported {} samples from {} games into {} shards at {}",
  55 |         manifest.sample_count,
  56 |         manifest.game_count,
  57 |         manifest.shards.len(),
  58 |         args.out.display()
  59 |     );
  60 |     Ok(())
  61 | }
```

*Provenance: byte-identical to the completed reference's
`src/bin/selfplayexport.rs` lines 39–61.*

The parser holds the three flags' state — both directories optional until
the final check, the capacity defaulted to 4096 — and walks the argument
iterator one token at a time. Values are separate arguments only:
`--flag=value` is not supported and falls into the `unknown flag` arm. A
missing value is a named error per flag; a non-numeric capacity reports the
parse error; a zero capacity is rejected after the loop; and the two
required flags fail with `--corpus is required` / `--out is required` only
after every token is consumed, so `selfplayexport --out o --corpus c` in
any order parses.

**Source:** [`gomoku/crates/selfplay/src/bin/selfplayexport.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayexport.rs),
lines 63–100 (to the end of the file). *Created in this chapter.*

```rust
  63 | fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
  64 |     let mut corpus = None;
  65 |     let mut out = None;
  66 |     let mut shard_capacity = 4096;
  67 |     while let Some(arg) = args.next() {
  68 |         match arg.as_str() {
  69 |             "--corpus" => {
  70 |                 corpus = Some(PathBuf::from(
  71 |                     args.next()
  72 |                         .ok_or_else(|| "--corpus requires a value".to_owned())?,
  73 |                 ));
  74 |             }
  75 |             "--out" => {
  76 |                 out = Some(PathBuf::from(
  77 |                     args.next()
  78 |                         .ok_or_else(|| "--out requires a value".to_owned())?,
  79 |                 ));
  80 |             }
  81 |             "--shard-capacity" => {
  82 |                 let value = args
  83 |                     .next()
  84 |                     .ok_or_else(|| "--shard-capacity requires a value".to_owned())?;
  85 |                 shard_capacity = value
  86 |                     .parse()
  87 |                     .map_err(|error| format!("invalid --shard-capacity '{value}': {error}"))?;
  88 |             }
  89 |             other => return Err(format!("unknown flag: {other}")),
  90 |         }
  91 |     }
  92 |     if shard_capacity == 0 {
  93 |         return Err("--shard-capacity must be greater than zero".to_owned());
  94 |     }
  95 |     Ok(Args {
  96 |         corpus: corpus.ok_or_else(|| "--corpus is required".to_owned())?,
  97 |         out: out.ok_or_else(|| "--out is required".to_owned())?,
  98 |         shard_capacity,
  99 |     })
 100 | }
```

*Provenance: byte-identical to the completed reference's
`src/bin/selfplayexport.rs` lines 63–100.*

## 10. `src/lib.rs`: the chapter-06 public surface (chapter-6 slice form)

The crate root changes by exactly eight lines: four module declarations in
alphabetical position (`export` after `corpus`, `mix` after `generate`,
`sample` and `sampler` between `record` and `schedule`) and four re-export
lines in their modules' alphabetical group positions. Nothing from chapters
01–05 changes shape. The quoted file is the **chapter-6 slice form**: it is
the chapter-5 surface plus this chapter's additions, with no
chapter-07 content — and chapter 07 never adds any (its verifier is a
binary and four test functions), which is why the scripted whole-file
comparison found this slice form byte-identical to the completed
reference's `src/lib.rs`.

**Source:** [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs),
lines 1–42 (the whole file, chapter-6 slice form). *Touched in this
chapter.*

```rust
   1 | //! Synchronous, single-threaded baseline self-play.
   2 | //!
   3 | //! One seeded game runs a complete Swap2 ceremony and then alternates two
   4 | //! persistent baseline players. The result is entirely in memory: opening
   5 | //! provenance, per-seat options, ordered plies, search metadata, and outcome.

   7 | #![deny(missing_docs)]

   9 | mod anchor;
  10 | mod corpus;
  11 | mod export;
  12 | mod game;
  13 | mod generate;
  14 | mod mix;
  15 | mod opening;
  16 | mod record;
  17 | mod sample;
  18 | mod sampler;
  19 | mod schedule;

  21 | pub use anchor::{AnchorError, AnchorSeed, extract_anchor_seeds};
  22 | pub use corpus::{
  23 |     CorpusWriteOptions, Manifest, OverwritePolicy, read_corpus, read_manifest, write_corpus,
  24 |     write_corpus_with_options,
  25 | };
  26 | pub use export::{ExportError, ExportManifest, SAMPLE_EXPORT_FORMAT_VERSION, export_corpus};
  27 | pub use game::{
  28 |     Game, GameError, Outcome, PlyRecord, PlySearch, WinReason, play_game, play_game_with_opening,
  29 | };
  30 | pub use generate::{GenerateError, generate_games, generate_games_with};
  31 | pub use mix::{MixError, SourceMixer, WeightedSource};
  32 | pub use opening::{
  33 |     GeneratedOpening, Opening, OpeningAnswer, OpeningLoadError, OpeningParseError,
  34 |     OpeningProvenance, OpeningSource, TournamentOpenings,
  35 | };
  36 | pub use record::{
  37 |     CertifiedSuffix, GAME_RECORD_FORMAT_VERSION, GameHeader, GameRecord, GuardSettings,
  38 |     RECORDED_PRIOR_TOP_K, RecordedPly, SeatConfig, SeatPairing, TssProbeSettings,
  39 | };
  40 | pub use sample::{SampleError, augment_sample, position_is_holdout, replay_sample};
  41 | pub use sampler::{CorpusSampler, SamplerError};
  42 | pub use schedule::{EXPLORATION_PLIES, ExplorationSchedule};
```

*Provenance: chapter-6 slice form — the chapter-5 `src/lib.rs` plus the
four module declarations at lines 11, 14, 17, 18 and the four re-export
lines at 26, 31, 40, 41; no chapter-07 content exists in the completed
reference's crate root to exclude, and the scripted whole-file comparison
(`14-provenance-diff.log`) confirms this slice form is byte-identical to
the completed reference's `src/lib.rs`. The slice's tests passed with
exactly this file compiled in.*

## 11. `tests/read_path.rs`: the read-path specification

The chapter's test target, quoted in full across this section. All eleven
tests share one fabricated record — no game is played — and the helpers
come first: `TestDir`, a self-cleaning temporary directory keyed by process
id and clock nanos with a `Drop` that removes it; the `mv` coordinate
helper; and `six_ply_record`, the fixture the chapter paper specifies: a
Swap2 opening placing `(5, 5)`, `(10, 10)`, `(5, 6)` with the answer "take
Black" and generated provenance; six post-opening plies — `(10, 11)`,
`(5, 7)`, `(10, 12)`, `(5, 8)`, `(10, 13)`, `(5, 9)` — on which White
builds a horizontal line on row 10 and Black on row 5; a Black win by
five-or-more; every searched ply carrying a one-entry visit histogram on
the played move, a prior of 1.0, and a root value of 0.25; and ply 5
unsearched (`(ply != 5).then(...)` leaves `search: None`), standing in for
a TSS-bypassed certified move. `GameRecord::from_game` converts the
fixture, so the tests exercise the same conversion the runner uses.

**Source:** [`gomoku/crates/selfplay/tests/read_path.rs`](../../../../gomoku/crates/selfplay/tests/read_path.rs),
lines 1–82 (imports, helpers, and the fixture). *Created in this chapter.*

```rust
   1 | use std::path::{Path, PathBuf};
   2 | use std::process::Command;
   3 | use std::time::{SystemTime, UNIX_EPOCH};

   5 | use engine::{Color, Move, Transform};
   6 | use player::PlayerOptions;
   7 | use selfplay::{
   8 |     CorpusSampler, ExportManifest, Game, GameRecord, Opening, OpeningAnswer, OpeningProvenance,
   9 |     Outcome, PlyRecord, PlySearch, SampleError, SourceMixer, WeightedSource, WinReason,
  10 |     augment_sample, position_is_holdout, replay_sample, write_corpus,
  11 | };

  13 | struct TestDir(PathBuf);

  15 | impl TestDir {
  16 |     fn new() -> Self {
  17 |         let nanos = SystemTime::now()
  18 |             .duration_since(UNIX_EPOCH)
  19 |             .expect("system clock is after the Unix epoch")
  20 |             .as_nanos();
  21 |         Self(
  22 |             std::env::temp_dir().join(format!("selfplay-read-path-{}-{nanos}", std::process::id())),
  23 |         )
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

  41 | fn six_ply_record() -> GameRecord {
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
  61 |             .enumerate()
  62 |             .map(|(ply, played)| PlyRecord {
  63 |                 mv: played,
  64 |                 search: (ply != 5).then(|| PlySearch {
  65 |                     side_to_move: if ply % 2 == 0 {
  66 |                         Color::White
  67 |                     } else {
  68 |                         Color::Black
  69 |                     },
  70 |                     visits: vec![(played, 1)],
  71 |                     prior: vec![(played, 1.0)],
  72 |                     root_value: 0.25,
  73 |                 }),
  74 |             })
  75 |             .collect(),
  76 |         outcome: Outcome::Won {
  77 |             winner: Color::Black,
  78 |             reason: WinReason::FiveOrMore,
  79 |         },
  80 |         certified_suffix: None,
  81 |     })
  82 | }
```

*Provenance: byte-identical to the completed reference's
`tests/read_path.rs` lines 1–82 (whole-file identity holds for the entire
file).*

The replay tests pin the position and its side to move first: replaying to
ply 3 yields exactly the opening stones plus the played ones — Black
`[(5, 5), (5, 6), (5, 7)]` in board order, White `[(10, 10), (10, 11),
(10, 12)]` — with Black to move. Then the policy contract in three tests:
a replaced three-entry histogram `[(10, 11) → 3, (0, 0) → 1, (14, 14) →
2]` normalizes to `[0.5, 1/6, 1/3]` in the recorded order with masses
summing to 1 within 1e-6; the unsearched ply 5 yields the one-hot
`[(played_move, 1.0)]`; and a nonempty all-zero histogram is
`SampleError::ZeroVisitTotal { ply: 0 }`, never a NaN. The value test pins
the side-to-move perspective: at ply 0 White is to move against a Black win
(−1.0), at ply 1 Black is to move (+1.0), and a draw zeros both. The
fragment then closes with the augmentation test — one D4 transform applied
to the stones and the policy together — and the holdout test, which pins
the canonical position's Zobrist key as the partition input; both are
discussed after the quote.

**Source:** [`gomoku/crates/selfplay/tests/read_path.rs`](../../../../gomoku/crates/selfplay/tests/read_path.rs),
lines 84–188 (the position, policy, value, augmentation, and
holdout tests — seven in all). *Created in this chapter.*

```rust
  84 | #[test]
  85 | fn replay_at_ply_has_opening_and_played_stones_with_the_right_side_to_move() {
  86 |     let sample = replay_sample(&six_ply_record(), 3).expect("fabricated record replays");

  88 |     assert_eq!(sample.black, [mv(5, 5), mv(5, 6), mv(5, 7)]);
  89 |     assert_eq!(sample.white, [mv(10, 10), mv(10, 11), mv(10, 12)]);
  90 |     assert_eq!(sample.to_move, Color::Black);
  91 | }

  93 | #[test]
  94 | fn raw_visits_become_a_sparse_normalized_policy() {
  95 |     let mut record = six_ply_record();
  96 |     record.plies[0].visits = vec![(mv(10, 11), 3), (mv(0, 0), 1), (mv(14, 14), 2)];

  98 |     let sample = replay_sample(&record, 0).expect("fabricated record replays");

 100 |     assert_eq!(
 101 |         sample.policy,
 102 |         vec![
 103 |             (mv(10, 11), 0.5),
 104 |             (mv(0, 0), 1.0 / 6.0),
 105 |             (mv(14, 14), 1.0 / 3.0)
 106 |         ]
 107 |     );
 108 |     let mass: f32 = sample
 109 |         .policy
 110 |         .iter()
 111 |         .map(|(_, probability)| probability)
 112 |         .sum();
 113 |     assert!((mass - 1.0).abs() < 1e-6, "policy mass was {mass}");
 114 | }

 116 | #[test]
 117 | fn a_ply_without_search_uses_the_played_move_as_its_policy() {
 118 |     let record = six_ply_record();
 119 |     assert!(record.plies[5].visits.is_empty());

 121 |     let sample = replay_sample(&record, 5).expect("TSS-bypassed ply replays");

 123 |     assert_eq!(sample.policy, [(record.plies[5].mv, 1.0)]);
 124 | }

 126 | #[test]
 127 | fn outcome_value_uses_each_position_side_to_move_perspective() {
 128 |     let mut record = six_ply_record();

 130 |     assert_eq!(replay_sample(&record, 0).unwrap().to_move, Color::White);
 131 |     assert_eq!(replay_sample(&record, 0).unwrap().value, -1.0);
 132 |     assert_eq!(replay_sample(&record, 1).unwrap().to_move, Color::Black);
 133 |     assert_eq!(replay_sample(&record, 1).unwrap().value, 1.0);

 135 |     record.outcome = Outcome::Draw;
 136 |     assert_eq!(replay_sample(&record, 0).unwrap().value, 0.0);
 137 |     assert_eq!(replay_sample(&record, 1).unwrap().value, 0.0);
 138 | }

 140 | #[test]
 141 | fn d4_augmentation_transforms_stones_and_policy_together() {
 142 |     let mut record = six_ply_record();
 143 |     record.plies[0].visits = vec![(mv(0, 0), 3), (mv(10, 11), 1)];
 144 |     let canonical = replay_sample(&record, 0).unwrap();
 145 |     let transform = Transform::Rot90;

 147 |     let augmented = augment_sample(&canonical, transform);

 149 |     let expected = |moves: &[Move]| -> Vec<Move> {
 150 |         moves
 151 |             .iter()
 152 |             .map(|&mv| transform.transform_move(mv))
 153 |             .collect()
 154 |     };
 155 |     assert_eq!(augmented.black, expected(&canonical.black));
 156 |     assert_eq!(augmented.white, expected(&canonical.white));
 157 |     assert_eq!(
 158 |         augmented.policy,
 159 |         [
 160 |             (transform.transform_move(mv(0, 0)), 0.75),
 161 |             (transform.transform_move(mv(10, 11)), 0.25),
 162 |         ]
 163 |     );
 164 |     assert_eq!(augmented.policy[0].0, mv(14, 0));
 165 |     assert_eq!(augmented.to_move, canonical.to_move);
 166 |     assert_eq!(augmented.value, canonical.value);
 167 | }

 169 | #[test]
 170 | fn holdout_uses_the_canonical_position_zobrist_and_train_partition() {
 171 |     let record = six_ply_record();
 172 |     let sample = replay_sample(&record, 2).unwrap();
 173 |     let expected = sample.board().unwrap().zobrist() % 100 < train::split::HOLDOUT_PERCENTAGE;

 175 |     assert_eq!(train::split::is_holdout(&sample), expected);
 176 |     assert_eq!(position_is_holdout(&record, 2).unwrap(), expected);
 177 | }

 179 | #[test]
 180 | fn nonempty_zero_visit_histogram_is_rejected_instead_of_producing_nan() {
 181 |     let mut record = six_ply_record();
 182 |     record.plies[0].visits = vec![(mv(0, 0), 0), (mv(1, 1), 0)];

 184 |     assert!(matches!(
 185 |         replay_sample(&record, 0),
 186 |         Err(SampleError::ZeroVisitTotal { ply: 0 })
 187 |     ));
 188 | }
```

*Provenance: byte-identical to the completed reference's
`tests/read_path.rs` lines 84–188.*

The D4 test deserves one comment: it computes its expectations through the
same `transform_move` the implementation calls, then adds two anchors that
make the test more than a tautology — the policy masses are pinned to
`0.75` and `0.25` literally, and `Rot90(0, 0)` is pinned to `(14, 0)`
literally, so a wrong rotation direction fails the test even though the
structural comparison would pass. The holdout test computes the expected
partition by hand — `zobrist % 100 < HOLDOUT_PERCENTAGE` on the replayed
board — and asserts that both `train::split::is_holdout` and
`position_is_holdout` agree with it: the selfplay read path uses the
synthetic datasets' content-based rule unchanged, on the canonical
position.

The stream tests pin determinism and exactness. The sampler test builds two
`CorpusSampler`s over the same one-record vector with seed 42 and asserts
the 64-draw streams are equal and every drawn policy is a distribution —
augmentation never breaks the policy invariant, because `augment_sample`
moves masses without touching values. The directory test round-trips
through chapter 02's `write_corpus` (shard capacity 2) and
`CorpusSampler::from_corpus`. The mixer test is the exactness proof: two
infinite marker sources at weights 1 and 3, seed 7, 4 000 draws, exactly
1 000 and 3 000 — an equality, because the round-robin schedule meets the
weights per cycle.

**Source:** [`gomoku/crates/selfplay/tests/read_path.rs`](../../../../gomoku/crates/selfplay/tests/read_path.rs),
lines 190–257 (the two sampler tests and the mixer test). *Created in this
chapter.*

```rust
 190 | #[test]
 191 | fn corpus_sampler_is_deterministic_under_one_seed() {
 192 |     let records = vec![six_ply_record()];
 193 |     let first = CorpusSampler::new(records.clone(), 42).expect("nonempty corpus");
 194 |     let second = CorpusSampler::new(records, 42).expect("nonempty corpus");

 196 |     let first: Vec<_> = first
 197 |         .take(64)
 198 |         .collect::<Result<Vec<_>, _>>()
 199 |         .expect("all fabricated draws replay");
 200 |     let second: Vec<_> = second
 201 |         .take(64)
 202 |         .collect::<Result<Vec<_>, _>>()
 203 |         .expect("all fabricated draws replay");

 205 |     assert_eq!(first, second);
 206 |     assert!(first.iter().all(|sample| {
 207 |         let mass: f32 = sample
 208 |             .policy
 209 |             .iter()
 210 |             .map(|(_, probability)| probability)
 211 |             .sum();
 212 |         (mass - 1.0).abs() < 1e-6
 213 |     }));
 214 | }

 216 | #[test]
 217 | fn corpus_sampler_loads_a_corpus_directory() {
 218 |     let root = TestDir::new();
 219 |     write_corpus(&[six_ply_record()], root.path(), 2).expect("fabricated corpus writes");

 221 |     let sampler = CorpusSampler::from_corpus(root.path(), 42).expect("directory corpus loads");
 222 |     let samples: Vec<_> = sampler
 223 |         .take(4)
 224 |         .collect::<Result<Vec<_>, _>>()
 225 |         .expect("directory-backed draws replay");

 227 |     assert_eq!(samples.len(), 4);
 228 | }

 230 | #[test]
 231 | fn source_mixer_respects_weights_over_a_large_seeded_draw() {
 232 |     let marker = |value| train::Sample {
 233 |         black: Vec::new(),
 234 |         white: Vec::new(),
 235 |         to_move: Color::Black,
 236 |         policy: vec![(mv(0, 0), 1.0)],
 237 |         value,
 238 |     };
 239 |     let mut mixer = SourceMixer::new(
 240 |         vec![
 241 |             WeightedSource::new(1, std::iter::repeat(marker(-1.0))),
 242 |             WeightedSource::new(3, std::iter::repeat(marker(1.0))),
 243 |         ],
 244 |         7,
 245 |     )
 246 |     .expect("nonzero source weights");

 248 |     let draws: Vec<_> = mixer.by_ref().take(4_000).collect();
 249 |     assert_eq!(
 250 |         draws.iter().filter(|sample| sample.value == -1.0).count(),
 251 |         1_000
 252 |     );
 253 |     assert_eq!(
 254 |         draws.iter().filter(|sample| sample.value == 1.0).count(),
 255 |         3_000
 256 |     );
 257 | }
```

*Provenance: byte-identical to the completed reference's
`tests/read_path.rs` lines 190–257.*

The export test closes the chapter at both ends of the seam. It writes the
one-record corpus, drives the compiled binary through
`env!("CARGO_BIN_EXE_selfplayexport")` with `--shard-capacity 2`, and
requires success. Then the exported directory loads through
`train::dataset::read_dataset` — `nettrain`'s own reader — and must equal
the six samples `replay_sample` derives for plies 0–5, in order; every
read-back sample's `is_holdout` must agree with
`position_is_holdout(&record, ply)` for its ply, so the split survives the
round trip. Finally the manifest parses as an `ExportManifest` with
`game_count == 1`, `sample_count == 6`, and shards exactly
`["shard-000.bin", "shard-001.bin", "shard-002.bin"]` — six samples at
capacity 2. This test is also why `tests/binary.rs` stays at three tests:
the `selfplayexport` command line is pinned here, inside the read-path
target.

**Source:** [`gomoku/crates/selfplay/tests/read_path.rs`](../../../../gomoku/crates/selfplay/tests/read_path.rs),
lines 259–304 (to the end of the file). *Created in this chapter.*

```rust
 259 | #[test]
 260 | fn export_command_writes_samples_that_the_real_train_reader_loads() {
 261 |     let root = TestDir::new();
 262 |     let corpus = root.path().join("corpus");
 263 |     let dataset = root.path().join("dataset");
 264 |     let record = six_ply_record();
 265 |     write_corpus(std::slice::from_ref(&record), &corpus, 2).expect("fabricated corpus writes");

 267 |     let output = Command::new(env!("CARGO_BIN_EXE_selfplayexport"))
 268 |         .arg("--corpus")
 269 |         .arg(&corpus)
 270 |         .arg("--out")
 271 |         .arg(&dataset)
 272 |         .arg("--shard-capacity")
 273 |         .arg("2")
 274 |         .output()
 275 |         .expect("selfplayexport runs");
 276 |     assert!(
 277 |         output.status.success(),
 278 |         "stderr: {}",
 279 |         String::from_utf8_lossy(&output.stderr)
 280 |     );

 282 |     let expected: Vec<_> = (0..record.plies.len())
 283 |         .map(|ply| replay_sample(&record, ply).unwrap())
 284 |         .collect();
 285 |     let read_back = train::dataset::read_dataset(&dataset).expect("nettrain's reader loads export");
 286 |     assert_eq!(read_back, expected);
 287 |     for (ply, sample) in read_back.iter().enumerate() {
 288 |         assert_eq!(
 289 |             train::split::is_holdout(sample),
 290 |             position_is_holdout(&record, ply).unwrap()
 291 |         );
 292 |     }

 294 |     let manifest: ExportManifest = serde_json::from_slice(
 295 |         &std::fs::read(dataset.join("manifest.json")).expect("export manifest was written"),
 296 |     )
 297 |     .expect("export manifest is valid JSON");
 298 |     assert_eq!(manifest.game_count, 1);
 299 |     assert_eq!(manifest.sample_count, 6);
 300 |     assert_eq!(
 301 |         manifest.shards,
 302 |         ["shard-000.bin", "shard-001.bin", "shard-002.bin"]
 303 |     );
 304 | }
```

*Provenance: byte-identical to the completed reference's
`tests/read_path.rs` lines 259–304.*

## 12. Verification

The chapter's commands, run from the slice's `gomoku/` — the chapter
paper's verification set, plus the `train` and `player` suites and the
workspace gate as regression evidence for a chapter that touches no other
crate, listed in the order the slice's verification run executed and logged
them, with the debug build folded into the capstone:

```text
cargo test -p selfplay --test read_path
cargo test -p selfplay
cargo test -p train
cargo test -p player
cargo test --workspace
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build -p selfplay && <the capstone below>
```

The first command is the focused selection — the chapter's surface in
isolation, including the export test, which compiles and drives the real
`selfplayexport` binary over a fabricated one-game corpus and loads the
result with the real `train::dataset` reader. The second is the fast gate;
cargo prints one report per test target, never one aggregate line, and the
chapter's closing state is the sum of the per-target reports. The third and
fourth are cross-crate regressions the chapter paper does not require —
both crates are untouched — run here as evidence that the slice reads them
without changing them. The fifth is the workspace gate. The last two are
the project's standing quality gates. No `--release -- --ignored` run is
chapter-06 work: no game-playing test exists in the chapter, and the
release-only MANUAL discipline of chapter 04 is untouched.

Evidence actually measured for this paper (2026-11-14, on the chapter-6
slice constructed in the scratch copy described under Provenance — not a
re-run of the completed reference, whose gate evidence the
[hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md)
records; logs at `/tmp/selfplay-ch6-verify-logs/`, scratch-local and
ephemeral):

| Command | Result on the chapter-6 slice |
|---------|-------------------------------|
| `cargo test -p selfplay --test read_path` | 11 passed, 0 failed, 0 ignored (0.16 s) — all eleven read-path tests green |
| `cargo test -p selfplay` | lib target: 2 passed, 0 ignored (0.05 s); `selfplayanchors` binary unit target: 0 tests; `selfplayexport` binary unit target: 0 tests; `selfplaygen` binary unit target: 4 passed, 0 ignored (6.25 s); anchors target: 4 passed (0.00 s); binary target: 3 passed (0.26 s); corpus target: 10 passed, 0 ignored (0.02 s); dedup target: 1 passed; game target: 5 passed, 2 ignored (12.50 s, debug); grading target: 0 passed, 1 ignored; openings target: 5 passed; read_path target: 11 passed (0.04 s); schedule target: 1 passed, 1 ignored; doc-tests: 0 — **46 passed, 4 ignored** in total |
| `cargo test -p train` | src unit tests: 39 passed (0.21 s); `tests` targets: 3 passed (0.17 s); doc-tests: 0 — **42 passed, 0 ignored**; the crate is untouched |
| `cargo test -p player` | src unit tests: 17 passed; `tests/decisions.rs`: 13 passed (0.26 s); `tests/guard.rs`: 13 passed (0.05 s); doc-tests: 0 — **43 passed, 0 ignored**; the crate is untouched |
| `cargo test --workspace` | **exit 0; 850 passed, 0 failed, 11 ignored** as the raw sum over the log's 64 `test result` lines — deduplicated to unique harness invocations, **848 passed, 0 failed, 11 ignored across the workspace's 62 test/doc suites** (the count notes below reconcile the two) |
| capstone (below) | `Exported 1860 samples from 48 games into 4 shards` in 0.12 s (debug); the manifest on disk agrees; the rerun against the same `--out` failed with the refusal message and exit code 1; the input corpus's SHA-256 hashes were identical before and after |
| `cargo clippy --all-targets -- -D warnings` | clean, workspace-wide (11.40 s) |
| `cargo fmt --all -- --check` | no changes |

Four count notes keep the evidence precise. First, the chapter-06 closing
state is the sum of the per-target reports: chapter 05's targets keep their
state (game 5 passed / 2 MANUAL-ignored, corpus 10/0, openings 5/0,
schedule 1/1, dedup 1/0, grading 0/1, anchors 4/0, binary 3/0, and the
`src` unit tests 6/0 — the library's 2 plus the `selfplaygen` binary's 4),
and this chapter adds `tests/read_path.rs` 11/0 — 46 passed, 4 ignored,
the four ignored tests exactly chapter 04's set. Second, the corpus target
again shows three `test result` lines in the full-suite log: its
cross-process determinism test re-executes the test binary twice as a child
process, and the children's one-test reports (1 passed, 9 filtered out
each) appear interleaved in the parent log — chapter 02's mechanism,
unchanged; the workspace log carries the same two child lines. Third, the
workspace row's 850 passed / 11 ignored is the raw sum over that log's 64
`test result` lines, which double-counts those two corpus child reports;
deduplicated to unique harness invocations, the chapter-6 workspace gate is
848 passed, 11 ignored across 62 suites (the raw `selfplay`-only sum is
likewise 48 passed where the deduplicated closing state is 46). Fourth, the
completed reference's suites are larger entirely because chapter 07 adds
the four `selfplayverify_*` binary tests, and both arithmetics close on
that +4: its full `selfplay` suite is 50 passed, 4 ignored — the
hand-off's recorded number, deduplicated; the raw sum is 52 with the same
two child lines — and its workspace gate is 852 passed, 11 ignored across
63 suites deduplicated (854 raw across 65 `test result` lines), matching
850 + 4 = 854 raw and 848 + 4 = 852 deduplicated. The reference numbers
describe the completed reference, not this chapter's expectation.

### The capstone: 1 860 samples from 48 games into 4 shards

Export a corpus the reference already owns and read the samples back,
without touching any committed data. The input is the reference's 48-game
baseline-32 corpus (`data/selfplay/v0-b32/` in the reference worktree,
read-only — SHA-256 hashes of its manifest and three shards were recorded
before the run and verified unchanged after); the output is a temporary
directory outside every repository. From the slice's `gomoku/`, in debug
(nothing plays games, so no release build is needed):

```text
cargo build -p selfplay

dir=$(mktemp -d)

target/debug/selfplayexport \
  --corpus <reference-worktree>/data/selfplay/v0-b32 \
  --out "$dir/export" \
  --shard-capacity 512

cat "$dir/export/manifest.json"

target/debug/selfplayexport \
  --corpus <reference-worktree>/data/selfplay/v0-b32 \
  --out "$dir/export" \
  --shard-capacity 512
```

On the slice (debug build, 2026-11-14, temporary directory
`/tmp/selfplay-ch6-capstone.n14qsb`, retained and scratch-local), the
export exited 0 in 0.12 s and reported
`Exported 1860 samples from 48 games into 4 shards at
/tmp/selfplay-ch6-capstone.n14qsb/export` — matching the reference's
acceptance-run shape of exactly this corpus (1 860 post-opening plies, 4
shards at capacity 512). The read-back already happened inside the binary:
it loaded its own export through `train::dataset::read_dataset` and checked
the count against the manifest before printing that line. The manifest on
disk, byte-exact as `to_writer_pretty` wrote it:

```json
{
  "format_version": 1,
  "game_count": 48,
  "sample_count": 1860,
  "shards": [
    "shard-000.bin",
    "shard-001.bin",
    "shard-002.bin",
    "shard-003.bin"
  ],
  "workspace_version": "0.1.0"
}
```

The rerun against the same `--out` failed as designed: stderr carried
`selfplayexport failed: failed to export samples: refusing to export
samples into a non-empty directory` and the exit code was 1 — an export
never overwrites a dataset by accident, so each export points at a fresh
directory.

Three properties land in one exercise, with honesty notes to keep them in
proportion. The schema shows end to end: `game_count` counts the records,
`sample_count` counts post-opening plies, `shards` lists the shard files in
sample order, and `workspace_version` ties the export to the reader's
format version. The counts match the reference's measured acceptance export
of the same corpus — expected, because export order is corpus order and
nothing in the path rolls dice — but this paper's run is debug, not the
release run the reference logged at 0.18 s; the 0.12 s here is a
like-for-like seam check, not a benchmark. And the read-back check proves
the seam, not the data's quality: the shards load through the consumer's
reader and the counts agree, which is all this chapter claims.

### Historical integration smoke (quoted, attributed, not re-run)

The end-to-end continuation of the seam — the exported shards training the
tiny `nettrain` configuration — was measured once in the reference's
acceptance run and is quoted here from its logs (`acceptance-evidence.md`
§5 in the reference worktree); this paper did not re-run it, and chapter 07
owns the full gates. On the same 48-game baseline-32 corpus, the
reference's release `selfplayexport` produced the same 1 860 samples in 4
shards (0.18 s), and an unmodified release `nettrain` — tiny configuration
(8 channels, 1 residual block, batch size 16), seed 7, 50 steps, eval
interval 50 — split the dataset into **1 665 train and 195 holdout**
samples (the Zobrist rule over the canonical export, 10.5 % held out), ran
50/50 steps in 0.47 s, wrote a checkpoint, and moved train loss from
**6.5483584 at step 1 to 6.252371 at step 50** (`metrics.jsonl`; step-50
holdout loss 6.1449027). This is a smoke result: it proves the net consumed
game-derived sample shards end to end. It is not a model-quality assessment
and not a playing-strength claim of any kind.

## 13. Troubleshooting and pitfalls

Symptoms first, causes second, in the order a learner meets them.

- **`error[E0433]` / unresolved imports for `rand` traits.** The workspace
  pins `rand 0.10`, whose range-sampling trait is `RngExt` with
  `random_range` — not the `Rng` / `gen_range` pair of older releases.
  `use rand::{RngExt, SeedableRng, rngs::StdRng};` is the chapter-6 import
  line; if the compiler cannot find `random_range`, the trait import, not
  the method, is what is missing.
- **`SampleError::PlyOutOfBounds` on the last move.** Plies are zero-based
  and post-opening; a record with *n* plies accepts `0..n`, and
  `replay_sample(&record, n)` is out of bounds. The export loop's
  `0..record.plies.len()` is the correct idiom.
- **A NaN policy in training.** The read path cannot produce one — the
  nonempty-zero histogram is `ZeroVisitTotal` — so a NaN upstream means the
  record was written corrupt; find the ply named in the error rather than
  patching the read path to fabricate a distribution.
- **Confusing the empty histogram with the zero histogram.** Empty means
  "no search ran" (the TSS bypass) and yields the one-hot on the played
  move — deliberate behavior cloning. Nonempty-but-zero means "corrupt
  record" and is a hard error. Collapsing the two cases either fabricates a
  distribution for honest plies or hides corruption.
- **A holdout split that changes between runs.** The Zobrist key was taken
  after augmentation. Zobrist keys are orientation-specific; the rule is
  `zobrist(canonical board) % 100 < 10`, evaluated before any transform.
  `position_is_holdout` and the canonical export path are the only
  split-safe surfaces; `CorpusSampler` output is already augmented and must
  never be re-partitioned.
- **Deriving the side to move from ply parity.** Swap2 answers decide who
  moved first, so parity is not authority; `board.to_move()` after replay
  is. A parity-derived side silently flips every z in corpora whose Swap2
  answer was "take White"-shaped.
- **A mixed stream that never ends.** `SourceMixer` ends only when every
  source is exhausted; one `CorpusSampler` in the mix makes the stream
  infinite. Drive it with `take` or a step budget, and give finite sources
  their weights knowing an infinite neighbor never yields the floor.
- **Expecting the weights "on average."** The mixer is not a random
  distribution that approaches the weights; it is a weighted round-robin
  that meets them exactly per cycle. A test that samples few draws and
  asserts approximate proportions tests the wrong contract.
- **`refusing to export samples into a non-empty directory`.** Working as
  designed: exports never overwrite. Point each export at a fresh
  directory — `mktemp -d` in scripts — rather than deleting the old one in
  place; a deleted-then-recreated dataset loses the refusal's protection
  against mixing two exports' files.
- **`--flag=value` rejected as an unknown flag.** The binary takes values
  as separate arguments only; `selfplayexport --corpus=c` fails with exit
  code 2 and the usage text. Write `--corpus c`.
- **A replay failure deep into a large export.** Validation replays every
  position before the output directory is touched, so the failure leaves no
  partial dataset to trip over — the error names the game index and ply;
  inspect that record with chapter 02's reader rather than re-running the
  export blindly.
- **Editing the `train` crate to make the export fit.** The seam is shaped
  so `nettrain --data` reads the shards unchanged: canonical samples in,
  Zobrist split on load, seeded D4 per sample at batch time. A perceived
  need to modify the consumer is the signal that the export, not the
  consumer, has drifted.

## References

**This chapter:**

- [reading-back.md](reading-back.md) — the chapter paper: intention,
  specification as tests, the execution-order walkthrough, the read-path
  contract, and the integration smoke evidence.
- [reading-back-design.md](reading-back-design.md) — the conceptual story:
  replay as the only trusted reconstruction, stones not planes, the
  canonical holdout, and the export seam.
- [reading-back-implementation.md](reading-back-implementation.md) — the
  hiding layer this paper quotes the construction contracts of; its steps
  fix the chapter-6 slice forms and the test fixtures.

**This track:**

- [Tutorial README](../README.md) — conventions, gates, the chapter map,
  and the reference-provenance rule this paper's Provenance section
  applies.
- [Self-play primer](../00-selfplay-primer.md) — §4.6 states this chapter's
  contract; the glossary defines π, z, the prior, and the holdout split.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md)
  and its [solution](../02-recording-games/recording-games-solution.md) —
  the `GameRecord` format, `Opening::board`, `GameRecord::from_game`, and
  the `read_corpus` / `write_corpus` mechanics this chapter consumes.
- [Chapter 03 — recording the search](../03-recording-the-search/recording-the-search.md)
  and its [solution](../03-recording-the-search/recording-the-search-solution.md) —
  what the recorded visits, priors, and root values mean, and the honesty
  rule for unsearched plies.
- [Chapter 05 — certified endgames](../05-certified-endgames/certified-endgames.md)
  and its [solution](../05-certified-endgames/certified-endgames-solution.md) —
  why unsearched plies exist (the certified own-side TSS bypass), the
  provenance discipline this paper continues, and the chapter-5 scratch
  (`/tmp/selfplay-ch5-verify`) this chapter's slice was copied from.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  decision S2 (store games, not positions; replay, encode, and apply one
  random D4 transform at sample time), §3.3 (the read path), decision S4
  (source mixing).
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  the reference-implementation layout and the measured headline numbers this
  paper quotes with attribution.
- [docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) —
  §7 the locked D4 augmentation and the encode–transform commutation, §9
  the store-games replay discipline.

**Prerequisite tracks:**

- [Datagen tutorial](../../datagen-tutorial/README.md) — the `train` crate:
  `Sample`, the shard mechanics, the dataset reader, and the Zobrist-keyed
  holdout split with its conservative unbuildable-sample default.
- [Net tutorial](../../net-tutorial/README.md) — `nettrain`, the consumer:
  canonical shards in, the Zobrist split on load, seeded per-sample D4 at
  batch time, and the `planes::assemble` batcher that owns the four
  model-input planes.
- [Engine tutorial](../../13-engine-tutorial/README.md) — the bitboard
  `Board`, `Board::zobrist`, the D4 `Transform`s, and the
  `encode_commutes_with_every_transform` proof the augmentation discipline
  relies on.

**Source (created by chapters 01–06; the links resolve once the chapters
land on `main`):**

- [`gomoku/crates/selfplay/src/sample.rs`](../../../../gomoku/crates/selfplay/src/sample.rs) —
  `SampleError`, `replay_sample`, `augment_sample`, `position_is_holdout`
  (sections 1–5). Created in this chapter.
- [`gomoku/crates/selfplay/src/sampler.rs`](../../../../gomoku/crates/selfplay/src/sampler.rs) —
  `SamplerError`, `CorpusSampler` (section 6). Created in this chapter.
- [`gomoku/crates/selfplay/src/mix.rs`](../../../../gomoku/crates/selfplay/src/mix.rs) —
  `MixError`, `WeightedSource`, `SourceMixer` (section 7). Created in this
  chapter.
- [`gomoku/crates/selfplay/src/export.rs`](../../../../gomoku/crates/selfplay/src/export.rs) —
  `SAMPLE_EXPORT_FORMAT_VERSION`, `ExportManifest`, `ExportError`,
  `export_corpus` (section 8). Created in this chapter.
- [`gomoku/crates/selfplay/src/bin/selfplayexport.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayexport.rs) —
  the export binary (section 9). Created in this chapter.
- [`gomoku/crates/selfplay/tests/read_path.rs`](../../../../gomoku/crates/selfplay/tests/read_path.rs) —
  the read-path specification, eleven tests (section 11). Created in this
  chapter.
- [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs) —
  declares the four new modules and re-exports the new vocabulary (section
  10). Touched in this chapter.

**Source (unchanged in this chapter; the `selfplay` links resolve once
chapters 01–06 land on `main`, and the `train` and `engine` links resolve
today — except `train/src/storage.rs`, which lands with this track's
chapter 02, by extraction):**

- [`gomoku/crates/selfplay/tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs) —
  unchanged at three tests; the `selfplayexport` command line is pinned by
  the read-path export test, and the four `selfplayverify_*` tests arrive
  in chapter 07.
- [`gomoku/crates/selfplay/src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs),
  [`gomoku/crates/selfplay/src/corpus.rs`](../../../../gomoku/crates/selfplay/src/corpus.rs),
  [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs),
  and the chapters 01–05 vocabulary — consumed through call sites only;
  scripted whole-file comparisons confirm byte-equality with the chapter-5
  slice.
- [`gomoku/crates/train/src/sample.rs`](../../../../gomoku/crates/train/src/sample.rs),
  [`gomoku/crates/train/src/storage.rs`](../../../../gomoku/crates/train/src/storage.rs),
  [`gomoku/crates/train/src/dataset.rs`](../../../../gomoku/crates/train/src/dataset.rs),
  [`gomoku/crates/train/src/split.rs`](../../../../gomoku/crates/train/src/split.rs) —
  the consumed `train` surface: `Sample`, `write_shard`, `read_dataset`,
  `is_holdout` / `HOLDOUT_PERCENTAGE`. Not touched in this chapter.
- [`gomoku/crates/engine/src/symmetry.rs`](../../../../gomoku/crates/engine/src/symmetry.rs)
  and
  [`gomoku/crates/engine/src/encode.rs`](../../../../gomoku/crates/engine/src/encode.rs) —
  the D4 transforms and the encode–transform commutation. Not touched in
  this chapter.
- [`gomoku/crates/selfplay/Cargo.toml`](../../../../gomoku/crates/selfplay/Cargo.toml) —
  no manifest changes; `selfplay` already depends on `engine`, `train`,
  `player`, `rand`, `serde`, `serde_json`, and `thiserror`, and a binary in
  `src/bin/` needs no entry.

**Reference provenance:** the completed reference implementation lives in
the read-only sibling worktree `rust-ml-selfplay-ref` (branch
`selfplay-ref-v2`, base `9028aa8`, changes uncommitted); it is quoted and
compared against, never linked, and it is deleted once the tutorial is
complete on `main`. The chapter-6 slice in this paper was constructed per
the implementation plan in an untouched copy of the verified chapter-5
scratch (`/tmp/selfplay-ch5-verify` → `/tmp/selfplay-ch6-verify`) and
gate-verified there on 2026-11-14 (logs at
`/tmp/selfplay-ch6-verify-logs/`, including the scripted provenance
comparisons in `14-provenance-diff.log`); the reference worktree, the
chapter-5 scratch, and the earlier scratch copies
(`/tmp/selfplay-ch1-verify`, `/tmp/selfplay-ch2-verify`,
`/tmp/selfplay-ch3-verify`, `/tmp/selfplay-ch4-verify`) were not modified.
The capstone export lives at `/tmp/selfplay-ch6-capstone.n14qsb/`
(scratch-local, ephemeral); its input corpus was read-only, with SHA-256
hashes recorded before and after to prove it. The quoted integration smoke
is the reference worktree's `acceptance-evidence.md` §5, cited here — not
re-run — exactly as the chapter paper cites it.

**External sources:**

- Allis, L. V. (1994). *Searching for Solutions in Games and Artificial
  Intelligence*. PhD thesis, Maastricht University.
  <https://doi.org/10.26481/dis.19940923la> — threat-space search, the
  prover behind the TSS bypasses whose plies carry no visits.
- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). <https://doi.org/10.1038/nature24270> — the
  (s, π, z) training record: policy from search visit counts, value from
  the game outcome, board symmetries as augmentation.
