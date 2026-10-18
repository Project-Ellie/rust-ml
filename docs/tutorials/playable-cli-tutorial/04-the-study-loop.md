# Chapter 04 — The study loop

## Abstract

This chapter turns the AI seat from a curiosity into a study
instrument. You will add `Take` and `Release` for either seat at any
moment, wire per-seat simulation overrides (`SetSims`) through to the
search, and implement the AlphaZero-style opening schedule that keeps
AI-vs-AI self-play from replaying the same game forever. The payoff is
the workflow the whole design exists for: **steal the AI's seat mid-game,
branch off into a variation, navigate back, and hand the seat back** —
without the table ever changing modes.

## Glossary

| Term | Definition |
|------|------------|
| **Take** | The command that makes a seat `Human`. `Take Both` turns the table into hotseat instantly. |
| **Release** | The command that makes a seat `Ai` using the session defaults plus any per-seat simulation override. |
| **SeatSel** | `Black`, `White`, or `Both` — the target of `Take`, `Release`, and `SetSims`. |
| **Opening schedule `[experiment]`** | AI-vs-AI from an empty root: first 10 plies use temperature 1.0 + Dirichlet noise (ε = 0.25, α = 0.3), then argmax. Explicit flags always win. |
| **`explicit_selection`** | `AiConfig` flag recording whether `temperature`/`noise_epsilon` were set by the user. The schedule applies only when it is `false` and both values are at their defaults. |
| **`SessionInit` constructors** | `hotseat`, `hotseat_from_opening`, `versus_ai`, `ai_vs_ai`, and the `new_*` aliases — ergonomic entry points for tests and the future TUI. |
| **Idempotency** | `Take`/`Release` emit `SeatChanged` even when the seat already has the requested occupant; the final state is unchanged. |
| **Per-seat simulations** | `black_sims`/`white_sims` overrides, set at startup or runtime with `SetSims`, that reach the searching AI verbatim. |

## Context

Chapter 03 seated the AI: `release white`, `step`, and `info` work, the
RNG is seeded, and `SearchInfo` stores the search record on the history
node. That already supports "play against the AI." What it does not yet
support is the **study loop**: the moment when you decide the AI's move
is interesting but you want to see what happens if you answered
differently three plies ago.

The commands `Take`, `Release`, and `SetSims` were parsed in chapter 02;
this chapter makes them do the right thing mid-game and mid-variation.
The opening schedule is new behaviour inside `execute_step`, not a new
command or mode. Everything still flows through the same `Command ->
Result<Vec<Event>>` core contract.

## Intention

1. Extend `gomoku/crates/cli/src/actor.rs`:
   * add `Occupant::is_human` and `is_ai` helpers;
   * add `AiConfig::explicit_selection` so the core can tell whether the
     user overrode temperature or Dirichlet noise.
2. Extend `gomoku/crates/cli/src/session.rs`:
   * add the `SessionInit` convenience constructors (`hotseat`,
     `hotseat_from_opening`, `versus_ai`, `ai_vs_ai`, plus `new_*`
     aliases);
   * implement `opening_schedule_active` with the exact rule and apply an
     *effective* `AiConfig` inside `execute_step`;
   * keep `Take`/`Release`/`SetSims` idempotent and mid-variation safe.
3. Extend `gomoku/crates/cli/src/bin/sessiondemo.rs` to accept
   `--seed`.
4. Create `gomoku/crates/cli/study_demo.txt` and
   `gomoku/crates/cli/tests/study.rs` with the TDD checklist tests.
5. Understand why the schedule is keyed to cursor ply, not node
   identity, and why that makes branching back into the first 10 plies
   re-enter the schedule.

Observable done-state: a seeded AI-vs-AI script produces a non-repeating
opening; a second script shows a human taking Black, branching, and
handing the seat back; the TDD checklist passes; the gates are clean.

## Mental mapping

### The table is the instrument, not the AI

A chess engine has an "analysis mode" bolted onto a "game mode." The
study table has neither. There are only two seats and a tree. When you
want to explore a variation, you do not leave a mode; you `back`, `var`,
or `play` a new move at the cursor. When you want the AI to stop
moving for one side, you `take` that seat. When you want it back, you
`release` it. The board, the cursor, and the history are untouched by
occupant changes.

### The opening schedule: why argmax self-play is useless

