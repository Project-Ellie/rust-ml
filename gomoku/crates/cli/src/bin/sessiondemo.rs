//! Chapter-02 capstone: a headless REPL over the session core.
//!
//! Reads commands from stdin, executes them through `SessionCore`, and
//! prints the resulting events plus a compact ASCII board. Demonstrates
//! the full command language in human-vs-human (hotseat) form.
//!
//! Run interactively:
//!
//! ```text
//! cargo run -p cli --bin sessiondemo
//! ```
//!
//! Run a scripted game non-interactively:
//!
//! ```text
//! cargo run -p cli --bin sessiondemo -- --script script.txt
//! ```

use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Write, stdin, stdout};

use cli::command::Command;
use cli::fmt::render_board;
use cli::session::{SessionCore, SessionInit};

const HELP: &str = r#"Commands (case-insensitive):
  SQUARE           play a move, e.g. "h8"
  play SQUARE      same as above
  step             let the AI play
  back [N]         go back N plies (default 1)
  forward [N]      go forward N plies (default 1)
  var N            choose child variation N
  take black|white|both      make seat(s) human
  release black|white|both   release seat(s) to AI
  sims black|white N         set per-seat simulations
  info             show AI search info at the cursor
  new              reset to opening
  h, help, ?       show this help
  q, quit, exit    quit
  hint             show the first move of the puzzle's line
  reveal           show the puzzle's full claimed line
  next / prev      load the next/previous puzzle (clamps at the ends)
"#;

fn parse_seed(args: &[String]) -> Option<u64> {
    args.iter()
        .position(|a| a == "--seed")
        .and_then(|i| args.get(i + 1).and_then(|s| s.parse::<u64>().ok()))
}

fn parse_flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).map(String::as_str))
}

fn parse_index(args: &[String]) -> Result<Option<usize>, String> {
    let Some(i) = args.iter().position(|a| a == "--index") else {
        return Ok(None);
    };
    let value = args
        .get(i + 1)
        .ok_or_else(|| "--index requires a value".to_string())?;
    value
        .parse::<usize>()
        .map(Some)
        .map_err(|_| format!("invalid --index value '{}'", value))
}

fn build_init(args: &[String]) -> SessionInit {
    let seed = parse_seed(args);
    match parse_flag_value(args, "--puzzle") {
        Some(path) => {
            let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Cannot read puzzle pack '{}': {}", path, e);
                std::process::exit(1);
            });
            let pack = cli::puzzle::PuzzlePack::from_json(&text).unwrap_or_else(|e| {
                eprintln!("Invalid puzzle pack '{}': {}", path, e);
                std::process::exit(1);
            });
            let index = parse_index(args)
                .unwrap_or_else(|e| {
                    eprintln!("{e}");
                    std::process::exit(1);
                })
                .unwrap_or(0);
            let init = SessionInit::puzzle(std::path::PathBuf::from(path), pack, index)
                .unwrap_or_else(|e| {
                    eprintln!("Cannot load puzzle: {}", e);
                    std::process::exit(1);
                });
            SessionInit { seed, ..init }
        }
        None => SessionInit {
            seed,
            ..SessionInit::new_hotseat()
        },
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let script_path = args
        .iter()
        .position(|a| a == "--script")
        .and_then(|i| args.get(i + 1));

    let init = build_init(&args);

    let mut core = SessionCore::new(init);

    print_state(&core, "Session started. Type 'h' for help.");

    match script_path {
        Some(path) => {
            let file = File::open(path).unwrap_or_else(|e| {
                eprintln!("Cannot open script '{}': {}", path, e);
                std::process::exit(1);
            });
            let reader = BufReader::new(file);
            for line in reader.lines() {
                let line = line.unwrap_or_else(|e| {
                    eprintln!("Cannot read line {}", e);
                    std::process::exit(1);
                });

                process_line(&line, &mut core, false);
            }
        }

        None => {
            let stdin = stdin();
            print!("session> ");
            stdout().flush().unwrap();
            for line in stdin.lock().lines() {
                let line = line.expect("Cannot read line from stdin");
                let should_quit = process_line(&line, &mut core, true);
                if should_quit {
                    break;
                }
                print!("session> ");
                stdout().flush().unwrap();
            }
        }
    }
}

