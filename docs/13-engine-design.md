# Chapter 13 — The Engine Design

This document is the detailed design for the `engine` crate — milestone 1
of the Gomoku build ([Chapter 12](12-gomoku-architecture.md)). It was
agreed in dialogue before any code was written. Chapter 12 is the
high-level architecture and already reflects every decision here; this
document adds the concrete types, invariants, and test plan.

## Locked decisions

| # | Decision | Reason |
|---|---|---|
| 1 | **Overlines count as a win** (freestyle rules) | Wolfie's call; simplest rule, consistent with the freestyle commitment (chapter 12, §3) |
| 2 | **Swap2 opening protocol from the start** | Freestyle Gomoku is a proven first-player win (chapter 12, §3); Swap2 keeps self-play on open ground |
| 3 | **17×17 encoding, border ring as opponent stones** | Learning-signal consistency near edges — the learning-side edge problem (chapter 12, §7) |
| 4 | **Alpha-epsilon, not tabula rasa** | Hand-woven tactical detection (win-in-1, forced block, win-in-2) gives the learning curve a head start; doubles as MCTS mock evaluator and synthetic-data generator |
| 5 | **Board stores absolute colors** | Swap2's opening places 3 non-alternating stones; a relative me/you store cannot express that |
| 6 | **Naive reference engine** in `src/reference.rs` behind `#[cfg(any(test, feature = "testutil"))]` | Permanent differential-testing oracle, not throwaway scaffolding |
| 7 | **Public API as tight as possible** | Visibility as enforcement (chapter 12, §6) is cheapest at creation time |

## Crate layout

```text
crates/engine/
├── Cargo.toml          # deps: thiserror, serde. dev: proptest, criterion
└── src/
    ├── lib.rs          # re-exports ONLY: Board, Move, Color, Outcome,
    │                   #   PlayError, Symmetry, MoveSet, tactics::*, encode::*, Swap2
    ├── bitboard.rs     # pub(crate): stride-16 [u64;4], shifts, iterators
    ├── board.rs        # Board: play/undo, legality, status
    ├── win.rs          # has_five (overline counts — decision 1)
    ├── zobrist.rs      # const-generated tables
    ├── symmetry.rs     # D4 group, const tables
    ├── moveset.rs      # public set-of-moves type (logical indexing)
    ├── tactics.rs      # immediate wins, forced blocks, double threats
    ├── opening.rs      # Swap2 as a typestate machine
    ├── encode.rs       # 17×17 planes, border-as-opponent, no Burn
    └── reference.rs    # #[cfg(any(test, feature = "testutil"))] naive oracle
```

## The bitboard and the padding invariant

```rust
pub(crate) struct Bitboard(pub(crate) [u64; 4]); // 240 bits used, stride 16

const fn idx(r: u8, c: u8) -> usize { r as usize * 16 + c as usize }

impl Bitboard {
    fn shr(&self, s: u32) -> Bitboard {   // s < 64; multi-word shift
        let w = self.0;
        Bitboard([
            (w[0] >> s) | (w[1] << (64 - s)),
            (w[1] >> s) | (w[2] << (64 - s)),
            (w[2] >> s) | (w[3] << (64 - s)),
            w[3] >> s,
        ])
    }
}
```

With the invariant **"padding bits (column 15 of each row, bits
240–255) are always zero"**, a 5-chain cannot wrap: every wrap path
crosses a padding bit and the AND-chain dies. So `has_five` needs no
per-direction edge masks at all. Masks are needed in exactly one place:
any complement operation (`!occupied`) sets padding bits, so complements
must immediately AND with a `VALID` mask. The highest-risk code in the
crate becomes one testable invariant:

```rust
const VALID: Bitboard = /* all 225 real cells, padding zero */;

fn empty_cells(&self) -> Bitboard { /* !occupied, then & VALID */ }

#[cfg(test)]
fn assert_clean(b: &Bitboard) {  // proptest invariant, checked after random op sequences
    assert!((*b & !VALID).is_zero());
}
```

