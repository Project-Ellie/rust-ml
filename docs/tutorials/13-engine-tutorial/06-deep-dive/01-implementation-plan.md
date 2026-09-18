# 01 — Slice 6 implementation plan: intention, transforms, and the commuting diagram (with reference solution)

The slice file ([../06-symmetry-and-encoding.md](../06-symmetry-and-encoding.md))
gives you the contract and the checklist. This paper is the opt-in other
half — but unlike the slice-4/5 plans, it does not start at the first
red test. It starts where good implementation starts: **what exactly is
it that you want to achieve** — first in the project as a whole, then in
this chapter, then in the mathematics. The red→green sequence and the
verified reference solution follow after that.

**Verified before written.** Every step below was executed in a scratch
copy of the engine: **54 unit tests + 2 differential properties** green,
`clippy --all-targets --features testutil -- -D warnings` and
`fmt --check` clean, the commutation property green at its hard-coded
10,000 random boards, and the differential suite green at
`PROPTEST_CASES=10000` and `100000`.

---

## Part 1 — The intention, top down: what this slice is *for*

Zoom all the way out. Project-Ellie is an AlphaZero loop:

```text
self-play games ──► replay buffer (stores GAMES, not planes)
                  ──► sampler: replay ▸ encode ▸ one random transform
                  ──► network trains on (planes, π, z)
```

The store-games decision (ch. 12) is what puts this slice where it is.
Because the buffer stores game histories rather than tensor-ready
planes, **every training sample is re-encoded and re-transformed on the
fly**, ten thousand times per iteration, on CPU workers, forever. That
is why encoding and transforms live in the *engine* — the dependency
island, Burn-free, testable — and not in the trainer.

Now the deeper intention, the one worth forming before you touch code:

> **The network must never be able to tell whether it is looking at a
> position or at one of its seven other forms.**

A Gomoku position rotated 90° is the same position. Same value, same
winning threats, same best moves — just relabeled. If you train without
transforms, the network must spend capacity learning "the same
position, rotated" eight separate times, and the edge cases will
disagree with each other. If you sample one random transform per
example, you get 8× data for free *and* you force the network's
capacity onto what actually varies. This is AlphaZero's exact usage —
augmentation at sample time, never baked into the architecture, never
averaged at search time (ch. 12 §7).

But augmentation is only free if it is *correct*. A wrong transform —
one that turns a won position into a lost one, or permutes the planes
differently than the moves — silently teaches the network that
positions mean things they don't. You would never notice in the loss
curve. That is why this slice's true deliverable is not the enum or the
tables; it is **one property test**: *encode commutes with every
transform*. Hold that sentence. Everything in this slice exists to make
that property true and keep it true.

## Part 2 — Drilling into the chapter: three artifacts, four decisions

The chapter asks for two files, but the intention decomposes into three
artifacts:

1. **The 8 transforms as data.** `Transform` — an enum, 8 values — plus
   `TABLES: [[u8; 225]; 8]`: for each transform, where does each
   logical cell *go*. Plus the group operations you actually need:
   `ALL` (iterate), `inverse` (undo), `transform_move` (act on moves),
   `permute` (act on 225-entry planes and policy vectors).
2. **The encoding.** `encode(&Board) -> Planes`: two 17×17 `u8` planes,
   `me`/`you` *relative to the side to move*, with the border ring set
   as opponent stones.
3. **The commutation property.** The proptest that binds 1 and 2
   together. Not an afterthought — the chapter calls it "the
   deliverable", and Part 1 explains why.

Four design decisions to understand *before* implementing, because each
one will otherwise look like an arbitrary choice:

- **Transforms live in 15×15 space; the border is added after.** The
  border ring is D4-invariant — rotating a ring gives the same ring —
  so the 289-entry plane never needs transforming. Permute the 225
  inner cells, re-add the ring. This is why `TABLES` has 225 entries,
  not 289, and why the commutation property can hold *exactly* rather
  than approximately.
