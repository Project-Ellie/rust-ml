# Chapter 01 — The crate

## Context

You have finished milestone 1. The `engine` crate is locked: it knows the
rules of freestyle 15×15 Gomoku, it can encode a board into 17×17 planes,
it can report legal moves and terminal status, and it does all of this
without Burn, without `rand`, and without any I/O. That isolation is not
an accident — it is the *dependency island* rule from the engine tutorial
and from chapter 12 §6, and it is what lets the same oracle run inside a
GPU worker, inside a unit test, and inside your head.

Milestone 2 is the **policy improvement operator**: the Monte Carlo Tree
Search that turns a raw network opinion `(p, v)` into a stronger move and
a training target `π`. The theory, the parallelization story, and the
list of things we deliberately do *not* build are all in
[primer.md](primer.md) §§1–2 and §7. This tutorial is the doing. Each
chapter is one hour-sized step, and every chapter follows the same five
sections you read about in the README: Context, Intention, Mental
mapping, Low-level design, Solution, TDD checklist, Done when.

In this first step you do not write a search algorithm. You write a home
for it.

## Intention

Create a new workspace member crate named `mcts` inside `gomoku/crates/`,
give it a path dependency on `engine`, add the small handful of external
dependencies the search needs, declare the module map that the next nine
chapters will fill, and write one trivial test that proves the crate
builds and that the first module (`tree`) exists.

Observable done-state: `cargo test -p mcts` passes from the `gomoku/`
directory, clippy is green, and `cargo fmt --all` makes no changes.

## Mental mapping

### Why a new crate, instead of code in `engine`?

The engine must stay a dependency island. MCTS needs `rand` for Dirichlet
noise and temperature sampling; it needs a trait seam for the network
evaluator; it will eventually need channels and batching logic in the
self-play worker. None of that belongs inside the rules oracle. So the
boundary is:

* `engine` — rules only, no Burn, no `rand`, no I/O.
* `mcts` — search only, depends on `engine`, has `rand`, has the
  `Evaluator` trait, but still no Burn and no threads in this milestone.
* `net` / `selfplay` — arrive later, behind the trait and behind
  channels (primer §6.4).

If you ever feel tempted to add a method to `engine` because the tree
finds it convenient, stop. That is a conversation, not an edit. The
engine's public surface is locked.

### Workspace mechanics

The `gomoku/` directory is already a Cargo workspace. Its `Cargo.toml`
lists members like `engine` and `cli`. Adding `mcts` means creating the
new directory and adding one line to the workspace manifest. Cargo then
resolves the whole workspace together: `mcts` can depend on `engine` via
a `path` dependency, and every crate in the workspace shares one
`Cargo.lock`.

The path dependency is what keeps the two crates in sync as you work.
When `engine` changes, `mcts` rebuilds automatically. When you run `cargo
test` from `gomoku/`, both crates are tested.

### The module map

Read this map now; you will build every one of these modules yourself.
No module is optional, and the order matters.

* `tree` — the arena: `Tree`, `Node`, `Edge`, `NodeId`, and the `Q`
  statistic. Chapter 02.
* `select` — PUCT descent from the root to a leaf. Chapter 03.
* `eval` — the `Evaluator` trait, `EvalRequest`, `EvalResult`, and the
  masked-softmax helper. Chapter 04.
* `expand` — terminal check, then asking the evaluator for priors and
  creating edges. Chapter 05.
* `backup` — sign-flipped value propagation up the selection path.
  Chapter 06.
* `search` — the top-level `search()` loop that runs 400 simulations.
  Chapter 07.
* `policy` — turning visit counts into a move: temperature sampling,
  argmax, and root Dirichlet noise. Chapter 08.
* `mock` — a deterministic `TacticsEvaluator` that gives the search
  enough tactical taste to pass the acceptance suite with no network.
  Chapter 09.

In this chapter you only declare the map. The modules themselves are
empty placeholders except for `tree`, which lands in Chapter 02.
Declaring the full map now lets `lib.rs` tell the truth about the crate's
public surface from day one.

### Edition 2024 and the lint contract

Use Rust edition 2024. It is the current edition, it is what the rest of
the workspace uses, and it causes no friction for the code you will
write.

At the top of `lib.rs` you will see three lint directives:

```rust
#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
```

The first two are discipline: every public item gets a doc comment, and
clippy's pedantic lint group is on. The `allow` is the interesting one.
PUCT is inherently numeric: visit counts are `u32`, but the formula needs
`f32` square roots and divisions. Converting `u32` to `f32` triggers
`cast_precision_loss`, and converting back — which the tree does not do,
but Rust's lint cannot know that — would trigger
`cast_possible_truncation`. These casts are not bugs here; they are the
algorithm. Visit counts in a 400-simulation search stay far below `f32`
precision limits, and every cast is deliberate.

> **Excursion — why the allow is honest, not lazy**
>
> Rust's cast lints are conservative. `as f32` from `u32` can lose
> precision once the value exceeds about 16 million. Our visit counts
> will not. The `rust-numeric-casts-semantics.md` article in the
> `../../Collateral` repo explains exactly which casts are silent
> truncations and which are exact widening; read it once so you know
> *why* an `allow` is appropriate here instead of a suppression you
> regret later. In this crate, the allow lives once at crate root and
> is justified by the algorithm, not sprinkled around individual
> expressions.

