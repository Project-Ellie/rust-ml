use engine::{Color, Move, PlayError, Status};

pub trait GameBoard {
    fn play (&mut self, mv: Move) -> Result<(), PlayError>;
    fn status(&self) -> Status;
    fn to_move(&self) -> Color;
    fn stone_at(&self, mv: Move) -> Option<Color>;
    fn moves(&self) -> &[Move];
    fn name(&self) -> &'static str;
}

pub struct NaiveBoard(engine::reference::Board);
impl GameBoard for NaiveBoard {
    fn play(&mut self, mv: Move) -> Result<(), PlayError> {
        self.0.play(mv)
    }

    fn status(&self) -> Status {
        self.0.status()
    }

    fn to_move(&self) -> Color {
        self.0.to_move()
    }

    fn stone_at(&self, mv: Move) -> Option<Color> {
        self.0.stone_at(mv)
    }

    fn moves(&self) -> &[Move] {
        self.0.moves()
    }

    fn name(&self) -> &'static str {
       "naive"
    }
}

pub struct FastBoard(engine::Board);

impl GameBoard for FastBoard {
    fn play(&mut self, mv: Move) -> Result<(), PlayError> {
        self.0.play(mv)
    }

    fn status(&self) -> Status {
        self.0.status()
    }

    fn to_move(&self) -> Color {
        self.0.to_move()
    }

    fn stone_at(&self, mv: Move) -> Option<Color> {
        self.0.stone_at(mv)
    }

    fn moves(&self) -> &[Move] {
        self.0.moves()
    }

    fn name(&self) -> &'static str {
        "fast"
    }
}
#[derive(Debug, Clone, Copy)]
pub enum EngineKind { Naive, Fast }

pub fn board_for(kind: EngineKind) -> Box<dyn GameBoard> {
    match kind {
        EngineKind::Naive => Box::new(NaiveBoard(engine::reference::Board::new())),
        EngineKind::Fast => Box::new(FastBoard(engine::Board::new())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_boards_agree_through_the_trait() {
        let mut boards: Vec<Box<dyn GameBoard>> =
            vec![board_for(EngineKind::Naive), board_for(EngineKind::Fast)];
        let script = [(7u8, 7u8), (7, 8), (8, 7), (8, 8), (6, 7)];
        for (r, c) in script {
            let mv = Move::new(r, c).unwrap();
            for b in &mut boards {
                assert!(b.play(mv).is_ok());
            }
            let [a, b] = &mut boards[..] else {
                unreachable!()
            };
            assert_eq!(a.status(), b.status());
            assert_eq!(a.stone_at(mv), b.stone_at(mv));
        }
    }

    #[test]
    fn playerror_travels_through_the_trait() {
        let mut b = board_for(EngineKind::Fast);
        let mv = Move::new(7,7).unwrap();
        b.play(mv).unwrap();
        assert_eq!(b.play(mv), Err(PlayError::Occupied));
    }
}

