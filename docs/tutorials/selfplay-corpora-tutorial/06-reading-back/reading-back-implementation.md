# Chapter 06 — Reading back: implementation plan

## Abstract

This document is the implementation plan for
[chapter 06 of the selfplay-corpora tutorial](reading-back.md): the
hiding layer between the chapter paper and the solution, for the
learner who builds the code without reading the reference. It turns
the chapter's walkthrough into a numbered construction order along the
execution path of one position travelling from a stored record to a
training batch: the new `sample` module gains the four-variant
`SampleError` and the three free functions `replay_sample`,
`augment_sample`, and `position_is_holdout`; the new `sampler` module
gains `SamplerError` and the seeded, infinite `CorpusSampler`; the new
`mix` module gains `MixError`, `WeightedSource`, and the weighted
round-robin `SourceMixer`; the new `export` module gains
`SAMPLE_EXPORT_FORMAT_VERSION`, `ExportManifest`, the four-variant
`ExportError`, and the validate-before-write `export_corpus`; the new
`selfplayexport` binary drives the export and reads its own output
back through the real `train` dataset reader; and the export audit,
regression, gates, and capstone close the chapter. Every artifact
names its file, its behavioral contract in prose, the helpers to
reuse, and the idioms to apply by name; method bodies never appear.
The plan pins the exact test names, fixtures, and assertions of the
chapter's eleven new tests — all default-run, none MANUAL, all living
in the one new target `tests/read_path.rs` — and the per-target counts
that sum to the chapter's closing state of 46 passed and 4 ignored for
`selfplay`. The intended reader has completed chapter 05 (suite state:
35 passed, 4 ignored), has read the chapter paper and, ideally, the
[design paper](reading-back-design.md), and wants low-level guidance
without solution leakage.

## Glossary

| Term | Definition |
|------|------------|
| **Game record** | `GameRecord`: the durable serialization of one complete game — a header (seed, seat pairing, evaluator identity, the Swap2 opening), the ordered post-opening `plies`, the `outcome`, and an optional certified suffix. Chapter 02 fixed the format. |
| **Recorded ply** | `RecordedPly { mv, visits, prior, root_value }`: one played post-opening move and its durable search data. `visits` holds sparse raw root visit counts (`Vec<(Move, u32)>`, descending); `prior` holds up to 16 root priors for audit; both are empty when no search ran, and `root_value` is then `None`. |
| **Sample** | `train::Sample`: the labelled training record — `black` and `white` stone lists in absolute color (colors as played, never relative to the side to move), `to_move`, a sparse `policy` of `(Move, f32)` pairs, and a `value` target from the side-to-move perspective. It stores stones, never planes. |
| **π (policy target)** | The normalized root visit-count distribution of the search at a ply; the policy training target (Silver et al. 2017). Computed at read time from the recorded raw counts. |
| **z (value target)** | The game outcome in {+1, 0, −1} from the perspective of the side to move at the sampled ply; the value training target. |
| **Behavior cloning** | The policy target at an unsearched ply: a one-hot distribution on the move actually played, `[(played_move, 1.0)]`, rather than a distribution derived from search. Unlike the visit-normalized π at searched plies, it carries no search information — it supervises the network toward the played (typically machine-certified) move directly. |
| **Canonical orientation** | The position exactly as played, with no D4 transform applied. Records and exported shards are canonical; augmentation happens per sample at read time. |
| **D4** | The dihedral group of the square: the 8 rotations and reflections of the board, `engine::Transform::ALL`. |
| **Zobrist key** | A 64-bit hash of a board position, maintained incrementally by the engine (`Board::zobrist`). Orientation-specific: a transformed position hashes differently. |
| **Holdout split** | The train/holdout partition of the `train` crate: a sample is held out iff `zobrist(board) % 100 < HOLDOUT_PERCENTAGE` (= 10). Content-based, so collection order cannot leak correlated positions into both sides. |
| **CorpusSampler** | The seeded, infinite iterator of augmented training samples over a corpus: each draw picks a random game, a random ply, and a random D4 transform, then replays and augments on demand. |
| **SourceMixer / WeightedSource** | The seeded weighted round-robin over sample iterators: each `WeightedSource` contributes `weight` consecutive items per turn, and the seed picks the initial source. |
| **Shard** | A fixed-capacity file of length-delimited bincode records named `shard-NNN.bin`; the `train` crate's storage unit, shared by synthetic datasets, game corpora, and sample exports. |
| **ExportManifest** | The JSON provenance of an exported sample dataset: format version, game count, sample count, shard file names in sample order, and the workspace version. |
| **Swap2** | The project's opening protocol: one side places three stones, the other side chooses a color or places two more stones, and the first side then chooses a color. The placements are the ceremony, not plies; the answer decides which color moved first. |
| **Post-opening ply** | One played move after the Swap2 ceremony, counted from zero. Every `ply` index in this chapter counts in these plies. |
| **TSS bypass** | The certified own-side threat-space search (TSS; Allis 1994) proof that short-circuits a decision: the line's first move is played and no search runs. Such a ply records empty `visits` and `prior` and `root_value: None` (chapter 03's honesty rule). |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games. It never runs by default; it runs on explicit selection, in release mode. This chapter adds none. |
| **TDD** | Test-driven development: write the test, watch it fail (red), implement, watch it pass (green). |
| **Claim labels** | The project's provenance labels (chapter 12 of the wiki): **[paper]** (stated in a cited paper), **[derived]** (computed from cited or measured values), **[experiment]** (a free parameter, to be settled by measurement). |

## How to use this plan

Read [the chapter paper](reading-back.md) first. It carries the
intention, the specification as tests, and the execution-order
walkthrough; the [design paper](reading-back-design.md) carries the
reasoning, the two-format ownership table, and the ten invariants.
This plan assumes both and adds only construction detail: which file,
which item, which contract, which test goes red then green. Work one
step at a time, in order — the order is the execution order of the
read path: replay first (the position, then its policy and value
targets), then augmentation, then the holdout partition, then the
sampling and mixing streams, then the export through the real binary
last. The production files land in dependency order — `src/sample.rs`,
then `src/sampler.rs` and `src/mix.rs` (which call `sample.rs`), then
`src/export.rs` (which calls `replay_sample`), then
`src/bin/selfplayexport.rs` (which calls `export_corpus`) — and each
step writes its tests first. The one test target,
`tests/read_path.rs`, grows test group by test group at the start of
each step; the `src/lib.rs` re-exports land inside each step, before
that step's tests are watched green. Open
[reading-back-solution.md](reading-back-solution.md) only if you are
stuck for more than twenty minutes, or after the chapter is complete.

## Context

The chapter-05 slice — the verified starting state — holds the
following. Selfplay paths are under
[`gomoku/crates/selfplay/`](../../../../gomoku/crates/selfplay/);
the crate does not exist on `main` until the learner lands it chapter
by chapter.

- [`src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs)
  (chapters 02–03) carries `GameRecord` with its `header`, `plies`,
  `outcome`, and `certified_suffix` fields; `GameHeader` with the
  `opening` slot; `RecordedPly { mv, visits, prior, root_value }` with
  `visits: Vec<(Move, u32)>`, `prior: Vec<(Move, f32)>`, and
  `root_value: Option<f32>`; and `GameRecord::from_game`, the
  conversion the runner and this chapter's test fixture share. **This
  chapter does not touch `src/record.rs`.**
- [`src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs)
  (chapters 01–05) carries `Game` (seed, per-seat options, opening,
  plies, outcome, certified suffix), `PlyRecord { mv, search }` with
  `search: Option<PlySearch>`, `PlySearch { side_to_move, visits,
  prior, root_value }`, `Outcome`, and `WinReason`. The fixture
  fabricates a `Game` by hand and converts it with
  `GameRecord::from_game`, so no test plays a game.
- [`src/opening.rs`](../../../../gomoku/crates/selfplay/src/opening.rs)
  (chapters 01 and 04) carries `Opening::new` — constant constructor
  from the three initial cells (placement order Black, White, Black),
  an `OpeningAnswer`, and an `OpeningProvenance` — and
  `Opening::board`, which executes the ceremony through the engine's
  Swap2 typestate into a fresh `Result<Board, PlayError>`. Replay
  starts here.
- [`src/corpus.rs`](../../../../gomoku/crates/selfplay/src/corpus.rs)
  (chapter 02) carries `write_corpus`, which borrows a record slice,
  an output directory, and a shard capacity and returns an
  `io::Result<Manifest>`, and `read_corpus`, which decodes a corpus
  directory into `Vec<GameRecord>` in record order.
