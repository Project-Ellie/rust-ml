# Sub-chapter 05f — The binary

Status: IMPLEMENTED. This sub-chapter replaces the chapter-01 clap
skeleton in `src/main.rs` with the real front-end, rewrites the
README as the user documentation, and adds the capstone tests. It
changes no library behavior.

## Abstract

Sub-chapters a–e built the study table. This sub-chapter opens it to
the user. `src/main.rs` parses the command line with clap, rejects
invalid flag combinations, builds a `SessionInit`, and calls
`study::run` — nothing else, because all the logic already exists in
the library. `play` is the default subcommand: the Swap2 ceremony
first unless `--no-swap2`, then the study table. `--ai` seats the AI
opposite the human, and `--color` selects the human seat. `selfplay`
seats the AI in both seats, and Enter steps the search forward. The
README retires the legacy-UI description and documents both
subcommands. The capstone drives the real binary through pipes: a
scripted selfplay with a fixed seed, a scripted win against the
tactics seat, a `--no-swap2` session, and a full Swap2 ceremony, plus
the usage errors.

## Glossary

| Term | Definition |
|------|------------|
| **Default subcommand** | `play` runs when no subcommand is given. Clap parses the flags into the top-level struct. |
| **Usage error** | An invalid flag combination. The message goes to stderr; the exit code is 2, the code clap itself uses. |
| **Explicit selection** | `--temperature` or `--noise` passed by the user. It sets `explicit_selection` on the `AiConfig`, which disables the opening schedule (chapter 04). |
| **Per-seat override** | `--black-sims` / `--white-sims`. Stored on the `SessionInit` and applied to the initial AI seat configs. |
| **`CARGO_BIN_EXE_<name>`** | The environment variable through which cargo hands the built binary's path to the integration tests. |
| **Capstone** | The end-to-end tests of the sub-chapter, driving the real binary through pipes. |

## Context

Read [the design paper](../00-design.md) first. Its section
"f — The binary" assigns this sub-chapter: the sub-chapters a–e
contain all the logic; this file only validates and dispatches.

Chapter 01 deleted the legacy hotseat/puzzle UI (owner decision,
2026-10-17: the study table is the single CLI) and left `main.rs` as
a clap skeleton with nothing to dispatch to. Chapters 02–04 built the
session core; sub-chapters a–e built the table on top of it. This
sub-chapter is the last one: it connects the user to the table.

The reference front-end (`git show 8847c4a:gomoku/crates/cli/src/main.rs`)
is the starting point, but it predates two things: chapter 04's
`explicit_selection` flag on `AiConfig`, and this redesign's
in-session Swap2 ceremony (the reference ran the ceremony as a
separate program that returned a board; here the ceremony is a
session phase, so `play` only sets `swap2: true` on the
`SessionInit`). The current code is the authority.

## Intention

1. Replace `src/main.rs`: the clap structs, the two subcommands, the
   validation, the `SessionInit` builders, and the dispatch into
   `study::run`.
2. Change `--temperature` and `--noise` from defaulted floats to
   `Option<f32>`, so an explicit flag is distinguishable from the
   default. An explicit flag sets `explicit_selection` on the
   `AiConfig`.
3. Rewrite `README.md`: `play`, `selfplay`, the ceremony, both input
   models, the search flags, the demos. No legacy UI, no skeleton
   wording.
4. Add `tests/binary.rs`: the capstone through the real binary, plus
   the usage errors.
5. Prove that nothing else changed: the three gates pass, and the two
   scripted demo transcripts stay byte-identical.

## Mental mapping

### The binary is wiring

`main.rs` has no session logic. It reads flags, rejects impossible
combinations, assembles one `SessionInit`, and calls `study::run`.
That is the whole file. When a test of this sub-chapter fails, the
defect is a wrong `SessionInit` or a wrong exit code — every other
behavior has a home in the library, with its own tests.

The construction is deliberately done with struct literals, not the
`SessionInit` constructors (`hotseat`, `versus_ai`, `ai_vs_ai`). The
constructors cover the standard seat assignments, and tests and demos
use them. The binary needs per-seat simulation overrides on the
initial AI seats, which no constructor expresses, so it builds the
struct field by field.

### Explicit means Option

Chapter 04 gave the session an opening schedule: AI-vs-AI from an
empty root plays the first ten plies with temperature 1.0 and
Dirichlet noise, to vary the opening. The schedule applies only when
the user did not configure the selection — that is what
`explicit_selection` records.

The chapter-01 skeleton declared `--temperature` and `--noise` with
`default_value_t = 0.0`. That destroys the information: a user who
typed `--temperature 0.0` is then indistinguishable from a user who
typed nothing, and the schedule would apply to a session the user
explicitly configured. The flags are `Option<f32>`: `None` means not
passed, `Some(_)` means explicit, and explicit sets the flag on the
`AiConfig`.

### The per-seat overrides mirror the `sims` command

