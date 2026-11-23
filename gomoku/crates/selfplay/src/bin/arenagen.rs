//! The promotion gate: a trained-net candidate vs an incumbent — the
//! baseline by default, or the reigning champion net (--incumbent-net).
//!
//! Plays color-swapped pairs at an equal simulation budget with no
//! exploration, and reports the candidate's score with a Wilson 95%
//! interval. The candidate is promoted when the interval's lower bound
//! clears 0.5. Arena games measure strength; they are not training data
//! and are not written as a corpus.

use std::env;
use std::path::{Path, PathBuf};
use std::process;

use baseline::{BaselineEvaluator, Device};
use engine::Color;
use mcts::Evaluator;
use neteval::NetEvaluator;
use player::{GuardConfig, OwnTssConfig, PlayerOptions};
use selfplay::arena::{ArenaConfig, ArenaResult, GameResult, play_arena};
use selfplay::corpus::write_corpus;
use selfplay::opening::TournamentOpenings;
use selfplay::record::GameRecord;

const USAGE: &str = "arenagen --net <RUN_DIR> --openings <PATH> [--openings <PATH> ...]
            [--games <N>] [--sims <N>] [--seed-base <N>]
            [--incumbent-net <RUN_DIR>] [--record <DIR>]

Play a trained-net candidate against the baseline incumbent in
color-swapped pairs at an equal simulation budget, with no exploration.

Required flags:
  --net <RUN_DIR>       candidate: a nettrain run directory (model.mpk +
                        run.json)
  --openings <PATH>     tournament opening list (repeatable, accumulated)

Optional numeric flags (syntax is `--flag value`; `--flag=value` is not
accepted):
  --games <N>           number of games; must be positive and even
                        (default: 20)
  --sims <N>            MCTS simulations per move for BOTH seats
                        (default: 400)
  --seed-base <N>       base seed of pair 0 (default: 0)
  --incumbent-net <RUN_DIR>
                        seat a trained net as the incumbent instead of the
                        baseline (the AlphaZero gate: candidate vs champion)
  --record <DIR>        also write the games as a corpus (browsable in the
                        GUI's Lab Explorer)

Operational rules:
  * Pair p plays the same opening (seed base+p) twice, candidate once as
    Black and once as White.
  * No exploration: temperature 0 and no root noise from the first ply.
  * Resignation is enabled; the own-TSS probe and the defensive guard run
    with the baseline budgets on both seats.
  * The verdict is the Wilson 95%% interval on the candidate's score
    (win 1, draw 1/2, loss 0): promoted when the lower bound clears 0.5.
  * Parallel arenas are separate processes over disjoint seed bases;
    merge the tallies by hand.";

#[derive(Debug)]
struct Args {
    net: PathBuf,
    opening_paths: Vec<PathBuf>,
    games: usize,
    sims: u32,
    seed_base: u64,
    incumbent_net: Option<PathBuf>,
    record: Option<PathBuf>,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut net = None;
    let mut opening_paths = Vec::new();
    let mut games: usize = 20;
    let mut sims: u32 = 400;
    let mut seed_base: u64 = 0;
    let mut incumbent_net: Option<PathBuf> = None;
    let mut record: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        let mut value = |flag: &str| {
            args.next()
                .ok_or_else(|| format!("{flag} requires a value"))
        };
        match arg.as_str() {
            "--net" => net = Some(PathBuf::from(value(&arg)?)),
            "--openings" => opening_paths.push(PathBuf::from(value(&arg)?)),
            "--games" => {
                let text = value(&arg)?;
                games = text
                    .parse()
                    .map_err(|error| format!("invalid --games '{text}': {error}"))?;
            }
            "--sims" => {
                let text = value(&arg)?;
                sims = text
                    .parse()
                    .map_err(|error| format!("invalid --sims '{text}': {error}"))?;
            }
            "--seed-base" => {
                let text = value(&arg)?;
                seed_base = text
                    .parse()
                    .map_err(|error| format!("invalid --seed-base '{text}': {error}"))?;
            }
            "--record" => record = Some(PathBuf::from(value(&arg)?)),
            "--incumbent-net" => incumbent_net = Some(PathBuf::from(value(&arg)?)),
            other => return Err(format!("unknown flag: {other}")),
        }
    }

    let net = net.ok_or_else(|| "--net is required".to_owned())?;
    if opening_paths.is_empty() {
        return Err("at least one --openings path is required".to_owned());
    }
    Ok(Args {
        net,
        opening_paths,
        games,
        sims,
        seed_base,
        incumbent_net,
        record,
    })
}

