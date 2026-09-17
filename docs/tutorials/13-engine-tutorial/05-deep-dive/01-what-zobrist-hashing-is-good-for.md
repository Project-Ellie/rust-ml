# 01 — What Zobrist hashing is good for

You have never heard of this concept, so this paper starts at zero. By
the end you will know what problem it solves, why the solution is one
of the neatest tricks in game programming, where exactly our design
spends it (chapter 12 §7, chapter 13 "Zobrist, compile-time"), and —
just as important — where the literature's most famous use of it is one
we explicitly walk away from.

Terminology follows the [tutorial glossary](../../13-engine-tutorial/README.md#glossary).

## 1. The problem: position identity

Throughout the system we keep needing to answer one question:

> *"Have I seen this exact board position before?"*

Concretely, for us that question comes from:

- the **replay buffer**, which wants to deduplicate self-play positions
  (chapter 12 §9: the buffer stores games and samples positions —
  knowing two samples are the same position is how you notice your
  buffer is less diverse than it looks),
- **tests**, which want to say "these two boards are the same position"
  cheaply, thousands of times per proptest run,
- **debugging**, where "path A and path B reached the same position —
  do they agree on everything else?" is the canary for a whole class of
  engine bugs.

The naive answers all work and all hurt:

1. **Compare the boards.** Two bitboards plus side to move: compare
   4 + 4 words and a flag. Fast for one comparison — but "have I seen
   this before?" against a collection of *n* stored positions is O(n)
   board comparisons per query. At a replay buffer of millions of
   positions, that is a non-starter.
2. **Store the boards.** Keep every position so you can hash-map them.
   A position is ~70 bytes (two `[u64; 4]`, a flag, move count). A
   hundred million positions is gigabytes of keys alone — and you still
   pay a full-board hash per lookup.
3. **Hash the board from scratch each time.** Feed all 225 cells into a
   hasher: O(cells) per query, every query, even though you only just
   placed one stone and *knew* the previous hash.

What we want is a **small, fixed-size fingerprint** of a position that

- fits in a machine word (so it can be a `HashMap<u64, _>` key, cheap
  to copy, cheap to compare),
- **updates in O(1)** when one stone is placed or removed — no rescan,
- is **the same no matter which move order produced the position**,
- and is **reproducible across runs and machines**, because the replay
  buffer outlives any single process.

That fingerprint is the Zobrist key.

## 2. The trick: XOR is the whole idea

The method is from **Albert L. Zobrist**, *A New Hashing Method with
Application for Game Playing*, Technical Report #88, Computer Sciences
Department, University of Wisconsin–Madison, 1970 — one of the oldest
and most durable ideas in computer games (chess engines have run on it
for fifty years).

Setup, done once: draw one random 64-bit number for every **(cell,
color)** pair — for us that is `2 × 225 = 450` numbers — plus one extra
number for **side to move**. That is the whole table:

```text
ZOBRIST[color][cell]  -> random u64     (450 entries)
SIDE_TO_MOVE          -> random u64     (1 entry)
```

The key of a position is the **XOR of the table entries of everything
that is true about the position**:

```text
key = ZOBRIST[Black][every cell with a black stone]
    ^ ZOBRIST[White][every cell with a white stone]
    ^ SIDE_TO_MOVE                    (if White is to move, say)
```

Why XOR and not, say, addition? Three properties of XOR do all the
work:

- **`a ^ a = 0` and `a ^ b ^ b = a`** — XOR is *its own inverse*.
  Placing a stone XORs its entry in; removing the stone XORs the same
  entry back out. **Undo needs no history beyond the move itself.** No
  undo stack of old keys, no subtraction-with-borrow edge cases: the
  same line of code runs in `play` and in `undo`.
- **XOR is commutative and associative** — the key does not depend on
  the order the stones were placed. Two games that reach the *same
  position by different move orders* (a **transposition**) produce the
  *same key*. That is precisely what "position identity" means — and it
  falls out of the algebra for free.
- **Flipping a fact is one XOR.** Side to move changed? XOR
  `SIDE_TO_MOVE` — in `play` *and* in `undo`. One instruction.

So the per-move update, in either direction, is:

```text
key ^= ZOBRIST[color_of_stone][cell];   // place or remove — same line
key ^= SIDE_TO_MOVE;                    // flip the mover — same line
```

O(1), branch-free, two XORs. A worked miniature: 2×2 board, 4-bit toy
keys. `ZOB[B][0]=1011`, `ZOB[B][2]=0110`, `ZOB[W][1]=1101`,
`SIDE=1111`. Position "black on cells 0 and 2, white on cell 1, White
to move":

```text
1011 ^ 0110 ^ 1101 ^ 1111 = 1011
```

Remove the black stone on cell 2 (undo): `1011 ^ 0110 = 1101`, then
flip the mover back: `1101 ^ 1111 = 0010` — which is exactly the key of
"black on 0, white on 1, Black to move", computed from scratch. The
incremental key and the from-scratch key can never silently diverge;
they are the same XOR sum, just accumulated differently. That identity
is what slice 5's roundtrip proptest hammers over 10k random play/undo
walks.

## 3. Collision honesty: when is 64 bits enough?

A key is a *fingerprint*, not the position itself: different positions
can collide. How worried should we be?

The birthday bound: with `n` distinct positions and 64-bit keys, the
collision probability is about `n² / 2⁶⁵`.

| n positions | collision probability |
|---|---|
| 10⁶ | ≈ 1 in 37 trillion |
| 10⁸ (huge replay buffer) | ≈ 0.03% |
| 10⁹ | ≈ 2.7% |

Our scale: the replay window starts at 250k positions and grows
sublinearly (chapter 12 §11); even a year of self-play stays orders of
magnitude below 10⁸. **Treat collisions as impossible** — which is what
chapter 12 §7 says, with the numbers above as the receipt. The slice's
own test (no zeros, no duplicates among the 450 table entries) guards
the generator itself, because a broken RNG is a far likelier failure
than a birthday collision.

One honest caveat: dedup that *drops* a colliding position would be
silently wrong, so the buffer should treat the key as a
**lookup hint, not proof** — on a key hit, confirm with a real board
comparison before concluding "same position". That costs one board
compare per rare hit and makes the collision probability irrelevant
outright.

## 4. Why the table is a compile-time `const fn`

The design generates the table at *compile time* from a fixed-seed
xorshift (chapter 13, "Zobrist, compile-time"). Why not `rand` at
startup like every chess engine tutorial does?

Because our keys must be **reproducible forever**. The replay buffer is
persisted to a journal and reloaded across runs (chapter 12 §9). If the
table were drawn fresh at every process start, a key stored on Tuesday
would mean nothing on Wednesday — dedup across restarts would be
garbage, and "same position" would become run-dependent. A compile-time
table means: zero runtime initialization, zero dependencies in the
dependency-island engine, and the same keys on every machine, every
run, forever. Determinism is the feature; the `const fn` is just how
Rust lets us have it for free.

## 5. Where the design spends the key

Four concrete uses, all in chapters 12/13:

1. **Replay-buffer dedup** (ch. 12 §7): keys are the dedup index of the
   persisted game store. Cheap to store (~8 bytes vs ~70 for a board),
   cheap to look up, reproducible across restarts (§4 above).
2. **Cheap position identity in tests** (ch. 12 §7): the proptest
   suite compares positions by key thousands of times per run instead
   of comparing boards.
3. **Debug assertions across paths** (ch. 12 §7): "two different paths
   reached the same key — assert they agree on the full position" is a
   cheap canary for engine bugs that produce inconsistent state.
4. **The incremental ↔ from-scratch property** (ch. 13): `play`/`undo`
   update the key incrementally; `Board::from_position` (the Swap2
   hand-off) computes it from scratch; a proptest asserts they agree
   after random walks. This is not a *use* of the key so much as the
   proof that uses 1–3 can trust it.

Note what the key makes possible *structurally*: `Board` can stay a
small `Clone`-cheap value type while still participating in
identity-based bookkeeping — the key is the position's ambassador to
every hash table in the system.

## 6. The famous use we reject: transposition tables in MCTS

If you read classic game-engine literature, Zobrist keys exist for one
reason above all: the **transposition table**. Chess and Go engines
store search results keyed by position hash, so when two move orders
reach the same position, the second search reuses the first one's work
— the tree becomes a **graph**, and the effective search depth grows
enormously. It is one of the biggest wins in classical search.

Our design says, explicitly (ch. 12 §7): **the tree stays a tree. No
transposition merging in MCTS.** Why walk away from the classic win?

- **PUCT statistics are path-dependent in spirit.** AlphaZero's MCTS
  keeps visit counts, prior-weighted action values, and virtual losses
  *per tree node*. A DAG node with multiple parents accumulates
  statistics from different search contexts; how to correctly merge or
  propagate them is a research topic, not an engineering detail. The
  MCTS review (ch. 10, #9) catalogs transposition-table MCTS variants
  precisely because each one makes different compromises.
- **The payoff is small here.** Transposition tables pay off when
  transpositions are frequent *and* re-evaluation is expensive. In
  Gomoku, stones are never removed within a game, so transpositions
  exist (move-order permutations) but are far rarer than in chess
  (pieces shuttle back and forth) — and each re-evaluation is one
  network forward pass, already amortized by the evaluator's batching.
- **AlphaZero itself does not do it**, and our whole risk budget is
  built on replicating a known-good algorithm at small scale, not on
  improving it speculatively.

So the shelf, labeled (ch. 12 §7): *transposition-table MCTS exists in
the literature, interacts subtly with PUCT, revisit only if profiling
ever shows duplicate subtrees dominating the search.* The Zobrist key
is exactly what you would need to implement it — which is part of why
building the key now is cheap insurance, not speculative machinery.

## 7. Summary

- Position identity — "have I seen this board before?" — is a recurring
  systems question; naive answers are O(n) per query or O(cells) per
  hash.
- Zobrist (1970): one random word per (cell, color) + side to move;
  key = XOR of what is present. XOR being commutative and self-inverse
  gives O(1) updates, free undo, and transposition-independence.
- At our scale (≤10⁸ positions), 64-bit collision risk is negligible;
  treat keys as lookup hints confirmed by a real compare on hits, and
  the risk is zero.
- Compile-time table because replay dedup must mean the same thing
  across runs and machines.
- We spend keys on: replay dedup, test identity, debug assertions, and
  the incremental↔from-scratch proof. We deliberately do **not** spend
  them on transposition-table MCTS — tree stays a tree, as in
  AlphaZero; the literature variant is documented and shelved.

Next time you see `key: u64` in the `Board` struct, you know it is not
"a hash" in the vague sense — it is the position's O(1)-maintained,
run-independent identity card, with fifty years of engine history
behind the trick.
