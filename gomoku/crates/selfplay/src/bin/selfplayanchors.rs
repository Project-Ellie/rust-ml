//! Re-verify certified self-play suffixes and emit milestone-4 anchor seeds.

use std::fs::File;
use std::path::PathBuf;
use std::{env, process};

use selfplay::anchor::extract_anchor_seeds;
use selfplay::corpus::read_corpus;

const USAGE: &str = "Usage: selfplayanchors --corpus <DIR> --out <JSON>

Re-reads a selfplay corpus, requires every certified suffix to pass engine
verify_line and match the played line, and writes replayable anchor seeds.
Games without certificates emit no anchor data and no negative labels.
The output file must not already exist.";

#[derive(Debug, PartialEq, Eq)]
struct Args {
    corpus: PathBuf,
    out: PathBuf,
}

fn parse_args(args: Vec<String>) -> Result<Args, String> {
    let mut corpus = None;
    let mut out = None;
    let mut iter = args.into_iter().skip(1);

    while let Some(arg) = iter.next() {
        let value = iter
            .next()
            .ok_or_else(|| format!("{arg} requires a value"))?;
        match arg.as_str() {
            "--corpus" => corpus = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown flag: {arg}")),
        }
    }

    let corpus = corpus.ok_or_else(|| "--corpus is required".to_owned())?;
    let out = out.ok_or_else(|| "--out is required".to_owned())?;

    Ok(Args { corpus, out })
}

fn run(args: Args) -> Result<(), String> {
    let records =
        read_corpus(&args.corpus).map_err(|error| format!("failed to read corpus: {error}"))?;
    let seeds = extract_anchor_seeds(&records)
        .map_err(|error| format!("certificate extraction failed: {error}"))?;

    let file = File::options()
        .write(true)
        .create_new(true)
        .open(&args.out)
        .map_err(|error| format!("failed to create '{}': {error}", args.out.display()))?;
    serde_json::to_writer_pretty(file, &seeds)
        .map_err(|error| format!("failed to write anchor JSON: {error}"))?;

    eprintln!("Wrote {} verified anchor seeds", seeds.len());
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return;
    }

    match parse_args(args) {
        Ok(parsed) => {
            if let Err(error) = run(parsed) {
                eprintln!("selfplayanchors failed: {error}");
                process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            process::exit(2);
        }
    }
}
