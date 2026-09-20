# 06 — Backup: the sign convention

## Context

You can select a path. You can expand a leaf. You can ask an evaluator for a value. Now the last phase of one simulation: the leaf has produced a value, and every edge on the selection path must learn from it. That learning is backup.

Backup is the simplest function in the crate by line count and the most expensive function to get wrong by Elo. Every MCTS implementation that has ever failed to solve a win-in-1 puzzle has failed here. Every DeepGomoku scar lives here. Primer §4.4 says "read this twice"; this chapter says it a few more times, with numbers.

The contract is tiny: given a path from root to leaf and a leaf value, update `N` and `W` on every edge so that `Q = W / N` converges to the true mean value of choosing that edge. The difficulty is entirely in the sign.

## Intention

By the end of this chapter you will have implemented, in `src/backup.rs`:

- `backup(tree, path, leaf_value)` where `path` is `&[(NodeId, usize)]` ordered root-to-leaf.
- Increment `N` on every edge in the path.
- Add `±leaf_value` to `W` on every edge, alternating sign at each level so that the edge into the leaf receives `-leaf_value` and the edge above it receives `+leaf_value`, and so on up to the root.
- Leave terminal values and network values handled by the same rule because both are defined from the side-to-move's perspective.

The observable behavior is: after one backup, every edge on the path has `N = 1` and `W` equal to the leaf value seen from the perspective of the player who chose at that edge.

## Mental mapping

### The one idea that matters: perspectives

There is no single "value of a position" in MCTS. There are only values from someone's point of view. The network's `v` is from the perspective of the player to move at the leaf. Each edge in the tree belongs to the player who chose that edge at that node. Those two perspectives are not the same, and they alternate as you walk up the tree.

This is the entire chapter. Everything else is bookkeeping.

Consider a path of three nodes: root, child, leaf. At the root, some player — call them Player A — chooses a move. At the child, Player B chooses a move. At the leaf, it is Player A's turn again, and the evaluator says the position is worth `+0.8` for the player to move. That means `+0.8` for Player A at the leaf.

Now look at the edge from child to leaf. That edge represents the move Player B chose at the child. If the resulting position is worth `+0.8` for Player A, then it is worth `-0.8` for Player B. So that edge's `W` should increase by `-0.8`.

Look at the edge from root to child. That edge represents the move Player A chose at the root. Player B then moved to the leaf, where Player A is happy (`+0.8`). So the root-to-child edge is also good for Player A, and its `W` should increase by `+0.8`.

Summary:

| edge | chosen by | W increment for leaf value +0.8 |
|---|---|---|
| root → child | Player A | `+0.8` |
| child → leaf | Player B | `-0.8` |

The sign flips every level. That flip is not an implementation detail; it is the translation between "good for the player to move at the leaf" and "good for the player who chose each edge."

> **Excursion — why the edge into the leaf is negative**
>
> This is the most common point of confusion. The leaf value is from the side to move *at the leaf*. The edge into the leaf was chosen by the player who *just moved* to reach the leaf — the opponent of the side to move. What is good for the side to move is bad for the player who just moved. Hence the edge into the leaf gets the negated value.
>
> Another way to say it: the value of an edge is the value of the position *after* the move, from the perspective of the player who *made* the move. If the position after my move is good for my opponent, it is bad for me. So my edge's `W` gets the negative of the leaf value.

### Reproducing the primer's diagram as a table

Primer §4.4 shows the same idea with a diagram. Here is the table version, which you should fill in yourself before writing code. Assume a three-edge path root → a → b → leaf, and a leaf value of `+0.6` for the side to move at the leaf.

| edge | player who chose | sign relative to leaf | W increment |
|---|---|---|---|
| b → leaf | Player who moved to leaf | opposite | `-0.6` |
| a → b | Their opponent | same | `+0.6` |
| root → a | Player who moved to leaf | opposite | `-0.6` |

Check your own implementation against this table with concrete numbers. Do not trust the algebra until the arithmetic feels obvious.

### Two equivalent formulations

There are two ways to implement backup, and both are correct.

**Formulation 1: negate once, then flip per level.**

Start with `value = -leaf_value`. This is the value from the perspective of the player who chose at the leaf's parent. Walk the path in reverse, adding `value` to each edge, then flipping `value = -value` for the next level up.

```rust
let mut value = -leaf_value;
for &(node_id, edge_index) in path.iter().rev() {
    edge.w += value;
    edge.n += 1;
    value = -value;
}
```

**Formulation 2: flip per step.**

Keep the leaf value as-is. At each step up, before adding, negate according to whether the edge is at an odd or even distance from the leaf.

The reference uses formulation 1 because it keeps the variable `value` semantically tied to the current edge: at every iteration, `value` is exactly the increment that edge should receive. There is no parity math to get wrong. The first edge processed is the edge into the leaf, and it receives `-leaf_value` — visibly correct.

Either formulation works. Pick one and do not second-guess it in the middle of debugging. The reference's choice is the one quoted in the solution; if you deviate, make sure your tests still pass.

### Terminal values obey the same rule

In chapter 05 we defined terminal values from the side-to-move's perspective: `-1.0` for a won position (the side to move has lost), `0.0` for a draw. That was not an arbitrary choice. It was the choice that lets backup use one uniform rule for every leaf value, terminal or network.

If terminal values had been defined from the winner's perspective, backup would need a branch: "if terminal, negate differently; if network, use the leaf value." That branch is a bug farm. By defining both kinds of value the same way, the backup code stays simple and obviously correct.

