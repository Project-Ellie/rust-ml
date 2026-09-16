# `empty_moves()` — why that signature, why that loop, and what `+ '_` means

*A paper on `Board::empty_moves` in the Gomoku engine (slice 3). Same
approach as the `shr` paper: start from what the function is for, then
derive the design and check every claim against measurement.*

Prerequisite reading: [Stride-16 and why `shr`](01-stride16-and-shr.md)
(stride-16 layout, guard bits, `VALID`).

---

## 0. The answer in five lines

1. `empty_moves(&self) -> impl Iterator<Item = Move> + '_` is **lazy on
   purpose**: the callers that matter want *one* move far more often than
   they want all 225, and laziness makes "the next empty cell" cost
   ~0.2 ns instead of ~110 ns.
2. It returns **`Move` (logical, stride-15)**, not a bit index, because
   the bitboard's stride-16 layout is a private implementation detail. A
   bit index in the public API would leak it.
3. The loop is **unavoidable** — someone has to name each empty cell —
   but it is **cheap per item** (0.3–0.5 ns, ~1.4 cycles) and it iterates
   over *results*, not over cells: whole words of occupied cells are
   skipped with one `&`.
4. The inevitable full sweep is **not the hot path**: it happens once per
   MCTS node expansion, next to a neural-network evaluation three orders
   of magnitude more expensive. Bulk questions ("how many empties?") have
   a bit-level answer that costs 0.6 ns, which is why `empty_cells()`
   exists next to `empty_moves()`.
5. `+ '_` says the returned iterator **borrows the board** — it is a
   view, not a snapshot. Since Rust 2024 that is the default and the
   annotation is redundant, but it documents the contract and it is what
   makes `for mv in b.empty_moves() { b.play(mv) }` a compile error
   instead of a logic bug.

**A note on a conflict this paper helped settle.** The design chapter
used to say "`MoveSet` is what tactics and `empty_moves` return", while
this tutorial's slice-3 contract declares
`impl Iterator<Item = Move> + '_`. One of the two had to give. §3 records
the trade-off and the resolution (lazy iterator stays; the eager set is
one `.collect()` away, and the design sentence now says so).

---

## 1. What the function is for

Four consumers, and they ask *different* questions:

| consumer | question it asks | how often |
|---|---|---|
| MCTS node expansion | "all legal moves, to build children/priors" | once per expanded node |
| tactic fast path (`immediate_wins`) | "which empties would complete a five?" | once per position (config knob) |
| `is_legal` / tests / draw detection | "is this cell empty?" and "how many are empty?" | very often |
| Swap2 validation, CLI, debugging | "list the empties" | rare |

Those collapse into two kinds of request:

- **bulk / bit-level**: *how many*, *is this one empty*, *which empties
  are inside this mask?*
- **enumeration**: *give me the empty cells, one at a time, lazily*

Two primitives, one per kind — and that is exactly what the design has:

```rust
fn empty_cells(&self) -> Bitboard;                        // bulk, stride-16, pub(crate)
pub fn empty_moves(&self) -> impl Iterator<Item = Move> + '_;   // enumeration, public
```

Trying to serve both with one function is what produces either an
allocation you did not want (`Vec<Move>` always) or a loop you cannot
avoid (a `&[Move]` cache that must be maintained on every play/undo).
The split is the design.

---

## 2. The signature, token by token

```rust
pub fn empty_moves(&self) -> impl Iterator<Item = Move> + '_
```

### 2.1 `pub fn … (&self)`

- `pub`: the consumer that needs it (MCTS, `mcts` crate) is **outside the
  engine crate** — ch. 12 splits the workspace that way. So it cannot be
  `pub(crate)`.
- `&self`, not `&mut self`: enumeration does not mutate, and the shared
  borrow composes — you can hold the iterator *and* call
  `board.stones(color)`, `board.status()` etc. in the same expression. A
  `&mut self` version would make `for mv in b.empty_moves() { … b.status() … }`
  illegal for no benefit.
- It also means two threads with `&Board` can both enumerate, which is
  the shape the self-play workers want.

### 2.2 `impl Iterator<Item = Move>`

The alternatives, and what each costs. ("nameable" = can the caller write
the returned type in a `struct` field or a function parameter.)

