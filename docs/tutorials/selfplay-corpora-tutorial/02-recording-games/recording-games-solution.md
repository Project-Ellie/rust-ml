> **Opt-in solution — read it only after attempting the chapter with the
> [implementation plan](recording-games-implementation.md), or after being
> stuck for more than twenty minutes.**
>
> This paper is inspiration, not authority over the learner's own code. The
> learner's implementation on `main` is the reference; the artifacts below
> exist so a stuck or finished learner can compare. No learner source is
> changed by this paper.

# Chapter 02 — Recording games: solution

## Abstract

This paper is the annotated reference companion to
[chapter 02 of the selfplay-corpora tutorial](recording-games.md). It quotes
everything the chapter creates, along the execution path of one corpus write
followed by one read: the two manifest additions to the `selfplay` crate, the
extraction of the `train` crate's private shard framing into the shared
`train::storage` module and the rewiring of `write_dataset` and
`read_dataset`, the record slot on `Game` (`certified_suffix`), the
exploration-schedule record vocabulary, the serializable seat mirrors
(`TssProbeSettings`, `GuardSettings`, `SeatConfig`) and the `SeatPairing`, the
`GameRecord` conversion, the corpus vocabulary (`Manifest`,
`OverwritePolicy`, `CorpusWriteOptions`), the writer
(`write_corpus_with_options` and `write_corpus`), the readers (`read_manifest`
and `read_corpus`), the crate's public surface, and the chapter's
specification — all ten corpus tests with their helpers, plus the four
`train` dataset tests whose framing behavior the corpus reader inherits.
Every snippet carries an explanation, source line numbers, and an exact
provenance note: most fragments are byte-identical to the completed reference
implementation, and the few that are not are the chapter-2 slice forms the
approved papers specify — each such difference is named explicitly. All
chapter gates ran green on the chapter-2 slice on 2026-11-13; the precise
evidence is in the Provenance and Verification sections. The intended reader
is the learner who has read the chapter paper and, ideally, worked the
[implementation plan](recording-games-implementation.md) first.

## Glossary

| Term | Definition |
|------|------------|
| **Game record** | The durable serialization of one complete game: a header (configuration and provenance), the per-ply records, the outcome, and an optional certified suffix. The artifact is `GameRecord`. |
| **Corpus** | A set of game records produced at one declared pairing, stored as shard files plus a manifest in one directory. |
| **Shard** | A fixed-capacity file of length-delimited bincode records; the `train` crate's storage unit, named `shard-NNN.bin`. |
| **Length-delimited framing** | Writing a `u32` little-endian length prefix before each bincode payload, so the reader knows where one record ends and the next begins. |
| **Manifest** | The JSON file next to the shards — the artifact `Manifest`, serialized as `manifest.json`: format version, seed set, pairing, game count, shard list, and workspace version. |
| **Format version** | The durable-layout version number, `GAME_RECORD_FORMAT_VERSION = 1`, stamped into every record header and every manifest so a future layout change is detectable. |
| **Pairing** | The declared Black-vs-White seat configuration of a corpus. The pairing is the corpus's declared identity and lives in every record header. |
| **Seat** | One of the two players of a generated game: a `player::AiPlayer` running the baseline evaluator at a declared simulation budget. |
| **Simulation budget** | The number of Monte Carlo Tree Search (MCTS) simulations a seat runs per move decision. |
| **Ply** | One move played after the opening. Ply indices in this chapter are zero-based and count post-opening moves only. |
| **Record mirror** | A serializable type in the record's own vocabulary that copies the fields of a gameplay type field for field. Mirrors keep the durable format decoupled from another crate's internals. |
| **Certified suffix** | A terminal segment of a game in which every attacker move belongs to a machine-verified forced-win line produced by the engine's threat-space search (TSS) prover. This chapter adds the record slot; chapter 05 fills it. |
| **Overwrite policy** | The explicit choice, `OverwritePolicy`, between refusing a non-empty output directory (`Refuse`, the default) and replacing it (`Allow`). |
| **Round trip** | Writing a value and reading it back; a byte-exact round trip additionally requires that re-writing the read-back value reproduces the original file bytes. |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games. This chapter adds none; the two ignored tests in the verification counts belong to chapter 01. |
| **Provenance** | The recorded origin of the data: the game's seed, the opening's source, and the identity of the evaluator that produced the decisions. |
| **Chapter-2 slice** | The chapter-2 subset of the completed reference: chapters 01 and 02 together, without the later chapters' additions. Defined and verified in the Provenance section. |
| **bincode** | The binary encoding library (version 2) that encodes shard payloads under its standard configuration. |
| **serde** | The serialization framework whose `Serialize` and `Deserialize` derives every record type carries; `serde_json` encodes the manifest. |
| **π (visit-distribution target)** | The normalized root visit-count distribution of the search at a ply; the policy training target (Silver et al. 2017). The record stores the raw counts; normalization belongs to chapters 03 and 06. |
| **z (outcome target)** | The game result in {+1, 0, −1} from the perspective of the side to move at a ply; derived from the stored outcome at read time (chapter 06), not stored per ply. |

## Provenance, the chapter-2 slice, and verification

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
established the provenance discipline this paper continues.

One structural fact shapes everything below: the worktree holds the
**completed** crate — all seven chapters at once. Its
`gomoku/crates/selfplay/src/record.rs` already carries chapter 03's
`RECORDED_PRIOR_TOP_K` constant and top-k prior wording, its
`src/schedule.rs` already carries chapter 04's
`ExplorationSchedule::options_for_ply`, its `src/game.rs` already carries
chapter 03's `From<SearchStats>` conversion, chapter 04's schedule
application, and chapter 05's suffix tracker with the final
`certified_suffix: suffix.finish()` assembly, its `src/lib.rs` declares the
later chapters' modules and re-exports, and the `player` crate there carries
chapter 05's `Decision.own_proof`. Quoting those regions verbatim would leak
later chapters and would contradict this chapter's approved papers, which fix
the chapter-2 form: no `RECORDED_PRIOR_TOP_K`, no `From<SearchStats>`, no
`options_for_ply`, no suffix tracker, and a `None`-initialized slot (see
[the implementation plan](recording-games-implementation.md), steps 2, 3,
and 5). This paper therefore distinguishes two kinds of quotation:

- **Reference-verbatim fragments.** Most of the chapter-2 code is stable:
  the completed reference carries it unchanged. These fragments are quoted
  exactly, and each carries a note naming the reference file and line range
  it was compared against. The comparisons were scripted, not eyeballed;
  every range reported as byte-identical was checked programmatically
  (trailing newline structure included).
- **Chapter-2 slice forms.** Where the completed reference has already grown
  past chapter 02, this paper quotes the chapter-2 form specified by the
  approved papers, and names exactly what the completed reference adds
  later. No later-chapter implementation is quoted.

### The slice was built and run

Quoting a slice the learner could not compile would violate the track's
first rule — a tutorial that does not run error-free is a defect. So the
chapter-2 slice was constructed and gate-verified on 2026-11-13 in a fresh
scratch copy of the reference worktree at `/tmp/selfplay-ch2-verify` (the
reference itself was not touched, and chapter 01's scratch copy at
`/tmp/selfplay-ch1-verify` was not altered — it served as the source of the
chapter-1 slice files only):

1. The reference worktree was copied in full, excluding its `.git` pointer.
2. The `player` crate was restored to its `main` state — verified identical
   to the reference base, since no commit between `9028aa8` and the current
   `main` touches `gomoku/` — and chapter 01's serde touch to `ResignReason`
   was re-applied (one manifest line, one derive list). Chapter 05's
   `player` changes were thereby removed; chapter 02 does not touch `player`.
3. The `train` crate was kept at the reference state: the shared
   `train::storage` extraction *is* this chapter's own change, and no later
   chapter touches `train`.
4. The `selfplay` crate was reduced to the chapter-2 form below: the
   manifest kept (it is already this chapter's complete manifest),
   `src/corpus.rs` and `tests/corpus.rs` kept verbatim (they are entirely
   this chapter's), `src/opening.rs` and `tests/game.rs` restored to the
   chapter-1 slice forms, `src/lib.rs`, `src/game.rs`, `src/record.rs`, and
   `src/schedule.rs` reduced to the chapter-2 slice forms quoted below, and
   the later chapters' modules (`anchor`, `export`, `generate`, `mix`,
   `sample`, `sampler`), binaries, and test files deleted.

The chapter's verification commands were run from the copy's `gomoku/`, with
stdout and stderr saved to `/tmp/selfplay-ch2-verify-logs/` (a scratch path,
not a repository artifact):

```text
cargo test -p selfplay
cargo test -p selfplay --test corpus
cargo test -p selfplay --test corpus identical_seeds_produce_byte_identical_corpora_across_process_restarts
cargo test -p train
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

All six commands passed on the slice; the measured evidence — per-target
counts, timings, and the two count notes that relate the slice's counts to
the completed reference's — is tabled in [§12](#12-verification). The slice
fragments quoted below are exactly the files those runs exercised.

### Reading rules

The section order is the execution order of one corpus write followed by one
read, preceded by the build preparation (the manifest, the `train`
extraction, the record slot) and followed by the public surface, the
specification tests, and the verification commands. This is the
implementation plan's construction order; the chapter walkthrough instead
introduces the shared shard mechanics at first use, so the two papers
diverge in ordering deliberately.

Every code fragment carries a right-aligned `NN |` line-number margin. For
fragments from files this chapter creates, the numbers are the chapter-2
file's own line numbers — the same numbers the learner's new files will
have. For the pre-existing files the chapter edits (the three `train`
sources and chapter 01's `game.rs`), the numbers are the edited file's
line numbers. The margin is the only annotation added; the code text itself
is quoted exactly, and each fragment's provenance note states whether it is
byte-identical to the completed reference (with the reference's line range)
or a chapter-2 slice form (with the exact delta). Blank separators between
lines appear without a margin; no fragment skips a nonblank line inside its
quoted range.

File links in this paper point into the learner's tree on `main`. Links to
files this chapter creates — for example
[`gomoku/crates/selfplay/src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs) —
resolve once chapters 01 and 02 land. The reference worktree is not linked:
it is temporary provenance, deleted once the tutorial is complete on `main`,
and everything needed from it is quoted here. The scratch copy and its logs
(`/tmp/selfplay-ch2-verify`, `/tmp/selfplay-ch2-verify-logs/`) are cited as
evidence of this paper's own verification; they are ephemeral and not part
of the repository.

## 1. The crate manifest: two additions

Chapter 01's manifest deliberately omitted two dependencies, reserving them
for this chapter. `serde_json` (from the workspace table) encodes the
manifest, and the `train` path dependency brings the shared shard mechanics
of section 2 across the crate boundary. One absence stays deliberate:
`bincode` never appears here — `selfplay` never encodes or decodes a record
directly; all framing goes through `train::storage`, so the binary encoding
remains `train`'s dependency alone. No workspace-manifest change is needed:
chapter 01 already registered the member.

**Source:** `gomoku/crates/selfplay/Cargo.toml`, lines 1–16 (the complete
file after the edit). *Created by chapter 01; this chapter appends lines 14
and 16.*

```text
   1 | [package]
   2 | name = "selfplay"
   3 | description = "Synchronous baseline self-play game runner."
   4 | version.workspace = true
   5 | edition.workspace = true
   6 | license.workspace = true

   8 | [dependencies]
   9 | baseline = { path = "../baseline" }
  10 | engine = { path = "../engine" }
  11 | player = { path = "../player" }
  12 | rand = "0.10.3"
  13 | serde = { workspace = true }
  14 | serde_json = { workspace = true }
  15 | thiserror = { workspace = true }
  16 | train = { path = "../train" }
```

*Provenance: byte-identical to the reference worktree's
`gomoku/crates/selfplay/Cargo.toml` lines 1–16 — the completed reference's
manifest is this chapter's; no later chapter extends it. The chapter-1
manifest ended at line 14 (`thiserror`); the edit inserts `serde_json` after
`serde` and appends `train` at the end, keeping the table's ordering.*

