# Sub-chapter 04 — Wins and blocks: implementation plan

This document turns the walkthrough in `wins-and-blocks.md` into a
construction order. It names the files, structs, enums, functions, and
tests. It states behavioral contracts in prose. It does not contain
bodies.

Read `wins-and-blocks.md` first. Work one step at a time. Write tests
before code. Open `wins-and-blocks-solution.md` only if you are stuck
for more than twenty minutes, or after the sub-chapter is complete.

---

## Step 0 — Add the `testutil` dev-dependency

In `gomoku/crates/baseline/Cargo.toml`, add `features = ["testutil"]`
to the existing `engine` dev-dependency entry. This gives the tests
access to `engine::reference::board_from_ascii`.

Gate: `cargo check -p baseline` still passes.

---

## Step 1 — Create `baseline/src/adapter.rs`

### 1.1 Module documentation

Write a module-level doc comment that explains the purpose: reconstruct
an absolute-color `engine::Board` from the side-to-move-relative planes
carried by the MCTS seam.

### 1.2 Imports

Bring in:

* `engine::{Board, Color, Move, Planes}`
* `std::collections::HashSet`

### 1.3 `reconstruct_board`

Signature:

```rust
pub fn reconstruct_board(planes: &Planes, legal: &[Move]) -> Board
```

Behavior:

* Walk the inner 15×15 region of `planes`.
* For each cell, if `planes.me[dst] == 1`, push the move into the
  side-to-move stone list; else if `planes.you[dst] == 1`, push it into
  the opponent stone list.
* Decide the side to move by parity: equal counts mean `Color::Black`,
  otherwise `Color::White`.
* If `to_move` is White, swap the two lists so that the first list is
  always Black and the second is always White.
* Build the board with `Board::from_position`. Expect it to succeed for
  any encoded position that came from legal play.
* Collect the supplied `legal` moves into a `HashSet` and the board's
  `empty_moves()` into another `HashSet`. Use `debug_assert_eq!` to
  require them to match.
* Return the board.

Document that this function deliberately copies the approach of the
private `reconstruct_board` helper in `crates/mcts/src/mock.rs`.

### 1.4 Tests in `adapter.rs`

Add a `#[cfg(test)] mod tests` block.

Helper:

```rust
fn request_from_board(board: &Board) -> (Planes, Vec<Move>)
```

Behavior: encode the board and collect its empty moves.

Test `reconstruct_roundtrip_parity`:

* Start from `Board::new()`.
* Play a short script of five moves.
* Build the request, reconstruct the board, and assert that
  `to_move()` and the empty-move list match the original.

Test `reconstruct_rejects_alternation_illegal`:

* Start from `Board::new()`, play one Black stone so White is to move.
* Encode the board to get a request.
* Swap `planes.me` and `planes.you`.
* Call `reconstruct_board` and assert that it panics. Use
  `#[should_panic(expected = "Encoded planes must form a valid position")]`.

### 1.5 Gate

Run `cargo test -p baseline reconstruct`. Expect two failures until the
next steps are done, or expect them to pass if you run the full suite
only after wiring.

---

## Step 2 — Extend `baseline/src/readout.rs`

### 2.1 Add tier constants

Add public documented constants:

* `T1_WIN: f32 = 64.0`
* `T2_FORCED_BLOCK: f32 = 32.0`
* `T3_DOUBLE_THREAT: f32 = 16.0`
* `T4A_DOUBLE_THREE: f32 = 8.0`
* `T4B_THREE: f32 = 4.0`
* `T4C_DEFENSIVE: f32 = 2.0`

Add a private constant:

* `T5_MAX: f32 = 1.0` (mark `#[allow(dead_code)]` because gravity is
  passed in, not looked up by a constant).

Document that T3–T4C are planted for the next sub-chapters.

### 2.2 Extend `policy_logits`

Change the signature to:

```rust
pub fn policy_logits(
    gravity: &[[f32; 15]; 15],
    own: &ThreatMaps,
    opp: &ThreatMaps,
    legal: &[Move],
) -> [f32; 225]
```

Behavior:

* Initialize the output vector with `f32::NEG_INFINITY`.
* For each legal move, start the logit at `gravity[r][c]`.
* If `own.wins[r][c] > 0.0`, add `T1_WIN`.
* If `opp.wins[r][c] > 0.0`, add `T2_FORCED_BLOCK`.
* Store the result at `logits[mv.index()]`.

Do not use T3–T4C yet.

### 2.3 Add the domination test

Test `domination_invariant_holds`:

