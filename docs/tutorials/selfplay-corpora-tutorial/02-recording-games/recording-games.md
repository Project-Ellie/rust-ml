# Chapter 02 — Recording games

## Abstract

This chapter teaches the `selfplay` crate to persist complete games.
Chapter 01 produced an in-memory `Game`; this chapter defines its
durable form, the `GameRecord`, and the corpus that holds many of
them: fixed-capacity shards of length-delimited bincode records plus a
JSON manifest. The shard mechanics are not new code: they are
extracted from the `train` crate's existing dataset writer and reader
into a shared `train::storage` module that both the synthetic tactical
datasets and the game corpora use. The chapter's public surface is
`GameRecord`, `GameHeader`, `RecordedPly`, `CertifiedSuffix`,
`SeatPairing`, `SeatConfig`, `GAME_RECORD_FORMAT_VERSION`,
`GameRecord::from_game`, `write_corpus`, `write_corpus_with_options`,
`read_corpus`, `read_manifest`, `OverwritePolicy::{Refuse, Allow}`,
and the shared `train::storage::{write_shard, read_shard}`. The
conclusions: a written corpus reads back as values equal to what
was written, and re-writing those read-back values reproduces the
original file byte for byte; a corrupt or truncated shard is an error, never a
silent guess; an existing corpus is never overwritten without an
explicit opt-in; and identical seeds produce byte-identical corpora
across process restarts. The chapter also adds one incremental slot
to chapter 01's `Game` — `certified_suffix`, which stays `None` until
chapter 05 teaches the runner to detect certified endgames. What this
chapter deliberately does not do: it does not prove that recorded
moves are legal or that recorded outcomes are correct — that replay
verification is chapter 07's gate. The intended reader is working the
selfplay-corpora tutorial in order and has completed chapter 01.

## Glossary

| Term | Definition |
|------|------------|
| **Game record** | The durable serialization of one complete game: a header (configuration and provenance), the per-ply records, the outcome, and an optional certified suffix. The artifact is `GameRecord`. |
| **Corpus** | A set of game records produced at one declared pairing, stored as shard files plus a manifest in one directory. |
| **Shard** | A fixed-capacity file of length-delimited bincode records; the `train` crate's storage unit, named `shard-NNN.bin`. |
| **Length-delimited framing** | Writing a `u32` little-endian length prefix before each bincode payload, so the reader knows where one record ends and the next begins. |
| **Manifest** | The JSON file next to the shards — the artifact `Manifest`, serialized as `manifest.json`: format version, seed set, pairing, game count, shard list, and workspace version. The shards are the data; the manifest is the provenance. |
| **Format version** | The durable-layout version number, `GAME_RECORD_FORMAT_VERSION = 1`, stamped into every record header and every manifest so a future layout change is detectable. |
| **Pairing** | The declared Black-vs-White seat configuration of a corpus — the per-seat simulation budgets and settings. The pairing is the corpus's declared identity and lives in every record header. |
| **Seat** | One of the two players of a generated game: a `player::AiPlayer` running the baseline evaluator at a declared simulation budget. |
| **Simulation budget** | The number of Monte Carlo Tree Search (MCTS) simulations a seat runs per move decision; written baseline-X for a budget of X. |
| **Provenance** | The recorded origin of the data: the game's seed, the opening's source (generated or drawn from a named list at a named index), and the identity of the evaluator that produced the decisions. |
| **Certified suffix** | A terminal segment of a game in which every attacker move belongs to a machine-verified forced-win line produced by the engine's threat-space search (TSS) prover (Allis 1994). This chapter adds the record slot; chapter 05 fills it. |
| **Overwrite policy** | The explicit choice, `OverwritePolicy`, between refusing a non-empty output directory (`Refuse`, the default) and replacing it (`Allow`). |
| **Round trip** | Writing a value and reading it back; a byte-exact round trip additionally requires that re-writing the read-back value reproduces the original file bytes. |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games. This chapter adds none; the two ignored tests in the verification counts belong to chapter 01. |
| **π (visit-distribution target)** | The normalized root visit-count distribution of the search at a ply; the policy training target (Silver et al. 2017). The record stores the raw counts; normalization is chapter 03's and chapter 06's concern. |
| **z (outcome target)** | The game result in {+1, 0, −1} from the perspective of the side to move at a ply; derived from the stored outcome at read time, not stored per ply. |

## Context

Four sources supply what this chapter assembles. Each summary states
what the source delivered and why this chapter needs it.

- [Chapter 01 — the game runner](../01-the-game-runner/the-game-runner.md) —
  built the in-memory `Game`: the seed, both seats' `PlayerOptions`,
  the `Opening` with its `OpeningProvenance`, the ordered `PlyRecord`
  list with its optional `PlySearch` (raw visit counts, prior, root
  value), and the `Outcome`/`WinReason` vocabulary. Serde derives are
  scoped, not blanket: chapter 01 derived them on the opening and
  outcome/win-reason vocabulary only — the values that cross into the
  record unchanged — while `Game`, `PlyRecord`, and `PlySearch`
  carry no serde derives, which is why this chapter defines its own
  record types and a conversion instead of serializing the `Game`
  itself. One seed fixes the entire game, so the seed is the
  complete identity of a record. This chapter persists that
  structure; it does not replay or re-run anything.
