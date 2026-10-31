# Chapter 07 — Positions from files: puzzles

Chapter 06 finished the visualization layer. This chapter enrolls the
first external position source into the study table: **puzzle packs**. A
puzzle is not a game mode; it is a *position with a claimed solution
line*, and the study table is where you study it. Loading a pack seeds
the variation-tree root, attacker to move, with Swap2 skipped; from
there, seats, branching, and every overlay from chapter 06 work by
construction. The felt capstone: **load a pack, ask for a hint, try a
wrong move, take it back, reveal the line, move to the next puzzle.**

## Architecture

```text
pack.json (git-ignored corpus, or the committed fixture)
   │
   ▼
PuzzlePack::from_json      pure parsing + validation, no I/O below main
   │                            (coordinate conversion happens here)
   ▼
SessionInit::puzzle ─────► SessionCore
   root = puzzle position        │
   attacker to move              ├── hint ──────► Event::Hint
   Swap2 skipped                 ├── reveal ────► Event::SolutionRevealed
                                 └── next/prev ─► Event::PuzzleLoaded
                                                 (clamp at the pack ends)
   │
   ├── main.rs: gomoku play --puzzle pack.json [--index N]
   │     (exit 2 + fetch.sh hint on a missing or invalid pack)
   │
   └── sessiondemo --puzzle pack.json --script puzzle_demo.txt
         (the scripted capstone)

render.rs    side-panel line: Puzzle: <i+1> of <count> · <attacker> to move
```

The core/shell doctrine is unchanged: parsing lives in the library
behind `SessionInit`, file I/O lives in `main.rs` and `sessiondemo`, and
events remain the only output of the core. Tests never touch `data/`:
a committed mini fixture under `crates/cli/tests/fixtures/` carries
every test.

## The sub-chapters

The old monolithic chapter 07 has been restructured into four
sub-chapters. Each sub-chapter has its own tutorial, design paper,
implementation notes, and opt-in solution file in its folder, and a
slice commit on `reference/chapter-07-redesign` that the solution
quotes verbatim.

| sub-chapter | what it teaches |
|---|---|
| [a — The pack parser](07-positions-from-files-puzzles/a-pack-parser/pack-parser.md) | A pack file becomes validated data — or a typed error that names exactly what is wrong. |
| [b — The puzzle enrolls](07-positions-from-files-puzzles/b-puzzle-session/puzzle-session.md) | Loading a puzzle seeds the study table: the position is the root, the attacker is to move, and every tool still works. |
| [c — The study commands](07-positions-from-files-puzzles/c-study-commands/study-commands.md) | `hint`, try, take back, `reveal`, `next` — the study flow through the command language. |
| [d — The puzzle flag](07-positions-from-files-puzzles/d-puzzle-flag/puzzle-flag.md) | `gomoku play --puzzle pack.json --index N` at the real table, with the panel line and the fetch.sh error path. |

## Background documents

* [00-design.md](07-positions-from-files-puzzles/00-design.md) — the
  design paper: a puzzle is a position not a mode, the coordinate trap,
  unlabelled packs are data, clamp-don't-wrap, the session keeps the
  whole pack, and missing data is a first-class error.

Next: [Chapter 08 — Positions from files: game records](08-positions-from-files-game-records.md)

## Glossary

| Term | Definition |
|------|------------|
| **Pack** | A JSON file of puzzle positions: `{v, count, items: [{i, a, d, b, w, l}]}`. The corpus format of `data/puzzles/files/`. |
| **Solution line** | The pack's claimed forced-win line for one puzzle: the `l` field, a list of `[x, y, side]` triples. A claim, not a proof — milestone 4 re-verifies every line with the threat-space search (TSS) prover before use. |
| **Attacker** | The side with the claimed forced win (the `a` field); always the side to move when the puzzle loads. In VCF (victory by continuous four) puzzles, the side that wins by an unbroken forcing sequence. |
| **Hint** | The command that shows the first move of the loaded puzzle's claimed line. |
| **Reveal** | The command that shows the full claimed line. |
| **Clamp** | `next` / `prev` stop at the pack ends with a typed error instead of wrapping around. |
| **`PuzzleStudy`** | The session context carrying the pack path, the current index, and the whole parsed pack, so `next` / `prev` need no further I/O. |
| **Fixture discipline** | Tests load only the committed `crates/cli/tests/fixtures/mini-pack.json`, never the git-ignored corpora under `data/`. |

## References

* [Playable CLI design paper](../../specs/2026-10-17-playable-cli-design.md)
  — the "positions from files" amendment (owner decision, 2026-10-17).
* [data/puzzles/README.md](../../../data/puzzles/README.md) — the
  corpus format, provenance, and the milestone-4 labelling plan.
* [Chapter 01 — The variation tree](01-the-variation-tree.md) — the
  tree a puzzle enrolls into.
* [Chapter 06 — Seeing the mind](06-seeing-the-mind.md) — the overlays
  that compose with a loaded puzzle.
* [README.md](README.md) — chapter map and tutorial conventions.
* The per-sub-chapter solution files live in the `a-pack-parser`,
  `b-puzzle-session`, `c-study-commands`, and `d-puzzle-flag` folders.
