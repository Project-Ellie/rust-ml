//! The `gomoku` binary — a clap skeleton over the headless `cli` library.
//!
//! The legacy hotseat/puzzle UI is deleted (owner decision, 2026-10-17:
//! the study table will be the single CLI). The `play` and `selfplay`
//! subcommands parse their arguments, but there is no terminal front-end
//! to dispatch to until the study-table chapter lands.

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "gomoku",
    about = "Terminal Gomoku client",
    subcommand_required = false,
    arg_required_else_help = false
)]
struct CliArgs {
    #[command(subcommand)]
    cmd: Option<Commands>
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Play at the study table (not yet implemented).
    Play(PlayArgs),
    /// AI-vs-AI self-play (not yet implemented).
    SelfPlay(SearchArgs),
}

#[derive(Args, Debug, Clone)]
struct PlayArgs {
    #[command(flatten)]
    search: SearchArgs,

    /// Skip the Swap2 opening and start from an empty board.
    #[arg(long)]
    no_swap2: bool,
}

#[derive(Args, Debug, Clone)]
struct SearchArgs {
    /// AI occupant kind (currently only tactics).
    #[arg(long, value_name = "AI")]
    ai: Option<AiKind>,

    /// Human seat color when --ai is used.
    #[arg(long, value_name = "COLOR")]
    color: Option<PlayerColor>,
    
    /// Number of MCTS simulations per move.
    #[arg(long, value_name = "N", default_value_t = 400)]
    sims: u32,

    /// PUCT exploration constant.
    #[arg(long, value_name = "F", default_value_t = 1.5)]
    c_puct: f32,

    /// Move-selection temperature.
    #[arg(long, value_name = "F", default_value_t = 0.0)]
    temperature: f32,

    /// Dirichlet noise weight.
    #[arg(long, value_name = "F", default_value_t = 0.0)]
    noise: f32,

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
    White
}

fn main() {
    let args = CliArgs::parse();

    match args.cmd {
        None | Some(Commands::Play(_) | Commands::SelfPlay(_)) => {
            eprintln!("Function coming soon. Be patient")
        }
    }
}