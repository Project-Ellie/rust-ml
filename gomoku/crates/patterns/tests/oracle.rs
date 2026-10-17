use engine::Color::{Black, White};
use engine::reference::board_from_ascii;
use engine::{Board, Color, Move, MoveSet};
use patterns::oracle::{Direction, open_three_makers};

mod common;

fn first_marked(m: MoveSet) -> Option<(u8, u8)> {
    for r in 0u8..15 {
        for c in 0u8..15 {
            if m.contains(Move::new(r, c).unwrap()) {
                return Some((r, c));
            }
        }
    }
    None
}

#[test]
fn quiet_board_has_no_open_three_makers() {
    let image = "
    O  .  .  .  .  .  O  .  .  .  .  .  .  X  .
    .  O  .  .  .  .  .  .  .  .  .  .  .  .  X
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  O  O  X  X  O  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  O  O
    X  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  X  .  .  .  .  .  .  .
    .  .  .  .  .  X  .  .  .  .  .  .  .  .  .
    .  .  .  .  X  .  .  .  .  .  .  .  .  X  X
    .  .  .  O  .  .  .  .  .  .  .  .  .  .  .
    .  .  X  .  .  .  .  .  O  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    ";
    let board = board_from_ascii(image);

    for c in [Color::White, Color::Black] {
        let movesets = open_three_makers(&board, c);
        for dir in Direction::ALL {
            let moveset = movesets[dir as usize];
            let suspicious = first_marked(moveset);
            assert_eq!(
                moveset,
                MoveSet::EMPTY,
                "{c:?} {dir:?} at {suspicious:?} is a three-maker!"
            );
        }
    }
}

#[test]
fn xxx_and_x_xx_patterns_fire_horizontally() {
    let image = "
    .  .  o  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  o  .  o  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  x  .  x  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  x  x  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  o  o  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  x  .
    ";
    let board = board_from_ascii(image);

    // the positions next to the black pair should fire
    let blacks = open_three_makers(&board, Color::Black)[Direction::Horizontal as usize];
    let whites = open_three_makers(&board, Color::White)[Direction::Horizontal as usize];
    for r in 0u8..15 {
        for c in 0u8..15 {
            if [(8, 4), (8, 5), (8, 8), (8, 9), (4, 5), (4, 7), (4, 9)].contains(&(r, c)) {
                assert!(blacks.contains(Move::new(r, c).unwrap()));
            } else if [(11, 4), (11, 5), (11, 8), (11, 9), (2, 5), (2, 7), (2, 9)].contains(&(r, c))
            {
                assert!(whites.contains(Move::new(r, c).unwrap()));
            } else {
                assert!(!whites.contains(Move::new(r, c).unwrap()));
                assert!(!blacks.contains(Move::new(r, c).unwrap()));
            }
        }
    }
}

#[test]
fn xxx_and_x_xx_patterns_fire_vertically() {
    // Transposed twin of the horizontal test: each color has an adjacent
    // pair (4 makers) and a gapped pair (3 makers, one per family) along
    // a column. Black: adjacent (6,8)(7,8), gapped (6,4)x(8,4).
    // White: adjacent (6,11)(7,11), gapped (6,2)x(8,2).
    let image = "
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    o  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  o  .  x  .  .  .  x  .  .  o  .  .  .
    .  .  .  .  .  .  .  .  x  .  .  o  .  .  .
    .  .  o  .  x  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  x
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    ";
    let board = board_from_ascii(image);

    let blacks = open_three_makers(&board, Color::Black)[Direction::Vertical as usize];
    let whites = open_three_makers(&board, Color::White)[Direction::Vertical as usize];
    for r in 0u8..15 {
        for c in 0u8..15 {
            if [(4, 8), (5, 8), (8, 8), (9, 8), (5, 4), (7, 4), (9, 4)].contains(&(r, c)) {
                assert!(blacks.contains(Move::new(r, c).unwrap()));
            } else if [(4, 11), (5, 11), (8, 11), (9, 11), (5, 2), (7, 2), (9, 2)].contains(&(r, c))
            {
                assert!(whites.contains(Move::new(r, c).unwrap()));
            } else {
                assert!(!whites.contains(Move::new(r, c).unwrap()));
                assert!(!blacks.contains(Move::new(r, c).unwrap()));
            }
        }
    }
}

