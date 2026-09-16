# Chapter 6 — Polish

Optional, short, and each item stands alone. Three upgrades, in
order of usefulness:

1. **Last-move marker** — the most recent stone shows as `[x]`
   instead of ` x `, so you always see what just happened.
2. **Colors** — dim dots, bright stones. ANSI color via crossterm's
   `Stylize`, applied only in the alternate-screen draw path.
3. **Undo** — a `u` command that takes back the last move. Both engine
   boards support it, so the trait gains one method and the UI gains one
   command; it is the only item here that touches game state rather than
   pixels. Full solution at the end.

(Row labels used to be the first item here. They moved into chapter 3
along with the frame: a board you can read without counting is not
polish, it is the minimum.)

## Rust toolbox

**One renderer, two paints.** Items 1 and 2 tempt you into two
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
a styled line is no longer 53 bytes and `&line[25..28]` is no longer
a cell. The plain path keeps honest, sliceable strings; the styled
path is verified by eye (or with snapshot tests, later).

## Exercise 1 — the last-move marker

Chapter 3 already frames and labels the board; the one thing missing
is memory of the previous move. Change `render_lines` so the cell of
`board.moves().last()` renders as `[x]` / `[o]`

That is the whole exercise. It costs no new geometry: the brackets
replace the two spaces around the glyph rather than adding to them, so
every line stays 53 characters and the chapter-3 byte offsets
(`4 + 3*c .. 7 + 3*c`) still describe the cells.

(Once you refactor to the `paint` closure below, the `cell` helper from
chapter 3 has no callers left — delete it rather than silencing the
warning.)

## Exercise 2 — colors

Add `render_styled` (same layout, styled cells), switch `draw` in
`ui.rs` to it, leave `run_plain` on the plain renderer — piping
`--plain` output into a file should stay ANSI-free.

## Exercise 3 — an undo command

Contract, in four small pieces:

1. **The trait** (`crates/cli/src/board.rs`) gains `fn undo(&mut self);`
   with a doc comment stating the precondition. Both adapters forward —
   one line each, no logic (if you write logic in an adapter, the
   abstraction is leaking).
2. **The parser** (`input.rs`) gains `Command::Undo`, matched for
   `"u" | "undo"`, and the usage string mentions it.
3. **`dispatch`** (`ui.rs`) gains one arm. This is where the
   precondition is enforced: an empty history must produce a message, not
   a panic. After a successful undo, refresh the message — the status
   line already says whose turn it is, so say what happened.
4. **Tests.** One per piece: the parser keyword, the trait path on *both*
   boards (play, undo, `stone_at` back to `None`, status `Ongoing`,
   `to_move` back to Black), the UI refusing an empty undo, and — the
   pleasant one — that the `[x]` bracket walks back to the previous
   stone, because the renderer reads `moves().last()` and undo shortens
   the history.

Three things that make this easy, and worth noticing:

- **Undo re-opens a finished game.** Both boards set `Status::Ongoing`
  when the winning move is taken back (the oracle simply replays the
  shorter history), so the UI needs no special case: `status_line`
  flips from `Black wins!` back to `Black to move` on its own.
- **`u` on an empty board is a real path**, not a theoretical one — it
  is the first command a player types after `new`.
- **The bitboard board and the oracle agree on all of it**, which is why
  the trait test can assert the same behaviour twice in a loop over
  `EngineKind`.

## Done when

The alt-screen UI shows the framed, labeled board, a bracketed last
move, and colored stones; `u` takes back the last move and says so;
`cargo test -p cli` is green; `--plain` output is still plain ASCII.

Back to: [Side quest home](README.md) · then continue tutorial 13,
[slice 4](../13-engine-tutorial/04-win-detection.md)

---

## Solutions

Every snippet below is verified against the real engine: 15 `cli` tests,
`cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`
clean, and scripted `--plain` / alternate-screen games including undo.

### `crates/cli/src/render.rs` — final version (marker + colors)

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

