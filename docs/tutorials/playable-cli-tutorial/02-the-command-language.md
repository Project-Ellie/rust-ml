# Chapter 02 — The command language

## Abstract

This chapter wires the study table's **headless brain**. You will define
`Command` and `Event` as the shared vocabulary between every front-end,
build a line parser that turns text into typed commands, and implement
`SessionCore` — a state machine that takes commands in and emits events
and snapshots out. There is no AI yet: both seats are human, and
`Step`, `Release`, and `SetSims` return a typed
`AiSeatsNotImplemented` error. That is deliberate API-first design: the
contract is complete, the intelligence lands in chapter 03.

The deeper point is **REST-readiness**. `SessionCore::execute` does not
care whether the `Command` came from a terminal line, a JSON POST body,
or a future GUI websocket. The same `Vec<Event>` + `Snapshot` response
works for all of them. The command language is the protocol the web
server will one day speak.

## Glossary

| Term | Definition |
|------|------------|
| **Command** | A typed intent issued to `SessionCore` (`Play`, `Step`, `Back`, `Take`, …). The only way to change core state. |
| **Event** | A fact about what changed (`Moved`, `Branched`, `CursorMoved`, …). The only thing `execute` returns. |
| **SeatSel** | A seat selector: `Black`, `White`, or `Both`. Used by `Take`, `Release`, and `SetSims`. |
| **CommandError** | A typed error returned by the parser and by `execute`. Never a panic, never a print. |
| **SessionCore** | The headless state machine: history tree, seat table, RNG, and `execute`/`snapshot`/`board`. |
| **SessionInit** | Constructor input for `SessionCore`: occupants, AI defaults, per-seat sims overrides, seed, opening board. |
| **Snapshot** | Serde-serializable read model of the session for renderers. |
| **API-first design** | Defining the full public contract before all implementations are ready; `Step`/`Release`/`SetSims` return `AiSeatsNotImplemented` in Task 2. |
| **Errors-as-data** | Illegal input and illegal state are returned as values, not logged, not printed, and never panicked on. |

## Context

Chapter 01 gave you the variation tree (`History`): silent branching,
`back`, `forward`, `var`, and replay-to-cursor. This chapter puts that
tree inside a session and wraps it in a command language.

The primer established the core/shell split: a headless library plus a
thin terminal front-end. This chapter builds the library side. The bin
gets one new demo, `sessiondemo`, which is just a stdin loop over the
public API. Every behavior is testable by feeding command sequences to
`SessionCore` and asserting on the event stream.

## Intention

1. Create `src/command.rs` with `Command`, `Event`, `SeatSel`,
   `CommandError`, and a table-driven `parse` function.
2. Create `src/session.rs` with `SessionInit`, `Snapshot`, and
   `SessionCore`. Support hotseat play, navigation, branching,
   `New`, and `Take`.
3. Make `Step`, `Release`, and `SetSims` return typed
   `CommandError::AiSeatsNotImplemented` errors — the AI arrives in
   chapter 03, but the API is already stable.
4. Create `src/bin/sessiondemo.rs`: a REPL that reads lines, parses
   them, executes them, and prints events + an ASCII board.
5. Create `tests/session.rs` with headless integration tests.

Observable done-state: `cargo run -q -p cli --bin sessiondemo` plays a
full hotseat game from a script; `cargo test -p cli` is green; the
crate's clippy and fmt gates pass.

## Mental mapping

### The command/event vocabulary is the protocol

The old CLI had ad-hoc terminal input. The study table has a vocabulary
that any front-end can speak:

| Front-end | Speaks |
|-----------|--------|
| Terminal (`sessiondemo`, later `gomoku play`) | Text lines → `parse` |
| JSON/REST handler (future) | `serde` of `Command` + `Snapshot` |
| GUI (future) | Same `Command` enum, different transport |

Because the parser lives in the **library**, every front-end shares the
same grammar and the same error variants. The core never reads stdin; the
shell never peeks inside the core.

### Snapshot exists so renderers render from data

`Snapshot` is a plain read model: ply, branch options, seat kinds,
status, last move. The TUI (chapter 05) and any future JSON endpoint get
the same data shape. Rendering from `Snapshot` and `Event` keeps the
shell thin and keeps the core testable without terminal mocking.

