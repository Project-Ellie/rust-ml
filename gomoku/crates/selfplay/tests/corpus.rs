use engine::{Color, Move};
use player::PlayerOptions;
use selfplay::corpus::{
    CorpusWriteOptions, OverwritePolicy, SeatEvaluators, check_writable, read_corpus,
    read_manifest, write_corpus, write_corpus_with_options,
};
use selfplay::game::{Game, Outcome, PlyRecord, PlySearch, WinReason};
use selfplay::opening::{Opening, OpeningAnswer, OpeningProvenance};
use selfplay::record::{CertifiedSuffix, GAME_RECORD_FORMAT_VERSION, GameRecord};
use selfplay::schedule::ExplorationSchedule;
use std::path::{Path, PathBuf};
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
            "selfplay-corpus-{name}-{}-{nanos}-{sequence}",
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
    Move::new(row, col).expect("Test coordinates are on the board")
}

fn minimal_game(seed: u64) -> Game {
    let opening = Opening::new(
        [mv(5, 5), mv(10, 10), mv(5, 6)],
        OpeningAnswer::Take(Color::Black),
        OpeningProvenance::Listed {
            list: "test".to_owned(),
            index: 3,
        },
    );

    let moves = [
        mv(10, 11),
        mv(5, 7),
        mv(10, 12),
        mv(5, 8),
        mv(10, 13),
        mv(5, 9),
    ];

    let plies = moves
        .into_iter()
        .enumerate()
        .map(|(ply, mv)| PlyRecord {
            mv,
            search: (ply != 5).then(|| PlySearch {
                side_to_move: if ply % 2 == 0 {
                    Color::White
                } else {
                    Color::Black
                },
                visits: vec![(mv, 1)],
                prior: vec![
                    (mv, 0.75),
                    (Move::new(0, ply as u8).expect("on the board."), 0.25),
                ],
                root_value: ply as f32 / 10.0,
            }),
        })
        .collect();

    Game {
        seed,
        schedule: ExplorationSchedule::default(),
        black_options: PlayerOptions {
            simulations: 1,
            ..PlayerOptions::default()
        },
        white_options: PlayerOptions {
            simulations: 2,
            ..PlayerOptions::default()
        },
        opening,
        plies,
        outcome: Outcome::Won {
            winner: Color::Black,
            reason: WinReason::FiveOrMore,
        },
        certified_suffix: None,
    }
}

#[test]
fn game_record_preserves_sparse_visits_including_the_played_move() {
    let game = minimal_game(17);
    let record = GameRecord::from_game(&game);

    for (recorded, played) in record.plies.iter().zip(&game.plies) {
        if played.search.is_some() {
            assert!(!recorded.visits.is_empty());
            assert!(recorded.visits.iter().any(|(visit, _)| *visit == played.mv));
        } else {
            assert!(recorded.visits.is_empty());
        }
    }
}

#[test]
fn game_record_preserves_a_certified_suffix() {
    let game = minimal_game(17);
    let suffix = CertifiedSuffix {
        from_ply: 0,
        line: game.moves().collect(),
    };
    let game = Game {
        certified_suffix: Some(suffix.clone()),
        ..game
    };

    let record = GameRecord::from_game(&game);

    assert_eq!(record.certified_suffix, Some(suffix));
}

#[test]
fn manifest_records_pairing_and_seed_set_and_round_trips() {
    let dir = TestDir::new("manifest");
    let records = [minimal_game(4), minimal_game(17)].map(|game| GameRecord::from_game(&game));

    let manifest = write_corpus(&records, dir.path(), 2).expect("writing the corpus");

    assert_eq!(manifest.format_version, GAME_RECORD_FORMAT_VERSION);
    assert_eq!(manifest.seeds, vec![4, 17]);
    assert_eq!(manifest.pairing, records[0].header.config);
    assert_eq!(manifest.game_count, 2);
    assert_eq!(manifest.workspace_version, env!("CARGO_PKG_VERSION"));

    let read_back = read_manifest(dir.path()).expect("reading the manifest");
    assert_eq!(read_back, manifest);
}

#[test]
fn shard_capacity_two_rolls_over_and_manifest_lists_every_shard() {
    let dir = TestDir::new("rollover");
    let records: Vec<_> = (1..=5)
        .map(|seed| GameRecord::from_game(&minimal_game(seed)))
        .collect();

    let manifest = write_corpus(&records, dir.path(), 2).expect("writing the corpus");

    assert_eq!(
        manifest.shards,
        ["shard-000.bin", "shard-001.bin", "shard-002.bin"].map(String::from)
    );
    assert_eq!(manifest.game_count, 5);

    let read_back = read_corpus(dir.path()).expect("reading the corpus");
    assert_eq!(read_back, records);
}

