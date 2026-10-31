# Sub-chapter 07a — The pack parser

## Abstract

This sub-chapter turns a puzzle pack file into validated data. A pack is
a JSON object that holds Gomoku positions with a claimed solution line.
The parser checks the format, the coordinates, the stones, the turn
invariant, and the line. If anything is wrong, it returns a typed
`PuzzleError` that names the exact item and the exact problem. If the
pack is clean, you get a `PuzzlePack` of replayable `Puzzle` values.

## Glossary

| Term | Definition |
|------|------------|
| **Pack** | A JSON file of puzzle positions: `{v, count, items: [{i, a, d, b, w, l}]}`. The corpus format of `data/puzzles/files/`. |
| **Puzzle** | One parsed position plus its claimed line, in the type `Puzzle`. |
| **`PuzzlePack`** | The public type that holds all `Puzzle` items in file order. |
| **`PuzzlePack::from_json`** | The entry point that parses a pack text and validates every item. |
| **`PuzzleError`** | The typed error enum that reports what is wrong with a pack. |
| **Attacker** | The side to move in a puzzle, stored in the `a` field. |
| **Solution line** | The pack's claimed forced-win line, stored in the `l` field as `[x, y, side]` triples. |
| **Line side marker** | `1` means Black, `2` means White. The first marker must be the attacker's side. |
| **Coordinate trap** | File coordinates are `[x, y]` with `x` as the column and `y` as the row. They map to `Move::new(y, x)`. Swapping them silently mirrors every position. |
| **Pack schema** | The private `PackFile` / `PackItem` serde types that read the JSON. |
| **Fixture discipline** | Tests load only the committed `crates/cli/tests/fixtures/mini-pack.json`, never the git-ignored corpora under `data/`. |

## Context

Chapter 06 finished the visualization layer. Chapter 07 adds external
position sources. The first source is puzzle packs. The packs already
exist in `data/puzzles/`, but their content is third-party and
git-ignored. This sub-chapter builds only the parser. The session
constructor, the study commands, and the `--puzzle` flag are the next
three sub-chapters.

The parser is a library slice. It touches no files, no terminal, and no
network. `main.rs` and `sessiondemo` will read the pack text later and
hand it to `PuzzlePack::from_json`.

## Intention

The learner enables this user-facing behavior:

1. A JSON pack text becomes a `PuzzlePack` with all positions validated.
2. Every puzzle is a replayable `Board` with the attacker to move.
3. File coordinates use the legacy convention `[x, y]` = column, row.
4. Invalid JSON, count mismatch, off-board coordinates, duplicate
   stones, overlapping colors, bad turn counts, unknown attackers, bad
   line markers, and a line that starts with the wrong side are all
   rejected with a typed `PuzzleError`.
5. A missing `l` field is allowed and produces an empty solution line.
6. `PuzzlePack::get` returns the puzzle at a zero-based index or an
   `IndexOutOfRange` error.

## Specification as tests

The tests below are ordered along the execution path of
`PuzzlePack::from_json`: file text → serde → count check → item parsing
→ coordinate conversion → stone validation → board construction → line
validation → index lookup.

### Happy path

* `parses_valid_pack` — the committed mini fixture parses into three
  puzzles. The first puzzle is Black to move and has one solution move.
  The third puzzle has a three-ply line.
* `file_coordinates_are_x_column_y_row` — file stone `[7, 3]` lands on
  column 7, row 3, not the other way around. The solution move `[7, 7]`
  formats as `h8`.
* `board_is_replayable_position_with_attacker_to_move` — the parsed
  `Board` reports the attacker as the side to move and its move history
  is empty, because the position was loaded from the pack rather than
  played from the start.

### Validation errors

* `malformed_json_is_rejected` — a text that is not JSON returns
  `PuzzleError::Json`.
* `count_mismatch_is_rejected` — a top-level `count` that does not equal
  `items.len()` returns `PuzzleError::CountMismatch`.
* `off_board_coordinate_is_rejected` — a stone coordinate outside the
  15×15 board returns `PuzzleError::InvalidCoord`.
* `off_board_line_coordinate_is_rejected` — an off-board coordinate in
  the solution line also returns `PuzzleError::InvalidCoord`.
* `duplicate_stone_is_rejected` — the same cell twice in one color's
  list returns `PuzzleError::DuplicateStone`.
