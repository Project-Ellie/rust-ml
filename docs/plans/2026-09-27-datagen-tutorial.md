# Datagen Tutorial Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) or executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce `docs/tutorials/datagen-tutorial/` — primer + 8 chapters + opt-in per-chapter solutions, mcts-tutorial style — whose solutions quote a verified reference implementation of the milestone-3 synthetic-data generator.

**Architecture:** Two workstreams interleaved per task. (1) The **reference implementation**: the `train` crate + `datagen` binary, built in a git worktree on branch `reference/datagen`, TDD'd, gates green; it is *not* merged to `main` (the learner will recreate it chapter by chapter — same pattern as the mcts tutorial, whose reference crate lives outside the repo). (2) The **tutorial documents** in the main repo: each chapter written immediately after its reference slice is verified, the solution file quoting the verified code verbatim.

**Tech Stack:** Rust workspace (`gomoku/`), engine crate (rules/tactics/Zobrist, dependency island), `rand` (`StdRng`, seeded), `bincode 2` + `serde` (locked in ch. 12), `serde_json` (manifest). Spec: `docs/specs/2026-09-27-datagen-tutorial-design.md`.

**Key constraints (from spec + repo rules):**
- Engine stays a dependency island. The ONE sanctioned engine change is adding `Serialize, Deserialize` to `Move` and `Color` derives (ch. 12 budgets serde in engine's deps: "none beyond std + serde + thiserror").
- No `train → mcts` edge. Label shaping is implemented in `train` directly.
- Store positions, not planes. Labels at generation time. D4 augmentation is the reader's job (net tutorial).
- Value labels are `+1.0 / -1.0 / 0.0` — NOT the mcts mock's scaffolding values.
- Gates after every task, in the worktree: `cargo test -p train`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`. Engine gates rerun after the serde-derive touch.
- Generated datasets are local-only, never committed.

---

## File structure

Reference worktree (branch `reference/datagen`, worktree at `~/workspace/Project-Ellie/rust-ml-ref-datagen`):

```text
gomoku/crates/train/
├── Cargo.toml              # deps: engine, rand, bincode 2, serde, serde_json, thiserror
└── src/
    ├── lib.rs              # crate docs, pub modules, #![deny(missing_docs)] at the end
    ├── sample.rs           # Sample record, serde, bincode roundtrip      (chapter 1)
    ├── playout.rs          # seeded random playouts + ply sampling         (chapter 2)
    ├── label.rs            # TacticalClass, classify, label shaping        (chapter 3)
    ├── collect.rs          # Quotas, collector, Zobrist dedup              (chapter 4)
    ├── shard.rs            # streaming shard writer + JSON manifest        (chapter 5)
    ├── dataset.rs          # shard reader + soundness invariants           (chapter 6)
    ├── split.rs            # held-out partition + stats report             (chapter 7)
    └── bin/datagen.rs      # CLI: --seed --out --win --block --quiet       (chapter 8)
```

Main repo:

```text
docs/tutorials/datagen-tutorial/
├── README.md
├── 00-datagen-primer.md
├── 01-the-train-crate.md            + 01-the-train-crate/01-solution.md
├── 02-random-playouts.md            + 02-random-playouts/01-solution.md
├── 03-tactics-labels.md             + 03-tactics-labels/01-solution.md
├── 04-the-quota-collector.md        + 04-the-quota-collector/01-solution.md
├── 05-shards-and-manifest.md        + 05-shards-and-manifest/01-solution.md
├── 06-reading-back.md               + 06-reading-back/01-solution.md
├── 07-the-held-out-split.md         + 07-the-held-out-split/01-solution.md
└── 08-acceptance.md                 + 08-acceptance/01-solution.md
```

---

### Task 0: Reference worktree setup

**Files:**
- Create: git worktree `~/workspace/Project-Ellie/rust-ml-ref-datagen` on branch `reference/datagen`

- [ ] **Step 1: Create the worktree**

```bash
cd ~/workspace/Project-Ellie/rust-ml
git worktree add ~/workspace/Project-Ellie/rust-ml-ref-datagen -b reference/datagen
```

Expected: `Preparing worktree (new branch 'reference/datagen')`.

- [ ] **Step 2: Verify the workspace builds there**

Run: `cd ~/workspace/Project-Ellie/rust-ml-ref-datagen/gomoku && cargo check -p engine`
Expected: clean.

No commit (worktree starts clean at `main`).

---

### Task 1: The `train` crate + Sample record (chapter 1)

**Files:**
- Create: `gomoku/crates/train/Cargo.toml`, `gomoku/crates/train/src/lib.rs`, `gomoku/crates/train/src/sample.rs`
- Modify: `gomoku/Cargo.toml` (workspace members), engine `Move`/`Color` derives (add `Serialize, Deserialize` — the one sanctioned engine touch; locate the types, currently in `board.rs` / `moveset.rs` area)
- Test: `#[cfg(test)]` in `sample.rs`
- Docs: `docs/tutorials/datagen-tutorial/01-the-train-crate.md` + `01-the-train-crate/01-solution.md` (main repo)

**Contract:**

```rust
/// One labelled training position. Stores stones, never planes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Sample {
    pub black: Vec<engine::Move>,
    pub white: Vec<engine::Move>,
    pub to_move: engine::Color,
    pub policy: Vec<(engine::Move, f32)>,
    pub value: f32,
}

impl Sample {
    /// Build from a played move prefix: `history[..ply]` is on the
    /// board; colors alternate from Black; `to_move` follows from parity.
    pub fn from_position(history: &[engine::Move], ply: usize, policy: Vec<(engine::Move, f32)>, value: f32) -> Self;
    /// Rebuild the board via `engine::Board::from_position`.
    pub fn board(&self) -> Result<engine::Board, engine::PositionError>;
}
```

(Exact error type for `from_position` per engine's public API — check `engine::Board::from_position` signature at execution time and use its error type.)

- [ ] **Step 1:** Add `train` to workspace members; create crate skeleton with `//! ` crate docs explaining: stores positions not planes, no Burn yet, no I/O outside `shard.rs`/`bin`.
- [ ] **Step 2:** Add serde derives to engine `Move` + `Color`. Run engine gates: `cargo test -p engine --features testutil`, clippy, fmt — all must stay green (additive change).
- [ ] **Step 3:** Write failing test `sample_roundtrips_through_bincode`: build a Sample (3 black, 2 white stones, policy over 4 moves, value 1.0), `bincode::serde::encode_to_vec(&s, bincode::config::standard())`, `decode_from_slice`, assert equality. Verify it fails (types don't exist).
- [ ] **Step 4:** Implement `Sample` + `from_position` + `board()`. Tests: roundtrip green; `from_position` parity test (even ply → Black to move; stones split by alternation); `board()` roundtrip: rebuilt board's `empty_moves` count == 225 − ply.
- [ ] **Step 5:** Gates in worktree: `cargo test -p train`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.
- [ ] **Step 6:** Write chapter 1 doc (Context: milestone 3 begins, the train crate's slot in the locked layout; Intention: crate + Sample; Mental mapping: store-positions-not-planes rationale with the replay-buffer parallel, excursion on bincode-vs-rkyv from ch. 12's decision table, excursion on why serde derives in engine were budgeted from day one; Low-level design: the contract above; TDD checklist; Done-when; commit `feat(train): crate skeleton + Sample record`).
- [ ] **Step 7:** Write `01-the-train-crate/01-solution.md` quoting the verified `sample.rs` verbatim.
- [ ] **Step 8:** Commit reference: `feat(train): crate skeleton + Sample record` (worktree branch). Commit docs on main: `docs(tutorials): datagen chapter 1`.

---

### Task 2: Seeded random playout driver (chapter 2)

**Files:**
- Create: `gomoku/crates/train/src/playout.rs`
- Docs: `02-random-playouts.md` + solution

**Contract:**

```rust
/// Play a uniformly random legal game to terminal; return the move history.
pub fn random_game(rng: &mut impl rand::Rng) -> Vec<engine::Move>;

/// Choose `n` distinct ply indices in `1..=game.len()` (uniform without
/// replacement), so one game contributes at most `n` positions.
pub fn sample_plies(game_len: usize, n: usize, rng: &mut impl rand::Rng) -> Vec<usize>;
```

- [ ] **Step 1:** Failing tests: `random_game_terminates_within_225_plies` (seeded `StdRng::seed_from_u64(7)`, 100 games, all histories ≤ 225, every prefix legal via replay through `Board::play`); `sample_plies_are_distinct_and_in_range`; determinism: same seed → same history.
- [ ] **Step 2:** Implement (`board.empty_moves()` collected, `rng.random_range`, play until `status() != Ongoing`). Note in a comment: mirrors the engine differential harness pattern but lives in `train` because engine is rand-free by design.
- [ ] **Step 3:** Gates. Chapter doc (excursion: why engine is a dependency island and rand lives here; excursion: sampling without replacement via partial Fisher–Yates; the per-game sample cap from the spec — pinned at `n = 3`). Solution file. Commits: `feat(train): seeded random playout driver` / `docs(tutorials): datagen chapter 2`.

---

### Task 3: Tactics labeling (chapter 3)

**Files:**
- Create: `gomoku/crates/train/src/label.rs`
- Docs: `03-tactics-labels.md` + solution

**Contract:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TacticalClass { Win, Block, Quiet }

/// Win: side to move has an immediate win. Else Block: opponent has
/// one (forced blocks exist). Else Quiet.
pub fn classify(board: &engine::Board) -> TacticalClass;

/// The shaped training target: 90% mass over the tactical moves
/// (win/block), 10% over the rest; uniform when quiet. Value is
/// +1.0 / -1.0 / 0.0 by class.
pub fn label(board: &engine::Board) -> (Vec<(engine::Move, f32)>, f32);
```

- [ ] **Step 1:** Failing tests on ASCII puzzles (`engine::reference::board_from_ascii` — needs `engine/testutil`? No: `reference` is public; verify at execution): open-four → Win, policy argmax ∈ `immediate_wins`, masses sum to 1 ± 1e-6, tactical moves each get `0.9/k`; closed-four for opponent → Block, value −1.0; scattered stones → Quiet, uniform policy, value 0.0.
- [ ] **Step 2:** Implement `classify` + `label` (~40 lines over `engine::immediate_wins` / `engine::forced_blocks` / `board.empty_moves()`).
- [ ] **Step 3:** Gates. Chapter doc (the signal ladder excursion — ch. 14 §5.2 verbatim reference; why ±1 not the mock's 0.95/−0.90: training targets the loss can reach vs search scaffolding; excursion: class imbalance preview → motivates chapter 4). Solution file. Commits: `feat(train): tactics labeling` / `docs(tutorials): datagen chapter 3`.

---

### Task 4: The quota collector (chapter 4)

**Files:**
- Create: `gomoku/crates/train/src/collect.rs`
- Docs: `04-the-quota-collector.md` + solution

**Contract:**

```rust
#[derive(Debug, Clone, Copy)]
pub struct Quotas { pub win: usize, pub block: usize, pub quiet: usize }

/// Run random games until every class bucket is full. Positions are
/// deduplicated by Zobrist key; each game yields at most
/// `MAX_PLIES_PER_GAME` (3) samples. Deterministic under `rng` seed.
pub fn collect(quotas: Quotas, rng: &mut impl rand::Rng) -> Vec<crate::sample::Sample>;
```

- [ ] **Step 1:** Failing tests: tiny quotas (win: 2, block: 2, quiet: 3) with seeded rng → exact per-class counts; no duplicate Zobrist keys (rebuild each sample's board, collect keys into a set, assert len == samples); determinism: two runs, same seed → identical sample vectors.
- [ ] **Step 2:** Implement: loop games → `sample_plies` → classify → if bucket not full and key unseen, push labeled `Sample`. Note: quiet dominates random play — the quiet bucket fills first and caps; the loop continues for win/block. Document expected game counts (~hundreds for tiny quotas).
- [ ] **Step 3:** Gates. Chapter doc (excursion: Zobrist keys paying off a second time — O(1) dedup; excursion: rejection sampling and why quiet capping is distribution-by-design, quoting ch. 14 §5.2's phrase; honest note: symmetric duplicates NOT rejected — augmentation is the reader's job). Solution file. Commits: `feat(train): quota collector with Zobrist dedup` / `docs(tutorials): datagen chapter 4`.

---

### Task 5: Streaming shard writer + manifest (chapter 5)

**Files:**
- Create: `gomoku/crates/train/src/shard.rs`
- Docs: `05-shards-and-manifest.md` + solution

**Contract:**

```rust
pub const SHARD_SIZE: usize = 4096;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub seed: u64,
    pub quotas: Quotas,           // serde derives added to Quotas here
    pub counts: std::collections::BTreeMap<String, usize>,
    pub shards: Vec<String>,
    pub engine_version: &'static str, // env!("CARGO_PKG_VERSION") of engine at build
}

/// Write samples as `shard-000.bin`, `shard-001.bin`, … (bincode 2
/// streaming, one length-delimited record after another) plus
/// `manifest.json`. Returns the manifest.
pub fn write_dataset(samples: &[Sample], out_dir: &std::path::Path, seed: u64, quotas: Quotas) -> std::io::Result<Manifest>;
```

- [ ] **Step 1:** Failing tests: write 10 samples with `SHARD_SIZE` temporarily… no — keep SHARD_SIZE fixed; test with 10 samples → 1 shard + manifest exists; manifest counts match; shard file bytes decode back to the same 10 samples (uses `bincode::serde::encode_to_vec` + `encode_to_vec(u32 len)` prefix per record, or `bincode::serde::encode_to_writer` in a loop — pick the streaming API at execution and document the choice in the chapter).
- [ ] **Step 2:** Implement. Create `out_dir` if missing; error (don't silently overwrite) if shards already exist.
- [ ] **Step 3:** Gates. Chapter doc (excursion: streaming vs one-big-file — crash safety, memory; excursion: length-delimited framing — why raw concatenated bincode records are undecodable without it; manifest as provenance: seed + quotas + engine version, mirroring ch. 12's run-journal honesty). Solution file. Commits: `feat(train): shard writer + manifest` / `docs(tutorials): datagen chapter 5`.

---

### Task 6: Reading back — iterator + soundness (chapter 6)

**Files:**
- Create: `gomoku/crates/train/src/dataset.rs`
- Docs: `06-reading-back.md` + solution

**Contract:**

```rust
/// Iterate all samples in all shards of a dataset directory, in order.
pub fn read_dataset(dir: &std::path::Path) -> std::io::Result<Vec<Sample>>;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SoundnessError { /* IllegalPosition, ArgmaxNotTactical, BadPolicyMass, … */ }

/// Engine-truth check: policy masses sum to 1; win sample's argmax ∈
/// immediate_wins; block sample's argmax ∈ forced_blocks; board rebuilds.
pub fn check_soundness(sample: &Sample) -> Result<(), SoundnessError>;
```

- [ ] **Step 1:** Failing tests: roundtrip — collect tiny dataset (seeded), write to `std::env::temp_dir()` subdir, read back, assert equal; soundness green on every sample; a hand-corrupted sample (argmax set to a non-tactical move) fails with the right variant.
- [ ] **Step 2:** Implement reader (manifest optional for reading — shards suffice; note why) + soundness.
- [ ] **Step 3:** Gates. Chapter doc (excursion: trust boundaries — data written by your own code still gets validated on read, the engine differential-harness lesson generalized; why the reader doesn't need the manifest). Solution file. Commits: `feat(train): dataset reader + soundness gate` / `docs(tutorials): datagen chapter 6`.

---

### Task 7: Held-out split + stats report (chapter 7)

**Files:**
- Create: `gomoku/crates/train/src/split.rs`
- Docs: `07-the-held-out-split.md` + solution

**Contract:**

```rust
/// Deterministic partition: sample belongs to held-out iff
/// `zobrist(board) % 10 == 0` (~10%). Position-in-file never decides,
/// so quota ordering cannot skew the split.
pub fn is_holdout(sample: &Sample) -> bool;

#[derive(Debug)]
pub struct Stats { pub total: usize, pub per_class: [(&'static str, usize); 3], pub ply_histogram: Vec<usize>, pub holdout: usize, pub train: usize }

pub fn stats(samples: &[Sample]) -> Stats;
```

- [ ] **Step 1:** Failing tests: split is deterministic and partition-complete (train ∩ holdout = ∅, union = all); over a seeded 1k-sample dataset, holdout fraction within 5–15%; stats counts match per-class quotas.
- [ ] **Step 2:** Implement.
- [ ] **Step 3:** Gates. Chapter doc (excursion: data leakage — why random-row splits leak correlated positions into both sides and key-hash splits don't; the held-out set is what the net tutorial's >90%-top-1 benchmark reads). Solution file. Commits: `feat(train): held-out split + stats` / `docs(tutorials): datagen chapter 7`.

---

### Task 8: The `datagen` binary + acceptance (chapter 8)

**Files:**
- Create: `gomoku/crates/train/src/bin/datagen.rs`
- Docs: `08-acceptance.md` + solution

**Contract:** hand-rolled arg parsing (no clap — keeps train's deps to the locked set), flags `--seed <u64>` (default 0), `--out <path>` (required), `--win <n>` `--block <n>` `--quiet <n>` (defaults `[experiment]`: 20_000 / 20_000 / 10_000 — tune at execution so generation takes minutes). Runs collect → write_dataset → read-back soundness pass over every sample → prints the stats report.

- [ ] **Step 1:** Failing integration test `gomoku/crates/train/tests/datagen.rs`: run the binary with tiny quotas into a temp dir, assert exit 0, manifest + shard exist, read-back soundness green.
- [ ] **Step 2:** Implement binary.
- [ ] **Step 3:** Full acceptance run in worktree: generate the real dataset (defaults above) into `data/synthetic/` (local-only), time it, run the stats report, verify soundness gate, then **regenerate with the same seed into a second dir and `diff -r`** — must be byte-identical.
- [ ] **Step 4:** Gates. Chapter doc (acceptance bar restated from the spec; the byte-identical-regeneration ritual; what the net tutorial will consume). Solution file. Commits: `feat(train): datagen binary + acceptance` / `docs(tutorials): datagen chapter 8`.

---

### Task 9: Primer + README (main repo only)

**Files:**
- Create: `docs/tutorials/datagen-tutorial/00-datagen-primer.md`, `README.md`

- [ ] **Step 1:** Write the primer, mcts-primer style: §1 why synthetic data (AlphaGomoku stall lesson, ch. 12 §3); §2 the signal ladder (ch. 14 §5.2); §3 the dataset contract (Sample, classes, quotas, shards, manifest — the learner's map); §4 pipeline overview (playout → label → collect → write → read → split); §5 seed discipline and determinism; §6 what this tutorial defers (Burn, external data, augmentation, TSS labels); §7 common bugs (quiet-dominated datasets, leakage-prone splits, scaffolding values leaking into training labels, position-in-file splits).
- [ ] **Step 2:** Write README: chapter index, how the tutorial works (contracts + TDD + opt-in solutions, stuck >20 min rule), gates per chapter, links to spec and ch. 12/14.
- [ ] **Step 3:** Verify every inbound/outbound link (`grep -rn "datagen-tutorial" docs/`), add the tutorial to `docs/WARM-UP.md`'s tutorial paragraph if appropriate — but NO milestone-status change (that's a separate conversation with Wolfie; milestone 3 is not complete, the tutorial merely exists).
- [ ] **Step 4:** Commit on main: `docs(tutorials): datagen primer + README`.

---

### Task 10: Final verification + cleanup decision

- [ ] **Step 1:** Worktree: full gates one last time (`cargo test -p train`, engine gates, clippy, fmt). Confirm all 8 solution files quote code that matches the final worktree state (re-extract if drift).
- [ ] **Step 2:** Main repo: read all 10 tutorial documents end-to-end for consistency (chapter numbers, cross-links, commit messages matching chapters, TDD checklists matching actual test names in the reference).
- [ ] **Step 3:** Present Wolfie the cleanup choice: keep branch `reference/datagen` pushed-but-unmerged for future cross-checking (recommended — the mcts reference no longer exists anywhere, which makes solution maintenance guesswork), or delete worktree + branch. Remove the worktree either way (`git worktree remove`).
- [ ] **Step 4:** Final commit if any fixes; report.

---

## Self-review notes

- **Spec coverage:** every spec section maps to a task — crate placement + data model (T1), playouts (T2), labels (T3), quotas/dedup/seeds (T4), shards/manifest (T5), read-back + soundness (T6), split/stats (T7), acceptance (T8), pedagogy artifacts (T1–T9), deferrals respected everywhere.
- **Type consistency:** `Sample`, `Quotas`, `TacticalClass`, `Manifest`, `SoundnessError` names used identically across tasks; `MAX_PLIES_PER_GAME = 3` pinned once (T2 note, T4 contract).
- **Open-at-execution items (deliberate, not placeholders):** exact `from_position` error type name, bincode 2 streaming API choice, `board_from_ascii` visibility for train tests, default quota sizes tuned for runtime. Each names where it's resolved.
