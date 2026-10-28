# Sub-chapter 05d — The alternate-screen shell: implementation plan

This document turns the walkthrough in `shell.md` into a construction
order. It names the files, structs, enums, traits, methods, and
functions to write. It states the behavioral contract for each artifact
and the helpers to reuse. It does not contain bodies.

## How to use this document

1. Read `shell.md` first. This plan mirrors steps 0 through 7 and TDD
   groups A through E.
2. Work one step at a time. Write tests before code.
3. Watch each test fail red, then pass green.
4. Open `shell-solution.md` only if stuck for more than twenty minutes,
   or after the sub-chapter is complete.
5. Every step lists a gate. The three project gates come at the end.

---

## Step 0 — `screen.rs`: the `Screen` trait and the `AltScreen` RAII guard

### 0.1 Add the module declaration

In `src/tui/mod.rs`, add `pub mod screen;`. `render` and `opening`
already exist. Do not add `input` or `study` yet.

### 0.2 Create `src/tui/screen.rs`

#### The `Screen` trait

Write a public trait `Screen`:

```rust
pub trait Screen {
    fn draw(&mut self, frame: &[String]) -> io::Result<()>;
    fn styled(&self) -> bool { false }
}
```

`draw` takes the screen and a shared slice of frame lines and returns
`io::Result<()>`.

`styled` defaults to `false` so test recordings and pipes stay plain.
`AltScreen` overrides it to `true`.

#### The `AltScreen` guard

Write a public unit struct `AltScreen`.

Write an inherent `impl AltScreen` block with:

- `pub fn enter() -> io::Result<Self>`

  Enable raw mode, enter the alternate screen, and hide the cursor.

Write `impl Drop for AltScreen`:

- Restore the terminal in fixed order: show the cursor, leave the
  alternate screen, then disable raw mode. Ignore errors with `let _
  = ...`.

#### Implement `Screen` for `AltScreen`

- `draw`: clear the whole screen, queue every frame line at its absolute
  position, and flush stdout once. Batch cursor moves and prints with
  `queue!`, then call `flush`. Do not diff against the previous frame.
- `styled`: return `true`.

### 0.3 Gate

Run `cargo check -p cli`. This step has no headless test; the guard is a
terminal-only contract.

---

## Step 1 — `render.rs`: frame pieces for the shell

### 1.1 Extend `StudyView`

In `src/tui/render.rs`, add a private field to `StudyView`:

```rust
message: Option<&'a str>,
```

Initialize it to `None` in `StudyView::new`.

Add a public chainable setter `message`:

- Takes `self` by value and a shared string slice.
- Stores it in the `message` field and returns `self`.
- Mark it `#[must_use]`.

### 1.2 Extend `render_study`

Change the bottom-line rule:

- If `command_line` is `Some`, append `:{buffer}`.
- Else if `message` is `Some`, append the message text.
- Else append no bottom line.

The command line wins over the message line.

### 1.3 Ceremony fallback on the board

`SessionCore::board()` stays empty during a Swap2 ceremony. Ceremony
stones live in the opening snapshot's placement list.

Change the signatures of `study_board_lines` and `study_cell` so each
also accepts `Option<&OpeningSnapshot>`.

In `study_cell`:

- First look up the stone with `board.stone_at(mv)`.
- If that returns `None` and an opening snapshot is present, scan the
  snapshot's `placements` vector for a pair whose move equals `mv`. If
  found, use the paired color.
- Use the resulting stone to choose the glyph, last-move brackets, and
  cursor marker as before. Ceremony stones get no last-move brackets.

### 1.4 Ceremony block in the side panel

In `render_side_panel`, the game-state block is only correct when no
ceremony is in progress.

Refactor that block into a private helper `position_lines(lines, board,
snapshot)` that appends the same lines.

Add a private helper `ceremony_lines(lines, opening)` that appends:

- `Swap2: <phase>`.
- `Stones: <list>` or `Stones: (none)`.
- At `Placing3` or `Placing2`: `Next: <color>` and
  `Enter place · u undo · d done`.
- At `FirstChoice`: `b black · w white · m more`.
- At `FinalChoice`: `b black · w white`.

No `Next:` line appears at a choice phase.

In `render_side_panel`, after the seat lines, dispatch on
`snapshot.opening`:

- `Some(opening)` → `ceremony_lines`.
- `None` → `position_lines`.

