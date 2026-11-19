//! Strength-graded baseline self-play corpus generator.
//!
//! One invocation owns one contiguous base-seed range. Additional processes
//! with disjoint `--seed-base` ranges are the parallelism model.

use std::env;
use std::path::PathBuf;
use std::process;

use player::{OwnTssConfig, PlayerOptions};
use selfplay::corpus::{read_corpus, read_manifest, write_corpus};
use selfplay::generate::generate_games;
use selfplay::opening::{OpeningSource, TournamentOpenings};
use selfplay::record::GameRecord;

const USAGE: &str = "selfplaygen --out <DIR> --openings <PATH> [--openings <PATH> ...]
            [--black-sims <N>] [--white-sims <N>] [--games <N>]
            [--seed-base <N>] [--shard-capacity <N>]

Generate a corpus of strength-graded baseline-vs-baseline self-play games.

Required flags:
  --out <DIR>           output directory for the corpus
  --openings <PATH>     tournament opening list (repeatable, accumulated)

Optional numeric flags (syntax is `--flag value`; `--flag=value` is not
accepted; a repeated numeric flag takes its last value):
  --black-sims <N>      Black MCTS simulations per move (default: 800)
  --white-sims <N>      White MCTS simulations per move (default: 800)
  --games <N>           number of games to generate (default: 1, >0)
  --seed-base <N>       first base seed of the contiguous range (default: 0)
  --shard-capacity <N>  maximum games per shard (default: 1000, >0)

Operational rules:
  * One process generates the base seeds seed-base through
    seed-base + games - 1.
  * Both seats use the baseline own-TSS probe (10 000 nodes / 30 plies).
  * Parallel generation is separate processes over disjoint seed ranges.";

#[derive(Debug)]
struct Args {
    black_sims: u32,
    white_sims: u32,
    games: usize,
    seed_base: u64,
    shard_capacity: usize,
    own_tss: OwnTssConfig,
    out_dir: PathBuf,
    opening_paths: Vec<PathBuf>,
}

fn parse_value<T: std::str::FromStr>(flag: &str, value: Option<&str>) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    let value = value.ok_or_else(|| format!("{flag} requires a value"))?;
    value
        .parse()
        .map_err(|error| format!("invalid {flag} value '{value}': {error}"))
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut black_sims: u32 = 800;
    let mut white_sims: u32 = 800;
    let mut games: usize = 1;
    let mut seed_base: u64 = 0;
    let mut shard_capacity: usize = 1000;
    let mut out_dir: Option<PathBuf> = None;
    let mut opening_paths = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--black-sims" => black_sims = parse_value(&arg, args.next().as_deref())?,
            "--white-sims" => white_sims = parse_value(&arg, args.next().as_deref())?,
            "--games" => games = parse_value(&arg, args.next().as_deref())?,
            "--seed-base" => seed_base = parse_value(&arg, args.next().as_deref())?,
            "--shard-capacity" => shard_capacity = parse_value(&arg, args.next().as_deref())?,
            "--out" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--out requires a value".to_owned())?;
                out_dir = Some(PathBuf::from(value));
            }
            "--openings" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--openings requires a value".to_owned())?;
                opening_paths.push(PathBuf::from(value));
            }
            other => return Err(format!("unknown flag: {other}")),
        }
    }

    if games == 0 {
        return Err("--games must be greater than zero".to_owned());
    }
    if shard_capacity == 0 {
        return Err("--shard-capacity must be greater than zero".to_owned());
    }
    if opening_paths.is_empty() {
        return Err("at least one --openings path is required".to_owned());
    }
    let out_dir = out_dir.ok_or_else(|| "--out is required".to_owned())?;

    Ok(Args {
        black_sims,
        white_sims,
        games,
        seed_base,
        shard_capacity,
        own_tss: OwnTssConfig::baseline(),
        out_dir,
        opening_paths,
    })
}

fn main() {
    let mut raw_args = env::args();
    let _program = raw_args.next().unwrap_or_else(|| "selfplaygen".to_owned());

    if raw_args.any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return;
    }

    let args = match parse_args(env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("Error: {message}\n{USAGE}");
            process::exit(2);
        }
    };

    if let Err(error) = run(&args) {
        eprintln!("selfplaygen failed: {error}");
        process::exit(1);
    }
}

fn run(args: &Args) -> Result<(), String> {
    let mut pool = TournamentOpenings::load(&args.opening_paths)
        .map_err(|error| format!("failed to load openings: {error}"))?;
    run_with_opening_source(args, &mut pool)
}

