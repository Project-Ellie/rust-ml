# Chapter 07 — The simulation loop

## Context

Chapters 02–06 built the four phases in isolation: the arena tree,
PUCT selection, the evaluator seam, expansion, and backup. Each piece
was tested by hand. Now the machine turns on.

A complete search is nothing more than `simulations` repetitions of
one cycle:

```text
select → expand → backup
```

This chapter wires the cycle into a top-level `search()` function and
gives it a configuration knob: how many simulations to run, and how
aggressively to explore. The output is not a single move — that is the
next chapter's job — but a grown tree whose root edges carry the visit
counts that will become both the competitive move and the self-play
training target (primer §5).

## Intention

By the end of this chapter you will have:

* a `SearchConfig` struct with sensible v1 defaults (`simulations: 400`,
  `c_puct: 1.5`);
* a `SearchOutcome` that returns the tree and the root node id;
* a `search(root_board, evaluator, config)` entry point that runs the
  loop;
* one green unit test proving the loop's bookkeeping: the sum of root
  edge visits equals the number of simulations, exactly.

You will *not* yet prove that the loop wins tactical positions. That
requires the tactics-shaped evaluator from chapter 09, and the
chapter ordering matters.

## Mental mapping

### One Tree, many walks

The state that lives across all simulations is exactly one `Tree`.
The root node is created once, before the loop. Every simulation adds
at most one new evaluation to it (unless the selected leaf is already
terminal), and backup updates the edges along the selected path.

What is *recreated* per simulation is the board walk. Selection starts
from the root position and plays moves down the chosen path; the
easiest v1 implementation clones the root board once per simulation
and mutates that clone. The clone is cheap, and it keeps the
borrow-checker story trivial: `select` takes `&Tree`, `expand` takes
`&mut Tree`, and they never fight over the same data. (The
play/undo optimization is a future measurement, not a design question
— primer §4.1.)

### Root pre-expansion: the bookkeeping invariant

Here is the subtlest single detail of the whole loop, and the one that
separates a working search from a search whose root statistics lie to
you.

The obvious loop is:

```text
for _ in 0..simulations:
    selection = select(tree, root_board, c_puct)
    value = expand(tree, selection.leaf, selection.board, evaluator)
    backup(tree, selection.path, value)
```

Run this on an empty tree. The first call to `select` sees an
unexpanded root, returns `leaf = root` with an empty path, and the
first call to `expand` expands the root into 225 edges. No edge
receives a visit, because `backup` was given an empty path. After 400
simulations, the sum of root-edge visits is 399, not 400. The loop
ate one simulation expanding the root instead of counting it.

The reference fixes this by **pre-expanding the root** before the
loop. One call to `expand` turns the root into an `Expanded` node with
edges, and the 400 simulations each select through one of those edges
and increment it. The invariant becomes:

```text
sum(root edge N) == simulations
```

This is not a cosmetic choice. The invariant is asserted in the unit
test, and it is the sanity gate that everything later rests on: visit
counts are the currency of both move selection and the training label.
If they do not sum correctly, π is wrong.

> **Excursion — why not just special-case the first simulation?**
>
> You could detect an empty path and increment every root edge, or
> add one synthetic visit to the root. Both work in a spreadsheet and
> both fail in code: they introduce a branch that exists only to
> patch a bookkeeping mistake. Pre-expansion is the clean fix because
> it makes the root structurally identical to every other expanded
> node. The loop body never needs to know it is the first iteration.

### Re-visiting Terminal nodes

In chapter 05 you made `expand` idempotent: if `select` lands on a
node that is already `Expanded`, `expand` returns without re-evaluating
it. That defensive choice pays off here.

During a simulation, `select` stops at the first node that is **not**
`Expanded`: either `Unexpanded` or `Terminal`. Once a terminal node
exists in the tree, every future simulation that walks into it will
stop there. There is nothing to expand, and the value is exact. The
loop body therefore branches on `tree.node(leaf).state()`:

* `Terminal` — use the exact terminal value.
* `Unexpanded` — call `expand`.
* `Expanded` — should not happen in normal play (selection descends
  through expanded nodes), but a fallback to `0.0` keeps the loop from
  panicking if an edge case slips through.

The terminal value must be from the side-to-move perspective at the
leaf. The engine tells us who won: `Status::Won(_)`. The player who
just moved won, so the player to move has lost, value `-1.0`. Draw is
`0.0`. This reuses the same perspective convention that expansion and
backup already share (primer §4.2, §4.4).

### The configuration defaults and their honesty

`SearchConfig` has two fields:

```rust
pub simulations: u32,
pub c_puct: f32,
```

The defaults are:

* `simulations: 400` — an `[experiment]` tuning parameter. AlphaZero
  used 800 [paper], but v1 self-play throughput matters more than
  search depth while the network is still weak (primer §2). This
  number will move as you measure.
* `c_puct: 1.5` — ELF OpenGo's published value [paper — ELF]. It is
  **not** a DeepMind number; DeepMind never published `c_puct` for
  AlphaZero (primer §3 honesty note). Treat it as a knob, probably
  ±0.5, to be tuned by arena results.

Keep that provenance in the doc comments. Future you will thank past
you when the tuning sweep starts.

### What `search` returns and why it is the whole tree