#[test]
fn populated_search_fields_round_trip_byte_exactly() {
    let game = minimal_game(17);
    let record = GameRecord::from_game(&game);

    assert_eq!(record.header.format_version, GAME_RECORD_FORMAT_VERSION);
    assert_eq!(record.header.seed, game.seed);
    assert_eq!(record.header.config.black.simulations, 1);
    assert_eq!(record.header.config.white.simulations, 2);
    assert_eq!(
        record.header.evaluator,
        format!("baseline@{}", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(record.header.opening, game.opening);
    assert_eq!(record.outcome, game.outcome);
    assert_eq!(record.certified_suffix, None);
    assert_eq!(record.plies.len(), game.plies.len());
    assert!(record.plies.iter().any(|ply| !ply.prior.is_empty()));

    for (recorded, played) in record.plies.iter().zip(&game.plies) {
        assert_eq!(recorded.mv, played.mv);
        match &played.search {
            Some(search) => {
                assert_eq!(recorded.visits, search.visits);
                assert_eq!(recorded.prior, search.prior);
                assert_eq!(recorded.root_value, Some(search.root_value));
            }
            None => {
                assert!(recorded.visits.is_empty());
                assert!(recorded.prior.is_empty());
                assert_eq!(recorded.root_value, None);
            }
        }
    }

    // Write → read → write, then compare bytes: the format must be stable
    // under re-serialization, not just value-equal.
    let first = TestDir::new("roundtrip-first");
    let second = TestDir::new("roundtrip-second");
    write_corpus(std::slice::from_ref(&record), first.path(), 1).expect("first write");
    let read_back = read_corpus(first.path()).expect("reading back");
    assert_eq!(read_back, [record]);
    write_corpus(&read_back, second.path(), 1).expect("second write");

    for name in ["shard-000.bin", "manifest.json"] {
        let first_bytes = std::fs::read(first.path().join(name)).expect("first file");
        let second_bytes = std::fs::read(second.path().join(name)).expect("second file");
        assert_eq!(first_bytes, second_bytes, "{name} must be byte-identical");
    }
}

#[test]
fn write_refuses_a_non_empty_corpus_directory() {
    let dir = TestDir::new("refuse");
    std::fs::create_dir_all(dir.path()).expect("creating the output directory");
    std::fs::write(dir.path().join("keep.txt"), "precious").expect("placing the file");
    let record = GameRecord::from_game(&minimal_game(1));

    let error = write_corpus(std::slice::from_ref(&record), dir.path(), 1)
        .expect_err("a non-empty directory must be refused");

    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        std::fs::read(dir.path().join("keep.txt")).expect("keep.txt survives"),
        b"precious"
    );
    assert!(!dir.path().join("shard-000.bin").exists());
    assert!(!dir.path().join("manifest.json").exists());
}

#[test]
fn explicit_overwrite_replaces_the_previous_corpus() {
    let dir = TestDir::new("overwrite");
    let initial: Vec<_> = (1..=3)
        .map(|seed| GameRecord::from_game(&minimal_game(seed)))
        .collect();
    write_corpus(&initial, dir.path(), 1).expect("initial corpus");

    let replacement = GameRecord::from_game(&minimal_game(9));
    let manifest = write_corpus_with_options(
        std::slice::from_ref(&replacement),
        dir.path(),
        CorpusWriteOptions {
            shard_capacity: 2,
            overwrite: OverwritePolicy::Allow,
            evaluators: None,
        },
    )
    .expect("explicit overwrite");

    assert_eq!(manifest.seeds, vec![9]);
    assert_eq!(manifest.shards, ["shard-000.bin".to_owned()]);
    assert!(!dir.path().join("shard-001.bin").exists());
    let read_back = read_corpus(dir.path()).expect("reading the corpus");
    assert_eq!(read_back, [replacement]);
}

#[test]
fn invalid_explicit_overwrite_preserves_the_existing_corpus() {
    let dir = TestDir::new("invalid-overwrite");
    let initial = GameRecord::from_game(&minimal_game(3));
    write_corpus(std::slice::from_ref(&initial), dir.path(), 1).expect("initial corpus");

    let error = write_corpus_with_options(
        std::slice::from_ref(&initial),
        dir.path(),
        CorpusWriteOptions {
            shard_capacity: 0,
            overwrite: OverwritePolicy::Allow,
            evaluators: None,
        },
    )
    .expect_err("zero shard capacity is invalid");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    let manifest = read_manifest(dir.path()).expect("the existing corpus is intact");
    assert_eq!(manifest.seeds, vec![3]);
}

#[test]
fn corpus_writer_process_helper() {
    // Child-process entry point: passes trivially in every ordinary run and
    // writes the deterministic corpus only when the parent re-executes this
    // test binary with the output directory named in the environment.
    let Ok(output) = std::env::var("SELFPLAY_CORPUS_TEST_OUTPUT") else {
        return;
    };
    let records = [4, 17, 23].map(|seed| GameRecord::from_game(&minimal_game(seed)));
    write_corpus(&records, Path::new(&output), 2).expect("the child writes the corpus");
}

#[test]
fn identical_seeds_produce_byte_identical_corpora_across_process_restarts() {
    let first = TestDir::new("determinism-first");
    let second = TestDir::new("determinism-second");
    let binary = std::env::current_exe().expect("the test binary path");

    for dir in [&first, &second] {
        let status = std::process::Command::new(&binary)
            .args(["corpus_writer_process_helper", "--exact"])
            .env("SELFPLAY_CORPUS_TEST_OUTPUT", dir.path())
            .status()
            .expect("the child process runs");
        assert!(status.success());
    }

    let manifest = read_manifest(first.path()).expect("the first manifest");
    let first_manifest = std::fs::read(first.path().join("manifest.json")).expect("first bytes");
    let second_manifest = std::fs::read(second.path().join("manifest.json")).expect("second bytes");
    assert_eq!(
        first_manifest, second_manifest,
        "manifest.json must be byte-identical"
    );

    for shard in &manifest.shards {
        let first_bytes = std::fs::read(first.path().join(shard)).expect("first shard");
        let second_bytes = std::fs::read(second.path().join(shard)).expect("second shard");
        assert_eq!(first_bytes, second_bytes, "{shard} must be byte-identical");
    }
}

#[test]
fn per_seat_evaluator_pairing_lives_in_the_evaluator_string() {
    let game = minimal_game(7);

    let plain = GameRecord::from_game(&game);
    assert!(plain.header.evaluator.starts_with("baseline@"));

    let mixed =
        GameRecord::from_game_with_evaluators(&game, "net:first-corpus@0.1.0", "baseline@0.1.0");
    assert_eq!(
        mixed.header.evaluator,
        "black=net:first-corpus@0.1.0 white=baseline@0.1.0"
    );
    // Everything else about the record is untouched.
    assert_eq!(mixed.header.config, plain.header.config);
    assert_eq!(mixed.plies, plain.plies);
}

#[test]
fn manifest_records_seat_evaluators_only_when_declared() {
    let records: Vec<GameRecord> = (0..2)
        .map(|seed| GameRecord::from_game(&minimal_game(seed)))
        .collect();

    // Baseline corpora keep their previous manifest bytes: no new keys.
    let plain_dir = TestDir::new("manifest-plain");
    let plain = write_corpus(&records, plain_dir.path(), 1).expect("plain corpus writes");
    assert_eq!(plain.evaluators, None);
    let json = std::fs::read_to_string(plain_dir.path().join("manifest.json")).expect("manifest");
    assert!(!json.contains("evaluators"));

    // Net-seated corpora declare their per-seat evaluators in the manifest.
    let net_dir = TestDir::new("manifest-net");
    let manifest = write_corpus_with_options(
        &records,
        net_dir.path(),
        CorpusWriteOptions {
            shard_capacity: 1,
            overwrite: OverwritePolicy::Refuse,
            evaluators: Some(SeatEvaluators {
                black: "net:first-corpus@0.1.0".to_owned(),
                white: "baseline@0.1.0".to_owned(),
            }),
        },
    )
    .expect("net corpus writes");
    let evaluators = manifest.evaluators.clone().expect("evaluators recorded");
    assert_eq!(evaluators.black, "net:first-corpus@0.1.0");
    assert_eq!(evaluators.white, "baseline@0.1.0");

    // Older manifests without the field still parse.
    let read_back = read_manifest(plain_dir.path()).expect("plain manifest reads");
    assert_eq!(read_back, plain);
    let read_back = read_manifest(net_dir.path()).expect("net manifest reads");
    assert_eq!(read_back, manifest);
}

#[test]
fn check_writable_preflights_the_overwrite_gate() {
    let missing = TestDir::new("preflight-missing");
    check_writable(missing.path(), OverwritePolicy::Refuse).expect("a missing dir is writable");
    check_writable(missing.path(), OverwritePolicy::Allow).expect("a missing dir allows");

    let empty = TestDir::new("preflight-empty");
    std::fs::create_dir_all(empty.path()).expect("dir exists");
    check_writable(empty.path(), OverwritePolicy::Refuse).expect("an empty dir is writable");

    std::fs::write(empty.path().join("occupied"), "data").expect("marker file");
    let error = check_writable(empty.path(), OverwritePolicy::Refuse)
        .expect_err("a non-empty dir is refused");
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    check_writable(empty.path(), OverwritePolicy::Allow).expect("the allow policy passes");
}
