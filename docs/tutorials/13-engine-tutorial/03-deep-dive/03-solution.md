# Slice 3 — Solutions: bitboard and Board

This is the full, commented solution set for
[Slice 3](../03-bitboard-and-board.md), walking the TDD path cycle by
cycle. Same protocol as slice 2:

1. Read the cycle's **RED** test; say out loud which behavior it pins.
2. Type it. Watch it fail for the right reason.
3. Read the **GREEN** code line by line — if a line is unclear, stop
   there; that line is the lesson.
4. Type it. Watch it go green. Move on.

Do not copy-paste. The complete final state of every file is at the
end — use it to diff against your typed version.

This slice has a different rhythm than slice 2: the *unit* tests are
quick sanity checks, and the real proof is the **differential harness**
— thousands of random games where the fast engine must agree with your
oracle on every single ply. You already earned trust in the oracle;
now you spend it.

---

## Step 0 — Refactor under green: shared types move to `board.rs`

Both engines must speak the same `Color` / `Status` / `PlayError` — if
each defined its own, the differential test could never compare them.
So those three types leave `reference.rs` and move to `board.rs`, their
permanent home (ch. 13 crate layout puts them with `Board`).

**This is a refactor: no behavior changes, tests stay green
throughout.** Move first, run, *then* start the TDD cycles.

1. Cut `Color` (with its `impl`), `Status`, and `PlayError` out of
   `reference.rs`; paste them into `board.rs`, below the doc comment.
2. In `reference.rs`, replace them with:

   ```rust
   use crate::board::{Color, PlayError, Status};
   ```

