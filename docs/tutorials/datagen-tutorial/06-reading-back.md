# Chapter 06 — Reading back: dataset iterator + soundness

## Abstract

This chapter adds the read path to the `train` crate: a dataset
iterator that reconstructs every [`Sample`](01-the-train-crate.md) from
a directory of shards, and a soundness gate that rejects records whose
board cannot be rebuilt, whose policy masses do not sum to 1, or whose
policy argmax is not a tactical move for win/block positions. By the
end you will have a deterministic `read_dataset` function, a
`check_soundness` function with three error variants, and unit tests
that prove the roundtrip, the gate, and the empty-directory edge case.

## Glossary

| Term | Definition |
|------|------------|
| **Dataset iterator** | A function that walks all shards in order and yields decoded [`Sample`](01-the-train-crate.md) records. |
| **Soundness gate** | A defensive check run on every loaded sample before it is used for training. |
| **Length-delimited framing** | The little-endian `u32` length prefix written by [`shard::write_dataset`](05-shards-and-manifest.md) and read back here. |
| **Trust boundary** | Any interface where data produced by one component is consumed by another; even internal boundaries benefit from validation. |
| **Engine truth** | Using the engine's own tactics primitives (`immediate_wins`, `forced_blocks`) as the oracle for correctness. |
| **Policy argmax** | The move with the highest probability mass in the sparse policy target. |

## Context

Chapters 1–5 built the datagen pipeline in memory and added streaming
shard output. This chapter completes the storage story: reading the
shards back, and checking that what comes back is still rules-true.
The soundness gate is the last line of defense before a sample enters
a training batch; if a bug in the collector, labeler, or shard writer
corrupts a record, the gate catches it.

The design is locked in
[`docs/specs/2026-09-27-datagen-tutorial-design.md`](../../specs/2026-09-27-datagen-tutorial-design.md):
shards are the data, the manifest is provenance, and the reader does
not need the manifest.

## Intention

1. Create `gomoku/crates/train/src/dataset.rs` and register
   `pub mod dataset;` in `gomoku/crates/train/src/lib.rs`.
2. Define `SoundnessError` with at least these variants:
   * `IllegalPosition` — the stored stones do not rebuild a legal board.
   * `BadPolicyMass` — policy masses do not sum to 1 within tolerance.
   * `ArgmaxNotTactical` — for win/block samples, the policy argmax is
     not in the required tactical set.
3. Implement `read_dataset(dir: &Path) -> io::Result<Vec<Sample>>`:
   * Discover files matching `shard-*.bin`.
   * Sort them lexicographically.
   * Decode length-delimited bincode records in order.
   * Return the concatenated `Vec<Sample>`.
4. Implement `check_soundness(sample: &Sample) -> Result<(), SoundnessError>`:
   * Rebuild the board via [`Sample::board`](01-the-train-crate.md).
   * Reject any policy probability that is not finite; NaN comparisons
     are false, so finiteness must be checked explicitly before the
     mass sum.
   * Sum the policy masses and check within `1e-5` of `1.0`.
   * Classify via [`crate::label::classify`](03-tactics-labels.md).
   * For Win, require the argmax ∈ `engine::immediate_wins(&board, board.to_move())`.
   * For Block, require the argmax ∈ `engine::forced_blocks(&board)`.
   * Quiet has no tactical argmax requirement.
5. Write tests for: roundtrip read-back equals the written samples;
   every collected sample passes soundness; a win sample with a
   non-tactical argmax fails with `ArgmaxNotTactical`; a sample with
   halved masses fails with `BadPolicyMass`; a win sample whose policy
   contains `NaN` or `+inf` fails with `BadPolicyMass` instead of
   panicking; an overlapping-stone sample fails with `IllegalPosition`;
   and reading an empty dataset directory returns an empty vector.

Observable done-state: `cargo test -p train` passes, `cargo clippy
--all-targets -- -D warnings` is green, and `cargo fmt --all --
--check` makes no changes.

## Mental mapping

### The reader doesn't need the manifest

