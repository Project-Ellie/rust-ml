# Sub-chapter 07a — The pack parser: design paper

## Abstract

This paper records the decisions behind the puzzle pack parser. The
parser reads a JSON corpus file, validates every position, and turns it
into a `PuzzlePack` of replayable `Board` values. The hardest part is
not the JSON; it is the coordinate convention and the decision to reject
problems that the bitboard builder would otherwise hide.

## Glossary

| Term | Definition |
|------|------------|
| **File coordinate** | `[x, y]` in the pack, where `x` is the column and `y` is the row, 0-based. |
| **Engine coordinate** | `Move::new(row, col)`, where the first argument is the row. |
| **Coordinate trap** | The silent bug that happens when file `[x, y]` is read as `Move::new(x, y)` instead of `Move::new(y, x)`. |
| **Strict validation** | Rejecting duplicate stones and overlapping colors even though `Board::from_position` would collapse them. |
| **Turn invariant** | In Gomoku, Black moves first. The side to move and the stone counts must agree. |
| **`PackFile` / `PackItem`** | The private serde schema types. They ignore unknown fields and default the optional `l` field. |

## Context

The project owns three puzzle corpora in one JSON format. One corpus is
labelled with claimed solution lines; two are not. The labelled corpus
will later be re-verified by the threat-space search prover in milestone
4. Until then, the table needs only to read the packs safely.

The parser is the first slice of chapter 07 because every later slice
needs validated `Puzzle` data. The session constructor, the study
commands, and the startup flag all assume the pack has already been
checked.

## Decision 1 — File coordinates are `[x, y]` = column, row

The legacy puzzle viewer rendered these packs with `x` as the column and
`y` as the row. The engine stores moves as `Move::new(row, col)`. So the
parser maps file `[x, y]` to `Move::new(y, x)`. This happens in
`parse_stones` (`src/puzzle.rs`, lines 284–288) and in the line parser
(line 272).

The trap is that a swapped mapping still produces a legal board. Stones
stay inside the 15×15 grid, fives stay fives, and the parser tests still
pass except for the one test that pins the convention. That test
(`file_coordinates_are_x_column_y_row`, `tests/puzzle.rs`, lines 40–51)
exists because the bug is otherwise invisible.

## Decision 2 — Line side markers are `1` = Black, `2` = White

The corpus uses integers for line sides. `1` means Black and `2` means
White. This mapping is hard-coded in `parse_item` (`src/puzzle.rs`,
lines 254–263). The first move of the line must belong to the attacker;
otherwise the parser returns `InvalidSolutionSide` (lines 265–271).

Requiring the first line move to be the attacker's guarantees that the
pack's claim matches the position it describes. A line that starts with
the defender would be a different puzzle.

## Decision 3 — Validation is strict before the board is built

`Board::from_position` would silently collapse duplicate stones within a
color and would reject overlapping colors only at the position level.
The parser checks duplicates and overlap explicitly in `parse_item`
(`src/puzzle.rs`, lines 220–241). This produces `DuplicateStone` and
`OverlappingColors` errors that name the item, instead of a quiet data
loss.

The same strictness applies to the turn invariant. The parser lets
`Board::from_position` enforce the invariant, but it maps the error to a
rich `InvalidCounts` variant that records the stone counts and the
attacker (`src/puzzle.rs`, lines 243–251).

## Decision 4 — The turn invariant is enforced by `Board::from_position`

The parser does not duplicate the Gomoku turn rules. It hands the stone
lists and the attacker to `Board::from_position` and translates the
result. This keeps the rules in one place: the engine. The parser only
adds the item number and the counts to the error message.

## Decision 5 — The private schema reads only what it keeps

The corpora contain extra keys such as `v`, `d`, `u`, and `f`. Serde
ignores unknown fields by default, and the private `PackFile` /
`PackItem` types declare only the fields the parser actually uses
(`src/puzzle.rs`, lines 183–201). The schema does not carry `v` or `d`
because no consumer reads them yet. A field earns a place in the schema
when a consumer needs it, not before.

This keeps the public API small and makes the parser's memory footprint
honest: it does not keep data it cannot use. If the format changes
later, the schema can change without affecting callers.

### Open questions

* Format-version validation: the pack format carries a top-level `v`.
  The parser currently ignores it because only version 1 exists. Add
  `v` to `PackFile` and validate it when a version 2 format appears.
* Claimed depth: items carry `d`, a claimed line depth. Add it to
  `PackItem` if a consumer (for example, a solver-hint command) needs
  it.

## Decision 6 — Unlabelled packs are data, not errors

The `l` field is optional. Two of the three corpora are unlabelled, and
milestone 4 will label them with the TSS prover. The schema marks the
field with `#[serde(rename = "l", default)]` (`src/puzzle.rs`, line
200–201). A missing line becomes an empty `solution` vector.

This choice means `PuzzlePack::from_json` succeeds for unlabelled packs.
The later `hint` and `reveal` commands will return their own typed
error (`NoSolutionLine`) when a puzzle has no line.

## Decision 7 — `count` must equal `items.len()`

The top-level `count` field is a redundant length. The parser checks it
before parsing any item (`src/puzzle.rs`, lines 51–56). If it is wrong,
the parser returns `CountMismatch` immediately. This catches a truncated
or edited file before the parser does expensive validation work.

## Decision 8 — `get` bounds-checks with `IndexOutOfRange`

`PuzzlePack::get` is a small wrapper around `Vec::get` that turns
`None` into a typed error (`src/puzzle.rs`, lines 71–76). The error
records the requested index and the pack length. This lets the session
layer report a clean message when `--index` is too large, instead of
panicking.

## Central design rule and invariants

**Central rule:** a parsed puzzle is a replayable `Board` position with
the attacker to move, and every violation is named by a `PuzzleError`
variant.

Invariants:

1. `PuzzlePack::from_json` returns `Ok` only when every item passes all
   validation checks.
2. `Puzzle.board` always satisfies `board.to_move() == puzzle.attacker`.
3. `Puzzle.solution` is empty when the pack item has no `l` field.
4. `PuzzleError` always carries enough data to build a useful error
   message: the item number, the raw values, or the counts.

## Data flow

```text
pack JSON text
    |
PuzzlePack::from_json
    |
serde_json::from_str -> PackFile
    |
count == items.len()?
    |
for each item:
    |
parse_item
    |
    +-- parse attacker -> Color
    +-- parse_stones -> Vec<Move>
    +-- duplicate / overlap check
    +-- Board::from_position -> Board
    +-- parse line -> Vec<(Move, Color)>
    |
Puzzle { index, attacker, board, solution }
    |
PuzzlePack { items }
    |
PuzzlePack::get(index) -> &Puzzle or IndexOutOfRange
```

## References

* [Sub-chapter 07a — The pack parser](./pack-parser.md) — the tutorial.
* [Chapter 07 — Positions from files: puzzles](../../07-positions-from-files-puzzles.md) — the full chapter.
* [Sub-chapter 07b — The puzzle session](../b-puzzle-session/puzzle-session.md) — the next slice.
* [`data/puzzles/README.md`](../../../../../data/puzzles/README.md) — the pack format.
* [`gomoku/crates/cli/src/puzzle.rs`](../../../../../gomoku/crates/cli/src/puzzle.rs) — the parser module.
