# Sub-chapter 04 — Wins and blocks

## Abstract

This sub-chapter gives the baseline seat tactical sight. The evaluator
reconstructs an absolute-color `engine::Board` from the relative planes
that the MCTS seam carries. It then asks the convolutional threat
network for both sides' win maps and adds two tiers to the policy
readout: an immediate win for the side to move, and a forced block
against the opponent's win. The value stays flat at `0.0`. The result
is the first baseline player that ends games and stops the opponent
from ending them.

## Glossary

| Term | Definition |
|------|------------|
| **`reconstruct_board`** | The new function in `baseline/src/adapter.rs` that turns `engine::Planes` into an absolute-color `engine::Board`. |
| **Side-to-move-relative planes** | The `me` and `you` channels in `engine::Planes`. Channel `me` holds the stones of the side to move; channel `you` holds the opponent's stones. |
| **Parity reconstruction** | Deciding `board.to_move()` from stone counts. Black moves first, so equal Black and White stone counts mean Black to move. Unequal counts mean White to move. |
| **Alternation-illegal position** | A position that cannot arise from legal alternating play, such as "Black to move with one more Black stone than White stones on the board". |
| **`analyze_both`** | The `patterns::net` function that returns `[black_maps, white_maps]` in one batched forward pass. Index 0 is Black; index 1 is White. |
| **`[ThreatMaps; 2]`** | The return shape of `analyze_both`. The array holds one `ThreatMaps` struct per absolute color. |
| **T1_WIN** | The `readout::T1_WIN` constant: logit weight +64 for a cell that gives the side to move an immediate five. |
| **T2_FORCED_BLOCK** | The `readout::T2_FORCED_BLOCK` constant: logit weight +32 for a cell where the opponent would complete five. |
| **T3–T4C** | Planted constants in `readout.rs` for the next sub-chapters. They are declared now but not added to the policy sum in this sub-chapter. |
| **Domination invariant** | The theorem that each tier weight exceeds the maximum possible sum of every tier below it, so a cell in a higher tier always holds the strict logit maximum. |

## Context

Sub-chapter 03 made the seat hug the fight with a gravity prior. That
prior is useful on quiet boards, but it is blind to urgent tactics. A
player that only knows gravity will let the opponent complete five and
will miss its own winning move. This sub-chapter adds the first two
rungs of the trust ladder: the engine-proven win maps.

The work happens in three places:

* `baseline/src/adapter.rs` — a new module for the planes-to-board
  round trip.
* `baseline/src/readout.rs` — the policy readout grows from
  `(gravity, legal)` to `(gravity, own, opp, legal)` and gains the
  `T1_WIN` and `T2_FORCED_BLOCK` weights.
* `baseline/src/lib.rs` — `BaselineEvaluator::evaluate` now reconstructs
  the board, runs `analyze_both`, picks the correct side's maps, and
  calls the extended `policy_logits`.

The value readout is still flat. That layer is the next sub-chapter.

## Intention

The learner enables this behavior:

1. The evaluator sees an absolute-color board from relative planes.
2. The seat always knows whose turn it is, using stone-count parity.
3. The seat plays its own immediate win before anything else.
4. The seat blocks the opponent's immediate win before it plays any
   quiet move.
5. A debug assertion catches alternation-illegal positions and stale
   legal sets during development.

## Specification as tests

The tests are ordered along the execution path: planes arrive → board
reconstructs → threat network runs → readout builds logits → the
winning cell is the argmax, and the forced block is the argmax when no
win exists.

### Board reconstruction

* `adapter::tests::reconstruct_roundtrip_parity` — play a short script,
  encode the board, reconstruct it, and assert that the side to move
  and the legal set match.
* `adapter::tests::reconstruct_rejects_alternation_illegal` — encode a
  legal position with White to move, swap `me` and `you`, and assert
  that reconstruction panics because the counts are impossible for the
  claimed side to move.

### Tier weights and domination

* `readout::tests::domination_invariant_holds` — the constants prove
  themselves: `T2_FORCED_BLOCK` exceeds the sum of T3, T4a, T4b, T4c,
  and the maximum gravity value; `T1_WIN` exceeds the sum of T2 through
  T5.

### Win-in-one and forced-block behavior through the evaluator

* `t1_win_now_is_argmax` — on a board where the side to move completes
  five in one cell, that cell holds the strict maximum policy logit.
* `t2_forced_block_is_argmax` — on a board where the opponent threatens
  five in one cell and the side to move has no win, the blocking cell
  holds the strict maximum policy logit.
* `t2_exact_logit_on_crafted_board` — the forced-block cell's logit
  equals `T2_FORCED_BLOCK` plus the local gravity value, with no hidden
  extra terms.

## Walkthrough

The evaluator answers one question: "What are the policy logits and
value for this position?" This sub-chapter changes the answer from
"gravity only" to "gravity plus wins and blocks." The walkthrough
follows `BaselineEvaluator::evaluate` from `EvalRequest` to
`EvalResult`.

### 1. The request arrives

`mcts::expand` builds an `EvalRequest` with two fields: `planes`, the
17×17 side-to-move-relative encoding, and `legal`, the list of empty
cells. The evaluator never sees the `Board` directly; it must rebuild
one.

### 2. Reconstruct the absolute-color board

`adapter::reconstruct_board(&req.planes, &req.legal)` walks the inner
15×15 region of the planes. For each cell it checks `planes.me` and
`planes.you`. Because `me` means "stones of the side to move," the raw
counts do not yet name a color. Parity decides: Black moves first, so
equal counts mean Black to move, and White having one fewer stone
than Black means White to move. Once the side to move is known, `me` and
`you` are mapped to Black and White.