The session's `sims black 800` command updates two things: the stored
Black override and the live Black seat config. The `--black-sims`
flag is the same request at startup, so the binary does both too: it
stores the override on the `SessionInit` (a `release` later applies
it again) and applies it to the initial AI seat config
(`--black-sims 800 --sims 200` plays an 800-simulation Black against
a 200-simulation White). The reference stored the overrides without
applying them to the initial seats; the deviation is deliberate, and
it makes the seat labels on the panel tell the truth from the first
frame.

### One exit-code convention

Clap exits with code 2 for command-line errors. The binary's own
validation uses the same code, so a script cannot tell the two apart
and does not need to: usage errors are 2. Runtime errors from the
terminal or the streams print the I/O error and exit with code 1.

Three rules are validated: `--color` requires `--ai` (a human seat
color is meaningless without an AI seat), `--color` is invalid in
`selfplay` (no human seat exists), and `--no-swap2` placed before
`selfplay` is rejected (see the next section).

### Where a flag lands when it precedes the subcommand

`play` is the default subcommand, implemented by flattening
`PlayArgs` into the top-level clap struct next to the optional
subcommand. This has a consequence: a flag placed *before* the
subcommand token parses into the top-level struct, not the
subcommand's. For the `play` subcommand the `--no-swap2` flags merge
by boolean union — the union of two booleans is unambiguous. For
`selfplay` a misplaced `--no-swap2` is rejected: the flag has no
meaning there, and silently dropping it would report success for a
session the user did not ask for.

Misplaced search flags (`gomoku --sims 200 selfplay`) cannot be
detected this way: they parse into the top-level struct and are
indistinguishable from defaults, because the same flags exist on both
subcommands. The README and the help text document the correct
placement: flags follow the subcommand.

### The capstone tests the binary, not the library

Sub-chapter e tested the line-mode loop in memory. The capstone runs
the real process: `env!("CARGO_BIN_EXE_gomoku")` is the binary's
path, piped stdin carries the script, and piped stdout is what the
assertions read. A pipe is not a terminal, so the selection rule from
sub-chapter e puts the session into line mode — the feature that
exists for pipes is what makes the binary testable.

Three properties of the shell matter to the harness. End of input
quits the session, so closing stdin terminates the process without a
`quit` line. The scripts are shorter than a pipe buffer, so writing
the whole script before waiting for the process cannot deadlock. And
the assertions read stable substrings — seat labels, event texts, the
phase lines, the game-over line — not whole frames: the byte-exact
frame contract already lives in `tests/line_mode.rs`, and duplicating
it would test the renderer twice.

The determinism assertion deserves a sentence of its own. Two
selfplay processes with the same seed must print the same transcript,
byte for byte. The session seeds its RNG from `--seed`, the tactics
evaluator is deterministic, and nothing in the output depends on
wall-clock time, so the bytes must be equal. If they ever differ, the
defect is real: an unseeded random source reached the session.

## Low-level design

### `src/main.rs` (replaced, 270 lines)

The module doc states the contract: the library holds the logic; the
binary parses, validates, builds a `SessionInit`, and dispatches.

The clap structure:

```rust
struct CliArgs {
    #[command(flatten)]
    play: PlayArgs,            // the default subcommand's flags
    #[command(subcommand)]
    cmd: Option<Commands>,
}

enum Commands {
    Play(PlayArgs),
    Selfplay(SearchArgs),      // the variant name spells `selfplay`
}

struct PlayArgs {
    #[command(flatten)]
    search: SearchArgs,
    no_swap2: bool,            // --no-swap2
}

struct SearchArgs {
    ai: Option<AiKind>,          // --ai tactics
    color: Option<PlayerColor>,  // --color black|white
    sims: u32,                   // --sims, default 400
    c_puct: f32,                 // --c-puct, default 1.5
    temperature: Option<f32>,    // --temperature, default 0.0
    noise: Option<f32>,          // --noise, default 0.0
    black_sims: Option<u32>,     // --black-sims
    white_sims: Option<u32>,     // --white-sims
    seed: Option<u64>,           // --seed
}
```

The dispatch in `main()`: no subcommand means `play` with the
top-level flags; the `Play` arm merges a top-level `--no-swap2` by
boolean union; the `Selfplay` arm rejects a top-level `--no-swap2`.
A returned I/O error prints to stderr and exits with code 1.

```rust
fn usage_error(message: &str) -> !;   // stderr + exit code 2

fn run_play(play: &PlayArgs) -> io::Result<()>;      // validates --color/--ai
fn run_selfplay(search: &SearchArgs) -> io::Result<()>; // rejects --color

fn play_init(search: &SearchArgs, swap2: bool) -> SessionInit;
fn selfplay_init(search: &SearchArgs) -> SessionInit;
fn ai_config_from(search: &SearchArgs) -> AiConfig;
fn seat_config(base: &AiConfig, seat: Color, search: &SearchArgs) -> AiConfig;
fn player_color(color: PlayerColor) -> Color;
```