3. In `lib.rs`, extend the public surface (ch. 13: "re-exports ONLY
   what belongs to the API"):

   ```rust
   // Public surface (ch. 13, "Crate layout" — re-exports ONLY what
   // belongs to the API). Grows slice by slice.
   pub use board::{Board, Color, PlayError, Status};
   pub use moveset::Move;
   ```

   (`Board` does not exist yet — add it to the `pub use` in cycle 4,
   when it does. Rust will not let you re-export a name that isn't
   there.)

4. `cargo test -p engine` — still 17 green. If yes, the refactor is
   done. If no, the move broke an import; fix imports, not logic.

Notice what just happened: the tests from slice 2 acted as a safety
net for surgery on the very code they test. That is the second job of
a test suite.

---

## Cycle 1 — `Bitboard` set / test / clear / count

`Bitboard` is crate-internal machinery, so its tests live in its own
`#[cfg(test)]` module — not in `tests/` (integration tests only see the
public API; the tutorial's pitfalls section warns about this).

### RED

```rust
// bitboard.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_test_clear_on_corners_and_edges() {
        let bb = Bitboard::EMPTY;
        assert!(bb.is_zero());

        // All four corners of the *bitboard* square (stride 16).
        let bb = bb.with_bit(idx(0, 0));
        assert!(bb.test(idx(0, 0)));
        assert!(!bb.test(idx(0, 1))); // neighbor untouched
        assert_eq!(bb.count(), 1);

        let bb = bb
            .with_bit(idx(0, 14))
            .with_bit(idx(14, 0))
            .with_bit(idx(14, 14));
        assert_eq!(bb.count(), 4);

        let bb = bb.without_bit(idx(0, 0));
        assert!(!bb.test(idx(0, 0)));
        assert_eq!(bb.count(), 3);
    }

    #[test]
    fn bits_crossing_a_word_boundary_land_correctly() {
        // idx 62 lives in word 0, idx 64 in word 1 — the / and % math
        // in with_bit/test must agree on the seam.
        let bb = Bitboard::EMPTY
            .with_bit(idx(3, 14)) // 3*16+14 = 62
            .with_bit(idx(4, 0)); // 4*16+0  = 64
        assert!(bb.test(idx(3, 14)));
        assert!(bb.test(idx(4, 0)));
        assert!(!bb.test(idx(4, 1)));
        assert_eq!(bb.count(), 2);
    }
}
```

### GREEN

```rust
//! `Bitboard`: stride-16 `[u64; 4]`, shifts, iterators. `pub(crate)`.
//! Slice 3. See docs/13-engine-design.md, "The bitboard and the
//! padding invariant".

/// Stride-16 index of cell (r, c). THE conversion into bitboard-land;
/// `Move`-land uses stride 15. Confusing the two is *the* bug of this
/// slice — convert at the boundary, in exactly one place each way.
pub(crate) const fn idx(r: u8, c: u8) -> usize {
    r as usize * 16 + c as usize
}

/// 256 bits, of which 240 are addressable cells (15 rows × 16 stride)
/// and 225 are real board cells. Column 15 of every row plus bits
/// 240–255 are padding — the invariant says they are ALWAYS zero.
///
/// Newtype over `[u64; 4]`: `Copy` (32 bytes — cheaper than a
/// reference), and the wrapper keeps bitboard math from mixing with
/// plain integers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Bitboard(pub(crate) [u64; 4]);

impl Bitboard {
    pub(crate) const EMPTY: Bitboard = Bitboard([0; 4]);

    /// Returns a copy with bit `idx` set. `self` by value because
    /// `Copy`: reads like a persistent data structure, costs nothing.
    pub(crate) fn with_bit(self, idx: usize) -> Bitboard {
        let mut w = self.0;
        // Which u64, then which bit inside it. `1 << (idx % 64)`:
        // the shift amount is modulo the word size, so no overflow.
        w[idx / 64] |= 1 << (idx % 64);
        Bitboard(w)
    }

    pub(crate) fn without_bit(self, idx: usize) -> Bitboard {
        let mut w = self.0;
        w[idx / 64] &= !(1 << (idx % 64));
        Bitboard(w)
    }

    pub(crate) fn test(&self, idx: usize) -> bool {
        self.0[idx / 64] & (1 << (idx % 64)) != 0
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.0 == [0; 4]
    }

    pub(crate) fn count(&self) -> u32 {
        // count_ones = one POPCNT instruction per word on modern CPUs.
        self.0.iter().map(|w| w.count_ones()).sum()
    }
}
```

---

## Cycle 2 — Operators, `VALID`, and the padding invariant

### RED

```rust
    #[test]
    fn operators_make_bitboard_math_readable() {
        let a = Bitboard::EMPTY.with_bit(idx(7, 7)).with_bit(idx(7, 8));
        let b = Bitboard::EMPTY.with_bit(idx(7, 8)).with_bit(idx(7, 9));

        assert_eq!((a & b).count(), 1); // only (7,8) shared
        assert_eq!((a | b).count(), 3);
        assert!(!(!a).test(idx(7, 7))); // complement flips set bits off…
        assert!((!a).test(idx(0, 0))); // …and every unset bit on…
        assert!((!a).test(idx(0, 15))); // …INCLUDING padding. Mask!
    }

    #[test]
    fn valid_has_225_bits_and_clean_padding() {
        assert_eq!(VALID.count(), 225);
        assert!((VALID & !VALID).is_zero());
        assert_clean(&VALID);
    }

    #[test]
    #[should_panic]
    fn assert_clean_catches_dirty_padding() {
        let dirty = Bitboard::EMPTY.with_bit(idx(0, 15)); // padding column
        assert_clean(&dirty);
    }
```

### GREEN

The operator traits from `std::ops` — after these impls, `a & b`,
`a | b`, `!a` just work, and win detection gets to read like math:

```rust
use std::ops::{BitAnd, BitOr, Not};

impl BitAnd for Bitboard {
    type Output = Bitboard;
    fn bitand(self, rhs: Bitboard) -> Bitboard {
        // Word-wise AND; the `each_mut`+zip style is avoided on
        // purpose — four explicit words are the readable extreme.
        Bitboard([
            self.0[0] & rhs.0[0],
            self.0[1] & rhs.0[1],
            self.0[2] & rhs.0[2],
            self.0[3] & rhs.0[3],
        ])
    }
}

impl BitOr for Bitboard {
    type Output = Bitboard;
    fn bitor(self, rhs: Bitboard) -> Bitboard {
        Bitboard([
            self.0[0] | rhs.0[0],
            self.0[1] | rhs.0[1],
            self.0[2] | rhs.0[2],
            self.0[3] | rhs.0[3],
        ])
    }
}

impl Not for Bitboard {
    type Output = Bitboard;
    fn not(self) -> Bitboard {
        // WARNING (the padding invariant): `!` sets EVERY padding bit.
        // Any complement must immediately be masked: `!x & VALID`.
        Bitboard([!self.0[0], !self.0[1], !self.0[2], !self.0[3]])
    }
}
```

And the `VALID` mask — built by a `const` block, evaluated at compile
time (same trick as the Zobrist table in slice 5):

```rust
/// All 225 real cells set, all padding bits zero. The one mask that
/// makes complements safe: `!occupied & VALID`.
pub(crate) const VALID: Bitboard = {
    let mut w = [0u64; 4];
    let mut r = 0usize;
    while r < 15 {
        let mut c = 0usize;
        while c < 15 {
            let i = r * 16 + c;
            w[i / 64] |= 1 << (i % 64);
            c += 1;
        }
        r += 1;
    }
    Bitboard(w)
};

/// The crate's single most valuable assertion: padding bits are zero.
/// Checked in tests after random operation sequences (and, in slice 4,
/// by proptest). `#[cfg(test)]` — it does not exist in release builds.
#[cfg(test)]
pub(crate) fn assert_clean(b: &Bitboard) {
    assert!(
        (*b & !VALID).is_zero(),
        "padding invariant violated: {b:?} has bits outside the 225 real cells"
    );
}
```

Why this invariant is worth its own cycle: every win-detection AND in
slice 4 assumes a 5-chain cannot wrap around a row end. Wraps cross the
padding column. If padding is always zero, the AND-chain dies on every
wrap path — no per-direction edge masks, ever. One invariant replaces
four families of bugs.

---

## Cycle 3 — `shr`: multi-word shifts, and the shift-by-64 trap

### RED

```rust
    #[test]
    fn shr_moves_stones_down_by_the_shift_amount() {
        // Stone at (2,3) = idx 35. shr(s) SUBTRACTS s from every bit
        // index — it is a true >> on the 256-bit value. (Win detection
        // does not care which way the shift goes: b & b.shr(s) pairs
        // stones at i and i+s either way, and slice 4 relies on exactly
        // that. What matters is that the four DIRECTIONS are uniform.)
        let bb = Bitboard::EMPTY.with_bit(idx(2, 3));
        assert!(bb.shr(1).test(idx(2, 2))); //  35−1  = 34 = (2,2)  ←
        assert!(bb.shr(16).test(idx(1, 3))); // 35−16 = 19 = (1,3)  ↑
        assert!(bb.shr(15).test(idx(1, 4))); // 35−15 = 20 = (1,4)  ↗
        assert!(bb.shr(17).test(idx(1, 2))); // 35−17 = 18 = (1,2)  ↖
        assert_eq!(bb.shr(1).count(), 1); // shifts duplicate nothing
    }

    #[test]
    fn shr_carries_bits_across_word_boundaries() {
        // idx 77 lives in word 1 (bits 64–127); 77−17 = 60 lands in
        // word 0. This is the case a naive per-word `>>` gets wrong.
        let bb = Bitboard::EMPTY.with_bit(idx(4, 13)); // 4*16+13 = 77
        let shifted = bb.shr(17);
        assert!(shifted.test(idx(3, 12))); // 60 = 3*16+12
        assert_eq!(shifted.count(), 1);
    }
```

### GREEN

```rust
    /// Logical right shift of the whole 256-bit value by `s`,
    /// `0 < s < 64`. Bits shifted past the top are lost (fine: they
    /// are padding or beyond).
    pub(crate) fn shr(&self, s: u32) -> Bitboard {
        // THE TRAP: `x << 64` on u64 is a panic in debug builds and
        // SILENT GARBAGE in release (the hardware masks the shift
        // amount to 6 bits, so `<< 64` behaves like `<< 0`). Our
        // callers only ever pass 1, 15, 16, 17, 2*s of those — never
        // 0 or ≥64. debug_assert enforces it loudly in tests, free in
        // release. (Slice 4's staged AND exists precisely to keep
        // every shift under 64 — ch. 13.)
        debug_assert!(s > 0 && s < 64);
        let w = self.0;
        Bitboard([
            // Each output word keeps its own bits moved down by s, and
            // its TOP s bits come from the NEXT word's bottom s bits —
            // `w[k+1] << (64 - s)` is the carry bridge across the seam.
            (w[0] >> s) | (w[1] << (64 - s)),
            (w[1] >> s) | (w[2] << (64 - s)),
            (w[2] >> s) | (w[3] << (64 - s)),
            w[3] >> s,
        ])
    }
```

Read the first line of the body until it clicks: big-number bit 64+j
(for small j) must become bit 64+j−s after a right shift by s — that is
word 0, bit j+64−s, which is precisely where `w[1] << (64 - s)` puts
it. The `shr_carry` test proves the bridge exists — without it, every
line longer than a word seam would silently break.

---

## Cycle 4 — Fast `Board`: new / play / to_move, harness v1

### RED — unit sanity first

```rust
// board.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::moveset::Move;

    #[test]
    fn new_board_has_225_legal_moves_black_to_move() {
        let b = Board::new();
        assert_eq!(b.status(), Status::Ongoing);
        assert_eq!(b.to_move(), Color::Black);
        assert_eq!(b.empty_moves().count(), 225);
        assert!(b.moves().is_empty());
    }

    #[test]
    fn play_places_a_stone_and_flips() {
        let mut b = Board::new();
        let mv = Move::new(7, 7).unwrap();
        b.play(mv).unwrap();
        assert_eq!(b.to_move(), Color::White);
        assert!(!b.is_legal(mv)); // occupied now
        assert_eq!(b.empty_moves().count(), 224);
    }
}
```

### RED — harness v1 (the deliverable begins)

Create `tests/differential.rs`:

```rust
//! Differential tests: the fast bitboard engine must agree with the
//! naive reference oracle on every ply of thousands of random games.
//!
//! This is an INTEGRATION test: it links against `engine` as an
//! external crate, which means the library is built WITHOUT `cfg(test)`
//! — so the reference engine is only visible through the `testutil`
//! feature (see the `[[test]]` section in Cargo.toml).

use engine::reference;
use engine::{Board, Move, Status};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn fast_matches_naive(cells in prop::collection::vec(0u16..225, 1..=225)) {
        let mut fast = Board::new();
        let mut naive = reference::Board::new();
        for p in cells {
            let mv = Move::new((p / 15) as u8, (p % 15) as u8).unwrap();
            // v1: the fast engine has no win detection yet, so games
            // can only end on the naive side. Stop there, and compare
            // only MOVE ACCEPTANCE — status comparison arrives in
            // cycle 6 together with win detection. Grow the harness
            // with the implementation; never test what isn't built.
            if naive.status() != Status::Ongoing {
                break;
            }
            let fast_r = fast.play(mv);
            let naive_r = naive.play(mv);
            prop_assert_eq!(fast_r.is_ok(), naive_r.is_ok());
        }
    }
}
```

And register the target in `crates/engine/Cargo.toml`:

```toml
# The differential suite needs the reference engine, which only exists
# under the `testutil` feature. `required-features` makes cargo SKIP
# this target when the feature is off instead of failing to compile.
# Run it with: cargo test -p engine --features testutil
[[test]]
name = "differential"
required-features = ["testutil"]
```

### GREEN

```rust
//! `Board`: absolute colors + `to_move`, play/undo, legality, status.
//! Slice 3. See docs/13-engine-design.md, "Board".

