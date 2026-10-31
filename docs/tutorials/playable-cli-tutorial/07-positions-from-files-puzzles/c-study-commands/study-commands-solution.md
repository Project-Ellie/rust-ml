# Sub-chapter 07c — The study commands: solution

This file is opt-in. Read it only if you have been stuck for more than
twenty minutes, or after you have finished the sub-chapter and want to
compare your work.

Verified green from `gomoku/`: `cargo test -p cli`, `cargo clippy
--all-targets -- -D warnings`, `cargo fmt --all -- --check`.

---

This sub-chapter changes four source files, one integration test file,
and the shared test kit. `command.rs` adds the four study commands,
their events, their errors, and the parser. `session.rs` adds the
dispatch and the three executor helpers. `sessiondemo.rs` adds the
`--puzzle` / `--index` flags, `#`-comment skipping, and the puzzle
status line. `tests/session.rs` adds six new tests and strengthens the
reset test. `testkit` classifies the new events.

## `gomoku/crates/cli/src/command.rs`

`Command` gains the four study variants after `New`.

`gomoku/crates/cli/src/command.rs`, lines 50–63:

```rust
    /// Show the first move of the loaded puzzle's claimed line.
    Hint,
    /// Show the full claimed line of the loaded puzzle.
    Reveal,
    /// Load the next puzzle in the pack as a fresh tree root.
    ///
    /// Navigation clamps: at the last puzzle this is an error, not a
    /// wrap-around.
    NextPuzzle,
    /// Load the previous puzzle in the pack as a fresh tree root.
    ///
    /// Navigation clamps: at the first puzzle this is an error, not a
    /// wrap-around.
    PrevPuzzle,
```

`Event` gains `Hint`, `SolutionRevealed`, and `PuzzleLoaded`. The fields
of `PuzzleLoaded` are 0-based for machine consumers.

`gomoku/crates/cli/src/command.rs`, lines 158–176:

```rust
    /// The first move of the loaded puzzle's claimed line.
    Hint {
        /// First move of the claimed line.
        mv: Move,
    },
    /// The full claimed line of the loaded puzzle.
    SolutionRevealed {
        /// Claimed line: move and side for each ply.
        line: Vec<(Move, Color)>,
    },
    /// A puzzle was loaded as a fresh variation tree root.
    PuzzleLoaded {
        /// Zero-based index within the pack.
        index: usize,
        /// Number of puzzles in the pack.
        count: usize,
        /// Side to move (the attacker).
        attacker: Color,
    },
```

The `Display` implementation formats the human-readable messages.
`PuzzleLoaded` prints a 1-based index.

`gomoku/crates/cli/src/command.rs`, lines 212–228:

```rust
            Event::Hint { mv } => write!(f, "hint: the line starts at {}", fmt_move(*mv)),
            Event::SolutionRevealed { line } => {
                let text = line
                    .iter()
                    .map(|(mv, side)| format!("{} {side:?}", fmt_move(*mv)))
                    .collect::<Vec<_>>()
                    .join(" → ");
                write!(f, "solution: {text}")
            }
            Event::PuzzleLoaded {
                index,
                count,
                attacker,
            } => {
                // Display is 1-based; the event fields stay 0-based for
                // machine consumers.
                write!(f, "puzzle {} of {count} · {attacker:?} to move", index + 1)
            }
```

`CommandError` gains the four typed errors that the executors and the
front-end can distinguish.

`gomoku/crates/cli/src/command.rs`, lines 275–297:

```rust
    /// A puzzle command was issued but no puzzle pack is loaded.
    #[error("No puzzle loaded — start with 'gomoku play --puzzle <pack.json>'")]
    NoPuzzleLoaded,
    /// The loaded puzzle has no claimed solution line.
    #[error("Puzzle {} of {count} has no claimed line", index + 1)]
    NoSolutionLine {
        /// Zero-based index of the loaded puzzle.
        index: usize,
        /// Number of puzzles in the pack.
        count: usize,
    },
    /// `next` at the last puzzle — navigation clamps.
    #[error("Already at the last puzzle ({count} of {count})")]
    AtLastPuzzle {
        /// Number of puzzles in the pack.
        count: usize,
    },
    /// `prev` at the first puzzle — navigation clamps.
    #[error("Already at the first puzzle (1 of {count})")]
    AtFirstPuzzle {
        /// Number of puzzles in the pack.
        count: usize,
    },
```

The parser recognizes the four new words and rejects any trailing
tokens.

`gomoku/crates/cli/src/command.rs`, lines 401–418:

```rust
        "hint" => {
            ensure_no_more(&mut tokens, "hint")?;
            Ok(Command::Hint)
        }

        "reveal" => {
            ensure_no_more(&mut tokens, "reveal")?;
            Ok(Command::Reveal)
        }

        "next" => {
            ensure_no_more(&mut tokens, "next")?;
            Ok(Command::NextPuzzle)
        }

        "prev" => {
            ensure_no_more(&mut tokens, "prev")?;
            Ok(Command::PrevPuzzle)
        }
```

The parser test covers exact and case-insensitive matches plus error
cases.

`gomoku/crates/cli/src/command.rs`, lines 636–643:

```rust
    #[test]
    fn parse_puzzle_commands() {
        assert_eq!(parse("hint"), Ok(Command::Hint));
        assert_eq!(parse("REVEAL"), Ok(Command::Reveal));
        assert_eq!(parse("next"), Ok(Command::NextPuzzle));
        assert_eq!(parse("prev"), Ok(Command::PrevPuzzle));
        assert!(parse("hint x").is_err());
        assert!(parse("next 2").is_err());
    }
```

## `gomoku/crates/cli/src/session.rs`

The puzzle import is now used, so the `#[allow(unused_imports)]` from
sub-chapter 07b is removed.

`gomoku/crates/cli/src/session.rs`, line 12:

```rust
use crate::puzzle::{Puzzle, PuzzleError, PuzzlePack};
```

`SessionCore::execute` routes the four new `Command` variants.

`gomoku/crates/cli/src/session.rs`, lines 368–371:

```rust
            Command::Hint => self.execute_hint(),
            Command::Reveal => self.execute_reveal(),
            Command::NextPuzzle => self.execute_puzzle_nav(1),
            Command::PrevPuzzle => self.execute_puzzle_nav(-1),
```

`execute_hint` reads the first solution move, or returns
`NoSolutionLine` when the puzzle is unlabelled.

`gomoku/crates/cli/src/session.rs`, lines 631–641:

```rust
    fn execute_hint(&self) -> Result<Vec<Event>, CommandError> {
        let (study, puzzle) = self.puzzle_context()?;
        let (mv, _) = puzzle
            .solution
            .first()
            .ok_or(CommandError::NoSolutionLine {
                index: study.index,
                count: study.pack.items.len(),
            })?;
        Ok(vec![Event::Hint { mv: *mv }])
    }
```

`execute_reveal` returns the whole claimed line, or the same error when
there is none.

`gomoku/crates/cli/src/session.rs`, lines 643–654:

```rust
    fn execute_reveal(&self) -> Result<Vec<Event>, CommandError> {
        let (study, puzzle) = self.puzzle_context()?;
        if puzzle.solution.is_empty() {
            return Err(CommandError::NoSolutionLine {
                index: study.index,
                count: study.pack.items.len(),
            });
        }
        Ok(vec![Event::SolutionRevealed {
            line: puzzle.solution.clone(),
        }])
    }
```

`execute_puzzle_nav` clamps at the ends, updates the stored index,
rebuilds the history from the new puzzle board, clears stale search
info, and emits `PuzzleLoaded`. Seats and AI configuration are not
changed.

`gomoku/crates/cli/src/session.rs`, lines 656–688:

```rust
    /// Pack navigation for `next` (`delta` 1) and `prev` (`delta` -1).
    ///
    /// Navigation **clamps** at the pack ends rather than wrapping: the
    /// command fails with `AtFirstPuzzle` / `AtLastPuzzle` and the
    /// current puzzle stays loaded. The chosen puzzle replaces the
    /// variation tree root wholesale — every puzzle is a fresh study
    /// position; seats and AI configuration are untouched.
    fn execute_puzzle_nav(&mut self, delta: i64) -> Result<Vec<Event>, CommandError> {
        let (study, _) = self.puzzle_context()?;
        let count = study.pack.items.len();
        let next = study.index as i64 + delta;
        if next < 0 {
            return Err(CommandError::AtFirstPuzzle { count });
        }
        if next >= count as i64 {
            return Err(CommandError::AtLastPuzzle { count });
        }
        let index = next as usize;
        let board = study.pack.items[index].board.clone();
        let attacker = study.pack.items[index].attacker;

        if let Some(study) = self.puzzle.as_mut() {
            study.index = index;
        }
        self.history = History::from_opening(&board);
        self.search_info.clear();

        Ok(vec![Event::PuzzleLoaded {
            index,
            count,
            attacker,
        }])
    }
```

`puzzle_context` centralizes the "no pack loaded" check.

`gomoku/crates/cli/src/session.rs`, lines 690–696:

```rust
    fn puzzle_context(&self) -> Result<(&PuzzleStudy, &Puzzle), CommandError> {
        let study = self.puzzle.as_ref().ok_or(CommandError::NoPuzzleLoaded)?;
        // The index is valid by construction: navigation clamps and the
        // pack never shrinks.
        let puzzle = &study.pack.items[study.index];
        Ok((study, puzzle))
    }
```

## `gomoku/crates/cli/src/bin/sessiondemo.rs`

The help text lists the new study commands.

`gomoku/crates/cli/src/bin/sessiondemo.rs`, lines 38–40:

```rust
  hint             show the first move of the puzzle's line
  reveal           show the puzzle's full claimed line
  next / prev      load the next/previous puzzle (clamps at the ends)
```

`parse_flag_value` reads a flag and the argument that follows it.

`gomoku/crates/cli/src/bin/sessiondemo.rs`, lines 52–56:

```rust
fn parse_flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).map(String::as_str))
}
```

`build_init` loads a pack when `--puzzle` is given, falls back to a
hotseat init otherwise, and exits on bad data.

`gomoku/crates/cli/src/bin/sessiondemo.rs`, lines 58–85:

```rust
fn build_init(args: &[String]) -> SessionInit {
    let seed = parse_seed(args);
    match parse_flag_value(args, "--puzzle") {
        Some(path) => {
            let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Cannot read puzzle pack '{}': {}", path, e);
                std::process::exit(1);
            });
            let pack = cli::puzzle::PuzzlePack::from_json(&text).unwrap_or_else(|e| {
                eprintln!("Invalid puzzle pack '{}': {}", path, e);
                std::process::exit(1);
            });
            let index = parse_flag_value(args, "--index")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);
            let init = SessionInit::puzzle(std::path::PathBuf::from(path), pack, index)
                .unwrap_or_else(|e| {
                    eprintln!("Cannot load puzzle: {}", e);
                    std::process::exit(1);
                });
            SessionInit { seed, ..init }
        }
        None => SessionInit {
            seed,
            ..SessionInit::new_hotseat()
        },
    }
}
```

Script lines that start with `#` are skipped so the demo script can
carry comments.

`gomoku/crates/cli/src/bin/sessiondemo.rs`, line 133:

```rust
    if trimmed.is_empty() || trimmed.starts_with('#') {
```

When a pack is loaded, `print_state` shows the 1-based puzzle index and
the attacker.

`gomoku/crates/cli/src/bin/sessiondemo.rs`, lines 209–217:

```rust
    if let Some(study) = core.puzzle() {
        let puzzle = &study.pack.items[study.index];
        println!(
            "puzzle {} of {}  attacker {:?}",
            study.index + 1,
            study.pack.items.len(),
            puzzle.attacker
        );
    }
```

## `gomoku/crates/cli/tests/session.rs`

The reset test is strengthened to prove that `new` uses the initial
puzzle from `SessionInit`, not the current one.

`gomoku/crates/cli/tests/session.rs`, lines 380–388:

```rust
#[test]
fn new_resets_to_the_initially_loaded_puzzle() {
    let mut core = puzzle_session(0);
    core.execute(Command::NextPuzzle).unwrap();
    play(&mut core, 9, 9);
    let events = core.execute(Command::New).unwrap();
    assert!(events.iter().any(|e| matches!(e, &Event::Reset)));
    assert_eq!(core.puzzle().unwrap().index, 0);
    assert_eq!(core.board(), mini_pack().items[0].board);
}
```

`hint` returns the first move of the line.

`gomoku/crates/cli/tests/session.rs`, lines 391–395:

```rust
#[test]
fn hint_emits_first_solution_move() {
    let mut core = puzzle_session(0);
    let events = core.execute(Command::Hint).unwrap();
    assert_eq!(events, vec![Event::Hint { mv: mv(7, 7) }]);
}
```

`hint` on an unlabelled puzzle returns `NoSolutionLine`.

`gomoku/crates/cli/tests/session.rs`, lines 397–409:

```rust
#[test]
fn hint_errors_when_puzzle_has_no_line() {
    let text = r#"{ "v": 1, "count": 1, "items": [
        { "i": 0, "a": "black", "d": 1, "b": [], "w": [], "l": [] }
    ] }"#;
    let pack = puzzle::PuzzlePack::from_json(text).unwrap();
    let init = SessionInit::puzzle(std::path::PathBuf::from("pack.json"), pack, 0).unwrap();
    let mut core = SessionCore::new(init);
    assert_eq!(
        core.execute(Command::Hint).unwrap_err(),
        CommandError::NoSolutionLine { index: 0, count: 1 }
    );
}
```

