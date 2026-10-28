# Sub-chapter 05d — The alternate-screen shell: solution

> **Opt-in solution.** Read this only after you have tried the
> sub-chapter, or if you have been stuck for more than twenty minutes.
>
> This file quotes the code that sub-chapter d adds or changes, as
> annotated snippets with line numbers. Line numbers refer to the files
> after sub-chapter d is applied. Deletions are described in prose.
> Raw diffs are not shown. Verified green from `gomoku/`:
> `cargo test -p cli`, `cargo clippy --all-targets -- -D warnings`,
> and `cargo fmt --all -- --check`.

Scope is sub-chapter d only. `screen.rs` contains the `Screen` trait
and `AltScreen`; `LineScreen`, `LineInput`, and `select_shell` belong
to sub-chapter e and are omitted here, except for one forward
reference in `run`'s error message.

The order below mirrors the walkthrough steps 0–7: one run of the
program, from entry to terminal restoration.

## Step 0 — Entry and the RAII guard

The shell is wired into `src/tui/mod.rs`:

```rust
pub mod input;
pub mod opening;
pub mod render;
pub mod screen;
pub mod study;
```

`screen.rs` and `study.rs` are new. `input.rs`, `render.rs`,
`opening.rs`, and the other modules already existed.

The program enters through `run(init, hint)` in `src/tui/study.rs`.
`run` checks that stdout is a terminal. If stdout is not a terminal,
`run` returns a clear `Unsupported` I/O error. The line-mode fallback
for non-terminal output is sub-chapter e. Sub-chapter d does not
implement it.

`src/tui/study.rs`, lines 104–117:

```rust
pub fn run(init: SessionInit, hint: Option<&str>) -> io::Result<()> {
    if !io::stdout().is_terminal() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "the study table needs a terminal; the line-mode fallback is sub-chapter e",
        ));
    }
    // The guard restores the terminal on every exit path, panic
    // included. The binding must stay alive until the loop returns.
    let mut screen = AltScreen::enter()?;
    let mut input = RawInput;
    let mut core = SessionCore::new(init);
    study_loop(&mut core, &mut screen, &mut input, hint)
}
```

If stdout is a terminal, `run` calls `AltScreen::enter()`. That call
enables raw mode, enters the alternate screen, and hides the cursor.
The result is an `AltScreen` value. The value carries no data. Its
only job is to stay alive until the shell is done.

The idiom is RAII / `Drop`. The `Drop` implementation for `AltScreen`
restores the terminal. `Drop` runs on every exit path: a normal
return, an error propagated with `?`, and a panic. The Drop order is
fixed: show the cursor and leave the alternate screen first, then
disable raw mode. Both show/leave actions are escape sequences on
stdout; raw mode is a terminal attribute. Errors inside `Drop` are
ignored with `let _`. If restoration fails, there is nothing sensible
left to do. Panicking inside `Drop` during an unwind would abort the
process, which is the opposite of what the guard exists for.

`src/tui/screen.rs`, lines 45–94:

```rust
/// The full-screen terminal screen, and the RAII guard for it.
///
/// Raw mode and the alternate screen are global, sticky terminal
/// state. If the program returns early, errors out, or panics while
/// they are active, the user's terminal stays broken: no echo, no
/// line buffering, no scrollback. The value's `Drop` restores the
/// terminal, and `Drop` runs on every exit path, panic included. The
/// value carries no data; its job is to stay alive until the shell is
/// done.
///
/// The Drop order is fixed and deliberate: show the cursor and leave
/// the alternate screen first (both are escape sequences on stdout),
/// then disable raw mode (a terminal attribute). Writing escape
/// sequences after raw mode is off would still work, but restoring
/// the visible state before the input state reads better in a
/// terminal log and keeps every stdout write inside the raw-mode
/// section of the program's life.
pub struct AltScreen;

impl AltScreen {
    /// Put the terminal into TUI mode: raw mode on, alternate screen
    /// active, cursor hidden.
    ///
    /// The cursor is hidden because the shell draws its own cursor as
    /// angle brackets around a board cell; a second, terminal-drawn
    /// cursor would only distract.
    ///
    /// # Errors
    /// Returns an error when the terminal does not accept the mode
    /// switches (for example when stdout is not a terminal).
    pub fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        // `execute!` flushes: the mode switch must take effect before
        // the first frame is queued, not whenever a later flush
        // happens to run.
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(Self)
    }
}

impl Drop for AltScreen {
    fn drop(&mut self) {
        // Errors are ignored with `let _`. If restoration fails there
        // is nothing sensible left to do, and panicking inside Drop
        // during an unwind would abort the process — the opposite of
        // what this guard exists for.
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
```

No test pins the guard; it needs a terminal. The contract is
documented in the code and in this sub-chapter.

## Step 1 — The first frame

`run` now has a core and a live screen. It calls `study_loop`. Inside
the loop, the first thing that happens is `build_frame`.

`build_frame` assembles a `StudyView`. `StudyView` is the render
input. It already existed in sub-chapter b; this sub-chapter extends
it with chainable setters for the new display state. The caller
states only what it has; no call site is forced to pass `None, None,
false`.

`src/tui/render.rs`, lines 79–115:

```rust
    /// Highlight `mv` as the board cursor.
    ///
    /// The cursor cell gets angle brackets (`<x>`); a styled frame adds
    /// reverse video on top. When the cursor is on the last move, the cursor
    /// marker takes precedence, and the last-move brackets are not drawn on
    /// that cell.
    #[must_use]
    pub fn cursor(mut self, mv: Move) -> Self {
        self.cursor = Some(mv);
        self
    }

    /// Show `buffer` as the bottom line of the frame, behind a `:` prompt.
    ///
    /// The buffer is the text the user typed after opening the command
    /// line. The renderer adds the prompt and applies no styling, so the
    /// text is shown verbatim in both modes.
    #[must_use]
    pub fn command_line(mut self, buffer: &'a str) -> Self {
        self.command_line = Some(buffer);
        self
    }

    /// Show `text` as the bottom line of the frame.
    ///
    /// One bottom line exists. While the command line is open it shows
    /// the buffer and the message is not drawn; in normal mode the
    /// message carries the last event summary or the last error text.
    /// The line is never styled: it is feedback text, not furniture.
    #[must_use]
    pub fn message(mut self, text: &'a str) -> Self {
        self.message = Some(text);
        self
    }

    /// Turn ANSI styling on (`true`) or off (`false`).
    ///
    /// Unstyled output contains no escape sequences, so it is safe to pipe
    /// into files and to compare in tests.
    #[must_use]
    pub fn styled(mut self, styled: bool) -> Self {
        self.styled = styled;
        self
    }
```

