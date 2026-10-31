# Sub-chapter 07a — The pack parser: implementation plan

This document turns the walkthrough in `pack-parser.md` into a
construction order. It names the files, structs, enums, functions, and
tests. It states behavioral contracts in prose. It does not contain
bodies.

Read `pack-parser.md` first. Work one step at a time. Write tests before
code. Open `pack-parser-solution.md` only if you are stuck for more than
twenty minutes, or after the sub-chapter is complete.

---

## Step 0 — `Cargo.toml`: move `serde_json` to `[dependencies]`

The parser lives in the library, so `serde_json` must be available at
runtime. It is currently only in `[dev-dependencies]`. Add it under
`[dependencies]` using the workspace version.

```toml
serde_json = { workspace = true }
```

Keep the existing `serde` and `thiserror` entries unchanged.

---

## Step 1 — `lib.rs`: expose the puzzle module

### 1.1 Add the module declaration

In `gomoku/crates/cli/src/lib.rs`, add `pub mod puzzle;` among the other
module declarations.

---

## Step 2 — `src/puzzle.rs`: the parser module

Create `gomoku/crates/cli/src/puzzle.rs`.

### 2.1 Module documentation

Write a module-level doc comment that explains the purpose: puzzle pack
loading, the `[x, y]` coordinate convention, and the line side markers.

### 2.2 Imports

Bring in:

* `engine::{Board, Color, Move, PositionError}`
* `serde::Deserialize`

### 2.3 `Puzzle`

Write the public struct:

```rust
pub struct Puzzle {
    pub index: usize,
    pub attacker: Color,
    pub board: Board,
    pub solution: Vec<(Move, Color)>,
}
```

Behavior: a validated puzzle position with the attacker to move and the
pack's claimed line.

### 2.4 `PuzzlePack`

Write the public struct:

```rust
pub struct PuzzlePack {
    pub items: Vec<Puzzle>,
}
```

Behavior: the parsed contents of one pack file, in file order.

### 2.5 `PuzzlePack::from_json`

Signature:

```rust
pub fn from_json(text: &str) -> Result<Self, PuzzleError>
```

Behavior:
* Deserialize `text` into the private `PackFile` schema.
* Return `PuzzleError::Json` if deserialization fails.
* Compare `file.count` to `file.items.len()`. Return
  `PuzzleError::CountMismatch` if they differ.
* Parse each item with `parse_item(pos, item)`, collecting into a
  `Vec<Puzzle>`. Return the first error encountered.
* Return `PuzzlePack { items }`.

### 2.6 `PuzzlePack::get`

Signature:

```rust
pub fn get(&self, index: usize) -> Result<&Puzzle, PuzzleError>
```

Behavior:
* Return `Ok(&self.items[index])` when the index is in range.
* Return `PuzzleError::IndexOutOfRange { index, count }` otherwise.

### 2.7 `PuzzleError`

Write a public enum with a `#[derive(Debug, thiserror::Error)]` and a
`#[error("...")]` format string for each variant:

* `Json(#[from] serde_json::Error)`
* `CountMismatch { declared: usize, actual: usize }`
* `InvalidAttacker { item: u32, value: String }`
* `InvalidCoord { item: u32, x: u8, y: u8 }`
* `DuplicateStone { item: u32, x: u8, y: u8 }`
* `OverlappingColors { item: u32 }`
* `InvalidCounts { item: u32, black: usize, white: usize, attacker: Color }`
* `InvalidSide { item: u32, value: u8 }`
* `InvalidSolutionSide { item: u32, expected: Color, found: u8 }`
* `IndexOutOfRange { index: usize, count: usize }`

### 2.8 Private schema `PackFile`

Write:

```rust
struct PackFile {
    #[allow(dead_code)]
    v: u32,
    count: usize,
    items: Vec<PackItem>,
}
```

Derive `Debug` and `Deserialize`.

### 2.9 Private schema `PackItem`

Write:

```rust
struct PackItem {
    i: u32,
    a: String,
    #[allow(dead_code)]
    d: Option<u32>,
    #[serde(rename = "b")]
    black: Vec<[u8; 2]>,
    #[serde(rename = "w")]
    white: Vec<[u8; 2]>,
    #[serde(rename = "l", default)]
    line: Vec<[u8; 3]>,
}
```

Behavior: unknown keys are ignored. The `l` field defaults to empty when
absent.

### 2.10 `parse_item`

Signature:

```rust
fn parse_item(pos: usize, item: PackItem) -> Result<Puzzle, PuzzleError>
```

Behavior:
* Map `item.a` to `Color::Black` for `"black"`, `Color::White` for
  `"white"`, otherwise `InvalidAttacker`.