| return type | allocation | early exit | nameable | set algebra | notes |
|---|---|---|---|---|---|
| `Vec<Move>` | yes, every call | no | yes | after conversion | 225 B + header per call, per node, × 14 threads (allocator contention) |
| `&[Move]` | no | yes | yes | no | requires a *cached* empty-list field kept in sync by play/undo — an invariant you would have to test forever |
| `Box<dyn Iterator<Item = Move>>` | yes (1 box) | yes | yes | no | per-item vtable dispatch, no inlining — the wrong tool inside the engine (right tool in a UI, see the CLI tutorial) |
| `MoveSet` | no | no (eager) | yes | **yes** | 32-byte `Copy` value; stride-15; must materialise all 225 bits → the first legal move costs the same as the last |
| `impl Iterator<Item = Move> + '_` | **no** | **yes** | no (opaque) | no | what slice 3 declares |

Why `impl Iterator` and not `MoveSet`: the enumeration consumers are
**lazy or early-exiting** — MCTS rollouts and the tactics loop often stop
at the first hit (`find`, `any`, `take(1)`), and the CLI/tests just walk.
Measured (§5): the first empty cell costs **0.2 ns**; materialising all
225 costs **110 ns**. You cannot un-materialise an eager set, but you can
always materialise a lazy iterator (`b.empty_moves().collect()`), so the
iterator is the strictly more general primitive and `MoveSet` is a
*projection* of it. §3 returns to this.

Why `Item = Move` and not `usize`: `Move` is the validated public
vocabulary (`Move::new` rejects off-board cells, and its index is the
*logical* `r*15 + c`). Yielding the stride-16 bit index would (a) leak
the private layout into the public API, so every consumer would start
reasoning in stride-16, and (b) bypass the "convert at the boundary, in
exactly one place each way" rule.

Why not `impl Iterator<Item = (u8, u8)>`: same argument; `Move` is the
newtype that makes invalid cells unrepresentable (§ slice-2 toolbox).

### 2.3 What "opaque type" buys and costs

`impl Trait` in return position is an **opaque type**: the compiler knows
the concrete type, callers do not. Consequences, verified:

- ✅ You can still consume it freely: `.count()`, `.collect::<Vec<_>>()`,
  `.take(n)`, `.find(…)`, `.chain(…)`, and `Box::new(it)` all work — it
  is `Sized`, you just cannot *name* it.
- ❌ You cannot store it in a struct field or return it from a function
  without boxing or collecting — the type has no name.
- ❌ You cannot use capabilities you did not promise. Verified:
  `b.empty_moves().len()` → `error[E0599]: no method named 'len' found
  for opaque type 'impl Iterator<Item = u8> + '_'`. Only what is in the
  bounds is available; `ExactSizeIterator` is *not* promised, so `.len()`
  is not there. (`Iterator::count()` works — it is in the bounds — it
  just walks.)
- ✅ Auto traits leak: the returned iterator is inferred `Send`/`Sync`
  when the hidden type is (verified with `fn assert_send<T: Send>(_: T)`),
  so the workers can use it without you writing `+ Send`. This is
  deliberate Rust behaviour, not an accident you rely on by accident —
  but it *is* why no `+ Send` appears in the signature.

### 2.4 `+ '_` — the lifetime annotation

**What it denotes.** `&self` elides to some anonymous lifetime; `'_` in
the return type's bounds resolves to *that* lifetime (standard elision:
one input lifetime → output elides to it). So the signature says: *"the
returned iterator borrows the board"*.

**Why the borrow exists at all.** The implementation reads the board's
bitboards lazily, so the iterator holds a `&Board` (or a borrowed word
slice). Two designs are possible — and this is the real decision behind
the annotation:

| design | hidden type | needs `'_`? |
|---|---|---|
| iterator borrows the board | `EmptyMoves<'a> { board: &'a Board, … }` | yes (it captures `'_`) |
| iterator owns a copy of the 4 words | `EmptyMoves { words: [u64; 4], … }` (`Bitboard` is `Copy`) | no — no borrow to capture |

I verified both compile (`owns_capt`: an owning iterator *with* `+ '_`
also compiles — over-capturing is allowed).

**Why the design chose to borrow** — three reasons:

1. It is the honest description: the values yielded depend on board state
   that can change. An owning snapshot silently diverges: iterate, play a
   move, keep consuming, and you receive moves that are no longer empty.
