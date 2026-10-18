> **Opt-in solution — read only after you have tried the chapter, or if you have been stuck for more than twenty minutes.**
>
> This file quotes the four files that changed at commit `a377237`:
> `src/tui/overlay.rs` (new), `src/tui/study.rs`, `src/tui/render.rs`,
> and `src/tui/mod.rs`. All other files are unchanged from chapter 05.
> The quoted text was verified byte-for-byte against the reference
> worktree with `git show a377237:<path>`.

# Chapter 06 — opt-in solution

## Unchanged files

The following files are identical to their chapter-05 versions at
`a2e5373`:

* `gomoku/crates/cli/src/lib.rs`
* `gomoku/crates/cli/src/main.rs`
* `gomoku/crates/cli/src/history.rs`
* `gomoku/crates/cli/src/command.rs`
* `gomoku/crates/cli/src/session.rs`
* `gomoku/crates/cli/src/actor.rs`
* `gomoku/crates/cli/src/fmt.rs`
* `gomoku/crates/cli/src/tui/board.rs`
* `gomoku/crates/cli/src/tui/input.rs`
* `gomoku/crates/cli/src/tui/opening.rs`
* `gomoku/crates/cli/src/tui/puzzle.rs`
* `gomoku/crates/cli/src/tui/ui.rs`

## `gomoku/crates/cli/src/tui/mod.rs`

```rust
//! Existing terminal UI modules.
//!
//! Moved under `tui/` during the lib/bin split; their behavior is
//! unchanged.

pub mod board;
pub mod input;
pub mod opening;
pub mod overlay;
pub mod puzzle;
pub mod render;
pub mod study;
pub mod ui;
```

## `gomoku/crates/cli/src/tui/overlay.rs`

