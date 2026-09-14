# Slice 2 — Solutions: the reference engine

This is the full, commented solution set for
[Slice 2](02-the-reference-engine.md), walking the TDD path cycle by
cycle. Use it the way you promised:

1. Read a cycle's **RED** test. Make sure you can say out loud *why*
   this behavior belongs in the contract.
2. Type the test yourself. Run it. Watch it fail for the right reason.
3. Read the **GREEN** code line by line. If any line is unclear, stop
   there — that line is the lesson.
4. Type the code yourself. Run. Watch it go green. Commit mentally,
   move on.

Do not copy-paste. Typing is where the learning happens; this document
is the map, not the car.

The complete final state of both files is at the end — use it to diff
against your typed version.

---

## Step 0 — `Move` in `moveset.rs`

Everything else needs `Move`, so it gets the first cycle.

### RED

```rust
// at the bottom of moveset.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_off_board_cells() {
        assert_eq!(Move::new(15, 0), None);  // row one past the edge
        assert_eq!(Move::new(0, 15), None);  // col one past the edge
        assert_eq!(Move::new(14, 14), Some(Move(224))); // last legal cell
    }

    #[test]
    fn index_row_col_are_consistent() {
        let mv = Move::new(7, 3).unwrap();
        assert_eq!(mv.index(), 7 * 15 + 3); // 108
        assert_eq!(mv.row(), 7);
        assert_eq!(mv.col(), 3);
    }
}
```

Fails to compile — `Move` does not exist yet. That counts as RED for a
new type: the compiler is the test runner's messenger.

### GREEN

```rust
//! `MoveSet`: public set-of-moves bitset, logical stride-15 indexing.
//! Slice 3. Deliberately separate from the internal stride-16 `Bitboard`.

/// A board cell: logical index `row * 15 + col`, 0..=224.
///
/// Newtype over `u8`. The only way in from outside is `new`, which
/// validates — so a `Move` value in the wild is always on-board.
/// "Make invalid states unrepresentable" at the smallest possible scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Move(u8);

impl Move {
    /// Validated constructor. `None` if `row` or `col` is off the board.
    pub fn new(row: u8, col: u8) -> Option<Move> {
        if row < 15 && col < 15 {
            Some(Move(row * 15 + col))
        } else {
            None
        }
    }

    pub fn row(self) -> u8 {
        self.0 / 15
    }

    pub fn col(self) -> u8 {
        self.0 % 15
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }
}
```

Things to notice:

- `PartialEq` on `Move` is what lets the first test compare
  `Some(Move(224))`. `Debug` is what lets `assert_eq!` print failures.
- `row`/`col`/`index` take `self` by value — allowed and idiomatic
  because `Move` is `Copy`. No borrows, no lifetimes.
- The tutorial mentions a `pub(crate) fn from_index_unchecked` for
  trusted paths. We deliberately do **not** write it yet — TDD rule: no
  code without a caller. Slice 3 will need it; slice 3 will add it.

---

## Cycle 1 — a new board is empty, `Ongoing`, Black to move

### RED

```rust
// reference.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::moveset::Move;

    #[test]
    fn new_board_is_empty_ongoing_black_to_move() {
        let b = Board::new();
        assert_eq!(b.status(), Status::Ongoing);
        assert_eq!(b.to_move(), Color::Black); // Black always opens Gomoku
        assert_eq!(b.stone_at(Move::new(7, 7).unwrap()), None);
        assert!(b.moves().is_empty());
    }
}
```

### GREEN

The contract fixes the data model up front (the slice text lists the
fields), so the struct is designed once; only *behavior* grows from here:

