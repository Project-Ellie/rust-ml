# 02 — Slice 5 implementation plan (with reference solution)

The slice file ([../05-zobrist.md](../05-zobrist.md)) gives you the
contract and the checklist, and — by the tutorial's rule — withholds the
integration. This paper is the opt-in other half: the same slice as a
red→green sequence, each step with the test you write first, the smallest
code that makes it pass, and the gate. The last section is the **complete
reference solution** plus the exact hunks for `board.rs`.

Why *this* slice gets a plan: the concept is already covered in
[01 — what Zobrist hashing is good for](01-what-zobrist-hashing-is-good-for.md).
What is left is a short list of places to be exactly right — the
side-to-move convention, the stride-15/16 boundary, the undo color — and
exactly-right is what a plan should absorb.

**Verified before written.** Every step below was executed in a scratch
copy of the engine: **47 unit tests + 2 differential properties** green,
`clippy --all-targets --features testutil -- -D warnings` and
`fmt --check` clean, the differential suite green at
`PROPTEST_CASES=10000` and `100000`, and the roundtrip property green at
its hard-coded 10,000 walks. `lib.rs` was not touched — it already
declares `mod zobrist;`.

---

## Step 0 — before you write anything

- Slice 4 is done: staged shift-AND win detection integrated into
  `Board::play`, differential suite green.
- `zobrist.rs` already exists as a two-line stub and `lib.rs` already
  has `mod zobrist;`. You are filling in a file, not wiring a module.
- `Move::index()` returns the *logical* stride-15 cell (`r * 15 + c`),
  while `bitboard::idx(r, c)` returns the *physical* stride-16 index.
  This slice lives on that boundary.
- Baseline: `cargo test -p engine --features testutil` → 40 unit + 2
  differential.

## The build order

| step | behaviour it adds | new code |
|---|---|---|
| 1 | a compile-time table of 451 random u64s | `ZOBRIST`, `SIDE_TO_MOVE` |
| 2 | a from-scratch key and the side-to-move convention | `compute_key`, `stone`, `row` |
| 3 | `Board` carries a key and exposes it | `board.rs`: field, init, `zobrist()` |
| 4 | `play` updates the key | two XOR lines |
| 5 | `undo` un-updates it | two XOR lines |
| 6 | the two properties (incremental, roundtrip) | none |
| 7 | gates + commit | — |

Steps 1–3 are pure additions with no behaviour to break; the first step
that can *lie* is step 4, and step 5 is where the classic bug lives. The
test order is chosen so that every lie has exactly one red test naming it.

## Step 1 — the table, and a health check for the generator

```rust
#[test]
fn table_has_no_zeros_or_duplicates() {
    let mut seen = HashSet::with_capacity(451);
    for entry in ZOBRIST.into_iter().flatten().chain([SIDE_TO_MOVE]) {
        assert!(entry != 0, "zero table entry");
        assert!(seen.insert(entry), "duplicate table entry {entry:#x}");
    }
}
```

RED: `cannot find value 'ZOBRIST' in this scope`.

GREEN — the chapter's `const fn` generator, plus one refinement: make
`SIDE_TO_MOVE` the **451st value of the same stream** rather than a
second seed. Then "no duplicates" covers it too, and there is exactly
one seed to document:

```rust
const fn xorshift(mut x: u64) -> u64 {
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

pub(crate) const ZOBRIST: [[u64; 225]; 2] = {
    let mut table = [[0u64; 225]; 2];
    let mut state = SEED;
    let mut i = 0;
    while i < 450 {
        state = xorshift(state);
        table[i / 225][i % 225] = state;
        i += 1;
    }
    table
};

pub(crate) const SIDE_TO_MOVE: u64 = {
    let mut state = SEED;
    let mut i = 0;
    while i <= 450 {
        state = xorshift(state);
        i += 1;
    }
    state
};
```

Two measured notes on this step:

- Write the health test as a *chained* loop over all 451 values, not as
  a table loop plus `assert!(SIDE_TO_MOVE != 0)` on the side. The latter
  is a constant assertion and clippy's `assertions_on_constants` fires
  under `-D warnings`. (Hit on the first run of this plan.)
