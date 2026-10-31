# Sub-chapter 07d — The puzzle flag: implementation plan

This document turns the walkthrough in `puzzle-flag.md` into a
construction order. It names the files, structs, enums, functions, and
tests. It states behavioral contracts in prose. It does not contain
bodies.

Read `puzzle-flag.md` first. Work one step at a time. Write tests
before code. Open `puzzle-flag-solution.md` only if you are stuck for
more than twenty minutes, or after the sub-chapter is complete.

---

## Step 0 — `src/main.rs`: add the puzzle flags

### 0.1 Extend imports

Import `PuzzlePack` from `cli::puzzle`.
Import `PuzzleStudy` and `SessionInit` from `cli::session`.
Import `std::path::{Path, PathBuf}`.

### 0.2 Extend `PlayArgs`

Add:

```rust
/// Study puzzle N of a puzzle pack (skips Swap2).
#[arg(long, value_name = "PACK.json")]
puzzle: Option<PathBuf>,

/// Zero-based index of the puzzle to load (default 0).
#[arg(long, value_name = "N", requires = "puzzle")]
index: Option<usize>,
```

### 0.3 Reject `--puzzle` for `selfplay`

In the `Selfplay` arm of `main()`, if `args.play.puzzle.is_some()`,
call `usage_error("--puzzle is only valid with the 'play' subcommand")`.

### 0.4 Branch in `run_play`

After the `--color requires --ai` check, if `play.puzzle` is `Some`,
call `build_puzzle_init(&play.search, pack_path, index)` and return
`study::run(init, None)`.

### 0.5 Add `build_puzzle_init`

Write a function:

```rust
fn build_puzzle_init(search: &SearchArgs, pack_path: &Path, index: usize) -> SessionInit
```

Behavior:
* Read the file at `pack_path` with `std::fs::read_to_string`.
  * On failure, print `error: cannot read puzzle pack '{}': {e}` with
    the path display, then print `       puzzle corpora are
    git-ignored — fetch them with data/puzzles/fetch.sh`, then exit 2.
* Parse the text with `PuzzlePack::from_json`.
  * On failure, print `error: invalid puzzle pack '{}': {e}` with the
    path display, then exit 2.
* Fetch the puzzle at `index` with `pack.get(index)`.
  * On failure, print `error: {e} in '{}'` with the path display, then
    exit 2.
* Build a normal `play_init(search, false)`.
* Replace `init.opening` with `puzzle.board.clone()`.
* Set `init.puzzle` to `Some(PuzzleStudy { pack_path:
  pack_path.to_path_buf(), index, pack })`.
* Return `init`.

### 0.6 Gate

Run `cargo test -p cli`.

---

## Step 1 — `src/tui/render.rs`: puzzle panel line

### 1.1 Thread `core` into `position_lines`

Change the signature to:

```rust
fn position_lines(lines: &mut Vec<String>, core: &SessionCore, board: &Board, snapshot: &Snapshot)
```

Update the call site in `render_side_panel`.

### 1.2 Add the puzzle line

After the `Path:` line, if `core.puzzle()` is `Some`, push:

```rust
lines.push(format!(
    "Puzzle: {} of {} · {:?} to move",
    study.index + 1,
    study.pack.items.len(),
    puzzle.attacker
));
```

### 1.3 Add the render test

Add `the_panel_shows_puzzle_info_when_loaded`:

* Parse `tests/fixtures/mini-pack.json` with `PuzzlePack::from_json`.
* Build a `SessionInit::puzzle` with index 1.
* Create a `SessionCore`.
* Render the study view.
* Assert the joined frame contains
  `"Puzzle: 2 of 3 · White to move"`.

### 1.4 Gate

Run `cargo test -p cli --lib render::tests`.

---

## Step 2 — `src/repl.rs` and `tests/line_mode.rs`: help text

### 2.1 Update `src/repl.rs`

Append to `HELP`:

```text
  hint             show the first move of the puzzle's line
  reveal           show the puzzle's full claimed line
  next / prev      load the next/previous puzzle (clamps at the ends)
```

### 2.2 Update `tests/line_mode.rs`

Append the same three lines to `TWO_MOVE_TRANSCRIPT`.

### 2.3 Gate

Run `cargo test -p cli --test line_mode`.

---

## Step 3 — `tests/args.rs`: clap flag contract

### 3.1 Create the file

Create `gomoku/crates/cli/tests/args.rs`.

### 3.2 Add the helper

```rust
fn run_gomoku(args: &[&str]) -> std::process::Output
```

Use `Command::new(env!("CARGO_BIN_EXE_gomoku"))`.

### 3.3 Add `selfplay_rejects_puzzle_flag`

Run `run_gomoku(&["selfplay", "--puzzle", "x.json"])`. Assert failure
and exit code 2.

### 3.4 Add `index_requires_puzzle_flag`

Run `run_gomoku(&["play", "--index", "2"])`. Assert failure and exit
code 2.

### 3.5 Gate

Run `cargo test -p cli --test args`.

---

## Step 4 — Project gates

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

---

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `src/main.rs` | imports | `PuzzlePack`, `PuzzleStudy`, `SessionInit`, `Path`, `PathBuf` |
| `src/main.rs` | `PlayArgs::puzzle` | `Option<PathBuf>`, `--puzzle <PACK.json>` |
| `src/main.rs` | `PlayArgs::index` | `Option<usize>`, `--index <N>`, `requires = "puzzle"` |
| `src/main.rs` | `selfplay` rejection | `--puzzle` with `selfplay` exits 2 |
| `src/main.rs` | `run_play` branch | puzzle present → `build_puzzle_init` → `study::run(init, None)` |
| `src/main.rs` | `build_puzzle_init` | read/parse/index failures exit 2 with fetch.sh hint |
| `src/main.rs` | `build_puzzle_init` | seats compose as for Swap2 opening |
| `src/main.rs` | `build_puzzle_init` | attaches `PuzzleStudy` with path, index, and pack |
| `src/tui/render.rs` | `position_lines` signature | accepts `core` |
| `src/tui/render.rs` | puzzle panel line | under `Path:`, format `Puzzle: i+1 of count · attacker to move` |
| `src/tui/render.rs` | test | `the_panel_shows_puzzle_info_when_loaded` passes |
| `src/repl.rs` | `HELP` | lists `hint`, `reveal`, `next / prev` |
| `tests/line_mode.rs` | transcript | byte-exact match includes the new help lines |
| `tests/args.rs` | file exists | new integration test file |
| `tests/args.rs` | `selfplay_rejects_puzzle_flag` | exits 2 |
| `tests/args.rs` | `index_requires_puzzle_flag` | exits 2 |
