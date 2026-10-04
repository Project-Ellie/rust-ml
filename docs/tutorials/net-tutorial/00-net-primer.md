# Primer — phase-0 supervised training

This primer is the design authority for the net tutorial. Read it once
before starting the chapters, and keep it open: every chapter points back
to the sections it implements. The chapters are the *doing*; this file is
the *why*.

## Abstract

The `net` crate turns the synthetic tactics dataset from the datagen
tutorial into a trained policy/value network. It is the second half of
milestone 3 and the launchpad for the self-play loop that arrives in
milestone 5. The training is supervised, not self-play: every label is
rules-true or engine-verified, so the network learns what a Gomoku
tactic looks like before it is asked to guide search.

## 1. The Burn/application split

The most important architectural decision in this crate is what Burn
owns and what you own.

| Concern | Owner | Why |
|---------|-------|-----|
| Network definition (`Module`, `Conv2d`, `BatchNorm`, residual add) | Burn | This is what a framework is for. |
| Optimizer state (AdamW) and LR schedule | Burn | Same. |
| Batch math (forward, backward, loss reduction) | Burn | Autodiff and kernels. |
| Data loading, shard iteration, train/holdout split | Application (`net`) | The dataset is game records, not image folders. |
| D4 augmentation at batch time | Application (`net`) | Augmentation is an on-read concern; the network sees canonical positions at inference time. |
| Outer training loop (epoch, shuffle, eval cadence, checkpointing) | Application (`net`) | Self-play and training interleave later; `SupervisedTraining`'s dataset paradigm does not fit. |
| Evaluation metrics and acceptance bar | Application (`net`) | The bar is project-specific (policy-argmax agreement, value-sign accuracy). |
| Saving/loading records | Application (`net`) | `DefaultRecorder` is the tool; the protocol (filename, sidecar) is ours. |

This split is locked in [docs/09-toward-alphazero.md](../../09-toward-alphazero.md)
and [docs/12-gomoku-architecture.md](../../12-gomoku-architecture.md). The
tutorial chapters 1–7 are the application half; chapter 3 is where Burn's
`Module` derive enters, and chapters 4–5 are where Burn's loss and
optimizer APIs enter.

## 2. Network anatomy

The network is a single residual tower with two heads.

```text
input:  [batch, 4, 17, 17]
          │
          ▼
   conv 4 → 128, batch norm, ReLU
          │
          ▼
   residual block × 10
   (3×3 conv → BN → ReLU → 3×3 conv → BN → add → ReLU)
          │
          ├──► policy head: 1×1 conv → 225 logits
          │
          └──► value head:  conv 1×1 channels→1 → flatten → linear 256 → ReLU → dense 1 → tanh
```

### Input planes (4 × 17 × 17)

The board is 15×15, but the engine stores it as 17×17 with a one-cell
border ring. The border is encoded as "opponent stone" in the `you`
plane, which gives convolutions wall information without padding
artifacts.

| Plane | Meaning |
|-------|---------|
| 0 — `me` | Stones of the player to move. |
| 1 — `you` | Stones of the opponent, **including the border ring**. |
| 2 — `my_last` | One-hot: the most recent move played by the player to move. |
| 3 — `opp_last` | One-hot: the most recent move played by the opponent. |

The engine's `encode` function produces only planes 0 and 1.
Adding the two history planes is an **invention (I1)** presented as a
choice in chapter 1: `net` builds them from `Board::moves()`, so the
engine stays a dependency island.

### Trunk (128 channels, 10 residual blocks)

This is the locked production default from
[docs/12-gomoku-architecture.md](../../12-gomoku-architecture.md). A
smaller **test-scale config (I7)** keeps the test suite in seconds;
chapter 3 makes the channel and block counts configurable.

### Policy head (225 logits)

One logit per legal move, indexed in row-major order: `idx = r * 15 + c`
for `r, c ∈ 0..15`. The loss applies `log_softmax` over all 225 entries
and compares against the soft target π; illegal moves receive zero mass
in the target, so the network learns that they are never chosen.

### Value head (tanh scalar)

A single scalar in `[-1, +1]` estimating the outcome from the side to
move's perspective.

## 3. The signal ladder

Phase 0 uses the cleanest signals the project will ever see. The ladder
is defined in [docs/14-openings-and-supervised-curriculum.md](../../14-openings-and-supervised-curriculum.md)
§5.2:

1. **Solver-verified labels** (future milestone 4 anchor set): exact,
   narrow, weird slice of position space.
