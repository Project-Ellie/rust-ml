//! Small text-formatting helpers shared by the demo binaries.
//!
//! These are intentionally plain `String`-returning functions so they can
//! be used by both the unit tests and the demo binaries without pulling
//! in any terminal dependencies.

use engine::{Board, Color, Move};

/// Render a 15×15 board as compact ASCII.
///
/// Black stones are shown as `X`, White stones as `O`, empty cells as `.`.
/// The optional `last_move` is marked with lowercase `x`/`o`.
#[must_use]
pub fn render_board(board: &Board, last_move: Option<Move>) -> String {
    let mut lines = Vec::new();
    lines.push("    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o".to_string());
    for r in 0..15u8 {
        let mut row = format!("{:2}  ", r + 1);
        for c in 0..15u8 {
            let mv = Move::new(r, c).expect("board coordinates are valid");
            let is_last = last_move == Some(mv);
            let ch = match board.stone_at(mv) {
                Some(Color::Black) if is_last => 'x',
                Some(Color::White) if is_last => 'o',
                Some(Color::Black) => 'X',
                Some(Color::White) => 'O',
                None => '.',
            };
            row.push(ch);
            row.push_str("  ");
        }
        lines.push(row);
    }
    lines.join("\n")
}

/// Format a move as algebraic notation (`h8`).
///
/// The canonical implementation is [`crate::notation::algebraic`]. This
/// helper stays so the call sites that predate the notation module keep
/// working unchanged.
#[must_use]
pub fn format_move(mv: Move) -> String {
    crate::notation::algebraic(mv)
}
