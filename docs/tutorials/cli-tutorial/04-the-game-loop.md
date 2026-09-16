# Chapter 4 — The game loop

Goal: playable end-to-end. This version still scrolls like any
`println!` program — deliberately. Getting the *loop logic* right in
the simple world means chapter 5 only swaps the display, nothing
else.

## Contract

`crates/cli/src/input.rs`:

```rust
pub enum Command {
    Play(Move),
    New,
    Quit,
    Help,
}

/// Parse one line of user input. Never panics; typos become `Err`
/// messages the UI can display.
pub fn parse(input: &str) -> Result<Command, String>;
```

Accepted syntax:

| Input | Meaning |
|-------|---------|
| `7 7`, `7,7`, `7, 7` | play at row 7, col 7 |
| `new` | start over (same engine) |
| `h`, `help`, `?` | show usage |
| `q`, `quit`, `exit` | leave |

(`u` / `undo` is deliberately absent here: taking moves back is
[chapter 6](06-polish.md)'s optional exercise, because both boards
already support it and it is easier to add once the loop is working.)

`crates/cli/src/ui.rs`:

```rust
/// One player action applied to the board. Returns the message to
/// display next, or `None` to quit.
pub fn dispatch(board: &mut Box<dyn GameBoard>, kind: EngineKind, input: &str)
    -> Option<String>;

/// The scrolling front-end: render, prompt, read, repeat.
pub fn run_plain(board: Box<dyn GameBoard>, kind: EngineKind) -> io::Result<()>;
```

Why `dispatch` exists: chapters 4 and 5 run *different* loops over
the *same* action handling. Extracting it now means chapter 5 changes
zero game logic.

## Rust toolbox

**`split` with a pattern.** `text.split([',', ' '])` accepts an array
of chars as the pattern and yields a piece per separator —
`"7, 7"` → `["7", "", "7"]`. Filter the empties and any mix of
commas/spaces parses identically.

**The message-passing loop.** State between iterations is a single
`String` (the message line): empty after a good move, an error after
a bad one, help text after `h`. One piece of loop state, displayed in
one place — the simplest UI architecture that exists, and chapter 5
keeps it unchanged.

**`Result<(), PlayError>` is user-facing.** You don't match on
`Occupied` vs `GameOver` in the UI at all — `thiserror`'s `Display`
(`e.to_string()`) is already the message. Typed errors for callers
who branch, display text for callers who show; the UI shows.

## TDD checklist (parser)

1. `"7 7"` → `Play` with row 7, col 7
2. `"7,7"` and `" 7 , 7 "` → same
3. `"q"`, `"quit"` → `Quit`; `"new"` → `New`; `"h"` → `Help`
4. `"7 15"` → `Err` mentioning the board
5. `"banana"` → `Err` with usage
6. `""` → `Err` with usage

## The loop

```text
loop {
    print board + status + message
    print "> ", flush
    read line            (EOF → quit)
    dispatch:
        Play → board.play; error → message
        New  → fresh board from board_for(kind)
        Help → usage → message
        Quit → break
}
```

Two details worth noticing in the solution: `read_line` returning `0`
means EOF (Ctrl-D) — treat it as quit, or the loop spins forever on a
closed stdin. And `print!` without a newline needs an explicit
`flush()`, or the prompt appears *after* the user types.

## Done when

You can play Black vs yourself to a five-in-a-row, see the win
announced, get slapped for playing an occupied cell, restart with
`new`, and leave with `q` — on both engines.

Next: [Chapter 5 — The alternate screen](05-alternate-screen.md)

---

## Solution

### `crates/cli/src/input.rs`

```rust
//! Command parsing — pure functions, no I/O.

use engine::Move;

#[derive(Debug)]
pub enum Command {
    Play(Move),
    New,
    Quit,
    Help,
}

const USAGE: &str = "usage: <row> <col> (0-14) · 'new' restarts · 'q' quits";

/// Parse one line of user input. Never panics; typos become `Err`
/// messages the UI can display.
pub fn parse(input: &str) -> Result<Command, String> {
    let text = input.trim().to_lowercase();
    match text.as_str() {
        "q" | "quit" | "exit" => return Ok(Command::Quit),
        "new" => return Ok(Command::New),
        "h" | "help" | "?" => return Ok(Command::Help),
        _ => {}
    }

    let parts: Vec<&str> = text
        .split([',', ' '])
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() != 2 {
        return Err(USAGE.to_string());
    }
    let row: u8 = parts[0].parse().map_err(|_| USAGE.to_string())?;
    let col: u8 = parts[1].parse().map_err(|_| USAGE.to_string())?;
    let mv = Move::new(row, col)
        .ok_or_else(|| format!("({row}, {col}) is off the board — coordinates are 0-14"))?;
    Ok(Command::Play(mv))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn played(input: &str) -> (u8, u8) {
        match parse(input) {
            Ok(Command::Play(mv)) => (mv.row(), mv.col()),
            other => panic!("{input:?} should be a move, got {other:?}"),
        }
    }

    #[test]
    fn space_separated_coordinates() {
        assert_eq!(played("7 7"), (7, 7));
    }

    #[test]
    fn comma_separated_coordinates() {
        assert_eq!(played("7,7"), (7, 7));
        assert_eq!(played(" 7 , 7 "), (7, 7));
    }

    #[test]
    fn keywords() {
        assert!(matches!(parse("q"), Ok(Command::Quit)));
        assert!(matches!(parse("quit"), Ok(Command::Quit)));
        assert!(matches!(parse("new"), Ok(Command::New)));
        assert!(matches!(parse("h"), Ok(Command::Help)));
        assert!(matches!(parse("?"), Ok(Command::Help)));
    }

    #[test]
    fn off_board_is_an_error() {
        let err = parse("7 15").unwrap_err();
        assert!(err.contains("off the board"), "{err}");
    }

    #[test]
    fn garbage_gets_usage() {
        for bad in ["banana", "", "7", "1 2 3", "7 x"] {
            let err = parse(bad).unwrap_err();
            assert!(err.contains("usage"), "{bad:?} -> {err}");
        }
    }
}
```

### `crates/cli/src/ui.rs`

```rust
//! Front-ends. The scrolling one lives here; chapter 5 adds the
//! alternate-screen one — both share `dispatch`.

use std::io::{self, Write};

use crate::board::{board_for, EngineKind, GameBoard};
use crate::input::{parse, Command};
use crate::render::{render_lines, status_line};

const HELP: &str = "enter <row> <col> to play (e.g. '7 7') · 'new' restarts · 'q' quits";

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
```

### `crates/cli/src/main.rs`

```rust
mod board;
mod input;
mod render;
mod ui;

use board::{board_for, EngineKind};

fn main() {
    let kind = parse_engine_arg().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });
    let board = board_for(kind);
    if let Err(e) = ui::run_plain(board, kind) {
        eprintln!("terminal error: {e}");
        std::process::exit(1);
    }
}

// parse_engine_arg unchanged from chapter 2
```

Play:

```text
$ cargo run -p cli
     0  1  2  3  4  5  6  7  8  9 10 11 12 13 14
   +---------------------------------------------+
 0 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 0
...
14 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 14
   +---------------------------------------------+
     0  1  2  3  4  5  6  7  8  9 10 11 12 13 14
engine: fast · move 0 · Black to move
enter <row> <col> to play (e.g. '7 7') · 'new' restarts · 'q' quits
> 7 7
```