#[test]
fn xxx_and_x_xx_patterns_fire_diagonally_down() {
    // Same fixture shape along the \ diagonal. Black: adjacent (5,5)(6,6),
    // gapped (10,4)x(12,6). White: adjacent (5,9)(6,10), gapped (10,9)x(12,11).
    let image = "
    .  .  o  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  x  .  .  .  o  .  .  .  .  .
    .  .  .  .  .  .  x  .  .  .  o  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  x  .  .  .  .  o  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  x  .  .  .  .  o  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  x
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    ";
    let board = board_from_ascii(image);

    let blacks = open_three_makers(&board, Color::Black)[Direction::DiagDown as usize];
    let whites = open_three_makers(&board, Color::White)[Direction::DiagDown as usize];
    for r in 0u8..15 {
        for c in 0u8..15 {
            if [(4, 4), (7, 7), (3, 3), (8, 8), (11, 5), (9, 3), (13, 7)].contains(&(r, c)) {
                assert!(blacks.contains(Move::new(r, c).unwrap()));
            } else if [(4, 8), (7, 11), (3, 7), (8, 12), (11, 10), (9, 8), (13, 12)]
                .contains(&(r, c))
            {
                assert!(whites.contains(Move::new(r, c).unwrap()));
            } else {
                assert!(!whites.contains(Move::new(r, c).unwrap()));
                assert!(!blacks.contains(Move::new(r, c).unwrap()));
            }
        }
    }
}

#[test]
fn xxx_does_not_fire_when_end_is_opponent_blocked() {
    // Extra attention for the cell between a black and a white,
    // This is *not* a threat, here, we only test those ones
    let image = "
    .  .  .  .  .  .  .  .  .  .  x  x  .  o  .
    .  .  x  .  o  o  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  x  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  x  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    x  .  .  .  .  .  .  .  .  .  .  o  .  .  .
    .  .  .  .  .  o  .  .  .  .  .  .  .  .  .
    o  .  .  .  o  .  .  .  .  .  .  .  .  .  x
    o  .  .  .  .  .  .  .  .  .  .  .  .  .  x
    .  .  x  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  o
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    ";
    let b = board_from_ascii(image);

    let diag_down = Direction::DiagDown as usize;
    let diag_up = Direction::DiagUp as usize;
    let vertical = Direction::Vertical as usize;
    let horizontal = Direction::Horizontal as usize;

    assert!(!open_three_makers(&b, Black)[diag_down].contains(Move::new(7, 10).unwrap()));

    assert!(!open_three_makers(&b, Black)[horizontal].contains(Move::new(0, 12).unwrap()));

    assert!(!open_three_makers(&b, Black)[vertical].contains(Move::new(12, 14).unwrap()));

    assert!(!open_three_makers(&b, White)[diag_up].contains(Move::new(11, 3).unwrap()));

    assert!(!open_three_makers(&b, White)[vertical].contains(Move::new(9, 0).unwrap()));

    assert!(!open_three_makers(&b, White)[horizontal].contains(Move::new(1, 3).unwrap()));
}

