# Sub-chapter 05e — The line-mode fallback

Status: IMPLEMENTED. This sub-chapter adds the fallback for pipes, CI,
and redirects: a second `Screen` implementation and a second
`InputSource` implementation. It removes the `Unsupported` error from
sub-chapter d. It changes no session behavior.

## Abstract

The shell from sub-chapter d serves a terminal. This sub-chapter makes
the same loop serve everything else. `LineScreen` in `src/tui/screen.rs`
writes a frame as plain lines: one `writeln!` per line, no cursor
positioning, no styling. `LineInput` in `src/tui/study.rs` reads
complete lines and maps them through the shared grammar: the empty
line steps an AI seat to move, every other line goes through
`command::parse`, and end of input quits. `run` selects the shell by
one question — is stdout a terminal — and the answer is a pure
decision that a test pins. The loop itself changes in two lines: the
frame shows the board cursor only when the input source has one, and
the help screen lists the normal-mode keys only when they exist.

## Glossary

| Term | Definition |
|------|------------|
| **Canonical mode** | Terminal input mode with line buffering and echo. The terminal driver delivers a complete line at a time. The opposite of raw mode. |
| **Line mode** | The fallback shell: plain text output, complete-line input, no board cursor, no single keys. |
| **`LineScreen`** | The second `Screen` implementation. It writes the frame's lines to a writer and nothing more. |
| **`LineInput`** | The `InputSource` that reads complete lines from a `BufRead` reader and maps them through `command::parse`. |
| **Empty-line step** | An empty input line emits `Command::Step` when the side-to-move seat is AI. It does nothing otherwise. |
| **Selection rule** | The dispatch in `run`: a terminal gets the full-screen shell; anything else gets the line-mode fallback. |
| **EOF** | End of input on the reader. A piped script ends at EOF, so the line-mode loop treats EOF as quit. |
| **Golden transcript** | The byte-exact expected output of a scripted session, stored as a constant and asserted with one equality. |

## Context

Read [the design paper](../00-design.md) first. Its section
"e — The line-mode fallback" assigns this sub-chapter, and its target
structure shows the point: `AltScreen` and `LineScreen` sit below the
same `Screen` trait, and the loop above the trait does not know which
one is running.

Sub-chapter d ([d-shell](../d-shell/shell.md)) built the loop generic
over `Screen` and `InputSource` for exactly this moment. The fallback
adds implementations; it does not copy the loop. The reference build
did copy the loop: `run_alt` and `run_line` shared their logic
verbatim and differed only in rendering, and the two Swap2 loops
repeated the duplication. This sub-chapter is the proof that the
redesign's trait split removes that pattern.

Sub-chapter a ([a-refactor](../a-refactor/refactor.md)) supplies the
two shared texts: the grammar (`command::parse`, used by every input
path) and the help text (`repl::HELP`, reused by the help screen).
Its checklist also supplies the regression gate: the two scripted
demo transcripts must stay byte-identical.

## Intention

1. Extend `src/tui/screen.rs` with `LineScreen`: generic over the
   writer, one `writeln!` per frame line, one flush. A `stdout()`
   constructor serves production; tests capture the output in a
   `Vec<u8>` and recover it with `into_inner`.
2. Extend the `InputSource` trait in `src/tui/study.rs` with a
   defaulted predicate, `has_normal_mode`. Two consumers hang on the
   answer: the board cursor on the frame and the key table on the
   help screen.
3. Add `LineInput` to `src/tui/study.rs`: generic over `BufRead`,
   one line per call, EOF maps to quit. The line mapping is a pure
   function next to it: the empty line follows the reference rule,
   everything else goes through `command::parse`, parse errors become
   messages.
4. Replace the `Unsupported` error in `run` with the selection rule,
   expressed as a pure function from the terminal check to the shell
   choice.
5. Test headless: unit tests for the mapping and the predicate;
   integration tests in `tests/line_mode.rs` drive the real
   `study_loop` with a scripted `LineInput` and an output-capturing
   `LineScreen` and assert the exact transcript.
6. Prove that nothing else changed: the three gates pass, and the two
   scripted demo transcripts stay byte-identical.

## Mental mapping

### A fallback is a second implementation of a small trait