- **Planes are relative (`me` = side to move), while `Board` is
  absolute.** Decision 5 (ch. 13): Swap2's non-alternating opening
  cannot be expressed in a relative store, so the *rules engine* keeps
  absolute colors. But the *network* always plays "me" — relative
  planes halve what it must learn. The conversion happens exactly once,
  in `encode`, at the system boundary.
- **`u8` arrays, no tensors.** The engine stays Burn-free; the `net`
  crate converts. This slice stays CPU-testable — which is what makes
  the 10k-board commutation proptest a unit test instead of a GPU job.
- **Const-generated tables, same technique as Zobrist.** Two primitives
  (`rot90`, `flip`) as `const fn`, everything else composed in a
  `while` loop at compile time. Zero runtime init, and — this is the
  elegant part — **the group structure is visible in the construction
  itself**. More on that in Part 3.

## Part 3 — The beauty: group theory, made executable

### Words first

The chapter now carries this note, and it matters enough to repeat:
the eight elements of D4 are **transforms** — functions from positions
to positions. A **symmetry** is not a thing but a relationship:
transform *T* is a symmetry *of position p* when *T(p) = p*. The empty
board has all eight symmetries. A single center stone has all eight. A
generic mid-game position has none but the identity. Keep the
distinction and the code reads correctly: `Transform::Rot90` is a
transform; "this position is symmetric under `Rot180`" is a symmetry
claim — a fixed point.

### The group, generated

D4, the symmetry group of the square, is 8 elements generated by two:
a quarter turn *r* and a mirror flip *f*, with relations

```text
r⁴ = e        f² = e        f·r = r⁻¹·f
```

The third relation is the interesting one: it says the group is *not*
commutative — flip-then-rotate differs from rotate-then-flip — and it
tells you exactly *how* they differ. Every element has a normal form:
*r^k* or *r^k·f* for k ∈ {0,1,2,3}. Look at the table builder in the
reference solution: `Id, r1, r2, r3, f, rot90(f), rot90(rot90(f)), …`
— it is the normal form, written in Rust. The code doesn't just
*compute* the group; it *displays* it.

One consequence you will encode in `inverse()`: every reflection is its
own inverse. Proof in one line using the third relation:
*(r^k·f)² = r^k·(f·r^k)·f = r^k·r^−k·f·f = e*. Mirror twice, you're
back. So `inverse` pairs Rot90 ↔ Rot270, keeps Rot180, and maps each
`FlipRotK` to itself — and when the inverse-roundtrip proptest passes,
it has verified that one-line proof 10,000 times over.

### Orbits: what the transforms *do* to the 225 cells

A transform partitions the board's cells into **orbits** — sets of
cells that cycle into each other. On 15×15 the orbit structure is
unusually pretty, and worth knowing *before* you debug anything in this
slice:

- **The center (7,7) is alone.** Orbit of size 1: every transform maps
  it to itself. (Debugging corollary: if a transform seems to do
  nothing, check whether you tested it on the center.)
- **56 cells lie on the four mirror axes** (both diagonals, the middle
  row, the middle column — minus the center, which is on all four).
  Each has an orbit of size **4**: the axis cells are fixed by one
  reflection, so the group can only reach 8/2 = 4 distinct cells.
- **The remaining 168 cells are generic**: orbits of size **8**.

Count: 1 + 14·4 + 21·8 = 225. And Burnside's lemma confirms the orbit
count from the other direction — (225 + 1 + 1 + 1 + 4·15)/8 = **36
orbits**. If you ever print a transformed board and a stone lands
somewhere "impossible", the orbit structure is your mental checksum:
center stays, axis cells stay on axes, generic cells stay generic.

### Symmetric positions (fixed points), and why augmentation doesn't mind

A position with a nontrivial symmetry — say, invariant under Rot180 —
produces *duplicate* training samples under augmentation. Harmless: the
samples are still *correct* (the commutation property guarantees the
transformed planes and policy describe the same position), and
symmetric positions are vanishingly rare in real games. But notice the
terminology doing its work: "this position has a Rot180 symmetry" is
now a precise statement — Rot180 is one of its fixed-point transforms.

