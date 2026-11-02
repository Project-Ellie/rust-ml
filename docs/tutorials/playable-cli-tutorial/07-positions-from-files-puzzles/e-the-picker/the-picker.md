# Sub-chapter 07e — The picker

## Abstract

This sub-chapter wires the native OS file dialog into the puzzle flag.
A user can now start the study table with a bare `gomoku play
--puzzle` — no path — and choose the pack in a file dialog that opens
at the puzzle corpus, filtered to `.json` files. The valued form
`--puzzle pack.json` is unchanged: scripts, demos, and tests keep
their explicit paths and never see a dialog. Cancelling the dialog is
a quiet exit, not an error.

The dialog is one shell-layer function in `main.rs`, built on the
`rfd` crate. The library is untouched. The dialog opens before the
study table starts, so the terminal needs no suspend/resume handling.

## Glossary

| Term | Definition |
|------|------------|
| **Picker** | The native OS file dialog opened by a bare `--puzzle` flag. On macOS it is `NSOpenPanel`; on Linux, GTK3 or the XDG portal; on Windows, the COM dialog. |
| **Optional-value flag** | A clap flag that may appear with or without a value. `--puzzle` means "ask me"; `--puzzle pack.json` means "use this". |
| **`Option<Option<PathBuf>>`** | The clap idiom for an optional-value flag: `None` = flag absent, `Some(None)` = bare flag, `Some(Some(path))` = valued flag. |
| **`pick_puzzle_pack`** | The `main.rs` function that builds and runs the `rfd` dialog. The only function in the project that touches `rfd`. |
| **`resolve_pack_path`** | The `main.rs` function that turns the parsed flag into a path, calling the picker only for a bare flag. Generic over the picker, so tests stub it. |
| **Cancel** | Closing the dialog without a selection. Resolves to no path; the binary prints one line and exits 0. |
| **Blocking dialog** | `rfd::FileDialog::pick_file` blocks the calling thread until the dialog closes. Correct here: the binary has nothing else to do at startup, and the project bans async runtimes. |

## Context

Sub-chapters 07a–07d built the parser, the session context, the study
commands, and the `--puzzle pack.json` startup flag. Every one of
those paths names the pack on the command line. That is the right
shape for scripts and tests, but a human who studies puzzles daily
should not type corpus paths. This sub-chapter adds the chooser.

The core/shell doctrine is unchanged — and sharpened. File I/O already
lives in `main.rs`; a GUI dialog is I/O of the same kind, so it lives
there too, in exactly one function. The `cli` library, `sessiondemo`,
and every test keep working with explicit paths only.

The sub-chapter also adds the first unit tests inside `main.rs`. The
reason is a constraint, not a preference: an integration test cannot
run a bare `--puzzle`, because that would open a real dialog on the
test machine. So the parse shape and the picker seam are tested where
they live, with `CliArgs::try_parse_from` and a stubbed picker.

## Intention

The learner enables this user-facing behavior:

1. `gomoku play --puzzle` opens the native file dialog at
   `data/puzzles/`, filtered to `.json`. Picking a pack opens
   the study table on puzzle 1 of that pack.
2. `--index N` composes with the bare flag: the picked pack opens at
   puzzle N+1.
3. `gomoku play --puzzle pack.json` is byte-for-byte the 07d behavior.
   The dialog never opens when a path is given.
4. Cancelling the dialog prints `no pack selected` and exits 0.
5. `gomoku selfplay --puzzle` is rejected with exit 2, bare or valued.
6. `sessiondemo --puzzle` still requires a path. A scripted demo has
   no one to answer a dialog.
7. The dialog falls back to the working directory when
   `data/puzzles/` does not exist.

## Specification as tests

The tests are ordered along the execution path: parse shape first,
then the resolution seam. All of them live in a new `#[cfg(test)] mod
tests` at the bottom of `main.rs` — the binary target's first unit
tests, run by `cargo test -p cli` like any other.

### Parse shape

