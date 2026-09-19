//! Board rendering — pure functions, no I/O.

use crate::board::GameBoard;
use crate::opening::OpeningState;
use crossterm::style::Stylize;
use engine::{Color, Move, PlayError, Proof, Status};

fn glyph(stone: Option<Color>) -> char {
    match stone {
        None => '.',
        Some(Color::Black) => 'x',
        Some(Color::White) => 'o',
    }
}

/// Shared layout: row label, bar, 15 cells, bar, row label — inside a
/// ruler/border frame. `paint(glyph, is_last_move, move)` produces one
/// cell's visible characters; `cell_width` is used for the ruler and frame.
fn render_with(
    board: &dyn GameBoard,
    cell_width: usize,
    paint: impl Fn(char, bool, Move) -> String,
) -> Vec<String> {
    let ruler: String = (0..15).map(|c| format!("{c:^cell_width$}")).collect();
    let ruler = format!("    {ruler}    ");
    let border = format!("   +{}+   ", "-".repeat(cell_width * 15));
    let last = board.moves().last().copied();

    let mut lines = Vec::with_capacity(19);
    lines.push(ruler.clone());
    lines.push(border.clone());
    for row in 0..15u8 {
        let mut line = format!("{row:>2} |");
        for col in 0..15u8 {
            let mv = Move::new(row, col).expect("0..15 is always on board");
            line.push_str(&paint(glyph(board.stone_at(mv)), Some(mv) == last, mv));
        }
        line.push_str(&format!("| {row:<2}"));
        lines.push(line);
    }
    lines.push(border);
    lines.push(ruler);
    lines
}

/// Plain ASCII: tests, `--plain`, piping into a file.
pub fn render_lines(board: &dyn GameBoard) -> Vec<String> {
    render_with(board, 3, |g, is_last, _mv| {
        if is_last {
            format!("[{g}]")
        } else {
            format!(" {g} ")
        }
    })
}

/// ANSI-coloured: the alternate-screen UI.
pub fn render_styled(board: &dyn GameBoard) -> Vec<String> {
    render_with(board, 3, |g, is_last, _mv| {
        let styled = match g {
            '.' => ".".dark_grey(),
            'x' => "x".cyan().bold(),
            'o' => "o".yellow(),
            _ => unreachable!("glyph is one of . x o"),
        };
        if is_last {
            format!("[{styled}]")
        } else {
            format!(" {styled} ")
        }
    })
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

// -----------------------------------------------------------------------------
// Swap2 opening rendering

/// Plain rendering of the Swap2 opening position.
pub fn render_opening_lines(state: &OpeningState) -> Vec<String> {
    render_opening(state, |glyph, is_last, _mv| {
        if is_last {
            format!("[{glyph}]")
        } else {
            format!(" {glyph} ")
        }
    })
}

/// Styled rendering of the Swap2 opening position.
pub fn render_opening_styled(state: &OpeningState) -> Vec<String> {
    render_opening(state, |glyph, is_last, _mv| {
        let styled = match glyph {
            '.' => ".".dark_grey(),
            'x' => "x".cyan().bold(),
            'o' => "o".yellow(),
            _ => unreachable!("glyph is one of . x o"),
        };
        if is_last {
            format!("[{styled}]")
        } else {
            format!(" {styled} ")
        }
    })
}

fn render_opening(state: &OpeningState, paint: impl Fn(char, bool, Move) -> String) -> Vec<String> {
    let board = TemporaryOpeningBoard::from(state);
    render_with(&board, 3, paint)
}

/// A temporary `GameBoard` view of an opening position, used only for
/// rendering.
struct TemporaryOpeningBoard {
    moves: Vec<Move>,
    colors: Vec<Color>,
}

impl TemporaryOpeningBoard {
    fn from(state: &OpeningState) -> Self {
        let mut moves = Vec::with_capacity(state.placements().len());
        let mut colors = Vec::with_capacity(state.placements().len());
        for &(mv, color) in state.placements() {
            moves.push(mv);
            colors.push(color);
        }
        Self { moves, colors }
    }
}

impl GameBoard for TemporaryOpeningBoard {
    fn play(&mut self, _mv: Move) -> Result<(), PlayError> {
        unimplemented!("TemporaryOpeningBoard is render-only")
    }

    fn undo(&mut self) {
        unimplemented!("TemporaryOpeningBoard is render-only")
    }

    fn status(&self) -> Status {
        Status::Ongoing
    }

    fn to_move(&self) -> Color {
        let black = self.colors.iter().filter(|&&c| c == Color::Black).count() as u8;
        let white = self.colors.len() as u8 - black;
        if white < black {
            Color::White
        } else {
            Color::Black
        }
    }

    fn stone_at(&self, mv: Move) -> Option<Color> {
        self.moves
            .iter()
            .zip(&self.colors)
            .find(|(m, _)| **m == mv)
            .map(|(_, &c)| c)
    }

    fn moves(&self) -> &[Move] {
        &self.moves
    }

    fn name(&self) -> &'static str {
        "opening"
    }
}

