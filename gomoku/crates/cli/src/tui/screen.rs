//! The screen abstraction: where a finished frame goes.
//!
//! One trait, one method. The loop in [`crate::tui::study`] builds a
//! frame as a `Vec<String>` and hands it to a [`Screen`]; the screen
//! decides how the lines reach the user. Two implementations exist.
//! [`AltScreen`] is the full-screen one: crossterm, the alternate
//! screen, and an RAII guard that restores the terminal on every exit
//! path. [`LineScreen`] is the fallback for pipes, CI logs, and
//! redirects: it appends the frame's lines to a writer and nothing
//! more.
//!
//! The trait is deliberately small. A screen does not know about
//! sessions, commands, or keys; it knows how to draw lines. That is
//! what lets a test replace the terminal with a `Vec` of recorded
//! frames.

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::Print,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};

/// A destination for one rendered frame.
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

impl Screen for AltScreen {
    /// Paint the frame: one clear, then every line at its absolute
    /// position, all queued and flushed once.
    ///
    /// Painting a frame is dozens of cursor moves and prints. `queue!`
    /// batches them into one buffer and the single `flush` at the end
    /// sends them together; flushing per line is what would make the
    /// picture flicker. Absolute positioning (`MoveTo(0, y)` per line)
    /// makes the paint independent of where a previous frame left the
    /// cursor.
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
