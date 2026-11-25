//! Strength-graded baseline self-play corpus generator.
//!
//! One invocation owns one contiguous base-seed range. Additional processes
//! with disjoint `--seed-base` ranges are the parallelism model.

use std::env;
use std::path::{Path, PathBuf};
use std::process;

use baseline::{BaselineEvaluator, Device};
use mcts::Evaluator;
use neteval::NetEvaluator;
use player::{OwnTssConfig, PlayerOptions};
use selfplay::corpus::{
    CorpusWriteOptions, OverwritePolicy, SeatEvaluators, check_writable, read_corpus,
    read_manifest, write_corpus_with_options,
};
use selfplay::generate::generate_games_with_seats;
use selfplay::opening::{OpeningSource, TournamentOpenings};
use selfplay::record::GameRecord;
use selfplay::schedule::{EXPLORATION_PLIES, ExplorationSchedule};

const USAGE: &str = "selfplaygen --out <DIR> --openings <PATH> [--openings <PATH> ...]
            [--black-sims <N>] [--white-sims <N>] [--games <N>]
            [--seed-base <N>] [--shard-capacity <N>]
            [--black-net <RUN_DIR>] [--white-net <RUN_DIR>]
            [--exploration-plies <N>] [--exploration-temperature <T>]
            [--exploration-epsilon <E>] [--max-plies <N>] [--overwrite]

Generate a corpus of self-play games, baseline or trained-net seated.

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
  --exploration-plies <N>  plies with temperature + root noise (default: 10;
                        0 disables exploration)
  --exploration-temperature <T>  move-selection temperature during the
                        exploration window (default: 1.0)
  --exploration-epsilon <E>  Dirichlet noise mix fraction at the root
                        (default: 0.25)
  --max-plies <N>       hard cap on post-ceremony plies; 0 means uncapped
                        (default: 0)
  --overwrite           replace a non-empty output directory (default: refuse)

Seat flags:
  --black-net <RUN_DIR> seat a trained net (nettrain run directory holding
                        model.mpk and run.json) for Black
  --white-net <RUN_DIR> seat a trained net for White

Operational rules:
  * One process generates the base seeds seed-base through
    seed-base + games - 1.
  * Seats without --black-net/--white-net run the baseline evaluator; all
    seats use the own-TSS probe (10 000 nodes / 30 plies).
  * Net evaluators run on the Flex CPU backend and are reloaded per game.
  * Parallel generation is separate processes over disjoint seed ranges.";

#[derive(Debug)]
struct Args {
    black_sims: u32,
    white_sims: u32,
    games: usize,
    seed_base: u64,
    shard_capacity: usize,
    exploration_plies: usize,
    exploration_temperature: f32,
    exploration_epsilon: f32,
    max_plies: usize,
    black_net: Option<PathBuf>,
    white_net: Option<PathBuf>,
    overwrite: bool,
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
    let mut exploration_plies: usize = EXPLORATION_PLIES;
    let mut exploration_temperature: f32 = 1.0;
    let mut exploration_epsilon: f32 = 0.25;
    let mut max_plies: usize = 0;
    let mut black_net: Option<PathBuf> = None;
    let mut white_net: Option<PathBuf> = None;
    let mut overwrite = false;
    let mut out_dir: Option<PathBuf> = None;
    let mut opening_paths = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--black-sims" => black_sims = parse_value(&arg, args.next().as_deref())?,
            "--white-sims" => white_sims = parse_value(&arg, args.next().as_deref())?,
            "--games" => games = parse_value(&arg, args.next().as_deref())?,
            "--seed-base" => seed_base = parse_value(&arg, args.next().as_deref())?,
            "--shard-capacity" => shard_capacity = parse_value(&arg, args.next().as_deref())?,
            "--exploration-plies" => {
                exploration_plies = parse_value(&arg, args.next().as_deref())?;
            }
            "--exploration-temperature" => {
                exploration_temperature = parse_value(&arg, args.next().as_deref())?;
            }
            "--exploration-epsilon" => {
                exploration_epsilon = parse_value(&arg, args.next().as_deref())?;
            }
            "--max-plies" => {
                max_plies = parse_value(&arg, args.next().as_deref())?;
            }
            "--overwrite" => overwrite = true,
            "--black-net" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--black-net requires a value".to_owned())?;
                black_net = Some(PathBuf::from(value));
            }
            "--white-net" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--white-net requires a value".to_owned())?;
                white_net = Some(PathBuf::from(value));
            }
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
        exploration_plies,
        exploration_temperature,
        exploration_epsilon,
        max_plies,
        black_net,
        white_net,
        overwrite,
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

/// Short provenance label for one seat: `net:<run-dir-name>` or `baseline`.
fn seat_label(net: Option<&Path>) -> String {
    net.map_or_else(
        || "baseline".to_owned(),
        |dir| {
            let name = dir
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| dir.display().to_string());
            format!("net:{name}")
        },
    )
}

