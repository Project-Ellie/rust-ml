# Side Quest — A Terminal UI for the Engine

This tutorial builds a playable terminal front-end for your Gomoku
engine: an ASCII board that redraws in place (no scrolling — display
area on top, command line at the bottom), human-vs-human for now, on
**either board implementation**, selected at startup:

```text
 0  1  2  3  4  5  6  7  8  9 10 11 12 13 14
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 ...
 .  .  .  .  .  .  . [x] o  .  .  .  .  .  .
 ...
 0  1  2  3  4  5  6  7  8  9 10 11 12 13 14

engine: fast · move 2 · Black to move

> 7 9
```

Every field is exactly 3 characters wide: ` . ` empty, ` x ` black,
` o ` white. Column numbers sit on top *and* on the bottom, so your
eyes never have to travel far. It is the embryo of chapter 12's
`gomoku play` — later the opponent seat belongs to the network.

## Wait — a trait? Chapter 13 said *no shared trait*!

It did — **inside the engine crate**. That rule protects the hot loop:
self-play calls `Board` methods millions of times per second, so the
engine keeps static dispatch and the differential tests drive both
board types directly. No abstraction before a consumer needs one.

This tutorial is that consumer. The CLI genuinely selects a board
**at runtime** (`--engine naive|fast`), so *it* owns a trait —
defined in the `cli` crate, implemented for the engine's types from
the outside. The engine never sees it:

- The engine crate changes only in ways chapter 13 already plans:
  the public re-exports from its "Crate layout" section, plus one
  small additive method (chapter 1 below).
- All UI code calls the board through `&dyn GameBoard` — one vtable
  hop per human keystroke. Self-play and MCTS will never touch this
  trait.
- The adapters only compile because both boards speak the same API —
  slice 3's "same method names" contract, now enforced from outside
  the crate. If the two boards ever drift apart, the CLI stops
  compiling. Free regression test.

## Prerequisite

**Slices 2 and 3 of the
[engine tutorial](../13-engine-tutorial/README.md) are complete** —
the naive oracle *and* the bitboard `Board` exist, and
`Color`/`Status`/`PlayError` have moved into their shared module.
If you are mid-slice-3: finish it first. This side quest is the
reward — playing on the board you just proved correct.

## How each chapter works

Unlike tutorial 13 (which withholds implementations so you build the
engine yourself), this is a side quest: **each chapter ends with a
full solution**. The rhythm is still contract-first — read the
contract, try it yourself, then compare with the solution. The
solutions are verified: they compile and their tests pass against a
slice-3-complete engine.

Chapters 1–4 give you a working, scrolling version. Chapter 5 is the
"double cool" one: alternate screen, fixed layout, zero scrolling.
Chapter 6 is optional polish (last-move marker, colors, and an
`undo` command).

## The chapters

| # | File | You build |
|---|------|-----------|
| 1 | [The CLI crate](01-the-cli-crate.md) | workspace wiring, engine re-exports, `Board::stone_at` |
| 2 | [The GameBoard trait](02-the-gameboard-trait.md) | object-safe trait, adapters, `Box<dyn>` factory |
| 3 | [Rendering](03-rendering.md) | pure render function, 3-char cells, ASCII frame, four-sided rulers |
| 4 | [The game loop](04-the-game-loop.md) | command parser, scrolling REPL, playable |
| 5 | [The alternate screen](05-alternate-screen.md) | crossterm, RAII guard, draw-in-place UI |
| 6 | [Polish](06-polish.md) | last-move marker, colors, undo command |

## Commands cheat sheet

```bash
cd gomoku
cargo run -p cli                          # play on the fast board
cargo run -p cli -- --engine naive        # play on the oracle
cargo run -p cli -- --plain               # scrolling fallback UI
cargo test -p cli                         # render + parser tests
```

## Done when

You can start the tool on either board, play a full game to a win,
see it announced, take moves back with `u`, restart with `new`, and
quit with `q` — all without the terminal scrolling a single line.