## 2. The `train` extraction: `train::storage`

The shard mechanics are not new code. The datagen tutorial built the framing
privately inside `write_dataset` and `read_dataset`; this chapter moves it
behind a shared module so game corpora and tactical datasets use exactly the
same framing, guards, and error behavior — one storage mechanic, two record
types, and neither knows the other exists. The extraction is a refactor: it
adds no tests and removes none, and the unchanged `train` suite is the
regression gate.

### 2.1 `src/storage.rs`: the shared framing

The new module opens with its contract — length-delimited bincode storage
shared by dataset-like corpora, each record encoded with bincode 2 plus
serde and prefixed by its payload length as a little-endian `u32`, no other
framing — followed by the imports and the two public constants.
`LENGTH_PREFIX_BYTES` fixes the four-byte prefix width; `MAX_RECORD_LEN`
caps an accepted payload at 1 MiB so a corrupt prefix cannot force a huge
allocation, deliberately far larger than any legitimate record.

**Source:** [`gomoku/crates/train/src/storage.rs`](../../../../gomoku/crates/train/src/storage.rs),
lines 1–20. *Not yet present on `main`; the chapter creates it.*

```rust
   1 | //! Length-delimited bincode storage shared by dataset-like corpora.
   2 | //!
   3 | //! Each record is encoded with bincode 2 + serde and prefixed by its payload
   4 | //! length as a little-endian `u32`. Shard files contain no other framing.

   6 | use std::fs::File;
   7 | use std::io::{self, BufReader, BufWriter, Read, Write};
   8 | use std::path::Path;

  10 | use serde::Serialize;
  11 | use serde::de::DeserializeOwned;

  13 | /// Number of bytes in a record's little-endian length prefix.
  14 | pub const LENGTH_PREFIX_BYTES: usize = 4;

  16 | /// Maximum payload size accepted by [`read_shard`].
  17 | ///
  18 | /// This bounds allocations when a corrupt length prefix is encountered. It is
  19 | /// deliberately far larger than the small records used by current corpora.
  20 | pub const MAX_RECORD_LEN: usize = 1 << 20;
```

`write_shard` opens its destination with `create_new` semantics — an
existing shard is an error, never a silent truncation — then encodes each
record with bincode 2's serde integration under the standard configuration,
rejects a payload that does not fit a `u32`, and writes prefix then payload
in slice order, flushing at the end. The doc comment carries the honest
caveat: the file is created before encoding begins, so a mid-write encoding
or I/O failure can leave a partial shard behind — the corpus-level manifest
discipline of section 8 is what makes a partial write detectable.

**Source:** `gomoku/crates/train/src/storage.rs`, lines 22–50.

```rust
  22 | /// Write records to one new length-delimited bincode shard.
  23 | ///
  24 | /// The destination is opened with create-new semantics, so an existing shard
  25 | /// is never truncated. The file is created before encoding begins; an encoding
  26 | /// or I/O failure can therefore leave a partial shard behind.
  27 | ///
  28 | /// # Errors
  29 | ///
  30 | /// Returns an error if the file cannot be created, a record exceeds `u32`
  31 | /// length, bincode encoding fails, or writing fails.
  32 | pub fn write_shard<T: Serialize>(records: &[T], path: &Path) -> io::Result<()> {
  33 |     let file = File::options().write(true).create_new(true).open(path)?;
  34 |     let mut writer = BufWriter::new(file);

  36 |     for record in records {
  37 |         let bytes = bincode::serde::encode_to_vec(record, bincode::config::standard())
  38 |             .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
  39 |         let len = u32::try_from(bytes.len()).map_err(|_| {
  40 |             io::Error::new(
  41 |                 io::ErrorKind::InvalidData,
  42 |                 "encoded record exceeds the u32 length limit",
  43 |             )
  44 |         })?;
  45 |         writer.write_all(&len.to_le_bytes())?;
  46 |         writer.write_all(&bytes)?;
  47 |     }

  49 |     writer.flush()
  50 | }
```

`read_shard` loops over the framing: zero prefix bytes read is a clean end
of shard; one to three leftover prefix bytes are a torn record — a shard
truncated mid-write — whose follow-up exact read surfaces `UnexpectedEof`,
never a clean end; a prefix above the cap is `InvalidData`, with a message
naming both the offending length and the cap; a truncated payload propagates
`UnexpectedEof`; and an undecodable payload is `InvalidData`. Records
collect in file order.

**Source:** `gomoku/crates/train/src/storage.rs`, lines 52–85 (to the end
of the file).

```rust
  52 | /// Read every length-delimited bincode record from one shard, in order.
  53 | ///
  54 | /// # Errors
  55 | ///
  56 | /// Returns an error if the file cannot be opened, a prefix or payload is
  57 | /// truncated, a payload exceeds [`MAX_RECORD_LEN`], or bincode decoding fails.
  58 | pub fn read_shard<T: DeserializeOwned>(path: &Path) -> io::Result<Vec<T>> {
  59 |     let file = File::open(path)?;
  60 |     let mut reader = BufReader::new(file);
  61 |     let mut records = Vec::new();
  62 |     let mut len_buf = [0_u8; LENGTH_PREFIX_BYTES];

  64 |     loop {
  65 |         match reader.read(&mut len_buf)? {
  66 |             0 => break,
  67 |             n => reader.read_exact(&mut len_buf[n..])?,
  68 |         }
  69 |         let len = u32::from_le_bytes(len_buf) as usize;
  70 |         if len > MAX_RECORD_LEN {
  71 |             return Err(io::Error::new(
  72 |                 io::ErrorKind::InvalidData,
  73 |                 format!("shard record length {len} exceeds maximum allowed {MAX_RECORD_LEN}"),
  74 |             ));
  75 |         }
  76 |         let mut bytes = vec![0_u8; len];
  77 |         reader.read_exact(&mut bytes)?;
  78 |         let (record, _): (T, usize) =
  79 |             bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
  80 |                 .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
  81 |         records.push(record);
  82 |     }

  84 |     Ok(records)
  85 | }
```

*Provenance: byte-identical to the reference worktree's
`gomoku/crates/train/src/storage.rs` lines 1–20, 22–50, and 52–85 — the
complete file. The function bodies are the moved implementations:
`read_shard`'s loop is the former private `dataset.rs::read_shard`
generalized over the record type, and `write_shard`'s encode-prefix-write
sequence is the former inline loop body of `write_dataset`, likewise
generalized.*

### 2.2 `src/lib.rs`: register the module

One line joins `train`'s module list, in alphabetical position. The module
is public — `selfplay` depends on it across the crate boundary:

**Source:** [`gomoku/crates/train/src/lib.rs`](../../../../gomoku/crates/train/src/lib.rs),
lines 16–26 after the edit.

```rust
  16 | #![deny(missing_docs)]
  17 | pub mod collect;
  18 | pub mod dataset;
  19 | pub mod label;
  20 | pub mod playout;
  21 | pub mod sample;
  22 | pub mod shard;
  23 | pub mod split;
  24 | pub mod storage;

  26 | pub use sample::Sample;
```

*Provenance: byte-identical to the reference worktree's
`gomoku/crates/train/src/lib.rs` lines 16–26. On `main` the module list ends
at `split`; the edit appends `pub mod storage;` (line 24).*

### 2.3 `src/shard.rs`: `write_dataset` delegates

The per-sample streaming loop — the `Option<BufWriter<File>>` state, the
`shard_index`/`shard_count` counters, the inline bincode encode and length
check, the per-record `write_all` calls, and the final flush — disappears.
What stays is the class-count tally, unchanged, and what replaces the
streaming is the slice-chunking idiom: chunk the samples by `SHARD_SIZE`,
delegate each chunk to `write_shard` under the same zero-padded
`shard-NNN.bin` naming, and collect the names in write order. The manifest
assembly and its create-new write below the quoted region stay exactly as
they were. Same file names, same manifest, same overwrite protection — only
the framing moves. The test-local shard-reading helper in `shard.rs`'s test
module stays as well: it decodes the written bytes independently of the
shared reader, which is what keeps the round-trip test honest.

**Source:** [`gomoku/crates/train/src/shard.rs`](../../../../gomoku/crates/train/src/shard.rs),
lines 53–80 after the edit (the `write_dataset` signature, the tally, and
the new delegation; the manifest write follows at lines 82–99, unchanged).

```rust
  53 | pub fn write_dataset(
  54 |     samples: &[Sample],
  55 |     out_dir: &Path,
  56 |     seed: u64,
  57 |     quotas: Quotas,
  58 | ) -> io::Result<Manifest> {
  59 |     fs::create_dir_all(out_dir)?;

  61 |     let mut counts: BTreeMap<String, usize> = BTreeMap::new();
  62 |     counts.insert("win".to_string(), 0);
  63 |     counts.insert("block".to_string(), 0);
  64 |     counts.insert("quiet".to_string(), 0);

  66 |     for sample in samples {
  67 |         match sample.value {
  68 |             1.0 => *counts.get_mut("win").expect("win key inserted above") += 1,
  69 |             -1.0 => *counts.get_mut("block").expect("block key inserted above") += 1,
  70 |             0.0 => *counts.get_mut("quiet").expect("quiet key inserted above") += 1,
  71 |             _ => {}
  72 |         }
  73 |     }

  75 |     let mut shards = Vec::new();
  76 |     for (shard_index, chunk) in samples.chunks(SHARD_SIZE).enumerate() {
  77 |         let name = format!("shard-{shard_index:03}.bin");
  78 |         crate::storage::write_shard(chunk, &out_dir.join(&name))?;
  79 |         shards.push(name);
  80 |     }
```

*Provenance: byte-identical to the reference worktree's
`gomoku/crates/train/src/shard.rs` lines 53–80. On `main` the region between
the tally and the manifest holds the streaming loop this chapter deletes;
the tally itself and everything from the manifest assembly onward are
untouched.*

### 2.4 `src/dataset.rs`: `read_dataset` delegates

The reader keeps its directory listing, its `shard-` prefix / `.bin` suffix
filter, and its lexicographic sort — the corpus reader in section 9 mirrors
exactly this discipline — and the per-shard body becomes one delegation.
The two private framing constants and the private production `read_shard`
function are deleted; they now live in `storage.rs`.

**Source:** [`gomoku/crates/train/src/dataset.rs`](../../../../gomoku/crates/train/src/dataset.rs),
lines 50–70 after the edit.

```rust
  50 | pub fn read_dataset(dir: &Path) -> io::Result<Vec<Sample>> {
  51 |     let mut shard_paths: Vec<_> = fs::read_dir(dir)?
  52 |         .filter_map(|entry| entry.ok().map(|e| e.path()))
  53 |         .filter(|path| {
  54 |             path.is_file()
  55 |                 && path
  56 |                     .file_name()
  57 |                     .and_then(|n| n.to_str())
  58 |                     .map(|n| n.starts_with("shard-") && n.ends_with(".bin"))
  59 |                     .unwrap_or(false)
  60 |         })
  61 |         .collect();
  62 |     shard_paths.sort_unstable();

  64 |     let mut samples = Vec::new();
  65 |     for path in shard_paths {
  66 |         samples.extend(crate::storage::read_shard(&path)?);
  67 |     }

  69 |     Ok(samples)
  70 | }
```

*Provenance: byte-identical to the reference worktree's
`gomoku/crates/train/src/dataset.rs` lines 50–70. On `main` the loop body
opens the file and calls the private framing reader, which is deleted
together with the two private constants; the imports above the quoted
region slim accordingly.*

The test module gains exactly one import — the cap constant, for the
oversized-prefix fixture. It is the only shared item the tests import: the
defenses are exercised through `read_dataset`, not by calling the storage
functions directly.

**Source:** `gomoku/crates/train/src/dataset.rs`, lines 154–160 after the
edit.

```rust
 154 | #[cfg(test)]
 155 | mod tests {
 156 |     use super::*;
 157 |     use crate::collect::{Quotas, collect};
 158 |     use crate::shard::write_dataset;
 159 |     use crate::storage::MAX_RECORD_LEN;
 160 |     use engine::immediate_wins;
```