use crate::bitboard::{Bitboard, VALID, idx};
use crate::moveset::Move;

// … Color, Status, PlayError live here since step 0 …

/// The production board. Two bitboards in ABSOLUTE colors (ch. 13,
/// decision 5: Swap2's non-alternating opening cannot be expressed in
/// a relative me/you store), plus side to move, status, and full
/// move history (encoding, undo, game records).
///
/// `PartialEq` is derived for tests ("undo everything ⇒ equals
/// `Board::new()`"); it compares bitboards, not game meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    black: Bitboard,
    white: Bitboard,
    to_move: Color,
    status: Status,
    moves: Vec<Move>,
}

impl Board {
    pub fn new() -> Board {
        Board {
            black: Bitboard::EMPTY,
            white: Bitboard::EMPTY,
            to_move: Color::Black,
            status: Status::Ongoing,
            moves: Vec::new(),
        }
    }

    pub fn play(&mut self, mv: Move) -> Result<(), PlayError> {
        // Guards first, mutation second — a rejected move changes
        // nothing, by construction (same discipline as the oracle).
        if self.status != Status::Ongoing {
            return Err(PlayError::GameOver);
        }
        let i = idx(mv.row(), mv.col()); // THE boundary conversion
        if (self.black | self.white).test(i) {
            return Err(PlayError::Occupied);
        }
        match self.to_move {
            Color::Black => self.black = self.black.with_bit(i),
            Color::White => self.white = self.white.with_bit(i),
        }
        self.moves.push(mv);
        self.to_move = self.to_move.other();
        Ok(())
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn to_move(&self) -> Color {
        self.to_move
    }

    pub fn moves(&self) -> &[Move] {
        &self.moves
    }

    /// Internal: the raw bitboard of one color. `pub(crate)`, NOT
    /// `pub` — `Bitboard` is a crate-private type, and Rust refuses
    /// to leak private types through public functions (E0446).
    /// Tactics (slice 7) uses this; the outside world observes moves,
    /// not bits.
    pub(crate) fn stones(&self, color: Color) -> Bitboard {
        match color {
            Color::Black => self.black,
            Color::White => self.white, // arm-for-arm mirror —
        }                               // copy-paste the line above
    }                                   // and the harness will catch
}                                       // it, but why feed it?
```

Run: `cargo test -p engine --features testutil`. 10k random games agree
on move acceptance. First differential green.

---

## Cycle 5 — `is_legal` and `empty_moves`

### RED

```rust
    #[test]
    fn empty_moves_and_legality_track_the_stones() {
        let mut b = Board::new();
        let mv = Move::new(7, 7).unwrap();
        assert!(b.is_legal(mv));

        b.play(mv).unwrap();
        assert!(!b.is_legal(mv));
        assert_eq!(b.empty_moves().count(), 224);
        assert!(b.empty_moves().all(|m| m != mv));

        // After a full game the iterator is empty — and terminates
        // (no infinite yield of padding cells!).
        for r in 0..15u8 {
            for c in 0..15u8 {
                let _ = b.play(Move::new(r, c).unwrap());
            }
        }
        assert_eq!(b.empty_moves().count(), 0);
    }
```

### GREEN

```rust
    pub fn is_legal(&self, mv: Move) -> bool {
        self.status == Status::Ongoing
            && !(self.black | self.white).test(idx(mv.row(), mv.col()))
    }