`build_frame` in `src/tui/study.rs` passes the cursor, the command
line buffer or the message, and the screen's styling choice into the
view.

`src/tui/study.rs`, lines 170–188:

```rust
fn build_frame(
    core: &SessionCore,
    state: &InputState,
    events: &[Event],
    message: Option<&str>,
    styled: bool,
) -> Vec<String> {
    let view = StudyView::new(core, events)
        .cursor(state.cursor)
        .styled(styled);
    let view = match &state.mode {
        InputMode::CommandLine(buffer) => view.command_line(buffer),
        InputMode::Normal => match message {
            Some(text) => view.message(text),
            None => view,
        },
    };
    render_study(&view)
}
```

`render_study` turns the `StudyView` into `Vec<String>`. The board
draws ceremony placements as stones. `SessionCore::board()` stays
empty during the ceremony by design: the placements are not game
moves. So the cell lookup falls back to the opening snapshot's
placement list. The list holds at most five entries; a linear scan is
fast enough for a per-frame draw.

`src/tui/render.rs`, lines 126–158:

```rust
pub fn render_study(view: &StudyView) -> Vec<String> {
    let core = view.core;
    let board = core.board();
    let snapshot = core.snapshot();
    // During a Swap2 ceremony the tree board is empty by design (the
    // placements are not game moves), so the stones to draw come from
    // the opening snapshot.
    let opening = snapshot.opening.as_ref();
    let board_lines = study_board_lines(
        &board,
        opening,
        snapshot.last_move,
        view.cursor,
        view.styled,
    );
    let panel_lines = render_side_panel(core, &board, &snapshot, view.recent_events, view.styled);
    let rows = board_lines.len().max(panel_lines.len());
    let mut out = Vec::with_capacity(rows + 1);
    for i in 0..rows {
        let left = board_lines.get(i).map(String::as_str).unwrap_or("");
        let right = panel_lines.get(i).map(String::as_str).unwrap_or("");
        out.push(format!("{left}  {right}"));
    }
    // One bottom line: the command line while it is open (the user's
    // text always at the bottom of the screen), else the message when
    // the loop has one to show.
    if let Some(buffer) = view.command_line {
        out.push(format!(":{buffer}"));
    } else if let Some(text) = view.message {
        out.push(text.to_string());
    }
    out
}
```

The cell lookup in `study_cell` falls back from the board to the
ceremony placement list.

`src/tui/render.rs`, lines 237–248:

```rust
    // Ceremony placements are not in the tree board, so the board
    // lookup falls back to the placement list. The list holds at most
    // five entries; a linear scan is fast enough for a per-frame draw.
    let stone = board.stone_at(mv).or_else(|| {
        opening.and_then(|opening| {
            opening
                .placements
                .iter()
                .find(|(m, _)| *m == mv)
                .map(|(_, color)| *color)
        })
    });
```

The side panel replaces the game-state block with a ceremony block
when a ceremony is in progress. The ceremony block shows the phase,
the placements in order, `Next: <Color>` at a placement phase, and
the guidance line for the phase. A choice phase has no next stone, so
the panel shows no `Next:` line there.

`src/tui/render.rs`, lines 357–392:

```rust
fn ceremony_lines(lines: &mut Vec<String>, opening: &OpeningSnapshot) {
    lines.push(format!("Swap2: {:?}", opening.phase));
    let stones = if opening.placements.is_empty() {
        "(none)".to_string()
    } else {
        opening
            .placements
            .iter()
            .map(|(mv, color)| format!("{}:{color:?}", algebraic(*mv)))
            .collect::<Vec<_>>()
            .join(" ")
    };
    lines.push(format!("Stones: {stones}"));
    match opening.phase {
        OpeningPhase::Placing3 | OpeningPhase::Placing2 => {
            // The strict alternation fixes the next stone's color, so
            // the panel states it without a source: there is nothing
            // to select. The snapshot guarantees a color at a
            // placement phase.
            let color = opening
                .next_color
                .expect("a placement phase always has a next color");
            lines.push(format!("Next: {color:?}"));
            lines.push("Enter place · u undo · d done".to_string());
        }
        OpeningPhase::FirstChoice => {
            // A choice phase has no next stone, so there is no `Next:`
            // line; showing a default here would report a stone that
            // does not exist.
            lines.push("b black · w white · m more".to_string());
        }
        OpeningPhase::FinalChoice => {
            lines.push("b black · w white".to_string());
        }
    }
}
```

The four frame tests pin the renderer additions. They use byte
offsets on the rendered board line, the same technique as the
existing last-move test.

`src/tui/render.rs`, lines 611–628:

```rust
    #[test]
    fn the_ceremony_draws_its_placements_as_stones() {
        // The tree board stays empty during the ceremony by design;
        // the stones on the frame come from the opening snapshot.
        let mut core = SessionCore::new(SessionInit::new_hotseat().with_swap2(true));
        assert!(core.board().moves().is_empty());
        play(&mut core, 7, 7); // h8, Black by the alternation
        play(&mut core, 7, 8); // i8, White by the alternation

        let frame = render_study(&StudyView::new(&core, &[]));
        let row7 = &frame[2 + 7];
        // Four gutter bytes, then three bytes per column: h8 starts at
        // byte 25. Ceremony stones get no last-move brackets: the
        // marker belongs to the tree, and the tree is empty.
        assert_eq!(&row7[25..28], " x ");
        assert_eq!(&row7[28..31], " o ");
    }
```

`src/tui/render.rs`, lines 629–659:

```rust
    #[test]
    fn the_ceremony_panel_shows_phase_next_color_and_keys() {
        let mut core = SessionCore::new(SessionInit::new_hotseat().with_swap2(true));
        let frame = render_study(&StudyView::new(&core, &[]));
        let text = frame.join("\n");
        assert!(text.contains("Swap2: Placing3"), "the phase: {text}");
        assert!(text.contains("Stones: (none)"), "no placements yet: {text}");
        assert!(
            text.contains("Next: Black"),
            "the color of the first stone: {text}"
        );
        assert!(
            text.contains("Enter place · u undo · d done"),
            "the placement keys: {text}"
        );
        // The ceremony block replaces the game-state lines: the tree is
        // empty, so there is no side to move.
        assert!(
            !text.contains("To move:"),
            "no game state during the ceremony: {text}"
        );

        play(&mut core, 7, 7);
        let text = render_study(&StudyView::new(&core, &[])).join("\n");
        assert!(text.contains("Stones: h8:Black"), "the placements: {text}");
        assert!(
            text.contains("Next: White"),
            "the alternated color of the next stone: {text}"
        );
    }
```