*Provenance: byte-identical to the reference worktree's
`gomoku/crates/train/src/dataset.rs` lines 154–160; on `main` the import
block is the same minus line 159. The four tests of this module that pin
the corpus reader's inherited behavior are quoted in section 11.*

The extraction's gate is the unchanged suite: `cargo test -p train` reports
39 library tests plus 3 integration tests, none ignored, before and after.

## 3. `Game` grows the record slot

Before anything can be recorded, chapter 01's `Game` gains one field. The
slot joins now — with the record format, not with chapter 05's detection —
so that the conversion of section 6 is total from this chapter onward: a
`Game` never carries information the `GameRecord` would drop.

### 3.1 `CertifiedSuffix`: the slot's type

The type behind the slot is record vocabulary, so it lives in the new
`src/record.rs`: `from_ply`, the zero-based ply from which a terminal
segment is certified, and `line`, the verified forced-win line. The doc
comment states the contract — the detection that populates it is chapter
05's subject.

**Source:** [`gomoku/crates/selfplay/src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs),
lines 143–150. *Not yet present on `main`; the chapter creates it.*

```rust
 143 | /// A machine-certified terminal segment, populated by chapter 5.
 144 | #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
 145 | pub struct CertifiedSuffix {
 146 |     /// Zero-based ply from which the suffix is certified.
 147 |     pub from_ply: u16,
 148 |     /// Moves in the verified forced-win line.
 149 |     pub line: Vec<Move>,
 150 | }
```

*Provenance: byte-identical to the completed reference's `src/record.rs`
lines 150–157.*

### 3.2 The field on `Game`

Chapter 01's import line gains the one new name the runner's return value
needs:

**Source:** [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs),
line 7 after the edit. *Created by chapter 01; touched here.*

```rust
   7 | use crate::{CertifiedSuffix, GeneratedOpening, Opening, OpeningSource};
```

*Provenance: chapter-2 slice form. Chapter 01's line 7 was
`use crate::{GeneratedOpening, Opening, OpeningSource};` — the edit adds
`CertifiedSuffix` in alphabetical position. The completed reference's
imports additionally name `engine::Proof` and `player::SearchStats` (its
lines 2–3) and, in the crate block (its lines 7–10), `ExplorationSchedule`
and `record::RECORDED_PRIOR_TOP_K`, which chapters 03–05 introduce.*

The field itself lands after `outcome`, and its doc comment states the
honesty rule: absence means no anchor data, never a negative value label.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 68–74 after the
edit.

```rust
  68 |     pub plies: Vec<PlyRecord>,
  69 |     /// Terminal winner or draw.
  70 |     pub outcome: Outcome,
  71 |     /// Certified endgame slot: `None` until chapter 5's detection populates
  72 |     /// it. Absence means no anchor data, never a negative value label.
  73 |     pub certified_suffix: Option<CertifiedSuffix>,
  74 | }
```

*Provenance: chapter-2 slice form. The completed reference's `Game` (its
lines 85–91) documents the field as "Latest own-side proof whose line was
followed to completion during play." — the vocabulary of chapter 05's
tracker. The slice documents the slot this chapter actually ships: present
in the record, `None` until chapter 05 populates it.*

### 3.3 The runner initializes the slot

One line in the returned `Game` sets the slot to `None` — no tracker, no
detection, nothing else in the runner changes. Chapter 01's tests stay green
without an edit: `Game` derives `PartialEq`, so the determinism test
compares `None` to `None`.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 194–203 after the
edit (to the end of the file).

```rust
 194 |     Ok(Game {
 195 |         seed,
 196 |         black_options,
 197 |         white_options,
 198 |         opening,
 199 |         plies,
 200 |         outcome,
 201 |         certified_suffix: None,
 202 |     })
 203 | }
```

*Provenance: chapter-2 slice form. The completed reference's assembly (its
lines 249–258) sets `certified_suffix: suffix.finish()` — the tracker's
result — where the slice sets `None`. Everything else in the assembly is
line-for-line identical.*

## 4. `src/schedule.rs`: the exploration vocabulary

The header in section 6 embeds the exploration schedule because the schedule
is part of the corpus's identity — two corpora generated at different
schedules are different data even at equal budgets — so the vocabulary must
exist first. `EXPLORATION_PLIES` fixes the default window at ten
post-opening plies, mirroring the CLI's first-ten-ply AI-vs-AI practice (the
doc comment's bracketed `[experiment]` pointer is the reference's own, into
the design documentation). `ExplorationSchedule` carries the window length,
the temperature, and the two Dirichlet settings; the struct's doc comment
defines what the recorded values mean, and the serde derives let the header
record them.

**Source:** [`gomoku/crates/selfplay/src/schedule.rs`](../../../../gomoku/crates/selfplay/src/schedule.rs),
lines 1–25. *Not yet present on `main`; the chapter creates it.*

```rust
   1 | use serde::{Deserialize, Serialize};

   3 | /// Number of post-opening plies that use temperature and root noise.
   4 | ///
   5 | /// This is the chapter-4 [experiment] schedule boundary, mirroring the CLI's
   6 | /// first-ten-ply AI-vs-AI practice.
   7 | pub const EXPLORATION_PLIES: usize = 10;

   9 | /// Serializable exploration schedule shared by both seats of a corpus.
  10 | ///
  11 | /// Plies `0..plies` after the Swap2 ceremony use the recorded temperature and
  12 | /// Dirichlet settings. Every later ply uses temperature zero and no noise.
  13 | /// Unlike the interactive CLI, corpus generation has no explicit-selection
  14 | /// opt-out and starts from a non-empty Swap2 board rather than an empty root.
  15 | #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
  16 | pub struct ExplorationSchedule {
  17 |     /// Number of post-opening plies with exploration enabled.
  18 |     pub plies: usize,
  19 |     /// Visit-count sampling temperature during the exploration window.
  20 |     pub temperature: f32,
  21 |     /// Root Dirichlet noise mix fraction during the exploration window.
  22 |     pub noise_epsilon: f32,
  23 |     /// Root Dirichlet concentration during the exploration window.
  24 |     pub noise_alpha: f32,
  25 | }
```

The `Default` — a 10-ply window at temperature 1.0 with noise ε = 0.25,
α = 0.3, the root-noise settings of Silver et al. 2017 — is the only
schedule that exists until chapter 04, and it is what the conversion of
section 6 records. The boundary is stated once and held: no
`options_for_ply` method, no per-ply application, no schedule parameter on
the runner — the type exists so the header can record the corpus's identity;
its behavior is chapter 04's subject.

**Source:** `gomoku/crates/selfplay/src/schedule.rs`, lines 27–36 (to the
end of the chapter-2 file).

```rust
  27 | impl Default for ExplorationSchedule {
  28 |     fn default() -> Self {
  29 |         Self {
  30 |             plies: EXPLORATION_PLIES,
  31 |             temperature: 1.0,
  32 |             noise_epsilon: 0.25,
  33 |             noise_alpha: 0.3,
  34 |         }
  35 |     }
  36 | }
```

*Provenance: chapter-2 slice form, in three byte-identical pieces against
the completed reference's `src/schedule.rs`: slice line 1 is reference line
2, slice lines 3–25 are reference lines 4–26, and slice lines 27–36 are
reference lines 48–57. The delta is exactly chapter 04's share: the
reference's line 1 (`use player::PlayerOptions;`, needed only by the
behavior) and its lines 28–46 (the `options_for_ply` method).*

## 5. `src/record.rs`: the seat mirrors and the pairing

The record defines its own serializable vocabulary for the seat
configuration, because `PlayerOptions` belongs to the `player` crate and
carries no serde derives — serializing the gameplay types directly would
couple the durable format to another crate's internals, and the format could
no longer version on its own terms. The file's opening names what the
mirrors need: the engine's `Move` and `SearchBudget` (the mirrors flatten
the embedded budget struct), the three `player` configuration types, the
serde derives, and the crate's own vocabulary — the schedule of section 4
and chapter 01's `Game`, `Opening`, and `Outcome`. Between the imports and
the mirrors sits the format-version constant: the durable-layout version,
stamped into every header and every manifest. It is a stamp, not yet a
rejection rule — no reader in this tutorial rejects a mismatched version;
the stamp is the seam that makes a future rule possible.

**Source:** `gomoku/crates/selfplay/src/record.rs`, lines 1–8.

```rust
   1 | use engine::{Move, SearchBudget};
   2 | use player::{GuardConfig, OwnTssConfig, PlayerOptions};
   3 | use serde::{Deserialize, Serialize};

   5 | use crate::{ExplorationSchedule, Game, Opening, Outcome};

   7 | /// Current durable game-record layout version.
   8 | pub const GAME_RECORD_FORMAT_VERSION: u16 = 1;
```

*Provenance: byte-identical to the completed reference's `src/record.rs`
lines 1–8. The reference continues at its lines 10–15 with
`RECORDED_PRIOR_TOP_K` — chapter 03's prior-truncation constant — which the
chapter-2 form does not have; the version constant is this chapter's only
constant.*

`TssProbeSettings` mirrors `player::OwnTssConfig` field for field, except
that the embedded engine `SearchBudget` is flattened into `max_nodes` and
`max_depth` — the mirror has no nested budget struct. The `From` conversion
destructures the budget with the struct-pattern idiom:

**Source:** `gomoku/crates/selfplay/src/record.rs`, lines 10–33.

```rust
  10 | /// Serializable own-side TSS probe settings for one seat.
  11 | #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  12 | pub struct TssProbeSettings {
  13 |     /// Whether the probe runs before ordinary search.
  14 |     pub enabled: bool,
  15 |     /// Maximum threat-space nodes explored by one probe.
  16 |     pub max_nodes: u32,
  17 |     /// Maximum proof depth in plies.
  18 |     pub max_depth: u8,
  19 | }

  21 | impl From<OwnTssConfig> for TssProbeSettings {
  22 |     fn from(config: OwnTssConfig) -> Self {
  23 |         let SearchBudget {
  24 |             max_nodes,
  25 |             max_depth,
  26 |         } = config.budget;
  27 |         Self {
  28 |             enabled: config.enabled,
  29 |             max_nodes,
  30 |             max_depth,
  31 |         }
  32 |     }
  33 | }
```

*Provenance: byte-identical to the completed reference's `src/record.rs`
lines 17–40.*

`GuardSettings` mirrors `player::GuardConfig` the same way — `enabled` and
`max_candidates` copied, the budget flattened:

**Source:** `gomoku/crates/selfplay/src/record.rs`, lines 35–61.

```rust
  35 | /// Serializable defensive refutation-guard settings for one seat.
  36 | #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  37 | pub struct GuardSettings {
  38 |     /// Whether candidate refutation probes are enabled.
  39 |     pub enabled: bool,
  40 |     /// Maximum candidates inspected after move selection.
  41 |     pub max_candidates: usize,
  42 |     /// Maximum threat-space nodes explored per candidate.
  43 |     pub max_nodes: u32,
  44 |     /// Maximum refutation depth in plies.
  45 |     pub max_depth: u8,
  46 | }

  48 | impl From<GuardConfig> for GuardSettings {
  49 |     fn from(config: GuardConfig) -> Self {
  50 |         let SearchBudget {
  51 |             max_nodes,
  52 |             max_depth,
  53 |         } = config.budget;
  54 |         Self {
  55 |             enabled: config.enabled,
  56 |             max_candidates: config.max_candidates,
  57 |             max_nodes,
  58 |             max_depth,
  59 |         }
  60 |     }
  61 | }
