# Design: datagen-tutorial (milestone 3, part 1)

Status: approved by Wolfie 2026-09-27 (brainstorming dialogue; options
recorded below). Next step: implementation plan via the writing-plans
skill.

## Abstract

The datagen tutorial is the first of two tutorials covering milestone 3
(net + train on synthetic data; ch. 12 §13 row 3). It teaches the
learner to build the synthetic-data generator: a binary in the new
`train` crate that produces the tactics-labelled attack/defense dataset
the phase-0 network trains on. The second tutorial (net-tutorial, own
spec round) covers the `net` crate, supervised training, and the
milestone acceptance benchmark (>90% top-1 on held-out synthetic
threats). External behavior-cloning supplements (ch. 14 decisions
D2/D3) are explicitly deferred to a later addendum.

## Glossary

- **Attack/defense set** — the milestone-3 synthetic dataset: positions
  labelled by the engine's tactics module (win / block / quiet).
- **Win class** — positions where the side to move has at least one
  immediate win (`immediate_wins` non-empty).
- **Block class** — positions with no own immediate win but at least
  one forced block (`forced_blocks` non-empty).
- **Quiet class** — all remaining positions.
- **Shard** — one bincode file of streamed sample records.
- **Manifest** — the small metadata file next to the shards: class
  counts, quota configuration, seed, engine version.
- **Phase 0** — supervised pre-training before any self-play exists
  (ch. 14 §5.3, locked decision D2).
- **Signal ladder** — ch. 14 §5.2's ordering of training signals by
  noise; the synthetic tier is "rules-true by construction,
  distribution by design".

## Decisions taken during brainstorming

1. **Packaging: two tutorials, not one.** datagen-tutorial first,
   net-tutorial second. This breaks the one-milestone-one-tutorial
   pattern of milestones 1–2 deliberately: Wolfie wants a clean break
   between the no-Burn and Burn halves. (Chosen over option A, one
   milestone-scoped tutorial.)
2. **External supplements deferred.** The milestone row's "+ phase-0
   external supplements per ch. 14 D2/D3" is out of scope for both
   tutorials. The acceptance benchmark measures only the synthetic
   set; external data carries its own risk surface (license screening,
   decision-D5 re-adjudication) and becomes its own addendum later.
3. **Generator approach: hybrid random-play + engine labels + class
   quotas.** Random playouts produce natural-looking positions; the
   tactics module supplies rules-true labels; per-class quotas supply
   distribution-by-design. (Chosen over motif templates — narrow,
   gameable distribution — and over unfiltered random-play collection,
   which is quiet-dominated.)

## Tutorial shape and pedagogy

Mirrors the mcts tutorial exactly:

- Location: `docs/tutorials/datagen-tutorial/`.
- `README.md` index; `00-datagen-primer.md` is the design authority
  (why synthetic data — the AlphaGomoku stall lesson; the signal
  ladder; sample-format rationale; seed discipline; what milestone 4
  will add on top).
- Chapters `01`…`08`, each with: Context, Intention, Mental mapping
  (with excursions — curriculum learning, class imbalance,
  serialization format trade-offs, RNG stream discipline), Low-level
  design with exact API contracts, TDD checklist, Done-when gates,
  commit message.
- Per-chapter opt-in solution folders (`01-solution.md` per chapter),
  same convention as the mcts tutorial: contracts in the chapter,
  verified reference code in the companion folder.
- The learner writes the code; chapters withhold implementations.

## Code placement

