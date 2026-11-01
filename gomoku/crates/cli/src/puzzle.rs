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
use std::collections::HashSet;

/// One puzzle position, parsed and validated
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puzzle {
    /// zero-based index within the pack
    pub index: usize,
    /// Side to move (the attacker in VCF terminology)
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
    /// # Errors
    /// Returns `PuzzleError::IndexOutOfRange` if `index` is not in the pack.
    pub fn get(&self, index: usize) -> Result<&Puzzle, PuzzleError> {
        self.items.get(index).ok_or(PuzzleError::IndexOutOfRange {
            index,
            count: self.items.len(),
        })
    }
}

/// Everything that can go wrong when parsing a puzzle pack
#[derive(Debug, thiserror::Error)]
pub enum PuzzleError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Count mismatch. Declared {declared}, actual {actual}.")]
    CountMismatch { declared: usize, actual: usize },

    #[error("Puzzle {item} Invalid Attacker: '{value}'")]
    InvalidAttacker { item: u32, value: String },

    /// A coordinate is outside of the 15x15 board.
    #[error("Puzzle {item}: coordinate ({x}, {y}) is off the board")]
    InvalidCoord { item: u32, x: u8, y: u8 },

    #[error("Puzzle {item}: duplicate stone at ({x} {y})")]
    DuplicateStone { item: u32, x: u8, y: u8 },

    #[error("Puzzle {item}: black and white stones overlap")]
    OverlappingColors { item: u32 },

    #[error("puzzle {item}: invalid counts (black={black}, white={white}, attacker={attacker:?})")]
    InvalidCounts {
        item: u32,
        black: usize,
        white: usize,
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
        /// requested zero-based index.
        index: usize,
        /// Number of puzzles in the pack
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
    /// the file's own puzzle number; used in error messages.
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

    let mut black_seen = HashSet::new();
    for mv in &black {
        if !black_seen.insert(mv.index()) {
            return Err(PuzzleError::DuplicateStone {
                item: item.i,
                x: mv.col(),
                y: mv.row(),
            });
        }
    }
    let mut white_seen = HashSet::new();
    for mv in &white {
        if !white_seen.insert(mv.index()) {
            return Err(PuzzleError::DuplicateStone {
                item: item.i,
                x: mv.col(),
                y: mv.row(),
            });
        }
        if black_seen.contains(&mv.index()) {
            return Err(PuzzleError::OverlappingColors { item: item.i });
        }
    }

    let board = Board::from_position(&black, &white, attacker).map_err(|e| match e {
        PositionError::InvalidCounts => PuzzleError::InvalidCounts {
            item: item.i,
            black: black.len(),
            white: white.len(),
            attacker,
        },
        PositionError::OverlappingColors => PuzzleError::OverlappingColors { item: item.i },
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
