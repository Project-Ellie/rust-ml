# Chapter 9 — The tactics-shaped evaluator

## Context

The machine is assembled and it can walk: it selects with PUCT, expands
leaves, backs up values, and chooses a move from visit counts. Every
behavior so far was scored by `UniformEvaluator`, an evaluator with no
opinions at all. Uniform is exactly the right tool for proving that the
tree machinery moves without binding — but it is the wrong tool for
proving that the machine *thinks*. In this chapter you give the search
taste, deterministically, without a GPU (primer §6.2's mock row, §7
item 6).

This is also the chapter where the engine tutorial's slice-7 promise is
cashed in. Go back and read the **ML refresh** section of
[07-tactics.md](../13-engine-tutorial/07-tactics.md); it said the
tactics module is a *perfect prior for the tactical subset* of
positions, usable from day one as the mock evaluator in the MCTS
tactical tests. Today is that day. The same three functions you proved
against the engine oracle — `immediate_wins`, `forced_blocks`,
`double_threats` — now shape an `Evaluator` whose opinions are bitboard
truth.

## Intention

Build `src/mock.rs` containing `TacticsEvaluator`, an implementation of
the `Evaluator` trait that:

* concentrates prior mass on immediate wins for the side to move,
* concentrates prior mass on forced blocks when the side to move has no
  win but the opponent does,
* falls back to near-uniform noise for quiet positions,
* assigns crude but clearly labeled tutorial values: `+0.95` for a
  winning position, `−0.90` when a forced block is required, `0.00`
  otherwise.

Then write unit tests that prove the mock has these opinions on three
ASCII puzzles (won position, must-block position, quiet position), and
run the sign-sentinel tests that chapter 07 deferred — the tests that
make a sign error impossible to miss.

Done means `cargo test -p mcts` is green and the search finally *prefers*
something for a reason.

## Mental mapping

### What the mock knows

The real evaluator, the neural network, will one day emit `(p, v)` for
any board. The mock emits `(p, v)` too, but it derives them from the
engine's tactics module instead of from weights. That is perfectly legal
because the search is generic over `Evaluator`; the tree never learns how
the answer was produced (primer §6.2).

The shaping rule is intentionally simple:

```text
if side-to-move has immediate wins:
    prior: 90% mass split over the winning moves, 10% over the rest
    value: +0.95
else if opponent has immediate wins (forced blocks exist):
    prior: 90% mass split over the blocking moves, 10% over the rest
    value: −0.90
else:
    prior: near-uniform over legal moves
    value: 0.00
```

The numbers are scaffolding, not truths. `0.95` says "this position is
basically won"; `−0.90` says "this position is in danger and the only
salvation is the block". They do not need to be exact because the
tactical tests only ask whether the search *finds* the win or the block,
not whether it values it to three decimal places. Label them loudly in
comments so future-you does not mistake tutorial constants for tuned
hyperparameters.

> **Excursion — why the mock makes the search provable.**
> A random evaluator would still, occasionally, stumble onto a winning
> move. That makes a failing test hard to interpret: was the bug in the
> search, or did the dice just roll badly? The mock is deterministic and
> hand-reasonable. If the mock says "move X is a win" and the search
> does not visit move X, you know the search has a bug — period. The
> prior is strong enough that PUCT's exploration term cannot drown it
> at 50 simulations, and the value has the right sign so that backup
> reinforces the good line rather than punishing it.

### Why this beats a random evaluator for proving the search

There are two separate things to prove about milestone 2.

First, that the *machinery* works: selections happen, edges get visits,
games terminate. For that you want `UniformEvaluator`, because it
exercises the tree without adding opinionated bias.

Second, that the *direction* of the search is correct: when a winning
move exists, the search should find it; when a block is forced, the
search should take it. For that you need an evaluator with a known
preference, and `TacticsEvaluator` is exactly that. It is also the
smallest possible step toward the real network: the same `Evaluator`
trait, the same `EvalRequest`/`EvalResult` shapes, the same mask-then-
normalize pipeline you built in chapter 04.

The mock is also what makes the **sign sentinels** meaningful. The sign
convention in backup (primer §4.4) says that a value good for the
side to move at the leaf becomes bad for the player who chose the
parent's edge. If the convention is correct, a won position gets a
positive value at the leaf, backed up as a negative value at the parent
(which is correct, because the parent chose a move that lets the
opponent win), and so on. With a sane prior, the result is that the
winning move accumulates visits. If the sign is flipped, the search will
*actively avoid* the winning move — a failure so dramatic that you will
see it in one test run.

### The ML-refresh excursion: what a prior *is* in PUCT terms

In `Q(s,a) + c_puct · P(s,a) · √ΣN / (1 + N(s,a))`, the `P(s,a)` term
is the prior. It is the evaluator's one-shot opinion about how good move
`a` is *before any search has happened*. The exploration term multiplies
`P(s,a)` by a visit-count bonus, so high-prior moves get explored first.
Early in training a real network's priors are noise, which means the
search wastes simulations rediscovering basic tactics. The tactics
module is a perfect prior for exactly those tactics: immediate wins and
forced blocks get nearly all the prior mass, so the search does not
wander.

Milestone 3's network will eventually replace the mock. The beauty of
the trait boundary is that the search crate does not change at all. The
same `search()` function, the same `select_move()` logic, the same
acceptance tests will run with a Burn-backed evaluator instead of
`TacticsEvaluator`. The mock is the stand-in that lets you prove the
search before the net exists.