`run_play` passes a hint only when the ceremony runs ("Swap2 opening
in progress. '?' shows the help."); the hint is the session's first
message line. `run_selfplay` passes "Enter steps the AI."

### `README.md` (rewritten)

Sections: build (with the `naive-engine` feature note), play at the
study table (with the ceremony and its commands), the full-screen
shell (the normal-mode key table), line mode, the study commands, the
search flags (with the opening-schedule note), self-play, the
headless demos, and the coordinate system. All invocations use
`cargo run -p cli --bin gomoku`: the crate has three binaries, so the
binary name is required.

### `tests/binary.rs` (new, 259 lines, 8 tests)

The harness is one function: `run_gomoku(args, script) -> Output`
spawns the binary, writes the script to stdin, closes stdin (EOF),
and waits. `stdout_text` and `stderr_text` decode the captures.

Four session tests:

1. `selfplay --sims 25 --seed 7` with three empty lines and no
   `quit`: both seat labels show `AI(25)`, moves happen, `Ply: 3`,
   EOF terminates, and a second run prints identical bytes.
2. `play --ai tactics --no-swap2` with a scripted Black win: the
   White replies are typed parking moves, credited to the AI seat —
   `play` forces a move for the side to move regardless of the seat's
   occupant. Asserts the seat labels, the moves, and both game-over
   lines.
3. `play --no-swap2` hotseat, two moves, EOF: both seats human, no
   "Swap2" anywhere.
4. No subcommand at all: the default invocation runs the Swap2
   ceremony. The script plays the full protocol (default and
   override placements, an undo, `done`, `more`, the second round,
   the final choice), then one game move. Asserts the hint, the four
   phase lines in protocol order, the finish event, and `Ply: 1`.

Four usage-error tests: `--color` without `--ai`, `--color` in
selfplay, `--no-swap2` before `selfplay` (and clap's own rejection of
`--no-swap2` after it), and an invalid flag value. All exit with
code 2.

## Solution (opt-in)

The added and changed code, as annotated snippets with line numbers,
is in [binary-solution.md](binary-solution.md). Open it only if you
have been stuck for more than twenty minutes, or after you have
finished the sub-chapter and want to compare your work.

## TDD checklist

The rhythm is red, green, extend. Run from `gomoku/`. Capture the
golden transcripts first (sub-chapter a's checklist); both diffs must
be empty at the end.

1. **Red.** Write `tests/binary.rs` against the skeleton. The session
   tests fail (the skeleton prints "Function coming soon" for every
   invocation), and the usage-error tests fail (the skeleton accepts
   every combination). The harness itself compiles: cargo sets
   `CARGO_BIN_EXE_gomoku` for the skeleton binary too.
2. **Green.** Replace `src/main.rs` with the design above. The eight
   tests pass.
3. **The README.** Rewrite it; no test reads it, so review it by
   running every command it quotes.
4. **The gates.** Run all three gates and both transcript diffs.

## Verification commands

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Plus the transcript capture and diff from sub-chapter a's checklist.
Sessiondemo does not use `main.rs`; both diffs must be empty.

One manual check beyond the gates, in a real terminal:

```text
cargo run -p cli --bin gomoku -- play --ai tactics
```

The full-screen shell draws the board and the ceremony panel; the
keys act; `q` restores the terminal.

## Done when

* `gomoku play` (or no subcommand) runs the Swap2 ceremony and then
  the study table; `--no-swap2` starts at an empty board.
* `gomoku play --ai tactics [--color white]` seats the AI opposite
  the human; `gomoku selfplay` seats the AI in both seats, and Enter
  steps.
* `--color` without `--ai`, `--color` in selfplay, and a misplaced
  `--no-swap2` are usage errors with exit code 2.
* `--temperature` / `--noise` are `Option`s; an explicit flag sets
  `explicit_selection`.
* The README documents both subcommands and both input models, and
  every command it quotes runs.
* All three gates pass, and both demo transcripts are unchanged.
  (Result here: 152 tests, up from 144. `tests/binary.rs` adds 8.)

Next: [Chapter 06 — Seeing the mind](../../06-seeing-the-mind.md)

## References

* [00-design.md](../00-design.md) — the contract; section f assigns
  this sub-chapter.
* [e-fallback/fallback.md](../e-fallback/fallback.md) — the selection
  rule that makes the binary testable through pipes, and the EOF
  contract the harness relies on.
* [c-swap2/swap2.md](../c-swap2/swap2.md) — the in-session ceremony
  that `play` starts with `swap2: true`.
* [a-refactor/refactor.md](../a-refactor/refactor.md) — the
  transcript regression checklist.
* [04-the-study-loop.md](../../04-the-study-loop.md) — the opening
  schedule and the `explicit_selection` semantics.
* clap's derive documentation: <https://docs.rs/clap>.
