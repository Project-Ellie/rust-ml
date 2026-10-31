# Sub-chapter 07d — The puzzle flag: design paper

## Abstract

This paper records the decisions behind the `--puzzle` startup flag.
The flag lets a user open the study table directly on a puzzle
position. Swap2 is skipped, the `--ai` / `--color` / `--sims` flags
compose as for any opening, and missing data files are first-class
errors that point at the corpus fetch script.

## Glossary

| Term | Definition |
|------|------------|
| **Puzzle pack** | A JSON file of puzzle positions produced by the parser from sub-chapter 07a. |
| **`SessionInit::puzzle`** | The constructor that attaches a `PuzzleStudy` to a session. |
| **`PuzzleStudy`** | The context carrying the pack path, the current index, and the parsed pack. |
| **Exit code 2** | The convention for usage and data errors, matching clap's own errors. |
| **Exit code 1** | The convention for session I/O errors, used by `run_play` and `run_selfplay`. |

## Context

Sub-chapters 07a, 07b, and 07c built the parser, the session context,
and the study commands. This sub-chapter is the surface: the command
line that loads a pack and the panel that shows which puzzle is open.

The core/shell doctrine says the library must not do I/O. The puzzle
file is therefore read and parsed in `main.rs`, and the parsed pack is
handed to `SessionInit::puzzle`.

## Decision 1 — `--puzzle` belongs to `play`, not `selfplay`

A puzzle is a position for human study. `selfplay` is a game
generator; starting it mid-position would contradict its purpose. The
binary rejects `--puzzle` for `selfplay` with exit code 2.

## Decision 2 — `--index` requires `--puzzle`

An index selects a puzzle within a pack. Without a pack, an index has
no meaning. clap's `requires` attribute enforces this at parse time,
so the error path is the same as any other malformed argument.

## Decision 3 — Missing data files are first-class errors

The puzzle corpora under `data/puzzles/` are git-ignored. A user who
runs `--puzzle data/puzzles/files/vcf-material-763.json` before
running `data/puzzles/fetch.sh` will hit "file not found". That case
is a usage/data error, not a crash, so the binary exits 2 and prints a
message that names the path and points at `data/puzzles/fetch.sh`.

## Decision 4 — Invalid packs exit 2, not 1

A malformed JSON file or an out-of-range index is also a usage/data
error. The same `build_puzzle_init` helper handles read failures,
parse failures, and index failures with the same exit code 2.

## Decision 5 — `sessiondemo` keeps its own exit code

`sessiondemo` is a demo binary, not a tool. Its existing parse/flag
errors use exit code 1. This sub-chapter does not change that; only
the `gomoku` tool uses exit code 2 for usage/data errors.

## Decision 6 — Seats and AI flags compose exactly as for Swap2

`build_puzzle_init` builds a normal `play_init` first, then replaces
the opening board and attaches the puzzle context. This means `--ai`,
`--color`, `--sims`, `--black-sims`, `--white-sims`, and `--seed` all
behave identically to a Swap2 opening. There is no special "puzzle
seat logic".

## Decision 7 — The puzzle panel line lives under `Path:`

The side panel already shows the position's path tail. The puzzle line
is one more fact about the current position, so it is placed
immediately under `Path:`. It is not a separate mode block and does
not replace the `Ply:` / `To move:` lines.

## Decision 8 — HELP lists the puzzle commands

`hint`, `reveal`, and `next / prev` are part of the typed command
language, so they belong in `repl.rs` `HELP`. The same lines are added
to the byte-exact line-mode transcript so the transcript stays
byte-identical to the rendered help screen.

## Data flow

```text
argv
  |
  v
PlayArgs (clap)
  |
  v
main() Selfplay arm  --puzzle? --→ usage_error → exit 2
  |
  v
run_play()
  |
  +-- puzzle flag absent --→ Swap2 opening path
  |
  +-- puzzle flag present --→ build_puzzle_init()
          |
          v
      read file --fail--> exit 2 + fetch.sh hint
          |
          v
      PuzzlePack::from_json --fail--> exit 2
          |
          v
      pack.get(index) --fail--> exit 2
          |
          v
      play_init(search, false)
          |
          v
      init.opening = puzzle board
      init.puzzle = PuzzleStudy { ... }
          |
          v
      study::run(init, None)
          |
          v
      render_study()
          |
          v
      render_side_panel()
          |
          v
      position_lines() --puzzle?--> "Puzzle: i+1 of count · attacker to move"
```

## References

* [Sub-chapter 07a — The pack parser](../a-pack-parser/pack-parser.md) — the parser consumed by this slice.
* [Sub-chapter 07b — The puzzle session](../b-puzzle-session/puzzle-session.md) — the `PuzzleStudy` context.
* [Sub-chapter 07c — Study commands](../c-study-commands/study-commands.md) — the commands listed in HELP.
* [Chapter 07 — Positions from files: puzzles](../../07-positions-from-files-puzzles.md) — the chapter overview.
* [Playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md) — the "positions from files" amendment.