```

*Provenance: byte-identical to the completed reference's `src/record.rs`
lines 42–68.*

`SeatConfig` mirrors one seat's `PlayerOptions`: the five scalars copied
verbatim, the probe and guard converted through the two mirrors above. The
derive list carries serde's two traits but not `Eq` — the `f32` fields
forbid it. Recording the base options as supplied — before any per-ply
adjustment — is what makes the recorded configuration an honest answer to
"what was asked for".

**Source:** `gomoku/crates/selfplay/src/record.rs`, lines 63–98.

```rust
  63 | /// Exact search and move-selection configuration for one seat.
  64 | #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
  65 | pub struct SeatConfig {
  66 |     /// Number of MCTS simulations per decision.
  67 |     pub simulations: u32,
  68 |     /// PUCT exploration constant.
  69 |     pub c_puct: f32,
  70 |     /// Base visit-count sampling temperature; chapter 4's exploration
  71 |     /// schedule adjusts it per ply.
  72 |     pub temperature: f32,
  73 |     /// Dirichlet noise mix fraction.
  74 |     pub noise_epsilon: f32,
  75 |     /// Dirichlet concentration parameter.
  76 |     pub noise_alpha: f32,
  77 |     /// Own-side TSS probe settings.
  78 |     pub tss_probe: TssProbeSettings,
  79 |     /// Defensive refutation-guard settings.
  80 |     pub guard: GuardSettings,
  81 |     /// Whether heuristic resignation is enabled.
  82 |     pub resign: bool,
  83 | }

  85 | impl From<PlayerOptions> for SeatConfig {
  86 |     fn from(options: PlayerOptions) -> Self {
  87 |         Self {
  88 |             simulations: options.simulations,
  89 |             c_puct: options.c_puct,
  90 |             temperature: options.temperature,
  91 |             noise_epsilon: options.noise_epsilon,
  92 |             noise_alpha: options.noise_alpha,
  93 |             tss_probe: options.own.into(),
  94 |             guard: options.guard.into(),
  95 |             resign: options.resign,
  96 |         }
  97 |     }
  98 | }
```

*Provenance: chapter-2 slice form, identical to the completed reference's
`src/record.rs` lines 70–105 except one doc comment: the reference documents
the `temperature` field as "the pairing's exploration schedule overrides
this on every ply (zero after the opening window)" — chapter 04's behavior,
which the slice does not have. The slice documents the base-temperature
meaning and names chapter 04 as the adjuster. The struct, the other fields,
and the `From` conversion are line-for-line identical.*

`SeatPairing` groups the two seat mirrors with the schedule: the declared
Black-vs-White configuration that is the corpus's identity (design spec S5).
It lives in every record header, and section 8's writer enforces its
uniformity per corpus.

**Source:** `gomoku/crates/selfplay/src/record.rs`, lines 100–109.

```rust
 100 | /// The declared Black-vs-White seat pairing.
 101 | #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
 102 | pub struct SeatPairing {
 103 |     /// Base configuration used whenever Black moved.
 104 |     pub black: SeatConfig,
 105 |     /// Base configuration used whenever White moved.
 106 |     pub white: SeatConfig,
 107 |     /// Per-ply selection schedule shared by both seats.
 108 |     pub exploration: ExplorationSchedule,
 109 | }
```

*Provenance: byte-identical to the completed reference's `src/record.rs`
lines 107–116.*

## 6. `src/record.rs`: the header, the recorded ply, and the conversion

`GameHeader` answers the five questions a future reader of the corpus will
ask. `format_version`: *which layout is this?* `seed`: *which game is
this?* — chapter 01 established that one seed fixes the entire game, so the
seed is the record's complete regeneration key. `config`: *who played, at
what strength?* `evaluator`: *whose decisions are these?* — the string
`baseline@<version>`, built from the compile-time crate version, so baseline
records and the records of a future trained network stay distinguishable in
one replay buffer. `opening`: *where did the opening come from?* — cloned
from the game with its provenance, so a listed opening never masquerades as
a generated one. No `Copy` here: the owned `String` forbids it.

**Source:** `gomoku/crates/selfplay/src/record.rs`, lines 111–124.

```rust
 111 | /// Configuration and provenance for one recorded game.
 112 | #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
 113 | pub struct GameHeader {
 114 |     /// Durable layout version.
 115 |     pub format_version: u16,
 116 |     /// Seed of the game's complete RNG stream.
 117 |     pub seed: u64,
 118 |     /// Exact per-seat configuration, including the simulation pairing.
 119 |     pub config: SeatPairing,
 120 |     /// Identity of the evaluator that produced both seats' decisions.
 121 |     pub evaluator: String,
 122 |     /// Complete Swap2 description, including its external-list provenance.
 123 |     pub opening: Opening,
 124 | }
```

*Provenance: byte-identical to the completed reference's `src/record.rs`
lines 118–131.*

`RecordedPly` is one played move as durable data: the move, the sparse raw
root visit counts, the sparse prior entries, and the optional root value.
The field docs carry the honesty rule — empty when search did not run, and
a resignation produces no ply and no fabricated value — and the forward
pointer that normalization into π happens at read time (chapters 03 and 06).

**Source:** `gomoku/crates/selfplay/src/record.rs`, lines 126–141.

```rust
 126 | /// One played move and its durable search targets.
 127 | #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
 128 | pub struct RecordedPly {
 129 |     /// Move played at this ply.
 130 |     pub mv: Move,
 131 |     /// Sparse raw root visit counts for visited children, in descending order.
 132 |     /// Empty when search did not run. Normalization into π happens at read time.
 133 |     pub visits: Vec<(Move, u32)>,
 134 |     /// All root priors after optional Dirichlet noise, in descending order;
 135 |     /// empty when search did not run.
 136 |     pub prior: Vec<(Move, f32)>,
 137 |     /// Root estimate from the recording side-to-move's perspective, or `None`
 138 |     /// when a certified own-side proof bypassed search. A resignation ends the
 139 |     /// game without a played move, so it produces no ply or fabricated value.
 140 |     pub root_value: Option<f32>,
 141 | }
```

*Provenance: chapter-2 slice form, identical to the completed reference's
`src/record.rs` lines 133–148 except the `prior` field's doc comment: the
reference reads "Up to [`RECORDED_PRIOR_TOP_K`] root priors after optional
Dirichlet noise, in descending order; …" — "up to" because chapter 03
truncates the recorded prior. Chapter 02 persists chapter 01's raw slot
as-is, so the slice documents "all root priors". This is the same slice
delta chapter 01's paper recorded on `PlySearch.prior`.*

`GameRecord` is the durable form itself: header, plies, outcome, and the
certified-suffix slot. The outcome needs no mirror — chapter 01 derived
serde on it for exactly this purpose. The `PartialEq` derive is what makes
the round-trip assertions in the tests one-liners.

**Source:** `gomoku/crates/selfplay/src/record.rs`, lines 152–164.

```rust
 152 | /// Durable, serde-serializable form of one complete self-play game.
 153 | #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
 154 | pub struct GameRecord {
 155 |     /// Configuration and provenance.
 156 |     pub header: GameHeader,
 157 |     /// Played post-opening moves in order.
 158 |     pub plies: Vec<RecordedPly>,
 159 |     /// Terminal winner or draw.
 160 |     pub outcome: Outcome,
 161 |     /// Verified own-side forced-win line followed during play. Absence means
 162 |     /// no anchor data, never a negative value label.
 163 |     pub certified_suffix: Option<CertifiedSuffix>,
 164 | }
```

*Provenance: byte-identical to the completed reference's `src/record.rs`
lines 159–171.*

The conversion is the chapter's entry point, and it is pure: no board
replay, no search, no I/O. `from_game` is the `#[must_use]` name the
chapters and tests call; it delegates to the `From<&Game>` implementation
that does the work. The header takes the version stamp, the game's seed, a
`SeatPairing` whose seats convert through section 5's `From`
implementations and whose exploration is the schedule's `Default`, the
evaluator string from `env!("CARGO_PKG_VERSION")`, and the cloned opening.
Each ply maps through the `map_or_else` idiom: a searched ply's visits,
prior, and root value copy across unchanged; an unsearched ply records
empty visits, an empty prior, and no root value. The outcome copies, and
the certified-suffix slot clones across. This is the store-games decision
made mechanical: because chapter 01 kept the opening, every ply, and the
outcome in memory, persistence is a transformation of values, not a
re-computation.

**Source:** `gomoku/crates/selfplay/src/record.rs`, lines 166–208 (to the
end of the chapter-2 file).

```rust
 166 | impl GameRecord {
 167 |     /// Convert chapter 1's in-memory game without replaying or searching.
 168 |     #[must_use]
 169 |     pub fn from_game(game: &Game) -> Self {
 170 |         Self::from(game)
 171 |     }
 172 | }

 174 | impl From<&Game> for GameRecord {
 175 |     fn from(game: &Game) -> Self {
 176 |         Self {
 177 |             header: GameHeader {
 178 |                 format_version: GAME_RECORD_FORMAT_VERSION,
 179 |                 seed: game.seed,
 180 |                 config: SeatPairing {
 181 |                     black: game.black_options.into(),
 182 |                     white: game.white_options.into(),
 183 |                     exploration: ExplorationSchedule::default(),
 184 |                 },
 185 |                 evaluator: format!("baseline@{}", env!("CARGO_PKG_VERSION")),
 186 |                 opening: game.opening.clone(),
 187 |             },
 188 |             plies: game
 189 |                 .plies
 190 |                 .iter()
 191 |                 .map(|ply| RecordedPly {
 192 |                     mv: ply.mv,
 193 |                     visits: ply
 194 |                         .search
 195 |                         .as_ref()
 196 |                         .map_or_else(Vec::new, |search| search.visits.clone()),
 197 |                     prior: ply
 198 |                         .search
 199 |                         .as_ref()
 200 |                         .map_or_else(Vec::new, |search| search.prior.clone()),
 201 |                     root_value: ply.search.as_ref().map(|search| search.root_value),
 202 |                 })
 203 |                 .collect(),
 204 |             outcome: game.outcome,
 205 |             certified_suffix: game.certified_suffix.clone(),
 206 |         }
 207 |     }
 208 | }
```

*Provenance: byte-identical to the completed reference's `src/record.rs`
lines 173–215 — the conversion needs no slice adaptation, because chapter
01's raw slots are exactly what this chapter persists.*

## 7. `src/corpus.rs`: the corpus vocabulary

The corpus module opens with its own imports — the file system, `io`, paths,
the serde derives, and the crate's record vocabulary — and three types.
`Manifest` is the corpus's provenance and shard inventory: format version,
seeds in corpus order, the pairing, the game count, the shard file names in
record order, and the workspace version. Declaration order matters for
bytes: the pretty JSON writer emits fields in declaration order, which the
byte-comparison tests rely on. `OverwritePolicy` is the existing-directory
choice — `Refuse` or `Allow` — with no serde derives, because the policy
never crosses into a file, and no `Default`: the safe default is embodied in
the short-path function below, not in a derive. `CorpusWriteOptions` bundles
the shard capacity and the policy. Note what is absent: there is no
`CorpusError` enum. Every corpus function returns `io::Result`, and the
`io::ErrorKind` carries the meaning — the shard mechanics already spoke that
vocabulary, and the corpus layer inherits it rather than wrapping it.

**Source:** [`gomoku/crates/selfplay/src/corpus.rs`](../../../../gomoku/crates/selfplay/src/corpus.rs),
lines 1–42. *Not yet present on `main`; the chapter creates it.*

```rust
   1 | use std::fs::{self, File};
   2 | use std::io;
   3 | use std::path::Path;

   5 | use serde::{Deserialize, Serialize};

   7 | use crate::{GAME_RECORD_FORMAT_VERSION, GameRecord, SeatPairing};

   9 | /// Corpus provenance and shard inventory.
  10 | #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
  11 | pub struct Manifest {
  12 |     /// Durable game-record layout version.
  13 |     pub format_version: u16,
  14 |     /// Seeds in corpus order; together they are the exact seed set.
  15 |     pub seeds: Vec<u64>,
  16 |     /// Declared Black-vs-White configuration for every game.
  17 |     pub pairing: SeatPairing,
  18 |     /// Number of games across all shards.
  19 |     pub game_count: usize,
  20 |     /// Shard file names in record order.
  21 |     pub shards: Vec<String>,
  22 |     /// Workspace package version shared by the selfplay and train crates.
  23 |     pub workspace_version: String,
  24 | }

  26 | /// Whether an existing corpus directory may be replaced.
  27 | #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  28 | pub enum OverwritePolicy {
  29 |     /// Refuse every non-empty output directory.
  30 |     Refuse,
  31 |     /// Explicitly remove the existing output directory before writing.
  32 |     Allow,
  33 | }

  35 | /// Options controlling shard layout and overwrite protection.
  36 | #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  37 | pub struct CorpusWriteOptions {
  38 |     /// Maximum number of games in one shard.
  39 |     pub shard_capacity: usize,
  40 |     /// Existing-directory behavior.
  41 |     pub overwrite: OverwritePolicy,
  42 | }
```

