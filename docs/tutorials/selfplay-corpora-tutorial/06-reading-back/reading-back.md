# Chapter 06 — Reading back

## Abstract

This chapter turns durable game records into deterministic training
and validation data, without baking any augmentation into the stored
games. Chapters 01–05 wrote corpora of `GameRecord`s — one complete
game per record, opened by Swap2 (the opening ceremony that fixes
the initial stones and assigns the colors), with per-ply visit
counts, sparse priors, root-value estimates, the outcome, and
optional certified suffixes.
This chapter builds the read path back. First, `replay_sample`
replays one record's opening and its first `ply` moves through a
fresh engine board and derives a `train::Sample` at that position:
the stone lists and side to move come from the replay, the policy
target π is the ply's normalized visit distribution (a one-hot on
the played move when no search ran), and the value target z is the
game outcome from the side to move's perspective. Second,
`augment_sample` applies one D4 transform — one of the eight
rotations and reflections of the square board — to the stones and
the policy together, leaving side to move and value invariant; the
stored games stay canonical, so augmentation is a read-time choice,
not a write-time commitment. Third, `position_is_holdout` partitions
positions with the synthetic datasets' rule — the canonical
position's Zobrist key modulo 100 below 10 — evaluated before any
transform, because a rotated board hashes differently and a random
orientation must never move a position between training and holdout.
Fourth, the `CorpusSampler` yields an infinite seeded stream of
random game → ply → transform draws, and the `SourceMixer` combines
weighted sample streams under one seed. Fifth, `export_corpus`
materializes one canonical sample per post-opening ply as ordinary
`train`-format shards plus a manifest, and the `selfplayexport`
binary drives it: `selfplayexport --corpus <DIR> --out <DIR>
[--shard-capacity <N>]`. The exported shards are exactly what
`nettrain --data` already reads — no network or training change.
The conclusions: the read path is deterministic end to end (same
corpus, same shards; same seed, same sample stream); the export is
canonical, so the existing holdout split stays comparable across the
synthetic and the self-play sources; and the seam holds in practice —
the reference's acceptance run exported 1 860 samples from 48 games
into 4 shards and trained the tiny `nettrain` configuration on them
for 50 steps, with train loss moving from 6.548 to 6.252. What this
chapter deliberately does not do: it does not verify corpora for
legality or statistics (`selfplayverify` and the acceptance gates
are chapter 07), and it changes nothing about how the network
trains. The intended reader is working the selfplay-corpora tutorial
in order and has completed chapter 05.

## Glossary

| Term | Definition |
|------|------------|
| **Game record** | `GameRecord`: the durable serialization of one complete game — a header (seed, seat pairing, evaluator identity, the Swap2 opening), the ordered post-opening `plies`, the `outcome`, and an optional certified suffix. Chapter 02 fixed the format. |
| **Recorded ply** | `RecordedPly { mv, visits, prior, root_value }`: one played post-opening move and its durable search data. `visits` holds sparse raw root visit counts, descending; `prior` holds up to 16 root priors for audit; both are empty when no search ran, and `root_value` is then `None`. |
| **Sample** | `train::Sample`: the labelled training record — `black` and `white` stone lists in absolute color, `to_move`, a sparse `policy` of `(move, probability)` pairs, and a `value` target from the side-to-move perspective. It stores stones, never planes. |
| **π (policy target)** | The normalized root visit-count distribution of the search at a ply; the policy training target (Silver et al. 2017). Computed at read time from the recorded raw counts. |
| **z (value target)** | The game outcome in {+1, 0, −1} from the perspective of the side to move at the sampled ply; the value training target. |
| **Prior** | The evaluator's policy probabilities over root moves before search, recorded sparsely (top 16) for audit. Audit data, not a training target: the read path never turns priors into a policy target. |
| **Canonical orientation** | The position exactly as played, with no D4 transform applied. Records and exported shards are canonical; augmentation happens per sample, never per batch — one transform per drawn sample in the sampler, one seeded transform per sample in `nettrain`'s batcher. |
| **D4** | The dihedral group of the square: the 8 rotations and reflections of the board, `engine::Transform::ALL`. |
| **Zobrist key** | A 64-bit hash of a board position, maintained incrementally by the engine (`Board::zobrist`). Orientation-specific: a transformed position hashes differently. |
| **Holdout split** | The train/holdout partition of the `train` crate: a sample is held out iff `zobrist(board) % 100 < HOLDOUT_PERCENTAGE` (= 10). Content-based, so collection order cannot leak correlated positions into both sides. |
| **CorpusSampler** | The seeded, infinite iterator of augmented training samples over a corpus: each draw picks a random game, a random ply, and a random D4 transform. |
| **SourceMixer / WeightedSource** | The seeded weighted round-robin over sample iterators: each `WeightedSource` contributes `weight` consecutive items per turn, and the seed picks the initial source. |
| **Shard** | A fixed-capacity file of length-delimited bincode records; the `train` crate's storage unit, shared by synthetic datasets, game corpora, and sample exports. |
| **ExportManifest** | The JSON provenance of an exported sample dataset: format version, game count, sample count, shard file names in sample order, and the workspace version. |
| **TSS bypass** | The certified own-side threat-space search (TSS; Allis 1994) proof that short-circuits a decision: the line's first move is played and no search runs. Such a ply records empty `visits` and `prior` and `root_value: None` (chapter 03's honesty rule). |