/// Build the evaluator factory for one seat, validating a net seat by
/// loading its checkpoint once up front. Net evaluators are reloaded per
/// game — cheap next to even a single move of net-guided search.
fn seat_factory(net: Option<&Path>) -> Result<Box<dyn FnMut() -> Box<dyn Evaluator>>, String> {
    match net {
        Some(dir) => {
            NetEvaluator::load(dir).map_err(|error| {
                format!("failed to load net checkpoint {}: {error}", dir.display())
            })?;
            let dir = dir.to_owned();
            Ok(Box::new(move || {
                Box::new(NetEvaluator::load(&dir).expect("checkpoint loaded once at startup"))
                    as Box<dyn Evaluator>
            }))
        }
        None => Ok(Box::new(|| {
            Box::new(BaselineEvaluator::new(Device::flex())) as Box<dyn Evaluator>
        })),
    }
}

fn run_with_opening_source(
    args: &Args,
    opening_source: &mut impl OpeningSource,
) -> Result<(), String> {
    // Fail fast: generation is expensive, so the overwrite gate runs before
    // the first game, not after the last. The write path keeps its own gate.
    let overwrite_policy = if args.overwrite {
        OverwritePolicy::Allow
    } else {
        OverwritePolicy::Refuse
    };
    check_writable(&args.out_dir, overwrite_policy)
        .map_err(|error| format!("output directory {}: {error}", args.out_dir.display()))?;

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

    let schedule = ExplorationSchedule {
        plies: args.exploration_plies,
        temperature: args.exploration_temperature,
        noise_epsilon: args.exploration_epsilon,
        noise_alpha: ExplorationSchedule::default().noise_alpha,
    };
    let max_plies = if args.max_plies == 0 {
        None
    } else {
        Some(args.max_plies)
    };
    let black_factory = seat_factory(args.black_net.as_deref())?;
    let white_factory = seat_factory(args.white_net.as_deref())?;

    eprintln!(
        "Generating {} games: base seeds {}..={}, {}-{} vs {}-{}",
        args.games,
        args.seed_base,
        last_seed,
        seat_label(args.black_net.as_deref()),
        args.black_sims,
        seat_label(args.white_net.as_deref()),
        args.white_sims
    );

    let games = generate_games_with_seats(
        (0..args.games).map(|offset| args.seed_base + offset as u64),
        black_options,
        white_options,
        schedule,
        max_plies,
        opening_source,
        black_factory,
        white_factory,
    )
    .map_err(|error| format!("generation failed: {error}"))?;

    // Per-seat descriptors are recorded as soon as any seat runs a net;
    // pure-baseline corpora keep their previous record and manifest bytes.
    let mixed = args.black_net.is_some() || args.white_net.is_some();
    let baseline_descriptor = || format!("baseline@{}", env!("CARGO_PKG_VERSION"));
    let descriptor = |net: Option<&Path>| {
        net.map_or_else(baseline_descriptor, |dir| {
            format!("{}@{}", seat_label(Some(dir)), env!("CARGO_PKG_VERSION"))
        })
    };
    let black_descriptor = descriptor(args.black_net.as_deref());
    let white_descriptor = descriptor(args.white_net.as_deref());

    let records: Vec<GameRecord> = games
        .iter()
        .map(|game| {
            if mixed {
                GameRecord::from_game_with_evaluators(game, &black_descriptor, &white_descriptor)
            } else {
                GameRecord::from_game(game)
            }
        })
        .collect();

    write_corpus_with_options(
        &records,
        &args.out_dir,
        CorpusWriteOptions {
            shard_capacity: args.shard_capacity,
            overwrite: overwrite_policy,
            evaluators: mixed.then(|| SeatEvaluators {
                black: black_descriptor.clone(),
                white: white_descriptor.clone(),
            }),
            max_plies,
        },
    )
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
    fn parses_net_seats_and_exploration_window() {
        let args = parse_args(
            [
                "--out",
                "corpus",
                "--openings",
                "list.txt",
                "--black-net",
                "runs/first-corpus",
                "--white-net",
                "runs/second",
                "--exploration-plies",
                "30",
            ]
            .map(ToOwned::to_owned)
            .into_iter(),
        )
        .expect("parses");

        assert_eq!(args.black_net, Some(PathBuf::from("runs/first-corpus")));
        assert_eq!(args.white_net, Some(PathBuf::from("runs/second")));
        assert_eq!(args.exploration_plies, 30);
        assert_eq!(args.exploration_temperature, 1.0);
        assert_eq!(args.exploration_epsilon, 0.25);
        assert_eq!(args.max_plies, 0);
    }

    #[test]
    fn net_seats_and_exploration_window_default_off() {
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

        assert_eq!(args.black_net, None);
        assert_eq!(args.white_net, None);
        assert_eq!(args.exploration_plies, EXPLORATION_PLIES);
        assert_eq!(args.exploration_temperature, 1.0);
        assert_eq!(args.exploration_epsilon, 0.25);
        assert_eq!(args.max_plies, 0);
        assert!(!args.overwrite);
    }

    #[test]
    fn parses_overwrite_flag() {
        let args = parse_args(
            ["--out", "corpus", "--openings", "list.txt", "--overwrite"]
                .map(ToOwned::to_owned)
                .into_iter(),
        )
        .expect("parses");

        assert!(args.overwrite);
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
    fn parses_exploration_temperature_epsilon_and_max_plies() {
        let args = parse_args(
            [
                "--out",
                "corpus",
                "--openings",
                "list.txt",
                "--exploration-plies",
                "5",
                "--exploration-temperature",
                "0.5",
                "--exploration-epsilon",
                "0.1",
                "--max-plies",
                "50",
            ]
            .map(ToOwned::to_owned)
            .into_iter(),
        )
        .expect("parses");

        assert_eq!(args.exploration_plies, 5);
        assert_eq!(args.exploration_temperature, 0.5);
        assert_eq!(args.exploration_epsilon, 0.1);
        assert_eq!(args.max_plies, 50);
    }

    #[test]
    fn last_exploration_flag_value_wins() {
        let args = parse_args(
            [
                "--out",
                "corpus",
                "--openings",
                "list.txt",
                "--exploration-temperature",
                "0.2",
                "--exploration-temperature",
                "0.3",
                "--exploration-epsilon",
                "0.0",
                "--exploration-epsilon",
                "0.05",
                "--max-plies",
                "10",
                "--max-plies",
                "20",
            ]
            .map(ToOwned::to_owned)
            .into_iter(),
        )
        .expect("parses");

        assert_eq!(args.exploration_temperature, 0.3);
        assert_eq!(args.exploration_epsilon, 0.05);
        assert_eq!(args.max_plies, 20);
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
    fn refuses_a_non_empty_output_dir_before_generating() {
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
        let dir = TestDir::new("preflight");
        std::fs::create_dir_all(dir.path()).expect("dir exists");
        std::fs::write(dir.path().join("occupied"), "prior corpus").expect("marker file");

        let args = Args {
            black_sims: 1,
            white_sims: 1,
            games: 1,
            seed_base: 11,
            shard_capacity: 1,
            exploration_plies: EXPLORATION_PLIES,
            exploration_temperature: 1.0,
            exploration_epsilon: 0.25,
            max_plies: 0,
            black_net: None,
            white_net: None,
            overwrite: false,
            own_tss: OwnTssConfig::disabled(),
            out_dir: dir.path().to_owned(),
            opening_paths: vec![PathBuf::from("unused.txt")],
        };

        let error = run_with_opening_source(&args, &mut FixedOpening(opening.clone()))
            .expect_err("a non-empty output directory fails fast");
        assert!(
            error.contains("refusing to write a corpus into a non-empty directory"),
            "'{error}' names the refusal"
        );
        // The gate ran before generation: the marker file is untouched.
        assert_eq!(
            std::fs::read_to_string(dir.path().join("occupied")).expect("marker reads"),
            "prior corpus"
        );

        // With --overwrite the same run replaces the directory instead.
        let args = Args {
            overwrite: true,
            ..args
        };
        run_with_opening_source(&args, &mut FixedOpening(opening)).expect("overwrite run succeeds");
        assert!(!dir.path().join("occupied").exists());
        assert_eq!(
            read_manifest(dir.path())
                .expect("manifest reads back")
                .game_count,
            1
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
            exploration_plies: EXPLORATION_PLIES,
            exploration_temperature: 1.0,
            exploration_epsilon: 0.25,
            max_plies: 0,
            black_net: None,
            white_net: None,
            overwrite: false,
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

    #[test]
    fn max_plies_caps_the_game_as_a_draw_and_records_the_cap() {
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
        let dir = TestDir::new("cap");

        let args = Args {
            black_sims: 1,
            white_sims: 1,
            games: 1,
            seed_base: 11,
            shard_capacity: 1,
            exploration_plies: EXPLORATION_PLIES,
            exploration_temperature: 1.0,
            exploration_epsilon: 0.25,
            max_plies: 2,
            black_net: None,
            white_net: None,
            overwrite: false,
            own_tss: OwnTssConfig::disabled(),
            out_dir: dir.path().to_owned(),
            opening_paths: vec![PathBuf::from("unused.txt")],
        };

        run_with_opening_source(&args, &mut FixedOpening(opening)).expect("run succeeds");

        let records = read_corpus(dir.path()).expect("corpus reads back");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].plies.len(), 2);
        assert_eq!(records[0].outcome, selfplay::game::Outcome::Draw);

        let manifest = read_manifest(dir.path()).expect("manifest reads back");
        assert_eq!(manifest.max_plies, Some(2));
    }
}