- [`src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs)
  declares seven modules (`anchor`, `corpus`, `game`, `generate`,
  `opening`, `record`, `schedule`) behind `#![deny(missing_docs)]`
  and re-exports the chapter-05 surface. Every public item this
  chapter adds needs doc comments; the deny turns a missing doc into
  a build error.
- [`Cargo.toml`](../../../../gomoku/crates/selfplay/Cargo.toml) needs
  **no changes**: by chapter 06 the crate already depends on
  `baseline`, `engine`, `player`, `rand`, `serde`, `serde_json`,
  `thiserror`, and `train` (chapter 01 added `rand`; chapter 02's
  step 0 added `serde_json` and `train`), and a binary in `src/bin/`
  needs no manifest entry. One deliberate absence continues:
  `bincode` never appears in this manifest — the length-delimited
  framing lives behind `train::storage`, so `train`'s dependency
  covers it. Verify the eight dependencies before writing anything;
  if a step seems to need a dependency, the step has drifted.
- The `train` crate (the datagen tutorial's, consumed unchanged)
  carries `Sample` with its five public fields, its `Debug`, `Clone`,
  `PartialEq`, and serde derives (the equality assertions and the
  shard encoding both rely on them), and `Sample::board`, which
  rebuilds an engine `Board` from the stored stones;
  `train::split::is_holdout` and `train::split::HOLDOUT_PERCENTAGE`
  (= 10), the content-based partition with its conservative
  unbuildable-sample default; `train::dataset::read_dataset`, the
  shard-discovering reader `nettrain` trains from;
  `train::shard::SHARD_SIZE` (= 4096), the export binary's default
  capacity; and `train::storage::write_shard`, the shared
  length-delimited writer that opens every shard **create-new**.
  `train::storage` is chapter 02's extraction — the shared shard
  mechanics used by synthetic datasets and game corpora alike — and
  this chapter consumes it unchanged. Note for orientation: on
  `main`, `train/src/storage.rs` does not exist until the learner's
  chapter 02 lands it; the remaining `train` files are live.
- The engine (consumed, unchanged) carries `Board` with `play`,
  `to_move`, `stone_at`, and `zobrist`; `Move::new`; `Color`;
  `Transform` with `Transform::ALL` (all eight elements),
  `Transform::Rot90`, and `transform_move`; and `PlayError`, the
  rejection carried inside this chapter's replay errors.
- `nettrain` (the consumer, untouched) reads its `--data` directory
  through `train::dataset::read_dataset` and partitions with
  `train::split::is_holdout` on load — the two calls the export seam
  is shaped to, verified in
  [`net/src/bin/nettrain.rs`](../../../../gomoku/crates/net/src/bin/nettrain.rs).
- Test targets at the chapter-05 state: `tests/game.rs` 5 passed / 2
  MANUAL-ignored; `tests/corpus.rs` 10/0; `tests/openings.rs` 5/0;
  `tests/schedule.rs` 1/1; `tests/dedup.rs` 1/0;
  `tests/grading.rs` 0/1; `tests/anchors.rs` 4/0; `tests/binary.rs`
  3/0; the `src` unit tests 6/0 (the library target's two tracker
  tests plus the `selfplaygen` binary's four) — **35 passed, 4
  ignored** in sum.

Three scope boundaries keep the chapter honest. First,
[`tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs)
gains **no** chapter-06 tests and stays at the three tests chapter 05
left there: the `selfplayexport` command line is pinned inside
`tests/read_path.rs` itself, whose export test drives the compiled
binary end to end, and the release-binary smoke on a real corpus is
quoted acceptance evidence, not a unit-test matter. The target
reaches its final seven tests only when chapter 07 adds the four
`selfplayverify_*` tests. Second, no chapter-07 artifact is named
anywhere in this chapter's code: no `selfplayverify` binary, no
verification module, no corpus-statistics vocabulary. Third, no other
crate is touched: the chapter only reads `engine`, `train`, and the
crate's own chapters 01–05 vocabulary, so `cargo test -p train`,
`cargo test -p player`, and `cargo test -p net` are not chapter-06
gates.

---

## Step 1 — `src/sample.rs` and the seven replay-and-target tests (red → green)

Create
[`src/sample.rs`](../../../../gomoku/crates/selfplay/src/sample.rs) and
[`tests/read_path.rs`](../../../../gomoku/crates/selfplay/tests/read_path.rs).
Declare the `sample` module in `src/lib.rs`, in alphabetical position
(after `record`, before `schedule`), and add the re-export group —
`SampleError`, `replay_sample`, `augment_sample`, `position_is_holdout`
— in alphabetical position among the groups. The re-exports land
**before** the integration tests are watched green: the tests import
crate-root names only.

### 1.1 The shared fixture and the first seven tests, red

Create `tests/read_path.rs` with the imports, the two fixture helpers,
and the seven tests below. Imports: the engine's `Color`, `Move`, and
`Transform`; the `player` crate's `PlayerOptions`; and, from the crate
root, `Game`, `GameRecord`, `Opening`, `OpeningAnswer`,
`OpeningProvenance`, `Outcome`, `PlyRecord`, `PlySearch`, `SampleError`,
`WinReason`, `augment_sample`, `position_is_holdout`, and
`replay_sample`. `train::split` is used by path. The red state is a
compile error — the module does not exist.

Two helpers, added above the tests:

- `mv(row, col)` — the shared fixture constructor from the other test
  targets, validating through `Move::new`, expected with the message
  "test coordinate is on the board".
- `six_ply_record()` — the one fabricated record all eleven tests of
  this chapter share; no game is ever played. Build a `Game` by hand
  and convert it with chapter 02's `GameRecord::from_game`:
  - seed 17; both option sets the `PlayerOptions` defaults;
  - the opening's three initial stones at (5,5), (10,10), and (5,6)
    through `Opening::new`; the answer is the take-`Black` variant, so
    White moves first after the ceremony; the provenance is
    `Generated`;
  - six post-opening plies — (10,11), (5,7), (10,12), (5,8), (10,13),
    (5,9) — White builds a horizontal line on row 10, Black on row 5.
    Each ply carries search metadata exactly when its index is not 5,
    built with the bool-then-conversion idiom: `side_to_move` White on
    even plies and Black on odd plies (matching the ceremony's answer),
    a one-entry visit histogram on the played move with count 1, a
    one-entry prior of 1.0, and a root value of 0.25. Ply 5 carries
    `search: None` — the `from_game` conversion turns it into empty
    visits, an empty prior, and `root_value: None`, standing in for a
    TSS-bypassed certified move;
  - the outcome is a Black win, reason five-or-more;
  - the certified-suffix slot is `None`.

The fixture's load-bearing properties: the take-Black answer makes
White the first post-opening mover, so ply parity alone cannot tell
you the side to move — only the replayed board can; and ply 5 is the
honesty-rule case, an unsearched ply with no histogram.

The seven tests, written and watched red in the specification's
execution order — replay first (board and side to move), then the
policy target, then the value target, then augmentation, then the
holdout partition:

1. `replay_at_ply_has_opening_and_played_stones_with_the_right_side_to_move`
   — `replay_sample(&six_ply_record(), 3)`, expected with the message
   "fabricated record replays". Assert equality on the sample's Black
   stones against exactly `[(5,5), (5,6), (5,7)]` — two opening stones
   and one played — and on the White stones against exactly
   `[(10,10), (10,11), (10,12)]`; assert the side to move is
   `Color::Black`. The list order pins a contract: stone lists are
   enumerated from the rebuilt board by color in row-major board
   order, not in play order. The side to move pins the other: ply 3 is
   Black's turn, and the board says so — ply parity would say so here
   only by coincidence of this fixture's answer.
2. `raw_visits_become_a_sparse_normalized_policy` — replace the ply-0
   visit histogram with `[(10,11) → 3, (0,0) → 1, (14,14) → 2]`;
   replay at ply 0. Assert the policy equals exactly
   `[((10,11), 0.5), ((0,0), 1.0/6.0), ((14,14), 1.0/3.0)]` — sparse,
   in the recorded order, exact `f32` equality on each pair — then sum
   the masses and assert the total is within 1e-6 of 1, with the mass
   named in the failure message. π is the normalized visit
   distribution, nothing else.
3. `a_ply_without_search_uses_the_played_move_as_its_policy` — assert
   the record's ply-5 visits are empty (the fixture contract); replay
   at ply 5, expected with the message "TSS-bypassed ply replays";
   assert the policy equals exactly `[(played_move, 1.0)]` where the
   played move is the record's ply-5 move. An unsearched ply
   contributes deliberate behavior cloning on the move actually played
   — never a fabricated distribution.
4. `nonempty_zero_visit_histogram_is_rejected_instead_of_producing_nan`
   — replace the ply-0 histogram with `[(0,0) → 0, (1,1) → 0]`:
   nonempty, but with zero total. Assert with the matches idiom that
   replay at ply 0 fails with `SampleError::ZeroVisitTotal` carrying
   `ply: 0`. A corrupt record gets a named error, never a NaN policy
   that surfaces three crates away as a training divergence.
5. `outcome_value_uses_each_position_side_to_move_perspective` — on
   the unmodified fixture: replay at ply 0 and assert the side to move
   is White and the value is −1.0; replay at ply 1 and assert the side
   to move is Black and the value is +1.0. Then set the outcome to a
   draw and assert both plies' values are 0.0. z is always the outcome
   from the sampled position's side to move — the replayed board
   supplies that side, never the recording seat and never ply parity.
6. `d4_augmentation_transforms_stones_and_policy_together` — replace
   the ply-0 histogram with `[(0,0) → 3, (10,11) → 1]`; replay at ply
   0 for the canonical sample; augment with `Transform::Rot90`. Assert
   both augmented stone lists equal the element-wise transform of the
   canonical lists (a small expected-list closure over
   `transform_move`); assert the policy equals exactly
   `[(Rot90(0,0), 0.75), (Rot90(10,11), 0.25)]`; assert
   `Rot90(0,0) == (14,0)` through the first policy entry, pinning the
   rotation direction; assert the side to move and the value are
   unchanged from the canonical sample. One transform moves stones and
   policy masses together; the game facts are D4-invariant.
7. `holdout_uses_the_canonical_position_zobrist_and_train_partition` —
   replay at ply 2. Compute the expected partition by hand: rebuild
   the engine board from the sample's own stones through
   `Sample::board` (the sample stores stones, never planes — the board
   is recovered, so the planes are recoverable at batch time), take
   its Zobrist key, and apply the synthetic rule
   `zobrist % 100 < train::split::HOLDOUT_PERCENTAGE`. Assert
   `train::split::is_holdout(&sample)` agrees with the expectation,
   and assert `position_is_holdout(&record, 2)` agrees with both. The
   selfplay read path uses the synthetic datasets' content-based rule
   unchanged, on the canonical position, before any augmentation.

### 1.2 `SampleError`, green

A public error enum in `src/sample.rs`, deriving `Debug` and the
`thiserror` error derive — no `Clone`, no `PartialEq`: equality and
cloning are not part of this chapter's contract, and the tests match
on variants with the matches idiom rather than comparing for
equality. Four variants, in the
reference's declaration order — the vocabulary order, not the check
order — each with a doc comment and documented fields:

1. `PlyOutOfBounds` — the requested post-opening ply does not exist.
   Fields: `ply` (a `usize`, the requested zero-based ply) and
   `ply_count` (a `usize`, the number of post-opening plies in the
   record). Display: "ply {ply} is outside the record's {ply_count}
   plies".
2. `ZeroVisitTotal` — a nonempty visit histogram contains no actual
   visits. Field: `ply` (a `usize`). Display: "visit histogram at ply
   {ply} has zero total visits".
3. `Opening` — the record's Swap2 opening was invalid. A tuple variant
   wrapping the engine's `PlayError`, marked as the error source.
   Display: "opening replay failed: {0}".
4. `Replay` — a recorded move was rejected during replay. Fields:
   `ply` (a `usize`), `mv` (the rejected `Move`), and `source` (the
   `PlayError`). Display: "move {mv:?} at ply {ply} was rejected:
   {source}" — the move renders with its debug format, the same idiom
   as chapter 05's `AnchorError::Replay`.

The module opens with a module-level doc comment that states the
placement decision: replay lives in `selfplay`, not `train`, because
replay owns the `GameRecord` and opening vocabulary, and the
dependency already points `selfplay → train` for storage, so `train`
stays the shared sample and shard-format crate without learning about
self-play records. The module's imports: the engine's `Board`,
`Color`, `Move`, `PlayError`, and `Transform`; the `thiserror`
derive; `train::Sample`; and the crate's `GameRecord` and `Outcome`.

### 1.3 `replay_sample`, green

A public free function borrowing a `GameRecord` and consuming a
zero-based post-opening `ply: usize`, returning
`Result<Sample, SampleError>`. The doc comment states the two
contracts: the sample describes the position *immediately before*
`ply` is played, and it is in canonical orientation, with both targets
derived from the record at this same ply. Behavioral contract, in
execution order:

1. **Replay the position.** Rebuild the board through a private
   helper (§1.5): the opening executes, then the first `ply` recorded
   moves play through it.
2. **Derive π from the recorded visits.** Sum the histogram's counts,
   widening each `u32` count to `u64` for the total. A nonempty
   histogram whose total is zero fails with `ZeroVisitTotal { ply }` —
   the rejection happens before any division. An *empty* histogram is
   the honest case: the policy is the one-hot `[(played_move, 1.0)]`.
   Place above the one-hot branch the comment the reference carries,
   stating its content exactly: a certified TSS move bypasses
   search, so there is no visit histogram to normalize; its
   deterministic one-hot policy is deliberately behavior cloning for
   that ply; the game-outcome value target is unchanged. A nonempty
   histogram normalizes: each entry keeps its move and its recorded
   order, and its mass is the count divided by the total, computed in
   `f32` — the test's exact-equality assertions pin this arithmetic.
3. **Derive z from the outcome.** Take the side to move from the
   replayed board. A draw is 0.0; a win by the side to move is +1.0;
   a win by its opponent is −1.0. The recorded `prior` and
   `root_value` are read by no code path here — they are audit data.
4. **Assemble the sample.** The stone lists come from the board by
   color through the private enumeration helper; the side to move from
   the board; the policy and value as above. Stones, never planes.

The returned `train::Sample` relies on the engine's proven
encode–transform commutation
(`encode_commutes_with_every_transform` in
[`engine/src/encode.rs`](../../../../gomoku/crates/engine/src/encode.rs))
only downstream: because transform-then-encode equals
encode-then-permute, augmentation can live on stone lists and the
17×17 border is added exactly once, inside `net`'s batcher.

### 1.4 `augment_sample` and `position_is_holdout`, green

`augment_sample` is a public free function borrowing a canonical
`Sample` and consuming one `Transform`, returning a new `Sample` —
mark it must-use, because the result is the only product. Behavioral
contract: map both stone lists and every policy move through
`transform.transform_move`; carry the side to move and the value over
unchanged. The doc comment states the invariance (D4 rotations and
reflections preserve wins, losses, draws, and turn order) and the
deferral (the function operates on stone lists, so the 17×17 border is
still added later by `net`).

`position_is_holdout` is a public free function borrowing a
`GameRecord` and consuming a ply, returning `Result<bool, SampleError>`
— the same replay errors as `replay_sample`, which it composes.
Behavioral contract: replay, then apply `train::split::is_holdout` to
the canonical sample. The doc comment states the two load-bearing
facts: the rule deliberately evaluates the canonical position before
augmentation, matching `nettrain`, because Zobrist keys are
orientation-specific and a random D4 orientation must not move a
position between training and holdout; and callers that already own a
canonical `Sample` call `train::split::is_holdout` directly.

### 1.5 The two private helpers

Both stay private — no `pub`, not even crate-visible:

- `replay_board(record, ply) -> Result<Board, SampleError>` — the
  bounds check first: a `ply` at or beyond the record's ply count
  fails with `PlyOutOfBounds { ply, ply_count }`. Then
  `record.header.opening.board()` executes the Swap2 ceremony, its
  rejection mapped into `Opening`. Then the first `ply` recorded moves
  play in order through the fresh board, enumerated so the first
  rejection maps into `Replay { ply: index, mv, source }` with the
  zero-based ply index of the rejected move. The engine's legality
  gate is the only position check the read path trusts.
- `stones(board, color) -> Vec<Move>` — enumerate the board indices 0
  to 224 in row-major order, construct each `Move` from its row and
  column (the range contains only valid cells; expect accordingly),
  and keep exactly the cells where `board.stone_at` reports the given
  color. Board order, not play order — test 1 pins it.

Watch the seven tests compile and pass:

```text
cargo test -p selfplay --test read_path
```

reports exactly **7 passed, 0 ignored**.

---

## Step 2 — `src/sampler.rs`: the seeded `CorpusSampler` (red → green)

Create
[`src/sampler.rs`](../../../../gomoku/crates/selfplay/src/sampler.rs).
Declare the `sampler` module in `src/lib.rs` (alphabetical, after
`sample`) and re-export `CorpusSampler` and `SamplerError` before
watching green.

### 2.1 The two sampler tests, red

Extend `tests/read_path.rs`. New imports: the standard library's path
types (`Path`, `PathBuf`) and time sources (`SystemTime`,
`UNIX_EPOCH`); and, from the crate root, `CorpusSampler` and
`write_corpus`. One new helper, added above the tests:

- `TestDir` — the same temporary-directory discipline chapter 05's
  binary tests use: a unique directory under the system temporary
  directory whose name joins the prefix `selfplay-read-path`, the
  process id, and a nanosecond timestamp; a `path` accessor; removal
  on `Drop`, ignoring errors. The directory cleans itself up when the
  test ends.

The two tests, in this order:

1. `corpus_sampler_is_deterministic_under_one_seed` — build two
   `CorpusSampler`s from the same one-record vector (clone the
   vector), both with seed 42, each construction expected with the
   message "nonempty corpus". Take 64 draws from each, collecting the
   `Result` items into vectors with the collect-a-`Result` idiom,
   expected with "all fabricated draws replay". Assert the two streams
   are equal — `Sample` derives `PartialEq`, augmentation included —
   and assert every drawn sample's policy masses sum to 1 within
   1e-6. Same corpus, same seed, same stream; augmentation never
   breaks the policy invariant.
2. `corpus_sampler_loads_a_corpus_directory` — under a fresh
   `TestDir`, `write_corpus` writes the one-record corpus at shard
   capacity 2, expected with "fabricated corpus writes";
   `CorpusSampler::from_corpus(dir, 42)` loads it, expected with
   "directory corpus loads"; four draws replay successfully, expected
   with "directory-backed draws replay", and the drawn vector has
   length 4. The directory-backed constructor is the same stream over
   shard-read records.

The red state is a compile error — the module does not exist.

### 2.2 `SamplerError` and `CorpusSampler`, green

`SamplerError` is a public error enum deriving `Debug`, the error
derive, `PartialEq`, and `Eq` — equality is affordable here because no
variant carries a source. One variant: `EmptyCorpus` — no records were
supplied, or every record has zero post-opening plies. Display: "a
corpus sampler needs at least one game with a post-opening ply".

`CorpusSampler` is a public struct with three **private** fields: the
records vector, `sampleable_games` (a vector of `usize` game
indices), and one `StdRng`. The doc comment carries the split-duty
prohibition, stating: each item replays its selected record on demand,
so positions are never materialized in advance; the stream yields
augmented samples for training; consumers constructing a split dataset
use direct replay plus `position_is_holdout` before augmentation, or
the canonical export path, because applying the Zobrist rule after a
D4 transform would hash a different orientation.

Two associated functions and the iterator:

- `new(records: Vec<GameRecord>, seed: u64) -> Result<Self,
  SamplerError>` — construction filters the corpus to the games that
  have at least one post-opening ply (the enumerate-filter-map idiom,
  keeping the indices) and rejects a corpus with none up front with
  `EmptyCorpus` — a fail-fast check: the error surfaces at
  construction, not as a panic on the first draw. The RNG is
  `StdRng::seed_from_u64(seed)`; one seed per stream, the chapter-01
  discipline at the stream level.
- `from_corpus(dir: &Path, seed: u64) -> io::Result<Self>` — a thin
  composition of `read_corpus` and `new`: corpus I/O and decoding
  errors propagate, and an empty corpus surfaces as
  `io::ErrorKind::InvalidData`. I/O lives at the boundary; the
  deterministic core has exactly one input.
- `Iterator::next` — the item type is `Result<Sample, SampleError>`,
  so replay errors surface at draw time rather than being hidden by
  the iterator, and the stream never yields `None`. Each draw consumes
  three uniform randoms from the one seeded stream **in this fixed
  order**: a game slot among the sampleable games (ranged random over
  the slot count, then index), a ply within that game (ranged random
  over the game's ply count), and a transform (ranged random over
  `Transform::ALL`, then index). The draw then replays on demand —
  `replay_sample` plus `augment_sample`. The draw order is part of the
  determinism contract: same records, same seed, same stream.

One rand-version note, grounded in the manifest's `rand = "0.10.3"`:
the ranged-random method is `random_range`, brought in by the `RngExt`
extension trait (not the `Rng` trait of older releases), and `StdRng`
lives under `rand::rngs`. Chapter 04's opening code already applies
this idiom; reuse it.

The module's imports: the standard library's `io` and `Path`; the
engine's `Transform`; the rand extension trait, the seeding trait, and
`StdRng`; the error derive; `train::Sample`; and the crate's
`GameRecord`, `SampleError`, `augment_sample`, and `replay_sample`.

Watch both tests compile and pass:

```text
cargo test -p selfplay --test read_path
```

reports exactly **9 passed, 0 ignored**.

---

## Step 3 — `src/mix.rs`: the weighted `SourceMixer` (red → green)

Create
[`src/mix.rs`](../../../../gomoku/crates/selfplay/src/mix.rs). Declare
the `mix` module in `src/lib.rs` (alphabetical, after `generate`) and
re-export `MixError`, `SourceMixer`, and `WeightedSource` before
watching green.

### 3.1 The mixer test, red

Extend `tests/read_path.rs` — new imports from the crate root:
`SourceMixer` and `WeightedSource`. One test:

`source_mixer_respects_weights_over_a_large_seeded_draw` — a marker
closure builds a `train::Sample` from a value: empty stone lists,
`Color::Black` to move, a one-hot policy on (0,0), and the given
value, so the source of a draw is identifiable by its `value`. Build a
mixer from two infinite sources — `WeightedSource::new(1, …)` over an
unbounded repetition of the −1.0 marker, `WeightedSource::new(3, …)`
over the 1.0 marker, both through the standard infinite-repetition
iterator — with seed 7, construction expected with "nonzero source
weights". Take 4 000 draws through the by-reference idiom so the
mixer stays owned by the test. Assert exactly 1 000 draws carry value
−1.0 and exactly 3 000 carry value 1.0. The mix is not a random
distribution that merely approaches the weights; it is a deterministic
weighted round-robin that matches them exactly over a full cycle — an
equality, not a confidence interval.

The red state is a compile error — the module does not exist.

### 3.2 `MixError`, `WeightedSource`, and `SourceMixer`, green

`MixError` is a public error enum deriving `Debug`, the error derive,
`PartialEq`, and `Eq`. Two variants, each documented:

1. `Empty` — no sources were supplied. Display: "a source mixer needs
   at least one source".
2. `ZeroWeight` — one source had zero weight and could never be drawn.
   Field: `index` (a `usize`, the zero-based source index). Display:
   "source {index} has zero weight".

`WeightedSource<T>` is a public struct, generic over the item type,
with three **private** fields: `weight` (a `u32`), the boxed iterator
(`Box<dyn Iterator<Item = T>>`), and an `exhausted` flag. Its one
associated function `new(weight: u32, samples: impl Iterator<Item = T>
+ 'static) -> Self` boxes the iterator and starts unexhausted. The doc
comment states the two contracts: the item type is generic so a mixer
can combine plain synthetic `Sample` iterators and fallible corpus
draws (`Result<Sample, SampleError>`) without hiding replay errors;
and a zero weight is rejected downstream by `SourceMixer::new`,
alongside the other sources — `WeightedSource::new` itself does not
validate.

`SourceMixer<T>` is a public struct with four **private** fields: the
sources vector, the `cursor` (a `usize`), `emitted_in_turn` (a `u32`),
and `active` (a `usize`, the count of sources not yet exhausted). The
doc comment states the scheduling reasoning: each source contributes
`weight` consecutive items per turn; the seed picks the initial
source, making the stream deterministic without introducing a second
random scheduling policy; exhausted finite sources are skipped, the
mixer ends after every source is exhausted, and infinite sources such
as `CorpusSampler` make the mixed stream infinite.

- `new(sources: Vec<WeightedSource<T>>, seed: u64) -> Result<Self,
  MixError>` — an empty vector fails with `Empty`; the **first**
  zero-weight source fails with `ZeroWeight { index }` (the
  position-finding idiom). The seed is used exactly once: a freshly
  seeded `StdRng` draws the initial cursor uniformly over the source
  count, with the same `random_range` idiom as the sampler.
  `emitted_in_turn` starts at zero; `active` starts at the source
  count.
- `Iterator::next` — the item type is `T`. The discipline is weighted
  round-robin, in a loop: when `active` is zero, the stream ends.
  Otherwise poll the current source: an item increments
  `emitted_in_turn`, and when the count reaches the current source's
  weight the cursor advances before the item is returned; a `None`
  from the source marks it exhausted exactly once (decrementing
  `active`) and advances the cursor without yielding. Advancement is a
  private helper: the cursor steps cyclically modulo the source count
  and `emitted_in_turn` resets to zero. Exhaustion semantics are part
  of the contract: a finite source that is exhausted is marked once
  and skipped from then on.

The module's imports: the rand extension trait, the seeding trait, and
`StdRng`; and the error derive. The mixer knows nothing about samples,
boards, or records — decision S4's artifact is generic.

Watch the test compile and pass:

```text
cargo test -p selfplay --test read_path
```

reports exactly **10 passed, 0 ignored**.

---

## Step 4 — `src/export.rs` and `src/bin/selfplayexport.rs`: the export path (red → green, together)

Create
[`src/export.rs`](../../../../gomoku/crates/selfplay/src/export.rs) and
[`src/bin/selfplayexport.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayexport.rs).
These two milestones share one red phase and one green phase: the
chapter's eleventh test drives the compiled binary over a fabricated
one-game corpus and loads the result with the real `train` reader, so
neither half can go green alone. Declare the `export` module in
`src/lib.rs` (alphabetical, after `corpus`) and re-export
`ExportError`, `ExportManifest`, `SAMPLE_EXPORT_FORMAT_VERSION`, and
`export_corpus` before watching green. No manifest entry: Cargo
discovers the binary by its path.

**Why the binary must exist before the test compiles.** The export
test obtains the binary's path from Cargo's compile-time binary
environment variable for `selfplayexport`
(`env!("CARGO_BIN_EXE_selfplayexport")`). The `env!` macro resolves at
compile time, and Cargo sets the variable for integration tests only
when the binary target exists — so once the eleventh test is added,
`tests/read_path.rs` compiles only if
`src/bin/selfplayexport.rs` exists, just as it compiles only if the
crate root re-exports the names the tests import. The red state of
this step is therefore a compile error on two counts — the missing
`export` module items and the missing binary target — and both land
before the green watch. This is also why the export test is added
last: adding it in an earlier step would break compilation of the
whole target.

### 4.1 The export test, red

Extend `tests/read_path.rs`. New imports: the standard library's
`Command`; and, from the crate root, `ExportManifest`. `serde_json`,
`std::fs`, `train::dataset`, and `train::split` are used by path. One
test:

`export_command_writes_samples_that_the_real_train_reader_loads` —
under a fresh `TestDir`, name two subdirectories, `corpus` and
`dataset`. Write the one-record corpus with `write_corpus` over a
one-element slice of the record at shard capacity 2, expected with
"fabricated corpus writes". Run the compiled binary with `--corpus`,
`--out`, and `--shard-capacity 2`, capturing output; the spawn is
expected with "selfplayexport runs", and success is asserted with the
process's stderr in the failure message. Then verify the seam at both
ends, in this order:

1. **The samples.** Build the expectation by replaying the record at
   every ply 0–5 with `replay_sample`, in order. Load the exported
   directory through `train::dataset::read_dataset` — the very reader
   `nettrain` uses — expected with "nettrain's reader loads export".
   Assert equality between the read-back vector and the expectation:
   six canonical samples, in corpus order.
2. **The split survives the round trip.** For every read-back sample,
   assert `train::split::is_holdout` agrees with
   `position_is_holdout(&record, ply)` at its ply.
3. **The manifest.** Read `manifest.json` from the export directory,
   expected with "export manifest was written"; deserialize it as an
   `ExportManifest` through `serde_json`, expected with "export
   manifest is valid JSON". Assert `game_count == 1`,
   `sample_count == 6`, and shards exactly `["shard-000.bin",
   "shard-001.bin", "shard-002.bin"]` — six samples at capacity 2, in
   sample order.

### 4.2 `SAMPLE_EXPORT_FORMAT_VERSION`, `ExportManifest`, `ExportError`, green

`src/export.rs` opens with a module-level doc comment that states the
seam in one sentence pair: `nettrain --data` reads length-delimited
sample shards and ignores manifest details, so this module is the
integration seam and no network or training changes are needed;
exported positions stay canonical, so `nettrain` computes the Zobrist
holdout split before its existing seeded batch-time D4 transform.

`SAMPLE_EXPORT_FORMAT_VERSION` is a public `u16` constant, value 1 —
the sample-export manifest layout version, documented as independent
of the game-record format version (`GAME_RECORD_FORMAT_VERSION`): the
two are both 1 today by coincidence, not by coupling, and evolve
independently.

`ExportManifest` is a public struct deriving `Debug`, `Clone`,
`PartialEq`, `Eq`, and serde's two derives — equality is part of the
contract here, because the export test and future gates compare
manifests. Five public fields, each documented: `format_version` (a
`u16`), `game_count` (a `usize`, the number of source games),
`sample_count` (a `usize`, the number of replayed samples — one per
post-opening ply), `shards` (a vector of `String`, the shard file
names in sample order), and `workspace_version` (a `String`, the
workspace package version, shared with the `train` reader so format
drift is visible). The serialized shape, pretty-printed, has the same
fields and values as the chapter paper's example; the JSON examples
use compact arrays for readability and are not byte-exact in
whitespace — they pin the semantic schema, not `serde_json`'s pretty
output:

```json
{
  "format_version": 1,
  "game_count": 1,
  "sample_count": 6,
  "shards": ["shard-000.bin", "shard-001.bin", "shard-002.bin"],
  "workspace_version": "0.1.0"
}
```

`ExportError` is a public error enum deriving `Debug` and the error
derive — no equality: one variant carries a `SampleError` source and
one wraps `io::Error`. Four variants, in the reference's declaration
order — the vocabulary order, not the check order; the check order is
§4.3's contract — each documented:

1. `EmptyCorpus` — no source records were supplied. Display: "cannot
   export an empty corpus".
2. `ZeroShardCapacity` — shard capacity must be positive. Display:
   "shard capacity must be greater than zero".
3. `Sample` — one source position failed replay or target derivation.
   Fields: `game_index` (a `usize`), `ply` (a `usize`), and `source`
   (the `SampleError`, marked as the error source). Display: "game
   {game_index} ply {ply}: {source}".
4. `Io` — filesystem, framing, serialization, or manifest I/O failed.
   A transparent tuple variant wrapping `io::Error`, marked with the
   from-conversion attribute so the `?` operator converts I/O failures
   throughout the function.

The module's imports: the standard library's filesystem module and
`File`, `io`, and `Path`; serde's two derive macros; the error derive;
and the crate's `GameRecord`, `SampleError`, and `replay_sample`.

### 4.3 `export_corpus`, green

A public free function borrowing a slice of `GameRecord`, an output
directory path, and a `shard_capacity: usize`, returning
`Result<ExportManifest, ExportError>`. The doc comment states: shards
use exactly the same bincode framing and `Sample` serialization as
`train::dataset::read_dataset` reads; the manifest carries game-export
provenance rather than synthetic tactical quotas; and any non-empty
output directory is refused before files are changed. The `# Errors`
section states that replay validation finishes before the output
directory is modified. Behavioral contract, in this exact order — the
ordering is the robustness story:

1. **Validate the inputs.** An empty record slice fails with
   `EmptyCorpus`; a zero capacity fails with `ZeroShardCapacity`.
2. **Replay every position before touching the disk.** Iterate the
   records with their zero-based game indices and, within each, every
   post-opening ply; `replay_sample` derives the canonical sample, and
   the first failure becomes `ExportError::Sample { game_index, ply,
   source }`, aborting the export. Collect the samples in corpus
   order. When this step finishes, every position in the corpus has
   been replayed and no partial dataset can exist.
3. **Refuse a non-empty output directory.** Read the directory: if it
   exists and holds any entry, fail with an `AlreadyExists` I/O error
   whose message is exactly "refusing to export samples into a
   non-empty directory", converted through the `Io` variant; if it
   does not exist, proceed; any other read failure propagates. An
   existing *empty* directory is accepted. Unlike the corpus writer,
   the export offers no opt-in overwrite: an export is cheap to redo,
   so refusal has no escape hatch. Then create the directory,
   parents included.
4. **Write the shards.** Chunk the samples by `shard_capacity` and
   enumerate the chunks; each shard's name is `shard-{index:03}.bin`
   (zero-padded to three digits), written through
   `train::storage::write_shard` — chapter 02's shared extraction,
   which opens every shard create-new and frames each record with its
   little-endian length prefix. Collect the names in write order:
   sample order is corpus order, game by game, ply by ply.
5. **Write the manifest.** Assemble the `ExportManifest`: the format
   version constant, the record count, the sample count, the shard
   names, and the workspace version from Cargo's compile-time package
   version environment variable. Open `manifest.json` through the
   file-options builder with write access and **create-new**
   semantics, and serialize with `serde_json`'s pretty writer, mapping
   a serialization failure into an `InvalidData` I/O error. Return the
   manifest.

Note what the export never writes: no D4 variants, no holdout labels.
The shards stay canonical; `nettrain` computes the split on load and
applies its own seeded transform per sample within each batch. Export
order is fully determined by the corpus and the framing is fixed, so
the same corpus exports the same bytes — the determinism the
chapter-07 byte-identity gates rely on.

### 4.4 The binary: `USAGE`, `Args`, `parse_args`, green

`src/bin/selfplayexport.rs` opens with a one-line module doc comment
naming the binary's job: convert a self-play game corpus into a
`nettrain --data` sample dataset. Imports: the standard library's
`env`, `PathBuf`, and `process`; and, from the crate root,
`export_corpus` and `read_corpus` — the binary speaks crate-root names
only. `train::dataset` is used by path; the crate's `train` dependency
covers it.

`USAGE` is a private string constant with exactly the five lines the
chapter paper pins:

```text
Usage: selfplayexport --corpus <DIR> --out <DIR> [--shard-capacity <usize>]
Default: --shard-capacity 4096
Writes one canonical train::Sample per post-opening ply plus manifest.json.
nettrain --data <DIR> reads the shards directly and applies seeded D4 at batch time.
Use `--flag value` only; `--flag=value` is not supported.
```

`Args` is a private struct — **no derives**; unlike chapter 05's
`selfplayanchors`, nothing here compares or debug-prints the parsed
flags — holding three fields: `corpus: PathBuf`, `out: PathBuf`, and
`shard_capacity: usize`.

`parse_args` is a private function consuming the argument iterator
(owned strings) and returning `Result<Args, String>` — no error type,
because the binary reports strings. Hand-rolled, in the style of
`selfplaygen` and `selfplayanchors`: no dependency. Behavioral
contract — note the one deliberate difference from chapter 05's
parser, which consumed the value before matching the flag: this parser
**matches the flag first** and consumes the following argument inside
each arm, so a trailing unknown flag reports the unknown flag, not a
missing value:

1. The corpus and output slots start empty; the capacity starts at
   4096 — the `train` crate's `SHARD_SIZE`, the synthetic datasets'
   shard capacity.
2. Per argument, match the flag name: `--corpus` and `--out` consume
   the following argument as the flag's path value, a missing value
   failing with "--corpus requires a value" or "--out requires a
   value"; `--shard-capacity` consumes the following argument (a
   missing value fails with "--shard-capacity requires a value") and
   parses it as `usize`, a parse failure failing with "invalid
   --shard-capacity '{value}': {error}"; anything else fails with
   "unknown flag: {arg}".
3. After the loop, a zero capacity fails with "--shard-capacity must
   be greater than zero" — the binary rejects it before
   `export_corpus`'s `ZeroShardCapacity` ever runs. Then an empty
   corpus slot fails with "--corpus is required" and an empty output
   slot with "--out is required".

The vocabulary is two required flags and one option, values as
separate arguments only; `--flag=value` is not supported.

### 4.5 `run` and `main`, green

`run` is a private function consuming the `Args` and returning
`Result<(), String>`. Four steps, in execution order:

1. Read the corpus with chapter 02's `read_corpus`; a failure maps to
   "failed to read corpus: {error}".
2. Export with `export_corpus`, passing the records, the output
   directory, and the capacity; a failure maps to "failed to export
   samples: {error}" — the `ExportError`'s display carries the game
   index, the ply, or the I/O detail.
3. **Read the export back through the consumer's reader.** Load the
   output directory with `train::dataset::read_dataset`; a failure
   maps to "failed to read back exported samples: {error}". If the
   read-back count differs from the manifest's `sample_count`, fail
   with "read-back sample count {read} differs from manifest
   {count}". The exporter verifies its own output through the very
   reader `nettrain` uses before it reports success, so a framing or
   ordering defect fails the export command itself rather than some
   later training run.
4. Report on stderr, exactly: "Exported {samples} samples from
   {games} games into {shards} shards at {dir}" — the shard count from
   the manifest's shard list, the directory displayed.

`main` owns the exit codes, in the same shape as chapter 05's binary.
If any raw process argument is `--help` or `-h`, print the usage to
stdout and return — exit code 0; the check runs before parsing, so
`--help` works even alongside invalid flags. Otherwise parse the
arguments after the program name; on a parse failure, print the
message and the usage to stderr and exit with code 2. On a run
failure, print "selfplayexport failed: {error}" to stderr and exit
with code 1. Success falls off the end — exit code 0.

The binary carries **no** `#[cfg(test)]` module: like
`selfplayanchors`, its whole specification lives in the integration
target — here `tests/read_path.rs`, not `tests/binary.rs`. This is why
the chapter's `src` unit tests remain at 6.

Watch the eleventh test compile and pass:

```text
cargo test -p selfplay --test read_path
```

reports exactly **11 passed, 0 ignored** — the chapter's complete new
surface. The reference runs the whole target in about 0.2 s in debug:
no test plays a game, and the export test drives a one-game fabricated
corpus.

---

## Step 5 — export audit, regression, gates, capstone

### 5.1 The export audit

This step is the final audit, not the first appearance of exports —
each step landed its re-export group before its green watch. The crate
root now declares eleven modules — `anchor`, `corpus`, `export`,
`game`, `generate`, `mix`, `opening`, `record`, `sample`, `sampler`,
`schedule`, alphabetical — behind the unchanged
`#![deny(missing_docs)]`, and re-exports exactly the chapter-06
surface alongside the chapter-05 surface:

- From `sample`: `SampleError`, `augment_sample`, `position_is_holdout`,
  `replay_sample`.
- From `sampler`: `CorpusSampler`, `SamplerError`.
- From `mix`: `MixError`, `SourceMixer`, `WeightedSource`.
- From `export`: `ExportError`, `ExportManifest`,
  `SAMPLE_EXPORT_FORMAT_VERSION`, `export_corpus`.
- From `game`, `record`, `opening`, `corpus`, `anchor`, `generate`,
  `schedule`: nothing new.

Deliberate absences. The private helpers `replay_board` and `stones`
stay private in `sample.rs`; `CorpusSampler`'s, `WeightedSource`'s,
and `SourceMixer`'s fields and the mixer's `advance` helper stay
private; the binary's `USAGE`, `Args`, `parse_args`, `run`, and `main`
live in the binary and are never exported; `train::storage` stays
where chapter 02 put it — the shared extraction this chapter merely
consumes. No `selfplayverify` binary or verification module exists;
`tests/binary.rs` is unchanged at three tests. If any test reached
for a path deeper than the crate root, fix the test to use these
re-exports.

### 5.2 Regression

The chapter-05 targets stay green unchanged — game 5/2, corpus 10/0,
openings 5/0, schedule 1/1, dedup 1/0, grading 0/1, anchors 4/0,
binary 3/0, the `src` unit tests 6/0 — because the chapter only reads
what earlier chapters wrote: the record format, the corpus mechanics,
and the runner are untouched, and no earlier test imports the new
vocabulary. `cargo test -p train`, `cargo test -p player`, and
`cargo test -p net` are not required: all three crates are untouched.

No `--release` run is chapter-06 work. Nothing in this chapter plays
a game: every test fabricates its records or exports a one-game
corpus in debug, no test is added to the MANUAL set, and the four
ignored tests remain exactly chapter 04's set. The release-only,
game-playing verification discipline of chapter 04 is untouched, and
the capstone below needs no release build either. A `--release` flag
belongs in a chapter-06 command only if some future instruction
explicitly requires it — none does.

### 5.3 The standing gates

From `gomoku/`:

```text
cargo test -p selfplay
cargo test -p selfplay --test read_path
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the fast gate, and its report needs a per-target
reading: cargo prints one report per test target, never one aggregate
line. The chapter's closing state is the sum — game 5 passed / 2
ignored, corpus 10/0, openings 5/0, schedule 1/1, dedup 1/0, grading
0/1, anchors 4/0, binary 3/0, the `src` unit tests 6/0 (the library
target's two tracker tests plus the `selfplaygen` binary's four; the
new `selfplayexport` binary carries no unit tests), and the new
`tests/read_path.rs` 11/0 — **46 passed, 4 ignored** in total. The
chapter-05 state was 35 passed, 4 ignored; this chapter adds exactly
the eleven read-path tests and nothing else. The completed reference's
full suite is larger (50 passed, 4 ignored) only because chapter 07
adds its four `selfplayverify_*` tests; do not generalize that number
into this chapter's expectation. The focused selection runs the
chapter's surface in isolation, including the export test, which
compiles and drives the real `selfplayexport` binary and the real
`train::dataset` reader. The last two commands are the project's
standing quality gates; both must be green with no changes.

### 5.4 The capstone: export a corpus you own, read it back

Export an existing learner corpus — the one your chapter-04 or
chapter-05 capstone generated — into a temporary output directory and
watch the seam work end to end. Nothing here plays games, so the
debug build suffices; `mktemp -d` keeps every write out of the
repository's `data/` tree. From `gomoku/`:

```text
cargo build -p selfplay

dir=$(mktemp -d)

target/debug/selfplayexport \
  --corpus <path-to-your-corpus> \
  --out "$dir/export"

cat "$dir/export/manifest.json"
```

The stderr line reports the shape of your dataset — "Exported N
samples from G games into S shards at …" — and the manifest on disk
says the same thing as JSON: `game_count`, `sample_count`, and the
shard names in sample order. The read-back already happened before
that line printed: the binary loaded its own export through
`train::dataset::read_dataset`, the reader `nettrain` uses, and
checked the count against the manifest.

Two boundary experiments complete the picture. Re-run the export with
the same `--out`:

```text
target/debug/selfplayexport \
  --corpus <path-to-your-corpus> \
  --out "$dir/export"
```

The run fails: stderr carries "refusing to export samples into a
non-empty directory" and the exit code is 1 — an export never
overwrites a dataset by accident, so point each export at a fresh
directory. Then export the same corpus into a second fresh directory
and compare the two exports (a file-comparison tool over the shard
pairs): the same corpus yields the same shards, because export order
is corpus order and nothing in the path uses randomness.

**Do not re-run the training smoke.** The end-to-end training seam —
game records in, training steps out — was measured in the reference's
acceptance run and is quoted in the chapter paper's integration smoke
evidence (from the reference worktree's `acceptance-evidence.md` §5
and the
[hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md)):
on the 48-game baseline-32 corpus, the export produced **1 860
samples in 4 shards** at capacity 512 in 0.18 s, and the unmodified
`nettrain` tiny configuration (8 channels, 1 residual block, batch 16)
trained 50 steps on them, splitting **1 665 train / 195 holdout** and
moving train loss from **6.5483584 at step 1 to 6.252371 at step 50**
(step-50 holdout loss 6.1449027). Those numbers are the reference's
measured evidence that the seam holds; they are a smoke result, not a
model-quality or playing-strength claim, and reproducing the training
run is chapter 07's acceptance work, not this capstone's.

---

## Pitfalls

Each pitfall below is a defect an independent implementation is likely
to produce; each names the test or gate that catches it. Skim this
list before starting step 1, and again before the gates.

1. **Side to move from ply parity.** Swap2 answers decide which color
   moved first, so parity is wrong whenever the answer changes the
   order. Take the side to move from the replayed board. Pinned by
   `replay_at_ply_has_opening_and_played_stones_with_the_right_side_to_move`
   (the fixture's take-Black answer makes White the first mover).
2. **π from the recorded prior, or z from the recorded root value.**
   The prior is a truncated, noise-perturbed audit record; the root
   value is the recording seat's interim guess, absent on bypassed
   plies. π is the normalized visit histogram; z is the game outcome
   from the sampled side to move — the AlphaGo Zero discipline
   **[paper]** (Silver et al. 2017). Pinned by
   `raw_visits_become_a_sparse_normalized_policy` and
   `outcome_value_uses_each_position_side_to_move_perspective`.
3. **An empty histogram treated as an error, or patched with a
   fabricated distribution.** An unsearched ply is the honest case:
   the one-hot on the played move is deliberate behavior cloning of a
   machine-certified move. Pinned by
   `a_ply_without_search_uses_the_played_move_as_its_policy`.
4. **Dividing by a zero visit total.** A nonempty all-zero histogram
   must fail with `SampleError::ZeroVisitTotal` *before* any division;
   a NaN policy would surface three crates away as a training
   divergence. Pinned by
   `nonempty_zero_visit_histogram_is_rejected_instead_of_producing_nan`.
5. **Transforming stones but not the policy, or transforming the game
   facts.** One transform moves both stone lists and every policy move
   together; `to_move` and `value` are D4-invariant. Pinned by
   `d4_augmentation_transforms_stones_and_policy_together`, including
   the rotation direction (`Rot90(0,0) == (14,0)`).
6. **The holdout rule applied after augmentation.** Zobrist keys are
   orientation-specific; hashing a randomly rotated position could
   flip its partition per draw. The rule runs on the canonical
   position, and `CorpusSampler` is a training-stream artifact, never
   a split source. Pinned by
   `holdout_uses_the_canonical_position_zobrist_and_train_partition`
   and stated in the sampler's doc comment.
7. **Stone lists in play order.** The sample's lists are enumerated
   from the board by color in row-major board order; play order fails
   test 1's exact-list assertions.
8. **A different draw order in the sampler.** The determinism contract
   is one seeded stream consumed in the fixed order game slot → ply →
   transform. Any other order — or a second RNG — yields a different
   stream under the same seed. Pinned by
   `corpus_sampler_is_deterministic_under_one_seed`'s 64-draw equality.
9. **Random weighted choice in the mixer.** A per-draw random choice
   matches the weights only in expectation. The schedule is weighted
   round-robin — `weight` consecutive items per turn, seeded initial
   source — and the weights are met exactly per cycle. Pinned by
   `source_mixer_respects_weights_over_a_large_seeded_draw`'s exact
   1 000 / 3 000 counts.
10. **Writing before validating.** Every position in the corpus
    replays before the output directory is modified; a replay failure
    leaves no partial dataset. And a non-empty output directory is
    refused outright, with no opt-in — stricter than the corpus
    writer. Pinned by the validate-then-write ordering in §4.3 and
    exercised by the capstone's rerun experiment.
11. **Forgetting create-new semantics.** `train::storage::write_shard`
    opens shards create-new, and the manifest opens create-new through
    the file-options builder; both conversions flow through the `Io`
    variant's from-attribute.
12. **Encoding planes into the sample.** `train::Sample` stores
    stones, never planes; the 17×17 border is added exactly once,
    inside `net`'s batcher, and the engine's proven encode–transform
    commutation guarantees equivalence. An export that encoded planes
    would freeze one encoding into the corpus — decision S2 forbids
    it.
13. **rand API drift.** The manifest pins `rand = "0.10.3"`: the
    ranged-random method is `random_range` from the `RngExt` trait,
    and `StdRng` lives under `rand::rngs`. The `gen_range` / `Rng`
    spelling of older releases does not compile here.
14. **Missing doc comments.** `src/lib.rs` keeps
    `#![deny(missing_docs)]`; every new public item — modules, types,
    variants, fields, constants, functions — needs a doc comment, or
    the build fails before any test runs.
15. **A twelfth test.** The chapter specifies exactly eleven tests,
    all default-run, none MANUAL. Resist pinning extras — no help-text
    test for the binary (its command line is pinned by the export test
    itself), no `ExportManifest` round-trip unit test, no split-source
    test for the sampler. `tests/binary.rs` gains nothing.

---

## Done when

- `cargo test -p selfplay` reports **46 passed, 4 ignored** as the sum
  of its per-target reports: game 5 passed / 2 MANUAL-ignored
  (unchanged), corpus 10/0 (unchanged), openings 5/0 (unchanged),
  schedule 1/1 (unchanged), dedup 1/0 (unchanged), grading 0/1
  (unchanged), anchors 4/0 (unchanged), binary 3/0 (unchanged — no
  chapter-06 tests), the `src` unit tests 6/0 (unchanged — the new
  binary carries none), and `tests/read_path.rs` 11/0 (new). The
  chapter-05 state of 35/4 plus exactly eleven new default tests.
- `cargo test -p selfplay --test read_path` reports **11 passed, 0
  ignored**: the seven replay-and-target tests, the two sampler tests,
  the mixer test, and the export test through the compiled binary and
  the real `train::dataset` reader.
- No new MANUAL test exists, and nothing in this chapter plays a game:
  every test fabricates its records or exports a one-game corpus in
  debug. No `--release` run is chapter-06 work; the capstone builds
  and runs in debug.
- `cargo test -p train`, `cargo test -p player`, and
  `cargo test -p net` are not required: all three crates are
  untouched.
- `cargo clippy --all-targets -- -D warnings` is green.
- `cargo fmt --all -- --check` makes no changes.
- The capstone exported an existing learner corpus into a `mktemp`
  directory, its stderr line and manifest agreed, the rerun against
  the same output failed with the non-empty-directory refusal and exit
  code 1, and two fresh exports of the same corpus produced identical
  shards.
- The commit message is
  `feat(selfplay): reading back — selfplay-corpora chapter 06`.

## Verification commands

From `gomoku/`:

```text
cargo test -p selfplay
cargo test -p selfplay --test read_path
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the fast gate: this chapter adds nothing slow to
it (the reference runs the whole new target in about 0.2 s in debug).
The focused selection runs the chapter's surface in isolation — the
replay, augmentation, holdout, sampler, and mixer tests, plus the
export test, which compiles and drives the real `selfplayexport`
binary over a fabricated one-game corpus and loads the result with the
real `train::dataset` reader. No `--release -- --ignored` run is
chapter-06 work: game-playing verification stays MANUAL and
release-only, exactly as chapter 04 left it.

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `selfplay/src/sample.rs` | `SampleError` | four variants in declaration order — `PlyOutOfBounds { ply, ply_count }`, `ZeroVisitTotal { ply }`, `Opening(PlayError)`, `Replay { ply, mv, source }` — with the exact displays; `Debug` + the error derive only |
| `selfplay/src/sample.rs` | `replay_sample` | record + ply → `Result<Sample, SampleError>`; position immediately before the ply, canonical; bounds → opening → prefix replay through a private helper; π from the normalized visits in recorded order, one-hot on the played move when empty (with the behavior-cloning comment), `ZeroVisitTotal` before any division; z from the outcome against the board's side to move; stones enumerated row-major by color |
| `selfplay/src/sample.rs` | `augment_sample` | must-use; both stone lists and every policy move through `transform_move`; `to_move` and `value` carried unchanged |
| `selfplay/src/sample.rs` | `position_is_holdout` | replay + `train::split::is_holdout` on the canonical position; the doc bars post-augmentation evaluation and points `Sample` owners at `train::split::is_holdout` |
| `selfplay/src/sampler.rs` | `SamplerError` | one variant `EmptyCorpus` with the exact display; `Debug`, the error derive, `PartialEq`, `Eq` |
| `selfplay/src/sampler.rs` | `CorpusSampler` | three private fields; `new` filters to games with plies and fails fast; `from_corpus` composes `read_corpus` and maps emptiness to `InvalidData`; `next` draws game slot → ply → transform from one seeded `StdRng` and replays on demand; items are `Result<Sample, SampleError>`; the doc bars split duty |
| `selfplay/src/mix.rs` | `MixError` | `Empty` and `ZeroWeight { index }` with the exact displays; equality derives |
| `selfplay/src/mix.rs` | `WeightedSource<T>` | generic; `new` boxes without validating; zero weights rejected by the mixer |
| `selfplay/src/mix.rs` | `SourceMixer<T>` | four private fields; `new` rejects empty then the first zero weight, seeds only the initial cursor; `next` is weighted round-robin — `weight` consecutive items per turn, exhaustion marked once and skipped, `None` when all exhausted |
| `selfplay/src/export.rs` | `SAMPLE_EXPORT_FORMAT_VERSION`, `ExportManifest` | the `u16` constant 1, independent of the game-record version; five documented fields; `Debug`, `Clone`, `PartialEq`, `Eq`, serde's two derives |
| `selfplay/src/export.rs` | `ExportError` | four variants in declaration order — `EmptyCorpus`, `ZeroShardCapacity`, `Sample { game_index, ply, source }`, `Io` (transparent, from-attribute) — with the exact displays |
| `selfplay/src/export.rs` | `export_corpus` | validate inputs → replay every position → refuse a non-empty directory ("refusing to export samples into a non-empty directory") → create directory → chunks to `shard-NNN.bin` via `train::storage::write_shard` → create-new pretty manifest; no D4 variants, no holdout labels |
| `selfplay/src/bin/selfplayexport.rs` | `USAGE`, `Args`, `parse_args` | the five-line usage verbatim; no derives on `Args`; flag matched before its value is consumed; default capacity 4096; zero capacity and missing required flags rejected after the loop; unknown flag reported as such |
| `selfplay/src/bin/selfplayexport.rs` | `run`, `main` | read → export → read back through `train::dataset::read_dataset` → count check → stderr report; help → 0, parse failure → message + usage → 2, run failure → "selfplayexport failed: …" → 1; no unit-test module |
| `selfplay/tests/read_path.rs` | eleven tests + `TestDir`, `mv`, `six_ply_record` | the exact names, fixtures, and assertions of §§1.1, 2.1, 3.1, 4.1; all default-run; no twelfth test; crate-root imports only |
| `selfplay/src/lib.rs` | four new modules, four re-export groups | alphabetical positions; `#![deny(missing_docs)]` unchanged; re-exports land before each step's green watch; helpers and binary internals stay private |
| untouched | `Cargo.toml`, `tests/binary.rs`, `train`, `engine`, `net`, `player`, chapters 01–05 sources | no manifest changes; the binary target stays at 3 tests; `train::storage` is chapter 02's extraction, consumed unchanged |

## References

**This chapter:**

- [reading-back.md](reading-back.md) — the chapter paper: intention,
  specification as tests, execution-order walkthrough, the read-path
  contract, and the integration smoke evidence.
- [reading-back-design.md](reading-back-design.md) — the conceptual
  story: why games stay canonical and everything else is a read-time
  decision; the two-format ownership table; the ten invariants this
  plan builds under.
- [reading-back-solution.md](reading-back-solution.md) — the
  annotated reference
  snippets. Open only after twenty stuck minutes, or afterwards to
  compare.

**This track:**

- [Tutorial README](../README.md) — conventions, gates, the chapter
  map, and the reference provenance.
- [Self-play primer](../00-selfplay-primer.md) — §4.6 states this
  chapter's contract; the invariants list fixes what the read path
  must not drift from.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md)
  and its [implementation plan](../02-recording-games/recording-games-implementation.md)
  — the `GameRecord` format, the `read_corpus` / `write_corpus`
  mechanics, the `train::storage` extraction this chapter writes
  through, and the step-0 manifest additions (`serde_json`, `train`)
  that make this chapter manifest-free.
- [Chapter 03 — recording the search](../03-recording-the-search/recording-the-search.md)
  and its [implementation plan](../03-recording-the-search/recording-the-search-implementation.md)
  — what the recorded visits, priors, and root values mean, and the
  honesty rule for unsearched plies.
- [Chapter 05 — certified endgames](../05-certified-endgames/certified-endgames.md)
  and its [implementation plan](../05-certified-endgames/certified-endgames-implementation.md)
  — why unsearched plies exist, the replay-from-primary-evidence
  discipline, the `TestDir` idiom, and the binary style (whose
  value-first parser step 4.4 deliberately does *not* copy).
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  decision S2 (store games, not positions), decision S3 (the per-ply
  payload and its audit fields), §3.3 (the read path), decision S4
  (source mixing).
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  this chapter's public surface and the measured integration smoke
  numbers.
- [docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) —
  §7 the D4 group, the orientation-specific Zobrist keys, and the
  exact encode–transform commutation; §9 the store-games replay
  discipline; the claim-label convention.
- [docs/14-openings-and-supervised-curriculum.md](../../../14-openings-and-supervised-curriculum.md) —
  decision D6, the mixture the `SourceMixer` serves.

**Prerequisite tracks:**

- [Datagen tutorial](../../datagen-tutorial/README.md) — the `train`
  crate: `Sample` (stones, never planes; `Sample::board` rebuilds the
  engine position), the shard mechanics, the dataset reader, and the
  Zobrist-keyed holdout split.
- [Net tutorial](../../net-tutorial/README.md) — `nettrain`, the
  consumer: canonical shards in, split on load, one seeded D4
  transform per sample within each batch.
- [Engine tutorial](../../13-engine-tutorial/README.md) — the
  bitboard `Board`, `Board::zobrist`, the D4 `Transform`s, and the
  encode–transform commutation.

**Source (live on `main`, consumed unchanged):**

- [`gomoku/crates/train/src/sample.rs`](../../../../gomoku/crates/train/src/sample.rs) —
  `Sample`: the five fields, the equality derive the tests rely on,
  and `Sample::board`.
- [`gomoku/crates/train/src/split.rs`](../../../../gomoku/crates/train/src/split.rs) —
  `is_holdout` and `HOLDOUT_PERCENTAGE`: the content-based partition.
- [`gomoku/crates/train/src/dataset.rs`](../../../../gomoku/crates/train/src/dataset.rs) —
  `read_dataset`: the reader the exporter reads back through and
  `nettrain` trains from.
- [`gomoku/crates/train/src/shard.rs`](../../../../gomoku/crates/train/src/shard.rs) —
  `SHARD_SIZE` (4096), the export binary's default shard capacity.
- [`gomoku/crates/engine/src/symmetry.rs`](../../../../gomoku/crates/engine/src/symmetry.rs) —
  `Transform`, `Transform::ALL`, and `transform_move`.
- [`gomoku/crates/engine/src/encode.rs`](../../../../gomoku/crates/engine/src/encode.rs) —
  the 17×17 border-padded encoding and the proptest
  `encode_commutes_with_every_transform` the stones-not-planes
  discipline relies on.
- [`gomoku/crates/engine/src/board.rs`](../../../../gomoku/crates/engine/src/board.rs) —
  `Board::play`, `to_move`, `stone_at`, and `zobrist`: the replay
  target's surface.
- [`gomoku/crates/net/src/bin/nettrain.rs`](../../../../gomoku/crates/net/src/bin/nettrain.rs) —
  the consumer's load path: `read_dataset` in, `is_holdout` on load.
  Untouched.

**Source (created by earlier chapters of this track; lands on `main`
as the track is worked):**

- `gomoku/crates/selfplay/src/record.rs`,
  `gomoku/crates/selfplay/src/game.rs`,
  `gomoku/crates/selfplay/src/opening.rs`,
  `gomoku/crates/selfplay/src/corpus.rs` — the record, game, opening,
  and corpus vocabulary this chapter consumes. Chapters 01–05's, not
  touched in this chapter.
- `gomoku/crates/train/src/storage.rs` — the shared shard mechanics
  (`write_shard`, `read_shard`, create-new) the export writes through.
  Chapter 02's extraction; not touched in this chapter.

**Source (in the read-only reference worktree; lands on `main` in
this chapter):**

- `gomoku/crates/selfplay/src/sample.rs` — new: `SampleError`,
  `replay_sample`, `augment_sample`, `position_is_holdout`, the
  private `replay_board` and `stones`, and the behavior-cloning
  comment. Step 1.
- `gomoku/crates/selfplay/src/sampler.rs` — new: `SamplerError` and
  `CorpusSampler`, including the doc comment that bars the sampler
  from split duty. Step 2.
- `gomoku/crates/selfplay/src/mix.rs` — new: `MixError`,
  `WeightedSource`, `SourceMixer`, including the
  deterministic-schedule doc comment. Step 3.
- `gomoku/crates/selfplay/src/export.rs` — new:
  `SAMPLE_EXPORT_FORMAT_VERSION`, `ExportManifest`, `ExportError`,
  `export_corpus`, including the module comment that states the
  `nettrain` seam. Step 4.
- `gomoku/crates/selfplay/src/bin/selfplayexport.rs` — new: the export
  binary with the read-back self-check. Step 4.
- `gomoku/crates/selfplay/src/lib.rs` — declares the four new modules
  and re-exports the new vocabulary. Every step.
- `gomoku/crates/selfplay/tests/read_path.rs` — the read-path
  specification: all eleven tests. Grown across steps 1–4.
- `gomoku/crates/selfplay/tests/binary.rs` — unchanged this chapter;
  gains the `selfplayverify` tests in chapter 07. Not touched.
- [`gomoku/crates/selfplay/Cargo.toml`](../../../../gomoku/crates/selfplay/Cargo.toml) —
  no manifest changes.

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). <https://doi.org/10.1038/nature24270> —
  the (s, π, z) training record: policy from search visit counts,
  value from the game outcome, board symmetries as augmentation.
- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  <https://doi.org/10.26481/dis.19940923la> — threat-space search, the
  prover behind the TSS bypasses whose plies carry no visits.