### The core owns a seeded RNG already

`SessionInit` carries `seed: Option<u64>`. The core constructs a
`StdRng` from it, even though Task 2 has no AI to use it. That seed is
what will make chapter 03's `Step` reproducible and chapter 07's golden
transcripts bit-stable. Defining it now avoids a constructor breaking
change later.

### `board()` replays root→cursor

`SessionCore::board()` delegates to `History::board()`, which replays the
path from the root to the cursor. The cost is at most 225 plies on a
15×15 board — microseconds. The payoff is simplicity: the cursor can sit
anywhere in the tree and the board is always correct.

## Low-level design

### Files you will create or modify

```text
gomoku/crates/cli/
├── src/
│   ├── lib.rs              (add pub mod command, session, actor, fmt)
│   ├── command.rs          (new: Command, Event, parser, CommandError)
│   ├── session.rs          (new: SessionInit, Snapshot, SessionCore)
│   ├── actor.rs            (new: Occupant, OccupantKind, AiConfig)
│   ├── fmt.rs              (new: ASCII board + move formatting helpers)
│   └── bin/sessiondemo.rs  (new: REPL capstone)
└── tests/session.rs        (new: headless integration tests)
```

`src/history.rs` from chapter 01 is **unchanged** in this chapter. The
verified diff from chapter 01 (`ad1aa26`) to this chapter (`5a5a5f6`)
adds the files above and touches `Cargo.toml` and `src/bin/historydemo.rs`
(formatting pass), but `history.rs` itself has zero diff.

### Contracts (from the verified reference)

`src/actor.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OccupantKind { Human, Ai }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Occupant {
    Human,
    Ai(AiConfig),
}

impl Occupant {
    pub fn kind(&self) -> OccupantKind;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AiConfig {
    pub evaluator: EvaluatorKind,
    pub simulations: u32,
    pub c_puct: f32,
    pub temperature: f32,
    pub noise_epsilon: f32,
}

impl Default for AiConfig { fn default() -> Self; }

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum EvaluatorKind { Tactics }
```

`src/command.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Play(Move),
    Step,
    Back { n: u32 },
    Forward { n: u32 },
    Var { index: usize },
    Take { seat: SeatSel },
    Release { seat: SeatSel },
    SetSims { seat: SeatSel, n: u32 },
    New,
    Help,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeatSel { Black, White, Both }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Moved { mv: Move, by: OccupantKind },
    Branched { options: Vec<Move> },
    CursorMoved { ply: u32 },
    BranchPoint { options: Vec<(usize, Move)> },
    SeatChanged { seat: Color, now: OccupantKind },
    ConfigChanged { seat: Color, sims: u32 },
    GameOver { result: Status },
    Reset,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CommandError {
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Illegal move: {0}")]
    IllegalMove(#[from] PlayError),
    #[error("Game is already over")]
    GameOver,
    #[error("AI seats are not yet implemented (Task 3): {0}")]
    AiSeatsNotImplemented(String),
    #[error("Forward reached a leaf")]
    ForwardAtLeaf,
    #[error("No branch point at cursor")]
    NotABranchPoint,
    #[error("Variation index {index} out of bounds (options: {count})")]
    VarOutOfBounds { index: usize, count: usize },
    #[error("Invalid seat: {0}")]
    InvalidSeat(String),
    #[error("Simulation count must be greater than zero")]
    ZeroSims,
}

pub fn parse(input: &str) -> Result<Command, CommandError>;
```

`src/session.rs`:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct SessionInit {
    pub black: Occupant,
    pub white: Occupant,
    pub default_config: AiConfig,
    pub black_sims: Option<u32>,
    pub white_sims: Option<u32>,
    pub seed: Option<u64>,
    pub opening: Board,
}

impl SessionInit {
    pub fn new_hotseat() -> Self;
    pub fn new_hotseat_from_opening(opening: &Board) -> Self;
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub ply: u32,
    pub branch_options: Vec<(usize, Move)>,
    pub black: OccupantKind,
    pub white: OccupantKind,
    pub status: Status,
    pub last_move: Option<Move>,
}

