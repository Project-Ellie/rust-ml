# Sub-chapter 07d — The puzzle flag

## Abstract

This sub-chapter wires the puzzle pack into the `gomoku play` command
line. A user can now start the study table directly on a puzzle with
`gomoku play --puzzle pack.json --index N`. The `--ai`, `--color`, and
`--sims` flags compose exactly as they do for a Swap2 opening, because
a puzzle is just an opening position: Swap2 is skipped, the puzzle
board seeds the variation tree root, and everything else in the table
works by construction.

The sub-chapter also adds the puzzle context to the side panel and
lists the new puzzle commands in the help text.

## Glossary

| Term | Definition |
|------|------------|
| **Puzzle flag** | The `--puzzle <PACK.json>` argument of the `play` subcommand. |
| **Index flag** | The `--index <N>` argument, zero-based, that selects one puzzle from the pack. |
| **`PlayArgs`** | The clap struct that holds all flags for the `play` subcommand. |
| **`build_puzzle_init`** | The function in `main.rs` that reads a pack file, parses it, and builds a `SessionInit`. |
| **`PuzzleStudy`** | The context carried by `SessionInit` that holds the pack path, the current index, and the parsed pack. |
| **Exit code 2** | The convention for usage and data errors in this binary, matching clap's own error code. |
| **`data/puzzles/fetch.sh`** | The script that downloads the git-ignored puzzle corpora; missing-pack errors point here. |

## Context

Sub-chapters 07a, 07b, and 07c built the puzzle parser, the puzzle
session context, and the `hint` / `reveal` / `next` / `prev` commands.
This sub-chapter is the last slice of chapter 07: it exposes puzzles
through the command line and renders the loaded puzzle in the side
panel.

The core/shell doctrine is unchanged. File I/O lives in `main.rs`; the
`cli` library receives a parsed `PuzzlePack` through
`SessionInit::puzzle` and never touches the filesystem again.

## Intention

The learner enables this user-facing behavior:

1. `gomoku play --puzzle crates/cli/tests/fixtures/mini-pack.json`
   opens the study table on puzzle 1 of the pack.
2. `gomoku play --puzzle pack.json --index 1` opens puzzle 2 (the
   index is zero-based).
3. `--ai`, `--color`, and `--sims` still work: an AI seat studies the
   puzzle the same way it would study a Swap2 opening.
4. Swap2 is skipped when `--puzzle` is present; the puzzle position is
   the opening.
5. A missing or invalid pack exits with code 2, names the path, and
   points at `data/puzzles/fetch.sh`.
6. `--index` without `--puzzle` is rejected by clap with exit code 2.
7. `gomoku selfplay --puzzle pack.json` is rejected with exit code 2.
8. When a pack is loaded, the side panel shows
   `Puzzle: <i+1> of <count> · <attacker> to move` under `Path:`.
9. The help text lists `hint`, `reveal`, and `next / prev`.

## Specification as tests

The tests below are ordered along the execution path: argv parsing,
the flag contract, the startup build, and the rendered panel.

### clap flag contract

* `selfplay_rejects_puzzle_flag` — running `gomoku selfplay --puzzle
  x.json` fails and exits 2.
* `index_requires_puzzle_flag` — running `gomoku play --index 2`
  without `--puzzle` fails and exits 2.

These tests live in a new integration test file because they exercise
only the binary's front door. On this branch, clap-level contract
tests are grouped in `tests/args.rs`.

### Rendering

* `the_panel_shows_puzzle_info_when_loaded` — a puzzle session renders
  the line `Puzzle: 2 of 3 · White to move` in the side panel.

### Help text

* `TWO_MOVE_TRANSCRIPT` in `tests/line_mode.rs` grows the three puzzle
  command lines, preserving the byte-exact transcript.

## Walkthrough

### 1. The user types the command

```text
gomoku play --puzzle crates/cli/tests/fixtures/mini-pack.json --index 1
```

`gomoku` is the binary target defined in `gomoku/crates/cli/Cargo.toml`.
clap parses the command line. `PlayArgs` holds the puzzle flag, the
index flag, and all the search flags.

### 2. clap enforces `--index` requires `--puzzle`

The `index` field uses `#[arg(long, value_name = "N", requires = "puzzle")]`.
An index without a pack is meaningless, so clap rejects it before
`main()` runs. The exit code is 2, clap's own convention for argument
errors.

### 3. `selfplay` rejects `--puzzle`

If the subcommand is `selfplay`, `main()` checks `args.play.puzzle`.
A game generator has no business starting mid-position, so it calls
`usage_error` and exits 2.

### 4. `run_play` branches on the puzzle flag

In `run_play`, if `play.puzzle` is `Some`, the function calls
`build_puzzle_init` and enters the study table immediately, passing
`None` as the opening hint. The normal Swap2 path is skipped.

### 5. `build_puzzle_init` reads and validates the pack

`build_puzzle_init` opens the file with `std::fs::read_to_string`. If
the file is missing, it prints an error that names the path and points
at `data/puzzles/fetch.sh`, then exits 2. The corpora are git-ignored,
so "not found" most likely means "not fetched yet".

The text is parsed with `PuzzlePack::from_json`. A parse failure also
exits 2. An out-of-range index exits 2 as well.

### 6. The session init composes puzzle and seats

`build_puzzle_init` starts from a normal `play_init` with `swap2:
false`, then replaces the opening board with the puzzle board and
attaches a `PuzzleStudy` context. Because the base init already
handles `--ai`, `--color`, `--sims`, `--black-sims`, `--white-sims`,
and `--seed`, those flags compose exactly as they do for a Swap2
opening.

### 7. `study::run` starts the table

The study table receives the same `SessionInit` it would receive for a
normal game. It does not know puzzles exist until it asks the core for
the `PuzzleStudy` context.

