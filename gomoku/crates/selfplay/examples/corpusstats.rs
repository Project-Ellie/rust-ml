//! Print per-spec-group and overall statistics for self-play corpora.
//!
//! Usage:
//!     cargo run -p selfplay --release --example corpusstats -- <ROOT>
//!
//! `<ROOT>` may be either:
//!
//! * a corpus root containing chunk subdirectories such as `400v400-w00-c00`,
//!   each holding a `manifest.json`; or
//! * a single corpus directory that contains `manifest.json` directly.
//!
//! Chunk directories are grouped by the prefix before the first `-`, e.g.
//! `400v400-w00-c00` belongs to the `400v400` group.  For each group and for
//! the whole data set the tool prints:
//!
//! 1. game count;
//! 2. ply-length distribution (min, median, mean, p95, max);
//! 3. outcome split by color (Black / White / Draw);
//! 4. termination mix (five-in-a-row / resignation / capped / true draw);
//! 5. decision-ply statistics, including the TSS/proof share of plies;
//! 6. move-prefix diversity at ply 10.
//!
//! A ply with `root_value == None` marks a TSS/proof move that bypassed
//! ordinary search; such plies are treated as "decided" for the decision-ply
//! calculation and counted separately as the TSS share of recorded plies.
//! Capped games are detected as `Draw` outcomes whose replayed board is not
//! yet full.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use engine::{Board, Status};
use selfplay::corpus::read_corpus;
use selfplay::game::{Outcome, WinReason};
use selfplay::record::{GameRecord, RecordedPly};

fn main() -> std::io::Result<()> {
    let root = parse_root_arg()?;
    let groups = load_groups(&root)?;

    for (name, records) in &groups {
        print_stats(name, records);
    }
    print_stats("Overall", &flatten_groups(&groups));

    Ok(())
}

fn parse_root_arg() -> std::io::Result<PathBuf> {
    let mut args = env::args_os().skip(1);
    let path = args.next().map(PathBuf::from).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "usage: corpusstats <ROOT>",
        )
    })?;
    if args.next().is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "usage: corpusstats <ROOT>",
        ));
    }
    if !path.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} is not a directory", path.display()),
        ));
    }
    Ok(path)
}

/// Discover corpora under `root` and group them by spec group.
///
/// If `root` itself contains `manifest.json` it is treated as a single corpus
/// whose group name is the directory name.  Otherwise each immediate
/// subdirectory with a `manifest.json` becomes one corpus; its group is the
/// substring before the first `-` in the directory name.  Directories without
/// a manifest are silently skipped.
fn load_groups(root: &Path) -> std::io::Result<Vec<(String, Vec<GameRecord>)>> {
    let mut entries: Vec<(String, PathBuf)> = Vec::new();

    if root.join("manifest.json").is_file() {
        let name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("single")
            .to_owned();
        entries.push((name, root.to_path_buf()));
    } else {
        for child in fs::read_dir(root)? {
            let child = child?;
            let path = child.path();
            if !path.is_dir() {
                continue;
            }
            if !path.join("manifest.json").is_file() {
                // Incomplete or non-corpus directory; skip as requested.
                continue;
            }
            let dir_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_owned();
            let group = dir_name
                .split_once('-')
                .map(|(prefix, _)| prefix.to_owned())
                .unwrap_or(dir_name);
            entries.push((group, path));
        }
    }

    // Sort by group name for deterministic output.
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut groups: Vec<(String, Vec<GameRecord>)> = Vec::new();
    for (group, path) in entries {
        let records = read_corpus(&path).map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!("failed to read corpus {}: {}", path.display(), e),
            )
        })?;
        if let Some(last) = groups.last_mut().filter(|(g, _)| g == &group) {
            last.1.extend(records);
        } else {
            groups.push((group, records));
        }
    }

    Ok(groups)
}

fn flatten_groups(groups: &[(String, Vec<GameRecord>)]) -> Vec<GameRecord> {
    groups
        .iter()
        .flat_map(|(_, rs)| rs.iter().cloned())
        .collect()
}

fn print_stats(name: &str, records: &[GameRecord]) {
    let n = records.len();
    println!("=== {name} ===");
    if n == 0 {
        println!("  (no games)");
        println!();
        return;
    }

    println!("  Games: {n}");

    let lengths: Vec<usize> = records.iter().map(|r| r.plies.len()).collect();
    println!("  Ply length: {}", describe_distribution(&lengths));

    let (black, white, draw) = color_split(records);
    println!(
        "  Outcome by color: Black {} ({:.1}%), White {} ({:.1}%), Draw {} ({:.1}%)",
        black,
        pct(black, n),
        white,
        pct(white, n),
        draw,
        pct(draw, n)
    );

    let (five, resign, capped, true_draw) = termination_mix(records);
    println!(
        "  Termination: Five {} ({:.1}%), Resign {} ({:.1}%), Capped {} ({:.1}%), Draw {} ({:.1}%)",
        five,
        pct(five, n),
        resign,
        pct(resign, n),
        capped,
        pct(capped, n),
        true_draw,
        pct(true_draw, n)
    );

    let (decided, never) = decision_plies(records);
    if decided.is_empty() {
        println!("  Decision ply: none of the games decided");
    } else {
        println!(
            "  Decision ply (n={}): median={}; never decided={}",
            decided.len(),
            percentile(&decided, 50),
            never
        );
    }

    let (tss_plies, total_plies) = tss_share(records);
    println!(
        "  TSS share: {}/{} plies = {:.2}%",
        tss_plies,
        total_plies,
        if total_plies == 0 {
            0.0
        } else {
            (tss_plies as f64 / total_plies as f64) * 100.0
        }
    );

    let (distinct, reached) = diversity_at_ply10(records);
    println!(
        "  Diversity at ply 10: {distinct} distinct prefixes / {reached} games reaching ply 10 = {:.1}%",
        if reached == 0 {
            0.0
        } else {
            (distinct as f64 / reached as f64) * 100.0
        }
    );

    println!();
}

