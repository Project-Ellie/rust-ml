# Chapter 3 — Rendering

Goal: a **pure function** turns any `&dyn GameBoard` into 19 lines of
text. No I/O, no escape codes — that's why it's trivially testable.
The scrolling UI (chapter 4) and the alternate-screen UI (chapter 5)
both consume the same lines.

## The layout

```text
     0  1  2  3  4  5  6  7  8  9 10 11 12 13 14      <- ruler
   +---------------------------------------------+   <- border
 0 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 0   <- row 0
 1 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 1 
 ...
14 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 14
   +---------------------------------------------+   <- border
     0  1  2  3  4  5  6  7  8  9 10 11 12 13 14      <- ruler again
```

- Every field is exactly **3 characters**: ` . `, ` x `, ` o `.
- Column numbers centered in 3 characters: Rust's `{:^3}` format.
  Two-digit numbers get their padding on the right (`10 `) — close
  enough, and the columns stay perfectly aligned.
- The ruler appears top **and** bottom, so after 15 rows of scanning
  you never scroll your eyes back up. Row numbers repeat down both
  sides for the same reason — `7 7` stops being a guess.
- A frame of `+`, `-`, `|` marks the board's edges: the dashes are
  contiguous, and each `+` sits directly under a `|`. Plain ASCII on
  purpose — no box-drawing characters to trip the byte offsets below.
- Every line is **53** characters: 4 bytes of row label and bar, 45 of
  cells, 4 of bar and row label.
- Column `c` of a row starts at byte `4 + 3*c` (so `7 7` is 25..28),
  and row `r` is `lines[2 + r]` — ruler, border, then the rows.

## Contract

`crates/cli/src/render.rs`:

```rust
/// The board as 19 lines of exactly 53 characters each:
/// ruler, border, 15 framed rows, border, ruler.
pub fn render_lines(board: &dyn GameBoard) -> Vec<String>;

/// One line of context for below the board.
pub fn status_line(board: &dyn GameBoard) -> String;
```

`status_line` examples:

```text
engine: fast · move 12 · Black to move
Black wins! — type 'new' to start over
Draw! — type 'new' to start over
```

## Rust toolbox

**`format!("{c:^3}")`** — the fill/alignment specifier: `^` centers,
`3` is the minimum width. Same family as `{:>2}` (right-align) and
`{:<2}` (left-align), which the row labels use: `{row:>2} |` on the
left, `| {row:<2}` on the right — so `14` occupies the same bytes as
` 0 ` and the frame stays straight.

**`&dyn GameBoard` as a parameter.** A shared reference to a trait
object — read-only access, zero ownership claims. This is the
borrow-checker-friendly way to say "I only look at the board".

**`expect` with a message.** `Move::new(row, col)` can't fail inside
`0..15`, but it returns `Option`. `.expect("0..15 is always on
board")` documents *why* the panic is unreachable — better than a
bare `unwrap()` that future readers must re-derive.

## TDD checklist

1. empty board → 19 lines, all 53 chars wide, rulers and borders equal
2. empty board → row `r` is `{r:>2} |` + `" . "` × 15 + `| {r:<2}`
3. after `play((7,7))` as Black: row 7 shows ` x ` at bytes 25..28
4. a White reply at (7,8) shows ` o ` at bytes 28..31
5. `status_line`: ongoing / won / draw variants

Write test 1, watch it fail (`render_lines` doesn't exist), implement
the minimum, continue.

## Pitfalls

- Slice strings by **bytes** (`&row[25..28]`) only because everything
  here is ASCII. Once a styled line or a Unicode glyph enters the
  picture (chapter 6), switch to `chars()`-based checks or you'll
  panic on a non-UTF-8 boundary.
- Keep `render_lines` pure. The urge to `println!` inside it will
  come; resist — purity is what lets chapter 5 redraw the whole
  screen 60 times a second if it wants to.

## Done when

All five tests green, on both engines (the agreement test from
chapter 2 already guarantees identical `stone_at`, so one engine per
render test is enough).

Next: [Chapter 4 — The game loop](04-the-game-loop.md)

---

## Solution

### `crates/cli/src/render.rs`

```rust
//! Board rendering — pure functions, no I/O.

use crate::board::GameBoard;
use engine::{Color, Move, Status};

/// One cell, exactly three characters wide.
fn cell(glyph: char) -> String {
    format!(" {glyph} ")
}

fn glyph(stone: Option<Color>) -> char {
    match stone {
        None => '.',
        Some(Color::Black) => 'x',
        Some(Color::White) => 'o',
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
    use crate::board::{board_for, EngineKind};

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

        assert_eq!(status_line(&*board), "Black wins! — type 'new' to start over");
    }
}
```

### `crates/cli/src/main.rs` — register the module

```rust
mod board;
mod render;

use board::{board_for, EngineKind};

fn main() {
    let kind = parse_engine_arg().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });
    let board = board_for(kind);
    for line in render::render_lines(&*board) {
        println!("{line}");
    }
    println!("{}", render::status_line(&*board));
}

// parse_engine_arg unchanged from chapter 2
```

(Keep `parse_engine_arg` from chapter 2 verbatim; the elision above
is only to avoid repeating it in every chapter.)

Run it — a framed board with rulers on all four sides, straight from
the trait object.