This is an example of a design decision paying off across chapters. Chapter 04 established the value perspective. Chapter 05 honored it for terminals. Chapter 06 reaps the benefit: one loop, one rule.

### Why this catches sign errors instantly in tactical puzzles

Chapter 07 will wire selection, expansion, and backup into a loop called `search`. Chapter 09 will add a tactics-shaped evaluator that gives high prior and near-certain value to immediate wins and forced blocks. Chapter 10 will run tactical puzzles: positions where the winning move is obvious to the engine.

If the sign convention is wrong, the search will avoid the winning move. Here is why. Suppose the root has a move that wins immediately. The child node after that move is a won position for the player who moved, value `-1.0` from the side to move at the child. Backup should add `+1.0` to the root edge for that winning move (because it is good for the player who chose it). If you instead add `-1.0`, the edge looks like a terrible move. PUCT will avoid it. The tactical test fails.

That is why the tactical suite is the ultimate sign-convention test. A wrong sign does not crash; it silently plays worse. The puzzle corpus catches it because the correct answer is known from engine truth.

### The visit-count invariant

After `backup`, every edge on the path has `N` incremented by exactly 1. The root's `Σ_b N(s, b)` therefore increases by 1 per simulation, which is exactly what the PUCT formula expects. If you ever forget to increment `N` on some edge, the exploration term will be wrong and the search will under-explore that subtree.

A simple invariant test: build a path by hand, call backup, and assert that `N` is 1 on every edge in the path and 0 on every edge not in the path. This catches off-by-one errors in the loop.

### Edge ordering in the path

The path is passed as `&[(NodeId, usize)]` in root-to-leaf order: first the edge chosen at the root, last the edge chosen at the leaf's parent. Backup must walk it in reverse, from leaf back to root, because the sign flip starts at the leaf. The reference uses `path.iter().rev()`.

If you walked it root-to-leaf instead, the first edge would receive the wrong sign and the alternation would be backwards. This is another classic sign-error shape. The test with a hand-built three-edge path catches it because the asserted `W` values will be backwards.

## Low-level design

Create `gomoku/crates/mcts/src/backup.rs` and add `pub mod backup;` to `lib.rs`.

Signature:

```rust
pub fn backup(tree: &mut Tree, path: &[(NodeId, usize)], leaf_value: f32);
```

Dependencies:

```rust
use crate::tree::{NodeId, Tree};
```

Implementation:

```rust
let mut value = -leaf_value;
for &(node_id, edge_index) in path.iter().rev() {
    let edge = &mut tree.node_mut(node_id).edges_mut()[edge_index];
    edge.n += 1;
    edge.w += value;
    value = -value;
}
```

Tests live in `#[cfg(test)] mod tests` at the bottom of `backup.rs`:

1. `backup_sign_convention` — hand-build a root → child → leaf path. Call `backup(&mut tree, &path, 0.8)`. Assert:
   - root edge: `n == 1`, `w == 0.8`, `q() == 0.8`.
   - child edge: `n == 1`, `w == -0.8`, `q() == -0.8`.
2. `backup_increments_visits_along_path` — create edges with pre-existing `n` and `w`, call backup with value `0.5`, assert `n` increments and `w` updates with the correct sign. The reference starts root edge at `n = 5, w = 2.0` and expects `n = 6, w = 2.5` (the value flips twice between root and the edge below it, so the root edge receives `+0.5`).
3. `backup_on_empty_path_is_noop` — `backup(&mut tree, &[], 0.5)` should leave the tree unchanged.

Use a helper to build edges:

```rust
fn make_edge(mv: (u8, u8), prior: f32, n: u32, w: f32) -> Edge { ... }
```

The `make_edge` helper belongs in the test module; it is not part of the public API.

## Solution (opt-in)

The complete reference implementation for this step lives in the deep-dive folder. Open it only after you have tried the step yourself, or when you have been stuck for more than twenty minutes.

- [06-deep-dive/01-solution.md](06-deep-dive/01-solution.md)

The solution quotes `src/backup.rs` from the verified reference crate verbatim.

## TDD checklist

1. **Red:** write the `backup` signature. `cargo check -p mcts` fails.
2. **Green:** implement backup with `let mut value = -leaf_value` and a reverse iteration over `path`.
3. **Red:** write `backup_sign_convention` with a two-edge hand-built path and leaf value `0.8`. Predict the `W` values on paper first.
4. **Green:** confirm the root edge has `W == 0.8` and the child edge has `W == -0.8`.
5. **Red:** write `backup_increments_visits_along_path` with non-zero starting `N` and `W`.
6. **Green:** confirm visit counts increment by exactly 1 and `W` updates with the correct sign.
7. **Red:** write `backup_on_empty_path_is_noop`.
8. **Green:** confirm the tree is unchanged.
9. **Gates:** `cargo test -p mcts`, `cargo clippy -p mcts --all-targets -- -D warnings`, `cargo fmt --all`.

As a sanity exercise, repeat the hand-computed table from the Mental mapping section with your own numbers (for example, leaf value `-0.3`) and trace through your code line by line before running the test.

## Done when

- `cargo test -p mcts` passes.
- `cargo clippy -p mcts --all-targets -- -D warnings` passes.
- `cargo fmt --all` passes.
- `backup` increments `N` on every edge in the path.
- `backup` alternates the sign of `leaf_value` at every level, starting with `-leaf_value` on the edge into the leaf.
- A hand-computed three-edge path matches the actual `W` values to the last decimal.
- Terminal values and network values are handled by the same rule.

Commit: `feat(mcts): backup with sign convention`

Next: [Chapter 07 — The simulation loop](07-the-simulation-loop.md)