fn describe_distribution(values: &[usize]) -> String {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    let min = sorted[0];
    let max = sorted[n - 1];
    let mean = values.iter().sum::<usize>() as f64 / n as f64;
    let median = percentile(&sorted, 50);
    let p95 = percentile(&sorted, 95);
    format!("min={min} median={median} mean={mean:.1} p95={p95} max={max}")
}

/// Return the element at the requested percentile from an already-sorted slice.
/// Uses the nearest-rank definition: the value at index `ceil(p/100 * n) - 1`.
fn percentile(sorted: &[usize], p: usize) -> usize {
    let n = sorted.len();
    if n == 0 {
        return 0;
    }
    let rank = ((p as f64 / 100.0) * n as f64).ceil() as usize;
    let rank = rank.clamp(1, n);
    sorted[rank - 1]
}

fn pct(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        (part as f64 / whole as f64) * 100.0
    }
}

fn color_split(records: &[GameRecord]) -> (usize, usize, usize) {
    let mut black = 0;
    let mut white = 0;
    let mut draw = 0;
    for r in records {
        match r.outcome {
            Outcome::Won { winner, .. } => match winner {
                engine::Color::Black => black += 1,
                engine::Color::White => white += 1,
            },
            Outcome::Draw => draw += 1,
        }
    }
    (black, white, draw)
}

fn termination_mix(records: &[GameRecord]) -> (usize, usize, usize, usize) {
    let mut five = 0;
    let mut resign = 0;
    let mut capped = 0;
    let mut true_draw = 0;

    for r in records {
        match r.outcome {
            Outcome::Won {
                reason: WinReason::FiveOrMore,
                ..
            } => five += 1,
            Outcome::Won {
                reason: WinReason::Resignation(_),
                ..
            } => resign += 1,
            Outcome::Draw => {
                // Distinguish a true board-fill draw from a ply-capped game.
                let board = replay(&r.plies);
                if board.status() == Status::Draw {
                    true_draw += 1;
                } else {
                    capped += 1;
                }
            }
        }
    }

    (five, resign, capped, true_draw)
}

/// Replay the recorded plies on a fresh board.  Panics on an illegal move,
/// which should never happen in a well-formed corpus.
fn replay(plies: &[RecordedPly]) -> Board {
    let mut board = Board::new();
    for ply in plies {
        board
            .play(ply.mv)
            .expect("recorded move should replay legally");
    }
    board
}

fn decision_plies(records: &[GameRecord]) -> (Vec<usize>, usize) {
    let mut decided = Vec::new();
    let mut never = 0;

    for r in records {
        if let Some(ply) = first_sustained_decision(&r.plies) {
            decided.push(ply);
        } else {
            never += 1;
        }
    }

    decided.sort_unstable();
    (decided, never)
}

/// The first ply index such that every ply from that index onward is
/// "decided": either `root_value` is absent (TSS/proof) or its absolute value
/// is at least 0.9.  Returns `None` if no such trailing run exists.
fn first_sustained_decision(plies: &[RecordedPly]) -> Option<usize> {
    let mut start_of_run: Option<usize> = None;
    for (i, ply) in plies.iter().enumerate().rev() {
        let decided = ply.root_value.is_none_or(|v| v.abs() >= 0.9);
        if decided {
            start_of_run = Some(i);
        } else {
            break;
        }
    }
    start_of_run
}

fn tss_share(records: &[GameRecord]) -> (usize, usize) {
    let mut tss = 0;
    let mut total = 0;
    for r in records {
        for ply in &r.plies {
            total += 1;
            if ply.root_value.is_none() {
                tss += 1;
            }
        }
    }
    (tss, total)
}

fn diversity_at_ply10(records: &[GameRecord]) -> (usize, usize) {
    let mut reached = 0;
    let mut prefixes = HashSet::new();
    for r in records {
        if r.plies.len() >= 10 {
            reached += 1;
            let prefix: Vec<engine::Move> = r.plies.iter().take(10).map(|p| p.mv).collect();
            prefixes.insert(prefix);
        }
    }
    (prefixes.len(), reached)
}
