//! Chapter-02 capstone: a headless REPL over the session core.
//!
//! Reads commands from stdin, executes them through `SessionCore`, and
//! prints the resulting events plus a compact ASCII board. Demonstrates
//! the full command language in human-vs-human (hotseat) form.
//!
//! The binary is only a shell: argument handling, the input loop, and the
//! actual printing. Everything else — parsing, the `info` command, and the
//! state rendering — lives in [`cli::repl`], where tests can reach it.
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

use cli::repl;
use cli::session::{SessionCore, SessionInit};

fn parse_seed(args: &[String]) -> Option<u64> {
    args.iter()
        .position(|a| a == "--seed")
        .and_then(|i| args.get(i + 1).and_then(|s| s.parse::<u64>().ok()))
}

/// Print one batch of output lines.
///
/// [`cli::repl`] returns text instead of printing it, so this helper is
/// the single place where the binary touches stdout for content. (The
/// prompt below is the one exception: it must not end with a newline.)
fn print_lines(lines: &[String]) {
    for line in lines {
        println!("{line}");
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let script_path = args
        .iter()
        .position(|a| a == "--script")
        .and_then(|i| args.get(i + 1));
    let seed = parse_seed(&args);

    let init = SessionInit {
        seed,
        ..SessionInit::new_hotseat()
    };
    let mut core = SessionCore::new(init);

    print_lines(&repl::print_state(
        &core,
        "Session started. Type 'h' for help.",
    ));

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
                let outcome = repl::process_line(&line, &mut core, false);
                // The quit flag is ignored on purpose: a script continues
                // after a `quit` line, exactly as the pre-extraction
                // sessiondemo did when it discarded the return value.
                print_lines(&outcome.lines);
            }
        }

        None => {
            let stdin = stdin();
            print!("session> ");
            stdout().flush().unwrap();
            for line in stdin.lock().lines() {
                let line = line.expect("Cannot read line from stdin");
                let outcome = repl::process_line(&line, &mut core, true);
                print_lines(&outcome.lines);
                if outcome.quit {
                    break;
                }
                print!("session> ");
                stdout().flush().unwrap();
            }
        }
    }
}