    pub fn empty_moves(&self) -> impl Iterator<Item = Move> + '_ {
        // `!occupied` sets ALL padding bits — mask immediately.
        // This is the one place complements are allowed, and the mask
        // is non-negotiable (the padding invariant, ch. 13).
        let empty = !(self.black | self.white) & VALID;

        // The classic set-bit walk, word by word. `bits & (bits - 1)`
        // clears the lowest set bit — commit it to memory, you will
        // meet it in every bitboard codebase.
        empty.0.into_iter().enumerate().flat_map(|(w, mut bits)| {
            std::iter::from_fn(move || {
                if bits == 0 {
                    return None; // word exhausted → next word
                }
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let i = w * 64 + bit; // stride-16 index
                // Back across the boundary: stride-16 → (row, col) →
                // stride-15 Move. The `unwrap` is justified by VALID:
                // padding bits are never set in `empty`, so c < 15
                // always. Drop the mask and this panics — LOUD, never
                // silently wrong.
                Some(Move::new((i / 16) as u8, (i % 16) as u8).unwrap())
            })
        })
    }
```

Type notes worth pausing on:

- `empty.0.into_iter()` — arrays are `IntoIterator` by value since
  Rust 2021; this yields the four `u64`s, no slicing needed.
- `impl Iterator<Item = Move> + '_` — you never write the concrete type
  (a `FlatMap<Enumerate<IntoIter…>>` monstrosity); the compiler checks
  that *some* iterator is returned. The `+ '_` ties it to `&self`
  (here it captures only `empty`, an owned copy — the bound is
  harmless either way).