#[test]
fn xxx_does_not_fire_when_edge_hugging() {
    // Same clause as the opponent-blocked test, but the window's
    // required-empty end is off-board (Blocked): every pair hugs an edge,
    // and the completion cell on the edge side must stay silent.
    let image = "
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    x  .  .  o  .  .  .  .  .  .  .  .  .  o  .
    x  .  .  .  o  .  .  .  .  .  .  .  o  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  x  x  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  o  o  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    o  .  .  .  x  .  .  .  .  .  .  .  x  .  .
    o  .  .  x  .  .  .  .  .  .  .  .  .  x  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    ";
    let b = board_from_ascii(image);

    let diag_down = Direction::DiagDown as usize;
    let diag_up = Direction::DiagUp as usize;
    let vertical = Direction::Vertical as usize;
    let horizontal = Direction::Horizontal as usize;

    assert!(!open_three_makers(&b, Black)[diag_down].contains(Move::new(14, 14).unwrap()));

    assert!(!open_three_makers(&b, Black)[diag_up].contains(Move::new(14, 2).unwrap()));

    assert!(!open_three_makers(&b, Black)[horizontal].contains(Move::new(5, 14).unwrap()));

    assert!(!open_three_makers(&b, Black)[vertical].contains(Move::new(0, 0).unwrap()));

    assert!(!open_three_makers(&b, White)[diag_down].contains(Move::new(0, 2).unwrap()));

    assert!(!open_three_makers(&b, White)[diag_up].contains(Move::new(0, 14).unwrap()));

    assert!(!open_three_makers(&b, White)[vertical].contains(Move::new(14, 0).unwrap()));

    assert!(!open_three_makers(&b, White)[horizontal].contains(Move::new(7, 0).unwrap()));

    // The open side of every edge-hugging pair still fires its straight
    // _XXX_ anchor — proof the detector is alive on this board, so the
    // rejections above are clause violations, not a dead oracle.
    assert!(open_three_makers(&b, Black)[diag_down].contains(Move::new(11, 11).unwrap()));

    assert!(open_three_makers(&b, Black)[diag_up].contains(Move::new(11, 5).unwrap()));

    assert!(open_three_makers(&b, Black)[horizontal].contains(Move::new(5, 11).unwrap()));

    assert!(open_three_makers(&b, Black)[vertical].contains(Move::new(3, 0).unwrap()));

    assert!(open_three_makers(&b, White)[diag_down].contains(Move::new(3, 5).unwrap()));

    assert!(open_three_makers(&b, White)[diag_up].contains(Move::new(3, 11).unwrap()));

    assert!(open_three_makers(&b, White)[vertical].contains(Move::new(10, 0).unwrap()));

    assert!(open_three_makers(&b, White)[horizontal].contains(Move::new(7, 3).unwrap()));
}

#[test]
fn anti_four_margin_rejects_extension_to_four() {
    // An own stone one cell beyond the window end makes the completed
    // shape a broken four, not an open three: the candidate between
    // pair and single must stay silent ((8,4) for Black, (11,6) for
    // White). The pair's open side still fires (the anchors). The cell
    // adjacent to the single ((8,3), (11,7)) is silent too, but by the
    // window-end rule, not the margin.
    let image = "
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  x  .  .  x  x  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  o  o  .  .  o  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    ";
    let b = board_from_ascii(image);

    let blacks = open_three_makers(&b, Black)[Direction::Horizontal as usize];
    let whites = open_three_makers(&b, White)[Direction::Horizontal as usize];
    for r in 0u8..15 {
        for c in 0u8..15 {
            if [(8, 7), (8, 8)].contains(&(r, c)) {
                assert!(blacks.contains(Move::new(r, c).unwrap()));
            } else if [(11, 2), (11, 3)].contains(&(r, c)) {
                assert!(whites.contains(Move::new(r, c).unwrap()));
            } else {
                assert!(!whites.contains(Move::new(r, c).unwrap()));
                assert!(!blacks.contains(Move::new(r, c).unwrap()));
            }
        }
    }
}

