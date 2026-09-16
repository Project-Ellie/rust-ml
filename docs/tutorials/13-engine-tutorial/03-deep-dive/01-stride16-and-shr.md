# Why `shr`? — the stride-16 shift, derived from what it is for

*A short paper on `Bitboard::shr` in the Gomoku engine (slice 3,
`gomoku/crates/engine/src/bitboard.rs`). Written to be read top-down:
goal → representation → the trick → why a shift is the primitive →
mechanism → the invariants it depends on → contract → tests.*

Every concrete example below was checked against the real shift
semantics with a script (2000 random 4-word cases verified that the
four-line implementation equals a true 256-bit right shift).

---

## 1. What we are trying to achieve

The rules engine sits in the innermost loop of the whole system. During
self-play it answers the same questions millions of times per run, on 14
worker threads, while a neural network does the expensive thinking. Two
requirements follow:

1. **"Is this position decided?" must be nearly free.** If the rules
   engine is not negligible next to the net, self-play throughput dies.
2. **"Does *this* colour have five in a row?" must be answerable from a
   board representation alone** — not by replaying the move history, not
   by walking cells, so that the same primitive also serves tactic
   detection ("can I complete five here?") and position validation
   (`Board::from_position`).

The naive oracle (slice 2) answers this by walking: from a stone, look
at the four axes and count neighbours. Simple, obviously correct, and
full of per-cell bounds checks and unpredictable branches. It is the
*oracle* — it stays as the thing the fast version must agree with.

So the design goal for `has_five` is: **turn "are there five stones in a
line?" into branchless bit algebra over a handful of 64-bit words.**

## 2. The representation this forces

Bit algebra needs cells to be *bits*, at *linearly addressable,
uniformly strided* indices. Hence:

```rust
const fn idx(r: u8, c: u8) -> usize { r as usize * 16 + c as usize }
pub(crate) struct Bitboard(pub(crate) [u64; 4]);   // 256 bits, stride 16
```

| word | bit indices | rows stored as 16-bit slots |
|------|-------------|-----------------------------|
| `w[0]` | 0–63 | rows 0–3 |
| `w[1]` | 64–127 | rows 4–7 |
| `w[2]` | 128–191 | rows 8–11 |
| `w[3]` | 192–255 | rows 12–14 (slot 3 = bits 240–255 unused) |

One row slot, MSB → LSB:

```text
 bit:  15  14  13  12  ...   2   1   0
        ^   c14 c13 c12 ...  c2  c1  c0
        |
        guard bit (column 15) — always zero
```

**The stride is 16, the board is 15 wide.** That one spare bit per row is
not a rounding accident; it is the mechanism that makes the shift trick
safe (§5). Everything else in this paper follows from it.

## 3. The trick: five-in-a-row as AND-of-shifted-copies

If stones are bits, then "a stone with a neighbour to its right" is:

```text
b & b.shr(1)     → a bit set at every position that starts a run of ≥ 2
```

Because `b.shr(s)` has bit `j` set exactly when `b` had bit `j+s`, the
AND keeps `j` only if *both* `j` and `j+s` are stones. Repeat and you
count *runs*: 2 → 4 → 5.

Four directions are four values of `s`, because `idx = r*16 + c`:

| direction | `s` | what `+s` means in `(r, c)` |
|---|---|---|
| horizontal | 1 | `(r, c+1)` |
| vertical | 16 | `(r+1, c)` |
| diagonal ↗/↙ | 15 | `(r+1, c-1)` |
| diagonal ↘/↖ | 17 | `(r+1, c+1)` |

Each axis is covered once; the sign of the direction doesn't matter for
"is there a line of five".

### Why not just AND five shifted copies?

The naive expression is

```rust
// DON'T: shifts up to 4·17 = 68 — beyond the 64-bit range
five = b & b.shr(s) & b.shr(2*s) & b.shr(3*s) & b.shr(4*s);
```

`4 × 17 = 68`, and `u64 << 64` is not "zero", it is a panic in debug and
garbage in release. Dead end.

