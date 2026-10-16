//! Engine differential gates — the chapter-4 acceptance.
//!
//! The network's win and double-threat maps must agree *exactly* with
//! the engine oracles ([`engine::immediate_wins`] and
//! [`engine::double_threats`]) on seeded random mid-game boards, for
//! both colours.
//!
//! * Default suite: seeds `0..100` (`cargo test -p patterns`).
//! * Full proof: seeds `0..1000`, `#[ignore]`d
//!   (`cargo test -p patterns --release -- --ignored`).
//!
//! The full range is a strict prefix-extension of the default range,
//! so strengthening the proof only ever extends the seed set.

mod common;

use std::ops::Range;

use burn::tensor::Device;
use common::seeded_board;
use engine::{Board, Color, Move, MoveSet};
use patterns::net::{ThreatNet, analyze};

/// Seeds covered by the default (every-commit) gate.
const DEFAULT_SEEDS: Range<u64> = 0..100;

/// Seeds covered by the full proof gate — a strict extension of
/// [`DEFAULT_SEEDS`].
const FULL_SEEDS: Range<u64> = 0..1000;

#[test]
fn same_seed_create_same_board() {
    for seed in 1..33 {
        let b1 = seeded_board(seed);
        let b2 = seeded_board(seed);
        assert_ne!(b1.zobrist(), 0);
        assert_eq!(b1.zobrist(), b2.zobrist(), "Boards should be the same.");
    }
}

#[test]
fn wins_default() {
    check_wins(DEFAULT_SEEDS);
}

#[test]
fn double_threats_default() {
    check_double_threats(DEFAULT_SEEDS);
}

#[test]
#[ignore = "full 1000-board proof: cargo test -p patterns --release -- --ignored"]
fn wins_full() {
    check_wins(FULL_SEEDS);
}

#[test]
#[ignore = "full 1000-board proof: cargo test -p patterns --release -- --ignored"]
fn double_threats_full() {
    check_double_threats(FULL_SEEDS);
}

/// Run the win-map gate over `seeds`, both colours per board.
fn check_wins(seeds: Range<u64>) {
    let device = Device::flex();
    let net = ThreatNet::new(&device);
    let mut oracle_nonempty = 0u32;

    for seed in seeds {
        let board = seeded_board(seed);
        for side in [Color::Black, Color::White] {
            let want = engine::immediate_wins(&board, side);
            oracle_nonempty += u32::from(!want.is_empty());
            let got = set_from_map(&net.win_map(&board, side, &device));
            assert_agreement(seed, &board, side, "win map", &got, &want);
        }
    }

    // Coverage report (not an assertion): uniform random boards are
    // not guaranteed to contain immediate wins, so a hard floor here
    // would make the gate brittle rather than strong.
    eprintln!("win-map gate: engine oracle non-empty for {oracle_nonempty} board/colour samples");
}

/// Run the double-threat gate over `seeds`, both colours per board.
fn check_double_threats(seeds: Range<u64>) {
    let device = Device::flex();
    let net = ThreatNet::new(&device);
    let mut oracle_nonempty = 0u32;

    for seed in seeds {
        let board = seeded_board(seed);
        for side in [Color::Black, Color::White] {
            let want = engine::double_threats(&board, side);
            oracle_nonempty += u32::from(!want.is_empty());
            let maps = analyze(&net, &board, side, &device);
            let got = set_from_map(&maps.double_threats);
            assert_agreement(seed, &board, side, "double-threat map", &got, &want);
        }
    }

    eprintln!(
        "double-threat gate: engine oracle non-empty for {oracle_nonempty} board/colour samples"
    );
}

/// Threshold a network map into a set of cells (`> 0.0` means set).
fn set_from_map(map: &[[f32; 15]; 15]) -> MoveSet {
    let mut set = MoveSet::EMPTY;
    for r in 0..15u8 {
        for c in 0..15u8 {
            if map[r as usize][c as usize] > 0.0 {
                set.insert(Move::new(r, c).expect("(r, c) is in-board"));
            }
        }
    }
    set
}

/// Assert exact set equality between network and engine. The failure
/// message names the seed, the colour, and every disagreeing cell,
/// and renders the board, so any mismatch is directly reproducible
/// via `seeded_board(seed)`.
fn assert_agreement(
    seed: u64,
    board: &Board,
    side: Color,
    map_name: &str,
    got: &MoveSet,
    want: &MoveSet,
) {
    if got == want {
        return;
    }

    panic!(
        "{map_name} disagrees with engine\n\
         seed: {seed} (reproduce with seeded_board({seed}))\n\
         colour: {side:?}\n\
         only in network: {}\n\
         only in engine:  {}\n\
         board (x = Black, o = White):\n{}",
        cell_list(got, want),
        cell_list(want, got),
        render(board)
    );
}

/// Format the cells present in `a` but absent from `b` as
/// `(row, col)` pairs.
fn cell_list(a: &MoveSet, b: &MoveSet) -> String {
    let cells: Vec<String> = a
        .iter()
        .filter(|mv| !b.contains(*mv))
        .map(|mv| format!("({}, {})", mv.row(), mv.col()))
        .collect();
    if cells.is_empty() {
        "none".to_string()
    } else {
        cells.join(", ")
    }
}

/// Render the board as a 15×15 grid (`x` = Black, `o` = White).
fn render(board: &Board) -> String {
    let mut out = String::new();
    for r in 0..15u8 {
        for c in 0..15u8 {
            let mv = Move::new(r, c).expect("(r, c) is in-board");
            let ch = match board.stone_at(mv) {
                Some(Color::Black) => 'x',
                Some(Color::White) => 'o',
                None => '.',
            };
            out.push(ch);
            out.push(' ');
        }
        out.push('\n');
    }
    out
}
