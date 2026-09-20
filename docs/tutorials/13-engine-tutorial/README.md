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

- **Deep dives** — companion folders carry the chapter's root name
(`04-win-detection/` deepens `04-win-detection.md`; so far: [03-bitboard-and-board](03-bitboard-and-board/README.md),
[04-win-detection](04-win-detection/README.md),
[05-zobrist](05-zobrist/README.md),
[06-symmetry-and-encoding](06-symmetry-and-encoding/README.md), and
[07-tactics](07-tactics/README.md)). A chapter tells
you *what* to build; a deep dive derives *why it is shaped that way*,
with measured numbers and compiler experiments. Read them after the
chapter, when a contract raises a "why this type?" question.
- **Implementation plans** — where a slice's thinking is already done, its
companion folder may also grow a step-by-step build plan ending in the
reference solution (first: [04-win-detection/02](04-win-detection/02-implementation-plan.md)).
The slice files still withhold implementations; plans are opt-in.
- **Rust toolbox** — short sections on the language idioms the slice
  needs (const fn, operator traits, PhantomData, ...). You know basic
  Rust; these boxes cover the specific moves each slice requires.
- **ML refresh** — extra-short briefs on the ML concept a design choice
  serves (canonical encoding, augmentation, priors, ...). Skim to
  refresh; skip if fresh. Deeper training-side concepts (AdamW, soft
  cross-entropy, LR schedules) arrive with the net/train tutorials.

## Glossary

Fixed terminology for the whole tutorial. Every slice, deep dive, and
implementation plan uses exactly these names for these concepts.

- **Stride-16 layout** — the mapping `idx(r, c) = r * 16 + c` from board
  cells to bit indices: 15 rows of 15 cells, each row stored in a 16-bit
  slot of a `[u64; 4]` (256 bits total, 240 addressable, 225 real cells).
- **Padding bits** — the 31 bit positions that are not board cells: the
  **padding column** (column 15 of every row, 15 bits) and the
  **high padding** (bits 240–255, 16 bits).
- **Padding invariant** — padding bits are always zero, for every
  bitboard the engine produces. It is what terminates runs at row ends
  without per-direction edge masks.
- **VALID** — the mask of the 225 real cells. Required after every `!`,
  because a complement sets all padding bits.
- **Mover** — the side that placed the last stone.
- **5-window** — five consecutive bit indices along one direction
  (`i, i+s, i+2s, i+3s, i+4s`).
- **Overline** — a run of six or more stones. Counts as a win (chapter
  13, decision 1).
- **Wrap** — an index path that crosses a row boundary while staying in
  the flat index space (e.g. `(7,14) → (7,15) → (8,0)` along `+1`).
- **Phantom five** — a 5-window whose cells are not collinear on the
  board, possible only across a wrap. The padding invariant makes them
  impossible in engine-produced bitboards.
- **Staged AND** — the 2→4→5 shift-AND construction of `has_five`
  (`b & b.shr(s)`, then `& shr(2s)`, then `& shr(s)`), which keeps every
  shift below 64.
- **Whole-board check** — `has_five_any`: "does a five exist anywhere
  in this bitboard?" The MCTS shape.
- **Neighbourhood check** — `wins_by_placing`: "did the stone just
  placed complete a five?" The `Board::play` shape.
- **Walk-based oracle** — a win detector that walks cells and counts
  neighbours: the reference engine's algorithm, the slice-3 interim
  detector in `board.rs`, and the test oracle kept in `win.rs`'s tests.
  Slow, obviously correct, and the thing the staged AND must agree with.
- **Differential test** — a property test asserting that the fast
  engine and the reference engine agree on every ply of random games.

## The slices

| # | File | You build | Slice of ch. 13 |
|---|------|-----------|------------------|
| 1 | [The workspace](01-the-workspace.md) | — (already scaffolded; orient yourself) | 1 |
| 2 | [The reference engine](02-the-reference-engine.md) | `Move`, naive `reference.rs` + corpus | 2 |
| 3 | [Bitboard and Board](03-bitboard-and-board.md) | `Bitboard`, `Board`, differential tests · [deep dives](03-bitboard-and-board/README.md) | 3 |
| 4 | [Win detection](04-win-detection.md) | `has_five`, edge/overline corpus | 4 |
| 5 | [Zobrist keys](05-zobrist.md) | const table, incremental key, undo | 5 |
| 6 | [Symmetry and encoding](06-symmetry-and-encoding.md) | D4 tables, 17×17 planes | 6 |
| 7 | [Tactics](07-tactics.md) | `MoveSet`, win-in-1/2 detection | 7 |
| 8 | [Threat-space search](08-threat-space-search.md) | bounded prover + line verifier | 8 |
| 9 | [The Swap2 opening](09-swap2-opening.md) | typestate opening machine | 9 |
| 10 | [Benchmarks and hardening](10-benchmarks-and-hardening.md) | criterion, visibility sweep | 10–11 |

## Commands cheat sheet

```bash
cd gomoku
cargo test -p engine                 # run the crate's tests
cargo test -p engine <name>          # run one test
cargo test -p engine --features testutil   # with the reference engine exposed
PROPTEST_CASES=10000 cargo test -p engine  # more property iterations
cargo bench -p engine                # slice 10
```

## Done means

The milestone-1 acceptance row from chapter 12: property tests green,
10k random games match the reference, the TSS soundness gate (ch. 12,
§12 item 3) at 100%, `has_five` ≥ 50M checks/s — plus a public API
surface you can show without embarrassment.
