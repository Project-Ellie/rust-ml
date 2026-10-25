# Chapter 05 redesign — the study table as a sub-project

Status: implemented on main (sub-chapters a–f, 152 cli tests green).
This paper remains the architecture contract for the sub-chapters.

Decided (2026-10, with the owner):

- Folder layout: one subfolder per sub-chapter, named `a-refactor`,
  `b-renderer`, and so on. Each subfolder contains two documents: the
  tutorial (`refactor.md`) and the solution (`refactor-solution.md`).
  The sub-chapter name appears in both file names.
- The old chapter-level files `01-solution.md` and `02-git-diff.md`
  are removed.
- Input model: option C, vim-style modal input. The full-screen shell
  has a normal mode (single keys for frequent actions) and a command
  line opened with `:`, where the full command language is available.
  Canonical-mode line input stays for the fallback, sessiondemo, and
  scripts. Keys are only translations to `Command` values; the session
  core sees no difference.
- Swap2 vocabulary: typed commands `play h8 [color]`, `back`, `done`,
  `black`, `white`, `more`. Normal-mode keys: Enter places, `u` removes
  the last placement, `d` ends a placement phase, `b` / `w` choose a
  color in a choice phase, `m` places two more stones. `u` has one
  phase-dependent meaning (undo placement during the opening, tree
  navigation afterwards); the session knows the phase, so this is
  unambiguous.
- Swap2 placement order: the Swap2 rule fixes only the final stone
  counts of each placement phase (two black + one white; three black +
  two white after `more`). The placement order is free. The default
  order alternates, starting with black (black-white-black, then
  black-white); the user can override the color of the next stone.
  (Override mechanics: `b` / `w` select the next stone's color in
  normal mode; `play h8 white` uses an optional color token in
  canonical mode.)
- All documents follow ASD-STE100: short sentences, one meaning per
  word, active voice, no figurative language.

## Why the restructure

The reference build implements the study table as one chapter with a
465-line `tui/study.rs`. That file contains five concerns at once:
event loop, Swap2 ceremony, line-mode fallback, algebraic parsing, and
screen management. It has six internal comments. It contains two pairs
of duplicated loops: `run_alt`/`run_line` and
`run_swap2_alt`/`run_swap2_line` share their logic verbatim and differ
only in rendering. It re-implements algebraic parsing that already
exists in `command.rs` (identical logic, different error strings). It
implements a 440-line Swap2 state mirror although the engine already
provides a complete Swap2 typestate machine. The chapter adds zero
integration tests. Test helpers (`mv`, `play`, AI-config builders) are
copied across four test files.

That is too much material for one self-study chapter. The duplication
also makes the structure difficult to see. The redesign splits the
chapter into six sub-chapters. Each sub-chapter has one aspect, its own
tests, and a size that fits one study session.

## The concerns, separated

| Concern | Location after the redesign | Sub-chapter |
|---|---|---|
| Algebraic notation (`h8` ↔ `Move`) | `src/notation.rs` (one canonical implementation) | a |
| Line interpreter (`process_line`, `print_state`, `print_search_info` from sessiondemo) | `src/repl.rs` (library code; output not bound to stdout) | a |
| Test helpers (`mv`, `play`, `step`, `event_type`, config builders) | `tests/common/mod.rs` (one shared kit) | a |
| Study-table rendering (pure: state in, lines out) | `src/tui/render.rs` | b |
| Swap2 opening (engine typestate + session replay log) | `src/session.rs` + new commands in `src/command.rs` | c |
| Full-screen shell (alternate screen, frame painting) | `src/tui/study.rs` + `src/tui/screen.rs` | d |
| Line-mode fallback | one more `Screen` implementation, ~40 lines | e |
| The binary (`play`/`selfplay` argument handling, README) | `src/main.rs` | f |

Dependency rule: rendering depends on the session; the shell depends on
rendering; never the other direction. Crossterm is used only inside
`src/tui/`. The session core stays headless. Tests drive the session
core directly.

## Target structure

```text
main.rs          arguments ──► SessionInit ──► study::run(init)
                                                  │
                 ┌────────────────────────────────┘
                 ▼
tui/study.rs     the loop: read line ──► command::parse ──► SessionCore::execute
                 │                                                │
                 ▼                                                ▼
tui/screen.rs    Screen::draw(frame) ◄── render.rs (pure) ◄── events
                 │
        ┌────────┴─────────┐
        ▼                  ▼
  AltScreen           LineScreen
  (crossterm,         (println!,
   RAII guard)         the fallback)
```

