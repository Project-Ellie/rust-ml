# 01 — The staged AND, and why win detection needs no edge masks

Slice [04 — Win detection](../04-win-detection.md) specifies three
functions and one code sketch. This paper derives that sketch and states
its measured properties. Terminology follows the
[tutorial glossary](../README.md#glossary) throughout.

Contents: the two caller questions (§1) → the naive five-term AND and
why it fails (§2) → the staged construction (§3) → why no edge masks
are needed (§4) → where a mask is needed (§5) → overlines (§6) → the
two caller shapes, measured (§7) → summary (§8). The benchmark
methodology behind every number is in the appendix.

All measurements were taken on the same machine (M5 Max) with a
stand-in harness over the real `bitboard.rs`; numbers are illustrative,
not engine-benchmark output. Series conventions live in
[03-deep-dive/README.md](../03-deep-dive/README.md).

---

## 1. Two callers, two questions

Win detection serves two different callers:

| caller | question | shape |
|--------|----------|-------|
| MCTS leaf expansion | "is this position terminal?" | **whole-board check**: examine one colour's stones, any direction |
| `Board::play` | "did the stone just placed win?" | **neighbourhood check**: examine only lines through one cell |

The design's `has_five_any(&Bitboard)` is the whole-board check. That is
the shape the milestone-1 benchmark measures, and slice 9 fixes the bar:

> **`has_five` ≥ 50M checks/s** (one call ≤ 20 ns; a "check" is one
> `has_five_any` call)

Section 7 covers the neighbourhood check and its precondition. The rest
of the paper derives the whole-board check.

---

## 2. The naive five-term AND, and why it fails

A run of five along a direction with index stride `s` is "stones at
`i, i+s, i+2s, i+3s, i+4s`". The direct translation:

```rust
let all = *b & b.shr(s) & b.shr(2 * s) & b.shr(3 * s) & b.shr(4 * s);
!all.is_zero()
```

With `DIRS = [1, 16, 15, 17]`, the largest shift is `4·17 = 68`. Rust's
shift operators mask the shift amount to the low 6 bits rather than
rejecting large shifts, so the failure is direction-dependent and, in a
release build, silent. Measured on genuine fives, release build:

```text
genuine five               staged    naive   4*s
horizontal (s=1)             true     true    4
vertical   (s=16)            true     true   64     <- true for the wrong reason
diag down-left (s=15)        true     true   60
diag down-right (s=17)       true    false   68     <- misses a real five
```

Three distinct outcomes, none of them a compile error:

- `s = 1` and `s = 15` (shifts 4 and 60) are correct. The naive chain
  works for two of the four directions, which is what makes it
  dangerous: it passes tests that exercise only those directions.
- `s = 16` asks for `shr(64)`. The masked shift amount makes `shr(64)`
  behave like `shr(0)` (verified directly), so the chain degenerates to
  `b & b & b & b & b = b`. A single stone answers "true": false
  positives on almost every board.
- `s = 17` asks for `shr(68)`, which masks to `shr(4)`: the words are
  shifted by the wrong amount and the chain misses real fives.

In a debug build the `debug_assert!(s > 0 && s < 64)` inside `shr` turns
all of this into a panic, so tests catch it. A `--release` self-play run
does not. Rule of thumb: if a bit trick's correctness depends on a
shift amount staying under 64, the fact that it compiles carries no
information.

---

## 3. The staged construction

The fix is to build run lengths by doubling, so every individual shift
is small:

```rust
let two  = *b & b.shr(s);          // bit i set  <=>  stones at i and i+s          (run >= 2 here)
let four = two & two.shr(2 * s);   // bit i set  <=>  stones at i, i+s, i+2s, i+3s (run >= 4 here)
let five = four & four.shr(s);     // bit i set  <=>  stones at i .. i+4s          (run >= 5 here)
```

Each line is an identity on bit meanings:

- `two` bit `i` = `b_i & b_{i+s}` (because `b.shr(s)` places bit `i+s`
  at position `i`).
- `four` bit `i` = `two_i & two_{i+2s}` = `b_i & b_{i+s} & b_{i+2s} & b_{i+3s}`.
- `five` bit `i` = `four_i & four_{i+s}` = five consecutive stones
  starting at `i`.

Two quantities must be kept apart:

| quantity | value (s = 17) | meaning |
|---|---|---|
| largest single shift written | `2·s = 34` | what `shr` is asked for — must be < 64 |
| furthest index the chain reaches | `4·s = 68` | how far the information travels — may exceed 64 |

The chain reaches 68 in index space, two hops at a time. Additionally,
`1 → 2 → 4 → 5` is the shortest addition chain to 5, so the staged form
is also the cheapest: 3 shifts and 3 ANDs per direction instead of 4 and
4. The 64-bit constraint forced the faster algorithm.

Cost per direction, on `[u64; 4]`: 3 × `shr` (each 4 words × 2 shifts +
1 OR = 12 word ops) + 3 ANDs (4 word ops each) + `is_zero` (≈ 4 ORs +
test) ≈ **53 word ops**. Measured: **1.90 ns** for one direction,
**8.25 ns** for all four — about 6 word ops per cycle, which indicates
LLVM keeps the four words in two SIMD registers and shifts two at a
time. The `[u64; 4]` shape is exactly 256 bits, and 256 bits is exactly
two NEON vectors on this machine. (53 word ops in 1.90 ns is ≈ 6 ops
per cycle at ~4.5 GHz, ≈ 8.5 cycles per direction.)

---

## 4. Why no edge masks are needed

Most bitboard algorithms carry per-direction masks to stop runs wrapping
across a row boundary. This one does not, because of the padding
invariant (derived in
[03-deep-dive/01](../03-deep-dive/01-stride16-and-shr.md)):

> The padding column (column 15 of every row) and the high padding
> (bits 240–255) are always zero.

A wrap is only possible if a 5-window's index path crosses a padding
bit; padding bits are zero, so the AND is zero at that position. The
wrap-attack case from the slice-4 checklist, traced cell by cell:

```text
stones: (7,12) (7,13) (7,14)                and (8,0) (8,1)
the +1 path from (7,12):  (7,12) (7,13) (7,14) (7,15)=PADDING (8,0)
                                              ^ the AND is zero here
```

That is one hand-built case. The general claim is checkable by
exhaustion over the whole index space, all four directions:

- **0** 5-windows exist whose five bit indices are all inside `VALID`
  and whose cells are not collinear. (These would be phantom fives —
  silent false positives. There are none.)
- 5-windows that leave `VALID` and are therefore zeroed by padding:
  `s=1`: 91, `s=16`: 91, `s=15`: 135, `s=17`: 135.
- For scale: a 15×15 board holds 572 genuine 5-windows (165 horizontal
  + 165 vertical + 121 + 121 diagonal).

**The padding column's value, measured.** The same enumeration with
stride 15 (no padding column; the same visual directions are now
`s = 1, 15, 14, 16` — the direction table is stride-specific, see below):

```text
stride 15 (s = 1, 15, 14, 16) -> phantom 5-windows per direction: {1: 56, 15: 0, 14: 48, 16: 40}
stride 16 (s = 1, 16, 15, 17) -> phantom 5-windows per direction: {1: 0, 16: 0, 15: 0, 17: 0}
```

144 phantom fives without the padding column, zero with it. One unused
bit per row removes the entire category of edge handling.

One consequence: **the `DIRS` table is stride-specific, not geometric.**
In stride 15 the same visual directions would be `[1, 15, 14, 16]`;
`[1, 16, 15, 17]` is the geometry multiplied by the stride. `DIRS` and
the layout are one design decision, not two.

---

## 5. Where the mask is needed: complements

The invariant holds because nothing ever sets a padding bit. Exactly one
operation can: `!`. Hence `empty_moves` computes `!occupied & VALID`,
and the rule is mechanical: after every `!`, immediately `& VALID`.

The failure mode has the same shape as the wrap attack. Take a board
whose only **empty** cells form the wrap pattern (row 7 columns 12–14,
row 8 columns 0–1). Then `!occupied` sets bit 127 = (7,15) — padding,
therefore zero in `occupied` — and that phantom stone connects the two
fragments into a five:

```text
raw: bit 127 (row 7, col 15 = padding) set = true
masked:                                set = false
has_five_any(&raw)    = true   <- phantom five across the row seam
has_five_any(&masked) = false  <- correct: no five among the empties
walk-based oracle(&raw) = false
```

Note the last line: code that walks cells cannot hit this case, because
it never sees bit 127 as a cell. The bug class exists only for code that
does arithmetic on bits — which is the code written for speed. This is
why `assert_clean` is a proptest invariant rather than a unit test: the
dangerous states are produced by operator sequences no one would think
to write down, and the invariant catches all of them.

---

## 6. Overlines, and what "≥" hides

`five` bit `i` means "at least five consecutive stones starting at `i`",
not "exactly five". A run of six therefore sets two bits (`i` and
`i+1`); a run of nine sets five. For a boolean answer this implements
chapter 13's decision 1 — overlines count — with no extra code.

It stops being free the moment something is counted:

- `five.count()` is not "number of wins"; it is the number of
  5-windows, which grows with run length. It must not be exposed as a
  win count.
- A "how many distinct winning lines exist" API would need to collapse
  runs before counting; it cannot be built on this primitive directly.

Slice 7's `MoveSet`-returning detectors ask a per-move question instead
(`immediate_wins` looks for cells that *complete* a five), which is why
they are built on top of the primitive rather than on its popcount.

---

## 7. The two caller shapes, measured

The same primitive serves the two rows of §1's table at different costs:

```text
has_five_any (whole-board check, 4 directions)   8.25 ns/call   121 M/s
full recheck after placing one stone             7.60 ns/call   132 M/s
neighbourhood check (walk out from the stone)    3.96 ns/call   252 M/s
walk-based oracle (for scale)                  122.24 ns/call     8 M/s
```

- **`Board::play` wants the neighbourhood check.** After placing a
  stone, only lines through that stone can be new, so walking out from
  it in four directions (~8 cell tests per direction) is ~2× cheaper
  than recomputing the whole board — and it is the only shape that also
  reports which line won.
- **MCTS wants the whole-board check.** A leaf "is this terminal?" has
  no privileged cell; it must look everywhere.
- **The neighbourhood check has a precondition**: exactly one stone was
  added since the last check. `Board::undo`, symmetry transforms, and
  the reference oracle's replay all violate it, so those paths must use
  the whole-board check. (This is the same class of assumption as
  "check only the mover's colour" — true for `play`, not for arbitrary
  board arithmetic.)

The tactics module (slice 7) pays the whole-board check 225 times per
call:

```text
tactics-style: 225 x has_five_any(with mv)   1905.93 ns/call
```

≈ **1.9 µs per `immediate_wins`**, the measured translation of the
design's "[derived] ~225 × ~60 ops ≈ 13k ops per call". At ~1000
simulations per second per worker, 1.9 µs per expansion step is not
affordable, but as a priors/tactics aid called once per leaf it is
noise.

---

## 8. Summary

- The naive five-term AND is not slow-but-correct: shifts of 64 and 68
  are masked, so it answers incorrectly for two of four directions in
  release builds. The staged AND (`s, 2s, s`) keeps every shift under
  64 — and is the shortest addition chain to 5.
- The padding invariant removes per-direction edge masks: 144 phantom
  fives without it, 0 with it, on the same enumeration.
- A mask is still required after every `!`; the counterexample is the
  wrap pattern in the complement domain, where a padding bit connects
  two row fragments into a phantom five.
- "≥ 5" implements the overline rule for free, but makes the result
  mask unsuitable for counting wins.
- The whole-board check (~8 ns) and the neighbourhood check (~4 ns) are
  different shapes for different callers; the choice is determined by
  who is asking.
- The slice-9 bar (one `has_five_any` call ≤ 20 ns) is met at 8.25 ns,
  ~2.4× margin, ~15× faster than the walk-based oracle it replaces. The
  system-level "50M checks/s" of chapter 12 is then a statement about
  14 workers (≈ 1.7G checks/s available), not about one core.

---

## Appendix — how these numbers were measured

Benchmarks of pure bit functions go wrong in two opposite directions,
and the numbers above required three harnesses to get right. The same
call, `has_five_any` on a 30-stone board, measured:

```text
0.44 ns/call    naive loop: the compiler hoisted the whole call out
66.56 ns/call   black_box per call: correct, but the black_box barrier
                breaks vectorization and inflates the number ~8x
 8.25 ns/call   per-batch over 8 different boards, no per-call barrier
```

1. **Loop-invariant code motion.** A pure function of an unchanged
   value is hoisted out of the timing loop even if its *result* is
   passed through `black_box`. A benchmark reporting 0.4 ns for a
   50-word-op computation — or `0.00 ns` for
   `has_five_any(&Bitboard::EMPTY)`, which is a constant and is folded
   outright — is measuring the optimizer.
2. **`black_box` in the inner loop.** Blocking the hoist by boxing
   every call forces each result to materialize in memory, prevents
   register/SIMD scheduling, and over-measures by ~8×.

The working method is to make the input genuinely change: cycle over a
handful of distinct boards and time the whole batch. Slice 9's
criterion bench should do the same — `has_five_any` over several
boards, `black_box` on the input slice and on the accumulator, never on
the individual call. For completeness: `#[inline(never)]`,
`#[inline(always)]`, and the default made no measurable difference here
(66.6 / 71.9 / 72.4 ns in the per-call harness), so call overhead was
never a factor.

---

Derivation of the shift primitive itself: [03-deep-dive/01 — Stride-16 and why `shr`](../03-deep-dive/01-stride16-and-shr.md).
Consumers: [slice 04](../04-win-detection.md), [slice 07 — tactics](../07-tactics.md),
benchmark and bar: [slice 10](../10-benchmarks-and-hardening.md).