*Provenance: byte-identical to the completed reference's `src/corpus.rs`
lines 1–42. The whole of `corpus.rs` is this chapter's; no later chapter
touches it.*

## 8. `src/corpus.rs`: the writer

`write_corpus` is the convenience entry: it delegates to
`write_corpus_with_options` with the given capacity and
`OverwritePolicy::Refuse`. The safe default is the short path; replacement
is always an explicit, typed decision at the call site.

**Source:** `gomoku/crates/selfplay/src/corpus.rs`, lines 44–63.

```rust
  44 | /// Write game records with default overwrite protection.
  45 | ///
  46 | /// # Errors
  47 | ///
  48 | /// Returns an error if validation, directory creation, serialization, or
  49 | /// writing fails. A non-empty output directory is always refused.
  50 | pub fn write_corpus(
  51 |     records: &[GameRecord],
  52 |     out_dir: &Path,
  53 |     shard_capacity: usize,
  54 | ) -> io::Result<Manifest> {
  55 |     write_corpus_with_options(
  56 |         records,
  57 |         out_dir,
  58 |         CorpusWriteOptions {
  59 |             shard_capacity,
  60 |             overwrite: OverwritePolicy::Refuse,
  61 |         },
  62 |     )
  63 | }
```

*Provenance: byte-identical to the completed reference's `src/corpus.rs`
lines 44–63.*

`write_corpus_with_options` runs four phases in a fixed order, and the order
is the protection. Phase one is validation, before anything on disk is
touched: a zero capacity is `InvalidInput`; an empty record slice is
`InvalidInput` — the pairing is read from the first record, so a corpus
without games cannot state its identity; and a record whose header
configuration differs from the first record's is `InvalidInput` — one
corpus, one pairing, enforced at the boundary rather than discovered by a
reader months later. The doc comment's overwrite paragraph states the limit
honestly: `Allow` is remove-then-write, not an atomic swap.

**Source:** `gomoku/crates/selfplay/src/corpus.rs`, lines 65–102 (the doc
comment, the signature, and phase one).

```rust
  65 | /// Write game records to fixed-capacity length-delimited bincode shards and a
  66 | /// JSON manifest.
  67 | ///
  68 | /// With [`OverwritePolicy::Allow`], the entire existing output directory is
  69 | /// removed before writing. Without that explicit opt-in, any non-empty
  70 | /// directory is refused before files are changed.
  71 | ///
  72 | /// # Errors
  73 | ///
  74 | /// Returns an error if validation, directory creation, serialization, or
  75 | /// writing fails.
  76 | pub fn write_corpus_with_options(
  77 |     records: &[GameRecord],
  78 |     out_dir: &Path,
  79 |     options: CorpusWriteOptions,
  80 | ) -> io::Result<Manifest> {
  81 |     let shard_capacity = options.shard_capacity;
  82 |     if shard_capacity == 0 {
  83 |         return Err(io::Error::new(
  84 |             io::ErrorKind::InvalidInput,
  85 |             "shard capacity must be greater than zero",
  86 |         ));
  87 |     }
  88 |     let Some(first) = records.first() else {
  89 |         return Err(io::Error::new(
  90 |             io::ErrorKind::InvalidInput,
  91 |             "a corpus needs at least one game to declare its pairing",
  92 |         ));
  93 |     };
  94 |     if records
  95 |         .iter()
  96 |         .any(|record| record.header.config != first.header.config)
  97 |     {
  98 |         return Err(io::Error::new(
  99 |             io::ErrorKind::InvalidInput,
 100 |             "every game in a corpus must use the declared pairing",
 101 |         ));
 102 |     }
```

*Provenance: byte-identical to the completed reference's `src/corpus.rs`
lines 65–102.*

Phases two through four. The overwrite gate inspects the directory with
`fs::read_dir`: a missing directory is fine (the `NotFound` arm), other
inspection errors propagate, an existing empty directory is fine, and a
non-empty directory branches on the policy — `Refuse` returns
`AlreadyExists` without touching anything, `Allow` removes the entire
directory tree. Because validation runs first, a rejected call never
destroys data even under `Allow`; the `entries.next().transpose()?` idiom
surfaces a mid-iteration directory error instead of ignoring it. Phase three
creates the directory, chunks the records by capacity, and delegates each
chunk to `train::storage::write_shard` under the zero-padded three-digit
naming — the padding makes shard indices 0 through 999 sort
lexicographically in write order, which the reader relies on. Phase four
assembles the `Manifest` from what was just written — so it never claims
shards that do not exist — and serializes it as pretty JSON to
`manifest.json`, again under `create_new`. Writing the manifest last keeps
the datagen tutorial's crash-safety discipline: a directory with shards but
no manifest is an interrupted write. The function returns the `Manifest`
value, so the caller holds in memory exactly what is on disk.

**Source:** `gomoku/crates/selfplay/src/corpus.rs`, lines 104–145 (phases
two, three, and four).

```rust
 104 |     match fs::read_dir(out_dir) {
 105 |         Ok(mut entries) => {
 106 |             if entries.next().transpose()?.is_some() {
 107 |                 match options.overwrite {
 108 |                     OverwritePolicy::Refuse => {
 109 |                         return Err(io::Error::new(
 110 |                             io::ErrorKind::AlreadyExists,
 111 |                             "refusing to write a corpus into a non-empty directory",
 112 |                         ));
 113 |                     }
 114 |                     OverwritePolicy::Allow => fs::remove_dir_all(out_dir)?,
 115 |                 }
 116 |             }
 117 |         }
 118 |         Err(error) if error.kind() == io::ErrorKind::NotFound => {}
 119 |         Err(error) => return Err(error),
 120 |     }

 122 |     fs::create_dir_all(out_dir)?;
 123 |     let mut shards = Vec::new();
 124 |     for (shard_index, chunk) in records.chunks(shard_capacity).enumerate() {
 125 |         let name = format!("shard-{shard_index:03}.bin");
 126 |         train::storage::write_shard(chunk, &out_dir.join(&name))?;
 127 |         shards.push(name);
 128 |     }

 130 |     let manifest = Manifest {
 131 |         format_version: GAME_RECORD_FORMAT_VERSION,
 132 |         seeds: records.iter().map(|record| record.header.seed).collect(),
 133 |         pairing: first.header.config,
 134 |         game_count: records.len(),
 135 |         shards,
 136 |         workspace_version: env!("CARGO_PKG_VERSION").to_owned(),
 137 |     };
 138 |     let manifest_file = File::options()
 139 |         .write(true)
 140 |         .create_new(true)
 141 |         .open(out_dir.join("manifest.json"))?;
 142 |     serde_json::to_writer_pretty(manifest_file, &manifest)
 143 |         .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
 144 |     Ok(manifest)
 145 | }
```

*Provenance: byte-identical to the completed reference's `src/corpus.rs`
lines 104–145.*

## 9. `src/corpus.rs`: the readers

`read_manifest` opens `manifest.json` and deserializes it: a missing file is
the operating system's error, propagated unchanged, and malformed JSON maps
to `InvalidData`. `read_corpus` lists the directory, keeps regular files
whose names start with `shard-` and end with `.bin` — so `manifest.json` and
stray files are ignored — sorts the paths lexicographically, and extends one
record vector by `train::storage::read_shard` per shard. A directory without
matching shards yields an empty vector. This directory walk mirrors
`read_dataset`'s discipline rather than inheriting it; the inheritance
through `train::storage` covers the framing defenses, not the listing.

The honesty boundary of the chapter is here, and the doc comments hold it:
checked at read time are framing integrity, decodability, and the manifest's
JSON shape — nothing more. No reader rejects a mismatched format version.
No move legality, no outcome correctness, no manifest-versus-shard
cross-check — those are chapter 07's replay verification. A corpus that
reads back cleanly from this API is *well-formed*, not *true*.

**Source:** `gomoku/crates/selfplay/src/corpus.rs`, lines 147–182 (to the
end of the file).

```rust
 147 | /// Read and deserialize a corpus's `manifest.json`.
 148 | ///
 149 | /// # Errors
 150 | ///
 151 | /// Returns an error if the manifest cannot be opened or contains invalid JSON
 152 | /// for the current manifest layout.
 153 | pub fn read_manifest(dir: &Path) -> io::Result<Manifest> {
 154 |     let file = File::open(dir.join("manifest.json"))?;
 155 |     serde_json::from_reader(file).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
 156 | }

 158 | /// Read all game records from `shard-*.bin` files in lexicographic order.
 159 | ///
 160 | /// # Errors
 161 | ///
 162 | /// Returns an error if the directory or a shard cannot be read, or if a record
 163 | /// has invalid framing or bincode contents.
 164 | pub fn read_corpus(dir: &Path) -> io::Result<Vec<GameRecord>> {
 165 |     let mut shard_paths: Vec<_> = fs::read_dir(dir)?
 166 |         .filter_map(|entry| entry.ok().map(|entry| entry.path()))
 167 |         .filter(|path| {
 168 |             path.is_file()
 169 |                 && path
 170 |                     .file_name()
 171 |                     .and_then(|name| name.to_str())
 172 |                     .is_some_and(|name| name.starts_with("shard-") && name.ends_with(".bin"))
 173 |         })
 174 |         .collect();
 175 |     shard_paths.sort_unstable();

 177 |     let mut records = Vec::new();
 178 |     for path in shard_paths {
 179 |         records.extend(train::storage::read_shard(&path)?);
 180 |     }
 181 |     Ok(records)
 182 | }
```

*Provenance: byte-identical to the completed reference's `src/corpus.rs`
lines 147–182.*

## 10. `src/lib.rs`: the public surface

The crate root keeps chapter 01's doc comment and `#![deny(missing_docs)]`,
declares the five modules in alphabetical order, and curates the re-exports.
The implementation plan lands the re-exports incrementally — the record,
schedule, and seat-mirror names as the conversion tests first need them, the
writer and manifest names as the write tests name them, the readers as the
read-back halves name them — and audits the finished surface last; the
finished file is quoted once, complete. The audit's deliberate absences: no
`RECORDED_PRIOR_TOP_K` (chapter 03), no schedule behavior beyond `Default`
(chapter 04), no later modules.

**Source:** [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs),
lines 1–27 (the complete chapter-2 file). *Created by chapter 01; this
chapter extends it.*

```rust
   1 | //! Synchronous, single-threaded baseline self-play.
   2 | //!
   3 | //! One seeded game runs a complete Swap2 ceremony and then alternates two
   4 | //! persistent baseline players. The result is entirely in memory: opening
   5 | //! provenance, per-seat options, ordered plies, search metadata, and outcome.

   7 | #![deny(missing_docs)]

   9 | mod corpus;
  10 | mod game;
  11 | mod opening;
  12 | mod record;
  13 | mod schedule;

  15 | pub use corpus::{
  16 |     CorpusWriteOptions, Manifest, OverwritePolicy, read_corpus, read_manifest, write_corpus,
  17 |     write_corpus_with_options,
  18 | };
  19 | pub use game::{
  20 |     Game, GameError, Outcome, PlyRecord, PlySearch, WinReason, play_game, play_game_with_opening,
  21 | };
  22 | pub use opening::{GeneratedOpening, Opening, OpeningAnswer, OpeningProvenance, OpeningSource};
  23 | pub use record::{
  24 |     CertifiedSuffix, GAME_RECORD_FORMAT_VERSION, GameHeader, GameRecord, GuardSettings,
  25 |     RecordedPly, SeatConfig, SeatPairing, TssProbeSettings,
  26 | };
  27 | pub use schedule::{EXPLORATION_PLIES, ExplorationSchedule};
```

