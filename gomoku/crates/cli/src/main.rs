mod board;
mod input;
mod opening;
mod render;
mod ui;

use board::{EngineKind, board_for};

const USAGE: &str = if cfg!(feature = "naive-engine") {
    "usage: gomoku [--engine naive|fast] [--plain] [--no-swap2]\n\nSwap2 opening is only available with the fast engine; use --no-swap2 to start from an empty board with any engine."
} else {
    "usage: gomoku [--engine fast] [--plain] [--no-swap2]\n\nThis build was compiled without the naive-engine feature; only the fast engine is available."
};

fn main() {
    let (kind, plain, swap2) = parse_args().unwrap_or_else(|e| {
        eprintln!("{e}");
        eprintln!("{USAGE}");
        std::process::exit(2);
    });

    #[cfg(feature = "naive-engine")]
    if swap2 && kind == EngineKind::Naive {
        eprintln!("error: the Swap2 opening requires the fast engine");
        eprintln!("       use --no-swap2 to start from an empty board with the naive engine");
        std::process::exit(2);
    }

    let board = board_for(kind);
    let result = if plain {
        ui::run_plain(board, kind, swap2)
    } else {
        ui::run_ui(board, kind, swap2)
    };
    if let Err(e) = result {
        eprintln!("terminal error: {e}");
        std::process::exit(1);
    }
}

fn parse_args() -> Result<(EngineKind, bool, bool), String> {
    let mut kind = EngineKind::Fast;
    let mut plain = false;
    let mut swap2 = true;
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
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    Ok((kind, plain, swap2))
}
