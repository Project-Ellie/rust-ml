# Sub-chapter 07c — The study commands: design paper

## Abstract

This paper records the decisions behind the four study commands:
`hint`, `reveal`, `next`, and `prev`. The commands turn a loaded puzzle
pack into an interactive study flow through the same command language a
web server will one day speak. The design favors explicit typed errors
over silent state changes: navigation clamps at pack ends, unlabelled
puzzles are reported but not rejected, and read-only commands never
play moves.

## Glossary

| Term | Definition |
|------|------------|
| **Study command** | `hint`, `reveal`, `next`, or `prev`. |
| **Claimed line** | The pack's `l` field: a sequence of moves with side markers. |
| **Clamp** | Navigation that stops at the pack ends with an error instead of wrapping. |
| **Unlabelled pack** | A pack whose items have no `l` field; positions only. |
| **`NoSolutionLine`** | Typed error for a hint or reveal on an unlabelled puzzle. |
| **`PuzzleLoaded`** | Event emitted by successful `next` / `prev`; fields are 0-based. |
| **Read-only command** | A command that emits information but does not mutate the board. |

## Context

The playable CLI design paper states that the command language is the
shared vocabulary between every front-end. Sub-chapter 07a defined that
vocabulary for packs (`PuzzlePack::from_json`), and sub-chapter 07b
added the session constructor (`SessionInit::puzzle`) that stores a
whole pack in memory. This sub-chapter extends the command vocabulary
and the session executor so the user can study the loaded pack.

## Decision 1 — The command language is the seam

`hint`, `reveal`, `next`, and `prev` are ordinary `Command` variants.
They are parsed by the same `command::parse` function and executed by
the same `SessionCore::execute` method as every other command. Keeping
them in the shared vocabulary means a future web server can speak them
without a separate protocol layer.

## Decision 2 — Clamp, do not wrap

`next` at the last puzzle and `prev` at the first puzzle could wrap to
the opposite end. Wrapping is silent, and because every puzzle load
replaces the variation tree root, it discards any branches the user was
studying without warning. Clamping returns typed errors
(`AtFirstPuzzle` and `AtLastPuzzle`) and leaves the current puzzle
loaded. A front-end can catch those errors and grey out the navigation
button.

## Decision 3 — Unlabelled packs are data, not errors

Two of the three puzzle corpora are positions only; milestone 4 will
label them with the TSS prover. A missing `l` field therefore parses
successfully to an empty solution. `hint` and `reveal` on such a puzzle
return `NoSolutionLine { index, count }`, naming the puzzle so a
front-end can grey out the study buttons instead of crashing.

## Decision 4 — Hint and reveal are read-only

`hint` and `reveal` only emit events. They never play a move, even when
the hinted move is legal. A "try" is still a `Command::Play` like any
other; the variation tree holds it as a branch. This preserves the
study-table metaphor: the user compares their own try against the
claimed line, takes it back, or asks for the answer.

## Decision 5 — PuzzleLoaded keeps machine fields 0-based and human text 1-based

`Event::PuzzleLoaded` carries `index` and `count` as 0-based values so
that machine consumers do not need to reverse an off-by-one transform.
Its `Display` implementation adds 1 to the index and prints
`puzzle {} of {count} · {attacker:?} to move`. This separation is the
same one used for move coordinates inside the engine.

## Decision 6 — Navigation rebuilds the tree wholesale but preserves seats and AI configuration

`execute_puzzle_nav` replaces `self.history` with
`History::from_opening(&new_board)` and clears `self.search_info`. It
does not touch seats, overrides, or AI configuration. The invariant is
that a puzzle is just a position source; the rest of the session state
is the same table you already have. If White was an AI at 800
simulations on puzzle 4, it still is on puzzle 5.

## Decision 7 — `new` resets to the initially loaded puzzle

`SessionInit::puzzle` stores the initial index. `reset` restores
`self.puzzle` from `self.init.puzzle`, so `Command::New` always returns
to the puzzle the session started with, not to the puzzle the user
navigated to. This makes `new` predictable: it is "start this study
session over", not "go back one puzzle".

## Decision 8 — The demo binary is allowed to exit on bad data

`sessiondemo` is a developer tool, not a production server. When
`--puzzle` points to an unreadable or invalid file, `build_init` prints
an error and exits with code 1. This matches the existing `--script`
behavior and keeps the demo script small and self-contained.

## Data flow

```text
typed word            typed word              typed word
   |                       |                       |
   v                       v                       v
"hint"                 "reveal"              "next" / "prev"
   |                       |                       |
   +-------> command::parse <----------------------+
                       |
                  Command variant
                       |
              SessionCore::execute
                       |
       execute_hint / execute_reveal / execute_puzzle_nav
                       |
              Event::Hint / Event::SolutionRevealed / Event::PuzzleLoaded
                       |
              front-end renders message / board
```

## References

* [Playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md)
  — §2 command language as shared vocabulary.
* [Sub-chapter 07a — The pack parser](../a-pack-parser/pack-parser.md)
* [Sub-chapter 07b — The puzzle session](../b-puzzle-session/puzzle-session.md)
* [Sub-chapter 07d — The puzzle flag](../d-puzzle-flag/puzzle-flag.md)
* [docs/12-gomoku-architecture.md](../../../../12-gomoku-architecture.md)
