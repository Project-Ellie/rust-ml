# 05 — Expansion

## Context

Chapter 03 gave you selection: a walk from the root down to a leaf, choosing edges by PUCT, producing a path of `(node, edge)` pairs. Chapter 04 gave you the evaluation seam: a trait that turns encoded planes into policy logits and a value. This chapter connects the two. When selection stops at a leaf, the leaf is in one of three states: unexpanded and ongoing, unexpanded and terminal, or expanded from a previous simulation. Expansion is the function that decides which of those states applies and updates the tree accordingly.

The inputs are the tree, the leaf node id, the board position at that leaf, and a mutable evaluator. The output is a single `f32`: the leaf's value from the perspective of the side to move at that leaf. That value is then handed to backup in chapter 06. This chapter is therefore the bridge between the tree's structure and the tree's learning signal.

Primer §4.2 and §4.3 govern the logic. Read them alongside this chapter.

## Intention

By the end of this chapter you will have implemented, in `src/expand.rs`:

- `expand(tree, leaf, board, evaluator) -> f32`.
- A terminal-first check using `board.status()`.
- Exact values for terminal positions: `Status::Won(_) -> -1.0`, `Status::Draw -> 0.0`.
- No evaluator call for terminals.
- For ongoing positions: encode the board, call the evaluator, run `masked_softmax`, and create one edge per legal move with prior `P`, `N = 0`, `W = 0`.
- Eager child node creation: every new edge immediately gets a child node so selection can descend without mutating the tree.
- Idempotency: re-expanding an already-expanded node or a terminal node must be safe and must not call the evaluator again.

The observable behavior is: after calling `expand`, the leaf node is either `Terminal` with no edges and a known exact value, or `Expanded` with edges whose priors sum to 1 and child nodes attached.

## Mental mapping

### Terminal check first

The very first thing `expand` does is ask the engine for the board status. Not encode. Not evaluate. Status. This ordering is the most valuable optimization in the whole search and also the most important correctness habit.

There are three outcomes:

- `Status::Won(color)` — the player `color` just made five in a row (overlines count; this is freestyle Gomoku, ch. 13). The position is over. The side to move at this leaf is the *other* player, because `color` just moved. Therefore, from the side-to-move's perspective, this position is a loss. Value = `-1.0`.
- `Status::Draw` — the board is full and no one has five. Value = `0.0`.
- `Status::Ongoing` — the game continues, so we need a network evaluation.

The terminal check pays off twice, as primer §4.2 notes.

First, **exact values propagate certainty.** Suppose the search reaches a leaf where White can win next move. That leaf is `Won` from White's point of view, value `-1.0` from Black's point of view. When backup flips that sign up the tree, the edge leading *into* the leaf accumulates `+1.0` for White (good for the player who chose that edge), and the level above accumulates `-1.0` for Black. The tree learns, from engine truth rather than network guess, that "this move leads to a forced win." Multi-ply forced wins are discovered because the search backs up exact terminal values through intermediate nodes until the root sees a move that guarantees victory.

Second, **we save GPU evaluations.** A terminal position needs no network. At 400 simulations per move, many leaves will be near the end of the game; skipping the network there is free performance.

> **Excursion — why a won leaf is `-1.0` from the side to move**
>
> This is the perspective convention introduced in chapter 04 and fully exploited in chapter 06. When the engine says `Status::Won(Black)`, Black has just played the winning move. It is now White's turn at the leaf, but the game is already over. White has lost. From White's perspective — the side to move — the value is `-1.0`.
>
> If you defined terminal value from the winner's perspective, backup would need a separate rule for terminals vs. network evaluations. By defining both from the side-to-move's perspective, one backup rule handles everything. That uniformity is chapter 06's whole story.

### Never evaluate a terminal

The test for this is a `PanicEvaluator`: an evaluator whose `evaluate` method panics. If `expand` ever calls it on a terminal position, the test fails loudly. Write that test. It is a one-line implementation — `panic!("evaluator must not be called on a terminal leaf")` — and it guards against a real class of bug where the network is invoked on a finished game and returns garbage that corrupts the backup statistics.

In the reference implementation, the panic evaluator is a private test struct. You can keep it private; it exists only to prove a contract.

### The expansion sequence for ongoing positions

When the board is ongoing, expansion is a pipeline:

```text
1. Collect legal moves from the board.
2. Encode the board into Planes.
3. Build EvalRequest { planes, legal }.
4. Call evaluator.evaluate(req) -> EvalResult.
5. Run masked_softmax(result.policy, legal) -> Vec<(Move, f32)>.
6. For each (move, prior):
       push an Edge { mv, prior, n: 0, w: 0.0, child: None }
       call tree.add_child(leaf, edge_index) to create and attach the child node
7. Set the node state to Expanded.
8. Return result.value.clamp(-1.0, 1.0).
```

Each step has already been prepared by earlier chapters or the engine. The new work is wiring them together and making sure the edge priors sum to 1.

### Eager child creation

When an edge is created, the reference immediately calls `tree.add_child(leaf, edge_index)` to allocate a child node and set `edge.child = Some(child_id)`. This is eager expansion: the full one-ply subtree under the leaf exists as soon as the leaf is expanded.

The alternative is lazy expansion, where edges exist but child nodes are created only when selection first descends through them. Lazy expansion saves a small amount of memory and a few node allocations, but it complicates selection: selection would encounter edges with `child == None` and need a mutable tree to create the child on the fly. That mutability fight is real in Rust, especially during a read-only descent.

