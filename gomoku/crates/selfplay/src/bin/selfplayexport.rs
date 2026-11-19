//! Convert a self-play game corpus into a `nettrain --data` sample dataset.

use std::env;
use std::path::PathBuf;
use std::process;

use selfplay::corpus::read_corpus;
use selfplay::export::export_corpus;

const USAGE: &str = "Usage: selfplayexport --corpus <DIR> --out <DIR> [--shard-capacity <usize>]
Default: --shard-capacity 4096
Writes one canonical train::Sample per post-opening ply plus manifest.json.
nettrain --data <DIR> reads the shards directly and applies seeded D4 at batch time.
Use `--flag value` only; `--flag=value` is not supported.";

struct Args {
    corpus: PathBuf,
    out: PathBuf,
    shard_capacity: usize,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut corpus = None;
    let mut out = None;
    let mut shard_capacity: usize = 4096;
    let mut iter = args;

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--corpus" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--corpus requires a value".to_owned())?;
                corpus = Some(PathBuf::from(value));
            }
            "--out" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--out requires a value".to_owned())?;
                out = Some(PathBuf::from(value));
            }
            "--shard-capacity" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--shard-capacity requires a value".to_owned())?;
                shard_capacity = value
                    .parse()
                    .map_err(|error| format!("invalid --shard-capacity '{value}': {error}"))?;
            }
            _ => return Err(format!("unknown flag: {arg}")),
        }
    }

    if shard_capacity == 0 {
        return Err("--shard-capacity must be greater than zero".to_owned());
    }
    let corpus = corpus.ok_or_else(|| "--corpus is required".to_owned())?;
    let out = out.ok_or_else(|| "--out is required".to_owned())?;

    Ok(Args {
        corpus,
        out,
        shard_capacity,
    })
}

fn run(args: Args) -> Result<(), String> {
    let records =
        read_corpus(&args.corpus).map_err(|error| format!("failed to read corpus: {error}"))?;

    let manifest = export_corpus(&records, &args.out, args.shard_capacity)
        .map_err(|error| format!("failed to export samples: {error}"))?;

    let read_back = train::dataset::read_dataset(&args.out)
        .map_err(|error| format!("failed to read back exported samples: {error}"))?;
    if read_back.len() != manifest.sample_count {
        return Err(format!(
            "read-back sample count {} differs from manifest {}",
            read_back.len(),
            manifest.sample_count
        ));
    }

    eprintln!(
        "Exported {} samples from {} games into {} shards at {}",
        manifest.sample_count,
        manifest.game_count,
        manifest.shards.len(),
        args.out.display()
    );

    Ok(())
}

fn main() {
    let raw: Vec<String> = env::args().collect();
    if raw.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return;
    }

    match parse_args(raw.into_iter().skip(1)) {
        Ok(args) => {
            if let Err(error) = run(args) {
                eprintln!("selfplayexport failed: {error}");
                process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            process::exit(2);
        }
    }
}