#[test]
fn overline_present_rejects_adjacent_three_maker() {
    // Five in a row is terminal, so this board cannot go through
    // board_from_ascii (it replays moves and panics on game-over
    // boards); from_position sets the bitboards directly. Black has
    // already won — and extending the five to six (an overline, still a
    // win in freestyle) is a winning move, not an open-three maker:
    // (8,4) and (8,10) must stay silent. The white corner stones only
    // satisfy the count invariant.
    let black: Vec<Move> = (5u8..=9).map(|c| Move::new(8, c).unwrap()).collect();
    let white: Vec<Move> = [(0u8, 0u8), (0, 14), (14, 0), (14, 14)]
        .iter()
        .map(|&(r, c)| Move::new(r, c).unwrap())
        .collect();
    let b = Board::from_position(&black, &white, White).unwrap();

    // Verified: every moveset on this board is empty, all directions,
    // both colors. Sweep the horizontal set and name the two adjacent
    // cells — they are the point of the test.
    let blacks = open_three_makers(&b, Black)[Direction::Horizontal as usize];
    assert!(!blacks.contains(Move::new(8, 4).unwrap()));
    assert!(!blacks.contains(Move::new(8, 10).unwrap()));
    for r in 0u8..15 {
        for c in 0u8..15 {
            assert!(!blacks.contains(Move::new(r, c).unwrap()));
        }
    }
}

#[test]
fn double_three_fork_fires_in_two_directions() {
    // Two pairs pointing at the same empty cell: the fork cell completes
    // an open three in two directions at once — the unanswerable
    // double-three. Black fork (7,8), white fork (12,2).
    // Targeted assertions only: a straight-straight fork unavoidably
    // makes the inner stones a diagonal pair as well ((6,8),(7,7) here),
    // so the diagonal movesets contain extra makers — geometry, not a
    // bug. Do not exact-set this board.
    let image = "
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  x  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  x  .  .  .  .  .  .
    .  .  .  .  .  .  x  x  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  o  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  o  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  o  o  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    ";
    let b = board_from_ascii(image);

    let blacks = open_three_makers(&b, Black);
    assert!(blacks[Direction::Horizontal as usize].contains(Move::new(7, 8).unwrap()));
    assert!(blacks[Direction::Vertical as usize].contains(Move::new(7, 8).unwrap()));

    let whites = open_three_makers(&b, White);
    assert!(whites[Direction::Horizontal as usize].contains(Move::new(12, 2).unwrap()));
    assert!(whites[Direction::Vertical as usize].contains(Move::new(12, 2).unwrap()));
}

#[test]
fn diagonal_up_direction_is_independent() {
    // Only '/' pairs on the board: DiagUp must produce the pair's four
    // makers, and the other three directions must come back completely
    // empty. A sign-flip (scanning '\' twice) fires nothing or leaks
    // into DiagDown; a shared accumulator for both diagonals fails the
    // emptiness sweep.
    // White pair (7,7),(8,6) -> {(5,9),(6,8),(9,5),(10,4)}.
    // Black pair (10,11),(11,10) -> {(8,13),(9,12),(12,9),(13,8)}.
    let image = "
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  o  .  .  .  .  .  .  .
    .  .  .  .  .  .  o  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  x  .  .  .
    .  .  .  .  .  .  .  .  .  .  x  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
    ";
    let b = board_from_ascii(image);

    let blacks = open_three_makers(&b, Black);
    let whites = open_three_makers(&b, White);
    let diag_up = Direction::DiagUp as usize;
    let other_dirs = [
        Direction::Horizontal as usize,
        Direction::Vertical as usize,
        Direction::DiagDown as usize,
    ];
    for r in 0u8..15 {
        for c in 0u8..15 {
            if [(5, 9), (6, 8), (9, 5), (10, 4)].contains(&(r, c)) {
                assert!(whites[diag_up].contains(Move::new(r, c).unwrap()));
            } else if [(8, 13), (9, 12), (12, 9), (13, 8)].contains(&(r, c)) {
                assert!(blacks[diag_up].contains(Move::new(r, c).unwrap()));
            } else {
                assert!(!whites[diag_up].contains(Move::new(r, c).unwrap()));
                assert!(!blacks[diag_up].contains(Move::new(r, c).unwrap()));
            }
            for dir in other_dirs {
                assert!(!blacks[dir].contains(Move::new(r, c).unwrap()));
                assert!(!whites[dir].contains(Move::new(r, c).unwrap()));
            }
        }
    }
}
