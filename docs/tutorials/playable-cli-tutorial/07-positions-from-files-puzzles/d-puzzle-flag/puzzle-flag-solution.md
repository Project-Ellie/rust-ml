# Sub-chapter 07d — The puzzle flag: solution

This file is opt-in. Read it only if you have been stuck for more than
twenty minutes, or after you have finished the sub-chapter and want to
compare your work.

Verified green from `gomoku/`: `cargo test -p cli`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.

Quotes slice commit 0489ecc on reference/chapter-07-redesign.

---

`gomoku/crates/cli/src/main.rs` gains the puzzle imports, the two new
flags on `PlayArgs`, the `selfplay` rejection, the `run_play` branch,
and the `build_puzzle_init` helper.

`gomoku/crates/cli/src/main.rs`, lines 12–18:

```rust
use clap::{Args, Parser, Subcommand, ValueEnum};
use cli::actor::{AiConfig, EvaluatorKind, Occupant};
use cli::puzzle::PuzzlePack;
use cli::session::{PuzzleStudy, SessionInit};
use cli::tui::study;
use engine::{Board, Color};
use std::path::{Path, PathBuf};
```

`gomoku/crates/cli/src/main.rs`, lines 56–62:

```rust
    /// Study puzzle N of a puzzle pack (skips Swap2).
    #[arg(long, value_name = "PACK.json")]
    puzzle: Option<PathBuf>,

    /// Zero-based index of the puzzle to load (default 0).
    #[arg(long, value_name = "N", requires = "puzzle")]
    index: Option<usize>,
```

`gomoku/crates/cli/src/main.rs`, lines 146–148:

```rust
            if args.play.puzzle.is_some() {
                usage_error("--puzzle is only valid with the 'play' subcommand");
            }
```

`gomoku/crates/cli/src/main.rs`, lines 176–180:

```rust
    // A puzzle pack seeds the study table directly; Swap2 is skipped.
    if let Some(pack_path) = &play.puzzle {
        let init = build_puzzle_init(&play.search, pack_path, play.index.unwrap_or(0));
        return study::run(init, None);
    }
```

`gomoku/crates/cli/src/main.rs`, lines 191–227:

```rust
/// Load a puzzle pack and build the `SessionInit` for studying it.
///
/// Exits with code 2 (usage/data error) when the pack is missing or
/// invalid; the puzzle corpora are git-ignored, so the message points at
/// the corpus's `data/puzzles/fetch.sh`.
fn build_puzzle_init(search: &SearchArgs, pack_path: &Path, index: usize) -> SessionInit {
    let text = std::fs::read_to_string(pack_path).unwrap_or_else(|e| {
        eprintln!(
            "error: cannot read puzzle pack '{}': {e}",
            pack_path.display()
        );
        eprintln!("       puzzle corpora are git-ignored — fetch them with data/puzzles/fetch.sh");
        std::process::exit(2);
    });
    let pack = PuzzlePack::from_json(&text).unwrap_or_else(|e| {
        eprintln!("error: invalid puzzle pack '{}': {e}", pack_path.display());
        std::process::exit(2);
    });
    let opening = pack
        .get(index)
        .unwrap_or_else(|e| {
            eprintln!("error: {e} in '{}'", pack_path.display());
            std::process::exit(2);
        })
        .board
        .clone();

    // Seats and AI flags compose exactly as for a Swap2 opening.
    let mut init = play_init(search, false);
    init.opening = opening;
    init.puzzle = Some(PuzzleStudy {
        pack_path: pack_path.to_path_buf(),
        index,
        pack,
    });
    init
}
```

In `gomoku/crates/cli/src/tui/render.rs`, `position_lines` now takes
the `SessionCore` so it can read the puzzle context, and it adds the
puzzle line under `Path:`.

`gomoku/crates/cli/src/tui/render.rs`, lines 476–478:

```rust
    match &snapshot.opening {
        Some(opening) => ceremony_lines(&mut lines, opening),
        None => position_lines(&mut lines, core, board, snapshot),
    }
```