## Context

Five sources supply what this chapter combines. Each summary states
what the source delivered and why this chapter needs it.

- [Chapter 02 — recording games](../02-recording-games/recording-games.md) —
  built the durable format this chapter reads: `GameRecord`,
  `GameHeader` with the Swap2 `Opening`, `RecordedPly`, and the
  corpus mechanics `read_corpus` / `write_corpus` on the shared
  `train::storage` shard layer. The read path consumes exactly what
  the write path stored; no format change occurs here.
- [Chapter 03 — recording the search](../03-recording-the-search/recording-the-search.md) —
  fixed what the recorded search data means: raw visit counts
  (normalization into π happens at read time — this chapter is that
  read time), sparse top-16 priors for audit, root-value estimates
  from the recording side's perspective, and the honesty rule that a
  TSS-bypassed ply carries empty visits and priors and
  `root_value: None`.
- [Chapter 05 — certified endgames](../05-certified-endgames/certified-endgames.md) —
  explains why unsearched plies exist at all: a certified own-side
  proof bypasses search, and its move is played without a visit
  histogram. The read path must give such plies an honest policy
  target.
- [Datagen tutorial](../../datagen-tutorial/README.md) — built the
  `train` crate this chapter targets: the `Sample` record
  (`train/src/sample.rs`), the length-delimited shard mechanics
  (`train/src/storage.rs`), the dataset reader
  (`train::dataset::read_dataset`), and the content-based holdout
  rule (`train::split::is_holdout`,
  `train::split::HOLDOUT_PERCENTAGE`). Exported corpora are ordinary
  datasets in every sense the reader cares about.
- [Net tutorial](../../net-tutorial/README.md) — built `net` and
  `nettrain`, the consumer: `nettrain --data <DIR>` reads sample
  shards, computes the holdout split on the canonical position, and
  applies its own seeded D4 transform per sample within each batch.
  The export path is
  designed so this binary needs no modification.

The [primer](../00-selfplay-primer.md) §4.6 states this chapter's
contract: draw a random game, replay it through the engine to a
random ply, and emit position s, the normalized visit distribution π
of that ply, and the outcome z from the side to move's perspective —
plus one random D4 transform — with the train/holdout split derived
from the position's Zobrist key, the same discipline as the
synthetic sets. The
[design spec](../../../specs/2026-11-11-selfplay-corpora-design.md)
is the design authority: decision S2 (store games, not positions;
replay, encode, and apply one random D4 transform at sample time),
§3.3 (the read path), and S4 (the sampler must mix sources, hence
`SourceMixer`).

## Intention

You can turn a stored corpus into training and validation data you
can trust, reproduce, and feed to the unchanged `nettrain` binary.
Given a corpus directory, one command —
`selfplayexport --corpus <DIR> --out <DIR> [--shard-capacity <N>]` —
writes ordinary sample shards that `nettrain --data` reads directly,
plus a manifest that says how many games became how many samples in
how many shards. Given several corpora (or a corpus and the
synthetic tactical set), the `CorpusSampler` and the `SourceMixer`
give you seeded, weighted, infinite training streams whose every
draw is a replayed, correctly targeted, D4-augmented position. The
stored games never change: augmentation, sampling, and the holdout
partition are all read-time decisions, so the same corpus serves
every future experiment, and the same seed always reproduces the
same stream. And the honesty rules of the earlier chapters survive
the translation: a ply that was never searched contributes its
played move as a deliberate one-hot policy target — never a
fabricated distribution — and the recorded priors stay what they
are, audit data.

## Specification as tests

The tests live in one new target,
[`tests/read_path.rs`](../../../../gomoku/crates/selfplay/tests/read_path.rs)
under
[`gomoku/crates/selfplay/`](../../../../gomoku/crates/selfplay/).
The list below is ordered along the execution path of the feature:
replay first (board, then policy, then value), then augmentation,
then the holdout partition, then the sampling and mixing streams,
then the export through the real binary last.

This chapter specifies **eleven** new tests: **all eleven run by
default, none is ignored**. No game-playing test is added — every
test fabricates its records by hand or drives the export binary over
a one-game fabricated corpus — so the fast gate stays fast (the
reference runs the whole target in about 0.2 s in debug) and the
release-only MANUAL discipline of chapter 04 is untouched.