fn main() {
    let raw: Vec<String> = env::args().collect();
    if raw.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return;
    }

    let args = match parse_args(raw.into_iter().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            process::exit(2);
        }
    };

    if let Err(error) = run(&args) {
        eprintln!("arenagen failed: {error}");
        process::exit(1);
    }
}

fn run(args: &Args) -> Result<(), String> {
    // Validate the checkpoint before a single game is played: the arena
    // reloads it per game, and a bad path must fail fast.
    NetEvaluator::load(&args.net).map_err(|error| {
        format!(
            "failed to load net checkpoint {}: {error}",
            args.net.display()
        )
    })?;
    let mut pool = TournamentOpenings::load(&args.opening_paths)
        .map_err(|error| format!("failed to load openings: {error}"))?;

    let candidate_dir = args.net.clone();
    let candidate = move || {
        Box::new(NetEvaluator::load(&candidate_dir).expect("checkpoint loaded once at startup"))
            as Box<dyn Evaluator>
    };
    // The incumbent defaults to the baseline; --incumbent-net seats the
    // reigning champion instead (the round-2+ gate).
    if let Some(dir) = &args.incumbent_net {
        NetEvaluator::load(dir).map_err(|error| {
            format!(
                "failed to load incumbent checkpoint {}: {error}",
                dir.display()
            )
        })?;
    }
    let incumbent_dir = args.incumbent_net.clone();
    let incumbent = move || match &incumbent_dir {
        Some(dir) => Box::new(NetEvaluator::load(dir).expect("checkpoint loaded once at startup"))
            as Box<dyn Evaluator>,
        None => Box::new(BaselineEvaluator::new(Device::flex())) as Box<dyn Evaluator>,
    };
    let incumbent_desc = match &args.incumbent_net {
        Some(dir) => run_descriptor(dir),
        None => format!("baseline@{}", env!("CARGO_PKG_VERSION")),
    };

    let base_options = PlayerOptions {
        resign: true,
        own: OwnTssConfig::baseline(),
        guard: GuardConfig::baseline(),
        ..PlayerOptions::default()
    };
    let config = ArenaConfig {
        games: args.games,
        sims: args.sims,
        seed_base: args.seed_base,
    };

    eprintln!(
        "Arena: {} vs {}, {} games at {} sims, pairs from seed {}",
        args.net.display(),
        incumbent_desc,
        args.games,
        args.sims,
        args.seed_base
    );
    let result = play_arena(config, base_options, &mut pool, candidate, incumbent)
        .map_err(|error| format!("arena failed: {error}"))?;

    for (index, game) in result.games.iter().enumerate() {
        let symbol = match game.result {
            GameResult::Win => "W",
            GameResult::Loss => "L",
            GameResult::Draw => "D",
        };
        eprintln!(
            "  game {:>3}  seed {:>6}  candidate {:?}: {}",
            index + 1,
            game.seed,
            game.candidate_color,
            symbol
        );
    }

    let (wins, draws, losses) = result.tally();
    eprintln!(
        "Score: {wins}W {draws}D {losses}L — {:.1}/{} ({:.1}%)",
        result.score,
        result.game_count(),
        100.0 * result.score / result.game_count() as f64
    );
    eprintln!(
        "Wilson 95% interval: [{:.3}, {:.3}]",
        result.interval.0, result.interval.1
    );
    if result.promoted() {
        eprintln!("Verdict: PROMOTED — the lower bound clears 0.5");
    } else {
        eprintln!("Verdict: not promoted");
    }

    if let Some(dir) = &args.record {
        record_corpus(&result, &args.net, &incumbent_desc, dir)?;
    }

    Ok(())
}

