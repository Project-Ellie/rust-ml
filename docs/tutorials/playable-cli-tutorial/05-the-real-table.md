# Chapter 05 — The real table

## Abstract

This chapter puts the study table on screen. Everything so far was
headless: `historydemo` printed ASCII trees and `sessiondemo` spoke in
events. Now the `gomoku` binary grows a real rendered board with a
side panel, star points, a last-move marker, and the command loop that
drives `SessionCore` from the terminal.

The payoff is two commands:

* `gomoku play --ai tactics` — you sit down at the table.
* `gomoku selfplay --seed 7` — the AI sits in both seats and you step
  it forward with Enter.

To get there you will:

* add the `study` module as a thin crossterm shell that owns a
  `SessionCore` and translates terminal lines into `Command`s and
  `Event`s;
* extend the renderer with star points, the last-move marker, a
  coloured gutter, and a right-hand side panel;
* wire the `play` and `selfplay` clap subcommands in `main.rs`,
  including the legacy-flow dispatch and the `--color`-without-`--ai`
  validation;
* implement the non-tty line-mode fallback so the same binary can be
  scripted for demos, chapter quotes, and tests.

The chapter's mental-mapping centerpiece is the **shell doctrine**:
the core stays ignorant of crossterm, and all visual state — including
overlay mode — lives in the shell. The capstone is a real captured
non-tty session, quoted below.

## Glossary

| Term | Definition |
|------|------------|
| **Shell** | The crossterm front-end in `src/tui/study.rs`. It renders `Snapshot`s and `Event`s; the core never knows it exists. |
| **Study-table screen** | The alternate-screen TUI: rendered board + side panel + status line + command prompt. |
| **Empty-line step** | Pressing Enter on an empty input line emits `Command::Step` when the side-to-move seat is AI, ignored otherwise. |
| **Non-tty fallback** | The line-mode path taken when `stdout` is not a terminal. Produces the same content as the alternate-screen path but as plain text, so it can be piped or redirected. |
| **Hoshi** | The nine star points on a 15×15 Gomoku board, at rows and columns 3, 7, and 11. |
| **Last-move marker** | Brackets around the cell of the most recently played stone, so the eye can follow the flow. |
| **Side panel** | The right-hand readout showing the seat table, ply, variation path, branch options, last events, and key hints. |
| **Seat table** | The `Black:` / `White:` lines in the side panel, showing each seat's occupant and, for an AI, its simulation budget. |

## Context

By the end of chapter 03 the AI could sit at the table and play through
`Step`; chapter 04 added take/release mid-game, per-seat `sim`s, the
opening schedule, and silent branching. All of that lives in the
headless `SessionCore`. This chapter is the **shell**: the thin
crossterm layer that makes the core playable in a terminal.

The design authority is the playable CLI spec §4 (core/shell), §8
(clap startup interface), and §10 (the opening). The work is confined
to the `cli` crate and, unlike previous chapters, touches only the
binary and the `tui/` modules — the library's command/event contract is
already complete.

Two important constraints from the spec govern this chapter:

1. **The core stays ignorant of crossterm.** `SessionCore` already
   takes `Command`s and returns `Event`s; the shell must not add new
   commands that require core changes (with one tiny, justified
   exception: exposing seat accessors on `SessionCore` so the panel can
   read them).
2. **No raw-key input.** The shell reads lines, just like
   `sessiondemo`. Every action a user can take is expressible as a line
   that `command::parse` understands.

## Intention

1. Create `gomoku/crates/cli/src/tui/study.rs`:
   * own a `SessionCore` and drive it from a read-eval-paint loop;
   * on an empty line, issue `Command::Step` when the side-to-move seat
     is AI;
   * branch on `IsTerminal`: alternate-screen path for real terminals,
     line-mode path for pipes;
   * run the Swap2 ceremony first when it is enabled, then hand its
     resulting `Board` into `SessionInit`.
2. Extend `gomoku/crates/cli/src/tui/render.rs`:
   * render the study board with star points on empty hoshi cells;
   * bracket the last move;
   * colour the coordinate gutter in grey when styling is on;
   * render the side panel from `SessionCore::snapshot()` plus seat
     accessors.