Two count notes, continued from chapter 05. First,
[`tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs)
gains **no** chapter-06 tests: the `selfplayexport` command line is
pinned inside `tests/read_path.rs` itself — the export test drives
the compiled binary through `env!("CARGO_BIN_EXE_selfplayexport")`
end to end — and the release-binary smoke on a real corpus is
acceptance evidence (quoted below), not a unit-test matter. The
target stays at the three tests chapter 05 left there, and the four
`selfplayverify_*` tests of chapter 07 complete it at seven. Second,
chapter 05 ended with the suite at **35 passed, 4 ignored**; this
chapter's eleven default tests bring the chapter-06 state to **46
passed, 4 ignored**. The completed reference's full suite is 50
passed, 4 ignored; the remaining 4 default tests belong to chapter
07.

All eleven tests share one fabricated record, built by
`six_ply_record()`: no game is played. The fixture's Swap2 opening
places stones at `(5, 5)`, `(10, 10)`, `(5, 6)` with the answer
"take Black" and generated provenance; the six post-opening plies
are `(10, 11)`, `(5, 7)`, `(10, 12)`, `(5, 8)`, `(10, 13)`,
`(5, 9)` — White builds a horizontal line on row 10, Black on
row 5 — and the outcome is a Black win by five-or-more. Every
searched ply carries a one-entry visit histogram on the played move,
a prior of 1.0, and a root value of 0.25; ply 5 is unsearched
(empty visits, empty prior, `root_value: None`), standing in for a
TSS-bypassed certified move. `GameRecord::from_game` converts the
fixture, so the tests exercise the same conversion the runner uses.

### Replay: the position and its side to move

- `replay_at_ply_has_opening_and_played_stones_with_the_right_side_to_move` —
  `replay_sample(&record, 3)` replays the opening and the first
  three plies. The sample's Black stones are exactly
  `[(5, 5), (5, 6), (5, 7)]` (two from the opening, one played), its
  White stones are `[(10, 10), (10, 11), (10, 12)]`, and the side to
  move is Black — ply 3 is Black's turn. Replay reconstructs the
  position from primary evidence: the opening description in the
  header plus the played moves, through a real engine board.

### Replay: the policy target

- `raw_visits_become_a_sparse_normalized_policy` — the ply-0 visit
  histogram is replaced with `[(10, 11) → 3, (0, 0) → 1,
  (14, 14) → 2]`; the sample's policy is exactly
  `[((10, 11), 0.5), ((0, 0), 1.0/6.0), ((14, 14), 1.0/3.0)]`, and
  the masses sum to 1 within 1e-6. π is the normalized visit
  distribution, sparse, in the recorded order.
- `a_ply_without_search_uses_the_played_move_as_its_policy` — ply 5
  has empty visits; its sample's policy is exactly
  `[(played_move, 1.0)]`. An unsearched ply contributes behavior
  cloning on the move that was actually played — a deliberate,
  honest one-hot — never a fabricated distribution.
- `nonempty_zero_visit_histogram_is_rejected_instead_of_producing_nan` —
  a nonempty histogram whose counts are all zero,
  `[(0, 0) → 0, (1, 1) → 0]`, cannot be normalized: replay fails
  with `SampleError::ZeroVisitTotal { ply: 0 }`. A corrupt record
  gets a named error, never a NaN policy.

### Replay: the value target

- `outcome_value_uses_each_position_side_to_move_perspective` — the
  fixture's outcome is a Black win. At ply 0 the side to move is
  White and the value is `−1.0`; at ply 1 the side to move is Black
  and the value is `+1.0`. With the outcome changed to a draw, both
  values are `0.0`. z is always from the sampled position's side to
  move, never from the recording seat's.

### Augmentation

- `d4_augmentation_transforms_stones_and_policy_together` — with the
  ply-0 visits set to `[(0, 0) → 3, (10, 11) → 1]`, the canonical
  sample under `Transform::Rot90` yields stone lists equal to the
  element-wise transform of the canonical lists, a policy of
  `[(Rot90(0, 0), 0.75), (Rot90(10, 11), 0.25)]` with
  `Rot90(0, 0) == (14, 0)`, and an unchanged side to move and value.
  One transform moves stones and policy masses together; the game
  facts — who moves, who won — are D4-invariant.

### The holdout partition

- `holdout_uses_the_canonical_position_zobrist_and_train_partition` —
  for the replayed ply-2 sample, the expected partition is computed
  by hand as
  `sample.board().unwrap().zobrist() % 100 < train::split::HOLDOUT_PERCENTAGE`;
  the test asserts that `train::split::is_holdout(&sample)` agrees,
  and that `position_is_holdout(&record, 2)` agrees with both. The
  selfplay read path uses the synthetic datasets' content-based rule
  unchanged, on the canonical position.

### The seeded sampler

- `corpus_sampler_is_deterministic_under_one_seed` — two
  `CorpusSampler`s built from the same one-record vector with seed
  42 yield identical 64-sample streams, and every drawn sample's
  policy masses sum to 1 within 1e-6. Same corpus, same seed, same
  stream — and augmentation never breaks the policy invariant.
- `corpus_sampler_loads_a_corpus_directory` — `write_corpus` writes
  the one-record corpus to a temporary directory (shard capacity 2),
  `CorpusSampler::from_corpus(dir, 42)` loads it, and four draws
  replay successfully. The directory-backed constructor is the same
  stream over shard-read records. The temporary directory cleans
  itself up on drop.

### The weighted mixer

- `source_mixer_respects_weights_over_a_large_seeded_draw` — two
  infinite marker sources, weights 1 and 3, seed 7: over 4 000
  draws, exactly 1 000 come from the weight-1 source and exactly
  3 000 from the weight-3 source. The mix is not a random
  distribution that merely approaches the weights; it is a
  deterministic weighted round-robin that matches them exactly over
  a full cycle.

### Export through the real binary

- `export_command_writes_samples_that_the_real_train_reader_loads` —
  `write_corpus` writes the one-record corpus; the compiled
  `selfplayexport` runs with `--corpus`, `--out`, and
  `--shard-capacity 2`, and exits successfully. The exported
  directory then loads through `train::dataset::read_dataset` — the
  very reader `nettrain` uses — and equals the six samples that
  `replay_sample` derives for plies 0–5, in order. Every read-back
  sample's `train::split::is_holdout` agrees with
  `position_is_holdout(&record, ply)` for its ply. The manifest
  parses as an `ExportManifest` with `game_count == 1`,
  `sample_count == 6`, and shards exactly
  `["shard-000.bin", "shard-001.bin", "shard-002.bin"]` — six
  samples at capacity 2. The seam is proven at both ends: the binary
  writes, the real reader loads, and the split survives the round
  trip.

## Walkthrough

The order is the execution order of one position travelling from a
stored record to a training batch. Four source files are new —
[`src/sample.rs`](../../../../gomoku/crates/selfplay/src/sample.rs),
[`src/sampler.rs`](../../../../gomoku/crates/selfplay/src/sampler.rs),
[`src/mix.rs`](../../../../gomoku/crates/selfplay/src/mix.rs), and
[`src/export.rs`](../../../../gomoku/crates/selfplay/src/export.rs)
— plus the binary
[`src/bin/selfplayexport.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayexport.rs);
[`src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs)
re-exports the new vocabulary
(`SampleError`, `replay_sample`, `augment_sample`,
`position_is_holdout`, `SamplerError`, `CorpusSampler`, `MixError`,
`SourceMixer`, `WeightedSource`, `ExportError`, `ExportManifest`,
`SAMPLE_EXPORT_FORMAT_VERSION`, `export_corpus`). No other crate is
touched: the chapter only reads `engine`, `train`, and the crate's
own chapters 01–05 vocabulary.

### 1. A corpus directory becomes records

Everything starts from chapter 02's mechanics: `read_corpus` reads
the manifest and decodes every shard into `Vec<GameRecord>`. The
in-memory constructors (`CorpusSampler::new`, `export_corpus`) take
records directly, so tests never touch the disk for the derivation
logic; the directory-backed constructor
(`CorpusSampler::from_corpus`) is a thin composition of
`read_corpus` and `new`, with an empty corpus surfaced as
`io::ErrorKind::InvalidData`.

### 2. Replay rebuilds the position

`replay_sample(record, ply)` yields the sample for the position
*immediately before* `ply` is played. A `ply` beyond the record's
plies fails with `SampleError::PlyOutOfBounds { ply, ply_count }`.
Replay starts from the header's Swap2 description —
`record.header.opening.board()` rebuilds the post-ceremony board
(`SampleError::Opening` on rejection) — and plays the first `ply`
recorded moves through it (`SampleError::Replay { ply, mv, .. }` on
the first illegal move). The engine board is the only position
representation the read path trusts: stone lists for the sample are
enumerated from the board by color, in board order, and the side to
move comes from the board, not from ply parity — Swap2 answers
decide who moved first, and the board already knows.

### 3. The recorded search becomes π, the outcome becomes z

With the position in hand, the two targets derive from the record at
the same ply. The policy target: the recorded visit histogram —
sparse raw counts, descending, exactly as chapter 03 stored them —
is normalized to a probability mass per move. A nonempty histogram
whose counts sum to zero is rejected
(`SampleError::ZeroVisitTotal`), because a corrupt record deserves a
named error, not a NaN. An *empty* histogram is the honest case: no
search ran (the TSS bypass of chapters 03 and 05), so the policy is
the one-hot `[(played_move, 1.0)]` — deliberate behavior cloning of
a machine-certified move, with the source comment in `src/sample.rs`
saying exactly that. The value target: `+1.0` when the outcome's
winner is the side to move, `−1.0` when the winner is the opponent,
`0.0` for a draw — always from the sampled position's side to move,
which the board supplies.

Two recorded fields deliberately do **not** become targets. The
sparse `prior` (top 16 root priors after noise) is audit data — a
selfplay-corpora format invention for inspecting what the evaluator
believed before search — and the read path never trains on it; π
comes from visits, not priors. The `root_value` estimate is
likewise audit data: z comes from the game outcome, the AlphaGo Zero
discipline (Silver et al. 2017), not from the seat's interim guess.

The returned `train::Sample` stores **stones, not planes**. The
engine's plane encoding (the 17×17 border-padded planes of
`engine::encode`) is applied later, inside `net`'s batcher at batch
time. This is safe because the engine proved the commutation in its
symmetry slice (`encode_commutes_with_every_transform` in
`engine/src/encode.rs`): transforming the stones and then encoding
yields the same planes as encoding and then permuting. So
augmentation can live on stone lists, the storage format stays in
engine vocabulary, and the border is added exactly once, by the
consumer that needs it.

### 4. One D4 transform augments the sample

`augment_sample(sample, transform)` maps both stone lists and every
policy move through `transform.transform_move`, and leaves `to_move`
and `value` untouched. The semantics are the group structure: D4
rotations and reflections preserve wins, losses, draws, and turn
order, so the game facts of a sample are invariant and only its
geometry changes. One sample becomes eight possible training views
of the same position — the augmentation the design locked in
chapter 12 §7 — and because the stored record stays canonical, the
eight-fold multiplicity costs nothing on disk.

### 5. The canonical position decides the holdout

`position_is_holdout(record, ply)` replays and applies the synthetic
datasets' rule unchanged: rebuild the board, take its Zobrist key,
held out iff `zobrist % 100 < HOLDOUT_PERCENTAGE` (= 10). Two
properties are load-bearing. First, the rule is content-based, so
holdout membership is a function of the position itself: shard
layout, export order, and corpus size cannot move a position between
splits, and holdout metrics stay comparable across the synthetic and
self-play sources. Second, the key is computed on the **canonical**
position, before any augmentation: Zobrist keys are
orientation-specific, so hashing after a random D4 transform would
hash a different orientation and could flip the partition per draw.
This is why `CorpusSampler` — which augments every draw — is a
training-stream artifact, not a split-dataset artifact; its doc
comment says so directly: consumers constructing a split use direct
replay plus `position_is_holdout`, or the canonical export path of
step 7. (`train::split::is_holdout` also treats a sample whose
stones cannot rebuild a legal board as held out — the conservative
default keeps corrupt data out of training.)

### 6. The sampler: seeded random game → ply → transform

`CorpusSampler::new(records, seed)` builds an infinite iterator over
the corpus. Each `next` draws three uniform randoms from one
`StdRng` seeded with `seed`: a game slot among the games that have
at least one post-opening ply, a ply within that game, and a
transform from `Transform::ALL`. The draw then replays on demand —
`replay_sample` plus `augment_sample` — so positions are never
materialized in advance and the stream's memory is the corpus
itself. A corpus with no post-opening plies at all is rejected up
front with `SamplerError::EmptyCorpus`. Determinism is the contract:
same records, same seed, same stream, which is exactly what the
specification's 64-draw equality test pins. Items are
`Result<Sample, SampleError>` — replay errors surface at draw time
rather than being hidden by the iterator.

### 7. The mixer: weighted, seeded, finite-source-aware

`SourceMixer` combines any number of sample streams into one. Each
`WeightedSource::new(weight, iterator)` boxes a stream with a
positive relative weight; `SourceMixer::new(sources, seed)` rejects
an empty source list (`MixError::Empty`) and any zero weight
(`MixError::ZeroWeight`), and seeds the *initial* source — the one
scheduling randomness the mixer takes. From there the discipline is
weighted round-robin: the current source contributes `weight`
consecutive items, then the cursor advances. An exhausted finite
source is skipped, and the mixer ends when every source is
exhausted; infinite sources such as `CorpusSampler` make the mixed
stream infinite. Because the schedule is deterministic, the mix
matches the weights exactly over a full cycle — the 1 000 / 3 000
split of the specification is an equality, not a confidence
interval. This is decision S4's artifact: the phase-1 mixture —
self-play corpora, the synthetic tactical set, the external
supplements — is one `SourceMixer` away, and the mixer is generic
over the item type so it combines plain `Sample` iterators and
fallible corpus draws alike.

### 8. Export: canonical samples as train-format shards

`export_corpus(records, out_dir, shard_capacity)` is the batch
alternative to streaming: it materializes **one canonical sample per
post-opening ply**, in corpus order — game by game, ply by ply.
Validation comes first: an empty record list is
`ExportError::EmptyCorpus`, a zero capacity is
`ExportError::ZeroShardCapacity`, and every position in the corpus
is replayed before the output directory is touched — a replay
failure is `ExportError::Sample { game_index, ply, .. }`, and no
partial dataset exists. Then the overwrite discipline: a non-empty
output directory is refused outright (`AlreadyExists`), mirroring
the corpus writer's rule that exports never overwrite by accident.
The samples are chunked by `shard_capacity` into
`shard-000.bin`, `shard-001.bin`, … through
`train::storage::write_shard` — the same length-delimited bincode
framing the synthetic datasets use — and a pretty-printed
`manifest.json` is created with create-new semantics. The manifest
is an `ExportManifest` — the example below uses compact arrays for
readability and is not byte-exact in whitespace; it pins the
semantic schema, not `serde_json`'s pretty output:

```json
{
  "format_version": 1,
  "game_count": 1,
  "sample_count": 6,
  "shards": ["shard-000.bin", "shard-001.bin", "shard-002.bin"],
  "workspace_version": "0.1.0"
}
```

(`format_version` is `SAMPLE_EXPORT_FORMAT_VERSION`;
`workspace_version` is the workspace package version, shared with
the `train` reader so format drift is visible.) Export order is
fully determined by the corpus, and the framing is fixed, so the
same corpus exports the same bytes — determinism here is what makes
the chapter-07 byte-identity gates possible. Note what is *not*
exported: no D4 variants, no holdout labels. The shards stay
canonical, and `nettrain` computes the Zobrist split on load and
applies its own seeded D4 transform per sample within each batch —
the module comment in
`src/export.rs` states the seam in one sentence: `nettrain --data`
reads the shards and ignores manifest details, so no network or
training changes.

### 9. The binary: read, export, read back

`selfplayexport` is a thin driver with hand-rolled argument parsing
— no dependency — in the style of `selfplaygen` and
`selfplayanchors`. The vocabulary is two required flags and one
option, values as separate arguments only (`--flag=value` is not
supported): `--corpus <DIR>`, `--out <DIR>`, and
`--shard-capacity <usize>` (default 4096). The exact usage text:

```text
Usage: selfplayexport --corpus <DIR> --out <DIR> [--shard-capacity <usize>]
Default: --shard-capacity 4096
Writes one canonical train::Sample per post-opening ply plus manifest.json.
nettrain --data <DIR> reads the shards directly and applies seeded D4 at batch time.
Use `--flag value` only; `--flag=value` is not supported.
```

`--help` (or `-h`) prints it and exits successfully; a parse failure
prints the message and the usage and exits with code 2. The run path
is four steps: `read_corpus` loads the records (a read failure
reports `failed to read corpus: …`); `export_corpus` writes the
dataset (a failure reports `failed to export samples: …`); the
binary then reads the export back through
`train::dataset::read_dataset` and fails if the read-back count
differs from the manifest's `sample_count` — the exporter verifies
its own output through the consumer's reader before reporting
success. On success, one line goes to stderr:
`Exported 1860 samples from 48 games into 4 shards at <DIR>`. Any
run failure prints `selfplayexport failed: …` and exits with code 1.

## The read-path contract

The chapter's semantics, in one place.

- **Augmentation is a read-time decision.** Records and exported
  shards are always canonical. The streaming path augments per draw
  (`CorpusSampler`); the batch path augments per sample within each
  training batch (`nettrain`'s seeded batcher). Nothing D4-shaped is ever written
  to disk, so no experiment is ever locked into someone else's
  augmentation choices, and the engine's proven
  encode-after-transform commutation guarantees the two paths see
  equivalent planes.
- **The holdout partition is stable.** It is computed from the
  canonical position's Zobrist key with the synthetic datasets' own
  rule — `zobrist % 100 < 10` — never after augmentation, never
  from a sample's position in a file. The same position is always in
  the same split, in every source, in every run.
- **π is visits, z is the outcome, the prior is audit.** The policy
  target is the normalized recorded visit histogram; the value
  target is the game outcome from the sampled side to move. The
  recorded sparse priors and root-value estimates travel along for
  audit and analysis and are never trained on.
- **Unsearched plies are honest.** A TSS-bypassed certified move has
  no visit histogram; its one-hot policy on the played move is
  deliberate behavior cloning, and its value target still comes from
  the outcome. A corrupt nonempty-but-zero histogram is a named
  error, not a NaN.
- **Sampling and mixing are seeded and exact.** One `StdRng` per
  stream; game → ply → transform per draw; weighted round-robin with
  a seeded initial source. Same seed, same stream; weights are met
  exactly per cycle, not approximately.
- **Export validates before it writes, and never overwrites.** All
  positions replay before the output directory is modified; a
  non-empty output directory is refused; the manifest is create-new;
  the binary reads its own export back through the real reader and
  checks the count before it reports success.

## Integration smoke evidence

The end-to-end seam — game records in, training steps out — was
measured in the reference's acceptance run and is quoted here from
its logs (`acceptance-evidence.md` §5 in the reference worktree);
the learner does not re-run it, and chapter 07 owns the full gates.
On the 48-game baseline-32 corpus (1 860 post-opening plies):

```text
selfplayexport \
  --corpus data/selfplay/v0-b32 \
  --out data/selfplay/v0-b32-export \
  --shard-capacity 512
```

exported **1 860 samples from 48 games into 4 shards** in 0.18 s,
and the exporter's own read-back through `train::dataset` matched
the manifest count. Training on the export, unmodified:

```text
nettrain \
  --out data/selfplay/v0-b32-nettrain-smoke \
  --data data/selfplay/v0-b32-export \
  --config tiny --device flex --seed 7 --steps 50 \
  --dataloader-workers 0 --eval-interval 50 --progress off
```

ran the tiny configuration (8 channels, 1 residual block, batch size
16) for 50/50 steps in 0.47 s and wrote a checkpoint. The dataset
split into **1 665 train and 195 holdout** samples — the Zobrist
rule over the canonical export, 10.5 % held out. Train loss moved
from **6.5483584 at step 1 to 6.252371 at step 50**
(`metrics.jsonl`; step-50 holdout loss 6.1449027). This is a smoke
result: it proves the net consumed game-derived sample shards end to
end. It is not a model-quality assessment and not a playing-strength
claim of any kind.

## What this chapter defers

- Corpus verification and the acceptance gates — the
  `selfplayverify` binary (legality replay and JSON statistics), the
  determinism and legality gates, the grading observation, and the
  first corpus's quality report — chapter 07. This chapter quotes
  that chapter's integration smoke early because the smoke is this
  chapter's seam, proven.
- Any change to `net`, `nettrain`, or the training loop. The export
  exists precisely so that none is needed; the later
  service/training redesign (evaluator service, dynamic batching,
  GPU scheduling) is milestone 5, not this track.

## TDD checklist

Work one artifact at a time, red to green. No manifest changes:
`selfplay` already depends on `engine`, `train`, `rand`,
`thiserror`, `serde`, and `serde_json`, and a binary in `src/bin/`
needs no manifest entry. No other crate is touched.

1. The replay module
   [`src/sample.rs`](../../../../gomoku/crates/selfplay/src/sample.rs):
   the four-variant `SampleError`, then `replay_sample`,
   `augment_sample`, and `position_is_holdout`. Red in the
   specification's order: the replay test first (board and side to
   move), then the two policy tests and the zero-visit rejection,
   then the value test, then the D4 test, then the holdout test.
   Green against all seven. Re-export the vocabulary from
   [`src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs).
