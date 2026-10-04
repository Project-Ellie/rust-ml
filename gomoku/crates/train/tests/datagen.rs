//! Integration test for the `datagen` binary.
//!
//! Runs the compiled binary with tiny quotas, verifies that it exits
//! successfully, writes a manifest and at least one shard, and that
//! every sample passes the train crate's soundness gate when read back.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use train::dataset::{check_soundness, read_dataset};
use train::shard::Manifest;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let pid = std::process::id();
    std::env::temp_dir().join(format!("{prefix}-{pid}-{nanos}"))
}

#[test]
fn datagen_runs_end_to_end_with_tiny_quotas() {
    let out_dir = unique_temp_dir("train-datagen-integration");
    let _ = fs::remove_dir_all(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_datagen"))
        .args([
            "--seed".as_ref(),
            "7".as_ref(),
            "--win".as_ref(),
            "2".as_ref(),
            "--block".as_ref(),
            "2".as_ref(),
            "--quiet".as_ref(),
            "3".as_ref(),
            "--out".as_ref(),
            out_dir.as_os_str(),
        ])
        .output()
        .expect("datagen binary should be runnable");

    assert!(output.status.success(), "datagen should exit with status 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Soundness gate passed"),
        "stderr should confirm the soundness gate passed"
    );
    assert!(
        stdout.contains("Stats {"),
        "stdout should contain the stats report"
    );

    let manifest_path = out_dir.join("manifest.json");
    assert!(manifest_path.exists(), "manifest.json should exist");

    let manifest_bytes = fs::read_to_string(&manifest_path).expect("manifest should be readable");
    let manifest: Manifest =
        serde_json::from_str(&manifest_bytes).expect("manifest should be valid JSON");
    assert_eq!(manifest.seed, 7);
    assert_eq!(manifest.quotas.win, 2);
    assert_eq!(manifest.quotas.block, 2);
    assert_eq!(manifest.quotas.quiet, 3);

    let shard_paths: Vec<_> = fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("shard-") && n.ends_with(".bin"))
                    .unwrap_or(false)
        })
        .collect();
    assert!(!shard_paths.is_empty(), "at least one shard should exist");

    let samples = read_dataset(&out_dir).expect("dataset should be readable");
    assert_eq!(
        samples.len(),
        manifest.quotas.win + manifest.quotas.block + manifest.quotas.quiet,
        "read-back sample count should match the requested quotas"
    );

    for (i, sample) in samples.iter().enumerate() {
        assert_eq!(
            check_soundness(sample),
            Ok(()),
            "sample {i} should pass the soundness gate"
        );
    }

    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn datagen_fails_without_required_out_flag() {
    let status = Command::new(env!("CARGO_BIN_EXE_datagen"))
        .args(["--seed", "1", "--win", "1", "--block", "1", "--quiet", "1"])
        .status()
        .expect("datagen binary should be runnable");

    assert!(
        !status.success(),
        "datagen should exit non-zero without --out"
    );
}

#[test]
fn datagen_rejects_unknown_flag() {
    let out_dir = unique_temp_dir("train-datagen-unknown-flag");
    let _ = fs::remove_dir_all(&out_dir);

    let status = Command::new(env!("CARGO_BIN_EXE_datagen"))
        .args([
            "--seed".as_ref(),
            "1".as_ref(),
            "--win".as_ref(),
            "1".as_ref(),
            "--block".as_ref(),
            "1".as_ref(),
            "--quiet".as_ref(),
            "1".as_ref(),
            "--out".as_ref(),
            out_dir.as_os_str(),
            "--nonsense".as_ref(),
        ])
        .status()
        .expect("datagen binary should be runnable");

    assert!(
        !status.success(),
        "datagen should exit non-zero on unknown flag"
    );
    let _ = fs::remove_dir_all(&out_dir);
}
