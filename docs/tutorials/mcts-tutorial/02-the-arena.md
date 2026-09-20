# Chapter 02 — The arena

## Context

You now have an `mcts` crate that compiles and one trivial test that
proves the `tree` module is wired into `lib.rs`. Before a search can
select, expand, evaluate, or back up anything, it needs somewhere to
put the growing tree. That somewhere is the subject of this chapter.

The design is fixed by primer §6.3: the tree is an **arena** of nodes,
and child links are `u32` indices into one contiguous `Vec`, not
pointers, not boxes, not reference-counted cells. Every other part of
the search — selection, backup, expansion — assumes this representation.
Get it right now and the rest of the milestone is straightforward; get
it wrong and you will fight the borrow checker for nine chapters.

## Intention

Implement `tree.rs`: a `NodeId` alias for `u32`, an `Edge` struct
carrying move, prior, visit count, cumulative value, and an optional
child node id, a `NodeState` enum with `Unexpanded`, `Expanded`, and
`Terminal` variants, a `Node` struct holding state and a vector of
edges, and a `Tree` struct that owns a `Vec<Node>` and provides root
access, node lookup, and child creation.

Observable done-state: you can build a small tree by hand in a test,
attach children to edges, read the tree back through the public API,
and verify that `q()` returns `0.0` for unvisited edges and the correct
mean for visited edges.

## Mental mapping

### The borrow-checker battlefield

A game tree is naturally recursive: a node owns edges, edges own child
nodes, children own more edges. In Rust the naive shapes run into
trouble immediately.

* `Box<Node>` with direct child `Box<Node>` fields makes the tree
  immutable-by-default and painful to mutate during search.
* `Rc<RefCell<Node>>` lets you mutate, but every edge becomes a
  reference-counted, dynamically-borrow-checked pointer. That is fine
  for a toy, but the hot loop of MCTS touches edges 400 times per move.
  You do not want runtime borrow checks and atomic reference counts in
  the inner loop.
* An arena with `u32` indices replaces ownership fights with one big
  `Vec`. The `Vec` owns every node. An edge stores an index. To mutate
  a child, you borrow the `Vec` mutably and index into it. To read a
  node during selection, you borrow immutably and index into it. The
  borrow checker is happy because the owner is the `Tree`, not the
  edges.

This is the classic engine-programming answer to the recursive-ownership
problem, and it happens to be cache-friendly: the nodes and edges of a
small tree live in contiguous memory, which is exactly what the 400-
simulation hot loop wants.

### Indices as manual pointers

A `NodeId` is just a `u32`. It is not checked by the compiler for
liveness. If you hold an id and then the `Vec` reallocates or you use
the wrong id, you get a panic at runtime. That is the trade: you give
up compiler-checked liveness in exchange for no borrow fights and no
allocation per node.

The discipline that makes this safe is simple and worth stating out
loud:

1. Node ids are created only by `Tree::add_child`, which appends to the
   arena and returns the new index.
2. A `NodeId` is stored only inside an `Edge::child` field of the parent
   it was created for.
3. The arena only grows; it never reorders or deletes nodes. Therefore
   an id remains valid for the lifetime of the `Tree`.

In this chapter you will write tests that build a tree by hand. That is
not how production code creates trees — production code uses `expand()`
in Chapter 05 — but hand-building is the best way to prove the arena
invariants hold.

### Edge statistics: P, N, W, and Q

Each edge stores three raw numbers and one derived value, exactly as in
primer §3:

* `prior: f32` — `P(s, a)`, assigned once when the parent is expanded
  and never changed.
* `n: u32` — `N(s, a)`, the visit count. Incremented during backup.
* `w: f32` — `W(s, a)`, the cumulative value from the perspective of
  the player who chose the move at the parent node.
* `q()` — `Q(s, a) = W / N`, computed on demand.

The perspective on `w` is crucial and easy to get wrong. The edge
belongs to the player who *made* the move it represents. A high positive
`w` means "this move was good for the player who played it." When we
back up a network value `v` — which is always from the perspective of
the player *to move* at the leaf — we will have to flip the sign at
every level. That flip lives in Chapter 06; for now, store `w` as an
opaque `f32` and trust that its meaning will be filled in later.

> **Excursion — why `q()` returns `0.0` when `n == 0`**
>
> An unvisited edge has no experience, so its exploitation value is
> undefined. PUCT handles this by convention: `Q = 0.0` for unvisited
> edges. That is not a claim that the move is average; it is a
> deliberate choice to let the exploration term `U` dominate until
> visits accumulate.
>
> Why `0.0` and not, say, the parent `Q` or the prior? Because PUCT's
> dynamics are already balanced by `c_puct` and the prior. If you
> initialize unvisited edges to a nonzero value, you quietly bias the
> search away from the formula in primer §3. The reference
> implementation follows the formula exactly: `Q = 0.0` when `N = 0`.
>
> Notice also that `w` may be nonzero even when `n == 0` in a
> hand-built test, but `q()` still returns `0.0`. The test in this
> chapter checks that.