This is the same approach as the private `reconstruct_board` helper in
`crates/mcts/src/mock.rs`. The adapter carries a deliberate, documented
copy because the precedent is private to the mock.

### 3. Assert the legal set

After `Board::from_position` succeeds, the adapter collects the
reconstructed board's empty cells into a `HashSet` and compares it with
the supplied `legal` set. In debug builds a mismatch panics. This catches
alternation-illegal positions and stale legal sets early. During normal
MCTS search every evaluated position is alternation-legal, so parity
reconstruction is sound.

### 4. Run the threat network for both colors

`BaselineEvaluator` owns a `ThreatNet`. It calls
`patterns::net::analyze_both(&self.net, &board, &self.device)`. The
function returns `[black_maps, white_maps]`: index 0 is Black, index 1
is White. `evaluate` then matches on `board.to_move()` to pick `(own,
opp)`.

### 5. Compute gravity

The gravity kernel from sub-chapter 02 still runs on the occupancy
plane. On quiet boards the gravity term is the only non-zero signal.
On tactical boards it becomes a small tie-breaker below the threat
tiers.

### 6. Build the policy logits

`readout::policy_logits` starts every legal cell at its gravity value
and every occupied or illegal cell at `f32::NEG_INFINITY`. It then adds:

* `T1_WIN` (+64) if `own.wins` is positive at that cell.
* `T2_FORCED_BLOCK` (+32) if `opp.wins` is positive at that cell.

The constants `T3_DOUBLE_THREAT`, `T4A_DOUBLE_THREE`, `T4B_THREE`, and
`T4C_DEFENSIVE` are declared in this sub-chapter as planted seeds, but
`policy_logits` does not use them yet.

### 7. Why the tiers are safe

The one question that forces the whole table is this: **What breaks if
gravity ever outvotes a forced block?** The answer is immediate loss.
So the tier weights are derived, not tuned. Each tier must exceed the
maximum possible sum of every tier below it:

* `T2_FORCED_BLOCK` = 32, and the largest lower stack is
  16 + 8 + 4 + 2 + 1 = 31. Gap ≥ 1.
* `T1_WIN` = 64, and the largest lower stack is
  32 + 16 + 8 + 4 + 2 + 1 = 63. Gap ≥ 1.

This gives **argmax safety**: a cell in the highest occupied tier always
holds the strict logit maximum. It also gives **softmax mass safety**: a
lone forced-block cell outweighs every cell that is not itself a forced
block or a double threat by at least e¹⁷ per cell, because the largest
lower stack without T3 is only 8 + 4 + 2 + 1 = 15. Even against 224 such
competitors the miss probability stays below 10⁻³.

When a cell carries T3 (own double threat) and another cell carries T2
(forced block), the guaranteed margin shrinks to the invariant minimum
of 1. Blocking and claiming an unblockable threat are both defensible,
so a near coin-flip between them is acceptable. The search and the
future value readout adjudicate those cases.

### 8. Return the result

`evaluate` returns `EvalResult { policy, value: 0.0 }`. The value is
still flat. `mcts::eval::masked_softmax` normalizes the logits over the
legal moves later, inside the search.

## TDD checklist

Work one test group at a time, red to green.

From `baseline/src/adapter.rs`:

1. `reconstruct_roundtrip_parity`
2. `reconstruct_rejects_alternation_illegal`

From `baseline/src/readout.rs`:

3. `domination_invariant_holds`

From `baseline/src/lib.rs`:

4. `t1_win_now_is_argmax`
5. `t2_forced_block_is_argmax`
6. `t2_exact_logit_on_crafted_board`

Keep the existing tests from sub-chapters 01 and 03 green as you
change `evaluate`.

## Verification commands

From `gomoku/`:

```text
cargo test -p baseline
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Capstone

Play at the study table:

```text
gomoku play --ai baseline --sims 4000
```

This is the first sub-chapter where the seat shows real tactics. Try
two experiments:

1. Sneak an open four. Set up four of your stones in a row with both
   ends open. On the next move the baseline will block one end. It
   recognized the forced block as the only move that matters.
2. Give the baseline four of its own stones in a row. It will play the
   fifth cell and announce the win. It recognized its own win before
   any gravity move.

Gravity still guides quiet opening moves, but wins and blocks now
override it completely.

## Done when

* The six new tests pass.
* The tests from sub-chapters 01 and 03 still pass.
* `cargo clippy --all-targets -- -D warnings` passes.
* `cargo fmt --all -- --check` makes no changes.
* The evaluator is deterministic and contains no RNG.
* The chapter is committed with:
  `feat(baseline): wins and blocks — baseline sub-chapter 04`.

## References

* [Baseline primer](../00-baseline-primer.md) — the trust ladder, tier
  derivation, and adapter pipeline.
* [Sub-chapter 03 — Gravity takes the wheel](../03-gravity-takes-the-wheel/gravity-takes-the-wheel.md)
  — the gravity kernel and policy readout v1.
* [Sub-chapter 05 — The honest value](../05-the-honest-value/the-honest-value.md)
  — the value readout that replaces the flat `0.0`.
* `gomoku/crates/baseline/src/adapter.rs` — planes-to-board
  reconstruction.
* `gomoku/crates/baseline/src/readout.rs` — policy logits and tier
  constants.
* `gomoku/crates/baseline/src/lib.rs` — the `BaselineEvaluator` and its
  tests.
* `gomoku/crates/baseline/Cargo.toml` — the `engine` `testutil`
  dev-dependency for `board_from_ascii`.
* `gomoku/crates/mcts/src/mock.rs` — the reconstruction precedent.
* `gomoku/crates/patterns/src/net.rs` — `analyze_both` returns
  `[black_maps, white_maps]`.