- Why test for zeros and duplicates at all? Not because random u64s
  collide (2⁻⁶⁴ per pair) — because a *broken generator* produces them
  deterministically. xorshift's one absorbing state is 0; a typo'd
  shift constant is how you get there. The test is a generator test,
  not a probability statement.

## Step 2 — `compute_key` and the one convention you must state

The test pins the convention *before* the implementation exists:

```rust
#[test]
fn key_depends_on_side_to_move() {
    let black = Bitboard::EMPTY.with_bit(idx(7, 7));
    let white = Bitboard::EMPTY.with_bit(idx(0, 0));
    let k_black = compute_key(&black, &white, Color::Black);
    let k_white = compute_key(&black, &white, Color::White);
    assert_ne!(k_black, k_white);
    assert_eq!(k_black ^ k_white, SIDE_TO_MOVE);
}
```

RED: `cannot find function 'compute_key'`.

GREEN:

```rust
/// The one place that maps a color to its table row.
fn row(color: Color) -> usize {
    match color {
        Color::Black => 0,
        Color::White => 1,
    }
}

/// Table entry for a stone of `color` on logical (stride-15) `cell`.
pub(crate) fn stone(color: Color, cell: usize) -> u64 {
    ZOBRIST[row(color)][cell]
}

/// Convention: `SIDE_TO_MOVE` is set iff WHITE is to move, so an empty
/// board (Black to move) has key 0.
pub(crate) fn compute_key(black: &Bitboard, white: &Bitboard, to_move: Color) -> u64 {
    let mut key = match to_move {
        Color::Black => 0,
        Color::White => SIDE_TO_MOVE,
    };
    for r in 0..15u8 {
        for c in 0..15u8 {
            let i = idx(r, c); // stride-16 bitboard index
            let cell = r as usize * 15 + c as usize; // logical cell
            if black.test(i) {
                key ^= stone(Color::Black, cell);
            }
            if white.test(i) {
                key ^= stone(Color::White, cell);
            }
        }
    }
    key
}
```

Three decisions to notice, because every later step leans on them:

1. **The convention**: `SIDE_TO_MOVE` set ⇔ White to move ⇔ empty
   board has key `0`. Either polarity works; this one makes
   `Board::new()` start at zero and matches "flip on every play".
2. **`stone()` owns the color→row mapping.** `Board::play` will need the
   same mapping; if it matches on `Color` itself, the mapping lives in
   two places and can be flipped in one.
3. **The stride boundary is crossed exactly once**, in this loop: `idx`
   (stride 16) for `test`, `r * 15 + c` (stride 15) for the table.
   Incremental updates never cross it — `Move` already carries the
   logical index. If you ever index `ZOBRIST` with a stride-16 value
   you read another cell's entry *silently*; 225 < 240 means no panic.

## Step 3 — `Board` carries a key

```rust
#[test]
fn new_board_key_is_zero() {
    assert_eq!(Board::new().zobrist(), 0);
    assert_eq!(Board::new().zobrist(), from_scratch(&Board::new()));
}
```

(`from_scratch` is a small test helper: `compute_key(&b.stones(Black),
&b.stones(White), b.to_move())`. `stones` is already `pub(crate)`.)

RED: `no method named 'zobrist'`.

GREEN, three hunks: a `key: u64` field, `key: 0` in `new()` (with a
comment naming the convention — zero is a *consequence*, not a choice),
and the getter. Do not be tempted to compute the key lazily in
`zobrist()`: the whole point of the field is O(1) after O(1) updates,
and slice 7's tactics + the replay buffer will call it per node.

## Step 4 — `play` updates the key (and the vacuous-equality trap)

```rust
/// THE point of Zobrist: position identity, not path identity.
#[test]
fn different_move_orders_reaching_the_same_position_agree() {
    let order_a = [(7, 7), (0, 0), (7, 8), (0, 1), (8, 7)];
    let order_b = [(8, 7), (0, 1), (7, 8), (0, 0), (7, 7)];
    let mut a = Board::new();
    let mut b = Board::new();
    for &(r, c) in &order_a {
        a.play(Move::new(r, c).unwrap()).unwrap();
    }
    for &(r, c) in &order_b {
        b.play(Move::new(r, c).unwrap()).unwrap();
    }
    assert_eq!(a.stones(Color::Black), b.stones(Color::Black)); // same position,
    assert_eq!(a.stones(Color::White), b.stones(Color::White)); // reached differently
    assert_eq!(a.zobrist(), b.zobrist());
    assert_eq!(a.zobrist(), from_scratch(&a));
}
```