### Eager versus lazy child creation

An edge has an optional child: `child: Option<NodeId>`. In this design,
children are created **eagerly** during expansion: as soon as a node is
expanded, every legal move gets an edge, and `expand()` immediately
creates the child node for every edge and stores its id. That means
`select()` can traverse the tree without ever needing to mutate it.

The alternative is lazy creation: edges start with `child: None`, and
`select()` creates the child node the first time the edge is chosen.
Lazy creation saves memory if many edges are never visited, but it
forces `select()` to take a mutable tree and to handle child creation
mid-descent. The reference chose eager creation because it keeps
`select()` simple and immutable, and because Gomoku trees at 400
simulations are small enough that memory is not the bottleneck.

You do not have to agree with the choice, but you do have to know it
exists. If you later profile and find node count is a problem, lazy
creation is the first knob to turn.

### NodeState tri-state

A node is in exactly one of three states:

* `Unexpanded` — the node exists but has no edges. It has never been
  evaluated.
* `Expanded` — the node has been evaluated and has outgoing edges.
* `Terminal` — the game is over at this node. No edges, no evaluation
  needed.

`select()` uses this state to decide when to stop. It walks down while
the current node is `Expanded`; it stops when it reaches `Unexpanded` or
`Terminal`. Expansion — the subject of Chapter 05 — converts
`Unexpanded` into either `Expanded` or `Terminal`.

## Low-level design

### File

`gomoku/crates/mcts/src/tree.rs`.

The tests live in the same file, inside `#[cfg(test)] mod tests`,
following the engine tutorial convention.

### Types and signatures

```rust
pub type NodeId = u32;

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub mv: engine::Move,
    pub prior: f32,
    pub n: u32,
    pub w: f32,
    pub child: Option<NodeId>,
}

impl Edge {
    pub fn q(&self) -> f32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    Unexpanded,
    Expanded,
    Terminal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    state: NodeState,
    edges: Vec<Edge>,
}

impl Node {
    pub fn new() -> Self;
    pub fn state(&self) -> NodeState;
    pub(crate) fn state_mut(&mut self) -> &mut NodeState;
    pub fn edges(&self) -> &[Edge];
    pub fn edges_mut(&mut self) -> &mut Vec<Edge>;
}

impl Default for Node;

#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    pub fn new() -> Self;
    pub fn root(&self) -> NodeId;
    pub fn node(&self, id: NodeId) -> &Node;
    pub fn node_mut(&mut self, id: NodeId) -> &mut Node;
    pub fn add_child(&mut self, parent: NodeId, edge_index: usize) -> NodeId;
}

impl Default for Tree;
```

Notes:

* `Node::state_mut` and `Node::edges_mut` are `pub(crate)` because only
  `expand()` and `backup()` inside this crate need to mutate state and
  edges. External callers observe the tree through the immutable API.
* `Tree::node` and `Tree::node_mut` panic on out-of-bounds ids. That is
  intentional: an out-of-bounds id is always a bug in the caller, and a
  panic is the honest response.
* `Tree::add_child` appends a new node, attaches its id to the
  specified edge of the parent, and returns the new id.

### Test strategy

The tests build small trees by hand. This is the only chapter where you
will manually push edges into `edges_mut()`; later chapters use
`expand()`. The hand-built approach lets you assert exact node ids and
exact edge structure.

You need tests for:

1. A new tree has root id `0`, state `Unexpanded`, and no edges.
2. Adding children produces stable, predictable ids (`1`, `2`, `3`, ...)
   and those ids remain valid after further growth.
3. Children attach to the correct parent edge.
4. `q()` returns `0.0` when `n == 0`, regardless of `w`.
5. `q()` returns the correct mean when `n > 0`.

## Solution (opt-in)

The complete reference for this chapter — `tree.rs` including its
built-in tests — lives in
[02-deep-dive/01-solution.md](02-deep-dive/01-solution.md). Open it
only if you have been stuck for more than twenty minutes, or after the
chapter for comparison.

## TDD checklist

1. **Red:** Declare the types and methods in `tree.rs` without
   implementing them. Run `cargo test -p mcts`. Expect compile errors
   or failing tests.
2. **Green:** Implement `Tree::new`, `Tree::root`, `Tree::node`,
   `Tree::node_mut`, `Tree::add_child`, `Node::new`, the accessors, and
   `Edge::q`. Write the five tests above. Run `cargo test -p mcts`.
   All green.
3. **Refactor:** Check that `Default` implementations delegate to `new`,
   that `pub(crate)` visibility is used only where needed, and that
   every public item has a doc comment. Run clippy and fmt.

## Done when

* `cargo test -p mcts` passes from `gomoku/`.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all` makes no changes.
* Commit message: `feat(mcts): arena tree (nodes, edges, u32 indices)`.

Next: [Chapter 03 — Selection: PUCT](03-selection-puct.md)