Dedicated unit tests at columns 0 and 14 stay — belt and braces.

## Win detection (overlines count)

The naive 5-term AND needs shifts up to `4·17 = 68 > 64`. The staged
version stays under 64 and detects *five or more*, which is decision 1
for free:

```rust
const DIRS: [u32; 4] = [1, 16, 15, 17]; // horizontal, vertical, two diagonals

fn has_five_dir(b: &Bitboard, s: u32) -> bool {
    let two  = *b & b.shr(s);          // runs of >= 2
    let four = two & two.shr(2 * s);   // runs of >= 4
    let five = four & four.shr(s);     // runs of >= 5
    !five.is_zero()
}
```

`Board::play` checks only the color just placed — a win always involves
the last stone. Draw when `moves.len() == 225`.

## Board

```rust
pub struct Board {
    black: Bitboard,   // absolute colors (decision 5)
    white: Bitboard,
    to_move: Color,
    status: Status,    // enum, not bool flags
    moves: Vec<Move>,  // history: encoding, undo, game records
    key: u64,          // Zobrist, incremental
}

enum Status { Ongoing, Won(Color), Draw }
```

`me()` / `you()` accessors restore the relative view where it matters
(encoding, tactics). API: `play`, `undo`, `is_legal`, `empty_moves`,
`stones(color)`, `status`, `zobrist`, `from_position` (validated, for
Swap2 hand-off). `undo` pops the last move, clears the bit, XORs the key
back (XOR is its own inverse), and restores `Status::Ongoing`.

## Zobrist, compile-time

No `rand` dependency in the engine — the table is generated by a
`const fn` at compile time, reproducible forever:

```rust
const fn xorshift(mut x: u64) -> u64 {
    x ^= x << 13; x ^= x >> 7; x ^= x << 17; x
}

const ZOBRIST: [[u64; 225]; 2] = {
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

`play` XORs incrementally; a proptest asserts incremental key ==
from-scratch key after random play/undo sequences.

## Move and MoveSet — why two bit types

```rust
pub struct Move(u8);  // logical index r*15+c, 0..=224 — validated constructor

pub struct MoveSet([u64; 4]);  // logical stride-15 indexing; public
```

Deliberately separate from the internal stride-16 `Bitboard`: two layouts
behind one type is a bug farm; two types make mixing them a compile
error. `MoveSet` is what tactics and `empty_moves` return; MCTS iterates
it. `Bitboard` stays `pub(crate)`.

## Tactics — the alpha-epsilon module

Engine owns *detection* (rules-level truth, pure bit ops). How much the
detections shape priors is an `[experiment]` knob in selfplay/mcts
(blend weight, decay over iterations). The same module serves three more
roles: mock evaluator for the MCTS milestone-2 tactical tests, fast-path
in self-play (play an immediate win without search — config knob), and
generator for the milestone-3 synthetic attack/defense set.

```rust
/// Cells where `side` completes five immediately.
pub fn immediate_wins(b: &Board, side: Color) -> MoveSet {
    let stones = b.stones(side);
    let mut out = MoveSet::EMPTY;
    for mv in b.empty_moves() {
        if has_five_any(&stones.with(mv)) {   // hypothetical placement
            out.insert(mv);
        }
    }
    out
}

/// Cells the side to move MUST play — opponent's immediate wins.
pub fn forced_blocks(b: &Board) -> MoveSet {
    immediate_wins(b, b.to_move().other())
}

/// Moves creating >= 2 immediate wins for `side` (open four, four-three):
/// unanswerable — the win-in-2 detector.
pub fn double_threats(b: &Board, side: Color) -> MoveSet { /* same primitive */ }
```

Cost: ~225 × ~60 ops ≈ 13k ops per call — fine at leaf expansion, not
per PUCT step. Test corpus: hand-built puzzles (win-in-1, must-block,
double threat, near-misses) plus differential checks against the naive
engine's line scanner.

## Swap2 as a typestate machine

The opening protocol has a strict sequence. Typestate turns invalid
transitions into compile errors at zero runtime cost:

```rust
pub struct Swap2<State> { black: Bitboard, white: Bitboard, _state: PhantomData<State> }