* `bare_puzzle_flag_parses_as_a_picker_request` — parsing
  `gomoku play --puzzle` yields `Some(None)` for the puzzle field.
* `valued_puzzle_flag_parses_as_a_path` — parsing
  `gomoku play --puzzle pack.json` yields `Some(Some("pack.json"))`.
* `index_composes_with_a_bare_puzzle_flag` — parsing
  `gomoku play --puzzle --index 1` succeeds; clap's `requires` counts
  a bare flag as present.

### The resolution seam

`resolve_pack_path` takes the parsed flag and a picker closure. The
tests inject stubs:

* `explicit_path_never_opens_the_picker` — resolving
  `Some(Some(path))` with a picker that panics returns the path. The
  panic proves the dialog never opens for a valued flag.
* `bare_flag_uses_the_picker_result` — resolving `Some(None)` with a
  picker that returns `Some(picked)` returns `picked`.
* `picker_cancel_resolves_to_none` — resolving `Some(None)` with a
  picker that returns `None` returns `None`.

### Regression

The 07d integration tests in `tests/args.rs`
(`selfplay_rejects_puzzle_flag`, `index_requires_puzzle_flag`) and the
scripted puzzle runs keep passing unchanged. The dialog itself is
verified manually — see *Done when*. Automation never drives a GUI;
that is the fixture discipline extended one notch.

## Walkthrough

### 1. The user types the bare flag

```text
gomoku play --puzzle
```