`src/tui/render.rs`, lines 660–681:

```rust
    #[test]
    fn the_choice_phase_panel_shows_the_choice_keys_and_no_next_stone() {
        let mut core = SessionCore::new(SessionInit::new_hotseat().with_swap2(true));
        for (r, c) in [(7, 7), (8, 8), (9, 9)] {
            play(&mut core, r, c);
        }
        core.execute(Command::Done).unwrap();

        let text = render_study(&StudyView::new(&core, &[])).join("\n");
        assert!(text.contains("Swap2: FirstChoice"), "the phase: {text}");
        assert!(
            text.contains("b black · w white · m more"),
            "the first-choice keys: {text}"
        );
        // A choice phase has no next stone, so the panel must not
        // report one.
        assert!(
            !text.contains("Next:"),
            "no next stone at a choice phase: {text}"
        );
    }
```

`src/tui/render.rs`, lines 682–704:

```rust
    #[test]
    fn the_message_is_the_bottom_line_unless_the_command_line_is_open() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        let bare = render_study(&StudyView::new(&core, &[]));
        let with_message = render_study(&StudyView::new(&core, &[]).message("Moved h8 by Human"));
        // The message adds exactly one line, at the bottom.
        assert_eq!(with_message.len(), bare.len() + 1);
        assert_eq!(with_message.last().unwrap(), "Moved h8 by Human");

        // While the command line is open it owns the bottom line.
        let both = render_study(
            &StudyView::new(&core, &[])
                .message("Moved h8 by Human")
                .command_line("pla"),
        );
        assert_eq!(both.len(), bare.len() + 1);
        assert_eq!(both.last().unwrap(), ":pla");
    }
```

## Step 2 — The frame reaches the terminal

The loop calls `screen.draw(&frame)`. `Screen` is a trait with one
required method.

`src/tui/screen.rs`, lines 23–43:

```rust
pub trait Screen {
    /// Draw one frame: the lines of the study table, top to bottom.
    ///
    /// The frame is complete; the screen repaints everything and does
    /// no diffing. A 15×15 board frame is about twenty short lines, and
    /// terminals are fast, so a full repaint per input event costs
    /// nothing measurable.
    fn draw(&mut self, frame: &[String]) -> io::Result<()>;

    /// Whether frames for this screen carry ANSI styling.
    ///
    /// The renderer takes styling as a runtime parameter, and the
    /// screen makes the choice: the alternate screen wants colors and
    /// attributes; pipes and CI logs want plain text. The default is
    /// unstyled, so a test screen that records frames gets plain text
    /// without opting out.
    fn styled(&self) -> bool {
        false
    }
}
```

`Screen` is a trait for two reasons. First, tests replace it with
`TestScreen`, which records frames in a `Vec`. Second, sub-chapter e
adds a line-mode screen; the trait lets the same loop serve both
screens. The default `styled()` returns `false`, so test recordings
and pipes get plain text without opting out. `AltScreen` overrides
`styled()` to return `true`.

`AltScreen::draw` clears the screen, queues every line at its
absolute position, then flushes once. The idiom is the `queue!` /
`flush` split. Painting a frame is dozens of cursor moves and prints.
Sending them one by one is what makes a picture flicker; queueing
them into one buffer and flushing once sends them as one batch.
Absolute positioning makes the paint independent of where a previous
frame left the cursor. There is no diffing. The renderer repaints the
whole frame for every keypress. A 15×15 frame is about twenty short
lines, and terminals are fast.

`src/tui/screen.rs`, lines 96–122:

```rust
impl Screen for AltScreen {
    /// Paint the frame: one clear, then every line at its absolute
    /// position, all queued and flushed once.
    fn draw(&mut self, frame: &[String]) -> io::Result<()> {
        let mut out = io::stdout();
        queue!(out, Clear(ClearType::All))?;
        for (y, line) in frame.iter().enumerate() {
            // The frame is about twenty lines, so the y coordinate fits
            // a u16 (the terminal's coordinate type) with room to
            // spare.
            queue!(out, MoveTo(0, y as u16), Print(line))?;
        }
        out.flush()
    }

    /// The full-screen shell owns the terminal, so styling is safe.
    fn styled(&self) -> bool {
        true
    }
}
```

## Step 3 — One input

After the frame is on screen, the loop reads one input through the
`InputSource` trait.

`src/tui/study.rs`, lines 32–57:

```rust
pub trait InputSource {
    /// Wait for the next key and translate it into an outcome.
    ///
    /// `state` and `core` are the translator's inputs: the state
    /// changes (cursor motion, buffer edits) happen here, inside the
    /// source, because the source is what reads keys.
    fn next_outcome(
        &mut self,
        state: &mut InputState,
        core: &SessionCore,
    ) -> io::Result<InputOutcome>;

    /// Wait for one keypress and discard it.
    ///
    /// The help screen uses this: any key returns to the table, and
    /// the dismissing key must not also act on the table. Reading it
    /// through the source keeps the discard on the same input path as
    /// every other key.
    fn wait_any_key(&mut self) -> io::Result<()>;
}
```

`RawInput` is the terminal implementation. It reads crossterm events
and passes key events to `handle_key`.

`src/tui/study.rs`, lines 59–74:

```rust
/// The raw-mode input source: one terminal keypress per call.
pub struct RawInput;

impl InputSource for RawInput {
    fn next_outcome(
        &mut self,
        state: &mut InputState,
        core: &SessionCore,
    ) -> io::Result<InputOutcome> {
        Ok(handle_key(read_key_press()?, state, core))
    }

    fn wait_any_key(&mut self) -> io::Result<()> {
        read_key_press().map(|_| ())
    }
}
```

`read_key_press` filters for `KeyEventKind::Press`. Some platforms
report release and repeat events; without the filter, every key would
act twice there. Non-key events are skipped. The loop repaints the
whole frame on every key, so a resize needs no special handling.

`src/tui/study.rs`, lines 76–92:

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

## Step 4 — The key becomes a meaning

`handle_key` is a pure function: a `KeyEvent`, the mutable
`InputState`, and the read-only `SessionCore` go in; an
`InputOutcome` comes out. It performs no I/O.

`InputMode`, `InputState`, and `InputOutcome` are the vocabulary:

`src/tui/input.rs`, lines 40–108:

```rust
/// The input mode of the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    /// Single keys act immediately.
    Normal,
    /// A `:` command line is open; the payload is the buffer typed so
    /// far.
    CommandLine(String),
}

/// The shell's input state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputState {
    /// The current mode.
    pub mode: InputMode,
    /// The board cursor: the square the next Enter acts on.
    pub cursor: Move,
}

impl InputState {
    /// Fresh input state: normal mode, the cursor on the center point
    /// h8.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode: InputMode::Normal,
            cursor: Move::new(7, 7).expect("the center point is on the board"),
        }
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

/// What one key press means for the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputOutcome {
    /// Execute this command on the session core.
    Command(Command),
    /// Show the full-screen help; any key returns to the table.
    Help,
    /// Leave the shell.
    Quit,
    /// Show this text on the message line. Parse errors and keys that
    /// do not apply in the current phase use this variant.
    Message(String),
    /// The key changed only input state (cursor moved, buffer edited),
    /// or the key is not mapped. Nothing reaches the session.
    None,
}
```

`handle_key` dispatches on the mode. Two small machines share the
work: the command-line editor while a command line is open, and the
normal-mode mapping otherwise.

`src/tui/input.rs`, lines 110–122:

```rust
pub fn handle_key(key: KeyEvent, state: &mut InputState, core: &SessionCore) -> InputOutcome {
    match &state.mode {
        InputMode::Normal => handle_normal_key(key, state, core),
        InputMode::CommandLine(_) => handle_command_line_key(key, state),
    }
}
```

`handle_normal_key` checks global keys first. They mean the same
thing in every phase: arrows and `hjkl` move the cursor, `:` opens
the command line, `?` is help, `q` is quit. Cursor motion clamps at
the board edges. Up and left use `saturating_sub`; down and right use
`.min(14)`. The clamped coordinates are always valid, so `Move::new`
cannot fail.

`src/tui/input.rs`, lines 174–211:

```rust
fn handle_normal_key(key: KeyEvent, state: &mut InputState, core: &SessionCore) -> InputOutcome {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            move_cursor(&mut state.cursor, -1, 0);
            return InputOutcome::None;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            move_cursor(&mut state.cursor, 1, 0);
            return InputOutcome::None;
        }
        KeyCode::Left | KeyCode::Char('h') => {
            move_cursor(&mut state.cursor, 0, -1);
            return InputOutcome::None;
        }
        KeyCode::Right | KeyCode::Char('l') => {
            move_cursor(&mut state.cursor, 0, 1);
            return InputOutcome::None;
        }
        KeyCode::Char(':') => {
            state.mode = InputMode::CommandLine(String::new());
            return InputOutcome::None;
        }
        KeyCode::Char('?') => return InputOutcome::Help,
        KeyCode::Char('q') => return InputOutcome::Quit,
        _ => {}
    }

    // Phase-dependent keys. The session knows the phase; the snapshot
    // reports it. This is what makes one key per action safe: `u`
    // cannot mean two things at once, because the session always knows
    // which phase the session is in.
    match core.snapshot().opening {
        Some(opening) => opening_key(key, state, &opening),
        None => playing_key(key, state, core),
    }
}
```

`move_cursor` uses `saturating_sub` and `saturating_add().min(14)` to
clamp the cursor inside the 15×15 board.

`src/tui/input.rs`, lines 284–296:

```rust
fn move_cursor(cursor: &mut Move, dr: i8, dc: i8) {
    let row = match dr {
        -1 => cursor.row().saturating_sub(1),
        1 => cursor.row().saturating_add(1).min(14),
        _ => cursor.row(),
    };
    let col = match dc {
        -1 => cursor.col().saturating_sub(1),
        1 => cursor.col().saturating_add(1).min(14),
        _ => cursor.col(),
    };
    *cursor = Move::new(row, col).expect("clamped coordinates stay on the board");
}
```

The cursor tests pin the global motion keys and the edge clamping.

`src/tui/input.rs`, lines 368–401:

```rust
    #[test]
    fn arrows_and_hjkl_move_the_cursor() {
        let core = playing_core();
        let mut state = InputState::new();
        assert_eq!(state.cursor, Move::new(7, 7).unwrap());

        handle_key(key(KeyCode::Up), &mut state, &core);
        assert_eq!(state.cursor, Move::new(6, 7).unwrap());
        handle_key(key(KeyCode::Left), &mut state, &core);
        assert_eq!(state.cursor, Move::new(6, 6).unwrap());
        handle_key(char_key('j'), &mut state, &core);
        assert_eq!(state.cursor, Move::new(7, 6).unwrap());
        handle_key(char_key('l'), &mut state, &core);
        assert_eq!(state.cursor, Move::new(7, 7).unwrap());
    }

    #[test]
    fn the_cursor_clamps_at_the_board_edges() {
        let core = playing_core();
        let mut state = InputState {
            cursor: Move::new(0, 0).unwrap(),
            ..InputState::new()
        };
        // The top-left corner: up and left cannot underflow.
        handle_key(char_key('k'), &mut state, &core);
        handle_key(key(KeyCode::Left), &mut state, &core);
        assert_eq!(state.cursor, Move::new(0, 0).unwrap());

        state.cursor = Move::new(14, 14).unwrap();
        // The bottom-right corner: down and right cannot pass index 14.
        handle_key(char_key('j'), &mut state, &core);
        handle_key(key(KeyCode::Right), &mut state, &core);
        assert_eq!(state.cursor, Move::new(14, 14).unwrap());
    }
```

`src/tui/input.rs`, lines 403–428:

```rust
    #[test]
    fn the_global_keys_work_in_every_phase() {
        let mut state = InputState::new();
        for core in [
            playing_core(),
            placing_core(),
            first_choice_core(),
            final_choice_core(),
        ] {
            assert_eq!(
                handle_key(char_key('?'), &mut state, &core),
                InputOutcome::Help
            );
            assert_eq!(
                handle_key(char_key('q'), &mut state, &core),
                InputOutcome::Quit
            );
            let outcome = handle_key(char_key(':'), &mut state, &core);
            assert_eq!(outcome, InputOutcome::None);
            assert_eq!(state.mode, InputMode::CommandLine(String::new()));
            // Close the line again for the next phase's core.
            handle_key(key(KeyCode::Esc), &mut state, &core);
            assert_eq!(state.mode, InputMode::Normal);
        }
    }
```

After the global keys, phase-dependent keys dispatch on
`core.snapshot().opening`. The session knows the phase, so one key
can safely carry one meaning per phase. `Enter` during play is `play`
at the cursor for a human seat and `step` for an AI seat. The seat
table decides; the session validates the result.

`src/tui/input.rs`, lines 256–275:

```rust
fn playing_key(key: KeyEvent, state: &InputState, core: &SessionCore) -> InputOutcome {
    match key.code {
        // Enter is the obvious next action: a human seat plays at the
        // cursor, an AI seat searches. The seat table decides; the
        // session validates the result (an occupied square, a finished
        // game) and the loop shows the error.
        KeyCode::Enter => {
            let side = core.board().to_move();
            if core.occupant(side).is_human() {
                InputOutcome::Command(Command::Play { mv: state.cursor })
            } else {
                InputOutcome::Command(Command::Step)
            }
        }
        KeyCode::Char('u') => InputOutcome::Command(Command::Back { n: 1 }),
        // The ceremony keys (`d`, `b`, `w`, `m`) have no meaning during
        // play and are ignored like any other unmapped key.
        _ => InputOutcome::None,
    }
}
```

The ceremony table covers placement phases and choice phases. At a
placement phase, `Enter` places the next stone, `u` undoes, `d` ends
the round, and `b`/`w` are unmapped because the strict alternation
fixes the color. At a choice phase, `b` and `w` choose the color, and
`m` is `more` at the first choice. `Enter` and `u` are unmapped at a
choice phase.

`src/tui/input.rs`, lines 212–254:

```rust
fn opening_key(key: KeyEvent, state: &mut InputState, opening: &OpeningSnapshot) -> InputOutcome {
    match opening.phase {
        OpeningPhase::Placing3 | OpeningPhase::Placing2 => match key.code {
            // Enter places the next stone at the cursor; the
            // ceremony's strict alternation decides its color.
            KeyCode::Enter => InputOutcome::Command(Command::Play { mv: state.cursor }),
            // During the ceremony the session routes `back` to the
            // replay log, so `u` removes the last placement.
            KeyCode::Char('u') => InputOutcome::Command(Command::Back { n: 1 }),
            KeyCode::Char('d') => InputOutcome::Command(Command::Done),
            // `b`/`w` bind to the color choice, which exists only at a
            // choice phase. At a placement phase the keys are unmapped:
            // the alternation fixes the next stone's color, so there
            // is nothing to select.
            // `m` is the decline of the first choice. During placement
            // it applies nowhere, so the key explains itself instead of
            // producing the session's typed error with no visible
            // context.
            KeyCode::Char('m') => InputOutcome::Message(MORE_ONLY_AT_FIRST_CHOICE.to_string()),
            _ => InputOutcome::None,
        },
        // At a choice phase the only stone-related keys are the choice
        // itself. Enter places nothing here, and `u` is not mapped:
        // removing a placement belongs to the placement phases.
        OpeningPhase::FirstChoice | OpeningPhase::FinalChoice => match key.code {
            KeyCode::Char('b') => InputOutcome::Command(Command::ChooseColor {
                color: Color::Black,
            }),
            KeyCode::Char('w') => InputOutcome::Command(Command::ChooseColor {
                color: Color::White,
            }),
            // `more` exists only at the first choice. At the final
            // choice the key gets the same message as at a placement
            // phase, so the rule has one sentence everywhere.
            KeyCode::Char('m') if opening.phase == OpeningPhase::FirstChoice => {
                InputOutcome::Command(Command::More)
            }
            KeyCode::Char('m') => InputOutcome::Message(MORE_ONLY_AT_FIRST_CHOICE.to_string()),
            _ => InputOutcome::None,
        },
    }
}
```

The phase-dependent tests build ceremony cores through the public
session API. They pin every entry in the phase tables.

`src/tui/input.rs`, lines 429–455:

```rust
    #[test]
    fn during_play_enter_plays_at_the_cursor_for_a_human_seat() {
        let core = playing_core();
        let mut state = InputState::new();
        state.cursor = Move::new(3, 4).unwrap();
        assert_eq!(
            handle_key(key(KeyCode::Enter), &mut state, &core),
            InputOutcome::Command(Command::Play {
                mv: Move::new(3, 4).unwrap()
            })
        );
    }

    #[test]
    fn during_play_enter_steps_for_an_ai_seat() {
        let mut core = playing_core();
        core.execute(Command::Release {
            seat: SeatSel::Black,
        })
        .unwrap();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(key(KeyCode::Enter), &mut state, &core),
            InputOutcome::Command(Command::Step)
        );
    }

    #[test]
    fn during_play_u_is_back_and_the_ceremony_keys_are_unmapped() {
        let core = playing_core();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(char_key('u'), &mut state, &core),
            InputOutcome::Command(Command::Back { n: 1 })
        );
        for c in ['d', 'b', 'w', 'm'] {
            assert_eq!(
                handle_key(char_key(c), &mut state, &core),
                InputOutcome::None,
                "'{c}' is unmapped during play"
            );
        }
    }
```

`src/tui/input.rs`, lines 473–501:

```rust
    #[test]
    fn at_a_placement_phase_enter_places_the_next_stone() {
        let core = placing_core();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(key(KeyCode::Enter), &mut state, &core),
            InputOutcome::Command(Command::Play {
                mv: Move::new(7, 7).unwrap()
            })
        );
    }

    #[test]
    fn at_a_placement_phase_b_and_w_are_unmapped() {
        // The alternation fixes the next stone's color, so `b`/`w`
        // select nothing at a placement phase. The binding exists only
        // at a choice phase, where the same keys choose the color.
        for core in [placing_core(), placing2_core()] {
            let mut state = InputState::new();
            for c in ['b', 'w'] {
                assert_eq!(
                    handle_key(char_key(c), &mut state, &core),
                    InputOutcome::None,
                    "'{c}' is unmapped at a placement phase"
                );
            }
        }
    }

    #[test]
    fn at_a_placement_phase_u_undoes_and_d_is_done() {
        for core in [placing_core(), placing2_core()] {
            let mut state = InputState::new();
            assert_eq!(
                handle_key(char_key('u'), &mut state, &core),
                InputOutcome::Command(Command::Back { n: 1 })
            );
            assert_eq!(
                handle_key(char_key('d'), &mut state, &core),
                InputOutcome::Command(Command::Done)
            );
        }
    }
```

`src/tui/input.rs`, lines 517–576:

```rust
    #[test]
    fn at_a_placement_phase_m_is_a_message_not_a_command() {
        let core = placing_core();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(char_key('m'), &mut state, &core),
            InputOutcome::Message(MORE_ONLY_AT_FIRST_CHOICE.to_string())
        );
    }

    #[test]
    fn at_the_first_choice_b_w_choose_and_m_is_more() {
        let core = first_choice_core();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(char_key('b'), &mut state, &core),
            InputOutcome::Command(Command::ChooseColor {
                color: Color::Black
            })
        );
        assert_eq!(
            handle_key(char_key('w'), &mut state, &core),
            InputOutcome::Command(Command::ChooseColor {
                color: Color::White
            })
        );
        assert_eq!(
            handle_key(char_key('m'), &mut state, &core),
            InputOutcome::Command(Command::More)
        );
    }

    #[test]
    fn at_the_final_choice_b_w_choose_and_m_is_a_message() {
        let core = final_choice_core();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(char_key('b'), &mut state, &core),
            InputOutcome::Command(Command::ChooseColor {
                color: Color::Black
            })
        );
        assert_eq!(
            handle_key(char_key('m'), &mut state, &core),
            InputOutcome::Message(MORE_ONLY_AT_FIRST_CHOICE.to_string())
        );
    }

    #[test]
    fn at_a_choice_phase_enter_and_u_are_unmapped() {
        let core = first_choice_core();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(key(KeyCode::Enter), &mut state, &core),
            InputOutcome::None
        );
        assert_eq!(
            handle_key(char_key('u'), &mut state, &core),
            InputOutcome::None
        );
    }
```