pub struct Placing3;    // player A places 2 black + 1 white, any order
pub struct FirstChoice; // player B: take a color, or place 2 more
pub struct Placing2;    // player B placed 2 more (1 black + 1 white)
pub struct FinalChoice; // player A picks a color

impl Swap2<Placing3> {
    pub fn place(&mut self, mv: Move, color: Color) -> Result<(), PlayError>;
    pub fn finish(self) -> Result<Swap2<FirstChoice>, PlayError>; // validates 2B+1W
}
impl Swap2<FirstChoice> {
    pub fn take_black(self) -> Board;
    pub fn take_white(self) -> Board;
    pub fn place_two_more(self) -> Swap2<Placing2>;
}
impl Swap2<FinalChoice> {
    pub fn take_black(self) -> Board;
    pub fn take_white(self) -> Board;
}
```

Each terminal arm hands a `Board` via `Board::from_position(black,
white, to_move)`, which validates no overlap and plausible stone counts
and computes the Zobrist key from scratch.

## Encoding (Burn-free, in the engine)

```rust
pub const EXT: usize = 17;

pub struct Planes {
    pub me:  [u8; EXT * EXT],   // stones of side to move; border = 0
    pub you: [u8; EXT * EXT],   // opponent stones; border ring = 1
}
// idx17 = (r+1)*17 + (c+1)

pub fn encode(b: &Board) -> Planes;
```

Symmetry stays in 15×15 space (`[[u8; 225]; 8]` const tables — `u8`
suffices) and the border is added *after* transformation: the border
ring is D4-invariant, so `encode(sym(b)) == embed(sym15(stones))` holds
exactly. That gives the key property test: **encode commutes with
symmetry**. Planes as `u8` arrays keep the engine Burn-free; `net`
converts them to tensors. Chapter 10's last-move planes (planes 2–3) are
a five-line addition reading `moves.last()` — decide 2-vs-4 planes when
the `net` crate lands; the store-games decision (chapter 9) keeps both
choices retro-compatible.

## Reference oracle

`reference.rs`: `[[Cell; 15]; 15]`, loop-based win scan, same method
names as `Board` — no shared trait (avoids premature abstraction; the
tests drive both). Compiled under `#[cfg(any(test, feature =
"testutil"))]` so `mcts`/`arena` tests can use it later without shipping
it in release builds.

## Test plan (maps to milestone-1 acceptance)

| Test | Kind |
|---|---|
| Win corpus: 4 dirs × edges × overline × near-miss 4s | unit |
| Padding invariant after random op sequences | proptest |
| Fast vs. naive: legality/status/outcome per ply, 10k random games | proptest differential |
| Undo: play/undo random walks == pristine board, key roundtrip | proptest |
| Zobrist incremental == from-scratch | proptest |
| Symmetry: inverse roundtrip, win preserved, encode commutes | proptest |
| Tactics puzzle corpus + differential vs naive | unit + proptest |
| Swap2: scripted protocol sequences + invalid inputs rejected | unit |
| `has_five` ≥ 50M checks/s, play/undo throughput | criterion (built last) |

## Build order (vertical TDD slices)

1. Workspace + `engine` skeleton, fmt/clippy hygiene.
2. `reference.rs` + hand-written corpus (the oracle earns trust first).
3. `Bitboard` + `Board` (play/undo/status) — differential tests green.
4. `win.rs` edge corpus + padding invariant.
5. Zobrist.
6. Symmetry + encode + commutation property.
7. Tactics.
8. Swap2 typestate.
9. Criterion bench.
10. Tighten visibility (`pub(crate)` sweep), rustdoc, done.

Tutorial: [tutorials/13-engine-tutorial/](tutorials/13-engine-tutorial/README.md) — the build-your-own companion to this design.

Back to: [Wiki home](README.md) · Previous: [Chapter 12 — The Gomoku Architecture](12-gomoku-architecture.md)