RED — and here is the trap: **`assert_eq!(a.zobrist(), b.zobrist())`
passes vacuously if the key never moves** (0 == 0). The assert that
actually goes red is the last one, against `from_scratch`. When you run
this step, confirm the failure is `left: 0, right: <nonzero>` — if the
test is green *before* you touch `play`, it is green for the wrong
reason. (The two `stones` asserts matter for the same reason: without
them, two *different* positions with colliding paths would "pass".)

GREEN, in `play`, immediately before the `to_move` flip:

```rust
self.key ^= crate::zobrist::stone(self.to_move, mv.index());
self.key ^= crate::zobrist::SIDE_TO_MOVE;
self.to_move = self.to_move.other();
```

Placement is the whole game:

- **After** the legality checks, so a rejected play leaves the key
  untouched (the step-6 proptest pins this explicitly).
- **Before** the flip, so `self.to_move` is still the *mover's* color.
- `mv.index()`, not the local `i` — `i` is stride-16, the table is 225.

## Step 5 — `undo` (walk it by hand first)

Before coding, take the chapter's advice literally. One example on
paper: Black plays cell X, White plays cell Y, undo once. Track the key
through each XOR: which color's entry removes Y's stone — the `to_move`
at entry to `undo` (White-to-move state? no — Black just... *walk it*),
or its `other()`? Two minutes here is the whole slice.

```rust
#[test]
fn undo_everything_restores_the_initial_key() {
    let mut b = Board::new();
    let script = [(5, 6), (5, 7), (4, 6), (3, 6)];
    for &(r, c) in &script {
        b.play(Move::new(r, c).unwrap()).unwrap();
    }
    for _ in script {
        b.undo();
    }
    assert_eq!(b.zobrist(), Board::new().zobrist());
}
```

RED: `left: <nonzero>, right: 0`.

GREEN, in `undo`, between the stone removal and the flip back:

```rust
// XOR the stone back out with the color of the stone BEING
// REMOVED: before the flip back, that is to_move.other().
self.key ^= crate::zobrist::stone(self.to_move.other(), mv.index());
self.key ^= crate::zobrist::SIDE_TO_MOVE;
self.to_move = self.to_move.other(); // flip back
```

Free regression you did not write: `Board` derives `PartialEq`, and the
slice-3 test `undo_everything_returns_a_pristine_board` compares whole
boards — with the `key` field added, that test silently became a key
roundtrip too.

## Step 6 — the two properties

**Incremental.** Random plies; after every *accepted* play the
incremental key must equal from-scratch, and after every *rejected* one
it must be unchanged:

```rust
proptest! {
    #[test]
    fn incremental_matches_from_scratch(cells in prop::collection::vec(0u16..225, 1..=60)) {
        let mut b = Board::new();
        for p in cells {
            let mv = Move::new((p / 15) as u8, (p % 15) as u8).unwrap();
            let before = b.zobrist();
            if b.play(mv).is_ok() {
                prop_assert_eq!(b.zobrist(), from_scratch(&b));
            } else {
                prop_assert_eq!(b.zobrist(), before, "failed play changed the key");
            }
        }
    }
}
```

**Roundtrip — the real test.** Random play/undo walks (the same
`(cell, coin)` shape as the differential `undo_walks_match_naive`),
from-scratch checked at *every* step, hard-coded to 10,000 cases
because the slice's "Done when" says so:

```rust
proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn roundtrip_play_undo_walks(
        ops in prop::collection::vec((0u16..225, 0u8..3), 1..=100)
    ) {
        let mut b = Board::new();
        for (p, coin) in ops {
            if coin == 0 && !b.moves().is_empty() {
                b.undo();
            } else {
                let mv = Move::new((p / 15) as u8, (p % 15) as u8).unwrap();
                let _ = b.play(mv); // may be Occupied/GameOver — fine
            }
            prop_assert_eq!(b.zobrist(), from_scratch(&b));
        }
    }
}
```