3. Extend `gomoku/crates/cli/src/main.rs`:
   * parse `play` (default) and `selfplay` subcommands via clap;
   * route `--puzzle`, `--plain`, and `--engine naive` to the legacy
     flows unchanged;
   * route the modern study path through `study::run` or
     `study::run_with_swap2`;
   * validate that `--color` requires `--ai`, that `--engine naive`
     cannot be combined with `--ai` or `selfplay`, and that legacy
     flags are rejected before `selfplay`.
4. Add the small seat-accessor API that `study.rs` and `render.rs`
   need on `SessionCore` (`black`, `white`, `occupant`,
   `default_config`).
5. Keep every existing legacy test green; add snapshot tests for the
   new study rendering.

Observable done-state: `gomoku play --ai tactics --no-swap2` draws the
board and side panel; `gomoku selfplay --sims 200 --seed 7` steps
through AI-vs-AI with Enter; the captured non-tty output matches the
frames below; the TDD checklist passes; the gates are clean.

## Mental mapping

### The shell doctrine

`SessionCore` is the headless state machine. `study.rs` is the shell.
The contract between them is exactly the public API:

```rust
impl SessionCore {
    pub fn new(init: SessionInit) -> Self;
    pub fn execute(&mut self, cmd: Command) -> Result<Vec<Event>, CommandError>;
    pub fn snapshot(&self) -> Snapshot;
    pub fn board(&self) -> Board;
    pub fn search_info(&self) -> Option<&SearchInfo>;
    pub fn black(&self) -> &Occupant;
    pub fn white(&self) -> &Occupant;
    pub fn occupant(&self, color: Color) -> &Occupant;
    pub fn default_config(&self) -> &AiConfig;
}
```

The shell does three things and nothing else:

1. **Read a line** from stdin.
2. **Translate** it: `parse_study_input` turns the line into a
   `Parsed` value (quit, help, command, or error string).
3. **Paint**: call `render_study(core, events, styled)` and print it.

All visual state belongs in the shell. The policy overlay mode (chapter
06) will be a shell variable; the value strip toggle will be a shell
variable. The *data* for those overlays comes from the core via
`search_info()`, but the decision of what to display is shell state.

This is why the core can later be wrapped in a REST handler or a GUI
without redesign: it already speaks commands and events, not crossterm.

### Enter = Step when the AI is to move

In the alternate-screen loop, the user types lines. An **empty line**
(just Enter) is a convenience:

```rust
fn empty_line_command(core: &SessionCore) -> Option<Command> {
    if core.board().status() != Status::Ongoing {
        return None;
    }
    let side = core.board().to_move();
    if core.occupant(side).is_ai() {
        Some(Command::Step)
    } else {
        None
    }
}
```

If the side-to-move seat is AI, Enter issues `Step`. If it is human,
Enter is ignored — the user must type coordinates. This makes
AI-vs-AI self-play a relaxing sequence of Enter presses.

### Why the non-tty fallback matters

`std::io::IsTerminal` lets the shell ask whether stdout is a real
terminal. If it is not, the shell takes the **line-mode** path:

* no alternate screen;
* no cursor hiding or clearing;
* each frame is printed as plain text, exactly like `sessiondemo`.

This is not an afterthought. It exists for three reasons:

1. **Scripted demos.** Chapter quotes and acceptance transcripts can
   be generated by piping input into the binary.
2. **Chapter capture.** This very chapter quotes a non-tty session;
   without the fallback, capturing the output would require terminal
   emulation.
3. **Future test harnesses.** The rendering functions are unit-tested,
   but end-to-end shell behaviour can also be exercised by piping
   commands and inspecting stdout.

The fallback uses the same `render_study` function as the
alternate-screen path; the only difference is whether crossterm queues
it or `println!` emits it.

### Rendering upgrades

The study board reuses the 19-line frame from the existing renderer but
adds three details:

* **Star points:** empty cells at the hoshi intersections (3/7/11) show
  `+` instead of `.`.
* **Last-move marker:** the most recent stone is bracketed: `[x]` or
  `[o]`.
* **Coloured gutter:** the coordinate ruler and row labels are grey in
  styled mode.

The **side panel** is rendered as a separate `Vec<String>` and joined
line-by-line to the right of the board. It shows:

* the seat table (`Black: AI(400)` / `White: Human`);
* ply count and the last eight moves of the variation path;
* branch options when the cursor is at a branch point;
* the last five events;
* key hints for help, quit, and the upcoming overlays.

Because the panel is joined after the board, the overall width grows
but the board layout does not jump.

### main.rs dispatch

`main.rs` now parses clap subcommands and dispatches:

* `--puzzle` → puzzle flow (unchanged).
* `--plain` or `--engine naive` → legacy hotseat UI (unchanged).
* `play` with modern flags → `study::run_with_swap2` (Swap2 on) or
  `study::run` (Swap2 off).
* `selfplay` → both seats AI, `--no-swap2` implied, `study::run` with an
  "Enter = step" hint.

The validation rules are:

* `--engine naive` with `--ai` or `selfplay` → exit 2.
* `--color` without `--ai` → exit 2.
* `--color` with `selfplay` → exit 2.
* Legacy `play` flags placed before `selfplay` → exit 2.

These are startup errors, not runtime errors, so they are reported
before any crossterm state is entered.

### The Swap2 handoff

Swap2 is a front-end ceremony. When it finishes, its resulting
`Board` becomes the root of the variation tree via `SessionInit`:

```rust
let make_init = |board: Board| build_play_init(&play.search, board);
if swap2 {
    study::run_with_swap2(make_init, None)
} else {
    study::run(make_init(Board::new()), None)
}
```

The core does not know that Swap2 happened; it only knows the opening
position it was given.

## Low-level design

### Files changed vs chapter 04

The diff from chapter 04 (`ce6c4dd`) to this chapter (`a2e5373`) is
confined to the `cli` crate:

```text
$ git diff ce6c4dd..a2e5373 --stat -- gomoku/crates/cli
 gomoku/crates/cli/src/main.rs       | 150 +++++++++---
 gomoku/crates/cli/src/session.rs    |  34 ++-
 gomoku/crates/cli/src/tui/mod.rs    |   1 +
 gomoku/crates/cli/src/tui/render.rs | 243 +++++++++++++++++++-
 gomoku/crates/cli/src/tui/study.rs  | 442 ++++++++++++++++++++++++++++++++++++
 5 files changed, 835 insertions(+), 35 deletions(-)
```

`src/tui/board.rs`, `src/tui/input.rs`, `src/tui/opening.rs`,
`src/tui/puzzle.rs`, `src/tui/ui.rs`, `src/lib.rs`, `src/command.rs`,
`src/actor.rs`, `src/history.rs`, `src/fmt.rs`, the demo binaries, and
the integration tests are unchanged from chapter 04.

### `src/main.rs`

`main.rs` becomes a clap parser and dispatcher. Key structures:

* `CliArgs` with a flattened `PlayArgs` for the legacy default
  invocation and an optional `Commands` subcommand.
* `Commands::Play(PlayArgs)` and `Commands::Selfplay(SearchArgs)`.
* `PlayArgs` contains both the legacy flags (`engine`, `plain`,
  `no_swap2`, `puzzle`, `index`) and a flattened `SearchArgs`.
* `SearchArgs` holds the AI/search flags (`ai`, `color`, `sims`,
  `c_puct`, `temperature`, `noise`, `black_sims`, `white_sims`,
  `seed`).

The dispatch flow:

1. Parse `CliArgs`.
2. Normalize `args.cmd == None` to `Mode::Play` with `args.play`.
3. For `Selfplay`, reject any legacy flags that were placed before the
   subcommand.
4. In `run_play`, handle puzzle mode, then legacy `--plain`/`naive`,
   then the study-table path.
5. In `run_selfplay`, build a `SessionInit` with both seats AI and call
   `study::run`.

`ai_config_from` turns `SearchArgs` into an `AiConfig`, recording
whether `--temperature` or `--noise` was explicitly set so the opening
schedule can apply only when they were not.

### `src/tui/study.rs`

The new study module has two public entry points:

```rust
pub fn run(init: SessionInit, hint: Option<&str>) -> io::Result<()>;
pub fn run_with_swap2(
    build_init: impl FnOnce(Board) -> SessionInit,
    hint: Option<&str>,
) -> io::Result<()>;
```