Keep the rest of the panel unchanged.

### 1.5 Tests — group A

Write these four tests in the `tests` module of `src/tui/render.rs`:

- `the_ceremony_draws_its_placements_as_stones`

  Arrange a Swap2 hotseat core, place two stones through the public
  API, render a frame with no cursor, and assert the board row shows
  the two stones as `x` and `o` at the expected byte offsets with no
  last-move brackets.
- `the_ceremony_panel_shows_phase_next_color_and_keys`

  Render a fresh Swap2 hotseat core and assert the panel shows
  `Placing3`, an empty stones marker, `Next: Black`, and the placement
  keys. Place one stone and assert the panel updates to show the stone
  list and `Next: White`, with no `To move:` line.
- `the_choice_phase_panel_shows_the_choice_keys_and_no_next_stone`

  Place three stones in a Swap2 hotseat core, call `Command::Done`, and
  render. Assert the panel shows `FirstChoice`, the choice keys, and no
  `Next:` line.
- `the_message_is_the_bottom_line_unless_the_command_line_is_open`

  Render a plain hotseat frame, then render with a message, and assert
  the message adds exactly one bottom line. Then render with both a
  message and a command-line buffer, and assert the bottom line is the
  buffer behind a colon, not the message.

### 1.6 Gate

Run `cargo test -p cli --lib render::tests`.

---

## Step 2 — `input.rs`: normal mode, cursor, and global keys

### 2.1 Add the module declaration

In `src/tui/mod.rs`, add `pub mod input;`. Create `src/tui/input.rs`.

### 2.2 Vocabulary enums and state

Write a public enum `InputMode`:

```rust
pub enum InputMode {
    Normal,
    CommandLine(String),
}
```

Write a public struct `InputState`:

```rust
pub struct InputState {
    pub mode: InputMode,
    pub cursor: Move,
}
```

Write an inherent `impl InputState` block with `pub fn new() -> Self`.
It returns `Normal` mode with the cursor on `h8` (`Move::new(7, 7)`).

Write `impl Default for InputState` that returns `InputState::new()`.

### 2.3 Outcomes

Write a public enum `InputOutcome`:

```rust
pub enum InputOutcome {
    Command(Command),
    Help,
    Quit,
    Message(String),
    None,
}
```

### 2.4 Top-level dispatch

Write `handle_key(key, state, core)` that dispatches on `state.mode`:
`Normal` → `handle_normal_key`; `CommandLine` → `handle_command_line_key`.

### 2.5 Global keys in normal mode

Write a private function `handle_normal_key`:

- Check global keys first. They mean the same thing in every phase.
- Up/down arrows and `k`/`j` move the cursor one row.
- Left/right arrows and `h`/`l` move the cursor one column.
- `:` opens the command line (`InputMode::CommandLine(String::new())`).
- `?` returns `InputOutcome::Help`.
- `q` returns `InputOutcome::Quit`.
- Any other key falls through to phase-dependent dispatch.

Write a private function `move_cursor(cursor, dr, dc)`:

- Clamp the new row and column inside the 15×15 board.
- Use `saturating_sub` for `-1` deltas and `saturating_add(...).min(14)`
  for `+1` deltas.
- Assign `Move::new(row, col)` to the cursor. The clamped coordinates
  are always valid.

After global keys, `handle_normal_key` dispatches on
`core.snapshot().opening`: `Some` → `opening_key`, `None` → `playing_key`.

### 2.6 Tests — group B

- `arrows_and_hjkl_move_the_cursor`

  Start from `h8` in a playing core and press Up, Left, `j`, and `l`.
  Assert the cursor moves one step in the expected direction each time.
- `the_cursor_clamps_at_the_board_edges`

  Place the cursor at `(0, 0)` and press `k` and Left; assert it stays
  at `(0, 0)`. Place it at `(14, 14)` and press `j` and Right; assert
  it stays at `(14, 14)`.
- `the_global_keys_work_in_every_phase`

  For playing, placement-phase, first-choice, and final-choice cores,
  assert `?` returns `Help`, `q` returns `Quit`, and `:` returns `None`
  and sets the mode to `CommandLine(String::new())`. Close the line
  with Esc between phases.

### 2.7 Gate

Run `cargo test -p cli --lib input::tests`. The three new tests must
pass.

---