```rust
//! Naive array-based reference engine — the differential-testing oracle.
//! Slice 2. Compiled only for tests and the `testutil` feature.
//!
//! Optimized for *obvious correctness*, not speed: readable loops,
//! bounds checks, no bit tricks. Every clever optimization in the fast
//! engine must agree with this code (ch. 13, "Reference oracle").

use crate::moveset::Move;

/// Stone color. Absolute (Black/White), not relative (me/you) —
/// ch. 13, decision 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black,
    White,
}

impl Color {
    pub fn other(self) -> Color {
        match self {
            Color::Black => Color::White,
            Color::White => Color::Black,
        }
    }
}

/// One board cell. Private — the outside world sees `Option<Color>`
/// through `stone_at`; the `Cell` representation is our business.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cell {
    Empty,
    Stone(Color),
}

/// Game state. An enum, not bool flags (`won: bool, drawn: bool`) —
/// with flags, `won && drawn` is representable nonsense; here it is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ongoing,
    Won(Color),
    Draw,
}

/// The naive board. `[[Cell; 15]; 15]` + full move history.
pub struct Board {
    cells: [[Cell; 15]; 15],
    to_move: Color,
    status: Status,
    moves: Vec<Move>,
}

impl Board {
    pub fn new() -> Board {
        Board {
            // Array-repeat syntax `[value; N]` works only because
            // `Cell: Copy` — every slot gets a copy of the same value.
            cells: [[Cell::Empty; 15]; 15],
            to_move: Color::Black,
            status: Status::Ongoing,
            moves: Vec::new(),
        }
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn to_move(&self) -> Color {
        self.to_move
    }

    /// `None` = empty cell. Translates our private `Cell` into the
    /// public `Option<Color>` vocabulary.
    pub fn stone_at(&self, mv: Move) -> Option<Color> {
        match self.cells[mv.row() as usize][mv.col() as usize] {
            Cell::Empty => None,
            Cell::Stone(color) => Some(color),
        }
    }

    /// Full history, in play order. Used by the draw rule (225 moves),
    /// encoding, and undo — the contract demands we keep it.
    pub fn moves(&self) -> &[Move] {
        &self.moves
    }
}

// Clippy's `new_without_default` lint (an error under `-D warnings`):
// a `new()` without args should come with `Default`.
impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}
```

---

## Cycle 2 — `play` places a stone and flips `to_move`

### RED

```rust
    #[test]
    fn play_places_stone_and_flips_to_move() {
        let mut b = Board::new();
        let mv = Move::new(7, 7).unwrap(); // tengen, the center
        b.play(mv).unwrap();
        assert_eq!(b.stone_at(mv), Some(Color::Black));
        assert_eq!(b.to_move(), Color::White);
    }
```

Fails to compile: no `play` method. RED.

### GREEN

```rust
    pub fn play(&mut self, mv: Move) -> Result<(), PlayError> {
        self.cells[mv.row() as usize][mv.col() as usize] = Cell::Stone(self.to_move);
        self.moves.push(mv);
        self.to_move = self.to_move.other();
        Ok(())
    }
```

with the error type born alongside — one variant for now:

```rust
/// Why a move can be rejected. `thiserror` derives `Display` + `Error`
/// from the `#[error(...)]` strings; we add `PartialEq` so tests can
/// compare the error value directly.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PlayError {
    #[error("cell is already occupied")]
    Occupied,
}
```

No win check, no legality — deliberately. Cycle 4's test will be a
*genuine* RED because of exactly this. Resist writing it early.

---

## Cycle 3 — `play` on an occupied cell is an error

### RED

```rust
    #[test]
    fn play_on_occupied_cell_is_rejected() {
        let mut b = Board::new();
        let mv = Move::new(5, 5).unwrap();
        b.play(mv).unwrap(); // Black

        // White tries the same cell — occupied by *either* color counts.
        assert_eq!(b.play(mv), Err(PlayError::Occupied));

        // A rejected move must not mutate anything: stone and turn stay.
        assert_eq!(b.stone_at(mv), Some(Color::Black));
        assert_eq!(b.to_move(), Color::White);
        assert_eq!(b.moves().len(), 1);
    }
```

Fails: the current `play` happily overwrites the stone and returns `Ok`.

### GREEN

```rust
    pub fn play(&mut self, mv: Move) -> Result<(), PlayError> {
        // Guards first, mutation second — that ordering is what makes
        // "a rejected move changes nothing" true by construction.
        if self.stone_at(mv).is_some() {
            return Err(PlayError::Occupied);
        }
        self.cells[mv.row() as usize][mv.col() as usize] = Cell::Stone(self.to_move);
        self.moves.push(mv);
        self.to_move = self.to_move.other();
        Ok(())
    }
