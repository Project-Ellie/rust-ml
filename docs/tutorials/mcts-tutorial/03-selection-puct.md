# Chapter 03 — Selection: PUCT

## Context

You now have an arena tree. It can hold nodes and edges, and every edge
can report its `Q`. The next piece of the search is the descent: start
at the root, repeatedly choose the edge with the highest PUCT score,
and stop when you reach a node that is either unexpanded or terminal
(primer §4.1). This chapter builds only that descent. Expansion — the
act of turning an unexpanded leaf into an expanded or terminal node — is
Chapter 05, so for now you will test `select()` on hand-built trees.

The formula itself is in primer §3. This chapter is where you turn that
formula into code and, more importantly, into tests you can compute by
hand.

## Intention

Implement `select.rs`: a `puct_score()` function that computes `Q + U`
for one edge, a `Selection` struct that records where the descent ended,
what the board looks like at that leaf, and the path taken, and a
`select()` function that walks the tree from the root according to PUCT
until it hits an unexpanded or terminal node.

Observable done-state: you can build a tiny expanded tree by hand, run
`select()` from an empty board, and assert that it follows the PUCT
winner, records the correct path, and stops at the right leaf. You can
also reproduce the numeric example from primer §3 in a unit test with an
epsilon comparison.

## Mental mapping

### PUCT, worked by hand

Before you write the function, compute the example from primer §3
yourself. The table is worth reproducing here because every number in it
will appear in a test.

A root node has three edges. `c_puct = 1.5`. The total visit count over
the three edges is `ΣN = 60 + 30 + 10 = 100`.

| move | P | N | W | Q = W/N | U | Q + U |
|---|---|---|---|---|---|---|
| a | 0.50 | 60 | 30 | 0.50 | 1.5·0.5·√100/(1+60) ≈ 0.12 | **0.62** |
| b | 0.30 | 30 | 12 | 0.40 | 1.5·0.3·√100/(1+30) ≈ 0.15 | 0.55 |
| c | 0.20 | 10 | −1 | −0.10 | 1.5·0.2·√100/(1+10) ≈ 0.27 | 0.17 |

Check move `a` carefully. `Q = 30 / 60 = 0.5`. The exploration term is
`1.5 * 0.50 * sqrt(100) / (1 + 60) = 1.5 * 0.5 * 10 / 61 = 7.5 / 61 ≈
0.123`. So `Q + U ≈ 0.623`, which the table rounds to **0.62**.

Move `b`: `Q = 12 / 30 = 0.4`. `U = 1.5 * 0.30 * 10 / 31 = 4.5 / 31 ≈
0.145`. `Q + U ≈ 0.545`, rounded to **0.55**.

Move `c`: `Q = -1 / 10 = -0.1`. `U = 1.5 * 0.20 * 10 / 11 = 3.0 / 11 ≈
0.273`. `Q + U ≈ 0.173`, rounded to **0.17**.

Your test should recreate these three edges, call `puct_score()` on
each, and assert that `a > b > c` and each value is within `0.01` of the
table. This one test catches most PUCT bugs before they ever see a real
board.

### Exploitation versus exploration

`Q` is exploitation: "what has this move been worth on average?" `U` is
exploration: "how much should I still want to look here?"

The subtlety is in the numerator of `U`: `sqrt(Σ_b N(s,b))`. The sum is
over *all* edges of the parent. That means every time *any* sibling is
visited, the exploration bonus of every unvisited sibling grows. Move
`c` in the example looks bad, but as `a` and `b` keep being visited,
`ΣN` rises and `c`'s `U` rises with it. Eventually `c` gets another
chance. PUCT never permanently writes a move off; it just waits until
the cumulative pressure of "I have looked elsewhere enough" becomes too
large to ignore.

This is why the mock evaluator in Chapter 09 can rescue a young network:
even if the network's prior is noisy, PUCT will eventually explore the
high-prior moves that the tactics module says matter.

### Floating-point argmax

The descent needs to find the edge with the maximum `Q + U`. The naive
way is an iterator fold starting from `f32::NEG_INFINITY`. That is what
the reference does, and it is correct for our numbers, but it is worth
knowing the pitfalls:

* **Ties.** If two edges have exactly the same score, the fold keeps the
  first one it saw. That makes the descent deterministic, which is good
  for tests, but it also means the search can become blind to genuinely
  tied moves. For now, deterministic first-wins is fine; later, when
  move selection is involved, you will add explicit tie handling if
  needed.
* **NaN.** A `NaN` prior or a `NaN` value would poison the comparison.
  The reference does not explicitly guard against `NaN` because the
  inputs come from controlled sources: the evaluator's masked softmax or
  exact terminal values. If you ever feed a network output directly into
  PUCT, add a debug assertion or a normalization step.

The test from the worked example is also where you practice epsilon
comparisons. Never compare two `f32` scores with `==` unless you are
testing against exact constants you constructed. Use `abs() < 1e-6` or,
for the table, `abs() < 0.01`.