The design paper's sentence for this sub-chapter: a fallback is a
second implementation of a small trait, not a second copy of the
loop. The traits are small on purpose. `Screen` has one required
method (`draw`) and one defaulted one (`styled`). `InputSource` has
two required methods (`next_outcome`, `wait_any_key`) and, after this
sub-chapter, one defaulted predicate (`has_normal_mode`). Small
traits are what make a second implementation cheap: `LineScreen` is
about forty lines including its comments, and `LineInput` is smaller.

The loop did not change shape. It still draws a frame, reads one
outcome, applies it, and repeats. Two lines inside it changed, and
both are deletions of a false assumption: sub-chapter d assumed every
input source has a board cursor and every shell has normal-mode keys.
Line mode has neither, so the loop now asks instead of assuming.

### One grammar, one help text

Line mode's mapping has no special cases beyond the empty line. The
grammar already knows `h`, `help`, and `?` as `Command::Help` and
`q`, `quit`, and `exit` as `Command::Quit`, and the loop already
intercepts both commands — sub-chapter d built that interception for
the `:` command line, and a typed line is the same command. A special
case in `LineInput` would be a second implementation of a rule that
already has one.

The same argument covers the help text. The help screen reuses
`repl::HELP`, so the help the fallback prints and the help
sessiondemo prints cannot drift apart.

What about `repl::process_line`? The design paper says the fallback's
input side "parses them with the repl code from sub-chapter a". The
sharing is real but one level down: `process_line` is sessiondemo's
own loop — it translates a line, executes the command on the core,
and formats the resulting state as text. `LineInput` only translates;
`study_loop` executes; `render_study` formats. The pieces that can be
shared without duplicating a loop are the grammar and the help text,
and sub-chapter a already extracted both. Nothing new needed
extraction. One consequence: `info` stays a repl-only command,
because it reads search state instead of producing a `Command`. The
fallback, like the full-screen shell, answers `info` with a parse
error.

### The empty line keeps the half of Enter that survives

In the full-screen shell, Enter has two meanings during play: at a
human seat it plays at the cursor; at an AI seat it steps the AI.
Line mode has no cursor, so the first meaning cannot apply. The
second survives, and it is the reference build's rule: an empty line
steps when the side-to-move seat is AI, and does nothing otherwise.

Two guards come first. A finished game has nothing to step — the
reference's own guard. A ceremony in progress refuses `step`: the AI
searches game positions, and ceremony stones are not a game position.
Without the ceremony guard, an empty line during the opening with an
AI Black seat would answer every Enter with the `OpeningInProgress`
error. The reference predates the in-session ceremony (its ceremony
ran as a separate program), so the ceremony guard is this
sub-chapter's extension of its rule, documented in the code.

### What the fallback does not say

A shell must not advertise input it cannot take. Two displays
follow from that rule:

1. **No cursor marker.** The renderer draws the cursor as angle
   brackets around a cell. In line mode no input moves the cursor,
   so the frame omits the marker instead of drawing a marker that
   never moves.
2. **No key table in the help screen.** The normal-mode keys do not
   exist in line mode, so the help frame shows the typed command
   language only. The dismissal note also changes: canonical mode
   delivers a line at a time, so the note reads "Press Enter to
   return to the table."

Both hang on one predicate: `InputSource::has_normal_mode`. The
full-screen source answers `true` (the default); the line source
answers `false`. The ceremony needs no exception: the strict
alternation colors every placement, so the typed vocabulary (`play
h8`, `done`, `black`, `white`, `more`, `back`) is complete, and a
trailing color token on `play` is a parse error in both shells.

### EOF is quit

A piped script ends at end of input. `read_line` reports zero bytes
there, and `LineInput` maps that to `Quit` — the reference build's
line loop broke on end of input for the same reason. The mapping
differs from sub-chapter d's `ScriptedInput` test double, which
treats an exhausted script as an error. The difference is deliberate:
the test double exists to catch a badly written test, and the
production source exists to run real scripts to their end.

### The selection rule is a pure decision

`run` dispatches on `std::io::stdout().is_terminal()`: a terminal
gets `AltScreen` and `RawInput`; anything else gets `LineScreen` and
`LineInput`. The decision is extracted as `select_shell(bool) ->
Shell`, a pure function a test pins. `run` keeps only the side
effects: entering the guard, locking stdin, constructing the values.

