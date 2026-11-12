# Hand-off: writing the selfplay-corpora tutorial

**Date:** 2026-11-11. **For:** the session that writes the tutorial.
**Design authority:** [2026-11-11-selfplay-corpora-design.md](2026-11-11-selfplay-corpora-design.md)
(owner decisions S1–S8; supersedes the mentor-datagen spec).

---

## The mission

Write the **selfplay-corpora tutorial** — primer + 7 chapters, each
with four papers (chapter, design, implementation plan, meticulously
commented solution), in the tradition of the datagen/baseline
tutorials. The reference implementation is **already built, tested,
and acceptance-measured**; the chapters quote it. Wolfie's standing
requirement: **the tutorial must run error-free** — every command and
every quoted code fragment verified against the reference.

When the tutorial is written and Wolfie starts working it, the
`selfplay` crate lands on `main` chapter by chapter (the datagen
tutorial convention). The reference worktree is deleted only after
the tutorial is complete on `main`.

## Where the reference implementation lives

Worktree: `~/workspace/Project-Ellie/rust-ml-selfplay-ref`
(branch `selfplay-ref-v2`, based on `main` @ `9028aa8`, all changes
uncommitted — **never commit there**).

Crate: `gomoku/crates/selfplay/` (workspace member, registration
already in the worktree's `gomoku/Cargo.toml`).

```text
src/
  lib.rs       # public surface, #![deny(missing_docs)]
  game.rs      # ch 1+3: the runner, Game, PlySearch (visits/prior/value)
  opening.rs   # ch 1+4: OpeningSource, GeneratedOpening, TournamentOpenings
  record.rs    # ch 2+3: GameRecord, RecordedPly, RECORDED_PRIOR_TOP_K = 16
  corpus.rs    # ch 2: shard writer/reader, Manifest, overwrite protection
  schedule.rs  # ch 4: ExplorationSchedule, EXPLORATION_PLIES = 10
  generate.rs  # ch 4: seeded generation + Zobrist dedup (derive: base+n)
  anchor.rs    # ch 5: AnchorSeed, extract_anchor_seeds (verify_line gate)
  sample.rs    # ch 6: replay_sample, augment_sample (D4), holdout
  sampler.rs   # ch 6: CorpusSampler (seeded random game→ply→transform)
  mix.rs       # ch 6: SourceMixer (weighted, seeded)
  export.rs    # ch 6: export_corpus → train-format sample shards
  bin/
    selfplaygen.rs      # ch 4: corpus generator
    selfplayanchors.rs  # ch 5: anchor extraction
    selfplayexport.rs   # ch 6: corpus → train Sample dataset
    selfplayverify.rs   # ch 7: replay/legality/statistics verifier
tests/
  game.rs corpus.rs openings.rs schedule.rs dedup.rs binary.rs
  anchors.rs read_path.rs grading.rs   # grading is #[ignore] (MANUAL)
```

Also touched (small, deliberate): `crates/train/src/storage.rs` (new
shared shard mechanics used by both tactical datasets and game
corpora), `crates/player/src/lib.rs` (serde for `ResignReason`;
`Decision.own_proof: Option<engine::Proof>` threads the verified TSS
line through), `crates/net` untouched.

## Public API per chapter (for the solution papers)

- **Ch 1**: `play_game(seed, black_options, white_options)`,
  `play_game_with_opening(.., &mut impl OpeningSource)`, `Game`,
  `PlyRecord`, `PlySearch`, `Outcome`, `WinReason`, `Opening`,
  `OpeningAnswer`, `OpeningProvenance`, `GeneratedOpening`.
- **Ch 2**: `GameRecord`, `GameHeader`, `RecordedPly`,
  `CertifiedSuffix`, `SeatPairing`, `SeatConfig`,
  `GAME_RECORD_FORMAT_VERSION = 1`, `GameRecord::from_game`,
  `write_corpus`, `write_corpus_with_options`, `read_corpus`,
  `read_manifest`, `OverwritePolicy::{Refuse, Allow}`;
  `train::storage::{write_shard, read_shard}` (shared mechanics).
- **Ch 3**: `PlySearch::{visits, prior, root_value}` +
  `From<player::SearchStats>`; `RECORDED_PRIOR_TOP_K`; unsearched
  plies keep empty visits/priors and `root_value: None`.
- **Ch 4**: `TournamentOpenings::{parse, load}` implementing
  `OpeningSource`; `ExplorationSchedule::options_for_ply`;
  `generate_games[_with]`; `selfplaygen` CLI
  (`--out --openings [--openings...] [--black-sims 800]
  [--white-sims 800] [--games 1] [--seed-base 0]
  [--shard-capacity 1000]`).
- **Ch 5**: `AnchorSeed { game_index, from_ply, line }`,
  `extract_anchor_seeds`, `selfplayanchors --corpus --out`;
  `player::Decision.own_proof`.
- **Ch 6**: `replay_sample`, `augment_sample`, `position_is_holdout`,
  `CorpusSampler`, `SourceMixer`/`WeightedSource`, `export_corpus`,
  `selfplayexport --corpus --out [--shard-capacity]`.
- **Ch 7**: `selfplayverify --corpus` (JSON stats; hard errors on
  illegal moves/outcome mismatch).

## Measured acceptance evidence (quote these numbers)

Full details: `rust-ml-selfplay-ref/acceptance-evidence.md` + raw
logs in `acceptance-logs/`. Headline numbers [measured]:

- **Determinism**: two release runs (3 games, baseline-8v8, seed base
  424242) → byte-identical shards (SHA-256
  `4388d84e…ae52`) and manifests.
- **Legality**: `selfplayverify` on 48 games: 48/48 games,
  1 860/1 860 moves legal, outcomes match; `selfplayanchors`:
  47/47 certified suffixes verify (100 %).
- **First corpus** (`data/selfplay/v0-b32/`, 48 games baseline-32v32,
  seeds 100000–100047): 122.68 s → **1 408 games/hour**, ~2.56 s/game
  release. Outcomes: Black 23 / White 24 / draw 1. Length (plies
  after opening): min 7, median 27, mean 38.75, max 222 (the draw);
  **20.8 % end within 16 plies**. 28/36 opening offers seen, max
  share 8.33 %. 47/48 games carry a certified suffix.
- **Grading observation** (report-only, 20 games, 32v1): the
  budget-1 seat won 15/20 — see "open questions" below.
- **Integration smoke**: `selfplayexport` → 1 860 samples / 4 shards;
  `nettrain --config tiny` trained 50 steps on them; train loss
  6.548 → 6.252.

## Open questions to address honestly in the chapters

1. **Grading inversion** (ch 4/7): in the 20-game probe the
   budget-1 seat beat the budget-32 seat 15–20. Small sample, and the
   exploration schedule (τ = 1 + Dirichlet noise for the first 10
   plies) handicaps the stronger seat; also note the probe used fixed
   colors (strong always Black). Do NOT paper over it; present it as
   an observation and let the acceptance chapter discuss it. Wolfie
   will explore manually.
2. **`data/openings/piskvork-openings.txt` line 41** contains an
   off-board coordinate (`-8,-6`); the parser correctly rejects the
   file, but a run pointing at it failed fast with no games. The
   committed Gomocup lists (36 offers total) are the working pool.
3. **Debug-mode slowness**: an 8-sim game takes ~25 s in debug.
   Chapters must steer game-playing verification to `--release`; the
   slow tests are `#[ignore]`d with MANUAL comments.

## Tutorial-writing conventions (from the tutorial doctrine)

- Four papers per chapter; the **implementation plan** is the hiding
  layer for learners who don't read the solution (signatures in
  prose, method bodies never).