The tutorial creates the **`train` crate** (its slot is locked in ch.
12's workspace layout). Initial dependencies: `engine`, `rand`,
`bincode 2`, `serde`, `thiserror` (workspace convention). `burn` and
`net` are added by the net tutorial. The generator is a binary target:

```bash
cargo run -p train --bin datagen -- --seed 42 --out data/synthetic/
```

The engine remains a dependency island: no I/O, no `rand`. All
randomness and all file output live in `train`. There is deliberately
no `train → mcts` edge (ch. 12's locked layout has none); the label
shaping is implemented in datagen directly (~30 lines over engine
tactics calls), so the mock evaluator's search-scaffolding constants
stay in the mcts crate where they belong.

## Data model — store positions, not planes

One record per position:

```text
Sample {
    black: Vec<Move>,        # absolute colors, engine vocabulary
    white: Vec<Move>,
    to_move: Color,
    policy: Vec<(Move, f32)>, # sparse shaped target over legal moves
    value: f32,              # +1.0 win, -1.0 block, 0.0 quiet
}
```

Rationale:

- **Store positions, encode on read.** Mirrors the locked store-games
  replay-buffer decision (ch. 12): planes are derived, never stored.
  The net tutorial's training code encodes with `engine::encode` and
  applies one random D4 transform per sample — augment-on-read, free
  because the dataset holds canonical positions only.
- **Sparse policy.** A dense `[f32; 225]` per sample is 900 bytes of
  mostly-irrelevant mass; the shaped distribution is fully described
  by the legal-move list with per-move probabilities. The training
  code scatters it into a dense target tensor at batch time.
- **Serialization: bincode 2** (locked in ch. 12), streamed shard
  files (append-friendly, crash-safe per shard) plus a small manifest
  (class counts, quota config, seed, engine version string).

## Labels — engine truth, shaped

Classification per sampled position, in order:

1. `immediate_wins(board, to_move)` non-empty → **win**
2. else `forced_blocks(board)` non-empty → **block**
3. else → **quiet**

Policy target: win/block classes get 90% of mass split evenly over the
tactical moves and 10% split evenly over the remaining legal moves;
quiet gets uniform over legal moves. Value target: `+1.0` / `-1.0` /
`0.0`.

Decision: exact integer values, **not** the mcts mock's
`0.95 / -0.90 / 0.00` — those constants are search scaffolding for the
sign sentinels; training labels should be clean targets the loss can
actually reach. The 90/10 split is `[experiment]` scaffolding and is
labeled as such in the tutorial.

## Collection — quotas, dedup, seeds

- Random playouts on the engine (same pattern as the differential
  harness: random legal move until terminal). Positions are sampled at
  random plies during the playout — not only at game end — with the
  number of samples per game bounded so a single game cannot dominate
  a class bucket (exact bound pinned in chapter 4).
- Samples are bucketed by class until per-class quotas are met.
  Default shape: balanced win/block, quiet capped (exact ratios
  `[experiment]`, tuned in chapter 8; quiet dominates random play, so
  the cap is what keeps the dataset tactical).
- **Duplicate rejection by Zobrist key** — the engine's O(1)
  incremental keys pay off again; one position enters the dataset once
  (symmetric duplicates are *not* rejected — D4 augmentation is the
  reader's job, not a dedup concern).
- One seeded `StdRng` drives all randomness; `--seed` makes a dataset
  bit-reproducible. The seed is recorded in the manifest. Determinism
  is promised for generation (pure CPU code); this is consistent with
  ch. 12's determinism policy.

## Chapter breakdown (8 chapters)

1. **The `train` crate + sample record types** — workspace wiring,
   serde derives, bincode roundtrip test.
2. **Seeded random playout driver** — random legal games on the
   engine; position sampling at random plies; determinism test under
   fixed seed.
3. **Tactics labeling** — classify win/block/quiet; shape policy and
   value; unit tests on ASCII puzzles (engine's `board_from_ascii`).
4. **Quota collector** — class buckets, caps, Zobrist dedup,
   termination when quotas are met.
5. **Streaming shard writer + manifest** — bincode shard files,
   manifest writing, crash-safety reasoning.
6. **Reading back** — dataset iterator over shards; soundness
   invariants checked on read (every win sample's policy argmax is in
   `immediate_wins`; every block sample's argmax is in
   `forced_blocks`; probabilities sum to 1).
7. **Held-out split + stats report** — deterministic train/held-out
   partition (by Zobrist key hash, not by position in file, so
   quotas don't skew the split); per-class counts, ply histogram,
   label-soundness summary.
8. **Acceptance** — generate the milestone-3 dataset, verify soundness
   gates, verify byte-identical regeneration under the same seed,
   inspect the stats report.

## Acceptance

Chapter 8's bar:

- The generator produces the dataset (size `[experiment]`, on the
  order of 50–100k positions; tuned so generation takes minutes, not
  hours).
- All soundness invariants pass on read-back.
- Regeneration with the same seed is byte-identical.
- The stats report shows the designed class distribution.

The >90% top-1 held-out benchmark is the **net tutorial's** acceptance
criterion, not this one's: datagen's job is to hand it trustworthy
data. Generated datasets are **local-only** (like the `data/` corpora)
— never committed.

## What this tutorial deliberately does not contain

- Burn, tensors, models, training loops (net tutorial).
- External corpora: parsing, license screening, decision-D5
  re-adjudication (deferred addendum).
- D4 augmentation implementation (net tutorial, augment-on-read).
- TSS-prover labels (milestone 4's anchor set).

## References

- `docs/12-gomoku-architecture.md` — §13 milestone row 3 (acceptance
  bar), workspace crate layout, serialization decision (bincode 2),
  determinism policy, store-games replay-buffer decision.
- `docs/14-openings-and-supervised-curriculum.md` — §5.2 signal
  ladder, §5.3 phase 0 (locked D2), D5 ruleset re-adjudication.
- `docs/tutorials/mcts-tutorial/` — style and structure template.
- `docs/11-pitfalls.md` — recorder precision trap (net tutorial's
  concern, cross-referenced from the primer).
