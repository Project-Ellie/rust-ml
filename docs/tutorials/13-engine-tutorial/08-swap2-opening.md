# Slice 8 — The Swap2 opening

The protocol that keeps self-play on open ground, encoded so that wrong
sequences *do not compile*. Design: ch. 13, "Swap2 as a typestate
machine"; background: ch. 12, §3.

## The protocol (freestyle Swap2)

1. Player A places three stones: two Black, one White, anywhere.
2. Player B chooses: play **White**, play **Black**, or **place two
   more stones** (one Black, one White).
3. If B placed two more, player A chooses a color.
4. Normal alternating play begins. The side to move is determined by
   stone counts, not by who chose: after every branch the counts differ
   by exactly one (2B+1W or 3B+2W), so White — the color with fewer
   stones — moves first, whoever holds it.

(Verify step 4's phrasing against your reference implementation while
coding — the invariant that matters: after the opening, counts differ by
exactly one and `to_move` is the color with fewer stones.)

## Contract

`opening.rs`:

```rust
pub struct Swap2<State> { /* black, white bitboards + PhantomData */ }

pub struct Placing3;
pub struct FirstChoice;
pub struct Placing2;
pub struct FinalChoice;

impl Swap2<Placing3> {
    pub fn new() -> Self;
    pub fn place(&mut self, mv: Move, color: Color) -> Result<(), PlayError>;
    pub fn finish(self) -> Result<Swap2<FirstChoice>, PlayError>;  // needs exactly 2B + 1W
}

impl Swap2<FirstChoice> {
    pub fn take_black(self) -> Board;
    pub fn take_white(self) -> Board;
    pub fn place_two_more(self) -> Swap2<Placing2>;
}

impl Swap2<Placing2> {
    pub fn place(&mut self, mv: Move, color: Color) -> Result<(), PlayError>;
    pub fn finish(self) -> Result<Swap2<FinalChoice>, PlayError>;  // now 3B + 2W
}

impl Swap2<FinalChoice> {
    pub fn take_black(self) -> Board;
    pub fn take_white(self) -> Board;
}
```

`Board` gains:

```rust
/// Builds a board from an arbitrary valid position (opening hand-off).
/// Errors: overlapping colors, stone counts inconsistent with `to_move`.
pub fn from_position(black: &[Move], white: &[Move], to_move: Color)
    -> Result<Board, PositionError>;
```

(`&[Move]`, not bitboards: `Bitboard` is `pub(crate)`, so the public
entry point takes move lists and builds the bitboards internally.)

`from_position` computes the Zobrist key from scratch (`compute_key`)
and validates counts: |#black − #white| ≤ 1, with `to_move` the color
having fewer-or-equal stones.

## Rust toolbox: typestate

```rust
pub struct Swap2<State> {
    black: Bitboard,
    white: Bitboard,
    _state: PhantomData<State>,
}
```

- `PhantomData<State>` is a zero-size marker: the struct carries the
  type parameter without storing anything. Runtime cost: nothing.
- Methods live on `Swap2<Placing3>` — so calling `take_black` on a
  fresh `Swap2::new()` is a **compile error**, not a runtime rejection.
  The protocol's illegal states are unrepresentable; that is the whole
  pattern (ch. 13 cites the same idea behind typed builders).
- Transitions consume `self` (`fn finish(self) -> Swap2<FirstChoice>`).
  Ownership does the work: after the move, the old state value is gone
  and cannot be reused.
- Monomorphization stamps a tiny specialized type per state — no
  vtables, no dispatch.

When *not* typestate: if states must be stored in one collection or
chosen at runtime (e.g., loading a saved game), an enum wins. Here the
flow is fixed and linear — typestate is the right tool.

## TDD checklist

1. Happy path, branch 1: place 2B + 1W → `finish` → `take_white` →
   `Board` with correct stones, `to_move == White` (counts are 2B+1W;
   the color with fewer stones moves)
2. Branch 2: → `take_black`
3. Branch 3: → `place_two_more` → place 1B + 1W → `finish` →
   `take_black` / `take_white`
4. `finish` with wrong counts (1B+1W, 3B+0W) → `Err`
5. `place` on an occupied cell → `Err`
6. `from_position` rejects overlap and impossible counts; accepts the
   empty board with `to_move == Black`
7. Key correctness: the board coming out of an opening has
   `zobrist() == compute_key(...)` — and a board reached *via opening*
   vs the identical position built by `play` sequences have equal keys

## ML refresh: why the opening rule matters to learning

Freestyle Gomoku is a proven first-player win (ch. 12, §3). Self-play
under plain rules would converge toward "whoever is Black wins" — the
network learns an opening-book truth, not Gomoku. Swap2 equalizes:
player A must place an opening *balanced enough* that B's color choice
is genuinely hard, so the agent learns to evaluate positions, not
sides. It is the cheapest form of domain knowledge you can add — one
protocol, no heuristics — and it is why this module is in milestone 1
instead of "later".

## Pitfalls

- Keep the color-vs-player mapping straight: "player A" and "Black" are
  different axes during the opening. Name variables `player` vs `color`
  accordingly.
- `take_black`/`take_white` must set `to_move` from *stone counts*, not
  from who chose. Test 7 exists for this.
- Resist adding a `Debug` print of the state machine's internals via
  `PhantomData` — `#[derive(Debug)]` on `Swap2<S>` needs `S: Debug`;
  derive `Debug` on the four marker structs and move on.

## Done when

All branches + rejections green; key-equality test green; gates green.
Commit: `feat(engine): Swap2 opening as typestate machine`.

Next: [Slice 9 — Benchmarks and hardening](09-benchmarks-and-hardening.md)
