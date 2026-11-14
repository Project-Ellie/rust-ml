# Chapter 02 — Recording games: implementation plan

## Abstract

This document is the implementation plan for
[chapter 02 of the selfplay-corpora tutorial](recording-games.md): the
hiding layer between the chapter paper and the solution, for the
learner who builds the code without reading the reference. It turns
the chapter's walkthrough into a numbered construction order along the
execution path of one corpus write followed by one read: the crate
manifest, the `train` extraction into a shared `train::storage`
module, the record slot on `Game`, the exploration-schedule
vocabulary, the serializable seat mirrors, the `GameRecord`
conversion, the corpus writer, the readers, the overwrite tests, and
the cross-process determinism pair — ten tests in the new corpus
target, all running by default, plus the unchanged `train` suite as
the extraction's regression gate. Every artifact names its file, its
behavioral contract in prose, the helpers to reuse, and the idioms to
apply by name; method bodies never appear. The intended reader has
completed chapter 01, has read the chapter paper and, ideally, the
[design paper](recording-games-design.md), and wants low-level
guidance without solution leakage.

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
| **Simulation budget** | The number of Monte Carlo Tree Search (MCTS) simulations a seat runs per move decision; written baseline-X for a budget of X. |
| **Ply** | One move played after the opening. Ply indices are zero-based and count post-opening moves only. |
| **Record mirror** | A serializable type in the record's own vocabulary that copies the fields of a gameplay type field for field. Mirrors keep the durable format decoupled from another crate's internals. |
| **Certified suffix** | A terminal segment of a game in which every attacker move belongs to a machine-verified forced-win line produced by the engine's threat-space search (TSS) prover (Allis 1994). This chapter adds the record slot; chapter 05 fills it. |
| **Overwrite policy** | The explicit choice, `OverwritePolicy`, between refusing a non-empty output directory (`Refuse`) and replacing it (`Allow`). |
| **Round trip** | Writing a value and reading it back; a byte-exact round trip additionally requires that re-writing the read-back value reproduces the original file bytes. |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games; it never runs by default and runs on explicit selection, in release mode. This chapter adds none; the two ignored tests in the verification counts belong to chapter 01. |
| **TDD** | Test-driven development: write the test, watch it fail (red), implement, watch it pass (green). |
| **bincode** | The binary encoding library (version 2) that encodes shard payloads under its standard configuration. |
| **serde** | The serialization framework whose `Serialize` and `Deserialize` derives every record type carries; `serde_json` encodes the manifest. |

## How to use this plan

Read [the chapter paper](recording-games.md) first. It carries the
intention, the specification as tests, and the execution-order
walkthrough; the [design paper](recording-games-design.md) carries the
reasoning. This plan assumes both and adds only construction detail:
which file, which item, which contract, which test goes red then
green. Work one step at a time, in order — the order is the execution
order of one corpus write. Open
[recording-games-solution.md](recording-games-solution.md)
only if you are stuck for more than twenty minutes, or after the
chapter is complete.

## Context

Chapter 01 left a working `selfplay` crate: the in-memory `Game` with
its seed, both seats' `PlayerOptions`, the `Opening` with its
provenance, the ordered `PlyRecord` list with its optional `PlySearch`
slots, and the `Outcome`. Nothing was written to disk. This chapter
persists that value. Two existing sources supply the mechanics: the
`train` crate's private framing inside `write_dataset` and
`read_dataset` (length-delimited bincode shards plus a JSON manifest),
and the `player` crate's option types (`PlayerOptions`,
`OwnTssConfig`, `GuardConfig`) that the record mirrors because they
carry no serde derives. The chapter paper's Context section summarizes
each source in full.

Three scope guards keep the chapter slice honest. First, the record
persists chapter 01's raw `PlySearch` slots as-is: no conversion from
`player::SearchStats`, no `RECORDED_PRIOR_TOP_K` constant, and no
prior truncation appear anywhere in this chapter — all three belong
to chapter 03. Second, `ExplorationSchedule` is vocabulary only: the
type, its fields, its serde derives, and its `Default` exist so the
header can record the corpus's identity, but `options_for_ply` and
any per-ply schedule behavior are chapter 04's — do not write them.
Third, `certified_suffix` is a slot: the `Game` gains the field and
the runner initializes it to `None`, but no detection logic appears —
chapter 05 owns that. No corpus generation, no sampler, no export, no
verifier appears either.

---

## Step 0 — `selfplay/Cargo.toml`: the manifest additions

Chapter 01's crate manifest deliberately omitted two dependencies that
this chapter first uses. In
`gomoku/crates/selfplay/Cargo.toml`, add:

1. `serde_json` from the workspace dependency table — the manifest
   encoding. The workspace already pins it; the crate simply opts in.
2. `train` as a path dependency on `../train` — the shared shard
   mechanics of step 1.

One deliberate absence: `bincode` never appears in this manifest. The
`selfplay` crate never encodes or decodes a record directly; all
framing goes through `train::storage`, so the binary encoding stays
`train`'s dependency alone. No workspace manifest change is needed:
chapter 01 already registered the crate member.

Tests that go red then green: none; the manifest change compiles away
until the new modules exist.

---

## Step 1 — the `train` extraction: `train::storage`

The framing that `write_dataset` and `read_dataset` implement
privately becomes a shared module. This step is a refactor: write no
new tests, delete none, and watch the existing `train` suite stay
green at every commit-sized increment. The suite is the proof that the
mechanics moved without changing.

### 1.1 `src/storage.rs`: the shared framing

Create `gomoku/crates/train/src/storage.rs` with a module doc comment
stating the framing contract — length-delimited bincode storage shared
by dataset-like corpora, each record encoded with bincode 2 plus serde
and prefixed by its payload length as a little-endian `u32`, no other
framing — and four public items:

- `LENGTH_PREFIX_BYTES` — a public `usize` constant, the value 4: the
  byte width of one record's little-endian length prefix.
- `MAX_RECORD_LEN` — a public `usize` constant, 1 MiB expressed as a
  bit shift: the maximum payload `read_shard` accepts. Its doc comment
  states the purpose: the cap bounds allocations when a corrupt length
  prefix is encountered, and it is deliberately far larger than any
  legitimate record.
- `write_shard` — a public generic function over any serializable
  record type. It accepts a slice of records and a destination path,
  and returns an `io::Result` of the unit type. Behavioral contract,
  in execution order: open the destination with create-new semantics
  (an existing shard is an error, never a silent truncation) and wrap
  it in a buffered writer; for each record in slice order, encode it
  with bincode 2's serde integration under the standard configuration,
  mapping an encoding failure to `InvalidData`; reject a payload whose
  length does not fit a `u32` with `InvalidData` and a message stating
  that the encoded record exceeds the `u32` length limit; write the
  length as four little-endian bytes, then the payload; flush at the
  end. The doc comment carries the honest caveat: the file is created
  before encoding begins, so a mid-write encoding or I/O failure can
  leave a partial shard behind. The `# Errors` doc section names the
  failure sources: file creation, an over-`u32` record, bincode
  encoding, writing.
- `read_shard` — a public generic function over any
  deserialization-owning record type. It accepts a path and returns an
  `io::Result` of a vector of records. Behavioral contract, in
  execution order: open the file and wrap it in a buffered reader;
  loop over the framing — read up to `LENGTH_PREFIX_BYTES` prefix
  bytes, where zero bytes read is a clean end of shard and one to
  three bytes is a torn prefix whose follow-up exact read surfaces
  `UnexpectedEof` (corruption, never a clean end); interpret the
  prefix as a little-endian `u32` and reject a length above
  `MAX_RECORD_LEN` with `InvalidData` and a message naming both the
  offending length and the cap; read the payload exactly (a truncated
  payload propagates `UnexpectedEof`); decode the payload with bincode
  2's serde integration under the standard configuration, mapping a
  decoding failure to `InvalidData`; collect the records in file
  order. The `# Errors` doc section names the failure sources: opening,
  truncation, an over-cap prefix, bincode decoding.

Apply the existing private implementations in
[`dataset.rs`](../../../../gomoku/crates/train/src/dataset.rs) and
[`shard.rs`](../../../../gomoku/crates/train/src/shard.rs) as the
guide — the extraction moves them; it does not redesign them.

### 1.2 `src/lib.rs`: register the module