```rust
//! Policy overlay and value strip — pure functions, no I/O.
//!
//! These helpers turn a cursor node's [`SearchInfo`] into per-cell render
//! data and a side-panel value strip. They are deliberately testable
//! without a real MCTS search: callers can construct [`SearchInfo`] directly.

use engine::{Color, Move};

use crate::actor::{AiConfig, SearchInfo};

/// Active cell-overlay mode for the policy heatmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PolicyOverlay {
    /// No overlay; show the bare board.
    #[default]
    Off,
    /// Heatmap by share of total visits.
    Visits,
    /// Heatmap by prior probability, normalised to the max prior so the
    /// brightest cell is always the move the evaluator liked best.
    Prior,
}

impl PolicyOverlay {
    /// Cycle order: Off → Visits → Prior → Off.
    #[must_use]
    pub fn cycle(self) -> Self {
        match self {
            PolicyOverlay::Off => PolicyOverlay::Visits,
            PolicyOverlay::Visits => PolicyOverlay::Prior,
            PolicyOverlay::Prior => PolicyOverlay::Off,
        }
    }

    /// Short label used in status messages.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            PolicyOverlay::Off => "off",
            PolicyOverlay::Visits => "visits",
            PolicyOverlay::Prior => "prior",
        }
    }
}

/// Per-cell render data for a policy overlay cell.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayCell {
    /// Background intensity in 0.0–1.0, used to select the ANSI-256 colour.
    pub intensity: f32,
    /// Visible text for hot cells; empty for dim cells.
    pub text: String,
    /// Whether this cell is hot enough to print its value.
    pub is_hot: bool,
}

/// Produce render data for one board cell under the given overlay mode.
///
/// Returns `None` for empty cells with no overlay contribution (e.g. an
/// unvisited move in Visits mode, or Off mode).
#[must_use]
pub fn overlay_cell(info: &SearchInfo, mv: Move, mode: PolicyOverlay) -> Option<OverlayCell> {
    match mode {
        PolicyOverlay::Off => None,
        PolicyOverlay::Visits => visits_cell(info, mv),
        PolicyOverlay::Prior => prior_cell(info, mv),
    }
}

fn visits_cell(info: &SearchInfo, mv: Move) -> Option<OverlayCell> {
    let total: u32 = info.visits.iter().map(|(_, n)| n).sum();
    if total == 0 {
        return None;
    }
    let visits = info
        .visits
        .iter()
        .find(|(m, _)| *m == mv)
        .map(|(_, n)| *n)
        .unwrap_or(0);
    let share = visits as f32 / total as f32;
    let is_hot = share >= 0.10;
    let text = if is_hot {
        visits.to_string()
    } else {
        String::new()
    };
    Some(OverlayCell {
        intensity: share,
        text,
        is_hot,
    })
}

fn prior_cell(info: &SearchInfo, mv: Move) -> Option<OverlayCell> {
    let max_prior = info
        .prior
        .iter()
        .map(|(_, p)| p)
        .fold(0.0f32, |a, &b| a.max(b));
    if max_prior == 0.0 {
        return None;
    }
    let prior = info
        .prior
        .iter()
        .find(|(m, _)| *m == mv)
        .map(|(_, p)| *p)
        .unwrap_or(0.0);
    let intensity = prior / max_prior;
    let is_hot = intensity >= 0.10;
    let text = if is_hot {
        format!("{:.0}%", prior * 100.0)
    } else {
        String::new()
    };
    Some(OverlayCell {
        intensity,
        text,
        is_hot,
    })
}

/// Map an intensity 0.0–1.0 to an ANSI-256 background colour.
///
/// This is the same discrete ramp used by `patterns::render_heatmap`:
/// hot values glow red→orange→yellow, lower values fade to light grey,
/// and the near-zero dim band uses a muted grey.
#[must_use]
pub fn heatmap_bg(intensity: f32) -> u8 {
    match (intensity * 10.0).round() as i32 {
        10 => 196,
        9 => 208,
        8 => 220,
        3 => 39,
        _ => 250,
    }
}

/// Render one styled cell with an ANSI-256 background.
#[must_use]
pub fn render_overlay_cell_styled(glyph: char, cell: &OverlayCell) -> String {
    let bg = heatmap_bg(cell.intensity);
    let content = cell_content(glyph, cell);
    format!("\x1b[48;5;{bg}m{content}\x1b[0m")
}

/// Render one plain cell (no ANSI escapes).
#[must_use]
pub fn render_overlay_cell_plain(glyph: char, cell: &OverlayCell) -> String {
    cell_content(glyph, cell)
}

fn cell_content(glyph: char, cell: &OverlayCell) -> String {
    if cell.is_hot {
        pad_numeric_cell(&cell.text)
    } else {
        format!(" {} ", glyph)
    }
}

fn pad_numeric_cell(text: &str) -> String {
    if text.len() >= 3 {
        text[..3].to_string()
    } else {
        format!("{:>3}", text)
    }
}

/// Build the value-strip panel lines from a cursor's search info.
///
/// `side_at_search` is the side to move when the search was run (the
/// parent of the cursor node). `current_config` is the live config of that
/// same seat now; if it differs from the fingerprint stored in
/// [`SearchInfo::config`], a staleness note is appended.
#[must_use]
pub fn value_strip(
    info: Option<&SearchInfo>,
    current_config: Option<&AiConfig>,
    side_at_search: Option<Color>,
) -> Vec<String> {
    let Some(info) = info else {
        return Vec::new();
    };

    let total: u32 = info.visits.iter().map(|(_, n)| n).sum();
    let value_sign = if info.value >= 0.0 { '+' } else { '-' };
    let side = side_at_search.unwrap_or(Color::Black);

    let mut lines = Vec::new();
    lines.push(format!(
        "value {}{:.2} for {:?}",
        value_sign,
        info.value.abs(),
        side
    ));
    lines.push(format!("total visits: {total}"));
    lines.push("top moves:".to_string());
    for (i, (mv, n)) in info.visits.iter().take(3).enumerate() {
        let share = if total > 0 {
            *n as f32 / total as f32
        } else {
            0.0
        };
        lines.push(format!(
            "  {}. ({}, {}) {:>5.1}%",
            i + 1,
            mv.row(),
            mv.col(),
            share * 100.0
        ));
    }

    let stale = match current_config {
        None => true,
        Some(cfg) => cfg != &info.config,
    };
    if stale {
        lines.push(format!("(!searched at sims={})", info.config.simulations));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Move;

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    fn dummy_config(simulations: u32) -> AiConfig {
        AiConfig {
            simulations,
            ..AiConfig::default()
        }
    }

    fn search_info_with_visits(
        visits: &[(Move, u32)],
        prior: &[(Move, f32)],
        value: f32,
    ) -> SearchInfo {
        SearchInfo {
            visits: visits.to_vec(),
            prior: prior.to_vec(),
            value,
            config: dummy_config(400),
        }
    }

    #[test]
    fn cycle_order_is_off_visits_prior_off() {
        assert_eq!(PolicyOverlay::Off.cycle(), PolicyOverlay::Visits);
        assert_eq!(PolicyOverlay::Visits.cycle(), PolicyOverlay::Prior);
        assert_eq!(PolicyOverlay::Prior.cycle(), PolicyOverlay::Off);
    }

    #[test]
    fn overlay_off_returns_none() {
        let info = search_info_with_visits(&[(mv(7, 7), 100)], &[(mv(7, 7), 1.0)], 0.3);
        assert!(overlay_cell(&info, mv(7, 7), PolicyOverlay::Off).is_none());
    }

    #[test]
    fn zero_visits_gives_zero_intensity_and_not_hot() {
        let info = search_info_with_visits(&[(mv(7, 7), 100)], &[], 0.0);
        let cell = overlay_cell(&info, mv(0, 0), PolicyOverlay::Visits).unwrap();
        assert_eq!(cell.intensity, 0.0);
        assert!(!cell.is_hot);
        assert!(cell.text.is_empty());
    }

    #[test]
    fn single_hot_move_has_full_intensity() {
        let info = search_info_with_visits(&[(mv(7, 7), 400)], &[], 0.5);
        let cell = overlay_cell(&info, mv(7, 7), PolicyOverlay::Visits).unwrap();
        assert_eq!(cell.intensity, 1.0);
        assert!(cell.is_hot);
        assert_eq!(cell.text, "400");
    }

    #[test]
    fn visit_share_computes_intensity_correctly() {
        let info = search_info_with_visits(&[(mv(7, 7), 300), (mv(6, 6), 100)], &[], 0.0);
        let hot = overlay_cell(&info, mv(7, 7), PolicyOverlay::Visits).unwrap();
        assert!((hot.intensity - 0.75).abs() < 1e-6);
        assert!(hot.is_hot);

        let warm = overlay_cell(&info, mv(6, 6), PolicyOverlay::Visits).unwrap();
        assert!((warm.intensity - 0.25).abs() < 1e-6);
        assert!(warm.is_hot); // 25% is above the 10% threshold
    }

    #[test]
    fn small_visit_share_is_dim() {
        let info = search_info_with_visits(&[(mv(7, 7), 390), (mv(6, 6), 10)], &[], 0.0);
        let dim = overlay_cell(&info, mv(6, 6), PolicyOverlay::Visits).unwrap();
        assert_eq!(dim.intensity, 0.025);
        assert!(!dim.is_hot);
    }

    #[test]
    fn prior_normalises_by_max_prior() {
        let info = search_info_with_visits(&[], &[(mv(7, 7), 0.5), (mv(6, 6), 0.25)], 0.0);
        let best = overlay_cell(&info, mv(7, 7), PolicyOverlay::Prior).unwrap();
        assert_eq!(best.intensity, 1.0);
        assert!(best.is_hot);
        assert_eq!(best.text, "50%");

        let half = overlay_cell(&info, mv(6, 6), PolicyOverlay::Prior).unwrap();
        assert_eq!(half.intensity, 0.5);
        assert!(half.is_hot);
        assert_eq!(half.text, "25%");
    }

    #[test]
    fn below_threshold_prior_is_dim() {
        let info = search_info_with_visits(&[], &[(mv(7, 7), 0.5), (mv(6, 6), 0.04)], 0.0);
        let dim = overlay_cell(&info, mv(6, 6), PolicyOverlay::Prior).unwrap();
        assert_eq!(dim.intensity, 0.08);
        assert!(!dim.is_hot);
        assert!(dim.text.is_empty());
    }

    #[test]
    fn value_strip_formats_sign_and_color() {
        let info = SearchInfo {
            visits: vec![(mv(7, 7), 300), (mv(6, 6), 100)],
            prior: vec![],
            value: 0.42,
            config: dummy_config(400),
        };
        let lines = value_strip(Some(&info), Some(&dummy_config(400)), Some(Color::White));
        assert_eq!(lines[0], "value +0.42 for White");
        assert!(lines.iter().any(|l| l.contains("total visits: 400")));
        assert!(lines.iter().any(|l| l.contains("75.0%")));
    }

    #[test]
    fn value_strip_negative_value_keeps_sign() {
        let info = SearchInfo {
            visits: vec![(mv(7, 7), 100)],
            prior: vec![],
            value: -0.17,
            config: dummy_config(400),
        };
        let lines = value_strip(Some(&info), Some(&dummy_config(400)), Some(Color::Black));
        assert_eq!(lines[0], "value -0.17 for Black");
    }

    #[test]
    fn value_strip_appends_staleness_when_config_differs() {
        let info = SearchInfo {
            visits: vec![(mv(7, 7), 100)],
            prior: vec![],
            value: 0.0,
            config: dummy_config(400),
        };
        let current = dummy_config(800);
        let lines = value_strip(Some(&info), Some(&current), Some(Color::Black));
        assert!(lines.iter().any(|l| l.contains("(!searched at sims=400)")));
    }

    #[test]
    fn value_strip_is_stale_when_seat_is_human() {
        let info = SearchInfo {
            visits: vec![(mv(7, 7), 100)],
            prior: vec![],
            value: 0.0,
            config: dummy_config(400),
        };
        let lines = value_strip(Some(&info), None, Some(Color::Black));
        assert!(lines.iter().any(|l| l.contains("(!searched at sims=400)")));
    }

    #[test]
    fn value_strip_no_search_info_returns_empty() {
        let lines = value_strip(None, None, None);
        assert!(lines.is_empty());
    }

    #[test]
    fn styled_hot_cell_has_ansi_background() {
        let cell = OverlayCell {
            intensity: 1.0,
            text: "400".to_string(),
            is_hot: true,
        };
        let out = render_overlay_cell_styled('.', &cell);
        assert!(out.contains("\x1b[48;5;196m"));
        assert!(out.contains("400"));
        assert!(out.contains("\x1b[0m"));
    }

    #[test]
    fn plain_hot_cell_has_no_ansi_escapes() {
        let cell = OverlayCell {
            intensity: 1.0,
            text: "400".to_string(),
            is_hot: true,
        };
        let out = render_overlay_cell_plain('.', &cell);
        assert!(!out.contains('\x1b'));
        assert_eq!(out, "400");
    }
}
```

## `gomoku/crates/cli/src/tui/study.rs`

