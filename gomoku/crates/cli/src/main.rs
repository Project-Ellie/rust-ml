//! The `gomoku` binary — the clap front-end of the study table.
//!
//! All session logic lives in the `cli` library. This file does three
//! things: parse the command line, reject invalid flag combinations,
//! and build the [`SessionInit`] that [`cli::tui::study::run`] runs.
//!
//! `play` is the default subcommand: the Swap2 ceremony first unless
//! `--no-swap2`, then the study table. `selfplay` seats the AI in both
//! seats; Enter steps the search forward. The terminal or line-mode
//! selection happens inside `study::run`, not here.

use clap::{Args, Parser, Subcommand, ValueEnum};
use cli::actor::{AiConfig, EvaluatorKind, Occupant};
use cli::puzzle::PuzzlePack;
use cli::session::{PuzzleStudy, SessionInit};
use cli::tui::study;
use engine::{Board, Color};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "gomoku",
    about = "Terminal Gomoku client",
    subcommand_required = false,
    arg_required_else_help = false
)]
struct CliArgs {
    /// The `play` flags at the top level: `play` is the default
    /// subcommand, so `gomoku --no-swap2` means `gomoku play
    /// --no-swap2`.
    #[command(flatten)]
    play: PlayArgs,

    #[command(subcommand)]
    cmd: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Play at the study table: the Swap2 opening first, unless
    /// --no-swap2.
    Play(PlayArgs),
    /// AI-vs-AI self-play; Enter steps the search forward.
    Selfplay(SearchArgs),
}

#[derive(Args, Debug, Clone)]
struct PlayArgs {
    #[command(flatten)]
    search: SearchArgs,

    /// Skip the Swap2 opening and start from an empty board.
    #[arg(long)]
    no_swap2: bool,

    /// Study puzzle N of a puzzle pack (skips Swap2).
    #[arg(long, value_name = "PACK.json")]
    puzzle: Option<PathBuf>,

    /// Zero-based index of the puzzle to load (default 0).
    #[arg(long, value_name = "N", requires = "puzzle")]
    index: Option<usize>,
}

#[derive(Args, Debug, Clone)]
struct SearchArgs {
    /// AI occupant kind (currently only tactics). The AI takes the
    /// seat opposite the human; see --color.
    #[arg(long, value_name = "AI")]
    ai: Option<AiKind>,

    /// Human seat color when --ai is used (default: black).
    #[arg(long, value_name = "COLOR")]
    color: Option<PlayerColor>,

    /// Number of MCTS simulations per move (default 400).
    #[arg(long, value_name = "N", default_value_t = 400)]
    sims: u32,

    /// PUCT exploration constant (default 1.5).
    #[arg(long, value_name = "F", default_value_t = 1.5)]
    c_puct: f32,

    /// Move-selection temperature (default 0.0). An explicit value
    /// marks the selection as user-configured, which disables the
    /// opening schedule in AI-vs-AI sessions.
    #[arg(long, value_name = "F")]
    temperature: Option<f32>,

    /// Dirichlet noise weight (default 0.0). An explicit value marks
    /// the selection as user-configured, which disables the opening
    /// schedule in AI-vs-AI sessions.
    #[arg(long, value_name = "F")]
    noise: Option<f32>,

    /// Override the simulation count of the Black seat.
    #[arg(long, value_name = "N")]
    black_sims: Option<u32>,

    /// Override the simulation count of the White seat.
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
    Baseline,
}

