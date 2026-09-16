# Slice 6 — Symmetry and encoding

Two deliverables that meet in one beautiful property test: the D4
symmetry group, and the 17×17 border-as-opponent plane encoding.
Design: ch. 13, "Encoding" and "Move and MoveSet"; ch. 12, §7
"Symmetry".

## Contract

`symmetry.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symmetry { Id, Rot90, Rot180, Rot270, Flip, FlipRot90, FlipRot180, FlipRot270 }

impl Symmetry {
    pub const ALL: [Symmetry; 8];
    pub fn inverse(self) -> Symmetry;
    pub fn transform_move(self, mv: Move) -> Move;
    /// Permute 225 logical cells (planes, policy vectors).
    pub fn permute<T: Copy>(self, cells: &[T; 225]) -> [T; 225];
}
```

Tables are const-generated `[[u8; 225]; 8]` — same `while`-loop
const-eval technique as Zobrist. Build them from two primitives
(rotate-90: `(r, c) → (c, 14 − r)`; flip: `(r, c) → (r, 14 − c)`) and
compose.

`encode.rs`:

```rust
pub const EXT: usize = 17;

pub struct Planes {
    pub me:  [u8; EXT * EXT],   // stones of side to move; border = 0
    pub you: [u8; EXT * EXT],   // opponent stones; border ring = 1
}

pub fn encode(b: &Board) -> Planes;
```

Cell `(r, c)` lands at `(r + 1) * 17 + (c + 1)`; the border ring
(`r ∈ {0, 16}` or `c ∈ {0, 16}`) is set in `you` only.

## Rust toolbox

**Const-eval composition.** `Rot180`'s table is `rot90(rot90(i))` —
compute it inside the same const `while` loop by applying the primitive
twice. Write the two primitives as `const fn` and the table builder
calls them. Zero runtime cost, and the group structure is visible in
code.

**Generic `permute<T: Copy>`.** One function serves `u8` planes today
and `f32` policy vectors in the trainer later. Monomorphization stamps
out a specialized copy per `T` — no runtime dispatch, no allocation.

**Why transform in 15×15 and embed after.** The border ring is
D4-invariant (rotating a ring gives the same ring). So
`encode(transform(b)) == embed(permute(inner_planes(b)))` holds exactly,
and you never need 289-entry permutation tables. This is a design
decision, not a trick — write it in a doc comment.

## TDD checklist

1. `Rot90` four times = `Id` on a hand-picked move; same for `Flip`
   twice
2. `sym.inverse()` actually inverts: for all 8 symmetries and a few
   moves, `sym.inverse().transform_move(sym.transform_move(mv)) == mv`
3. Proptest: step 2 for *all* moves 0..225
4. Win preservation: random won positions (plant a five, fill the rest
   randomly) stay won under all 8 transforms (drive both engines or the
   naive check — your choice, say which in a comment)
5. `encode` on `Board::new()`: `me` all zero, `you` has exactly the
   64-cell border ring set (17·4 − 4 = 64)
6. A stone of the side to move at (0, 0) lands at index `1 * 17 + 1 = 18`
   of `me` (an opponent stone at (0, 0) lands in `you` instead)
7. **The commutation property** (the deliverable): for random boards and
   all 8 symmetries, `encode(transform(b))` equals `encode(b)` with its
   inner 15×15 region permuted by `sym.permute` and the border re-added.
   Write a small helper `permute_planes17` in the *test* that does the
   naive thing; assert equality.

Step 7 is the property the training pipeline will silently rely on ten
thousand times per iteration: augmentation must never change what a
position *means*.

## ML refresh: augmentation and equivariance

**Why 8 symmetries.** Every Gomoku position has 8 equivalent forms with
identical value and identically-permuted policy. Sampling one random
transform per training example is 8× data for free — AlphaZero's exact
usage. It also forces the network to spend zero capacity learning "the
same position, rotated".

**Why border-as-opponent (your design, now in code).** A convolution is
*translation-equivariant*: the same 3×3 filter slides everywhere and
must mean the same thing everywhere. With zero padding, border cells are
a special case — the filter sees "nothing" past the edge, so edge
positions need separately-learned behavior. With the border ring set as
opponent stones, the filter sees a *wall*, which is semantically true:
you cannot build a line through the edge, exactly as through an enemy
stone. One set of filters now works at the center and at the edge — that
is the "consistent learning signal" this encoding buys.

## Pitfalls

- `permute` direction: is `out[i] = cells[table[i]]` or
  `out[table[i]] = cells[i]`? Pick one, document it, and let the
  commutation test catch an inversion — it will.
- Border count: 64 cells, not 60 or 68. Test 5 exists because
  off-by-four happens.
- Keep `Planes` as `u8` arrays. The `net` crate converts to tensors;
  the engine stays Burn-free and this slice stays CPU-testable.

## Done when

Commutation property green on 10k random boards; all gates green.
Commit: `feat(engine): D4 symmetries + 17x17 border encoding`.

Next: [Slice 7 — Tactics](07-tactics.md)
