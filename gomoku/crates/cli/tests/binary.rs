//! The `gomoku` binary, end to end: the real process with piped stdin
//! and stdout.
//!
//! A pipe is not a terminal, so every session here runs the line-mode
//! fallback: plain text frames out, complete lines in. That is what
//! makes the binary testable without a terminal — sub-chapter e's
//! selection rule, exercised through the operating system this time.
//! The assertions read stable substrings (seat labels, event texts,
//! the panel's phase and game-over lines), not whole frames: the
//! byte-exact frame contract already lives in `tests/line_mode.rs`.

use std::io::Write;
use std::process::{Command, Output, Stdio};

/// Run the `gomoku` binary with `args`, feed `script` to its stdin,
/// and return the captured output.
///
/// `CARGO_BIN_EXE_gomoku` is the path of the binary target; cargo
/// sets it when it builds the integration tests. The scripts here fit
/// in one pipe buffer, so writing before waiting cannot deadlock. A
/// process that exits early (a usage error) closes its end of the
/// pipe before reading, so the write error is ignored — those tests
/// assert on the exit code and stderr, not on the script.
fn run_gomoku(args: &[&str], script: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gomoku"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the gomoku binary spawns");
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let _ = stdin.write_all(script.as_bytes());
    // Closing stdin sends EOF; the line-mode loop reads EOF as quit.
    drop(stdin);
    child
        .wait_with_output()
        .expect("the binary runs to completion")
}