### The commuting diagram — the actual deliverable

Everything converges on one square:

```text
                transform t
        Board ───────────────► Board (t·b)
          │                        │
   encode │                        │ encode
          ▼                        ▼
       Planes ───────────────► Planes
            permute inner 15×15 by t,
            re-add the (invariant) ring
```

*encode ∘ transform = embed ∘ permute ∘ extract* — the two paths around
the square agree **exactly**. When a mathematician says "the diagram
commutes", this is what they mean, and it is the precise sense in which
augmentation "never changes what a position means". Your proptest is
this diagram, quantified over 10,000 random boards × 8 transforms. If
`permute`'s direction is ever inverted, or the border is added before
transforming, or `encode` forgets the me/you swap — this test, and only
a test of this shape, catches it.

## Part 4 — The build: red→green

### Step 0 — before you write anything

- Slice 5 is done: Zobrist keys, 47 unit + 2 differential green.
- `symmetry.rs` and `encode.rs` exist as doc-comment stubs; `lib.rs`
  declares both modules. You are filling in files.
- **Unlike slices 4–5, this slice grows the public API**: `Transform`,
  `encode`, `Planes`, `EXT` get `pub use` in `lib.rs` (ch. 13's crate
  layout already lists them). That is also why nothing in this slice
  needs a `#[allow(dead_code)]`: `pub` re-exported items are never
  dead.
- Baseline: `cargo test -p engine --features testutil` → 47 + 2.

### The build order

| step | behaviour it adds | new code |
|---|---|---|
| 1 | the two primitives; rot90⁴ = id, flip² = id | `rot90`, `flip` const fns |
| 2 | the enum, the tables, `transform_move`, `ALL`, `inverse` | most of `symmetry.rs` |
| 3 | inverse roundtrip, all moves (proptest) | none |
| 4 | `permute`; win preservation (proptest) | `permute`, `transform_board_for_test` |
| 5 | `encode`; the 64-cell ring; relative planes | `encode.rs` |
| 6 | **the commutation property** (10k boards) | test-only helper |
| 7 | `lib.rs` re-exports, gates, commit | two `pub use` lines |

### Step 1 — primitives, and the group laws you can feel

```rust
#[test]
fn rot90_four_times_and_flip_twice_are_identity() {
    let m = mv(3, 11);
    let r = Transform::Rot90;
    assert_eq!(r.transform_move(r.transform_move(r.transform_move(r.transform_move(m)))), m);
    let f = Transform::Flip;
    assert_eq!(f.transform_move(f.transform_move(m)), m);
}
```

RED: nothing exists yet. Write the enum with `Id, Rot90, Flip` only,
plus `transform_move` backed by the two primitives *applied directly*
(no tables yet) — this is the one slice where the intermediate
implementation is genuinely simpler than the final one. The two
relations `r⁴ = e` and `f² = e` are the first group laws, felt on one
hand-picked off-center, off-axis move (`(3, 11)` — remember the orbit
lesson: center and axis cells are degenerate test subjects).

GREEN when both chains return `(3, 11)`.

### Step 2 — tables, and the group made visible

Now replace the direct application with the const tables and grow the
enum to all 8. RED for the four `FlipRotK` variants (they don't exist).
GREEN: the `TABLES` const from the reference solution — note how rows
5–7 are composed from row 4 and `rot90`, the normal form *r^k·f* in
plain sight. Add `ALL`, `index`, and `inverse` — write `inverse` from
the one-line proof in Part 3 (reflections are self-inverse), and add
the hand-picked roundtrip test over all 8 transforms and four moves
(corner, center, edge, generic — one per orbit type, now you know why).

### Step 3 — the roundtrip, quantified

Proptest the roundtrip over *all* cells × all 8 transforms. This is the
test that turns "I believe the tables are permutations" into "the
tables are permutations, measured". A table with a collision (two cells
mapping to one) fails here immediately: some cell is unreachable, its
preimage test breaks.

