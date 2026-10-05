//! A representation of a gomoku board ready for pattern processing
//!
use engine::{Board, Color, Move};

/// Padding radious around the 15x15 board
pub const PAD: usize = 5;

/// Padded plane side length = 15 + 2 * PAD
pub const PADDED: usize = 25;

/// Two channel padded input for side `s`
pub struct Planes {
    /// the stones of the player about to move
    pub stones: [[f32; PADDED]; PADDED],
    /// the opponents stone
    pub blocked: [[f32; PADDED]; PADDED],
}

/// Build the two padded input channels for side `s`.
///
/// The border is treated as blocked, not empty. This makes edge
/// behaviour fall out of the pattern kernels: a required-empty cell
/// that lies off the board is not empty, so a three hugging the edge
/// is correctly rejected as an open three.
pub fn planes(b: &Board, s: Color) -> Planes {
    let mut out = Planes {
        stones: [[0.0; PADDED]; PADDED],
        blocked: [[1.0; PADDED]; PADDED],
    };

    for r in 0..15u8 {
        for c in 0..15u8 {
            let pr = (r as usize) + PAD;
            let pc = (c as usize) + PAD;
            let mv = Move::new(r, c).expect("Move must be legal");
            match b.stone_at(mv) {
                Some(color) if color == s => {
                    out.stones[pr][pc] = 1.0;
                    out.blocked[pr][pc] = 0.0;
                }
                Some(_) => {
                    out.stones[pr][pc] = 0.0;
                    out.blocked[pr][pc] = 1.0;
                }
                None => {
                    out.stones[pr][pc] = 0.0;
                    out.blocked[pr][pc] = 0.0;
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Board, Color, Move};

    const BORDER_CELLS: usize = PADDED * PADDED - 15 * 15;
    fn count(v: f32, plane: &[[f32; PADDED]; PADDED]) -> usize {
        plane.iter().flatten().filter(|&&x| x == v).count()
    }
    #[test]
    fn empty_board_has_zero_stones_and_blocked_border_only() {
        let b = Board::new();
        let p = planes(&b, Color::Black);

        assert_eq!(count(1.0, &p.stones), 0, "No stones on empty board");
        assert_eq!(
            count(0.0, &p.blocked),
            15 * 15,
            "in-board opponent's cells empty"
        );
        assert_eq!(count(1.0, &p.blocked), BORDER_CELLS, "borders are blocked.")
    }

    #[test]
    fn stones_are_placed_for_the_requested_side() {
        let mut b = Board::new();
        b.play(Move::new(7, 7).unwrap()).unwrap();
        b.play(Move::new(6, 6).unwrap()).unwrap();
        b.play(Move::new(8, 8).unwrap()).unwrap();

        let black = planes(&b, Color::Black);
        let white = planes(&b, Color::White);

        assert_eq!(black.stones[7 + PAD][7 + PAD], 1.0);
        assert_eq!(black.stones[8 + PAD][8 + PAD], 1.0);
        assert_eq!(
            black.stones[6 + PAD][6 + PAD],
            0.0,
            "opponent stone is not own"
        );

        assert_eq!(white.stones[6 + PAD][6 + PAD], 1.0);
        assert_eq!(white.stones[7 + PAD][7 + PAD], 0.0);

        // Black channel sees the white stone as blocked, not as a stone.
        assert_eq!(black.blocked[6 + PAD][6 + PAD], 1.0);
        assert_eq!(black.blocked[7 + PAD][7 + PAD], 0.0);
    }

    #[test]
    fn corner_and_edge_stones_are_mapped_correctly() {
        let mut b = Board::new();
        b.play(Move::new(0, 0).unwrap()).unwrap();
        b.play(Move::new(0, 14).unwrap()).unwrap();
        b.play(Move::new(14, 0).unwrap()).unwrap();
        b.play(Move::new(14, 14).unwrap()).unwrap();
        b.play(Move::new(7, 14).unwrap()).unwrap();

        let p = planes(&b, Color::Black);
        assert_eq!(p.stones[PAD][PAD], 1.0);
        assert_eq!(p.stones[14 + PAD][PAD], 1.0);
        assert_eq!(p.stones[7 + PAD][14 + PAD], 1.0);
        assert_eq!(p.stones[PAD][14 + PAD], 0.0, "white stone is not black");
        assert_eq!(p.blocked[PAD][14 + PAD], 1.0, "white stone is blocked");
    }

    #[test]
    fn border_ring_is_exactly_five_cells_wide() {
        let b = Board::new();
        let p = planes(&b, Color::Black);

        // Inside the 15×15 active region blocked is 0; outside it is 1.
        for (r, row) in p.blocked.iter().enumerate() {
            for (c, &v) in row.iter().enumerate() {
                let in_board = (PAD..PADDED - PAD).contains(&r) && (PAD..PADDED - PAD).contains(&c);
                if in_board {
                    assert_eq!(v, 0.0, "({r},{c}) must be empty");
                } else {
                    assert_eq!(v, 1.0, "({r},{c}) must be border");
                }
            }
        }
    }
}