/// The captured stdout as text.
fn stdout_text(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

/// The captured stderr as text.
fn stderr_text(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

#[test]
fn selfplay_steps_at_empty_lines_and_terminates_at_eof() {
    // Both seats AI. An empty line steps the side to move, so three
    // empty lines play three moves. The script has no `quit` line:
    // end of input terminates the session.
    let args = ["selfplay", "--sims", "25", "--seed", "7"];
    let script = "\n\n\n";
    let first = run_gomoku(&args, script);
    assert!(
        first.status.success(),
        "selfplay exits successfully: {}",
        stderr_text(&first)
    );
    let text = stdout_text(&first);
    // The seat labels prove the search flags reached both seats.
    assert!(text.contains("Black: AI(25)"), "the Black seat: {text}");
    assert!(text.contains("White: AI(25)"), "the White seat: {text}");
    assert!(text.contains("Moved"), "the AI played moves: {text}");
    assert!(text.contains("by Ai"), "the mover is the AI seat: {text}");
    assert!(text.contains("Ply: 3"), "three steps happened: {text}");

    // The seed makes the run reproducible: a second process prints
    // the same transcript, byte for byte.
    let second = run_gomoku(&args, script);
    assert_eq!(
        first.stdout, second.stdout,
        "the same seed reproduces the session"
    );
}

#[test]
fn play_against_the_ai_supports_a_scripted_short_win() {
    // `--ai tactics` seats the AI in White (the default human seat is
    // Black); `--no-swap2` skips the ceremony. Black plays five on
    // row 8. The White replies are typed parking moves: `play`
    // forces a move for the side to move regardless of the seat's
    // occupant, and the session credits it to the AI seat. A script
    // cannot out-calculate the tactics evaluator, and it does not
    // have to — the AI's own moves are covered by the selfplay test.
    let output = run_gomoku(
        &["play", "--ai", "tactics", "--no-swap2"],
        "h8\na1\ni8\na2\nj8\na3\nk8\na4\nl8\nquit\n",
    );
    assert!(
        output.status.success(),
        "the session exits successfully: {}",
        stderr_text(&output)
    );
    let text = stdout_text(&output);
    assert!(text.contains("Black: Human"), "the human seat: {text}");
    assert!(text.contains("White: AI(400)"), "the AI seat: {text}");
    assert!(
        text.contains("Moved h8 by Human"),
        "the human's first move: {text}"
    );
    assert!(
        text.contains("Moved a1 by Ai"),
        "the typed White reply is credited to the AI seat: {text}"
    );
    assert!(
        text.contains("Moved l8 by Human"),
        "the winning move: {text}"
    );
    assert!(
        text.contains("Game over: Black wins"),
        "the panel's game-over line: {text}"
    );
    assert!(
        text.contains("Game over: Won(Black)"),
        "the game-over event: {text}"
    );
}

#[test]
fn play_no_swap2_starts_at_an_empty_board() {
    let output = run_gomoku(&["play", "--no-swap2"], "h8\ni8\n");
    assert!(
        output.status.success(),
        "the session exits successfully: {}",
        stderr_text(&output)
    );
    let text = stdout_text(&output);
    assert!(
        text.contains("Black: Human") && text.contains("White: Human"),
        "no --ai means hotseat: {text}"
    );
    assert!(
        text.contains("Moved h8 by Human") && text.contains("Moved i8 by Human"),
        "both moves played: {text}"
    );
    assert!(text.contains("Ply: 2"), "two plies happened: {text}");
    assert!(!text.contains("Swap2"), "the ceremony did not run: {text}");
    // The script has no `quit` line: end of input terminated the
    // session.
}

#[test]
fn the_default_invocation_runs_the_swap2_ceremony() {
    // No subcommand at all: `play` is the default, and the ceremony
    // is the default opening. The script runs the full protocol with
    // typed commands: alternated placements, an undo, the
    // count-validated `done`, the declined first choice, the second
    // round, and the final choice. The `k8` afterwards is the first
    // move of the game.
    let script = "play h8\n\
                  play i8\n\
                  back\n\
                  play i8\n\
                  play j8\n\
                  done\n\
                  more\n\
                  play g8\n\
                  play f8\n\
                  done\n\
                  white\n\
                  k8\n\
                  quit\n";
    let output = run_gomoku(&[], script);
    assert!(
        output.status.success(),
        "the session exits successfully: {}",
        stderr_text(&output)
    );
    let text = stdout_text(&output);
    assert!(
        text.contains("Swap2 opening in progress"),
        "the hint names the ceremony: {text}"
    );
    // The phases appear in protocol order across the frames.
    let mut at = 0;
    for phase in [
        "Swap2: Placing3",
        "Swap2: FirstChoice",
        "Swap2: Placing2",
        "Swap2: FinalChoice",
    ] {
        let found = text[at..]
            .find(phase)
            .unwrap_or_else(|| panic!("{phase} missing after byte {at}: {text}"));
        at += found + phase.len();
    }
    assert!(
        text.contains("Swap2 finished: White chosen"),
        "the ceremony finished: {text}"
    );
    // After the ceremony the game runs from the opening board, White
    // to move; `k8` is that first move.
    assert!(
        text.contains("Moved k8 by Human"),
        "the game accepts moves after the ceremony: {text}"
    );
    assert!(text.contains("Ply: 1"), "one game ply happened: {text}");
}

#[test]
fn color_without_ai_is_a_usage_error() {
    let output = run_gomoku(&["play", "--color", "white"], "");
    assert_eq!(
        output.status.code(),
        Some(2),
        "usage errors exit with code 2: {}",
        stderr_text(&output)
    );
    assert!(
        stderr_text(&output).contains("--color requires --ai"),
        "the error names the rule: {}",
        stderr_text(&output)
    );
}

#[test]
fn color_in_selfplay_is_a_usage_error() {
    let output = run_gomoku(&["selfplay", "--color", "black"], "");
    assert_eq!(
        output.status.code(),
        Some(2),
        "usage errors exit with code 2: {}",
        stderr_text(&output)
    );
    assert!(
        stderr_text(&output).contains("--color is not valid for selfplay"),
        "the error names the rule: {}",
        stderr_text(&output)
    );
}

#[test]
fn no_swap2_does_not_reach_selfplay() {
    // Before the subcommand the flag parses into the top-level play
    // args and the binary rejects it. After the subcommand clap
    // rejects it as unknown to selfplay. Both exits are code 2.
    let before = run_gomoku(&["--no-swap2", "selfplay"], "");
    assert_eq!(before.status.code(), Some(2));
    assert!(
        stderr_text(&before).contains("--no-swap2 is only valid with the 'play' subcommand"),
        "the error names the rule: {}",
        stderr_text(&before)
    );
    let after = run_gomoku(&["selfplay", "--no-swap2"], "");
    assert_eq!(after.status.code(), Some(2));
}

#[test]
fn an_invalid_flag_value_is_a_clap_usage_error() {
    // Clap's own parse errors use the same exit code as the binary's
    // validation: the shell sees one convention.
    let output = run_gomoku(&["play", "--sims", "abc"], "");
    assert_eq!(output.status.code(), Some(2));
}
