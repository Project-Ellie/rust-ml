# Sub-chapter 07e — The picker: design paper

## Abstract

This paper records the decisions behind the native file picker for
`--puzzle`. A bare flag opens an OS file dialog at the puzzle corpus;
a valued flag keeps the 07d behavior exactly. The dialog is one
blocking `rfd` call in `main.rs`, made before the TUI starts, and
cancelling it is a quiet exit 0 — not an error.

## Glossary

| Term | Definition |
|------|------------|
| **`rfd`** | The Rusty File Dialogs crate: native open/save dialogs on macOS, Linux, and Windows. |
| **`pick_puzzle_pack`** | The single `main.rs` function that names `rfd`. |
| **`resolve_pack_path`** | The flag-to-path resolution, generic over the picker so tests stub it. |
| **Exit code 0** | Success — including a cancelled dialog. |
| **Exit code 2** | Usage and data errors, matching clap's own convention. |

## Context

Sub-chapter 07d gave `play` a `--puzzle pack.json` flag. Every entry
into puzzle study names a corpus path on the command line. Paths are
right for scripts and tests and wrong for daily human use: the corpus
location is project knowledge the user should not have to type. A file
chooser is standard desktop functionality; the question this paper
answers is which crate provides it, and how it fits the project's
doctrines.

The alternatives considered: a hand-rolled in-TUI file browser (a
whole widget to build and maintain, against the core/shell effort
budget), a ratatui explorer widget (the study table is a hand-rolled
crossterm TUI, not ratatui — the widget would force a renderer
migration), and an embedded fuzzy finder (heavyweight, and a second
text UI inside the first). A native dialog via `rfd` is the only
option that adds zero rendering code to the project.

## Decision 1 — `rfd`, pinned exactly, vetted on arrival

`rfd` is the de-facto standard native-dialog crate. On macOS it drives
`NSOpenPanel` through objc2 with no system dependencies; on Windows it
uses the COM dialogs; on Linux it needs either the default `gtk3`
backend (gtk-3 development packages at build time) or the
`xdg-portal` feature (D-Bus only). The workspace pins key dependencies
exactly so upgrades stay deliberate; `rfd` joins that discipline with
an `=x.y.z` pin, and the slice commit records the `cargo tree`
footprint (the objc2 family on macOS) so the dependency cost is a
decision, not a surprise.

## Decision 2 — The blocking API, because async is banned

Chapter 12 locks no-tokio for this project, and the study table is a
single-threaded loop. `rfd` offers `AsyncFileDialog` for runtime
users; this project uses `FileDialog::pick_file`, which blocks the
calling thread until the dialog closes. Blocking is not a compromise
here: at startup the binary has exactly one job — learn which pack to
load — and the modal dialog *is* that job.

## Decision 3 — The picker runs before the TUI starts

The bare flag resolves in `run_play`, upstream of `study::run`. Raw
mode and the alternate screen do not exist yet when the dialog opens,
so there is no terminal state to suspend and restore. The dialog being
a separate OS window makes this doubly true: even a mid-session picker
would not touch the terminal frame buffer, but this sub-chapter does
not build one. Mid-session pack switching is out of scope; `next` /
`prev` already cover movement within a pack.

## Decision 4 — A bare flag asks, a valued flag tells

`--puzzle` becomes an optional-value flag: `Option<Option<PathBuf>>`
with clap's `num_args = 0..=1`. The valued form is byte-for-byte the
07d contract — scripts, `sessiondemo`, and every test keep explicit
paths and never see a dialog. This preserves the project's
scriptability rule: everything automatable stays automatable, and the
interactive affordance is an addition, never the only path.
`sessiondemo` does not get the bare form; a scripted demo has no one
to answer a dialog.

## Decision 5 — Cancel is not an error

Closing the dialog without a selection prints `no pack selected` and
exits 0. Exit 2 stays reserved for usage and data errors (a missing or
invalid pack, `--index` without `--puzzle`, `--puzzle` under
`selfplay`). A user who cancels a chooser made no mistake; punishing
the gesture with an error code would train scripts to treat "user
changed their mind" as failure.

## Decision 6 — The seam is a closure, and automation never drives a GUI

`resolve_pack_path` takes the parsed flag and an
`impl FnOnce() -> Option<PathBuf>` picker. The unit tests inject a
panicking stub (proving a valued flag never opens the dialog) and
constant stubs (proving the bare flag's picked and cancelled paths).
The native dialog itself has no automated test: like the git-ignored
corpora, it is outside what CI can reach, and like them it gets a
manual verification ritual in the tutorial's *Done when*. This is the
fixture discipline extended one notch — not everything observable is
scriptable, and the design names that boundary instead of pretending
otherwise.

## Decision 7 — The dialog starts at the corpus, with a fallback

The picker opens at `data/puzzles/` filtered to `.json`. When
the corpus directory does not exist, the picker falls back to the
working directory rather than erroring, because the dialog itself is
the discovery interface. The exit-2 `data/puzzles/` hint stays where
it belongs: on the valued path, where a named file is missing.

## Decision 8 — One helper, two sources

`pick_puzzle_pack` is a thin call over a generic `pick_file(title,
directory, extensions)` helper in `main.rs`. Chapter 08's `--game`
reuses that helper with the `.psq` filter and the
`data/gomocup-games/` directory when its own optional-value slice
lands — the game-record flag gains a bare form on the same terms
(bare asks, valued tells, cancel exits 0, `sessiondemo` keeps
requiring a path). Chapter 08's reference build predates this paper;
its flag gains the optional value in a follow-up slice, and this
decision is recorded there so the follow-up is a contract, not an
afterthought.

## Data flow

```text
argv
  |
  v
PlayArgs (clap)        puzzle: Option<Option<PathBuf>>
  |
  v
main() Selfplay arm    --puzzle present? --→ usage_error → exit 2
  |
  v
run_play()
  |
  +-- flag absent -------------→ Swap2 opening path (unchanged)
  |
  +-- flag present --→ resolve_pack_path(flag, pick_puzzle_pack)
          |
          +-- Some(Some(path)) --→ path        (picker never called)
          |
          +-- Some(None) --→ pick_puzzle_pack()
                  |
                  v
              rfd FileDialog     (native window, blocking, pre-TUI)
                  |
                  +-- cancel --→ "no pack selected" → exit 0
                  |
                  +-- picked --→ path
          |
          v
      build_puzzle_init(path, index)     (the 07d flow, unchanged)
          |
          v
      study::run(init, None)
```

## References

* [Sub-chapter 07d — The puzzle flag: design paper](../d-puzzle-flag/puzzle-flag-design.md) — the flag contract this paper extends.
* [Chapter 07 redesign design paper](../00-design.md) — the chapter-level decisions, including fixture discipline.
* [Chapter 12 — Gomoku architecture](../../../../12-gomoku-architecture.md) — the no-async-runtime decision.
* [`rfd` crate documentation](https://docs.rs/rfd) — backends, features, and the sync/async split.
* [Playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md) — the "positions from files" amendment.