### 8. The side panel shows the puzzle line

`render_side_panel` calls `position_lines`. After the `Path:` line, if
`core.puzzle()` returns a study context, the panel adds one more line:

```text
Puzzle: 2 of 3 · White to move
```

The line lives under `Path:` because the puzzle context is one more
fact about the position, not a mode.

### 9. The help text lists the puzzle commands

`src/repl.rs` `HELP` gains three lines for `hint`, `reveal`, and
`next / prev`. The same text is appended to the byte-exact transcript
in `tests/line_mode.rs`.

## Low-level design

### [`gomoku/crates/cli/src/main.rs`](../../../../../gomoku/crates/cli/src/main.rs)

* Import `PuzzlePack` from `cli::puzzle` and `PuzzleStudy` and
  `SessionInit` from `cli::session`.
* Import `std::path::{Path, PathBuf}`.
* Add `puzzle: Option<PathBuf>` and `index: Option<usize>` to
  `PlayArgs`.
  * `puzzle` is `#[arg(long, value_name = "PACK.json")]`.
  * `index` is `#[arg(long, value_name = "N", requires = "puzzle")]`.
* In the `Selfplay` arm of `main()`, reject `--puzzle` with
  `usage_error`.
* In `run_play`, if `play.puzzle` is `Some`, call
  `build_puzzle_init(...)` and return `study::run(init, None)`.
* Add `build_puzzle_init(search, pack_path, index)`:
  * read the file, exit 2 on failure with the fetch.sh hint;
  * parse the pack, exit 2 on failure;
  * fetch the puzzle at `index`, exit 2 on out-of-range;
  * build a normal `play_init(search, false)`;
  * replace `init.opening` with the puzzle board;
  * set `init.puzzle` to a `PuzzleStudy` with the path, index, and pack.

### [`gomoku/crates/cli/src/tui/render.rs`](../../../../../gomoku/crates/cli/src/tui/render.rs)

* Pass `core` into `position_lines`.
* After the `Path:` line in `position_lines`, if `core.puzzle()` is
  `Some`, push the puzzle info line.
* Add `the_panel_shows_puzzle_info_when_loaded`.

### [`gomoku/crates/cli/src/repl.rs`](../../../../../gomoku/crates/cli/src/repl.rs)

* Append the three puzzle command lines to `HELP`.

### [`gomoku/crates/cli/tests/line_mode.rs`](../../../../../gomoku/crates/cli/tests/line_mode.rs)

* Update `TWO_MOVE_TRANSCRIPT` with the same three puzzle command lines.

### [`gomoku/crates/cli/tests/args.rs`](../../../../../gomoku/crates/cli/tests/args.rs)

* Create this new file.
* Add a `run_gomoku` helper that uses `env!("CARGO_BIN_EXE_gomoku")`.
* Add `selfplay_rejects_puzzle_flag`.
* Add `index_requires_puzzle_flag`.

## TDD checklist

Work one test group at a time, red to green.

From `gomoku/crates/cli/tests/args.rs`:

1. `selfplay_rejects_puzzle_flag`
2. `index_requires_puzzle_flag`

From `gomoku/crates/cli/src/tui/render.rs`:

3. `the_panel_shows_puzzle_info_when_loaded`

From `gomoku/crates/cli/tests/line_mode.rs`:

4. Update `TWO_MOVE_TRANSCRIPT` and run
   `the_two_move_transcript_is_byte_exact`.

## Verification commands

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Done when

* `gomoku play --puzzle pack.json --index N` opens the study table on
  the selected puzzle.
* `--ai`, `--color`, and `--sims` compose as they do for a Swap2
  opening.
* Swap2 is skipped when `--puzzle` is present.
* A missing or invalid pack exits 2, names the path, and points at
  `data/puzzles/fetch.sh`.
* `--index` without `--puzzle` exits 2.
* `gomoku selfplay --puzzle` exits 2.
* The side panel shows `Puzzle: <i+1> of <count> · <attacker> to move`
  under `Path:`.
* The help text lists `hint`, `reveal`, and `next / prev`.
* The three new tests pass and the byte-exact transcript is unchanged
  except for the added help lines.
* The three project gates pass.
* A scripted run shows the puzzle line and a hint. From `gomoku/`:

  ```text
  printf 'hint\nreveal\nq\n' | cargo run -q -p cli --bin gomoku -- play --puzzle crates/cli/tests/fixtures/mini-pack.json --index 1
  ```

  Expect puzzle 2's position, the panel line
  `Puzzle: 2 of 3 · White to move`, and `hint: the line starts at i5`.
* A missing pack shows the fetch.sh hint. From `gomoku/`:

  ```text
  cargo run -q -p cli --bin gomoku -- play --puzzle /nonexistent.json; echo "exit: $?"
  ```

  Expect the fetch.sh hint and exit code 2.

## References

* [Sub-chapter 07a — The pack parser](../a-pack-parser/pack-parser.md) — the parser this sub-chapter consumes.
* [Sub-chapter 07b — The puzzle session](../b-puzzle-session/puzzle-session.md) — the `SessionInit::puzzle` constructor and `PuzzleStudy` context.
* [Sub-chapter 07c — Study commands](../c-study-commands/study-commands.md) — the `hint`, `reveal`, `next`, and `prev` commands.
* [Chapter 07 — Positions from files: puzzles](../../07-positions-from-files-puzzles.md) — the chapter overview.
* [Playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md) — the "positions from files" amendment.
* [`gomoku/crates/cli/src/main.rs`](../../../../../gomoku/crates/cli/src/main.rs) — command-line parsing and startup.
* [`gomoku/crates/cli/src/tui/render.rs`](../../../../../gomoku/crates/cli/src/tui/render.rs) — the side panel.
