# Side track — terminals, TUIs, and crossterm

A self-contained excursion. No Gomoku knowledge required, no board, no
engine — just you, a terminal, and a small moving `@`. Read this before
sub-chapter d; everything the study-table shell does with the terminal
is built from the pieces below.

## Two ways to talk to a terminal

**Line mode (canonical mode).** Your program prints lines and reads lines. The
terminal driver buffers input until Enter, handles backspace for you,
and echoes what you type. `println!` and `stdin().read_line` are all
you need. Scrollback is yours: everything you print scrolls into the
user's terminal history. Every demo binary in this tutorial so far is a
line-mode program.

**Full-screen (raw + alternate screen).** Your program owns a
rectangle. Two switches make it work:

- the **alternate screen** — a second, separate buffer. Switching to it
  hides the user's scrollback; leaving it restores whatever was there
  before. This is why quitting `less` or `vim` gives you your shell
  back untouched.
- **raw mode** — the terminal driver stops buffering and echoing.
  Every keypress reaches your program immediately, including arrows
  and Ctrl-combinations. In raw mode, your program must restore the
  terminal state when it exits. If it does not, the terminal stays in
  raw mode and appears broken.

A full-screen TUI then runs a simple rhythm: **state → render → wait
for input → update state → render again.** Rendering repaints the whole
frame every time. There is no diffing obligation on you — a 15×15 board
is a few dozen short lines; terminals are fast.

## Under the hood: ANSI escape sequences

Everything above is bytes. The terminal interprets in-band commands
called escape sequences; crossterm's job is to emit the right ones
portably. Three examples show the pattern:

```text
ESC [ ? 1049 h     enter the alternate screen  (h = set mode)
ESC [ ? 1049 l     leave it                    (l = reset mode)
ESC [ 5 ; 12 H     move the cursor to row 5, column 12
ESC [ 2 J          clear the whole screen
ESC [ 33 m hello ESC [ 0 m    print "hello" in yellow, then reset
```

You could write these with `print!("\x1b[2J")`. Crossterm exists so you
don't memorize the manual and so Windows terminals work too.

## crossterm in one tour

The crate provides a set of small command values plus two macros that send
them. The entire API the study table uses fits in this table:

| Piece | What it does |
|---|---|
| `terminal::enable_raw_mode()` / `disable_raw_mode()` | line buffering off/on |
| `terminal::{EnterAlternateScreen, LeaveAlternateScreen}` | the second buffer |
| `terminal::{Clear, ClearType::All}` | wipe the frame |
| `cursor::{MoveTo(x, y), Hide, Show}` | position/visibility (0-based, x = column) |
| `style::Print(text)` | print at the cursor |
| `Stylize` trait | `"text".cyan().bold()` → a styled, printable value |
| `execute!(w, cmds…)` | send commands and flush immediately |
| `queue!(w, cmds…)` | send commands, flush later — batch a whole frame |
| `event::{poll, read}` + `Event::Key` | raw-mode input, one keypress at a time |

The `queue!` / flush split matters: painting a frame is dozens of
cursor moves and prints. Queuing them and flushing once per frame is
what keeps the picture from flickering.

## The RAII guard

Raw mode and the alternate screen are global, sticky state. If your
program returns early, errors out, or *panics* while they are active,
the user's terminal stays broken. Rust's answer is a guard whose `Drop`
restores everything — Drop runs on every exit path, panic included:

```rust
use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{
        self, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use std::io;

/// Restores the terminal on drop — early return, error, or panic.
/// `enter` puts the terminal into TUI mode; the value's only job is
/// to stay alive until the TUI is done.
struct TuiGuard;

impl TuiGuard {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(Self) // the guard carries no data; its Drop is the point
    }
}

impl Drop for TuiGuard {
    fn drop(&mut self) {
        // Ignore errors: if restoration fails there is nothing
        // sensible left to do, and panicking inside Drop during an
        // unwind would abort the process.
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
```

The usage is one line at the top of the TUI entry point — the binding
must not be dropped early, hence the underscore:

```rust,ignore
let _guard = TuiGuard::enter()?;
```

## A complete tiny TUI: move the `@`

Forty lines, all of them explained. Save as `src/main.rs` in a scratch
crate with `crossterm = "0.29"` and `cargo run`. Arrows move, `q`
quits.

```rust
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode},
    execute,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};

// The guard from the previous section, verbatim.
struct TuiGuard;
impl TuiGuard {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(Self)
    }
}
impl Drop for TuiGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

fn main() -> io::Result<()> {
    let _guard = TuiGuard::enter()?;

    // The entire application state: where the @ is.
    let (mut x, mut y) = (5u16, 2u16);

    loop {
        // 1. RENDER. Repaint the whole frame from state.
        //    Queue the commands, then one flush: no flicker.
        let mut out = io::stdout();
        execute!(out, Clear(ClearType::All), MoveTo(x, y))?;
        print!("@");
        out.flush()?;

        // 2. WAIT for one input event. Raw mode means we see
        //    arrow keys directly — no Enter involved.
        if let Event::Key(key) = event::read()? {
            // 3. UPDATE state. saturating_sub keeps us inside the
            //    top/left edges: u16 cannot go negative, and
            //    saturating_sub clamps at 0 instead of panicking.
            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Left => x = x.saturating_sub(1),
                KeyCode::Right => x += 1,
                KeyCode::Up => y = y.saturating_sub(1),
                KeyCode::Down => y += 1,
                _ => {} // every other key: ignore
            }
        }
    }
}
```

Notice the loop's shape: **render everything, read one event, update
state, repeat.** The renderer is dumb — it does not know what changed;
it repaints the world. The study table uses the same loop shape.

## What the study table takes from this

The chapter-5 shell uses everything in this excursion: raw mode,
`event::read`, the alternate screen, the RAII guard, `queue!`-batched
repaints, cursor positioning, and `Stylize`. The `@` example above is
the pattern: read one key event, update state, repaint the frame.

The shell adds one idea of its own: **modal input**, in the style of
vim. Keys do different things in different modes. In *normal mode*,
single keys perform frequent actions: arrows and `hjkl` move the board
cursor, Enter places a stone or steps the AI, `u` undoes, `?` shows
help, `q` quits. Pressing `:` opens a *command line* at the bottom of
the screen; there you type the full command language (`var 2`,
`sims 800`, `info`), Enter executes, Esc cancels. The command line is a
small editor that the program implements itself — in raw mode there is
no terminal line editing, so Backspace handling and echo are your code.

The line-mode fallback, sessiondemo, and scripts do not use raw mode.
They read complete lines in canonical mode and parse them with the same
command language. One command language, two ways to enter it.

## Going further

- crossterm's own docs and examples: <https://docs.rs/crossterm>
- When you want widgets (layout, borders, scrollable lists) instead of
  hand-placed lines: <https://ratatui.rs> sits on top of crossterm.
  The study table implements its layout directly. The purpose is that
  you learn what a widget library would do for you.
- The escape-sequence reference behind everything:
  <https://invisible-island.net/xterm/ctlseqs/ctlseqs.html>
