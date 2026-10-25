# Sub-chapter 05f — The binary: solution

> **Opt-in solution — read only after you have tried the sub-chapter, or
> if you have been stuck for more than twenty minutes.**
>
> This file quotes the code that sub-chapter f adds or changes, as
> annotated snippets with line numbers. Line numbers refer to the files
> after the sub-chapter is applied. Deletions are described in prose;
> raw diffs are not shown. Verified green: `cargo test -p cli`,
> `cargo clippy --all-targets -- -D warnings`, and
> `cargo fmt --all -- --check` all pass from `gomoku/`. The two
> scripted demo transcripts (sub-chapter a's checklist) are
> byte-identical to before.

Reading order: the binary (`src/main.rs`), then the README, then the
capstone tests (`tests/binary.rs`).

## src/main.rs (replaced, 270 lines)

The whole file is new code. The chapter-01 skeleton — the clap
structs with a `main` that printed "Function coming soon. Be patient"
for every invocation — is deleted. The module doc (lines 1–10) states
the contract: all session logic lives in the `cli` library; the
binary parses, validates, builds a `SessionInit`, and dispatches into
`study::run`. The terminal or line-mode selection happens inside
`study::run`, not in the binary.

### The clap structure

`CliArgs` carries the flattened `PlayArgs` next to the optional
subcommand: that combination is what makes `play` the default. Flags
before the subcommand token parse into the top-level struct; the
field doc says so.

src/main.rs, lines 18–43:

```rust
#[derive(Parser, Debug, Clone)]
#[command(
    name = "gomoku",
    about = "Terminal Gomoku client",
    subcommand_required = false,
    arg_required_else_help = false
)]
struct CliArgs {
    /// The `play` flags at the top level: `play` is the default
    /// subcommand, so `gomoku --no-swap2` means `gomoku play
    /// --no-swap2`.
    #[command(flatten)]
    play: PlayArgs,

    #[command(subcommand)]
    cmd: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Play at the study table: the Swap2 opening first, unless
    /// --no-swap2.
    Play(PlayArgs),
    /// AI-vs-AI self-play; Enter steps the search forward.
    Selfplay(SearchArgs),
}
```

The variant is `Selfplay`, not `SelfPlay`: clap kebab-cases variant
names, and `SelfPlay` would spell the subcommand `self-play`. The doc
comments on the variants become the subcommand help text.

`PlayArgs` (lines 45–54) is the flattened `SearchArgs` plus the
`--no-swap2` boolean. `SearchArgs` (lines 55–97) holds the search
flags. Two details matter. The `sims` and `c_puct` flags carry
`default_value_t`, because their defaults are plain values. The
`temperature` and `noise` flags do not: they are `Option<f32>`, so an
explicit flag is distinguishable from the default. The doc comments
name the consequence.

src/main.rs, lines 74–84:

```rust
    /// Move-selection temperature (default 0.0). An explicit value
    /// marks the selection as user-configured, which disables the
    /// opening schedule in AI-vs-AI sessions.
    #[arg(long, value_name = "F")]
    temperature: Option<f32>,

    /// Dirichlet noise weight (default 0.0). An explicit value marks
    /// the selection as user-configured, which disables the opening
    /// schedule in AI-vs-AI sessions.
    #[arg(long, value_name = "F")]
    noise: Option<f32>,
