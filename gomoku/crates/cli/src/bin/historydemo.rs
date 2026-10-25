//! Chapter-01 capstone: a scripted walk through the variation tree.
//!
//! Plays a short main line, branches, navigates back, chooses a variation,
//! extends it, and finally returns to the root. Prints the board, ply, and
//! variation path after each step.
//!
//! Run with:
//!
//! ```text
//! cargo run -p cli --bin historydemo
//! ```

use cli::history::{ForwardOutcome, History, PlayOutcome};
use engine::{Board, Color, Move};

fn main() {
    println!("variation-tree demo");
    println!("=================================\n");

    let mut history = History::new();
    print_step(&history, "start: empty board, root cursor");

    // Six-move main line.
    let main_line = [h8(), h7(), i8(), i7(), j8(), j7()];
    for mv in main_line {
        match history.play(mv) {
            Ok(PlayOutcome::Moved(_)) => {}
            other => panic!("unexpected play outcome: {other:?}"),
        }
    }
    print_step(&history, "main line: h8 h7 i8 i7 j8 j7");

    // Rewind three plies.
    let backed = history.back(3);
    assert_eq!(backed, 3);
    print_step(&history, "back 3");

    // Branch to a different move.
    let branch = h9();
    history.play(branch).unwrap();
    print_step(&history, "branch: h9");

    // Return to the branch point.
    history.back(1);
    print_step(&history, "back to branch point");

    // Forward reaches the branch point and asks for a choice.
    match history.forward(1) {
        ForwardOutcome::NeedsChoice(ref choices) => {
            print_step(&history, "forward: NeedsChoice");
            println!(
                "    choices: {}\n",
                choices
                    .iter()
                    .map(|&m| notation(m))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        other => panic!("expected NeedsChoice, got {other:?}"),
    }

    // Choose the second variation (h9).
    let chosen = history.var(1).unwrap();
    print_step(&history, &format!("var 1 -> {}", notation(chosen)));

    // Extend the branch.
    let extension = i9();
    history.play(extension).unwrap();
    print_step(&history, "extend: i9");

    // Back to the root.
    let to_root = history.back(100);
    print_step(&history, &format!("back {to_root} to root"));
}

fn print_step(history: &History, label: &str) {
    println!("{label}");
    println!("ply {}  path: {}", history.ply(), path_string(history));
    println!("{}", render_board(&history.board()));
    println!();
}

fn path_string(history: &History) -> String {
    let board = history.board();
    // The cursor path is the move history on a replayed board. For a root
    // built from an empty board this is exactly the path we navigated.
    if board.moves().is_empty() {
        "(root)".to_string()
    } else {
        board
            .moves()
            .iter()
            .map(|&m| notation(m))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn render_board(board: &Board) -> String {
    let mut lines = Vec::new();
    lines.push("    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o".to_string());
    for r in 0..15u8 {
        let mut row = format!("{:2}  ", r + 1);
        for c in 0..15u8 {
            let ch = match board.stone_at(Move::new(r, c).unwrap()) {
                Some(Color::Black) => 'X',
                Some(Color::White) => 'O',
                None => '.',
            };
            row.push(ch);
            row.push_str("  ");
        }
        lines.push(row);
    }
    lines.join("\n")
}

fn notation(mv: Move) -> String {
    let col = (b'a' + mv.col()) as char;
    let row = mv.row() + 1;
    format!("{col}{row}")
}

fn h8() -> Move {
    Move::new(7, 7).unwrap()
}
fn h7() -> Move {
    Move::new(6, 7).unwrap()
}
fn i8() -> Move {
    Move::new(7, 8).unwrap()
}
fn i7() -> Move {
    Move::new(6, 8).unwrap()
}
fn j8() -> Move {
    Move::new(7, 9).unwrap()
}
fn j7() -> Move {
    Move::new(6, 9).unwrap()
}
fn h9() -> Move {
    Move::new(8, 7).unwrap()
}
fn i9() -> Move {
    Move::new(8, 8).unwrap()
}