*Provenance: chapter-2 slice form. The crate doc (lines 1–5) is
byte-identical to the completed reference's `src/lib.rs` lines 1–5. The
reference then declares and re-exports the later chapters' modules as well —
`anchor`, `export`, `generate`, `mix`, `sample`, `sampler`, and the
chapter-04 opening-list names; chapter 02 declares exactly `corpus`,
`game`, `opening`, `record`, and `schedule` and re-exports exactly the
thirty-one names above (chapter 01's thirteen plus this chapter's
eighteen).*

## 11. `tests/corpus.rs` — the specification

The tests are the chapter's specification: ten tests, all running by
default, none ignored. This chapter adds no MANUAL tests, because nothing
here plays a game — every `Game` in the file is built by hand. The file's
lexical order differs from the execution-path order below; all ten bodies
are quoted in full.

The file opens with its imports — the standard-library machinery for the
temporary directories and the child processes, the engine and `player`
vocabulary, and the crate-root re-exports of section 10, never deeper paths
— and its three helpers. `TestDir` creates a unique temporary directory per
test — the test name, the process id, a nanosecond timestamp, and an atomic
sequence number — and removes it on `Drop`, so tests never collide and never
leak directories. `mv(row, col)` validates a test coordinate.
`minimal_game(seed)` builds a complete six-ply `Game` by hand: a listed
opening answered with `OpeningAnswer::Take(Color::Black)`; six plies whose
first five carry a `PlySearch` — one visit entry naming the ply's own played
move with count 1, two prior entries (the played move and one audit move at
row zero), and a small distinct root value per ply — and whose sixth ply,
the winning move, carries no search, modeling a certified-proof move that
bypassed search. Black is configured at budget 1 and White at budget 2
through the struct-update-syntax idiom, so the two seats are distinguishable
in the recorded pairing; the outcome is Black's win by five-or-more; and the
certified-suffix slot is unset. The hand-built fixtures are deliberate: the
`From<player::SearchStats>` conversion is chapter 03's subject, so no
fixture goes near a real search.

**Source:** [`gomoku/crates/selfplay/tests/corpus.rs`](../../../../gomoku/crates/selfplay/tests/corpus.rs),
lines 1–108. *Not yet present on `main`; the chapter creates it.*

```rust
   1 | use std::path::{Path, PathBuf};
   2 | use std::process::Command;
   3 | use std::sync::atomic::{AtomicU64, Ordering};
   4 | use std::time::{SystemTime, UNIX_EPOCH};

   6 | use engine::{Color, Move};
   7 | use player::PlayerOptions;
   8 | use selfplay::{
   9 |     CertifiedSuffix, CorpusWriteOptions, GAME_RECORD_FORMAT_VERSION, Game, GameRecord, Opening,
  10 |     OpeningAnswer, OpeningProvenance, Outcome, OverwritePolicy, PlyRecord, PlySearch, WinReason,
  11 |     read_corpus, read_manifest, write_corpus, write_corpus_with_options,
  12 | };

  14 | static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

  16 | struct TestDir(PathBuf);

  18 | impl TestDir {
  19 |     fn new(name: &str) -> Self {
  20 |         let nanos = SystemTime::now()
  21 |             .duration_since(UNIX_EPOCH)
  22 |             .expect("system clock is after the Unix epoch")
  23 |             .as_nanos();
  24 |         let sequence = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
  25 |         let path = std::env::temp_dir().join(format!(
  26 |             "selfplay-corpus-{name}-{}-{nanos}-{sequence}",
  27 |             std::process::id()
  28 |         ));
  29 |         let _ = std::fs::remove_dir_all(&path);
  30 |         Self(path)
  31 |     }

  33 |     fn path(&self) -> &Path {
  34 |         &self.0
  35 |     }
  36 | }

  38 | impl Drop for TestDir {
  39 |     fn drop(&mut self) {
  40 |         let _ = std::fs::remove_dir_all(&self.0);
  41 |     }
  42 | }

  44 | fn mv(row: u8, col: u8) -> Move {
  45 |     Move::new(row, col).expect("test coordinate is on the board")
  46 | }

  48 | fn minimal_game(seed: u64) -> Game {
  49 |     let opening = Opening::new(
  50 |         [mv(5, 5), mv(10, 10), mv(5, 6)],
  51 |         OpeningAnswer::Take(Color::Black),
  52 |         OpeningProvenance::Listed {
  53 |             list: "test-openings".to_owned(),
  54 |             index: 3,
  55 |         },
  56 |     );
  57 |     let moves = [
  58 |         mv(10, 11),
  59 |         mv(5, 7),
  60 |         mv(10, 12),
  61 |         mv(5, 8),
  62 |         mv(10, 13),
  63 |         mv(5, 9),
  64 |     ];
  65 |     let plies = moves
  66 |         .into_iter()
  67 |         .enumerate()
  68 |         .map(|(ply, mv)| PlyRecord {
  69 |             mv,
  70 |             // The winning ply models a certified TSS bypass: no search ran.
  71 |             search: (ply != 5).then(|| PlySearch {
  72 |                 side_to_move: if ply % 2 == 0 {
  73 |                     Color::White
  74 |                 } else {
  75 |                     Color::Black
  76 |                 },
  77 |                 visits: vec![(mv, 1)],
  78 |                 prior: vec![
  79 |                     (mv, 0.75),
  80 |                     (
  81 |                         Move::new(0, ply as u8).expect("audit coordinate is on the board"),
  82 |                         0.25,
  83 |                     ),
  84 |                 ],
  85 |                 root_value: ply as f32 / 10.0,
  86 |             }),
  87 |         })
  88 |         .collect();

  90 |     Game {
  91 |         seed,
  92 |         black_options: PlayerOptions {
  93 |             simulations: 1,
  94 |             ..PlayerOptions::default()
  95 |         },
  96 |         white_options: PlayerOptions {
  97 |             simulations: 2,
  98 |             ..PlayerOptions::default()
  99 |         },
 100 |         opening,
 101 |         plies,
 102 |         outcome: Outcome::Won {
 103 |             winner: Color::Black,
 104 |             reason: WinReason::FiveOrMore,
 105 |         },
 106 |         certified_suffix: None,
 107 |     }
 108 | }
```

*Provenance: byte-identical to the completed reference's `tests/corpus.rs`
lines 1–108 — the whole test file is this chapter's; the completed reference
carries it unchanged.*

### Recording one game: the conversion tests

The first conversion test pins the as-is rule: chapter 01's raw slot data is
persisted verbatim, and a ply with no search produces no fabricated data.
For every ply whose search ran, the recorded visit list is non-empty and
contains the played move — meaningful because the fixture's one visit entry
*is* the played move; for the no-search ply, the recorded list is empty.

**Source:** `gomoku/crates/selfplay/tests/corpus.rs`, lines 244–261.

```rust
 244 | #[test]
 245 | fn game_record_preserves_sparse_visits_including_the_played_move() {
 246 |     let record = GameRecord::from_game(&minimal_game(17));

 248 |     for (recorded, played) in record.plies.iter().zip(&minimal_game(17).plies) {
 249 |         if played.search.is_some() {
 250 |             assert!(!recorded.visits.is_empty());
 251 |             assert!(
 252 |                 recorded
 253 |                     .visits
 254 |                     .iter()
 255 |                     .any(|(candidate, _)| *candidate == recorded.mv)
 256 |             );
 257 |         } else {
 258 |             assert!(recorded.visits.is_empty());
 259 |         }
 260 |     }
 261 | }
```

*Provenance: byte-identical to the completed reference's `tests/corpus.rs`
lines 244–261.*

The second conversion test pins the slot: a `Game` whose `certified_suffix`
is set by hand — a suffix starting at ply 0 whose line is the game's full
move list — converts into a `GameRecord` carrying exactly that suffix. The
format already has room for chapter 05's certificates; detection and
extraction are chapter 05's subject.

**Source:** `gomoku/crates/selfplay/tests/corpus.rs`, lines 263–275.

```rust
 263 | #[test]
 264 | fn game_record_preserves_a_certified_suffix() {
 265 |     let suffix = CertifiedSuffix {
 266 |         from_ply: 0,
 267 |         line: minimal_game(17).moves().collect(),
 268 |     };
 269 |     let game = Game {
 270 |         certified_suffix: Some(suffix.clone()),
 271 |         ..minimal_game(17)
 272 |     };

 274 |     assert_eq!(GameRecord::from_game(&game).certified_suffix, Some(suffix));
 275 | }
```

*Provenance: byte-identical to the completed reference's `tests/corpus.rs`
lines 263–275.*

### Writing shards and the manifest

The manifest test writes two records — seeds 4 and 17, shard capacity 2 —
and asserts on the returned `Manifest`: the format version is the constant,
the seeds are in corpus order, the pairing equals the first record's header
configuration, the game count is 2, and the workspace version equals the
test binary's own compile-time package version. Then `read_manifest` returns
an equal value — the value on disk round-trips into the value in memory.

**Source:** `gomoku/crates/selfplay/tests/corpus.rs`, lines 110–124.

```rust
 110 | #[test]
 111 | fn manifest_records_pairing_and_seed_set_and_round_trips() {
 112 |     let out = TestDir::new("manifest");
 113 |     let records = [4_u64, 17].map(|seed| GameRecord::from_game(&minimal_game(seed)));

 115 |     let manifest = write_corpus(&records, out.path(), 2).expect("corpus write succeeds");
 116 |     let read_back = read_manifest(out.path()).expect("manifest read succeeds");

 118 |     assert_eq!(manifest.format_version, GAME_RECORD_FORMAT_VERSION);
 119 |     assert_eq!(manifest.seeds, vec![4, 17]);
 120 |     assert_eq!(manifest.pairing, records[0].header.config);
 121 |     assert_eq!(manifest.game_count, 2);
 122 |     assert_eq!(manifest.workspace_version, env!("CARGO_PKG_VERSION"));
 123 |     assert_eq!(read_back, manifest);
 124 | }
```

*Provenance: byte-identical to the completed reference's `tests/corpus.rs`
lines 110–124.*

The rollover test writes five records at shard capacity 2 and asserts the
layout: three shards listed in the manifest as `shard-000.bin`,
`shard-001.bin`, `shard-002.bin`, in that order; a game count of 5; and
`read_corpus` returning the five records equal and in order. The last shard
is short — capacity is a maximum, not a padding requirement.

**Source:** `gomoku/crates/selfplay/tests/corpus.rs`, lines 185–198.

```rust
 185 | #[test]
 186 | fn shard_capacity_two_rolls_over_and_manifest_lists_every_shard() {
 187 |     let out = TestDir::new("rollover");
 188 |     let records = [1_u64, 2, 3, 4, 5].map(|seed| GameRecord::from_game(&minimal_game(seed)));

 190 |     let manifest = write_corpus(&records, out.path(), 2).expect("corpus write succeeds");

 192 |     assert_eq!(
 193 |         manifest.shards,
 194 |         vec!["shard-000.bin", "shard-001.bin", "shard-002.bin"]
 195 |     );
 196 |     assert_eq!(read_corpus(out.path()).unwrap(), records);
 197 |     assert_eq!(manifest.game_count, 5);
 198 | }
```

*Provenance: byte-identical to the completed reference's `tests/corpus.rs`
lines 185–198.*

### Reading back: the byte-exact round trip

The chapter's longest test has two halves. The first half pins the header
and the per-ply mapping: version stamp, seed, the two seat budgets (1 and
2), the evaluator string against the compile-time package version, the
opening and the outcome equal to the game's, the absent suffix, matching ply
counts, at least one non-empty recorded prior, and — for every zipped pair —
equal moves, with a searched ply's visits, prior, and root value equal to
the search's and an unsearched ply recording empty visits, an empty prior,
and no root value. The second half writes the record, reads it back equal,
then writes the read-back record into a second directory and compares file
bytes: `shard-000.bin` and `manifest.json` must be byte-identical across the
write–read–write cycle. Equality of values is not enough; the format must
also be stable under re-serialization, because chapter 06 reads corpora and
chapter 07 regenerates them.