```

---

## Cycle 4 — horizontal five wins

### RED

Two tests, one per color (the contract says "test White too" — a
first-player-only test suite is how asymmetric bugs ship):

```rust
    #[test]
    fn horizontal_five_wins_for_black() {
        let mut b = Board::new();
        // Black builds a row on rank 7; White parks junk far away,
        // in non-consecutive cells so White can never form a line.
        let script = [
            (7, 3), (0, 0),
            (7, 4), (0, 2),
            (7, 5), (0, 4),
            (7, 6), (0, 6),
            (7, 7), // the fifth stone — game ends here
        ];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(b.status(), Status::Won(Color::Black));
    }

    #[test]
    fn horizontal_five_wins_for_white() {
        let mut b = Board::new();
        // White wins, so White makes the last move: the script ends
        // on a White stone. Black's junk sits on rank 12 with gaps.
        let script = [
            (12, 0), (4, 4),
            (12, 2), (4, 5),
            (12, 4), (4, 6),
            (12, 6), (4, 7),
            (12, 8), (4, 8),
        ];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(b.status(), Status::Won(Color::White));
    }
```

Both fail — `status` stays `Ongoing` forever. True RED.

### GREEN

The win scanner. Write it in the general four-direction shape right
away — for a *reader-verifiable oracle*, the general walker is simpler
than four special cases, because it has one obvious idea: *count both
ways from the new stone*.

```rust
/// The four axes as (drow, dcol) step vectors. Each axis is walked in
/// both directions, so four entries cover all eight rays.
const DIRECTIONS: [(i32, i32); 4] = [
    (0, 1),  // horizontal
    (1, 0),  // vertical
    (1, 1),  // diagonal ↘
    (1, -1), // diagonal ↙
];

impl Board {
    /// Does the stone just placed at (r, c) complete five-or-more?
    /// Only the last move can create a win, so checking lines through
    /// the new stone is sufficient — no full-board scan needed.
    fn wins_from(&self, r: usize, c: usize, color: Color) -> bool {
        DIRECTIONS.iter().any(|&(dr, dc)| {
            // 1 = the new stone itself, plus the consecutive same-color
            // run on each side of it along this axis.
            1 + self.count_dir(r, c, dr, dc, color)
              + self.count_dir(r, c, -dr, -dc, color)
                >= 5
        })
    }

    /// Consecutive `color` stones starting one step (dr, dc) away from
    /// (r, c) and walking off in that direction. Stops at the board
    /// edge or the first cell that is not `color`.
    fn count_dir(&self, r: usize, c: usize, dr: i32, dc: i32, color: Color) -> usize {
        let mut n = 0;
        // Signed arithmetic so "one step before column 0" is -1, not
        // a panic or a wrap to 255 — the range check then ends the walk.
        let mut nr = r as i32 + dr;
        let mut nc = c as i32 + dc;
        while (0..15).contains(&nr)
            && (0..15).contains(&nc)
            && self.cells[nr as usize][nc as usize] == Cell::Stone(color)
        {
            n += 1;
            nr += dr;
            nc += dc;
        }
        n
    }
}
```

And one new line in `play`, after the push:

```rust
        self.moves.push(mv);
        if self.wins_from(mv.row() as usize, mv.col() as usize, self.to_move) {
            self.status = Status::Won(self.to_move);
        }
        self.to_move = self.to_move.other();
```

Green. Note what just happened: `wins_from` handles **all four axes**,
so the next two cycles add no production code at all.

---

## Cycles 5 and 6 — vertical, both diagonals: tests that pass immediately

### RED → GREEN (instantly)

```rust
    #[test]
    fn vertical_five_wins() {
        let mut b = Board::new();
        let script = [
            (2, 5), (0, 0),
            (3, 5), (0, 2),
            (4, 5), (0, 4),
            (5, 5), (0, 6),
            (6, 5),
        ];
        for (r, c) in script { b.play(Move::new(r, c).unwrap()).unwrap(); }
        assert_eq!(b.status(), Status::Won(Color::Black));
    }

    #[test]
    fn diagonal_down_right_five_wins() {
        let mut b = Board::new();
        let script = [
            (2, 2), (0, 0),
            (3, 3), (0, 2),
            (4, 4), (0, 4),
            (5, 5), (0, 6),
            (6, 6),
        ];
        for (r, c) in script { b.play(Move::new(r, c).unwrap()).unwrap(); }
        assert_eq!(b.status(), Status::Won(Color::Black));
    }

    #[test]
    fn diagonal_down_left_five_wins() {
        let mut b = Board::new();
        let script = [
            (2, 8), (0, 0),
            (3, 7), (0, 2),
            (4, 6), (0, 4),
            (5, 5), (0, 6),
            (6, 4),
        ];
        for (r, c) in script { b.play(Move::new(r, c).unwrap()).unwrap(); }
        assert_eq!(b.status(), Status::Won(Color::Black));
    }