---

## Cycle 6 — Win status (simple version) and harness v2 at 10k

Slice 4 gives win detection the full bit-twiddling treatment. For this
slice, the *simple* version: the same walk-based algorithm as the
oracle, reading bits instead of array cells. The differential harness
then proves the two agree — and in slice 4 it proves the clever
version still does.

### RED — one scripted unit test

```rust
    #[test]
    fn horizontal_five_wins() {
        let mut b = Board::new();
        let script = [
            (7, 3), (0, 0),
            (7, 4), (0, 2),
            (7, 5), (0, 4),
            (7, 6), (0, 6),
            (7, 7),
        ];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(b.status(), Status::Won(Color::Black));
        // A decided game rejects further play.
        assert_eq!(
            b.play(Move::new(13, 13).unwrap()),
            Err(PlayError::GameOver)
        );
    }
```

### RED — harness v2: compare everything

Upgrade the loop body in `tests/differential.rs`:

```rust
        for p in cells {
            let mv = Move::new((p / 15) as u8, (p % 15) as u8).unwrap();
            // v2: the fast engine now ends games itself.
            if fast.status() != Status::Ongoing {
                break;
            }
            let fast_r = fast.play(mv);
            let naive_r = naive.play(mv);
            prop_assert_eq!(fast_r.is_ok(), naive_r.is_ok());
            prop_assert_eq!(fast.status(), naive.status());
            prop_assert_eq!(fast.to_move(), naive.to_move());
        }
```

### GREEN

