# 02 — Slice 4 implementation plan (with reference solution)

The slice file ([../04-win-detection.md](../04-win-detection.md)) gives you
the contract and the checklist, and — by the tutorial's rule — withholds
the implementation. This paper is the opt-in other half: the same slice as
a red→green sequence, each step with the test you write first, the smallest
code that makes it pass, and the gate. The last section is the **complete
reference solution** plus the exact hunks for `board.rs` and `bitboard.rs`.

Why *this* slice gets a plan: the thinking is already done in
[01 — the staged AND](01-staged-and-and-no-edge-masks.md). What is left is
mechanical, and mechanical is exactly what a plan should absorb.

**Verified before written.** Every step below was executed in a scratch copy
of the engine: **38 unit tests + 2 differential properties** green,
`clippy --all-targets --features testutil -- -D warnings` and `fmt --check`
clean, and the differential suite green at `PROPTEST_CASES=10000` and
`100000`. `lib.rs` was not touched — slice 3 already declared `mod win;`.

---

## Step 0 — before you write anything

- Slice 3 is done: `Bitboard` with `test`/`with_bit`/`without_bit`/`shr`/
  `is_zero`/`count`, `VALID`, `assert_clean`, and the differential suite
  green.
- `bitboard.rs` carries three `#[allow(dead_code)]` on `is_zero`, `count`,
  `shr`, with a comment saying slice 4 will consume them. That comment is
  your to-do list for step 8.
- `win.rs` already exists as the two-line stub and `lib.rs` already has
  `mod win;`. You are filling in a file, not wiring a module.
- Baseline: `cargo test -p engine --features testutil` → 31 unit + 2
  differential (2 Move + 16 reference + 7 bitboard + 6 board).

## The build order

| step | behaviour it adds | new code |
|---|---|---|
| 1 | a horizontal five is found | `has_five_dir` (staged) + `has_five_any` |
| 2 | all four directions | three more strides in `DIRS` |
| 3 | fives on every edge and corner | none — the padding invariant does it |
| 4 | overlines count; fours and broken fives do not | none |
| 5 | the wrap attack through `Board` | none |
| 6 | planted runs and random sets agree with the walk-based oracle | none |
| 7 | `Board::play` uses the new detector; the walk-based oracle is deleted | `board.rs` |
| 8 | the dead-code allows retire | `bitboard.rs` |
| 9 | gates + commit | — |
| 10 | *optional:* neighbourhood check — 2× on the play path | `win.rs`, `board.rs` |

Steps 3–6 are test-only on purpose. In a bit-trick slice most of the work is
*proving* the shape you already derived; those tests are the ones that fail
first if `shr`, `VALID` or `DIRS` are ever touched again.

## Step 1 — one five, and the shape that finds it

```rust
fn stones(cells: &[(u8, u8)]) -> Bitboard {
    cells.iter().fold(Bitboard::EMPTY, |b, &(r, c)| b.with_bit(idx(r, c)))
}

#[test]
fn finds_a_horizontal_five() {
    assert!(has_five_any(&stones(&[(7, 3), (7, 4), (7, 5), (7, 6), (7, 7)])));
}
```

RED: `cannot find function 'has_five_any' in this scope`.

(Step 2 folds this case into the four-direction table. Keep both if you like;
the reference solution keeps only the table.)

GREEN — the smallest *correct* implementation, staged from the first line:

```rust
pub(crate) const DIRS: [u32; 4] = [1, 16, 15, 17];

pub(crate) fn has_five_dir(b: &Bitboard, s: u32) -> bool {
    let two = *b & b.shr(s);          // bit i: stones at i and i+s
    let four = two & two.shr(2 * s);  // bit i: stones at i .. i+3s
    let five = four & four.shr(s);    // bit i: stones at i .. i+4s
    !five.is_zero()
}

pub(crate) fn has_five_any(b: &Bitboard) -> bool {
    DIRS.iter().any(|&s| has_five_dir(b, s))
}
```

(If you want an even smaller red state, start `DIRS` as `[1]` and let step 2
add the rest. It changes nothing but the size of the first failure.)

Do **not** "start simple" with the five-term AND. It compiles, it passes this
test, and in release it silently lies for `s = 16` and `s = 17` — see
[01, section 2](01-staged-and-and-no-edge-masks.md). There is no version of
this function worth writing twice.

Gate: `cargo test -p engine --features testutil win::`

## Step 2 — the other three directions

Table-driven, one row per direction, each with a name so a failure names
itself:

```rust
let cases: &[(&str, &[(u8, u8)])] = &[
    ("horizontal", &[(7, 3), (7, 4), (7, 5), (7, 6), (7, 7)]),
    ("vertical", &[(2, 9), (3, 9), (4, 9), (5, 9), (6, 9)]),
    ("diag down-right", &[(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)]),
    ("diag down-left", &[(1, 13), (2, 12), (3, 11), (4, 10), (5, 9)]),
];
for (name, cells) in cases {
    assert!(has_five_any(&stones(cells)), "{name}");
}
```

Code change: `DIRS` gains `16, 15, 17`. If a diagonal fails, the *stride* is
wrong, not the algorithm: 15 and 17 are the two diagonals in stride-16
indexing, and the direction table is stride-specific (paper 01, section 4).

## Step 3 — edges and corners (where you expect masks and need none)

Rows 0 and 14, columns 0 and 14, both corner diagonals — one table, no new
code, green on the first run. This is the test that *proves* the padding
invariant is doing the edge work. If it ever fails, look at `VALID` and
`assert_clean`, not at `has_five`.

## Step 4 — overlines and near-misses

Six in a row → true. Nine in a row → true (decision 1: overlines count).
Four in a row → false. Five with a gap → false. Empty board → false. All 225
cells (`VALID`) → true — that last one is cheap and catches a `shr` that
drops bits at a word boundary.

## Step 5 — the wrap attack, at both levels

The phantom five lives on ONE colour's bitboard: a single colour must hold
the whole seam pattern — `(7,12)`, `(7,13)`, `(7,14)` and `(8,0)`, `(8,1)` —
so that the `+1` index path `124 → 125 → 126 → 127 → 128` could connect if
padding were not zero. Two tests pin this.

Bitboard level (where the claim lives):

```rust
#[test]
fn wrap_pattern_is_not_a_five() {
    let seam = stones(&[(7, 12), (7, 13), (7, 14), (8, 0), (8, 1)]);
    assert!(!has_five_any(&seam));
}
```

Board level (the regression sentinel): through alternating play one colour
can still own the whole pattern — the opponent fills distant cells. Black
ends with exactly the seam shape; White's four fillers are a deliberate
4-run, not a win:

```rust
#[test]
fn wrap_attack_is_not_a_win() {
    let mut board = Board::new();
    let script = [
        (7, 12), (0, 0),
        (7, 13), (0, 1),
        (7, 14), (0, 2),
        (8, 0),  (0, 3),
        (8, 1),
    ];
    for (r, c) in script {
        board.play(Move::new(r, c).unwrap()).unwrap();
    }
    assert_eq!(board.status(), Status::Ongoing);
}
```

Splitting the seam cells between the two colours instead — Black
`(7,12)/(7,13)/(7,14)`, White `(8,0)/(8,1)` — would make this test vacuous:
no single bitboard ever contains the pattern, so even a padding-ignorant
`has_five` would pass. (An earlier version of this plan had exactly that
script; if your copy does, replace it.)

Honest caveat: through `play` alone even the fixed test cannot fail —
`Move` only addresses columns 0–14, and the staged AND re-includes the
unshifted, invariant-holding operand at every stage
([03-deep-dive/01, §6](../03-deep-dive/01-stride16-and-shr.md)). The
sentinel fires when code that writes bits *directly* — an unmasked `!`, a
future symmetry transform, `from_position` — violates the invariant.

## Step 6 — the two properties

**Planted runs.** Sample a direction and a start cell *inside the window where
the run fits*; assert four stones are not a five, five stones are, and the
oracle agrees.

**Random sets.** `hash_set(0u8..225, 0..30)`, converted through
`stride-16 = (i / 15) * 16 + i % 15`, asserting the bit trick agrees with the
walk-based oracle on every set.

The oracle is the slice-3 walk-based detector — **keep it in the test
module** after step 7 deletes it from `board.rs`. Two implementations
that disagree are a bug report.

Pitfall, measured: an early version sampled `r0`/`c0` freely and used
`prop_assume!` to discard runs that fell off the board. At 10k cases:

```text
Test aborted: Too many global rejects
successes: 1817, global rejects: 1024
    1024 times at ...: cells.iter().all(|&(r, c)| r < 15 && c < 15)
```

~45% of draws were rejected and proptest's cap fired long before the property
did anything useful. Map the sample into the legal window instead —
`(r0, c0) = (a, b + 4 * u8::from(dc != 1))` with `a, b in 0..11` — zero
rejections, same coverage of the interesting cases.

