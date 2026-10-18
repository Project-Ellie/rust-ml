> **Opt-in solution — read only after you have tried the chapter, or if you have been stuck for more than twenty minutes.**
>
> This file quotes the reference implementation files for chapter 05,
> verified green at commit `8ad0061` in the reference worktree.

Open this only if you have been stuck for more than twenty minutes,
or after you have finished the chapter and want to compare your work
against the verified reference. The files below are quoted verbatim
from the reference worktree.

Files changed in this chapter:

* `gomoku/crates/cli/src/main.rs`
* `gomoku/crates/cli/src/tui/study.rs`
* `gomoku/crates/cli/src/tui/render.rs`
* `gomoku/crates/cli/src/tui/mod.rs`
* `gomoku/crates/cli/src/session.rs`

Files unchanged from chapter 04 include:
`src/tui/board.rs`, `src/tui/input.rs`, `src/tui/opening.rs`,
`src/tui/puzzle.rs`, `src/tui/ui.rs`, `src/lib.rs`, `src/command.rs`,
`src/actor.rs`, `src/history.rs`, `src/fmt.rs`, the demo binaries in
`src/bin/`, and the integration tests in `tests/`.

# `gomoku/crates/cli/src/main.rs`

```rust
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use engine::{Board, Color};

use cli::actor::{AiConfig, EvaluatorKind, Occupant};
use cli::session::SessionInit;
use cli::tui::board::{EngineKind, board_for};
use cli::tui::puzzle;
use cli::tui::study;
use cli::tui::ui;

const PLAY_USAGE: &str = if cfg!(feature = "naive-engine") {
    "usage: gomoku play [--engine naive|fast] [--plain] [--no-swap2] [--puzzle <path> [--index N]]\n\nSwap2 opening is only available with the fast engine; use --no-swap2 to start from an empty board with any engine. Puzzle mode skips the opening entirely."
} else {
    "usage: gomoku play [--engine fast] [--plain] [--no-swap2] [--puzzle <path> [--index N]]\n\nThis build was compiled without the naive-engine feature; only the fast engine is available. Puzzle mode skips the opening entirely."
};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "gomoku",
    about = "Terminal Gomoku client",
    subcommand_required = false,
    arg_required_else_help = false
)]
struct CliArgs {
    /// Legacy `play` flags used when no subcommand is given.
    #[command(flatten)]
    play: PlayArgs,

    #[command(subcommand)]
    cmd: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Play a human game or puzzle (legacy default).
    Play(PlayArgs),
    /// AI-vs-AI self-play.
    Selfplay(SearchArgs),
}

#[derive(Args, Debug, Clone)]
struct PlayArgs {
    #[command(flatten)]
    search: SearchArgs,

    /// Engine board to use.
    #[arg(long, value_name = "ENGINE")]
    engine: Option<String>,

    /// Use the scrolling plain-text UI.
    #[arg(long)]
    plain: bool,

    /// Skip the Swap2 opening and start from an empty board.
    #[arg(long)]
    no_swap2: bool,

    /// Load a puzzle file instead of playing a game.
    #[arg(long, value_name = "PATH")]
    puzzle: Option<PathBuf>,

    /// Puzzle index to start from.
    #[arg(long, value_name = "N")]
    index: Option<usize>,
}

#[derive(Args, Debug, Clone)]
struct SearchArgs {
    /// AI occupant kind (currently only `tactics`).
    #[arg(long, value_name = "AI")]
    ai: Option<AiKind>,

    /// Human seat color when `--ai` is used.
    #[arg(long, value_name = "COLOR")]
    color: Option<PlayerColor>,

    /// Number of MCTS simulations per move (default 400).
    #[arg(long, value_name = "N", default_value_t = 400)]
    sims: u32,

    /// PUCT exploration constant (default 1.5).
    #[arg(long, value_name = "F", default_value_t = 1.5)]
    c_puct: f32,

    /// Move-selection temperature (default 0.0).
    #[arg(long, value_name = "F")]
    temperature: Option<f32>,

    /// Dirichlet noise weight (default 0.0).
    #[arg(long, value_name = "F")]
    noise: Option<f32>,

    /// Override simulation count for Black.
    #[arg(long, value_name = "N")]
    black_sims: Option<u32>,

    /// Override simulation count for White.
    #[arg(long, value_name = "N")]
    white_sims: Option<u32>,

    /// Random seed for reproducible sessions.
    #[arg(long, value_name = "N")]
    seed: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
enum AiKind {
    Tactics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
enum PlayerColor {
    Black,
    White,
}

fn main() {
    let args = CliArgs::parse();

    let (mode, play_args, selfplay_args) = match args.cmd {
        None => (Mode::Play, Some(args.play), None),
        Some(Commands::Play(p)) => (Mode::Play, Some(p), None),
        Some(Commands::Selfplay(s)) => {
            // Reject legacy `play` flags placed before the subcommand.
            if args.play.engine.is_some()
                || args.play.plain
                || args.play.no_swap2
                || args.play.puzzle.is_some()
                || args.play.index.is_some()
            {
                eprintln!(
                    "error: --engine, --plain, --no-swap2, --puzzle, and --index are only valid with the 'play' subcommand"
                );
                eprintln!("{PLAY_USAGE}");
                std::process::exit(2);
            }
            (Mode::Selfplay, None, Some(s))
        }
    };

    let result = match mode {
        Mode::Play => {
            let play = play_args.expect("play mode has play args");
            run_play(&play)
        }
        Mode::Selfplay => {
            let search = selfplay_args.expect("selfplay mode has search args");
            run_selfplay(&search)
        }
    };

    if let Err(e) = result {
        eprintln!("terminal error: {e}");
        std::process::exit(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Play,
    Selfplay,
}

fn run_play(play: &PlayArgs) -> std::io::Result<()> {
    let kind = parse_engine(play.engine.as_deref())?;
    let plain = play.plain;
    let swap2 = !play.no_swap2;

    // Legacy puzzle mode is a separate flow, not part of the study table.
    if let Some(puzzle_path) = &play.puzzle {
        let index = play.index.unwrap_or(0);
        return run_puzzle_mode(puzzle_path.clone(), index, plain);
    }

    validate_search_args(&play.search, kind, false)?;

    #[cfg(feature = "naive-engine")]
    if swap2 && kind == EngineKind::Naive {
        eprintln!("error: the Swap2 opening requires the fast engine");
        eprintln!("       use --no-swap2 to start from an empty board with the naive engine");
        std::process::exit(2);
    }

    // Legacy paths: --plain and the naive engine keep the old hotseat UI.
    if plain || kind == EngineKind::Naive {
        if play.search.ai.is_some() {
            eprintln!("error: --ai is not available in --plain or --engine naive mode");
            eprintln!("{PLAY_USAGE}");
            std::process::exit(2);
        }
        let board = board_for(kind);
        return if plain {
            ui::run_plain(board, kind, swap2)
        } else {
            ui::run_ui(board, kind, swap2)
        };
    }

    // Study-table path: the modern front-end. Swap2 runs first unless
    // disabled, then the resulting board seeds the session.
    let make_init = |board: Board| build_play_init(&play.search, board);
    if swap2 {
        study::run_with_swap2(make_init, None)
    } else {
        study::run(make_init(Board::new()), None)
    }
}

fn run_selfplay(search: &SearchArgs) -> std::io::Result<()> {
    validate_search_args(search, EngineKind::Fast, true)?;

    let (config, black_sims, white_sims) = ai_config_from(search);
    let init = SessionInit {
        black: Occupant::Ai(config.clone()),
        white: Occupant::Ai(config.clone()),
        default_config: config,
        black_sims,
        white_sims,
        seed: search.seed,
        opening: Board::new(),
    };

    study::run(init, Some("Enter = step"))
}

fn run_puzzle_mode(path: PathBuf, index: usize, plain: bool) -> std::io::Result<()> {
    let puzzles = puzzle::load(&path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    if puzzles.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "puzzle file contains no puzzles",
        ));
    }
    if index >= puzzles.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("--index {index} is out of range (0..{})", puzzles.len()),
        ));
    }
    if plain {
        ui::run_plain_puzzle(puzzles, index)
    } else {
        ui::run_ui_puzzle(puzzles, index)
    }
}

fn parse_engine(value: Option<&str>) -> std::io::Result<EngineKind> {
    match value {
        None => Ok(EngineKind::Fast),
        Some("fast") => Ok(EngineKind::Fast),
        #[cfg(feature = "naive-engine")]
        Some("naive") => Ok(EngineKind::Naive),
        #[cfg(not(feature = "naive-engine"))]
        Some("naive") => {
            eprintln!("error: built without the naive-engine feature");
            eprintln!("{PLAY_USAGE}");
            std::process::exit(2);
        }
        Some(other) => {
            eprintln!("error: unknown engine '{other}'");
            eprintln!("{PLAY_USAGE}");
            std::process::exit(2);
        }
    }
}

fn validate_search_args(
    search: &SearchArgs,
    kind: EngineKind,
    is_selfplay: bool,
) -> std::io::Result<()> {
    if kind == EngineKind::Naive && (search.ai.is_some() || is_selfplay) {
        eprintln!("error: --engine naive cannot be used with --ai or selfplay");
        eprintln!("{PLAY_USAGE}");
        std::process::exit(2);
    }

    if !is_selfplay && search.color.is_some() && search.ai.is_none() {
        eprintln!("error: --color requires --ai");
        eprintln!("{PLAY_USAGE}");
        std::process::exit(2);
    }

    if is_selfplay && search.color.is_some() {
        eprintln!("error: --color is not valid for selfplay");
        eprintln!("{PLAY_USAGE}");
        std::process::exit(2);
    }

    Ok(())
}

fn ai_config_from(search: &SearchArgs) -> (AiConfig, Option<u32>, Option<u32>) {
    let temperature = search.temperature.unwrap_or(0.0);
    let noise = search.noise.unwrap_or(0.0);
    let explicit_selection = search.temperature.is_some() || search.noise.is_some();

    let config = AiConfig {
        evaluator: EvaluatorKind::Tactics,
        simulations: search.sims,
        c_puct: search.c_puct,
        temperature,
        noise_epsilon: noise,
        explicit_selection,
    };

    (config, search.black_sims, search.white_sims)
}

fn build_play_init(search: &SearchArgs, opening: Board) -> SessionInit {
    let (config, black_sims, white_sims) = ai_config_from(search);

    if let Some(AiKind::Tactics) = search.ai {
        let human_color = search
            .color
            .map(player_color_to_engine)
            .unwrap_or(Color::Black);
        let (black, white) = match human_color {
            Color::Black => (Occupant::Human, Occupant::Ai(config.clone())),
            Color::White => (Occupant::Ai(config.clone()), Occupant::Human),
        };
        SessionInit {
            black,
            white,
            default_config: config,
            black_sims,
            white_sims,
            seed: search.seed,
            opening,
        }
    } else {
        SessionInit {
            black: Occupant::Human,
            white: Occupant::Human,
            default_config: config,
            black_sims,
            white_sims,
            seed: search.seed,
            opening,
        }
    }
}

fn player_color_to_engine(color: PlayerColor) -> Color {
    match color {
        PlayerColor::Black => Color::Black,
        PlayerColor::White => Color::White,
    }
}
```

