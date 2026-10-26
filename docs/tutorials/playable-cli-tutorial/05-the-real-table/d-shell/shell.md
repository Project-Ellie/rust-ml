# Sub-chapter 05d — The alternate-screen shell

Status: IMPLEMENTED. This sub-chapter adds the full-screen shell. It
adds three modules to `src/tui/` and extends the renderer. It changes
no session behavior.

## Abstract

The session is headless and the renderer is pure. This sub-chapter
puts both behind a terminal. Three modules do the work.
`src/tui/screen.rs` defines the `Screen` trait and `AltScreen`, the
crossterm implementation with an RAII guard that restores the terminal
on every exit path. `src/tui/input.rs` is the input-translation layer:
vim-style modal input as a pure state machine, keys in and outcomes
out, with every mapping unit-tested without a terminal.
`src/tui/study.rs` is one loop over a `Screen` and an `InputSource`:
draw the frame, translate one key, apply the outcome, repeat. The
renderer learns to draw the Swap2 ceremony: placements appear as
stones, and the side panel shows the phase, the next stone's color,
and the keys that apply.

## Glossary

| Term | Definition |
|------|------------|
| **Alternate screen** | A second terminal buffer. Entering it hides the user's scrollback; leaving it restores the previous content. |
| **Raw mode** | Terminal input mode without buffering and without echo. Every keypress reaches the program immediately. |
| **RAII guard** | A value whose `Drop` restores a resource. Here: the terminal state. `Drop` runs on every exit path, panic included. |
| **`Screen`** | The trait with one method, `draw(&mut self, frame: &[String])`. A destination for a finished frame. |
| **Normal mode** | The input mode where single keys act immediately. |
| **Command line** | The input mode opened with `:`. A small editor over a text buffer; Enter parses the buffer with the shared grammar. |
| **`InputOutcome`** | What one keypress means: a command, help, quit, a message, or a state-only change. |
| **`InputSource`** | The trait that supplies input outcomes to the loop. The terminal implementation reads crossterm events; tests read a script. |
| **Board cursor** | The square the next Enter acts on. Arrows and `hjkl` move it. |
| **Message line** | The bottom line of the frame in normal mode: the last event summary or the last error text. |
| **Phase-dependent key** | A key whose meaning depends on the session phase. `u` removes a placement during the ceremony and goes back one ply afterwards. |

## Context

Read [the design paper](../00-design.md) first. Its section
"d — The alternate-screen shell" assigns this sub-chapter, and its
header records the decided input model (option C, vim-style modal
input) and the normal-mode key set.

Then read [the TUI excursion](../00-tui-excursion.md). It is required
reading, not background. Everything this sub-chapter does with the
terminal is built from its pieces: raw mode, the alternate screen,
`event::read`, the RAII guard, `queue!`-batched repaints, and the
loop shape (render everything, read one event, update state, repeat).
The excursion ends with the one idea this sub-chapter adds on its
own: modal input.

Sub-chapters b and c supply the two ends this sub-chapter connects.
The renderer ([b-renderer](../b-renderer/renderer.md)) turns a
`StudyView` into lines. The session ([c-swap2](../c-swap2/swap2.md))
knows the phase, and the phase is what makes the modal keys safe.

## Intention

1. Create `src/tui/screen.rs`: the `Screen` trait and `AltScreen`.
   The guard enables raw mode, enters the alternate screen, and hides
   the cursor on entry; its `Drop` restores everything.
2. Create `src/tui/input.rs`: `InputMode`, `InputState`,
   `InputOutcome`, and `handle_key`. Pure and headless. Every key
   mapping gets a unit test.
3. Extend `src/tui/render.rs`: draw ceremony placements as stones,
   add the ceremony block to the side panel, and add the message line
   to `StudyView`.
4. Create `src/tui/study.rs`: the `InputSource` trait, the raw-mode
   implementation, the screen-agnostic `study_loop`, the help screen,
   and `run`, the full-screen entry point.
5. Test the loop headless: a `TestScreen` records frames and a
   `ScriptedInput` plays back key scripts. Assert on frames, session
   state, and the clean quit.
6. Prove that nothing else changed: the three gates pass, and the two
   scripted demo transcripts stay byte-identical.

## Mental mapping

### The shell is a translation layer

The design paper fixes the doctrine: keys are only translations to
`Command` values; the session core sees no difference. This
sub-chapter is where that doctrine pays. `input.rs` never executes
anything. It maps a key to a `Command`, or to a shell concern (help,
quit, a message), or to nothing. `study.rs` executes the commands and
owns nothing else. The session tests from sub-chapter c already cover
what the commands do; the shell tests cover only what the keys mean.

Two consequences follow:

1. **One grammar exists.** The command line parses with
   `command::parse`. A second parser in the shell would drift from
   the first, the way the reference build's TUI parser drifted from
   its line parser.
2. **The state machine touches no I/O.** `handle_key` takes a
   `KeyEvent` value, not a terminal. Every mapping runs in a unit
   test.

### Phase-dependent keys are safe because the session knows the phase

Normal mode has few keys, and the Swap2 ceremony wants several
actions: place, undo, done, choose, more. The design paper's answer:
one key carries one meaning per phase, and the session reports the
phase through the snapshot. `b` and `w` choose a color at a choice
phase and do nothing at a placement phase: the strict alternation
fixes the next stone's color, so there is nothing to select. `u`
removes a placement during the ceremony and goes back one ply
afterwards. The two meanings never collide, because the phases never
overlap.

The panel teaches the active keys at the point where the user needs
them: a placement phase shows `Enter place · u undo · d done`, a
choice phase shows `b black · w white · m more`. The help screen lists
every key with its phase.

Keys that do not apply fall into two groups. Unmapped keys (for
example `x` in normal mode) are ignored. Keys that name a real command
at the wrong phase get the message line: `m` at a placement phase or
at the final choice shows `'more' applies only at the first Swap2
choice`, the same sentence the session's typed error uses. One rule,
one sentence, from the input layer or from the session.

### No color selection exists at a placement phase

Sub-chapter c fixed the placement colors by strict alternation, so
there is no color to select while placing. `b`/`w` bind to the color
choice, which exists only at a choice phase; at a placement phase the
keys are unmapped and the panel's `Next:` line states the
alternation's color without a source label. The input tests pin the
unmapped keys at both placement phases, and the choice-phase binding
is the same keys' only meaning.

### The guard restores the terminal, always

Raw mode and the alternate screen are global, sticky terminal state.
A program that exits — normally, by error, or by panic — while they
are active leaves the user's terminal without echo and without
scrollback. `AltScreen` is a value whose `Drop` restores the
terminal, and `Drop` runs on every exit path, panic included. The
usage is one line at the top of `run`: `AltScreen::enter()?`.

Inside `Drop`, errors are ignored with `let _`. If restoration fails,
there is nothing sensible left to do. Panicking inside `Drop` during
an unwind would abort the process, which is the opposite of what the
guard exists for. The Drop order is fixed: show the cursor and leave
the alternate screen first (both are escape sequences on stdout),
then disable raw mode (a terminal attribute).

The guard's `Drop` is documented in comments and not tested: the
test needs a terminal, and the sub-chapter's tests run headless by
design.

### One trait for the screen, one trait for the input

The loop in `study.rs` is generic over `Screen` and `InputSource`.
The traits exist for two reasons at once:

1. **Tests.** A `TestScreen` records frames in a `Vec`; a
   `ScriptedInput` plays back a list of key events. The full loop —
   rendering, ceremony, help screen, errors, quit — runs in tests
   without a terminal.
2. **Sub-chapter e.** The line-mode fallback is a second `Screen`
   implementation, not a second copy of the loop. The selection rule
   (terminal or pipe) lands there; for now, `run` on a non-terminal
   returns a clear error instead of writing escape sequences into a
   pipe.

`Screen` also carries `styled()`, a defaulted method. Sub-chapter b
established that the styling choice belongs to the screen: the
alternate screen wants ANSI styling, pipes and test recordings want
plain text. The loop asks the screen and passes the answer to the
renderer.

### Batched paint, one flush

`AltScreen::draw` clears the screen, then queues every line at its
absolute position (`MoveTo(0, y)`), then flushes once. Painting a
frame is dozens of cursor moves and prints. Sending them one by one
is what makes a picture flicker; the `queue!` / `flush` split sends
them as one batch. Absolute positioning makes the paint independent
of where a previous frame left the cursor.

There is no diffing. The renderer repaints the whole frame for every
keypress. A 15×15 frame is about twenty short lines, and terminals
are fast.

## Low-level design

### `src/tui/screen.rs` (new)

```rust
pub trait Screen {
    fn draw(&mut self, frame: &[String]) -> io::Result<()>;
    fn styled(&self) -> bool { false }   // the alternate screen overrides
}

pub struct AltScreen;

impl AltScreen {
    pub fn enter() -> io::Result<Self>; // raw mode, alternate screen, cursor hidden
}

impl Drop for AltScreen { /* show cursor, leave alternate screen, raw mode off */ }

impl Screen for AltScreen { /* queue!-batched clear + lines, one flush */ }
```

`AltScreen` carries no data. The value's job is to stay alive until
the shell is done; its `Drop` is the point.

### `src/tui/input.rs` (new)

