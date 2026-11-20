use engine::{Color, Move};
use player::PlayerOptions;
use selfplay::anchor::{AnchorError, AnchorSeed, extract_anchor_seeds};
use selfplay::game::{Game, Outcome, PlyRecord, WinReason};
use selfplay::opening::{Opening, OpeningAnswer, OpeningProvenance};
use selfplay::record::{CertifiedSuffix, GameRecord};
use selfplay::schedule::ExplorationSchedule;

fn mv(row: u8, col: u8) -> Move {
    Move::new(row, col).unwrap()
}

/// A hand-built game with a one-move certified suffix.
///
/// Black's open four on row 5 wins at either end, so (5, 4) is a verified
/// alternative line even though the game played (5, 9).
fn fabricated_record() -> GameRecord {
    let game = Game {
        seed: 17,
        schedule: ExplorationSchedule::default(),
        black_options: PlayerOptions::default(),
        white_options: PlayerOptions::default(),
        opening: Opening::new(
            [mv(5, 5), mv(10, 10), mv(5, 6)],
            OpeningAnswer::Take(Color::Black),
            OpeningProvenance::Generated,
        ),
        plies: vec![
            PlyRecord {
                mv: mv(10, 11),
                search: None,
            },
            PlyRecord {
                mv: mv(5, 7),
                search: None,
            },
            PlyRecord {
                mv: mv(10, 12),
                search: None,
            },
            PlyRecord {
                mv: mv(5, 8),
                search: None,
            },
            PlyRecord {
                mv: mv(10, 13),
                search: None,
            },
            PlyRecord {
                mv: mv(5, 9),
                search: None,
            },
        ],
        outcome: Outcome::Won {
            winner: Color::Black,
            reason: WinReason::FiveOrMore,
        },
        certified_suffix: Some(CertifiedSuffix {
            from_ply: 5,
            line: vec![mv(5, 9)],
        }),
    };
    GameRecord::from_game(&game)
}

#[test]
fn certified_suffix_emits_replayable_anchor_seed() {
    let record = fabricated_record();
    let seeds = extract_anchor_seeds(&[record]).expect("the certificate verifies");
    assert_eq!(
        seeds,
        vec![AnchorSeed {
            game_index: 0,
            from_ply: 5,
            line: vec![mv(5, 9)],
        }]
    );
}

#[test]
fn missing_certificate_emits_no_anchor_data() {
    let mut record = fabricated_record();
    record.certified_suffix = None;
    let seeds = extract_anchor_seeds(&[record]).expect("missing certificates are not errors");
    assert!(seeds.is_empty());
}

#[test]
fn verified_alternative_line_that_was_not_played_is_rejected() {
    let mut record = fabricated_record();
    // Both ends of Black's open four win immediately, but the game played 5,9.
    record.certified_suffix = Some(CertifiedSuffix {
        from_ply: 5,
        line: vec![mv(5, 4)],
    });
    let err = extract_anchor_seeds(&[record])
        .expect_err("a certificate must describe the line actually played");
    match err {
        AnchorError::SuffixNotPlayed {
            game_index,
            from_ply,
        } => {
            assert_eq!(game_index, 0);
            assert_eq!(from_ply, 5);
        }
        other => panic!("expected SuffixNotPlayed, got {other:?}"),
    }
}

#[test]
fn doctored_suffix_line_is_a_hard_error() {
    let mut record = fabricated_record();
    record.certified_suffix = Some(CertifiedSuffix {
        from_ply: 5,
        line: vec![mv(0, 0)],
    });
    let err = extract_anchor_seeds(&[record]).expect_err("tampering must abort extraction");
    match err {
        AnchorError::VerificationFailed {
            game_index,
            from_ply,
        } => {
            assert_eq!(game_index, 0);
            assert_eq!(from_ply, 5);
        }
        other => panic!("expected VerificationFailed, got {other:?}"),
    }
}