## Step 5 — The command-line editor

`:` opens the command line. While the command line is open,
`handle_key` dispatches to `handle_command_line_key`. Characters
append. `Backspace` pops a character; on an empty buffer it is a
no-op. `Esc` cancels and returns to normal mode. `Enter` on an empty
buffer closes the line quietly. `Enter` on text parses the buffer
with `command::parse`.

One grammar exists, and it lives in `command::parse`. The command
line reuses it. A second parser in the shell would drift from the
first, the way the reference build's TUI parser drifted from its line
parser.

The buffer must come out of the enum before the mode can change,
because `state.mode` is borrowed mutably. `std::mem::replace` takes
the old value and puts a new one in its place in one move.

`src/tui/input.rs`, lines 123–173:

```rust
fn handle_command_line_key(key: KeyEvent, state: &mut InputState) -> InputOutcome {
    let InputMode::CommandLine(mut buffer) = std::mem::replace(&mut state.mode, InputMode::Normal)
    else {
        unreachable!("handle_key dispatches here only in command-line mode")
    };
    match key.code {
        // Any character appends, including characters that are commands
        // in normal mode. The command line is text, not keys.
        KeyCode::Char(c) => {
            buffer.push(c);
            state.mode = InputMode::CommandLine(buffer);
            InputOutcome::None
        }
        KeyCode::Backspace => {
            buffer.pop();
            state.mode = InputMode::CommandLine(buffer);
            InputOutcome::None
        }
        // Esc cancels: the mode is already Normal (the replace above),
        // and the buffer is dropped with this frame.
        KeyCode::Esc => InputOutcome::None,
        KeyCode::Enter => {
            let text = buffer.trim().to_string();
            // An empty command line closes quietly, the way vim's `:`
            // followed by Enter does nothing.
            if text.is_empty() {
                return InputOutcome::None;
            }
            // One grammar exists, and it lives in `command::parse`. The
            // command line reuses it; a second parser here would drift.
            match command::parse(&text) {
                Ok(cmd) => InputOutcome::Command(cmd),
                Err(err) => InputOutcome::Message(err.to_string()),
            }
        }
        // Arrows and every other key do not edit; the buffer stays open.
        _ => {
            state.mode = InputMode::CommandLine(buffer);
            InputOutcome::None
        }
    }
}
```

The command-line tests pin editing, cancellation, the empty buffer,
parsing through the shared grammar, parse errors, and typed
`help`/`quit`.

`src/tui/input.rs`, lines 579–672:

```rust
    #[test]
    fn the_command_line_appends_and_backspace_edits() {
        let core = playing_core();
        let mut state = InputState::new();
        handle_key(char_key(':'), &mut state, &core);

        type_text(&mut state, &core, "pla");
        assert_eq!(state.mode, InputMode::CommandLine("pla".to_string()));

        handle_key(key(KeyCode::Backspace), &mut state, &core);
        assert_eq!(state.mode, InputMode::CommandLine("pl".to_string()));

        // Backspace on an empty buffer is a no-op, not an underflow.
        handle_key(key(KeyCode::Backspace), &mut state, &core);
        handle_key(key(KeyCode::Backspace), &mut state, &core);
        handle_key(key(KeyCode::Backspace), &mut state, &core);
        assert_eq!(state.mode, InputMode::CommandLine(String::new()));
    }

    #[test]
    fn esc_cancels_the_command_line() {
        let core = playing_core();
        let mut state = InputState::new();
        handle_key(char_key(':'), &mut state, &core);
        type_text(&mut state, &core, "play h8");
        assert_eq!(
            handle_key(key(KeyCode::Esc), &mut state, &core),
            InputOutcome::None
        );
        assert_eq!(state.mode, InputMode::Normal);
    }

    #[test]
    fn enter_on_an_empty_command_line_returns_to_normal() {
        let core = playing_core();
        let mut state = InputState::new();
        handle_key(char_key(':'), &mut state, &core);
        assert_eq!(
            handle_key(key(KeyCode::Enter), &mut state, &core),
            InputOutcome::None
        );
        assert_eq!(state.mode, InputMode::Normal);
    }

    #[test]
    fn enter_parses_the_buffer_with_the_shared_grammar() {
        let core = playing_core();
        let mut state = InputState::new();
        handle_key(char_key(':'), &mut state, &core);
        type_text(&mut state, &core, "play h8");
        assert_eq!(
            handle_key(key(KeyCode::Enter), &mut state, &core),
            InputOutcome::Command(Command::Play {
                mv: Move::new(7, 7).unwrap()
            })
        );
        assert_eq!(state.mode, InputMode::Normal);
    }

    #[test]
    fn a_parse_error_is_a_message_and_closes_the_line() {
        let core = playing_core();
        let mut state = InputState::new();
        handle_key(char_key(':'), &mut state, &core);
        type_text(&mut state, &core, "play");
        let outcome = handle_key(key(KeyCode::Enter), &mut state, &core);
        let InputOutcome::Message(text) = outcome else {
            panic!("a parse error is a message, got {outcome:?}");
        };
        assert!(text.contains("Parse error"), "the parser's text: {text}");
        assert_eq!(state.mode, InputMode::Normal);
    }

    #[test]
    fn the_typed_quit_and_help_are_commands_for_the_loop_to_intercept() {
        // `quit` and `help` typed on the command line translate to the
        // same Command values as everything else. The loop intercepts
        // them, because the session core treats both as no-ops.
        let core = playing_core();
        let mut state = InputState::new();
        handle_key(char_key(':'), &mut state, &core);
        type_text(&mut state, &core, "quit");
        assert_eq!(
            handle_key(key(KeyCode::Enter), &mut state, &core),
            InputOutcome::Command(Command::Quit)
        );

        handle_key(char_key(':'), &mut state, &core);
        type_text(&mut state, &core, "help");
        assert_eq!(
            handle_key(key(KeyCode::Enter), &mut state, &core),
            InputOutcome::Command(Command::Help)
        );
    }
```