```rust
pub enum InputMode { Normal, CommandLine(String) }

pub struct InputState {
    pub mode: InputMode,
    pub cursor: Move,
}

pub enum InputOutcome {
    Command(Command),
    Help,
    Quit,
    Message(String),
    None,
}

pub fn handle_key(key: KeyEvent, state: &mut InputState, core: &SessionCore) -> InputOutcome;
```

The module reads crossterm's key vocabulary (`KeyEvent`) but performs
no I/O. `InputState::new()` starts in normal mode with the cursor on
the center point h8.

The normal-mode table, global keys first:

- Arrows and `hjkl` move the cursor. Up/left clamp with
  `saturating_sub`; down/right clamp with `min` at index 14. The
  clamped coordinates are always valid, so `Move::new` cannot fail.
- `:` opens the command line. `?` is `Help`. `q` is `Quit`.
- Phase-dependent keys dispatch on `core.snapshot().opening`:
  - Placement phases (Placing3, Placing2): Enter is `play` at the
    cursor (the strict alternation colors the stone), `u` is `back`
    (the session routes it to the replay log), `d` is `done`, `b`/`w`
    are unmapped, `m` is a message.
  - Choice phases (FirstChoice, FinalChoice): `b`/`w` choose a color,
    `m` is `more` at the first choice only and a message otherwise.
    Enter and `u` are not mapped.
  - Playing (no ceremony): Enter is `play` at the cursor when the
    side-to-move seat is human and `step` when it is AI, `u` is
    `back`. The ceremony keys are unmapped.

The command line is a small editor. Characters append to the buffer.
Backspace edits. Esc cancels to normal mode. Enter on an empty buffer
closes the line quietly. Enter on text parses the buffer with
`command::parse`: success is a `Command` outcome, a parse error is a
`Message` outcome with the parser's own text, and both close the
line. Modifiers are not inspected anywhere: `?` arrives as
`Char('?')` with Shift on most layouts, and matching the key code
alone keeps the mapping layout-independent.

### `src/tui/render.rs` (extended)

`StudyView` gains one optional input with a chainable setter:

- `message(&str)` — the bottom line of the frame in normal mode.
  While the command line is open it owns the bottom line and the
  message is not drawn.

When the snapshot carries a ceremony, the frame changes in two
places:

- The board draws the placements as stones. `SessionCore::board()`
  stays empty during the ceremony by design (sub-chapter c: the
  placements are not game moves), so the cell lookup falls back to
  the opening snapshot's placement list. The list holds at most five
  entries; a linear scan is fast enough for a per-frame draw.
- The side panel replaces the game-state block (ply, side to move,
  path) with the ceremony block: the phase, the placements in order,
  `Next: <Color>` at a placement phase, and the
  guidance line for the phase. A choice phase has no next stone, so
  the panel shows no `Next:` line there.

### `src/tui/study.rs` (new)

```rust
pub trait InputSource {
    fn next_outcome(&mut self, state: &mut InputState, core: &SessionCore)
        -> io::Result<InputOutcome>;
    fn wait_any_key(&mut self) -> io::Result<()>;
}

pub struct RawInput;  // reads event::read(), filters for key presses

pub fn run(init: SessionInit, hint: Option<&str>) -> io::Result<()>;

pub fn study_loop<S: Screen, I: InputSource>(
    core: &mut SessionCore,
    screen: &mut S,
    input: &mut I,
    hint: Option<&str>,
) -> io::Result<()>;
```

`study_loop` holds the shell state next to the core: the input state,
the capped event list (20 entries, oldest dropped first), and the
message line. One iteration: build the frame with `render_study`,
`screen.draw(frame)`, `input.next_outcome(...)`, apply the outcome:

- `Quit`, or a typed `quit` command: return.
- `Help`, or a typed `help` command: draw the help frame
  (`repl::HELP` plus the normal-mode key table), wait for any key,
  return to the table. The session core treats `Help` and `Quit` as
  no-ops, so the loop intercepts both; typed and keyed forms do the
  same thing.
- `Command(cmd)`: execute on the core. On success, the events append
  to the list, and the message becomes the last event's summary. On
  error, the message becomes the error text.
- `Message(text)`: the message line takes the text.
- `None`: nothing reaches the session.

`run` is the full-screen entry point. When stdout is not a terminal
it returns an `Unsupported` I/O error; the line-mode dispatch is
sub-chapter e's job. Otherwise it enters the `AltScreen` guard,
builds the core from `init`, and runs the loop over `RawInput`.
`hint`, when given, is the first message line.

`RawInput` filters crossterm events for `KeyEventKind::Press`. Some
platforms (Windows) also report release and repeat events; without
the filter every key would act twice there. Non-key events are
skipped: the loop repaints the whole frame on every key, so a resize
needs no special handling.