## Step 3 — `input.rs`: phase-dependent keys

### 3.1 The `playing_key` helper

Write `playing_key(key, state, core)` called when
`core.snapshot().opening` is `None`:
  - Get `core.board().to_move()`.
  - If `core.occupant(side).is_human()` is true, return
    `Command::Play { mv: state.cursor }`.
  - Otherwise return `Command::Step`.
- `Char('u')` → `Command::Back { n: 1 }`.
- All other keys → `InputOutcome::None`.

### 3.2 The `opening_key` helper

Write `opening_key(key, state, opening)` called when
`core.snapshot().opening` is `Some`. Dispatch on `opening.phase`:

  `Placing3` and `Placing2`:
  - `Enter` → `Command::Play { mv: state.cursor }`.
  - `Char('u')` → `Command::Back { n: 1 }`.
  - `Char('d')` → `Command::Done`.
  - `Char('b')` and `Char('w')` → `InputOutcome::None`.
  - `Char('m')` → `InputOutcome::Message` with the constant text.

  `FirstChoice`:
  - `Char('b')` → `Command::ChooseColor { color: Color::Black }`.
  - `Char('w')` → `Command::ChooseColor { color: Color::White }`.
  - `Char('m')` → `Command::More`.
  - `Enter` and `Char('u')` → `InputOutcome::None`.

  `FinalChoice`:
  - `Char('b')` → `Command::ChooseColor { color: Color::Black }`.
  - `Char('w')` → `Command::ChooseColor { color: Color::White }`.
  - `Char('m')` → `InputOutcome::Message` with the constant text.
  - `Enter` and `Char('u')` → `InputOutcome::None`.

### 3.3 Shared message text

Define a private module-level constant:

```rust
const MORE_ONLY_AT_FIRST_CHOICE: &str = "'more' applies only at the first Swap2 choice";
```

### 3.4 Tests — group C

Build ceremony cores through the public session API:
`SessionInit::new_hotseat().with_swap2(true)`, `Command::Play`,
`Command::Done`, and `Command::More`. Use `testkit::mv` for moves.

- `during_play_enter_plays_at_the_cursor_for_a_human_seat`

  Set the cursor to a non-center square in a human-vs-human playing
  core, press Enter, and assert the outcome is `Command::Play` at that
  square.
- `during_play_enter_steps_for_an_ai_seat`

  Release Black to the AI in a playing core, press Enter, and assert
  the outcome is `Command::Step`.
- `during_play_u_is_back_and_the_ceremony_keys_are_unmapped`

  In a playing core, assert `u` returns `Command::Back { n: 1 }` and
  that `d`, `b`, `w`, and `m` return `InputOutcome::None`.
- `at_a_placement_phase_enter_places_the_next_stone`

  In a placement-phase core, press Enter and assert the outcome is
  `Command::Play` at the cursor.
- `at_a_placement_phase_b_and_w_are_unmapped`

  For both `Placing3` and `Placing2` cores, assert `b` and `w` return
  `InputOutcome::None`.
- `at_a_placement_phase_u_undoes_and_d_is_done`

  For both `Placing3` and `Placing2` cores, assert `u` returns
  `Command::Back { n: 1 }` and `d` returns `Command::Done`.
- `at_a_placement_phase_m_is_a_message_not_a_command`

  In a placement-phase core, press `m` and assert the outcome is
  `InputOutcome::Message` containing the constant text.
- `at_the_first_choice_b_w_choose_and_m_is_more`

  In a `FirstChoice` core, assert `b` and `w` return the matching
  `ChooseColor` command and `m` returns `Command::More`.
- `at_the_final_choice_b_w_choose_and_m_is_a_message`

  In a `FinalChoice` core, assert `b` returns
  `Command::ChooseColor { color: Color::Black }` and `m` returns
  `InputOutcome::Message` containing the constant text.
- `at_a_choice_phase_enter_and_u_are_unmapped`

  In a `FirstChoice` core, assert Enter and `u` return
  `InputOutcome::None`.

### 3.5 Gate

Run `cargo test -p cli --lib input::tests`. The thirteen input tests
must pass.

---

## Step 4 — `input.rs`: the command-line editor

### 4.1 The `handle_command_line_key` helper

Write `handle_command_line_key(key, state)`. Use `std::mem::replace` to take the `CommandLine(buffer)` value out of
  `state.mode` and put `InputMode::Normal` in its place.
