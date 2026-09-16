# Slice 3 — Bitboard and Board

The biggest slice. You build the production `Board` on stride-16
bitboards, and the differential test harness that proves it against your
oracle. Design: ch. 13, "The bitboard and the padding invariant" and
"Board".

## Contract

`bitboard.rs` (all `pub(crate)`):

```rust
pub(crate) struct Bitboard(pub(crate) [u64; 4]); // stride 16: idx = r*16 + c

impl Bitboard {
    pub(crate) const EMPTY: Bitboard;
    pub(crate) fn with_bit(self, idx: usize) -> Bitboard;
    pub(crate) fn without_bit(self, idx: usize) -> Bitboard;
    pub(crate) fn test(&self, idx: usize) -> bool;
    pub(crate) fn is_zero(&self) -> bool;
    pub(crate) fn shr(&self, s: u32) -> Bitboard;  // 0 < s < 64
    pub(crate) fn count(&self) -> u32;
}
```

Implement `BitAnd`, `BitOr`, `Not` for it (toolbox below). Also
`const VALID: Bitboard` — the 225 real cells — and a
`#[cfg(test)] fn assert_clean(&Bitboard)` checking padding bits are zero.

`board.rs`:

```rust
pub struct Board { /* black, white, to_move, status, moves */ }

impl Board {
    pub fn new() -> Board;
    pub fn play(&mut self, mv: Move) -> Result<(), PlayError>;
    pub fn undo(&mut self);                     // caller guarantees non-empty
    pub fn is_legal(&self, mv: Move) -> bool;
    pub fn status(&self) -> Status;
    pub fn to_move(&self) -> Color;
    pub fn stones(&self, color: Color) -> /* &Bitboard or iterator — your call */;
    pub fn empty_moves(&self) -> impl Iterator<Item = Move> + '_;
    pub fn moves(&self) -> &[Move];
}
```

Same rules as the reference. `Color`/`Status`/`PlayError` move out of
`reference.rs` into shared module(s) — both engines use them. Win
detection: for now, a *simple* version is fine (slice 4 gives it the
full treatment); the differential harness is the point of this slice.

## Rust toolbox

**Operator traits.** `impl BitAnd for Bitboard { type Output = Bitboard;
fn bitand(self, rhs: Bitboard) -> Bitboard }` — then `a & b` just works,
which makes win detection read like math. These live in `std::ops`.
Impl for the value type (not references) is fine here: `Bitboard` is 32
bytes and `Copy`... wait — should `Bitboard` be `Copy`? Yes: `[u64; 4]`
is `Copy`, so derive it. Small `Copy` types make operator impls painless.

**The shift-by-64 trap.** `x << 64` on `u64` is *undefined behavior
avoided by a panic* in debug builds (overflow checks) — and silent
garbage in release. `shr` must never receive `s == 0` or `s >= 64`.
Guard with `debug_assert!(s > 0 && s < 64)` — free in release, loud in
tests. This is why slice 4's staged AND exists (ch. 13).

**Iterating set bits.** The classic loop:

```rust
let mut bits = bb;                  // a u64
while bits != 0 {
    let idx = bits.trailing_zeros();
    bits &= bits - 1;               // clear the lowest set bit
    // ... yield idx
}
```

`bits & (bits - 1)` clears the lowest set bit — commit this to memory,
you will meet it in every bitboard codebase. For `empty_moves` you walk
the *complement* within `VALID` across four words.

**`impl Iterator<Item = Move> + '_` return.** Returning `impl Trait`
hides the concrete iterator type (a long `Map<Filter<...>>` you never
want to write). The `+ '_` ties it to `&self`'s lifetime.

## The differential harness (the real deliverable)

In `tests/differential.rs` (integration test — sees only the public API
plus `testutil`):

```rust
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn fast_matches_naive(plays in prop::collection::vec(0u16..225, 1..=225)) {
        let mut fast = Board::new();
        let mut naive = reference::Board::new();
        for p in plays {
            let mv = Move::new((p / 15) as u8, (p % 15) as u8).unwrap();
            if fast.status() != Status::Ongoing { break; }
            let fast_r = fast.play(mv);
            let naive_r = naive.play(mv);
            prop_assert_eq!(fast_r.is_ok(), naive_r.is_ok());
            prop_assert_eq!(fast.status(), naive.status());
        }
    }
}
```

**ML refresh: what proptest does for you.** Property testing is the
fuzzing half of ML-style thinking: instead of hand-picked examples you
define a *distribution* over inputs and an *invariant* that must hold
everywhere. When a case fails, proptest *shrinks* it — searches for the
smallest failing input — which is usually the exact edge case you would
never have written by hand. 10,000 random games is the milestone-1
acceptance number.

## TDD checklist

1. `Bitboard` set/test/clear on corners and edges; `count`
2. operator impls; `VALID` has exactly 225 bits; padding invariant holds
   for `VALID & !VALID == 0` (write `assert_clean` and use it)
3. `shr` correctness on hand-picked patterns (a stone at (0,0) shifted
   by 1, 15, 16, 17 lands where you expect)
4. `Board::new` / `play` / `to_move` — differential vs reference begins
5. `is_legal` + `empty_moves` (count empties = 225 − stones)
6. win/lose/draw status — differential green on 10k games
7. `undo`: play N moves, undo all → board equals `new()`; and after any
   random play/undo walk, differential still holds

## Pitfalls

- `idx = r * 16 + c` — the stride is **16** in bitboard-land and **15**
  in `Move`-land. Confusing them is *the* bug of this slice. Convert at
  the boundary, in exactly one place each way.
- `undo` on a finished game must restore `Ongoing` — undoing the winning
  move un-wins the game.
- Keep `Bitboard` `pub(crate)`. If a test needs it, the test belongs in
  `bitboard.rs`'s own `#[cfg(test)]` module, not in `tests/`.

## Done when

Differential 10k green including undo walks; gates green.
Commit: `feat(engine): bitboard Board with differential tests`.

Worked solution with the full TDD path: [03-solution.md](03-solution.md) — read, understand, then type it yourself.

Next: [Slice 4 — Win detection](04-win-detection.md)