## Step 7 — integration: `play` switches, the walk-based oracle is deleted

`board.rs` answers "did that win?" today with its own `DIRS`, `count_walk` and
`wins_from`. Replace the call and delete all three:

```diff
-        if self.wins_from(i, self.to_move) {
+        if crate::win::has_five_any(&self.stones(self.to_move)) {
```

Only the mover's stones are tested, because a win always involves the stone
just placed (chapter 13: "`Board::play` checks only the color just placed").
The `i` binding stays — the occupancy test and `with_bit(i)` still need it.

Gate: `cargo test -p engine --features testutil --no-fail-fast`. From this
point the **differential suite is testing `win.rs` against the oracle**
over random play/undo walks, the strongest check in the crate. Run it
wider: `PROPTEST_CASES=10000 cargo test -p engine --features testutil`.

## Step 8 — retire the dead-code allows

Delete the comment block and the three attributes, then let the compiler tell
you what is still unused. Measured outcome: `shr` and `is_zero` are consumed
by `win.rs`; `count` is used only by tests and assertions, so keep **exactly
one** narrow allow on it, with a comment naming the next real consumer.
(The alternative — `#[cfg(test)]` on `count` — is honest but gets moved back
the day a non-test caller wants it.)

Gate: `cargo clippy -p engine --all-targets --features testutil -- -D warnings`
must be silent.

## Step 9 — gates, evidence, commit

```bash
cargo fmt --all
cargo clippy -p engine --all-targets --features testutil -- -D warnings
cargo test  -p engine --features testutil --no-fail-fast
PROPTEST_CASES=10000 cargo test -p engine --features testutil
```

Commit: `feat(engine): staged shift-AND win detection` (the slice's own
message). Measured on the reference solution below: 38 unit + 2 differential
green at 1 / 10k / 100k cases, clippy and fmt clean, `lib.rs` unchanged.

## Optional step 10 — the neighbourhood check (2× on the play path)

Not in the slice contract; take it only if you want the `play` path faster.
Measured (paper 01, section 7): whole-board `has_five_any` **8.25 ns**, the
neighbourhood check **3.96 ns** per call.

```rust
/// Does the stone at `i` complete a line of five? Asks only about lines
/// through `i`. Precondition: `b` contains `i`, and `i` is the only
/// difference from the previous position — true for `Board::play`, NOT for
/// `undo`, symmetry transforms, or oracle replay.
pub(crate) fn wins_by_placing(b: &Bitboard, i: usize) -> bool {
    fn step(s: u32) -> (i32, i32) {
        match s {
            1 => (0, 1),
            16 => (1, 0),
            15 => (1, -1),
            17 => (1, 1),
            _ => unreachable!("direction is one of DIRS"),
        }
    }
    let (r, c) = ((i / 16) as i32, (i % 16) as i32);
    for &s in &DIRS {
        let (dr, dc) = step(s);
        let mut run = 1; // the stone at i itself
        for sign in [-1i32, 1] {
            let mut k = 1;
            loop {
                let (rr, cc) = (r + sign * dr * k, c + sign * dc * k);
                if !(0..15).contains(&rr) || !(0..15).contains(&cc) {
                    break;
                }
                if !b.test(idx(rr as u8, cc as u8)) {
                    break;
                }
                run += 1;
                k += 1;
            }
        }
        if run >= 5 {
            return true;
        }
    }
    false
}
```

Wire it in as `if crate::win::wins_by_placing(&self.stones(self.to_move), i)`,
and pin it with an equivalence test — assert, on every move of a scripted
game, that the neighbourhood check's answer equals
`board.status() == Status::Won(mover)`. The check is only legitimate if it
is *exactly* equivalent on `play`.

Consequence to expect: with `play` on the neighbourhood check, `has_five_any`
has no caller until MCTS asks "is this leaf terminal?", so it needs a narrow
allow with that comment. Both configurations compile and are clippy-clean —
but exactly one of
the two detectors should be the one `play` calls.

## Optional step 11 — slice 9's benchmark (not now)

Do not optimize before measuring, and do not write the bench here: slice 9
owns it, and [01, section 8](01-staged-and-and-no-edge-masks.md) documents the
two ways a criterion bench of this function goes wrong (loop-invariant code
motion; `black_box` per call).

---

## Reference solution

Assembled from the steps above; this is byte-for-byte what was verified.

### `crates/engine/src/win.rs` — complete

