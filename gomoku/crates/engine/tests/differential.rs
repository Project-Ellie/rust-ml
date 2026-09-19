//! Differential tests: the fast bitboard engine must agree with the
//! naive reference oracle on every ply of thousands of random games.
//!
//! This is an INTEGRATION test: it links against `engine` as an
//! external crate, which means the library is built WITHOUT `cfg(test)`
//! — so the reference engine is only visible through the `testutil`
//! feature (see the `[[test]]` section in Cargo.toml).

use engine::reference;
use engine::{Board, Color, Move, Status, double_threats, forced_blocks, immediate_wins};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn fast_matches_naive(cells in prop::collection::vec(0u16..225, 1..=225)) {
        let mut fast = Board::new();
        let mut naive = reference::Board::new();
        for p in cells {
            let mv = Move::new((p / 15) as u8, (p % 15) as u8).unwrap();
            prop_assert_eq!(fast.status(), naive.status());   // same outcome before the ply
            if fast.status() != Status::Ongoing { break; }
            let fast_r  = fast.play(mv);
            let naive_r = naive.play(mv);
            prop_assert_eq!(fast_r.is_ok(), naive_r.is_ok()); // same legality
            prop_assert_eq!(fast.status(), naive.status());   // same outcome after it
            prop_assert_eq!(fast.to_move(), naive.to_move())
       }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1_000))]

    #[test]
    fn undo_walks_match_naive(
        ops in prop::collection::vec((0u16..225, 0u8..3), 1..=100)
    ) {
        let mut fast = Board::new();
        let mut naive = reference::Board::new();
        for (p, coin) in ops {
            // ~1/3 of steps: undo, if there is anything to undo.
            if coin == 0 && !fast.moves().is_empty() {
                fast.undo();
                naive.undo();
            } else if fast.status() == Status::Ongoing {
                let mv = Move::new((p / 15) as u8, (p % 15) as u8).unwrap();
                let _ = fast.play(mv); // may be Occupied — both must agree
                let _ = naive.play(mv);
            }
            prop_assert_eq!(fast.status(), naive.status());
            prop_assert_eq!(fast.to_move(), naive.to_move());
            prop_assert_eq!(fast.moves(), naive.moves());
        }
    }
}

proptest! {
    // Tactics detection is expensive — `double_threats` is
    // O(empties^2 x scan) on BOTH sides of the comparison — so this
    // property runs fewer, meatier cases than the play/undo walks.
    // Scale up with PROPTEST_CASES for wide runs.
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// The fast bitboard tactics must agree with the naive line-scan
    /// oracle on random NON-TERMINAL positions (the through-cell scan
    /// answers a different question once a five exists — see
    /// reference.rs).
    #[test]
    fn tactics_match_reference(cells in prop::collection::vec(0u16..225, 1..=120)) {
        let mut fast = Board::new();
        for p in cells {
            if fast.status() != Status::Ongoing { break; }
            let _ = fast.play(Move::new((p / 15) as u8, (p % 15) as u8).unwrap());
        }
        // Compare on a NON-TERMINAL position: once a five exists, the
        // naive through-cell scan answers a different question than the
        // fast whole-board scan (see reference.rs). Undoing the
        // terminal move reopens the game — deterministic, unlike a
        // prop_assume, which at high PROPTEST_CASES aborts the test
        // with "too many global rejects" (measured: ~6% of random
        // 120-ply playouts reach a terminal position).
        if fast.status() != Status::Ongoing {
            fast.undo();
        }

        for side in [Color::Black, Color::White] {
            prop_assert_eq!(
                immediate_wins(&fast, side),
                reference::naive_immediate_wins(&fast, side),
                "immediate wins disagree for {:?}", side
            );
            prop_assert_eq!(
                double_threats(&fast, side),
                reference::naive_double_threats(&fast, side),
                "double threats disagree for {:?}", side
            );
        }
        prop_assert_eq!(forced_blocks(&fast), reference::naive_forced_blocks(&fast));
    }
}