### The staged version (what the design uses)

Fold in stages: 2 → 4 → 5. Shifts are `s`, `2s`, `s` — maximum `2·17 =
34`, safely inside 64:

```rust
let two  = b   & b.shr(s);      // positions starting a run of ≥ 2
let four = two & two.shr(2*s);  // run of ≥ 2 + another run of ≥ 2, 2s apart = ≥ 4
let five = four & four.shr(s);  // ≥ 4 followed by one more stone = ≥ 5
!five.is_zero()
```

Bonus that the spec wanted anyway: an **overline counts** (six in a row
also satisfies "run of ≥ 5"), which is locked decision 1 — for free.

Verified on a vertical five at `(0,0)…(4,0)` (indices 0, 16, 32, 48, 64):

```text
two  → {(0,0), (1,0), (2,0), (3,0)}     each cell that starts a 2-run
four → {(0,0), (1,0)}                   each cell that starts a 4-run
five → {(0,0)}                          the start of the 5-run
```

## 4. What `shr` must mean, precisely

This is the whole specification — everything mechanical follows from it:

> **`b.shr(s)` has bit `j` set if and only if `b` has bit `j + s`.**
> Equivalently: the entire 256-bit number is shifted right by `s` bits;
> bits that fall below index 0 are discarded, and zeros enter at the top.