One sentence for the docs, per the contract: piped output, CI, and
redirected sessions keep working — the fallback writes plain text and
reads complete lines, so a script drives the same loop as a terminal.

## Low-level design

### `src/tui/screen.rs` (extended)

```rust
pub struct LineScreen<W> { out: W }

impl LineScreen<io::Stdout> {
    pub fn stdout() -> Self;           // production
}

impl<W: Write> LineScreen<W> {
    pub fn new(out: W) -> Self;        // tests: Vec<u8>
    pub fn into_inner(self) -> W;      // recover the transcript
}

impl<W: Write> Screen for LineScreen<W> {
    fn draw(&mut self, frame: &[String]) -> io::Result<()> {
        // one writeln! per line, one flush at the end
    }
    // styled() keeps the default: false
}
```

Where `AltScreen` repaints the same twenty terminal rows,
`LineScreen` appends every frame below the previous one. A session
read back from a log then shows the frames in the order they
happened. That transcript is what the tests assert byte for byte.

### `src/tui/study.rs` (extended)

The trait gains one defaulted predicate:

```rust
pub trait InputSource {
    fn next_outcome(&mut self, state: &mut InputState, core: &SessionCore)
        -> io::Result<InputOutcome>;
    fn wait_any_key(&mut self) -> io::Result<()>;
    fn has_normal_mode(&self) -> bool { true }   // LineInput overrides
}
```

The new source and its mapping:

```rust
pub struct LineInput<R> { reader: R }

impl<R: BufRead> LineInput<R> {
    pub fn new(reader: R) -> Self;
}

impl<R: BufRead> InputSource for LineInput<R> {
    // next_outcome: read one line; 0 bytes (EOF) is Quit;
    // otherwise line_outcome(line, core)
    // wait_any_key: read one line and discard it
    fn has_normal_mode(&self) -> bool { false }
}

fn line_outcome(line: &str, core: &SessionCore) -> InputOutcome;
fn empty_line_outcome(core: &SessionCore) -> InputOutcome;
```

`line_outcome` is pure: the trimmed line in, the outcome out. The
empty line delegates to `empty_line_outcome` (ceremony guard,
game-over guard, then the seat check). Everything else goes through
`command::parse`; a parse error is a `Message` with the parser's own
text, the same as on the `:` command line.

`LineInput` lives in `study.rs`, not in `input.rs`. The choice:
`input.rs` is documented as the pure half of the shell — it performs
no I/O. An input source is the I/O edge: `RawInput` reads terminal
events, `LineInput` reads a byte stream. Both implementations sit
next to the trait they implement.

The loop changes in two places:

```rust
let cursor = input.has_normal_mode().then_some(state.cursor);
// build_frame takes cursor: Option<Move> and sets it on the view
// only when present
```

and the help call:

```rust
fn show_help<S: Screen, I: InputSource>(screen: &mut S, input: &mut I) -> io::Result<()> {
    screen.draw(&help_frame(input.has_normal_mode()))?;
    input.wait_any_key()
}
```

`help_frame(normal_mode: bool)` appends the key table and the
"Press any key" note only for a key-driven shell; otherwise it ends
with "Press Enter to return to the table."

The entry point:

```rust
pub fn run(init: SessionInit, hint: Option<&str>) -> io::Result<()> {
    let mut core = SessionCore::new(init);
    match select_shell(io::stdout().is_terminal()) {
        Shell::FullScreen => { /* AltScreen guard + RawInput */ }
        Shell::Line => { /* LineScreen::stdout() + LineInput over StdinLock */ }
    }
}

enum Shell { FullScreen, Line }

fn select_shell(stdout_is_terminal: bool) -> Shell;
```

One detail: `std::io::Stdin` does not implement `BufRead`, but
`StdinLock` does, so production constructs `LineInput::new(
io::stdin().lock())`.

### Tests

- `src/tui/study.rs`: 9 new unit tests. The empty-line rule per
  situation (human seats, an AI seat to move, the ceremony, a
  finished game), the grammar mapping (including the help and quit
  words and the ceremony vocabulary), the parse-error message, a
  scripted reader through EOF, the selection rule, the predicate, and
  the two help-frame shapes.
