# Slice 1 — The workspace

Nothing to code here — the skeleton is committed already. Your job is to
understand every line of it, because you will extend these files for the
next six crates.

## Tour

**`gomoku/Cargo.toml`** — the workspace root:

```toml
[workspace]
resolver = "3"
members = ["crates/engine"]

[workspace.package]
edition = "2024"

[workspace.dependencies]
burn = { version = "=0.21.0", default-features = false }
thiserror = "2"
# ... plus serde, proptest, criterion, crossterm — see the committed file
```

- `workspace.dependencies` is the *single source of truth* for versions.
  Member crates say `thiserror.workspace = true` and inherit. When `net`
  and `train` appear, they all get the same pinned Burn — no version
  drift between crates, ever.
- `resolver = "3"` matches edition 2024 (feature unification rules; not
  something you need to feel daily).
- Burn is declared here but **no engine code may use it**. The engine is
  a dependency island by design (ch. 12, §6): it runs on 14 worker
  threads at once, must be trivially `Send + Sync`, and must compile in
  seconds.

**`crates/engine/Cargo.toml`** — note the feature:

```toml
[features]
testutil = []
```

`reference.rs` compiles only under `#[cfg(any(test, feature =
"testutil"))]` — available to your tests now, and later to `mcts`/`arena`
test suites, never to release builds.

## Rust toolbox: `cfg` gating

`#[cfg(...)]` is compile-time conditional compilation — the code behind
it does not exist for the compiler when the condition is false. Common
conditions: `test` (set by `cargo test`), feature flags (set in
Cargo.toml or `--features`). You will use it three ways in this crate:
test modules (`#[cfg(test)]`), the reference engine (above), and
debug-only invariant checks (`debug_assert!`).

## Your tasks

1. `cd gomoku && cargo check && cargo test` — confirm the green baseline.
2. Read `crates/engine/src/lib.rs` and each module stub's doc comment.
   The comments name the slice that fills each file — that is your map.
3. Open `docs/13-engine-design.md` next to this tutorial. Every slice
   refers back to it.

## Done when

You can answer without looking: why does the engine not depend on Burn?
What does `testutil` do? Where does the Burn version pin live?

Next: [Slice 2 — The reference engine](02-the-reference-engine.md)
