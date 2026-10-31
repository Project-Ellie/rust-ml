# Sub-chapter 07a — The pack parser: solution

This file is opt-in. Read it only if you have been stuck for more than
twenty minutes, or after you have finished the sub-chapter and want to
compare your work.

Verified green from `gomoku/`: `cargo test -p cli`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`. Quotes slice commit e6dc12b on reference/chapter-07-redesign.

---

The slice changes five files. `Cargo.toml` and `src/lib.rs` are small
mechanical changes. `src/puzzle.rs`, `tests/fixtures/mini-pack.json`,
and `tests/puzzle.rs` are new.

## `Cargo.toml` — `serde_json` moves to `[dependencies]`

The parser lives in the library, so `serde_json` must be available at
runtime. It stays in `[dev-dependencies]` for the existing tests.

`gomoku/crates/cli/Cargo.toml`, lines 24–32:

```toml
[dependencies]
engine = { path = "../engine" }
mcts = { path = "../mcts" }
thiserror.workspace = true
clap = { workspace = true, features = ["derive" ]}
rand = "0.10.3"
serde = { version = "1.0.229", features = ["derive"] }
serde_json = { workspace = true }
crossterm.workspace = true
```

## `src/lib.rs` — expose the puzzle module

`gomoku/crates/cli/src/lib.rs`, lines 1–10:

```rust
pub mod actor;
pub mod command;
pub mod fmt;
pub mod history;
pub mod notation;
pub mod opening;
pub mod puzzle;
pub mod repl;
pub mod session;
pub mod tui;
```

## `src/puzzle.rs` — the full parser module

This file is the whole slice. It declares the public `Puzzle` and
`PuzzlePack` types, the `PuzzleError` enum, the private serde schema,
and the validation pipeline.

The schema lists only the fields the table reads. `v`, `d`, and the
corpora's `u`/`f` keys are still present in the file format, but serde
ignores them because they are not declared. A field earns a place in the
schema when a consumer needs it.

`gomoku/crates/cli/src/puzzle.rs`, lines 1–289:

```rust
//! Puzzle pack loading — positions with a claimed solution line.
//!
//! Puzzles are not a game mode; a puzzle is a position plus the pack's
//! claimed forced-win line, loaded into the study table for analysis.
//! The pack format is the corpus format (`data/puzzles/README.md` in the
//! main repo): JSON `{v, count, items}` with items `{i, a, d, b, w, l}`
//! where `a` is the attacker (side to move), `b`/`w` the stone lists,
//! and `l` the claimed line as `[x, y, side]` triples.
//!
//! Coordinate convention (kept from the legacy puzzle viewer, which
//! rendered these packs correctly): file coordinates are 0-based
//! `[x, y]` with `x` the column and `y` the row — `Move::new(y, x)`.
//! Line side markers are `1` = Black, `2` = White.

use engine::{Board, Color, Move, PositionError};
use serde::Deserialize;

/// One puzzle position, parsed and validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puzzle {
    /// Zero-based index within the pack.
    pub index: usize,
    /// Side to move (the attacker in VCF terminology).
    pub attacker: Color,
    /// The position, ready to seed a variation tree root.
    pub board: Board,
    /// The pack's claimed forced-win line with the side for each move.
    /// Empty for unlabelled packs (positions only).
    pub solution: Vec<(Move, Color)>,
}

/// A parsed puzzle pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PuzzlePack {
    /// All puzzles in file order.
    pub items: Vec<Puzzle>,
}

impl PuzzlePack {
    /// Parse and validate a pack from its JSON text.
    ///
    /// Every item is validated: coordinates on the board, no duplicate or
    /// overlapping stones, stone counts consistent with the attacker, and
    /// the claimed line (if any) starting with the attacker.
    ///
    /// # Errors
    /// Returns a typed `PuzzleError` describing the first violation.
    pub fn from_json(text: &str) -> Result<Self, PuzzleError> {
        let file: PackFile = serde_json::from_str(text)?;

        if file.count != file.items.len() {
            return Err(PuzzleError::CountMismatch {
                declared: file.count,
                actual: file.items.len(),
            });
        }

        let items = file
            .items
            .into_iter()
            .enumerate()
            .map(|(pos, item)| parse_item(pos, item))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { items })
    }

    /// The puzzle at `index`.
    ///
    /// # Errors
    /// Returns `PuzzleError::IndexOutOfRange` if `index` is not in the pack.
    pub fn get(&self, index: usize) -> Result<&Puzzle, PuzzleError> {
        self.items.get(index).ok_or(PuzzleError::IndexOutOfRange {
            index,
            count: self.items.len(),
        })
    }
}

