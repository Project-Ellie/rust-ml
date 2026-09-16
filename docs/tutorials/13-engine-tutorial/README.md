# Tutorial 13 — Build the Gomoku Engine

This tutorial turns [Chapter 13](../../13-engine-design.md) into code —
**written by you**. Each file is one slice of the build order. Read the
design chapter first, reflect, experiment a little, then work through the
slices in order.

## How each slice works

Every slice follows the same rhythm (the rust-tdd discipline):

1. **Contract first.** The slice gives you exact API signatures and doc
   comments. That is the spec — the implementation is yours.
2. **One behavior, one test.** Write one failing test, watch it fail
   (RED), write the minimum code to pass it (GREEN), move to the next
   behavior. Never write all tests first.
3. **Gates before done.** `cargo test`, `cargo clippy --all-targets --
   -D warnings`, `cargo fmt --all` — all green, then commit.

You will not find full implementations in these files. Key algorithms are
in chapter 13 (that is what it is for); the slices tell you *when* and
*why* to use them, and which tests prove them. When you are stuck for
more than ~20 minutes, stop and ask — that is a coaching conversation,
not a failure.

## Conventions used in these files

- **Deep dives** — `NN-deep-dive/` folders hang off the chapter they
deepen (so far: [03-deep-dive](03-deep-dive/README.md), then
[04-deep-dive](04-deep-dive/README.md)). A chapter tells
you *what* to build; a deep dive derives *why it is shaped that way*,
with measured numbers and compiler experiments. Read them after the
chapter, when a contract raises a "why this type?" question.
- **Rust toolbox** — short sections on the language idioms the slice
  needs (const fn, operator traits, PhantomData, ...). You know basic
  Rust; these boxes cover the specific moves each slice requires.
- **ML refresh** — extra-short briefs on the ML concept a design choice
  serves (canonical encoding, augmentation, priors, ...). Skim to
  refresh; skip if fresh. Deeper training-side concepts (AdamW, soft
  cross-entropy, LR schedules) arrive with the net/train tutorials.

## The slices

| # | File | You build | Slice of ch. 13 |
|---|------|-----------|------------------|
| 1 | [The workspace](01-the-workspace.md) | — (already scaffolded; orient yourself) | 1 |
| 2 | [The reference engine](02-the-reference-engine.md) | `Move`, naive `reference.rs` + corpus | 2 |
| 3 | [Bitboard and Board](03-bitboard-and-board.md) | `Bitboard`, `Board`, differential tests · [deep dives](03-deep-dive/README.md) | 3 |
| 4 | [Win detection](04-win-detection.md) | `has_five`, edge/overline corpus | 4 |
| 5 | [Zobrist keys](05-zobrist.md) | const table, incremental key, undo | 5 |
| 6 | [Symmetry and encoding](06-symmetry-and-encoding.md) | D4 tables, 17×17 planes | 6 |
| 7 | [Tactics](07-tactics.md) | `MoveSet`, win-in-1/2 detection | 7 |
| 8 | [The Swap2 opening](08-swap2-opening.md) | typestate opening machine | 8 |
| 9 | [Benchmarks and hardening](09-benchmarks-and-hardening.md) | criterion, visibility sweep | 9–10 |

## Commands cheat sheet

```bash
cd gomoku
cargo test -p engine                 # run the crate's tests
cargo test -p engine <name>          # run one test
cargo test -p engine --features testutil   # with the reference engine exposed
PROPTEST_CASES=10000 cargo test -p engine  # more property iterations
cargo bench -p engine                # slice 9
```

## Done means

The milestone-1 acceptance row from chapter 12: property tests green,
10k random games match the reference, `has_five` ≥ 50M checks/s — plus a
public API surface you can show without embarrassment.