`write_dataset` produces two kinds of files: `shard-NNN.bin` and
`manifest.json`. The shards contain every sample; the manifest records
seed, quotas, counts, and the shard list. The reader only needs the
shards because:

* **The shards are self-describing.** Each record is a complete
  `Sample` with its own stone lists, policy, and value. The file name
  pattern and length-delimited framing are enough to read them all in
  order.
* **The manifest is provenance, not data.** It answers audit
  questions: "What seed produced this dataset?" "What quotas were
  used?" "How many wins were written?" It is not required to decode
  the samples.
* **Crash recovery is simpler.** If a previous run died after writing
  some shards but before writing the manifest, a reader that only
  trusts shards can still load the completed work. A reader that
  required the manifest would reject the partial dataset outright.

This is the same philosophy as the store-games replay-buffer decision
in [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md):
separate the durable payload from the metadata that describes it.

> **Excursion — trust boundaries and the engine differential harness**
>
> The engine crate was validated with a differential harness: every
> fast bitboard result was compared against a slow reference
> implementation. The lesson was not just "test the engine"; it was
> "validate at the boundary where correctness matters."
>
> The dataset reader sits at a similar boundary. The data on disk was
> written by *your own code* a moment ago, but disks, serialization
> libraries, future refactoring, and future you are all sources of
> drift. Treating the written bytes as trusted would repeat the
> mistake of assuming correctness because the producer is familiar.
> Instead, `check_soundness` re-derives the board from the stored
> stones and re-runs the engine's tactics classification. It is a
> miniature differential check: the stored label is compared against
> the engine's current truth.
>
> This is especially important because the `net` tutorial will load
> these samples into a Burn training loop. A single corrupted argmax
> or a board that does not rebuild teaches the network nonsense. The
> soundness gate converts "trust the file" into "verify every
> assumption."

### Why check mass and argmax separately?

A valid policy is a probability distribution: masses sum to 1, and the
highest-mass move is the sample's strongest prediction. These are
independent invariants:

* **Mass sum** catches scaling bugs (halved probabilities), empty
  policies, or probabilities that leak outside `[0, 1]`.
* **Argmax tacticality** catches label corruption: a win sample whose
  policy peaks on a move that does not actually win.

Checking both means a corrupted sample is rejected even if only one
invariant is broken. The `1e-5` tolerance acknowledges that `f32`
rounding in the 90/10 split does not produce an exact `1.0`.

## Low-level design

### Files

```text
gomoku/crates/train/
└── src/
    ├── lib.rs          # add `pub mod dataset;`
    └── dataset.rs      # new module
```

### `src/lib.rs`

Add `pub mod dataset;` next to the existing module declarations and
update the crate-level doc comment to note that the dataset reader
lives in `dataset`.

### `src/dataset.rs`

```rust
pub enum SoundnessError {
    IllegalPosition,
    ArgmaxNotTactical,
    BadPolicyMass,
}

pub fn read_dataset(dir: &std::path::Path) -> std::io::Result<Vec<crate::sample::Sample>>;

pub fn check_soundness(sample: &crate::sample::Sample) -> Result<(), SoundnessError>;
```

Implementation notes:

* Define a `const LENGTH_PREFIX_BYTES: usize = 4` (or use `[0u8; 4]`
  directly) and document that it mirrors the framing in
  [`shard.rs`](05-shards-and-manifest.md). Avoid duplicating the magic
  number `4` without a name.
* Use `fs::read_dir(dir)?` to list the directory, keep entries whose
  file name starts with `shard-` and ends with `.bin`, collect them
  into a `Vec<PathBuf>`, and `sort_unstable()`.
* Open each shard with `BufReader::new(File::open(&path)?)` and decode
  records in a loop: read 4 bytes for the length, then read that many
  bytes, then `bincode::serde::decode_from_slice`. Treat a clean
  `UnexpectedEof` at the start of a length read as end-of-file; any
  other error propagates.