2. The borrow makes the *dangerous* pattern a compile error:
   ```rust
   for mv in board.empty_moves() { board.play(mv).unwrap(); }  // E0502
   ```
   cannot be written, because `play` needs `&mut self` while the iterator
   holds `&self`. With an owning snapshot this compiles and misbehaves —
   a bug that would be very hard to see, because it produces *plausible*
   moves.
3. It costs nothing: the board is `&Board` either way when the caller has
   one, and copying 32 bytes versus borrowing 8 is not a real difference.

**Why write it if Rust 2024 captures it anyway?** Verified behaviour:

| edition | `-> impl Iterator<Item = u8>` over `&self` |
|---|---|
| 2021 | `error[E0700]: hidden type for 'impl Iterator<Item = u8>' captures lifetime that does not appear in bounds` |
| 2024 | compiles (precise capturing: RPIT captures all in-scope lifetimes by default) |

So in this workspace (edition 2024) the annotation is **no longer
required by the compiler** — but it stays in the signature because:

- it documents the contract where readers look (nobody should have to
  know the edition's capturing default to understand the type),
- it is edition-portable: the same signature compiles in 2018/2021/2024,
  so the engine can move editions without touching this file,
- it makes intent explicit against the alternative: `+ use<>` means
  *"captures nothing"* — verified to fail here with E0700, which is
  exactly the error you want if someone later rewrites the iterator to
  own a snapshot and forgets that the public type promised a borrow.

Read the annotation as a sentence: *"an iterator of moves, which is a
view of the board it was asked of."*

---

## 3. `MoveSet` or iterator? (the trade-off, and how it was settled)

The design chapter's bird's-eye description of the two bit types said:

> `MoveSet` is what tactics and `empty_moves` return; MCTS iterates it.

The slice-3 contract — the thing you actually implement — says something
else:

```rust
pub fn empty_moves(&self) -> impl Iterator<Item = Move> + '_;   // slice 3
```

Both cannot be true, so the trade-off had to be argued explicitly.

**For `MoveSet`:** it is `Copy`, nameable, and supports *set algebra* —
`empty ∩ candidate_mask`, `immediate_wins | forced_blocks`. The engine
cannot expose `empty_cells() -> Bitboard` publicly (`Bitboard` is
`pub(crate)`), so if an outside crate (MCTS) ever needs bit-parallel
candidate filtering, `MoveSet` is the only public vehicle. Tactics
already return `MoveSet`, so the API would be uniform.

**For the lazy iterator:** everything in §2.2 — first item 0.2 ns versus
110 ns, no allocation, and materialisation is one `.collect()` away.
Also note the asymmetry: going *from* the iterator *to* a set is trivial
(logical index → bit index is the identity in stride-15 space), while
going from a set back to laziness is impossible — you have already paid.

**Resolution.** The iterator stays; the eager set becomes a projection of
it, one line away:

```rust
// moveset.rs — makes the eager projection explicit and one line away
impl MoveSet {
    pub fn from_empties(board: &Board) -> MoveSet { board.empty_moves().collect() }
}
impl FromIterator<Move> for MoveSet {
    fn from_iter<T: IntoIterator<Item = Move>>(iter: T) -> MoveSet {
        let mut set = MoveSet::EMPTY;
        for mv in iter { set.insert(mv); }
        set
    }
}
```

Then the design sentence reads: *"`MoveSet` is what tactics return, and
what you get by collecting `empty_moves()`."* If MCTS later turns out to
need masks, add `empty_set()`/masked iteration *then* — which is the
engine's own rule: no abstraction before a consumer needs it.

---

## 4. The loop: how it has to be written

Enumeration cannot be avoided — but *what* is enumerated can. The loop
runs over **set bits of the complement**, 64 cells at a time:

```rust
// board.rs
struct EmptyMoves<'a> {
    board: &'a Board,
    word: usize,   // next word to load, 0..=4
    bits: u64,     // remaining empty bits of the currently loaded word
    base: usize,   // bit index of that word's LSB (word * 64)
}

impl Iterator for EmptyMoves<'_> {
    type Item = Move;

    fn next(&mut self) -> Option<Move> {
        while self.bits == 0 {                     // nothing left in this word
            if self.word == 4 { return None; }     // all 256 bits considered
            let occupied = (self.board.black | self.board.white).0[self.word];
            self.bits = !occupied & VALID.0[self.word];   // <- the whole point
            self.base = self.word * 64;
            self.word += 1;
        }
        let idx = self.base + self.bits.trailing_zeros() as usize;
        self.bits &= self.bits - 1;                // clear lowest set bit
        Some(Move::from_bit_index(idx))            // stride-16 -> stride-15
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let here = self.bits.count_ones() as usize;
        (here, Some(here + 64 * (4 - self.word)))   // exact impl possible, see below
    }
}

pub fn empty_moves(&self) -> impl Iterator<Item = Move> + '_ {
    EmptyMoves { board: self, word: 0, bits: 0, base: 0 }
}
```

### Four things this code is saying

**a) `!occupied & VALID` is not optional.** `!occupied` sets every bit
that is not a stone — including all 15 guard bits (column 15 of each row)
and the 16 unused bits above index 239. Without the `& VALID` the
iterator would yield index 15 → `r=0, c=15` → `Move::new` returns `None`,
and any unchecked conversion path would produce a *fabricated* move in
someone else's row. This is the same padding rule as the `shr` paper,
seen from the enumeration side: complements are dirty, masks clean them.