### Step 4 — `permute`, and win preservation

`permute<T: Copy>` is five lines and one decision: **direction**.
`out[table[i]] = cells[i]` — entry *i* moves TO `table[i]`, the same
convention as `transform_move`. Write that sentence in the doc comment;
the chapter warns the commutation test will catch an inversion, and it
will — but only in step 6, two steps late. The doc comment is how you
catch it now.

Win preservation needs a transformed *board*. Here the design speaks:
**nothing in production transforms a whole `Board`** — the trainer
permutes planes (that's the cheap path around the diagram). Only tests
need it, so the helper is `#[cfg(test)] pub(crate)
transform_board_for_test`: replay the move history through
`transform_move`. Then the proptest: plant a Black five (the slice-4
window trick), give White 4 random disjoint fillers (never a white
five, never an illegal script), Black's fifth stone lands last — and
all 8 transformed boards must still be `Won(Black)`. Driven through the
fast board, so transforms, play, and win detection are exercised
together.

### Step 5 — `encode`, the ring, and the relative planes

Three corpus tests, then the implementation:

1. Empty board: `me` all zero; `you` has **exactly 64** cells
   (17·4 − 4 — off-by-four happens, the chapter says, because corners
   belong to two edges), and *only* the ring.
2. B(0,0), W(7,7), Black to move: `me[EXT + 1] = 1` (index 18),
   `you[8·17 + 8] = 1`. One ply earlier, White to move: the same Black
   stone is in `you`. The relative-planes decision, pinned.
3. (Implementation detail, measured): write the index as `EXT + 1`,
   not `1 * EXT + 1` — clippy's `identity_op` fires under `-D warnings`
   on the latter, however documentary its intent.

### Step 6 — the commuting diagram, as a proptest

Write `permute_planes17` in the *test*, deliberately the dumb way:
extract inner 15×15 → `t.permute` → re-embed with a fresh ring. Then,
at 10,000 hard-coded cases (the "Done when" says so):

```rust
prop_assert_eq!(
    encode(&transform_board_for_test(&b, t)),
    permute_planes17(&encode(&b), t),
);
```

Both directions around the square, computed independently, compared
exactly. When this is green, sit with it for a moment — you have
*proven, by exhaustive-ish measurement*, that your augmentation can
never change what a position means. This is the test Part 1 promised.

(Measured trap: proptest's `prop_assert_eq!` does not support inline
format capture — `"{t:?}"` fails to compile with "there is no argument
named `t`". Pass it explicitly: `"{:?}", t`.)

### Step 7 — re-exports, gates, commit

```rust
pub use encode::{EXT, Planes, encode};
pub use symmetry::Transform;
```

```bash
cargo fmt --all
cargo clippy -p engine --all-targets --features testutil -- -D warnings
cargo test  -p engine --features testutil --no-fail-fast
PROPTEST_CASES=10000 cargo test -p engine --features testutil
```

Commit: `feat(engine): D4 symmetries + 17x17 border encoding` — the
slice's own message (yes, "symmetries" in the historical message; the
code says `Transform`).

---

## Part 5 — Reference solution

Assembled from the steps above; byte-for-byte what was verified.

### `crates/engine/src/symmetry.rs` — complete

```rust
//! Dihedral group D4: 8 transforms, const-generated permutation tables
//! over logical 15×15 indices.
//! Slice 6. See docs/13-engine-design.md, "Encoding"; ch. 12 §7.
//!
//! Terminology: the group ELEMENTS are transforms — functions from
//! positions to positions. A "symmetry" is a relationship, not a
//! thing: transform T is a symmetry of position p iff T(p) = p.

use crate::moveset::Move;

/// One of the 8 transforms of the square board: the identity, three
/// rotations, and the four reflections. `FlipRotK` means FLIP FIRST,
/// then rotate by k·90° — function composition read right to left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    Id,
    Rot90,
    Rot180,
    Rot270,
    Flip,
    FlipRot90,
    FlipRot180,
    FlipRot270,
}

/// Where the stone at logical cell `i` GOES under a quarter turn:
/// (r, c) → (c, 14 − r).
const fn rot90(i: u8) -> u8 {
    let (r, c) = (i / 15, i % 15);
    c * 15 + (14 - r)
}

/// Where the stone at logical cell `i` GOES under a mirror flip of the
/// columns: (r, c) → (r, 14 − c).
const fn flip(i: u8) -> u8 {
    let (r, c) = (i / 15, i % 15);
    r * 15 + (14 - c)
}

/// `TABLES[t][i]` = the logical cell that cell `i` MOVES TO under
/// transform `t`. Same const-eval technique as the Zobrist table:
/// `while` loops, no iterators, zero runtime cost. The group structure
/// is visible in the construction — every table entry is built from
/// the two primitives by composition.
pub(crate) const TABLES: [[u8; 225]; 8] = {
    let mut tables = [[0u8; 225]; 8];
    let mut i = 0;
    while i < 225 {
        let r1 = rot90(i as u8);
        let r2 = rot90(r1);
        let r3 = rot90(r2);
        let f = flip(i as u8);
        tables[0][i] = i as u8; // Id
        tables[1][i] = r1; // Rot90
        tables[2][i] = r2; // Rot180
        tables[3][i] = r3; // Rot270
        tables[4][i] = f; // Flip
        tables[5][i] = rot90(f); // FlipRot90  = rot90 ∘ flip
        tables[6][i] = rot90(tables[5][i]); // FlipRot180 = rot180 ∘ flip
        tables[7][i] = rot90(tables[6][i]); // FlipRot270 = rot270 ∘ flip
        i += 1;
    }
    tables
};

impl Transform {
    pub const ALL: [Transform; 8] = [
        Transform::Id,
        Transform::Rot90,
        Transform::Rot180,
        Transform::Rot270,
        Transform::Flip,
        Transform::FlipRot90,
        Transform::FlipRot180,
        Transform::FlipRot270,
    ];

    fn index(self) -> usize {
        match self {
            Transform::Id => 0,
            Transform::Rot90 => 1,
            Transform::Rot180 => 2,
            Transform::Rot270 => 3,
            Transform::Flip => 4,
            Transform::FlipRot90 => 5,
            Transform::FlipRot180 => 6,
            Transform::FlipRot270 => 7,
        }
    }

    /// The group inverse. The rotations pair off (Rot90 ↔ Rot270);
    /// the four reflections are each their OWN inverse — mirror twice
    /// and you are back. In group notation: (r^k·f)² = r^k·(f·r^k)·f
    /// = r^k·r^−k·f·f = Id, because f·r = r^−1·f in D4.
    pub fn inverse(self) -> Transform {
        match self {
            Transform::Id => Transform::Id,
            Transform::Rot90 => Transform::Rot270,
            Transform::Rot180 => Transform::Rot180,
            Transform::Rot270 => Transform::Rot90,
            f @ (Transform::Flip
            | Transform::FlipRot90
            | Transform::FlipRot180
            | Transform::FlipRot270) => f,
        }
    }

    pub fn transform_move(self, mv: Move) -> Move {
        let i = TABLES[self.index()][mv.index()];
        // Table entries are images of valid cells under rot90/flip, so
        // they are valid cells — this never fails.
        Move::new(i / 15, i % 15).expect("permutation tables contain only valid cells")
    }

    /// Permute 225 logical cells (planes, policy vectors).
    ///
    /// DIRECTION (the one pitfall of this slice): `out[table[i]] =
    /// cells[i]` — the entry at cell i moves TO table[i], exactly as
    /// `transform_move` moves a stone. Both functions use the same
    /// convention; that is what makes encode commute with every
    /// transform. If you ever "fix" one without the other, the
    /// commutation proptest will catch it.
    pub fn permute<T: Copy>(self, cells: &[T; 225]) -> [T; 225] {
        let table = &TABLES[self.index()];
        let mut out = *cells;
        for (i, &dst) in table.iter().enumerate() {
            out[dst as usize] = cells[i];
        }
        out
    }
}