# `gomoku/crates/cli/src/tui/study.rs`

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
use crate::tui::render::render_study;

const HELP: &str = r#"Commands:
  SQUARE or play SQUARE     play a move
  step                      let the AI play
  back [N]                  go back N plies
  forward [N]               go forward N plies
  var N                     choose child variation N
  take black|white|both     make seat(s) human
  release black|white|both  release seat(s) to AI
  sims black|white N        set per-seat simulations
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

    loop {
        draw(&mut stdout, &core, &events, &message)?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break; // EOF
        }

        let trimmed = input.trim();
        if trimmed.is_empty() {
            if let Some(cmd) = empty_line_command(&core) {
                message = "thinking...".to_string();
                draw(&mut stdout, &core, &events, &message)?;
                stdout.flush()?;
                update(&mut core, cmd, &mut events, &mut message);
            }
            continue;
        }

        match parse_study_input(trimmed) {
            Parsed::Quit => break,
            Parsed::Help => message = HELP.to_string(),
            Parsed::Command(cmd) => update(&mut core, cmd, &mut events, &mut message),
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
) -> io::Result<()> {
    queue!(stdout, MoveTo(0, 0), Hide, Clear(ClearType::All))?;
    for line in render_study(core, events, true) {
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
    println!("{message}");

    loop {
        for line in render_study(&core, &events, false) {
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
            Parsed::Command(cmd) => update(&mut core, cmd, &mut events, &mut message),
            Parsed::Error(err) => message = err,
        }
    }

    Ok(())
}

// ------------------------------------------------------------------ Swap2 opening ceremony

fn parse_algebraic(token: &str) -> Result<(u8, u8), String> {
    let text = token.trim().to_ascii_lowercase();
    if text.len() < 2 {
        return Err(format!(
            "expected algebraic square like 'h8', got '{}'",
            token
        ));
    }
    let col_letter = text.chars().next().unwrap();
    if !('a'..='o').contains(&col_letter) {
        return Err(format!(
            "expected column letter a-o, got '{}' in '{}'",
            col_letter, token
        ));
    }
    let row_digits = &text[1..];
    let row = row_digits.parse::<u8>().map_err(|_| {
        format!(
            "expected row number 1-15, got '{}' in '{}'",
            row_digits, token
        )
    })?;
    let col = col_letter as u8 - b'a';
    if !(1..=15).contains(&row) || col > 14 {
        return Err(format!("{} is off the board — squares are a1-o15", token));
    }
    Ok((row - 1, col))
}

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
                    "enter <square> to place · 'd' done · 'b'/'w' choose · 'a' add two · 'u' undo · 'q' quit".to_string(),
                );
            }
            let token = parts[0];
            let (row, col) = match parse_algebraic(token) {
                Ok(v) => v,
                Err(e) => return OpeningCmd::Continue(e),
            };
            let mv = match Move::new(row, col) {
                Some(m) => m,
                None => {
                    return OpeningCmd::Continue(format!(
                        "{} is off the board — squares are a1-o15",
                        token
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
                format!("{party}: place {color:?} ({remaining} remain) — square · [u] undo")
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
                format!("{party}: place {color:?} ({remaining} remain) — square · [u] undo")
            }
        }
        OpeningPhase::FinalChoice => {
            format!("{party}: [b] take Black · [w] take White · [u] undo")
        }
    }
}