**b) Word-at-a-time, not cell-at-a-time.** Loading a word and masking it
moves the "skip occupied runs" work into one `&` per 64 cells.
`trailing_zeros` then finds the next *empty* bit directly — the iterator
never looks at occupied cells, so its cost scales with the number of
empties (and shrinks as the board fills: 0.85 ns first-move on an empty
board and the same 0.2 ns at 224 stones; a full sweep drops from 110 ns
to 2.7 ns).

**c) `bits &= bits - 1`** clears the lowest set bit — the idiom from the
slice-3 toolbox. Two instructions per item (`tzcnt`/`blsr`), and only the
bit-clear is loop-carried, so the CPU overlaps iterations instead of
stalling on a long dependency chain.

**d) The conversion happens exactly once, here.** `Move` is stride-15;
the bitboard is stride-16. The bridge:

```rust
// moveset.rs — the one place each way (see slice-3 pitfall: "stride is THE bug")
impl Move {
    /// Trusted path: `idx` must come from a masked bitboard, so `idx % 16 < 15`.
    pub(crate) fn from_bit_index(idx: usize) -> Move {
        debug_assert!(idx < 240 && idx % 16 < 15, "guard/tail bits must be masked off");
        let r = (idx >> 4) as u8;       // / 16
        let c = (idx & 15) as u8;       // % 16
        Move(r * 15 + c)                // logical index
    }
}
```

Worth memorising the algebra, because both directions are cheap (16 is a
power of two, so no divisions):

```text
bit index  = 16r + c          logical index = 15r + c
logical    = bit - r          where r = bit >> 4
bit        = logical + row()  because 15r + c + r = 16r + c
```

---

## 5. Why the loop is not a performance issue

Measured (Apple M5 Max, `rustc -O`, single thread, min of 5 × 200 k
iterations, release — a stand-in iterator with the same shape as above;
absolute numbers move ±30 % run to run, ratios are the point):

| stones | empties | `first()` | full sweep | per item | `.collect::<Vec<_>>()` |
|---|---|---|---|---|---|
| 0 | 225 | 0.85 ns | 108.8 ns | 0.48 ns | 158.1 ns |
| 60 | 165 | 0.23 ns | 86.0 ns | 0.52 ns | 131.4 ns |
| 200 | 25 | 0.23 ns | 7.6 ns | 0.31 ns | 59.5 ns |
| 224 | 1 | 0.22 ns | 2.7 ns | 2.71 ns | 12.5 ns |

Comparison paths measured the same way:

| alternative | cost |
|---|---|
| per-cell bit test over the bitboard, first hit | 0.30 ns (empty board) → 2.89 ns (224 stones) |
| per-cell bit test, whole board | ~36 ns, **constant** |
| `!(black\|white) & VALID`, then popcount (bulk count) | **0.6 ns**, constant |
| byte-per-cell scan (what the *oracle* has) | ~10 ns, vectorised |

**Reading of the numbers:**

1. **Laziness is the win.** "Give me the next empty cell" costs ~0.2 ns
   and does not care how full the board is. The eager alternative
   (`.collect()`) costs 12–158 ns *even when the caller wanted one move* —
   a 100–700× difference on the exact call MCTS rollouts and tactic
   probes make.
