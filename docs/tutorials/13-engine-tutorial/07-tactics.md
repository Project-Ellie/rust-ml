# Slice 7 — Tactics (the alpha-epsilon module)

The hand-woven common sense: win-in-1, forced blocks, and win-in-2
double threats — pure bitboard truth, no learning. Design: ch. 13,
"Tactics".

## Contract

`moveset.rs` grows the public set type:

```rust
/// A set of moves as a bitset over logical indices (stride 15).
pub struct MoveSet([u64; 4]);

impl MoveSet {
    pub const EMPTY: MoveSet;
    pub fn insert(&mut self, mv: Move);
    pub fn contains(&self, mv: Move) -> bool;
    pub fn len(&self) -> u32;
    pub fn iter(&self) -> impl Iterator<Item = Move> + '_;
}
```

`tactics.rs`:

```rust
/// Cells where `side` completes five immediately.
pub fn immediate_wins(b: &Board, side: Color) -> MoveSet;

/// Cells the side to move MUST play — the opponent's immediate wins.
pub fn forced_blocks(b: &Board) -> MoveSet;

/// Moves that create >= 2 immediate wins for `side` (open four,
/// four-three): unanswerable next move — the win-in-2 detector.
pub fn double_threats(b: &Board, side: Color) -> MoveSet;
```

`immediate_wins` is the ch. 13 snippet: for each empty cell, set the bit
hypothetically (`stones.with(mv)` — value semantics, no mutation), run
`has_five_any`. `double_threats` reuses it: for each candidate move of
`side`, build the hypothetical board and count `immediate_wins` — two or
more means the opponent cannot block both.

## Rust toolbox: a readable puzzle corpus with an ASCII parser

Tactical tests die of unreadability when written in coordinates. Build
this test helper (in `#[cfg(test)]` or `testutil`) and your corpus
becomes self-documenting:

```text
. . . . . . . .
. . X X X X . .
. . O O . . . .
```

```rust
/// Parses rows of X / O / . into a Board (Black = X, White = O).
/// Panics on malformed input — test code is allowed to panic.
fn board_from_ascii(rows: &str) -> Board;
```

Implementation hints: `str::lines`, `split_whitespace`,
`filter_map` over `char`. Decide and document what `to_move` is after
parsing (simplest: infer from stone counts — fewer stones moves next,
Black wins ties).

## TDD checklist

1. `MoveSet` insert/contains/len/iter roundtrip on hand-picked moves
2. Win-in-1: open four for Black → exactly the one (or two) winning
   cells — write for both colors
3. Forced block: White has an open four, Black to move →
   `forced_blocks` == White's winning cells
4. Double threat: the classic fork — Black plays the cell that opens two
   fours → that cell ∈ `double_threats`
5. Negative corpus: quiet positions (scattered stones, no fours) → all
   three functions empty; a *closed* four (blocked one end) produces a
   forced block but no double threat
6. Differential vs the reference engine: implement the same three
   functions naively in `reference.rs` (line scans), proptest random
   mid-game positions, demand equal `MoveSet`s
7. Edge puzzles: immediate win along row 0; double threat in a corner

## ML refresh: what a prior is, and why this is a head start

MCTS picks which edge to explore using **PUCT**: `Q(s,a) + c_puct ·
P(s,a) · √ΣN / (1 + N(s,a))`. The `P(s,a)` term is the *prior* — the
network's (or anyone's) one-shot opinion about move quality before any
search. Early in training the network's priors are noise, so the search
wastes thousands of simulations rediscovering "take the win, block the
loss". This module is a *perfect prior for the tactical subset* of
positions, available from day one:

- as the **mock evaluator** in the MCTS tactical tests (milestone 2),
- as an optional **fast path** in self-play (immediate win → play it,
  skip search — a config knob, measured as `[experiment]`),
- as the generator for the milestone-3 synthetic attack/defense set
  (AlphaGomoku's curriculum, our registered fallback).

How strongly these priors blend into training, and whether that blend
decays as the network matures, is a training-time decision — the engine
only vouches for what is *true*.

## Pitfalls

- `immediate_wins` takes a `side` argument — it answers for *either*
  color regardless of `to_move`. `forced_blocks` is where the turn
  matters. Keep that asymmetry; it is what makes the API honest.
- `double_threats` is O(empties × immediate_wins) ≈ 225 × 13k ops —
  fine at expansion time, never call it per PUCT step.
- A "double threat" that includes an immediate opponent win on the same
  board is *not* unanswerable (the opponent wins first). v1 accepts this
  simplification — self-play ordering (forced blocks checked first)
  handles it. Document it in the module docs; refine only if arena games
  show it matters.

## Done when

Puzzle corpus + differential green; gates green.
Commit: `feat(engine): tactical detection (wins, blocks, double threats)`.

Next: [Slice 8 — The Swap2 opening](08-swap2-opening.md)
