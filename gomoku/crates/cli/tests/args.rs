//! Command-line flag contract tests for the `gomoku` binary.
//!
//! These pin clap-level behavior: invalid flag combinations exit with
//! code 2 before the session starts.

use std::process::Command;

/// Run the `gomoku` binary with `args` and return the captured output.
///
/// `CARGO_BIN_EXE_gomoku` is the path of the binary target; cargo
/// sets it when it builds the integration tests.
fn run_gomoku(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gomoku"))
        .args(args)
        .output()
        .expect("the gomoku binary runs")
}

#[test]
fn selfplay_rejects_puzzle_flag() {
    let output = run_gomoku(&["selfplay", "--puzzle", "x.json"]);
    assert!(!output.status.success(), "expected failure");
    assert_eq!(
        output.status.code(),
        Some(2),
        "exit code should be 2, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn index_requires_puzzle_flag() {
    let output = run_gomoku(&["play", "--index", "2"]);
    assert!(!output.status.success(), "expected failure");
    assert_eq!(
        output.status.code(),
        Some(2),
        "exit code should be 2, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(feature = "baseline")]
#[test]
fn play_ai_baseline_parses() {
    let output = run_gomoku(&["play", "--ai", "baseline", "--no-swap2"]);
    assert!(
        output.status.success(),
        "baseline AI kind should parse, but got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn resign_flag_parses_in_default_play_and_selfplay() {
    for args in [
        vec!["--resign", "--no-swap2"],
        vec!["play", "--resign", "--ai", "tactics", "--no-swap2"],
        vec!["selfplay", "--resign", "--ai", "tactics"],
    ] {
        let output = run_gomoku(&args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn resign_sessiondemo_load_prints_concession_status() {
    use cli::session::{SessionCore, SessionInit};
    use std::io::Write;
    use std::process::Stdio;
    let core = SessionCore::new(SessionInit::hotseat());
    let mut value: serde_json::Value =
        serde_json::from_str(&core.export_record_json().unwrap()).unwrap();
    value["version"] = 2.into();
    value["history"]["nodes"][0]["resignation"] = serde_json::json!({
        "reason": "verified_opponent_vcf", "by": "Black", "candidates": 5
    });
    let dir = std::env::temp_dir().join(format!("gomoku-resign-demo-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("Study.json");
    std::fs::write(&path, value.to_string()).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_sessiondemo"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(
        child.stdin.take().unwrap(),
        "load {}\ntake black\nquit",
        path.display()
    )
    .unwrap();
    let output = child.wait_with_output().unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Black resigned: 5 top candidates"), "{text}");
    assert!(text.contains("heuristic; other moves unchecked"));
    assert!(
        text.contains("Historical/resumed: Black resigned"),
        "{text}"
    );
    assert!(text.contains("To move: Black"), "{text}");
}

#[test]
fn play_ai_rejects_invalid_value() {
    let output = run_gomoku(&["play", "--ai", "invalid", "--no-swap2"]);
    assert!(!output.status.success(), "expected failure");
    assert_eq!(
        output.status.code(),
        Some(2),
        "Exit code should be 2, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