```rust
/// Win directions as index steps in stride-16 land: →, ↓, ↙, ↘.
const DIRS: [i32; 4] = [1, 16, 15, 17];

impl Board {
    // inside play, after self.moves.push(mv):
    //
    //     if self.wins_from(i, self.to_move) {
    //         self.status = Status::Won(self.to_move);
    //     } else if self.moves.len() == 225 {
    //         // `else`: a winning 225th move is a win, not a draw.
    //         self.status = Status::Draw;
    //     }

    /// Simple walk-based win check through the last-placed stone —
    /// deliberately the SAME algorithm as the oracle, reading bits
    /// instead of array cells. Slice 4 swaps in the staged-AND bit
    /// version; the harness must stay green through that swap.
    fn wins_from(&self, i: usize, color: Color) -> bool {
        let stones = self.stones(color);
        DIRS.iter()
            .any(|&s| 1 + count_walk(stones, i, s) + count_walk(stones, i, -s) >= 5)
    }
}

/// Consecutive set bits starting one step away from `i`, walking in
/// `step` direction. Two termination mechanisms, both free:
///   - the index guard stops walks at the array edge (verticals);
///   - the PADDING INVARIANT stops horizontal/diagonal wraps: every
///     wrap path lands in column 15, whose bits are always zero.
fn count_walk(stones: Bitboard, i: usize, step: i32) -> usize {
    let mut n = 0;
    let mut x = i as i32 + step;
    while (0..240).contains(&x) && stones.test(x as usize) {
        n += 1;
        x += step;
    }
    n
}
```

Run `cargo test -p engine --features testutil`. **10,000 random games,
identical status at every ply** — that is the milestone-1 acceptance
sentence, and you just made it true. When (not if) proptest ever fails
here, read the shrunk case: it will be a single-digit number of moves
landing exactly on the edge you forgot.

---

## Cycle 7 — `undo`, on both engines

The fast board gets the cheap undo (pop, clear, un-flip). The oracle
gets the *obviously correct* undo: replay the history minus its last
move onto a fresh board. Different implementations, same contract —
which is exactly what differential testing eats for breakfast.

### RED — fast-board unit tests

```rust
    #[test]
    fn undo_everything_restores_a_pristine_board() {
        let mut b = Board::new();
        let script = [(7, 7), (3, 3), (7, 8), (3, 4)];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        for _ in 0..4 {
            b.undo();
        }
        assert_eq!(b, Board::new()); // why Board derives PartialEq
    }

    #[test]
    fn undoing_the_winning_move_unwins_the_game() {
        let mut b = Board::new();
        let script = [
            (7, 3), (0, 0), (7, 4), (0, 2), (7, 5), (0, 4), (7, 6), (0, 6), (7, 7),
        ];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(b.status(), Status::Won(Color::Black));

        b.undo();
        assert_eq!(b.status(), Status::Ongoing); // un-won
        assert_eq!(b.to_move(), Color::Black); // Black to move again
        assert!(b.is_legal(Move::new(7, 7).unwrap())); // cell free
        assert_eq!(b.moves().len(), 8);
    }
```

### RED — harness v3: random play/undo walks

Add to `tests/differential.rs` as a **separate `proptest!` block** —
the naive undo replays history, so walks must stay short and the case
count lower, or the suite gets slow:

```rust
proptest! {
    #![proptest_config(ProptestConfig::with_cases(1_000))]

    #[test]
    fn undo_walks_match_naive(
        ops in prop::collection::vec((0u16..225, 0u8..3), 1..=100)
    ) {
        let mut fast = Board::new();
        let mut naive = reference::Board::new();
        for (p, coin) in ops {
            // ~1/3 of steps: undo, if there is anything to undo.
            if coin == 0 && !fast.moves().is_empty() {
                fast.undo();
                naive.undo();
            } else if fast.status() == Status::Ongoing {
                let mv = Move::new((p / 15) as u8, (p % 15) as u8).unwrap();
                let _ = fast.play(mv); // may be Occupied — both must agree
                let _ = naive.play(mv);
            }
            prop_assert_eq!(fast.status(), naive.status());
            prop_assert_eq!(fast.to_move(), naive.to_move());
            prop_assert_eq!(fast.moves(), naive.moves());
        }
    }
}
```

### GREEN — fast undo (in `board.rs`)

```rust
    /// Undoes the last move. Caller guarantees the history is
    /// non-empty — hence `expect`, not `Result`: this is an internal
    /// contract, not user input.
    pub fn undo(&mut self) {
        let mv = self.moves.pop().expect("undo with non-empty history");
        let i = idx(mv.row(), mv.col());
        // The stone belongs to the side that PLAYED it — and `to_move`
        // was flipped after that play, so the owner is `to_move.other()`.
        match self.to_move.other() {
            Color::Black => self.black = self.black.without_bit(i),
            Color::White => self.white = self.white.without_bit(i),
        }
        self.to_move = self.to_move.other(); // flip back
        // Undoing the winning move un-wins the game; undoing into a
        // would-be draw likewise reopens it. Ongoing is always right.
        self.status = Status::Ongoing;
    }
```

### GREEN — oracle undo (in `reference.rs`)