Why both? `incremental_matches_from_scratch` only ever moves *forward*;
it cannot see `undo` at all. The roundtrip is the only test that
exercises the undo XORs under variation — and the undo color is the
classic bug of this slice. If you want to feel the test's teeth before
trusting it, flip `to_move.other()` to `to_move` in `undo` and watch it
go red (revert afterwards, obviously).

Note that `undo` after a won game is legal in this engine (it resets
`status` to `Ongoing`), so the walk needs no status guard on the undo
branch — the key algebra does not care about status either.

## Step 7 — gates, evidence, commit

```bash
cargo fmt --all
cargo clippy -p engine --all-targets --features testutil -- -D warnings
cargo test  -p engine --features testutil --no-fail-fast
PROPTEST_CASES=10000 cargo test -p engine --features testutil
```

Commit: `feat(engine): incremental Zobrist keys` (the slice's own
message). Measured on the reference solution below: 47 unit + 2
differential green, roundtrip green at its hard-coded 10k walks,
differential green at 10k and 100k cases, clippy and fmt clean,
`lib.rs` unchanged.

One lint you *will* meet: in a non-test build nothing calls
`compute_key` yet, so `-D warnings` fails on dead code. Same situation
as slice 4's `count` — keep exactly one narrow `#[allow(dead_code)]`
with a comment naming the next real consumer (the from-scratch
validation assertion, ch. 13). Do not delete the function; it is the
ground truth both proptests exist to serve.

---

## Reference solution

Assembled from the steps above; this is byte-for-byte what was verified.

### `crates/engine/src/zobrist.rs` — complete