* Parse Black and White stone lists with `parse_stones`.
* Check for duplicate stones within each color and overlapping stones
  between colors. Return the matching `DuplicateStone` or
  `OverlappingColors` error.
* Build the board with `Board::from_position(&black, &white, attacker)`.
  Map `PositionError::OverlappingColors` to `OverlappingColors` and
  `PositionError::InvalidCounts` to `InvalidCounts`.
* Walk the line. For each `[x, y, side]`:
  * Map `side` to a `Color`. Unknown markers return `InvalidSide`.
  * On the first line move, require the side to equal the attacker;
    otherwise return `InvalidSolutionSide`.
  * Convert `[x, y]` to `Move::new(y, x)`. An off-board value returns
    `InvalidCoord`.
  * Push `(mv, color)` into the solution vector.
* Return `Puzzle { index: pos, attacker, board, solution }`.

### 2.11 `parse_stones`

Signature:

```rust
fn parse_stones(item: u32, coords: &[[u8; 2]]) -> Result<Vec<Move>, PuzzleError>
```

Behavior:
* Map each `[x, y]` to `Move::new(y, x)`.
* Return `InvalidCoord { item, x, y }` when `Move::new` returns `None`.

### 2.12 Gate

Run `cargo test -p cli --test puzzle`. Expect failures.

---

## Step 3 — `tests/fixtures/mini-pack.json`

Create `gomoku/crates/cli/tests/fixtures/mini-pack.json`.

Content:
* Three puzzles.
* The first puzzle: Black attacker, a vertical four on column h, one
  Black stone at `h8` in the solution line.
* The second puzzle: White attacker, a horizontal four on row 5, one
  White stone at `i5` in the solution line.
* The third puzzle: Black attacker, a three-ply line.
* Each item includes the extra `u` and `f` keys.
* The top-level `count` is `3`.

---

## Step 4 — `tests/puzzle.rs`: the parser tests

Create `gomoku/crates/cli/tests/puzzle.rs`.

### 4.1 Module doc and imports

Write a short module doc that states the fixture discipline. Import
`cli::puzzle::{PuzzleError, PuzzlePack}` and `engine::{Color, Move}`.

Load the fixture:

```rust
const MINI_PACK: &str = include_str!("fixtures/mini-pack.json");
```

Add a helper `mv(r, c) -> Move`.

### 4.2 Group A — happy path

Write these tests:

* `parses_valid_pack`
* `file_coordinates_are_x_column_y_row`
* `board_is_replayable_position_with_attacker_to_move`

### 4.3 Group B — validation errors

Write these tests, using inline JSON strings:

* `malformed_json_is_rejected`
* `count_mismatch_is_rejected`
* `off_board_coordinate_is_rejected`
* `off_board_line_coordinate_is_rejected`
* `duplicate_stone_is_rejected`
* `overlapping_colors_are_rejected`
* `counts_disagreeing_with_attacker_are_rejected`
* `unknown_attacker_is_rejected`
* `line_starting_with_the_wrong_side_is_rejected`
* `unknown_line_side_marker_is_rejected`

### 4.4 Group C — optional data and lookup

Write these tests:

* `missing_line_defaults_to_empty_solution`
* `index_out_of_range_is_rejected`

### 4.5 Gate

Run `cargo test -p cli --test puzzle` until all 15 tests pass.

---

## Step 5 — Project gates

From `gomoku/`:

```text
cargo test -p cli --test puzzle
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

---

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `Cargo.toml` | `serde_json` in `[dependencies]` | library can deserialize JSON at runtime |
| `src/lib.rs` | `pub mod puzzle;` | module is public |
| `src/puzzle.rs` | `Puzzle` | public struct with the four fields |
| `src/puzzle.rs` | `PuzzlePack` | public struct with `items` |
| `src/puzzle.rs` | `PuzzlePack::from_json` | parses and validates, returns first error |
| `src/puzzle.rs` | `PuzzlePack::get` | returns `IndexOutOfRange` on bad index |
| `src/puzzle.rs` | `PuzzleError` | all ten variants with `thiserror` messages |
| `src/puzzle.rs` | `PackFile` / `PackItem` | private serde schema ignores unknown fields |
| `src/puzzle.rs` | `parse_item` | full validation pipeline in execution order |
| `src/puzzle.rs` | `parse_stones` | maps `[x, y]` to `Move::new(y, x)` |
| `tests/fixtures/mini-pack.json` | three puzzles | covers Black/White attackers and a line |
| `tests/puzzle.rs` | 15 tests | every test from the checklist passes |