```rust
//! The study-table screen — the real table (chapter 05).
//!
//! Owns a `SessionCore`, reads commands, executes them, and repaints an
//! event-driven board with a side panel. Falls back to line-mode output
//! when stdout is not a terminal so the session can be scripted.

use std::io::{self, IsTerminal, Stdout, Write};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::Print,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use engine::{Board, Color, Move, Status};

use crate::command::{Command, Event};
use crate::session::{SessionCore, SessionInit};
use crate::tui::opening::OpeningState;
use crate::tui::overlay::PolicyOverlay;
use crate::tui::render::{overlay_verdict, render_study_with_overlays};

const HELP: &str = r#"Commands:
  ROW COL or play ROW COL   play a move
  step                      let the AI play
  back [N]                  go back N plies
  forward [N]               go forward N plies
  var N                     choose child variation N
  take black|white|both     make seat(s) human
  release black|white|both  release seat(s) to AI
  sims black|white N        set per-seat simulations
  p                         cycle policy overlay (off→visits→prior)
  v                         toggle value strip
  t                         TSS forced-win overlay
  new                       reset to opening
  h, help, ?                show this help
  q, quit, exit             leave
"#;

/// Run the study table starting from `init`.
///
/// `hint` is shown on the first frame (e.g. "Enter = step" for self-play).
pub fn run(init: SessionInit, hint: Option<&str>) -> io::Result<()> {
    if io::stdout().is_terminal() {
        run_alt(init, hint)
    } else {
        run_line(init, hint)
    }
}

/// Run the study table after the Swap2 opening ceremony.
///
/// The opening is driven interactively; the resulting board is passed to
/// `build_init`, which constructs the `SessionInit` for the study table.
pub fn run_with_swap2(
    build_init: impl FnOnce(Board) -> SessionInit,
    hint: Option<&str>,
) -> io::Result<()> {
    let board = run_swap2()?;
    run(build_init(board), hint)
}

// ------------------------------------------------------------------ alternate-screen loop

struct AlternateScreen;

impl AlternateScreen {
    fn enter() -> io::Result<AlternateScreen> {
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(AlternateScreen)
    }
}

impl Drop for AlternateScreen {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
    }
}

fn run_alt(init: SessionInit, hint: Option<&str>) -> io::Result<()> {
    let _screen = AlternateScreen::enter()?;
    let mut stdout = io::stdout();
    let mut core = SessionCore::new(init);
    let mut events: Vec<Event> = Vec::new();
    let mut message = hint
        .map(|h| h.to_string())
        .unwrap_or_else(|| "Study table — type 'h' for help".to_string());
    let mut shell = ShellState::default();

    loop {
        draw(&mut stdout, &core, &events, &message, &shell)?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break; // EOF
        }

        let trimmed = input.trim();
        if trimmed.is_empty() {
            if let Some(cmd) = empty_line_command(&core) {
                message = "thinking...".to_string();
                draw(&mut stdout, &core, &events, &message, &shell)?;
                stdout.flush()?;
                update(&mut core, cmd, &mut events, &mut message);
            }
            continue;
        }

        match parse_study_input(trimmed) {
            Parsed::Quit => break,
            Parsed::Help => message = HELP.to_string(),
            Parsed::Policy => {
                shell.policy_overlay = shell.policy_overlay.cycle();
                shell.tss_proof = None;
                if shell.policy_overlay != PolicyOverlay::Off && core.search_info().is_none() {
                    message = "no search here yet".to_string();
                } else {
                    message = format!("policy overlay: {}", shell.policy_overlay.label());
                }
            }
            Parsed::Value => {
                shell.show_value_strip = !shell.show_value_strip;
                message = if shell.show_value_strip {
                    "value strip on"
                } else {
                    "value strip off"
                }
                .to_string();
            }
            Parsed::Tss => message = run_tss(&core, &mut shell),
            Parsed::Command(cmd) => {
                update(&mut core, cmd, &mut events, &mut message);
            }
            Parsed::Error(err) => message = err,
        }
    }

    Ok(())
}

fn draw(
    stdout: &mut Stdout,
    core: &SessionCore,
    events: &[Event],
    message: &str,
    shell: &ShellState,
) -> io::Result<()> {
    queue!(stdout, MoveTo(0, 0), Hide, Clear(ClearType::All))?;
    for line in render_study_with_overlays(
        core,
        events,
        true,
        shell.policy_overlay,
        shell.tss_proof.as_ref(),
        shell.show_value_strip,
    ) {
        queue!(stdout, Print(line), Print("\n"))?;
    }
    queue!(stdout, MoveTo(0, 19), Print(status_line(core)))?;
    queue!(stdout, MoveTo(0, 20), Print(message))?;
    queue!(stdout, MoveTo(0, 21), Print("> "), Show)?;
    stdout.flush()
}

// ------------------------------------------------------------------ line-mode fallback

fn run_line(init: SessionInit, hint: Option<&str>) -> io::Result<()> {
    let mut core = SessionCore::new(init);
    let mut events: Vec<Event> = Vec::new();
    let mut message = hint
        .map(|h| h.to_string())
        .unwrap_or_else(|| "Study table — type 'h' for help".to_string());
    let mut shell = ShellState::default();
    println!("{message}");

    loop {
        for line in render_study_with_overlays(
            &core,
            &events,
            false,
            shell.policy_overlay,
            shell.tss_proof.as_ref(),
            shell.show_value_strip,
        ) {
            println!("{line}");
        }
        println!("{}", status_line(&core));
        if !message.is_empty() {
            println!("{message}");
        }
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break; // EOF
        }

        let trimmed = input.trim();
        if trimmed.is_empty() {
            if let Some(cmd) = empty_line_command(&core) {
                println!("thinking...");
                update(&mut core, cmd, &mut events, &mut message);
            }
            continue;
        }

        match parse_study_input(trimmed) {
            Parsed::Quit => break,
            Parsed::Help => message = HELP.to_string(),
            Parsed::Policy => {
                shell.policy_overlay = shell.policy_overlay.cycle();
                shell.tss_proof = None;
                if shell.policy_overlay != PolicyOverlay::Off && core.search_info().is_none() {
                    message = "no search here yet".to_string();
                } else {
                    message = format!("policy overlay: {}", shell.policy_overlay.label());
                }
            }
            Parsed::Value => {
                shell.show_value_strip = !shell.show_value_strip;
                message = if shell.show_value_strip {
                    "value strip on"
                } else {
                    "value strip off"
                }
                .to_string();
            }
            Parsed::Tss => message = run_tss(&core, &mut shell),
            Parsed::Command(cmd) => update(&mut core, cmd, &mut events, &mut message),
            Parsed::Error(err) => message = err,
        }
    }

    Ok(())
}

// ------------------------------------------------------------------ Swap2 opening ceremony

fn run_swap2() -> io::Result<Board> {
    if io::stdout().is_terminal() {
        run_swap2_alt()
    } else {
        run_swap2_line()
    }
}

fn run_swap2_alt() -> io::Result<Board> {
    let _screen = AlternateScreen::enter()?;
    let mut stdout = io::stdout();
    let mut opening = OpeningState::new();
    let mut message = "Swap2 opening — Player A places two Black and one White stone".to_string();

    loop {
        queue!(stdout, MoveTo(0, 0), Hide, Clear(ClearType::All))?;
        for line in crate::tui::render::render_opening_styled(&opening) {
            queue!(stdout, Print(line), Print("\n"))?;
        }
        queue!(stdout, MoveTo(0, 19), Print(opening_status(&opening)))?;
        queue!(stdout, MoveTo(0, 20), Print(&message))?;
        queue!(stdout, MoveTo(0, 21), Print("> "), Show)?;
        stdout.flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }

        match dispatch_opening(&mut opening, input.trim()) {
            OpeningCmd::Continue(msg) => message = msg,
            OpeningCmd::Finish(board) => return Ok(board),
            OpeningCmd::Quit => return Err(io::Error::other("Swap2 opening aborted")),
        }
    }

    Err(io::Error::other("Swap2 opening aborted"))
}

fn run_swap2_line() -> io::Result<Board> {
    let mut opening = OpeningState::new();
    println!("Swap2 opening — Player A places two Black and one White stone");

    loop {
        for line in crate::tui::render::render_opening_lines(&opening) {
            println!("{line}");
        }
        println!("{}", opening_status(&opening));
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }

        match dispatch_opening(&mut opening, input.trim()) {
            OpeningCmd::Continue(msg) => println!("{msg}"),
            OpeningCmd::Finish(board) => return Ok(board),
            OpeningCmd::Quit => return Err(io::Error::other("Swap2 opening aborted")),
        }
    }

    Err(io::Error::other("Swap2 opening aborted"))
}

enum OpeningCmd {
    Continue(String),
    Finish(Board),
    Quit,
}

fn dispatch_opening(opening: &mut OpeningState, input: &str) -> OpeningCmd {
    let text = input.trim().to_ascii_lowercase();
    match text.as_str() {
        "q" | "quit" | "exit" => OpeningCmd::Quit,
        "u" | "undo" => {
            if opening.is_at_start() {
                OpeningCmd::Continue("nothing to undo".to_string())
            } else {
                opening.undo();
                OpeningCmd::Continue(opening_status(opening))
            }
        }
        "d" | "done" => match opening.finish() {
            Ok(()) => OpeningCmd::Continue(opening_status(opening)),
            Err(e) => OpeningCmd::Continue(e),
        },
        "b" | "black" => match opening.clone().choose(Color::Black) {
            Some(outcome) => OpeningCmd::Finish(outcome.board),
            None => OpeningCmd::Continue("choose a color when prompted: 'b' or 'w'".to_string()),
        },
        "w" | "white" => match opening.clone().choose(Color::White) {
            Some(outcome) => OpeningCmd::Finish(outcome.board),
            None => OpeningCmd::Continue("choose a color when prompted: 'b' or 'w'".to_string()),
        },
        "a" | "add" => {
            let next = opening.clone().add_two_more();
            match next {
                Some(state) => {
                    *opening = state;
                    OpeningCmd::Continue(format!(
                        "{} adds two more stones — place one Black, then one White",
                        opening.active_party().label()
                    ))
                }
                None => OpeningCmd::Continue(
                    "'a' is only available when choosing after the first three stones".to_string(),
                ),
            }
        }
        _ => {
            let parts: Vec<&str> = text.split([',', ' ']).filter(|s| !s.is_empty()).collect();
            if parts.len() != 2 {
                return OpeningCmd::Continue(
                    "enter <row> <col> to place · 'd' done · 'b'/'w' choose · 'a' add two · 'u' undo · 'q' quit".to_string(),
                );
            }
            let row: u8 = match parts[0].parse() {
                Ok(v) => v,
                Err(_) => {
                    return OpeningCmd::Continue(format!("'{}' is not a row number", parts[0]));
                }
            };
            let col: u8 = match parts[1].parse() {
                Ok(v) => v,
                Err(_) => {
                    return OpeningCmd::Continue(format!("'{}' is not a column number", parts[1]));
                }
            };
            let mv = match Move::new(row, col) {
                Some(m) => m,
                None => {
                    return OpeningCmd::Continue(format!(
                        "({row}, {col}) is off the board — coordinates are 0-14"
                    ));
                }
            };
            if !opening.can_place() {
                return OpeningCmd::Continue(
                    "all stones for this phase are placed — type 'd' to continue or 'u' to undo"
                        .to_string(),
                );
            }
            match opening.place(mv) {
                Ok(placed) => OpeningCmd::Continue(format!(
                    "{} placed {placed:?} at ({}, {})",
                    opening.active_party().label(),
                    mv.row(),
                    mv.col()
                )),
                Err(e) => OpeningCmd::Continue(e),
            }
        }
    }
}

fn opening_status(opening: &OpeningState) -> String {
    use crate::tui::opening::OpeningPhase;
    let party = opening.active_party().label();
    match opening.phase() {
        OpeningPhase::Placing3 => {
            if opening.placements().len() == 3 {
                format!("{party}: 3 stones placed — [d] done · [u] undo")
            } else {
                let color = opening.current_color().unwrap();
                let remaining = opening.remaining(color);
                format!("{party}: place {color:?} ({remaining} remain) — row col · [u] undo")
            }
        }
        OpeningPhase::FirstChoice => {
            format!("{party}: [b] take Black · [w] take White · [a] add two stones · [u] undo")
        }
        OpeningPhase::Placing2 => {
            if opening.placements().len() == 5 {
                format!("{party}: 5 stones placed — [d] done · [u] undo")
            } else {
                let color = opening.current_color().unwrap();
                let remaining = opening.remaining(color);
                format!("{party}: place {color:?} ({remaining} remain) — row col · [u] undo")
            }
        }
        OpeningPhase::FinalChoice => {
            format!("{party}: [b] take Black · [w] take White · [u] undo")
        }
    }
}

// ------------------------------------------------------------------ shared helpers

#[derive(Debug, Default)]
struct ShellState {
    policy_overlay: PolicyOverlay,
    tss_proof: Option<engine::Proof>,
    show_value_strip: bool,
}

enum Parsed {
    Quit,
    Help,
    Command(Command),
    Policy,
    Value,
    Tss,
    Error(String),
}

fn parse_study_input(input: &str) -> Parsed {
    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("p") {
        return Parsed::Policy;
    }
    if trimmed.eq_ignore_ascii_case("v") {
        return Parsed::Value;
    }
    if trimmed.eq_ignore_ascii_case("t") {
        return Parsed::Tss;
    }
    match trimmed.eq_ignore_ascii_case("h")
        || trimmed.eq_ignore_ascii_case("help")
        || trimmed == "?"
    {
        true => Parsed::Help,
        false => match crate::command::parse(trimmed) {
            Ok(Command::Help) => Parsed::Help,
            Ok(Command::Quit) => Parsed::Quit,
            Ok(cmd) => Parsed::Command(cmd),
            Err(e) => Parsed::Error(e.to_string()),
        },
    }
}

fn run_tss(core: &SessionCore, shell: &mut ShellState) -> String {
    if shell.tss_proof.is_some() {
        shell.tss_proof = None;
        return "TSS overlay hidden".to_string();
    }

    let board = core.board();
    if board.status() != Status::Ongoing {
        return "TSS demo is only available during ongoing games".to_string();
    }

    let budget = engine::SearchBudget {
        max_nodes: 100_000,
        max_depth: 9,
    };
    let side = board.to_move();
    let Some(proof) = engine::prove_forced_win(&board, side, budget) else {
        return "no forced win proven within budget".to_string();
    };

    if !engine::verify_line(&board, &proof) {
        return "internal error: unverifiable proof".to_string();
    }

    let verdict = overlay_verdict(&proof);
    shell.tss_proof = Some(proof);
    shell.policy_overlay = PolicyOverlay::Off;
    verdict
}

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

fn update(core: &mut SessionCore, cmd: Command, events: &mut Vec<Event>, message: &mut String) {
    match core.execute(cmd) {
        Ok(new_events) => {
            events.extend(new_events.iter().cloned());
            if events.len() > 20 {
                events.drain(0..events.len() - 20);
            }
            *message = new_events.last().map(|e| e.to_string()).unwrap_or_default();
        }
        Err(e) => *message = e.to_string(),
    }
}

fn status_line(core: &SessionCore) -> String {
    let snap = core.snapshot();
    match snap.status {
        Status::Ongoing => format!(
            "ply {} · {:?} to move · status ongoing",
            snap.ply,
            core.board().to_move()
        ),
        other => format!("ply {} · {:?}", snap.ply, other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionInit;

    #[test]
    fn empty_line_step_only_when_ai_to_move() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        assert!(empty_line_command(&core).is_none());

        let ai_init = SessionInit {
            black: crate::actor::Occupant::Ai(crate::actor::AiConfig::default()),
            ..SessionInit::new_hotseat()
        };
        let core = SessionCore::new(ai_init);
        assert!(matches!(empty_line_command(&core), Some(Command::Step)));
    }
}
```