`reveal` returns the full line.

`gomoku/crates/cli/tests/session.rs`, lines 411–425:

```rust
#[test]
fn reveal_emits_full_claimed_line() {
    let mut core = puzzle_session(2);
    let events = core.execute(Command::Reveal).unwrap();
    assert_eq!(
        events,
        vec![Event::SolutionRevealed {
            line: vec![
                (mv(7, 10), Color::Black),
                (mv(7, 6), Color::White),
                (mv(7, 11), Color::Black),
            ]
        }]
    );
}
```

`next` and `prev` replace the tree root and report the new position.

`gomoku/crates/cli/tests/session.rs`, lines 427–455:

```rust
#[test]
fn next_and_prev_load_a_fresh_root_and_report_position() {
    let mut core = puzzle_session(0);
    // Dirty the tree first; navigation must replace the root wholesale.
    play(&mut core, 9, 9);

    let events = core.execute(Command::NextPuzzle).unwrap();
    assert_eq!(
        events,
        vec![Event::PuzzleLoaded {
            index: 1,
            count: 3,
            attacker: Color::White,
        }]
    );
    assert_eq!(core.snapshot().ply, 0);
    assert_eq!(core.board(), mini_pack().items[1].board);

    let events = core.execute(Command::PrevPuzzle).unwrap();
    assert_eq!(
        events,
        vec![Event::PuzzleLoaded {
            index: 0,
            count: 3,
            attacker: Color::Black,
        }]
    );
    assert_eq!(core.board(), mini_pack().items[0].board);
}
```

Navigation clamps at both ends.

`gomoku/crates/cli/tests/session.rs`, lines 457–472:

```rust
#[test]
fn next_prev_clamp_at_pack_ends() {
    let mut core = puzzle_session(0);
    assert_eq!(
        core.execute(Command::PrevPuzzle).unwrap_err(),
        CommandError::AtFirstPuzzle { count: 3 }
    );
    core.execute(Command::NextPuzzle).unwrap();
    core.execute(Command::NextPuzzle).unwrap();
    assert_eq!(
        core.execute(Command::NextPuzzle).unwrap_err(),
        CommandError::AtLastPuzzle { count: 3 }
    );
    // Clamping leaves the current puzzle loaded.
    assert_eq!(core.puzzle().unwrap().index, 2);
}
```

Without a pack, every study command returns `NoPuzzleLoaded`.

`gomoku/crates/cli/tests/session.rs`, lines 474–493:

```rust
#[test]
fn puzzle_commands_error_without_a_pack() {
    let mut core = SessionCore::new(SessionInit::new_hotseat());
    assert_eq!(
        core.execute(Command::Hint).unwrap_err(),
        CommandError::NoPuzzleLoaded
    );
    assert_eq!(
        core.execute(Command::Reveal).unwrap_err(),
        CommandError::NoPuzzleLoaded
    );
    assert_eq!(
        core.execute(Command::NextPuzzle).unwrap_err(),
        CommandError::NoPuzzleLoaded
    );
    assert_eq!(
        core.execute(Command::PrevPuzzle).unwrap_err(),
        CommandError::NoPuzzleLoaded
    );
}
```

## `gomoku/crates/testkit/src/lib.rs`

The shared test kit classifies the three new events so transcript-style
assertions can use `event_types`.

`gomoku/crates/testkit/src/lib.rs`, lines 73–75:

```rust
        Event::Hint { .. } => "Hint",
        Event::SolutionRevealed { .. } => "SolutionRevealed",
        Event::PuzzleLoaded { .. } => "PuzzleLoaded",
```

## `gomoku/crates/cli/puzzle_demo.txt`

The capstone script is 13 lines. `#` lines are comments; the rest are
commands.

`gomoku/crates/cli/puzzle_demo.txt`, full file:

```text
# puzzle_demo.txt — studying a puzzle pack at the table.
# Run from gomoku/:
#   cargo run -p cli --bin sessiondemo -- --puzzle crates/cli/tests/fixtures/mini-pack.json --script crates/cli/puzzle_demo.txt
# Puzzle 1 of 3: Black to move, vertical four on column h. Where does the line start?
hint
# A wrong try: i9 is legal but is not the claimed move.
i9
back
# Give up and show the claimed line.
reveal
# Move on to the next puzzle in the pack (clamps at the ends).
next
hint
```

## References

* [Sub-chapter 07c tutorial](study-commands.md)
* [Sub-chapter 07c design paper](study-commands-design.md)
* [Sub-chapter 07c implementation plan](study-commands-implementation.md)
