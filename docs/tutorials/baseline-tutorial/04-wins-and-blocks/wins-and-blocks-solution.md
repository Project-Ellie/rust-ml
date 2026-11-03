# Sub-chapter 04 — Wins and blocks: solution

> **Opt-in solution — read only after you have tried the chapter, or
> when you have been stuck for more than twenty minutes.**

This file quotes the reference build at commit `08b729d` in the
`baseline-reference` worktree. The snippets show the four files that
changed in this sub-chapter.

---

## `Cargo.toml` — enable the `testutil` feature for tests

The forced-block tests build boards from ASCII diagrams, so the
`engine` dev-dependency needs the `testutil` feature.

`gomoku/crates/baseline/Cargo.toml`, lines 14–15:

```toml
[dev-dependencies]
engine = { path = "../engine", features = ["testutil"] }
```

The `features = ["testutil"]` addition is the only change to this file
in this sub-chapter. It gives tests access to
`engine::reference::board_from_ascii`.

---

## `src/adapter.rs` — reconstruct an absolute-color board

The new module turns relative planes into an absolute-color board. The
reconstruction deliberately copies the approach used by the mock
evaluator's private helper, with an attribution comment.

`gomoku/crates/baseline/src/adapter.rs`, lines 1–21:

```rust
//! Board reconstruction from side-to-move-relative planes.
//!
//! The MCTS seam speaks [`engine::Planes`] (relative to the side to
//! move), but the threat network needs an absolute-color
//! [`engine::Board`]. This module reconstructs one from the other.

use engine::{Board, Color, Move, Planes};
use std::collections::HashSet;

/// Reconstruct an absolute-color board from side-to-move-relative planes.
///
/// This deliberately copies the approach of the private
/// `reconstruct_board` helper in `crates/mcts/src/mock.rs` (the
/// `TacticsEvaluator`): stone-count parity decides the side to move,
/// then the `me`/`you` planes are mapped to absolute colors.
///
/// # Panics
///
/// Panics in debug builds if `legal` does not match the reconstructed
/// board's empty moves. This catches alternation-illegal positions
/// and stale legal sets early.
pub fn reconstruct_board(planes: &Planes, legal: &[Move]) -> Board {
```

The module doc states the seam mismatch: `Planes` is relative to the
side to move, but `ThreatNet` needs absolute colors. The attribution
comment names the precedent so future readers know the duplication is
intentional.

`gomoku/crates/baseline/src/adapter.rs`, lines 22–48:

```rust
    let mut black = Vec::new();
    let mut white = Vec::new();

    for r in 0..15u8 {
        for c in 0..15u8 {
            let dst = (r as usize + 1) * engine::EXT + (c as usize + 1);
            let mv = Move::new(r, c).expect("in-board indices");
            if planes.me[dst] == 1 {
                black.push(mv);
            } else if planes.you[dst] == 1 {
                white.push(mv);
            }
        }
    }

    let to_move = if white.len() == black.len() {
        Color::Black
    } else {
        Color::White
    };

    let (black, white) = if to_move == Color::White {
        (white, black)
    } else {
        (black, white)
    };

    let board = Board::from_position(&black, &white, to_move)
        .expect("Encoded planes must form a valid position");
```

The loop scans the inner 15×15 board. Because `planes.me` holds the
stones of the side to move, the first pass treats those stones as
Black. Parity then corrects the assignment: if White is to move, the
labels are swapped. `Board::from_position` enforces alternation-legality
and panics on impossible counts.

`gomoku/crates/baseline/src/adapter.rs`, lines 53–58:

```rust
    let supplied_legal: HashSet<_> = legal.iter().copied().collect();
    let computed_legal: HashSet<_> = board.empty_moves().collect();
    debug_assert_eq!(
        supplied_legal, computed_legal,
        "Supplied legal moves must match the reconstructed board's legal moves."
    );

    board
```

The debug assertion compares the legal set supplied by the search with
the empty cells of the reconstructed board. This is a development guard,
not a runtime recovery path. In release builds the check is compiled
away.

### Adapter tests

`gomoku/crates/baseline/src/adapter.rs`, lines 72–83:

```rust
    #[test]
    fn reconstruct_roundtrip_parity() {
        let mut board = Board::new();
        let script = [(7, 7), (0, 0), (6, 6), (14, 14), (8, 8)];
        for &(r, c) in &script {
            board.play(Move::new(r, c).unwrap()).unwrap();
        }
        let (planes, legal) = request_from_board(&board);
        let reconstructed = reconstruct_board(&planes, &legal);
        assert_eq!(reconstructed.to_move(), board.to_move());
        assert_eq!(reconstructed.empty_moves().collect::<Vec<_>>(), legal);
    }
```