fn process_line(line: &str, core: &mut SessionCore, interactive: bool) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return false;
    }
    if trimmed.eq_ignore_ascii_case("info") {
        print_search_info(core);
        return false;
    }
    match cli::command::parse(trimmed) {
        Ok(Command::Help) => {
            println!("{HELP}");
            return false;
        }
        Ok(Command::Quit) => {
            if interactive {
                println!("Bye.")
            }
            return true;
        }
        Ok(cmd) => match core.execute(cmd) {
            Ok(events) => {
                if events.is_empty() {
                    println!("(no events)")
                } else {
                    for event in &events {
                        println!("  {event}");
                    }
                }
                print_state(core, "");
            }
            Err(e) => {
                println!("Error {e}");
            }
        },
        Err(e) => {
            println!("Parse error {e}");
        }
    }
    false
}

fn print_search_info(core: &SessionCore) {
    match core.search_info() {
        None => println!("No search info at the cursor."),
        Some(info) => {
            let total_visits: u32 = info.visits.iter().map(|(_, n)| n).sum();
            println!("value {:.3}  total visits {total_visits}", info.value);
            if info.visits.is_empty() {
                println!("(no legal moves)");
                return;
            }
            let prior_by_move: std::collections::HashMap<_, _> =
                info.prior.iter().map(|(mv, p)| (*mv, *p)).collect();
            println!("top moves (visits, prior%):");
            for (i, (mv, visits)) in info.visits.iter().take(5).enumerate() {
                let prior = prior_by_move.get(mv).copied().unwrap_or(0.0);
                println!(
                    "  {}. ({}, {:.1}%) {}",
                    i + 1,
                    visits,
                    prior * 100.0,
                    cli::fmt::format_move(*mv)
                );
            }
        }
    }
}

fn print_state(core: &SessionCore, header: &str) {
    if !header.is_empty() {
        println!("{header}");
    }
    if let Some(study) = core.puzzle() {
        let puzzle = &study.pack.items[study.index];
        println!(
            "puzzle {} of {}  attacker {:?}",
            study.index + 1,
            study.pack.items.len(),
            puzzle.attacker
        )
    }
    let snapshot = core.snapshot();
    println!(
        "ply {}  status {:?}  black {:?}  white {:?}",
        snapshot.ply, snapshot.status, snapshot.black, snapshot.white
    );
    if !snapshot.branch_options.is_empty() {
        let opts: Vec<String> = snapshot
            .branch_options
            .iter()
            .map(|(i, m)| format!("{i}: {}", cli::fmt::format_move(*m)))
            .collect();
        println!("branches: {}", opts.join("  "));
    }
    println!("{}", render_board(&core.board(), snapshot.last_move));
    println!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_index_defaults_to_none() {
        let args = vec!["sessiondemo".into(), "--puzzle".into(), "pack.json".into()];
        assert_eq!(parse_index(&args), Ok(None));
    }

    #[test]
    fn valid_index_is_parsed() {
        let args = vec!["sessiondemo".into(), "--index".into(), "2".into()];
        assert_eq!(parse_index(&args), Ok(Some(2)));
    }

    #[test]
    fn malformed_index_is_rejected() {
        let args = vec!["sessiondemo".into(), "--index".into(), "foo".into()];
        assert!(parse_index(&args).is_err());
    }

    #[test]
    fn missing_index_value_is_rejected() {
        let args = vec!["sessiondemo".into(), "--index".into()];
        assert!(parse_index(&args).is_err());
    }
}