`run` selects between `run_alt` and `run_line` based on
`IsTerminal::is_terminal(&io::stdout())`. Both loops share the same
logic:

* draw/print the current frame;
* read a line;
* if empty, possibly issue `Step`;
* otherwise parse and execute;
* update the event tail and message.

The event tail keeps the last 20 events so the panel can show recent
history without recomputing it.

`run_with_swap2` runs the Swap2 ceremony first. The ceremony itself also
has alternate-screen and line-mode paths, using the existing
`OpeningState` and render helpers. Its result is handed to `build_init`,
which constructs the `SessionInit` for the study table.

### `src/tui/render.rs`

New functions:

```rust
pub fn render_study(core: &SessionCore, recent_events: &[Event], styled: bool) -> Vec<String>;
```

`render_study` splits the work into `render_study_board` and
`render_side_panel`, then joins them line by line.

`study_board_lines` builds the 19-line board with:

* `HOSHI = [3, 7, 11]`;
* `+` on empty hoshi cells;
* `[x]` / `[o]` for the last move;
* grey gutter when `styled` is true.

`render_side_panel` builds the right-hand panel from the snapshot and
seat accessors. It shows branch options only when
`snapshot.branch_options.len() > 1`.

### `src/session.rs`

The only core change is exposing seat accessors that the shell needs:

```rust
pub fn black(&self) -> &Occupant;
pub fn white(&self) -> &Occupant;
pub fn occupant(&self, color: Color) -> &Occupant;
pub fn default_config(&self) -> &AiConfig;
```

`occupant` was previously a private helper; it is now public. No other
behaviour changes.

## Solution (opt-in)