fn run_with_opening_source(
    args: &Args,
    opening_source: &mut impl OpeningSource,
) -> Result<(), String> {
    let games_u64: u64 = args
        .games
        .try_into()
        .map_err(|_| "game count does not fit in the seed range".to_owned())?;
    let last_seed = args
        .seed_base
        .checked_add(games_u64.saturating_sub(1))
        .ok_or_else(|| "seed-base plus game count overflows u64".to_owned())?;

    let black_options = PlayerOptions {
        simulations: args.black_sims,
        own: args.own_tss,
        ..PlayerOptions::default()
    };
    let white_options = PlayerOptions {
        simulations: args.white_sims,
        own: args.own_tss,
        ..PlayerOptions::default()
    };

    eprintln!(
        "Generating {} games: base seeds {}..={}, baseline-{} vs baseline-{}",
        args.games, args.seed_base, last_seed, args.black_sims, args.white_sims
    );

    let games = generate_games(
        (0..args.games).map(|offset| args.seed_base + offset as u64),
        black_options,
        white_options,
        opening_source,
    )
    .map_err(|error| format!("generation failed: {error}"))?;

    let records: Vec<GameRecord> = games.iter().map(GameRecord::from_game).collect();

    write_corpus(&records, &args.out_dir, args.shard_capacity)
        .map_err(|error| format!("failed to write corpus: {error}"))?;

    let read_back = read_corpus(&args.out_dir)
        .map_err(|error| format!("failed to read back corpus: {error}"))?;
    if read_back != records {
        return Err("read-back records differ from generated records".to_owned());
    }

    let manifest = read_manifest(&args.out_dir)
        .map_err(|error| format!("failed to read back manifest: {error}"))?;
    if manifest.pairing.black.simulations != args.black_sims
        || manifest.pairing.white.simulations != args.white_sims
    {
        return Err("manifest does not preserve the declared seat pairing".to_owned());
    }

    eprintln!(
        "Wrote manifest to {}",
        args.out_dir.join("manifest.json").display()
    );
    eprintln!(
        "Games: {}, shards: {}, accepted seeds: {:?}",
        records.len(),
        manifest.shards.len(),
        manifest.seeds
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Color, Move};
    use rand::prelude::StdRng;
    use selfplay::opening::{Opening, OpeningAnswer, OpeningProvenance};
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("System clock is after Unix epoch")
                .as_nanos();
            let sequence = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "selfplaygen-{name}-{}-{nanos}-{sequence}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn mv(row: u8, col: u8) -> Move {
        Move::new(row, col).expect("test coordinates on board")
    }

    struct FixedOpening(Opening);

    impl OpeningSource for FixedOpening {
        fn opening(&mut self, _rng: &mut StdRng) -> Opening {
            self.0.clone()
        }
    }

    #[test]
    fn generation_defaults_to_the_baseline_own_tss_probe() {
        let args = parse_args(
            [
                "--out".to_owned(),
                "corpus".to_owned(),
                "--openings".to_owned(),
                "list.txt".to_owned(),
            ]
            .into_iter(),
        )
        .expect("parses");

        assert_eq!(args.own_tss, OwnTssConfig::baseline());
    }

    #[test]
    fn parses_all_flags_with_last_numeric_value_winning() {
        let args = parse_args(
            [
                "--black-sims",
                "300",
                "--white-sims",
                "200",
                "--games",
                "2",
                "--games",
                "1",
                "--seed-base",
                "41",
                "--shard-capacity",
                "7",
                "--out",
                "out-dir",
                "--openings",
                "first.txt",
                "--openings",
                "second.txt",
            ]
            .map(ToOwned::to_owned)
            .into_iter(),
        )
        .expect("parses");

        assert_eq!(args.black_sims, 300);
        assert_eq!(args.white_sims, 200);
        assert_eq!(args.games, 1);
        assert_eq!(args.seed_base, 41);
        assert_eq!(args.shard_capacity, 7);
        assert_eq!(args.out_dir, PathBuf::from("out-dir"));
        assert_eq!(
            args.opening_paths,
            [PathBuf::from("first.txt"), PathBuf::from("second.txt")]
        );
    }

    #[test]
    fn rejects_missing_required_flags_and_zero_counts() {
        let error = parse_args(["--openings".to_owned(), "list.txt".to_owned()].into_iter())
            .expect_err("missing --out must fail");
        assert!(error.contains("--out"), "'{error}' missing '--out'");

        let error = parse_args(
            [
                "--out".to_owned(),
                "corpus".to_owned(),
                "--openings".to_owned(),
                "list.txt".to_owned(),
                "--games".to_owned(),
                "0".to_owned(),
            ]
            .into_iter(),
        )
        .expect_err("--games 0 must fail");
        assert!(
            error.contains("greater than zero"),
            "'{error}' missing 'greater than zero'"
        );
    }

    #[test]
    fn one_budget_one_game_writes_a_corpus_that_the_reader_round_trips() {
        let opening = Opening::new(
            [mv(5, 8), mv(5, 10), mv(7, 9)],
            OpeningAnswer::AddTwo {
                stones: [mv(9, 8), mv(9, 10)],
                final_choice: Color::White,
            },
            OpeningProvenance::Listed {
                list: "fixed-test".to_owned(),
                index: 0,
            },
        );
        let dir = TestDir::new("roundtrip");

        let args = Args {
            black_sims: 1,
            white_sims: 1,
            games: 1,
            seed_base: 11,
            shard_capacity: 1,
            own_tss: OwnTssConfig::disabled(),
            out_dir: dir.path().to_owned(),
            opening_paths: vec![PathBuf::from("unused.txt")],
        };

        run_with_opening_source(&args, &mut FixedOpening(opening.clone())).expect("run succeeds");

        let manifest = read_manifest(dir.path()).expect("manifest reads back");
        assert_eq!(manifest.game_count, 1);
        assert_eq!(manifest.seeds, vec![11]);
        assert_eq!(manifest.shards, ["shard-000.bin"]);

        let records = read_corpus(dir.path()).expect("corpus reads back");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].header.seed, 11);
        assert_eq!(records[0].header.config.black.simulations, 1);
        assert_eq!(records[0].header.config.white.simulations, 1);
        assert_eq!(records[0].header.opening, opening);
        assert!(!records[0].plies.is_empty());
    }
}