```rust
//! Zobrist keys: a 64-bit position fingerprint, updated in O(1) per
//! move and undone for free. Slice 5. See docs/13-engine-design.md,
//! "Zobrist, compile-time".
//!
//! The table is built by a `const fn` at compile time — no `rand`
//! dependency, zero runtime initialization, and the same keys on every
//! machine forever (replay-buffer dedup needs cross-run stability).

use crate::bitboard::{Bitboard, idx};
use crate::board::Color;

const fn xorshift(mut x: u64) -> u64 {
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// `[color][cell] -> random u64`, where `cell` is the LOGICAL cell
/// (`Move::index()`, stride 15) — 225 entries per color, not 240.
/// The stride-15/stride-16 conversion happens once, in `compute_key`;
/// incremental updates never see a stride-16 index because `Move`
/// already carries the logical one.
pub(crate) const ZOBRIST: [[u64; 225]; 2] = {
    let mut table = [[0u64; 225]; 2];
    let mut state = SEED;
    let mut i = 0;
    while i < 450 {
        state = xorshift(state);
        table[i / 225][i % 225] = state;
        i += 1;
    }
    table
};

/// Flipped on every `play` AND every `undo`. The 451st value of the
/// same stream, so it cannot collide with a stone entry either.
pub(crate) const SIDE_TO_MOVE: u64 = {
    let mut state = SEED;
    let mut i = 0;
    while i <= 450 {
        state = xorshift(state);
        i += 1;
    }
    state
};

/// The one place that maps a color to its table row.
fn row(color: Color) -> usize {
    match color {
        Color::Black => 0,
        Color::White => 1,
    }
}

/// Table entry for a stone of `color` on logical (stride-15) `cell`.
pub(crate) fn stone(color: Color, cell: usize) -> u64 {
    ZOBRIST[row(color)][cell]
}

/// From-scratch key — the ground truth the incremental key must match.
///
/// Convention: `SIDE_TO_MOVE` is set iff WHITE is to move, so an empty
/// board (Black to move) has key 0. `Board::play` flips `SIDE_TO_MOVE`
/// on every move, which tracks exactly this.
///
/// Only tests consume this today; the debug assertion that two paths
/// to the same position agree (ch. 13) is the next real consumer.
#[allow(dead_code)]
pub(crate) fn compute_key(black: &Bitboard, white: &Bitboard, to_move: Color) -> u64 {
    let mut key = match to_move {
        Color::Black => 0,
        Color::White => SIDE_TO_MOVE,
    };
    for r in 0..15u8 {
        for c in 0..15u8 {
            let i = idx(r, c); // stride-16 bitboard index
            let cell = r as usize * 15 + c as usize; // logical cell
            if black.test(i) {
                key ^= stone(Color::Black, cell);
            }
            if white.test(i) {
                key ^= stone(Color::White, cell);
            }
        }
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;
    use crate::moveset::Move;
    use proptest::prelude::*;
    use std::collections::HashSet;

    fn from_scratch(b: &Board) -> u64 {
        compute_key(
            &b.stones(Color::Black),
            &b.stones(Color::White),
            b.to_move(),
        )
    }

    /// 450 random u64s (plus SIDE_TO_MOVE): a zero or a duplicate would
    /// signal a broken generator, not bad luck — at 2^-64 per event,
    /// chance is not on the menu.
    #[test]
    fn table_has_no_zeros_or_duplicates() {
        let mut seen = HashSet::with_capacity(451);
        for entry in ZOBRIST.into_iter().flatten().chain([SIDE_TO_MOVE]) {
            assert!(entry != 0, "zero table entry");
            assert!(seen.insert(entry), "duplicate table entry {entry:#x}");
        }
    }

    /// The side-to-move convention, pinned: same stones, keys differ by
    /// exactly SIDE_TO_MOVE.
    #[test]
    fn key_depends_on_side_to_move() {
        let black = Bitboard::EMPTY.with_bit(idx(7, 7));
        let white = Bitboard::EMPTY.with_bit(idx(0, 0));
        let k_black = compute_key(&black, &white, Color::Black);
        let k_white = compute_key(&black, &white, Color::White);
        assert_ne!(k_black, k_white);
        assert_eq!(k_black ^ k_white, SIDE_TO_MOVE);
    }

    #[test]
    fn new_board_key_is_zero() {
        assert_eq!(Board::new().zobrist(), 0);
        assert_eq!(Board::new().zobrist(), from_scratch(&Board::new()));
    }

    /// THE point of Zobrist: position identity, not path identity.
    #[test]
    fn different_move_orders_reaching_the_same_position_agree() {
        let order_a = [(7, 7), (0, 0), (7, 8), (0, 1), (8, 7)];
        let order_b = [(8, 7), (0, 1), (7, 8), (0, 0), (7, 7)];
        let mut a = Board::new();
        let mut b = Board::new();
        for &(r, c) in &order_a {
            a.play(Move::new(r, c).unwrap()).unwrap();
        }
        for &(r, c) in &order_b {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(a.stones(Color::Black), b.stones(Color::Black)); // same position,
        assert_eq!(a.stones(Color::White), b.stones(Color::White)); // reached differently
        assert_eq!(a.zobrist(), b.zobrist());
        assert_eq!(a.zobrist(), from_scratch(&a));
    }

    /// The roundtrip, corpus-sized: play four, undo four, land exactly
    /// on the initial key. (Board's derived PartialEq already covers
    /// this implicitly; this pins the key explicitly.)
    #[test]
    fn undo_everything_restores_the_initial_key() {
        let mut b = Board::new();
        let script = [(5, 6), (5, 7), (4, 6), (3, 6)];
        for &(r, c) in &script {
            b.play(Move::new(r, c).unwrap()).unwrap();
        }
        for _ in script {
            b.undo();
        }
        assert_eq!(b.zobrist(), Board::new().zobrist());
    }

    proptest! {
        /// Incremental key == from-scratch key after every ply of random
        /// games. Rejected plays (occupied, game over) must leave the
        /// key untouched.
        #[test]
        fn incremental_matches_from_scratch(cells in prop::collection::vec(0u16..225, 1..=60)) {
            let mut b = Board::new();
            for p in cells {
                let mv = Move::new((p / 15) as u8, (p % 15) as u8).unwrap();
                let before = b.zobrist();
                if b.play(mv).is_ok() {
                    prop_assert_eq!(b.zobrist(), from_scratch(&b));
                } else {
                    prop_assert_eq!(b.zobrist(), before, "failed play changed the key");
                }
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10_000))]

        /// THE roundtrip property: random play/undo walks, key checked
        /// against from-scratch at EVERY step. Catches the classic bug
        /// (XORing the wrong color's entry on undo) that play-only
        /// tests cannot.
        #[test]
        fn roundtrip_play_undo_walks(
            ops in prop::collection::vec((0u16..225, 0u8..3), 1..=100)
        ) {
            let mut b = Board::new();
            for (p, coin) in ops {
                if coin == 0 && !b.moves().is_empty() {
                    b.undo();
                } else {
                    let mv = Move::new((p / 15) as u8, (p % 15) as u8).unwrap();
                    let _ = b.play(mv); // may be Occupied/GameOver — fine
                }
                prop_assert_eq!(b.zobrist(), from_scratch(&b));
            }
        }
    }
}
```