/// Everything that can go wrong when parsing a puzzle pack.
#[derive(Debug, thiserror::Error)]
pub enum PuzzleError {
    /// The text is not valid JSON for the pack format.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The top-level `count` field does not match `items.len()`.
    #[error("count mismatch: declared {declared}, actual {actual}")]
    CountMismatch {
        /// Value of the `count` field.
        declared: usize,
        /// Length of the `items` array.
        actual: usize,
    },

    /// The `a` (attacker) field is not "black" or "white".
    #[error("puzzle {item}: invalid attacker '{value}'")]
    InvalidAttacker {
        /// Puzzle index within the file (the item's `i` field).
        item: u32,
        /// Raw value from the file.
        value: String,
    },

    /// A coordinate is outside the 15×15 board.
    #[error("puzzle {item}: coordinate ({x}, {y}) is off the board")]
    InvalidCoord {
        /// Puzzle index within the file (the item's `i` field).
        item: u32,
        /// File x coordinate (column, 0-based).
        x: u8,
        /// File y coordinate (row, 0-based).
        y: u8,
    },

    /// The same cell appears twice in one color's stone list.
    #[error("puzzle {item}: duplicate stone at ({x}, {y})")]
    DuplicateStone {
        /// Puzzle index within the file (the item's `i` field).
        item: u32,
        /// File x coordinate (column) of the duplicated cell.
        x: u8,
        /// File y coordinate (row) of the duplicated cell.
        y: u8,
    },

    /// A cell appears in both Black's and White's stone lists.
    #[error("puzzle {item}: black and white stones overlap")]
    OverlappingColors {
        /// Puzzle index within the file (the item's `i` field).
        item: u32,
    },

    /// Stone counts violate the Gomoku turn invariant for the attacker.
    #[error("puzzle {item}: invalid counts (black={black}, white={white}, attacker={attacker:?})")]
    InvalidCounts {
        /// Puzzle index within the file (the item's `i` field).
        item: u32,
        /// Number of black stones.
        black: usize,
        /// Number of white stones.
        white: usize,
        /// Side to move according to the file.
        attacker: Color,
    },

    /// A line side marker is not 1 (Black) or 2 (White).
    #[error("puzzle {item}: invalid line side marker {value}")]
    InvalidSide {
        /// Puzzle index within the file (the item's `i` field).
        item: u32,
        /// Raw side value from the file.
        value: u8,
    },

    /// The first move of the claimed line is not by the attacker.
    #[error("puzzle {item}: line starts with side {found}, expected {expected:?}")]
    InvalidSolutionSide {
        /// Puzzle index within the file (the item's `i` field).
        item: u32,
        /// Expected side (the attacker).
        expected: Color,
        /// Side value found in the file.
        found: u8,
    },

    /// A requested puzzle index is not in the pack.
    #[error("puzzle index {index} out of range (pack holds {count})")]
    IndexOutOfRange {
        /// Requested zero-based index.
        index: usize,
        /// Number of puzzles in the pack.
        count: usize,
    },
}

// ------------------------------------------------------------------ pack file schema
//
// The schema lists only what the table reads. The pack format carries
// more (`v` format version, `d` claimed depth, the corpora's `u`/`f`),
// and serde ignores all of it: a field earns a place here when a
// consumer needs it, not before.

#[derive(Debug, Deserialize)]
struct PackFile {
    count: usize,
    items: Vec<PackItem>,
}

#[derive(Debug, Deserialize)]
struct PackItem {
    /// The file's own puzzle number; used in error messages.
    i: u32,
    a: String,
    #[serde(rename = "b")]
    black: Vec<[u8; 2]>,
    #[serde(rename = "w")]
    white: Vec<[u8; 2]>,
    /// Claimed forced-win line; absent in unlabelled packs.
    #[serde(rename = "l", default)]
    line: Vec<[u8; 3]>,
}