## Step 6 — The loop applies the outcome

`study_loop` is generic over `Screen` and `InputSource`. The loop
owns three pieces of shell state: the `InputState`, a capped list of
recent events, and the message line. `hint`, when given, is the first
message line.

One iteration: build the frame, draw it, read one outcome, apply it.
Typed `help`/`quit` and keyed `?`/`q` do the same thing because the
session core treats `Help` and `Quit` as no-ops and the loop
intercepts both.

`src/tui/study.rs`, lines 127–161:

```rust
pub fn study_loop<S: Screen, I: InputSource>(
    core: &mut SessionCore,
    screen: &mut S,
    input: &mut I,
    hint: Option<&str>,
) -> io::Result<()> {
    let mut state = InputState::new();
    let mut events: Vec<Event> = Vec::new();
    // The message line shows the last event summary or the last error
    // text; the caller's hint, when given, is the first message.
    let mut message: Option<String> = hint.map(str::to_string);
    loop {
        let frame = build_frame(core, &state, &events, message.as_deref(), screen.styled());
        screen.draw(&frame)?;
        match input.next_outcome(&mut state, core)? {
            InputOutcome::None => {}
            InputOutcome::Message(text) => message = Some(text),
            InputOutcome::Help => show_help(screen, input)?,
            InputOutcome::Quit => return Ok(()),
            // `help` and `quit` are part of the typed command language,
            // so they can arrive as commands from the command line. The
            // session core treats both as no-ops (they are shell
            // concerns), so the loop intercepts them and they do what
            // the same words do as normal-mode keys.
            InputOutcome::Command(Command::Help) => show_help(screen, input)?,
            InputOutcome::Command(Command::Quit) => return Ok(()),
            InputOutcome::Command(cmd) => match core.execute(cmd) {
                Ok(new_events) => {
                    record_events(&mut events, &mut message, &new_events);
                }
                Err(err) => message = Some(err.to_string()),
            },
        }
    }
}
```

`record_events` appends one batch of events to the capped list and
sets the message line to the batch's last event. The cap is 20
entries; the panel shows the newest 5. The oldest events drop first.

`src/tui/study.rs`, lines 190–204:

```rust
fn record_events(events: &mut Vec<Event>, message: &mut Option<String>, new_events: &[Event]) {
    if let Some(last) = new_events.last() {
        *message = Some(last.to_string());
    }
    events.extend_from_slice(new_events);
    let excess = events.len().saturating_sub(EVENT_CAP);
    // `drain` is lazy; `drop` on the iterator performs the removal.
    drop(events.drain(..excess));
}
```

`show_help` draws the help frame and waits for one key through the
input source. `help_frame` reuses `repl::HELP` for the typed command
language, then appends the normal-mode key table, then a dismissal
note. Reusing `repl::HELP` keeps the help screen and the line-mode
`help` command from drifting apart.

`src/tui/study.rs`, lines 206–228:

```rust
fn show_help<S: Screen, I: InputSource>(screen: &mut S, input: &mut I) -> io::Result<()> {
    screen.draw(&help_frame())?;
    input.wait_any_key()
}

fn help_frame() -> Vec<String> {
    // `HELP` ends with a newline; `str::lines` drops it, so the blank
    // separator between the two parts is added explicitly.
    let mut lines: Vec<String> = repl::HELP.lines().map(str::to_string).collect();
    lines.push(String::new());
    lines.extend(NORMAL_MODE_KEYS.lines().map(str::to_string));
    lines.push(String::new());
    lines.push("Press any key to return to the table.".to_string());
    lines
}
```

`src/tui/study.rs`, lines 230–246:

```rust
const NORMAL_MODE_KEYS: &str = r#"Normal-mode keys (single keys, no Enter):
  arrows, hjkl   move the board cursor
  Enter          play at the cursor (human seat), step the AI (AI seat);
                 during Swap2 placement: place the next stone
  u              undo: remove a Swap2 placement / go back one ply
  d              Swap2 placement: end the round (done)
  b, w           Swap2 choice: choose that color
  m              Swap2 first choice: place two more stones (more)
  :              open the command line (Esc cancels, Enter executes)
  ?              show this help
  q              quit
"#;
```

The loop tests run headless. `TestScreen` records every frame it is
asked to draw. Its `styled()` keeps the default `false`, so
assertions read plain text.

`src/tui/study.rs`, lines 256–286:

```rust
    struct TestScreen {
        frames: Vec<Vec<String>>,
    }

    impl TestScreen {
        fn new() -> Self {
            Self { frames: Vec::new() }
        }

        fn all_text(&self) -> String {
            self.frames
                .iter()
                .flat_map(|frame| frame.iter())
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    impl Screen for TestScreen {
        fn draw(&mut self, frame: &[String]) -> io::Result<()> {
            self.frames.push(frame.to_vec());
            Ok(())
        }
    }
```

`ScriptedInput` plays back a list of key events. Running out of keys
is an error, not a silent quit.

`src/tui/study.rs`, lines 288–321:

```rust
    struct ScriptedInput {
        keys: VecDeque<KeyEvent>,
    }

    impl ScriptedInput {
        fn new(keys: Vec<KeyEvent>) -> Self {
            Self { keys: keys.into() }
        }

        fn pop(&mut self) -> io::Result<KeyEvent> {
            self.keys.pop_front().ok_or_else(|| {
                io::Error::new(io::ErrorKind::UnexpectedEof, "the script ran out of keys")
            })
        }
    }

    impl InputSource for ScriptedInput {
        fn next_outcome(
            &mut self,
            state: &mut InputState,
            core: &SessionCore,
        ) -> io::Result<InputOutcome> {
            Ok(handle_key(self.pop()?, state, core))
        }

        fn wait_any_key(&mut self) -> io::Result<()> {
            self.pop().map(|_| ())
        }
    }
```

The test helpers build keys, type a command line, and run a scripted
session to completion.

`src/tui/study.rs`, lines 323–349:

```rust
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn char_key(c: char) -> KeyEvent {
        key(KeyCode::Char(c))
    }

    fn typed_line(text: &str) -> Vec<KeyEvent> {
        let mut keys = vec![char_key(':')];
        keys.extend(text.chars().map(char_key));
        keys.push(key(KeyCode::Enter));
        keys
    }

    fn run_scripted(
        mut core: SessionCore,
        keys: Vec<KeyEvent>,
    ) -> io::Result<(SessionCore, TestScreen)> {
        let mut screen = TestScreen::new();
        let mut input = ScriptedInput::new(keys);
        study_loop(&mut core, &mut screen, &mut input, None)?;
        Ok((core, screen))
    }
```