2. **Synthetic tactical data** (the datagen tutorial's output): rules-true
   by construction, distribution by design.
3. **Strong-engine records**: strong but stylized and occasionally wrong.
4. **Self-play bootstrap** (`π`, `z`): noisiest early, asymptotically exact.

Phase 0 trains on level 2. Every sample carries:

- A **policy target** π: sparse `(move, mass)` pairs. 90 % of the mass
  sits on the tactical set (winning moves, forced blocks, quiet best
  moves per the engine); the remaining 10 % is uniform over all other
  legal moves. This is the "soft 90/10" shape.
- A **value target** in `{+1, −1, 0}` from the side to move: `+1` if the
  position is a forced win, `−1` if it is a forced block (opponent has a
  winning threat), `0` otherwise.

Chapter 2 scatters π into a dense 225-vector (I3); chapter 4 combines
policy cross-entropy with value MSE.

## 4. Why the loop is manual

Burn has a `SupervisedTraining` helper. We do not use it.

The reason is future-proofing. In milestone 5 the training loop will
interleave self-play generation, replay-buffer sampling, network
updates, and arena gating. That loop does not fit the
`SupervisedTraining` model, which assumes a fixed dataset and a single
optimization objective. Building the loop by hand in chapter 5 means the
transition to self-play is a change of *data source*, not a rewrite of
*training infrastructure*.

The manual loop owns:

- epoch iteration over `read_dataset`,
- seeded shuffle (determinism),
- batching through `NetBatcher`,
- forward/backward/step through Burn's autodiff API,
- LR warmup + cosine schedule,
- periodic holdout evaluation,
- checkpointing after improvement.

## 5. Determinism policy

Seeded generation is sacred in this project
([docs/12-gomoku-architecture.md](../../12-gomoku-architecture.md)).
Training runs must be reproducible under a seed. Concretely:

- Network initialization is seeded.
- The training shuffle is seeded.
- D4 transform selection is seeded (chapter 2).
- The data generator, if used in tests, is seeded.

The holdout split is **not** re-seeded per run; it is fixed by
`train::split::is_holdout` over Zobrist keys. The acceptance ritual in
chapter 8 runs two short seeded trainings and checks that the final
losses match.

## 6. Deferred work

The net tutorial deliberately stops at the edge of the self-play loop.
These items are shelved for milestone 5:

- **Self-play generation:** the `selfplay` crate will produce games and
  MCTS visit-count policies.
- **MCTS coupling:** the network will evaluate leaf nodes and supply
  priors; that lives in `mcts` and `selfplay`, not in `net`.
- **Replay buffer:** `InMemDataset` or a ring buffer will feed the loop.
- **TSS labels:** the TSS prover will generate exact win/block labels
  for the anchor set; the net crate only consumes the labels.

The network you build here is the student that will later become the
teacher.

## 7. Inventions and locked decisions

The tutorial presents the following as **choices**, not as facts:

- **I1 — Plane assembly lives in `net`.** The engine emits two planes;
  `net` adds the last-move planes from `Board::moves()`.
- **I2 — `NetBatcher` implements Burn's `Batcher` trait.** Exact
  generics per Burn 0.21.
- **I3 — Policy target flattening.** Scatter `(move, mass)` into a
  row-major 225-vector.
- **I4 — Hand-rolled soft-target CE.** Burn's built-in CE expects hard
  class indices; policy uses soft targets, so we write the loss by hand.
- **I5 — Acceptance bar.** 80 % policy-argmax agreement / 85 % value-sign
  accuracy on holdout win/block samples, presented as a starting bar.
- **I6 — Checkpoint layout.** `model.mpk` via `DefaultRecorder` plus a
  `run.json` sidecar.
- **I7 — Test-scale config.** Tiny channel/block counts for fast tests;
  production default is 128×10.
- **I8 — Augmentation scope.** D4 applied at batch time, one random
  transform per sample per epoch, train split only; holdout canonical.

Locked decisions are cited in the chapters that implement them.

## References

- [docs/specs/2026-10-04-net-tutorial-design.md](../../specs/2026-10-04-net-tutorial-design.md) —
  the binding spec: locked decisions, inventions I1–I8, chapter map.
- [docs/09-toward-alphazero.md](../../09-toward-alphazero.md) —
  Burn/application split and manual-loop ownership.
- [docs/12-gomoku-architecture.md](../../12-gomoku-architecture.md) —
  network anatomy, optimizer, schedule, backend story, determinism
  policy, crate layout.
- [docs/14-openings-and-supervised-curriculum.md](../../14-openings-and-supervised-curriculum.md) —
  §5.2 signal ladder, §5.3 phase 0 (D2).
- [docs/11-pitfalls.md](../../11-pitfalls.md) — f16 `CompactRecorder`,
  argmax dim-keeping, stale-API traps.
- [docs/tutorials/datagen-tutorial/](../datagen-tutorial/) — the data
  source this crate consumes.
