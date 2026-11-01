//! Parser tests for puzzle packs (spec amendment "positions from files").
//!
//! The valid-pack tests load the committed mini fixture; malformed inputs
//! are inline JSON strings. Tests never touch the git-ignored corpora
//! under `data/`.

use cli::puzzle::{PuzzleError, PuzzlePack};
use engine::{Color, Move};

const MINI_PACK: &str = include_str!("fixtures/mini-pack.json");

fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}

#[test]
fn parses_valid_pack() {
    let pack = PuzzlePack::from_json(MINI_PACK).expect("mini fixture parses");
    assert_eq!(pack.items.len(), 3);

    let first = &pack.items[0];
    assert_eq!(first.index, 0);
    assert_eq!(first.attacker, Color::Black);
    assert_eq!(first.board.to_move(), Color::Black);
    assert_eq!(first.solution, vec![(mv(7, 7), Color::Black)]);

    let third = &pack.items[2];
    assert_eq!(third.index, 2);
    assert_eq!(third.attacker, Color::Black);
    assert_eq!(
        third.solution,
        vec![
            (mv(7, 10), Color::Black),
            (mv(7, 6), Color::White),
            (mv(7, 11), Color::Black),
        ]
    );
}

#[test]
fn file_coordinates_are_x_column_y_row() {
    // Legacy viewer convention: file [x, y] maps to Move::new(y, x) —
    // x is the column, y is the row.
    let pack = PuzzlePack::from_json(MINI_PACK).unwrap();
    let first = &pack.items[0];
    // File stone [7, 3]: column 7 ("h"), row 3.
    assert_eq!(first.board.stone_at(mv(3, 7)), Some(Color::Black));
    assert_eq!(first.board.stone_at(mv(7, 3)), None);
    // File line head [7, 7] renders as algebraic h8.
    assert_eq!(cli::fmt::format_move(first.solution[0].0), "h8");
}

#[test]
fn board_is_replayable_position_with_attacker_to_move() {
    let pack = PuzzlePack::from_json(MINI_PACK).unwrap();
    let white_puzzle = &pack.items[1];
    assert_eq!(white_puzzle.attacker, Color::White);
    assert_eq!(white_puzzle.board.to_move(), Color::White);
    assert_eq!(white_puzzle.board.moves().len(), 0);
}

#[test]
fn malformed_json_is_rejected() {
    let err = PuzzlePack::from_json("{ not json").unwrap_err();
    assert!(matches!(err, PuzzleError::Json(_)));
}

#[test]
fn count_mismatch_is_rejected() {
    let text = r#"{ "v": 1, "count": 2, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::CountMismatch {
            declared: 2,
            actual: 1
        }
    ));
}

#[test]
fn off_board_coordinate_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [[15, 3]], "w": [], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::InvalidCoord {
            item: 0,
            x: 15,
            y: 3
        }
    ));
}

#[test]
fn off_board_line_coordinate_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [], "l": [[7, 15, 1]] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::InvalidCoord {
            item: 0,
            x: 7,
            y: 15
        }
    ));
}

#[test]
fn duplicate_stone_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [[7, 3], [7, 3]], "w": [], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::DuplicateStone {
            item: 0,
            x: 7,
            y: 3
        }
    ));
}

#[test]
fn duplicate_white_stone_is_rejected_as_duplicate_not_overlap() {
    // A color duplicating itself is DuplicateStone, even for White —
    // OverlappingColors is reserved for a cell claimed by both colors.
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [[7, 3], [7, 3]], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::DuplicateStone {
            item: 0,
            x: 7,
            y: 3
        }
    ));
}

#[test]
fn overlapping_colors_are_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [[7, 3]], "w": [[7, 3]], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(err, PuzzleError::OverlappingColors { item: 0 }));
}

#[test]
fn counts_disagreeing_with_attacker_are_rejected() {
    // Black to move requires equal counts; here Black has one more.
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [[7, 3], [8, 3]], "w": [[0, 0]], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::InvalidCounts {
            item: 0,
            black: 2,
            white: 1,
            attacker: Color::Black,
        }
    ));
}

#[test]
fn unknown_attacker_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "red", "d": 1, "b": [], "w": [], "l": [] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(err, PuzzleError::InvalidAttacker { item: 0, .. }));
}

#[test]
fn line_starting_with_the_wrong_side_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [], "l": [[7, 7, 2]] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::InvalidSolutionSide {
            item: 0,
            expected: Color::Black,
            found: 2,
        }
    ));
}

#[test]
fn unknown_line_side_marker_is_rejected() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [], "l": [[7, 7, 1], [8, 7, 9]] }
    ] }"#;
    let err = PuzzlePack::from_json(text).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::InvalidSide { item: 0, value: 9 }
    ));
}

#[test]
fn missing_line_defaults_to_empty_solution() {
    // Unlabelled packs (positions only) carry no `l` field.
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [] }
    ] }"#;
    let pack = PuzzlePack::from_json(text).unwrap();
    assert!(pack.items[0].solution.is_empty());
}

#[test]
fn index_out_of_range_is_rejected() {
    let pack = PuzzlePack::from_json(MINI_PACK).unwrap();
    assert!(pack.get(0).is_ok());
    assert!(pack.get(2).is_ok());
    let err = pack.get(3).unwrap_err();
    assert!(matches!(
        err,
        PuzzleError::IndexOutOfRange { index: 3, count: 3 }
    ));
}