```

They pass without touching `reference.rs`. This is normal and valuable:
when one general implementation covers several behaviors, the extra
tests are **characterization tests** — they pin the behavior down so a
later "optimization" (hello, slice 3's bitboards) can't silently break
one axis while keeping the others. Write them anyway. Always.

---

## Cycle 7 — six in a row still wins (overline)

First, understand *how* an overline can even happen. If we check for a
win after every placement, a player with four in a row who adds a fifth
stone ends the game immediately — a sixth can never be appended later.
The only way six arise at once is a **bridge**: two shorter runs joined
by the new stone.

```
before:  ●●·●●●   (runs of 2 and 3 — no win anywhere)
play:        ▲
after:   ●●●●●●   six at once — overline
```

### RED

```rust
    #[test]
    fn overline_six_counts_as_a_win() {
        let mut b = Board::new();
        // Black builds the two arms (7,3)-(7,4) and (7,6)-(7,8),
        // then bridges them with (7,5). White junks on rank 0, gapped.
        let script = [
            (7, 3), (0, 0),
            (7, 4), (0, 2),
            (7, 6), (0, 4),
            (7, 7), (0, 6),
            (7, 8), (0, 8),
            // Black's runs so far: length 2 and 3. No five yet!
            (7, 5), // the bridge — six in a row, all at once
        ];
        for (r, c) in script { b.play(Move::new(r, c).unwrap()).unwrap(); }
        // Freestyle rules: overlines count (ch. 13, decision 1).
        assert_eq!(b.status(), Status::Won(Color::Black));
    }
```

### GREEN

Nothing to write — `>= 5` in `wins_from` already says "five or more".
The test passes. Its job is to make the *rule choice* executable: if
anyone ever changes the check to `== 5`, this test screams.

---

## Cycle 8 — four in a row is not a win (the near-miss)

### RED → GREEN (instantly)

```rust
    #[test]
    fn four_in_a_row_is_still_ongoing() {
        let mut b = Board::new();
        let script = [
            (7, 3), (0, 0),
            (7, 4), (0, 2),
            (7, 5), (0, 4),
            (7, 6), (0, 6),
        ];
        for (r, c) in script { b.play(Move::new(r, c).unwrap()).unwrap(); }
        assert_eq!(b.status(), Status::Ongoing);
        assert_eq!(b.to_move(), Color::Black); // and the game continues
    }
```

Passes immediately. Near-miss tests are where off-by-one bugs
(`>= 4`?) come to die — never skip them.

---

## Cycle 9 — wins at the edge and in the corner

The oracle's `count_dir` walks off the board constantly at edges; these
tests prove the bounds checks work on every side. (For slice 3 this
corpus becomes *the* critical one: bit shifts wrap silently.)

### RED → GREEN (instantly)

```rust
    #[test]
    fn five_in_the_top_left_corner_wins() {
        let mut b = Board::new();
        // Black fills rank 0 starting at column 0: the win walk runs
        // into the left edge and must stop cleanly.
        let script = [
            (0, 0), (7, 7),
            (0, 1), (7, 9),
            (0, 2), (9, 7),
            (0, 3), (9, 9),
            (0, 4),
        ];
        for (r, c) in script { b.play(Move::new(r, c).unwrap()).unwrap(); }
        assert_eq!(b.status(), Status::Won(Color::Black));
    }

    #[test]
    fn five_on_the_bottom_edge_wins_for_white() {
        let mut b = Board::new();
        // White wins along rank 14, ending in the bottom-right corner.
        let script = [
            (0, 0), (14, 10),
            (0, 2), (14, 11),
            (0, 4), (14, 12),
            (0, 6), (14, 13),
            (0, 8), (14, 14),
        ];
        for (r, c) in script { b.play(Move::new(r, c).unwrap()).unwrap(); }
        assert_eq!(b.status(), Status::Won(Color::White));
    }
```

---

## Cycle 10 — no moves after a decided game

### RED

```rust
    #[test]
    fn play_after_a_won_game_is_rejected() {
        let mut b = Board::new();
        let script = [
            (7, 3), (0, 0), (7, 4), (0, 2), (7, 5), (0, 4), (7, 6), (0, 6), (7, 7),
        ];
        for (r, c) in script { b.play(Move::new(r, c).unwrap()).unwrap(); }
        assert_eq!(b.status(), Status::Won(Color::Black));

        // The board is full of empty cells — but the game is over.
        assert_eq!(b.play(Move::new(13, 13).unwrap()), Err(PlayError::GameOver));
    }