The complete reference files for this chapter are quoted verbatim in
[05-the-real-table/01-solution.md](05-the-real-table/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare your work.

## TDD checklist

Run from `gomoku/` with `cargo test -p cli`.

### `src/tui/render.rs`

Exact new `#[test]` function names from commit `a2e5373`:

1. `study_board_shows_star_points_on_empty_cells`
2. `study_board_marks_last_move`
3. `study_side_panel_shows_seats_and_ply`
4. `study_side_panel_shows_branch_options`

### `src/tui/study.rs`

Exact new `#[test]` function name from commit `a2e5373`:

1. `empty_line_step_only_when_ai_to_move`

### Existing tests

All tests from chapters 01–04 continue to pass and are part of the
chapter-05 gate. Notably:

* `tests/history.rs`, `tests/command.rs`, `tests/session.rs`, and
  `tests/actor.rs` are unchanged and must remain green.
* The legacy puzzle, plain, and naive-engine flows keep working.

### Red-green-refactor rhythm

1. **Red:** Create `src/tui/study.rs`, extend `src/tui/render.rs` and
   `src/main.rs`, add the `SessionCore` accessors, and add the snapshot
   tests. Run `cargo test -p cli`. Expect failures.
2. **Green:** Implement the contracts above until `cargo test -p cli`
   passes.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any issues.

## Capstone: the study table gets its wood

Run the non-tty demo in the reference worktree:

```text
cd gomoku
printf '7 7\nstep\nq\n' | cargo run -q -p cli --bin gomoku -- play --ai tactics --color white --no-swap2
```

Because `--color white` seats the human in White, Black is the AI. The
study table lets a human force a move even on an AI seat; the move is
credited to the seat's occupant. The three frames below are the real
captured output.

Initial frame:

```text
     0  1  2  3  4  5  6  7  8  9 10 11 12 13 14       -- Study table --
   +---------------------------------------------+     Black: AI(400)
 0 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 0   White: Human
 1 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 1   
 2 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 2   Ply: 0
 3 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 3   Path: (root)
 4 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 4   
 5 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 5   Last events:
 6 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 6     (none)
 7 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 7   
 8 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 8   h help · q quit
 9 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 9   p policy overlay (ch.06)
10 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 10  v value strip (ch.06)
11 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 11  
12 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 12  
13 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 13  
14 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 14  
   +---------------------------------------------+     
     0  1  2  3  4  5  6  7  8  9 10 11 12 13 14       
```

After the `7 7` line, the table credits the Black AI with the move.
The last-move marker brackets the stone, and the side panel updates:

```text
     0  1  2  3  4  5  6  7  8  9 10 11 12 13 14       -- Study table --
   +---------------------------------------------+     Black: AI(400)
 0 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 0   White: Human
 1 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 1   
 2 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 2   Ply: 1
 3 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 3   Path: (7, 7)
 4 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 4   
 5 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 5   Last events:
 6 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 6     Moved (7, 7) by Ai
 7 | .  .  .  +  .  .  . [x] .  .  .  +  .  .  . | 7   
 8 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 8   h help · q quit
 9 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 9   p policy overlay (ch.06)
10 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 10  v value strip (ch.06)
11 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 11  
12 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 12  
13 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 13  
14 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 14  
   +---------------------------------------------+     
     0  1  2  3  4  5  6  7  8  9 10 11 12 13 14       
```

After the `step` line, White is to move and White is human, so `Step`
returns the typed error:

```text
Step requires an AI seat to move; White is human
```

Now run self-play. Empty lines become `Step` because both seats are AI:

```text
printf '\n\nq\n' | cargo run -q -p cli --bin gomoku -- selfplay --sims 200 --seed 7
```

The first empty line makes Black search and play:

```text
     0  1  2  3  4  5  6  7  8  9 10 11 12 13 14       -- Study table --
   +---------------------------------------------+     Black: AI(200)
 0 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 0   White: AI(200)
 1 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 1   
 2 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 2   Ply: 1
 3 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 3   Path: (9, 8)
 4 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 4   
 5 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 5   Last events:
 6 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 6     Moved (9, 8) by Ai
 7 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 7   
 8 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 8   h help · q quit
 9 | .  .  .  .  .  .  .  . [x] .  .  .  .  .  . | 9   p policy overlay (ch.06)
10 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 10  v value strip (ch.06)
11 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 11  
12 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 12  
13 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 13  
14 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 14  
   +---------------------------------------------+     
     0  1  2  3  4  5  6  7  8  9 10 11 12 13 14       
```

The second empty line makes White answer:

```text
     0  1  2  3  4  5  6  7  8  9 10 11 12 13 14       -- Study table --
   +---------------------------------------------+     Black: AI(200)
 0 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 0   White: AI(200)
 1 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 1   
 2 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 2   Ply: 2
 3 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 3   Path: (9, 8) (10, 7)
 4 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 4   
 5 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 5   Last events:
 6 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 6     Moved (9, 8) by Ai
 7 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 7     Moved (10, 7) by Ai
 8 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 8   
 9 | .  .  .  .  .  .  .  .  x  .  .  .  .  .  . | 9   h help · q quit
10 | .  .  .  .  .  .  . [o] .  .  .  .  .  .  . | 10  p policy overlay (ch.06)
11 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 11  v value strip (ch.06)
12 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 12  
13 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 13  
14 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 14  
   +---------------------------------------------+     
     0  1  2  3  4  5  6  7  8  9 10 11 12 13 14       
```

The board is on screen, the side panel is populated, and the table
responds to both typed coordinates and empty-line stepping. The study
table has its wood.

## Done when

* `cargo test -p cli` passes from `gomoku/` (including the new render
  snapshot tests and all chapter-04 tests).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* `cargo run -q -p cli --bin gomoku -- play --ai tactics --no-swap2`
  draws the study table in a terminal.
* The non-tty demo commands above produce the quoted frames.
* Commit message for the code slice:
  `feat(cli): study-table TUI with side panel + clap subcommands`.

Next: [Chapter 06 — Seeing the mind](06-seeing-the-mind.md)

## References

* [`00-playable-cli-primer.md`](00-playable-cli-primer.md) — the study
  table, core/shell, commands/events, the four knobs, and the no-depth
  sidebar.
* [`README.md`](README.md) — chapter map and tutorial conventions.
* [`docs/plans/2026-10-17-playable-cli.md`](../../plans/2026-10-17-playable-cli.md)
  — Task 5 (this chapter).
* [`docs/specs/2026-10-17-playable-cli-design.md`](../../specs/2026-10-17-playable-cli-design.md)
  — §4 (core/shell), §8 (clap startup interface), and §10 (the opening).
* [`docs/tutorials/convolutions-tutorial/06-the-fork.md`](../convolutions-tutorial/06-the-fork.md)
  — chapter template and solution-file precedent.