```rust
//! Win detection: staged two/four/five shift-AND, overlines count.
//! Slice 4. See docs/13-engine-design.md, "Win detection".

use crate::bitboard::Bitboard;

/// Index strides of the four line directions. These are *stride-16*
/// numbers, not geometry: with a different stride the same visual
/// directions would be different numbers.
pub(crate) const DIRS: [u32; 4] = [1, 16, 15, 17];

/// True if `b` contains five or more consecutive stones in the
/// direction whose index stride is `s`.
///
/// Staged on purpose: `4 * s` reaches 68 for `s = 17`, and a u64 shift
/// of >= 64 is MASKED, not rejected — the naive five-term AND answers
/// wrongly (silently, in release) for `s = 16` and `s = 17`. Doubling
/// keeps every shift written here at most `2 * s = 34` while the
/// information still travels the full `4 * s`.
///
/// Needs no edge masks: with the padding invariant (column 15 of every
/// row is zero) every wrap path crosses a zero bit and the AND dies.
/// Do not feed it a board built with `!` unless you masked with `VALID`.
pub(crate) fn has_five_dir(b: &Bitboard, s: u32) -> bool {
    let two = *b & b.shr(s); // bit i: stones at i and i+s
    let four = two & two.shr(2 * s); // bit i: stones at i .. i+3s
    let five = four & four.shr(s); // bit i: stones at i .. i+4s
    !five.is_zero()
}

/// True if `b` contains five or more in a row in any direction.
///
/// "Five or more": a six-run contains a five-run, so overlines count
/// (chapter 13, decision 1) at no extra cost. The result is a boolean
/// on purpose — `five`'s popcount is a count of 5-windows, which grows
/// with run length and is not a number of wins.
pub(crate) fn has_five_any(b: &Bitboard) -> bool {
    DIRS.iter().any(|&s| has_five_dir(b, s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitboard::{VALID, idx};
    use crate::board::{Board, Status};
    use crate::moveset::Move;
    use proptest::prelude::*;

    /// The slice-3 walk-based detector, kept here as the oracle: it walks
    /// cells, so it is immune to the bit-level traps this module lives on.
    fn count_walk(b: &Bitboard, s: u32) -> bool {
        let (dr, dc) = match s {
            1 => (0i32, 1i32),
            16 => (1, 0),
            15 => (1, -1),
            17 => (1, 1),
            _ => unreachable!("direction is one of DIRS"),
        };
        for r in 0..15i32 {
            for c in 0..15i32 {
                if !b.test(idx(r as u8, c as u8)) {
                    continue;
                }
                let five = (0..5).all(|k| {
                    let (rr, cc) = (r + dr * k, c + dc * k);
                    (0..15).contains(&rr)
                        && (0..15).contains(&cc)
                        && b.test(idx(rr as u8, cc as u8))
                });
                if five {
                    return true;
                }
            }
        }
        false
    }

    fn stones(cells: &[(u8, u8)]) -> Bitboard {
        cells
            .iter()
            .fold(Bitboard::EMPTY, |b, &(r, c)| b.with_bit(idx(r, c)))
    }

    #[test]
    fn detects_all_four_directions() {
        let cases: &[(&str, &[(u8, u8)])] = &[
            ("horizontal", &[(7, 3), (7, 4), (7, 5), (7, 6), (7, 7)]),
            ("vertical", &[(2, 9), (3, 9), (4, 9), (5, 9), (6, 9)]),
            ("diag down-right", &[(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)]),
            (
                "diag down-left",
                &[(1, 13), (2, 12), (3, 11), (4, 10), (5, 9)],
            ),
        ];
        for (name, cells) in cases {
            assert!(has_five_any(&stones(cells)), "{name}");
        }
    }

    #[test]
    fn detects_fives_along_every_edge() {
        let cases: &[(&str, &[(u8, u8)])] = &[
            ("row 0, cols 0-4", &[(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)]),
            (
                "row 14, cols 10-14",
                &[(14, 10), (14, 11), (14, 12), (14, 13), (14, 14)],
            ),
            (
                "col 0, rows 10-14",
                &[(10, 0), (11, 0), (12, 0), (13, 0), (14, 0)],
            ),
            (
                "col 14, rows 0-4",
                &[(0, 14), (1, 14), (2, 14), (3, 14), (4, 14)],
            ),
            (
                "corner diagonal (0,0)-(4,4)",
                &[(0, 0), (1, 1), (2, 2), (3, 3), (4, 4)],
            ),
            (
                "corner diagonal (0,14)-(4,10)",
                &[(0, 14), (1, 13), (2, 12), (3, 11), (4, 10)],
            ),
        ];
        for (name, cells) in cases {
            assert!(has_five_any(&stones(cells)), "{name}");
        }
    }

    #[test]
    fn overlines_count_and_near_misses_do_not() {
        let six: Vec<(u8, u8)> = (3..9).map(|c| (7u8, c)).collect();
        let nine: Vec<(u8, u8)> = (0..9).map(|c| (7u8, c)).collect();
        assert!(has_five_any(&stones(&six)), "six in a row");
        assert!(has_five_any(&stones(&nine)), "nine in a row");

        assert!(
            !has_five_any(&stones(&[(7, 3), (7, 4), (7, 5), (7, 6)])),
            "four in a row"
        );
        assert!(
            !has_five_any(&stones(&[(7, 3), (7, 4), (7, 6), (7, 7), (7, 8)])),
            "five with a gap"
        );
        assert!(!has_five_any(&Bitboard::EMPTY), "empty board");
        assert!(has_five_any(&VALID), "every cell filled");
    }

    /// The wrap attack at the bitboard level: one colour holds the whole
    /// seam pattern — (7,12), (7,13), (7,14) and (8,0), (8,1). The +1
    /// index path 124..128 crosses padding bit 127, which is always
    /// zero, so this must NOT be a five.
    #[test]
    fn wrap_pattern_is_not_a_five() {
        let seam = stones(&[(7, 12), (7, 13), (7, 14), (8, 0), (8, 1)]);
        assert!(!has_five_any(&seam));
    }

    /// The wrap attack through real coordinates: alternating play, but
    /// Black owns the whole seam pattern while White fills distant
    /// cells (a deliberate 4-run, not a win). If this ever reports a
    /// win, the padding invariant is broken — not the test.
    ///
    /// Note: splitting the seam cells between the two colours would
    /// make this test vacuous — the phantom five lives on a single
    /// bitboard, so one colour must own the whole pattern.
    #[test]
    fn wrap_attack_is_not_a_win() {
        let mut board = Board::new();
        let script = [
            (7, 12),
            (0, 0),
            (7, 13),
            (0, 1),
            (7, 14),
            (0, 2),
            (8, 0),
            (0, 3),
            (8, 1),
        ];
        for (r, c) in script {
            board.play(Move::new(r, c).unwrap()).unwrap();
        }
        assert_eq!(board.status(), Status::Ongoing);
    }

    proptest! {
        /// Plant a run of five inside the window where it fits, and a
        /// four-run: the four must NOT be a five, the five must be found,
        /// and the walk-based oracle must agree.
        ///
        /// Note the sampling: `a`/`b` are mapped into the legal start
        /// window instead of sampled freely plus `prop_assume!`. Assuming
        /// away ~45% of draws aborts the test at proptest's 1024 global
        /// rejects ("Too many global rejects") long before any real bug
        /// shows up.
        #[test]
        fn planted_runs_are_found(dir in 0usize..4, a in 0u8..11, b in 0u8..11) {
            let (dr, dc) = [(0i32, 1i32), (1, 0), (1, -1), (1, 1)][dir];
            let (r0, c0) = (a, b + 4 * u8::from(dc != 1));
            let cells: Vec<(u8, u8)> = (0..5)
                .map(|k| ((r0 as i32 + dr * k) as u8, (c0 as i32 + dc * k) as u8))
                .collect();
            prop_assert!(cells.iter().all(|&(r, c)| r < 15 && c < 15));

            let four = stones(&cells[..4]);
            let five = stones(&cells);
            prop_assert!(!has_five_any(&four), "four in a row is not a five");
            prop_assert!(has_five_any(&five));
            prop_assert_eq!(has_five_any(&five), count_walk(&five, DIRS[dir]));
        }

        /// Random stone sets: the bit trick and the walk-based oracle
        /// must agree on every one of them.
        #[test]
        fn agrees_with_the_slow_oracle(cells in proptest::collection::hash_set(0u8..225, 0..30)) {
            let bb = cells.iter().fold(Bitboard::EMPTY, |b, &i| {
                b.with_bit(((i / 15) as usize) * 16 + (i % 15) as usize)
            });
            prop_assert_eq!(
                has_five_any(&bb),
                DIRS.iter().any(|&s| count_walk(&bb, s)),
                "bitboard says {}, oracle says {}",
                has_five_any(&bb),
                DIRS.iter().any(|&s| count_walk(&bb, s))
            );
        }
    }
}
```