// ------------------------------------------------------------------ shared helpers

enum Parsed {
    Quit,
    Help,
    Command(Command),
    Error(String),
}

fn parse_study_input(input: &str) -> Parsed {
    match input.eq_ignore_ascii_case("h") || input.eq_ignore_ascii_case("help") || input == "?" {
        true => Parsed::Help,
        false => match crate::command::parse(input) {
            Ok(Command::Help) => Parsed::Help,
            Ok(Command::Quit) => Parsed::Quit,
            Ok(cmd) => Parsed::Command(cmd),
            Err(e) => Parsed::Error(e.to_string()),
        },
    }
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

# `gomoku/crates/cli/src/tui/render.rs`

```rust
//! Board rendering — pure functions, no I/O.

use crate::actor::Occupant;
use crate::command::Event;
use crate::session::SessionCore;
use crate::tui::board::GameBoard;
use crate::tui::opening::OpeningState;
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
    let ruler: String = (0..15u8)
        .map(|c| format!("{:^cell_width$}", (b'a' + c) as char))
        .collect();
    let ruler = format!("    {ruler}    ");
    let border = format!("   +{}+   ", "-".repeat(cell_width * 15));
    let last = board.moves().last().copied();

    let mut lines = Vec::with_capacity(19);
    lines.push(ruler.clone());
    lines.push(border.clone());
    for row in 0..15u8 {
        let mut line = format!("{:>2} |", row + 1);
        for col in 0..15u8 {
            let mv = Move::new(row, col).expect("0..15 is always on board");
            line.push_str(&paint(glyph(board.stone_at(mv)), Some(mv) == last, mv));
        }
        line.push_str(&format!("| {:<2}", row + 1));
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
    let board_lines = render_study_board(core, styled);
    let panel_lines = render_side_panel(core, recent_events, styled);
    let max_rows = board_lines.len().max(panel_lines.len());
    let mut out = Vec::with_capacity(max_rows);
    for i in 0..max_rows {
        let left = board_lines.get(i).map(String::as_str).unwrap_or("");
        let right = panel_lines.get(i).map(String::as_str).unwrap_or("");
        out.push(format!("{left}  {right}"));
    }
    out
}

fn render_study_board(core: &SessionCore, styled: bool) -> Vec<String> {
    study_board_lines(&core.board(), core.snapshot().last_move, styled)
}

const HOSHI: [u8; 3] = [3, 7, 11];

fn study_board_lines(board: &Board, last: Option<Move>, styled: bool) -> Vec<String> {
    let ruler_plain = build_ruler();
    let border_plain = format!("   +{}+   ", "-".repeat(45));
    let mut lines = Vec::with_capacity(19);
    lines.push(style_gutter(&ruler_plain, styled));
    lines.push(style_gutter(&border_plain, styled));
    for row in 0..15u8 {
        let label = style_gutter(&format!("{:>2}", row + 1), styled);
        let rlabel = style_gutter(&format!("{:<2}", row + 1), styled);
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
        s.push_str(&format!("{:^3}", (b'a' + c) as char));
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

fn render_side_panel(core: &SessionCore, recent_events: &[Event], styled: bool) -> Vec<String> {
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
            .map(|(i, m)| format!("[{}] {}", i + 1, crate::fmt::format_move(*m)))
            .collect();
        lines.push(format!(
            "{} variations: {}",
            snap.branch_options.len(),
            opts.join(" ")
        ));
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
        .map(|m| crate::fmt::format_move(*m))
        .collect();
    if moves.len() > tail.len() {
        format!("... {}", tail.join(" "))
    } else {
        tail.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::board::{EngineKind, board_for};
    use crate::tui::opening::OpeningState;

    #[test]
    fn empty_board_is_ruled_and_framed() {
        let board = board_for(EngineKind::Naive);
        let lines = render_lines(&*board);

        assert_eq!(lines.len(), 19);
        assert!(lines.iter().all(|l| l.chars().count() == 53));

        let ruler = "     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o     ";
        let border = "   +---------------------------------------------+   ";
        assert_eq!(lines[0], ruler);
        assert_eq!(lines[1], border);
        assert_eq!(lines[17], border);
        assert_eq!(lines[18], ruler);

        for (row, line) in lines[2..17].iter().enumerate() {
            assert_eq!(
                line,
                &format!("{:>2} |{}| {:<2}", row + 1, " . ".repeat(15), row + 1)
            );
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
        assert!(output.contains("h8"), "panel shows path tail");
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
        assert!(output.contains("[1] h7"), "panel shows first branch");
        assert!(output.contains("[2] i8"), "panel shows second branch");
    }
}
```

# `gomoku/crates/cli/src/tui/mod.rs`

```rust
//! Existing terminal UI modules.
//!
//! Moved under `tui/` during the lib/bin split; their behavior is
//! unchanged.

pub mod board;
pub mod input;
pub mod opening;
pub mod puzzle;
pub mod render;
pub mod study;
pub mod ui;
```

# `gomoku/crates/cli/src/session.rs`

```rust
//! Session core — headless state machine.
//!
//! `SessionCore` is the heart of the study table. It owns the variation
//! tree, the seat table, and AI search defaults. Commands go in; events
//! and snapshots come out. No terminal code, no I/O.

use std::collections::HashMap;

use engine::{Board, Color, Move, Status};
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::actor::{AiConfig, EvaluatorKind, Occupant, OccupantKind, SearchInfo};
use crate::command::{Command, CommandError, Event, SeatSel};
use crate::history::NodeId as HistoryNodeId;
use crate::history::{ForwardOutcome, History, PlayOutcome};

/// Initial state used to construct a `SessionCore`.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionInit {
    /// Black seat occupant.
    pub black: Occupant,
    /// White seat occupant.
    pub white: Occupant,
    /// Default AI configuration applied when releasing a seat.
    pub default_config: AiConfig,
    /// Optional per-seat simulation override for Black.
    pub black_sims: Option<u32>,
    /// Optional per-seat simulation override for White.
    pub white_sims: Option<u32>,
    /// Optional RNG seed for reproducible sessions.
    pub seed: Option<u64>,
    /// Opening board used as the root of the variation tree.
    pub opening: Board,
}

impl SessionInit {
    /// Start a hotseat session on an empty board.
    #[must_use]
    pub fn new_hotseat() -> Self {
        Self::hotseat()
    }

    /// Start a hotseat session from a custom opening position.
    #[must_use]
    pub fn new_hotseat_from_opening(opening: &Board) -> Self {
        Self::hotseat_from_opening(opening)
    }

    /// Human vs human on an empty board.
    #[must_use]
    pub fn hotseat() -> Self {
        Self {
            black: Occupant::Human,
            white: Occupant::Human,
            default_config: AiConfig::default(),
            black_sims: None,
            white_sims: None,
            seed: None,
            opening: Board::new(),
        }
    }

    /// Human vs human from a custom opening position.
    #[must_use]
    pub fn hotseat_from_opening(opening: &Board) -> Self {
        Self {
            black: Occupant::Human,
            white: Occupant::Human,
            default_config: AiConfig::default(),
            black_sims: None,
            white_sims: None,
            seed: None,
            opening: opening.clone(),
        }
    }

    /// One human seat and one AI seat on an empty board.
    ///
    /// `human_color` is the human seat; the opposite seat is AI using
    /// `ai_config`.
    #[must_use]
    pub fn versus_ai(human_color: Color, ai_config: AiConfig) -> Self {
        let (black, white) = match human_color {
            Color::Black => (Occupant::Human, Occupant::Ai(ai_config.clone())),
            Color::White => (Occupant::Ai(ai_config.clone()), Occupant::Human),
        };
        Self {
            black,
            white,
            default_config: ai_config,
            black_sims: None,
            white_sims: None,
            seed: None,
            opening: Board::new(),
        }
    }

    /// AI vs AI on an empty board.
    ///
    /// `config` configures both seats. `seed` makes the session reproducible.
    #[must_use]
    pub fn ai_vs_ai(config: AiConfig, seed: u64) -> Self {
        Self {
            black: Occupant::Ai(config.clone()),
            white: Occupant::Ai(config.clone()),
            default_config: config,
            black_sims: None,
            white_sims: None,
            seed: Some(seed),
            opening: Board::new(),
        }
    }
}

/// Read-only snapshot of a session, suitable for rendering or JSON export.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// Cursor ply in the variation tree.
    pub ply: u32,
    /// Branch options at the cursor, if any.
    pub branch_options: Vec<(usize, Move)>,
    /// Black seat occupant kind.
    pub black: OccupantKind,
    /// White seat occupant kind.
    pub white: OccupantKind,
    /// Game status at the cursor.
    pub status: Status,
    /// Last move on the cursor path, if any.
    pub last_move: Option<Move>,
}

impl Serialize for Snapshot {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Snapshot", 6)?;
        state.serialize_field("ply", &self.ply)?;
        state.serialize_field("branch_options", &self.branch_options)?;
        state.serialize_field("black", &self.black)?;
        state.serialize_field("white", &self.white)?;
        state.serialize_field("status", &status_as_str(self.status))?;
        state.serialize_field("last_move", &self.last_move)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Snapshot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct SnapshotHelper {
            ply: u32,
            branch_options: Vec<(usize, Move)>,
            black: OccupantKind,
            white: OccupantKind,
            status: String,
            last_move: Option<Move>,
        }

        let helper = SnapshotHelper::deserialize(deserializer)?;
        Ok(Snapshot {
            ply: helper.ply,
            branch_options: helper.branch_options,
            black: helper.black,
            white: helper.white,
            status: status_from_str(&helper.status).map_err(serde::de::Error::custom)?,
            last_move: helper.last_move,
        })
    }
}

fn status_as_str(status: Status) -> &'static str {
    match status {
        Status::Ongoing => "ongoing",
        Status::Won(Color::Black) => "won_black",
        Status::Won(Color::White) => "won_white",
        Status::Draw => "draw",
    }
}

fn status_from_str(s: &str) -> Result<Status, String> {
    match s {
        "ongoing" => Ok(Status::Ongoing),
        "won_black" => Ok(Status::Won(Color::Black)),
        "won_white" => Ok(Status::Won(Color::White)),
        "draw" => Ok(Status::Draw),
        _ => Err(format!("unknown status string: {s}")),
    }
}

/// The headless session state machine.
pub struct SessionCore {
    history: History,
    black: Occupant,
    white: Occupant,
    default_config: AiConfig,
    black_sims: Option<u32>,
    white_sims: Option<u32>,
    rng: StdRng,
    init: SessionInit,
    /// Search info stored per history node, populated when an AI search
    /// produced the move leading to that node.
    search_info: HashMap<HistoryNodeId, SearchInfo>,
}

impl SessionCore {
    /// Create a new session from the given initial state.
    #[must_use]
    pub fn new(init: SessionInit) -> Self {
        let seed = init.seed.unwrap_or_else(rand::random::<u64>);
        let rng = StdRng::seed_from_u64(seed);
        Self {
            history: History::from_opening(&init.opening),
            black: init.black.clone(),
            white: init.white.clone(),
            default_config: init.default_config.clone(),
            black_sims: init.black_sims,
            white_sims: init.white_sims,
            rng,
            init,
            search_info: HashMap::new(),
        }
    }

    /// Execute a single command, returning a vector of events describing
    /// what changed.
    ///
    /// # Errors
    /// Returns a typed `CommandError` for illegal moves, invalid arguments,
    /// or AI-driven commands that are not yet implemented.
    pub fn execute(&mut self, cmd: Command) -> Result<Vec<Event>, CommandError> {
        match cmd {
            Command::Play(mv) => self.execute_play(mv),
            Command::Step => self.execute_step(),
            Command::Back { n } => {
                let _moved = self.history.back(n);
                Ok(vec![Event::CursorMoved {
                    ply: self.history.ply(),
                }])
            }
            Command::Forward { n } => self.execute_forward(n),
            Command::Var { index } => self.execute_var(index),
            Command::Take { seat } => self.execute_take(seat),
            Command::Release { seat } => self.execute_release(seat),
            Command::SetSims { seat, n } => self.execute_set_sims(seat, n),
            Command::New => {
                self.reset();
                Ok(vec![Event::Reset])
            }
            Command::Help | Command::Quit => Ok(vec![]),
        }
    }

    /// A serializable read-model snapshot of the current session.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let cursor = self.history.cursor();
        let children = self.history.children(cursor);
        let branch_options = children.into_iter().enumerate().collect();
        Snapshot {
            ply: self.history.ply(),
            branch_options,
            black: self.black.kind(),
            white: self.white.kind(),
            status: self.history.board().status(),
            last_move: self.history.board().moves().last().copied(),
        }
    }

    /// The board at the current cursor, replayed from the root.
    #[must_use]
    pub fn board(&self) -> Board {
        self.history.board()
    }

    /// The `SearchInfo` stored on the cursor node, if any.
    ///
    /// A node only has search info when an AI `Step` produced the move
    /// leading to it.
    #[must_use]
    pub fn search_info(&self) -> Option<&SearchInfo> {
        self.search_info.get(&self.history.cursor())
    }

    /// The occupant of the Black seat.
    #[must_use]
    pub fn black(&self) -> &Occupant {
        &self.black
    }

    /// The occupant of the White seat.
    #[must_use]
    pub fn white(&self) -> &Occupant {
        &self.white
    }

    /// The default AI configuration used when releasing a seat.
    #[must_use]
    pub fn default_config(&self) -> &AiConfig {
        &self.default_config
    }

    /// The occupant of the given seat.
    #[must_use]
    pub fn occupant(&self, color: Color) -> &Occupant {
        match color {
            Color::Black => &self.black,
            Color::White => &self.white,
        }
    }

    fn execute_play(&mut self, mv: Move) -> Result<Vec<Event>, CommandError> {
        let board = self.history.board();
        if board.status() != Status::Ongoing {
            return Err(CommandError::GameOver);
        }

        let old_cursor = self.history.cursor();
        let outcome = self.history.play(mv)?;
        let mut events = Vec::new();

        let side = board.to_move();
        let by = self.occupant(side).kind();
        events.push(Event::Moved { mv, by });

        if let PlayOutcome::Moved(_) = outcome {
            // If the node we were at now has multiple children,
            // a sibling branch was created.
            let siblings = self.history.children(old_cursor);
            if siblings.len() > 1 {
                events.push(Event::Branched { options: siblings });
            }
        }

        let new_board = self.history.board();
        if new_board.status() != Status::Ongoing {
            events.push(Event::GameOver {
                result: new_board.status(),
            });
        }

        Ok(events)
    }

    fn execute_step(&mut self) -> Result<Vec<Event>, CommandError> {
        let board = self.history.board();
        if board.status() != Status::Ongoing {
            return Err(CommandError::GameOver);
        }

        let side = board.to_move();
        let config = match self.occupant(side) {
            Occupant::Ai(cfg) => cfg.clone(),
            Occupant::Human => {
                return Err(CommandError::StepRequiresAi { seat: side });
            }
        };

        let mut effective_config = config.clone();
        if self.opening_schedule_active(side) {
            effective_config.temperature = 1.0;
            effective_config.noise_epsilon = 0.25;
        }

        let mut evaluator = Self::make_evaluator(&effective_config.evaluator);
        let search_cfg = mcts::SearchConfig {
            simulations: effective_config.simulations,
            c_puct: effective_config.c_puct,
        };

        let mut outcome = mcts::search(&board, &mut *evaluator, &search_cfg);

        if effective_config.noise_epsilon > 0.0 {
            mcts::add_dirichlet_noise(
                &mut outcome.tree,
                outcome.root,
                effective_config.noise_epsilon,
                0.3,
                &mut self.rng,
            );
        }

        let info = SearchInfo::from_search(&outcome, &board, &effective_config);
        let visits = mcts::visit_distribution(&outcome.tree, outcome.root);
        let mv = mcts::select_move(&visits, effective_config.temperature, &mut self.rng)
            .expect("AI must select a move from a non-empty distribution");

        let old_cursor = self.history.cursor();
        let play_outcome = self.history.play(mv)?;
        let new_cursor = self.history.cursor();
        self.search_info.insert(new_cursor, info);

        let mut events = Vec::new();
        events.push(Event::Moved {
            mv,
            by: OccupantKind::Ai,
        });

        if let PlayOutcome::Moved(_) = play_outcome {
            let siblings = self.history.children(old_cursor);
            if siblings.len() > 1 {
                events.push(Event::Branched { options: siblings });
            }
        }

        let new_board = self.history.board();
        if new_board.status() != Status::Ongoing {
            events.push(Event::GameOver {
                result: new_board.status(),
            });
        }

        Ok(events)
    }

    fn execute_release(&mut self, seat: SeatSel) -> Result<Vec<Event>, CommandError> {
        self.validate_seat_arg(seat)?;
        let mut events = Vec::new();
        self.apply_to_seats(seat, |this, color| {
            let sims = match color {
                Color::Black => this.black_sims,
                Color::White => this.white_sims,
            };
            let mut config = this.default_config.clone();
            if let Some(s) = sims {
                config.simulations = s;
            }
            this.set_occupant(color, Occupant::Ai(config));
            events.push(Event::SeatChanged {
                seat: color,
                now: OccupantKind::Ai,
            });
        });
        Ok(events)
    }

    fn execute_set_sims(&mut self, seat: SeatSel, n: u32) -> Result<Vec<Event>, CommandError> {
        self.validate_seat_arg(seat)?;
        if n == 0 {
            return Err(CommandError::ZeroSims);
        }
        let mut events = Vec::new();
        self.apply_to_seats(seat, |this, color| {
            match color {
                Color::Black => this.black_sims = Some(n),
                Color::White => this.white_sims = Some(n),
            }
            // If the seat is currently AI, update its live config too so the
            // change takes effect on the next Step.
            if let Occupant::Ai(cfg) = this.occupant_mut(color) {
                cfg.simulations = n;
            }
            events.push(Event::ConfigChanged {
                seat: color,
                sims: n,
            });
        });
        Ok(events)
    }

    fn execute_forward(&mut self, n: u32) -> Result<Vec<Event>, CommandError> {
        match self.history.forward(n) {
            ForwardOutcome::Moved(_) => Ok(vec![Event::CursorMoved {
                ply: self.history.ply(),
            }]),
            ForwardOutcome::NeedsChoice(options) => {
                let indexed: Vec<_> = options.into_iter().enumerate().collect();
                Ok(vec![Event::BranchPoint { options: indexed }])
            }
            ForwardOutcome::AtLeaf => Err(CommandError::ForwardAtLeaf),
        }
    }

    fn execute_var(&mut self, index: usize) -> Result<Vec<Event>, CommandError> {
        let cursor = self.history.cursor();
        let children = self.history.children(cursor);
        if children.is_empty() {
            return Err(CommandError::NotABranchPoint);
        }
        if index >= children.len() {
            return Err(CommandError::VarOutOfBounds {
                index,
                count: children.len(),
            });
        }
        self.history.var(index)?;
        Ok(vec![Event::CursorMoved {
            ply: self.history.ply(),
        }])
    }

    fn execute_take(&mut self, seat: SeatSel) -> Result<Vec<Event>, CommandError> {
        self.validate_seat_arg(seat)?;
        let mut events = Vec::new();
        self.apply_to_seats(seat, |this, color| {
            let new_occupant = Occupant::Human;
            this.set_occupant(color, new_occupant);
            events.push(Event::SeatChanged {
                seat: color,
                now: OccupantKind::Human,
            });
        });
        Ok(events)
    }

    fn validate_seat_arg(&self, seat: SeatSel) -> Result<(), CommandError> {
        // SeatSel is enum-generated; all variants are valid.
        let _ = seat;
        Ok(())
    }

    fn apply_to_seats<F>(&mut self, seat: SeatSel, mut f: F)
    where
        F: FnMut(&mut Self, Color),
    {
        match seat {
            SeatSel::Black => f(self, Color::Black),
            SeatSel::White => f(self, Color::White),
            SeatSel::Both => {
                f(self, Color::Black);
                f(self, Color::White);
            }
        }
    }

    fn occupant_mut(&mut self, color: Color) -> &mut Occupant {
        match color {
            Color::Black => &mut self.black,
            Color::White => &mut self.white,
        }
    }

    fn set_occupant(&mut self, color: Color, occupant: Occupant) {
        match color {
            Color::Black => self.black = occupant,
            Color::White => self.white = occupant,
        }
    }

    fn reset(&mut self) {
        self.history = History::from_opening(&self.init.opening);
        self.black = self.init.black.clone();
        self.white = self.init.white.clone();
        self.default_config = self.init.default_config.clone();
        self.black_sims = self.init.black_sims;
        self.white_sims = self.init.white_sims;
        self.search_info.clear();
        // Keep the same RNG; reset does not reseed.
    }

    fn make_evaluator(kind: &EvaluatorKind) -> Box<dyn mcts::Evaluator> {
        match kind {
            EvaluatorKind::Tactics => Box::new(mcts::TacticsEvaluator::new()),
        }
    }

    /// [experiment] Opening schedule for AI-vs-AI from an empty root.
    ///
    /// When the user has not explicitly configured temperature or
    /// Dirichlet noise, the first 10 plies of each line use temperature
    /// 1.0 with ε = 0.25 / α = 0.3 Dirichlet noise to introduce opening
    /// variety. After ply 10 (and for any explicitly configured session)
    /// the configured values are used verbatim.
    ///
    /// The schedule is keyed to the cursor ply along the current path, so
    /// branching back into the first 10 plies re-enters the schedule.
    fn opening_schedule_active(&self, _side: Color) -> bool {
        self.init.opening == Board::new()
            && self.black.is_ai()
            && self.white.is_ai()
            && !self.default_config.explicit_selection
            && self.default_config.temperature == 0.0
            && self.default_config.noise_epsilon == 0.0
            && self.history.ply() < 10
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::PlayError;

    fn mv(r: u8, c: u8) -> Move {
        Move::new(r, c).unwrap()
    }

    fn play(core: &mut SessionCore, r: u8, c: u8) -> Vec<Event> {
        core.execute(Command::Play(mv(r, c))).unwrap()
    }

    #[test]
    fn new_session_has_empty_root_and_human_seats() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        assert_eq!(core.board(), Board::new());
        assert_eq!(core.snapshot().ply, 0);
        assert_eq!(core.snapshot().black, OccupantKind::Human);
        assert_eq!(core.snapshot().white, OccupantKind::Human);
        assert_eq!(core.snapshot().status, Status::Ongoing);
    }

    #[test]
    fn hotseat_game_to_black_win_emits_game_over() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());

        // Five in a row for Black on row 7.
        let script = [
            (7, 3),
            (0, 0),
            (7, 4),
            (0, 1),
            (7, 5),
            (0, 2),
            (7, 6),
            (0, 3),
            (7, 7),
        ];
        for (i, &(r, c)) in script.iter().enumerate() {
            let events = play(&mut core, r, c);
            let is_last = i == script.len() - 1;
            if is_last {
                assert!(
                    events.iter().any(|e| matches!(e, &Event::GameOver { .. })),
                    "last move should emit GameOver, got {events:?}"
                );
            } else {
                assert!(
                    !events.iter().any(|e| matches!(e, &Event::GameOver { .. })),
                    "game should not be over yet, got {events:?}"
                );
            }
        }

        assert_eq!(core.board().status(), Status::Won(Color::Black));

        let err = core.execute(Command::Play(mv(0, 4))).unwrap_err();
        assert_eq!(err, CommandError::GameOver);
    }

    #[test]
    fn illegal_move_returns_error_and_leaves_state_unchanged() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        play(&mut core, 7, 7);

        let snapshot_before = core.snapshot();
        let err = core.execute(Command::Play(mv(7, 7))).unwrap_err();
        assert_eq!(err, CommandError::IllegalMove(PlayError::Occupied));
        assert_eq!(core.snapshot(), snapshot_before);
    }

    #[test]
    fn branch_mid_game_emits_branched_event() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        play(&mut core, 7, 7);
        play(&mut core, 6, 7);
        play(&mut core, 7, 8);
        play(&mut core, 6, 8);
        play(&mut core, 7, 9);
        play(&mut core, 6, 9);

        // Back three plies and play a new move: creates a branch at ply 3.
        core.execute(Command::Back { n: 3 }).unwrap();
        let events = play(&mut core, 8, 7);
        assert!(
            events.iter().any(|e| matches!(e, &Event::Branched { .. })),
            "expected Branched event, got {events:?}"
        );

        let branched = events.iter().find_map(|e| match e {
            Event::Branched { options } => Some(options.clone()),
            _ => None,
        });
        assert_eq!(branched, Some(vec![mv(6, 8), mv(8, 7)]));
    }

    #[test]
    fn back_forward_var_navigation_emits_cursor_moved() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        play(&mut core, 7, 7);
        play(&mut core, 6, 7);
        play(&mut core, 7, 8);

        let events = core.execute(Command::Back { n: 2 }).unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, &Event::CursorMoved { ply: 1 }))
        );

        let events = core.execute(Command::Forward { n: 1 }).unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, &Event::CursorMoved { ply: 2 }))
        );

        core.execute(Command::Back { n: 1 }).unwrap();
        play(&mut core, 8, 7);
        core.execute(Command::Back { n: 1 }).unwrap();

        let events = core.execute(Command::Forward { n: 1 }).unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, &Event::BranchPoint { .. })),
            "expected BranchPoint, got {events:?}"
        );

        let events = core.execute(Command::Var { index: 1 }).unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, &Event::CursorMoved { ply: 2 }))
        );
    }

    #[test]
    fn new_emits_reset_and_restores_opening() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        play(&mut core, 7, 7);
        play(&mut core, 6, 7);

        let events = core.execute(Command::New).unwrap();
        assert!(events.iter().any(|e| matches!(e, &Event::Reset)));
        assert_eq!(core.board(), Board::new());
        assert_eq!(core.snapshot().ply, 0);
    }

    #[test]
    fn take_is_no_op_when_seats_are_human() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        let events = core
            .execute(Command::Take {
                seat: SeatSel::Both,
            })
            .unwrap();
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| matches!(
            e,
            &Event::SeatChanged {
                now: OccupantKind::Human,
                ..
            }
        )));
    }

    #[test]
    fn step_on_human_seat_returns_typed_error() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        assert_eq!(
            core.execute(Command::Step).unwrap_err(),
            CommandError::StepRequiresAi { seat: Color::Black }
        );
    }

    #[test]
    fn release_sets_seat_to_ai_and_take_reclaims_it() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        let events = core
            .execute(Command::Release {
                seat: SeatSel::White,
            })
            .unwrap();
        assert!(events.iter().any(|e| matches!(
            e,
            Event::SeatChanged {
                seat: Color::White,
                now: OccupantKind::Ai,
            }
        )));
        assert_eq!(core.snapshot().white, OccupantKind::Ai);

        let events = core
            .execute(Command::Take {
                seat: SeatSel::White,
            })
            .unwrap();
        assert!(events.iter().any(|e| matches!(
            e,
            Event::SeatChanged {
                seat: Color::White,
                now: OccupantKind::Human,
            }
        )));
        assert_eq!(core.snapshot().white, OccupantKind::Human);
    }

    #[test]
    fn set_sims_updates_override_and_emits_config_changed() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        let events = core
            .execute(Command::SetSims {
                seat: SeatSel::Black,
                n: 123,
            })
            .unwrap();
        assert_eq!(events.len(), 1);
        assert!(events.iter().any(|e| matches!(
            e,
            Event::ConfigChanged {
                seat: Color::Black,
                sims: 123,
            }
        )));
    }

    #[test]
    fn zero_sims_is_rejected() {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        let err = core
            .execute(Command::SetSims {
                seat: SeatSel::Black,
                n: 0,
            })
            .unwrap_err();
        assert_eq!(err, CommandError::ZeroSims);
    }

    #[test]
    fn seeded_determinism_smoke() {
        let init = SessionInit {
            seed: Some(12345),
            ..SessionInit::new_hotseat()
        };
        let mut a = SessionCore::new(init.clone());
        let mut b = SessionCore::new(init);

        let script = [(7, 7), (6, 7), (7, 8), (6, 8)];
        for &(r, c) in &script {
            let ev_a = play(&mut a, r, c);
            let ev_b = play(&mut b, r, c);
            assert_eq!(ev_a, ev_b);
        }

        assert_eq!(a.snapshot(), b.snapshot());
    }
}
```

