# Net tutorial — design spec (phase-0 supervised training)

**Status:** approved by owner 2026-10-04 ("create yet another deep-dive
tutorial for the 'real' net crate"). **Format precedent:** the datagen
and convolutions tutorials. **Audience:** Wolfie (strong DL theory,
basic Rust, basic Burn — MNIST curriculum + convolutions side quest).

This spec is the design authority for the tutorial authors. Locked
decisions are cited; everything else is an **invention** and marked as
such. Inventions must be presented in the tutorial as choices, not as
architecture-doc facts.

## Deliverables

1. **Reference implementation** — `gomoku/crates/net`, built TDD-first
   in a disposable git worktree of this repo, verified (gates + smoke
   runs), then deleted. Final crate sources are copied to
   `/tmp/net-reference/` for solution extraction before deletion.
2. **Tutorial** — `docs/tutorials/net-tutorial/` on `main`:
   `README.md`, `00-net-primer.md`, chapters `01`–`08`, per-chapter
   companion folders `NN-name/01-solution.md` quoting the verified
   reference **byte-for-byte** (extract + diff to verify).

## Locked decisions (cited)

- **Split** (docs/09): Burn owns network/optimizer/batch math; the
  application owns data loading, D4 augmentation, the outer training
  loop (manual, NOT `SupervisedTraining`), evaluation cadence, and
  checkpointing.
- **Network** (docs/12): input = 4 planes on the engine's 17×17 padded
  board (my stones, your stones, my last move, your last move); trunk
  = 128-channel residual, 10 blocks; policy head → 225 logits;
  value head → tanh scalar.
- **Loss** (docs/12, docs/14 §5.3): policy cross-entropy against the
  soft 90/10-shaped π + value MSE, combined; weight decay 1e-4.
- **Optimizer/schedule** (docs/12): AdamW; batch 1024; LR warmup 2k
  steps then cosine 3e-4 → 3e-6.
- **Targets** (datagen tutorial ch. 3): policy π is sparse
  (90 % mass on the tactical set, 10 % uniform over the rest);
  value ∈ {+1 win, −1 block, 0 quiet} for the side to move.
- **Backend** (docs/12, convolutions tutorial): backend-generic code;
  default CPU backend for tests (`NdArray` or `Flex` per convolutions
  precedent — verify what `patterns` used and match), `gpu` feature
  opt-in; GPU training only after CPU/GPU parity.
- **Data source** (datagen tutorial): `train::read_dataset`,
  `train::split::is_holdout` (Zobrist-hash split, ~10 %),
  `check_soundness` available for defensive validation.
- **Determinism** (docs/12 determinism policy + datagen ch. 8):
  seeded generation is sacred; training runs must be reproducible
  under a seed (seeded init, seeded shuffle, seeded augmentation).

## Inventions (tutorial must flag them as choices)

- **I1 — Plane assembly lives in `net`.** `engine::encode` emits only
  2 planes and the engine is a dependency island (no burn, no I/O).
  `net` builds all 4 planes from `engine::Board` + the `Sample`'s move
  history (last-move planes are trivial given the history). Engine
  untouched.
- **I2 — Batcher design.** A `NetBatcher<B: Backend>` implementing
  burn 0.21's `Batcher` trait: `Sample` (+ optional D4 transform) →
  `(Tensor<B,4> input [1,4,17,17], Tensor<B,2> policy target [1,225],
  Tensor<B,2> value target [1,1])` — exact generics per burn 0.21
  pinned source.
- **I3 — Policy target flattening.** Sparse π (move, mass) pairs are
  scattered into a 225-vector in row-major (r*15+c) order; moves on
  padding never occur (samples are legal positions).
- **I4 — Soft-target CE.** Policy loss = `-(π · log_softmax(logits))
  .sum(-1).mean()` written by hand (burn's built-in cross-entropy is
  for hard class indices — verify against pinned source and say so).
- **I5 — Acceptance bar.** After the chapter-8 acceptance run on the
  ~50 k synthetic dataset: holdout policy-argmax agreement with the
  label argmax ≥ 80 % on win/block samples, holdout value-sign
  accuracy ≥ 85 %. Presented as a starting bar the owner may tune.
- **I6 — Checkpoint layout.** `nettrain --out <dir>` writes
  `model.mpk` (DefaultRecorder — full precision; the f16
  CompactRecorder trap is a named pitfall, docs/11) + `run.json`
  (seed, config, final metrics — the run-journal idea).
- **I7 — Test-scale config.** Unit/integration tests use a tiny config
  (e.g. 8 channels, 1 block, batch 16) so the suite stays in seconds;
  the locked 128×10 is the production default only.
- **I8 — Augmentation scope.** D4 transform applied at batch time, one
  random transform per sample per epoch, train split only; holdout
  always canonical (matches "augmentation is an on-read concern",
  datagen ch. 4).

## Chapter map (five-part rhythm per chapter, like datagen)

| # | Chapter | What lands |
|---|---------|-----------|
| 00 | Primer: phase-0 supervised training | design authority: Burn/app split, network anatomy walkthrough, signal ladder recap, why manual loop, determinism policy, deferred work (self-play, MCTS coupling, replay buffer) |
| 01 | The `net` crate + planes | workspace wiring, burn `=0.21.0` dep (verify feature set against `patterns`), 4-plane assembly from Board+history (I1), plane→tensor conversion, tests: shapes, per-plane semantics, last-move one-hots, empty-history edge |
| 02 | Batcher + D4 augmentation | `NetBatcher` (I2), scatter π → 225 (I3), D4 transform of board+policy at batch time (I8), tests: batch shapes, augmentation legality (transformed stones still on board, π masses preserved), holdout canonical, seed determinism |
| 03 | The network module | `Module` derive, conv stem 4→128, residual block ×10, policy head →225 logits, value head →tanh scalar; tests: output shapes on both 17×17 input, seeded init determinism, tiny-config variant (I7) |
| 04 | Loss + metrics | hand-rolled soft-target CE (I4) + value MSE + combined loss; metrics: policy-argmax agreement, value-sign accuracy; tests on hand-computed values |
| 05 | The manual training loop | AdamW, warmup+cosine schedule, batch iteration with seeded shuffle, periodic holdout eval, per docs/09 manual-loop ownership; tests: single-batch overfit (loss ↓ over 50 steps), seeded run reproducibility (identical loss trajectory) |
| 06 | Checkpointing + run journal | DefaultRecorder save/load (I6), `run.json` sidecar; tests: save→load→bit-identical forward pass, journal roundtrip |
| 07 | The `nettrain` binary | CLI mirroring datagen's contract (parse_args pure+testable, exit 2 usage / 1 failure, progress→stderr, final report→stdout), integration test training 20 steps on a tiny generated dataset |
| 08 | Acceptance + polish | the 50 k acceptance protocol (I5 bar, timing note), determinism ritual (two short seeded runs → identical final losses), `#![deny(missing_docs)]`, crate doc |

Order rationale: planes before batches, batches before the module that
consumes them, module before loss, loss before the loop, loop before
persistence, persistence before the binary, binary before acceptance.

## Hard rules for every author/implementer

- Burn pinned `=0.21.0`. Verify every API against the pinned source
  (`~/.cargo/registry/src/*/burn-*-0.21.0/`) or the verified
  `burn-ml-coach` reference (`burn-api-0.21.md`); never blogs, never
  0.22/main-branch docs.
- Training code generic over `AutodiffBackend`; inference over
  `Backend`; the binary picks concrete backends.
- Engine untouched (dependency island). `net` depends on `engine` and
  `train`; no reverse edges; **no `net → mcts` edge**.
- Gates per code slice (run from `gomoku/` in the worktree):
  `cargo test -p net`, `cargo clippy --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`.
- Reference worktree commits are local; nothing is pushed.
- Solution files quote the reference byte-for-byte (extract + diff).
- Tutorial chapters withhold implementations (learn-by-doing, exact
  API contracts + TDD checklist); solutions live in companion folders,
  marked opt-in.
- Tests must be fast: tiny config (I7), tiny datasets (generate once
  per test via `train::collect` with tiny quotas or build samples
  directly).
- The 50 k acceptance *run* is documented as a protocol with expected
  output; the reference verification performs a reduced-scale version
  (e.g. 5–10 k samples, small config) and records real numbers in the
  tutorial's Status section.

## References

- docs/09-toward-alphazero.md — the Burn/application split.
- docs/12-gomoku-architecture.md — network anatomy, optimizer,
  schedule, backend story, determinism policy, crate layout.
- docs/14-openings-and-supervised-curriculum.md — §5.2 signal ladder,
  §5.3 phase 0 (D2).
- docs/11-pitfalls.md — f16 CompactRecorder, argmax dim-keeping,
  stale-API traps.
- docs/tutorials/datagen-tutorial/ — format precedent + data source.
- docs/tutorials/convolutions-tutorial/ — Burn 0.21 usage precedent.