* `overlapping_colors_are_rejected` — a cell in both `b` and `w`
  returns `PuzzleError::OverlappingColors`.
* `counts_disagreeing_with_attacker_are_rejected` — stone counts that
  break the Gomoku turn invariant for the attacker return
  `PuzzleError::InvalidCounts`.
* `unknown_attacker_is_rejected` — an `a` field that is not `black` or
  `white` returns `PuzzleError::InvalidAttacker`.
* `line_starting_with_the_wrong_side_is_rejected` — a solution line
  whose first side marker is not the attacker returns
  `PuzzleError::InvalidSolutionSide`.
* `unknown_line_side_marker_is_rejected` — a side marker that is not
  `1` or `2` returns `PuzzleError::InvalidSide`.

### Optional data

* `missing_line_defaults_to_empty_solution` — an item with no `l` field
  parses with an empty solution vector.

### Lookup

* `index_out_of_range_is_rejected` — `PuzzlePack::get` returns the
  puzzle for a valid index and `PuzzleError::IndexOutOfRange` for an
  invalid one.

## Walkthrough

The parser keeps an honest schema: it declares only the fields the
table reads. The pack format carries more (`v` format version, `d`
claimed depth, and the corpora's `u`/`f` keys), and serde ignores all
of them. A field earns a place in the schema when a consumer needs it.

### 1. The file text arrives

`PuzzlePack::from_json` takes an `&str`. It does not open a file. That
keeps the core free of I/O and makes tests easy: they pass inline
strings or `include_str!("fixtures/mini-pack.json")`.

### 2. Serde reads the private schema

`from_json` calls `serde_json::from_str::<PackFile>(text)`. The schema
types `PackFile` and `PackItem` are private. Serde ignores unknown
fields by default, so the extra `v`, `d`, `u`, and `f` keys in the
corpora are ignored. The `l` field is renamed to `line` and defaults to
an empty vector when it is missing.

### 3. Count is checked first

After deserialization, `from_json` compares `file.count` to
`file.items.len()`. If they differ, it returns `PuzzleError::CountMismatch`
before it looks at any item. This catches a corrupt pack early and gives
a clear message.

### 4. Each item is parsed

`from_json` enumerates the items and calls `parse_item(pos, item)`. The
`pos` is the zero-based pack index. The item's own `i` field is used
only in error messages.

### 5. Coordinates are swapped into moves

`parse_stones` turns each file coordinate `[x, y]` into
`Move::new(y, x)`. The file stores the column first; the engine stores
row first. Swapping them here is the legacy convention. The test
`file_coordinates_are_x_column_y_row` pins the convention, because
swapping would still produce a legal board but every puzzle would be
wrong.

### 6. Stones are checked for duplicates and overlap

`parse_item` builds a `HashSet` of occupied indices. It walks Black's
stones, then White's. A duplicate within one color returns
`DuplicateStone`. A cell already occupied by the other color returns
`OverlappingColors`. The bitboard builder would collapse duplicates
silently, so the parser checks explicitly.

### 7. Board enforces the turn invariant

`Board::from_position(&black, &white, attacker)` builds the board. It
returns `PositionError::InvalidCounts` when the stone counts do not
match the attacker. The parser maps that to `PuzzleError::InvalidCounts`.
`Board::from_position` also rejects overlapping colors, but the parser
has already checked for that.

### 8. The claimed line is validated

For each `[x, y, side]` triple in the line, the parser converts the
side marker to `Color::Black` (`1`) or `Color::White` (`2`). An unknown
marker returns `PuzzleError::InvalidSide`. The first move must be by the
attacker; otherwise the parser returns `PuzzleError::InvalidSolutionSide`.
The coordinate is converted with `Move::new(y, x)` and checked for being
on the board.

### 9. Accessing a puzzle by index

`PuzzlePack::get(index)` looks up `self.items.get(index)`. On success it
returns `&Puzzle`. On failure it returns `PuzzleError::IndexOutOfRange`
with the requested index and the pack length.

## Low-level design

### [`gomoku/crates/cli/Cargo.toml`](../../../../../gomoku/crates/cli/Cargo.toml)

Move `serde_json` from `[dev-dependencies]` into `[dependencies]`. The
parser lives in the library, so the library needs it at runtime. The
`serde` dependency is already present. `thiserror` is already in
`[dependencies]`.

### [`gomoku/crates/cli/src/lib.rs`](../../../../../gomoku/crates/cli/src/lib.rs)

Add `pub mod puzzle;` so the module is reachable from tests and from the
shell crates.

### [`gomoku/crates/cli/src/puzzle.rs`](../../../../../gomoku/crates/cli/src/puzzle.rs)

New module. Public surface:

```rust
pub struct Puzzle {
    pub index: usize,
    pub attacker: Color,
    pub board: Board,
    pub solution: Vec<(Move, Color)>,
}

pub struct PuzzlePack {
    pub items: Vec<Puzzle>,
}

impl PuzzlePack {
    pub fn from_json(text: &str) -> Result<Self, PuzzleError>;
    pub fn get(&self, index: usize) -> Result<&Puzzle, PuzzleError>;
}
```

`PuzzleError` variants: `Json`, `CountMismatch`, `InvalidAttacker`,
`InvalidCoord`, `DuplicateStone`, `OverlappingColors`, `InvalidCounts`,
`InvalidSide`, `InvalidSolutionSide`, `IndexOutOfRange`.

The private schema types `PackFile` and `PackItem` use serde derives and
field renames. Unknown fields are ignored. The `l` field defaults to an
empty vector.

`parse_item` is the item-level validation pipeline. `parse_stones` is
the helper that maps file coordinates to `Move` values.

### [`gomoku/crates/cli/tests/fixtures/mini-pack.json`](../../../../../gomoku/crates/cli/tests/fixtures/mini-pack.json)

A hand-verified fixture with three puzzles:

1. Black to move, a vertical four on column h, solution `h8`.
2. White to move, a horizontal four on row 5, solution `i5`.
3. Black to move, a three-ply forcing line.

Each item carries the extra `u` and `f` keys to prove the schema ignores
unknown fields.

### [`gomoku/crates/cli/tests/puzzle.rs`](../../../../../gomoku/crates/cli/tests/puzzle.rs)

Fifteen parser tests. They load the mini fixture or inline JSON. They
never read `data/`.

## TDD checklist

Work one test group at a time, red to green.

From `gomoku/crates/cli/tests/puzzle.rs`:

1. `parses_valid_pack`
2. `file_coordinates_are_x_column_y_row`
3. `board_is_replayable_position_with_attacker_to_move`
4. `malformed_json_is_rejected`
5. `count_mismatch_is_rejected`
6. `off_board_coordinate_is_rejected`
7. `off_board_line_coordinate_is_rejected`
8. `duplicate_stone_is_rejected`
9. `overlapping_colors_are_rejected`
10. `counts_disagreeing_with_attacker_are_rejected`
11. `unknown_attacker_is_rejected`
12. `line_starting_with_the_wrong_side_is_rejected`
13. `unknown_line_side_marker_is_rejected`
14. `missing_line_defaults_to_empty_solution`
15. `index_out_of_range_is_rejected`

## Verification commands

From `gomoku/`:

```text
cargo test -p cli --test puzzle
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Done when

* The 15 parser tests pass.
* `cargo clippy --all-targets -- -D warnings` passes.
* `cargo fmt --all -- --check` makes no changes.
* Every `PuzzleError` variant is exercised by at least one test.
* The parser uses only the committed fixture; no test reads `data/`.

## References

* [Chapter 07 — Positions from files: puzzles](../../07-positions-from-files-puzzles.md) — the full chapter this slice belongs to.
* [Sub-chapter 07b — The puzzle session](../b-puzzle-session/puzzle-session.md) — loads a parsed puzzle into the study table.
* [Sub-chapter 07c — The study commands](../c-study-commands/study-commands.md) — `hint`, `reveal`, `next`, `prev`.
* [Sub-chapter 07d — The puzzle flag](../d-puzzle-flag/puzzle-flag.md) — the `--puzzle` startup flag.
* [Sub-chapter 06c — The prior view](../../06-seeing-the-mind/c-prior-overlay/prior-overlay.md) — the chapter this work follows.
* [`data/puzzles/README.md`](../../../../../data/puzzles/README.md) — the pack format and the corpora.
* [`gomoku/crates/cli/src/puzzle.rs`](../../../../../gomoku/crates/cli/src/puzzle.rs) — the parser module.
* [`gomoku/crates/cli/tests/puzzle.rs`](../../../../../gomoku/crates/cli/tests/puzzle.rs) — the parser tests.
