mod board;
mod input;
mod render;
mod ui;

use board::{EngineKind, board_for};

fn main() {
    let (kind, plain) = parse_args().unwrap_or_else(|e| {
        eprintln!("{e}");
        eprintln!("usage: gomoku [--engine naive|fast] [--plain]");
        std::process::exit(2);
    });
    let board = board_for(kind);
    let result = if plain {
        ui::run_plain(board, kind)
    } else {
        ui::run_ui(board, kind)
    };
    if let Err(e) = result {
        eprintln!("terminal error: {e}");
        std::process::exit(1);
    }
}

fn parse_args() -> Result<(EngineKind, bool), String> {
    let mut kind = EngineKind::Fast;
    let mut plain = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--engine" => {
                let value = args.next().ok_or("--engine needs a value: naive|fast")?;
                kind = match value.as_str() {
                    "naive" => EngineKind::Naive,
                    "fast" => EngineKind::Fast,
                    other => return Err(format!("unknown engine '{other}'")),
                };
            }
            "--plain" => plain = true,
            "-h" | "--help" => {
                println!("usage: gomoku [--engine naive|fast] [--plain]");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    Ok((kind, plain))
}