impl SessionCore {
    pub fn new(init: SessionInit) -> Self;
    pub fn execute(&mut self, cmd: Command) -> Result<Vec<Event>, CommandError>;
    pub fn snapshot(&self) -> Snapshot;
    pub fn board(&self) -> Board;
}
```

### The line forms table

`command::parse` is case-insensitive and whitespace-separated.

| Input | `Command` | Notes |
|---|---|---|
| `7 7` | `Play(Move { row: 7, col: 7 })` | Bare numeric form; row col, 0–14. |
| `play 7 7` | `Play(Move { row: 7, col: 7 })` | Explicit verb form. |
| `step` | `Step` | AI seat search/play (Task 3). |
| `back` | `Back { n: 1 }` | Default `n = 1`. |
| `back 3` | `Back { n: 3 }` | Move cursor toward root by `n` plies. |
| `forward` | `Forward { n: 1 }` | Default `n = 1`. |
| `forward 5` | `Forward { n: 5 }` | Stop at branch points or leaves. |
| `var 0` | `Var { index: 0 }` | Choose child variation by index. |
| `take black` | `Take { seat: Black }` | Seat becomes `Human`. |
| `take white` | `Take { seat: White }` | Seat becomes `Human`. |
| `take both` | `Take { seat: Both }` | Both seats become `Human`. |
| `release black` | `Release { seat: Black }` | Seat becomes `Ai` (Task 3). |
| `release white` | `Release { seat: White }` | Seat becomes `Ai` (Task 3). |
| `release both` | `Release { seat: Both }` | Both seats become `Ai` (Task 3). |
| `sims black 800` | `SetSims { seat: Black, n: 800 }` | Per-seat simulation override. |
| `sims white 50` | `SetSims { seat: White, n: 50 }` | Per-seat simulation override. |
| `sims both 200` | `SetSims { seat: Both, n: 200 }` | Accepted by parser; Task 3 implements it. |
| `new` | `New` | Reset to opening. |
| `h`, `help`, `?` | `Help` | Show command summary. |
| `q`, `quit`, `exit` | `Quit` | Exit the front-end. |

All out-of-bounds coordinates, missing arguments, unknown verbs, and
extra tokens produce `CommandError::Parse`.

### Errors-as-data

`SessionCore::execute` returns `Result<Vec<Event>, CommandError>`. The
front-end decides how to report errors; the core never prints. The Task
2 implementation validates arguments even for not-yet-implemented AI
commands: `SetSims { seat, n: 0 }` returns `CommandError::ZeroSims`
before the `AiSeatsNotImplemented` path is reached. This keeps the
parser and state-machine contracts honest.

### Solution (opt-in)

The complete reference source for this chapter lives in
[02-the-command-language/01-solution.md](02-the-command-language/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare.

## Capstone: `sessiondemo` plays a scripted hotseat game

The chapter-02 binary is `sessiondemo`. Save this script as
`t2_hotseat.txt`:

```text
7 7
6 7
7 8
6 8
7 9
back 3
8 7
back 1
forward 1
var 1
new
q
```

Run it from `gomoku/`:

```text
cargo run -q -p cli --bin sessiondemo -- --script t2_hotseat.txt
```

Excerpt of the output (repeated identical board states are elided with
`[...]`):

```text
Session started. Type 'h' for help.
ply 0  status Ongoing  black Human  white Human
    0  1  2  3  4  5  6  7  8  9 10 11 12 13 14
 0  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 1  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 5  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 6  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 7  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 8  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .

  Moved (7, 7) by Human
  Moved (6, 7) by Human
  Moved (7, 8) by Human
  Moved (6, 8) by Human
  Moved (7, 9) by Human
ply 5  status Ongoing  black Human  white Human
    0  1  2  3  4  5  6  7  8  9 10 11 12 13 14
 0  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 1  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 5  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 6  .  .  .  .  .  .  .  O  O  .  .  .  .  .  .
 7  .  .  .  .  .  .  .  X  X  x  .  .  .  .  .
 8  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .

  Cursor moved to ply 2
ply 2  status Ongoing  black Human  white Human
branches: 0:(7, 8)
    0  1  2  3  4  5  6  7  8  9 10 11 12 13 14
 0  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 1  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 5  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 6  .  .  .  .  .  .  .  o  .  .  .  .  .  .  .
 7  .  .  .  .  .  .  .  X  .  .  .  .  .  .  .
 8  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .

  Moved (8, 7) by Human
  Branched: (7, 8) (8, 7)
