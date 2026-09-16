mod board;
mod render;

use board::{EngineKind, board_for};

fn main() {
    let kind = parse_engine_arg().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });
    let board = board_for(kind);
    for line in render::render_lines(&*board) {
        println!("{line}");
    }
    println!("{}", render::status_line(&*board));
}

fn parse_engine_arg() -> Result<EngineKind, String> {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next().as_deref()) {
        (None, None) => Ok(EngineKind::Fast),
        (Some("--engine"), Some("naive")) => Ok(EngineKind::Naive),
        (Some("--engine"), Some("fast")) => Ok(EngineKind::Fast),
        (Some("--engine"), Some(other)) => Err(format!("unknown engine option: {}", other)),
        (Some("--engine"), None) => Err("--engine needs a value: naive|fast".into()),
        (Some(other), _) => Err(format!(
            "unknown argument '{other}' (want: --engine naive|fast)"
        )),
        (None, Some(_)) => unreachable!(),
    }
}
