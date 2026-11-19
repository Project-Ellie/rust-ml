use std::process::Command;

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