fn parse_item(pos: usize, item: PackItem) -> Result<Puzzle, PuzzleError> {
    let attacker = match item.a.as_str() {
        "black" => Color::Black,
        "white" => Color::White,
        other => {
            return Err(PuzzleError::InvalidAttacker {
                item: item.i,
                value: other.to_string(),
            });
        }
    };

    let black = parse_stones(item.i, &item.black)?;
    let white = parse_stones(item.i, &item.white)?;

    // No duplicates within a color and no overlap between colors. The
    // bitboard builder would silently collapse duplicates, so check here.
    let mut seen = std::collections::HashSet::new();
    for mv in &black {
        if !seen.insert(mv.index()) {
            return Err(PuzzleError::DuplicateStone {
                item: item.i,
                x: mv.col(),
                y: mv.row(),
            });
        }
    }
    for mv in &white {
        if seen.contains(&mv.index()) {
            return Err(PuzzleError::OverlappingColors { item: item.i });
        }
        if !seen.insert(mv.index()) {
            return Err(PuzzleError::DuplicateStone {
                item: item.i,
                x: mv.col(),
                y: mv.row(),
            });
        }
    }

    let board = Board::from_position(&black, &white, attacker).map_err(|e| match e {
        PositionError::OverlappingColors => PuzzleError::OverlappingColors { item: item.i },
        PositionError::InvalidCounts => PuzzleError::InvalidCounts {
            item: item.i,
            black: black.len(),
            white: white.len(),
            attacker,
        },
    })?;

    let mut solution = Vec::with_capacity(item.line.len());
    for (ply, &[x, y, side]) in item.line.iter().enumerate() {
        let color = match side {
            1 => Color::Black,
            2 => Color::White,
            other => {
                return Err(PuzzleError::InvalidSide {
                    item: item.i,
                    value: other,
                });
            }
        };
        if ply == 0 && color != attacker {
            return Err(PuzzleError::InvalidSolutionSide {
                item: item.i,
                expected: attacker,
                found: side,
            });
        }
        let mv = Move::new(y, x).ok_or(PuzzleError::InvalidCoord { item: item.i, x, y })?;
        solution.push((mv, color));
    }

    Ok(Puzzle {
        index: pos,
        attacker,
        board,
        solution,
    })
}

fn parse_stones(item: u32, coords: &[[u8; 2]]) -> Result<Vec<Move>, PuzzleError> {
    coords
        .iter()
        .map(|&[x, y]| Move::new(y, x).ok_or(PuzzleError::InvalidCoord { item, x, y }))
        .collect()
}
```

## `tests/fixtures/mini-pack.json` — the committed fixture

The fixture holds three puzzles. The first two are simple one-move wins;
the third has a three-ply line. The extra `u` and `f` fields prove the
schema ignores unknown keys.

`gomoku/crates/cli/tests/fixtures/mini-pack.json`:

```json
{
  "v": 1,
  "count": 3,
  "items": [
    {
      "i": 0,
      "a": "black",
      "d": 1,
      "u": 0,
      "f": 0,
      "b": [[7, 3], [7, 4], [7, 5], [7, 6]],
      "w": [[0, 0], [0, 1], [0, 2], [7, 2]],
      "l": [[7, 7, 1]]
    },
    {
      "i": 1,
      "a": "white",
      "d": 1,
      "u": 0,
      "f": 0,
      "b": [[0, 0], [1, 0], [2, 0], [3, 0], [14, 14]],
      "w": [[4, 4], [5, 4], [6, 4], [7, 4]],
      "l": [[8, 4, 2]]
    },
    {
      "i": 2,
      "a": "black",
      "d": 3,
      "u": 0,
      "f": 0,
      "b": [[7, 7], [8, 7], [9, 7]],
      "w": [[5, 5], [6, 5], [7, 5]],
      "l": [[10, 7, 1], [6, 7, 2], [11, 7, 1]]
    }
  ]
}
```

## `tests/puzzle.rs` — the parser tests

The test file loads the fixture with `include_str!` and uses inline JSON
for the error cases. It never reads `data/`.

`gomoku/crates/cli/tests/puzzle.rs`, lines 1–215:

```I rust
//! Parser tests for puzzle packs (spec amendment "positions from files").
//!
//! The valid-pack tests load the committed mini fixture; malformed inputs
//! are inline JSON strings. Tests never touch the git-ignored corpora
//! under `data/`.

use cli::puzzle::{PuzzleError, PuzzlePack};
use engine::{Color, Move};

const MINI_PACK: &str = include_str!("fixtures/mini-pack.json");

fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}