### `crates/engine/src/board.rs` — two hunks

```diff
--- a/crates/engine/src/board.rs
+++ b/crates/engine/src/board.rs
@@ -50,23 +50,6 @@
     moves: Vec<Move>,
 }
 
-const DIRS: [i32; 4] = [1, 16, 15, 17];
-
-/// Consecutive set bits starting one step away from `i`, walking in
-/// `step` direction. Two termination mechanisms, both free:
-///   - the index range check stops walks at the array edge (verticals);
-///   - the PADDING INVARIANT stops horizontal/diagonal wraps: every
-///     wrap path lands in column 15, whose bits are always zero.
-fn count_walk(stones: Bitboard, i: usize, step: i32) -> usize {
-    let mut n = 0;
-    let mut x = i as i32 + step;
-    while (0..240).contains(&x) && stones.test(x as usize) {
-        n += 1;
-        x += step;
-    }
-    n
-}
-
 impl Board {
     pub fn new() -> Board {
         Board {
@@ -92,7 +75,7 @@
         }
         self.moves.push(mv);
 
-        if self.wins_from(i, self.to_move) {
+        if crate::win::has_five_any(&self.stones(self.to_move)) {
             self.status = Status::Won(self.to_move);
         } else if self.moves.len() == 225 {
             self.status = Status::Draw;
@@ -103,12 +86,6 @@
         Ok(())
     }
 
-    fn wins_from(&self, i: usize, color: Color) -> bool {
-        let stones = self.stones(color);
-        DIRS.iter()
-            .any(|&s| 1 + count_walk(stones, i, s) + count_walk(stones, i, -s) >= 5)
-    }
-
     pub fn status(&self) -> Status {
         self.status
     }
```

