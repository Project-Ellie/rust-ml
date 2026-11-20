use engine::{Color, Move, Transform};
use player::PlayerOptions;
use selfplay::corpus::write_corpus;
use selfplay::export::ExportManifest;
use selfplay::game::{Game, Outcome, PlyRecord, PlySearch, WinReason};
use selfplay::mix::{SourceMixer, WeightedSource};
use selfplay::opening::{Opening, OpeningAnswer, OpeningProvenance};
use selfplay::record::GameRecord;
use selfplay::sample::{SampleError, augment_sample, position_is_holdout, replay_sample};
use selfplay::sampler::CorpusSampler;
use selfplay::schedule::ExplorationSchedule;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn mv(row: u8, col: u8) -> Move {
    Move::new(row, col).expect("test coordinate is on the board")
}

fn six_ply_record() -> GameRecord {
    let game = Game {
        seed: 17,
        schedule: ExplorationSchedule::default(),
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
                search: Some(PlySearch {
                    side_to_move: Color::White,
                    visits: vec![(mv(10, 11), 1)],
                    prior: vec![(mv(10, 11), 1.0)],
                    root_value: 0.25,
                }),
            },
            PlyRecord {
                mv: mv(5, 7),
                search: Some(PlySearch {
                    side_to_move: Color::Black,
                    visits: vec![(mv(5, 7), 1)],
                    prior: vec![(mv(5, 7), 1.0)],
                    root_value: 0.25,
                }),
            },
            PlyRecord {
                mv: mv(10, 12),
                search: Some(PlySearch {
                    side_to_move: Color::White,
                    visits: vec![(mv(10, 12), 1)],
                    prior: vec![(mv(10, 12), 1.0)],
                    root_value: 0.25,
                }),
            },
            PlyRecord {
                mv: mv(5, 8),
                search: Some(PlySearch {
                    side_to_move: Color::Black,
                    visits: vec![(mv(5, 8), 1)],
                    prior: vec![(mv(5, 8), 1.0)],
                    root_value: 0.25,
                }),
            },
            PlyRecord {
                mv: mv(10, 13),
                search: Some(PlySearch {
                    side_to_move: Color::White,
                    visits: vec![(mv(10, 13), 1)],
                    prior: vec![(mv(10, 13), 1.0)],
                    root_value: 0.25,
                }),
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
        certified_suffix: None,
    };
    GameRecord::from_game(&game)
}

#[test]
fn replay_at_ply_has_opening_and_played_stones_with_the_right_side_to_move() {
    let sample = replay_sample(&six_ply_record(), 3).expect("fabricated record replays");
    assert_eq!(sample.black, vec![mv(5, 5), mv(5, 6), mv(5, 7)]);
    assert_eq!(sample.white, vec![mv(10, 10), mv(10, 11), mv(10, 12)]);
    assert_eq!(sample.to_move, Color::Black);
}

#[test]
fn raw_visits_become_a_sparse_normalized_policy() {
    let mut record = six_ply_record();
    record.plies[0].visits = vec![(mv(10, 11), 3), (mv(0, 0), 1), (mv(14, 14), 2)];

    let sample = replay_sample(&record, 0).expect("fabricated record replays");
    assert_eq!(
        sample.policy,
        vec![
            (mv(10, 11), 0.5),
            (mv(0, 0), 1.0 / 6.0),
            (mv(14, 14), 1.0 / 3.0)
        ]
    );

    let mass: f32 = sample.policy.iter().map(|(_, p)| p).sum();
    assert!(
        (mass - 1.0).abs() < 1e-6,
        "policy mass {mass} is not within 1e-6 of 1"
    );
}

#[test]
fn a_ply_without_search_uses_the_played_move_as_its_policy() {
    let record = six_ply_record();
    assert!(
        record.plies[5].visits.is_empty(),
        "fixture ply 5 must have empty visits"
    );

    let sample = replay_sample(&record, 5).expect("TSS-bypassed ply replays");
    assert_eq!(sample.policy, vec![(record.plies[5].mv, 1.0)]);
}

#[test]
fn nonempty_zero_visit_histogram_is_rejected_instead_of_producing_nan() {
    let mut record = six_ply_record();
    record.plies[0].visits = vec![(mv(0, 0), 0), (mv(1, 1), 0)];

    let err = replay_sample(&record, 0).expect_err("zero total visits must fail");
    assert!(matches!(err, SampleError::ZeroVisitTotal { ply: 0 }));
}

#[test]
fn outcome_value_uses_each_position_side_to_move_perspective() {
    let record = six_ply_record();

    let sample0 = replay_sample(&record, 0).expect("fabricated record replays");
    assert_eq!(sample0.to_move, Color::White);
    assert_eq!(sample0.value, -1.0);

    let sample1 = replay_sample(&record, 1).expect("fabricated record replays");
    assert_eq!(sample1.to_move, Color::Black);
    assert_eq!(sample1.value, 1.0);

    let mut draw_record = record.clone();
    draw_record.outcome = Outcome::Draw;

    assert_eq!(
        replay_sample(&draw_record, 0)
            .expect("fabricated record replays")
            .value,
        0.0
    );
    assert_eq!(
        replay_sample(&draw_record, 1)
            .expect("fabricated record replays")
            .value,
        0.0
    );
}