If both seats are AI with `temperature 0.0` and no noise, the exact same
line plays every time. That is useless for study and poor preparation
for a future training pipeline. AlphaZero solves this by sampling the
opening: high temperature and Dirichlet noise early, then argmax later.

This chapter's schedule is deliberately small and labeled
`[experiment]`:

| Condition | Effective config |
|-----------|------------------|
| AI-vs-AI from an empty root, no explicit temp/noise, cursor ply `< 10` | `temperature = 1.0`, `noise_epsilon = 0.25` |
| AI-vs-AI from an empty root, no explicit temp/noise, cursor ply `>= 10` | configured values (default argmax) |
| Any explicit `--temperature` or `--noise` flag | configured values verbatim |
| Not AI-vs-AI, or not an empty root | configured values verbatim |

The mcts crate's `add_dirichlet_noise` uses α = 0.3, so the effective
noise call is:

```rust
mcts::add_dirichlet_noise(
    &mut outcome.tree,
    outcome.root,
    0.25,
    0.3,
    &mut self.rng,
);
```

### `explicit_selection` is the flag, not the value

The schedule must not fire just because the user happened to type
`--temperature 0.0`. The `AiConfig` therefore carries an
`explicit_selection` bool in addition to the values. When the CLI parses
`--temperature` or `--noise`, it sets the flag to `true`. The schedule
applies only when:

```rust
!self.default_config.explicit_selection
    && self.default_config.temperature == 0.0
    && self.default_config.noise_epsilon == 0.0
```

The check uses `default_config`, not the effective config, so a seat
that has been temporarily switched to high temperature does not somehow
make every future search believe it was explicit.

### Eligibility is ply-based

`opening_schedule_active` checks `self.history.ply() < 10`. The cursor
ply is the length of the root-to-cursor path. That means:

* If you play 15 AI-vs-AI moves, then `back 6`, the next `step` at ply
  9 re-enters the schedule.
* If you branch off in the first 10 plies and the new variation stays
  inside that window, the new line also uses the schedule.

This is the intended behaviour: the schedule is a property of the
position's depth in the current variation, not a once-per-game flag.

### The take/release matrix

`execute_take` and `execute_release` use `apply_to_seats` to handle
`SeatSel::Both`. The semantics are uniform:

| Command | Seat state before | Seat state after | Emits `SeatChanged`? |
|---------|-------------------|------------------|----------------------|
| `take black` | Human | Human | Yes |
| `take black` | Ai | Human | Yes |
| `release white` | Human | Ai | Yes |
| `release white` | Ai | Ai | Yes |
| `take both` | any | Human (both) | Yes, one per seat |
| `release both` | any | Ai (both) | Yes, one per seat |

The operations are idempotent: repeating them does not panic, does not
reset the board, and does not skip the event. This is what makes
`take black` safe to type twice.

`Release` builds a fresh `Ai(default_config)` and then applies the
per-seat simulation override (`black_sims` or `white_sims`) if one
exists. That is how `SetSims` at runtime reaches the next search.

### Why `SessionInit` grew constructors

`SessionInit` is a public struct so any front-end can build it field by
field. But tests and the upcoming TUI repeatedly need the same four
shapes:

```rust
SessionInit::hotseat()
SessionInit::hotseat_from_opening(&board)
SessionInit::versus_ai(Color::Black, config)
SessionInit::ai_vs_ai(config, seed)
```

They are not strictly necessary — the chapter could keep building the
struct manually — but they remove a source of copy-paste errors and make
the test intent readable at a glance. `new_hotseat` and
`new_hotseat_from_opening` are kept as aliases for callers that prefer
the `new_` naming from earlier chapters.

### Per-seat simulations reach the search

`SetSims { seat, n }` does two things:

1. It stores the override (`black_sims` or `white_sims`).
2. If the seat is currently `Ai`, it updates the live `cfg.simulations`
   so the next `Step` uses the new budget immediately.

`Release` reads the override when constructing the new `Ai` config, so a
`SetSims` issued before a `Release` is also honoured. The stored
`SearchInfo::config.simulations` then records the actual budget used,
which is what tests assert.

## Low-level design

### Files changed vs chapter 03

Your work in this chapter is confined to the `cli` crate; the
footprint:

```text
$ git diff 251246c..d9e8fec --stat -- gomoku/crates/cli
 gomoku/crates/cli/src/actor.rs           |  21 +++
 gomoku/crates/cli/src/bin/sessiondemo.rs |  12 +-
 gomoku/crates/cli/src/session.rs         |  93 +++++++++--
 gomoku/crates/cli/study_demo.txt         |  13 ++
 gomoku/crates/cli/tests/actor.rs         |   1 +
 gomoku/crates/cli/tests/study.rs         | 263 +++++++++++++++++++++++++++++++
 6 files changed, 393 insertions(+), 10 deletions(-)
```

Files unchanged from chapter 03: `src/command.rs`, `src/history.rs`,
`src/fmt.rs`, `Cargo.toml`, `tests/history.rs`, and `tests/session.rs`.
`tests/actor.rs` gains only the `explicit_selection: false` field
initialization.

### `src/actor.rs`

Additions:

```rust
impl Occupant {
    pub fn is_human(&self) -> bool { matches!(self, Occupant::Human) }
    pub fn is_ai(&self) -> bool { matches!(self, Occupant::Ai(_)) }
}
```

And the new field on `AiConfig`:

```rust
pub explicit_selection: bool,
```

with `#[serde(default)]` so older serialized configs deserialize safely.
The default value is `false`, which lets the opening schedule apply by
default.

### `src/session.rs`

#### `SessionInit` constructors

`hotseat()` and `hotseat_from_opening()` are the canonical names;
`new_hotseat()` and `new_hotseat_from_opening()` delegate to them.
`versus_ai` puts the human in the requested seat and the AI in the
opposite seat. `ai_vs_ai` puts an AI in both seats and stores the seed.

#### Opening schedule application

Inside `execute_step`, before building the evaluator:

```rust
let mut effective_config = config.clone();
if self.opening_schedule_active(side) {
    effective_config.temperature = 1.0;
    effective_config.noise_epsilon = 0.25;
}
```

All subsequent calls (`make_evaluator`, `SearchConfig`,
`add_dirichlet_noise`, `SearchInfo::from_search`, `select_move`) use
`effective_config`. The stored `SearchInfo` therefore carries the
*temporary* effective values, which is what the acceptance test checks
when it asserts `info.config.temperature == 1.0` inside the scheduled
window.

#### `opening_schedule_active`

Implement this rule exactly:

```rust
fn opening_schedule_active(&self, _side: Color) -> bool {
    self.init.opening == Board::new()
        && self.black.is_ai()
        && self.white.is_ai()
        && !self.default_config.explicit_selection
        && self.default_config.temperature == 0.0
        && self.default_config.noise_epsilon == 0.0
        && self.history.ply() < 10
}
```

#### `Take`, `Release`, and `SetSims`

These functions are unchanged in shape from chapter 02/03. The study
loop relies on the fact that they already worked on the seat table
without touching the history tree:

* `execute_take` sets the seat to `Occupant::Human`.
* `execute_release` sets the seat to `Occupant::Ai(config)` with the
  per-seat sims override applied.
* `execute_set_sims` stores the override and updates a live AI config.

The only new requirement is that they all handle `SeatSel::Both` and
emit one `SeatChanged` event per affected seat.

### `src/bin/sessiondemo.rs`

Add `parse_seed` and forward the seed into `SessionInit`:

```rust
fn parse_seed(args: &[String]) -> Option<u64> { ... }

let init = SessionInit {
    seed,
    ..SessionInit::new_hotseat()
};
```

This makes the capstone commands bit-reproducible.

### `study_demo.txt`

A twelve-line script used by the capstone:

```text
release both
step
step
step
step
step
step
step
step
step
step
step
step
```

### `tests/study.rs`

New headless integration tests. See the TDD checklist for exact names.

## Solution (opt-in)