```rust
    /// Undoes the last move by REPLAYING the remaining history onto a
    /// fresh board. O(n) where the fast undo is O(1) — and obviously
    /// correct, which is the only virtue an oracle needs.
    pub fn undo(&mut self) {
        self.moves.pop().expect("undo with non-empty history");
        // A prefix of a legal game is always replayable: a game ends
        // only ON its winning/drawing move, and we just removed it.
        let history = self.moves.clone();
        *self = Board::new();
        for mv in history {
            self.play(mv).expect("replaying a valid history");
        }
    }
```

Plus one small oracle-side test (the oracle earns trust through tests,
always):

```rust
    #[test]
    fn undo_replays_history_without_the_last_move() {
        let mut b = Board::new();
        let script = [
            (7, 3), (0, 0), (7, 4), (0, 2), (7, 5), (0, 4), (7, 6), (0, 6), (7, 7),
        ];
        for (r, c) in script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(b.status(), Status::Won(Color::Black));

        b.undo();
        assert_eq!(b.status(), Status::Ongoing);
        assert_eq!(b.stone_at(Move::new(7, 7).unwrap()), None);
        assert_eq!(b.moves().len(), 8);
    }
```

---

## The complete files (final state)

Only the skeletons — your comments are the ones you typed above.

### `bitboard.rs` (complete, sans tests)

```rust
//! `Bitboard`: stride-16 `[u64; 4]`, shifts, iterators. `pub(crate)`.

use std::ops::{BitAnd, BitOr, Not};

pub(crate) const fn idx(r: u8, c: u8) -> usize {
    r as usize * 16 + c as usize
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Bitboard(pub(crate) [u64; 4]);

impl Bitboard {
    pub(crate) const EMPTY: Bitboard = Bitboard([0; 4]);

    pub(crate) fn with_bit(self, idx: usize) -> Bitboard {
        let mut w = self.0;
        w[idx / 64] |= 1 << (idx % 64);
        Bitboard(w)
    }

    pub(crate) fn without_bit(self, idx: usize) -> Bitboard {
        let mut w = self.0;
        w[idx / 64] &= !(1 << (idx % 64));
        Bitboard(w)
    }

    pub(crate) fn test(&self, idx: usize) -> bool {
        self.0[idx / 64] & (1 << (idx % 64)) != 0
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.0 == [0; 4]
    }

    pub(crate) fn count(&self) -> u32 {
        self.0.iter().map(|w| w.count_ones()).sum()
    }

    pub(crate) fn shr(&self, s: u32) -> Bitboard {
        debug_assert!(s > 0 && s < 64);
        let w = self.0;
        Bitboard([
            (w[0] >> s) | (w[1] << (64 - s)),
            (w[1] >> s) | (w[2] << (64 - s)),
            (w[2] >> s) | (w[3] << (64 - s)),
            w[3] >> s,
        ])
    }
}

impl BitAnd for Bitboard {
    type Output = Bitboard;
    fn bitand(self, rhs: Bitboard) -> Bitboard {
        Bitboard([
            self.0[0] & rhs.0[0],
            self.0[1] & rhs.0[1],
            self.0[2] & rhs.0[2],
            self.0[3] & rhs.0[3],
        ])
    }
}

impl BitOr for Bitboard {
    type Output = Bitboard;
    fn bitor(self, rhs: Bitboard) -> Bitboard {
        Bitboard([
            self.0[0] | rhs.0[0],
            self.0[1] | rhs.0[1],
            self.0[2] | rhs.0[2],
            self.0[3] | rhs.0[3],
        ])
    }
}

impl Not for Bitboard {
    type Output = Bitboard;
    fn not(self) -> Bitboard {
        Bitboard([!self.0[0], !self.0[1], !self.0[2], !self.0[3]])
    }
}

pub(crate) const VALID: Bitboard = {
    let mut w = [0u64; 4];
    let mut r = 0usize;
    while r < 15 {
        let mut c = 0usize;
        while c < 15 {
            let i = r * 16 + c;
            w[i / 64] |= 1 << (i % 64);
            c += 1;
        }
        r += 1;
    }
    Bitboard(w)
};

#[cfg(test)]
pub(crate) fn assert_clean(b: &Bitboard) {
    assert!(
        (*b & !VALID).is_zero(),
        "padding invariant violated: {b:?} has bits outside the 225 real cells"
    );
}

#[cfg(test)]
mod tests {
    // 6 tests: corners/edges, word boundary, operators, VALID,
    // assert_clean catches dirty, shr (+word-seam carry)
}
```

### `board.rs` (complete, sans tests)