/// Replay a board's history through a transform: same position,
/// rotated/flipped. Bijectivity of the table makes every transformed
/// move legal exactly when the original was. Test-only: the engine
/// transforms MOVES and PLANES; nothing in production transforms a
/// whole Board (the trainer permutes planes, per ch. 13).
#[cfg(test)]
pub(crate) fn transform_board_for_test(
    b: &crate::board::Board,
    t: Transform,
) -> crate::board::Board {
    let mut out = crate::board::Board::new();
    for &m in b.moves() {
        let _ = out.play(t.transform_move(m));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Board, Color, Status};
    use proptest::prelude::*;

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    #[test]
    fn rot90_four_times_and_flip_twice_are_identity() {
        let m = mv(3, 11);
        let r = Transform::Rot90;
        assert_eq!(
            r.transform_move(r.transform_move(r.transform_move(r.transform_move(m)))),
            m
        );
        let f = Transform::Flip;
        assert_eq!(f.transform_move(f.transform_move(m)), m);
    }

    #[test]
    fn inverse_undoes_every_transform_on_hand_picked_moves() {
        for m in [mv(0, 0), mv(7, 7), mv(2, 14), mv(14, 5)] {
            for t in Transform::ALL {
                assert_eq!(
                    t.inverse().transform_move(t.transform_move(m)),
                    m,
                    "{t:?} on {m:?}"
                );
            }
        }
    }

    proptest! {
        /// Inverse roundtrip for ALL moves and all transforms.
        #[test]
        fn inverse_roundtrip_for_all_moves(cell in 0u8..225, t_idx in 0usize..8) {
            let m = mv(cell / 15, cell % 15);
            let t = Transform::ALL[t_idx];
            prop_assert_eq!(t.inverse().transform_move(t.transform_move(m)), m);
        }

        /// Win preservation: a position won by construction stays won
        /// under all 8 transforms. Driven through the FAST board
        /// (replay of the transformed history), so this exercises
        /// transform_move + play + win detection together.
        ///
        /// Construction: Black gets a planted five; White gets 4
        /// random filler cells (never five, never blocking the
        /// script's legality). Black's fifth stone lands last.
        #[test]
        fn won_positions_stay_won_under_all_transforms(
            dir in 0usize..4,
            a in 0u8..11,
            b in 0u8..11,
            fillers in prop::collection::hash_set(0u8..225, 4),
        ) {
            let (dr, dc) = [(0i32, 1i32), (1, 0), (1, -1), (1, 1)][dir];
            let (r0, c0) = (a, b + 4 * u8::from(dc != 1));
            let five: Vec<u8> = (0..5)
                .map(|k| ((r0 as i32 + dr * k) * 15 + (c0 as i32 + dc * k)) as u8)
                .collect();
            prop_assume!(fillers.iter().all(|f| !five.contains(f)));
            let fillers: Vec<u8> = fillers.into_iter().collect();

            let mut board = Board::new();
            for k in 0..4 {
                board.play(mv(five[k] / 15, five[k] % 15)).unwrap();
                board.play(mv(fillers[k] / 15, fillers[k] % 15)).unwrap();
            }
            board.play(mv(five[4] / 15, five[4] % 15)).unwrap();
            prop_assert_eq!(board.status(), Status::Won(Color::Black));

            for t in Transform::ALL {
                let tb = transform_board_for_test(&board, t);
                prop_assert_eq!(tb.status(), Status::Won(Color::Black), "{:?}", t);
            }
        }
    }
}
```

### `crates/engine/src/encode.rs` — complete

```rust
//! Burn-free 17×17 plane encoding, border ring set in the `you` plane.
//! Slice 6. See docs/13-engine-design.md, "Encoding".
//!
//! DESIGN DECISION (not a trick): transforms live in 15×15 space and
//! the border is added AFTER transformation. The border ring is
//! D4-invariant — rotating a ring gives the same ring — so
//! `encode(transform(b)) == embed(permute(inner_planes(b)))` holds
//! exactly, and no 289-entry permutation tables exist anywhere. The
//! commutation proptest in this file is the property the training
//! pipeline relies on ten thousand times per iteration: augmentation
//! must never change what a position MEANS.

