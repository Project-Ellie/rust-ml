> **Opt-in solution — read only after you have tried the chapter, or if you have been stuck for more than twenty minutes.**
>
> This file quotes `tests/three_maps.rs` from the verified reference crate
> verbatim. The kernels themselves already appear in chapter 3's solution
> in their final form; the new artifact for chapter 5 is this direct test
> file.
>
> The file imports `patterns::oracle` for ground-truth comparisons. That
> oracle is the subject of chapter 6, so if you are working strictly in
> order you can either inline equivalent window checks now or run these
> tests after the oracle exists.

````rust
//! Direct ASCII tests for the threat network's open-three maker maps.
//!
//! Each hand-built board is evaluated with [`ThreatNet`] and the
//! resulting [`ThreatMaps::threes_per_dir`] channels are compared
//! *as exact cell sets* against the naive [`oracle::open_three_makers`].
//! The double-three fork test also compares [`ThreatMaps::double_threes`]
//! against [`oracle::double_threes`].

mod common;

use burn::backend::NdArray;
use engine::{Color, Move, MoveSet, reference::board_from_ascii};
use patterns::net::{ThreatMaps, ThreatNet, analyze};
use patterns::oracle::{Direction, double_threes, open_three_makers};

fn threes_set(maps: &ThreatMaps, dir: Direction) -> MoveSet {
    let mut s = MoveSet::EMPTY;
    for r in 0..15u8 {
        for c in 0..15u8 {
            if maps.threes_per_dir[dir as usize][r as usize][c as usize] > 0.0 {
                s.insert(Move::new(r, c).unwrap());
            }
        }
    }
    s
}

fn double_three_set(maps: &ThreatMaps) -> MoveSet {
    let mut s = MoveSet::EMPTY;
    for r in 0..15u8 {
        for c in 0..15u8 {
            if maps.double_threes[r as usize][c as usize] > 0.0 {
                s.insert(Move::new(r, c).unwrap());
            }
        }
    }
    s
}

fn assert_threes_match(maps: &ThreatMaps, b: &engine::Board, s: Color) {
    let oracle = open_three_makers(b, s);
    for dir in Direction::ALL {
        assert_eq!(
            threes_set(maps, dir),
            oracle[dir as usize],
            "three-maker mismatch in direction {dir:?}"
        );
    }
}

fn analyze_maps(b: &engine::Board, s: Color) -> ThreatMaps {
    let device = Default::default();
    let net = ThreatNet::<NdArray>::new(&device);
    analyze(&net, b, s, &device)
}

#[test]
fn quiet_board_has_empty_three_maps() {
    let b = board_from_ascii(
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
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        ",
    );
    let maps = analyze_maps(&b, Color::Black);
    assert_threes_match(&maps, &b, Color::Black);
}

#[test]
fn xxx_pattern_matches_oracle_horizontally() {
    // Existing black at (7,1),(7,2); O at (7,5) kills the broken
    // _XX_X_ at (7,4). Only (7,3) makes _XXX_.
    let b = board_from_ascii(
        "
        O . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . X X . . O . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        ",
    );
    let maps = analyze_maps(&b, Color::Black);
    assert_threes_match(&maps, &b, Color::Black);
}

#[test]
fn xxx_pattern_matches_oracle_diagonally_down() {
    // Existing black at (1,1),(2,2); O at (5,5) kills the broken
    // _XX_X_ at (4,4). Only (3,3) makes _XXX_.
    let b = board_from_ascii(
        "
        . . . . . . . . . . . . . . .
        . X . . . . . . . . . . . . .
        . . X . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . O . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . O
        ",
    );
    let maps = analyze_maps(&b, Color::Black);
    assert_threes_match(&maps, &b, Color::Black);
}

#[test]
fn x_xx_pattern_matches_oracle_horizontally() {
    // Existing black at (7,8),(7,9). (7,6) is the empty MAKER cell
    // that becomes the isolated X of `_X_XX_` after placement; filling
    // the gap (7,7) also makes _XXX_; extending the pair gives
    // _XXX_ at (7,10) and _XX_X_ at (7,11).
    let b = board_from_ascii(
        "
        O . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . X X . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . O
        ",
    );
    let maps = analyze_maps(&b, Color::Black);
    assert_threes_match(&maps, &b, Color::Black);
}

#[test]
fn x_xx_pattern_matches_oracle_diagonally_down() {
    // Pair at (9,9),(10,10); isolated X at (7,7). The gap (8,8)
    // is also a _XXX_ maker.
    let b = board_from_ascii(
        "
        O . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . X . . . . .
        . . . . . . . . . . X . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . O
        ",
    );
    let maps = analyze_maps(&b, Color::Black);
    assert_threes_match(&maps, &b, Color::Black);
}

#[test]
fn xx_x_pattern_matches_oracle_horizontally() {
    // Existing black at (7,6),(7,7); O at (7,3) kills the mirror
    // `_X_XX_` maker at (7,4) (blocks its leftmost required-empty
    // cell); to the right, (7,8) makes `_XXX_` and (7,9) is the lone
    // X of `_XX_X_`; to the left, (7,5) makes `_XXX_`.
    let b = board_from_ascii(
        "
        O . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . O . . X X . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        ",
    );
    let maps = analyze_maps(&b, Color::Black);
    assert_threes_match(&maps, &b, Color::Black);
}

#[test]
fn xx_x_pattern_matches_oracle_diagonally_down() {
    // Pair at (5,5),(6,6); O at (3,3) kills the mirror _XXX_ at
    // (4,4). Lone X at (8,8); gap (7,7) is also a _XXX_ maker.
    let b = board_from_ascii(
        "
        O . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . O . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . X . . . . . . . . .
        . . . . . . X . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        ",
    );
    let maps = analyze_maps(&b, Color::Black);
    assert_threes_match(&maps, &b, Color::Black);
}

#[test]
fn edge_hugging_three_does_not_fire() {
    // Black at (7,0),(7,2); candidate (7,1). The left empty end of
    // _XXX_ would be off the board.
    let b = board_from_ascii(
        "
        O . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        X . X . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        ",
    );
    let maps = analyze_maps(&b, Color::Black);
    assert_threes_match(&maps, &b, Color::Black);
}

#[test]
fn opponent_blocked_end_does_not_fire() {
    // Black at (7,6),(7,8); O at (7,9) blocks the right empty end.
    let b = board_from_ascii(
        "
        O . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . X . X O . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        ",
    );
    let maps = analyze_maps(&b, Color::Black);
    assert_threes_match(&maps, &b, Color::Black);
}

#[test]
fn anti_four_margin_rejects_three_maker() {
    // _XXX_ with an own stone at (7,10) immediately beyond the right end.
    let b = board_from_ascii(
        "
        O . . . . . . . . . . . . . O
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . X . X . X . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . O
        ",
    );
    let maps = analyze_maps(&b, Color::Black);
    assert_threes_match(&maps, &b, Color::Black);
}

#[test]
fn double_three_fork_matches_oracle() {
    // Horizontal and vertical threes through (7,7). There are other
    // single-direction makers, but only (7,7) has both.
    let b = board_from_ascii(
        "
        O . . . . . . . . . . . . . O
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . X . . . . . . .
        . . . . . . X . X . . . . . .
        . . . . . . . X . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        O . . . . . . . . . . . . . O
        ",
    );
    let maps = analyze_maps(&b, Color::Black);
    assert_threes_match(&maps, &b, Color::Black);
    assert_eq!(
        double_three_set(&maps),
        double_threes(&b, Color::Black),
        "double-three fork map must match the oracle"
    );
}
````
