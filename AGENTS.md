# AGENTS.md — rust-ml

**Session start: read [docs/WARM-UP.md](docs/WARM-UP.md) first.** It is
the maintained, compacted project context — what this repo is, the
architecture, the vocabulary, and the current implementation status.
Everything below is the short version plus the rules.

## Project in one paragraph

This repo resurrects Wolfie's TensorFlow
[DeepGomoku](https://github.com/Project-Ellie/DeepGomoku) as an
AlphaZero-style Gomoku agent in Rust with Burn 0.21.0. The root package
is a finished MNIST curriculum (`docs/` chapters 1–8, `examples/`); the
`gomoku/` workspace is the real project (chapters 9–13). Freestyle
Gomoku 15×15, overlines win, Swap2 opening, "alpha-epsilon" approach
(hand-woven tactics head start). System design: `docs/12-gomoku-architecture.md`;
engine design: `docs/13-engine-design.md`.

## Hard rules

1. **Burn is pinned to `=0.21.0`.** Never use `main`-branch or
   0.22-pre APIs (they remove the backend type parameter). Docs link
   the `v0.21.0` tag — check the pinned source, not blog posts.
2. **`engine` is a dependency island** — no Burn, no I/O, no `rand`.
   `cargo check -p engine` must pass with Burn nowhere in sight.
3. **The engine tutorial is Wolfie's learn-by-doing material.** Slices
   give contracts, Wolfie writes implementations. Coach, don't hand
   over solutions (reference solutions in `NN-deep-dive/` are opt-in).
4. **Gates before any commit:** `cargo test`,
   `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all` —
   all green.
5. **Respect documented decisions.** Chapters 12/13 contain locked
   decisions with reasoning (absolute colors, stride-16 + padding
   invariant, store-games replay buffer, phased v1 GPU scheduling, no
   tokio, no transposition merging in MCTS). Don't relitigate them
   silently; if something seems wrong, flag it explicitly.
6. **Full-precision recorders only** for model records
   (`CompactRecorder` is f16 — see `docs/11-pitfalls.md`).

## Layout

- `docs/` — the wiki: chapters 1–13, `tutorials/`, `WARM-UP.md`
- `src/`, `examples/` — MNIST curriculum (root package, done)
- `gomoku/crates/engine` — rules engine (milestone 1, in progress)
- `gomoku/crates/cli` — terminal UI on either board (side quest, done)

## Status snapshot

Milestone 1 (engine): slices 1–3 done (reference oracle, bitboard
Board, differential harness vs naive oracle) + CLI side quest complete.
Slices 4–9 pending (fast win detection, Zobrist, symmetry/encode,
tactics, Swap2, benches). Details and verification commands:
[docs/WARM-UP.md](docs/WARM-UP.md#current-status-verified-2026-09-16----update-this-section-as-work-lands).
Keep that section current as slices land.