use crate::bitboard::idx;
use crate::board::Board;

pub const EXT: usize = 17;

/// RELATIVE planes: `me` is always the side to move. (The ABSOLUTE
/// colors stay inside `Board` — ch. 13, decision 5; the network
/// always plays "me".)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planes {
    pub me: [u8; EXT * EXT],  // stones of side to move; border = 0
    pub you: [u8; EXT * EXT], // opponent stones; border ring = 1
}

pub fn encode(b: &Board) -> Planes {
    let mut me = [0u8; EXT * EXT];
    let mut you = [0u8; EXT * EXT];

    // The border ring: a wall of "opponent" stones one cell thick.
    // A convolution is translation-equivariant — with the ring, the
    // filter sliding past the edge sees a WALL, which is semantically
    // true: no line continues through the border, exactly as through
    // an enemy stone (ch. 12 §7). 17*4 - 4 = 64 cells.
    for k in 0..EXT {
        you[k] = 1; // top row
        you[(EXT - 1) * EXT + k] = 1; // bottom row
        you[k * EXT] = 1; // left column
        you[k * EXT + (EXT - 1)] = 1; // right column
    }

    // Cell (r, c) lands at (r + 1) * 17 + (c + 1) — the one stride-15
    // to stride-17 crossing in the crate.
    let mover = b.to_move();
    for r in 0..15u8 {
        for c in 0..15u8 {
            let i = idx(r, c); // stride-16 bitboard index
            let dst = (r as usize + 1) * EXT + (c as usize + 1);
            if b.stones(mover).test(i) {
                me[dst] = 1;
            } else if b.stones(mover.other()).test(i) {
                you[dst] = 1;
            }
        }
    }
    Planes { me, you }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Color;
    use crate::moveset::Move;
    use crate::symmetry::{Transform, transform_board_for_test};
    use proptest::prelude::*;

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    #[test]
    fn empty_board_has_empty_me_and_a_64_cell_border_ring() {
        let p = encode(&Board::new());
        assert!(p.me.iter().all(|&x| x == 0), "me must be all zero");
        assert_eq!(p.you.iter().filter(|&&x| x == 1).count(), 64, "17*4 - 4");
        // the ring, and ONLY the ring
        for r in 0..EXT {
            for c in 0..EXT {
                let on_ring = r == 0 || c == 0 || r == EXT - 1 || c == EXT - 1;
                assert_eq!(p.you[r * EXT + c], u8::from(on_ring), "at ({r}, {c})");
            }
        }
    }

    #[test]
    fn stones_land_relative_to_the_side_to_move() {
        // Black to move... so first give Black a stone and White one:
        // B(0,0), W(7,7). Now Black is to move: (0,0) is "me".
        let mut b = Board::new();
        b.play(mv(0, 0)).unwrap();
        b.play(mv(7, 7)).unwrap();
        assert_eq!(b.to_move(), Color::Black);
        let p = encode(&b);
        assert_eq!(p.me[EXT + 1], 1, "mover's (0,0) at index 18 of me");
        assert_eq!(p.you[8 * EXT + 8], 1, "opponent's (7,7) in you");

        // One ply earlier (White to move): Black's (0,0) is "you".
        let mut b1 = Board::new();
        b1.play(mv(0, 0)).unwrap();
        assert_eq!(b1.to_move(), Color::White);
        let p1 = encode(&b1);
        assert_eq!(p1.you[EXT + 1], 1, "opponent's (0,0) at index 18 of you");
        assert_eq!(p1.me[EXT + 1], 0);
    }

    /// The naive reference for the commutation property: extract the
    /// inner 15×15, permute it, re-embed with a fresh border ring.
    /// Deliberately written the dumb way — two implementations that
    /// disagree are a bug report.
    fn permute_planes17(p: &Planes, t: Transform) -> Planes {
        let mut me_inner = [0u8; 225];
        let mut you_inner = [0u8; 225];
        for r in 0..15usize {
            for c in 0..15usize {
                me_inner[r * 15 + c] = p.me[(r + 1) * EXT + (c + 1)];
                you_inner[r * 15 + c] = p.you[(r + 1) * EXT + (c + 1)];
            }
        }
        let me_perm = t.permute(&me_inner);
        let you_perm = t.permute(&you_inner);

        let mut out = Planes {
            me: [0; EXT * EXT],
            you: [0; EXT * EXT],
        };
        for k in 0..EXT {
            out.you[k] = 1;
            out.you[(EXT - 1) * EXT + k] = 1;
            out.you[k * EXT] = 1;
            out.you[k * EXT + (EXT - 1)] = 1;
        }
        for r in 0..15usize {
            for c in 0..15usize {
                out.me[(r + 1) * EXT + (c + 1)] = me_perm[r * 15 + c];
                out.you[(r + 1) * EXT + (c + 1)] = you_perm[r * 15 + c];
            }
        }
        out
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10_000))]

        /// THE deliverable of this slice: encode commutes with every
        /// transform. Transform the board, then encode == encode, then
        /// permute the inner planes and re-add the border.
        #[test]
        fn encode_commutes_with_every_transform(
            cells in prop::collection::vec(0u16..225, 1..=60),
            t_idx in 0usize..8,
        ) {
            let mut b = Board::new();
            for p in cells {
                let _ = b.play(mv((p / 15) as u8, (p % 15) as u8));
            }
            let t = Transform::ALL[t_idx];
            prop_assert_eq!(
                encode(&transform_board_for_test(&b, t)),
                permute_planes17(&encode(&b), t),
            );
        }
    }
}
```

### `crates/engine/src/lib.rs` — one hunk

```diff
 pub use board::{Board, Color, PlayError, Status};