```rust
//! `Board`: absolute colors + `to_move`, play/undo, legality, status.

use crate::bitboard::{Bitboard, VALID, idx};
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

const DIRS: [i32; 4] = [1, 16, 15, 17];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    black: Bitboard,
    white: Bitboard,
    to_move: Color,
    status: Status,
    moves: Vec<Move>,
}

impl Board {
    pub fn new() -> Board {
        Board {
            black: Bitboard::EMPTY,
            white: Bitboard::EMPTY,
            to_move: Color::Black,
            status: Status::Ongoing,
            moves: Vec::new(),
        }
    }

    pub fn play(&mut self, mv: Move) -> Result<(), PlayError> {
        if self.status != Status::Ongoing {
            return Err(PlayError::GameOver);
        }
        let i = idx(mv.row(), mv.col());
        if (self.black | self.white).test(i) {
            return Err(PlayError::Occupied);
        }
        match self.to_move {
            Color::Black => self.black = self.black.with_bit(i),
            Color::White => self.white = self.white.with_bit(i),
        }
        self.moves.push(mv);
        if self.wins_from(i, self.to_move) {
            self.status = Status::Won(self.to_move);
        } else if self.moves.len() == 225 {
            self.status = Status::Draw;
        }
        self.to_move = self.to_move.other();
        Ok(())
    }

    pub fn undo(&mut self) {
        let mv = self.moves.pop().expect("undo with non-empty history");
        let i = idx(mv.row(), mv.col());
        match self.to_move.other() {
            Color::Black => self.black = self.black.without_bit(i),
            Color::White => self.white = self.white.without_bit(i),
        }
        self.to_move = self.to_move.other();
        self.status = Status::Ongoing;
    }

    pub fn is_legal(&self, mv: Move) -> bool {
        self.status == Status::Ongoing
            && !(self.black | self.white).test(idx(mv.row(), mv.col()))
    }

    pub fn empty_moves(&self) -> impl Iterator<Item = Move> + '_ {
        let empty = !(self.black | self.white) & VALID;
        empty.0.into_iter().enumerate().flat_map(|(w, mut bits)| {
            std::iter::from_fn(move || {
                if bits == 0 {
                    return None;
                }
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let i = w * 64 + bit;
                Some(Move::new((i / 16) as u8, (i % 16) as u8).unwrap())
            })
        })
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn to_move(&self) -> Color {
        self.to_move
    }

    pub fn moves(&self) -> &[Move] {
        &self.moves
    }

    pub(crate) fn stones(&self, color: Color) -> Bitboard {
        match color {
            Color::Black => self.black,
            Color::White => self.white,
        }
    }

    fn wins_from(&self, i: usize, color: Color) -> bool {
        let stones = self.stones(color);
        DIRS.iter()
            .any(|&s| 1 + count_walk(stones, i, s) + count_walk(stones, i, -s) >= 5)
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

fn count_walk(stones: Bitboard, i: usize, step: i32) -> usize {
    let mut n = 0;
    let mut x = i as i32 + step;
    while (0..240).contains(&x) && stones.test(x as usize) {
        n += 1;
        x += step;
    }
    n
}

#[cfg(test)]
mod tests {
    // 5 tests: new board, play flips, empty_moves/legality,
    // horizontal five + game-over, undo×2
}
```

### Other touched files

- `reference.rs`: `Color`/`Status`/`PlayError` deleted, replaced by
  `use crate::board::{Color, PlayError, Status};`; `undo` added;
  one new test. Everything else untouched — the slice-2 corpus still
  guards it.
- `lib.rs`: `pub use board::{Board, Color, PlayError, Status};` and
  `pub use moveset::Move;`.
- `../../../../Cargo.toml`: the `[[test]]` block with `required-features`.
- `tests/differential.rs`: two `proptest!` blocks (10k games;
  1k undo walks) as written in cycles 4/6/7.

## Gates, then commit

```bash
cd gomoku
cargo test -p engine                          # 29 unit tests
cargo test -p engine --features testutil      # + 2 differential properties
cargo clippy -p engine --all-targets --features testutil -- -D warnings
cargo fmt --all
git add -A && git commit -m "feat(engine): bitboard Board with differential tests"
```

**Done when you can answer without looking:** Why does the stride-16
padding column terminate horizontal walks for free? Why must `!`
always be followed by `& VALID`? Which way does `shr` move bit indices
— and why does win detection not care? Why does `shr` forbid `s == 0`
and `s >= 64` — and what does the hardware do if you try? Why is
`stones` `pub(crate)` and not `pub`? Why does undo restore `Ongoing`
unconditionally? Why does the integration test need `testutil` while
the unit tests do not?

Next: [Slice 4 — Win detection](../04-win-detection.md)
