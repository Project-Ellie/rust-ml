# Chapter 5 — The alternate screen

The "double cool" chapter. The board stops scrolling and starts
behaving like a real terminal application: display area on top,
command line pinned at the bottom, full redraw per move — like the
window you're reading this in.

## The trick, in three ideas

1. **Alternate screen.** Terminals keep a second screen buffer
   (what `vim`, `less`, `htop` use). Enter it, and your shell history
   is preserved underneath; leave it, and the user is back where they
   started. Entering/leaving is two escape sequences — crossterm
   wraps them in `EnterAlternateScreen` / `LeaveAlternateScreen`.
2. **Cursor addressing.** Instead of printing line after line (which
   scrolls at the bottom edge), you *teleport* the cursor:
   `MoveTo(0, 0)`, print the frame, `MoveTo(0, 22)`, print the
   prompt. Nothing ever reaches the bottom edge; nothing scrolls.
3. **RAII guard.** The alternate screen must be left on *every* exit
   path — early return, `Err`, even panic. A struct whose `Drop`
   restores the terminal makes that automatic. This is the single
   most important pattern in terminal programming; skip it and one
   panic leaves your user's shell inside a dead screen buffer.

## Rust toolbox

**`Drop` as a scope guard.** Rust runs `Drop` when the guard value
goes out of scope — normally, via `?`-propagated error, or during
panic unwinding. One `let _screen = AlternateScreen::enter()?;` and
cleanup is handled for the rest of the function, forever. You have
used this all along (`File`, `MutexGuard`); writing your own is the
rite of passage.

**Cooked mode, deliberately.** We do *not* enable raw mode. The
terminal stays in its normal line-editing mode: `read_line` works,
echo works, backspace works, and `\n` printed to the screen is
translated to carriage-return + newline for us (the `ONLCR` termios
flag). A raw-mode, key-by-key editor is what `ratatui` apps do — the
real `gomoku` binary can grow up to that. Pinned prompt + cooked
input gets 95% of the effect for 5% of the code.

**`queue!` vs `execute!`.** `execute!` writes and flushes every time;
`queue!` only buffers. A frame is ~20 writes — queue them all, flush
once at the end.

## Contract

Add crossterm to the workspace, then in `ui.rs`:

```rust
/// Enters the alternate screen; leaving is this value's Drop.
struct AlternateScreen;

/// Draw one frame: board at (0,0), status below it, message below
/// that, prompt pinned at PROMPT_ROW. Never scrolls.
fn draw(stdout: &mut Stdout, board: &dyn GameBoard, message: &str)
    -> io::Result<()>;

/// The alternate-screen front-end.
pub fn run_ui(board: Box<dyn GameBoard>, kind: EngineKind) -> io::Result<()>;
```

Layout constants (fits any 80×24 terminal):

```text
rows  0..=18   the board (render_lines: ruler, border, 15 rows,
               border, ruler)
row   20       status line
row   21       message line
row   22       "> " prompt   (echo of Enter lands on 23 — the last row)
```

The board grew from 17 to 19 lines in chapter 3, so the lower rows
moved down by two. Row 23 is the *bottom* row of a 24-row terminal:
parking the prompt on 22 and letting the echo land on 23 is the
tightest layout that still cannot scroll.

`main.rs` gains `--plain` to keep the chapter-4 loop as a fallback,
and the run becomes:

```rust
let result = if plain { ui::run_plain(...) } else { ui::run_ui(...) };
```

## Steps

1. `crossterm = "0.29"` into `[workspace.dependencies]`; cli crate
   says `crossterm.workspace = true`.
2. Write the `AlternateScreen` guard and a `draw` that just renders
   the board + prompt, with a hard-coded "type q" — run it, quit,
   confirm your shell comes back intact.
3. Move the loop body from `run_plain` into `run_ui`, replacing the
   prints with `draw`. `dispatch` is reused untouched — if you have
   to change it, the chapter-4 extraction was incomplete.
4. Play a full game. Delight.

## Pitfalls

- **Ctrl-C.** In cooked mode, SIGINT kills the process *without*
  unwinding — `Drop` never runs, and the terminal stays in the
  alternate screen. If that happens: type `reset` (blindly if
  needed). The fix is a signal handler or raw mode; the real binary's
  problem, not this tutorial's. Quit with `q`.
- **Terminal smaller than 24 rows.** The layout is fixed; a tiny
  terminal wraps the 53-char lines and everything looks broken.
  Detecting size (`crossterm::terminal::size`) and refusing politely
  is a fine extra; chapter 6 keeps the simple path.
- **`panic!` in the draw path** shows as *nothing* — the alt screen
  is restored and the message is gone. Print panics to a file while
  debugging, or reproduce with `--plain`.

## Done when

A full game — moves, errors, win, `new`, `q` — happens with the
screen redrawing in place, the shell untouched before and after, on
both engines.

Next: [Chapter 6 — Polish](06-polish.md)

---

## Solution

### `gomoku/Cargo.toml` — workspace dependency

```toml
[workspace.dependencies]
# ... existing entries ...
crossterm = "0.29"
```

### `crates/cli/Cargo.toml` — use it

