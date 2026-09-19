# 01 — Slice 7 implementation plan: intention, threat logic, and the price of certainty (with reference solution)

The slice file ([../07-tactics.md](../07-tactics.md)) gives you the
contract and the checklist. This paper is the opt-in other half, in the
shape of the slice-6 plan: **what exactly is it that you want to
achieve** — first in the project as a whole, then in this chapter, then
in the tactics themselves. The red→green sequence and the verified
reference solution follow after that.

**Verified before written.** Every step below was executed in a scratch
copy of the engine: **61 unit tests + 3 differential properties** green,
`clippy --all-targets --features testutil -- -D warnings` and
`fmt --check` clean, the full suite green at `PROPTEST_CASES=10000`
(197 s debug), and the tactics property green at `PROPTEST_CASES=100000`
under `--release` (95 s — in debug that width exceeds 15 minutes; wide
tactics runs want release mode).

---

## Part 1 — The intention, top-down: buying common sense with bitboards

Zoom out to the training loop again. MCTS decides which edge to explore
with PUCT:

```text
Q(s,a) + c_puct · P(s,a) · √ΣN / (1 + N(s,a))
```

The `P(s,a)` term is the **prior** — a one-shot opinion about move
quality *before* any search. On day one of training the network's
priors are noise, so the search burns thousands of simulations
rediscovering what any club player knows instantly: *take the win,
block the loss*. AlphaZero accepts this cost — its priors improve as
the network matures. Project-Ellie is "alpha-epsilon": we *cheat* on
purpose, with a hand-woven tactics module that is a **perfect prior for
the tactical subset of positions**, available from the very first
self-play game.

That is the intention, and it decomposes into three planned uses (ch.
13, "Tactics"):

1. the **mock evaluator** in the milestone-2 MCTS tactical tests,
2. an optional **fast path** in self-play (immediate win → play it,
   skip search — a config knob, measured as `[experiment]`),
