# Sub-chapter 05d — The alternate-screen shell

Status: IMPLEMENTED (reference). This sub-chapter adds the full-screen shell. It adds three modules to `src/tui/` and extends the renderer. It changes no session behavior.

## 1 — The intention

An occupant plays and studies at the table full-screen. The terminal shows the board, the seats, the recent events, and a bottom line for messages. Single keys do the frequent work: arrows or `h j k l` move a cursor over the board, `Enter` plays (human seat) or steps (AI seat), `u` takes back, `d` ends a placement round, `b` and `w` choose a color at a choice phase, `m` asks for two more stones, `?` shows help, `q` quits. `:` opens a command line that accepts the full command language from chapter 02. However the program ends — quit, error, or panic — the terminal is exactly as it was before.

That intention gives four specification properties:

1. Every program state has a frame: board, side panel, bottom line.
2. Every key has exactly one meaning per session phase. Keys are only translations to `Command` values. Typed text goes through the one shared grammar (`command::parse`). No second grammar exists.
3. The shell repeats one cycle — paint, read one input, apply it — until quit. Errors land on the message line; they never crash the loop.
4. The terminal is restored on every exit path, panic included.

## 2 — Context

Read [the design paper](../00-design.md) first. Its section "d — The alternate-screen shell" assigns this sub-chapter, and its header records the decided input model (option C, vim-style modal input) and the normal-mode key set.

Then read [the TUI excursion](../00-tui-excursion.md). It is required reading, not background. Everything this sub-chapter does with the terminal is built from its pieces: raw mode, the alternate screen, the RAII guard, `queue!`-batched repaints, and the loop shape (render everything, read one event, update state, repeat). The excursion ends with the one idea this sub-chapter adds on its own: modal input.

Sub-chapters b and c supply the two ends this sub-chapter connects. The renderer ([b-renderer](../b-renderer/renderer.md)) turns a `StudyView` into lines. The session ([c-swap2](../c-swap2/swap2.md)) knows the phase, and the phase is what makes the modal keys safe.

The dependency rule is one direction only: rendering depends on the session; the shell depends on rendering; never the other way. Crossterm calls live only in `src/tui/`.

| Term | Definition |
|------|------------|
| **alternate screen** | A second terminal buffer. Entering it hides the user's scrollback; leaving it restores the previous content. |
| **raw mode** | Terminal input mode without buffering and without echo. Every keypress reaches the program immediately. |
| **RAII guard** | A value whose `Drop` restores a resource. Here: the terminal state. `Drop` runs on every exit path, panic included. |
| **`Screen`** | The trait with one method, `draw(&mut self, frame: &[String])`. A destination for a finished frame. |
| **normal mode** | The input mode where single keys act immediately. |
| **command line** | The input mode opened with `:`. A small editor over a text buffer; `Enter` parses the buffer with the shared grammar. |
| **`InputOutcome`** | What one keypress means: a command, help, quit, a message, or a state-only change. |
| **`InputSource`** | The trait that supplies input outcomes to the loop. The terminal implementation reads crossterm events; tests read a script. |
| **board cursor** | The square the next `Enter` acts on. Arrows and `hjkl` move it. |
| **message line** | The bottom line of the frame in normal mode: the last event summary or the last error text. |
| **phase-dependent key** | A key whose meaning depends on the session phase. `u` removes a placement during the ceremony and goes back one ply afterwards. |

## 3 — The specification, as tests

The tests are the specification. Each test is named with one behavior sentence: occupant does X → system changes state or reports an error. The tests are ordered along the execution path of the future program. Most tests are headless; the guard has no test because it needs a terminal and is a documented contract instead.

### Group A — the first frame (render, 4 tests)

- `the_ceremony_draws_its_placements_as_stones`
- `the_ceremony_panel_shows_phase_next_color_and_keys`
- `the_choice_phase_panel_shows_the_choice_keys_and_no_next_stone`
- `the_message_is_the_bottom_line_unless_the_command_line_is_open`

### Group B — the first keys (input, global + cursor, 3 tests)

- `arrows_and_hjkl_move_the_cursor`
- `the_cursor_clamps_at_the_board_edges`
- `the_global_keys_work_in_every_phase`