#[test]
fn parses_valid_pack() {
    let pack = PuzzlePack::from_json(MINI_PACK).expect("mini fixture parses");
    assert_eq!(pack.items.len(), 3);

    let first = &pack.items[0];
    assert_eq!(first.index, 0);
    assert_eq!(first.attacker, Color::Black);
    assert_eq!(first.board.to_move(), Color::Black);
    assert_eq!(first.solution, vec![(mv(7, 7), Color::Black)]);

    let third = &pack.items[2];
    assert_eq!(third.index, 2);
    assert_eq!(third.attacker, Color::Black);
    assert_eq!(
        third.solution,
        vec![
            (mv(7, 10), Color::Black),
            (mv(7, 6), Color::White),
            (mv(7, 11), Color::Black),
        ]
    );
}

#[test]
fn file_coordinates_are_x_column_y_row() {
    // Legacy viewer convention: file [x, y] maps to Move::new(y, x) —
    // x is the column, y is the row.
    let pack = PuzzlePack::from_json(MINI_PACK).unwrap();
    let first = &pack.items[0];
    // File stone [7, 3]: column 7 ("h"), row 3.
    assert_eq!(first.board.stone_at(mv(3, 7)), Some(Color::Black));
    assert_eq!(first.board.stone_at(mv(7, 3)), None);
    // File line head [7, 7] renders as algebraic h8.
    assert_eq!(cli::fmt::format_move(first.solution[0].0), "h8");
}

#[test]
fn board_is_replayable_position_with_attacker_to_move() {
    let pack = PuzzlePack::from_json(MINI_PACK).unwrap();
    let white_puzzle = &pack.items[1];
    assert_eq!(white_puzzle.attacker, Color::White);
    assert_eq!(white_puzzle.board.to_move(), Color::White);
    assert_eq!(white_puzzle.board.moves().len(), 0);
}

#[test]
fn malformed_json_is_rejected() {
    let err = PuzzlePack::from_json("{ not json").unwrap_err();
    assert!(matches!(err, PuzzleError::Json(_)));
}

#[test]
fn count_mismatch_is_rejected() {
    let text = r#"{ "v": 1, "count": 2, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::CountMismatch {
            declared: 2,
            actual: 1
        }
    ));
}

#[test]
fn off_board_coordinate_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [[15, 3]], "w": [], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::InvalidCoord {
            item: 0,
            x: 15,
            y: 3
        }
    ));
}

#[test]
fn off_board_line_coordinate_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [], "l": [[7, 15, 1]] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::InvalidCoord {
            item: 0,
            x: 7,
            y: 15
        }
    ));
}

#[test]
fn duplicate_stone_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [[7, 3], [7, 3]], "w": [], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::DuplicateStone {
            item: 0,
            x: 7,
            y: 3
        }
    ));
}

#[test]
fn overlapping_colors_are_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [[7, 3]], "w": [[7, 3]], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(err, PuzzleError::OverlappingColors { item: 0 }));
}

#[test]
fn counts_disagreeing_with_attacker_are_rejected() {
    // Black to move requires equal counts; here Black has one more.
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [[7, 3], [8, 3]], "w": [[0, 0]], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::InvalidCounts {
            item: 0,
            black: 2,
            white: 1,
            attacker: Color::Black,
        }
    ));
}

#[test]
fn unknown_attacker_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "red", "d": 1, "b": [], "w": [], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(err, PuzzleError::InvalidAttacker { item: 0, .. }));
}

#[test]
fn line_starting_with_the_wrong_side_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [], "l": [[7, 7, 2]] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::InvalidSolutionSide {
            item: 0,
            expected: Color::Black,
            found: 2,
        }
    ));
}

#[test]
fn unknown_line_side_marker_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [], "l": [[7, 7, 1], [8, 7, 9]] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::InvalidSide { item: 0, value: 9 }
    ));
}

#[test]
fn missing_line_defaults_to_empty_solution() {
    // Unlabelled packs (positions only) carry no `l` field.
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [] }
    ] }"#;
    let pack = PuzzlePack::from_json(text).unwrap();
    assert!(pack.items[0].solution.is_empty());
}

#[test]
fn index_out_of_range_is_rejected() {
    let pack = PuzzlePack::from_json(MINI_PACK).unwrap();
    assert!(pack.get(0).is_ok());
    assert!(pack.get(2).is_ok());
    let err = pack.get(3).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::IndexOutOfRange { index: 3, count: 3 }
    ));
}
```