- [Datagen tutorial chapter 05](../../datagen-tutorial/05-shards-and-manifest.md)
  and [chapter 06](../../datagen-tutorial/06-reading-back.md) — built
  the `train` crate's storage discipline: `write_dataset` streams
  `Sample` records into length-delimited bincode shards
  (`shard-000.bin`, `shard-001.bin`, …) with `create_new` overwrite
  protection and writes a JSON manifest last; `read_dataset` reads the
  shards back, rejecting oversized length prefixes and torn prefixes.
  That discipline is currently private to the tactical datasets. This
  chapter extracts it into a shared `train::storage` module so game
  corpora use exactly the same framing, guards, and error behavior —
  one storage mechanic, two record types.
- [Baseline tutorial](../../baseline-tutorial/README.md) — built
  `player`, whose `PlayerOptions` carries the per-seat configuration
  this chapter must record: simulation budget, the predictor-UCT
  exploration constant (PUCT; Rosin 2011), temperature, Dirichlet
  noise settings, the own-side threat-space search (TSS) probe
  configuration (`OwnTssConfig`), the refutation-guard configuration
  (`GuardConfig`), and the resignation flag. `PlayerOptions` has no
  serde derives, so the record defines its own serializable mirrors.
- The [tutorial README](../README.md) fixes this track's conventions;
  the [primer](../00-selfplay-primer.md) §3 pins the record format
  and the store-games rationale; the
  [design spec](../../../specs/2026-11-11-selfplay-corpora-design.md)
  is the design authority. Its decision S2 — *store games, not
  positions* — is why planes are never written: the record stays raw
  enough to re-encode under any future plane encoding and rich enough
  to train on without re-searching. Its §3.2 fixes the record's
  shape: the pairing lives in the header, π is stored sparse and
  normalized at read time, and z is derived from the stored outcome
  at read time.

## Intention

You can take the games the runner produces and put them on disk as a
corpus — a directory you can copy, archive, and hand to the later
chapters. Every record on disk tells you exactly how its game was
produced: the seed that regenerates it, the configuration of both
seats, the evaluator's identity, and where the opening came from.
Reading the corpus back gives you values equal to what you wrote, and
re-writing them reproduces the same bytes. A shard that was truncated
or corrupted surfaces as an error at read time, not as silently
wrong data. An existing corpus is never destroyed by accident:
writing into a non-empty directory is refused unless you explicitly
ask for replacement, and even then a rejected write leaves the old
corpus untouched. And because one seed fixes one game and nothing in
the format depends on the writing process, the same seeds regenerate
the same corpus — byte for byte, in any process, on any day.

## Specification as tests

The tests live in
[`gomoku/crates/selfplay/tests/corpus.rs`](../../../../gomoku/crates/selfplay/tests/corpus.rs).
The list below is ordered along the execution path of this chapter —
record conversion first, then the shard and manifest write, then the
read round trip, then malformed input and versioning, then overwrite
protection, then cross-process determinism. It is not the reference
file's lexical order.

This chapter specifies **ten** tests. All ten run by default; none is
ignored. The chapter adds no MANUAL game-playing tests, because
nothing here plays a game: every `Game` in the test file is built by
hand. One honesty note on the count: `corpus_writer_process_helper`
is the child-process entry point of the cross-process determinism
test. It is registered as a test so the test binary can re-execute
itself, and it passes trivially — returning immediately — whenever
its environment variable is unset.

The `train` side of this chapter is an extraction, not a new feature:
it moves existing code behind a shared module, and the existing
`train` suite is the regression gate. Four already-existing tests in
[`gomoku/crates/train/src/dataset.rs`](../../../../gomoku/crates/train/src/dataset.rs)
are part of this chapter's specification because the corpus reader
inherits their behavior verbatim; they are named at the
malformed-input stage below. The extraction adds no new `train` tests
and removes none.

A separation note, so later chapters do not double-count: all ten
tests of `tests/corpus.rs` belong to this chapter. Every fixture is
built through `minimal_game` with hand-constructed search data —
that is deliberate, because the `From<player::SearchStats>`
conversion is chapter 03's subject. Two tests assert the
`PlySearch`-derived record fields specifically:
`game_record_preserves_sparse_visits_including_the_played_move` and
`populated_search_fields_round_trip_byte_exactly`. One
sets a `CertifiedSuffix` by hand — detection and extraction are
chapter 05's subject. Later targets that *consume* `read_corpus`
(`tests/read_path.rs` in chapter 06, the verifier tests of chapter
07) are specified there, not here. Conversely, the two
`PlySearch`-conversion tests that exist in the completed reference's
[`gomoku/crates/selfplay/tests/game.rs`](../../../../gomoku/crates/selfplay/tests/game.rs)
belong to chapter 03, as chapter 01's paper states — a learner at
this chapter does not have them in their tree yet.

### The test helpers