This test plays five alternating moves, encodes the board, and asserts
that reconstruction recovers both the side to move and the legal set.

`gomoku/crates/baseline/src/adapter.rs`, lines 85–98:

```rust
    #[test]
    #[should_panic(expected = "Encoded planes must form a valid position")]
    fn reconstruct_rejects_alternation_illegal() {
        // Encode a position with one Black stone, White to move (legal).
        let mut board = Board::new();
        board.play(Move::new(7, 7).unwrap()).unwrap();
        assert_eq!(board.to_move(), Color::White);
        let (mut planes, legal) = request_from_board(&board);

        // Swap me/you to pretend it is Black to move with a Black
        // stone already placed — an alternation-illegal position.
        std::mem::swap(&mut planes.me, &mut planes.you);
        let _ = reconstruct_board(&planes, &legal);
    }
```

Swapping `me` and `you` makes the parity decoder think Black is to move
with one more Black stone than White stones on the board. That is
impossible in legal play, so `Board::from_position` panics.

---

## `src/readout.rs` — add T1 and T2 to the policy readout

`gomoku/crates/baseline/src/readout.rs`, lines 6–21:

```rust
/// Logit weight for an immediate win for the side to move.
pub const T1_WIN: f32 = 64.0;
/// Logit weight for a forced block (opponent immediate win).
pub const T2_FORCED_BLOCK: f32 = 32.0;
/// Logit weight for an own double threat.
pub const T3_DOUBLE_THREAT: f32 = 16.0;
/// Logit weight for an own double-three fork.
pub const T4A_DOUBLE_THREE: f32 = 8.0;
/// Logit weight for any own three-maker.
pub const T4B_THREE: f32 = 4.0;
/// Logit weight for any opponent three-maker or double-three.
pub const T4C_DEFENSIVE: f32 = 2.0;

/// Maximum possible gravity contribution after normalization.
#[allow(dead_code)]
const T5_MAX: f32 = 1.0;
```

All seven constants are declared now, but only `T1_WIN` and
`T2_FORCED_BLOCK` are used in this sub-chapter. T3–T4C are planted for
the next sub-chapters. `T5_MAX` is the upper bound of the gravity map
after normalization.

`gomoku/crates/baseline/src/readout.rs`, lines 23–48:

```rust
/// Build a 225-vector of policy logits from threat maps and gravity.
///
/// Tiers are applied by binary membership; occupied or illegal cells
/// receive `f32::NEG_INFINITY`.
#[must_use]
pub fn policy_logits(
    gravity: &[[f32; 15]; 15],
    own: &ThreatMaps,
    opp: &ThreatMaps,
    legal: &[Move],
) -> [f32; 225] {
    let mut logits = [f32::NEG_INFINITY; 225];
    for mv in legal {
        let r = mv.row() as usize;
        let c = mv.col() as usize;
        let mut logit = gravity[r][c];
        if own.wins[r][c] > 0.0 {
            logit += T1_WIN;
        }
        if opp.wins[r][c] > 0.0 {
            logit += T2_FORCED_BLOCK;
        }
        logits[mv.index()] = logit;
    }
    logits
}
```

The function starts every legal cell at its gravity value. It then adds
`T1_WIN` for own wins and `T2_FORCED_BLOCK` for opponent wins. Every
other cell keeps `f32::NEG_INFINITY`, matching the masked style used by
the mock evaluator. Only the `wins` maps are consulted in this
sub-chapter.

`gomoku/crates/baseline/src/readout.rs`, lines 90–106:

```rust
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn domination_invariant_holds() {
        assert!(
            T2_FORCED_BLOCK
                > T3_DOUBLE_THREAT + T4A_DOUBLE_THREE + T4B_THREE + T4C_DEFENSIVE + T5_MAX
        );
        assert!(
            T1_WIN
                > T2_FORCED_BLOCK
                    + T3_DOUBLE_THREAT
                    + T4A_DOUBLE_THREE
                    + T4B_THREE
                    + T4C_DEFENSIVE
                    + T5_MAX
        );
    }
```

This test proves the two load-bearing inequalities. `T2_FORCED_BLOCK`
exceeds the largest possible lower stack, and `T1_WIN` exceeds
everything below it. Because all weights are exact f32 integers, the
assertions use `==` logic through the strict inequality.

---

## `src/lib.rs` — assemble the evaluator

`gomoku/crates/baseline/src/lib.rs`, lines 9–11:

```rust
pub mod adapter;
pub mod gravity;
pub mod readout;
```

The new `adapter` module joins `gravity` and `readout`.

