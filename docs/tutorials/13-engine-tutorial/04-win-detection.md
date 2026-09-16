# Slice 4 — Win detection

Small slice, high leverage: the staged shift-AND `has_five`, plus the
corpus that attacks it where bitboards actually break — the edges.
Design: ch. 13, "Win detection".

## Contract

`win.rs` (all `pub(crate)`):

```rust
pub(crate) const DIRS: [u32; 4] = [1, 16, 15, 17];

/// True if `b` contains five or more consecutive stones in direction `s`.
pub(crate) fn has_five_dir(b: &Bitboard, s: u32) -> bool;

/// True if `b` contains five or more in a row, any direction.
pub(crate) fn has_five_any(b: &Bitboard) -> bool;
```

Use the staged form from the design:

```rust
let two  = *b & b.shr(s);          // runs of >= 2
let four = two & two.shr(2 * s);   // runs of >= 4
let five = four & four.shr(s);     // runs of >= 5
```

**Why staged, and why it detects overlines.** A naive five-term AND
needs `shr(4·17 = 68)` — over the 64-bit trap from slice 3. The staged
form never shifts more than 34. And because it asks "≥ 5", a six-run
contains a five-run and answers true: the overline rule (ch. 13,
decision 1) costs zero extra code.

**Why no edge masks.** Re-read the "padding invariant" section of ch. 13
before coding. Your slice-3 `assert_clean` is what makes this safe:
every wrap path crosses a zero padding bit, and the AND-chain dies
there. The danger is not in `has_five` — it is in any *complement* you
write elsewhere. Rule: after every `!`, immediately `& VALID`.

## Rust toolbox: table-driven tests

Four directions × several positions is a matrix — write it as data:

```rust
#[test]
fn detects_all_directions() {
    // (name, stones as (row, col) list)
    let cases: &[(&str, &[(u8, u8)])] = &[
        ("horizontal",  &[(7, 3), (7, 4), (7, 5), (7, 6), (7, 7)]),
        ("vertical",    &[(2, 9), (3, 9), (4, 9), (5, 9), (6, 9)]),
        // ...
    ];
    for (name, stones) in cases {
        // build a Bitboard, assert has_five_any
    }
}
```

One test, one table — the failure message names the case. (This is
*data-driven unit testing*, not a replacement for the proptest below.)

## TDD checklist

1. One horizontal five, center board → detected
2. All four directions, center → detected (table above)
3. Fives touching every edge: rows 0 and 14, columns 0 and 14
4. Corner diagonals: (0,0)–(4,4) and (0,14)–(4,10)
5. Overline: six in a row → true
6. Near-miss: four in a row → false; broken five (gap in middle) → false
7. **Wrap attack**: stones at columns 12–14 of row *r* and columns 0–1
   of row *r+1* → false (this is the case the padding invariant kills;
   if it ever passes, your invariant is broken, not the test)
8. Proptest: plant a random run of 4 at a random position/direction on
   an empty board, extend it by one more stone → detected; then plant
   random *non-line* stone sets (up to ~30 stones, rejection-sample away
   accidental fives) → not detected

Step 8 is the property half: the corpus covers what you can imagine, the
proptest covers what you cannot.

## ML refresh: why win detection sits on the hot path

MCTS expands a leaf per simulation; every expansion asks "is this
terminal?" Target: 50M checks/s across the system (slice 9 verifies).
This is the same pattern as value-head evaluation in AlphaZero — the
*terminal* answer must be essentially free so the network budget goes to
non-terminal judgment. A slow `has_five` would tax every simulation;
that is the entire justification for bit tricks over your reference
engine's loops.

## Pitfalls

- `shr(2 * s)` with `s = 17` is 34 — fine. Never write `shr(4 * s)`.
- The wrap-attack test (7) must use *real* coordinates through `Board`,
  not hand-set bits, so it also exercises the stride-16 mapping.
- Do not "optimize" the staged AND before the slice-9 benchmark. Measure
  first.

## Done when

Corpus + proptest green, differential from slice 3 still green, gates
green. Commit: `feat(engine): staged shift-AND win detection`.

## Deep dives

The shift primitive this slice builds on — why a right shift is the right
operation, why the guard bit makes wrapping fives impossible, and which
hand-picked patterns are worth asserting — is derived in
[03-deep-dive/01 — Stride-16 and why `shr`](03-deep-dive/01-stride16-and-shr.md).

Why the AND is *staged* rather than a five-term chain (and how the naive
version lies in two of the four directions), why no per-direction edge
masks are needed while `!occupied` definitely needs one, and how to
benchmark this without measuring the optimizer:
[04-deep-dive/01 — The staged AND](04-deep-dive/01-staged-and-and-no-edge-masks.md).

Ready to type? [04-deep-dive/02 — implementation plan](04-deep-dive/02-implementation-plan.md)
turns this slice into red→green steps and ends with the complete reference
solution.

Next: [Slice 5 — Zobrist keys](05-zobrist.md)
