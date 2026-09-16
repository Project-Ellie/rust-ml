//! `Board`: absolute colors + `to_move`, play/undo, legality, status.
//! Slice 3. See docs/13-engine-design.md, "Board".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black,
    White,
}

impl Color {
    pub fn other(self) -> Color {
        match self {
            Color::Black => Color::White,
            Color::White => Color::Black,
        }
    }
}

/// Game state. An enum, not bool flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ongoing,
    Won(Color),
    Draw,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PlayError {
    #[error("Cell is already occupied.")]
    Occupied,
    #[error("Game is already over.")]
    GameOver,
}

pub(crate) const fn idx(r: u8, c: u8) -> usize {
    r as usize * 16 + c as usize
}

/// 256 bits, of which 240 are addressable cells (15 rows x 16 stride)
/// and 225 are real board cells. Column 15 of every row plus bits
/// 240-255 are padding - the invariant says they are ALWAYS zero.
///
/// NewType over `[u64; 4]`: `Copy` (32 bytes - cheaper than a
/// reference), and the wrapper keeps bitboard math from mixing with
/// plain ntegers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Bitboard(pub(crate) [u64; 4]);

impl Bitboard {
    pub(crate) const EMPTY: Bitboard = Bitboard([0; 4]);

    pub(crate) fn with_bit(self, idx: usize) -> Bitboard {
        let mut w = self.0;
        w[idx / 64] |= 1 << (idx % 64);
        Bitboard(w)
    }

    pub(crate) fn without_bit(self, idx: usize) -> Bitboard {
        let mut w = self.0;
        w[idx / 64] &= !(1 << (idx % 64));
        Bitboard(w)
    }

    pub(crate) fn test(&self, idx: usize) -> bool {
        self.0[idx / 64] & (1 << idx % 64) != 0
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.0 == [0; 4]
    }

    pub(crate) fn count(&self) -> u32 {
        self.0.iter().map(|w| w.count_ones()).sum()
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_test_clear_on_corners_and_edges() {
        let bb = Bitboard::EMPTY;
        assert!(bb.is_zero());

        let bb = bb.with_bit(idx(0, 0));
        assert!(bb.test(idx(0, 0)));
        assert!(!bb.test(idx(1, 0)));
        assert_eq!(bb.count(), 1);

        let bb = bb.
            with_bit(idx(0, 14)).
            with_bit(idx(14, 0)).
            with_bit(idx(14, 14));
        assert_eq!(bb.count(), 4);

        let bb = bb.without_bit(idx(0, 0));
        assert!(!bb.test(idx(0, 0)));
        assert_eq!(bb.count(), 3);
    }

    #[test]
    fn bits_crossing_a_word_boundary_land_correctly() {
        let bb = Bitboard::EMPTY.
            with_bit(idx(3, 14)).
            with_bit(idx(4, 0));
        assert!(bb.test(idx(3, 14)));
        assert!(bb.test(idx(4, 0)));
        assert!(!bb.test(idx(4, 1)));
        assert_eq!(bb.count(), 2);
    }
}