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
    let mut black = Vec::new();
    let mut white = Vec::new();

    for r in 0..15u8 {
        for c in 0..15u8 {
            let dst = (r as usize + 1) * engine::EXT + c as usize + 1;
            let mv = Move::new(r, c).expect("r, c should resolve in-board");
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
        .expect("Encoded planes must form a valid position.");

    let supplied_legal: HashSet<_> = legal.iter().copied().collect();
    let computed_legal: HashSet<_> = board.empty_moves().collect();

    debug_assert_eq!(
        supplied_legal, computed_legal,
        "Supplied legal moves must match the reconstructed board's legal moves."
    );

    board
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Board, Move};

    fn request_from_board(board: &Board) -> (Planes, Vec<Move>) {
        (engine::encode(board), board.empty_moves().collect())
    }

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

    #[test]
    #[should_panic(expected = "Encoded planes must form a valid position.")]
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
}