2. The sampler
   [`src/sampler.rs`](../../../../gomoku/crates/selfplay/src/sampler.rs):
   `SamplerError` and `CorpusSampler` with `new`, `from_corpus`, and
   the three-draw `next`. Red: `corpus_sampler_is_deterministic_under_one_seed`,
   then `corpus_sampler_loads_a_corpus_directory`. Green in both.
3. The mixer
   [`src/mix.rs`](../../../../gomoku/crates/selfplay/src/mix.rs):
   `MixError`, `WeightedSource`, and the round-robin `SourceMixer`.
   Red: `source_mixer_respects_weights_over_a_large_seeded_draw`.
   Green.
4. The export
   [`src/export.rs`](../../../../gomoku/crates/selfplay/src/export.rs):
   `SAMPLE_EXPORT_FORMAT_VERSION`, `ExportManifest`, the
   four-variant `ExportError`, and `export_corpus` with
   validate-then-write ordering.
5. The binary
   [`src/bin/selfplayexport.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayexport.rs):
   the usage text, `parse_args`, and the four-step run path with the
   read-back count check. Red against steps 4+5 together:
   `export_command_writes_samples_that_the_real_train_reader_loads`
   drives the compiled binary and the real `train` reader. Green.
6. Regression: the game, corpus, openings, schedule, dedup, anchors,
   and existing binary targets stay green unchanged — the chapter
   only reads what earlier chapters wrote — and chapter 01's MANUAL
   pair still passes in release, untouched.

## Capstone

Export a corpus you already own and read the samples back, without
touching any committed data. Reuse the corpus your chapter-04 or
chapter-05 capstone generated (any corpus directory works; nothing
here plays games, so no release build is needed). From `gomoku/`:

```text
cargo build -p selfplay

dir=$(mktemp -d)

target/debug/selfplayexport \
  --corpus <path-to-your-corpus> \
  --out "$dir/export"

cat "$dir/export/manifest.json"
```

The stderr line reports the shape of your dataset —
`Exported N samples from G games into S shards at …` — and the
manifest on disk says the same thing as JSON: `game_count`,
`sample_count`, and the shard names in sample order. The read-back
already happened: the binary loaded its own export through
`train::dataset::read_dataset`, the reader `nettrain` uses, and
checked the count against the manifest before printing that line.
Two boundary experiments complete the picture. Re-run the export
with the same `--out`: the run fails with
`refusing to export samples into a non-empty directory` — an export
never overwrites a dataset by accident, so point each export at a
fresh directory. And export the same corpus twice into two fresh
directories and compare: the same corpus yields the same shards,
because export order is corpus order and nothing in the path rolls
dice. The reference's acceptance run of this seam — 1 860 samples, 4
shards, then 50 `nettrain` steps on the result — is quoted in the
integration smoke evidence above; reproducing the training run is
chapter 07's acceptance work, not this capstone's.

## Done when

- `cargo test -p selfplay` reports **46 passed, 4 ignored**: the
  chapter-05 targets keep their state (game 5 passed / 2
  MANUAL-ignored, corpus 10/0, openings 5/0, schedule 1/1, dedup
  1/0, grading 0/1, anchors 4/0, binary 3/0, the `src` unit tests
  6/0), and this chapter adds `tests/read_path.rs` 11/0. The four
  ignored tests are exactly chapter 04's set; this chapter adds
  none.
- `tests/binary.rs` is unchanged at 3 passed: `selfplayexport`'s
  command line is pinned by the read-path export test, and the
  release-binary smoke on a real corpus is quoted acceptance
  evidence, not a unit test.
- No new MANUAL test exists, and nothing in this chapter plays a
  game: every test fabricates its records or exports a one-game
  corpus in debug (the reference runs the whole new target in about
  0.2 s). The release-only, game-playing verification discipline of
  chapter 04 is unchanged, and no `--release` run is chapter-06
  work.
- `cargo test -p train` and `cargo test -p player` are not required:
  both crates are untouched.
- `cargo clippy --all-targets -- -D warnings` is green.
- `cargo fmt --all -- --check` makes no changes.
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

The first command is the fast gate: this chapter adds nothing slow
to it. The focused selection runs the chapter's surface in isolation
— the replay, augmentation, holdout, sampler, and mixer tests, plus
the export test, which compiles and drives the real
`selfplayexport` binary over a fabricated one-game corpus and loads
the result with the real `train::dataset` reader. No `--release --
--ignored` run is chapter-06 work: game-playing verification stays
MANUAL and release-only, exactly as chapter 04 left it.

## References

**This track:**

- [Tutorial README](../README.md) — conventions, gates, the chapter
  map, and the reference provenance.
- [Self-play primer](../00-selfplay-primer.md) — §4.6 states this
  chapter's contract; the glossary defines π, z, the prior, and the
  holdout split.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md) —
  the `GameRecord` format and the `read_corpus` / `write_corpus`
  mechanics this chapter consumes.
- [Chapter 03 — recording the search](../03-recording-the-search/recording-the-search.md) —
  what the recorded visits, priors, and root values mean, and the
  honesty rule for unsearched plies.
- [Chapter 05 — certified endgames](../05-certified-endgames/certified-endgames.md) —
  why unsearched plies exist: the certified own-side TSS bypass.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  decision S2 (store games, not positions), §3.3 (the read path),
  decision S4 (source mixing).
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  this chapter's public surface and the measured integration smoke
  numbers.
- [docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) —
  §7 the locked D4 augmentation, §9 the store-games replay
  discipline.

**Prerequisite tracks:**

- [Datagen tutorial](../../datagen-tutorial/README.md) — the
  `train` crate: `Sample`, the shard mechanics, the dataset reader,
  and the Zobrist-keyed holdout split.
- [Net tutorial](../../net-tutorial/README.md) — `nettrain`, the
  consumer: canonical shards in, seeded batch-time D4.
- [Engine tutorial](../../13-engine-tutorial/README.md) — the
  bitboard `Board`, `Board::zobrist`, the D4 `Transform`s, and the
  encode–transform commutation.

**Source:**

- [`gomoku/crates/selfplay/src/sample.rs`](../../../../gomoku/crates/selfplay/src/sample.rs) —
  new: `SampleError`, `replay_sample`, `augment_sample`,
  `position_is_holdout`. Created in this chapter.
- [`gomoku/crates/selfplay/src/sampler.rs`](../../../../gomoku/crates/selfplay/src/sampler.rs) —
  new: `SamplerError`, `CorpusSampler`. Created in this chapter.
- [`gomoku/crates/selfplay/src/mix.rs`](../../../../gomoku/crates/selfplay/src/mix.rs) —
  new: `MixError`, `WeightedSource`, `SourceMixer`. Created in this
  chapter.
- [`gomoku/crates/selfplay/src/export.rs`](../../../../gomoku/crates/selfplay/src/export.rs) —
  new: `SAMPLE_EXPORT_FORMAT_VERSION`, `ExportManifest`,
  `ExportError`, `export_corpus`. Created in this chapter.
- [`gomoku/crates/selfplay/src/bin/selfplayexport.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplayexport.rs) —
  new: the export binary. Created in this chapter.
