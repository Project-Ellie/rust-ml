# Net Tutorial — milestone 3 part 2, built by you

The build-it-yourself tutorial for the `net` crate. The theory, the
design decisions, and the integration story are **not** here — they are
in [00-net-primer.md](00-net-primer.md), which you read first (or at
least keep open; every chapter points at the primer sections it
implements). This folder is the *doing*: nine small steps from an empty
workspace slot to a trained phase-0 policy/value network, each one sized
so that you always know exactly what the next hour holds.

Milestone context: ch. 12 §13, milestone 3 row, phase 0. The acceptance
bar you are walking toward: after training on the ~50 k synthetic tactics
dataset, the network achieves ≥ 80 % holdout policy-argmax agreement with
the label argmax on win/block samples and ≥ 85 % holdout value-sign
accuracy. The full acceptance protocol lives in chapter 8; the Status
section below records the reduced-scale smoke numbers the tutorial was
verified against.

## How each step works (the five-part rhythm)

Every chapter follows the same five sections, in this order. The
structure is the pedagogy — it keeps the *why* separate from the
*what* separate from the *where*, so that by the time you write code
you have already written it in your head twice.

1. **Context.** Where we are: what exists, what just landed, what
   this step connects to. Short. Re-read it when you resume a session.
2. **Intention.** The particular goal of this step and what "done"
   looks like — the observable behavior, not the code. If you can't
   state the intention back in one sentence, re-read before touching
   the keyboard.
3. **Mental mapping.** The conceptual plan, still without code: what
   needs to happen, in what order, and *why that way*. This is where
   the chapter foreshadows the difficulties — the Rust-specifics
   (Burn generics, backend traits, `Module` derive) and the
   ML-specifics (soft-target CE, D4 augmentation, manual loop) —
   while they are still cheap to think about. Excursions included
   where they earn their keep.
4. **Low-level design.** The physical solution design: which files,
   which types, which function signatures, where the tests live. After
   this section you could implement the step without another thought
   about *structure* — all that remains is the writing.
5. **Solution.** A complete, compiled, tested reference
   implementation — in the chapter's companion folder (same root name),
   clearly marked. **Opt in.** The intended use: try the step yourself
   first; open the solution when stuck >20 minutes, or afterwards to
   compare. Every solution is extracted from a reference crate that
   compiles and passes the chapter's tests — it is not illustrative
   pseudocode.

Then the **TDD checklist** (the tests you write *before* or alongside
 the implementation — the chapters assume the engine, MCTS, and
datagen tutorials' red-green rhythm) and **Done when** (gates +
commit message).

## Rules of the road

- **Gates before every commit**, from `gomoku/`:
  `cargo test -p net` · `cargo clippy --all-targets -- -D warnings` ·
  `cargo fmt --all -- --check`. All green, no exceptions.
- **The engine stays a dependency island.** `net` depends on `engine`
  and `train`, never the reverse. The only engine code `net` uses is
  the public API (`Board`, `Move`, `Color`, `Transform`, `encode`,
  `EXT`).
- **No `net → mcts` edge.** The workspace layout deliberately has no
  such dependency; the network learns from `train` labels, not from
  search visit counts.
- **Burn is pinned to `=0.21.0`.** Verify every API against the pinned
  source (`~/.cargo/registry/src/*/burn-*-0.21.0/`) or the verified
  `burn-ml-coach` reference (`burn-api-0.21.md`); never blogs, never
  0.22/main-branch docs.
- **Training code is generic over `AutodiffBackend`; inference over
  `Backend`; the binary picks concrete backends.**
- **Commit after every step**, with the message the chapter names.
  Small commits are the tutorial's undo button.
- Stuck >20 minutes → coaching conversation (or the solution folder),
  per the engine, MCTS, and datagen tutorials' convention. Struggle is
  the point; misery is not.

## The map

| # | Chapter | What lands | Primer § |
|---|---------|-----------|----------|
| 00 | [Primer: phase-0 supervised training](00-net-primer.md) | design authority: Burn/app split, network anatomy, signal ladder, manual loop, determinism policy, deferred work | — |
| 01 | [The `net` crate + planes](01-planes-and-the-net-crate.md) | workspace wiring, burn `=0.21.0`, 4-plane assembly from `Board`+history (I1), tensor-ready plane layout, tests: shapes, semantics, last-move one-hots, empty-history edge | §2 |
| 02 | [The batcher + D4 augmentation](02-the-batcher-and-d4-augmentation.md) | `NetBatcher` (I2), scatter π → 225 (I3), D4 at batch time, train-split-only (I8), tests: batch shapes, mass preservation, canonical holdout, seed determinism | §2, §3 |
| 03 | The network module | `Module` derive, conv stem 4→128, residual block ×10, policy head →225 logits, value head →tanh scalar; tests: output shapes, seeded init determinism, tiny-config variant (I7) | §2 |
| 04 | Loss + metrics | hand-rolled soft-target CE (I4) + value MSE + combined loss; metrics: policy-argmax agreement, value-sign accuracy | §3 |
| 05 | The manual training loop | AdamW, warmup+cosine, seeded shuffle, periodic holdout eval; tests: single-batch overfit, seeded reproducibility | §4 |
| 06 | Checkpointing + run journal | `DefaultRecorder` save/load (I6), `run.json` sidecar; tests: bit-identical forward, journal roundtrip | §4 |
| 07 | The `nettrain` binary | CLI mirroring datagen's contract, integration test training 20 steps on a tiny dataset | §4 |
| 08 | Acceptance + polish | 50 k acceptance protocol (I5 bar), determinism ritual, `#![deny(missing_docs)]`, crate doc | §5 |

The order is deliberate: planes before batches, batches before the
module that consumes them, module before loss, loss before the loop,
loop before persistence, persistence before the binary, binary before
acceptance.

Three things the tutorial deliberately does **not** build (primer §6,
with the shelf labeled): self-play generation, MCTS coupling, and the
store-games replay buffer.

## Status

- 2026-10-04 — tutorial written, reference implementation verified
  (32 tests, clippy/fmt clean; reduced-scale smoke: 1500 samples, mid
  config 32ch×4blocks, 200 steps, ~83s release, train loss
  6.239→6.001). No chapters started yet.

Keep this section current as chapters land.

## References

- [docs/specs/2026-10-04-net-tutorial-design.md](../../specs/2026-10-04-net-tutorial-design.md) —
  the approved design spec: locked decisions, inventions I1–I8,
  chapter map, hard rules.
- [docs/09-toward-alphazero.md](../../09-toward-alphazero.md) —
  ch. 9: the Burn/application split; Burn owns network/optimizer/batch
  math, the application owns the loop.
- [docs/12-gomoku-architecture.md](../../12-gomoku-architecture.md) —
  ch. 12: network anatomy (4 planes, 128ch×10 trunk, 225+1 heads),
  optimizer/schedule, backend story, determinism policy, crate layout.
- [docs/14-openings-and-supervised-curriculum.md](../../14-openings-and-supervised-curriculum.md) —
  ch. 14: §5.2 the signal ladder, §5.3 phase 0 (locked decision D2).
- [docs/11-pitfalls.md](../../11-pitfalls.md) — the `CompactRecorder`
  f16 trap, argmax dim-keeping, stale-API traps.
- [docs/tutorials/datagen-tutorial/](../datagen-tutorial/) — format
  precedent + data source (`Sample`, `read_dataset`, `is_holdout`).
- [docs/tutorials/convolutions-tutorial/](../convolutions-tutorial/) —
  Burn 0.21 usage precedent.
- [00-net-primer.md](00-net-primer.md) — the design authority for this
  tutorial.