Three helpers build the fixtures. `TestDir` creates a unique
temporary directory per test — the test name, the process id, a
nanosecond timestamp, and an atomic sequence number — and removes it
on `Drop`, so tests never collide and never leak directories.
`mv(row, col)` builds an on-board `Move`. `minimal_game(seed)`
builds a complete six-ply `Game` by hand: a listed opening
(`OpeningProvenance::Listed { list: "test-openings", index: 3 }`,
answered with `OpeningAnswer::Take(Color::Black)`), six plies whose
first five carry a `PlySearch` with one visit entry and two prior
entries — and that one visit entry is the ply's played move, which
is what makes the first conversion test's "contains the played move"
assertion meaningful — Black
configured at budget 1 and White at budget 2 so the two seats are
distinguishable in the recorded pairing, and an outcome of
`Outcome::Won { winner: Color::Black, reason: WinReason::FiveOrMore }`.
The sixth ply — the winning move — carries `search: None`, modeling a
certified-proof move that bypassed search.

### Recording one game: the conversion

- `game_record_preserves_sparse_visits_including_the_played_move` —
  converting `minimal_game(17)` with `GameRecord::from_game`
  preserves the sparse visit data per ply: for every ply whose search
  ran, the recorded visit list is non-empty and contains the played
  move; for the no-search ply, the recorded visit list is empty. This
  pins the as-is rule: chapter 01's raw slot data is persisted
  verbatim, and a ply with no search produces no fabricated data.