**Source:** `gomoku/crates/selfplay/tests/corpus.rs`, lines 277–325 (to the
end of the file).

```rust
 277 | #[test]
 278 | fn populated_search_fields_round_trip_byte_exactly() {
 279 |     let out = TestDir::new("roundtrip");
 280 |     let game = minimal_game(17);
 281 |     let record = GameRecord::from_game(&game);

 283 |     assert_eq!(record.header.format_version, GAME_RECORD_FORMAT_VERSION);
 284 |     assert_eq!(record.header.seed, game.seed);
 285 |     assert_eq!(record.header.config.black.simulations, 1);
 286 |     assert_eq!(record.header.config.white.simulations, 2);
 287 |     assert_eq!(
 288 |         record.header.evaluator,
 289 |         format!("baseline@{}", env!("CARGO_PKG_VERSION"))
 290 |     );
 291 |     assert_eq!(record.header.opening, game.opening);
 292 |     assert_eq!(record.outcome, game.outcome);
 293 |     assert!(record.certified_suffix.is_none());
 294 |     assert_eq!(record.plies.len(), game.plies.len());
 295 |     assert!(record.plies.iter().any(|ply| !ply.prior.is_empty()));
 296 |     for (recorded, played) in record.plies.iter().zip(&game.plies) {
 297 |         assert_eq!(recorded.mv, played.mv);
 298 |         match &played.search {
 299 |             Some(search) => {
 300 |                 assert_eq!(recorded.visits, search.visits);
 301 |                 assert_eq!(recorded.prior, search.prior);
 302 |                 assert_eq!(recorded.root_value, Some(search.root_value));
 303 |             }
 304 |             None => {
 305 |                 assert!(recorded.visits.is_empty());
 306 |                 assert!(recorded.prior.is_empty());
 307 |                 assert_eq!(recorded.root_value, None);
 308 |             }
 309 |         }
 310 |     }

 312 |     write_corpus(std::slice::from_ref(&record), out.path(), 2).expect("corpus write succeeds");
 313 |     let read_back = read_corpus(out.path()).expect("corpus read succeeds");
 314 |     assert_eq!(read_back, vec![record]);

 316 |     let rewritten = TestDir::new("roundtrip-rewritten");
 317 |     write_corpus(&read_back, rewritten.path(), 2).expect("read-back record writes again");
 318 |     for file_name in ["shard-000.bin", "manifest.json"] {
 319 |         assert_eq!(
 320 |             std::fs::read(out.path().join(file_name)).unwrap(),
 321 |             std::fs::read(rewritten.path().join(file_name)).unwrap(),
 322 |             "{file_name} changed after a write-read-write round trip"
 323 |         );
 324 |     }
 325 | }
```

*Provenance: byte-identical to the completed reference's `tests/corpus.rs`
lines 277–325.*

### Malformed input: the inherited framing defenses

The corpus reader inherits its framing defenses from
`train::storage::read_shard`, and the tests pinning that behavior live with
the `train` suite — four already-existing tests that are part of this
chapter's specification. The first is the round trip the extraction must
preserve:

**Source:** `gomoku/crates/train/src/dataset.rs`, lines 186–207.

```rust
 186 |     #[test]
 187 |     fn roundtrip_read_back_written_dataset() {
 188 |         let samples = tiny_dataset();

 190 |         let out_dir = unique_temp_dir("train-dataset-roundtrip");
 191 |         let _ = fs::remove_dir_all(&out_dir);
 192 |         fs::create_dir_all(&out_dir).unwrap();
 193 |         let quotas = Quotas {
 194 |             win: 2,
 195 |             block: 2,
 196 |             quiet: 2,
 197 |         };
 198 |         write_dataset(&samples, &out_dir, 42, quotas).unwrap();

 200 |         let read_back = read_dataset(&out_dir).unwrap();
 201 |         assert_eq!(
 202 |             read_back, samples,
 203 |             "Read samples should match written samples."
 204 |         );

 206 |         let _ = fs::remove_dir_all(&out_dir);
 207 |     }
```

*Provenance: byte-identical to the reference worktree's
`gomoku/crates/train/src/dataset.rs` lines 186–207 — and to `main`'s; the
extraction does not touch the test bodies.*

The cap test builds a shard whose first length prefix exceeds
`MAX_RECORD_LEN` — the one constant the test module imports from the shared
module — and requires `InvalidData` with a message naming the cap:

**Source:** `gomoku/crates/train/src/dataset.rs`, lines 308–325.

```rust
 308 |     #[test]
 309 |     fn record_length_cap_rejects_invalid_shard() {
 310 |         let out_dir = unique_temp_dir("train-dataset-cap");
 311 |         let _ = fs::remove_dir_all(&out_dir);
 312 |         fs::create_dir_all(&out_dir).unwrap();

 314 |         let bad_len = (MAX_RECORD_LEN + 1) as u32;
 315 |         fs::write(out_dir.join("shard-000.bin"), bad_len.to_le_bytes()).unwrap();

 317 |         let err = read_dataset(&out_dir).expect_err("should reject oversized length prefix");
 318 |         assert_eq!(err.kind(), io::ErrorKind::InvalidData);
 319 |         assert!(
 320 |             err.to_string().contains("exceeds maximum allowed"),
 321 |             "error should name the cap: {err}"
 322 |         );

 324 |         let _ = fs::remove_dir_all(&out_dir);
 325 |     }
```

*Provenance: byte-identical to the reference worktree's
`gomoku/crates/train/src/dataset.rs` lines 308–325 — and to `main`'s, apart
from the import that brings `MAX_RECORD_LEN` into scope (section 2.4).*

The torn-prefix test writes two bytes of a four-byte prefix and requires
`UnexpectedEof` — a truncated record is corruption, never a clean end of
shard:

**Source:** `gomoku/crates/train/src/dataset.rs`, lines 343–357.

```rust
 343 |     #[test]
 344 |     fn torn_length_prefix_is_an_error_not_a_clean_eof() {
 345 |         let out_dir = unique_temp_dir("train-dataset-torn");
 346 |         let _ = fs::remove_dir_all(&out_dir);
 347 |         fs::create_dir_all(&out_dir).unwrap();

 349 |         // Two bytes of a four-byte length prefix: a truncated record,
 350 |         // not the end of the shard.
 351 |         fs::write(out_dir.join("shard-000.bin"), [7u8, 0]).unwrap();

 353 |         let err = read_dataset(&out_dir).expect_err("torn prefix must be rejected");
 354 |         assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);

 356 |         let _ = fs::remove_dir_all(&out_dir);
 357 |     }
```

*Provenance: byte-identical to the reference worktree's
`gomoku/crates/train/src/dataset.rs` lines 343–357 — and to `main`'s.*

The empty-directory test pins that reading a directory without shards yields
an empty vector. `read_corpus` does not inherit that behavior from
`read_dataset`; it mirrors it, applying the same filter and sort to its own
listing, so an empty corpus directory likewise reads back empty:

**Source:** `gomoku/crates/train/src/dataset.rs`, lines 359–369.

```rust
 359 |     #[test]
 360 |     fn read_empty_dataset_directory_returns_empty_vector() {
 361 |         let out_dir = unique_temp_dir("train-dataset-empty");
 362 |         let _ = fs::remove_dir_all(&out_dir);
 363 |         fs::create_dir_all(&out_dir).unwrap();

 365 |         let samples = read_dataset(&out_dir).unwrap();
 366 |         assert!(samples.is_empty());

 368 |         let _ = fs::remove_dir_all(&out_dir);
 369 |     }
```

*Provenance: byte-identical to the reference worktree's
`gomoku/crates/train/src/dataset.rs` lines 359–369 — and to `main`'s.*

### Overwrite protection

Three tests pin the gate. The refusal test places one unrelated file in the
output directory and requires `AlreadyExists` from the default
`write_corpus` — with the file's contents surviving byte for byte and
neither a shard nor a manifest appearing. Refusal happens before anything is
written.

**Source:** `gomoku/crates/selfplay/tests/corpus.rs`, lines 126–144.

```rust
 126 | #[test]
 127 | fn write_refuses_a_non_empty_corpus_directory() {
 128 |     let out = TestDir::new("overwrite-protection");
 129 |     std::fs::create_dir_all(out.path()).expect("test directory is created");
 130 |     std::fs::write(out.path().join("keep.txt"), b"do not overwrite")
 131 |         .expect("existing file is created");
 132 |     let record = GameRecord::from_game(&minimal_game(17));

 134 |     let error = write_corpus(&[record], out.path(), 2)
 135 |         .expect_err("a non-empty corpus directory must be refused");

 137 |     assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
 138 |     assert_eq!(
 139 |         std::fs::read(out.path().join("keep.txt")).expect("existing file survives"),
 140 |         b"do not overwrite"
 141 |     );
 142 |     assert!(!out.path().join("shard-000.bin").exists());
 143 |     assert!(!out.path().join("manifest.json").exists());
 144 | }
```

*Provenance: byte-identical to the completed reference's `tests/corpus.rs`
lines 126–144.*

The replacement test writes an initial corpus of three records at capacity
1 — three shards — then overwrites explicitly with one replacement record at
capacity 2. The new manifest lists seed 9 and exactly `shard-000.bin`;
`shard-001.bin` no longer exists; `read_corpus` returns exactly the
replacement. `Allow` replaces; it never merges.

**Source:** `gomoku/crates/selfplay/tests/corpus.rs`, lines 200–222.

```rust
 200 | #[test]
 201 | fn explicit_overwrite_replaces_the_previous_corpus() {
 202 |     let out = TestDir::new("explicit-overwrite");
 203 |     let old_records = [1_u64, 2, 3].map(|seed| GameRecord::from_game(&minimal_game(seed)));
 204 |     write_corpus(&old_records, out.path(), 1).expect("initial corpus write succeeds");
 205 |     assert_eq!(read_manifest(out.path()).unwrap().shards.len(), 3);

 207 |     let replacement = GameRecord::from_game(&minimal_game(9));
 208 |     let manifest = write_corpus_with_options(
 209 |         std::slice::from_ref(&replacement),
 210 |         out.path(),
 211 |         CorpusWriteOptions {
 212 |             shard_capacity: 2,
 213 |             overwrite: OverwritePolicy::Allow,
 214 |         },
 215 |     )
 216 |     .expect("explicit overwrite succeeds");

 218 |     assert_eq!(manifest.seeds, vec![9]);
 219 |     assert_eq!(manifest.shards, vec!["shard-000.bin"]);
 220 |     assert!(!out.path().join("shard-001.bin").exists());
 221 |     assert_eq!(read_corpus(out.path()).unwrap(), vec![replacement]);
 222 | }
```

*Provenance: byte-identical to the completed reference's `tests/corpus.rs`
lines 200–222.*

The invalid-overwrite test aims a zero shard capacity at a live corpus under
an explicit `Allow`. The rejection is `InvalidInput`, and the existing
corpus is fully intact afterwards — its manifest still lists the original
seed. Validation precedes the gate, so even an explicit `Allow` cannot
destroy data with a call that was never valid.

**Source:** `gomoku/crates/selfplay/tests/corpus.rs`, lines 224–242.

```rust
 224 | #[test]
 225 | fn invalid_explicit_overwrite_preserves_the_existing_corpus() {
 226 |     let out = TestDir::new("invalid-overwrite");
 227 |     let original = GameRecord::from_game(&minimal_game(3));
 228 |     write_corpus(std::slice::from_ref(&original), out.path(), 1).expect("initial corpus writes");

 230 |     let error = write_corpus_with_options(
 231 |         &[original],
 232 |         out.path(),
 233 |         CorpusWriteOptions {
 234 |             shard_capacity: 0,
 235 |             overwrite: OverwritePolicy::Allow,
 236 |         },
 237 |     )
 238 |     .expect_err("zero shard capacity is rejected");

 240 |     assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
 241 |     assert_eq!(read_manifest(out.path()).unwrap().seeds, vec![3]);
 242 | }
```