### `crates/engine/src/board.rs` — five hunks

```diff
--- a/crates/engine/src/board.rs
+++ b/crates/engine/src/board.rs
@@ -48,6 +48,7 @@
     to_move: Color,
     status: Status,
     moves: Vec<Move>,
+    key: u64,
 }
 
 impl Board {
@@ -58,6 +59,9 @@
             to_move: Color::Black,
             status: Status::Ongoing,
             moves: Vec::new(),
+            // Empty board, Black to move — the compute_key convention
+            // (SIDE_TO_MOVE set iff White to move) makes this 0.
+            key: 0,
         }
     }
 
@@ -81,6 +85,8 @@
             self.status = Status::Draw;
         }
 
+        self.key ^= crate::zobrist::stone(self.to_move, mv.index());
+        self.key ^= crate::zobrist::SIDE_TO_MOVE;
         self.to_move = self.to_move.other();
 
         Ok(())
@@ -94,6 +100,11 @@
         self.to_move
     }
 
+    /// The incremental Zobrist key of the current position.
+    pub fn zobrist(&self) -> u64 {
+        self.key
+    }
+
     pub fn moves(&self) -> &[Move] {
         &self.moves
     }
@@ -109,6 +120,10 @@
             Color::Black => self.black = self.black.without_bit(i),
             Color::White => self.white = self.white.without_bit(i),
         }
+        // XOR the stone back out with the color of the stone BEING
+        // REMOVED: before the flip back, that is to_move.other().
+        self.key ^= crate::zobrist::stone(self.to_move.other(), mv.index());
+        self.key ^= crate::zobrist::SIDE_TO_MOVE;
         self.to_move = self.to_move.other(); // flip back
         // Undoing the winning move un-wins the game; undoing into a
         // would-be draw likewise reopens it. Ongoing is always right.
```

### Verification log

```text
cargo fmt --all --check                                     clean
cargo clippy -p engine --all-targets --features testutil -- -D warnings
                                                            clean
cargo test -p engine --features testutil --no-fail-fast     47 passed + 2 passed
PROPTEST_CASES=10000  ... same                              47 passed + 2 passed
PROPTEST_CASES=100000 ... differential only                 2 passed
```

The 47 = the 40 from slices 2–4 plus the seven in `zobrist.rs`:
`table_has_no_zeros_or_duplicates`, `key_depends_on_side_to_move`,
`new_board_key_is_zero`,
`different_move_orders_reaching_the_same_position_agree`,
`undo_everything_restores_the_initial_key`,
`incremental_matches_from_scratch`, `roundtrip_play_undo_walks`
(the last at a hard-coded 10,000 cases). The 2 differential properties
are unchanged — they do not compare keys, but they drive `Board`
through thousands of play/undo walks, so any key update that corrupted
board state would still surface there.

## What this slice does not do

- **No transposition table, no transposition merging in MCTS** — the
  locked decision (ch. 13); deep-dive 01 explains why AlphaZero-style
  MCTS shelves the most famous use of Zobrist keys. The keys earn their
  keep at replay-buffer dedup, test identity, and from-scratch
  validation.
- **No dedup wiring** — the replay buffer lives in the training
  application, not the engine; it becomes a consumer of `zobrist()`
  much later.
- **No use of the key in `PartialEq`** — `Board`'s derived equality
  compares stones and key independently; key equality is never
  *substituted* for position equality (collisions are astronomically
  unlikely, not impossible — and tests should say what they mean).
