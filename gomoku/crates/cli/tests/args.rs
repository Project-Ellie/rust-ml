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