impl From<AiKind> for EvaluatorKind {
    fn from(arg: AiKind) -> Self {
        match arg {
            AiKind::Tactics => Self::Tactics,
            AiKind::Baseline => Self::Baseline,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
enum PlayerColor {
    Black,
    White,
}

fn main() {
    let args = CliArgs::parse();

    // No subcommand at all is `play` with the top-level flags.
    let result = match args.cmd {
        None => run_play(&args.play),
        Some(Commands::Play(mut play)) => {
            // A `--no-swap2` placed before the subcommand parses into
            // the flattened top-level play args. The union of two
            // booleans is unambiguous, so the flag applies wherever it
            // was placed. (Search flags cannot be merged this way —
            // two values would compete — so they belong after the
            // subcommand.)
            play.no_swap2 |= args.play.no_swap2;
            run_play(&play)
        }
        Some(Commands::Selfplay(search)) => {
            // For selfplay the same flattened parse would silently
            // drop a `--no-swap2` placed before the subcommand. That
            // flag has no meaning for selfplay, so it is rejected
            // instead of ignored.
            if args.play.no_swap2 {
                usage_error("--no-swap2 is only valid with the 'play' subcommand");
            }
            if args.play.puzzle.is_some() {
                usage_error("--puzzle is only valid with the 'play' subcommand");
            }
            run_selfplay(&search)
        }
    };

    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

/// Report a usage error and exit with code 2 — the code clap itself
/// uses for command-line errors, so the shell sees one convention.
fn usage_error(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(2);
}

/// `play`: validate the seat flags, build the session, enter the
/// study table.
fn run_play(play: &PlayArgs) -> std::io::Result<()> {
    // A human seat color is meaningful only when the opposite seat is
    // AI. Without --ai both seats are human and the flag would
    // silently do nothing; reject it instead.
    if play.search.color.is_some() && play.search.ai.is_none() {
        usage_error("--color requires --ai");
    }
    // A puzzle pack seeds the study table directly; Swap2 is skipped.
    if let Some(pack_path) = &play.puzzle {
        let init = build_puzzle_init(&play.search, pack_path, play.index.unwrap_or(0));
        return study::run(init, None);
    }
    let swap2 = !play.no_swap2;
    let init = play_init(&play.search, swap2);
    // The hint is the first message line of the session. The ceremony
    // panel shows the phase details; the hint only names what the user
    // is looking at.
    let hint = swap2.then_some("Swap2 opening in progress. '?' shows the help.");
    study::run(init, hint)
}

/// `selfplay`: both seats AI; Enter steps the search forward.
fn run_selfplay(search: &SearchArgs) -> std::io::Result<()> {
    // Both seats are AI, so a human seat color has no meaning.
    if search.color.is_some() {
        usage_error("--color is not valid for selfplay");
    }
    study::run(selfplay_init(search), Some("Enter steps the AI."))
}

/// Load a puzzle pack and build the `SessionInit` for studying it.
///
/// Exits with code 2 (usage/data error) when the pack is missing or
/// invalid; the message names the path and points at `data/puzzles/`,
/// where the packs live.
fn build_puzzle_init(search: &SearchArgs, pack_path: &Path, index: usize) -> SessionInit {
    let text = std::fs::read_to_string(pack_path).unwrap_or_else(|e| {
        eprintln!(
            "error: cannot read puzzle pack '{}': {e}",
            pack_path.display()
        );
        eprintln!("       puzzle packs live in data/puzzles/");
        std::process::exit(2);
    });
    let pack = PuzzlePack::from_json(&text).unwrap_or_else(|e| {
        eprintln!("error: invalid puzzle pack '{}': {e}", pack_path.display());
        std::process::exit(2);
    });
    let opening = pack
        .get(index)
        .unwrap_or_else(|e| {
            eprintln!("error: {e} in '{}'", pack_path.display());
            std::process::exit(2);
        })
        .board
        .clone();

    // Seats and AI flags compose exactly as for a Swap2 opening.
    let mut init = play_init(search, false);
    init.opening = opening;
    init.puzzle = Some(PuzzleStudy {
        pack_path: pack_path.to_path_buf(),
        index,
        pack,
    });
    init
}

/// Build the `SessionInit` for `play`.
///
/// Hotseat by default; with `--ai`, the AI takes the seat opposite
/// the human (default human seat: Black). `swap2` decides whether the
/// session opens with the ceremony. The search flags are stored even
/// for a hotseat: a `release` command mid-session builds its AI
/// config from `default_config` plus the stored per-seat overrides.
fn play_init(search: &SearchArgs, swap2: bool) -> SessionInit {
    let base = ai_config_from(search);
    let (black, white) = match search.ai {
        Some(AiKind::Tactics) | Some(AiKind::Baseline) => {
            let human = search.color.map(player_color).unwrap_or(Color::Black);
            let ai = Occupant::Ai(seat_config(&base, human.other(), search));
            match human {
                Color::Black => (Occupant::Human, ai),
                Color::White => (ai, Occupant::Human),
            }
        }
        None => (Occupant::Human, Occupant::Human),
    };
    SessionInit {
        black,
        white,
        default_config: base,
        black_sims: search.black_sims,
        white_sims: search.white_sims,
        seed: search.seed,
        opening: Board::new(),
        swap2,
        puzzle: None,
    }
}

/// Build the `SessionInit` for `selfplay`: both seats AI. The per-seat
/// overrides let the two seats differ (`--black-sims 800 --sims 200`
/// plays an 800-simulation Black against a 200-simulation White).
fn selfplay_init(search: &SearchArgs) -> SessionInit {
    let base = ai_config_from(search);
    SessionInit {
        black: Occupant::Ai(seat_config(&base, Color::Black, search)),
        white: Occupant::Ai(seat_config(&base, Color::White, search)),
        default_config: base,
        black_sims: search.black_sims,
        white_sims: search.white_sims,
        seed: search.seed,
        opening: Board::new(),
        swap2: false,
        puzzle: None,
    }
}

/// The base AI configuration from the search flags.
///
/// `--temperature` and `--noise` are `Option`s on purpose: an explicit
/// flag must be distinguishable from the default. The session's
/// opening schedule (chapter 04) applies only to configurations the
/// user did not shape, so an explicitly passed flag sets
/// `explicit_selection` — and a `default_value_t` on the flag would
/// destroy that distinction.
fn ai_config_from(search: &SearchArgs) -> AiConfig {
    AiConfig {
        evaluator: search.ai.map(Into::into).unwrap_or(EvaluatorKind::Tactics),
        simulations: search.sims,
        c_puct: search.c_puct,
        temperature: search.temperature.unwrap_or(0.0),
        noise_epsilon: search.noise.unwrap_or(0.0),
        explicit_selection: search.temperature.is_some() || search.noise.is_some(),
    }
}

/// The configuration for one AI seat: the base config with that
/// seat's simulation override applied. This mirrors the `sims`
/// command, which updates the stored override and the live seat
/// config together; the flag is the same request at startup.
fn seat_config(base: &AiConfig, seat: Color, search: &SearchArgs) -> AiConfig {
    let mut config = base.clone();
    let sims = match seat {
        Color::Black => search.black_sims,
        Color::White => search.white_sims,
    };
    if let Some(n) = sims {
        config.simulations = n;
    }
    config
}

/// Map the CLI color to the engine color.
fn player_color(color: PlayerColor) -> Color {
    match color {
        PlayerColor::Black => Color::Black,
        PlayerColor::White => Color::White,
    }
}