```

Fails: `PlayError::GameOver` doesn't exist, and `play` would happily
keep placing stones after the win.

### GREEN

Add the variant and the guard — *before* the occupied check, so a
finished game rejects everything uniformly:

```rust
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PlayError {
    #[error("cell is already occupied")]
    Occupied,
    #[error("the game is already over")]
    GameOver,
}
```

```rust
    pub fn play(&mut self, mv: Move) -> Result<(), PlayError> {
        if self.status != Status::Ongoing {
            return Err(PlayError::GameOver);
        }
        if self.stone_at(mv).is_some() {
            return Err(PlayError::Occupied);
        }
        // ... unchanged ...
    }
```

---

## Cycle 11 — 225 moves without a five is a draw

The interesting part is not the code, it is *constructing a full board
with no five-in-a-row*. Think it through:

- Row-major filling fails: on any ↘ diagonal the move index changes by
  16 per step (an even number), so same-parity — same color — stones
  line up. Five in a row, game over long before move 225.
- A perfect **checkerboard** works: if Black sits exactly on cells with
  `(r + c)` even and White on odd, then along *every* line in *every*
  direction the colors alternate — the longest run anywhere is 1.
- And here is the kicker: every intermediate position is a *subset* of
  the final board, and removing stones can never create a run. So if
  the final board has no five, no prefix of the game had one either.
  The fill order is safe by construction.
- Counts fit perfectly: 113 even cells for Black (moves 1, 3, …, 225),
  112 odd cells for White. Black makes the last move.

### RED

```rust
    #[test]
    fn full_board_without_five_is_a_draw() {
        let mut b = Board::new();

        // Split the 225 cells by checkerboard parity.
        let mut blacks = Vec::new(); // (r + c) even — 113 cells
        let mut whites = Vec::new(); // (r + c) odd  — 112 cells
        for r in 0..15u8 {
            for c in 0..15u8 {
                if (r + c) % 2 == 0 {
                    blacks.push(Move::new(r, c).unwrap());
                } else {
                    whites.push(Move::new(r, c).unwrap());
                }
            }
        }

        // Alternate strictly: Black, White, Black, White, …
        for i in 0..112 {
            b.play(blacks[i]).unwrap();
            b.play(whites[i]).unwrap();
        }
        assert_eq!(b.status(), Status::Ongoing); // 224 moves: still playing

        b.play(blacks[112]).unwrap(); // move 225 — board full
        assert_eq!(b.status(), Status::Draw);
        assert_eq!(b.moves().len(), 225);

        // A decided game rejects further moves — even though no cell
        // is free, GameOver (checked first) is the honest answer.
        assert_eq!(b.play(blacks[0]), Err(PlayError::GameOver));
    }
```

Fails: status stays `Ongoing` after move 225.

### GREEN

One branch in `play`, after the win check:

```rust
        if self.wins_from(mv.row() as usize, mv.col() as usize, self.to_move) {
            self.status = Status::Won(self.to_move);
        } else if self.moves.len() == 225 {
            // Only reachable if the last move did not win — the `else`
            // encodes "a winning final move is a win, not a draw".
            self.status = Status::Draw;
        }
```

---

## Cycle 12 — `moves()` returns the history in order

### RED → GREEN (instantly)

```rust
    #[test]
    fn moves_returns_history_in_play_order() {
        let mut b = Board::new();
        let script = [(7, 7), (3, 3), (7, 8), (3, 4)];
        for (r, c) in script { b.play(Move::new(r, c).unwrap()).unwrap(); }

        let expected: Vec<Move> = [(7, 7), (3, 3), (7, 8), (3, 4)]
            .iter()
            .map(|&(r, c)| Move::new(r, c).unwrap())
            .collect();
        assert_eq!(b.moves(), expected.as_slice());
    }
```

The `Vec` was pushed in order from cycle 2 on, so this passes. It still
earns its place: `moves()` is what encoding, undo, and the draw rule
all depend on — pin the contract.

---

## The complete files (final state)

Diff these against what you typed.

### `moveset.rs` (the `Move` half — `MoveSet` arrives in slice 3)

```rust
//! `MoveSet`: public set-of-moves bitset, logical stride-15 indexing.
//! Slice 3. Deliberately separate from the internal stride-16 `Bitboard`.

/// A board cell: logical index `row * 15 + col`, 0..=224.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Move(u8);