One loop, one parser, one renderer. The two screens differ only in how
a completed frame reaches the terminal. In the reference, this
difference is written out twice in each ceremony; here it exists once,
behind the `Screen` trait.

## File and size budget

No source file in this chapter exceeds approximately 250 lines. Most
files are much smaller. `tui/study.rs` decreases from 465 lines to
approximately 120. If a file grows past the budget during a
sub-chapter, that file contains more than one concern and must be
split.

## The sub-chapters

### a — Refactor: notation, the repl core, and the test kit

*Aspect: refactoring without behavior change. Existing code contains
reusable parts; this sub-chapter extracts them.*

- `src/notation.rs` — one canonical `parse_algebraic(&str) ->
  Result<Move, NotationError>` and the inverse `algebraic(Move) ->
  String`. `command.rs` delegates to it, which removes the drifted
  copy. The `notation()` helper and the hardcoded `h8()` constructors
  in historydemo are replaced by calls to this module.
- `src/repl.rs` — `process_line`, `print_state`, and
  `print_search_info` from sessiondemo, moved into the library. The
  extraction changes one detail: the printing helpers *return*
  `Vec<String>` instead of calling `println!` directly. The same code
  then serves the demo binary and the line-mode fallback in
  sub-chapter e. Sessiondemo keeps only argument handling and printing.
- `tests/common/mod.rs` — the shared test kit: `mv`, `play`, `step`,
  `event_type`, one AI-config builder, and event-assertion helpers. All
  four existing test files use it. The kit documents one deliberate
  difference from `AiConfig::default()`: the test configuration uses
  200 simulations (the default is 400) to keep the test suite fast.
  One builder, one comment, instead of two copies with different names.
- Own tests: notation round-trips and error cases; repl behavior tests
  that cover what sessiondemo exercised implicitly.
- Gate: the full existing test suite passes with unchanged behavior.
  This sub-chapter moves code; it does not change what the code does.

### b — The study-table renderer (pure)

*Aspect: rendering as a function, not a side effect.*

- `src/tui/render.rs` gains `render_study(snapshot, seats, events,
  styled) -> Vec<String>` and its helpers: board with hoshi points and
  last-move marker, side panel (seats, ply, branch options), event
  list. All functions are pure. The `styled: bool` parameter selects
  ANSI styling, so the same function serves the alternate screen and
  plain pipes. The frame description also includes an optional board
  cursor and an optional command-line buffer; the shell (sub-chapter d)
  needs both.
- Own tests: star points on empty cells, last-move marker, side-panel
  contents, branch-option rendering, and styled/unstyled equivalence
  (same lines except for escape sequences).

### c — Swap2 as a session phase

*Aspect: the opening is session state, not a separate program.*

The engine's `Swap2<State>` typestate makes invalid transitions
impossible at compile time, but it consumes `self` on each transition,
so it cannot undo. The session layer provides both properties: it keeps
a replay log of `(Move, Color)` placements and rebuilds the typestate
after an undo. The log is the source of truth. The typestate is a
validated projection of the log.

- `SessionCore` gains an opening phase: a session can start with Swap2
  in progress instead of at an empty tree.
- Commands during the opening use the same parser and the same command
  language as everything else. There is no separate single-letter
  interpreter. The vocabulary: `play h8 [color]` places the next stone
  (the optional color token overrides the default alternation), `back`
  removes the last placement, `done` ends a placement phase (the
  session validates the stone counts), `black` / `white` choose a color
  in a choice phase, `more` declines the first choice and starts the
  two-stone phase. In normal mode, Enter, `u`, `d`, `b`, `w`, `m`
  translate to the same commands.
- Events: `StonePlaced`, `OpeningPhaseChanged`, `OpeningFinished` — the
  renderer receives everything it needs to draw the ceremony without
  knowing its rules.
- Own tests, all headless: full protocol paths (take black, take white,
  place two more then choose), undo in the middle of a phase, count
  validation at the end of a placement phase, occupied-square
  rejection, and the invariant that the finished opening starts with
  White to move.

### d — The alternate-screen shell

*Aspect: the TUI is a renderer plus an input-translation layer.*

- `src/tui/screen.rs` — the `Screen` trait with one method:
  `draw(&mut self, frame: &Frame) -> io::Result<()>`. `AltScreen`
  implements it with crossterm: `queue!`-batched commands, absolute
  cursor positioning, and an RAII guard that enables raw mode and the
  alternate screen on entry and restores the terminal on drop,
  including on panic.
