//! Weighted multi-corpus export through `selfplayexport`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use engine::{Color, Move};
use player::PlayerOptions;
use selfplay::corpus::{read_manifest, write_corpus};
use selfplay::export::ExportManifest;
use selfplay::game::{Game, Outcome, PlyRecord, PlySearch, WinReason};
use selfplay::opening::{Opening, OpeningAnswer, OpeningProvenance};
use selfplay::record::GameRecord;
use selfplay::schedule::ExplorationSchedule;

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let sequence = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "selfplay-mix-{name}-{}-{nanos}-{sequence}",
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
    Move::new(row, col).expect("test coordinates are on the board")
}

/// The minimal legal fabricated game: a Take ceremony and six plies,
/// Black's five in row 5, columns 5–9.
fn minimal_record(seed: u64) -> GameRecord {
    let moves = [
        mv(10, 11),
        mv(5, 7),
        mv(10, 12),
        mv(5, 8),
        mv(10, 13),
        mv(5, 9),
    ];
    let game = Game {
        seed,
        schedule: ExplorationSchedule::default(),
        black_options: PlayerOptions {
            simulations: 1,
            ..PlayerOptions::default()
        },
        white_options: PlayerOptions::default(),
        opening: Opening::new(
            [mv(5, 5), mv(10, 10), mv(5, 6)],
            OpeningAnswer::Take(Color::Black),
            OpeningProvenance::Listed {
                list: "test".to_owned(),
                index: 3,
            },
        ),
        plies: moves
            .into_iter()
            .map(|mv| PlyRecord {
                mv,
                search: Some(PlySearch {
                    side_to_move: Color::Black,
                    visits: vec![(mv, 1)],
                    prior: vec![(mv, 0.75)],
                    root_value: 0.0,
                }),
            })
            .collect(),
        outcome: Outcome::Won {
            winner: Color::Black,
            reason: WinReason::FiveOrMore,
        },
        certified_suffix: None,
    };
    GameRecord::from_game(&game)
}

fn selfplayexport() -> &'static str {
    env!("CARGO_BIN_EXE_selfplayexport")
}

#[test]
fn two_corpora_mix_with_weights_and_record_their_sources() {
    let corpus_a = TestDir::new("a");
    let corpus_b = TestDir::new("b");
    let out = TestDir::new("out");
    write_corpus(&[minimal_record(1), minimal_record(2)], corpus_a.path(), 10)
        .expect("corpus a writes");
    write_corpus(&[minimal_record(3)], corpus_b.path(), 10).expect("corpus b writes");

    let status = Command::new(selfplayexport())
        .args(["--corpus", &format!("{}:2", corpus_a.path().display())])
        .args(["--corpus", &corpus_b.path().display().to_string()])
        .args(["--out", &out.path().display().to_string()])
        .status()
        .expect("the export binary runs");
    assert!(status.success());

    let manifest = read_export_manifest(out.path());
    assert_eq!(manifest.game_count, 3);
    assert_eq!(manifest.sample_count, 18, "three six-ply games");
    let sources = manifest.sources.expect("a mixed export records sources");
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0].weight, 2);
    assert_eq!(sources[0].games, 2);
    assert_eq!(sources[1].weight, 1);

    let dataset = train::dataset::read_dataset(out.path()).expect("dataset reads back");
    assert_eq!(dataset.len(), 18);
}

#[test]
fn a_zero_weight_source_is_rejected() {
    let corpus = TestDir::new("zero");
    let out = TestDir::new("zero-out");
    write_corpus(&[minimal_record(1)], corpus.path(), 10).expect("corpus writes");

    let output = Command::new(selfplayexport())
        .args(["--corpus", &format!("{}:0", corpus.path().display())])
        .args(["--out", &out.path().display().to_string()])
        .output()
        .expect("the export binary runs");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("weight"), "'{stderr}' names the weight");
}

fn read_export_manifest(dir: &Path) -> ExportManifest {
    let text = std::fs::read_to_string(dir.join("manifest.json")).expect("manifest reads");
    serde_json::from_str(&text).expect("manifest parses")
}

#[test]
fn a_single_unweighted_corpus_exports_exactly_as_before() {
    let corpus = TestDir::new("plain");
    let out = TestDir::new("plain-out");
    let record = minimal_record(7);
    write_corpus(std::slice::from_ref(&record), corpus.path(), 10).expect("corpus writes");

    let status = Command::new(selfplayexport())
        .args(["--corpus", &corpus.path().display().to_string()])
        .args(["--out", &out.path().display().to_string()])
        .status()
        .expect("the export binary runs");
    assert!(status.success());

    let manifest = read_export_manifest(out.path());
    assert_eq!(manifest.sources, None, "no mixing, no source list");
    assert_eq!(manifest.game_count, 1);
    assert_eq!(manifest.sample_count, 6);

    // The corpus manifest of the source is untouched provenance.
    let source = read_manifest(corpus.path()).expect("source manifest reads");
    assert_eq!(source.seeds, vec![7]);
}