`gomoku/crates/baseline/src/lib.rs`, lines 20–28:

```rust
/// The baseline evaluator.
///
/// v2 combines the gravity kernel with immediate wins and forced
/// blocks from the threat network. The value stays flat at `0.0`.
pub struct BaselineEvaluator {
    device: Device,
    kernel: GravityKernel,
    net: ThreatNet,
}
```

The struct now owns three things: the flex device, the gravity kernel,
and the threat network. The module-level doc comment above still says
"Burn-free (v0) policy and a flat value. The board planes are ignored."
That text is stale; the code has used Burn and the planes since
sub-chapter 02. The struct doc comment is accurate for v2.

`gomoku/crates/baseline/src/lib.rs`, lines 44–60:

```rust
impl Evaluator for BaselineEvaluator {
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
        let _ = &self.device;
        let board = adapter::reconstruct_board(&req.planes, &req.legal);
        let maps = patterns::net::analyze_both(&self.net, &board, &self.device);
        let (own, opp) = match board.to_move() {
            Color::Black => (&maps[0], &maps[1]),
            Color::White => (&maps[1], &maps[0]),
        };
        let gravity = self.kernel.compute(&req.planes);
        let policy = readout::policy_logits(&gravity, own, opp, &req.legal);
        EvalResult {
            policy,
            value: 0.0f32,
        }
    }
}
```

The execution order is: reconstruct the board, run `analyze_both`, pick
own and opponent maps by `to_move()`, compute gravity, build logits.
`analyze_both` returns `[black_maps, white_maps]`, so Black to move uses
index 0 as own and White to move uses index 1 as own. The value remains
flat at `0.0`.

### Evaluator-level tests

`gomoku/crates/baseline/src/lib.rs`, lines 179–212:

```rust
    #[test]
    fn t1_win_now_is_argmax() {
        let mut board = Board::new();
        let script = [
            (7, 3),
            (0, 0),
            (7, 4),
            (0, 2),
            (7, 5),
            (0, 4),
            (7, 6),
            (0, 6),
        ];
        for &(r, c) in &script {
            board.play(Move::new(r, c).unwrap()).unwrap();
        }
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req);

        let wins = engine::immediate_wins(&board, board.to_move());
        let argmax = board
            .empty_moves()
            .max_by(|a, b| {
                res.policy[a.index()]
                    .partial_cmp(&res.policy[b.index()])
                    .unwrap()
            })
            .unwrap();
        assert!(
            wins.contains(argmax),
            "argmax {argmax:?} must be a winning move"
        );
    }
```

This test builds a vertical four on column 7 for the side to move. The
only winning move is `(7, 7)`. The assertion checks that the argmax over
all empty cells is an immediate win.

`gomoku/crates/baseline/src/lib.rs`, lines 214–250:

```rust
    #[test]
    fn t2_forced_block_is_argmax() {
        use engine::reference::board_from_ascii;
        let board = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . O O O O . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            X . X . X . X . . . . . . . .
            ",
        );
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req.clone());

        let argmax = board
            .empty_moves()
            .max_by(|a, b| {
                res.policy[a.index()]
                    .partial_cmp(&res.policy[b.index()])
                    .unwrap()
            })
            .unwrap();
        let blocks = engine::forced_blocks(&board);
        assert!(blocks.contains(argmax), "argmax {argmax:?} must be a block");
    }
```

White has four stones in a row on row 9 with empty cells on both ends.
Black has no win. The side to move must block one end of White's four.
The test asserts that the argmax is a forced block.

`gomoku/crates/baseline/src/lib.rs`, lines 252–283:

```rust
    #[test]
    fn t2_exact_logit_on_crafted_board() {
        use engine::reference::board_from_ascii;
        let board = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . O O O O . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            X . X . X . X . . . . . . . .
            ",
        );
        let req = request_from_board(&board);
        let mut ev = BaselineEvaluator::new(Device::flex());
        let res = ev.evaluate(req.clone());

        let gravity = gravity::gravity_map(&req.planes);
        let blocks = engine::forced_blocks(&board);
        let mv = blocks.iter().next().unwrap();
        let expected = readout::T2_FORCED_BLOCK + gravity[mv.row() as usize][mv.col() as usize];
        assert_eq!(res.policy[mv.index()], expected);
    }
```

This test computes the expected logit for a forced-block cell by hand:
`T2_FORCED_BLOCK` plus the gravity value at that cell. The assertion
uses `==`, which is valid because both weights are exact.

---

## Verification

From `gomoku/` in the reference worktree:

```text
cargo test -p baseline
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

All green at commit `08b729d`.
