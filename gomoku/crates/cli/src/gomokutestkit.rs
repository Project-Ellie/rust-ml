use engine::Move;

#[cfg(test)]
pub fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}