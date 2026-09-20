# Slice 2 — The reference engine

You build the naive engine: `[[Cell; 15]; 15]`, `for` loops, zero tricks.
It is the *oracle* — the obviously-correct slow code that every clever
bitboard optimization must agree with (ch. 13, "Reference oracle").

**Why this comes first.** A differential test is only as good as its
oracle. This slice earns trust in the oracle with a hand-written corpus
*before* anything fast exists.

## Contract

`Move` lives in `moveset.rs` (it is shared by both engines):

```rust
/// A board cell: logical index `row * 15 + col`, 0..=224.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Move(u8);

impl Move {
    pub fn new(row: u8, col: u8) -> Option<Move>;  // None if row/col >= 15
    pub fn row(self) -> u8;
    pub fn col(self) -> u8;
    pub fn index(self) -> usize;                   // r * 15 + c
}
```

In `reference.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color { Black, White }

impl Color {
    pub fn other(self) -> Color;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status { Ongoing, Won(Color), Draw }

pub struct Board { /* cells, to_move, status, move history — your choice */ }

impl Board {
    pub fn new() -> Board;
    pub fn play(&mut self, mv: Move) -> Result<(), PlayError>;
    pub fn status(&self) -> Status;
    pub fn to_move(&self) -> Color;
    pub fn stone_at(&self, mv: Move) -> Option<Color>;  // None = empty
    pub fn moves(&self) -> &[Move];
}
```

`PlayError` is a `thiserror` enum: `Occupied`, `GameOver`. (Exact naming
is yours; the behaviors below are not.)

Rules: Black moves first, colors alternate, overlines count as a win,
draw at 225 moves, no moves after a decided game.

## Rust toolbox

**Small `Copy` enums.** `Color` and `Status` are fieldless enums — one
byte each. Deriving `Copy` lets them behave like integers: pass by
value, no clones, no borrow fights. Derive `PartialEq, Eq` so tests can
`assert_eq!`, and `Debug` so failures print something readable.

**`[[Cell; 15]; 15]` initialization.** `[[Cell::Empty; 15]; 15]` works
only because `Cell` is `Copy` — the array-repeat syntax copies one value
into every slot. Without `Copy` you would need `std::array::from_fn`.

**The newtype pattern.** `Move(u8)` instead of a bare `u8`: the
*validated constructor* (`new` returns `None` off-board) makes invalid
moves unrepresentable through the front door. Inside the crate you may
add a `pub(crate) fn from_index_unchecked` for trusted paths — document
why it is safe.

**Win detection, naive style.** After placing at `(r, c)`: for each of
the four axes (horizontal, vertical, two diagonals), walk both ways from
the new stone counting consecutive same-color stones; win if the total
is ≥ 5. Loops with bounds checks — readability over speed. This is also
the algorithm the corpus can verify by eye.

## TDD checklist — one behavior per cycle

1. `new()` → empty board, `Ongoing`, Black to move
2. `play` places a stone, flips `to_move`
3. `play` on an occupied cell → `Err`
4. horizontal five → `Won(Black)` (test White too)
5. vertical five
6. both diagonals
7. six in a row → still a win (overline, ch. 13 decision 1)
8. four in a row → `Ongoing` (the near-miss)
9. win at board edge and in a corner (rows/cols 0 and 14)
10. `play` after `Won` → `Err(GameOver)`
11. 225 moves with no five → `Draw`
12. `moves()` returns the history in order

Steps 4–9 are the corpus that must convince *you* the oracle is right.
Write them with literal coordinates — no helpers yet.

## ML refresh: why an oracle at all?

The fast engine (slice 3) trades readable code for bit tricks, and bit
tricks fail in ways that *look* fine — a wrong edge mask still wins most
games. Differential testing answers this: play thousands of random games
on both engines, demand identical status at every ply. The oracle's only
job is to be so simple that its correctness is obvious by inspection.
This is the same reason ML teams keep a "golden dataset" of hand-checked
examples: clever systems are validated against boring truths.

## Pitfalls

- Do not optimize. If you feel clever, you are building slice 3 early.
- Do not skip the near-miss tests (7–9). They are where oracle bugs hide.
- `Board` here is `reference::Board`. No trait, no shared abstraction —
  the tests drive both types directly (ch. 13).

## Done when

All 12 behaviors green, `cargo clippy -- -D warnings` clean,
`cargo fmt` clean. Commit: `feat(engine): naive reference engine + corpus`.

Worked solution with the full TDD path: [02-solution.md](03-bitboard-and-board/02-solution.md) — read, understand, then type it yourself.

Next: [Slice 3 — Bitboard and Board](03-bitboard-and-board.md)