2. **A full sweep is ~0.5 ns per move.** That is ~1.5 cycles per item on
   this machine: the loop is throughput-bound (bit-clear, `tzcnt`, a
   shift/mask pair, an add), not latency-bound. No branch misprediction
   is possible per item — the only branch is "is this word exhausted",
   taken 4 times per 225 items.
3. **The sweep is not on the hot path.** It happens once per expanded
   MCTS node, next to a policy/value network evaluation that is orders of
   magnitude more expensive (Flex on CPU: microseconds per batch; even a
   GPU-backed node costs more than 100 ns of bookkeeping). Per self-play
   *move* the engine also does thousands of
   `play`/`undo`/`has_five` calls; `has_five` is the primitive worth
   benchmarking (slice 9), and `empty_moves` is not it.
4. **Bulk questions should not iterate at all.** `empty_moves().count()`
   walks all empties (110 ns), while `empty_cells().count()` is 0.6 ns —
   180× cheaper — because it is four popcounts on 32 bytes. That
   asymmetry is the *reason* `empty_cells()` exists next to
   `empty_moves()`: bit-level for bulk, cell-level for enumeration. (The
   oracle-based test "count empties == 225 − stones" should therefore be
   written against `empty_cells()`, not by walking.)
5. **No allocation.** The iterator is a 32-byte state value (plus a
   reference) living in registers/L1. `Vec` would malloc + free per call
   — once per expanded node, across 14 worker threads. Allocation
   contention is a real, invisible tax in parallel search; laziness makes
   it disappear. (This is also why `Box<dyn Iterator>` is wrong here: it
   would re-introduce an allocation *and* vtable dispatch per item.)

**Is the bit iterator the fastest possible full sweep? No.** The table
above shows a per-cell bit scan over the same bitboard finishing a whole
pass in ~36 ns — three times faster than the =108 ns sweep — and a
vectorised byte-per-cell scan at ~10 ns. That is expected and fine:

- a full pass is not what this type is for; the *lazy* path is (0.2 ns
  for the move the caller usually wants, and *shrinking* cost as the
  board fills, where the constant-cost scans keep paying 36 ns forever),
- the scans win by being unrolled/vectorised over a fixed 225-iteration
  loop; the iterator cannot vectorise because the number of results is
  data-dependent, and it pays a per-item conversion plus `Option`/state
  bookkeeping,
- the engine does not *have* a byte-per-cell board to scan: it stores
  bitboards, which is precisely what makes `has_five`, legality masks and
  candidate sets cheap. Swapping the storage to win the empty-cell sweep
  would lose far more elsewhere.

So the honest design statement is: **`empty_moves` is the lazy, allocation-free
enumeration primitive, not the fastest full sweep.** If a full sweep ever
turns up in a profile, the fix is to make the *set* smaller (masked
candidate iteration, above), not to make the sweep faster.

**When this loop *would* matter, and the fixes:**

- **Repeated full sweeps.** Calling `empty_moves()` inside a loop over
  moves re-derives the complement every time. Materialise once:
  `let empties: MoveSet = board.empty_moves().collect();` — a 32-byte
  `Copy` value, and iterating a `MoveSet` is *cheaper per item* than the
  bitboard path (no stride conversion, no `& VALID` per word). Pay the
  conversion once instead of per pass.
- **A full sweep per node in a net-free inner loop** (a playout engine,
  or a fast-path "play the immediate win" heuristic run per node). Then
  the fix is not to hand-optimise `next()` but to **shrink the candidate
  set**: a winning cell must be adjacent to a stone, so
  `empty_cells() & neighbourhood(stones)` is a sound restriction for
  `immediate_wins`, and iterating a masked complement is a two-line
  variant of the same loop (keep the mask in the struct, `&` it into
  `self.bits`). This is the pay-off of having *both* primitives: masks
  cheaply, then enumerate only what survived.
- **Uniform random legal move** (if a rollout policy ever needs it):
  counting + selecting a bit in a `MoveSet` beats walking an iterator
  (`count_ones` to pick a word, then bit-select) — another reason the
  `MoveSet` projection in §3 is worth having available.

---

## 6. Tests

Unit (`src/board.rs`, needs `pub(crate)` access to `empty_cells`):

