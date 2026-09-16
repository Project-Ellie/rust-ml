//! Board rendering - pure functions, no I/O

use crate::board::GameBoard;
use engine::{Board, Color, Move, Status};

fn cell(glyph: char) -> String {
    format!(" {glyph} ")
}

fn glyph(stone: Option<Color>) -> char {
    match stone {
        None => '.',
        Some(Color::White) => 'o',
        Some(Color::Black) => 'x',
    }
}

/// The board as 19 lines of exactly 53 characters each:
/// ruler, border, 15 labeled rows, border, ruler.
pub fn render_lines(board: &dyn GameBoard) -> Vec<String> {
    let ruler: String = (0..15).map(|c| format!("{c:^3}")).collect();
    let ruler = format!("    {ruler}    "); // 4 + 45 + 4
    let border = format!("   +{}+   ", "-".repeat(45)); // '+' under the '|'

    let mut lines = Vec::with_capacity(19);
    lines.push(ruler.clone());
    lines.push(border.clone());
    for row in 0..15u8 {
        let mut line = format!("{row:>2} |");
        for col in 0..15u8 {
            let mv = Move::new(row, col).expect("0..15 is always on board");
            line.push_str(&cell(glyph(board.stone_at(mv))));
        }
        line.push_str(&format!("| {row:<2}"));
        lines.push(line);
    }
    lines.push(border);
    lines.push(ruler);
    lines
}

/// One line of context for below the board.
pub fn status_line(board: &dyn GameBoard) -> String {
    match board.status() {
        Status::Ongoing => format!(
            "engine: {} · move {} · {:?} to move",
            board.name(),
            board.moves().len(),
            board.to_move()
        ),
        Status::Won(color) => format!("{color:?} wins! — type 'new' to start over"),
        Status::Draw => "Draw! — type 'new' to start over".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{EngineKind, board_for};

    #[test]
    fn empty_board_is_ruled_and_framed() {
        let board = board_for(EngineKind::Naive);
        let lines = render_lines(&*board);

        assert_eq!(lines.len(), 19);
        assert!(lines.iter().all(|l| l.chars().count() == 53));

        let ruler = "     0  1  2  3  4  5  6  7  8  9 10 11 12 13 14     ";
        let border = "   +---------------------------------------------+   ";
        assert_eq!(lines[0], ruler);
        assert_eq!(lines[1], border);
        assert_eq!(lines[17], border);
        assert_eq!(lines[18], ruler);

        for (row, line) in lines[2..17].iter().enumerate() {
            assert_eq!(line, &format!("{row:>2} |{}| {row:<2}", " . ".repeat(15)));
        }
    }

    #[test]
    fn black_stone_lands_in_its_cell() {
        let mut board = board_for(EngineKind::Fast);
        board.play(Move::new(7, 7).unwrap()).unwrap();

        let lines = render_lines(&*board);
        let row7 = &lines[2 + 7]; // line 0 ruler, line 1 border
        assert_eq!(&row7[25..28], " x ");
    }

    #[test]
    fn white_reply_lands_next_to_it() {
        let mut board = board_for(EngineKind::Fast);
        board.play(Move::new(7, 7).unwrap()).unwrap();
        board.play(Move::new(7, 8).unwrap()).unwrap();

        let lines = render_lines(&*board);
        let row7 = &lines[2 + 7];
        assert_eq!(&row7[25..28], " x ");
        assert_eq!(&row7[28..31], " o ");
    }

    #[test]
    fn status_line_reports_to_move_and_result() {
        let mut board = board_for(EngineKind::Naive);
        assert_eq!(
            status_line(&*board),
            "engine: naive · move 0 · Black to move"
        );

        // Black wins with five on row 0; White parks on row 1.
        for (br, bc, wr, wc) in [(0, 0, 1, 0), (0, 1, 1, 1), (0, 2, 1, 2), (0, 3, 1, 3)] {
            board.play(Move::new(br, bc).unwrap()).unwrap();
            board.play(Move::new(wr, wc).unwrap()).unwrap();
        }
        board.play(Move::new(0, 4).unwrap()).unwrap();

        assert_eq!(
            status_line(&*board),
            "Black wins! — type 'new' to start over"
        );
    }
}
