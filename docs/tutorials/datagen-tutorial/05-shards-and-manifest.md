# Chapter 05 — Streaming shard writer + manifest

## Abstract

This chapter adds the first file I/O to the `train` crate: a streaming
shard writer that persists the collected [`Sample`](01-the-train-crate.md)
records to length-delimited bincode files, plus a JSON manifest that
records provenance (seed, quotas, workspace version) and per-class counts.
By the end you will have a deterministic `write_dataset` function, four
unit tests, and the reasoning needed to defend the streaming format
against the obvious alternative of one giant file.

## Glossary

| Term | Definition |
|------|------------|
| **Shard** | One bincode file containing a stream of `Sample` records, at most [`SHARD_SIZE`] per file. |
| **Manifest** | The small JSON file next to the shards: seed, quotas, class counts, shard list, and workspace version. |
| **Length-delimited framing** | Writing a `u32` little-endian length prefix before each bincode record so the reader knows where one record ends and the next begins. |
| **Streaming serialization** | Encoding and writing records one at a time, without holding the entire dataset in a single buffer. |
| **Crash safety** | The property that a partial write leaves previously completed shards intact and readable. |
| **Provenance** | Metadata that explains how a dataset was produced, so regeneration or debugging is possible. |
| **`create_new`** | A `std::fs::OpenOptions` flag that fails if the target file already exists; prevents silent overwrites. |

## Context

Chapters 1–4 built the in-memory datagen pipeline:
[`Sample`](01-the-train-crate.md),
[`random_game` / `sample_plies`](02-random-playouts.md),
[`classify` / `label`](03-tactics-labels.md), and the
[`collect`](04-the-quota-collector.md) quota collector. This chapter
persists that output. The result is the dataset format that the
`net` tutorial will read: a directory of shards plus a manifest.

The design is locked in
[`docs/specs/2026-09-27-datagen-tutorial-design.md`](../../specs/2026-09-27-datagen-tutorial-design.md)
and
[`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md):
store positions (not planes), use bincode 2, and keep shards
append-friendly and crash-safe.

## Intention

1. Add `serde::Serialize` and `serde::Deserialize` to
   [`Quotas`](04-the-quota-collector.md) in `collect.rs` so it can live
   in the manifest.
2. Create `gomoku/crates/train/src/shard.rs` and register
   `pub mod shard;` in `gomoku/crates/train/src/lib.rs`.
3. Define `pub const SHARD_SIZE: usize = 4096`.
4. Define `Manifest { seed, quotas, counts, shards, workspace_version }`
   with serde derives.
5. Implement `write_dataset(samples, out_dir, seed, quotas) -> io::Result<Manifest>`:
   * Create `out_dir` if it does not exist.
   * Fail if any shard file or `manifest.json` already exists (use
     `create_new`).
   * Rollover to `shard-000.bin`, `shard-001.bin`, ... every
     [`SHARD_SIZE`] records.
   * Encode each sample with bincode, prefix with a little-endian `u32`
     length, and write to the current shard.
   * Count samples by class and write `manifest.json` via `serde_json`.
6. Inherit the workspace package version in `crates/train/Cargo.toml`
   and add `serde_json` as a workspace dependency.
7. Write five tests: ten samples produce exactly one shard plus a
   manifest, the shard decodes back to the same samples, `SHARD_SIZE + 1`
   samples roll over into exactly two shards, an existing shard file
   causes an error, and an existing `manifest.json` causes an error.

Observable done-state: `cargo test -p train` passes, `cargo clippy
--all-targets -- -D warnings` is green, and `cargo fmt --all --
--check` makes no changes.

## Mental mapping

### Why streaming shards, not one big file?

The collector could build one enormous `Vec<Sample>` and serialize it
with a single `bincode::serde::encode_to_vec` call. That is simpler to
write, but it has two problems that matter for a training pipeline:

* **Memory.** A milestone-3 dataset is on the order of 50–100 k
  samples (chapter 8). Each sample is small, but the collector already
  holds the full vector in memory. Serializing the whole dataset into
  a second buffer doubles the peak footprint and pushes smaller
  machines toward swap. Streaming writes one encoded record at a time,
  so the extra memory is bounded by one sample plus the shard
  `BufWriter` buffer.
* **Crash safety.** If generation is interrupted, a single-file design
  leaves either a truncated file or zero bytes on disk. With shards,
  every completed `shard-NNN.bin` is valid and complete; only the
  currently-open shard can be truncated. Recovery in chapter 8 can
  therefore resume from the manifest plus the last shard, rather than
  re-running the entire generator.

> **Excursion — crash safety and the manifest write order**
>
> A careful writer flushes every completed shard before opening the
> next one, and writes the manifest only after the last shard is
> flushed. If the process dies after a shard is closed, the shard is
> intact and the manifest simply does not exist yet. If the process
> dies during manifest writing, the manifest may be truncated, but the
> shards are still complete. A recovery tool can therefore trust every
> shard file on disk and rebuild the manifest from them. The reverse
> order — manifest first, shards second — would leave a manifest
> pointing at missing or partial shards.
>
> Because `write_dataset` refuses to overwrite existing files, a failed
> run that leaves partial shards behind also blocks a retry until the
> output directory is cleaned. This is intentional: it prevents a
> half-finished dataset from being mistaken for a complete one.

### Why length-delimited framing?

bincode 2 is not self-delimiting over a stream. If you concatenate
`encode_to_vec(a)` and `encode_to_vec(b)` into one file, the reader has
no way to know where `a` ends and `b` begins. bincode encodes each
field contiguously and does not emit a total record length or sentinel
marker. Decoding from the concatenated bytes will either parse `a` plus
part of `b` as one value (and fail or misinterpret) or succeed by
accident and leave the reader out of sync with the file.

The fix is length-delimited framing: for each record, write the encoded
length as a little-endian `u32`, then write the bytes. The reader
reads 4 bytes, knows exactly how many bytes belong to the next record,
and can skip or decode without parsing the payload. This is the same
framing used by protocols like gRPC and by many append-only log
formats. It costs 4 bytes per record — negligible compared to the
sample size — and makes the shard format robust, seekable, and
independent of bincode's internal structure.

> **Excursion — why not use bincode's `Config` to add a length prefix?**
>
> bincode 2 has configuration options for array length encoding and
> endianness, but it does not provide a record-length wrapper. Adding
> one yourself is four lines: encode to a `Vec<u8>`, write
> `(len as u32).to_le_bytes()`, write the bytes. That small explicit
> step is preferable to pulling in a larger serialization framework
> just for framing.

### The manifest as provenance

The manifest is not just an inventory; it is an honesty document. It
records:

* `seed` — the RNG seed, so the dataset can be regenerated bit-for-bit.
* `quotas` — the collection targets that shaped the class distribution.
* `counts` — the actual number of win / block / quiet samples written.
* `shards` — the ordered list of shard files.
* `workspace_version` — the workspace package version shared by the crates that produced the labels.

This mirrors the run-journal philosophy in
[`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md):
a training artifact should carry enough metadata to reproduce or audit
it. If a future model behaves strangely, the manifest lets you check
whether the dataset was generated with the same seed, quotas, and
workspace version as a known-good run.