In [`train`'s `lib.rs`](../../../../gomoku/crates/train/src/lib.rs),
add the public module declaration for `storage` in the existing
alphabetical module list. The module is public: `selfplay` depends on
it across the crate boundary.

### 1.3 `src/shard.rs`: `write_dataset` delegates

Rewire `write_dataset` so the per-sample streaming loop disappears.
The class-count tally stays unchanged. After it, chunk the sample
slice by `SHARD_SIZE` with the slice-chunking idiom, and delegate each
chunk to `write_shard` under the same zero-padded three-digit
`shard-NNN.bin` naming as before, collecting the names in write order.
The manifest assembly and its create-new write stay exactly as they
are. Same file names, same manifest, same overwrite protection — only
the framing moves. The test-local shard-reading helper in `shard.rs`'s
test module stays: it decodes the written bytes independently of the
shared reader, which is what keeps the round-trip test honest.

### 1.4 `src/dataset.rs`: `read_dataset` delegates

Rewire `read_dataset` to extend its result vector by `read_shard` per
shard file; the directory listing, the shard-name filter, and the
lexicographic sort stay unchanged. Delete the two private framing
constants and the private production framing reader — they now live
in `storage.rs`. In the test module, import `MAX_RECORD_LEN` from
`crate::storage`: the oversized-prefix fixture
(`record_length_cap_rejects_invalid_shard`) references the constant,
and it is the only shared item the tests import — the defenses are
exercised through `read_dataset`, not by calling the storage functions
directly.

### 1.5 Gate

Run `cargo test -p train` from `gomoku/`. The count is unchanged —
39 library tests plus 3 integration tests, none ignored — and green.
Four of those tests are part of this chapter's specification because
the corpus reader inherits or mirrors their behavior:
`dataset::tests::roundtrip_read_back_written_dataset`,
`dataset::tests::record_length_cap_rejects_invalid_shard` (a prefix
above the cap is `InvalidData`, and the message names the cap),
`dataset::tests::torn_length_prefix_is_an_error_not_a_clean_eof` (one
to three leftover prefix bytes are `UnexpectedEof`), and
`dataset::tests::read_empty_dataset_directory_returns_empty_vector`.

Tests that go red then green: none by design. If any existing test
goes red, the extraction changed behavior — fix the extraction, never
the test.

---

## Step 2 — `Game` grows the record slot

Before anything can be recorded, chapter 01's `Game` gains one field.

### 2.1 `src/record.rs`: `CertifiedSuffix`

Create `gomoku/crates/selfplay/src/record.rs` — it will hold all
record vocabulary — and declare the module in `lib.rs`. The module
stays private — no `pub mod` — but the crate-root `pub use`
re-exports are not deferred to one late step: each lands as the
corpus integration tests first need it. Three checkpoints, named
here and repeated where they fire: the record, suffix, schedule, and
seat-mirror re-exports land before step 5's conversion tests can go
green; the writer, options, and manifest re-exports land before the
step-6/7 tests can go green; the reader re-exports land as the
step-7 read-back halves name them. Step 10 is then the final audit
of exactly the public surface, not the first time exports appear.
Define one public struct, `CertifiedSuffix`, with two public fields: `from_ply` (a
`u16`, the zero-based ply from which a terminal segment is certified)
and `line` (a vector of `Move` values, the verified forced-win line).
Derive `Debug`, `Clone`, `PartialEq`, `Eq`, and serde's `Serialize`
and `Deserialize`. The doc comment states the slot's contract: the
detection that populates it is chapter 05's subject.

### 2.2 `src/game.rs`: the slot on `Game`

Add one public field to `Game`, after `outcome`: `certified_suffix`,
an optional `CertifiedSuffix`. Its doc comment states that absence
means no anchor data, never a negative value label. In
`play_game_with_opening`, initialize the slot to `None` in the
returned game — no tracker, no detection, nothing else changes.

Chapter 01's tests stay green without an edit: `Game` derives
`PartialEq`, so the determinism test compares `None` to `None`. This
is why the slot joins the `Game` in this chapter rather than with
chapter 05's detection: from now on the conversion in step 5 is total,
and no chapter ever silently discards something the `Game` knew.

Tests that go red then green: none new; `cargo test -p selfplay`
reports the chapter-01 state, 3 passed and 2 ignored for the game
target.

---

## Step 3 — `src/schedule.rs`: the exploration vocabulary

Create `gomoku/crates/selfplay/src/schedule.rs` and declare the module
in `lib.rs`. This step writes record vocabulary only — the header in
step 5 embeds the schedule, so it must exist first.

### 3.1 `EXPLORATION_PLIES`

A public `usize` constant, the value 10: the number of post-opening
plies that use temperature and root noise under the default schedule.
Its doc comment notes that it mirrors the CLI's first-ten-ply
AI-vs-AI practice.

### 3.2 `ExplorationSchedule`

A public struct with four public fields: `plies` (a `usize`, the
exploration window's length), `temperature` (an `f32`),
`noise_epsilon` (an `f32`, the Dirichlet noise mix fraction), and
`noise_alpha` (an `f32`, the Dirichlet concentration parameter).
Derive `Debug`, `Clone`, `Copy`, `PartialEq`, and serde's `Serialize`
and `Deserialize`.

Implement `Default`: a window of `EXPLORATION_PLIES` plies at
temperature 1.0 with Dirichlet noise ε = 0.25 and α = 0.3 (the
root-noise settings of Silver et al. 2017). This is the only schedule
that exists until chapter 04, and it is what the conversion records.

The boundary, stated once and held: no `options_for_ply` method, no
per-ply application, no schedule parameter on the runner. The type
exists so the header can record the corpus's identity; its behavior
is chapter 04's subject.

Tests that go red then green: none; compilation under
`#![deny(missing_docs)]` is the gate. Document every field as you
write it.

---

## Step 4 — `src/record.rs`: the seat mirrors and the pairing

The record defines its own serializable vocabulary for the seat
configuration, because `PlayerOptions` belongs to the `player` crate
and carries no serde derives. Each mirror copies its gameplay original
field for field, through a plain `From` conversion. Three small
mirrors, then the pairing.

### 4.1 `TssProbeSettings`

A public struct mirroring `player::OwnTssConfig`, with three public
fields: `enabled` (a `bool`), `max_nodes` (a `u32`), and `max_depth`
(a `u8`). Derive `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, and
serde's `Serialize` and `Deserialize`. Implement the `From`
conversion from `OwnTssConfig`: copy `enabled`, and flatten the
embedded engine `SearchBudget` into its `max_nodes` and `max_depth`
fields — the mirror has no nested budget struct.

### 4.2 `GuardSettings`

A public struct mirroring `player::GuardConfig`, with four public
fields: `enabled` (a `bool`), `max_candidates` (a `usize`),
`max_nodes` (a `u32`), and `max_depth` (a `u8`). Same derive list as
`TssProbeSettings`. The `From` conversion copies `enabled` and
`max_candidates` and flattens the budget the same way.

### 4.3 `SeatConfig`

A public struct mirroring one seat's `PlayerOptions`, with eight
public fields: `simulations` (a `u32`), `c_puct` (an `f32` — the
predictor-UCT exploration constant, Rosin 2011), `temperature` (an
`f32`), `noise_epsilon` (an `f32`), `noise_alpha` (an `f32`),
`tss_probe` (a `TssProbeSettings`), `guard` (a `GuardSettings`), and
`resign` (a `bool`). Derive `Debug`, `Clone`, `Copy`, `PartialEq`,
and serde's two traits — not `Eq`, because the `f32` fields forbid
it. The `From<PlayerOptions>` conversion copies every scalar field
verbatim and converts the seat's own-side probe configuration
(`PlayerOptions.own`) and guard configuration through the mirrors of
steps 4.1 and 4.2. Recording the base options as supplied — before
any per-ply adjustment — is what makes the recorded configuration an
honest answer to "what was asked for".

### 4.4 `SeatPairing`

A public struct with three public fields: `black` (a `SeatConfig`),
`white` (a `SeatConfig`), and `exploration` (an
`ExplorationSchedule`). Same derive list as `SeatConfig`. The pairing
is the corpus's declared identity (design spec S5): it lives in every
record header, and step 6's writer enforces its uniformity per corpus.

Tests that go red then green: none; compilation is the gate. The
conversions run for the first time inside step 5's conversion.

---

## Step 5 — `src/record.rs`: `GameRecord` and the conversion (red → green)

This is the chapter's entry point: a pure conversion — no board
replay, no search, no I/O — that is total: every `Game` converts, and
nothing it carries is dropped.

### 5.1 The remaining record vocabulary

- `GAME_RECORD_FORMAT_VERSION` — a public `u16` constant, the value
  1: the durable-layout version, stamped into every header and every
  manifest. A stamp, not yet a rejection rule.
- `GameHeader` — a public struct with five public fields, in this
  order: `format_version` (a `u16`), `seed` (a `u64`), `config` (a
  `SeatPairing`), `evaluator` (a `String`), and `opening` (an
  `Opening`). Derive `Debug`, `Clone`, `PartialEq`, and serde's two
  traits — no `Copy`, because of the owned `String`.
- `RecordedPly` — a public struct with four public fields: `mv` (the
  played `Move`), `visits` (a vector of `(Move, u32)` pairs — the
  sparse raw root visit counts, empty when search did not run),
  `prior` (a vector of `(Move, f32)` pairs — the sparse prior
  entries, also empty when search did not run), and `root_value` (an
  optional `f32`). Same derive list as `GameHeader`. The field docs
  state the honesty rule: normalization into π happens at read time
  (chapters 03 and 06), and a resignation produces no ply and no
  fabricated value.
- `GameRecord` — a public struct with four public fields, in this
  order: `header` (a `GameHeader`), `plies` (a vector of
  `RecordedPly`), `outcome` (an `Outcome`), and `certified_suffix`
  (an optional `CertifiedSuffix`). Same derive list. The
  `PartialEq` derive is what makes the round-trip assertions in the
  tests one-liners.

### 5.2 The tests, red

Create `gomoku/crates/selfplay/tests/corpus.rs` with the three
helpers and the first two tests. For a brand-new conversion the red
state is a compile error, which is expected and correct.

**Helpers.** `TestDir` creates a unique temporary directory per test
— the test name, the process id, a nanosecond timestamp, and an
atomic sequence number combined into one path under the system
temporary directory — and removes the directory on `Drop`, so tests
never collide and never leak directories. `mv(row, col)` accepts two
`u8` coordinates and returns a `Move`, expecting that the test
coordinate is on the board. `minimal_game(seed)` accepts a `u64` seed
and returns a complete six-ply `Game` built by hand: a listed opening
(`OpeningProvenance::Listed` with list name `"test-openings"` and
index 3, answered with `OpeningAnswer::Take(Color::Black)`); six
plies whose first five carry a `PlySearch` — one visit entry naming
the ply's own played move with count 1, two prior entries (the played
move and one audit move at row zero), and a small distinct root value
per ply — and whose sixth ply, the winning move, carries no search,
modeling a certified-proof move that bypassed search; Black
configured at simulation budget 1 and White at budget 2 through the
struct-update-syntax idiom over the default options, so the two seats
are distinguishable in the recorded pairing; an outcome of
`Outcome::Won` with winner Black and reason `WinReason::FiveOrMore`;
and the certified-suffix slot unset.

**`game_record_preserves_sparse_visits_including_the_played_move`.**
Convert `minimal_game(17)` and walk the recorded plies zipped with
the game's plies: for every ply whose search ran, the recorded visit
list is non-empty and contains the played move; for the no-search
ply, the recorded visit list is empty. This pins the as-is rule —
raw slot data is persisted verbatim, and no-search plies fabricate
nothing.

**`game_record_preserves_a_certified_suffix`.** Build a suffix whose
`from_ply` is 0 and whose line is the game's full move list, set it
on the game through the struct-update-syntax idiom, convert, and
assert the record carries exactly that suffix. This pins the slot:
the format already has room for chapter 05's certificates.

### 5.3 The conversion, green

Two items:

- `From<&Game>` for `GameRecord` — the trait implementation that does
  the work. The header takes the version stamp, the game's seed, a
  `SeatPairing` whose seats convert from the game's two option sets
  through step 4's `From` implementations and whose exploration is
  the schedule's `Default`, an evaluator string of the form
  `baseline@<version>` built from the compile-time package version
  that Cargo sets, and the game's opening cloned with its provenance.
  Each ply maps to one `RecordedPly`: the played move always; when
  the ply's search is present, the visits, the prior, and the root
  value copy across unchanged (the optional-mapping idiom with an
  empty-collection fallback keeps this branch-free); when the search
  is absent, empty visits, an empty prior, and no root value. The
  outcome copies, and the certified-suffix slot clones across.
- `GameRecord::from_game` — an inherent method that borrows the game
  and returns the record, marked must-use, delegating to the trait
  implementation. It is the name the chapters and tests call.

`lib.rs` checkpoint: the corpus test file names the record
vocabulary at the crate root, so before these tests can compile
green, add the `pub use` re-exports for what steps 2 to 5 built —
from `record`: `CertifiedSuffix`, `GAME_RECORD_FORMAT_VERSION`,
`GameHeader`, `GameRecord`, `GuardSettings`, `RecordedPly`,
`SeatConfig`, `SeatPairing`, `TssProbeSettings`; from `schedule`:
`EXPLORATION_PLIES`, `ExplorationSchedule`. The chapter-01 items the
fixtures name (`Game`, `PlyRecord`, `PlySearch`, `Outcome`, `WinReason`,
`Opening`, `OpeningAnswer`, `OpeningProvenance`) are already at the
crate root from chapter 01.

Watch both conversion tests go green: the conversion runs end to end.

Scope guard, repeated because the temptation is real: no
`From<player::SearchStats>` conversion, no `RECORDED_PRIOR_TOP_K`
constant, no truncation, no normalization. The record persists
chapter 01's raw slots; chapter 03 owns everything derived from them.

---

## Step 6 — `src/corpus.rs`: the writer (red → green)

Create `gomoku/crates/selfplay/src/corpus.rs` and declare the module
in `lib.rs`. The writer runs four phases in a fixed order — validate,
gate, shards, manifest last — and the order is the protection.

### 6.1 The corpus vocabulary

- `Manifest` — a public struct with six public fields, in this order:
  `format_version` (a `u16`), `seeds` (a vector of `u64` in corpus
  order), `pairing` (a `SeatPairing`), `game_count` (a `usize`),
  `shards` (a vector of `String` file names in record order), and
  `workspace_version` (a `String`). Derive `Debug`, `Clone`,
  `PartialEq`, and serde's two traits. Declaration order matters for
  bytes: the pretty JSON writer emits fields in declaration order,
  which the byte-comparison tests rely on.
- `OverwritePolicy` — a public enum with two variants: `Refuse`
  (refuse every non-empty output directory) and `Allow` (remove the
  existing output directory before writing). Derive `Debug`, `Clone`,
  `Copy`, `PartialEq`, `Eq` — no serde, because the policy never
  crosses into a file, and no `Default` implementation: the safe
  default is embodied in the short-path function below, not in a
  derive.
- `CorpusWriteOptions` — a public struct with two public fields:
  `shard_capacity` (a `usize`, the maximum number of games in one
  shard) and `overwrite` (an `OverwritePolicy`). Same derive list as
  `OverwritePolicy`.

There is deliberately no corpus error enum: every corpus function
returns `io::Result`, and the `io::ErrorKind` carries the meaning.
The shard mechanics already spoke that vocabulary; the corpus layer
inherits it rather than wrapping it.

### 6.2 The tests, red

Three tests go red against a writer that does not exist yet.

**`manifest_records_pairing_and_seed_set_and_round_trips`.** Convert
`minimal_game(4)` and `minimal_game(17)`, write the two records with
`write_corpus` at shard capacity 2, and assert on the returned
`Manifest`: the format version equals `GAME_RECORD_FORMAT_VERSION`;
the seeds are 4 and 17 in corpus order; the pairing equals the first
record's header configuration; the game count is 2; the workspace
version equals the test binary's compile-time package version. Then
read the manifest back with `read_manifest` and assert equality with
the returned value. (The read-back assertion stays red until step 7;
write it now — it is part of this test's contract.)

**`shard_capacity_two_rolls_over_and_manifest_lists_every_shard`.**
Convert five games (seeds 1 through 5), write at shard capacity 2,
and assert: the manifest lists `shard-000.bin`, `shard-001.bin`,
`shard-002.bin` in that order; the game count is 5. The
`read_corpus` half — the five records back, equal and in order —
arrives in step 7. The last shard is short: capacity is a maximum,
not a padding requirement.

**`populated_search_fields_round_trip_byte_exactly`, first half.**
Convert `minimal_game(17)` and pin the header and the per-ply
mapping: the header's format version equals
`GAME_RECORD_FORMAT_VERSION`; the seed equals the game's seed; the
recorded Black and White simulation budgets are 1 and 2; the
evaluator string equals `baseline@<version>` with the compile-time
package version; the opening and the outcome equal the game's; the
record's certified suffix is absent; the ply counts match; at least
one recorded prior list is non-empty; and for every zipped pair of
recorded and played plies, the moves are equal, a searched ply's
visits, prior, and root value equal the search's, and an unsearched
ply has empty visits, an empty prior, and no root value. These
assertions pin step 5's conversion and need no new implementation —
but "pass already" holds only once step 6.3's writer exists and the
test crate compiles; until then the test is red as a whole, and its
write-read-write second half stays red until step 7's readers land.
Write the whole test now, or add the halves as their steps
arrive; either way, never weaken an assertion to reach green.

### 6.3 `write_corpus_with_options`, green

A public free function that accepts a slice of `GameRecord`, an
output directory path, and a `CorpusWriteOptions`, and returns an
`io::Result` of a `Manifest`. Behavioral contract, in the fixed phase
order:

1. **Validation, before anything on disk is touched.** A zero shard
   capacity is `InvalidInput` with the message "shard capacity must
   be greater than zero". An empty record slice is `InvalidInput`
   with the message "a corpus needs at least one game to declare its
   pairing" — the pairing is read from the first record, so a corpus
   without games cannot state its identity. A record whose header
   configuration differs from the first record's is `InvalidInput`
   with the message "every game in a corpus must use the declared
   pairing" — one corpus, one pairing, enforced at the boundary.
   Honesty note: only the zero-capacity rejection has a dedicated
   test (step 8); the other two are established by the source and
   documented in the chapter paper without a test to quote.
2. **The overwrite gate.** Inspect the output directory with the file
   system's directory reader. A missing directory is fine; other
   inspection errors propagate. An existing empty directory is fine.
   A non-empty directory branches on the policy: `Refuse` returns
   `AlreadyExists` with the message "refusing to write a corpus into
   a non-empty directory", without touching anything; `Allow` removes
   the entire directory tree. Because validation runs first, a
   rejected call never destroys data even under `Allow`. State the
   limits honestly in the doc comment: `Allow` is remove-then-write,
   not an atomic swap — a crash between removal and the manifest
   loses the old corpus, and a failed removal can leave the old
   directory partially removed, because directory removal deletes
   entry by entry. Only validation failures before removal guarantee
   the old corpus remains fully intact.
3. **The shards.** Create the directory (recursively), chunk the
   records by capacity with the slice-chunking idiom, and delegate
   each chunk to `train::storage::write_shard` under the zero-padded
   three-digit `shard-NNN.bin` naming, collecting the names in write
   order. The zero padding makes shard indices 0 through 999 sort
   lexicographically in write order — the property the reader relies
   on; no shard-count cap is enforced, so the property is a
   consequence of the padding width, not a guarantee for arbitrarily
   many shards. Create-new semantics inside `write_shard` mean a name
   collision is an error, never a silent truncation.
4. **The manifest, last.** Assemble the `Manifest` from what was just
   written — the version stamp, the seeds in corpus order, the
   pairing from the first record's header, the game count, the shard
   names in record order, and the workspace version from the
   compile-time package version — and serialize it as pretty JSON to
   `manifest.json`, again under create-new, mapping a serialization
   failure to `InvalidData`. Writing the manifest last keeps the
   datagen tutorial's crash-safety discipline: a directory with
   shards but no manifest is an interrupted write, and the manifest
   never claims shards that do not exist. The manifest is a
   completion marker and a provenance summary — not a checksum, not
   a lock, not a durability guarantee. Return the `Manifest` value,
   so the caller holds in memory exactly what is on disk.

The doc comment carries a `# Errors` section naming the failure
sources: validation, directory creation, serialization, writing.

### 6.4 `write_corpus`

A public free function that accepts a record slice, an output
directory path, and a shard capacity, and returns an `io::Result` of
a `Manifest`. It delegates to `write_corpus_with_options` with the
given capacity and `OverwritePolicy::Refuse`. The safe default is the
short path; replacement is always an explicit, typed decision at the
call site.

`lib.rs` checkpoint: add the writer, options, and manifest
re-exports the corpus tests name — from `corpus`: `write_corpus`,
`write_corpus_with_options`, `CorpusWriteOptions`, `OverwritePolicy`,
`Manifest` — so the step-6 tests compile green. The options types
join now even though step 8's overwrite tests are the first to name
them: one writer surface, one checkpoint.

Watch the three step-6.2 tests go green up to their read-back halves.

---

## Step 7 — `src/corpus.rs`: the readers (red → green)

### 7.1 The tests, red

Complete the read-back halves written in step 6.2:
`read_manifest` in the manifest test, and `read_corpus` in the
rollover test (five records back, equal and in order) and in
`populated_search_fields_round_trip_byte_exactly` — whose second
half writes one record, reads it back, requires equality, then
writes the read-back record into a second directory and compares
file bytes: `shard-000.bin` and `manifest.json` must be byte-identical
across the write–read–write cycle. Equality of values is not enough;
the format must also be stable under re-serialization, because
chapter 06 reads corpora and chapter 07 regenerates them. The bytes
agree for structural reasons: records write in slice order, no field
carries wall-clock time or process state, the version strings are
compile-time constants, bincode's standard configuration encodes
identical values identically (including the bit patterns of the
`f32` entries), and the pretty JSON writer emits fields in
declaration order.

### 7.2 `read_manifest`, green

A public free function that accepts a corpus directory path and
returns an `io::Result` of a `Manifest`. Open `manifest.json` in the
directory and deserialize it. A missing file is the operating
system's error, propagated unchanged; malformed JSON is mapped to
`InvalidData`. The `# Errors` doc section says exactly that.

### 7.3 `read_corpus`, green

A public free function that accepts a corpus directory path and
returns an `io::Result` of a vector of `GameRecord`. Behavioral
contract: list the directory (a missing directory is the operating
system's error), keep regular files whose names start with `shard-`
and end with `.bin` — so `manifest.json` and stray files are ignored
— sort the paths lexicographically, and extend one record vector by
`train::storage::read_shard` per shard. A directory without matching
shards yields an empty vector. Framing errors — torn prefixes,
oversized prefixes, undecodable payloads — propagate from the shared
reader unchanged. This directory walk mirrors `read_dataset`'s
discipline rather than inheriting it; the inheritance through
`train::storage` covers the framing defenses, not the listing.

The honesty boundary, stated in the doc comments and held everywhere:
checked at read time are framing integrity, decodability, and — by
`read_manifest` only, which `read_corpus` never calls — the manifest's
JSON shape — nothing more. No reader rejects a mismatched
format version (the stamp is the seam for a future rule, not a rule).
Move legality and outcome correctness are chapter 07's replay
verification; the manifest-versus-shard cross-check is implemented
nowhere in the tutorial — chapter 07's verifier never opens the
manifest, and its acceptance chapter records that limitation. A corpus
that reads back cleanly from this API is *well-formed*, not *true*.

`lib.rs` checkpoint: re-export `read_manifest` and `read_corpus`
from the crate root as the read-back halves name them.

Watch the three read-back halves go green: the corpus round-trips.

---

## Step 8 — the overwrite tests

Three tests pin the gate. With the phase order of step 6.3 in place,
they pass on first run; if any goes red, the phase order drifted —
fix the writer, never the test.

### 8.1 `write_refuses_a_non_empty_corpus_directory`

Create the output directory by hand, place one unrelated file
(`keep.txt` with known contents) in it, and call `write_corpus` with
one record. Assert the error kind is `AlreadyExists`; the file's
contents survive byte for byte; neither `shard-000.bin` nor
`manifest.json` appeared. Refusal happens before anything is written.

### 8.2 `explicit_overwrite_replaces_the_previous_corpus`

Write an initial corpus of three records at capacity 1 (three
shards). Then call `write_corpus_with_options` with
`OverwritePolicy::Allow`, capacity 2, and one replacement record
(seed 9). Assert: the new manifest lists seed 9 and exactly
`shard-000.bin`; `shard-001.bin` no longer exists; `read_corpus`
returns exactly the replacement. `Allow` replaces; it never merges.

### 8.3 `invalid_explicit_overwrite_preserves_the_existing_corpus`

Write an initial one-record corpus (seed 3). Call
`write_corpus_with_options` with `OverwritePolicy::Allow` and a zero
shard capacity. Assert the error kind is `InvalidInput`, and the
existing corpus is fully intact: its manifest still lists seed 3.
Validation precedes the gate, so even an explicit `Allow` cannot
destroy data with a call that was never valid.

---

## Step 9 — the cross-process determinism pair (capstone)

Two tests prove that the bytes of a corpus are a pure function of its
records — the property that makes a seed set plus a pairing a
complete description of a corpus.

### 9.1 `corpus_writer_process_helper`

The child-process entry point, registered as a test so the test
binary can re-execute itself. When the environment variable
`SELFPLAY_CORPUS_TEST_OUTPUT` is unset, it returns immediately — so
it passes trivially in every ordinary run. When the variable is set,
it converts `minimal_game` at seeds 4, 17, and 23 and writes the
three-record corpus at capacity 2 into the directory the variable
names.

### 9.2 `identical_seeds_produce_byte_identical_corpora_across_process_restarts`

The parent. Reserve two `TestDir` directories. Re-execute the current
test binary twice as a child process, filtered to
`corpus_writer_process_helper` with the exact-match flag, passing one
output directory per child through the environment variable; assert
each child exits successfully. Then read the first manifest, assert
the two `manifest.json` files are byte-identical, and for every shard
the manifest names, assert the two processes' files are
byte-identical. The manifest names the shard list, so the comparison
covers exactly the files the corpus claims to contain.

This pair is the in-tests form of the project's determinism gate, and
it is the chapter's capstone. Run it focused:

```text
cargo test -p selfplay --test corpus identical_seeds_produce_byte_identical_corpora_across_process_restarts
```

---

## Step 10 — `src/lib.rs`: audit the public surface, then gates

The three new modules stayed private throughout, and their
re-exports arrived incrementally at the checkpoints of steps 5, 6,
and 7. This step is the final audit, not the first appearance of
exports: the crate root re-exports exactly the chapter's public
surface and nothing more — every listed item present, no unlisted
item added — alongside chapter 01's exports:

- From `corpus`: `CorpusWriteOptions`, `Manifest`, `OverwritePolicy`,
  `read_corpus`, `read_manifest`, `write_corpus`,
  `write_corpus_with_options`.
- From `record`: `CertifiedSuffix`, `GAME_RECORD_FORMAT_VERSION`,
  `GameHeader`, `GameRecord`, `GuardSettings`, `RecordedPly`,
  `SeatConfig`, `SeatPairing`, `TssProbeSettings`.
- From `schedule`: `EXPLORATION_PLIES`, `ExplorationSchedule`.

Deliberate absences: no `RECORDED_PRIOR_TOP_K` (chapter 03), no
schedule behavior beyond `Default` (chapter 04). If anything in the
test file reached for a path deeper than the crate root, fix the
test to use these re-exports.

Then run the gates, from `gomoku/`:

```text
cargo test -p selfplay
cargo test -p selfplay --test corpus
cargo test -p train
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the fast gate: the corpus tests write only
hand-built records to temporary directories and finish in well under
a second, even in a debug build. Cargo prints one report per test
target, never one aggregate line; the chapter's slice is the sum of
the two per-target reports — 3 passed, 2 ignored for the game target
and 10 passed, 0 ignored for the corpus target, 13 passed and 2
ignored in total. The second focuses the chapter's target: 10
passed, 0 ignored. The third is the extraction's regression gate: 39
library tests plus 3 integration tests, unchanged and green. No
`--release` qualifier and no `--ignored` selection appear anywhere in
this chapter: nothing here plays a game. `cargo test -p player` is
not required: this chapter does not touch the `player` crate. The
last two are the project's standing quality gates; both must be green
with no changes.

---

## Done when

- `cargo test -p selfplay` reports **13 passed, 2 ignored**: the game
  target keeps its chapter-01 state (3 passed, 2 MANUAL-ignored), and
  the new corpus target reports 10 passed, 0 ignored.
- `cargo test -p train` is green with an **unchanged count** — 39
  library tests plus 3 integration tests, none ignored. The
  extraction adds no tests and removes none; the suite itself is the
  proof that the mechanics moved without changing.
- The capstone
  `identical_seeds_produce_byte_identical_corpora_across_process_restarts`
  passes under the focused command in step 9.
- `cargo clippy --all-targets -- -D warnings` is green.
- `cargo fmt --all -- --check` makes no changes.
- The commit message is
  `feat(selfplay): recording games — selfplay-corpora chapter 02`.

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `selfplay/Cargo.toml` | `serde_json` (workspace), `train` (path) | no `bincode`, no workspace-manifest change |
| `train/src/storage.rs` | `LENGTH_PREFIX_BYTES`, `MAX_RECORD_LEN`, `write_shard`, `read_shard` | create-new writes; torn prefix is `UnexpectedEof`; over-cap prefix and undecodable payload are `InvalidData`; partial-shard caveat documented |
| `train/src/lib.rs` | `pub mod storage;` | the module is public across the crate boundary |
| `train/src/shard.rs` | `write_dataset` delegates per chunk | same names, same manifest; test-local decoding helper stays |
| `train/src/dataset.rs` | `read_dataset` delegates; private framing deleted; tests import the cap from `crate::storage` | 39 + 3 tests green, count unchanged |
| `selfplay/src/record.rs` | `CertifiedSuffix` | `from_ply`, `line`; serde derives; slot only, no detection |
| `selfplay/src/game.rs` | `Game.certified_suffix`, initialized `None` | chapter-01 tests green untouched |
| `selfplay/src/schedule.rs` | `EXPLORATION_PLIES`, `ExplorationSchedule`, `Default` | vocabulary only; no `options_for_ply` |
| `selfplay/src/record.rs` | `TssProbeSettings`, `GuardSettings`, `SeatConfig`, `SeatPairing` | field-for-field mirrors; budgets flattened; `From` conversions |
| `selfplay/src/record.rs` | `GAME_RECORD_FORMAT_VERSION`, `GameHeader`, `RecordedPly`, `GameRecord`, `From<&Game>`, `from_game` | total pure conversion; raw slots as-is; must-use method delegating to the trait |
| `selfplay/src/corpus.rs` | `Manifest`, `OverwritePolicy`, `CorpusWriteOptions` | declaration order pins the JSON bytes; no `CorpusError` — `io::ErrorKind` carries the meaning |
| `selfplay/src/corpus.rs` | `write_corpus_with_options`, `write_corpus` | validate → gate → shards → manifest last; `Refuse` is the short path; no atomicity claimed |
| `selfplay/src/corpus.rs` | `read_manifest`, `read_corpus` | shard filter plus lexicographic sort; framing errors propagate; well-formed, not true |
| `selfplay/src/lib.rs` | incremental crate-root re-exports, audited in step 10 | exactly the chapter's surface; no `RECORDED_PRIOR_TOP_K` |
| `selfplay/tests/corpus.rs` | `TestDir`, `mv`, `minimal_game` | collision-free temporary directories; hand-built six-ply fixture, seats distinguishable |
| `selfplay/tests/corpus.rs` | ten tests | conversion (2), manifest and rollover (2), byte-exact round trip (1), overwrite (3), cross-process pair (2); all default-run |

## References

**This chapter:**

- [recording-games.md](recording-games.md) — the chapter paper:
  intention, specification as tests, execution-order walkthrough.
- [recording-games-design.md](recording-games-design.md) — the
  conceptual story and the design invariants this plan builds under.
- [recording-games-solution.md](recording-games-solution.md)
  — the annotated reference snippets. Open only after
  twenty stuck minutes, or afterwards to compare.

**This track:**

- [Tutorial README](../README.md) — conventions, gates, and the
  chapter map.
- [Self-play primer](../00-selfplay-primer.md) — §3 pins the record
  format and the store-games rationale; §8 lists the invariants this
  chapter starts to enforce.
- [Chapter 01 — the game runner](../01-the-game-runner/the-game-runner.md),
  its [design paper](../01-the-game-runner/the-game-runner-design.md),
  and its [implementation plan](../01-the-game-runner/the-game-runner-implementation.md)
  — the `Game` this chapter persists, the scoped serde derives
  declared for this chapter, and the manifest reservations this
  chapter redeems.
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

**Source (live on `main`):**

- [`gomoku/crates/train/src/shard.rs`](../../../../gomoku/crates/train/src/shard.rs) —
  `write_dataset`, whose private framing this chapter extracts;
  touched to delegate.
- [`gomoku/crates/train/src/dataset.rs`](../../../../gomoku/crates/train/src/dataset.rs) —
  `read_dataset` and its framing-defense tests; touched to delegate.
- [`gomoku/crates/train/src/lib.rs`](../../../../gomoku/crates/train/src/lib.rs) —
  gains the `storage` module registration.
- [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs) —
  `PlayerOptions` and `OwnTssConfig`, and
  [`gomoku/crates/player/src/guard.rs`](../../../../gomoku/crates/player/src/guard.rs) —
  `GuardConfig`: the types the record mirrors. Not touched in this
  chapter.
- [`gomoku/crates/engine/src/tss.rs`](../../../../gomoku/crates/engine/src/tss.rs) —
  `SearchBudget`, flattened into the seat mirrors. Not touched in
  this chapter.

**Source (created by this chapter; not yet present on `main`):**

- `gomoku/crates/train/src/storage.rs` — `write_shard`, `read_shard`,
  `LENGTH_PREFIX_BYTES`, `MAX_RECORD_LEN`.
- `gomoku/crates/selfplay/src/record.rs` — `GameRecord`,
  `GameHeader`, `RecordedPly`, `CertifiedSuffix`, `SeatPairing`,
  `SeatConfig`, `TssProbeSettings`, `GuardSettings`,
  `GAME_RECORD_FORMAT_VERSION`, `GameRecord::from_game`.
- `gomoku/crates/selfplay/src/corpus.rs` — `Manifest`,
  `OverwritePolicy`, `CorpusWriteOptions`, `write_corpus`,
  `write_corpus_with_options`, `read_corpus`, `read_manifest`.
- `gomoku/crates/selfplay/src/schedule.rs` — `ExplorationSchedule`
  and `EXPLORATION_PLIES`; record vocabulary only.
- `gomoku/crates/selfplay/tests/corpus.rs` — the chapter's
  specification.

**Source (created by chapter 01, touched by this chapter; not yet
present on `main`):**

- `gomoku/crates/selfplay/src/game.rs` — `Game` gains the
  `certified_suffix` slot; the runner initializes it to `None`.
- `gomoku/crates/selfplay/src/lib.rs` — gains the module
  declarations and the incremental re-exports.
- `gomoku/crates/selfplay/Cargo.toml` — gains `serde_json` and the
  `train` path dependency.

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). https://doi.org/10.1038/nature24270 —
  the temperature and Dirichlet noise settings the recorded schedule
  carries.
- Rosin, C. D. (2011). *Multi-armed bandits with episode context*
  (PUCT). https://doi.org/10.1007/s10472-011-9258-6 — the selection
  rule whose exploration constant the seat configuration records.
- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://doi.org/10.26481/dis.19940923la — threat-space search, the
  source of the certified suffixes the record reserves a slot for.
- bincode 2 — https://docs.rs/bincode/2 — the binary encoding of the
  shard payloads.
- serde — https://serde.rs — the serialization framework behind every
  record type and, through serde_json (https://docs.rs/serde_json),
  the JSON manifest.