## `gomoku/crates/cli/src/tui/render.rs`

```rust
//! Board rendering — pure functions, no I/O.

use crate::actor::{Occupant, SearchInfo};
use crate::command::Event;
use crate::session::SessionCore;
use crate::tui::board::GameBoard;
use crate::tui::opening::OpeningState;
use crate::tui::overlay::{
    PolicyOverlay, overlay_cell, render_overlay_cell_plain, render_overlay_cell_styled, value_strip,
};
use crossterm::style::Stylize;
use engine::{Board, Color, Move, PlayError, Proof, Status};

fn glyph(stone: Option<Color>) -> char {
    match stone {
        None => '.',
        Some(Color::Black) => 'x',
        Some(Color::White) => 'o',
    }
}

/// Shared layout: row label, bar, 15 cells, bar, row label — inside a
/// ruler/border frame. `paint(glyph, is_last_move, move)` produces one
/// cell's visible characters; `cell_width` is used for the ruler and frame.
fn render_with(
    board: &dyn GameBoard,
    cell_width: usize,
    paint: impl Fn(char, bool, Move) -> String,
) -> Vec<String> {
    let ruler: String = (0..15).map(|c| format!("{c:^cell_width$}")).collect();
    let ruler = format!("    {ruler}    ");
    let border = format!("   +{}+   ", "-".repeat(cell_width * 15));
    let last = board.moves().last().copied();

    let mut lines = Vec::with_capacity(19);
    lines.push(ruler.clone());
    lines.push(border.clone());
    for row in 0..15u8 {
        let mut line = format!("{row:>2} |");
        for col in 0..15u8 {
            let mv = Move::new(row, col).expect("0..15 is always on board");
            line.push_str(&paint(glyph(board.stone_at(mv)), Some(mv) == last, mv));
        }
        line.push_str(&format!("| {row:<2}"));
        lines.push(line);
    }
    lines.push(border);
    lines.push(ruler);
    lines
}

/// Plain ASCII: tests, `--plain`, piping into a file.
pub fn render_lines(board: &dyn GameBoard) -> Vec<String> {
    render_with(board, 3, |g, is_last, _mv| {
        if is_last {
            format!("[{g}]")
        } else {
            format!(" {g} ")
        }
    })
}

/// ANSI-coloured: the alternate-screen UI.
pub fn render_styled(board: &dyn GameBoard) -> Vec<String> {
    render_with(board, 3, |g, is_last, _mv| {
        let styled = match g {
            '.' => ".".dark_grey(),
            'x' => "x".cyan().bold(),
            'o' => "o".yellow(),
            _ => unreachable!("glyph is one of . x o"),
        };
        if is_last {
            format!("[{styled}]")
        } else {
            format!(" {styled} ")
        }
    })
}

/// One line of context for below the board.
pub fn status_line(board: &dyn GameBoard) -> String {
    match board.status() {
        Status::Ongoing => format!(
            "engine: {} · move {} · {:?} to move",
            board.name(),
            board.moves().len(),
            board.to_move()
        ),
        Status::Won(color) => format!("{color:?} wins! — type 'new' to start over"),
        Status::Draw => "Draw! — type 'new' to start over".to_string(),
    }
}

// -----------------------------------------------------------------------------
// Swap2 opening rendering

/// Plain rendering of the Swap2 opening position.
pub fn render_opening_lines(state: &OpeningState) -> Vec<String> {
    render_opening(state, |glyph, is_last, _mv| {
        if is_last {
            format!("[{glyph}]")
        } else {
            format!(" {glyph} ")
        }
    })
}

/// Styled rendering of the Swap2 opening position.
pub fn render_opening_styled(state: &OpeningState) -> Vec<String> {
    render_opening(state, |glyph, is_last, _mv| {
        let styled = match glyph {
            '.' => ".".dark_grey(),
            'x' => "x".cyan().bold(),
            'o' => "o".yellow(),
            _ => unreachable!("glyph is one of . x o"),
        };
        if is_last {
            format!("[{styled}]")
        } else {
            format!(" {styled} ")
        }
    })
}

fn render_opening(state: &OpeningState, paint: impl Fn(char, bool, Move) -> String) -> Vec<String> {
    let board = TemporaryOpeningBoard::from(state);
    render_with(&board, 3, paint)
}

/// A temporary `GameBoard` view of an opening position, used only for
/// rendering.
struct TemporaryOpeningBoard {
    moves: Vec<Move>,
    colors: Vec<Color>,
}

impl TemporaryOpeningBoard {
    fn from(state: &OpeningState) -> Self {
        let mut moves = Vec::with_capacity(state.placements().len());
        let mut colors = Vec::with_capacity(state.placements().len());
        for &(mv, color) in state.placements() {
            moves.push(mv);
            colors.push(color);
        }
        Self { moves, colors }
    }
}

impl GameBoard for TemporaryOpeningBoard {
    fn play(&mut self, _mv: Move) -> Result<(), PlayError> {
        unimplemented!("TemporaryOpeningBoard is render-only")
    }

    fn undo(&mut self) {
        unimplemented!("TemporaryOpeningBoard is render-only")
    }

    fn status(&self) -> Status {
        Status::Ongoing
    }

    fn to_move(&self) -> Color {
        let black = self.colors.iter().filter(|&&c| c == Color::Black).count() as u8;
        let white = self.colors.len() as u8 - black;
        if white < black {
            Color::White
        } else {
            Color::Black
        }
    }

    fn stone_at(&self, mv: Move) -> Option<Color> {
        self.moves
            .iter()
            .zip(&self.colors)
            .find(|(m, _)| **m == mv)
            .map(|(_, &c)| c)
    }

    fn moves(&self) -> &[Move] {
        &self.moves
    }

    fn name(&self) -> &'static str {
        "opening"
    }
}

// -----------------------------------------------------------------------------
// TSS proof overlay

/// Plain rendering with a verified TSS line overlaid as numbered cells.
pub fn render_lines_with_overlay(board: &dyn GameBoard, proof: &Proof) -> Vec<String> {
    let overlay = Overlay::new(proof);
    render_with(board, 3, |glyph, is_last, mv| {
        if let Some((n, side)) = overlay.get(mv) {
            return overlay_cell_plain(n, side);
        }
        plain_cell(glyph, is_last)
    })
}

/// Styled rendering with a verified TSS line overlaid as numbered cells.
pub fn render_styled_with_overlay(board: &dyn GameBoard, proof: &Proof) -> Vec<String> {
    let overlay = Overlay::new(proof);
    render_with(board, 3, |glyph, is_last, mv| {
        if let Some((n, side)) = overlay.get(mv) {
            return overlay_cell_styled(n, side, proof.winner);
        }
        let styled = match glyph {
            '.' => ".".dark_grey(),
            'x' => "x".cyan().bold(),
            'o' => "o".yellow(),
            _ => unreachable!("glyph is one of . x o"),
        };
        styled_cell(&styled, is_last)
    })
}

fn plain_cell(glyph: char, is_last: bool) -> String {
    if is_last {
        format!("[{glyph}]")
    } else {
        format!(" {glyph} ")
    }
}

fn styled_cell(styled: &dyn std::fmt::Display, is_last: bool) -> String {
    if is_last {
        format!("[{styled}]")
    } else {
        format!(" {styled} ")
    }
}

struct Overlay {
    cells: [Option<(usize, Side)>; 225],
}

impl Overlay {
    fn new(proof: &Proof) -> Self {
        let mut cells = [None; 225];
        for (i, &mv) in proof.line.iter().enumerate() {
            let side = if i % 2 == 0 {
                Side::Attacker
            } else {
                Side::Defender
            };
            cells[mv.index()] = Some((i + 1, side));
        }
        Self { cells }
    }

    fn get(&self, mv: Move) -> Option<(usize, Side)> {
        self.cells[mv.index()]
    }
}

#[derive(Debug, Clone, Copy)]
enum Side {
    Attacker,
    Defender,
}

// Overlay numbers are single-digit because the CLI's TSS budget caps
// max_depth at 9 (ui.rs) — that is what lets the overlay keep the
// normal width-3 grid. Raise the depth cap and the width must follow.
fn overlay_cell_plain(n: usize, side: Side) -> String {
    debug_assert!(n < 10, "single-digit overlay numbers (max_depth = 9)");
    match side {
        Side::Attacker => format!(" {n} "),
        Side::Defender => format!("[{n}]"),
    }
}

fn overlay_cell_styled(n: usize, side: Side, winner: Color) -> String {
    let s = n.to_string();
    let color = match side {
        Side::Attacker => winner,
        Side::Defender => winner.other(),
    };
    let styled = match color {
        Color::Black => s.cyan().bold(),
        Color::White => s.yellow(),
    };
    debug_assert!(n < 10, "single-digit overlay numbers (max_depth = 9)");
    match side {
        Side::Attacker => format!(" {styled} "),
        Side::Defender => format!("[{styled}]"),
    }
}

/// Verdict text for a verified proof overlay.
pub fn overlay_verdict(proof: &Proof) -> String {
    format!(
        "certified forced win for {:?} in {} ply",
        proof.winner,
        proof.line.len()
    )
}

// ------------------------------------------------------------------
// Study-table rendering

/// Render the full study view: board with star points, last-move marker,
/// and a right-hand side panel.
pub fn render_study(core: &SessionCore, recent_events: &[Event], styled: bool) -> Vec<String> {
    render_study_with_overlays(core, recent_events, styled, PolicyOverlay::Off, None, false)
}

/// Render the study view with optional policy/TSS overlays and value strip.
pub fn render_study_with_overlays(
    core: &SessionCore,
    recent_events: &[Event],
    styled: bool,
    policy_overlay: PolicyOverlay,
    tss_proof: Option<&Proof>,
    show_value_strip: bool,
) -> Vec<String> {
    let board_lines = render_study_board_with_overlays(core, styled, policy_overlay, tss_proof);
    let panel_lines =
        render_side_panel_with_value_strip(core, recent_events, styled, show_value_strip);
    let max_rows = board_lines.len().max(panel_lines.len());
    let mut out = Vec::with_capacity(max_rows);
    for i in 0..max_rows {
        let left = board_lines.get(i).map(String::as_str).unwrap_or("");
        let right = panel_lines.get(i).map(String::as_str).unwrap_or("");
        out.push(format!("{left}  {right}"));
    }
    out
}

fn render_study_board_with_overlays(
    core: &SessionCore,
    styled: bool,
    policy_overlay: PolicyOverlay,
    tss_proof: Option<&Proof>,
) -> Vec<String> {
    let board = core.board();
    let last = core.snapshot().last_move;

    // TSS and policy overlays are mutually exclusive in the shell; if both
    // are somehow active, TSS wins (it is a deterministic proof overlay).
    if let Some(proof) = tss_proof {
        return study_board_lines_with_tss(&board, last, proof, styled);
    }

    if policy_overlay != PolicyOverlay::Off
        && let Some(info) = core.search_info()
    {
        return study_board_lines_with_policy(&board, last, info, policy_overlay, styled);
    }

    study_board_lines(&board, last, styled)
}

const HOSHI: [u8; 3] = [3, 7, 11];

fn study_board_lines(board: &Board, last: Option<Move>, styled: bool) -> Vec<String> {
    let ruler_plain = build_ruler();
    let border_plain = format!("   +{}+   ", "-".repeat(45));
    let mut lines = Vec::with_capacity(19);
    lines.push(style_gutter(&ruler_plain, styled));
    lines.push(style_gutter(&border_plain, styled));
    for row in 0..15u8 {
        let label = style_gutter(&format!("{row:>2}"), styled);
        let rlabel = style_gutter(&format!("{row:<2}"), styled);
        let mut line = format!("{label} |");
        for col in 0..15u8 {
            let mv = Move::new(row, col).expect("0..15 is always on board");
            line.push_str(&study_cell(board, mv, last, styled));
        }
        line.push_str(&format!("| {rlabel}"));
        lines.push(line);
    }
    lines.push(style_gutter(&border_plain, styled));
    lines.push(style_gutter(&ruler_plain, styled));
    lines
}

fn build_ruler() -> String {
    let mut s = String::with_capacity(53);
    s.push_str("    ");
    for c in 0..15u8 {
        s.push_str(&format!("{c:^3}"));
    }
    s.push_str("    ");
    s
}

fn style_gutter(text: &str, styled: bool) -> String {
    if styled {
        text.dark_grey().to_string()
    } else {
        text.to_string()
    }
}

fn study_cell(board: &Board, mv: Move, last: Option<Move>, styled: bool) -> String {
    let is_last = Some(mv) == last;
    match board.stone_at(mv) {
        Some(color) => {
            let s = if styled {
                match color {
                    Color::Black => "x".cyan().bold().to_string(),
                    Color::White => "o".yellow().to_string(),
                }
            } else {
                match color {
                    Color::Black => "x".to_string(),
                    Color::White => "o".to_string(),
                }
            };
            if is_last {
                format!("[{s}]")
            } else {
                format!(" {s} ")
            }
        }
        None => {
            let is_hoshi = HOSHI.contains(&mv.row()) && HOSHI.contains(&mv.col());
            let g = if is_hoshi { "+" } else { "." };
            let s = if styled {
                g.dark_grey().to_string()
            } else {
                g.to_string()
            };
            format!(" {s} ")
        }
    }
}

fn study_board_lines_with_policy(
    board: &Board,
    last: Option<Move>,
    info: &SearchInfo,
    mode: PolicyOverlay,
    styled: bool,
) -> Vec<String> {
    let ruler_plain = build_ruler();
    let border_plain = format!("   +{}+   ", "-".repeat(45));
    let mut lines = Vec::with_capacity(19);
    lines.push(style_gutter(&ruler_plain, styled));
    lines.push(style_gutter(&border_plain, styled));
    for row in 0..15u8 {
        let label = style_gutter(&format!("{row:>2}"), styled);
        let rlabel = style_gutter(&format!("{row:<2}"), styled);
        let mut line = format!("{label} |");
        for col in 0..15u8 {
            let mv = Move::new(row, col).expect("0..15 is always on board");
            line.push_str(&policy_cell(board, mv, last, info, mode, styled));
        }
        line.push_str(&format!("| {rlabel}"));
        lines.push(line);
    }
    lines.push(style_gutter(&border_plain, styled));
    lines.push(style_gutter(&ruler_plain, styled));
    lines
}

fn policy_cell(
    board: &Board,
    mv: Move,
    last: Option<Move>,
    info: &SearchInfo,
    mode: PolicyOverlay,
    styled: bool,
) -> String {
    // Stones are never over-painted by the policy heatmap.
    if let Some(color) = board.stone_at(mv) {
        return stone_cell(color, Some(mv) == last, styled);
    }

    let glyph = if HOSHI.contains(&mv.row()) && HOSHI.contains(&mv.col()) {
        '+'
    } else {
        '.'
    };

    match overlay_cell(info, mv, mode) {
        Some(cell) if styled => render_overlay_cell_styled(glyph, &cell),
        Some(cell) => render_overlay_cell_plain(glyph, &cell),
        None => format!(" {} ", glyph),
    }
}

fn stone_cell(color: Color, is_last: bool, styled: bool) -> String {
    let s = if styled {
        match color {
            Color::Black => "x".cyan().bold().to_string(),
            Color::White => "o".yellow().to_string(),
        }
    } else {
        match color {
            Color::Black => "x".to_string(),
            Color::White => "o".to_string(),
        }
    };
    if is_last {
        format!("[{s}]")
    } else {
        format!(" {s} ")
    }
}

fn study_board_lines_with_tss(
    board: &Board,
    last: Option<Move>,
    proof: &Proof,
    styled: bool,
) -> Vec<String> {
    let overlay = Overlay::new(proof);
    let ruler_plain = build_ruler();
    let border_plain = format!("   +{}+   ", "-".repeat(45));
    let mut lines = Vec::with_capacity(19);
    lines.push(style_gutter(&ruler_plain, styled));
    lines.push(style_gutter(&border_plain, styled));
    for row in 0..15u8 {
        let label = style_gutter(&format!("{row:>2}"), styled);
        let rlabel = style_gutter(&format!("{row:<2}"), styled);
        let mut line = format!("{label} |");
        for col in 0..15u8 {
            let mv = Move::new(row, col).expect("0..15 is always on board");
            line.push_str(&tss_study_cell(board, mv, last, &overlay, proof, styled));
        }
        line.push_str(&format!("| {rlabel}"));
        lines.push(line);
    }
    lines.push(style_gutter(&border_plain, styled));
    lines.push(style_gutter(&ruler_plain, styled));
    lines
}

fn tss_study_cell(
    board: &Board,
    mv: Move,
    last: Option<Move>,
    overlay: &Overlay,
    proof: &Proof,
    styled: bool,
) -> String {
    if let Some(color) = board.stone_at(mv) {
        return stone_cell(color, Some(mv) == last, styled);
    }
    if let Some((n, side)) = overlay.get(mv) {
        if styled {
            return overlay_cell_styled(n, side, proof.winner);
        }
        return overlay_cell_plain(n, side);
    }
    let is_hoshi = HOSHI.contains(&mv.row()) && HOSHI.contains(&mv.col());
    let g = if is_hoshi { "+" } else { "." };
    let s = if styled {
        g.dark_grey().to_string()
    } else {
        g.to_string()
    };
    format!(" {s} ")
}

fn render_side_panel_with_value_strip(
    core: &SessionCore,
    recent_events: &[Event],
    styled: bool,
    show_value_strip: bool,
) -> Vec<String> {
    let snap = core.snapshot();
    let mut lines = Vec::new();
    lines.push(panel_line("-- Study table --", styled));
    lines.push(format!("Black: {}", seat_label(core.black())));
    lines.push(format!("White: {}", seat_label(core.white())));
    lines.push(String::new());
    lines.push(format!("Ply: {}", snap.ply));
    lines.push(format!("Path: {}", path_tail_string(core)));
    if snap.branch_options.len() > 1 {
        let opts: Vec<String> = snap
            .branch_options
            .iter()
            .map(|(i, m)| format!("[{}] ({}, {})", i + 1, m.row(), m.col()))
            .collect();
        lines.push(format!(
            "{} variations: {}",
            snap.branch_options.len(),
            opts.join(" ")
        ));
    }

    if show_value_strip {
        lines.push(String::new());
        let side_at_search = core.board().to_move().other();
        let current_config = match core.occupant(side_at_search) {
            Occupant::Ai(cfg) => Some(cfg),
            Occupant::Human => None,
        };
        let strip = value_strip(core.search_info(), current_config, Some(side_at_search));
        if strip.is_empty() {
            lines.push("Value strip:".to_string());
            lines.push("  (no search here yet)".to_string());
        } else {
            lines.push("Value strip:".to_string());
            lines.extend(strip);
        }
    }

    lines.push(String::new());
    lines.push("Last events:".to_string());
    if recent_events.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for ev in recent_events.iter().rev().take(5).rev() {
            lines.push(format!("  {ev}"));
        }
    }
    lines.push(String::new());
    lines.push(panel_line("h help · q quit", styled));
    lines.push(panel_line("p policy overlay (ch.06)", styled));
    lines.push(panel_line("v value strip (ch.06)", styled));
    lines
}

fn panel_line(text: &str, styled: bool) -> String {
    if styled {
        text.dark_grey().to_string()
    } else {
        text.to_string()
    }
}

fn seat_label(occupant: &Occupant) -> String {
    match occupant {
        Occupant::Human => "Human".to_string(),
        Occupant::Ai(cfg) => format!("AI({})", cfg.simulations),
    }
}

fn path_tail_string(core: &SessionCore) -> String {
    let board = core.board();
    let moves = board.moves();
    if moves.is_empty() {
        return "(root)".to_string();
    }
    let tail: Vec<String> = moves
        .iter()
        .rev()
        .take(8)
        .rev()
        .map(|m| format!("({}, {})", m.row(), m.col()))
        .collect();
    if moves.len() > tail.len() {
        format!("... {}", tail.join(" "))
    } else {
        tail.join(" ")
    }
}

#[cfg(test)]
pub fn render_policy_board_for_test(
    board: &Board,
    info: &SearchInfo,
    mode: PolicyOverlay,
    styled: bool,
) -> Vec<String> {
    study_board_lines_with_policy(board, board.moves().last().copied(), info, mode, styled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::AiConfig;
    use crate::tui::board::{EngineKind, board_for};
    use crate::tui::opening::OpeningState;
    use crate::tui::overlay::PolicyOverlay;

    #[test]
    fn empty_board_is_ruled_and_framed() {
        let board = board_for(EngineKind::Naive);
        let lines = render_lines(&*board);

        assert_eq!(lines.len(), 19);
        assert!(lines.iter().all(|l| l.chars().count() == 53));

        let ruler = "     0  1  2  3  4  5  6  7  8  9 10 11 12 13 14     ";
        let border = "   +---------------------------------------------+   ";
        assert_eq!(lines[0], ruler);
        assert_eq!(lines[1], border);
        assert_eq!(lines[17], border);
        assert_eq!(lines[18], ruler);

        for (row, line) in lines[2..17].iter().enumerate() {
            assert_eq!(line, &format!("{row:>2} |{}| {row:<2}", " . ".repeat(15)));
        }
    }

    #[test]
    fn last_move_wears_brackets() {
        let mut board = board_for(EngineKind::Fast);
        board.play(Move::new(7, 7).unwrap()).unwrap(); // x — last move
        board.play(Move::new(7, 8).unwrap()).unwrap(); // o — now last

        let lines = render_lines(&*board);
        let row7 = &lines[2 + 7]; // line 0 ruler, line 1 border
        // 4-char gutter, then 3 bytes per column.
        assert_eq!(&row7[25..28], " x ");
        assert_eq!(&row7[28..31], "[o]");
    }

    #[test]
    fn brackets_follow_an_undo() {
        let mut board = board_for(EngineKind::Fast);
        board.play(Move::new(7, 7).unwrap()).unwrap();
        board.play(Move::new(7, 8).unwrap()).unwrap();
        board.undo();

        let lines = render_lines(&*board);
        let row7 = &lines[2 + 7];
        assert_eq!(&row7[25..28], "[x]"); // the bracket walks back
        assert_eq!(&row7[28..31], " . ");
    }

    #[test]
    fn status_line_reports_to_move_and_result() {
        let mut board = board_for(EngineKind::Naive);
        assert_eq!(
            status_line(&*board),
            "engine: naive · move 0 · Black to move"
        );

        // Black wins with five on row 0; White parks on row 1.
        for (br, bc, wr, wc) in [(0, 0, 1, 0), (0, 1, 1, 1), (0, 2, 1, 2), (0, 3, 1, 3)] {
            board.play(Move::new(br, bc).unwrap()).unwrap();
            board.play(Move::new(wr, wc).unwrap()).unwrap();
        }
        board.play(Move::new(0, 4).unwrap()).unwrap();

        assert_eq!(
            status_line(&*board),
            "Black wins! — type 'new' to start over"
        );
    }

    #[test]
    fn opening_render_shows_placed_colors() {
        let mut state = OpeningState::new();
        state.place(Move::new(7, 7).unwrap()).unwrap();
        state.place(Move::new(6, 6).unwrap()).unwrap();
        state.place(Move::new(5, 5).unwrap()).unwrap();

        let lines = render_opening_lines(&state);
        // The last placement is White at (5,5), so that cell wears brackets.
        let row7 = &lines[2 + 7];
        assert_eq!(&row7[25..28], " x ");
        let row6 = &lines[2 + 6];
        assert_eq!(&row6[22..25], " x ");
        let row5 = &lines[2 + 5];
        assert_eq!(&row5[19..22], "[o]");
    }

    #[test]
    fn tss_overlay_numbers_attack_and_defence() {
        let mut board = board_for(EngineKind::Fast);
        // Build a VCF win-in-3 from the engine TSS tests.
        let script = [
            (7, 4),
            (7, 3),
            (7, 5),
            (0, 0),
            (7, 6),
            (0, 2),
            (6, 6),
            (0, 4),
            (8, 8),
            (0, 6),
        ];
        for &(r, c) in &script {
            board.play(Move::new(r, c).unwrap()).unwrap();
        }

        let proof = engine::prove_forced_win(
            board.engine_board().unwrap(),
            board.to_move(),
            engine::SearchBudget {
                max_nodes: 200_000,
                max_depth: 16,
            },
        )
        .expect("test position has a known forced win");
        assert!(engine::verify_line(board.engine_board().unwrap(), &proof));

        let lines = render_lines_with_overlay(&*board, &proof);
        assert_eq!(lines.len(), 19);
        // Width-3 cells like normal play — the overlay causes no layout jump.
        assert!(lines.iter().all(|l| l.chars().count() == 4 + 45 + 4));

        // The first attacking move should be visible as a single-digit number.
        let mv1 = proof.line[0];
        let row = &lines[2 + mv1.row() as usize];
        let start = 4 + mv1.col() as usize * 3;
        let cell = &row[start..start + 3];
        assert!(
            cell.trim().contains('1'),
            "attacking cell should show 1, got {cell:?}"
        );
    }

    #[test]
    fn study_board_shows_star_points_on_empty_cells() {
        let core = SessionCore::new(crate::session::SessionInit::new_hotseat());
        let lines = render_study(&core, &[], false);
        // Row 7 (a hoshi row) should contain '+' star markers.
        let row7 = &lines[2 + 7];
        assert!(
            row7.matches(" + ").count() >= 3,
            "expected hoshi markers on row 7, got: {row7}"
        );
    }

    #[test]
    fn study_board_marks_last_move() {
        let mut core = SessionCore::new(crate::session::SessionInit::new_hotseat());
        core.execute(crate::command::Command::Play(Move::new(7, 7).unwrap()))
            .unwrap();
        let lines = render_study(&core, &[], false);
        let output = lines.join("\n");
        assert!(
            output.contains("[x]"),
            "last Black move should be bracketed"
        );
    }

    #[test]
    fn study_side_panel_shows_seats_and_ply() {
        let mut core = SessionCore::new(crate::session::SessionInit::new_hotseat());
        let lines = render_study(&core, &[], false);
        let output = lines.join("\n");
        assert!(output.contains("Black: Human"), "panel shows Black seat");
        assert!(output.contains("White: Human"), "panel shows White seat");
        assert!(output.contains("Ply: 0"), "panel shows initial ply");

        core.execute(crate::command::Command::Play(Move::new(7, 7).unwrap()))
            .unwrap();
        let lines = render_study(&core, &[], false);
        let output = lines.join("\n");
        assert!(output.contains("Ply: 1"), "panel updates ply");
        assert!(output.contains("(7, 7)"), "panel shows path tail");
    }

    #[test]
    fn study_side_panel_shows_branch_options() {
        let mut core = SessionCore::new(crate::session::SessionInit::new_hotseat());
        // Main line: (7,7) (6,7). Back one and branch to (7,8).
        core.execute(crate::command::Command::Play(Move::new(7, 7).unwrap()))
            .unwrap();
        core.execute(crate::command::Command::Play(Move::new(6, 7).unwrap()))
            .unwrap();
        core.execute(crate::command::Command::Back { n: 1 })
            .unwrap();
        core.execute(crate::command::Command::Play(Move::new(7, 8).unwrap()))
            .unwrap();
        // Move back to the branch point (the parent of the two children).
        core.execute(crate::command::Command::Back { n: 1 })
            .unwrap();

        let lines = render_study(&core, &[], false);
        let output = lines.join("\n");
        assert!(
            output.contains("variations:"),
            "panel should list branch options, got: {output}"
        );
        assert!(output.contains("[1] (6, 7)"), "panel shows first branch");
        assert!(output.contains("[2] (7, 8)"), "panel shows second branch");
    }

    #[test]
    fn policy_overlay_snapshot_with_fake_search_info() {
        // Construct SearchInfo directly — no real MCTS search needed.
        let mut board = Board::new();
        board.play(Move::new(7, 7).unwrap()).unwrap();
        board.play(Move::new(6, 7).unwrap()).unwrap();

        let info = crate::actor::SearchInfo {
            visits: vec![
                (Move::new(7, 8).unwrap(), 320),
                (Move::new(8, 7).unwrap(), 80),
            ],
            prior: vec![
                (Move::new(7, 8).unwrap(), 0.6),
                (Move::new(8, 7).unwrap(), 0.2),
            ],
            value: 0.35,
            config: AiConfig::default(),
        };

        let lines = render_policy_board_for_test(&board, &info, PolicyOverlay::Visits, false);
        let row7 = &lines[2 + 7];

        // Hot cell (7,8) shows raw visit count 320; an unvisited cell stays dim.
        let col8_start = 4 + 8 * 3;
        assert_eq!(&row7[col8_start..col8_start + 3], "320");
        let row8 = &lines[2 + 8];
        assert_eq!(&row8[col8_start..col8_start + 3], " . ");
    }
}
```
