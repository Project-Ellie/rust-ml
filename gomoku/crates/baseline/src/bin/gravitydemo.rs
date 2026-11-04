//! Print the normalized gravity map for a small opening shape.

use baseline::gravity::gravity_map;
use engine::{Board, Move};
fn main() {
    let mut board = Board::new();
    board.play(Move::new(7, 7).unwrap()).unwrap();
    board.play(Move::new(6, 6).unwrap()).unwrap();
    board.play(Move::new(8, 8).unwrap()).unwrap();

    let planes = engine::encode(&board);
    let map = gravity_map(&planes);

    for row in &map {
        for &v in row {
            print!("{:>5.2} ", v);
        }
        println!();
    }
}
