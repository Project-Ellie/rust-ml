mod board;
mod input;
mod opening;
mod puzzle;
mod render;
mod ui;

use std::path::PathBuf;

use board::{EngineKind, board_for};

const USAGE: &str = if cfg!(feature = "naive-engine") {
    "usage: gomoku [--engine naive|fast] [--plain] [--no-swap2] [--puzzle <path> [--index N]]\n\nSwap2 opening is only available with the fast engine; use --no-swap2 to start from an empty board with any engine. Puzzle mode skips the opening entirely."
} else {
    "usage: gomoku [--engine fast] [--plain] [--no-swap2] [--puzzle <path> [--index N]]\n\nThis build was compiled without the naive-engine feature; only the fast engine is available. Puzzle mode skips the opening entirely."
};

fn main() {
    let config = parse_args().unwrap_or_else(|e| {
        eprintln!("{e}");
        eprintln!("{USAGE}");
        std::process::exit(2);
    });

    let result = match config {
        Config::Puzzle { path, index, plain } => run_puzzle_mode(path, index, plain),
        Config::Play { kind, plain, swap2 } => {
            #[cfg(feature = "naive-engine")]
            if swap2 && kind == EngineKind::Naive {
                eprintln!("error: the Swap2 opening requires the fast engine");
                eprintln!(
                    "       use --no-swap2 to start from an empty board with the naive engine"
                );
                std::process::exit(2);
            }
            let board = board_for(kind);
            if plain {
                ui::run_plain(board, kind, swap2)
            } else {
                ui::run_ui(board, kind, swap2)
            }
        }
    };
    if let Err(e) = result {
        eprintln!("terminal error: {e}");
        std::process::exit(1);
    }
}

enum Config {
    Play {
        kind: EngineKind,
        plain: bool,
        swap2: bool,
    },
    Puzzle {
        path: PathBuf,
        index: usize,
        plain: bool,
    },
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

fn parse_args() -> Result<Config, String> {
    let mut kind = EngineKind::Fast;
    let mut plain = false;
    let mut swap2 = true;
    let mut puzzle_path: Option<PathBuf> = None;
    let mut puzzle_index: Option<usize> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--engine" => {
                let value = args.next().ok_or(if cfg!(feature = "naive-engine") {
                    "--engine needs a value: naive|fast"
                } else {
                    "--engine needs a value: fast"
                })?;
                kind = match value.as_str() {
                    #[cfg(feature = "naive-engine")]
                    "naive" => EngineKind::Naive,
                    #[cfg(not(feature = "naive-engine"))]
                    "naive" => {
                        return Err("error: built without the naive-engine feature".to_string());
                    }
                    "fast" => EngineKind::Fast,
                    other => return Err(format!("unknown engine '{other}'")),
                };
            }
            "--plain" => plain = true,
            "--no-swap2" => swap2 = false,
            "--puzzle" => {
                let value = args.next().ok_or("--puzzle needs a file path")?;
                puzzle_path = Some(PathBuf::from(value));
            }
            "--index" => {
                let value = args.next().ok_or("--index needs a value")?;
                puzzle_index = Some(
                    value
                        .parse()
                        .map_err(|_| "--index must be a non-negative integer")?,
                );
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }

    if let Some(path) = puzzle_path {
        Ok(Config::Puzzle {
            path,
            index: puzzle_index.unwrap_or(0),
            plain,
        })
    } else {
        Ok(Config::Play { kind, plain, swap2 })
    }
}