*Provenance: byte-identical to the completed reference's `tests/corpus.rs`
lines 224–242.*

### Determinism across process restarts

The cross-process pair is the in-tests form of the project's determinism
gate: same seeds, byte-identical corpus, across process restarts. The helper
is the child-process entry point, registered as a test so the test binary
can re-execute itself; it passes trivially — returning immediately — in
every ordinary run, and only writes the three-record corpus (seeds 4, 17,
23; capacity 2) when its environment variable names an output directory.

**Source:** `gomoku/crates/selfplay/tests/corpus.rs`, lines 146–153.

```rust
 146 | #[test]
 147 | fn corpus_writer_process_helper() {
 148 |     let Some(out) = std::env::var_os("SELFPLAY_CORPUS_TEST_OUTPUT") else {
 149 |         return;
 150 |     };
 151 |     let records = [4_u64, 17, 23].map(|seed| GameRecord::from_game(&minimal_game(seed)));
 152 |     write_corpus(&records, Path::new(&out), 2).expect("child-process corpus write succeeds");
 153 | }
```

*Provenance: byte-identical to the completed reference's `tests/corpus.rs`
lines 146–153.*

The parent reserves two directories, re-executes the current test binary
twice with the helper's exact name filter and one output directory per
child, asserts both children exit successfully, and then compares bytes: the
two `manifest.json` files, and every shard the manifest names. The manifest
names the shard list, so the comparison covers exactly the files the corpus
claims to contain. The bytes agree for structural reasons: records write in
slice order, no field carries wall-clock time or process state, the two
version strings are compile-time constants, bincode's standard configuration
encodes identical values identically (including the `f32` bit patterns), and
the pretty JSON writer emits fields in declaration order. Byte-exact
regeneration is what makes a seed set plus a pairing a complete description
of a corpus — the property chapter 04 scales up and chapter 07 measures on
real games.

**Source:** `gomoku/crates/selfplay/tests/corpus.rs`, lines 155–183.

```rust
 155 | #[test]
 156 | fn identical_seeds_produce_byte_identical_corpora_across_process_restarts() {
 157 |     let first = TestDir::new("process-determinism-a");
 158 |     let second = TestDir::new("process-determinism-b");
 159 |     let executable = std::env::current_exe().expect("test binary path is available");

 161 |     for out in [&first, &second] {
 162 |         let status = Command::new(&executable)
 163 |             .arg("corpus_writer_process_helper")
 164 |             .arg("--exact")
 165 |             .env("SELFPLAY_CORPUS_TEST_OUTPUT", out.path())
 166 |             .status()
 167 |             .expect("writer child process starts");
 168 |         assert!(status.success(), "writer child process failed: {status}");
 169 |     }

 171 |     let manifest = read_manifest(first.path()).expect("first manifest reads");
 172 |     assert_eq!(
 173 |         std::fs::read(first.path().join("manifest.json")).unwrap(),
 174 |         std::fs::read(second.path().join("manifest.json")).unwrap()
 175 |     );
 176 |     for shard in manifest.shards {
 177 |         assert_eq!(
 178 |             std::fs::read(first.path().join(&shard)).unwrap(),
 179 |             std::fs::read(second.path().join(&shard)).unwrap(),
 180 |             "shard {shard} differs across process restarts"
 181 |         );
 182 |     }
 183 | }
```

*Provenance: byte-identical to the completed reference's `tests/corpus.rs`
lines 155–183.*

## 12. Verification

The chapter's commands, run from `gomoku/`:

```text
cargo test -p selfplay
cargo test -p selfplay --test corpus
cargo test -p selfplay --test corpus identical_seeds_produce_byte_identical_corpora_across_process_restarts
cargo test -p train
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the fast gate; cargo prints one report per test target,
never one aggregate line, and the chapter's slice is the sum of the per-target
reports. The second focuses the chapter's target; the third is the capstone
— one test process builds a corpus, a second builds it again, and the parent
compares every byte. The fourth is the extraction's regression gate. No
`--release` qualifier and no `--ignored` selection appear anywhere in this
chapter: nothing here plays a game, and the cross-process determinism test
runs in the default suite because its children write three small records
instead of playing. `cargo test -p player` is not required: this chapter
does not touch the `player` crate.

Evidence actually measured for this paper (2026-11-13, on the chapter-2
slice constructed in the scratch copy described under Provenance — not a
re-run of the completed reference, whose gate evidence the
[hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md)
records; logs at `/tmp/selfplay-ch2-verify-logs/`, scratch-local and
ephemeral):

| Command | Result on the chapter-2 slice |
|---------|-------------------------------|
| `cargo test -p selfplay` | lib target: 0 tests; corpus target: 10 passed, 0 failed, 0 ignored (0.01 s); game target: 3 passed, 0 failed, 2 ignored (12.46 s, debug build) — **13 passed, 2 ignored** in total |
| `cargo test -p selfplay --test corpus` | 10 passed, 0 failed, 0 ignored (0.01 s) |
| `cargo test -p selfplay --test corpus identical_seeds_produce_byte_identical_corpora_across_process_restarts` | 1 passed (0.01 s); both child processes wrote their corpus and exited successfully (their `corpus_writer_process_helper ... ok` lines appear interleaved in the parent log, because the children inherit its stdio) |
| `cargo test -p train` | 39 passed, 0 failed, 0 ignored for the library target (0.19 s); 3 passed for the integration target (0.01 s) — the unchanged count |
| `cargo clippy --all-targets -- -D warnings` | clean, workspace-wide |
| `cargo fmt --all -- --check` | no changes |

Two count notes keep the evidence precise. First, the game target's 3
passed, 2 ignored is chapter 01's slice state carried forward unchanged —
this chapter's runner edit adds a field, not a test, and the chapter-1
MANUAL tests stay ignored; in the completed reference the same target
reports 5 passed, 2 ignored because chapter 03 adds two tests. Second, the
corpus target's 10 passed is the same count in the slice and in the
completed reference, because the test file is byte-identical between them
(scripted comparison); the reference's larger full suite — 50 passed, 4
ignored — is entirely later chapters' targets plus chapter 05's two lib
unit tests. Both MANUAL game tests from chapter 01 remain available for
explicit release-mode runs, but no run of them is part of this chapter's
gates.

## References

**This chapter:**

- [recording-games.md](recording-games.md) — the chapter paper: intention,
  specification as tests, execution-order walkthrough.
- [recording-games-design.md](recording-games-design.md) — the conceptual
  story: mirrors, totality, the writer's phase order, the honesty boundary.
- [recording-games-implementation.md](recording-games-implementation.md) —
  the hiding layer this paper quotes the construction contracts of; its
  steps 2, 3, and 5 fix the chapter-2 slice forms.

**This track:**

- [Tutorial README](../README.md) — conventions, gates, the chapter map,
  and the reference-provenance rule this paper's Provenance section
  applies.
- [Self-play primer](../00-selfplay-primer.md) — §3 pins the record format
  and the store-games rationale; §8 lists the invariants this chapter
  starts to enforce.
- [Chapter 01 — the game runner](../01-the-game-runner/the-game-runner.md)
  and its [solution](../01-the-game-runner/the-game-runner-solution.md) —
  the `Game` this chapter persists, the serde derives chapter 01 declared
  for this chapter, and the provenance discipline this paper continues.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  decisions S2 (store games) and S5 (the pairing is the corpus's identity)
  and §3.2 (the record format).
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  the reference-implementation layout and the measured evidence of the
  completed tree.

**Prerequisite tracks:**

- [Datagen tutorial chapter 05](../../datagen-tutorial/05-shards-and-manifest.md)
  and [chapter 06](../../datagen-tutorial/06-reading-back.md) — the
  shard/manifest write discipline and the read-back defenses this chapter
  extracts into shared mechanics.
- [Baseline tutorial](../../baseline-tutorial/README.md) —
  `PlayerOptions`, `OwnTssConfig`, `GuardConfig`, and the honesty rule for
  certified-proof moves.
- [docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) —
  §9, the store-games replay discipline the record format serves.

**Source (live on `main`):**

- [`gomoku/crates/train/src/shard.rs`](../../../../gomoku/crates/train/src/shard.rs) —
  `write_dataset`, whose private framing this chapter extracts; touched to
  delegate (section 2.3).
- [`gomoku/crates/train/src/dataset.rs`](../../../../gomoku/crates/train/src/dataset.rs) —
  `read_dataset` and its framing-defense tests; touched to delegate
  (sections 2.4 and 11).
- [`gomoku/crates/train/src/lib.rs`](../../../../gomoku/crates/train/src/lib.rs) —
  gains the `storage` module registration (section 2.2).
- [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs)
  and [`gomoku/crates/player/src/guard.rs`](../../../../gomoku/crates/player/src/guard.rs) —
  `PlayerOptions`, `OwnTssConfig`, `GuardConfig` — the types the record
  mirrors. Not touched in this chapter.
- [`gomoku/crates/engine/src/tss.rs`](../../../../gomoku/crates/engine/src/tss.rs) —
  `SearchBudget`, flattened into the seat mirrors. Not touched in this
  chapter.

**Source (created by this chapter; the links resolve once the chapter lands
on `main`):**

- [`gomoku/crates/train/src/storage.rs`](../../../../gomoku/crates/train/src/storage.rs) —
  `write_shard`, `read_shard`, `LENGTH_PREFIX_BYTES`, `MAX_RECORD_LEN`
  (section 2.1).
- [`gomoku/crates/selfplay/src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs) —
  the record vocabulary and the conversion (sections 3, 5, and 6).
- [`gomoku/crates/selfplay/src/schedule.rs`](../../../../gomoku/crates/selfplay/src/schedule.rs) —
  the exploration vocabulary (section 4).
- [`gomoku/crates/selfplay/src/corpus.rs`](../../../../gomoku/crates/selfplay/src/corpus.rs) —
  the corpus vocabulary, the writer, the readers (sections 7–9).
- [`gomoku/crates/selfplay/tests/corpus.rs`](../../../../gomoku/crates/selfplay/tests/corpus.rs) —
  the chapter's specification (section 11).

**Source (created by chapter 01, touched by this chapter; the links resolve
once the chapters land on `main`):**

- [`gomoku/crates/selfplay/Cargo.toml`](../../../../gomoku/crates/selfplay/Cargo.toml) —
  gains `serde_json` and the `train` path dependency (section 1).
- [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs) —
  `Game` gains the `certified_suffix` slot; the runner initializes it to
  `None` (section 3).
- [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs) —
  gains the module declarations and the re-exports (section 10).

**Reference provenance:** the completed reference implementation lives in
the read-only sibling worktree `rust-ml-selfplay-ref` (branch
`selfplay-ref-v2`, base `9028aa8`, changes uncommitted); it is quoted and
compared against, never linked, and it is deleted once the tutorial is
complete on `main`. The chapter-2 slice forms in this paper were constructed
per the implementation plan in a fresh scratch copy of that worktree
(`/tmp/selfplay-ch2-verify`) and gate-verified there on 2026-11-13; the
reference worktree itself and chapter 01's scratch copy
(`/tmp/selfplay-ch1-verify`) were not modified.

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). <https://doi.org/10.1038/nature24270> — the
  (s, π, z) record discipline and the temperature and Dirichlet noise
  settings the recorded schedule carries.
- Rosin, C. D. (2011). *Multi-armed bandits with episode context* (PUCT).
  <https://doi.org/10.1007/s10472-011-9258-6> — the selection rule whose
  exploration constant `c_puct` the seat configuration records.
- Allis, L. V. (1994). *Searching for Solutions in Games and Artificial
  Intelligence*. PhD thesis, Maastricht University.
  <https://doi.org/10.26481/dis.19940923la> — threat-space search, the
  source of the certified suffixes the record reserves a slot for.
- bincode 2 — <https://docs.rs/bincode/2> — the binary encoding of the
  shard payloads.
- serde — <https://serde.rs> — the serialization framework behind every
  record type and, through serde_json (<https://docs.rs/serde_json>), the
  JSON manifest.
