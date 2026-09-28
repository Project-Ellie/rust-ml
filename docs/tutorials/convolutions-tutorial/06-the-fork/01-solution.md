# Chapter 06 — opt-in solution: `src/oracle.rs`

Open this only if you have been stuck for more than twenty minutes,
or after you have finished the chapter and want to compare your work
against the verified reference. The implementation below is the
complete `src/oracle.rs` from the reference crate, quoted verbatim.

The point of the oracle is not cleverness — it is **obvious
correctness**. Read it slowly; every line should map directly onto
the spec §3.4 definition.

````rust
//! Naive reference oracle for open-three threat detection.
//!
//! This module is intentionally slow and obviously correct. It
//! implements the v1 open-three definition from the design spec,
//! §3.4, by plain array scans — no bitboards, no convolution. It
//! exists only to serve as ground truth for the differential tests
//! that verify the hand-written conv kernels.
//!
//! The three exact window patterns (reading along a line, `X` = own
//! stone, `_` = empty in-board cell) are:
//!
//! * `_XXX_`   — five cells, three consecutive own stones,
//! * `_X_XX_`  — six cells, a gap before the last stone,
//! * `_XX_X_`  — six cells, a gap after the first stone.
//!
//! In every case the window's `_` cells must be genuinely empty
//! in-board cells, and the cells immediately beyond the two window
//! ends must not hold an own stone (the anti-four margin). A cell
//! off the board satisfies the margin automatically.

use engine::{Board, Color, Move, MoveSet};

/// The four line directions used by the oracle.
///
/// The discriminant values match the index order of
/// [`open_three_makers`]: horizontal, vertical, diagonal down-right,
/// diagonal up-right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum Direction {
    /// Left-to-right.
    Horizontal = 0,
    /// Top-to-bottom.
    Vertical = 1,
    /// Top-left to bottom-right.
    DiagDown = 2,
    /// Bottom-left to top-right.
    DiagUp = 3,
}

impl Direction {
    /// All four directions, in the canonical order.
    pub const ALL: [Direction; 4] = [
        Direction::Horizontal,
        Direction::Vertical,
        Direction::DiagDown,
        Direction::DiagUp,
    ];

    /// Step vector `(drow, dcol)` for walking along this direction.
    pub fn step(self) -> (i32, i32) {
        match self {
            Direction::Horizontal => (0, 1),
            Direction::Vertical => (1, 0),
            Direction::DiagDown => (1, 1),
            Direction::DiagUp => (-1, 1),
        }
    }
}

/// Returns the set of empty cells that create an open three in each
/// of the four directions.
///
/// The returned array is indexed by [`Direction`] discriminant:
/// `[horizontal, vertical, diag_down, diag_up]`.
pub fn open_three_makers(b: &Board, s: Color) -> [MoveSet; 4] {
    let grid = grid_from_board(b);
    let mut out = [MoveSet::EMPTY; 4];

    for r in 0..15u8 {
        for c in 0..15u8 {
            let mv = Move::new(r, c).expect("in-board coordinates");
            if b.stone_at(mv).is_some() {
                continue;
            }

            for dir in Direction::ALL {
                if open_three_at(&grid, r, c, s, dir.step()) {
                    out[dir as usize].insert(mv);
                }
            }
        }
    }

    out
}

/// Returns the set of empty cells where placing a stone of side `s`
/// creates open threes in two or more distinct directions.
pub fn double_threes(b: &Board, s: Color) -> MoveSet {
    let per_dir = open_three_makers(b, s);
    let mut out = MoveSet::EMPTY;

    for r in 0..15u8 {
        for c in 0..15u8 {
            let mv = Move::new(r, c).expect("in-board coordinates");
            let count = per_dir.iter().filter(|set| set.contains(mv)).count();
            if count >= 2 {
                out.insert(mv);
            }
        }
    }

    out
}

/// Snapshot of the board as a 15×15 grid of optional stone colors.
fn grid_from_board(b: &Board) -> [[Option<Color>; 15]; 15] {
    let mut grid = [[None; 15]; 15];
    for r in 0..15u8 {
        for c in 0..15u8 {
            let mv = Move::new(r, c).expect("in-board coordinates");
            grid[r as usize][c as usize] = b.stone_at(mv);
        }
    }
    grid
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CellState {
    /// Empty in-board cell.
    Empty,
    /// Occupied by the side we are evaluating for.
    Own,
    /// Occupied by the opponent or outside the board.
    Blocked,
}

/// True iff placing an `s`-stone at `(r, c)` creates an open three
/// along `(dr, dc)` according to the spec §3.4 definition.
fn open_three_at(
    grid: &[[Option<Color>; 15]; 15],
    r: u8,
    c: u8,
    s: Color,
    (dr, dc): (i32, i32),
) -> bool {
    // The three v1 window patterns. `true` = own stone (`X`),
    // `false` = required empty in-board cell (`_`).
    const PATTERNS: &[&[bool]] = &[
        &[false, true, true, true, false],        // _XXX_
        &[false, true, false, true, true, false], // _X_XX_
        &[false, true, true, false, true, false], // _XX_X_
    ];

    let cell = |k: i32| -> CellState {
        // The grid content at (r, c) is deliberately ignored: the
        // hypothetical stone occupies it; callers only query empty cells.
        let rr = r as i32 + dr * k;
        let cc = c as i32 + dc * k;
        if !(0..15).contains(&rr) || !(0..15).contains(&cc) {
            CellState::Blocked
        } else if rr == r as i32 && cc == c as i32 {
            CellState::Own // the hypothetical stone
        } else {
            match grid[rr as usize][cc as usize] {
                Some(color) if color == s => CellState::Own,
                None => CellState::Empty,
                Some(_) => CellState::Blocked,
            }
        }
    };

    for pattern in PATTERNS {
        let len = pattern.len() as i32;
        // Window start offsets that put the candidate cell (offset 0)
        // somewhere inside the window.
        for start in -(len - 1)..=0 {
            let mut contains_c = false;
            let mut window_ok = true;

            for (i, &needs_own) in pattern.iter().enumerate() {
                let k = start + i as i32;
                match (cell(k), needs_own) {
                    (CellState::Own, true) => {
                        if k == 0 {
                            contains_c = true;
                        }
                    }
                    (CellState::Empty, false) => {}
                    _ => {
                        window_ok = false;
                        break;
                    }
                }
            }

            if !window_ok || !contains_c {
                continue;
            }

            // Anti-four margin: the cells just beyond the two window
            // ends must not be own stones.
            let left_ok = !matches!(cell(start - 1), CellState::Own);
            let right_ok = !matches!(cell(start + len), CellState::Own);

            if left_ok && right_ok {
                return true;
            }
        }
    }

    false
}
````