impl Move {
    /// Validated constructor. `None` if `row` or `col` is off the board.
    pub fn new(row: u8, col: u8) -> Option<Move> {
        if row < 15 && col < 15 {
            Some(Move(row * 15 + col))
        } else {
            None
        }
    }

    pub fn row(self) -> u8 {
        self.0 / 15
    }

    pub fn col(self) -> u8 {
        self.0 % 15
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_off_board_cells() {
        assert_eq!(Move::new(15, 0), None);
        assert_eq!(Move::new(0, 15), None);
        assert_eq!(Move::new(14, 14), Some(Move(224)));
    }

    #[test]
    fn index_row_col_are_consistent() {
        let mv = Move::new(7, 3).unwrap();
        assert_eq!(mv.index(), 7 * 15 + 3);
        assert_eq!(mv.row(), 7);
        assert_eq!(mv.col(), 3);
    }
}
```

### `reference.rs` (complete)

```rust
//! Naive array-based reference engine — the differential-testing oracle.
//! Slice 2. Compiled only for tests and the `testutil` feature.

use crate::moveset::Move;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black,
    White,
}

impl Color {
    pub fn other(self) -> Color {
        match self {
            Color::Black => Color::White,
            Color::White => Color::Black,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cell {
    Empty,
    Stone(Color),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ongoing,
    Won(Color),
    Draw,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PlayError {
    #[error("cell is already occupied")]
    Occupied,
    #[error("the game is already over")]
    GameOver,
}

const DIRECTIONS: [(i32, i32); 4] = [(0, 1), (1, 0), (1, 1), (1, -1)];

pub struct Board {
    cells: [[Cell; 15]; 15],
    to_move: Color,
    status: Status,
    moves: Vec<Move>,
}

impl Board {
    pub fn new() -> Board {
        Board {
            cells: [[Cell::Empty; 15]; 15],
            to_move: Color::Black,
            status: Status::Ongoing,
            moves: Vec::new(),
        }
    }

    pub fn play(&mut self, mv: Move) -> Result<(), PlayError> {
        if self.status != Status::Ongoing {
            return Err(PlayError::GameOver);
        }
        if self.stone_at(mv).is_some() {
            return Err(PlayError::Occupied);
        }
        let (r, c) = (mv.row() as usize, mv.col() as usize);
        self.cells[r][c] = Cell::Stone(self.to_move);
        self.moves.push(mv);
        if self.wins_from(r, c, self.to_move) {
            self.status = Status::Won(self.to_move);
        } else if self.moves.len() == 225 {
            self.status = Status::Draw;
        }
        self.to_move = self.to_move.other();
        Ok(())
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn to_move(&self) -> Color {
        self.to_move
    }

    pub fn stone_at(&self, mv: Move) -> Option<Color> {
        match self.cells[mv.row() as usize][mv.col() as usize] {
            Cell::Empty => None,
            Cell::Stone(color) => Some(color),
        }
    }

    pub fn moves(&self) -> &[Move] {
        &self.moves
    }

    fn wins_from(&self, r: usize, c: usize, color: Color) -> bool {
        DIRECTIONS
            .iter()
            .any(|&(dr, dc)| 1 + self.count_dir(r, c, dr, dc, color)
                               + self.count_dir(r, c, -dr, -dc, color) >= 5)
    }

    fn count_dir(&self, r: usize, c: usize, dr: i32, dc: i32, color: Color) -> usize {
        let mut n = 0;
        let mut nr = r as i32 + dr;
        let mut nc = c as i32 + dc;
        while (0..15).contains(&nr)
            && (0..15).contains(&nc)
            && self.cells[nr as usize][nc as usize] == Cell::Stone(color)
        {
            n += 1;
            nr += dr;
            nc += dc;
        }
        n
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // … all tests from cycles 1–12, as written above …
}
```

## Gates, then commit

```bash
cd gomoku
cargo test -p engine
cargo clippy -p engine --all-targets -- -D warnings
cargo fmt --all
git add -A && git commit -m "feat(engine): naive reference engine + corpus"
```

You should have: 2 `Move` tests + 15 board tests, all green; clippy and
fmt silent.

**Done when you can answer without looking:** Why does only the last
stone need checking? Why can six-in-a-row only arise by bridging? Why
is the draw test's checkerboard provably safe at every prefix? Why does
the `else` in front of the draw branch matter?

Next: [Slice 3 — Bitboard and Board](03-bitboard-and-board.md)