- [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs) —
  re-exports the new vocabulary. Touched in this chapter.
- [`gomoku/crates/selfplay/tests/read_path.rs`](../../../../gomoku/crates/selfplay/tests/read_path.rs) —
  the read-path specification: all eleven tests. Created in this
  chapter.
- [`gomoku/crates/selfplay/tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs) —
  unchanged this chapter; gains the `selfplayverify` tests in
  chapter 07. Not touched in this chapter.
- [`gomoku/crates/train/src/sample.rs`](../../../../gomoku/crates/train/src/sample.rs),
  [`gomoku/crates/train/src/storage.rs`](../../../../gomoku/crates/train/src/storage.rs),
  [`gomoku/crates/train/src/dataset.rs`](../../../../gomoku/crates/train/src/dataset.rs),
  [`gomoku/crates/train/src/split.rs`](../../../../gomoku/crates/train/src/split.rs) —
  the consumed `train` surface: `Sample`, `write_shard`,
  `read_dataset`, `is_holdout` / `HOLDOUT_PERCENTAGE`. Not touched
  in this chapter.
- [`gomoku/crates/engine/src/symmetry.rs`](../../../../gomoku/crates/engine/src/symmetry.rs)
  and
  [`gomoku/crates/engine/src/encode.rs`](../../../../gomoku/crates/engine/src/encode.rs) —
  the D4 transforms and the encode–transform commutation the
  augmentation discipline relies on. Not touched in this chapter.

**Companion papers:**

- [reading-back-design.md](reading-back-design.md) — the conceptual
  story of the
  read path: why games stay canonical and everything else is a
  read-time decision.
- [reading-back-implementation.md](reading-back-implementation.md) —
  the hiding layer:
  low-level construction instructions, signatures in prose, method
  bodies never.
- [reading-back-solution.md](reading-back-solution.md) — the
  annotated reference
  snippets.

**External sources:**

- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://doi.org/10.26481/dis.19940923la — threat-space search, the
  prover behind the TSS bypasses whose plies carry no visits.
- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). https://doi.org/10.1038/nature24270 —
  the (s, π, z) training record: policy from search visit counts,
  value from the game outcome, board symmetries as augmentation.