- `src/tui/input.rs` — the input-translation layer, pure and
  headless-testable:
  - `InputMode { Normal, CommandLine(String) }` — vim style. `:` opens
    the command line, Enter executes it, Esc cancels it, Backspace
    edits it.
  - A board cursor. Arrow keys and `hjkl` move it; bounds are checked
    with `saturating_sub`.
  - `handle_key(key, state, core) -> InputOutcome` — normal mode maps
    Enter to `play` at the cursor (human seat) or `step` (AI seat),
    `u` to `back`, `?` to help, `q` to quit. Command-line mode parses
    the entered text with `command::parse`.
- `src/tui/study.rs` — one loop: draw the frame, get the next input
  outcome, execute, repeat. Input comes from an `InputSource`
  abstraction; the full-screen source reads key events with
  `event::read` and runs the state machine from `input.rs`.
- Own tests: mode transitions, cursor bounds, key-to-command mapping,
  command-line editing (Backspace, Esc), and a scripted key-event
  session through a `TestScreen` that records frames. No terminal is
  involved in any of these tests.

### e — The line-mode fallback

*Aspect: a fallback is a second implementation of a small trait, not a
second copy of the loop.*

- `LineScreen`: `draw` prints the frame with `println!`. Approximately
  40 lines including comments. Its input side reads complete lines and
  parses them with the repl code from sub-chapter a, so sessiondemo and
  the fallback share one code path.
- Selection rule: if stdout is a terminal, use `AltScreen`; otherwise
  use `LineScreen`. Piped output, CI, and redirected sessions continue
  to work.
- Own tests: a scripted session through `LineScreen` asserts the exact
  printed transcript; the selection rule is tested as a pure function
  of the terminal check.

### f — The binary

*Aspect: wiring and the user-facing surface.*

- `src/main.rs`: the clap front-end — `play` (Swap2 unless
  `--no-swap2`, then the table), `selfplay` (both seats AI, Enter
  steps), argument validation, `SessionInit` construction. The
  sub-chapters a–e contain all the logic; this file only validates and
  dispatches.
- The README gains the usage section.
- Capstone: a scripted selfplay session and a human-versus-tactics game
  transcript, both executed as headless tests.

## Cross-cutting decisions

1. **Input model: vim-style modal.** The full-screen shell uses raw
   mode with a normal mode (single keys) and a `:` command line. The
   fallback, sessiondemo, and scripts use canonical-mode line input.
   Both paths produce the same `Command` values and execute in the
   same session core. Every input path is testable without a terminal.
2. **Duplication is removed by extraction.** One loop, one parser, one
   notation module, one test kit, two implementations of a deliberately
   small `Screen` trait.
3. **The engine Swap2 typestate is reused.** The session wraps it
   behind a replay log. This gives compile-time-validated transitions
   and undo. The Swap2 protocol is not re-implemented in the cli crate.
4. **Commenting standard.** This is tutorial code. Every non-trivial
   reasoning step and every idiom gets an internal `//` comment:
   the RAII guard, the replay-log rebuild, `queue!` batching,
   `saturating_sub` on cursor motion, why `styled` is a parameter and
   not a feature flag. Doc comments describe what an item does;
   internal comments explain why the code does it that way. For this
   chapter, more comments are always better.
5. **Solution-file format.** Each solution presents new code as
   annotated snippets with line numbers. The git diff between
   sub-chapters defines which code is new. Solutions do not quote whole
   files and do not show raw diffs.
6. **Language standard.** All tutorial text and all code comments in
   this chapter follow ASD-STE100: short sentences, one meaning per
   word, active voice, no figurative language.

## Chapter layout

```text
05-the-real-table.md                index: what the table is, links to the sub-chapters
05-the-real-table/
  00-design.md                      this paper
  00-tui-excursion.md               the crossterm side track (read before sub-chapter d)
  a-refactor/
    refactor.md                      tutorial
    refactor-solution.md             solution
  b-renderer/
    renderer.md
    renderer-solution.md
  c-swap2/
    swap2.md
    swap2-solution.md
  d-shell/
    shell.md
    shell-solution.md
  e-fallback/
    fallback.md
    fallback-solution.md
  f-binary/
    binary.md
    binary-solution.md
```

## Status

All questions are decided. This paper is the contract for the
implementation of the six sub-chapters.