Note it is a *downward* shift in index space. On the board that means
"look at the cell one step **up** the index axis" — which is exactly what
`b & b.shr(s)` needs, because the run is then detected at its **lowest**
index (and `!five.is_zero()` doesn't care where).

Worked, verified examples (`cells_of(x)` lists set cells):

```text
stone at (4,0)   = idx 64  →  shr(16) → (3,0)   idx 48
stone at (1,1)   = idx 17  →  shr(1)  → (1,0)
                              shr(15) → (0,2)
                              shr(16) → (0,1)
                              shr(17) → (0,0)
```

Two of those cross a 16-bit row slot and the first one crosses a **word**
boundary (`w[1]` → `w[0]`) — see §5 for why that needs care.

## 5. The mechanism: a 256-bit shift out of four 64-bit shifts

There is no 256-bit primitive, so the shift is done word by word, and
each word must borrow the low bits of the word above it:

```rust
pub(crate) fn shr(&self, s: u32) -> Bitboard {
    debug_assert!(s > 0 && s < 64);
    let w = self.0;
    Bitboard([
        (w[0] >> s) | (w[1] << (64 - s)),   // word 0 also takes word 1's low s bits
        (w[1] >> s) | (w[2] << (64 - s)),
        (w[2] >> s) | (w[3] << (64 - s)),
        w[3] >> s,                          // nothing above word 3: zeros enter
    ])
}
```

Read the carry term carefully — it is the only subtle line:

- `w[k] >> s` shifts word `k` down by `s`, losing its own low `s` bits.
- Those lost bits are fine (they belong below the word), but word `k`'s
  **top `s` bits are now empty** and must be filled with word `k+1`'s
  **low `s` bits** — which is exactly `w[k+1] << (64 - s)`: shifting left
  by `64 - s` moves the low `s` bits up to the very top.
- `w[3] >> s` has no carry because there is nothing above the 256th bit;
  zeros entering the top is correct.
- Nothing above bit 255 with `w[0]`'s carry (there is no `w[-1]`), so
  bits pushed below index 0 simply vanish — also correct, since index
  `j = -1` does not exist.

### The shift-by-64 trap

`w[1] << (64 - s)` with `s == 0` becomes `w[1] << 64` — panic in debug,
UB-adjacent garbage in release. Likewise `s >= 64`. The design calls this
out explicitly in the slice-3 toolbox and guards it:

```rust
debug_assert!(s > 0 && s < 64);   // free in release, loud in tests
```

In practice the staged win detector never needs more than `2 · 17 = 34`.

### Why an inherent `fn shr`, not `impl Shr`

`std::ops::Shr` would let any generic code write `bb >> 0` or `bb >> 70`
and hit the trap silently. An inherent method keeps the contract on the
type, documents `0 < s < 64` in the signature's neighbourhood, and is not
part of the public API (`pub(crate)`) — `shr` is an implementation
detail of the engine, not a service offered to other crates.

## 6. Why the padding bit is what makes this legal

The trick has a failure mode: **wraps**. A shift does not know about
rows. `(r, 14) + 1` is row `r`'s guard bit, not `(r+1, 0)`, and
`(r, 0) + 15` is row `r`'s guard bit, not `(r-1, 1)`. Without a guard
bit there, a chain could "turn the corner" and produce a five that does
not exist on the board.

Table of wrap candidates and what stops each one (all four checked):

| direction `s` | would-be phantom neighbour | index arithmetic | stopped by |
|---|---|---|---|
| 1 | `(r,14) → (r+1,0)` | `16r+14 → 16r+15` | row `r`'s guard bit (15) |
| 16 | — | `16r+c → 16(r+1)+c` | nothing needed (pure stride) |
| 15 | `(r,0) → (r-1,1)` | `16r+0 → 16r+15` | row `r`'s guard bit (15) |
| 17 | `(r,14) → (r+2,0)` | `16r+14 → 16(r+1)+15` | row `r+1`'s guard bit (15) |

Because guard bits are **always zero**, the phantom pairing dies in the
AND. Verified: four stones at row 0, columns 11–14 plus a stone at
`(1,0)` — the kind of thing that would be a five if the chain could jump
rows — is correctly **not** a five.

### The sanitising property (why an intermediate shift may be "sloppy")

A shift *can* deposit a real stone onto a guard bit: `(1,14)` is index 30,
and `30 - 15 = 15` = `(0,15)`, the guard bit. Verified:

```text
cells(b)             = {(1,14)}
cells(b.shr(15))     = {(0,15)}     ← lands on a guard bit, "dirty"
cells(b & b.shr(15))= {}            ← the AND cleans it up
```

This is not luck, it is an induction, and it is worth internalising:

- `b` (and any board living in the engine) has **zero guard bits**.
- Every intermediate is `X & X.shr(s)` for some `X` that already has zero
  guard bits. For the AND to set guard bit `g`, `X` would need bit `g`
  set — it doesn't. So `two`, `four`, `five` all have zero guard bits.
- Therefore the *un-shifted* operand in each AND re-asserts the invariant
  at every stage. **The shifts are allowed to be geometrically sloppy;
  the ANDs are what enforce the geometry.**

### Two kinds of padding, and why `& VALID` is mandatory after `!`

| padding | count | purpose |
|---|---|---|
| guard bit per row (column 15) | 15 | geometry: kill wrap-around |
| unused high bits (240–255) | 16 | hygiene: nothing real lives there |

`256 − 31 = 225` real cells. Both kinds must stay zero, and the reason is
complement: `!occupied` sets *all* padding bits to one. Guard bits set to
one would resurrect exactly the phantom fives of the table above, and a
set bit at 240–255 would be dragged *down* into real cells by the next
right shift. Hence the locked rule from the design:

```rust
fn empty_cells(&self) -> Bitboard { /* !occupied, then & VALID */ }
// invariant, proptest-checked after random op sequences:
fn assert_clean(b: &Bitboard) { assert!((*b & !VALID).is_zero()); }
```

And therefore `VALID` — the mask of all 225 real cells, `0x7fff` per row
slot, `0x0000_7fff_7fff_7fff` for the tail word: the guard column is bit
15, the MSB of the slot, because `idx = r*16 + c` numbers columns from
the *low* end of the word.

## 7. Cost model (why this is fast, honestly)

Per direction: 3 `shr` calls + 3 `Bitboard` ANDs.

- one `shr` = 4 word shifts + 3 ORs ≈ 7 ALU ops
- one AND = 4 word ops
- → per direction ≈ `3·7 + 3·4` = **33 ops**, four directions ≈ 132 ops
  worst case, on 32-byte operands that stay in registers/L1
- **no branches and no bounds checks** inside a direction, and
  `DIRS.iter().any(...)` short-circuits on the first hit (most wins are
  found in direction 1 or 2, and most positions are decided by the
  common "does the stone just placed complete something" question)

The naive oracle does fewer *logical* operations but pays up to 32
bounds-checked loads and unpredictable branches per call. Branchless
uniformity is the actual win, and slice 9's criterion benchmark is what
decides the argument (acceptance: `has_five` ≥ 50M checks/s).

## 8. Testing `shr` (and a caveat about the slice-3 checklist)

Good patterns — every one of these is verified above:

| case | assertion |
|---|---|
| `(1,1)` shr 1 / 15 / 16 / 17 | lands at `(1,0)`, `(0,2)`, `(0,1)`, `(0,0)` |
| **word-boundary carry** | `(4,0)` = idx 64 shr 16 → `(3,0)` = idx 48 (uses the `w[1] << 48` path) |
| row-slot crossing | `(1,1)` shr 1 → `(1,0)`; `(3,14)` shr 1 → guard bit → empty |
| fall-off | any stone at idx < s disappears; `EMPTY.shr(s).is_zero()` |
| random property | for random `b`, `s`: result equals a reference 256-bit shift (big-int in the test, or compare against `shl` on the reversed word order) |

**Caveat.** The slice-3 checklist asks for `shr` correctness on
hand-picked patterns, naming `(0,0)` as the stone. Read that as a
reminder of the *shape* of the test, not a literal recipe: under `shr` a
stone at index 0 is the bottom of the address space, so all four results
are `EMPTY` (verified) — the assertion passes for the wrong reason. Use
`(1,1)` and the word-boundary case from the table above instead; a stone
at `(0,0)` only becomes interesting as the *target* of a shift, never as
its source.

## 9. The derivation in one chain

```text
"is the position decided?" must be ~free in the self-play hot loop
  → express five-in-a-row as bit algebra, not cell walking
  → one bit per cell, index = r*16 + c (stride 16: uniform, shiftable)
  → a run is "AND of copy shifted by the direction constant s"
  → naive 5-term AND needs 4·17 = 68 bits of shift  ✗ (u64 max 63)
  → staged 2→4→5 folding: max shift 2·17 = 34        ✓ (overlines free)
  → therefore: a whole-256-bit right shift = shr(s)
  → 256-bit shift = per-word shift + carry from the next word up
  → shifts can cross rows  → stride 16's spare bit per row is the guard
  → complement sets padding      → every complement ANDs with VALID
  → s = 0 or s ≥ 64 breaks `<< (64-s)`  → debug_assert!(0 < s < 64)
```

Cheat sheet — "what you want" → "what you write":

| you want | you write |
|---|---|
| cell `(r,c)` as an index | `idx(r, c)` |
| all stones of a colour | `Board::stones(color) -> &Bitboard` |
| "is there a five anywhere?" | `DIRS.iter().any(|s| has_five_dir(b, *s))` |
| "would *this* stone make five?" | `has_five_any(&stones.with_bit(idx))` |
| neighbours at distance `s` | `b.shr(s)` |
| board with padding guaranteed clean | `x & VALID` after any `!` |

## 10. Where `shr` is *not* the tool

- **Symmetry (D4)** is planned as const-generated index-permutation
  tables (slice 6, `[[u8; 225]; 8]`). Rotations/reflections are not
  uniform strides, so shifts are the wrong primitive there.
- **Tactics** (slice 7) does not need its own shifts: `immediate_wins` /
  `double_threats` reuse the `has_five` primitive with a hypothetical
  stone placed — which is why `has_five` must be a *pure function of a
  Bitboard*, not of the move history.
- **Encoding** (slice 6) works on `u8` planes, not bitboards.

So `shr` has exactly one reason to exist — win detection — and it earns
its keep by being correct, branchless, and reusable for the tactic
queries that are already written on top of `has_five`.