### Group C — keys per phase (input, 10 tests)

- `during_play_enter_plays_at_the_cursor_for_a_human_seat`
- `during_play_enter_steps_for_an_ai_seat`
- `during_play_u_is_back_and_the_ceremony_keys_are_unmapped`
- `at_a_placement_phase_enter_places_the_next_stone`
- `at_a_placement_phase_b_and_w_are_unmapped`
- `at_a_placement_phase_u_undoes_and_d_is_done`
- `at_a_placement_phase_m_is_a_message_not_a_command`
- `at_the_first_choice_b_w_choose_and_m_is_more`
- `at_the_final_choice_b_w_choose_and_m_is_a_message`
- `at_a_choice_phase_enter_and_u_are_unmapped`

### Group D — the command line (input, 6 tests)

- `the_command_line_appends_and_backspace_edits`
- `esc_cancels_the_command_line`
- `enter_on_an_empty_command_line_returns_to_normal`
- `enter_parses_the_buffer_with_the_shared_grammar`
- `a_parse_error_is_a_message_and_closes_the_line`
- `the_typed_quit_and_help_are_commands_for_the_loop_to_intercept`

### Group E — the whole loop (study, 8 tests, headless via `TestScreen` + `ScriptedInput`)

- `the_loop_plays_moves_and_quits_cleanly`
- `the_loop_runs_the_ceremony_on_alternated_placements`
- `a_wrong_phase_key_lands_on_the_message_line`
- `a_failed_command_lands_on_the_message_line`
- `help_is_a_full_screen_frame_and_any_key_dismisses_it`
- `typed_help_and_quit_do_what_the_keys_do`
- `a_parse_error_on_the_command_line_lands_on_the_message_line`
- `an_exhausted_script_is_an_error_not_a_silent_quit`

## 4 — The walkthrough: one run of the program

Put yourself in debugging mode. The program is about to run; follow it. Each step introduces one concept at the moment execution first touches it. The "why" lives inside the step where it matters.

### Step 0 — `run(init, hint)` starts

`run` is the entry point. It builds a `SessionCore` from `init`. It checks that stdout is a terminal. If stdout is not a terminal, `run` returns a clear `Unsupported` I/O error. The line-mode dispatch for non-terminal output is sub-chapter e; this sub-chapter does not implement it.

If stdout is a terminal, `run` calls `AltScreen::enter()`. That call enables raw mode, enters the alternate screen, and hides the cursor. The result is an `AltScreen` value. The value carries no data. Its only job is to stay alive until the shell is done.

The idiom is RAII / `Drop`. The `Drop` implementation for `AltScreen` restores the terminal. `Drop` runs on every exit path: a normal return, an error propagated with `?`, and a panic. The Drop order is fixed: show the cursor and leave the alternate screen first, then disable raw mode. Both show/leave actions are escape sequences on stdout; raw mode is a terminal attribute. Errors inside `Drop` are ignored with `let _`. If restoration fails, there is nothing sensible left to do. Panicking inside `Drop` during an unwind would abort the process, which is the opposite of what the guard exists for.

`src/tui/screen.rs`:

```rust
pub struct AltScreen;

impl AltScreen {
    pub fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(Self)
    }
}

impl Drop for AltScreen {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
```

No test pins the guard; it needs a terminal. The contract is documented in the code and in this sub-chapter.

### Step 1 — the first frame

`run` now has a core and a live screen. It calls `study_loop`. Inside the loop, the first thing that happens is `build_frame`.

`build_frame` assembles a `StudyView`. `StudyView` is the render input. It already existed in sub-chapter b; this sub-chapter extends it with chainable setters for the new display state:

- `cursor(mv)` — the board cursor.
- `command_line(buffer)` — the bottom line while the command line is open.
- `message(text)` — the bottom line in normal mode.
- `styled(flag)` — whether the frame carries ANSI styling.

`src/tui/render.rs`:

```rust
pub struct StudyView<'a> { /* core, recent_events, cursor, command_line, message, styled */ }

impl<'a> StudyView<'a> {
    pub fn new(core: &'a SessionCore, recent_events: &'a [Event]) -> Self { /* ... */ }
    pub fn cursor(mut self, mv: Move) -> Self { self.cursor = Some(mv); self }
    pub fn command_line(mut self, buffer: &'a str) -> Self { /* ... */ }
    pub fn message(mut self, text: &'a str) -> Self { /* ... */ }
    pub fn styled(mut self, styled: bool) -> Self { /* ... */ }
}
```

The idiom is chainable setters. The caller states only what it has; no call site is forced to pass `None, None, false`.

`render_study` turns the `StudyView` into `Vec<String>`. The board draws ceremony placements as stones. `SessionCore::board()` stays empty during the ceremony by design (sub-chapter c: the placements are not game moves), so the cell lookup falls back to the opening snapshot's placement list. The list holds at most five entries; a linear scan is fast enough for a per-frame draw.

The side panel replaces the game-state block with a ceremony block when a ceremony is in progress. The ceremony block shows the phase, the placements in order, `Next: <Color>` at a placement phase, and the guidance line for the phase. A choice phase has no next stone, so the panel shows no `Next:` line there.

The bottom line rule: while the command line is open, the buffer wins over the message. In normal mode, the message line carries the last event summary or the last error text.

The tests in group A pin this frame.

### Step 2 — the frame reaches the terminal

The loop calls `screen.draw(&frame)`. `Screen` is a trait with one required method:

```rust
pub trait Screen {
    fn draw(&mut self, frame: &[String]) -> io::Result<()>;
    fn styled(&self) -> bool { false }
}
```

`Screen` is a trait for two reasons. First, tests replace it with `TestScreen`, which records frames in a `Vec`. Second, sub-chapter e adds a line-mode screen; the trait lets the same loop serve both screens. The default `styled()` returns `false`, so test recordings and pipes get plain text without opting out. `AltScreen` overrides `styled()` to return `true`.

`AltScreen::draw` clears the screen, queues every line at its absolute position (`MoveTo(0, y)`), then flushes once. The idiom is the `queue!` / `flush` split. Painting a frame is dozens of cursor moves and prints. Sending them one by one is what makes a picture flicker; queueing them into one buffer and flushing once sends them as one batch. Absolute positioning makes the paint independent of where a previous frame left the cursor.

There is no diffing. The renderer repaints the whole frame for every keypress. A 15×15 frame is about twenty short lines, and terminals are fast.

`src/tui/screen.rs`:

```rust
impl Screen for AltScreen {
    fn draw(&mut self, frame: &[String]) -> io::Result<()> {
        let mut out = io::stdout();
        queue!(out, Clear(ClearType::All))?;
        for (y, line) in frame.iter().enumerate() {
            queue!(out, MoveTo(0, y as u16), Print(line))?;
        }
        out.flush()
    }

    fn styled(&self) -> bool { true }
}
```

### Step 3 — one input

After the frame is on screen, the loop reads one input through the `InputSource` trait:

```rust
pub trait InputSource {
    fn next_outcome(&mut self, state: &mut InputState, core: &SessionCore)
        -> io::Result<InputOutcome>;
    fn wait_any_key(&mut self) -> io::Result<()>;
}
```

`RawInput` is the terminal implementation. It reads crossterm events and passes key events to `handle_key`. It filters for `KeyEventKind::Press`. Some platforms (Windows) report release and repeat events; without the filter, every key would act twice there. Non-key events are skipped. The loop repaints the whole frame on every key, so a resize needs no special handling.

The idiom is a read loop plus a `let`-chain filter:

```rust
fn read_key_press() -> io::Result<KeyEvent> {
    loop {
        if let CrosstermEvent::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            return Ok(key);
        }
    }
}
```

### Step 4 — the key becomes a meaning

`handle_key` is a pure function: a `KeyEvent`, the mutable `InputState`, and the read-only `SessionCore` go in; an `InputOutcome` comes out. It performs no I/O.

`InputMode` is an enum with two variants: `Normal` and `CommandLine(String)`. `InputState` holds the mode and the board cursor. `InputOutcome` is an enum with five variants: `Command(Command)`, `Help`, `Quit`, `Message(String)`, and `None`.

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

