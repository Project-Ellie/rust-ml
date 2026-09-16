# Slice 5 — Zobrist keys

Give `Board` a 64-bit position fingerprint that updates in O(1) per
move — and undoes for free. Design: ch. 13, "Zobrist, compile-time".

## Contract

`zobrist.rs` (`pub(crate)`):

```rust
pub(crate) const ZOBRIST: [[u64; 225]; 2];  // [color][cell] -> random u64
pub(crate) const SIDE_TO_MOVE: u64;

/// From-scratch key — the ground truth the incremental key must match.
pub(crate) fn compute_key(black: &Bitboard, white: &Bitboard, to_move: Color) -> u64;
```

`Board` gains:

```rust
pub fn zobrist(&self) -> u64;
```

`play` XORs in `ZOBRIST[color][cell]` and `SIDE_TO_MOVE`; `undo` XORs the
same values back out.

## Rust toolbox

**`const fn` table generation.** The table is built at *compile time*:

```rust
const fn xorshift(mut x: u64) -> u64 {
    x ^= x << 13; x ^= x >> 7; x ^= x << 17; x
}

pub(crate) const ZOBRIST: [[u64; 225]; 2] = {
    let mut table = [[0u64; 225]; 2];
    let mut state = 0x9E37_79B9_7F4A_7C15;
    let mut i = 0;
    while i < 450 {
        state = xorshift(state);
        table[i / 225][i % 225] = state;
        i += 1;
    }
    table
};
```

Rules of the const-eval game: `while` loops work, `for` loops do not
(iterators are not const); no heap, no RNG crate, no floating point in
stable const contexts. The payoff: zero runtime initialization, zero
dependencies, and the same table on every machine forever — your Zobrist
keys are reproducible across runs, which matters for replay-buffer
dedup.

**XOR algebra is the whole trick.** `a ^ a = 0` and `a ^ b ^ b = a`.
Adding a stone *and removing it* are the same operation — that is why
`undo` needs no history beyond the move itself. (Side-to-move flips the
same way: XOR `SIDE_TO_MOVE` on every play *and* every undo.)

## Systems refresh: what the key buys

A 64-bit hash with 2⁻⁶⁴ collision odds per pair — for our scale, treat
collisions as impossible (chapter 7 of the design keeps the tree a tree;
no transposition merging depends on this). Uses: dedup in the replay
buffer, cheap position identity in tests, and later a debug assertion
that two paths to the same position agree. It is the chess-programming
answer to "how do I know I have seen this position before" without
storing positions.

## TDD checklist

1. `ZOBRIST` contains no zeros and no duplicates (a `#[cfg(test)]` scan —
   with 450 random u64s, duplicates would signal a broken generator)
2. `compute_key` differs between: same stones, different side to move
3. Two boards built by *different move orders* reaching the same
   position → equal keys (this is the point of Zobrist)
4. `play` updates the key: `b.play(mv)` → `b.zobrist() ==
   compute_key(...)` — write it as a proptest over random games
5. **The roundtrip property** (the real test): random play/undo walks —
   play k moves, undo k moves → key equals the initial key; and at every
   intermediate point, incremental key == `compute_key` from scratch

Step 5 catches the classic bug (XORing the wrong color's table entry on
undo) that steps 1–4 cannot.

## Pitfalls

- Index the table by the *logical* cell (`Move::index()`, stride 15) —
  the table has 225 entries, not 240. Another stride-15/stride-16
  boundary; keep it in one function.
- `undo` must XOR with the color of the stone *being removed* — after
  `to_move` has flipped back, that is `to_move`, not `to_move.other()`.
  Walk one example by hand before coding.
- Do not expose raw table internals publicly; `zobrist()` returns `u64`,
  full stop.

## Done when

Roundtrip proptest green over 10k walks; differential suite still green.
Commit: `feat(engine): incremental Zobrist keys`.

Next: [Slice 6 — Symmetry and encoding](06-symmetry-and-encoding.md)