- `game_record_preserves_a_certified_suffix` — a `Game` whose
  `certified_suffix` slot is set by hand (a suffix starting at ply 0
  whose line is the game's full move list) converts into a
  `GameRecord` carrying exactly that suffix. This pins the slot: the
  record format already has room for chapter 05's certificates, and
  conversion passes the slot through.
- The first half of `populated_search_fields_round_trip_byte_exactly`
  pins the header and the per-ply mapping: the header's format
  version equals `GAME_RECORD_FORMAT_VERSION`, the seed equals the
  game's seed, the recorded Black and White simulation budgets are 1
  and 2, the evaluator string is `baseline@<crate version>`, and the
  opening and outcome equal the game's. Every recorded ply equals its
  source: move, visits, prior, and root value when search ran; empty
  visits, empty prior, and `root_value: None` when it did not. The
  record's `certified_suffix` is `None` for an ordinary game.

### Writing shards and the manifest

- `manifest_records_pairing_and_seed_set_and_round_trips` — writing
  two records (seeds 4 and 17, shard capacity 2) with `write_corpus`
  returns a `Manifest` whose format version is
  `GAME_RECORD_FORMAT_VERSION`, whose seeds are `[4, 17]` in corpus
  order, whose pairing equals the first record's header
  configuration, whose game count is 2, and whose workspace version
  is the crate's version. `read_manifest` on the directory returns an
  equal value.
- `shard_capacity_two_rolls_over_and_manifest_lists_every_shard` —
  five records at shard capacity 2 produce three shards, listed in
  the manifest as `shard-000.bin`, `shard-001.bin`, `shard-002.bin`;
  the manifest's game count is 5; and `read_corpus` returns the five
  records equal and in order. The last shard is short — capacity is a
  maximum, not a padding requirement.

### Reading back: the round trip

- `populated_search_fields_round_trip_byte_exactly` — after the
  header and per-ply assertions above, the test writes one record,
  reads it back with `read_corpus`, and requires equality. Then it
  writes the read-back record into a second directory and compares
  file bytes: `shard-000.bin` and `manifest.json` are byte-identical
  across the write–read–write cycle. Equality of values is not
  enough; the format must also be stable under re-serialization,
  because chapter 06 reads corpora and chapter 07 regenerates them.

### Malformed input, truncation, and the format version

The corpus reader inherits its framing defenses from the shared
`train::storage::read_shard`, and the tests pinning that behavior
live with the `train` suite:

- `dataset::tests::record_length_cap_rejects_invalid_shard` — a shard
  whose first length prefix exceeds the maximum record payload
  (`MAX_RECORD_LEN`, 1 MiB) is rejected with
  `io::ErrorKind::InvalidData`, and the error message names the cap.
  The cap exists so a corrupt prefix cannot force a huge allocation.
- `dataset::tests::torn_length_prefix_is_an_error_not_a_clean_eof` —
  a shard ending with two bytes of a four-byte length prefix is
  rejected with `io::ErrorKind::UnexpectedEof`. A truncated record is
  corruption, not a clean end of shard.

Two neighboring `train` tests bound the same behavior from the other
side: `dataset::tests::roundtrip_read_back_written_dataset` is the
round trip the extraction must preserve, and
`dataset::tests::read_empty_dataset_directory_returns_empty_vector`
pins that reading a directory without shards yields an empty vector.
`read_corpus` does not inherit that directory behavior from
`read_dataset`; it mirrors it, applying the same shard-discovery
discipline — the `shard-` prefix and `.bin` suffix filter and the
lexicographic sort — to its own directory listing (walkthrough step
7), so an empty corpus directory likewise reads back as an empty
vector. The inheritance through `train::storage` covers the framing
defenses, not the directory walk.

The format version is specified differently — as a stamp, not yet as
a rejection rule: `manifest_records_pairing_and_seed_set_and_round_trips`
and `populated_search_fields_round_trip_byte_exactly` assert that
`GAME_RECORD_FORMAT_VERSION` is written into every manifest and every
record header. No reader in this tutorial rejects a foreign version;
the stamp is the seam that makes a future rejection rule possible.
What is validated and what is not is stated precisely in walkthrough
step 7.

### Overwrite protection

- `write_refuses_a_non_empty_corpus_directory` — a directory
  containing one unrelated file (`keep.txt`) is refused by the
  default `write_corpus` with `io::ErrorKind::AlreadyExists`; the
  file's contents survive, and neither `shard-000.bin` nor
  `manifest.json` appears. Refusal happens before anything is
  written.
- `explicit_overwrite_replaces_the_previous_corpus` — after an
  initial corpus of three records at capacity 1 (three shards),
  `write_corpus_with_options` with `OverwritePolicy::Allow` and one
  replacement record removes the old corpus entirely: the new
  manifest lists seed 9 and exactly one shard, `shard-001.bin` no
  longer exists, and `read_corpus` returns exactly the replacement.
  `Allow` replaces; it never merges.
- `invalid_explicit_overwrite_preserves_the_existing_corpus` — a
  zero shard capacity is rejected with `io::ErrorKind::InvalidInput`,
  and the existing corpus is fully intact afterwards (its manifest
  still lists the original seed). Validation happens before the
  overwrite gate, so even an explicit `Allow` cannot destroy data
  with a call that was never valid.

### Determinism across process restarts

- `identical_seeds_produce_byte_identical_corpora_across_process_restarts` —
  the test binary re-executes itself twice as a child process,
  filtered to `corpus_writer_process_helper`, with the output
  directory passed through an environment variable. Each child writes
  the same three-record corpus (seeds 4, 17, 23; capacity 2). The
  parent then compares bytes: the two `manifest.json` files are
  identical, and every shard named in the manifest is identical
  across the two processes.
- `corpus_writer_process_helper` — the child entry point: when its
  environment variable is set, it builds the three records and writes
  the corpus; when unset, it returns immediately.

This pair is the in-tests form of the project's determinism gate
(design spec §5, gate 1): same seeds, byte-identical corpus, across
process restarts. Nothing in the corpus depends on wall-clock time,
process state, or hash-map ordering, and this test proves it.

## Walkthrough

The order is the execution order of one corpus write followed by one
read. Each step names the artifact at the moment execution first
touches it.

### 1. `Game` grows a record slot

Before anything can be recorded, chapter 01's
[`Game`](../../../../gomoku/crates/selfplay/src/game.rs) gains one
field: `certified_suffix: Option<CertifiedSuffix>`. The runner sets
it to `None` for now; chapter 05 teaches the runner to notice when a
seat's verified forced-win line was followed to the end and to
populate the slot then. Adding the slot in this chapter — with the
record format, not with the detection — keeps the conversion in step
2 total from this chapter onward: a `Game` never carries information
the `GameRecord` would drop. The type behind the slot,
`CertifiedSuffix`, is record vocabulary: `from_ply: u16`, the
zero-based ply from which the suffix is certified, and
`line: Vec<Move>`, the verified forced-win line. Because `Game`
derives `PartialEq`, the new field leaves chapter 01's determinism
test untouched: both sides of the comparison are `None`.

### 2. `GameRecord::from_game`: conversion without replay

The entry point of this chapter is a pure conversion.
`GameRecord::from_game(&game)` — a `#[must_use]` method delegating to
the `From<&Game>` implementation — builds the durable record without
replaying the board, without searching, and without I/O. Everything
it needs is already in the `Game`. This is the store-games decision
(design spec S2) made mechanical: because chapter 01 kept the
opening, every ply, and the outcome in memory, persistence is a
transformation of values, not a re-computation.

The output, `GameRecord`, has four fields: `header: GameHeader`,
`plies: Vec<RecordedPly>`, `outcome: Outcome`, and
`certified_suffix: Option<CertifiedSuffix>`. The outcome and the
opening move across unchanged — chapter 01 already gave both serde
derives for exactly this purpose. The header and the plies are the
next two steps.

### 3. The header: version, seed, pairing, evaluator, opening

`GameHeader` carries five fields, each answering one question a
future reader of the corpus will ask:

- `format_version: u16` — *which layout is this?* Always
  `GAME_RECORD_FORMAT_VERSION`, the constant `1`. The format is the
  one the milestone-5 replay buffer will consume, so it will outlive
  this tutorial; the stamp makes a future layout change detectable
  instead of silently misread.
- `seed: u64` — *which game is this?* Chapter 01 established that one
  seed fixes the entire game, so the seed is the record's complete
  regeneration key.
- `config: SeatPairing` — *who played, at what strength?* The
  pairing is the corpus's declared identity (design spec S5), so it
  lives in every header, not just in the manifest. `SeatPairing`
  holds `black: SeatConfig`, `white: SeatConfig`, and
  `exploration: ExplorationSchedule`.
- `evaluator: String` — *whose decisions are these?* The string
  `baseline@<version>`, built from `env!("CARGO_PKG_VERSION")` at
  compile time. When a trained network eventually generates its own
  games, this field is what keeps baseline records and network
  records distinguishable in one replay buffer.
- `opening: Opening` — *where did the opening come from?* Cloned
  from the game, provenance included, so a listed opening never
  masquerades as a generated one.

`SeatConfig` is a serializable mirror of one seat's `PlayerOptions`:
`simulations`, `c_puct`, `temperature`, `noise_epsilon`,
`noise_alpha`, `tss_probe: TssProbeSettings`, `guard: GuardSettings`,
and `resign`. The mirror types exist because `PlayerOptions` belongs
to the `player` crate and carries no serde derives; giving the record
its own vocabulary keeps the durable format decoupled from another
crate's internals. The `From<PlayerOptions>` conversion copies every
field, flattening `OwnTssConfig` into `TssProbeSettings` (`enabled`,
`max_nodes`, `max_depth`) and `GuardConfig` into `GuardSettings`
(`enabled`, `max_candidates`, `max_nodes`, `max_depth`). Recording
the base options verbatim — the options as supplied, before any
per-ply adjustment — is what makes the recorded configuration an
honest answer to "what was asked for".

The `exploration` field deserves a precise scope statement.
`ExplorationSchedule` is declared in this chapter as record
vocabulary — its fields (`plies`, `temperature`, `noise_epsilon`,
`noise_alpha`), its serde derives, and its `Default`, which mirrors
the exploration practice of the CLI's AI-vs-AI sessions (an
exploration window of `EXPLORATION_PLIES` = 10 plies at temperature
1.0 with Dirichlet noise ε = 0.25, α = 0.3). The header records the
schedule because the schedule is part of the corpus's identity. What
this chapter does **not** build is the schedule's behavior:
`ExplorationSchedule::options_for_ply` and the runner's per-ply
application of it are chapter 04's subject, and `from_game` records
`ExplorationSchedule::default()` — the only schedule that exists
until then.

