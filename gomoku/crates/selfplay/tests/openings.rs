use engine::{Color, Move};
use rand::SeedableRng;
use rand::prelude::StdRng;
use selfplay::opening::{OpeningAnswer, OpeningProvenance, OpeningSource, TournamentOpenings};
use std::path::Path;

fn mv(row: u8, col: u8) -> Move {
    Move::new(row, col).expect("test coordinates on board")
}

#[test]
fn three_stone_and_longer_lines_parse_as_swap2_offers() {
    let three_stone = "0,0, 1,0, -1,0";
    let longer = "0,0, 1,0, -1,0, 0,1, 0,-1, 2,2, -2,-2";

    for text in [three_stone, longer] {
        let mut pool = TournamentOpenings::parse("test-list", text).expect("parses");
        assert_eq!(pool.len(), 1);

        let mut rng = StdRng::seed_from_u64(7);
        let opening = pool.opening(&mut rng);

        assert_eq!(opening.initial, [mv(7, 7), mv(7, 8), mv(7, 6)]);
        assert_eq!(
            opening.provenance,
            OpeningProvenance::Listed {
                list: "test-list".to_owned(),
                index: 0,
            }
        );
        opening.board().expect("opening executes");
    }
}

#[test]
fn malformed_lines_are_rejected_with_physical_line_numbers() {
    let cases: [(&str, &[&str]); 5] = [
        ("0,0, 1,0, -1,0\n0,0, 1,0, 1,0", &["line 2", "duplicate"]),
        ("0,0, 1,0", &["line 1", "at least three"]),
        ("0,0, 1,0, 8,0", &["line 1", "outside"]),
        ("0,0, abc,0, -1,0", &["line 1", "invalid x"]),
        ("0,0, 1,0, -1,0, 0", &["line 1", "x,y pairs"]),
    ];

    for (text, expected_substrings) in cases {
        let error = TournamentOpenings::parse("bad-list", text)
            .expect_err("malformed line must be rejected");
        let message = error.to_string();
        for substring in expected_substrings {
            assert!(
                message.contains(substring),
                "'{message}' missing '{substring}'"
            );
        }
    }
}

#[test]
fn sampled_offers_come_from_the_loaded_list_with_seeded_determinism() {
    let text = "0,0, 1,0, -1,0\n0,0, 0,1, 0,-1\n1,1, 1,0, 1,-1";

    let mut pool_a = TournamentOpenings::parse("test-list", text).expect("parses");
    let mut pool_b = TournamentOpenings::parse("test-list", text).expect("parses");
    assert_eq!(pool_a.len(), 3);
    assert_eq!(pool_b.len(), 3);

    let expected = [
        [mv(7, 7), mv(7, 8), mv(7, 6)],
        [mv(7, 7), mv(8, 7), mv(6, 7)],
        [mv(8, 8), mv(7, 8), mv(6, 8)],
    ];

    let mut rng_a = StdRng::seed_from_u64(91);
    let mut rng_b = StdRng::seed_from_u64(91);

    let mut saw_offer = [false; 3];
    let mut saw_take_black = false;
    let mut saw_take_white = false;
    let mut saw_add_two_black = false;
    let mut saw_add_two_white = false;

    for _ in 0..64 {
        let opening_a = pool_a.opening(&mut rng_a);
        let opening_b = pool_b.opening(&mut rng_b);
        assert_eq!(opening_a, opening_b);

        for (i, want) in expected.iter().enumerate() {
            if opening_a.initial == *want {
                saw_offer[i] = true;
            }
        }

        match opening_a.answer {
            OpeningAnswer::Take(Color::Black) => saw_take_black = true,
            OpeningAnswer::Take(Color::White) => saw_take_white = true,
            OpeningAnswer::AddTwo { final_choice, .. } => match final_choice {
                Color::Black => saw_add_two_black = true,
                Color::White => saw_add_two_white = true,
            },
        }

        opening_a.board().expect("every sampled opening executes");
    }

    assert!(saw_offer.iter().all(|b| *b), "every offer must appear");
    assert!(saw_take_black, "take-black answer must appear");
    assert!(saw_take_white, "take-white answer must appear");
    assert!(
        saw_add_two_black && saw_add_two_white,
        "both add-two final colors must appear"
    );
}

#[test]
fn valid_committed_tournament_lists_load_into_one_uniform_offer_pool() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/openings");
    let paths = [
        dir.join("gomocup-2024-freestyle15.txt"),
        dir.join("gomocup-2025-freestyle15.txt"),
        dir.join("gomocup-2026-freestyle15.txt"),
    ];

    let mut pool = TournamentOpenings::load(&paths).expect("committed lists load");
    assert_eq!(pool.len(), 36);
    assert!(!pool.is_empty());

    let mut rng = StdRng::seed_from_u64(123);
    let opening = pool.opening(&mut rng);

    let OpeningProvenance::Listed { list, index } = &opening.provenance else {
        panic!("committed-list sample must be Listed");
    };
    assert!(
        paths.iter().any(|p| p.to_string_lossy() == list.as_str()),
        "list identity must match one of the supplied paths"
    );
    assert!(*index < 41, "index {index} must be below 41");
    opening.board().expect("committed-list opening executes");
}

#[test]
fn malformed_committed_piskvork_line_is_rejected_with_its_line_number() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/openings/piskvork-openings.txt");

    let error = TournamentOpenings::load(&[path]).expect_err("piskvork list must fail to parse");
    let message = error.to_string();
    assert!(message.contains("line 41"), "'{message}' missing 'line 41'");
    assert!(message.contains("outside"), "'{message}' missing 'outside'");
}