### `crates/engine/src/bitboard.rs` — one hunk

```diff
--- a/crates/engine/src/bitboard.rs
+++ b/crates/engine/src/bitboard.rs	2026-09-16 19:44:28
@@ -36,15 +36,12 @@
         self.0[idx / 64] & (1 << (idx % 64)) != 0
     }
 
-    // The three methods below are what slice 4's staged win detection
-    // consumes — `count`/`is_zero` to inspect results, `shr` as the
-    // shift-AND primitive. Nothing in the crate uses them yet, hence the
-    // narrow allows; drop them when `win.rs` lands.
-    #[allow(dead_code)]
     pub(crate) fn is_zero(&self) -> bool {
         self.0 == [0; 4]
     }
 
+    /// Popcount over the four words. Used by tests and assertions;
+    /// slice 7's threat counting is the next real consumer.
     #[allow(dead_code)]
     pub(crate) fn count(&self) -> u32 {
         self.0.iter().map(|w| w.count_ones()).sum()
@@ -53,7 +50,6 @@
     /// Logical right shift of the whole 256-bit value by `s`,
     /// `0 < s < 64`. Bits shifted past the top are lost (fine: they
     /// are padding or beyond).
-    #[allow(dead_code)]
     pub(crate) fn shr(&self, s: u32) -> Bitboard {
         // THE TRAP: `x << 64` on u64 is a panic in debug builds and
         // SILENT GARBAGE in release (the hardware masks the shift
```

### Verification log

```text
cargo fmt --all --check                                     clean
cargo clippy -p engine --all-targets --features testutil -- -D warnings
                                                            clean
cargo test -p engine --features testutil --no-fail-fast     38 passed + 2 passed
PROPTEST_CASES=10000  ... same                              38 passed + 2 passed
PROPTEST_CASES=100000 ... same                              38 passed + 2 passed
```

The 38 = the 31 from slices 2–3 (2 Move + 16 reference + 7 bitboard +
6 board) plus the seven in `win.rs`:
`detects_all_four_directions`, `detects_fives_along_every_edge`,
`overlines_count_and_near_misses_do_not`, `wrap_pattern_is_not_a_five`,
`wrap_attack_is_not_a_win`, `planted_runs_are_found`,
`agrees_with_the_slow_oracle`. The 2 differential
properties are unchanged and now exercise `win.rs` through `Board`.

## What this slice does not do

- No `MoveSet`, no `immediate_wins`/`forced_blocks`/`double_threats` — that is
  slice 7, and it is the first real consumer of `count`.
- No criterion bench and no committed numbers — slice 9.
- No incremental/stateful detector. `has_five_any` is a fresh query per call;
  MCTS copies boards and asks the question cold, which is why the whole-board
  shape exists next to the neighbourhood check.
