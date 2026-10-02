//! Utitly functions for development support, visualization.
//!
use crate::Move;

/// Render a move history as an ASCII board, one row per line.
///
/// Each cell is printed as ` x ` (black), ` o ` (white), or ` . `
/// (empty). Black plays first, so even plies are `x` and odd plies
/// are `o`.
pub fn board_to_string(moves: Vec<Move>) -> String {
    let mut grid = [['.'; 15]; 15];
    for (ply, mv) in moves.iter().enumerate() {
        let g = if ply % 2 == 0 { 'x' } else { 'o' };
        grid[mv.row() as usize][mv.col() as usize] = g;
    }
    grid.iter()
        .map(|row| row.iter().map(|g| format!(" {g} ")).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Euclidian distance (squared) between any two positions
pub fn dist(m1: &Move, m2: &Move) -> f32 {
    let dr = m1.row() as f32 - m2.row() as f32;
    let dc = m1.col() as f32 - m2.col() as f32;
    dr * dr + dc * dc
}