`gomoku/crates/cli/src/tui/render.rs`, lines 506–526:

```rust
/// The game-state block of the side panel: ply, side to move or game
/// result, path tail, and branch options.
fn position_lines(lines: &mut Vec<String>, core: &SessionCore, board: &Board, snapshot: &Snapshot) {
    lines.push(format!("Ply: {}", snapshot.ply));
    // One line covers both states a game can be in: waiting for a side,
    // or finished with a result.
    match snapshot.status {
        Status::Ongoing => lines.push(format!("To move: {:?}", board.to_move())),
        Status::Won(color) => lines.push(format!("Game over: {color:?} wins")),
        Status::Draw => lines.push("Game over: draw".to_string()),
    }
    lines.push(format!("Path: {}", path_tail_string(board)));
    if let Some(study) = core.puzzle() {
        let puzzle = &study.pack.items[study.index];
        lines.push(format!(
            "Puzzle: {} of {} · {:?} to move",
            study.index + 1,
            study.pack.items.len(),
            puzzle.attacker
        ));
    }
```

The render test loads puzzle index 1 from the mini fixture and asserts
the panel text.

`gomoku/crates/cli/src/tui/render.rs`, lines 717–730:

```rust
    #[test]
    fn the_panel_shows_puzzle_info_when_loaded() {
        let pack = PuzzlePack::from_json(include_str!("../../tests/fixtures/mini-pack.json"))
            .expect("mini fixture parses");
        let init = SessionInit::puzzle(PathBuf::from("tests/fixtures/mini-pack.json"), pack, 1)
            .expect("index 1 is in the pack");
        let core = SessionCore::new(init);
        let frame = render_study(&StudyView::new(&core, &[]));
        let text = frame.join("\n");
        assert!(
            text.contains("Puzzle: 2 of 3 · White to move"),
            "panel shows puzzle info: {text}"
        );
    }
```

`gomoku/crates/cli/src/repl.rs` appends the three puzzle command lines
to `HELP`.

`gomoku/crates/cli/src/repl.rs`, lines 37–39:

```rust
  hint             show the first move of the puzzle's line
  reveal           show the puzzle's full claimed line
  next / prev      load the next/previous puzzle (clamps at the ends)
```

`gomoku/crates/cli/tests/line_mode.rs` updates the byte-exact
transcript with the same three lines.

`gomoku/crates/cli/tests/line_mode.rs`, lines 127–129:

```rust
  hint             show the first move of the puzzle's line
  reveal           show the puzzle's full claimed line
  next / prev      load the next/previous puzzle (clamps at the ends)
```

`gomoku/crates/cli/tests/args.rs` is new on this branch. The chapter
placed clap-level contract tests in a dedicated file because they
exercise only the binary's front door and do not need any session
state.

`gomoku/crates/cli/tests/args.rs`, full file:

```rust
//! Command-line flag contract tests for the `gomoku` binary.
//!
//! These pin clap-level behavior: invalid flag combinations exit with
//! code 2 before the session starts.

use std::process::Command;

/// Run the `gomoku` binary with `args` and return the captured output.
///
/// `CARGO_BIN_EXE_gomoku` is the path of the binary target; cargo
/// sets it when it builds the integration tests.
fn run_gomoku(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gomoku"))
        .args(args)
        .output()
        .expect("the gomoku binary runs")
}

#[test]
fn selfplay_rejects_puzzle_flag() {
    let output = run_gomoku(&["selfplay", "--puzzle", "x.json"]);
    assert!(!output.status.success(), "expected failure");
    assert_eq!(
        output.status.code(),
        Some(2),
        "exit code should be 2, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn index_requires_puzzle_flag() {
    let output = run_gomoku(&["play", "--index", "2"]);
    assert!(!output.status.success(), "expected failure");
    assert_eq!(
        output.status.code(),
        Some(2),
        "exit code should be 2, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
```
