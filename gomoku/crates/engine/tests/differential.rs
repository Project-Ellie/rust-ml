//! Differential tests: the fast bitboard engine must agree with the
//! naive reference oracle on every ply of thousands of random games.
//!
//! This is an INTEGRATION test: it links against `engine` as an
//! external crate, which means the library is built WITHOUT `cfg(test)`
//! — so the reference engine is only visible through the `testutil`
//! feature (see the `[[test]]` section in Cargo.toml).

use engine::reference;
use engine::{Board, Move, Status};
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