- Match on the key code:
  - `Char(c)`: push the character, restore
    `InputMode::CommandLine(buffer)`, return `None`.
  - `Backspace`: pop one character if not empty, restore
    `InputMode::CommandLine(buffer)`, return `None`.
  - `Esc`: return `None`. The mode is already `Normal`.
  - `Enter`: trim the buffer. If empty, return `None`. Otherwise parse
    with `command::parse`. On success return `Command(cmd)`; on error
    return `Message(err.to_string())`. The mode stays `Normal`.
  - Any other key: restore `InputMode::CommandLine(buffer)` and return
    `None`.

### 4.2 Tests — group D

- `the_command_line_appends_and_backspace_edits`

  Open the line with `:`, type `pla`, and assert the mode is
  `CommandLine("pla")`. Press Backspace and assert it becomes
  `CommandLine("pl")`. Press Backspace until the buffer is empty and
  assert it stays empty without panicking.
- `esc_cancels_the_command_line`

  Open the line, type `play h8`, press Esc, and assert the outcome is
  `None` and the mode is `Normal`.
- `enter_on_an_empty_command_line_returns_to_normal`

  Open the line and press Enter without typing. Assert the outcome is
  `None` and the mode is `Normal`.
- `enter_parses_the_buffer_with_the_shared_grammar`

  Open the line, type `play h8`, press Enter, and assert the outcome is
  `Command::Play` at `h8` and the mode is `Normal`.
- `a_parse_error_is_a_message_and_closes_the_line`

  Open the line, type `play`, press Enter, and assert the outcome is
  `InputOutcome::Message` containing `Parse error` and the mode is
  `Normal`.
- `the_typed_quit_and_help_are_commands_for_the_loop_to_intercept`

  Open the line, type `quit`, press Enter, and assert the outcome is
  `Command::Quit`. Open the line again, type `help`, press Enter, and
  assert the outcome is `Command::Help`.

### 4.3 Gate

Run `cargo test -p cli --lib input::tests`. All nineteen input tests
must pass.

---

## Step 5 — `study.rs`: the loop, input source, and run

### 5.1 Add the module declaration

In `src/tui/mod.rs`, add `pub mod study;`. Create `src/tui/study.rs`.

### 5.2 The `InputSource` trait

Write a public trait `InputSource`:

```rust
pub trait InputSource {
    fn next_outcome(&mut self, state: &mut InputState, core: &SessionCore)
        -> io::Result<InputOutcome>;
    fn wait_any_key(&mut self) -> io::Result<()>;
}
```

`next_outcome` produces an outcome; `wait_any_key` discards one input.

### 5.3 The `RawInput` source

Write a public unit struct `RawInput` and implement `InputSource`:

- `next_outcome` calls `read_key_press`, then `handle_key`, and returns
  the outcome.
- `wait_any_key` calls `read_key_press` and discards the result.

### 5.4 Read one key press

Write a private function `read_key_press`:

- Returns `io::Result<KeyEvent>`.
- Loops reading crossterm events with `event::read()`.
- Returns the key when the event is a key event and its `kind` is
  `KeyEventKind::Press`. Skips other events.

### 5.5 The `study_loop`

Write a public generic function `study_loop(core, screen, input, hint)`:

- Type parameters: `S: Screen`, `I: InputSource`.
- Returns `io::Result<()>`.
- Loop state:
  - `InputState::new()`.
  - Empty `Vec<Event>`.
  - `Option<String>` message initialized from `hint`.
- Loop body:
  1. `build_frame(core, &state, &events, message.as_deref(), screen.styled())`.
  2. `screen.draw(&frame)?`.
  3. `input.next_outcome(&mut state, core)?`, then match.

Outcome handling:

- `None`: do nothing.
- `Message(text)`: store the text as the message.
- `Help` and `Command(Command::Help)`: call `show_help(screen, input)?`.
- `Quit` and `Command(Command::Quit)`: return `Ok(())`.
- `Command(cmd)`: call `core.execute(cmd)`. On success, pass new events
  to `record_events`. On error, store `err.to_string()` as the message.

### 5.6 Build the frame

Write a private function `build_frame(core, state, events, message, styled)`:

- Returns `Vec<String>`.
- Start with `StudyView::new(core, events).cursor(state.cursor).styled(styled)`.
- If `state.mode` is `CommandLine(buffer)`, call `command_line(buffer)`.
- If `state.mode` is `Normal` and `message` is `Some(text)`, call
  `message(text)`.
- Return `render_study(&view)`.

### 5.7 Record recent events

Define a private constant:

```rust
const EVENT_CAP: usize = 20;
```

Write a private function `record_events(events, message, new_events)`:

- If `new_events` is not empty, set `*message` to the display text of
  the last event.
- Append all new events with `extend_from_slice`.
- If the vector length exceeds `EVENT_CAP`, drop the oldest entries.
  Compute the excess with `saturating_sub` and remove it with `drain`.



### 5.8 Help frame

Write a private function `show_help(screen, input)`:

- Generic over `S: Screen` and `I: InputSource`.
- Draws `help_frame()` and calls `input.wait_any_key()`.

Write a private function `help_frame`:

- Returns `Vec<String>`.
- Starts with the lines of `repl::HELP`.
- Adds a blank separator, the lines of `NORMAL_MODE_KEYS`, another blank
  separator, and the dismissal line
  `Press any key to return to the table.`.

Define a module-level constant `NORMAL_MODE_KEYS` as a raw string that
lists every normal-mode key and its meaning, matching the table in
`shell.md` word for word.

### 5.9 The `run` entry point

Write a public function `run(init, hint)`:

- Returns `io::Result<()>`.
- If `io::stdout().is_terminal()` is false, return an `io::Error` of
  kind `Unsupported` with a message that says the study table needs a
  terminal and the line-mode fallback is sub-chapter e.
- Otherwise:
  - Bind `AltScreen::enter()?` to a local variable that stays alive
    until the loop returns.
  - Create `RawInput`.
  - Create `SessionCore::new(init)`.
  - Call `study_loop` with these values and `hint`.

### 5.10 Test doubles

In the `tests` module of `src/tui/study.rs`, write:

`TestScreen`:

- Struct owning `Vec<Vec<String>> frames`.
- Constructor `new` for an empty recording.
- `impl Screen`: `draw` pushes a copy of the frame and returns `Ok(())`;
  `styled` keeps the default `false`.

`ScriptedInput`:

- Struct owning `VecDeque<KeyEvent> keys`.
- Constructor `new(keys)`.
- Private `pop` returning the front key or an `io::Error` of kind
  `UnexpectedEof` with the message `the script ran out of keys`.
- `impl InputSource`: `next_outcome` pops a key, calls `handle_key`, and
  returns the outcome; `wait_any_key` pops a key and discards it.

Test helpers:

- `key(code)` with no modifiers.
- `char_key(c)` for `Char` keys.
- `typed_line(text)` opens `:`, appends one key per character, and adds
  Enter.
- `run_scripted(core, keys)` creates a `TestScreen`, a `ScriptedInput`,
  runs `study_loop`, and returns the final core and screen.

### 5.11 Tests — group E

- `the_loop_plays_moves_and_quits_cleanly`

  Run a hotseat core with Enter, Right, Enter, `q`. Assert four frames,
  the two expected stones on the board, and that the last frame shows
  the last event.
- `the_loop_runs_the_ceremony_on_alternated_placements`

  Run a Swap2 core with Enter, Right, Enter, Right, Enter, `d`, `b`,
  `q`. Assert the opening finishes, the board contains the three placed
  stones, and one frame shows `Next: White`.
- `a_wrong_phase_key_lands_on_the_message_line`

  Run a Swap2 core with `m`, `q`. Assert the second frame shows the
  `'more' applies only at the first Swap2 choice` message.
- `a_failed_command_lands_on_the_message_line`

  Run a hotseat core with Enter, Enter, `q`. Assert a later frame shows
  `Illegal Move` on the message line.
- `help_is_a_full_screen_frame_and_any_key_dismisses_it`

  Run a hotseat core with `?`, `x`, `q`. Assert three frames, that the
  second frame contains the typed command help, the normal-mode key
  table, and the dismissal note, and that the dismissed key played no
  move.
- `typed_help_and_quit_do_what_the_keys_do`

  Type `help` on the command line, dismiss with `z`, then type `quit`.
  Assert the expected frame count and that the typed help produced a
  help frame.
- `a_parse_error_on_the_command_line_lands_on_the_message_line`

  Type `play` on the command line, then press `q`. Assert the recorded
  text contains `Parse error`.