> **Honest note — `workspace_version` is the workspace package version**
>
> `train` and `engine` both inherit `version.workspace = true`, so
> `env!("CARGO_PKG_VERSION")` inside `train` returns the same value as
> it would inside `engine`. The field name reflects that structural
> fact: it records the shared workspace version, not a per-crate string.

## Low-level design

### Files

```text
gomoku/crates/train/
├── Cargo.toml          # add serde_json workspace dependency
└── src/
    ├── collect.rs      # add serde derives to Quotas
    ├── lib.rs          # add `pub mod shard;`
    └── shard.rs        # new module
```

### `Cargo.toml`

Inherit the workspace package version (so the manifest records the
same version as `engine`) and add `serde_json` under `[dependencies]`:

```toml
[package]
name = "train"
description = "Synthetic training-data generator for the Gomoku AlphaZero-style agent."
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
engine = { path = "../engine" }
serde = { workspace = true }
thiserror = { workspace = true }
bincode = { version = "2", features = ["serde"] }
serde_json = { workspace = true }
rand = "0.10.3"
```

### `src/collect.rs`

Add serde derives to `Quotas`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Quotas { ... }
```

`PartialEq` is added so the manifest's quotas can be compared in tests.

### `src/lib.rs`

Add `pub mod shard;` next to the existing module declarations and
update the crate-level doc comment to note that file I/O now lives in
`shard`.

### `src/shard.rs`

```rust
pub const SHARD_SIZE: usize = 4096;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub seed: u64,
    pub quotas: crate::collect::Quotas,
    pub counts: std::collections::BTreeMap<String, usize>,
    pub shards: Vec<String>,
    /// Version string captured from the workspace package version.
    ///
    /// This crate inherits `version.workspace = true`, so the value is
    /// structurally shared by every crate that does the same — including
    /// `engine`, whose tactics module produced the labels.
    pub workspace_version: String,
}