### Tests

- `src/tui/input.rs`: 19 unit tests. Every key mapping per phase
  (placement, both choices, playing; human and AI seats), the modal
  transitions, the cursor bounds at both corners, Backspace on an
  empty buffer, Esc, the empty-buffer Enter, the parse-error path,
  and the typed `quit`/`help` interception contract.
- `src/tui/study.rs`: 8 loop tests over `TestScreen` and
  `ScriptedInput`. They cover a two-move hotseat sequence, the full
  ceremony path (the placements land in the strict alternation's
  colors), the wrong-phase key message, the illegal-move message,
  help as a full-screen frame with any-key dismissal, typed help and
  quit, the parse-error message, and an exhausted script as an error
  instead of a silent quit.
- `src/tui/render.rs`: 4 new tests. Ceremony placements render as
  stones on an otherwise empty tree board; the panel shows the phase,
  the placements, the next color, and the guidance;
  a choice phase shows no `Next:` line; the message is the bottom
  line unless the command line is open.
- The `AltScreen` guard's Drop order is documented in comments and
  not tested: the test needs a terminal.

## Solution (opt-in)

The added and changed code, as annotated snippets with line numbers,
is in [shell-solution.md](shell-solution.md). Open it only if you
have been stuck for more than twenty minutes, or after you have
finished the sub-chapter and want to compare your work.

## TDD checklist

The rhythm is red, green, extend. Run from `gomoku/`.

1. **The screen.** Create `screen.rs` with the `Screen` trait and the
   `AltScreen` guard. There is nothing to test headless here yet;
   `cargo check -p cli` is the gate. Draw a frame by hand in a scratch
   binary if you want to see it work.
2. **Normal mode, global keys.** Write the cursor-motion tests
   (including both corners), the `:`/`?`/`q` tests, and watch them
   fail against a stub. Implement `InputMode`, `InputState`,
   `InputOutcome`, and the global part of `handle_key`. Green.
3. **Phase-dependent keys.** Add the per-phase tests: playing (human
   and AI seats), placement (including the unmapped `b`/`w` and the
   `m` message), first choice, final choice. Build the ceremony cores in
   the tests through the public session API. Implement `opening_key`
   and `playing_key`. Green.
4. **The command line.** Add the editing tests: append, Backspace
   (also on an empty buffer), Esc, empty Enter, parse success, parse
   error, typed `quit`/`help`. Implement `handle_command_line_key`.
   Green.
5. **The renderer.** Add the ceremony frame tests and the
   message-line test. Extend `StudyView`, the cell lookup, and the
   side panel. Green. The existing styled/unstyled equivalence test
   must keep passing.
6. **The loop.** Add the loop tests with `TestScreen` and
   `ScriptedInput`. Implement `InputSource`, `RawInput`,
   `study_loop`, the help frame, and `run`. Green.
7. **The gates.** Run all three gates and both transcript diffs.

## Verification commands

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Plus the transcript capture and diff from sub-chapter a's checklist.
Neither script uses the shell; both diffs must be empty.

## Done when

* `Screen` has one method, and `AltScreen` restores the terminal from
  `Drop` on every exit path, with errors ignored inside `Drop`.
* `input.rs` translates every key of the modal table headless: cursor
  motion with clamping, the phase-dependent keys for both placement
  phases, both choice phases, and play, the command-line editor, and
  the parse-error path.
* The command line parses with `command::parse`. No second grammar
  exists.
* The renderer draws ceremony placements as stones, shows the
  ceremony block in the panel, and renders the message line.
* The loop runs headless in tests over a recording screen and a
  scripted input; `run` rejects a non-terminal stdout with a clear
  error.
* All three gates pass, and both demo transcripts are unchanged.
  (Result here: 130 tests, up from 99. The lib holds 78, up from
  47: +19 input tests, +8 loop tests, +4 render tests.)

Next: sub-chapter e — the line-mode fallback.

## References

* [00-design.md](../00-design.md) — the contract for all six
  sub-chapters, including the input model decision and the Swap2 key
  set.
* [00-tui-excursion.md](../00-tui-excursion.md) — required reading:
  raw mode, the alternate screen, the RAII guard, `queue!` batching,
  and the loop shape.
* [b-renderer/renderer.md](../b-renderer/renderer.md) — the renderer
  this sub-chapter extends; the `styled` contract and the view
  struct.
* [c-swap2/swap2.md](../c-swap2/swap2.md) — the session phases the
  keys dispatch on, and the event vocabulary the loop consumes.
* crossterm's API documentation: <https://docs.rs/crossterm>.