- `an_exhausted_script_is_an_error_not_a_silent_quit`

  Run a hotseat core with a single Enter key and no quit. Assert that
  `run_scripted` returns an error.

### 5.12 Gate

Run `cargo test -p cli`. The loop tests and all earlier tests must pass.

---

## Step 6 — Project gates and transcript diffs

From the `gomoku/` directory, run:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Then run the transcript capture and diff from sub-chapter a's
checklist. Neither script uses the shell; both diffs must be empty.

---

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `src/tui/mod.rs` | `pub mod screen;` | declared |
| `src/tui/mod.rs` | `pub mod input;` | declared |
| `src/tui/mod.rs` | `pub mod study;` | declared |
| `src/tui/screen.rs` | `Screen` trait | `draw` required, `styled` provided default `false` |
| `src/tui/screen.rs` | `AltScreen` struct | unit struct |
| `src/tui/screen.rs` | `AltScreen::enter` | raw mode + alternate screen + hide cursor |
| `src/tui/screen.rs` | `Drop for AltScreen` | show cursor, leave alternate screen, disable raw mode; errors ignored |
| `src/tui/screen.rs` | `Screen for AltScreen` | batch clear + absolute lines + flush; `styled` returns `true` |
| `src/tui/render.rs` | `StudyView.message` | `Option<&str>` field |
| `src/tui/render.rs` | `StudyView::message` | chainable setter, `#[must_use]` |
| `src/tui/render.rs` | `render_study` | command line wins, then message, then no line |
| `src/tui/render.rs` | `study_board_lines` | accepts `Option<&OpeningSnapshot>` |
| `src/tui/render.rs` | `study_cell` | board first, then ceremony placement fallback |
| `src/tui/render.rs` | `position_lines` | extracted game-state block |
| `src/tui/render.rs` | `ceremony_lines` | phase, stones, optional next color, guidance |
| `src/tui/render.rs` | `render_side_panel` | dispatches between ceremony and position |
| `src/tui/render.rs` | group A tests | all pass |
| `src/tui/input.rs` | `InputMode` enum | `Normal`, `CommandLine(String)` |
| `src/tui/input.rs` | `InputState` struct | `mode`, `cursor` |
| `src/tui/input.rs` | `InputState::new` | normal mode, cursor at `h8` |
| `src/tui/input.rs` | `Default for InputState` | calls `new()` |
| `src/tui/input.rs` | `InputOutcome` enum | five variants |
| `src/tui/input.rs` | `handle_key` | dispatches on mode |
| `src/tui/input.rs` | `handle_normal_key` | global keys first, then phase dispatch |
| `src/tui/input.rs` | `move_cursor` | clamps with `saturating_sub` / `min(14)` |
| `src/tui/input.rs` | `playing_key` | play/step for Enter, back for `u` |
| `src/tui/input.rs` | `opening_key` | phase table from `shell.md` |
| `src/tui/input.rs` | `MORE_ONLY_AT_FIRST_CHOICE` | exact constant text |
| `src/tui/input.rs` | `handle_command_line_key` | edit, cancel, parse via `command::parse` |
| `src/tui/input.rs` | groups B, C, D tests | all pass |
| `src/tui/study.rs` | `InputSource` trait | two methods |
| `src/tui/study.rs` | `RawInput` struct | implements `InputSource` |
| `src/tui/study.rs` | `read_key_press` | filters `KeyEventKind::Press` |
| `src/tui/study.rs` | `study_loop` | draw, read, apply, repeat |
| `src/tui/study.rs` | `build_frame` | assembles `StudyView` |
| `src/tui/study.rs` | `EVENT_CAP` | `20` |
| `src/tui/study.rs` | `record_events` | caps at twenty, message from last event |
| `src/tui/study.rs` | `show_help` | draws help, waits for key |
| `src/tui/study.rs` | `help_frame` | `repl::HELP` + key table + note |
| `src/tui/study.rs` | `NORMAL_MODE_KEYS` | constant text |
| `src/tui/study.rs` | `run` | terminal check, `AltScreen`, `RawInput`, loop |
| `src/tui/study.rs` | `TestScreen` | records frames, unstyled |
| `src/tui/study.rs` | `ScriptedInput` | plays back keys, errors on exhaustion |
| `src/tui/study.rs` | group E tests | all pass |