pub fn write_dataset(
    samples: &[crate::sample::Sample],
    out_dir: &std::path::Path,
    seed: u64,
    quotas: crate::collect::Quotas,
) -> std::io::Result<Manifest>;
```

Implementation notes:

* Use `std::fs::create_dir_all(out_dir)?` to create the output
  directory.
* Initialize `counts` as a `BTreeMap` with keys `"win"`, `"block"`,
  and `"quiet"`. The simplest honest source for the class count is the
  sample's value target: `label.rs` encodes win as `+1.0`, block as
  `-1.0`, and quiet as `0.0`. Match on `sample.value` and increment the
  corresponding bucket.
* Keep an `Option<BufWriter<File>>` for the current shard. On the
  first sample, and after every [`SHARD_SIZE`] samples, open a new file
  with `File::options().write(true).create_new(true).open(path)?`. The
  `create_new` flag fails if the file already exists.
* Rollover order matters: when `shard_count == SHARD_SIZE`, flush and
  close the current writer, increment the shard index, reset the count
  to zero, *then* write the next sample. This keeps the first shard
  indices `0..SHARD_SIZE`, the second `SHARD_SIZE..2*SHARD_SIZE`, and
  so on.
* Encode each sample with
  `bincode::serde::encode_to_vec(sample, bincode::config::standard())`.
  Map encoding errors to `io::ErrorKind::InvalidData`.
* Write the length prefix as `(len as u32).to_le_bytes()`. The encoded
  sample is tiny, but guard against `len > u32::MAX as usize` and
  return an error rather than truncating.
* After the loop, flush the last writer. Then build the `Manifest` and
  write it to `out_dir.join("manifest.json")`, again with
  `create_new`.

### Tests

Inside `#[cfg(test)] mod tests` in `shard.rs`:

1. `writes_one_shard_and_manifest_for_ten_samples` — collect ten
   samples with `Quotas { win: 3, block: 3, quiet: 4 }` and a seeded
   `StdRng`, write them to a unique temp directory, and assert:
   * `shard-000.bin` exists and `shard-001.bin` does not.
   * `manifest.json` exists.
   * Reading the shard back via length-delimited framing returns the
     same `Vec<Sample>`.
   * Manifest counts match the samples re-classified through
     `label::classify(sample.board().unwrap())`.
   * `manifest.seed`, `manifest.quotas`, `manifest.shards`, and
     `manifest.workspace_version` are correct.
2. `shard_rollover_after_shard_size_samples` — build `SHARD_SIZE + 1`
   cheap samples via `Sample::from_position`, write them, and assert
   that exactly two shards are produced, the first holds `SHARD_SIZE`
   samples, the second holds one, and both roundtrip back to the
   original samples.
3. `refuses_to_overwrite_existing_shard` — create the output
   directory and an empty `shard-000.bin`, then call `write_dataset`
   and assert it returns an error.
4. `refuses_to_overwrite_existing_manifest` — create the output
   directory and an empty `manifest.json`, then call `write_dataset`
   and assert it returns an error.
5. `empty_dataset_writes_no_shards_and_zero_counts` — write an empty
   slice and assert no shard files are created, the manifest exists,
   and all class counts are zero.

For temp directories, use `std::env::temp_dir()` plus a unique subdir
built from the process id and a timestamp. Clean the directory up at
the end of each test with `std::fs::remove_dir_all`. Do not add the
`tempfile` crate; it is not currently a workspace dependency.

## Solution (opt-in)

The complete reference code for this chapter — `shard.rs`, the
`lib.rs` registration line, the `Cargo.toml` dependency, and the
`Quotas` derive change — lives in
[05-shards-and-manifest/01-solution.md](05-shards-and-manifest/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare.

## TDD checklist

1. **Red:** Inherit the workspace package version in `Cargo.toml`,
   add `serde_json`, add serde derives to `Quotas`, create `shard.rs`
   with the `Manifest` struct, the `write_dataset` signature, and the
   four tests, leaving the body as `todo!()`. Register `pub mod shard;`
   in `lib.rs`. Run `cargo test -p train` and expect failures from the
   `todo!()` panics.
2. **Green:** Implement `write_dataset`. Re-run `cargo test -p train`.
   All twenty-one tests (five from this chapter plus sixteen from
   chapters 1–4) should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p train` passes from `gomoku/`.
* `cargo test -p engine --features testutil` still passes (no engine
  changes this slice).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Commit message in the reference worktree: `fix(train): workspace-inherited version + manifest overwrite test`.

Next: Chapter 06 — Reading back.

## References

* [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  workspace crate layout, store-games replay-buffer decision, and
  run-journal / provenance philosophy.
* [`docs/specs/2026-09-27-datagen-tutorial-design.md`](../../specs/2026-09-27-datagen-tutorial-design.md)
  — serialization decision: bincode 2, shards, manifest contents.
* [`docs/tutorials/datagen-tutorial/01-the-train-crate.md`](01-the-train-crate.md)
  — the `Sample` record and bincode roundtrip.
* [`docs/tutorials/datagen-tutorial/04-the-quota-collector.md`](04-the-quota-collector.md)
  — `Quotas`, `collect`, and Zobrist dedup.
* [`bincode 2 documentation`](https://docs.rs/bincode/2.0.1/bincode/) —
  `encode_to_vec` and `decode_from_slice` API.
* [`serde_json documentation`](https://docs.rs/serde_json/) —
  `to_writer_pretty`.
* [`std::fs::OpenOptions`](https://doc.rust-lang.org/std/fs/struct.OpenOptions.html)
  — `create_new` and `write`.
