# Chapter 3 — Rendering

Goal: a **pure function** turns any `&dyn GameBoard` into 17 lines of
text. No I/O, no escape codes — that's why it's trivially testable.
The scrolling UI (chapter 4) and the alternate-screen UI (chapter 5)
both consume the same lines.

## The layout

```text
 0  1  2  3  4  5  6  7  8  9 10 11 12 13 14      <- ruler, 45 chars
 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .      <- 15 board rows...
 ...
 0  1  2  3  4  5  6  7  8  9 10 11 12 13 14      <- ruler again
```

- Every field is exactly **3 characters**: ` . `, ` x `, ` o `.
- Column numbers centered in 3 characters: Rust's `{:^3}` format.
  Two-digit numbers get their padding on the right (`10 `) — close
  enough, and the columns stay perfectly aligned.
- The ruler appears top **and** bottom, so after 15 rows of scanning
  you never scroll your eyes back up.
- Column `c` of a row occupies bytes `3*c .. 3*c+3` — that
  arithmetic is what the tests assert.

## Contract

`crates/cli/src/render.rs`:

```rust
/// The board as 17 lines of exactly 45 characters each:
/// ruler, 15 rows, ruler.
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
`{:<2}` (left-align), which chapter 6 uses for row labels.

**`&dyn GameBoard` as a parameter.** A shared reference to a trait
object — read-only access, zero ownership claims. This is the
borrow-checker-friendly way to say "I only look at the board".

**`expect` with a message.** `Move::new(row, col)` can't fail inside
`0..15`, but it returns `Option`. `.expect("0..15 is always on
board")` documents *why* the panic is unreachable — better than a
bare `unwrap()` that future readers must re-derive.

## TDD checklist

1. empty board → 17 lines, all 45 chars wide, rulers equal
2. empty board → every board row is `" . "` × 15
3. after `play((7,7))` as Black: row 7 shows ` x ` at bytes 21..24
4. a White reply at (7,8) shows ` o ` at bytes 24..27
5. `status_line`: ongoing / won / draw variants

Write test 1, watch it fail (`render_lines` doesn't exist), implement
the minimum, continue.

## Pitfalls

- Slice strings by **bytes** (`&row[21..24]`) only because everything
  here is ASCII. The moment chapter 6 adds Unicode, switch to
  `chars()`-based checks or you'll panic on a non-UTF-8 boundary.
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

/// The board as 17 lines of exactly 45 characters each:
/// column ruler, 15 rows, column ruler.
pub fn render_lines(board: &dyn GameBoard) -> Vec<String> {
    let ruler: String = (0..15).map(|c| format!("{c:^3}")).collect();
    let mut lines = Vec::with_capacity(17);
    lines.push(ruler.clone());
    for row in 0..15u8 {
        let mut line = String::with_capacity(45);
        for col in 0..15u8 {
            let mv = Move::new(row, col).expect("0..15 is always on board");
            line.push_str(&cell(glyph(board.stone_at(mv))));
        }
        lines.push(line);
    }
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
    fn empty_board_is_seventeen_ruled_lines() {
        let board = board_for(EngineKind::Naive);
        let lines = render_lines(&*board);

        assert_eq!(lines.len(), 17);
        assert!(lines.iter().all(|l| l.chars().count() == 45));

        let ruler = " 0  1  2  3  4  5  6  7  8  9 10 11 12 13 14 ";
        assert_eq!(lines[0], ruler);
        assert_eq!(lines[16], ruler);

        let empty_row = " . ".repeat(15);
        for row in &lines[1..16] {
            assert_eq!(row, &empty_row);
        }
    }

    #[test]
    fn black_stone_lands_in_its_cell() {
        let mut board = board_for(EngineKind::Fast);
        board.play(Move::new(7, 7).unwrap()).unwrap();

        let lines = render_lines(&*board);
        let row7 = &lines[1 + 7]; // line 0 is the ruler
        assert_eq!(&row7[21..24], " x ");
    }

    #[test]
    fn white_reply_lands_next_to_it() {
        let mut board = board_for(EngineKind::Fast);
        board.play(Move::new(7, 7).unwrap()).unwrap();
        board.play(Move::new(7, 8).unwrap()).unwrap();

        let lines = render_lines(&*board);
        let row7 = &lines[1 + 7];
        assert_eq!(&row7[21..24], " x ");
        assert_eq!(&row7[24..27], " o ");
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

Run it — an empty board with rulers, straight from the trait object.