> **Excursion — the clone-per-simulation decision**
>
> During selection the tree stores moves on edges, not boards in nodes.
> To know whether a leaf is terminal, you need the board state at that
> leaf. The simplest approach is to clone the root board once per
> simulation and play every move on the path. That is what the v1
> reference does (primer §4.1).
>
> The alternative is to keep a single board and `play`/`undo` moves as
> you descend and back up. That avoids a clone, but it requires an
> `undo` operation and careful bookkeeping. Whether it is faster is a
> measurement question, not a design question. The reference leaves the
> play/undo optimization for later profiling because the clone is
> obviously correct and the hot loop is only 400 simulations deep.
>
> In this chapter, `select()` takes `root_board: &Board` and returns a
> `Selection { board, ... }` where `board` is the cloned board with the
> path played on it.

### Why the path records both node id and edge index

`Selection::path` is `Vec<(NodeId, usize)>`. Why both pieces?

* The `NodeId` identifies the parent node, which `backup()` needs so it
  can walk back up the tree and increment the right edge's `n` and `w`.
* The `usize` identifies which edge of that node was chosen, because a
  node's edges are stored in a `Vec<Edge>`.

You could store only the node ids and recompute the chosen edge, but
that would be both slower and more error-prone. Store both. Every
simulation creates exactly one path, and the path length is at most the
depth of the tree, so the memory cost is negligible.

### Where selection stops

`select()` loops while the current node is `Expanded`. It stops on
`Unexpanded` or `Terminal`. That is the contract that makes expansion
possible: an unexpanded leaf is ready to be evaluated, and a terminal
leaf is ready to back up an exact value. Do not try to expand inside
`select()`; that is Chapter 05's job.

## Low-level design

### File

`gomoku/crates/mcts/src/select.rs`.

Tests live in the same file under `#[cfg(test)] mod tests`.

### Types and signatures

```rust
use crate::tree::{Edge, NodeId, NodeState, Tree};
use engine::Board;

#[must_use]
pub fn puct_score(edge: &Edge, parent_visits: u32, c_puct: f32) -> f32;

#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub leaf: NodeId,
    pub board: Board,
    pub path: Vec<(NodeId, usize)>,
}

#[must_use]
pub fn select(tree: &Tree, root_board: &Board, c_puct: f32) -> Selection;
```

Notes:

* `puct_score` takes `parent_visits` explicitly rather than summing the
  edges itself. That keeps the function pure and easy to unit-test.
* `parent_visits` is `Σ_b N(s,b)`. If it is zero, the formula would
  divide by zero or take the square root of zero; the reference uses
  `parent_visits.max(1)` so that unvisited parents still produce a
  finite exploration term. This only matters for the root on the very
  first simulation.
* `select()` returns a cloned board with the path played. It panics only
  on internal invariant violations: an expanded node with no edges, or a
  move that the engine rejects as illegal. Those should never happen if
  `expand()` creates legal edges.

### The hand-computed PUCT test

Create three `Edge` values matching the primer table. Call
`puct_score(&edge, 100, 1.5)` for each. Assert:

```rust
assert!((score_a - 0.62).abs() < 0.01);
assert!((score_b - 0.55).abs() < 0.01);
assert!((score_c - 0.17).abs() < 0.01);
assert!(score_a > score_b);
assert!(score_b > score_c);
```

This test is the chapter's anchor. If it passes, your PUCT arithmetic
matches the design document.

### Other tests

1. **Unvisited high-prior beats visited low-prior when Q is equal.**
   With `ΣN = 10`, a visited edge with `P = 0.01`, `N = 10`, `W = 5.0`
   (`Q = 0.5`) should lose to an unvisited edge with `P = 0.99`, `N =
   0`, `W = 0.0` (`Q = 0.0`). This proves exploration dominates early.

2. **Select descends the PUCT winner and records the path.** Build a
   root with two edges, attach a child to edge 0, mark the root
   `Expanded`, and call `select()` from `Board::new()`. Assert the leaf
   is the child, the path is `[(0, 0)]`, and the board has one move
   played.

3. **Select stops at unexpanded.** A freshly created tree has only an
   unexpanded root. `select()` should return leaf `0` with an empty path
   and a board equal to the root board.

4. **Select stops at terminal.** Mark the root `Terminal` and assert
   the same empty-path behavior. Terminal nodes have no edges, so the
   loop body must never run.

## Solution (opt-in)

The complete reference for this chapter — `select.rs` including its
built-in tests — lives in
[03-deep-dive/01-solution.md](03-deep-dive/01-solution.md). Open it
only if you have been stuck for more than twenty minutes, or after the
chapter for comparison.

## TDD checklist

1. **Red:** Add `select.rs` to the crate, declare `pub mod select` in
   `lib.rs`, and write the `puct_worked_example_from_primer` test first.
   Run `cargo test -p mcts`. It will fail to compile because
   `puct_score` and `Edge` are not yet wired together.
2. **Green:** Implement `puct_score()` with the formula from primer §3.
   Implement `Selection` and `select()`. Run the primer test and the
   four structural tests. All should pass.
3. **Refactor:** Look at the argmax fold. Make sure it starts from
   `f32::NEG_INFINITY`, not `0.0` — a common bug that makes negative
   scores invisible. Check that the path records `(NodeId, usize)` and
   that the cloned board reflects every move on the path. Run clippy
   and fmt.

## Done when

* `cargo test -p mcts` passes from `gomoku/`.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all` makes no changes.
* Commit message: `feat(mcts): PUCT selection`.

Next: [Chapter 04 — The evaluation seam](04-the-evaluation-seam.md)
