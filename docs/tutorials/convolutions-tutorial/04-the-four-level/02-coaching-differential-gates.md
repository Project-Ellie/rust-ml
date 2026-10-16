# Coaching note — building the differential gates

> Opt-in companion to [Chapter 04 — The four level](../04-the-four-level.md).
> The chapter deliberately withholds the implementation of
> `tests/common/mod.rs` and `tests/differential.rs` — writing them is
> part of the chapter's work. This note is the middle ground between
> the chapter's contract and a quoted solution: shapes, signatures,
> and the decisions you will face, without implementation code.

The chapter already fixes everything that is a *decision*:

- File layout: `tests/common/mod.rs` (seeded random-board generator)
  and `tests/differential.rs` (the engine gate).
- Test names: `wins_default`, `double_threats_default` (default
  suite), `wins_full`, `double_threats_full` (`#[ignore]`d proof
  suite).
- Seed protocol: strict prefixes — `0..100` default, `0..1000` full
  (the full range re-covers the default one), both colors.
- Runtime budget: ~1 minute for the default suite, a few minutes in
  `--release` for the proof.

What follows is coaching for the part that is *yours*.

## `tests/common/mod.rs` — one job: determinism

```rust
//! Shared helpers for the differential gates.

/// Build a deterministic mid-game board from a seed.
/// Same seed → same board, always.
pub fn seeded_board(seed: u64) -> Board { /* ... */ }
```

**Approach:**

- Use `StdRng::seed_from_u64(seed)`. That is the workspace convention
  (`train::playout` and `mcts::policy` both do exactly this), and
  `rand` is already in patterns' `[dev-dependencies]`. No hand-rolled
  PRNG needed.
- Draw a **ply count** from the rng, then play that many uniformly
  random legal moves: `board.empty_moves().collect::<Vec<_>>()`,
  index with `rng.random_range(..)`. Mirror the structure of
  `train::playout::random_game` — it is the same idiom.
- Stop early if `board.status() != Status::Ongoing`.

**Decisions you will have to make:**

1. *Ply range.* Near-empty boards make the gate vacuous (no threats
   anywhere → trivial agreement). Mid-game lengths give tactical
   density. Draw from a range rather than fixing one length, so 100
   seeds cover varied board phases.
2. *Vacuousness guard.* Consider a sanity assertion across the whole
   seed range — e.g. the engine oracle was non-empty at least once —
   so the gate cannot pass by testing 100 boring boards. A judgment
   call, but cheap insurance.
3. *Terminal boards.* If a random line accidentally completes five,
   the board is terminal and the maps' semantics there are murky.
   Capping plies low enough makes this rare; whether to skip or
   accept such boards is a conscious choice you should make, not
   stumble into.

**One unit test to lock the contract:** `same_seed_same_board` —
compare `board.moves()` (or `zobrist()`) of two boards built from the
same seed.

## `tests/differential.rs` — the gate

```rust
mod common;

fn assert_wins_match(board: &Board, side: Color, net: &ThreatNet, device: &Device) { /* ... */ }
fn assert_double_threats_match(board: &Board, side: Color, net: &ThreatNet, device: &Device) { /* ... */ }

#[test]
fn wins_default() { /* seeds 0..100, both colors */ }

#[test]
fn double_threats_default() { /* seeds 0..100, both colors */ }

#[test]
#[ignore = "full 1000-board proof"]
fn wins_full() { /* seeds 0..1000 */ }

#[test]
#[ignore = "full 1000-board proof"]
fn double_threats_full() { /* seeds 0..1000 */ }
```

**Approach per seed:** build the board via `common::seeded_board`,
then for each color in `[Color::Black, Color::White]` compare the
net's map against the engine oracle cell-by-cell:

- *Oracle side:* `engine::immediate_wins(b, side)` /
  `engine::double_threats(b, side)` return a `MoveSet` — it has
  `contains(mv)` and `iter()`, so probe all 225 cells or fold it into
  a `[[bool; 15]; 15]`.
- *Net side:* threshold the `[[f32; 15]; 15]` at 0.5 into a bool
  grid. Write a tiny
  `fn bitgrid(map: &[[f32; 15]; 15]) -> [[bool; 15]; 15]` helper —
  never compare floats for equality.
- *Setup:* copy the `ThreatNet` construction + `Device::flex()` idiom
  from the existing `net::tests` — one-time init, reused across all
  seeds in the test.

**The one thing that will save you later:** make failures debuggable.
When a mismatch fires, the assert message should name the **seed, the
color, and the differing cells** (and ideally print the board).
"assertion failed somewhere in seeds 0..100" turns a 5-minute fix
into an archaeology session. A mismatch on seed `s` should be
reproducible by running exactly that seed in a scratch test.

**Protocol fidelity:**

- Strict prefix: `wins_full` runs `0..1000`, *not* `100..1000`.
- The default tests take no `#[ignore]`; the full ones do.

## Suggested TDD order

1. `common/mod.rs` + determinism test.
2. The four test shells with `todo!()` in the assert helpers (red).
3. Helpers implemented — green, if the glue from the chapter's
   earlier steps is correct (the three crafted differential tests
   passing is a good omen).
4. `cargo test -p patterns --release -- --ignored` — the chapter's
   acceptance ritual.
5. `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`.
6. Commit: `feat(patterns): four-level maps + engine differential gate`.