pub fn handle_key(key: KeyEvent, state: &mut InputState, core: &SessionCore)
    -> InputOutcome;
```

The state machine is a total function: every key maps to an outcome. `InputState::new()` starts in normal mode with the cursor on the center point h8.

Normal-mode keys are checked in two stages. Global keys come first; they mean the same thing in every phase:

- arrows and `hjkl` move the cursor;
- `:` opens the command line;
- `?` is `Help`;
- `q` is `Quit`.

Cursor motion clamps at the board edges. Up and left use `saturating_sub`; down and right use `.min(14)`. The clamped coordinates are always valid, so `Move::new` cannot fail.

After the global keys, phase-dependent keys dispatch on `core.snapshot().opening`. The opening snapshot tells the ceremony phase, or `None` for normal play:

| Phase | `Enter` | `u` | `d` | `b` | `w` | `m` |
|-------|---------|-----|-----|-----|-----|-----|
| Placing3 / Placing2 | place next stone | back | done | unmapped | unmapped | message |
| FirstChoice | unmapped | unmapped | unmapped | choose Black | choose White | more |
| FinalChoice | unmapped | unmapped | unmapped | choose Black | choose White | message |
| Playing | play / step | back | unmapped | unmapped | unmapped | unmapped |

The `Enter` key during play is `play` at the cursor for a human seat and `step` for an AI seat. The seat table decides; the session validates the result. An occupied square or a finished game produces an error that the loop shows on the message line.

Modifiers are not inspected anywhere. `?` arrives as `Char('?')` with the Shift modifier on most layouts; matching the code alone keeps the mapping layout-independent. Phase-dependent keys are safe because the session always knows the phase. Wrong-phase real commands, such as `m` at a placement phase, become `Message` with the session's own error sentence. Unmapped keys are ignored.

The tests in groups B and C pin this table.

### Step 5 — the command line is a small editor

`:` opens the command line. While the command line is open, `handle_key` dispatches to `handle_command_line_key`. Characters append. `Backspace` pops a character; on an empty buffer it is a no-op. `Esc` cancels and returns to normal mode. `Enter` on an empty buffer closes the line quietly. `Enter` on text parses the buffer with `command::parse`.

One grammar exists, and it lives in `command::parse`. The command line reuses it. A second parser in the shell would drift from the first, the way the reference build's TUI parser drifted from its line parser.

A successful parse produces `InputOutcome::Command(cmd)`. A parse error produces `InputOutcome::Message(err.to_string())` and closes the line. Both outcomes return the mode to normal.

The idiom here is `std::mem::replace`. The buffer must come out of the enum before the mode can change, because `state.mode` is borrowed mutably. `mem::replace` takes the old value and puts a new one in its place in one move:

```rust
let InputMode::CommandLine(mut buffer) =
    std::mem::replace(&mut state.mode, InputMode::Normal)
else {
    unreachable!("dispatched only in command-line mode")
};
```

The tests in group D pin the editor behavior.

### Step 6 — the loop applies the outcome

`study_loop` is generic over `Screen` and `InputSource`:

```rust
pub fn study_loop<S: Screen, I: InputSource>(
    core: &mut SessionCore,
    screen: &mut S,
    input: &mut I,
    hint: Option<&str>,
) -> io::Result<()>;
```

The loop owns three pieces of shell state: the `InputState`, a capped list of recent events, and the message line. `hint`, when given, is the first message line.

One iteration: build the frame, draw it, read one outcome, apply it.

- `InputOutcome::None`: nothing.
- `InputOutcome::Message(text)`: the message line takes the text.
- `InputOutcome::Help`, or `InputOutcome::Command(Command::Help)`: draw the help frame, call `wait_any_key`, then return to the table. The help frame reuses `repl::HELP` for the typed command language, then appends the normal-mode key table, then a dismissal note. Reusing `repl::HELP` keeps the help screen and the line-mode `help` command from drifting apart.
- `InputOutcome::Quit`, or `InputOutcome::Command(Command::Quit)`: return from the loop.
- `InputOutcome::Command(cmd)`: call `core.execute(cmd)`. On success, the new events append to the capped list; the message line becomes the last event's summary. On error, the message line becomes the error text.

The event cap is 20 entries; the panel shows the newest 5. The oldest events drop first. Typed `help`/`quit` and keyed `?`/`q` do the same thing because the session core treats `Help` and `Quit` as no-ops and the loop intercepts both.

The tests in group E run the loop headless. `TestScreen` records every frame it is asked to draw:

```rust
struct TestScreen { frames: Vec<Vec<String>> }

