//! `MoveSet`: public set-of-moves bitset, logical stride-15 indexing.
//! Slice 3. Deliberately separate from the internal stride-16 `Bitboard`.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Move(u8);

impl Move {
    pub fn new(row: u8, col: u8) -> Option<Move> {
        if row < 15 && col < 15 {
            Some(Move(row * 15 + col))
        } else {
            None
        }
    }

    pub fn row(self) -> u8 {
        self.0 / 15
    }

    pub fn col(self) -> u8 {
        self.0 % 15
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_off_board_cells() {
        assert_eq!(Move::new(0, 15), None);
        assert_eq!(Move::new(15, 0), None);
        assert_eq!(Move::new(7, 3), Some(Move(108)));
    }

    #[test]
    fn index_row_col_are_consistent() {
        let mv = Move::new(7, 3).unwrap();
        assert_eq!(mv.index(), 7 * 15 + 3); // 108
        assert_eq!(mv.row(), 7);
        assert_eq!(mv.col(), 3);
    }
}
