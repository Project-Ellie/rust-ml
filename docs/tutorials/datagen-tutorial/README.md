# Datagen Tutorial — milestone 3 part 1, built by you

The build-it-yourself tutorial for the `train` crate. The theory, the
design decisions, and the integration story are **not** here — they are
in [00-datagen-primer.md](00-datagen-primer.md), which you read first
(or at least keep open; every chapter points at the primer sections it
implements). This folder is the *doing*: eight small steps from an
empty workspace slot to a runnable synthetic-dataset generator, each one
sized so that you always know exactly what the next hour holds.

Milestone context: ch. 12 §13, milestone 3 row. The acceptance bar you
are walking toward: a deterministic `datagen` binary that produces the
milestone-3 synthetic tactics dataset, passes the soundness gate on
read-back, regenerates byte-identically under the same seed, and prints
a stats report with the designed class distribution. No Burn, no GPU,
no network — those arrive in the net tutorial.

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
   (serde derives, error handling, temp directories in tests) and the
   data-specifics (class imbalance, dedup, determinism) — while they
   are still cheap to think about. Excursions included where they earn
   their keep.
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
the implementation — the chapters assume the engine and MCTS tutorials'
red-green rhythm) and **Done when** (gates + commit message).

## Rules of the road

- **Gates before every commit**, from `gomoku/`:
  `cargo test -p train` · `cargo clippy --all-targets -- -D warnings` ·
  `cargo fmt --all -- --check`. All green, no exceptions.
- **The engine stays a dependency island.** `train` depends on `engine`,
  never the reverse. The only engine change allowed in this tutorial is
  adding `serde::Serialize`/`Deserialize` to `Move` and `Color` (chapter
  1). If you feel the engine needs a new method for datagen's sake,
  that is a conversation, not an edit.
- **No Burn in this crate.** The network arrives in the net tutorial;
  `train` is pure CPU code with `engine`, `rand`, `bincode`, `serde`,
  `serde_json`, and `thiserror`.
- **No `train → mcts` edge.** The workspace layout deliberately has no
  such dependency. The label shaping is implemented directly in
  `train`, so the MCTS mock's search-scaffolding constants stay in
  `mcts` where they belong.
- **`rand` is allowed and required** — but every random consumer takes
  a seeded RNG, so generation is reproducible (chapter 2 and chapter 8).
- **Commit after every step**, with the message the chapter names.
  Small commits are the tutorial's undo button.
- Stuck >20 minutes → coaching conversation (or the solution folder),
  per the engine and MCTS tutorials' convention. Struggle is the point;
  misery is not.

## The map

| # | Chapter | What lands | Primer § |
|---|---------|-----------|----------|
| 00 | [Primer: synthetic data](00-datagen-primer.md) | design authority: why synthetic data, signal ladder, dataset contract, pipeline, seed discipline, deferred work, common bugs | — |
| 01 | [The `train` crate + Sample record](01-the-train-crate.md) | workspace wiring, serde derives, `Sample`, bincode roundtrip test | §3 |
| 02 | [Seeded random playout driver](02-random-playouts.md) | `random_game`, `sample_plies`, `MAX_PLIES_PER_GAME = 3`, determinism test | §4 |
| 03 | [Tactics labeling](03-tactics-labels.md) | `TacticalClass`, `classify`, `label`, 90/10 shaping, exact ±1/0 values | §3 |
| 04 | [The quota collector](04-the-quota-collector.md) | `Quotas`, `collect`, Zobrist dedup, terminal-ply skip | §4 |
| 05 | [Streaming shard writer + manifest](05-shards-and-manifest.md) | `SHARD_SIZE = 4096`, length-delimited bincode, `Manifest`, overwrite protection | §3 |
| 06 | [Reading back: dataset iterator + soundness](06-reading-back.md) | `read_dataset`, `SoundnessError`, `check_soundness` | §3, §7 |
| 07 | [Held-out split + stats report](07-the-held-out-split.md) | `is_holdout` by Zobrist key, `Stats`, ply histogram | §3 |
| 08 | [Acceptance: the `datagen` binary + crate polish](08-acceptance.md) | `src/bin/datagen.rs`, integration tests, `#![deny(missing_docs)]`, byte-identical regeneration ritual | §4, §5 |

The order is deliberate: record before playout, playout before label,
label before collection, collection before persistence, persistence
before reading, reading before splitting, and everything before the
binary. By chapter 08 you have a self-contained tool that the net
tutorial can consume.

Three things the tutorial deliberately does **not** build (primer §6,
with the shelf labeled): Burn/tensors/training, external corpora, and
TSS-prover labels.

## Status

- 2026-09-27 — tutorial written, reference implementation verified
  (compiles, all chapter tests green, 50 k samples in ~45 s debug,
  13 shards / 41 MB, 9.84 % holdout). No chapters started yet.

Keep this section current as chapters land.

## References

- [docs/specs/2026-09-27-datagen-tutorial-design.md](../../specs/2026-09-27-datagen-tutorial-design.md) —
  the approved design spec: two tutorials (datagen + net), external
  corpora deferred, hybrid random-play + engine labels + quotas.
- [docs/12-gomoku-architecture.md](../../12-gomoku-architecture.md) —
  ch. 12: system architecture, milestone 3 acceptance row, workspace
  crate layout, determinism policy, store-games replay-buffer decision,
  and the AlphaGomoku curriculum lesson.
- [docs/14-openings-and-supervised-curriculum.md](../../14-openings-and-supervised-curriculum.md) —
  ch. 14: §5.2 the signal ladder, §5.3 phase 0 (locked decision D2),
  §5.4 decision D5 (re-adjudication).
- [docs/tutorials/mcts-tutorial/README.md](../mcts-tutorial/README.md) —
  prerequisite tutorial: milestone 2, the `mcts` crate, and the
  tactics-shaped mock evaluator whose scaffolding values must not leak
  into training labels.
- [00-datagen-primer.md](00-datagen-primer.md) — the design authority
  for this tutorial.