`search` does not return a move. It returns:

```rust
pub struct SearchOutcome {
    pub tree: Tree,
    pub root: NodeId,
}
```

Why the tree? Because the caller needs more than one number. The
root's outgoing edges hold:

* the chosen move (for competitive play);
* the visit-count distribution (for the self-play training target π);
* the prior and Q estimates (for diagnostics);
* the subtree under the chosen move (for future tree reuse, if you
  decide to implement it — primer §6.3).

Returning the tree makes `search` a pure factory of information. Move
selection is a thin layer on top, and that layer is chapter 08.

### The sign sentinels (conceptual now, tests in chapter 09)

With a sane prior, 50 simulations must be enough to make the winning
move the most-visited root edge in a won position, and the forced
block the most-visited root edge when the opponent threatens an
immediate win. These are the milestone-2 sign-convention sentinels
(primer §4.4, §9): if they fail, the first suspect is almost always
that backup is adding values from the wrong perspective.

The catch: a "sane prior" here means the tactics-shaped evaluator from
chapter 09. The `UniformEvaluator` you have now gives every legal move
the same prior and a flat value. Under a uniform prior, 50 simulations
will not reliably concentrate on the winning move; the search has no
information to concentrate with. So the sentinel tests live with the
tactics evaluator, not here. In this chapter you prove the loop's
bookkeeping with the uniform evaluator; in chapter 09 you prove its
strength with the tactics evaluator. That ordering is honest and
avoids writing tests that cannot pass yet.

## Low-level design

Add `src/search.rs` to the `mcts` crate and expose it in `src/lib.rs`.

### Types and signatures

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SearchConfig {
    pub simulations: u32,
    pub c_puct: f32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self { simulations: 400, c_puct: 1.5 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchOutcome {
    pub tree: Tree,
    pub root: NodeId,
}

pub fn search(
    root_board: &Board,
    evaluator: &mut dyn Evaluator,
    config: &SearchConfig,
) -> SearchOutcome
```

`search` uses:

* `Tree::new()` and `Tree::root()` for the arena;
* `expand` for the root pre-expansion and for leaf expansion inside
  the loop;
* `select` for the descent;
* `backup` for value propagation;
* `Board::status()` for terminal values.

### Loop body

```text
1. expand(root) once, before the loop
2. for _ in 0..config.simulations:
       sel = select(&tree, root_board, config.c_puct)
       value = match tree.node(sel.leaf).state() {
           Terminal => terminal_value(&sel.board),
           Unexpanded => expand(&mut tree, sel.leaf, &sel.board, evaluator),
           Expanded => 0.0, // defensive fallback
       }
       backup(&mut tree, &sel.path, value)
3. return SearchOutcome { tree, root }
```

`terminal_value(board)` is a small private helper:

```rust
fn terminal_value(board: &Board) -> f32 {
    match board.status() {
        Status::Won(_) => -1.0,
        Status::Draw | Status::Ongoing => 0.0,
    }
}
```

### Where the tests live

Unit tests live in `src/search.rs` under `#[cfg(test)]`. For this
chapter, only one test is required in the chapter's checklist:
`uniform_search_visits_sum_to_simulations`. The two
`TacticsEvaluator` sentinel tests (`immediate_win_for_side_to_move_gets_most_visits`,
`opponent_immediate_win_forces_block`) are listed in chapter 09's
checklist because they depend on the evaluator built there.

The reference implementation keeps all three tests in `search.rs`,
which is fine once the whole crate exists; the tutorial pacing simply
asks you to wait until chapter 09 before expecting them to compile.

## Solution (opt-in)

The complete reference implementation for this chapter lives in
[07-deep-dive/01-solution.md](07-deep-dive/01-solution.md). Open it
only if you have been stuck for more than twenty minutes, or after you
have finished the chapter and want to compare your shape to the
reference.

The solution file quotes `src/search.rs` verbatim, including the loop
implementation and the uniform-evaluator bookkeeping test. The two
tactics-sentinel tests are intentionally left for chapter 09's
solution because they need `TacticsEvaluator`.

## TDD checklist

Write these tests before or alongside the implementation, red-green as
usual.

1. **Uniform search visits sum to simulations.** From the empty board,
   run `search` with `UniformEvaluator::new(0.0)` and `simulations:
   50`. Sum the `n` fields of the root edges. Assert `total == 50`.
   This is the pre-expansion invariant; if it fails, root expansion
   is happening inside the loop instead of before it.

2. **(Deferred to chapter 09)** With `TacticsEvaluator`, a won
   position for the side to move must have a winning move as the
   most-visited root edge after 50 simulations.

3. **(Deferred to chapter 09)** With `TacticsEvaluator`, a position
   where the opponent threatens an immediate win must have a blocking
   move as the most-visited root edge after 50 simulations.

Run the tests with `cargo test -p mcts` from `gomoku/`. Expect only
test 1 to compile and pass in this chapter.

## Done when

From `gomoku/`:

```bash
cargo test -p mcts
cargo clippy -p mcts --all-targets -- -D warnings
cargo fmt --all
```

All green. Then commit:

```
feat(mcts): simulation loop + search entry point
```

Next: [Chapter 08 — From tree to move](08-from-tree-to-move.md)
