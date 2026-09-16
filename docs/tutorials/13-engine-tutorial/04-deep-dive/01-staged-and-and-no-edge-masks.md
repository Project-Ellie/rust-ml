# 01 — The staged AND, and why win detection needs no edge masks

Slice [04 — Win detection](../04-win-detection.md) hands you three functions
and one code sketch. This paper answers the questions the sketch raises:

- Why *staged* — two hops of length 2 and 4 — instead of the obvious
  five-term AND?
- Why does the whole thing need no per-direction edge masks, when almost
  every other bitboard algorithm does?
- Where is the mask needed, then — the design says "exactly one place";
  which, and what happens if you forget it?
- What does the ≥ 50M checks/s bar in slice 9 mean, and does this shape
  actually reach it?

Everything below was measured on the same machine (M5 Max) with a
stand-in harness over the real `bitboard.rs`; numbers are labelled where
they are illustrative rather than engine-benchmark output.

Series conventions live in [03-deep-dive/README.md](../03-deep-dive/README.md).

---

## 1. What the function is actually for

`has_five` answers two *different* questions, and the difference decides
which shape is right:

| caller | question | shape |
|--------|----------|-------|
| MCTS leaf expansion | "is this position terminal?" | whole board, any direction |
| `Board::play` | "did the stone I just placed win?" | the neighbourhood of one cell |

The design's `has_five_any(&Bitboard)` is the first shape — it takes a
whole colour's stones and asks whether a five exists anywhere. That is
what the milestone-1 benchmark measures, and slice 9 fixes the bar:

> **`has_five` ≥ 50M checks/s** (one call ≤ 20 ns)

Measured, per call, on 30-stone boards with no five:

```text
has_five_any (4 directions)      8.25 ns/call    121 M/s     <- the bar is 50 M/s
count_walk_any (loop scanner)  122.24 ns/call      8 M/s     <- the slice-3 interim
```

So the staged form clears the bar with ~2.4× margin, and it is **~15×
faster** than the loop scanner it replaces. Section 7 unpacks the two
shapes; section 8 explains why the numbers in this paragraph took three
attempts to measure honestly.

---

## 2. The obvious implementation is a silent bug

A run of five along a direction with index stride `s` is "stones at
`i, i+s, i+2s, i+3s, i+4s`". The direct translation:

```rust
let all = *b & b.shr(s) & b.shr(2 * s) & b.shr(3 * s) & b.shr(4 * s);
!all.is_zero()
```

With `DIRS = [1, 16, 15, 17]`, the largest shift is `4·17 = 68`. The
design says this "needs shifts up to 68 > 64"; the honest version is
worse than that, because the failure is **direction-dependent and
silent in release**. Measured, on genuine fives, release build:

```text
genuine five               staged    naive   4*s
horizontal (s=1)             true     true   4
vertical   (s=16)            true     true   64     <- "true" for the wrong reason
diag down-left (s=15)        true     true   60
diag down-right (s=17)       true    false   68     <- misses a real five
```

Three distinct outcomes, none of them a compile error:

- `s = 1` and `s = 15` (shifts 4 and 60) are **correct**. The naive chain
  works for two of the four directions, which is exactly what makes it
  dangerous to "optimize" into.
- `s = 16` asks for `shr(64)`. Rust's shift operators *mask* the shift
  amount, so `shr(64)` behaves like `shr(0)`, and the whole chain
  degenerates to `b & b & b & b & b = b`. Verified: `shr(64) == shr(0)`
  is `true`. A single stone then answers "true" — false positives on
  almost every board.
- `s = 17` asks for `shr(68)`, which masks to `shr(4)`: the words are
  shifted by the wrong amount and the AND chain misses real fives.

In a debug build the `debug_assert!(s > 0 && s < 64)` inside `shr` turns
all of this into a loud panic (`assertion failed: s > 0 && s < 64`) —
which is why the trap is survivable at all: your tests catch it, your
`--release` self-play does not. **Rule of thumb from this slice:** if a
bit trick's correctness depends on a shift amount staying under 64, then
the fact that it *compiles* tells you nothing.

---

## 3. Staging: reach 4·s without ever shifting 4·s

The fix is to build run lengths by doubling, so every individual shift is
small:

```rust
let two  = *b & b.shr(s);          // bit i set  <=>  stones at i and i+s          (run >= 2 here)
let four = two & two.shr(2 * s);   // bit i set  <=>  stones at i, i+s, i+2s, i+3s (run >= 4 here)
let five = four & four.shr(s);     // bit i set  <=>  stones at i .. i+4s          (run >= 5 here)
```

Each line is a pure identity on bit meanings — no geometry involved:

- `two` bit `i` = `b_i & b_{i+s}` (because `b.shr(s)` puts bit `i+s` at `i`).
- `four` bit `i` = `two_i & two_{i+2s}` = `b_i & b_{i+s} & b_{i+2s} & b_{i+3s}`.
- `five` bit `i` = `four_i & four_{i+s}` = five consecutive stones from `i`.