The complete target files for this chapter are quoted verbatim in
[04-the-study-loop/01-solution.md](04-the-study-loop/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare your work.

## TDD checklist

Run from `gomoku/` with `cargo test -p cli`.

### `tests/study.rs`

Write exactly these `#[test]` functions:

1. `scripted_study_transcript`
2. `opening_schedule_changes_ai_vs_ai_play`
3. `per_seat_sims_reach_search`
4. `take_and_release_are_idempotent`

### `tests/actor.rs`, `tests/session.rs`, `tests/history.rs`

All chapter-03 tests continue to pass and are part of the chapter-04
gate. Notable carry-overs:

* `tactics_ai_takes_win_in_one` and `tactics_ai_blocks_opponent_open_four`
  must still win/block.
* `seeded_determinism_same_seed_same_events_and_search_info` must still
  reproduce.
* The session and history navigation tests must still pass after the
  new constructors land.

### Red-green-refactor rhythm

1. **Red:** Add `explicit_selection` to `AiConfig`, extend
   `SessionInit`, implement `opening_schedule_active`, add
   `tests/study.rs`, and create `study_demo.txt`. Run
   `cargo test -p cli`. Expect failures.
2. **Green:** Implement the contracts above until `cargo test -p cli`
   passes.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any issues.

## Capstone: steal the seat, explore, hand it back

### AI-vs-AI opening variety

Run the study script:

```text
cd gomoku
cargo run -q -p cli --bin sessiondemo -- --seed 7 --script crates/cli/study_demo.txt
```

Expect these key frames:

Releasing both seats:

```text
  Black seat is now Ai
  White seat is now Ai
ply 0  status Ongoing  black Ai  white Ai
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
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
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
```

The first AI move is no longer the deterministic corner from chapter 03;
the schedule has sampled it:

```text
  Moved l2 by Ai
ply 1  status Ongoing  black Ai  white Ai
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 2  .  .  .  .  .  .  .  .  .  .  .  x  .  .  .  
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
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
```

After ten scheduled plies the next move falls back to argmax on the flat
tactics prior, which is why move eleven is the corner:

```text
  Moved a1 by Ai
ply 11  status Ongoing  black Ai  white Ai
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  x  .  .  .  O  .  .  .  O  O  .  .  .  X  .  
 2  .  .  .  .  .  .  .  .  .  .  .  X  .  .  .  
 3  .  .  .  .  .  .  .  X  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  .  .  .  .  X  .  .  .  
 5  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 8  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  X  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  O  .  .  .  .  O  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
```

The full run ends at ply 12. Because the first 10 plies were sampled,
running the same command with the same seed is reproducible, but running
it without a seed or with `explicit_selection: true` produces a
different opening.

### Take → branch → var → release

A short script demonstrating the study loop (save it as
`/tmp/study_loop_demo.txt`):

```text
release both
step
step
step
take black
back 1
play a1
back 1
var 1
release black
step
```

Run it:

```text
cargo run -q -p cli --bin sessiondemo -- --seed 7 --script /tmp/study_loop_demo.txt
```

Expect these frames:

Three AI-vs-AI moves, then taking Black's seat:

```text
  Moved i1 by Ai
ply 3  status Ongoing  black Ai  white Ai
 ...
  Black seat is now Human
ply 3  status Ongoing  black Human  white Ai
```

Going back one ply and playing `a1` creates a sibling branch:

```text
  Cursor moved to ply 2
 ...
  Moved a1 by Human
  Branched: i1 a1
ply 3  status Ongoing  black Human  white Ai
```

Going back to the branch point and choosing the new variation with `var 1`:

```text
  Cursor moved to ply 2
 ...
branches: 0:i1  1:a1
 ...
  Cursor moved to ply 3
ply 3  status Ongoing  black Human  white Ai
```

Releasing Black and letting the AI continue from the human's variation:

```text
  Black seat is now Ai
 ...
  Moved j12 by Ai
ply 4  status Ongoing  black Ai  white Ai
```

That is the whole workflow: the AI played, you took its seat, you
branched, you returned to the branch, and you handed the seat back. The
tree remembers both variations; neither is lost.
## Done when

* `cargo test -p cli` passes from `gomoku/` (including all chapter-03
  tests and the new `tests/study.rs` tests).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* The `sessiondemo` scripted demos print the frames above.
* Commit message for the code slice:
  `feat(cli): take/release seats, per-seat sims, opening schedule`.

Next: [Chapter 05 — The real table](05-the-real-table.md)

## References

* [`00-playable-cli-primer.md`](00-playable-cli-primer.md) — the study
  table, core/shell, the four knobs, and the opening schedule rationale.
* [`README.md`](README.md) — chapter map and tutorial conventions.
* [`docs/plans/2026-10-17-playable-cli.md`](../../plans/2026-10-17-playable-cli.md)
  — Task 4 (this chapter).
* [`docs/specs/2026-10-17-playable-cli-design.md`](../../specs/2026-10-17-playable-cli-design.md)
  — §6 (actors, knobs, opening schedule) and §5.2 (commands).
* [`docs/tutorials/mcts-tutorial/08-from-tree-to-move.md`](../mcts-tutorial/08-from-tree-to-move.md)
  — temperature, `select_move`, and `add_dirichlet_noise`.