* Assert `T2_FORCED_BLOCK > T3_DOUBLE_THREAT + T4A_DOUBLE_THREE +
  T4B_THREE + T4C_DEFENSIVE + T5_MAX`.
* Assert `T1_WIN > T2_FORCED_BLOCK + T3_DOUBLE_THREAT +
  T4A_DOUBLE_THREE + T4B_THREE + T4C_DEFENSIVE + T5_MAX`.

Use `#[allow(clippy::assertions_on_constants)]`.

### 2.4 Gate

Run `cargo test -p baseline readout`. The new domination test should
pass. Existing readout tests may need updates if their signatures
changed.

---

## Step 3 — Wire everything in `baseline/src/lib.rs`

### 3.1 Update imports

Ensure `baseline/src/lib.rs` imports or declares:

* `pub mod adapter;`
* `pub mod gravity;`
* `pub mod readout;`
* `use engine::Color;`
* `use gravity::GravityKernel;`
* `use mcts::{EvalRequest, EvalResult, Evaluator};`
* `use patterns::net::ThreatNet;`

### 3.2 Update `BaselineEvaluator` documentation

Note that the evaluator is now v2: gravity plus immediate wins and
forced blocks. Keep the flat value documented.

### 3.3 Update `BaselineEvaluator::evaluate`

Behavior:

* Call `adapter::reconstruct_board(&req.planes, &req.legal)`.
* Call `patterns::net::analyze_both(&self.net, &board, &self.device)`.
* Match on `board.to_move()`:
  * `Color::Black` ⇒ `own = &maps[0]`, `opp = &maps[1]`.
  * `Color::White` ⇒ `own = &maps[1]`, `opp = &maps[0]`.
* Compute gravity with `self.kernel.compute(&req.planes)`.
* Build policy with `readout::policy_logits(&gravity, own, opp, &req.legal)`.
* Return `EvalResult { policy, value: 0.0f32 }`.

### 3.4 Add evaluator-level tests

Helper (if not already present):

```rust
fn request_from_board(board: &Board) -> EvalRequest
```

Behavior: build `EvalRequest { planes: engine::encode(board), legal:
board.empty_moves().collect() }`.

Test `t1_win_now_is_argmax`:

* Build a board where the side to move has a vertical four on column 7
  and the fifth stone wins.
* Evaluate and find the argmax over empty moves.
* Assert that the argmax is one of `engine::immediate_wins(&board,
  board.to_move())`.

Test `t2_forced_block_is_argmax`:

* Use `engine::reference::board_from_ascii` to build a board where the
  opponent has four stones in a row with one empty cell on each end, and
  the side to move has no win.
* Evaluate and find the argmax over empty moves.
* Assert that the argmax is one of `engine::forced_blocks(&board)`.

Test `t2_exact_logit_on_crafted_board`:

* Use the same forced-block board.
* Evaluate, compute the gravity map with `gravity::gravity_map(&req.planes)`,
  pick one forced-block move, and assert that its logit equals
  `readout::T2_FORCED_BLOCK + gravity[row][col]`.

### 3.5 Gate

Run `cargo test -p baseline t1` and `cargo test -p baseline t2`. Expect
failures until the implementation is correct.

---

## Step 4 — Project gates

From `gomoku/`:

```text
cargo test -p baseline
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

---

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `Cargo.toml` | `engine` dev-dependency with `testutil` | tests can use `board_from_ascii` |
| `src/adapter.rs` | `reconstruct_board` | parity reconstruction + legal-set debug assert |
| `src/adapter.rs` | attribution comment | names `mcts/src/mock.rs` precedent |
| `src/adapter.rs` | `reconstruct_roundtrip_parity` | round-trip parity test passes |
| `src/adapter.rs` | `reconstruct_rejects_alternation_illegal` | swap panics |
| `src/readout.rs` | T1–T4C constants | declared and documented |
| `src/readout.rs` | `policy_logits` | takes gravity, own, opp, legal |
| `src/readout.rs` | T1/T2 applied | own wins +64, opp wins +32 |
| `src/readout.rs` | `domination_invariant_holds` | both inequalities pass |
| `src/lib.rs` | `evaluate` | reconstruct → analyze_both → pick maps → gravity → logits |
| `src/lib.rs` | map indexing | Black ⇒ `[0]`, White ⇒ `[1]` for own |
| `src/lib.rs` | `t1_win_now_is_argmax` | winning cell is argmax |
| `src/lib.rs` | `t2_forced_block_is_argmax` | blocking cell is argmax |
| `src/lib.rs` | `t2_exact_logit_on_crafted_board` | exact T2 + gravity value |