// -----------------------------------------------------------------------------
// TSS proof overlay

/// Plain rendering with a verified TSS line overlaid as numbered cells.
pub fn render_lines_with_overlay(board: &dyn GameBoard, proof: &Proof) -> Vec<String> {
    let overlay = Overlay::new(proof);
    render_with(board, 4, |glyph, is_last, mv| {
        if let Some((n, side)) = overlay.get(mv) {
            return overlay_cell_plain(n, side);
        }
        plain_cell(glyph, is_last)
    })
}

/// Styled rendering with a verified TSS line overlaid as numbered cells.
pub fn render_styled_with_overlay(board: &dyn GameBoard, proof: &Proof) -> Vec<String> {
    let overlay = Overlay::new(proof);
    render_with(board, 4, |glyph, is_last, mv| {
        if let Some((n, side)) = overlay.get(mv) {
            return overlay_cell_styled(n, side, proof.winner);
        }
        let styled = match glyph {
            '.' => ".".dark_grey(),
            'x' => "x".cyan().bold(),
            'o' => "o".yellow(),
            _ => unreachable!("glyph is one of . x o"),
        };
        styled_cell(&styled, is_last)
    })
}

fn plain_cell(glyph: char, is_last: bool) -> String {
    if is_last {
        format!("[{glyph} ]")
    } else {
        format!(" {glyph}  ")
    }
}

fn styled_cell(styled: &dyn std::fmt::Display, is_last: bool) -> String {
    if is_last {
        format!("[{styled} ]")
    } else {
        format!(" {styled}  ")
    }
}

struct Overlay {
    cells: [Option<(usize, Side)>; 225],
}

impl Overlay {
    fn new(proof: &Proof) -> Self {
        let mut cells = [None; 225];
        for (i, &mv) in proof.line.iter().enumerate() {
            let side = if i % 2 == 0 {
                Side::Attacker
            } else {
                Side::Defender
            };
            cells[mv.index()] = Some((i + 1, side));
        }
        Self { cells }
    }

    fn get(&self, mv: Move) -> Option<(usize, Side)> {
        self.cells[mv.index()]
    }
}

#[derive(Debug, Clone, Copy)]
enum Side {
    Attacker,
    Defender,
}

fn overlay_cell_plain(n: usize, side: Side) -> String {
    match side {
        Side::Attacker => {
            if n < 10 {
                format!(" {n}  ")
            } else {
                format!(" {n} ")
            }
        }
        Side::Defender => {
            if n < 10 {
                format!("[{n} ]")
            } else {
                format!("[{n}]")
            }
        }
    }
}

