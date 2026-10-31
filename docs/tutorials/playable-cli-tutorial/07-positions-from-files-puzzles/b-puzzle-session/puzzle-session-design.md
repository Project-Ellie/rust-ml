# Sub-chapter 07b — The puzzle enrolls: design paper

## Abstract

This paper records the decisions behind wiring a parsed puzzle pack
into the study session. A puzzle is a position, not a mode. The
session keeps the whole parsed pack in memory, uses `SessionInit` as
the source of truth for resets, and skips Swap2 because the puzzle
position is already the opening.

## Glossary

| Term | Definition |
|------|------------|
| **Puzzle** | A validated position with a claimed solution line (`Puzzle` in `src/puzzle.rs`). |
| **Puzzle pack** | A parsed `PuzzlePack`; a list of puzzles in file order. |
| **PuzzleStudy** | The session context carrying the pack path, current index, and the whole pack. |
| **Core/shell doctrine** | The rule that the core library does no I/O; file access lives in binaries. |
| **Attacker** | The side to move in the loaded puzzle position. |

## Context

Sub-chapter 07a implemented the pack parser. The parser validates the
JSON format and produces a `PuzzlePack`. This sub-chapter is the
bridge: it takes that value and seeds a `SessionCore` with it. The
next sub-chapters add commands that read the stored context and a
startup flag that builds it.

## Decision 1 — A puzzle is a position, not a mode

The legacy puzzle viewer had its own board and input loop. The study
table deletes that idea. Loading a puzzle produces the same kind of
session as a Swap2 opening or a hotseat game: a `SessionCore` with a
`History` rooted on a `Board`. The puzzle adds only metadata above the
session layer.

This means seats, branching, navigation, and the chapter-06 overlays
all work without puzzle-specific code below `session.rs`. The table
does not grade moves; a wrong try becomes a branch, exactly as it
would in any other position.

## Decision 2 — The session keeps the whole parsed pack

`SessionInit::puzzle` takes the parsed `PuzzlePack`, not a path. The
core stores the entire pack in `PuzzleStudy` so that sub-chapter 07c
can implement `NextPuzzle` and `PrevPuzzle` without further file I/O.
This follows the core/shell doctrine: `main.rs` and `sessiondemo` read
the file once at startup; the library never touches the filesystem.

## Decision 3 — `pack_path` is kept only for display and errors

`PuzzleStudy` stores `pack_path: PathBuf` even though the core does
not read it. The path is useful for error messages, side-panel labels,
and startup diagnostics. It is not the source of truth for loading;
the parsed `pack` is.

## Decision 4 — Reset uses `SessionInit` as the source of truth

`SessionCore::reset` restores `self.puzzle` from `self.init.puzzle`.
`self.init` records the session as it was first built, including the
initially loaded puzzle index. When sub-chapter 07c adds navigation,
the live `self.puzzle.index` can change, but `self.init.puzzle.index`
remains fixed. Therefore `Command::New` always returns to the puzzle
the session started with, not the one the user navigated to.

## Decision 5 — `swap2` is `false` because the puzzle position is the opening

`SessionInit::puzzle` sets `swap2: false`. A puzzle position is itself
the opening; running a Swap2 ceremony on top of it would replace the
position the user asked to study. The constructor also rejects the
combination implicitly: `SessionCore::new` already asserts that Swap2
requires an empty opening board.

## Data flow

```text
PuzzlePack::from_json (07a)
        |
   SessionInit::puzzle(pack_path, pack, index)
        |
   pack.get(index) validates index
        |
   opening = puzzle.board.clone()
        |
   SessionCore::new(init)
        |
   history = History::from_opening(&init.opening)
   core.puzzle = init.puzzle.clone()
        |
   core.puzzle() -> Option<&PuzzleStudy>
        |
   reset() restores core.puzzle from init.puzzle
```

## References

* [Sub-chapter 07a — The pack parser](../a-pack-parser/pack-parser.md) —
  the parser that produces `PuzzlePack`.
* [Sub-chapter 07c — Study commands](../c-study-commands/study-commands.md) —
  the commands that consume `PuzzleStudy`.
* [Sub-chapter 07d — The puzzle flag](../d-puzzle-flag/puzzle-flag.md) —
  the startup flag that feeds `SessionInit::puzzle`.
* [Chapter 07 overview](../../07-positions-from-files-puzzles.md) —
  the chapter spec and the "positions from files" amendment.