No path follows the flag. In 07d this was a clap error ("a value is
required"). From this sub-chapter on it is a request: *ask me which
pack*.

### 2. clap parses an optional value

The `puzzle` field of `PlayArgs` changes type from
`Option<PathBuf>` to `Option<Option<PathBuf>>`, with
`#[arg(long, value_name = "PACK.json", num_args = 0..=1)]`. The outer
`Option` is flag presence, the inner one is the value. This is the
standard clap idiom for flags like `--color[=WHEN]`; the three states
are exactly the three user intentions. The `index` field keeps its
`requires = "puzzle"` from 07d, and a bare `--puzzle` satisfies it:
clap's `requires` tests presence, not value.

### 3. `main()` rejects `selfplay` as before

The `Selfplay` arm checks `args.play.puzzle.is_some()`. Presence is
what matters, bare or valued, so this check does not change shape at
all.

### 4. `run_play` resolves the path before anything runs

Where 07d passed `play.puzzle` straight to `build_puzzle_init`,
`run_play` now calls:

```text
resolve_pack_path(&play.puzzle, pick_puzzle_pack)
```

The timing matters: this happens *before* `study::run`, so before raw
mode and the alternate screen exist. The dialog is a separate OS
window, and the terminal is still an ordinary terminal. There is no
suspend/resume problem, because the TUI has not started yet.

### 5. The picker builds and runs the dialog

`pick_puzzle_pack` is the only function that names `rfd`:

```text
rfd::FileDialog::new()
    .set_title("Load a puzzle pack")
    .set_directory(<corpus dir if it exists, else working dir>)
    .add_filter("puzzle packs", &["json"])
    .pick_file()
```

`pick_file` is blocking: the call returns `Option<PathBuf>` when the
dialog closes. Blocking is correct here — the binary has nothing else
to do while the user chooses, and the project's no-async decision
(chapter 12) rules out `rfd`'s `AsyncFileDialog`. One dependency, one
call site, one return value.

### 6. Cancel is a quiet exit

A cancelled dialog returns `None` from the picker, `None` from
`resolve_pack_path`, and `run_play` prints `no pack selected` and
exits 0. Exit 2 stays reserved for usage and data errors: a user who
cancels a chooser made no mistake.

### 7. A picked path joins the 07d flow

From a resolved path onward, nothing is new: `build_puzzle_init`
reads, parses, validates, and builds the session init exactly as in
07d, and `study::run` opens the table. The picker ends where the
parser begins.

## Low-level design

### [`gomoku/crates/cli/Cargo.toml`](../../../../../gomoku/crates/cli/Cargo.toml)

* Add `rfd` with an exact-version pin (`=x.y.z`), matching the
  workspace's deliberate-upgrade discipline. Note in a comment that
  Linux builds need either the default `gtk3` backend or the
  `xdg-portal` feature.

### [`gomoku/crates/cli/src/main.rs`](../../../../../gomoku/crates/cli/src/main.rs)

* `PlayArgs.puzzle` becomes `Option<Option<PathBuf>>` with
  `num_args = 0..=1`. Doc comment: bare means "open the picker".
* Add `pick_puzzle_pack() -> Option<PathBuf>`: builds the dialog as in
  step 5. The corpus-directory fallback (`data/puzzles/` if it
  exists, else the working directory) is computed here with
  `std::path::Path::is_dir`.
* Add `resolve_pack_path(flag: &Option<Option<PathBuf>>, pick: impl
  FnOnce() -> Option<PathBuf>) -> Option<PathBuf>`. Contract:
  * flag absent → unreachable in the caller; document that the caller
    only calls this when the flag is `Some`;
  * `Some(Some(path))` → `Some(path.clone())`, picker untouched;
  * `Some(None)` → `pick()`, once.
  `FnOnce` states the at-most-once property in the type.
* `run_play` matches the resolution: `Some(path)` → the 07d
  `build_puzzle_init` flow; `None` → print `no pack selected`, exit 0.
* Add `#[cfg(test)] mod tests` with the six tests from the
  specification. The seam tests use closures: a panicking closure for
  the never-called case, constant closures for the two picker cases.

### Everything else is untouched

The `cli` library, `sessiondemo`, `render.rs`, `repl.rs`, the
line-mode transcript, and `tests/args.rs` do not change. No new
commands, no new events, no new panel lines.

## TDD checklist

Work one group at a time, red to green. From `gomoku/`,
`cargo test -p cli` runs the new unit tests with everything else.

1. The three parse tests (red: the field type does not exist yet).
2. The three seam tests (red: `resolve_pack_path` does not exist).
3. Green: the flag type change, `pick_puzzle_pack`,
   `resolve_pack_path`, and the `run_play` match arm.
4. Regression: the full `cargo test -p cli` suite, including the 07d
   integration tests.

## Verification commands

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Done when

* The six new unit tests pass, and the whole `cli` suite stays green.
* The three project gates pass.
* Manual verification — the dialog cannot be scripted:
  * `cargo run -q -p cli --bin gomoku -- play --puzzle` opens the
    dialog at `data/puzzles/` (or the working directory if it does
    not exist), showing only `.json` files.
  * Picking `crates/cli/tests/fixtures/mini-pack.json` opens the study
    table on puzzle 1 with the panel line
    `Puzzle: 1 of 3 · Black to move`.
  * `cargo run -q -p cli --bin gomoku -- play --puzzle --index 1`
    followed by picking the fixture opens puzzle 2.
  * Cancelling the dialog prints `no pack selected`; `echo $?` shows
    `0`.
  * The 07d scripted run
    (`play --puzzle crates/cli/tests/fixtures/mini-pack.json --index 1`
    with `hint` / `reveal` piped in) is unchanged — no dialog appears.
* Commit message for the code slice:
  `feat(cli): bare --puzzle opens the native pack picker`.

## References

* [Sub-chapter 07d — The puzzle flag](../d-puzzle-flag/puzzle-flag.md) — the flag this sub-chapter extends.
* [The picker: design paper](the-picker-design.md) — the decisions behind this slice.
* [Chapter 07 — Positions from files: puzzles](../../07-positions-from-files-puzzles.md) — the chapter overview.
* [Chapter 12 — Gomoku architecture](../../../../12-gomoku-architecture.md) — the no-async-runtime decision that selects `rfd`'s blocking API.
* [`rfd` crate documentation](https://docs.rs/rfd) — `FileDialog`, filters, directories, and the Linux backend features.
* `data/puzzles/` — the packs the dialog starts at.