#[test]
fn d4_augmentation_transforms_stones_and_policy_together() {
    let mut record = six_ply_record();
    record.plies[0].visits = vec![(mv(0, 0), 3), (mv(10, 11), 1)];

    let canonical = replay_sample(&record, 0).expect("fabricated record replays");
    let augmented = augment_sample(&canonical, Transform::Rot90);

    let expected_black: Vec<Move> = canonical
        .black
        .iter()
        .map(|m| Transform::Rot90.transform_move(*m))
        .collect();
    let expected_white: Vec<Move> = canonical
        .white
        .iter()
        .map(|m| Transform::Rot90.transform_move(*m))
        .collect();

    assert_eq!(augmented.black, expected_black);
    assert_eq!(augmented.white, expected_white);
    assert_eq!(
        augmented.policy,
        vec![
            (Transform::Rot90.transform_move(mv(0, 0)), 0.75),
            (Transform::Rot90.transform_move(mv(10, 11)), 0.25),
        ]
    );
    assert_eq!(augmented.policy[0].0, mv(14, 0));
    assert_eq!(augmented.to_move, canonical.to_move);
    assert_eq!(augmented.value, canonical.value);
}

#[test]
fn holdout_uses_the_canonical_position_zobrist_and_train_partition() {
    let record = six_ply_record();
    let sample = replay_sample(&record, 2).expect("fabricated record replays");

    let board = sample.board().expect("sample rebuilds into a legal board");
    let expected = board.zobrist() % 100 < train::split::HOLDOUT_PERCENTAGE;

    assert_eq!(train::split::is_holdout(&sample), expected);
    assert_eq!(
        position_is_holdout(&record, 2).expect("fabricated record replays"),
        expected
    );
}

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
        let path = std::env::temp_dir().join(format!("selfplay-read-path-{pid}-{nanos}"));
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

#[test]
fn corpus_sampler_is_deterministic_under_one_seed() {
    let records = vec![six_ply_record()];

    let left: Vec<_> = CorpusSampler::new(records.clone(), 42)
        .expect("nonempty corpus")
        .take(64)
        .collect::<Result<_, _>>()
        .expect("all fabricated draws replay");

    let right: Vec<_> = CorpusSampler::new(records, 42)
        .expect("nonempty corpus")
        .take(64)
        .collect::<Result<_, _>>()
        .expect("all fabricated draws replay");

    assert_eq!(left, right);

    for sample in &left {
        let mass: f32 = sample.policy.iter().map(|(_, p)| p).sum();
        assert!(
            (mass - 1.0).abs() < 1e-6,
            "policy mass {mass} is not within 1e-6 of 1"
        );
    }
}

#[test]
fn corpus_sampler_loads_a_corpus_directory() {
    let dir = TestDir::new();
    let record = six_ply_record();
    let corpus_dir = dir.path().join("corpus");
    write_corpus(&[record], &corpus_dir, 2).expect("fabricated corpus writes");

    let samples: Vec<_> = CorpusSampler::from_corpus(&corpus_dir, 42)
        .expect("directory corpus loads")
        .take(4)
        .collect::<Result<_, _>>()
        .expect("directory-backed draws replay");

    assert_eq!(samples.len(), 4);
}

#[test]
fn export_command_writes_samples_that_the_real_train_reader_loads() {
    let dir = TestDir::new();
    let record = six_ply_record();
    let corpus_dir = dir.path().join("corpus");
    let dataset_dir = dir.path().join("dataset");

    write_corpus(std::slice::from_ref(&record), &corpus_dir, 2).expect("fabricated corpus writes");

    let output = Command::new(env!("CARGO_BIN_EXE_selfplayexport"))
        .arg("--corpus")
        .arg(&corpus_dir)
        .arg("--out")
        .arg(&dataset_dir)
        .arg("--shard-capacity")
        .arg("2")
        .output()
        .expect("selfplayexport runs");

    assert!(
        output.status.success(),
        "selfplayexport failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // The samples.
    let expected: Vec<_> = (0..6)
        .map(|ply| replay_sample(&record, ply).expect("fabricated record replays"))
        .collect();
    let read_back =
        train::dataset::read_dataset(&dataset_dir).expect("nettrain's reader loads export");
    assert_eq!(read_back, expected);

    // The split survives the round trip.
    for (ply, sample) in read_back.iter().enumerate() {
        assert_eq!(
            train::split::is_holdout(sample),
            position_is_holdout(&record, ply).expect("fabricated record replays")
        );
    }

    // The manifest.
    let manifest_path = dataset_dir.join("manifest.json");
    let manifest_json =
        std::fs::read_to_string(&manifest_path).expect("export manifest was written");
    let manifest: ExportManifest =
        serde_json::from_str(&manifest_json).expect("export manifest is valid JSON");
    assert_eq!(manifest.game_count, 1);
    assert_eq!(manifest.sample_count, 6);
    assert_eq!(
        manifest.shards,
        vec!["shard-000.bin", "shard-001.bin", "shard-002.bin"]
    );
}

#[test]
fn source_mixer_respects_weights_over_a_large_seeded_draw() {
    let marker = |value| train::Sample {
        black: vec![],
        white: vec![],
        to_move: Color::Black,
        policy: vec![(mv(0, 0), 1.0)],
        value,
    };

    let mut mixer = SourceMixer::new(
        vec![
            WeightedSource::new(1, std::iter::repeat(marker(-1.0))),
            WeightedSource::new(3, std::iter::repeat(marker(1.0))),
        ],
        7,
    )
    .expect("nonzero source weights");

    let draws: Vec<_> = (&mut mixer).take(4_000).collect();

    let negatives = draws.iter().filter(|s| s.value == -1.0).count();
    let positives = draws.iter().filter(|s| s.value == 1.0).count();

    assert_eq!(negatives, 1_000);
    assert_eq!(positives, 3_000);
}