### 4. The plies: raw slot data, as-is

Each `PlyRecord` converts into one `RecordedPly`: `mv`, the played
move; `visits: Vec<(Move, u32)>`, the sparse raw root visit counts;
`prior: Vec<(Move, f32)>`, the sparse prior entries; and
`root_value: Option<f32>`. When the ply's `search` is present, all
three fields are copied across unchanged — this chapter introduces no
conversion from `player::SearchStats`, no top-k truncation, and no
normalization, because chapter 01's runner already produced the raw
slots and chapter 03 owns everything derived from them. When the
ply's `search` is absent — a move played from a certified own-side
proof — the record keeps empty visits, an empty prior, and
`root_value: None`. Fabricating statistics for a move no search
produced would be dishonest telemetry; the baseline tutorial's
honesty rule is load-bearing here. A resignation ends the game
without a played move, so it produces no ply at all — and no
fabricated value.

Two read-time rules are already visible in this shape, though their
owners are later chapters: the stored visit counts are raw, so π is a
normalization of `visits` at sample time (chapter 03's and chapter
06's concern), and the outcome is stored once per game, so z is
derived per ply from the side to move at read time (chapter 06).

### 5. Serialization and the shared shard mechanics

All record types derive serde `Serialize` and `Deserialize`, and the
on-disk framing belongs to the `train` crate. This chapter extracts
the framing that `write_dataset` and `read_dataset` previously
implemented privately into a new shared module,
[`gomoku/crates/train/src/storage.rs`](../../../../gomoku/crates/train/src/storage.rs),
with two generic functions and two constants:

- `write_shard<T: Serialize>(records, path)` encodes each record with
  bincode 2 (`bincode::serde::encode_to_vec` under
  `bincode::config::standard()`), prefixes the payload with its
  length as a little-endian `u32`, and appends both to the shard
  through a buffered writer. The destination opens with `create_new`
  semantics: an existing shard is never truncated. One honest
  caveat, documented on the function: the file is created before
  encoding begins, so a mid-write encoding or I/O failure can leave a
  partial shard behind.
- `read_shard<T: DeserializeOwned>(path)` loops over the framing: a
  clean end-of-file after zero bytes ends the shard; one to three
  leftover prefix bytes are a torn record and surface as
  `UnexpectedEof`; a prefix above `MAX_RECORD_LEN` (1 MiB) is
  `InvalidData`, so a
  corrupt prefix cannot force a huge allocation; a payload that fails
  bincode decoding is `InvalidData`.
- `LENGTH_PREFIX_BYTES = 4` and `MAX_RECORD_LEN = 1 << 20` are now
  `pub` constants of the shared module.

The existing `train` code then delegates:
[`shard.rs`](../../../../gomoku/crates/train/src/shard.rs)'s
`write_dataset` chunks its samples by `SHARD_SIZE` and calls
`write_shard` per chunk — same file names, same manifest, same
`create_new` protection — and
[`dataset.rs`](../../../../gomoku/crates/train/src/dataset.rs)'s
`read_dataset` calls `read_shard` per shard file. Only the former
production framing reader — `dataset.rs`'s private `read_shard`
function — and its private framing constants are deleted; the
test-local shard-reading helper in `shard.rs`'s tests stays, because
it decodes the written bytes independently of the shared reader. The
extraction is behavior-preserving by construction and by test: the
`train` suite is unchanged and must stay green. The malformed-input
tests of the specification exercise the shared module through
`read_dataset`; they import only `crate::storage::MAX_RECORD_LEN`
for the oversized-prefix fixture rather than importing and calling
every shared function directly. One dependency note: `bincode`
stays a `train` dependency; `selfplay` never encodes directly, so its
manifest gains only `serde_json` and the `train` path dependency.

### 6. `write_corpus_with_options`: validate, protect, write, manifest last

The corpus writer,
[`write_corpus_with_options`](../../../../gomoku/crates/selfplay/src/corpus.rs),
runs four phases in a fixed order, and the order is the protection:

1. **Validation, before anything on disk is touched.** A zero
   `shard_capacity` is `InvalidInput` ("shard capacity must be
   greater than zero"). An empty record slice is `InvalidInput` ("a
   corpus needs at least one game to declare its pairing") — the
   pairing is read from the first record, so a corpus without games
   cannot state its identity. A record whose header configuration
   differs from the first record's is `InvalidInput` ("every game in
   a corpus must use the declared pairing") — one corpus, one
   pairing, enforced at the boundary. Only the zero-capacity
   rejection has a dedicated test
   (`invalid_explicit_overwrite_preserves_the_existing_corpus`); the
   other two are established by the source and documented here
   without a test to quote.
2. **The overwrite gate.** The output directory is inspected with
   `fs::read_dir`. A missing directory is fine. An existing *empty*
   directory is fine. A non-empty directory branches on
   `CorpusWriteOptions.overwrite`: `OverwritePolicy::Refuse` returns
   `AlreadyExists` without touching anything; `OverwritePolicy::Allow`
   removes the entire directory with `fs::remove_dir_all`. Because
   validation runs first, a rejected call never destroys data even
   under `Allow`. `write_corpus(records, out_dir, shard_capacity)` is
   the convenience wrapper that always passes `Refuse` — the safe
   default is the short path.
3. **The shards.** After `fs::create_dir_all`, the records are
   chunked by `shard_capacity` and each chunk becomes one
   `shard-{index:03}.bin` through `train::storage::write_shard`. The
   zero-padded names sort lexicographically in write order, which the
   reader in step 7 relies on. `create_new` inside `write_shard`
   means a name collision is an error, never a silent truncation.
4. **The manifest, last.** The `Manifest` is assembled from what was
   just written — `format_version`, `seeds` in corpus order,
   `pairing` taken from the first record's header configuration,
   `game_count`, `shards` in record order, and `workspace_version`
   from `env!("CARGO_PKG_VERSION")` — and serialized with
   `serde_json::to_writer_pretty` to `manifest.json`, again under
   `create_new`. Writing the manifest last keeps the datagen
   tutorial's crash-safety discipline: a directory with shards but no
   manifest is an interrupted write, and the manifest is never a
   claim about shards that do not exist. The function returns the
   `Manifest` value, so callers hold in memory exactly what is on
   disk.

### 7. `read_corpus` and `read_manifest`: what is checked, what is not

`read_manifest(dir)` opens `manifest.json` and deserializes it; a
missing file is the operating system's error, and malformed JSON is
mapped to `InvalidData`.

`read_corpus(dir)` lists the directory, keeps regular files whose
names start with `shard-` and end with `.bin` (so `manifest.json`
and stray files are ignored), sorts the paths lexicographically, and
extends one `Vec<GameRecord>` by `train::storage::read_shard` per
shard. Framing errors — torn prefixes, oversized prefixes,
undecodable payloads — propagate from step 5. A directory without
matching shards yields an empty vector.

The honesty boundary of this chapter is here, stated plainly:

- **Checked at read time:** framing integrity and decodability —
  the shard mechanics of step 5 — plus the manifest's JSON shape.
- **Not checked anywhere in this tutorial:** the format version. It
  is stamped into every header and manifest, and no reader rejects a
  mismatched one yet; the stamp is the seam for a future rule.
- **Deferred to chapter 07:** semantic validation. `read_corpus`
  does not replay the games: it does not check move legality, does
  not check that the recorded outcome matches the engine's verdict,
  and does not cross-check the manifest against the shards. Those
  checks are the verifier binary's job, specified in the acceptance
  chapter. A corpus that reads back cleanly from this chapter's API
  is *well-formed*, not *true*.

### 8. Why the bytes agree

The round-trip test's second half and the cross-process test pin the
same property at two scales: the bytes of a corpus are a pure
function of the records. The reasons are structural. Records are
written in slice order, so iteration order is fixed. No record field
carries wall-clock time or process state. The two version strings —
`evaluator` in the header and `workspace_version` in the manifest —
come from `env!("CARGO_PKG_VERSION")`, fixed at compile time.
bincode's standard configuration encodes identical values
identically, including the bit patterns of the `f32` prior and
root-value entries, and `serde_json::to_writer_pretty` emits struct
fields in declaration order. The manifest names the shard list, so
the cross-process test compares exactly the files the corpus claims
to contain. Byte-exact regeneration is what makes a seed set plus a
pairing a complete description of a corpus — the property chapter 04
scales up and chapter 07 measures on real games.

## What this chapter defers

- The `From<player::SearchStats>` conversion for `PlySearch`, the
  `RECORDED_PRIOR_TOP_K` prior truncation, the sparse record's
  read-time normalization semantics, and the sign-convention tests —
  chapter 03. This chapter persists chapter 01's raw slot data as-is.
- `ExplorationSchedule::options_for_ply`, the runner's per-ply
  schedule application, tournament opening lists, seeded generation
  with deduplication, and the `selfplaygen` binary — chapter 04.
- Teaching the runner to detect and populate `certified_suffix`
  (the `SuffixTracker`), the `verify_line` re-verification gate, and
  anchor-seed extraction — chapter 05. This chapter adds the record
  slot and nothing else.
- Replay, sampling, source mixing, and export into `train`-format
  sample shards — chapter 06.
- Replay verification of recorded moves and outcomes, corpus
  statistics, and the acceptance gates — chapter 07.

## TDD checklist

Work one artifact at a time, red to green, in
[`gomoku/crates/selfplay/`](../../../../gomoku/crates/selfplay/) and
[`gomoku/crates/train/`](../../../../gomoku/crates/train/). First the
manifest edits: the `selfplay`
[crate manifest](../../../../gomoku/crates/selfplay/Cargo.toml)
gains `serde_json` (workspace) and the `train` path dependency — both
already exist in the workspace, and chapter 01's paper reserved them
for this chapter. The `train` manifest needs nothing; `bincode` is
already there.

1. The `train` extraction. Create `src/storage.rs` with
   `LENGTH_PREFIX_BYTES`, `MAX_RECORD_LEN`, `write_shard`, and
   `read_shard`; register `pub mod storage;` in
   [`train`'s `lib.rs`](../../../../gomoku/crates/train/src/lib.rs).
   Rewire `shard.rs::write_dataset` to chunk and delegate, and
   `dataset.rs::read_dataset` to delegate; delete the private framing
   constants and `dataset.rs`'s private production read function (the
   test-local framing helper in `shard.rs`'s tests stays); point the
   dataset tests at `crate::storage::MAX_RECORD_LEN`. This step is a refactor:
   write no new tests, and watch the existing `cargo test -p train`
   stay green at every commit-sized increment.
2. The record slot on `Game`: the `CertifiedSuffix` type in
   `src/record.rs`, then `Game.certified_suffix` set to `None` by the
   runner. Chapter 01's tests stay green — whole-game equality
   compares `None` to `None`.
3. The serializable seat vocabulary in `src/record.rs`:
   `TssProbeSettings`, `GuardSettings`, `SeatConfig`, and their
   `From` conversions; `SeatPairing`; and the `ExplorationSchedule`
   vocabulary type with `EXPLORATION_PLIES`, its fields, its serde
   derives, and its `Default` — in `src/schedule.rs`, registered and
   re-exported, with its behavior deferred to chapter 04.
4. `GAME_RECORD_FORMAT_VERSION`, `GameHeader`, `RecordedPly`,
   `GameRecord`, the `From<&Game>` implementation, and the
   `GameRecord::from_game` method. Red:
   `game_record_preserves_sparse_visits_including_the_played_move`
   and `game_record_preserves_a_certified_suffix`; then green. This
   is the moment the conversion first runs end to end.
5. `Manifest`, `OverwritePolicy`, `CorpusWriteOptions`,
   `write_corpus_with_options`, and the `write_corpus` wrapper in
   `src/corpus.rs`, registered in
   [`selfplay`'s `lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs).
   Red: `manifest_records_pairing_and_seed_set_and_round_trips`,
   `shard_capacity_two_rolls_over_and_manifest_lists_every_shard`,
   and the first half of
   `populated_search_fields_round_trip_byte_exactly`; then green.
6. `read_manifest` and `read_corpus`. Red: the round-trip half of
   `populated_search_fields_round_trip_byte_exactly`; then green.
7. The overwrite tests:
   `write_refuses_a_non_empty_corpus_directory`,
   `explicit_overwrite_replaces_the_previous_corpus`,
   `invalid_explicit_overwrite_preserves_the_existing_corpus` — red
   only if the gate came after the writing; keep the phase order and
   they pass on first run.
8. The cross-process pair:
   `corpus_writer_process_helper`, then
   `identical_seeds_produce_byte_identical_corpora_across_process_restarts`.

## Capstone

The chapter's summit is already in the default suite:

```text
cargo test -p selfplay --test corpus identical_seeds_produce_byte_identical_corpora_across_process_restarts
```

One test process builds a corpus, a second process builds it again,
and the parent compares every byte — manifest and shards alike. Two
processes, one byte stream: that is the determinism the whole track
stands on, produced entirely by code this chapter owns, in a fraction
of a second, without playing a single game.

## Done when

- `cargo test -p selfplay` reports **13 passed, 2 ignored**: the game
  target keeps its chapter-01 state (3 passed, 2 MANUAL-ignored), and
  the new corpus target reports 10 passed, 0 ignored. (In the
  completed reference the same corpus target reports exactly these 10
  passed; the full reference suite is larger because later chapters
  add their targets.)
- `cargo test -p train` is green with an **unchanged count** — 39
  library tests plus 3 integration tests, none ignored. The
  extraction adds no tests and removes none; the suite itself is the
  proof that the mechanics moved without changing.
- `cargo test -p player` is not required: this chapter does not touch
  the `player` crate.
- `cargo clippy --all-targets -- -D warnings` is green.
- `cargo fmt --all -- --check` makes no changes.
- The commit message is
  `feat(selfplay): recording games — selfplay-corpora chapter 02`.

## Verification commands

From `gomoku/`:

```text
cargo test -p selfplay
cargo test -p selfplay --test corpus
cargo test -p train
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the fast gate; the corpus tests write only
hand-built records to temporary directories and finish in well under
a second, even in a debug build. The second command focuses on this
chapter's target when the suite has grown. No `--release` qualifier
and no `--ignored` selection appear anywhere in this chapter: nothing
here plays a game, and the cross-process determinism test runs in the
default suite because its children write three small records instead
of playing. The third command is the extraction's regression gate.
The `player` regression of chapter 01 is absent on purpose: the
chapter's only cross-crate touch is the `train` extraction.

## References

**This track:**

- [Tutorial README](../README.md) — conventions, gates, and the
  chapter map.
- [Self-play primer](../00-selfplay-primer.md) — §3 pins the record
  format and the store-games rationale; §8 lists the invariants this
  chapter starts to enforce.
- [Chapter 01 — the game runner](../01-the-game-runner/the-game-runner.md) —
  the `Game` this chapter persists, and the serde derives it declared
  for this chapter.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  decisions S2 (store games) and S5 (the pairing is the corpus's
  identity) and §3.2 (the record format).
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  the per-chapter public surface this chapter implements.

**Prerequisite tracks:**

- [Datagen tutorial chapter 05](../../datagen-tutorial/05-shards-and-manifest.md)
  and [chapter 06](../../datagen-tutorial/06-reading-back.md) — the
  shard/manifest write discipline and the read-back defenses this
  chapter extracts into shared mechanics.
- [Baseline tutorial](../../baseline-tutorial/README.md) —
  `PlayerOptions`, `OwnTssConfig`, `GuardConfig`, and the honesty
  rule for certified-proof moves.
- [docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) —
  §9, the store-games replay discipline the record format serves.

**Source:**

- [`gomoku/crates/selfplay/src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs) —
  `GameRecord`, `GameHeader`, `RecordedPly`, `CertifiedSuffix`,
  `SeatPairing`, `SeatConfig`, `TssProbeSettings`, `GuardSettings`,
  `GAME_RECORD_FORMAT_VERSION`, `GameRecord::from_game`. Created in
  this chapter.
- [`gomoku/crates/selfplay/src/corpus.rs`](../../../../gomoku/crates/selfplay/src/corpus.rs) —
  `Manifest`, `OverwritePolicy`, `CorpusWriteOptions`,
  `write_corpus`, `write_corpus_with_options`, `read_corpus`,
  `read_manifest`. Created in this chapter.
- [`gomoku/crates/selfplay/src/schedule.rs`](../../../../gomoku/crates/selfplay/src/schedule.rs) —
  `ExplorationSchedule` and `EXPLORATION_PLIES`. Created in this
  chapter as record vocabulary; behavior arrives in chapter 04.
- [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs) —
  `Game` gains the `certified_suffix` slot. Touched in this chapter.
- [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs) —
  the crate's public surface. Touched in this chapter.
- [`gomoku/crates/selfplay/tests/corpus.rs`](../../../../gomoku/crates/selfplay/tests/corpus.rs) —
  this chapter's specification. Created in this chapter.
- [`gomoku/crates/train/src/storage.rs`](../../../../gomoku/crates/train/src/storage.rs) —
  `write_shard`, `read_shard`, `LENGTH_PREFIX_BYTES`,
  `MAX_RECORD_LEN`. Created in this chapter by extraction.
- [`gomoku/crates/train/src/shard.rs`](../../../../gomoku/crates/train/src/shard.rs)
  and
  [`gomoku/crates/train/src/dataset.rs`](../../../../gomoku/crates/train/src/dataset.rs) —
  `write_dataset` and `read_dataset`, now delegating to the shared
  mechanics. Touched in this chapter.
- [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs) —
  `PlayerOptions`, `OwnTssConfig`, `GuardConfig` — the types the
  record mirrors. Not touched in this chapter.

**Companion papers:**

- [recording-games-design.md](recording-games-design.md) —
  the conceptual story of the record and the corpus.
- [recording-games-implementation.md](recording-games-implementation.md)
  — the hiding layer: low-level construction
  instructions.
- [recording-games-solution.md](recording-games-solution.md)
  — the annotated reference snippets.

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). https://doi.org/10.1038/nature24270 —
  the (s, π, z) record discipline and the temperature and Dirichlet
  noise settings the recorded schedule carries.
- Rosin, C. D. (2011). *Multi-armed bandits with episode context*
  (PUCT). https://doi.org/10.1007/s10472-011-9258-6 — the selection
  rule whose exploration constant `c_puct` the seat configuration
  records.
- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://doi.org/10.26481/dis.19940923la — threat-space search, the
  source of the certified suffixes the record reserves a slot for.
- bincode 2 — https://docs.rs/bincode/2 — the binary encoding of the
  shard payloads.
- serde — https://serde.rs — the serialization framework behind every
  record type and the JSON manifest.
