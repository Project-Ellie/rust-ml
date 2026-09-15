# Chapter 6 — Polish

Optional, short, and each item stands alone. Three upgrades, in
order of usefulness:

1. **Row labels** — the board numbers its rows on the left and right,
   mirroring the column rulers. (You asked for top and bottom; after
   typing `7 7` blind a few times you'll want these.)
2. **Last-move marker** — the most recent stone shows as `[x]`
   instead of ` x `, so you always see what just happened.
3. **Colors** — dim dots, bright stones. ANSI color via crossterm's
   `Stylize`, applied only in the alternate-screen draw path.

## Rust toolbox

**One renderer, two paints.** Items 2 and 3 tempt you into two
near-identical `render_lines` copies (plain for tests and `--plain`,
styled for the alt screen). Resist with a closure parameter:

```rust
fn render_with(board: &dyn GameBoard, paint: impl Fn(char, bool) -> String)
    -> Vec<String>;
```

`paint` receives the cell glyph and the is-this-the-last-move flag,
and returns 3 visible characters (plus invisible ANSI in the styled
case). `render_lines` and `render_styled` become one-liners over it.
`impl Fn(...)` in argument position is a generic in disguise — no
`dyn`, no allocation, fully inlined.

**Why tests keep the plain renderer.** ANSI escape codes are bytes:
a styled line is no longer 51 bytes and `&line[24..27]` is no longer
a cell. The plain path keeps honest, sliceable strings; the styled
path is verified by eye (or with snapshot tests, later).

## Exercise 1+2 — labels and marker

Change `render_lines` so:

- each row is `{row:>2} ` + 45 chars of cells + ` {row:<2}`
- the ruler gains 3 spaces of padding on each side
- every line is now **51** characters
- the cell of `board.moves().last()` renders as `[x]` / `[o]`

Update the chapter-3 tests: column `c` now lives at bytes
`3 + 3*c .. 3 + 3*c + 3`, and the *last played* stone wears brackets.

## Exercise 3 — colors

Add `render_styled` (same layout, styled cells), switch `draw` in
`ui.rs` to it, leave `run_plain` on the plain renderer — piping
`--plain` output into a file should stay ANSI-free.

## Done when

The alt-screen UI shows labeled rows, a bracketed last move, and
colored stones; `cargo test -p cli` is green; `--plain` output is
still plain ASCII.

Back to: [Side quest home](README.md) · then continue tutorial 13,
[slice 4](../13-engine-tutorial/04-win-detection.md)

---

## Solutions

### `crates/cli/src/render.rs` — final version (all three items)

```rust
//! Board rendering — pure functions, no I/O.

use crate::board::GameBoard;
use crossterm::style::Stylize;
use engine::{Color, Move, Status};

fn glyph(stone: Option<Color>) -> char {
    match stone {
        None => '.',
        Some(Color::Black) => 'x',
        Some(Color::White) => 'o',
    }
}

/// Shared layout: 51-char lines — row label, 15 three-char cells,
/// row label — framed by padded rulers. `paint(glyph, is_last_move)`
/// produces one cell's 3 visible characters.
fn render_with(board: &dyn GameBoard, paint: impl Fn(char, bool) -> String) -> Vec<String> {
    let ruler: String = (0..15).map(|c| format!("{c:^3}")).collect();
    let ruler = format!("   {ruler}   ");
    let last = board.moves().last().copied();

    let mut lines = Vec::with_capacity(17);
    lines.push(ruler.clone());
    for row in 0..15u8 {
        let mut line = format!("{row:>2} ");
        for col in 0..15u8 {
            let mv = Move::new(row, col).expect("0..15 is always on board");
            line.push_str(&paint(glyph(board.stone_at(mv)), Some(mv) == last));
        }
        line.push_str(&format!(" {row:<2}"));
        lines.push(line);
    }
    lines.push(ruler);
    lines
}

/// Plain ASCII: tests, `--plain`, piping into files.
pub fn render_lines(board: &dyn GameBoard) -> Vec<String> {
    render_with(board, |g, is_last| {
        if is_last {
            format!("[{g}]")
        } else {
            format!(" {g} ")
        }
    })
}

/// ANSI-colored: the alternate-screen UI.
pub fn render_styled(board: &dyn GameBoard) -> Vec<String> {
    render_with(board, |g, is_last| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{board_for, EngineKind};

    #[test]
    fn empty_board_is_seventeen_ruled_lines() {
        let board = board_for(EngineKind::Naive);
        let lines = render_lines(&*board);

        assert_eq!(lines.len(), 17);
        assert!(lines.iter().all(|l| l.chars().count() == 51));

        let ruler = "    0  1  2  3  4  5  6  7  8  9 10 11 12 13 14    ";
        assert_eq!(lines[0], ruler);
        assert_eq!(lines[16], ruler);

        for (row, line) in lines[1..16].iter().enumerate() {
            let expected = format!("{row:>2} {} {row:<2}", " . ".repeat(15));
            assert_eq!(line, &expected);
        }
    }

    #[test]
    fn last_move_wears_brackets() {
        let mut board = board_for(EngineKind::Fast);
        board.play(Move::new(7, 7).unwrap()).unwrap(); // x — last move
        board.play(Move::new(7, 8).unwrap()).unwrap(); // o — now last

        let lines = render_lines(&*board);
        let row7 = &lines[1 + 7]; // line 0 is the ruler
        // 3-char left gutter, then 3 bytes per column.
        assert_eq!(&row7[24..27], " x ");
        assert_eq!(&row7[27..30], "[o]");
    }

    #[test]
    fn status_line_reports_to_move_and_result() {
        let mut board = board_for(EngineKind::Naive);
        assert_eq!(
            status_line(&*board),
            "engine: naive · move 0 · Black to move"
        );

        // Black builds five on row 0; White parks on row 1.
        for (bc, wc) in [(0u8, 0u8), (1, 1), (2, 2), (3, 3)] {
            board.play(Move::new(0, bc).unwrap()).unwrap();
            board.play(Move::new(1, wc).unwrap()).unwrap();
        }
        board.play(Move::new(0, 4).unwrap()).unwrap();

        assert_eq!(
            status_line(&*board),
            "Black wins! — type 'new' to start over"
        );
    }
}
```

### `crates/cli/src/ui.rs` — one-line change in `draw`

```rust
    for line in render_styled(board) {   // was: render_lines
        queue!(stdout, Print(line), Print("\n"))?;
    }
```

(plus `use crate::render::{render_styled, status_line};` — keep
`render_lines` imported for `run_plain`.)

## Where this goes next

- **`parse` is the human's policy network.** The "read a line, return
  a move" seam is exactly where an MCTS player plugs in later — same
  shape, different brain. That's ch. 12's `gomoku play`.
- **ratatui.** When the UI grows (side panel with win rates, move
  list), raw crossterm stops scaling and a retained widget tree
  starts paying off. The `GameBoard` trait and `render_with` survive
  that migration unchanged.