```toml
[dependencies]
engine = { path = "../engine", features = ["testutil"] }
crossterm.workspace = true
```

### `crates/cli/src/ui.rs` — the full file

```rust
//! Front-ends. `run_plain` scrolls (debugging fallback); `run_ui`
//! owns the alternate screen. Both share `dispatch`.

use std::io::{self, Stdout, Write};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::Print,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::board::{board_for, EngineKind, GameBoard};
use crate::input::{parse, Command};
use crate::render::{render_lines, status_line};

const HELP: &str = "enter <row> <col> to play (e.g. '7 7') · 'new' restarts · 'q' quits";

// ---------------------------------------------------------------- shared

/// One player action applied to the board. Returns the message to
/// display next, or `None` to quit.
pub fn dispatch(
    board: &mut Box<dyn GameBoard>,
    kind: EngineKind,
    input: &str,
) -> Option<String> {
    match parse(input) {
        Ok(Command::Play(mv)) => Some(match board.play(mv) {
            Ok(()) => String::new(),
            Err(e) => e.to_string(),
        }),
        Ok(Command::New) => {
            *board = board_for(kind);
            Some("new game — Black to move".to_string())
        }
        Ok(Command::Help) => Some(HELP.to_string()),
        Ok(Command::Quit) => None,
        Err(e) => Some(e),
    }
}

// ------------------------------------------------------- scrolling UI

/// The scrolling front-end: render, prompt, read, repeat.
pub fn run_plain(mut board: Box<dyn GameBoard>, kind: EngineKind) -> io::Result<()> {
    let mut message = HELP.to_string();
    loop {
        for line in render_lines(&*board) {
            println!("{line}");
        }
        println!("{}", status_line(&*board));
        if !message.is_empty() {
            println!("{message}");
        }
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break; // EOF (Ctrl-D): leave quietly
        }
        match dispatch(&mut board, kind, &input) {
            Some(next) => message = next,
            None => break,
        }
    }
    Ok(())
}

// -------------------------------------------------- alternate-screen UI

const STATUS_ROW: u16 = 20;
const MESSAGE_ROW: u16 = 21;
const PROMPT_ROW: u16 = 22;

/// Enters the alternate screen; leaving is this value's `Drop` —
/// so every exit path (early return, `?`, panic unwind) restores
/// the user's terminal.
struct AlternateScreen;

impl AlternateScreen {
    fn enter() -> io::Result<AlternateScreen> {
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(AlternateScreen)
    }
}

impl Drop for AlternateScreen {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
    }
}

/// Draw one frame at fixed coordinates. Cooked mode's ONLCR turns
/// each "\n" into CR+LF, so plain `Print` walks down the screen.
fn draw(stdout: &mut Stdout, board: &dyn GameBoard, message: &str) -> io::Result<()> {
    queue!(stdout, MoveTo(0, 0), Hide, Clear(ClearType::All))?;
    for line in render_lines(board) {
        queue!(stdout, Print(line), Print("\n"))?;
    }
    queue!(stdout, MoveTo(0, STATUS_ROW), Print(status_line(board)))?;
    queue!(stdout, MoveTo(0, MESSAGE_ROW), Print(message))?;
    queue!(stdout, MoveTo(0, PROMPT_ROW), Print("> "), Show)?;
    stdout.flush()
}

/// The alternate-screen front-end: same loop, zero scrolling.
pub fn run_ui(mut board: Box<dyn GameBoard>, kind: EngineKind) -> io::Result<()> {
    let _screen = AlternateScreen::enter()?;
    let mut stdout = io::stdout();
    let mut message = HELP.to_string();
    loop {
        draw(&mut stdout, &*board, &message)?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break; // EOF
        }
        match dispatch(&mut board, kind, &input) {
            Some(next) => message = next,
            None => break,
        }
    }
    Ok(())
}
```

### `crates/cli/src/main.rs`

```rust
mod board;
mod input;
mod render;
mod ui;

use board::{board_for, EngineKind};

fn main() {
    let (kind, plain) = parse_args().unwrap_or_else(|e| {
        eprintln!("{e}");
        eprintln!("usage: gomoku [--engine naive|fast] [--plain]");
        std::process::exit(2);
    });
    let board = board_for(kind);
    let result = if plain {
        ui::run_plain(board, kind)
    } else {
        ui::run_ui(board, kind)
    };
    if let Err(e) = result {
        eprintln!("terminal error: {e}");
        std::process::exit(1);
    }
}

fn parse_args() -> Result<(EngineKind, bool), String> {
    let mut kind = EngineKind::Fast;
    let mut plain = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--engine" => {
                let value = args.next().ok_or("--engine needs a value: naive|fast")?;
                kind = match value.as_str() {
                    "naive" => EngineKind::Naive,
                    "fast" => EngineKind::Fast,
                    other => return Err(format!("unknown engine '{other}'")),
                };
            }
            "--plain" => plain = true,
            "-h" | "--help" => {
                println!("usage: gomoku [--engine naive|fast] [--plain]");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    Ok((kind, plain))
}
```

Run it:

```text
$ cargo run -p cli -- --engine fast
```

The board appears in a clean screen, the prompt waits at the bottom,
and `q` hands your shell back exactly as you left it.