3. the **generator** for the milestone-3 synthetic attack/defense set
   (AlphaGomoku's curriculum, our registered fallback).

Notice what is *not* in this slice: any decision about how strongly
these detections blend into training, or whether the blend decays as
the network matures. That deferral is itself the design, and it is
worth stating as crisply as the chapter does:

> **The engine vouches for what is true. Training decides what to do
> with it.**

Every function you write here returns a *certainty* — a set of cells
about which there is nothing to learn. This module will be the only
part of the whole system that never estimates, never approximates,
never generalizes. That epistemic purity is why detection lives in the
engine (the dependency island, the oracle-tested part of the world)
while all three *uses* live elsewhere. Hold onto that when you write
the doc comments: the module's job is to be right, and to be provably
right — the differential oracle is not a nice-to-have, it is the point.

## Part 2 — Drilling into the chapter: four artifacts, five decisions

The chapter's contract decomposes into four artifacts:

1. **`MoveSet`** — the public set-of-moves type, a 225-bit bitset in
   logical (stride-15) indexing. The tactics functions all return it;
   MCTS will consume it. Deliberately separate from the internal
   stride-16 `Bitboard`: this type speaks the crate's public vocabulary.
2. **The three functions** — `immediate_wins`, `forced_blocks`,
   `double_threats` — one shared primitive (hypothetical placement +
   win scan) applied at three depths of indirection.
3. **The ASCII puzzle corpus** — `board_from_ascii`, because tactical
   tests written in coordinates die of unreadability. Tests should read
   like a puzzle book.
4. **The naive oracle + differential property** — the same three
   functions written the dumb way (line scans through cells), compared
   on random positions. Certainty, measured.

Five design decisions to understand *before* implementing:

- **The side/`to_move` asymmetry is the API's honesty.**
  `immediate_wins(b, side)` answers for *either* color regardless of
  whose turn it is — "where could Black complete five" is a
  turn-independent fact. `forced_blocks(b)` is where the turn enters:
  the side to move must answer *the opponent's* wins. Keep the
  asymmetry exactly as the contract states it.
- **Hypotheticals are value-semantics.** `stones.with_bit(i)` returns a
  new bitboard; nothing mutates the board. Tactical queries are pure —
  callable at expansion time, in parallel, without touching game state.
- **The cost budget is part of the contract.** `immediate_wins` ≈ 225
  hypothetical placements ≈ 13k ops — cheap. `double_threats` is
  O(empties × immediate_wins) ≈ 3M ops — fine at leaf expansion, *never*
  per PUCT step. The module docs must say so; a future caller will not
  read the chapter.
- **A subtlety the contract sentence leaves open — decide it now.**
  Read the contract literally: "moves after which `side` has ≥ 2
  immediate wins". Now consider a move that *is* an immediate win: on
  the hypothetical board the five already exists, so *every* empty cell
  "completes five" under the whole-board scan — the move qualifies,
  degenerately. Two readings: include immediate wins (literal set
  comprehension) or exclude them (the contract's own clarification:
  "open fours and double fours — *unanswerable next move*" — and after
  a five there *is* no next move). This plan **excludes**, for two
  reasons that reinforce each other: semantically, the three sets then
  partition cleanly (win now / must block / unblockable threat — no
  overlap to confuse priors); pragmatically, the exclusion is exactly
  what makes the fast whole-board scan and the naive through-cell scan
  answer the *same question*, so the differential oracle can compare
  them (Part 4, step 5, has the divergence analysis). Documented in
  the module docs as v1 simplification 3, next to the four-three blind
  spot — a deliberate resolution of an ambiguous contract sentence,
  not a silent deviation.
- **Your own existing code is the foundation.** `Board::empty_moves()`
  (slice 3) already implements the masked-empty walk; `has_any_five`
  (slice 4) is the win scan; `Bitboard::with_bit`/`without_bit` are the
  hypotheticals. Note the real names differ from the ch. 13 sketch
  (`with_bit`, not `with`; `has_any_five`, not `has_five_any`) — the
  chapter says "the chapter stays authoritative for the API", but where
  chapter and *landed code* disagree, the landed code wins; the sketch
  was written before slice 3.

## Part 3 — The background: threat arithmetic, and where truth ends

### The counting argument

Why is an open four unanswerable? Not because of any clever evaluation
— because of **arithmetic**: an open four has *two* cells that complete
five, and the rules grant the opponent exactly *one* move. Two threats,
one answer, done. A closed four has one winning cell — one threat, one
answer, survivable. A double four threatens twice — unanswerable again.
Tactics, at this depth, is not search and not learning; it is counting
to two.

`double_threats` is the *constructive* side of the same argument: it
finds the moves that *create* a count of two. The fork is the
geometric version — one stone completing two lines at once — but the
detection doesn't know geometry; it counts immediate wins after the
hypothetical and compares with 2. That the geometric intuition and the
arithmetic implementation coincide is precisely what the puzzle corpus
pins down.

### The gapped four — threats are a global property

One rung subtler: `X X X _ X` (a "broken" or gapped four) has exactly
one winning cell — the gap — because only the gap completes five. Fine.
But now consider *creating* gapped fours: the pattern `O . O . O . O`
looks like harmless filler, and yet playing the middle gap produces
`O O O _ O` *and* `O _ O O O` simultaneously — two gapped fours, two
winning cells, an unanswerable double threat. (This is not a made-up
example: an early draft of this slice's own fork puzzle parked White's
filler stones in exactly that pattern on rank 0, and the differential
mindset caught it only because the test asserted *exactness*. The
oracle does not do "harmless filler" — see Part 4, step 5.)

The lesson generalizes: threat status is not a property of local
shape alone but of the whole line, and the only reliable way to see it
is to *play the hypothetical and count*. That is why the implementation
is a nested hypothetical scan and not a pattern matcher. Pattern
matching would be faster and wrong in instructive ways.

### The ladder, and the exact boundary of v1

Gomoku threats form a ladder, each rung forcing the opponent's reply
one ply earlier:

```text
three ──► open three ──► four ──► open four ──► five
 (ignored    (must answer     (must answer    (unanswerable:   (game
  by v1)      or it becomes    immediately)    two winning      over)
               an open four)                   cells, one move)
```

v1 certifies the top of the ladder: fives (win detection), fours
(immediate wins / forced blocks), and one constructive ply of "count
two" (double threats). The rung it *knowingly* skips is the
**four-three**: a move creating a four *and* an open three has exactly
one immediate win now (the three matures next ply), so the counting
criterion doesn't fire — yet the position is just as lost, because the
opponent's forced block of the four doesn't answer the maturing three.
Catching it requires two plies of threat logic — a small search — and
that is exactly the documented boundary: **v1 is threat arithmetic,
not threat search**. The doc comment names the gap so that nobody
"discovers" it during a lost arena game; the milestone-3 synthetic
curriculum is where the network gets to *learn* what the engine
declines to *certify*.

This is the same epistemic discipline as Part 1: the module's value is
that every answer is certain. Widening the criterion to "probably
lost" positions would poison the well — the mock evaluator and the
fast path both rely on the sets being *truth*. When v2 wants the
four-three, it will be a search, and it will live behind measurements.

### Why the oracle is a line scan, not a second bitboard

The naive reference implements the same three functions with
through-cell walks on a 15×15 array — the most obvious code imaginable.
Two implementations that disagree are a bug report; two *identical*
implementations that agree prove nothing. Maximum conceptual distance
(bit-parallel whole-board scans vs. per-cell walks) is what makes
agreement meaningful. One subtlety makes the comparison possible at
all: on a board that already contains a five, the whole-board scan and
the through-cell scan answer *different questions* (the five exists no
matter where you hypothetically play, so "whole-board" says every cell
wins; "through-cell" says only cells on the five's line). Non-terminal
positions contain no five — except the hypothetical ones created by
immediate wins, which is the second, independent reason `double_threats`
excludes them. The semantic partition and the oracle comparability are
the same decision, arrived at from two directions. That is what a good
design decision feels like.

## Part 4 — The build: red→green

### Step 0 — before you write anything

- Slices 1–5 done: 47 unit + 2 differential green. `tactics.rs` is a
  doc-comment stub; `lib.rs` declares the module.
- This slice grows the public API: `MoveSet`, `immediate_wins`,
  `forced_blocks`, `double_threats` get `pub use` in `lib.rs`. No
  `#[allow(dead_code)]` anywhere.
- It also *shrinks* nothing and changes no existing behavior — Board,
  win detection, and the reference engine only gain code.
- Baseline: `cargo test -p engine --features testutil` → 47 + 2.

### The build order

| step | behaviour it adds | new code |
|---|---|---|
| 1 | `MoveSet` roundtrip | bitset in `moveset.rs` |
| 2 | `board_from_ascii` + parser tests | `reference.rs` |
| 3 | `immediate_wins`, open/broken-four puzzles | `tactics.rs` primitive |
| 4 | either-color answer, `forced_blocks` | one-liner |
| 5 | `double_threats`, the exact fork puzzle | nested primitive |
| 6 | negative corpus, closed-four partition | tests only |
| 7 | edge puzzles (row 0, corner) | tests only |
| 8 | naive oracle + differential property | `reference.rs`, `differential.rs` |

### Step 1 — MoveSet

RED: insert/contains/len/iter roundtrip over hand-picked moves chosen
to cross word seams — indices 0, 63, 64, 108, 224 — plus idempotent
insert. GREEN: the four-word bitset from the reference solution.
(Measured trap, pre-empted: a public `len()` without `is_empty()`
trips clippy under `-D warnings`; write `is_empty` in the same breath.
The set-index `< 225` invariant — bits 225+ can never be set because
the only way in is `insert(Move)` — belongs in the doc comment; it is
what makes `iter()`'s `Move(...)` construction sound.)

### Step 2 — the ASCII parser, strict on purpose

RED: a two-stone board parses with correct `stone_at` and inferred
`to_move` (equal counts → Black; one more X → White); `should_panic`
tests for a short row and for unreachable counts. GREEN:
`board_from_ascii` in `reference.rs` (it lives next to the oracle —
`cfg(any(test, feature = "testutil"))` makes it available to both unit
and integration tests). Strictness decisions, all deliberate: exactly
15 whitespace-separated cells per row, exactly 15 non-blank rows (blank
lines tolerated so raw strings can indent); stone counts must be
reachable by alternating play from Black; the position must be
non-terminal — replay goes through `Board::play`, so a five anywhere in
the puzzle panics with a clear message. A parser that accepts sloppy
input produces tests that *look* like they say something they don't;
the corpus is the deliverable, so the corpus language is strict.
(War story from this very build: strict parsing caught four
hand-written puzzle rows of 14 and 16 tokens within the first minute.)

### Step 3 — `immediate_wins`, and the dirty-complement trap

RED: the open-four puzzle (exactly two winning cells) and the
broken-four puzzle (exactly one — the gap). GREEN: the shared
primitive `winning_cells(stones, empty)`: walk the empty bits, place
hypothetically, scan.

THE TRAP of the slice is in computing `empty`: `!(black | white)` sets
*every padding bit* — `Bitboard::not()` is a raw complement, and the
stride-16 invariant demands complements be masked with `VALID`
immediately. Skip the mask and you "discover" 31 phantom empty cells in
the padding; `mv_at` then panics on row 15 — loud, not silently wrong,
but only because `Move::new` validates. `Board::empty_moves()` (your
slice-3 code) documents the same rule; `clean_empties` in `tactics.rs`
is the bitboard twin. (`iter_set_bits` is the one new bitboard
primitive — the classic `bits & (bits - 1)` walk, factored out so the
tactics code reads as intent.)

### Step 4 — either color, and the turn

RED: a White open four detected with `side = White` while Black is to
move; `forced_blocks` on the same shape equals White's winning cells
with Black to move. GREEN: `forced_blocks` is the one-liner
`immediate_wins(b, b.to_move().other())`. Resist adding a `side`
parameter to `forced_blocks` — the contract's asymmetry is the point:
*threats* are turn-independent facts, *obligations* belong to the side
to move.

### Step 5 — `double_threats`, the exclusion, and the exact fork

RED: the classic fork — two closed threes meeting at (7,7) — asserting
the set is **exactly** `{(7,7)}`. Exactness is the strong version of
the chapter's "that cell ∈ double_threats", and it earns its keep
immediately: it forces your filler stones to be *provably* quiet. (An
exact-set assertion on the first draft of this very puzzle failed —
the rank-0 fillers `O . O . O . O` handed White a double threat of
their own at (0,3), two gapped fours with one stone. Part 3's global
property, caught by the corpus, not by reasoning. Scatter your fillers
on different rows, columns, and diagonals, and let the test prove it.)

GREEN: the nested primitive — for each empty cell, build the
hypothetical, skip immediate wins (the exclusion), count
`winning_cells` on the reduced empty set, keep the cell at ≥ 2.

Why the exclusion is right, in one paragraph: with a five already on
the hypothetical board, the whole-board scan (`has_any_five`) reports
*every* empty cell as a win, so an immediate win would degenerately
qualify as a double threat — polluting the "unanswerable" set with
moves that are simply *wins*, and, incidentally, making the naive
through-cell oracle (which only sees the five through cells on its own
line) disagree for reasons that have nothing to do with tactics.
Excluding restores both the semantics and the oracle. Document it as
v1 simplification 3 in the module docs, next to the four-three gap and
the opponent-wins-first acceptance from the chapter's pitfalls.

### Step 6 — the negative corpus, and the partition witness

Two puzzles. First, scattered stones: all three functions empty — the
corpus needs explicit proof that the module doesn't cry wolf. Second,
the closed four `O X X X X .`: Black's `immediate_wins` is exactly
`{(7,7)}`, White (to move) sees `forced_blocks == {(7,7)}`, and
Black's `double_threats` is *empty* — because the only four-completing
move is the immediate win itself, excluded by step 5. This puzzle is
the **partition witness**: win-now and threat sets are disjoint, by
construction, in a test.

### Step 7 — edge puzzles

Immediate win along row 0 (the open four's two cells both exist — the
edge bounds the *perpendicular* direction, not the in-row one) and the
corner fork: two threes hugging the edges, where playing the corner
creates two fours whose *other* ends are "blocked" by the board edge
itself — membership assertions for (0,0) and for the two edge-open
four completions (0,4) and (4,0). Edge geometry is where stride math
goes to be wrong; the corpus should live there a while.

### Step 8 — the oracle, the differential property, and two measured traps

The naive trio in `reference.rs` (through-cell scans; the existing
`wins_from`/`count_dir` refactor into free functions over cell arrays,
so hypothetical boards can be scanned without a `Board`), then the
property: random playouts, compare all three functions for both colors
against the oracle.

Two traps, both measured in this build:

1. **`prop_assume!(ongoing)` aborts at scale.** The comparison is only
   meaningful on non-terminal positions (Part 3's "different
   questions"). Rejecting terminal playouts with `prop_assume` works at
   100 cases and *aborts the test* at 100,000: ~6% of random 120-ply
   playouts reach a terminal position, and proptest's global-reject
   budget (1024) runs out. The fix is deterministic and free: `undo()`
   the terminal move — the pre-terminal position is exactly the
   non-terminal board you wanted, and undoing a win reopens the game
   by construction (slice 3's undo semantics, now earning rent).
2. **Cost is real.** `double_threats` is O(empties² × scan) on *both*
   sides of the comparison. The property runs 100 cases by default
   (fewer, meatier cases than the play/undo walks); 10k cases ≈ 3.5
   min debug; 100k wants `--release` (95 s vs. 15+ min). Put these
   numbers in the test comment — the next person to run a wide sweep
   should not have to measure this again.

### Step 9 — re-exports, gates, commit

```rust
pub use moveset::{Move, MoveSet};
pub use tactics::{double_threats, forced_blocks, immediate_wins};
```

```bash
cargo fmt --all
cargo clippy -p engine --all-targets --features testutil -- -D warnings
cargo test  -p engine --features testutil --no-fail-fast
PROPTEST_CASES=10000 cargo test -p engine --features testutil
```

Commit: `feat(engine): tactical detection (wins, blocks, double threats)`.

---

## Part 5 — Reference solution

Assembled from the steps above; byte-for-byte what was verified. New
code only — existing functions that merely gained delegating bodies
(`reference::Board::wins_from`, `count_dir`) are shown as hunks.

### `crates/engine/src/tactics.rs` — complete

```rust
//! Alpha-epsilon tactics: immediate wins, forced blocks, double threats.
//! Slice 7. See docs/13-engine-design.md, "Tactics".
//!
//! Pure bitboard truth, no learning: every answer this module gives is
//! a CERTAINTY. How much these certainties shape priors is a
//! training-time `[experiment]` knob — the engine only vouches for what
//! is true.
//!
//! THREE DOCUMENTED v1 SIMPLIFICATIONS (all deliberate, all measured
//! against their cost):
//!
//! 1. **Four-three blind spot.** `double_threats` counts immediate
//!    wins after one move. A four-three has exactly ONE immediate win
//!    now (the three matures next ply), so it escapes — the full
//!    win-in-2 search is out of scope for the engine milestone.
//! 2. **Opponent-wins-first.** A "double threat" on a board where the
//!    OPPONENT has an immediate win is answerable — they win before it
//!    matures. v1 accepts this; self-play checks forced blocks first.
//! 3. **Immediate wins are not double threats.** A move that completes
//!    five ENDS the game; calling it a "threat" would be a category
//!    error, so `double_threats` excludes it and the three sets
//!    partition cleanly: win now / must block / unblockable threat.
//!    (The pragmatic bonus: with the five-already-present case gone,
//!    the fast whole-board scan and the naive through-cell scan answer
//!    the SAME question — see `reference.rs`.)
//!
//! COST: `immediate_wins` is ~225 hypothetical placements ≈ 13k ops.
//! `double_threats` is O(empties × immediate_wins) ≈ 3M ops — fine at
//! leaf expansion, never call it per PUCT step.

use crate::bitboard::{Bitboard, VALID};
use crate::board::{Board, Color};
use crate::moveset::{Move, MoveSet};
use crate::win::has_any_five;

/// Cells where `side` completes five immediately. Answers for EITHER
/// color regardless of `to_move` — that asymmetry with `forced_blocks`
/// (where the turn matters) is what makes the API honest.
pub fn immediate_wins(b: &Board, side: Color) -> MoveSet {
    winning_cells(b.stones(side), clean_empties(b))
}

/// Cells the side to move MUST play — the opponent's immediate wins.
pub fn forced_blocks(b: &Board) -> MoveSet {
    immediate_wins(b, b.to_move().other())
}

/// Moves after which `side` has >= 2 immediate wins: open fours and
/// double fours — unanswerable next move. Immediate wins themselves
/// are EXCLUDED (simplification 3 above). See the module docs for the
/// four-three blind spot.
pub fn double_threats(b: &Board, side: Color) -> MoveSet {
    let stones = b.stones(side);
    let empty = clean_empties(b);
    let mut out = MoveSet::EMPTY;
    for i in empty.iter_set_bits() {
        let hypo = stones.with_bit(i);
        if has_any_five(&hypo) {
            continue; // immediate win: ends the game, not a threat
        }
        if winning_cells(hypo, empty.without_bit(i)).len() >= 2 {
            out.insert(mv_at(i));
        }
    }
    out
}

/// The shared primitive: every empty cell whose hypothetical
/// occupation completes five. `empty` MUST be VALID-masked (padding
/// bits clear) — `clean_empties` guarantees it, and `without_bit`
/// preserves it.
fn winning_cells(stones: Bitboard, empty: Bitboard) -> MoveSet {
    let mut out = MoveSet::EMPTY;
    for i in empty.iter_set_bits() {
        if has_any_five(&stones.with_bit(i)) {
            out.insert(mv_at(i));
        }
    }
    out
}

/// The empty cells as a CLEAN bitboard. `!occupied` sets every padding
/// bit — the complement must be masked immediately (the padding
/// invariant, ch. 13; `Board::empty_moves` documents the same rule).
fn clean_empties(b: &Board) -> Bitboard {
    !(b.stones(Color::Black) | b.stones(Color::White)) & VALID
}

/// Stride-16 bitboard index → logical Move. Only ever called with
/// VALID-masked bits, so the column is < 15 and `new` cannot fail.
fn mv_at(i: usize) -> Move {
    Move::new((i / 16) as u8, (i % 16) as u8).expect("VALID-masked bits are real cells")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::board_from_ascii;

    fn set_of(cells: &[(u8, u8)]) -> MoveSet {
        let mut s = MoveSet::EMPTY;
        for &(r, c) in cells {
            s.insert(Move::new(r, c).unwrap());
        }
        s
    }

    #[test]
    fn open_four_has_exactly_two_winning_cells() {
        let b = board_from_ascii(
            "
            O . O . O . O . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . X X X X . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            ",
        );
        assert_eq!(immediate_wins(&b, Color::Black), set_of(&[(7, 3), (7, 8)]));
    }

    #[test]
    fn broken_four_has_exactly_one_winning_cell() {
        // X X X _ X with the gap at (7,6): only the gap completes five.
        let b = board_from_ascii(
            "
            O . O . O . O . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . X X . X X . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            ",
        );
        assert_eq!(immediate_wins(&b, Color::Black), set_of(&[(7, 6)]));
    }

    #[test]
    fn immediate_wins_answers_for_either_color() {
        // Same shape as the open-four test, but the FOUR is White's.
        let b = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . O O O O . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            X . X . X . X . . . . . . .
            ",
        );
        assert_eq!(immediate_wins(&b, Color::White), set_of(&[(9, 1), (9, 6)]));
        // ...while Black's scattered stones threaten nothing.
        assert!(immediate_wins(&b, Color::Black).is_empty());
    }

    #[test]
    fn forced_blocks_are_the_opponents_winning_cells() {
        // White has the open four, Black to move (equal counts).
        let b = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . O O O O . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            X . X . X . X . . . . . . .
            ",
        );
        assert_eq!(b.to_move(), Color::Black);
        assert_eq!(forced_blocks(&b), set_of(&[(9, 1), (9, 6)]));
    }

    #[test]
    fn the_classic_fork_is_exactly_one_double_threat() {
        // Two closed threes (blocked by O at the top / on the left).
        // Playing (7,7) opens TWO fours, each with exactly one winning
        // cell: (8,7) and (7,8). No other move creates more than one
        // four, so the set is EXACTLY {(7,7)}. The remaining O stones
        // are scattered filler — chosen to be genuinely quiet (an
        // earlier draft parked them on rank 0 as `O . O . O . O`,
        // which is ITSELF a double threat: (0,3) fills two gapped
        // fours. The oracle does not do "harmless filler".)
        let b = board_from_ascii(
            "
            . . . . . . . . . . . . . . O
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . O . . . . . . .
            . . . . . . . X . . . . . . .
            . . . . . . . X . . . . . . .
            . . . . . . . X . . . . . . .
            . . . O X X X . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . O . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            O . . . . . . . . . . . . . O
            ",
        );
        assert!(immediate_wins(&b, Color::Black).is_empty(), "no four yet");
        assert_eq!(double_threats(&b, Color::Black), set_of(&[(7, 7)]));
        assert!(double_threats(&b, Color::White).is_empty());
    }

    #[test]
    fn closed_four_forces_a_block_but_is_no_double_threat() {
        // X X X X blocked by O on the left: ONE winning cell (7,7).
        // White to move (X has one more stone) must play it. And the
        // winning move itself is NOT a double threat — the partition.
        let b = board_from_ascii(
            "
            O . O . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . O X X X X . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            ",
        );
        assert_eq!(b.to_move(), Color::White);
        assert_eq!(immediate_wins(&b, Color::Black), set_of(&[(7, 7)]));
        assert_eq!(forced_blocks(&b), set_of(&[(7, 7)]));
        assert!(double_threats(&b, Color::Black).is_empty());
    }

    #[test]
    fn quiet_positions_have_no_tactics() {
        let b = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . X . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . O . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . O . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . X . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            ",
        );
        assert!(immediate_wins(&b, Color::Black).is_empty());
        assert!(immediate_wins(&b, Color::White).is_empty());
        assert!(forced_blocks(&b).is_empty());
        assert!(double_threats(&b, Color::Black).is_empty());
        assert!(double_threats(&b, Color::White).is_empty());
    }

    #[test]
    fn immediate_win_along_the_top_edge() {
        // Open four on row 0: the board edge is no obstacle for the
        // in-row winning cells (0,0) and (0,5).
        let b = board_from_ascii(
            "
            . X X X X . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            O . O . O . O . . . . . . .
            ",
        );
        assert_eq!(immediate_wins(&b, Color::Black), set_of(&[(0, 0), (0, 5)]));
    }

    #[test]
    fn double_threat_in_the_corner() {
        // Two threes hugging the edges. Playing the CORNER (0,0) makes
        // two fours whose only open ends are (0,4) and (4,0) — the
        // edge itself "blocks" the other ends. (0,4) and (4,0) are
        // also double threats (they open a four each... on the edge).
        let b = board_from_ascii(
            "
            . X X X . . . . . . . . . . .
            X . . . . . . . . . . . . . .
            X . . . . . . . . . . . . . .
            X . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            O . O . O . O . O . O . . . .
            ",
        );
        assert!(immediate_wins(&b, Color::Black).is_empty());
        let threats = double_threats(&b, Color::Black);
        assert!(
            threats.contains(Move::new(0, 0).unwrap()),
            "the corner fork"
        );
        assert!(
            threats.contains(Move::new(0, 4).unwrap()),
            "open four on the edge"
        );
        assert!(
            threats.contains(Move::new(4, 0).unwrap()),
            "open four on the edge"
        );
    }
}
```

### `crates/engine/src/moveset.rs` — one insertion (after `impl Move`)

```rust
/// A set of moves as a bitset over logical indices (stride 15).
///
/// 225 bits in 4 `u64` words; the top 31 bits of word 3 can never be
/// set, because the only way in is `insert(Move)` and a `Move` is
/// always < 225. Value semantics, `Copy` — a set you can pass around
/// like a number. Deliberately separate from the internal stride-16
/// `Bitboard`: this type speaks the PUBLIC logical vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MoveSet([u64; 4]);

impl MoveSet {
    pub const EMPTY: MoveSet = MoveSet([0; 4]);

    pub fn insert(&mut self, mv: Move) {
        let i = mv.index();
        self.0[i / 64] |= 1 << (i % 64);
    }

    pub fn contains(&self, mv: Move) -> bool {
        let i = mv.index();
        self.0[i / 64] & (1 << (i % 64)) != 0
    }

    pub fn len(&self) -> u32 {
        self.0.iter().map(|w| w.count_ones()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.0 == [0; 4]
    }

    pub fn iter(&self) -> impl Iterator<Item = Move> + '_ {
        self.0.iter().enumerate().flat_map(|(w, &word)| {
            (0..64).filter_map(move |bit| {
                if word >> bit & 1 == 1 {
                    // Invariant: only bits < 225 can be set, so the
                    // index always fits a u8 and is a valid cell.
                    Some(Move((w * 64 + bit) as u8))
                } else {
                    None
                }
            })
        })
    }
}
```

…and two tests in its `mod tests`:

```rust
    #[test]
    fn moveset_roundtrip_across_word_boundaries() {
        let mut set = MoveSet::EMPTY;
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);

        // indices 0, 63, 64, 108, 224 — one per interesting seam
        let moves = [
            Move::new(0, 0).unwrap(),   //   0
            Move::new(4, 3).unwrap(),   //  63
            Move::new(4, 4).unwrap(),   //  64
            Move::new(7, 3).unwrap(),   // 108
            Move::new(14, 14).unwrap(), // 224
        ];
        for &mv in &moves {
            set.insert(mv);
        }
        assert!(!set.is_empty());
        assert_eq!(set.len(), 5);
        for &mv in &moves {
            assert!(set.contains(mv), "{mv:?} must be in the set");
        }
        assert!(!set.contains(Move::new(7, 7).unwrap()));

        let mut collected: Vec<usize> = set.iter().map(|m| m.index()).collect();
        collected.sort_unstable();
        assert_eq!(collected, vec![0, 63, 64, 108, 224]);
    }

    #[test]
    fn insert_is_idempotent() {
        let mut set = MoveSet::EMPTY;
        let mv = Move::new(7, 7).unwrap();
        set.insert(mv);
        set.insert(mv);
        assert_eq!(set.len(), 1);
    }
```

### `crates/engine/src/bitboard.rs` — one insertion (inside `impl Bitboard`)

```rust
    /// Iterate the indices of all set bits, lowest first — the classic
    /// `bits & (bits - 1)` lowest-bit walk, word by word. Walks
    /// whatever bits are set, padding included; masking dirty
    /// complements is the caller's business (the padding invariant).
    pub(crate) fn iter_set_bits(self) -> impl Iterator<Item = usize> {
        self.0.into_iter().enumerate().flat_map(|(w, mut bits)| {
            std::iter::from_fn(move || {
                if bits == 0 {
                    return None;
                }
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                Some(w * 64 + bit)
            })
        })
    }
```

### `crates/engine/src/reference.rs` — import, two delegating bodies, and the slice-7 section

```diff
-use crate::moveset::Move;
+use crate::moveset::{Move, MoveSet};
```

```diff
     fn wins_from(&self, r: usize, c: usize, color: Color) -> bool {
-        DIRECTIONS.iter().any(|&(dr, dc)| {
-            1 + self.count_dir(r, c, dr, dc, color) + self.count_dir(r, c, -dr, -dc, color) >= 5
-        })
+        wins_from_cells(&self.cells, r, c, color)
     }
 
     pub fn count_dir(&self, r: usize, c: usize, dr: i32, dc: i32, color: Color) -> usize {
-        let mut n = 0;
-        let mut nr = r as i32 + dr;
-        let mut nc = c as i32 + dc;
-        while (0..15).contains(&nr)
-            && (0..15).contains(&nc)
-            && self.cells[nr as usize][nc as usize] == Cell::Stone(color)
-        {
-            n += 1;
-            nr += dr;
-            nc += dc;
-        }
-        n
+        count_dir_cells(&self.cells, r, c, dr, dc, color)
     }
```

Appended after `impl Default for Board`:

```rust
// --- Slice 7: naive tactics oracle + the ASCII puzzle parser ---------

/// Free-function twin of `Board::wins_from`, so the naive tactics can
/// scan HYPOTHETICAL cell arrays. Counts the cell (r, c) itself as 1 —
/// exactly the "place a stone here" semantics.
fn wins_from_cells(cells: &[[Cell; 15]; 15], r: usize, c: usize, color: Color) -> bool {
    DIRECTIONS.iter().any(|&(dr, dc)| {
        1 + count_dir_cells(cells, r, c, dr, dc, color)
            + count_dir_cells(cells, r, c, -dr, -dc, color)
            >= 5
    })
}

fn count_dir_cells(
    cells: &[[Cell; 15]; 15],
    r: usize,
    c: usize,
    dr: i32,
    dc: i32,
    color: Color,
) -> usize {
    let mut n = 0;
    let mut nr = r as i32 + dr;
    let mut nc = c as i32 + dc;
    while (0..15).contains(&nr)
        && (0..15).contains(&nc)
        && cells[nr as usize][nc as usize] == Cell::Stone(color)
    {
        n += 1;
        nr += dr;
        nc += dc;
    }
    n
}

/// Replays a fast board's history into the naive representation.
fn naive_from(b: &crate::board::Board) -> Board {
    let mut nb = Board::new();
    for &mv in b.moves() {
        nb.play(mv).expect("a valid fast history replays naively");
    }
    nb
}

/// Naive `immediate_wins`: line scans through each empty cell.
///
/// VALID ON NON-TERMINAL BOARDS ONLY. Once a five exists, the fast
/// whole-board scan (`has_any_five`) and this through-cell scan answer
/// different questions — that divergence is precisely why
/// `double_threats` excludes immediate wins (see tactics.rs).
pub fn naive_immediate_wins(b: &crate::board::Board, side: Color) -> MoveSet {
    let nb = naive_from(b);
    let mut out = MoveSet::EMPTY;
    for r in 0..15 {
        for c in 0..15 {
            if nb.cells[r][c] == Cell::Empty && wins_from_cells(&nb.cells, r, c, side) {
                out.insert(Move::new(r as u8, c as u8).unwrap());
            }
        }
    }
    out
}

/// Naive `forced_blocks`. Same non-terminal precondition.
pub fn naive_forced_blocks(b: &crate::board::Board) -> MoveSet {
    naive_immediate_wins(b, b.to_move().other())
}

/// Naive `double_threats`: place each candidate, count cells that
/// would complete five, keep the candidates with two or more.
/// Immediate wins are skipped — same exclusion as the fast version.
/// Same non-terminal precondition.
pub fn naive_double_threats(b: &crate::board::Board, side: Color) -> MoveSet {
    let nb = naive_from(b);
    let mut out = MoveSet::EMPTY;
    for r in 0..15usize {
        for c in 0..15usize {
            if nb.cells[r][c] != Cell::Empty {
                continue;
            }
            if wins_from_cells(&nb.cells, r, c, side) {
                continue; // immediate win: ends the game, not a threat
            }
            let mut hypo = nb.cells;
            hypo[r][c] = Cell::Stone(side);
            let mut wins = 0;
            for r2 in 0..15 {
                for c2 in 0..15 {
                    if hypo[r2][c2] == Cell::Empty && wins_from_cells(&hypo, r2, c2, side) {
                        wins += 1;
                    }
                }
            }
            if wins >= 2 {
                out.insert(Move::new(r as u8, c as u8).unwrap());
            }
        }
    }
    out
}

/// Parses rows of `X` / `O` / `.` into a fast Board (Black = X,
/// White = O): 15 whitespace-separated cells per row, 15 non-blank
/// rows (blank lines are tolerated, so raw-string literals can
/// indent). Stone counts must be reachable by alternating play from
/// Black: equal counts (Black to move — Black wins ties) or one more
/// X (White to move). The position must be non-terminal; a completed
/// five (overlines included) aborts the replay with a panic.
/// Panics on malformed input — test code is allowed to panic.
pub fn board_from_ascii(rows: &str) -> crate::board::Board {
    let mut xs: Vec<(u8, u8)> = Vec::new();
    let mut os: Vec<(u8, u8)> = Vec::new();
    let mut nrows = 0usize;
    for line in rows.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }
        let r = nrows;
        assert!(r < 15, "more than 15 rows");
        assert_eq!(tokens.len(), 15, "row {r} has {} cells, expected 15", tokens.len());
        for (c, t) in tokens.iter().enumerate() {
            match *t {
                "X" => xs.push((r as u8, c as u8)),
                "O" => os.push((r as u8, c as u8)),
                "." => {}
                other => panic!("row {r}, col {c}: unexpected token {other:?} (want X, O, or .)"),
            }
        }
        nrows += 1;
    }
    assert_eq!(nrows, 15, "expected 15 rows, got {nrows}");
    assert!(
        xs.len() == os.len() || xs.len() == os.len() + 1,
        "unreachable stone counts: {} X vs {} O (alternating play from Black first)",
        xs.len(),
        os.len()
    );

    // Replay in parse order, strictly alternating X/O. Cells are
    // distinct by construction, so the only way `play` can fail is a
    // completed five mid-history — i.e. a broken puzzle, which is
    // exactly what a test helper should report.
    let mut b = crate::board::Board::new();
    for k in 0..os.len() {
        let (r, c) = xs[k];
        b.play(Move::new(r, c).unwrap()).expect("puzzle contains a five (X)?");
        let (r, c) = os[k];
        b.play(Move::new(r, c).unwrap()).expect("puzzle contains a five (O)?");
    }
    if xs.len() > os.len() {
        let &(r, c) = xs.last().unwrap();
        b.play(Move::new(r, c).unwrap()).expect("puzzle contains a five (X)?");
    }
    b
}
```

…and three parser tests in `reference.rs`'s `mod tests` (a two-stone
board with `to_move` inference both ways; `should_panic` on a short row
and on impossible counts — the full bodies are mechanical; follow the
pattern of the tactics puzzles above, one X at (0,0) for the
White-to-move case).

### `crates/engine/tests/differential.rs` — the third property

```diff
 use engine::reference;
-use engine::{Board, Move, Status};
+use engine::{Board, Color, Move, Status, double_threats, forced_blocks, immediate_wins};
 use proptest::prelude::*;
```

```rust
proptest! {
    // Tactics detection is expensive — `double_threats` is
    // O(empties^2 x scan) on BOTH sides of the comparison — so this
    // property runs fewer, meatier cases than the play/undo walks.
    // Scale up with PROPTEST_CASES for wide runs.
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// The fast bitboard tactics must agree with the naive line-scan
    /// oracle on random NON-TERMINAL positions (the through-cell scan
    /// answers a different question once a five exists — see
    /// reference.rs).
    #[test]
    fn tactics_match_reference(cells in prop::collection::vec(0u16..225, 1..=120)) {
        let mut fast = Board::new();
        for p in cells {
            if fast.status() != Status::Ongoing { break; }
            let _ = fast.play(Move::new((p / 15) as u8, (p % 15) as u8).unwrap());
        }
        // Compare on a NON-TERMINAL position: once a five exists, the
        // naive through-cell scan answers a different question than the
        // fast whole-board scan (see reference.rs). Undoing the
        // terminal move reopens the game — deterministic, unlike a
        // prop_assume, which at high PROPTEST_CASES aborts the test
        // with "too many global rejects" (measured: ~6% of random
        // 120-ply playouts reach a terminal position).
        if fast.status() != Status::Ongoing {
            fast.undo();
        }

        for side in [Color::Black, Color::White] {
            prop_assert_eq!(
                immediate_wins(&fast, side),
                reference::naive_immediate_wins(&fast, side),
                "immediate wins disagree for {:?}", side
            );
            prop_assert_eq!(
                double_threats(&fast, side),
                reference::naive_double_threats(&fast, side),
                "double threats disagree for {:?}", side
            );
        }
        prop_assert_eq!(forced_blocks(&fast), reference::naive_forced_blocks(&fast));
    }
}
```

### `crates/engine/src/lib.rs` — one hunk

```diff
 pub use board::{Board, Color, PlayError, Status};
-pub use moveset::Move;
+pub use moveset::{Move, MoveSet};
+pub use tactics::{double_threats, forced_blocks, immediate_wins};
```

### Verification log

```text
cargo fmt --all --check                                     clean
cargo clippy -p engine --all-targets --features testutil -- -D warnings
                                                            clean
cargo test -p engine --features testutil --no-fail-fast     61 passed + 3 passed
PROPTEST_CASES=10000  ... full suite                        61 passed + 3 passed (197 s)
PROPTEST_CASES=100000 ... tactics property, --release       1 passed (95 s)
```

The 61 = the 47 from slices 2–5 plus 14 new: `moveset` ×2
(`moveset_roundtrip_across_word_boundaries`, `insert_is_idempotent`),
`tactics` ×9 (the puzzle corpus), `reference` ×3 (parser happy path +
two `should_panic`s). The third differential property is
`tactics_match_reference` (100 cases by default — fewer, meatier).

## What this slice does not do

- **No four-three / win-in-2 search.** The documented blind spot: v1 is
  threat *arithmetic*, not threat *search*. When arena games say it
  matters, it arrives as a measured v2, not a silent criterion change.
- **No opponent-wins-first filtering.** A double threat on a board
  where the opponent has an immediate win is answerable; v1 accepts
  this and relies on self-play checking forced blocks first.
- **No prior blending, no fast path, no synthetic-set generation.** All
  three *uses* are milestone-2/3 wiring decisions with `[experiment]`
  knobs; this slice only delivers detection that vouches for itself.
- **No `Board` changes.** Everything the tactics needed
  (`empty_moves()`, `stones()`, `with_bit`/`without_bit`, `has_any_five`)
  already existed from slices 3–4; the only new bitboard primitive is
  `iter_set_bits`, pub(crate).