The eight loop tests are the specification for the whole shell.

`src/tui/study.rs`, lines 351–537:

```rust
    #[test]
    fn the_loop_plays_moves_and_quits_cleanly() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        let keys = vec![
            key(KeyCode::Enter),
            key(KeyCode::Right),
            key(KeyCode::Enter),
            char_key('q'),
        ];
        let (core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        assert_eq!(screen.frames.len(), 4);
        assert!(
            screen.frames.iter().all(|frame| frame.len() >= 19),
            "every frame holds at least the 19 board lines"
        );
        assert_eq!(
            core.board().stone_at(Move::new(7, 7).unwrap()),
            Some(Color::Black)
        );
        assert_eq!(
            core.board().stone_at(Move::new(7, 8).unwrap()),
            Some(Color::White)
        );
        assert!(
            screen.frames[3]
                .iter()
                .any(|line| line.contains("Moved i8")),
            "the last frame shows the last event: {:?}",
            screen.frames[3]
        );
    }

    #[test]
    fn the_loop_runs_the_ceremony_on_alternated_placements() {
        let core = SessionCore::new(SessionInit::new_hotseat().with_swap2(true));
        let keys = vec![
            key(KeyCode::Enter),
            key(KeyCode::Right),
            key(KeyCode::Enter),
            key(KeyCode::Right),
            key(KeyCode::Enter),
            char_key('d'),
            char_key('b'),
            char_key('q'),
        ];
        let (core, screen) = run_scripted(core, keys).expect("a scripted ceremony runs clean");

        assert!(core.snapshot().opening.is_none());
        assert_eq!(core.board().to_move(), Color::White);
        assert_eq!(
            core.board().stone_at(Move::new(7, 7).unwrap()),
            Some(Color::Black)
        );
        assert_eq!(
            core.board().stone_at(Move::new(7, 8).unwrap()),
            Some(Color::White)
        );
        assert_eq!(
            core.board().stone_at(Move::new(7, 9).unwrap()),
            Some(Color::Black)
        );

        assert!(
            screen.frames[1]
                .iter()
                .any(|line| line.contains("Next: White")),
            "the panel shows the next stone's color: {:?}",
            screen.frames[1]
        );
    }

    #[test]
    fn a_wrong_phase_key_lands_on_the_message_line() {
        let core = SessionCore::new(SessionInit::new_hotseat().with_swap2(true));
        let keys = vec![char_key('m'), char_key('q')];
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        assert_eq!(screen.frames.len(), 2);
        assert!(
            screen.frames[1]
                .iter()
                .any(|line| line.contains("'more' applies only at the first Swap2 choice")),
            "the message line shows the key's explanation: {:?}",
            screen.frames[1]
        );
    }

    #[test]
    fn a_failed_command_lands_on_the_message_line() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        let keys = vec![key(KeyCode::Enter), key(KeyCode::Enter), char_key('q')];
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        assert!(
            screen.frames[2]
                .iter()
                .any(|line| line.contains("Illegal Move")),
            "the session's error text shows on the message line: {:?}",
            screen.frames[2]
        );
    }

    #[test]
    fn help_is_a_full_screen_frame_and_any_key_dismisses_it() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        let keys = vec![char_key('?'), char_key('x'), char_key('q')];
        let (core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        assert_eq!(screen.frames.len(), 3);
        let help = screen.frames[1].join("\n");
        assert!(
            help.contains("Commands (case-insensitive):"),
            "the typed language: {help}"
        );
        assert!(help.contains("Normal-mode keys"), "the key table: {help}");
        assert!(
            help.contains("Press any key to return to the table."),
            "the note: {help}"
        );
        assert_eq!(core.snapshot().ply, 0);
        assert!(
            !screen.all_text().contains("Moved"),
            "the dismissed help key played no move"
        );
    }

    #[test]
    fn typed_help_and_quit_do_what_the_keys_do() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        let mut keys = typed_line("help");
        keys.push(char_key('z'));
        keys.extend(typed_line("quit"));
        let expected_frames = keys.len();
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        assert_eq!(screen.frames.len(), expected_frames);
        let help = screen.frames[6].join("\n");
        assert!(
            help.contains("Normal-mode keys"),
            "typed help opens the help screen"
        );
        assert_eq!(screen.frames[2].last().unwrap(), ":h");
    }

    #[test]
    fn a_parse_error_on_the_command_line_lands_on_the_message_line() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        let mut keys = typed_line("play");
        keys.push(char_key('q'));
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        assert!(
            screen.all_text().contains("Parse error"),
            "the parser's error text shows on the message line"
        );
    }

    #[test]
    fn an_exhausted_script_is_an_error_not_a_silent_quit() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        let keys = vec![key(KeyCode::Enter)];
        let result = run_scripted(core, keys);
        assert!(result.is_err(), "an exhausted script returns an error");
    }
```

## Step 7 — The end

When `Quit` arrives, `study_loop` returns. `run` returns. The
`AltScreen` value drops out of scope. Its `Drop` restores the
terminal: show cursor, leave alternate screen, disable raw mode. The
same restoration happens if an I/O error propagated up with `?` or if
a panic unwound the stack. That is the RAII guarantee from step 0.

`src/tui/screen.rs`, lines 85–94:

```rust
impl Drop for AltScreen {
    fn drop(&mut self) {
        // Errors are ignored with `let _`. If restoration fails there
        // is nothing sensible left to do, and panicking inside Drop
        // during an unwind would abort the process — the opposite of
        // what this guard exists for.
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
```

## Verification

The d-only code compiles and the d-only tests pass. A quick check on
a temporary worktree of `reference/chapter-05-redesign` with
`LineScreen`, `LineInput`, `select_shell`, and the `line_mode`
integration test removed gives:

* `cargo test -p cli --lib` — 78 tests pass (input: 19; study loop:
  8; render additions: 4; plus unchanged pre-existing suites).
* `cargo test -p cli --test actor --test history --test opening
  --test session --test study` — 51 integration tests pass.
* `cargo clippy -p cli --all-targets -- -D warnings` — clean.
* `cargo fmt --all -- --check` — clean.

The `binary` and `line_mode` integration tests are sub-chapter e
concerns; they expect the line-mode fallback that d deliberately does
not implement.

## Section list

1. Step 0 — Entry and the RAII guard
2. Step 1 — The first frame
3. Step 2 — The frame reaches the terminal
4. Step 3 — One input
5. Step 4 — The key becomes a meaning
6. Step 5 — The command-line editor
7. Step 6 — The loop applies the outcome
8. Step 7 — The end
9. Verification

Final line count: 1669 lines.