* For `check_soundness`:
  * `sample.board()` returns `Result<Board, PositionError>`. Map any
    error to `SoundnessError::IllegalPosition`.
  * Sum `sample.policy.iter().map(|(_, p)| p)`. If `|sum - 1.0| >
    1e-5`, return `BadPolicyMass`.
  * `let class = crate::label::classify(&board);`.
  * For `TacticalClass::Win`, compute `engine::immediate_wins(&board,
    board.to_move())`, find the policy argmax, and require
    `wins.contains(argmax)`.
  * For `TacticalClass::Block`, compute `engine::forced_blocks(&board)`,
    find the policy argmax, and require `blocks.contains(argmax)`.
  * For `TacticalClass::Quiet`, only the mass check applies.
* Use `thiserror` derives for `SoundnessError`; it is already a
  dependency.

### Tests

Inside `#[cfg(test)] mod tests` in `dataset.rs`:

1. `roundtrip_reads_back_written_dataset` — collect six samples with
   `Quotas { win: 2, block: 2, quiet: 2 }` and seeded `StdRng`, write
   them with `shard::write_dataset`, read them back with
   `read_dataset`, and assert equality with the originals.
2. `collected_samples_pass_soundness` — collect the same tiny dataset
   and assert `check_soundness(sample).is_ok()` for every sample.
3. `win_argmax_outside_tactical_moves_fails` — find a win sample from
   the tiny dataset, replace its policy with a single non-tactical
   legal move carrying all mass, and assert
   `ArgmaxNotTactical`.
4. `bad_policy_mass_fails` — take any collected sample, halve every
   probability, and assert `BadPolicyMass`.
5. `illegal_position_fails` — take any collected sample, copy one of
   its white stones into the black list (or vice versa), and assert
   `IllegalPosition`.
6. `read_empty_dataset_directory_returns_empty_vector` — create an
   empty temp directory and assert `read_dataset` returns an empty
   vector without error.

For temp directories, copy the `unique_temp_dir` helper from
[`shard.rs`](05-shards-and-manifest.md). Clean up with
`fs::remove_dir_all` at the end of each test.

## Solution (opt-in)

The complete reference code for this chapter — `dataset.rs` and the
`lib.rs` registration line — lives in
[06-reading-back/01-solution.md](06-reading-back/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare.

## TDD checklist

1. **Red:** Create `dataset.rs` with the `SoundnessError` enum, the
   `read_dataset` and `check_soundness` signatures, and the tests
   listed below, leaving the function bodies as `todo!()`. Register
   `pub mod dataset;` in `lib.rs`. Run `cargo test -p train` and expect
   failures from the `todo!()` panics.
2. **Green:** Implement `read_dataset` and `check_soundness`. Re-run
   `cargo test -p train`. All tests should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

## Done when

* `cargo test -p train` passes from `gomoku/`.
* `cargo test -p engine --features testutil` still passes (no engine
  changes this slice).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Commit message in the reference worktree: `feat(train): dataset reader + soundness gate`.

Next: Chapter 07 — Held-out split + stats report.

## References

* [`docs/12-gomoku-architecture.md`](../../12-gomoku-architecture.md) —
  workspace crate layout, store-games replay-buffer decision, and the
  run-journal / provenance philosophy.
* [`docs/specs/2026-09-27-datagen-tutorial-design.md`](../../specs/2026-09-27-datagen-tutorial-design.md)
  — serialization decision: bincode 2, shards are data, manifest is
  provenance.
* [`docs/tutorials/datagen-tutorial/01-the-train-crate.md`](01-the-train-crate.md)
  — the `Sample` record and `Sample::board`.
* [`docs/tutorials/datagen-tutorial/03-tactics-labels.md`](03-tactics-labels.md)
  — `classify`, `label`, and `TacticalClass`.
* [`docs/tutorials/datagen-tutorial/04-the-quota-collector.md`](04-the-quota-collector.md)
  — `collect`, `Quotas`, and Zobrist dedup.
* [`docs/tutorials/datagen-tutorial/05-shards-and-manifest.md`](05-shards-and-manifest.md)
  — `write_dataset`, shard framing, and manifest format.
* [`gomoku/crates/engine/src/board.rs`](../../../gomoku/crates/engine/src/board.rs)
  — `Board::from_position` and `PositionError`.
