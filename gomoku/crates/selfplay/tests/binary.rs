use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use engine::{Color, Move};
use player::PlayerOptions;
use selfplay::anchor::AnchorSeed;
use selfplay::corpus::write_corpus;
use selfplay::game::{Game, Outcome, PlyRecord, WinReason};
use selfplay::opening::{Opening, OpeningAnswer, OpeningProvenance};
use selfplay::record::{CertifiedSuffix, GameRecord};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new() -> Self {
        let pid = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("selfplayanchors-test-{pid}-{nanos}-{sequence}"));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn mv(row: u8, col: u8) -> Move {
    Move::new(row, col).unwrap()
}

fn certified_record() -> GameRecord {
    let game = Game {
        seed: 17,
        black_options: PlayerOptions::default(),
        white_options: PlayerOptions::default(),
        opening: Opening::new(
            [mv(5, 5), mv(10, 10), mv(5, 6)],
            OpeningAnswer::Take(Color::Black),
            OpeningProvenance::Generated,
        ),
        plies: vec![
            PlyRecord {
                mv: mv(10, 11),
                search: None,
            },
            PlyRecord {
                mv: mv(5, 7),
                search: None,
            },
            PlyRecord {
                mv: mv(10, 12),
                search: None,
            },
            PlyRecord {
                mv: mv(5, 8),
                search: None,
            },
            PlyRecord {
                mv: mv(10, 13),
                search: None,
            },
            PlyRecord {
                mv: mv(5, 9),
                search: None,
            },
        ],
        outcome: Outcome::Won {
            winner: Color::Black,
            reason: WinReason::FiveOrMore,
        },
        certified_suffix: Some(CertifiedSuffix {
            from_ply: 5,
            line: vec![mv(5, 9)],
        }),
    };
    GameRecord::from_game(&game)
}

#[test]
fn selfplaygen_help_documents_corpus_and_grading_flags() {
    let binary = env!("CARGO_BIN_EXE_selfplaygen");
    let output = Command::new(binary)
        .arg("--help")
        .output()
        .expect("selfplaygen --help spawns");

    assert!(output.status.success(), "--help exits successfully");
    let text = String::from_utf8(output.stdout).expect("help output is UTF-8");

    for flag in [
        "--black-sims",
        "--white-sims",
        "--games",
        "--seed-base",
        "--shard-capacity",
        "--out",
        "--openings",
    ] {
        assert!(text.contains(flag), "help text missing '{flag}'");
    }
}

#[test]
fn selfplayanchors_help_documents_corpus_extraction() {
    let binary = env!("CARGO_BIN_EXE_selfplayanchors");
    let output = Command::new(binary)
        .arg("--help")
        .output()
        .expect("selfplayanchors --help runs");

    assert!(output.status.success(), "--help exits successfully");
    let text = String::from_utf8(output.stdout).expect("help output is UTF-8");

    for flag in ["--corpus", "--out"] {
        assert!(text.contains(flag), "help text missing '{flag}'");
    }
    assert!(
        text.contains("verify_line"),
        "help text must name the soundness gate"
    );
}

#[test]
fn selfplayanchors_reads_a_corpus_and_writes_json_anchor_seeds() {
    let dir = TestDir::new();
    let record = certified_record();
    let corpus_dir = dir.path().join("corpus");
    write_corpus(&[record], &corpus_dir, 1).expect("fabricated corpus writes");

    let out = dir.path().join("anchors.json");
    let binary = env!("CARGO_BIN_EXE_selfplayanchors");
    let output = Command::new(binary)
        .arg("--corpus")
        .arg(&corpus_dir)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("selfplayanchors spawns");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let text = std::fs::read_to_string(&out).expect("anchor JSON was written");
    let seeds: Vec<AnchorSeed> = serde_json::from_str(&text).expect("anchor output is valid JSON");
    assert_eq!(
        seeds,
        vec![AnchorSeed {
            game_index: 0,
            from_ply: 5,
            line: vec![mv(5, 9)],
        }]
    );
}

#[test]
fn selfplayverify_rejects_a_corpus_without_game_records() {
    let dir = TestDir::new();
    let corpus_dir = dir.path().join("corpus");
    std::fs::create_dir_all(&corpus_dir).expect("empty corpus directory exists");

    let binary = env!("CARGO_BIN_EXE_selfplayverify");
    let output = Command::new(binary)
        .arg("--corpus")
        .arg(&corpus_dir)
        .output()
        .expect("selfplayverify runs");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "stderr: {stderr}");
    assert!(stderr.contains("no game records"), "stderr: {stderr}");
}

#[test]
fn selfplayverify_rejects_an_illegal_recorded_move() {
    let dir = TestDir::new();
    let mut record = certified_record();
    record.plies[1].mv = mv(5, 5);
    let corpus_dir = dir.path().join("corpus");
    write_corpus(&[record], &corpus_dir, 1).expect("fabricated corpus writes");

    let binary = env!("CARGO_BIN_EXE_selfplayverify");
    let output = Command::new(binary)
        .arg("--corpus")
        .arg(&corpus_dir)
        .output()
        .expect("selfplayverify runs");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "stderr: {stderr}");
    assert!(stderr.contains("ply 1"), "stderr: {stderr}");
    assert!(stderr.contains("occupied"), "stderr: {stderr}");
}

#[test]
fn selfplayverify_rejects_a_recorded_outcome_that_disagrees_with_the_engine() {
    let dir = TestDir::new();
    let mut record = certified_record();
    record.outcome = Outcome::Won {
        winner: Color::White,
        reason: WinReason::FiveOrMore,
    };
    let corpus_dir = dir.path().join("corpus");
    write_corpus(&[record], &corpus_dir, 1).expect("fabricated corpus writes");

    let binary = env!("CARGO_BIN_EXE_selfplayverify");
    let output = Command::new(binary)
        .arg("--corpus")
        .arg(&corpus_dir)
        .output()
        .expect("selfplayverify runs");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "stderr: {stderr}");
    assert!(stderr.contains("final status"), "stderr: {stderr}");
    assert!(stderr.contains("game 0"), "stderr: {stderr}");
}

#[test]
fn selfplayverify_reports_replay_and_corpus_statistics_as_json() {
    let dir = TestDir::new();
    let record = certified_record();
    let corpus_dir = dir.path().join("corpus");
    write_corpus(&[record], &corpus_dir, 1).expect("fabricated corpus writes");

    let binary = env!("CARGO_BIN_EXE_selfplayverify");
    let output = Command::new(binary)
        .arg("--corpus")
        .arg(&corpus_dir)
        .output()
        .expect("selfplayverify runs");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let summary: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("summary is JSON");

    assert_eq!(summary["game_count"], 1);
    assert_eq!(summary["verified_plies"], 6);
    assert_eq!(summary["certified_suffixes"], 1);
    assert_eq!(summary["outcomes"]["black"], 1);
    assert_eq!(summary["outcomes"]["white"], 0);
    assert_eq!(summary["outcomes"]["draw"], 0);
    assert_eq!(summary["length_histogram_plies"]["6"], 1);
    assert_eq!(summary["games_ending_within_16_plies"], 1);
    assert_eq!(summary["opening_offers"]["Generated"], 1);
}