For Gomoku at 400 simulations, the memory cost of eager children is negligible. A fully expanded root has 225 edges and 225 child nodes. A typical search expands a few hundred nodes. Even a thousand nodes is a couple of contiguous `Vec`s. Eager children keep selection simple: every edge with `child.is_some()` can be descended through with an immutable borrow of the tree. The reference keeps `child: Option<NodeId>` in the `Edge` type anyway, both because the tree API already supports lazy attachment and because future experiments might revisit laziness. For now, the convention is: expansion always attaches a child.

### Idempotency

In later chapters, selection may reach a leaf that has already been expanded by an earlier simulation. For example, if a previous simulation expanded the root and then selected the same edge again, the child node exists and may itself have been expanded. In the normal flow this is handled by selection stopping only at unexpanded or terminal nodes, but `expand` should still be defensive.

If `expand` is called on a node whose state is already `Expanded`, it should not re-run the evaluator. The reference returns `0.0` as a safe default. This path is not expected in normal search, but it prevents accidental double evaluation if the selection logic ever changes. Similarly, calling `expand` on a `Terminal` node should re-read the board status and return the same exact value without creating edges.

The idempotency rule is: `expand` is safe to call multiple times on the same leaf, and it never calls the evaluator for a terminal.

### The return value

`expand` returns the leaf value. This is the value that backup will propagate up the path. For terminals it is exact (`-1.0` or `0.0`). For ongoing positions it is the evaluator's value, clamped to `[-1, 1]`. Clamping is defensive: a misbehaving evaluator should not be able to inject infinite statistics into the tree. The network will be trained to output values in that range, but the search should not trust it.

## Low-level design

Create `gomoku/crates/mcts/src/expand.rs` and add `pub mod expand;` to `lib.rs`.

Signature:

```rust
pub fn expand(
    tree: &mut Tree,
    leaf: crate::tree::NodeId,
    board: &engine::Board,
    evaluator: &mut dyn Evaluator,
) -> f32;
```

Dependencies within `mcts`:

```rust
use crate::eval::{EvalRequest, Evaluator, masked_softmax};
use crate::tree::{NodeState, Tree};
```

Dependencies from `engine`:

```rust
use engine::{Board, Status};
```

Implementation outline:

```rust
match board.status() {
    Status::Won(_) => { set Terminal; -1.0 }
    Status::Draw => { set Terminal; 0.0 }
    Status::Ongoing => {
        if already Expanded { return 0.0; }
        let legal: Vec<_> = board.empty_moves().collect();
        let planes = engine::encode(board);
        let result = evaluator.evaluate(EvalRequest { planes, legal: legal.clone() });
        let dist = masked_softmax(&result.policy, &legal);
        for (mv, prob) in dist {
            let edge_index = tree.node(leaf).edges().len();
            tree.node_mut(leaf).edges_mut().push(Edge { mv, prior: prob, n: 0, w: 0.0, child: None });
            tree.add_child(leaf, edge_index);
        }
        set Expanded;
        result.value.clamp(-1.0, 1.0)
    }
}
```

Tests live in `#[cfg(test)] mod tests` at the bottom of `expand.rs`:

1. `terminal_won_leaf_has_no_edges_and_returns_minus_one` — build a won position, call `expand` with a `PanicEvaluator`, assert value `-1.0`, state `Terminal`, zero edges.
2. `terminal_draw_leaf_returns_zero` — build a full-board draw (the no-five stripe pattern is provided in the reference), call with `PanicEvaluator`, assert value `0.0`, state `Terminal`, zero edges.
3. `ongoing_leaf_gets_edges_summing_to_one` — empty board, `UniformEvaluator::new(0.1)`, assert one evaluator call, state `Expanded`, 225 edges, prior sum `1.0`, every edge has `n == 0`, `w == 0.0`, and `child.is_some()`.

For the draw-board test, the reference builds a position where columns 0-1 and 4-5 are one color on even rows and the other color on odd rows, and columns 2-3 are the opposite, such that no five consecutive same-color cells ever occur. You can copy the construction exactly from the solution.

## Solution (opt-in)

The complete reference implementation for this step lives in the deep-dive folder. Open it only after you have tried the step yourself, or when you have been stuck for more than twenty minutes.

- [05-expansion/01-solution.md](05-expansion/01-solution.md)

The solution quotes `src/expand.rs` from the verified reference crate verbatim.

## TDD checklist

1. **Red:** write the `expand` signature. `cargo check -p mcts` fails.
2. **Green:** implement the `Status::Won` branch: set `Terminal`, return `-1.0`.
3. **Red:** write `terminal_won_leaf_has_no_edges_and_returns_minus_one` using a `PanicEvaluator`. Watch it panic if you called the evaluator.
4. **Green:** confirm no panic, correct value, correct state, zero edges.
5. **Green:** implement the `Status::Draw` branch: set `Terminal`, return `0.0`.
6. **Red:** write `terminal_draw_leaf_returns_zero`.
7. **Green:** confirm value, state, zero edges.
8. **Green:** implement the `Status::Ongoing` branch: collect legal moves, encode, evaluate, `masked_softmax`, create edges and eager children, set `Expanded`, return clamped value.
9. **Red:** write `ongoing_leaf_gets_edges_summing_to_one`.
10. **Green:** assert 225 edges, priors sum to 1, children attached, one evaluator call.
11. **Gates:** `cargo test -p mcts`, `cargo clippy -p mcts --all-targets -- -D warnings`, `cargo fmt --all`.

## Done when

- `cargo test -p mcts` passes.
- `cargo clippy -p mcts --all-targets -- -D warnings` passes.
- `cargo fmt --all` passes.
- Terminal positions return exact values and never call the evaluator.
- Ongoing positions become `Expanded` with one edge per legal move, priors summing to 1, and eager child nodes.
- Re-expansion is safe.

Commit: `feat(mcts): expansion (terminal-first, edge priors)`

Next: [Chapter 06 — Backup: the sign convention](06-backup.md)