impl Screen for TestScreen {
    fn draw(&mut self, frame: &[String]) -> io::Result<()> {
        self.frames.push(frame.to_vec());
        Ok(())
    }
}
```

`ScriptedInput` plays back a list of key events. Running out of keys is an error, not a silent quit:

```rust
struct ScriptedInput { keys: VecDeque<KeyEvent> }

impl InputSource for ScriptedInput {
    fn next_outcome(&mut self, state: &mut InputState, core: &SessionCore)
        -> io::Result<InputOutcome>
    {
        Ok(handle_key(self.pop()?, state, core))
    }
}
```

### Step 7 — the end

When `Quit` arrives, `study_loop` returns. `run` returns. The `AltScreen` value drops out of scope. Its `Drop` restores the terminal: show cursor, leave alternate screen, disable raw mode. The same restoration happens if an I/O error propagated up with `?` or if a panic unwound the stack. That is the RAII guarantee from step 0.

## 5 — TDD checklist

The rhythm is red, green, extend. Run from `gomoku/`.

0. `screen.rs` skeleton plus the `AltScreen` guard. `cargo check -p cli` is the gate. There is no headless test for the guard.
1. Renderer frame tests (group A) → `StudyView` setters plus ceremony drawing.
2. Input global keys and cursor (group B) → `InputMode`, `InputState`, `InputOutcome`, and the global part of `handle_key`.
3. Phase-dependent keys (group C) → `opening_key` and `playing_key`. Build ceremony cores in tests through the public session API.
4. Command line (group D) → `handle_command_line_key`.
5. Loop (group E) → `InputSource`, `RawInput`, `study_loop`, help frame, and `run`.
6. The three gates plus both transcript diffs byte-identical.

## 6 — Solution (opt-in)

The added and changed code, as annotated snippets with line numbers, is in [shell-solution.md](shell-solution.md). Open it only if you have been stuck for more than twenty minutes, or after you have finished the sub-chapter and want to compare your work.

## 7 — Done when

* `Screen` has one method, and `AltScreen` restores the terminal from `Drop` on every exit path, with errors ignored inside `Drop`.
* `input.rs` translates every key of the modal table headless: cursor motion with clamping, the phase-dependent keys for both placement phases, both choice phases, and play, the command-line editor, and the parse-error path.
* The command line parses with `command::parse`. No second grammar exists.
* The renderer draws ceremony placements as stones, shows the ceremony block in the panel, and renders the message line.
* The loop runs headless in tests over a recording screen and a scripted input; `run` rejects a non-terminal stdout with a clear error.
* All three gates pass, and both demo transcripts are unchanged. (Result here: 130 tests, up from 99. The lib holds 78, up from 47: +19 input tests, +8 loop tests, +4 render tests.)

## 8 — Verification commands

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Plus the transcript capture and diff from sub-chapter a's checklist. Neither script uses the shell; both diffs must be empty.

## 9 — References

* [00-design.md](../00-design.md) — the contract for all six sub-chapters, including the input model decision and the Swap2 key set.
* [00-tui-excursion.md](../00-tui-excursion.md) — required reading: raw mode, the alternate screen, the RAII guard, `queue!` batching, and the loop shape.
* [b-renderer/renderer.md](../b-renderer/renderer.md) — the renderer this sub-chapter extends; the `styled` contract and the view struct.
* [c-swap2/swap2.md](../c-swap2/swap2.md) — the session phases the keys dispatch on, and the event vocabulary the loop consumes.
* crossterm's API documentation: <https://docs.rs/crossterm>.

Next: sub-chapter e — the line-mode fallback.