```

`AiKind` and `PlayerColor` (lines 99–110) are the two value enums,
`rename_all = "lower"`, one `Tactics` variant and two color variants.

### The dispatch

`main` (lines 112–144) is one match. The `None` arm plays with the
top-level flags. The `Play` arm merges a top-level `--no-swap2` by
boolean union — unambiguous, so the flag applies wherever it was
placed. The `Selfplay` arm rejects the same flag instead of dropping
it.

src/main.rs, lines 112–144:

```rust
fn main() {
    let args = CliArgs::parse();

    // No subcommand at all is `play` with the top-level flags.
    let result = match args.cmd {
        None => run_play(&args.play),
        Some(Commands::Play(mut play)) => {
            // A `--no-swap2` placed before the subcommand parses into
            // the flattened top-level play args. The union of two
            // booleans is unambiguous, so the flag applies wherever it
            // was placed. (Search flags cannot be merged this way —
            // two values would compete — so they belong after the
            // subcommand.)
            play.no_swap2 |= args.play.no_swap2;
            run_play(&play)
        }
        Some(Commands::Selfplay(search)) => {
            // For selfplay the same flattened parse would silently
            // drop a `--no-swap2` placed before the subcommand. That
            // flag has no meaning for selfplay, so it is rejected
            // instead of ignored.
            if args.play.no_swap2 {
                usage_error("--no-swap2 is only valid with the 'play' subcommand");
            }
            run_selfplay(&search)
        }
    };

    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
```

`usage_error` (lines 146–151) prints to stderr and exits with code 2
— the code clap itself uses for command-line errors, so the shell
sees one convention. Runtime errors from `study::run` exit with code
1.

### The two runners

The validation rules live next to the dispatch that needs them.
`run_play` rejects `--color` without `--ai` and builds the hint only
when the ceremony runs; `run_selfplay` rejects `--color`.

src/main.rs, lines 153–178:

```rust
/// `play`: validate the seat flags, build the session, enter the
/// study table.
fn run_play(play: &PlayArgs) -> std::io::Result<()> {
    // A human seat color is meaningful only when the opposite seat is
    // AI. Without --ai both seats are human and the flag would
    // silently do nothing; reject it instead.
    if play.search.color.is_some() && play.search.ai.is_none() {
        usage_error("--color requires --ai");
    }
    let swap2 = !play.no_swap2;
    let init = play_init(&play.search, swap2);
    // The hint is the first message line of the session. The ceremony
    // panel shows the phase details; the hint only names what the user
    // is looking at.
    let hint = swap2.then_some("Swap2 opening in progress. '?' shows the help.");
    study::run(init, hint)
}

/// `selfplay`: both seats AI; Enter steps the search forward.
fn run_selfplay(search: &SearchArgs) -> std::io::Result<()> {
    // Both seats are AI, so a human seat color has no meaning.
    if search.color.is_some() {
        usage_error("--color is not valid for selfplay");
    }
    study::run(selfplay_init(search), Some("Enter steps the AI."))
}
```

### The SessionInit builders

Both builders use struct literals, not the `SessionInit`
constructors. The constructors cover the standard seat assignments;
the binary needs per-seat simulation overrides on the initial AI
seats, which no constructor expresses. Note what `play_init` stores
even for a hotseat: the search flags survive as `default_config` plus
the per-seat overrides, so a `release` command mid-session builds its
AI config from them.

src/main.rs, lines 187–210:

```rust
fn play_init(search: &SearchArgs, swap2: bool) -> SessionInit {
    let base = ai_config_from(search);
    let (black, white) = match search.ai {
        Some(AiKind::Tactics) => {
            let human = search.color.map(player_color).unwrap_or(Color::Black);
            let ai = Occupant::Ai(seat_config(&base, human.other(), search));
            match human {
                Color::Black => (Occupant::Human, ai),
                Color::White => (ai, Occupant::Human),
            }
        }
        None => (Occupant::Human, Occupant::Human),
    };
    SessionInit {
        black,
        white,
        default_config: base,
        black_sims: search.black_sims,
        white_sims: search.white_sims,
        seed: search.seed,
        opening: Board::new(),
        swap2,
    }
}
```

The ceremony is one field: `swap2` set on the init. Sub-chapter c put
the ceremony inside the session, so the binary has no ceremony code
at all — the reference front-end still ran the Swap2 as a separate
program that returned a board, and its `run_with_swap2` entry point
is gone.

`selfplay_init` (lines 215–227) seats the AI in both seats, each with
its own config, and sets `swap2: false`. `ai_config_from` (lines
237–246) applies the explicit-selection rule:

src/main.rs, lines 237–246:

```rust
fn ai_config_from(search: &SearchArgs) -> AiConfig {
    AiConfig {
        evaluator: EvaluatorKind::Tactics,
        simulations: search.sims,
        c_puct: search.c_puct,
        temperature: search.temperature.unwrap_or(0.0),
        noise_epsilon: search.noise.unwrap_or(0.0),
        explicit_selection: search.temperature.is_some() || search.noise.is_some(),
    }
}
```

`seat_config` (lines 252–262) applies one seat's simulation override
to a clone of the base config — the same request the `sims` command
makes mid-session, made here at startup. `player_color` (lines
265–270) maps the CLI color to the engine color.

A note on size: `main.rs` is 270 lines, past the chapter's
approximate 250-line budget. The bulk is clap declarations and their
doc text, which is the user-facing surface of this sub-chapter. The
file holds one concern — wiring — which is the budget's stated
purpose.

## README.md (rewritten, 144 lines)

The old README described the deleted legacy UI: 0-based coordinate
input, the puzzle mode, the `--engine` and `--plain` flags. All of it
is retired. The new README documents the study table. Sections:

* **Build** — unchanged, including the `naive-engine` feature note.
* **Play at the study table** — the invocations:

  ```bash
  cargo run -p cli --bin gomoku                      # Swap2 opening, then the study table
  cargo run -p cli --bin gomoku -- play              # same, spelled out
  cargo run -p cli --bin gomoku -- play --no-swap2   # start from an empty board
  cargo run -p cli --bin gomoku -- play --ai tactics # the AI takes White
  cargo run -p cli --bin gomoku -- play --ai tactics --color white  # the AI takes Black
  ```

  Every invocation names the binary: the crate has three binaries, so
  plain `cargo run -p cli` is ambiguous and cargo rejects it.
* **The Swap2 opening** — the protocol in three sentences, then the
  ceremony commands (`play <square> [black|white]`, `back`, `done`,
  `black` / `white`, `more`).
* **The full-screen shell** — the normal-mode key table and the `:`
  command line.
* **Study commands** — the typed language table.
* **Line mode** — the pipe behavior and the EOF contract, with the
  `printf` example.
* **Search flags** — the flag table with defaults, and the
  opening-schedule note: without explicit `--temperature` or
  `--noise`, AI-vs-AI from an empty board plays the first ten plies
  with temperature 1.0 and Dirichlet noise.
* **Self-play**, **Headless demos**, **Coordinate system** —
  algebraic notation, `h8` is the centre.

## tests/binary.rs (new, 259 lines, 8 tests)

The module doc (lines 1–9) names the two facts the file stands on: a
pipe is not a terminal, so every session runs the line-mode fallback;
and the assertions read stable substrings, because the byte-exact
frame contract already lives in `tests/line_mode.rs`.

### The harness

One function spawns the binary, feeds the script, and captures the
output. The script fits in one pipe buffer, so writing before waiting
cannot deadlock. Closing stdin sends EOF, which the line-mode loop
reads as quit.

tests/binary.rs, lines 24–39:

```rust
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
```

The ignored write error is deliberate: a process that exits early (a
usage error) closes its end of the pipe before reading, and those
tests assert on the exit code and stderr, not on the script.
`stdout_text` and `stderr_text` (lines 42–49) decode the captures.

### The session tests

`selfplay_steps_at_empty_lines_and_terminates_at_eof` (lines 51–79)
runs `selfplay --sims 25 --seed 7` with three empty lines and no
`quit`. The assertions: exit success, both seat labels show
`AI(25)`, moves happen (`Moved`, `by Ai`), `Ply: 3`. Then the
determinism check — a second process with the same seed must print
identical bytes:

```rust
    // The seed makes the run reproducible: a second process prints
    // the same transcript, byte for byte.
    let second = run_gomoku(&args, script);
    assert_eq!(
        first.stdout, second.stdout,
        "the same seed reproduces the session"
    );
```

`play_against_the_ai_supports_a_scripted_short_win` (lines 81–122)
runs `play --ai tactics --no-swap2`. Black plays five on row 8; the
White replies are typed parking moves. The comment in the test
explains why: `play` forces a move for the side to move regardless of
the seat's occupant, and the session credits it to the AI seat — a
script cannot out-calculate the tactics evaluator, and does not have
to. The assertions pin the seat labels (`Black: Human`,
`White: AI(400)`), the credited moves (`Moved a1 by Ai`), and both
game-over lines (`Game over: Black wins` on the panel, `Game over:
Won(Black)` as the event text).

`play_no_swap2_starts_at_an_empty_board` (lines 124–145) is the
negative control for the ceremony: two hotseat moves, `Ply: 2`, and
no "Swap2" anywhere in the transcript. It ends at EOF.

`the_default_invocation_runs_the_swap2_ceremony` (lines 147–203)
passes no subcommand at all: the default path runs the ceremony. The
script is the full protocol — a default placement, a color-token
override, an undo, the count-validated `done`, `more`, the second
round, the final choice — followed by one game move. The phases must
appear in protocol order, asserted with the forward-scan idiom from
`tests/line_mode.rs`:

```rust
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
```

The test also asserts the ceremony hint, the finish event
(`Swap2 finished: White chosen`), and that the game accepts moves
afterwards (`Moved k8 by Human`, `Ply: 1`).

### The usage-error tests

Four tests (lines 205–259) pin the exit-code convention: `--color`
without `--ai`, `--color` in selfplay, `--no-swap2` before `selfplay`
plus clap's own rejection of `--no-swap2` after it, and an invalid
flag value. One example of the shape:

```rust
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
```

## Deviations from the design paper's wording and the reference

Four, all documented in the code where they apply:

1. **Per-seat overrides are applied to the initial AI seats.** The
   reference stored `--black-sims` / `--white-sims` on the
   `SessionInit` without applying them to the initial seat configs,
   so `selfplay --black-sims 800` started both seats at the same
   simulation count until a `release`. The session's own `sims`
   command updates the stored override and the live config together;
   the flag now makes the same request at startup, and the panel's
   seat labels tell the truth from the first frame.
2. **A top-level `--no-swap2` merges into the `play` subcommand.**
   The reference ignored the flattened top-level args when the `play`
   subcommand was given, silently dropping `gomoku --no-swap2 play`.
   A boolean union is unambiguous, so the flag applies. Misplaced
   search flags remain undetectable (they parse into the top-level
   struct and look like defaults); the tutorial's mental-mapping
   section states this limit.
3. **`--temperature` / `--noise` are `Option`s again.** The
   chapter-01 skeleton declared them with `default_value_t = 0.0`,
   which predates chapter 04's `explicit_selection` and would have
   made an explicit `--temperature 0.0` indistinguishable from no
   flag. The reference already had the `Option` shape; the skeleton's
   regression is corrected.
4. **The ceremony runs with a hint.** The reference passed no hint;
   `run_play` passes one when the ceremony starts, because the
   ceremony is now the session's opening phase and the message line
   is free at startup. Selfplay's hint changed wording ("Enter steps
   the AI."), not meaning.

The reference's `run_with_swap2` entry point is not a deviation but a
deletion: sub-chapter c moved the ceremony into the session, and
`study::run(init, hint)` is the only entry point.

## Verification

From `gomoku/`, after the sub-chapter:

* `cargo test -p cli` — 152 tests pass (144 before): 87 lib tests, 8
  new binary integration tests, and the unchanged suites (7 actor,
  9 history, 3 line-mode, 20 opening, 13 session, 4 study,
  1 doctest). The binary tests run in about 0.3 s.
* `cargo clippy --all-targets -- -D warnings` — clean.
* `cargo fmt --all -- --check` — clean.
* Both demo transcripts are byte-identical to before. Sessiondemo
  does not import `main.rs`; the repl code is untouched.
* Manual smoke tests through the built binary (line mode, piped):
  `play --no-swap2` plays and quits at EOF; the bare invocation runs
  the full ceremony and then accepts game moves;
  `selfplay --sims 25 --seed 7` steps on empty lines and two runs
  print identical bytes; all three usage errors exit with code 2 and
  name their rule on stderr.