/// `net:<dir-name>@<version>` — the provenance string for a net seat.
fn run_descriptor(dir: &Path) -> String {
    let name = dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.display().to_string());
    format!("net:{name}@{}", env!("CARGO_PKG_VERSION"))
}

/// Write the arena's games as a corpus: per-record evaluator strings carry
/// the seat pairing (the candidate's color flips per game), and the
/// manifest's seat block stays empty rather than averaging into a lie.
fn record_corpus(
    result: &ArenaResult,
    net: &Path,
    incumbent: &str,
    dir: &Path,
) -> Result<(), String> {
    let candidate = run_descriptor(net);

    let records: Vec<GameRecord> = result
        .played
        .iter()
        .zip(&result.games)
        .map(|(game, entry)| {
            let (black, white) = match entry.candidate_color {
                Color::Black => (candidate.as_str(), incumbent),
                Color::White => (incumbent, candidate.as_str()),
            };
            GameRecord::from_game_with_evaluators(game, black, white)
        })
        .collect();

    write_corpus(&records, dir, 1000)
        .map_err(|error| format!("failed to write the arena corpus: {error}"))?;
    eprintln!("Recorded {} games to {}", records.len(), dir.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_required_and_optional_flags() {
        let args = parse_args(
            [
                "--net",
                "runs/first-corpus",
                "--openings",
                "list.txt",
                "--games",
                "10",
                "--sims",
                "200",
                "--seed-base",
                "1000",
            ]
            .map(ToOwned::to_owned)
            .into_iter(),
        )
        .expect("parses");

        assert_eq!(args.net, PathBuf::from("runs/first-corpus"));
        assert_eq!(args.opening_paths, [PathBuf::from("list.txt")]);
        assert_eq!(args.games, 10);
        assert_eq!(args.sims, 200);
        assert_eq!(args.seed_base, 1000);
    }

    #[test]
    fn parses_the_record_flag() {
        let args = parse_args(
            [
                "--net",
                "runs/x",
                "--openings",
                "list.txt",
                "--record",
                "arena-out",
            ]
            .map(ToOwned::to_owned)
            .into_iter(),
        )
        .expect("parses");
        assert_eq!(args.record, Some(PathBuf::from("arena-out")));

        let args = parse_args(
            ["--net", "runs/x", "--openings", "list.txt"]
                .map(ToOwned::to_owned)
                .into_iter(),
        )
        .expect("parses");
        assert_eq!(args.record, None);
    }

    #[test]
    fn parses_the_incumbent_flag() {
        let args = parse_args(
            [
                "--net",
                "runs/x",
                "--openings",
                "list.txt",
                "--incumbent-net",
                "runs/champ",
            ]
            .map(ToOwned::to_owned)
            .into_iter(),
        )
        .expect("parses");
        assert_eq!(args.incumbent_net, Some(PathBuf::from("runs/champ")));

        let args = parse_args(
            ["--net", "runs/x", "--openings", "list.txt"]
                .map(ToOwned::to_owned)
                .into_iter(),
        )
        .expect("parses");
        assert_eq!(args.incumbent_net, None);
    }

    #[test]
    fn defaults_and_missing_flags() {
        let args = parse_args(
            ["--net", "runs/x", "--openings", "list.txt"]
                .map(ToOwned::to_owned)
                .into_iter(),
        )
        .expect("parses");
        assert_eq!(args.games, 20);
        assert_eq!(args.sims, 400);
        assert_eq!(args.seed_base, 0);

        let error = parse_args(
            ["--openings", "list.txt"]
                .map(ToOwned::to_owned)
                .into_iter(),
        )
        .expect_err("--net is required");
        assert!(error.contains("--net"), "'{error}' missing '--net'");

        let error = parse_args(["--net", "runs/x"].map(ToOwned::to_owned).into_iter())
            .expect_err("--openings is required");
        assert!(
            error.contains("--openings"),
            "'{error}' missing '--openings'"
        );
    }
}