### Honest inheritance: the four-three / double-three blind spots

The mock inherits the engine's documented v1 gaps. `double_threats`
finds open fours and double fours but not the four-three, because a
four-three has only one immediate win *now* and the three matures on the
next ply. The mock therefore does not concentrate prior mass on four-three
positions. That is honest: the engine's tactics module promises only
what it can prove. Deeper forced wins belong to the TSS prover (engine
slice 8), whose verified wins become the milestone-4 anchor set. The
mock's job is to make milestone 2's tactical suite pass, not to solve
every Gomoku position.

### Reconstructing a `Board` from encoded planes

Here is a subtlety you will hit immediately. The `Evaluator` trait
receives `EvalRequest`, which carries encoded `Planes` and a legal-move
list, not a `Board`. But the tactics module functions take `&Board`.
So the mock must reconstruct a `Board` from the encoded planes.

The encoding is relative to the side to move (`me` / `you`), while the
engine stores absolute colors (`Black` / `White`). The reconstruction
works because you can infer `to_move` from stone-count parity: if Black
and White counts are equal, Black is to move; otherwise White is. Then
assign the `me` plane to the player to move and the `you` plane to the
other player. After rebuilding, assert that the supplied legal moves
match `board.empty_moves()` — a cheap safety net that catches any
encoding mismatch during tests.

This reconstruction is only needed for the mock. The real network will
consume the planes directly and never need a `Board`.

## Low-level design

### File: `src/mock.rs`

A new module with one public type:

```rust
pub struct TacticsEvaluator;
```

Implement `Default`, `new()`, and `Evaluator` for it. The `evaluate`
method does the three-case prior/value split described above. It also
needs a private helper to rebuild a `Board` from `EvalRequest`.

Keep the shaping numbers visible and clearly labeled as tutorial
scaffolding. You can use named constants:

```rust
const WIN_HIGHLIGHT: f32 = 2.2;
const BLOCK_HIGHLIGHT: f32 = 2.2;
const MULTI_HIGHLIGHT: f32 = 1.6;
const OTHER_LOGIT: f32 = -1.2;
const WIN_VALUE: f32 = 0.95;
const BLOCK_VALUE: f32 = -0.90;
```

The reference solution uses inline literals for compactness, but the
important thing is that a reader can see which numbers are deliberate
and which are placeholders. Use logits, not probabilities, because
`EvalResult` stores policy logits and the expansion code does softmax. Set highlighted moves to a positive
logit and everything else to a negative logit; the gap between them
concentrates the softmax output. For the near-uniform fallback, set
every legal move's logit to `ln(n)` so that after softmax the
distribution is uniform.

### Tests in `src/mock.rs`

Three unit tests inside `#[cfg(test)] mod tests`:

1. `tactics_evaluator_concentrates_on_immediate_win` — build a board by
   scripted play so Black has an open four on row 7, then assert that
   the value is `0.95` and that every winning move has a positive logit.
2. `tactics_evaluator_values_forced_block_negatively` — use
   `engine::reference::board_from_ascii` to set White with a closed four,
   assert value is `−0.90`.
3. (Optional but recommended) a quiet-position test where all three
   tactics sets are empty and the value is `0.00`.

### Sign sentinels in `src/search.rs`

Add two tests that chapter 07 deferred:

1. `immediate_win_for_side_to_move_gets_most_visits` — ASCII puzzle with
   Black open four, 50 simulations, assert that the most-visited edge is
   a winning move.
2. `opponent_immediate_win_forces_block` — ASCII puzzle with White open
   four, Black to move, 50 simulations, assert that the most-visited
   edge is a block.

These are the sign sentinels. If backup flips the sign, the winning-move
test will fail because the search avoids the win; the forced-block test
will fail because the search avoids the block.

### Wire into `src/lib.rs`

Add `pub mod mock;` and `pub use mock::TacticsEvaluator;` so the
acceptance tests in chapter 10 can import it. You can do this now or in
chapter 10; either way, the chapter-09 tests only need `mock.rs` to
compile.

## Solution (opt-in)

A complete, compiled, tested reference lives in
[09-the-tactics-evaluator/01-solution.md](09-the-tactics-evaluator/01-solution.md). Open it only
if you are stuck for more than twenty minutes or after you have written
your own version and want to compare. The solution quotes `mock.rs`
verbatim from the verified reference crate, plus the sign-sentinel tests
from `search.rs`.

## TDD checklist

1. `TacticsEvaluator::new()` compiles and is `Default`.
2. Quiet board: value `0.00`, all legal moves have equal positive logit
   (or at least equal softmax probability).
3. Win-in-1 for side to move: value `0.95`, winning moves have logits
   much higher than non-winning moves.
4. Forced-block position: value `−0.90`, blocking moves have logits
   much higher than non-blocking moves.
5. Sign sentinel — win-in-1 at 50 simulations: the most-visited root
   edge is a winning move.
6. Sign sentinel — forced block at 50 simulations: the most-visited
   root edge is a blocking move.
7. `cargo test -p mcts` all green; `cargo clippy -p mcts --all-targets
   -- -D warnings` clean; `cargo fmt --all`.

## Done when

From `gomoku/`:

```bash
cargo test -p mcts
cargo clippy -p mcts --all-targets -- -D warnings
cargo fmt --all
```

All green.

Commit: `feat(mcts): tactics-shaped mock evaluator`

Next: [Chapter 10 — Acceptance](10-acceptance.md)