ply 3  status Ongoing  black Human  white Human
    0  1  2  3  4  5  6  7  8  9 10 11 12 13 14
 0  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 1  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 5  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
 6  .  .  .  .  .  .  .  O  .  .  .  .  .  .  .
 7  .  .  .  .  .  .  .  X  .  .  .  .  .  .  .
 8  .  .  .  .  .  .  .  x  .  .  .  .  .  .  .
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .

  Cursor moved to ply 2
[... same ply-2 board as above ...]

  Branch point: 0: (7, 8) 1: (8, 7)
[... same ply-2 board as above ...]

  Cursor moved to ply 3
[... same ply-3 branch board as above ...]

  Reset
[... empty board, same as initial ...]
```

The script plays five moves, rewinds three plies, plays a branch move at
(8, 7), navigates back to the branch point, chooses the new branch with
`var 1`, and finally resets to a fresh board. The terminal prints nothing
that the core did not emit as an `Event`.

## TDD checklist

Run these from `gomoku/` with `cargo test -p cli`.

### `tests/session.rs` (integration tests)

1. `hotseat_game_to_black_win_emits_game_over_and_blocks_further_play`
2. `illegal_move_returns_error_and_leaves_state_unchanged`
3. `branch_mid_game_emits_branched_event_with_correct_options`
4. `back_forward_and_var_emit_navigation_events`
5. `new_emits_reset_and_returns_to_opening`
6. `take_is_no_op_for_human_seats`
7. `step_release_and_set_sims_are_not_yet_implemented`
8. `seeded_determinism_smoke`

### `src/command.rs` parser unit tests

1. `parse_play_bare_numbers`
2. `parse_play_explicit`
3. `parse_step`
4. `parse_back_default_and_explicit`
5. `parse_forward_default_and_explicit`
6. `parse_var`
7. `parse_take_release`
8. `parse_sims`
9. `parse_new_help_quit`
10. `parse_errors`
11. `parse_out_of_bounds_move_is_error`

### `src/session.rs` inline tests

`src/session.rs` also contains an inline `#[cfg(test)]` module with the
same behavioral checks (useful while developing the core before moving
them to `tests/session.rs`). The names mirror the integration tests:
`new_session_has_empty_root_and_human_seats`,
`hotseat_game_to_black_win_emits_game_over`,
`illegal_move_returns_error_and_leaves_state_unchanged`,
`branch_mid_game_emits_branched_event`,
`back_forward_var_navigation_emits_cursor_moved`,
`new_emits_reset_and_restores_opening`,
`take_is_no_op_when_seats_are_human`,
`step_release_set_sims_return_not_yet_implemented`,
`zero_sims_is_rejected`, and `seeded_determinism_smoke`.

Follow red-green-refactor:

1. **Red:** Add the parser and core types, with stubs that return the
   typed errors. Run `cargo test -p cli` and watch the tests fail.
2. **Green:** Implement `parse`, `SessionCore::execute`, and the
   snapshot logic until all tests pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any issues.

## Done when

* `cargo test -p cli` passes from `gomoku/`.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes from `gomoku/`.
* `cargo run -q -p cli --bin sessiondemo -- --script <your_hotseat_script>`
  plays a full game, branches, navigates, and resets without panics.
* Commit message for the code slice:
  `feat(cli): command language + SessionCore + sessiondemo`.

Next: [Chapter 03 — The AI takes a seat](03-the-ai-takes-a-seat.md)

## References

* [`00-playable-cli-primer.md`](00-playable-cli-primer.md) — the study
  table, core/shell, commands/events, the four knobs.
* [`docs/specs/2026-10-17-playable-cli-design.md`](../../specs/2026-10-17-playable-cli-design.md)
  — §5 session core, commands, events, parser.
* [`docs/plans/2026-10-17-playable-cli.md`](../../plans/2026-10-17-playable-cli.md)
  — Task 2 (chapter 02).
* [`01-the-variation-tree.md`](01-the-variation-tree.md) — the
  `History` variation tree this chapter wraps.