```rust
#[test]
fn empty_moves_agrees_with_the_bitboard_count() {
    let mut b = Board::new();
    assert_eq!(b.empty_moves().count(), 225);
    for (r, c) in [(7, 7), (7, 8), (0, 0), (14, 14)] {
        b.play(Move::new(r, c).unwrap()).unwrap();
    }
    assert_eq!(b.empty_moves().count(), 225 - 4);
    assert_eq!(b.empty_cells().count(), 225 - 4);       // bit-level path agrees
}

#[test]
fn empty_moves_never_yields_a_guard_cell() {
    // the trap of forgetting `& VALID`: index 15 would appear as (0, 15) / phantom rows
    let b = Board::new();
    for mv in b.empty_moves() {
        assert!(mv.row() < 15 && mv.col() < 15);
        assert_eq!(Move::new(mv.row(), mv.col()), Some(mv));   // round-trips
    }
}

#[test]
fn empty_moves_is_lazy_and_deterministic() {
    let b = Board::new();
    assert_eq!(b.empty_moves().next(), Move::new(0, 0));   // ascending logical index
    let mut it = b.empty_moves();
    it.next();
    assert_eq!(it.next(), Move::new(0, 1));                // state advances by one
}
```

Integration (`tests/differential.rs`, public API + `testutil`) — the
oracle is the honest source of truth:

```rust
proptest! {
    #[test]
    fn empty_moves_matches_the_oracle(plays in prop::collection::vec(0u16..225, 0..=225)) {
        let mut fast = Board::new();
        let mut naive = reference::Board::new();
        for p in plays {
            let mv = Move::new((p / 15) as u8, (p % 15) as u8).unwrap();
            if fast.status() != Status::Ongoing { break; }
            let (a, b) = (fast.play(mv), naive.play(mv));
            prop_assert_eq!(a.is_ok(), b.is_ok());
        }
        // every yielded move is empty on the oracle, and nothing is missed
        let mut yielded = 0usize;
        for mv in fast.empty_moves() {
            prop_assert_eq!(naive.stone_at(mv), None);
            yielded += 1;
        }
        let mut oracle_empties = 0usize;
        for r in 0..15u8 { for c in 0..15u8 {
            if naive.stone_at(Move::new(r, c).unwrap()).is_none() { oracle_empties += 1; }
        }}
        prop_assert_eq!(yielded, oracle_empties);
    }
}
```

Two contracts worth *documenting in the doc comment* because callers can
and will rely on them: the **order is ascending logical index**
(row-major, `(0,0), (0,1), …, (14,14)`) and the iterator is
**deterministic** — that is what makes self-play reproducible from a seed.

---

## 7. Cheat sheet

```text
"how many cells are empty?"              → empty_cells().count()            (~0.6 ns)
"is (r,c) empty?"                        → bit test / is_legal              (~0 ns)
"give me empty cells, one at a time"     → empty_moves()                    (0.2 ns first, 0.5 ns/item)
"give me all of them, as a set"          → empty_moves().collect::<MoveSet>()  (pay once, iterate cheaper)
"give me empties inside a candidate set" → iterate a masked complement (internal, stride-16)
```

Design derivation, in one chain:

```text
consumers ask two different questions (bulk vs enumerate)
  → two primitives: empty_cells() (Bitboard, pub(crate)) + empty_moves()
  → enumeration must be lazy    → impl Iterator, not Vec/MoveSet
  → cells are bitboard-private  → yield Move (logical), never a bit index
  → the iterator is a view      → + '_  (borrow, not snapshot; play-while-iterating is E0502)
  → the loop is over results    → word mask (!occupied & VALID) + trailing_zeros
  → the loop is cheap           → 0.5 ns/item, no allocation, not on the hot path
  → bulk work would be silly    → popcount instead (0.6 ns), or a MoveSet snapshot
```

## Appendix — how the numbers were produced

Single-file Rust benchmark, compiled `rustc -O`, each variant run as min-of-5 rounds of 200 000 calls, `black_box` on the
board and on the sink, boards generated from a seeded xorshift so the
stone sets are reproducible. The iterator under test is the same shape as
§4 (including the stride-16 → stride-15 conversion per item and the
`Option`). It is a *stand-in* for `empty_moves`, not the engine itself:
treat the table as orders of magnitude, not as a benchmark result — the
real numbers belong in slice 9's criterion bench.