/// Shared layout: 53-char lines — row label, bar, 15 three-char
/// cells, bar, row label — inside a ruler/border frame.
/// `paint(glyph, is_last_move)`
/// produces one cell's 3 visible characters.
fn render_with(board: &dyn GameBoard, paint: impl Fn(char, bool) -> String) -> Vec<String> {
    let ruler: String = (0..15).map(|c| format!("{c:^3}")).collect();
    let ruler = format!("    {ruler}    ");
    let border = format!("   +{}+   ", "-".repeat(45));
    let last = board.moves().last().copied();

    let mut lines = Vec::with_capacity(19);
    lines.push(ruler.clone());
    lines.push(border.clone());
    for row in 0..15u8 {
        let mut line = format!("{row:>2} |");
        for col in 0..15u8 {
            let mv = Move::new(row, col).expect("0..15 is always on board");
            line.push_str(&paint(glyph(board.stone_at(mv)), Some(mv) == last));
        }
        line.push_str(&format!("| {row:<2}"));
        lines.push(line);
    }
    lines.push(border);
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
            let expected = format!("{row:>2} |{}| {row:<2}", " . ".repeat(15));
            assert_eq!(line, &expected);
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

### Exercise 3 solution — `undo`

The trait gains one method; everything else is a forward or one arm.

**`crates/cli/src/board.rs`** — trait and both adapters:

```rust
pub trait GameBoard {
    fn play(&mut self, mv: Move) -> Result<(), PlayError>;
    /// Undo the last move. Callers must ensure the history is non-empty:
    /// both implementations panic otherwise.
    fn undo(&mut self);
    // ... status, to_move, stone_at, moves, name unchanged
}

impl GameBoard for FastBoard {
    // ... play unchanged
    fn undo(&mut self) {
        self.0.undo()
    }
    // ... the rest unchanged
}
```

(The `NaiveBoard` block is identical apart from `self.0` being the oracle.)

**`crates/cli/src/input.rs`** — one variant, one arm, one usage hint:

```rust
#[derive(Debug)]
pub enum Command {
    Play(Move),
    Undo,
    New,
    Quit,
    Help,
}

const USAGE: &str = "usage: <row> <col> (0-14) · 'u' undoes · 'new' restarts · 'q' quits";

// inside parse(), with the other keywords:
    "u" | "undo" => return Ok(Command::Undo),
```

**`crates/cli/src/ui.rs`** — the guard and the message:

```rust
const HELP: &str =
    "enter <row> <col> to play (e.g. '7 7') · 'u' undoes · 'new' restarts · 'q' quits";

// inside dispatch(), after the Play arm:
        Ok(Command::Undo) => {
            if board.moves().is_empty() {
                Some("nothing to undo".to_string())
            } else {
                board.undo();
                Some(format!(
                    "undid the last move — {:?} to move",
                    board.to_move()
                ))
            }
        }
```

**The four tests** that pin it down:

```rust
// board.rs (cli) — the trait path, on both boards at once
#[test]
fn undo_travels_through_the_trait_on_both_boards() {
    for kind in [EngineKind::Naive, EngineKind::Fast] {
        let mut board = board_for(kind);
        let mv = Move::new(7, 7).unwrap();
        board.play(mv).unwrap();
        assert_eq!(board.moves().len(), 1);

        board.undo();
        assert_eq!(board.moves().len(), 0);
        assert_eq!(board.stone_at(mv), None);
        assert_eq!(board.status(), Status::Ongoing);
        assert_eq!(board.to_move(), Color::Black);
    }
}

// board.rs (cli) — taking back the winning move re-opens the game
#[test]
fn undo_after_a_win_reopens_the_game() {
    let mut board = board_for(EngineKind::Fast);
    let script = [
        (7, 3),
        (0, 0),
        (7, 4),
        (0, 2),
        (7, 5),
        (0, 4),
        (7, 6),
        (0, 6),
        (7, 7),
    ];
    for (r, c) in script {
        board.play(Move::new(r, c).unwrap()).unwrap();
    }
    assert_eq!(board.status(), Status::Won(Color::Black));

    board.undo();
    assert_eq!(board.status(), Status::Ongoing);
    assert!(board.moves().len() == 8);
}

// render.rs — the bracket follows the shortened history
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

// ui.rs — the empty-history path is a message, not a panic
#[test]
fn undo_with_empty_history_is_refused_not_panicked() {
    let mut board = board_for(EngineKind::Fast);
    let msg = dispatch(&mut board, EngineKind::Fast, "u").unwrap();
    assert_eq!(msg, "nothing to undo");
}
```

(`ui.rs` tests need `use engine::Move;` alongside its existing imports;
`render.rs`/`board.rs` tests already import what they use.)

## Where this goes next

- **`parse` is the human's policy network.** The "read a line, return
  a move" seam is exactly where an MCTS player plugs in later — same
  shape, different brain. That's ch. 12's `gomoku play`.
- **ratatui.** When the UI grows (side panel with win rates, move
  list), raw crossterm stops scaling and a retained widget tree
  starts paying off. The `GameBoard` trait and `render_with` survive
  that migration unchanged.