The two quantities that matter are easy to confuse, so keep them apart:

| quantity | value (s = 17) | meaning |
|---|---|---|
| largest single shift written | `2·s = 34` | what `shr` is asked for — must be < 64 |
| furthest index the chain reaches | `4·s = 68` | how far the *information* travels — may exceed 64 |

The chain reaches 68 in index space, two hops at a time. That is the
whole trick, and it is not a workaround: `1 → 2 → 4 → 5` is the shortest
addition chain to 5, so the staged form is also the *cheapest* one —
3 shifts and 3 ANDs per direction instead of 4 and 4. The 64-bit
constraint forced a better algorithm.

Cost per direction, on `[u64; 4]`: 3 × `shr` (each 4 words × 2 shifts +
1 or = 12 ops) + 3 ANDs (4 each) + `is_zero` (≈ 4 ors + test) ≈ **53 word
ops**. Measured: **1.90 ns** for one direction, **8.25 ns** for all four —
i.e. ~6 word-ops per cycle, which only makes sense if LLVM is keeping the
four words in two SIMD registers and doing the shifts two-at-a-time. The
`[u64; 4]` shape is not accidental: it is exactly 256 bits, and 256 bits
is exactly two NEON vectors on this machine.

---

## 4. Why no edge masks are needed

Almost every bitboard algorithm carries per-direction masks to stop runs
wrapping across a row boundary. This one does not, because of the padding
invariant from slice 3 (see [03-deep-dive/01](../03-deep-dive/01-stride16-and-shr.md)):

> column 15 of every row, and bits 240–255, are **always zero**.

A wrap is only possible if a run's 5-path crosses such a bit; padding
bits are zero, so the AND chain dies exactly there. The wrap-attack case
from the slice-4 checklist, traced cell by cell:

```text
stones: (7,12) (7,13) (7,14)                    and (8,0) (8,1)
the +1 path from (7,12):  (7,12) (7,13) (7,14) (7,15)=PADDING (8,0)
                                              ^ the chain dies here
```

That is one hand-built case. The claim itself is checkable by exhaustion,
so I checked it — over the whole index space, all four directions:

- **0** five-paths exist whose five bit indices are all inside `VALID`
  *and* whose cells are not collinear. (These would be silent false
  positives; there are none.)
- Paths that leave `VALID` and therefore die on padding:
  `s=1`: 91, `s=16`: 91, `s=15`: 135, `s=17`: 135.
- For completeness: a 15×15 board holds 572 genuine five-windows
  (165 horizontal + 165 vertical + 121 + 121 diagonal).

**The guard bit's value, measured.** Re-run the same enumeration with
stride 15 (no guard column) and the same `DIRS` table:

```text
stride 15, no guard bit  -> phantom (non-collinear) 5-paths per direction: {1: 56, 16: 161, 15: 165, 17: 157}
stride 16, column-15 guard -> phantom 5-paths per direction:            {1: 0, 16: 0, 15: 0, 17: 0}
```

539 phantom fives versus none. One wasted bit per row buys the entire
absence of edge handling — that is the trade the padding invariant is.

One consequence worth naming: **the `DIRS` table is stride-specific, not
geometric.** In stride 15 the same visual directions would be
`[1, 15, 14, 16]`; `[1, 16, 15, 17]` is geometry *times* the stride.
So `DIRS` and the layout are one design decision, not two.

---

## 5. Where the mask *is* needed: complements

The invariant survives because nothing ever sets a padding bit. Exactly
one operation can: `!`. So `empty_cells()` is `!occupied & VALID`, and
the rule is mechanical — *after every `!`, immediately `& VALID`*.

This is not hygiene, it is a live bug generator, and the failure has the
same shape as the wrap attack. Take a board whose only **empty** cells
form the wrap pattern (row 7 columns 12–14, row 8 columns 0–1). Then
`!occupied` sets bit 127 = (7,15) — padding, so zero in `occupied` — and
that phantom stone glues the two fragments into a "five":

```text
raw: bit 127 (row 7, col 15 = padding) set = true
masked:                                 set = false
has_five_any(&raw)    = true   <- phantom five across the row seam
has_five_any(&masked) = false  <- correct: no five among the empties
count_walk_any(&raw)  = false  <- the loop scanner was never exposed to this
```

Note the last line: code that walks *cells* cannot hit this, because it
never sees bit 127 as a cell. The bug class exists only for code that
does arithmetic on bits — which is precisely the code you are writing for
speed. This is why `assert_clean` is a proptest invariant rather than a
unit test: the dangerous states are produced by operator sequences you
would not think to write down, and the invariant catches them all.

---

## 6. Overlines for free — and what "≥" hides

`five` bit `i` means "at least five consecutive stones starting at `i`",
not "exactly five". A run of six therefore sets **two** bits (`i` and
`i+1`), a run of nine sets five. For a boolean answer that is exactly
chapter 13's decision 1 — overlines count, no extra code, no special case.

It stops being free the moment you *count* something:

- `five.count()` is not "number of wins"; it is "number of 5-windows",
  which grows with run length. Never expose it as a win count.
- A "how many distinct ways can this player win" API would need
  collapsing runs, i.e. `remove_duplicates`-style work — do not build it
  on this primitive.

Slice 7's `MoveSet`-returning detectors ask a per-move question instead
(`immediate_wins` looks for cells that *complete* a five), which is why
they are built on top of the primitive rather than on this count.

---

## 7. Two questions, two shapes (measured)

The same primitive serves the two rows of section 1's table at very
different costs:

```text
has_five_any (whole board, 4 directions)        8.25 ns/call   121 M/s
full recheck after placing one stone            7.60 ns/call   132 M/s
neighbourhood probe (walk out from the stone)   3.96 ns/call   252 M/s
count_walk_any (loop scanner, for scale)      122.24 ns/call     8 M/s
```

- **`Board::play` wants the probe.** After placing a stone, only lines
  through that stone can be new, so walking out from it in four
  directions (~8 cell tests per direction) is ~2× cheaper than
  recomputing the whole board — and it is the only shape that also tells
  you *which* line won.
- **MCTS wants the board check.** A leaf "is this terminal?" has no
  privileged cell; it must look everywhere.
- **The probe has a precondition**: it assumes exactly one stone was
  added. `Board::undo`, symmetry transforms, and the reference oracle's
  replay all violate that, so they must use the board form. (This is the
  same class of assumption as `has_five`'s "check only the colour just
  played" — true for `play`, not for arbitrary board arithmetic.)

The tactics module (slice 7) pays the board form 225 times per call:

```text
tactics-style: 225 x has_five_any(with mv)   1905.93 ns/call
```

≈ **1.9 µs per `immediate_wins`**, which is the measured translation of
the design's "[derived] ~225 × ~60 ops ≈ 13k ops per call". That is the
number behind "fine at leaf expansion, not per PUCT step": at ~1000
simulations per second per worker you cannot afford 1.9 µs per expansion
step, but as a priors/tactics aid called once per leaf it is noise.

---

## 8. How to benchmark bit twiddling without lying to yourself

The numbers in this paper took three harnesses. The same call,
`has_five_any` on a 30-stone board, measured:

```text
0.44 ns/call    naive loop: the compiler hoisted the whole call out
66.56 ns/call   black_box per call: correct, but the black_box barrier
                breaks vectorization and inflates the number ~8x
 8.25 ns/call   per-batch over 8 different boards, no per-call barrier
```

Two traps, in opposite directions:

1. **Loop-invariant code motion.** A pure function of an unchanged value
   is hoisted out of the timing loop even if you `black_box` its
   *result*. A benchmark that reports 0.4 ns for a 50-op computation —
   or `0.00 ns` for `has_five_any(&Bitboard::EMPTY)`, which is a
   constant and gets folded outright — is measuring the optimizer.
2. **`black_box` in the inner loop.** Blocking that hoist by boxing every
   call forces each result to materialize in memory, kills the
   register/SIMD scheduling, and over-measures by ~8×.

The fix is to make the *input* genuinely change: cycle over a handful of
distinct boards and time the whole batch. Slice 9's criterion bench
should do the same — `has_five_any` over several boards, `black_box` on
the input slice and on the accumulator, never on the individual call.
(For completeness: `#[inline(never)]`, `#[inline(always)]` and the
default made no measurable difference here — 66.6 / 71.9 / 72.4 ns in the
per-call harness — so call overhead was never the story.)

And read the target literally. Slice 9 defines a "check" as one
`has_five_any` call and the bar as ≤ 20 ns; at 8.25 ns this shape passes
with margin. The system-level "50M checks/s" of chapter 12 is then a
statement about 14 workers (≈ 1.7G checks/s available), not about one
core.

---

## 9. What to remember

- The naive 5-term AND is not slow-but-correct: shifts of 64 and 68 are
  *masked*, so two of four directions silently lie in release. Staging
  (`s, 2s, s`) is what keeps every shift under 64 — and happens to be the
  shortest addition chain as well.
- The padding invariant is what removes per-direction edge masks: 539
  phantom fives without it, 0 with it, on the same enumeration.
- Masks are still required after every `!`; the counterexample is the
  wrap pattern in the complement domain, where the padding bit glues two
  row fragments into a phantom five.
- "≥ 5" gives overlines for free, but makes the result mask a poor thing
  to count.
- Whole-board check (~8 ns) and neighbourhood probe (~4 ns) are different
  products for different callers; pick by who is asking.
- Benchmarks of pure bit functions need changing inputs; otherwise you
  measure LICM, in either direction.

---

Derivation of the shift primitive itself: [03-deep-dive/01 — Stride-16 and why `shr`](../03-deep-dive/01-stride16-and-shr.md).
Consumers: [slice 04](../04-win-detection.md), [slice 07 — tactics](../07-tactics.md),
benchmark and bar: [slice 09](../09-benchmarks-and-hardening.md).
