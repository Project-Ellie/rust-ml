//! Convert self-play game corpora into a `nettrain --data` sample dataset.

use std::env;
use std::path::PathBuf;
use std::process;

use selfplay::corpus::read_corpus;
use selfplay::export::{export_corpus, export_mixed};

const USAGE: &str =
    "Usage: selfplayexport --corpus <DIR>[:<WEIGHT>] [--corpus <DIR>[:<WEIGHT>] ...]
                    --out <DIR> [--shard-capacity <usize>] [--mix-seed <u64>]
Default: --shard-capacity 4096, --mix-seed 0, per-corpus weight 1.
Writes one canonical train::Sample per post-opening ply plus manifest.json.
With several --corpus flags the sample streams mix by seeded weighted
round-robin; the manifest records the source list. A single unweighted
--corpus exports exactly as before.
nettrain --data <DIR> reads the shards directly and applies seeded D4 at batch time.
Use `--flag value` only; `--flag=value` is not supported.";

struct Args {
    corpora: Vec<(PathBuf, u32)>,
    out: PathBuf,
    shard_capacity: usize,
    mix_seed: u64,
}

/// Split a `--corpus` value into path and weight: a trailing `:<WEIGHT>`
/// sets the mix weight, anything else is the plain path with weight 1.
fn parse_corpus(value: &str) -> Result<(PathBuf, u32), String> {
    if let Some((dir, weight)) = value.rsplit_once(':')
        && let Ok(weight) = weight.parse::<u32>()
    {
        if weight == 0 {
            return Err(format!("invalid corpus weight 0 in '{value}'"));
        }
        return Ok((PathBuf::from(dir), weight));
    }
    Ok((PathBuf::from(value), 1))
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut corpora = Vec::new();
    let mut out = None;
    let mut shard_capacity: usize = 4096;
    let mut mix_seed: u64 = 0;
    let mut iter = args;

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--corpus" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--corpus requires a value".to_owned())?;
                corpora.push(parse_corpus(&value)?);
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
            "--mix-seed" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--mix-seed requires a value".to_owned())?;
                mix_seed = value
                    .parse()
                    .map_err(|error| format!("invalid --mix-seed '{value}': {error}"))?;
            }
            _ => return Err(format!("unknown flag: {arg}")),
        }
    }

    if shard_capacity == 0 {
        return Err("--shard-capacity must be greater than zero".to_owned());
    }
    if corpora.is_empty() {
        return Err("--corpus is required".to_owned());
    }
    let out = out.ok_or_else(|| "--out is required".to_owned())?;

    Ok(Args {
        corpora,
        out,
        shard_capacity,
        mix_seed,
    })
}

fn run(args: Args) -> Result<(), String> {
    let mixed = args.corpora.len() > 1 || args.corpora[0].1 != 1;
    let manifest = if mixed {
        let mut sources = Vec::with_capacity(args.corpora.len());
        for (dir, weight) in &args.corpora {
            let name = dir
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| dir.display().to_string());
            let records = read_corpus(dir)
                .map_err(|error| format!("failed to read corpus {}: {error}", dir.display()))?;
            sources.push((name, records, *weight));
        }
        export_mixed(&sources, &args.out, args.shard_capacity, args.mix_seed)
            .map_err(|error| format!("failed to export samples: {error}"))?
    } else {
        let records = read_corpus(&args.corpora[0].0)
            .map_err(|error| format!("failed to read corpus: {error}"))?;
        export_corpus(&records, &args.out, args.shard_capacity)
            .map_err(|error| format!("failed to export samples: {error}"))?
    };

    let read_back = train::dataset::read_dataset(&args.out)
        .map_err(|error| format!("failed to read back exported samples: {error}"))?;
    if read_back.len() != manifest.sample_count {
        return Err(format!(
            "read-back sample count {} differs from manifest {}",
            read_back.len(),
            manifest.sample_count
        ));
    }

    if let Some(sources) = &manifest.sources {
        for source in sources {
            eprintln!(
                "  source {}: {} games, weight {}",
                source.corpus, source.games, source.weight
            );
        }
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
