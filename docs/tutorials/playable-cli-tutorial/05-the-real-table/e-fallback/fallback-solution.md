# Sub-chapter 05e — The line-mode fallback: solution

> **Opt-in solution — read only after you have tried the sub-chapter, or
> if you have been stuck for more than twenty minutes.**
>
> This file quotes the code that sub-chapter e adds or changes, as
> annotated snippets with line numbers. Line numbers refer to the files
> after the sub-chapter is applied. Deletions are described in prose;
> raw diffs are not shown. Verified green: `cargo test -p cli`,
> `cargo clippy --all-targets -- -D warnings`, and
> `cargo fmt --all -- --check` all pass from `gomoku/`. The two
> scripted demo transcripts (sub-chapter a's checklist) are
> byte-identical to before.

Reading order: the screen first (`screen.rs`), then the study
additions (`study.rs`), then the integration tests
(`tests/line_mode.rs`).

## src/tui/screen.rs (extended, 179 lines, +57)

The module doc (lines 1–17) now names both implementations:
`AltScreen` repaints the same twenty terminal rows; `LineScreen`
appends every frame below the previous one, so a session read back
from a log shows the frames in order. That order property is what
makes the transcript testable.

`LineScreen` is generic over the writer. Tests pass a `Vec<u8>` and
recover it with `into_inner`; production uses the named `stdout()`
constructor. `styled()` keeps the trait's default `false`: pipes and
CI logs get plain text.

src/tui/screen.rs, lines 126–164:

```rust
/// The line-mode screen: the fallback for pipes, CI, and redirects.
///
/// `draw` writes the frame's lines and nothing else: no cursor
/// positioning, no clearing, no styling (`styled()` keeps the trait's
/// default `false`). Where [`AltScreen`] repaints the same twenty
/// terminal rows, `LineScreen` appends every frame below the previous
/// one, so a session read back from a log shows the frames in the
/// order they happened. That transcript is what the tests assert byte
/// for byte.
///
/// The type is generic over the writer so tests can capture the output
/// in a `Vec<u8>`; production constructs it with [`LineScreen::stdout`].
pub struct LineScreen<W> {
    out: W,
}

impl LineScreen<io::Stdout> {
    /// A screen that prints to stdout.
    #[must_use]
    pub fn stdout() -> Self {
        Self { out: io::stdout() }
    }
}

impl<W: Write> LineScreen<W> {
    /// A screen over any writer.
    #[must_use]
    pub fn new(out: W) -> Self {
        Self { out }
    }

    /// Give the writer back.
    ///
    /// Tests capture the transcript in a `Vec<u8>` and recover it here
    /// after the loop returned.
    pub fn into_inner(self) -> W {
        self.out
    }
}
```

The draw is `AltScreen`'s batching idiom reduced to its minimum: one
write per line, one flush at the end, no cursor commands at all.

src/tui/screen.rs, lines 166–179:

```rust
impl<W: Write> Screen for LineScreen<W> {
    /// Print the frame: one `writeln!` per line, one flush at the end.
    ///
    /// The flush mirrors [`AltScreen`]'s batching: the frame goes out
    /// as one unit, not line by line whenever the buffer happens to
    /// fill. stdout is line-buffered, but the generic writer (a file,
    /// a pipe buffer, a `Vec`) makes no such promise.
    fn draw(&mut self, frame: &[String]) -> io::Result<()> {
        for line in frame {
            writeln!(self.out, "{line}")?;
        }
        self.out.flush()
    }
}
```

## src/tui/study.rs (extended, 950 lines, +9 unit tests)

### The trait predicate

The `InputSource` trait gains one defaulted method. Sub-chapter d
assumed two things about every shell: a board cursor exists, and the
normal-mode keys exist. Line mode has neither, so the loop now asks.
The default is `true`, and `RawInput` and the test doubles keep it.

src/tui/study.rs, lines 69–80:

```rust
    /// Whether this source has a normal mode: single keys that act, a
    /// board cursor, and a `:` command line.
    ///
    /// The loop asks this question in two places. A `true` answer
    /// puts the board cursor on the frame and the normal-mode key
    /// table on the help screen. A `false` answer (the line-mode
    /// source) omits both: a cursor that no input can move and a key
    /// table for keys that do not exist would both be false
    /// statements about the shell.
    fn has_normal_mode(&self) -> bool {
        true
    }
```

### The line-mode source

`LineInput` lives in `study.rs`, not in `input.rs` — see the
deviations section for the placement argument. The struct doc (lines
100–116) states the two rules of the mapping: no special cases beyond
the empty line, because the grammar already knows the help and quit
words; and no cursor-only keys, because there is no cursor to move
and no normal mode to press them in.

src/tui/study.rs, lines 117–131:

```rust
pub struct LineInput<R> {
    reader: R,
}

impl<R: BufRead> LineInput<R> {
    /// A source over any line reader.
    ///
    /// Production passes a locked stdin (`StdinLock` implements
    /// [`BufRead`]; `Stdin` itself does not). Tests pass an in-memory
    /// reader such as `Cursor<&str>`.
    #[must_use]
    pub fn new(reader: R) -> Self {
        Self { reader }
    }
}
```

The `InputSource` implementation (lines 133–166) does three things:
reads one line, maps EOF to quit, and delegates the text to the pure
`line_outcome`. The input state is explicitly unused — a line carries
its own meaning, so there is no cursor to move, no buffer to edit,
and no override to arm. `wait_any_key` reads and discards one line:
the "any key" of canonical mode is the Enter key, and EOF dismisses
the help screen too.

src/tui/study.rs, lines 133–151:

```rust
impl<R: BufRead> InputSource for LineInput<R> {
    fn next_outcome(
        &mut self,
        state: &mut InputState,
        core: &SessionCore,
    ) -> io::Result<InputOutcome> {
        // `state` exists for key-driven sources. A line of text
        // carries its own meaning: there is no cursor to move, no
        // buffer to edit, and no override to arm.
        let _ = state;
        let mut line = String::new();
        // `read_line` reports 0 bytes at EOF. A piped script ends at
        // EOF, so EOF means quit — the reference build's line loop
        // broke on end of input for the same reason.
        if self.reader.read_line(&mut line)? == 0 {
            return Ok(InputOutcome::Quit);
        }
        Ok(line_outcome(&line, core))
    }
```

Note the deliberate contrast with sub-chapter d's `ScriptedInput`
test double, which treats an exhausted script as an error. The double
exists to catch a badly written test; the production source exists to
run real scripts to their end.

### The mapping

Two small pure functions follow (lines 168–212). `line_outcome`
trims, delegates the empty line, and sends everything else through
`command::parse` — one grammar exists, and a second parser here would
drift from it, the way the reference build's two line parsers drifted
apart. A parse error becomes a `Message` with the parser's own text,
the same as on the `:` command line.

src/tui/study.rs, lines 186–209:

```rust
/// The empty line, by the reference build's rule: `step` when the
/// side-to-move seat is AI, nothing otherwise.
///
/// This is the surviving half of Enter's normal-mode meaning. Enter at
/// a human seat plays at the cursor, and a cursor does not exist in
/// line mode, so that half cannot apply. Two guards come first. A
/// finished game has nothing to step (the reference's own guard). And
/// a ceremony in progress refuses `step` — the AI searches game
/// positions, and ceremony stones are not a game position — so the
/// empty line stays quiet there instead of answering every Enter with
/// the `OpeningInProgress` error. The reference predates the
/// in-session ceremony, so the ceremony guard is this sub-chapter's
/// extension of its rule.
fn empty_line_outcome(core: &SessionCore) -> InputOutcome {
    let board = core.board();
    if core.snapshot().opening.is_some() || board.status() != Status::Ongoing {
        return InputOutcome::None;
    }
    if core.occupant(board.to_move()).is_ai() {
        InputOutcome::Command(Command::Step)
    } else {
        InputOutcome::None
    }
}
```

### The selection rule

`run` (lines 229–265) lost the `Unsupported` error. The
`SessionCore` is now built before the match, so a construction panic
happens before the terminal is touched at all. The `FullScreen` arm
is sub-chapter d's body unchanged; the `Line` arm constructs the
fallback pair. Production stdin needs a lock: `StdinLock` implements
`BufRead`, and `Stdin` itself does not.

src/tui/study.rs, lines 243–263:

```rust
pub fn run(init: SessionInit, hint: Option<&str>) -> io::Result<()> {
    let mut core = SessionCore::new(init);
    match select_shell(io::stdout().is_terminal()) {
        Shell::FullScreen => {
            // The guard restores the terminal on every exit path,
            // panic included. The binding must stay alive until the
            // loop returns.
            let mut screen = AltScreen::enter()?;
            let mut input = RawInput;
            study_loop(&mut core, &mut screen, &mut input, hint)
        }
        Shell::Line => {
            let mut screen = LineScreen::stdout();
            // `StdinLock` implements `BufRead`; `Stdin` itself does
            // not. The lock borrows from a static handle, so it lives
            // as long as the loop needs it.
            let mut input = LineInput::new(io::stdin().lock());
            study_loop(&mut core, &mut screen, &mut input, hint)
        }
    }
}
```

The decision is extracted (lines 267–289): a private `Shell` enum and
`select_shell(stdout_is_terminal: bool) -> Shell`. One bit in, one
choice out. The unit test pins both directions; `run` keeps only the
side effects.

### The loop's two changed lines

The first change is the cursor. `build_frame` (lines 356–388) now
takes `cursor: Option<Move>` and sets it on the view only when
present; the loop computes the option from the predicate.

src/tui/study.rs, lines 310–323:

```rust
    loop {
        // The frame shows the board cursor only when the input source
        // has one to move. The line-mode source has none, so it draws
        // no marker instead of a marker no key can move.
        let cursor = input.has_normal_mode().then_some(state.cursor);
        let frame = build_frame(
            core,
            &state,
            cursor,
            &events,
            message.as_deref(),
            screen.styled(),
        );
        screen.draw(&frame)?;
```

The second change is the help call. `help_frame` now takes the
predicate and shapes two outputs from it: the key table, and the
dismissal note — canonical mode delivers a line at a time, so the
line-mode note names the Enter key.

src/tui/study.rs, lines 406–438:

```rust
/// Draw the help screen and wait for one input to dismiss it.
fn show_help<S: Screen, I: InputSource>(screen: &mut S, input: &mut I) -> io::Result<()> {
    screen.draw(&help_frame(input.has_normal_mode()))?;
    input.wait_any_key()
}

/// The help frame: the typed command language, then — for a shell
/// with a normal mode — the normal-mode key table, then the dismissal
/// note.
///
/// The command-language part reuses [`repl::HELP`], so the help screen
/// and the line-mode `help` command cannot drift apart. The key table
/// is conditional: in line mode the keys do not exist, and a help
/// text must not advertise input the shell cannot take. The frame is
/// unstyled text: it must stay readable on every terminal, and the
/// help screen has no stones to highlight.
fn help_frame(normal_mode: bool) -> Vec<String> {
    // `HELP` ends with a newline; `str::lines` drops it, so the blank
    // separator between the parts is added explicitly.
    let mut lines: Vec<String> = repl::HELP.lines().map(str::to_string).collect();
    let note = if normal_mode {
        lines.push(String::new());
        lines.extend(NORMAL_MODE_KEYS.lines().map(str::to_string));
        "Press any key to return to the table."
    } else {
        // Canonical mode delivers a line at a time, so the dismissing
        // "key" is Enter.
        "Press Enter to return to the table."
    };
    lines.push(String::new());
    lines.push(note.to_string());
    lines
}
```

### The unit tests

Nine new tests (lines 460–950, shared module with sub-chapter d's
loop tests). The empty-line rule gets four: human seats stay quiet,
an AI seat to move steps, the ceremony stays quiet even with an AI
Black seat, and a finished game stays quiet even with an AI seat to
move. The last two are the discriminators: without the guards, both
would answer `Step`. One test pins the grammar mapping across the
help words, the quit words, and the ceremony vocabulary. One example
of the style:

```rust
#[test]
fn the_empty_line_stays_quiet_during_the_ceremony() {
    // An AI Black seat would qualify for a step during play, but
    // the ceremony refuses step: the AI searches game positions,
    // and ceremony stones are not a game position.
    let mut core = SessionCore::new(SessionInit::new_hotseat().with_swap2(true));
    core.execute(Command::Release {
        seat: SeatSel::Black,
    })
    .unwrap();
    assert_eq!(line_outcome("", &core), InputOutcome::None);
}
```

The remaining tests: the parse error is a `Message` with the parser's
text; a scripted reader yields outcomes until EOF and then quits,
repeatably, with the input state untouched; `select_shell` maps both
directions; the predicate distinguishes the sources; and
`help_frame(false)` drops the key table and changes the note.

## tests/line_mode.rs (new, 269 lines, 3 tests)

tests/line_mode.rs, lines 1–11:

```rust
//! The line-mode fallback, end to end: scripted lines through the real
//! `LineInput` and the real `study_loop`, with the output captured by a
//! `LineScreen` over an in-memory writer. No terminal is involved.

use cli::session::{SessionCore, SessionInit};
use cli::tui::screen::LineScreen;
use cli::tui::study::{LineInput, study_loop};
use engine::Color;
use std::io::Cursor;

use testkit::{mv, swap2_core};
```

The file drives the real `study_loop` with the real `LineInput` and
the real `LineScreen`; only the streams are in-memory. The helper is
the whole harness:

tests/line_mode.rs, lines 13–24:

```rust
/// Run the line-mode loop over a script and return the finished core
/// and the captured transcript.
fn run_line_script(mut core: SessionCore, script: &str) -> (SessionCore, String) {
    let mut screen = LineScreen::new(Vec::new());
    let mut input = LineInput::new(Cursor::new(script.to_string()));
    study_loop(&mut core, &mut screen, &mut input, None).expect("a scripted session runs clean");
    let bytes = screen.into_inner();
    (
        core,
        String::from_utf8(bytes).expect("the transcript is UTF-8"),
    )
}
```

### The golden transcript

The first test runs a two-move hotseat session with an illegal repeat
of the first move, the help screen dismissed with Enter, and quit,
and asserts equality against `TWO_MOVE_TRANSCRIPT` (lines 34–156):
122 lines, six frames (initial, one per processed line, the help
frame, the frame after the dismissal). The constant is not quoted
here; it is ordinary renderer output, and the doc comment above it
carries a warning worth repeating:

tests/line_mode.rs, lines 26–34:

```rust
/// The exact transcript of [`the_two_move_transcript_is_byte_exact`],
/// byte for byte: the initial frame, one frame per processed line,
/// the help frame, and the frame after the dismissal.
///
/// WARNING: many lines end in two spaces. The renderer joins the board
/// and the panel with a two-space gutter, and rows past the end of the
/// panel end there. The raw string preserves those spaces; do not
/// strip trailing whitespace in this file.
const TWO_MOVE_TRANSCRIPT: &str = r#"     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o       -- Study table --
```

The constant was built from verified output and then reviewed line by
line before freezing: stones in the right cells, the bracketed last
move, the error text on the message line, no cursor markers anywhere.
A golden test is only as good as the review its constant got. After
the equality assertion, named assertions pin the individual
properties — no `<` anywhere, the session's error text, the
typed-language help without the key table, the Enter dismissal note —
so a regression says what it broke before the byte comparison dumps
two frames.

### The typed ceremony

The second test runs the full Swap2 protocol with no keys: default
and override placements, a `back`, the count-validated `done`,
`more`, the second round, and the final choice. The phase lines must
appear in protocol order in the transcript, asserted by scanning
forward from the last hit:

tests/line_mode.rs, lines 216–228:

```rust
    // The phases appear in protocol order across the frames.
    let mut at = 0;
    for phase in [
        "Swap2: Placing3",
        "Swap2: FirstChoice",
        "Swap2: Placing2",
        "Swap2: FinalChoice",
    ] {
        let found = transcript[at..]
            .find(phase)
            .unwrap_or_else(|| panic!("{phase} missing after byte {at}: {transcript}"));
        at += found + phase.len();
    }
```

The session assertions close the test: the ceremony is finished,
White moves first, and the five stones sit where the script put them.
One negative assertion pins the input model: the transcript contains
no `(override)` panel line, because the `b`/`w` keys that arm an
override do not exist in line mode — the typed `play f8 white`
carries its color on the command.

The third test is the EOF contract: a script of one move and no
`quit` line runs the move and quits at end of input, instead of
erroring the way a key script would.

## Deviations from the design paper's wording

Five, all documented in the code where they apply:

1. **`LineInput` lives in `study.rs`, not in `input.rs`.** The paper
   names no file. `input.rs` documents itself as the pure half of the
   shell — it performs no I/O — and an input source is the I/O edge:
   `RawInput` reads terminal events, `LineInput` reads a byte stream.
   Both implementations sit next to the trait they implement, and the
   module doc of `study.rs` records the choice.
2. **The shared repl code is the grammar and the help text, not
   `process_line`.** The paper says the fallback's input side "parses
   them with the repl code from sub-chapter a". `process_line` is
   sessiondemo's own loop: it translates, executes, and formats.
   `LineInput` only translates; `study_loop` executes; `render_study`
   formats. The shareable pieces — `command::parse` and `repl::HELP`
   — were already extracted in sub-chapter a, so nothing new was
   extracted and no logic was duplicated. One consequence: `info`
   stays a repl-only command (it reads search state instead of
   producing a `Command`), and the fallback answers it with a parse
   error, exactly as the full-screen shell does.
3. **The empty-line rule has a ceremony guard.** The reference rule
   is `Step` iff the side-to-move seat is AI, guarded by game-over.
   The reference predates the in-session ceremony, where `step` is
   always the `OpeningInProgress` error; the guard keeps the empty
   line quiet during the opening instead of answering every Enter
   with an error.
4. **The trait gains `has_normal_mode`.** The paper's fallback
   section does not mention it. Three displays hang on it: the frame
   shows no cursor marker that no input can move, the help screen
   lists no keys that do not exist, and the dismissal note names the
   Enter key. A defaulted method keeps `RawInput` and the test
   doubles unchanged.
5. **EOF is quit, not an error.** `LineInput` maps end of input to
   `Quit` (the reference's line loop broke on EOF), while
   sub-chapter d's `ScriptedInput` treats exhaustion as an error.
   The difference is deliberate: the double guards tests, the
   production source runs real scripts.

## Verification

From `gomoku/`, after the sub-chapter:

* `cargo test -p cli` — 144 tests pass (132 before): 87 lib tests (78
  before; +9 line-mode unit tests), 3 line-mode integration tests,
  and the unchanged suites (7 actor, 9 history, 20 opening, 13
  session, 4 study, 1 doctest).
* `cargo clippy --all-targets -- -D warnings` — clean.
* `cargo fmt --all -- --check` — clean.
* Both demo transcripts are byte-identical to before. The fallback
  adds no output path to sessiondemo; the repl code is untouched.
* One check beyond the gates: the real `run()` was exercised through
  a pipe with a temporary example binary (deleted after use). Its
  output was byte-identical to the in-memory golden transcript, so
  the production stdin/stdout path and the test path agree.

A note on size: `study.rs` is 950 lines, 459 without tests (263
without tests after sub-chapter d). The growth is the second input
source, its mapping, and the selection rule, under the chapter's
commenting standard. The file holds one concern — the loop and its
input edges — which is the budget's stated purpose. A `line.rs` split
for size alone would separate `LineInput` from the trait and from
`RawInput`, and the alternative placement, `input.rs`, is the
documented no-I/O module. The trade reads worse than the overage, as
it did in sub-chapter d.
