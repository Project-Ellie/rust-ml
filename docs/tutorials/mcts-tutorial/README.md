# MCTS Tutorial — milestone 2, built by you, baby steps

The build-it-yourself tutorial for the `mcts` crate. The theory,
the design decisions, and the integration story are **not** here —
they are in [primer.md](primer.md), which you read first (or at least
keep open; every chapter points at the primer sections it implements).
This folder is the *doing*: ten small steps from an empty crate to a
proven Monte Carlo Tree Search, each one sized so that you always know
exactly what the next hour holds.

Milestone context: ch. 12 §13, milestone 2 row. The acceptance bar you
are walking toward: win-in-1 solved at 50 simulations, forced blocks
taken, win-in-3 found, and 1,000 random-evaluator games without an
illegal move, a hang, or a panic — with **no network, no GPU, no
threads** (primer §7).

## How each step works (the five-part rhythm)

Every chapter follows the same five sections, in this order. The
structure is the pedagogy — it keeps the *why* separate from the
*what* separate from the *where*, so that by the time you write code
you have already written it in your head twice.

1. **Context.** Where we are: what exists, what just landed, what
   this step connects to. Short. Re-read it when you resume a session.
2. **Intention.** The particular goal of this step and what "done"
   looks like — the observable behavior, not the code. If you can't
   state the intention back in one sentence, re-read before touching
   the keyboard.
3. **Mental mapping.** The conceptual plan, still without code: what
   needs to happen, in what order, and *why that way*. This is where
   the chapter foreshadows the difficulties — the Rust-specifics
   (ownership shapes, borrow-checker traps, numeric pitfalls) and the
   algorithm-specifics (sign conventions, normalization order) — while
   they are still cheap to think about. Excursions included where they
   earn their keep.
4. **Low-level design.** The physical solution design: which files,
   which types, which function signatures, where the tests live. After
   this section you could implement the step without another thought
   about *structure* — all that remains is the writing.
5. **Solution.** A complete, compiled, tested reference implementation
   — in the chapter's companion folder (same root name), clearly marked. **Opt
   in.** The intended use: try the step yourself first; open the
   solution when stuck >20 minutes, or afterwards to compare. Every
   solution is extracted from a reference crate that compiles and
   passes the chapter's tests — it is not illustrative pseudocode.

Then the **TDD checklist** (the tests you write *before* or alongside
the implementation — the chapters assume the engine tutorial's
red-green rhythm) and **Done when** (gates + commit message).

## Rules of the road

- **Gates before every commit**, from `gomoku/`:
  `cargo test` (workspace) · `cargo clippy --all-targets -- -D warnings` ·
  `cargo fmt --all`. All green, no exceptions.
- **The engine stays untouched.** `mcts` depends on `engine`, never
  the reverse. If you feel the engine needs a new method for the
  tree's sake, that is a conversation, not an edit (the engine's
  public surface is locked, ch. 13). Legality, status, encoding:
  the tree asks the engine, always (primer §6.1).
- **No Burn, no GPU, no threads in this crate** (primer §6.4). The
  network arrives in milestone 3 behind the `Evaluator` trait; the
  threads arrive in milestone 5 behind channels. `mcts` is pure,
  single-threaded Rust, and that is why it is so testable.
- **`rand` is allowed here** (unlike the engine, which must stay
  deterministic) — but every random consumer takes a seeded RNG, so
  tests are reproducible (chapter 08).
- **Commit after every step**, with the message the chapter names.
  Small commits are the tutorial's undo button.
- Stuck >20 minutes → coaching conversation (or the solution folder),
  per the engine tutorial's convention. Struggle is the point;
  misery is not.

## The map

| # | Chapter | What lands | Primer § |
|---|---------|-----------|----------|
| 01 | [The crate](01-the-crate.md) | `mcts` workspace member, module map, first test | §7 |
| 02 | [The arena](02-the-arena.md) | `Tree`/`Node`/`Edge`, `u32` child indices, hand-built trees | §6.3 |
| 03 | [Selection: PUCT](03-selection-puct.md) | the formula, the descent, the path | §3, §4.1 |
| 04 | [The evaluation seam](04-the-evaluation-seam.md) | `Evaluator` trait, request/result, mask-then-normalize | §4.3, §6.2 |
| 05 | [Expansion](05-expansion.md) | terminal-first, edges with priors | §4.2–4.3 |
| 06 | [Backup: the sign convention](06-backup.md) | value ascent, hand-computed `Q` | §4.4 |
| 07 | [The simulation loop](07-the-simulation-loop.md) | `search()`, root statistics, win-in-1 | §2, §4.5 |
| 08 | [From tree to move](08-from-tree-to-move.md) | π from counts, temperature, Dirichlet noise | §5 |
| 09 | [The tactics-shaped evaluator](09-the-tactics-evaluator.md) | the mock that makes search provable | §6.2, §7 |
| 10 | [Acceptance](10-acceptance.md) | tactical suite + 1,000-game sanity, surface polish | §7, §9 |

The order is deliberate: structure before search, search before
policy, policy before proof. Through chapter 06 you are building a
machine; 07 turns it on; 08–09 give it taste; 10 proves it.

Two things the tutorial deliberately does **not** build (primer §8,
with the shelf labeled): transposition merging, and any form of tree
parallelism. Resignation, gating, and Elo belong to later milestones.

## Status

- 2026-09-19 — tutorial written, reference implementation verified
  (compiles, all chapter tests green). No chapters started yet.

Keep this section current as chapters land.