### `rand` is allowed, but seeded

Unlike the engine, the `mcts` crate uses randomness: Dirichlet noise at
the root, temperature sampling early in self-play. Every random consumer
takes a seeded RNG, so tests are reproducible. You will see that pattern
first in Chapter 08. For now you only add the dependencies.

## Low-level design

### Files you will create

```text
gomoku/crates/mcts/
├── Cargo.toml
└── src/
    └── lib.rs
```

The other module files (`tree.rs`, `select.rs`, and so on) are declared
in `lib.rs` as placeholders in this chapter; they become real files in
the chapters that follow.

### `Cargo.toml`

```toml
[package]
name = "mcts"
version = "0.1.0"
edition = "2024"

[dependencies]
engine = { path = "/Users/wgiersche/workspace/Project-Ellie/rust-ml/gomoku/crates/engine" }
rand = "0.9"
rand_distr = "0.5"

[dev-dependencies]
engine = { path = "/Users/wgiersche/workspace/Project-Ellie/rust-ml/gomoku/crates/engine", features = ["testutil"] }
rand = "0.9"
```

Notes:

* `engine` appears twice: once as a normal dependency, and once as a
  dev-dependency with the `testutil` feature. The `testutil` feature
  unlocks test helpers like ASCII board parsing. Production builds do
  not pull it in.
* `rand_distr` is for the Dirichlet distribution, used only in Chapter
  08. Adding it now keeps the dependency story complete.

### `src/lib.rs`

```rust
//! Reference implementation of single-threaded PUCT MCTS for Gomoku.
//!
//! This crate is milestone 2 of the rust-ml AlphaZero-style Gomoku
//! agent. It depends only on the `engine` crate (rules oracle) and on
//! `rand`/`rand_distr` for root Dirichlet noise and temperature
//! sampling. No Burn, no GPU, no threads: the search is deliberately
//! single-threaded so it can be tested and reasoned about in
//! isolation.
//!
//! Design document (locked): `docs/tutorials/mcts-tutorial/primer.md`
//! in the rust-ml repository. The code follows the notation and sign
//! convention of primer §§3–6 exactly.
//!
//! Public modules:
//!
//! * [`tree`] — arena tree with edge statistics.
//! * [`select`] — PUCT descent from root to leaf.
//! * [`eval`] — `Evaluator` trait, `masked_softmax`, and test stubs.
//! * [`expand`] — terminal check + network expansion of a leaf.
//! * [`backup`] — sign-flipped value propagation up the path.
//! * [`search`] — the top-level `search(root_board, evaluator, config)`.
//! * [`policy`] — visit-count extraction, temperature sampling,
//!   Dirichlet noise at the root.
//! * [`mock`] — deterministic `TacticsEvaluator` for the tactical suite.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
// PUCT and probability arithmetic intentionally mix integer counts
// with f32/f64. Visit counts stay far below f32 precision limits in
// all realistic searches, and f64->f32 truncation is explicit storage
// narrowing, not a logic bug.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

pub mod backup;
pub mod eval;
pub mod expand;
pub mod mock;
pub mod policy;
pub mod search;
pub mod select;
pub mod tree;

#[cfg(test)]
mod tests {
    #[test]
    fn tree_module_exists() {
        // If this compiles, the `tree` module is declared.
        let _ = super::tree::Tree::new();
    }
}
```

The `Tree::new()` call is the trivial proof that the crate wires
together. It will not do anything interesting yet, because `Tree` itself
is built in Chapter 02. The test exists only to force the first real
compile.

### Workspace membership

Do not forget to add `mcts` to `gomoku/Cargo.toml` under `[workspace]`
`members`. Without that line, Cargo will not see the crate when you run
`cargo test` from `gomoku/`.

## Solution (opt-in)

The complete, compiled, tested reference for this chapter lives in
[01-the-crate/01-solution.md](01-the-crate/01-solution.md). Open it only
if you have been stuck for more than twenty minutes, or after you have
finished the chapter and want to compare.

## TDD checklist

Follow the red-green rhythm from the engine tutorial.

1. **Red:** Create `gomoku/crates/mcts/Cargo.toml` and
   `gomoku/crates/mcts/src/lib.rs` as above, add `mcts` to the workspace
   manifest, and run `cargo test -p mcts` from `gomoku/`. Expect a
   compile failure because `tree`, `select`, and the other declared
   modules have no files yet.
2. **Green:** Create empty placeholder files for every declared module:
   `src/tree.rs`, `src/select.rs`, `src/eval.rs`, `src/expand.rs`,
   `src/backup.rs`, `src/search.rs`, `src/policy.rs`, `src/mock.rs`.
   Re-run `cargo test -p mcts`. The trivial `tree_module_exists` test
   should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all` from `gomoku/`. Fix any doc-comment or formatting
   issues. The crate should now be clean.

## Done when

* `cargo test -p mcts` passes from `gomoku/`.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all` makes no changes.
* Commit message: `feat(mcts): crate skeleton`.

Next: [Chapter 02 — The arena](02-the-arena.md)