- `tests/line_mode.rs` (new): 4 integration tests through the real
  `study_loop` with a scripted `LineInput` and a `LineScreen` over a
  `Vec<u8>`.
  - A two-move hotseat session with an error line, the help screen,
    and quit asserts the exact transcript, byte for byte, against a
    stored constant. Named assertions on the side pin the properties
    the bytes carry: no cursor marker, the session's error text, the
    typed-language help without the key table, the Enter dismissal
    note.
  - A Swap2 ceremony runs entirely on typed commands: alternated
    placements, an undo, the count-validated `done`, the declined
    first choice, and the final choice. The phases must appear in
    protocol order, and the finished board holds the five stones,
    colored B, W, B, W, B by the strict alternation, with White to
    move.
  - A color token on `play` (`play h8 white`) is a parse error, the
    transcript shows the parser's message, and the failed line
    places nothing.
  - A script without a `quit` line ends at EOF, and the loop quits
    instead of erroring.

## Solution (opt-in)

The added and changed code, as annotated snippets with line numbers,
is in [fallback-solution.md](fallback-solution.md). Open it only if
you have been stuck for more than twenty minutes, or after you have
finished the sub-chapter and want to compare your work.

## TDD checklist

The rhythm is red, green, extend. Run from `gomoku/`. Capture the
golden transcripts first (sub-chapter a's checklist); both diffs must
be empty at the end.

1. **The screen.** Add `LineScreen` with its two constructors and
   `into_inner`. Nothing is headless-testable here yet beyond what
   the integration tests will cover; `cargo check -p cli` is the
   gate.
2. **The mapping.** Write the empty-line tests (human seats, an AI
   seat to move, the ceremony guard, the game-over guard), the
   grammar test, and the parse-error test against stubs. Implement
   `line_outcome` and `empty_line_outcome`. Green.
3. **The source.** Write the scripted-reader test (outcomes until
   EOF, then quit) and the predicate test. Implement `LineInput` and
   the `has_normal_mode` default on the trait. Green.
4. **The selection.** Write the `select_shell` test. Replace the
   `Unsupported` error in `run` with the match. Wire the cursor
   predicate into the loop and the key-table predicate into the help
   frame, with the help-frame shape test. All sub-chapter-d tests
   must stay green unchanged.
5. **The transcripts.** Write `tests/line_mode.rs`: the byte-exact
   two-move transcript, the typed ceremony, the EOF quit. Build the
   expected transcript constant from verified output, then review it
   line by line before freezing it — a golden test is only as good
   as the review its constant got.
6. **The gates.** Run all three gates and both transcript diffs.

## Verification commands

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Plus the transcript capture and diff from sub-chapter a's checklist.
Neither script touches the study shell; both diffs must be empty.

## Done when

* `LineScreen` draws with `writeln!` only, keeps `styled()` at its
  default `false`, is generic over the writer, and has a `stdout()`
  constructor.
* `LineInput` reads complete lines, is generic over `BufRead`, maps
  the empty line by the reference rule (with the ceremony and
  game-over guards), sends everything else through `command::parse`,
  and maps EOF to quit.
* `run` selects the shell with `is_terminal()` and the `Unsupported`
  error is gone. The selection rule is a pure, tested function.
* The frame shows no cursor marker in line mode, and the help screen
  shows no key table there. Both hang on `has_normal_mode`.
* All three gates pass, and both demo transcripts are unchanged.
  (Result here: 143 tests, up from 130. The lib holds 87, up from 78:
  +9 line-mode unit tests. `tests/line_mode.rs` adds 4.)

Next: sub-chapter f — the binary.

## References

* [00-design.md](../00-design.md) — the contract for all six
  sub-chapters; section e assigns this one.
* [d-shell/shell.md](../d-shell/shell.md) — the loop, the `Screen`
  and `InputSource` traits, and the help screen this sub-chapter
  reuses.
* [a-refactor/refactor.md](../a-refactor/refactor.md) — the grammar
  and the help text the fallback shares with sessiondemo, and the
  transcript regression checklist.
* [c-swap2/swap2.md](../c-swap2/swap2.md) — the ceremony the typed
  commands drive, and the reason `step` is refused during it.