+pub use encode::{EXT, Planes, encode};
 pub use moveset::Move;
+pub use symmetry::Transform;
```

### Verification log

```text
cargo fmt --all --check                                     clean
cargo clippy -p engine --all-targets --features testutil -- -D warnings
                                                            clean
cargo test -p engine --features testutil --no-fail-fast     54 passed + 2 passed
PROPTEST_CASES=10000  ... same                              54 passed + 2 passed
PROPTEST_CASES=100000 ... differential only                 2 passed
```

The 54 = the 47 from slices 2–5 plus the seven in `symmetry.rs` and
`encode.rs`: `rot90_four_times_and_flip_twice_are_identity`,
`inverse_undoes_every_transform_on_hand_picked_moves`,
`inverse_roundtrip_for_all_moves`,
`won_positions_stay_won_under_all_transforms`,
`empty_board_has_empty_me_and_a_64_cell_border_ring`,
`stones_land_relative_to_the_side_to_move`,
`encode_commutes_with_every_transform` (the last at a hard-coded 10,000
boards).

## What this slice does not do

- **No augmentation sampler** — choosing *which* random transform per
  sample is the trainer's business, later. The engine offers the 8;
  the sampler picks 1. (And never averaged at search time — ch. 12 §7.)
- **No last-move planes** (planes 2–3 of the network input): ch. 13
  defers the 2-vs-4-plane decision to the `net` crate; it's a five-line
  addition reading `moves.last()`.
- **No policy-vector permute call site** — `permute` is generic over
  `T: Copy` precisely so the trainer can later permute `[f32; 225]`
  policy targets through monomorphization, but nothing calls it with
  `f32` yet.
- **No `Board::transform` in production** — boards are never
  transformed outside tests; the cheap path around the diagram is
  permuting planes, and keeping it that way is a design decision, not
  an omission.