fn overlay_cell_styled(n: usize, side: Side, winner: Color) -> String {
    let s = n.to_string();
    let color = match side {
        Side::Attacker => winner,
        Side::Defender => winner.other(),
    };
    let styled = match color {
        Color::Black => s.cyan().bold(),
        Color::White => s.yellow(),
    };
    match side {
        Side::Attacker => {
            if n < 10 {
                format!(" {styled}  ")
            } else {
                format!(" {styled} ")
            }
        }
        Side::Defender => {
            if n < 10 {
                format!("[{styled} ]")
            } else {
                format!("[{styled}]")
            }
        }
    }
}

/// Verdict text for a verified proof overlay.
pub fn overlay_verdict(proof: &Proof) -> String {
    format!(
        "certified forced win for {:?} in {} ply",
        proof.winner,
        proof.line.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{EngineKind, board_for};
    use crate::opening::OpeningState;

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
    fn last_move_wears_brackets() {
        let mut board = board_for(EngineKind::Fast);
        board.play(Move::new(7, 7).unwrap()).unwrap(); // x — last move
        board.play(Move::new(7, 8).unwrap()).unwrap(); // o — now last

        let lines = render_lines(&*board);
        let row7 = &lines[2 + 7]; // line 0 ruler, line 1 border
        // 4-char gutter, then 3 bytes per column.
        assert_eq!(&row7[25..28], " x ");
        assert_eq!(&row7[28..31], "[o]");
    }

    #[test]
    fn brackets_follow_an_undo() {
        let mut board = board_for(EngineKind::Fast);
        board.play(Move::new(7, 7).unwrap()).unwrap();
        board.play(Move::new(7, 8).unwrap()).unwrap();
        board.undo();

        let lines = render_lines(&*board);
        let row7 = &lines[2 + 7];
        assert_eq!(&row7[25..28], "[x]"); // the bracket walks back
        assert_eq!(&row7[28..31], " . ");
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

    #[test]
    fn opening_render_shows_placed_colors() {
        let mut state = OpeningState::new();
        state.place(Move::new(7, 7).unwrap()).unwrap();
        state.place(Move::new(6, 6).unwrap()).unwrap();
        state.place(Move::new(5, 5).unwrap()).unwrap();

        let lines = render_opening_lines(&state);
        // The last placement is White at (5,5), so that cell wears brackets.
        let row7 = &lines[2 + 7];
        assert_eq!(&row7[25..28], " x ");
        let row6 = &lines[2 + 6];
        assert_eq!(&row6[22..25], " x ");
        let row5 = &lines[2 + 5];
        assert_eq!(&row5[19..22], "[o]");
    }

    #[test]
    fn tss_overlay_numbers_attack_and_defence() {
        let mut board = board_for(EngineKind::Fast);
        // Build a VCF win-in-3 from the engine TSS tests.
        let script = [
            (7, 4),
            (7, 3),
            (7, 5),
            (0, 0),
            (7, 6),
            (0, 2),
            (6, 6),
            (0, 4),
            (8, 8),
            (0, 6),
        ];
        for &(r, c) in &script {
            board.play(Move::new(r, c).unwrap()).unwrap();
        }

        let proof = engine::prove_forced_win(
            board.engine_board().unwrap(),
            board.to_move(),
            engine::SearchBudget {
                max_nodes: 200_000,
                max_depth: 16,
            },
        )
        .expect("test position has a known forced win");
        assert!(engine::verify_line(board.engine_board().unwrap(), &proof));

        let lines = render_lines_with_overlay(&*board, &proof);
        assert_eq!(lines.len(), 19);
        // Each cell is 4 visible characters wide, so the whole line is wider.
        assert!(lines.iter().all(|l| l.chars().count() == 4 + 60 + 4));

        // The first attacking move should be visible as a single-digit number.
        let mv1 = proof.line[0];
        let row = &lines[2 + mv1.row() as usize];
        let start = 4 + mv1.col() as usize * 4;
        let cell = &row[start..start + 4];
        assert!(
            cell.trim().contains('1'),
            "attacking cell should show 1, got {cell:?}"
        );
    }
}