- Intention = use-case, never "write struct X". Tests are the
  specification, ordered along the execution path. Concepts appear
  when execution first touches them; the "why" folds into the step.
  ASD-STE100 sentences.
- Quote the reference exactly: `git diff` in the worktree shows every
  line; verify test names and counts against the code before quoting.
- Keep the chapter bones: glossary, context links, TDD checklist,
  done-when, verification commands.
- Wolfie is the didactic gate: produce the chapter, he corrects.

## Session-start checklist for the writing session

1. `cd ~/workspace/Project-Ellie/rust-ml` — read `docs/WARM-UP.md`,
   the design spec, and this hand-off.
2. Reference worktree: `~/workspace/Project-Ellie/rust-ml-selfplay-ref`
   (gates green as of 2026-11-11: `cargo test --workspace` 852
   passed, clippy clean; `cargo test -p selfplay` 50 passed / 4
   ignored, ~20 s).
3. Write `docs/tutorials/selfplay-corpora-tutorial/` in the MAIN
   checkout (follow the datagen-tutorial layout: README + primer +
   per-chapter folder with the four papers).
4. Do not commit the reference worktree; do not delete it until the
   tutorial is complete and Wolfie has worked it.

## Repo states at hand-off

- **rust-ml `main` @ `9028aa8`** (pushed): selfplay-corpora spec +
  doc amendments; before it `5c9c67a` net-tutorial closure,
  `8361ae3` net metric/robustness fixes, `8857f09` baseline opening
  focus. Untracked and intentional: `data/check-points/`,
  `data/synthetic/` (local corpora).
- **gomoku-gui `main` @ `4706581`** (pushed): the full visual/
  interaction batch. Untracked: `result.md` (session log).
