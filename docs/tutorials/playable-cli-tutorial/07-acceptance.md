# Chapter 07 — Acceptance: proof, not vibes

## Abstract

This is the final chapter of the playable CLI tutorial. It adds no new
features; it pins the study table with three acceptance artifacts: a
bit-reproducible golden transcript, a scripted game that the study
table adjudicates end-to-end, and a measured sims-vs-strength table.
By the end you will have run the full proof ritual, seen the measured
numbers reproduced below, and understood why a 100% win rate against a
weak baseline is not a strength claim — it is a warning about what
"better" means.

## Glossary

| Term | Definition |
|------|------------|
| **Golden transcript** | A seeded `SessionCore` command script whose `Vec<Event>` output is asserted exactly, move for move and event for event. |
| **Event-stream equality** | The reproducibility contract: two runs with the same seed produce the same `Event` sequence, independent of ANSI rendering or terminal state. |
| **Scripted game** | A headless test that drives a real game to a terminal status through the core's public API, proving the table can adjudicate a forcing line. |
| **Sims-vs-strength table** | A `#[ignore]`d measurement that pits the tactics AI at 50/200/800 simulations against a uniform-random opponent, 20 seeded games per level. |
| **Uniform-random baseline** | An opponent that plays uniformly random legal moves. It is a sanity check, not a strength yardstick. |
| **Baseline tutorial** | The sibling tutorial whose evaluator becomes `EvaluatorKind::Baseline` behind a cargo feature (spec §13). |
| **Opening schedule `[experiment]`** | The AlphaGo Zero opening schedule from chapter 04: temperature 1.0 + Dirichlet noise for the first 10 plies of an AI-vs-AI line, then argmax. |
| **Crate polish** | Adding `#![deny(missing_docs)]` to `src/lib.rs` so every public item carries a doc comment. |
| **Gate ritual** | The full acceptance check: default tests, workspace tests, clippy, fmt, and the ignored measurement. |

## Context

Chapters 01–06 built the study table: the variation tree, the command
language, the `SessionCore` state machine, human and AI seats, the
rendered board, and the overlays that show what the AI thinks.
Everything works in isolation; every behavior is reachable through the
public `execute(Command) -> Result<Vec<Event>, CommandError>` contract.

What remains is the proof that the whole thing hangs together under
three different lenses:

1. **Reproducibility.** With a fixed seed, the same command script
   produces the same event stream every time. This is the contract
   that makes the other two artifacts possible.
2. **End-to-end adjudication.** The core can run a real game to
   checkmate — or, in Gomoku, to a five-in-a-row — and report the
   terminal status.
3. **Measured strength.** The tactics-shaped AI is measurably stronger
   than random play, but the measurement also reveals the limits of
   that claim.

This chapter is the controller's checkpoint before the tutorial is
considered done: run the gates, quote the numbers, and write down the
honest reading.

## Intent

Add `gomoku/crates/cli/tests/acceptance.rs` with three tests:

1. `golden_transcript_seeded_study_session` — a scripted study session
   under seed 42 reproduces its `Vec<Event>` and final `Snapshot`
   exactly.
2. `scripted_human_beats_tactics_ai` — the study table adjudicates a
   forcing fork against the 200-sim tactics AI and ends with
   `Status::Won(Color::Black)`.
3. `sims_vs_strength_measured` — a `#[ignore]`d measurement that runs
   20 seeded games per level at 50/200/800 simulations against a
   uniform-random baseline and prints the win/loss/draw table.

Then add `#![deny(missing_docs)]` to `src/lib.rs`, run the gate ritual,
and commit.

Observable done-state: `cargo test -p cli`, `cargo test --workspace`,
`cargo clippy --all-targets -- -D warnings`, and
`cargo fmt --all -- --check` are all green, and the ignored measurement
reproduces the table quoted below.

## The three acceptance artifacts

The reference implementation contains exactly these three tests,
landed across three commits: `b6f5aab` introduced them, `e06a692`
amended the measurement to a uniform-random baseline (the story
below), and `fe0105f` dropped an unused import the clippy gate
caught. The diff from chapter 06 (`ee4a66a`) is small and confined:

```text
$ git diff ee4a66a..fe0105f --stat -- gomoku/crates/cli
 gomoku/crates/cli/src/lib.rs          |   2 +
 gomoku/crates/cli/src/tui/board.rs    |  11 +
 gomoku/crates/cli/src/tui/input.rs    |   7 +
 gomoku/crates/cli/src/tui/render.rs   |   1 +
 gomoku/crates/cli/tests/acceptance.rs | 335 ++++++++++++++++++++++++++++++++++
 5 files changed, 356 insertions(+)
```

The four `src/tui/` changes are minor clean-ups that came along with
the final gate pass; the acceptance story lives in
`tests/acceptance.rs` and the `#![deny(missing_docs)]` line in
`src/lib.rs`.

### a) Golden transcript — event-stream equality

`golden_transcript_seeded_study_session` drives a seeded AI-vs-AI study
session through `SessionCore` with no terminal involved. The test
asserts two things: the exact `Vec<Event>` produced by the script, and
the exact `Snapshot` at the end.

The script, in English:

1. Start an AI-vs-AI session with seed `42` and the default 200-sim
   tactics config. The opening schedule `[experiment]` is active for
   the first 10 plies, so the first six `Step` commands use
   temperature 1.0 + Dirichlet noise.
2. Change per-seat sims: Black to 100, White to 50. The core emits
   `ConfigChanged` events.
3. `Take Black` — the seat becomes `Human`.
4. `Back 2`, then play a human branch move at `a1`. The tree
   branches at ply 4; the core emits a `Branched` event listing the
   two children.
5. `Back 1`, then `Var 0` to return to the original main-line child.
6. `Release Black` — the seat becomes `Ai` again, using the current
   default config.
7. Step twice more.

The expected event stream is asserted literally:

```rust
let expected = vec![
    Event::Moved {
        mv: mv(4, 3),
        by: OccupantKind::Ai,
    },
    Event::Moved {
        mv: mv(9, 6),
        by: OccupantKind::Ai,
    },
    Event::Moved {
        mv: mv(3, 1),
        by: OccupantKind::Ai,
    },
    Event::Moved {
        mv: mv(11, 12),
        by: OccupantKind::Ai,
    },
    Event::Moved {
        mv: mv(13, 3),
        by: OccupantKind::Ai,
    },
    Event::Moved {
        mv: mv(7, 3),
        by: OccupantKind::Ai,
    },
    Event::ConfigChanged {
        seat: Color::Black,
        sims: 100,
    },
    Event::ConfigChanged {
        seat: Color::White,
        sims: 50,
    },
    Event::SeatChanged {
        seat: Color::Black,
        now: OccupantKind::Human,
    },
    Event::CursorMoved { ply: 4 },
    Event::Moved {
        mv: mv(0, 0),
        by: OccupantKind::Human,
    },
    Event::Branched {
        options: vec![mv(13, 3), mv(0, 0)],
    },
    Event::CursorMoved { ply: 4 },
    Event::CursorMoved { ply: 5 },
    Event::SeatChanged {
        seat: Color::Black,
        now: OccupantKind::Ai,
    },
    Event::Moved {
        mv: mv(3, 0),
        by: OccupantKind::Ai,
    },
    Event::Branched {
        options: vec![mv(7, 3), mv(3, 0)],
    },
    Event::Moved {
        mv: mv(2, 9),
        by: OccupantKind::Ai,
    },
];
```

And the final snapshot:

```rust
let expected_snapshot = Snapshot {
    ply: 7,
    branch_options: vec![],
    black: OccupantKind::Ai,
    white: OccupantKind::Ai,
    status: Status::Ongoing,
    last_move: Some(mv(2, 9)),
};
```

**Why event-stream equality, not ANSI output?** The core/shell split
deliberately separates behavior from rendering. ANSI output depends on
cell widths, colour settings, terminal width, and whether the shell is
in alternate-screen mode. `Event`s are data: the same command script
must produce the same events regardless of how the front-end paints
them. A golden transcript on ANSI strings would couple the test to
presentation details and would break every time a gutter width
changed. Event-stream equality is the right contract because it tests
the core's public promise.

### b) Scripted human beats the tactics AI

`scripted_human_beats_tactics_ai` proves that the study table can
adjudicate a real game end-to-end. The test sets up a position with a
classic fork, plays the winning move through the core, lets the AI
respond, then completes the win.

The opening board is built with `engine::reference::board_from_ascii`:
Black has three stones in a vertical line at column 7 (rows 4, 5, 6)
and three stones in a horizontal line at row 7 (columns 4, 5, 6). The
intersection at `h8` creates two open fours simultaneously. White
has a few scattered stones that do not interfere. It is Black to move.

The script:

1. Create a session with Black as `Human`, White as `Ai` at 200
   simulations, seed `7`, and the fork position as the opening root.
2. Human plays `h8`. The core emits a `Moved` event with
   `by: OccupantKind::Human`.
3. `Step` — White's tactics AI searches and blocks one of the two
   open fours.
4. Use `engine::immediate_wins(&core.board(), Color::Black)` to find
   the remaining winning move and play it.
5. Assert the final status is `Status::Won(Color::Black)` and that the
   last recorded move belongs to the scripted side.

The 200-sim AI may or may not find the best block; the fork is a
forcing win regardless. The test does not assert *which* move the AI
played — only that the game reached the expected terminal state
through the core's normal command flow.

### c) Measured sims-vs-strength

`sims_vs_strength_measured` is `#[ignore]`d because it is a
measurement, not a fast gate. Run it explicitly:

```bash
cd gomoku
cargo test -p cli --release --test acceptance -- --ignored --nocapture
```

The test plays 20 seeded games at each of three simulation levels:
Black is the tactics AI at 50/200/800 simulations; White is a test
harness playing uniform-random legal moves through `Command::Play`
(White's seat is `Human`). The seed for each game is derived from a
base seed plus the level and game index so the measurement is
reproducible.

Measured output on the reference machine (Apple M5 Max, macOS / Darwin
27, release profile, 20 games per level, base seed 2026):

```text
sims-vs-strength (tactics AI Black vs uniform-random White)
games per level: 20, base seed: 2026
sims     wins losses  draws  games win_rate
50         20      0      0     20     100%
200        20      0      0     20     100%
800        20      0      0     20     100%
```

The table is honest but not triumphant. A 100% win rate against
uniform-random play is the minimum bar for any search that understands
tactics: even 50 simulations of Monte Carlo Tree Search (MCTS) with
PUCT selection (predictor UCT, Rosin 2011) and a tactics-shaped
evaluator never loses to pure randomness. The table therefore tells us very
little about the *gradient* of strength as simulations increase.

## Honest reading: what the measurement actually means

**100% at every level is not a boring non-result.** It is a specific,
useful negative claim: uniform-random play is not a strength yardstick
for this engine. If you want to know whether 800 sims is meaningfully
stronger than 50 sims, you need a real opponent. The next rung is the
baseline tutorial's evaluator (spec §13), which becomes
`EvaluatorKind::Baseline` behind a cargo feature and enrolls into the
same seat abstraction with zero redesign.

**There is a second, subtler lesson in the commit history.** The first
version of this measurement, at commit `b6f5aab`, used a "near-random"
opponent defined as 1 simulation with temperature 1.0. That sounds
like randomness, but 1 simulation of PUCT collapses to a deterministic
top-prior greed: the evaluator's highest prior is always selected. The
result was an unexplained cliff at 50 and 200 simulations — some
losses that disappeared at 800 — and the table was hard to interpret.
The amendment at `e06a692` replaced that opponent with a truly
uniform-random driver, which yielded the clean 100% table above.

The meta-lesson of the chapter is that **acceptance criteria are
hypotheses about what "better" means.** Measuring against the wrong
baseline is worse than not measuring, because it gives you a number
that looks like evidence but is not. The 100% table is useful exactly
because it exposes the baseline's inadequacy and points to the next
measurement.

## Crate polish and the gate ritual

### `#![deny(missing_docs)]`

Add the attribute directly after the module documentation in
`gomoku/crates/cli/src/lib.rs`:

```rust
//! Terminal Gomoku client library.
//!
//! The `cli` crate is split into a headless library (history, command,
//! session, actor) and a thin binary front-end. The `tui` module holds
//! the existing terminal UI pieces moved here unchanged.

#![deny(missing_docs)]
```

On the reference build this compiles cleanly with the public surface
already documented from chapters 01–06. If the compiler reports missing
doc comments, add them; documenting the public contract is part of
locking the crate.

### The full gate ritual — measured on the reference worktree

Run from `gomoku/`:

1. **Default suite.** `cargo test -p cli`:
   ```text
   test result: ok. 90 passed; 0 failed; 0 ignored
   test result: ok. 2 passed; 0 failed; 1 ignored
   test result: ok. 7 passed; 0 failed; 0 ignored
   test result: ok. 9 passed; 0 failed; 0 ignored
   test result: ok. 10 passed; 0 failed; 0 ignored
   test result: ok. 4 passed; 0 failed; 0 ignored
   ```
   Total: **122 passed, 1 ignored** (the ignored measurement).

2. **Workspace suite.** `cargo test --workspace`:
   All crate test suites pass. The `cli` acceptance suite contributes
   the same 122 passed / 1 ignored; other crates add their own green
   suites. The full run finishes in roughly 70 s on the reference
   machine, dominated by the `mcts` and `patterns` proof suites.

3. **Clippy.** `cargo clippy --all-targets -- -D warnings`: green.
   Getting there was the gate doing its job: the amendment at
   `e06a692` replaced a hand-rolled `Board::new()` with
   `SessionInit::ai_vs_ai` and left `use engine::{Board, ...}` behind,
   which `-D warnings` refused to pass. The fix (`fe0105f`) is the
   one-line import cleanup. Run the gate; it must pass clean, not
   "clean except".

4. **Format.** `cargo fmt --all -- --check`:
   No changes.

5. **Ignored measurement.** `cargo test -p cli --release --test acceptance -- --ignored --nocapture`:
   Produces the table quoted above and finishes in about 1 s on the
   reference machine because the uniform-random baseline resigns
   quickly.

The final state (`fe0105f`) is the one worth committing.

## Solution (opt-in)

The complete reference files for this chapter are quoted verbatim in
[07-acceptance/01-solution.md](07-acceptance/01-solution.md). Open it
only if you have been stuck for more than twenty minutes, or after you
have finished the chapter and want to compare your work.

The solution file quotes two commits: `b6f5aab` introduced the three
acceptance tests, and `e06a692` amended the measurement to use a
uniform-random baseline. The quoted text was verified byte-for-byte
against the reference worktree with `git show <commit>:<path>`.

## TDD checklist

This chapter adds exactly three tests in `tests/acceptance.rs`. All
previous chapters' tests continue to pass.

1. `golden_transcript_seeded_study_session`
2. `scripted_human_beats_tactics_ai`
3. `sims_vs_strength_measured`

Run them from `gomoku/`:

```bash
# Default suite (first two tests)
cargo test -p cli

# The measurement (ignored by default)
cargo test -p cli --release --test acceptance -- --ignored --nocapture
```

### Red-green-refactor rhythm

1. **Red:** Create `tests/acceptance.rs` with the three tests above
   and add `#![deny(missing_docs)]` to `src/lib.rs`. Run
   `cargo test -p cli`. Expect the first two tests to fail because the
   expected event sequence and snapshot do not yet match your
   implementation.
2. **Green:** Adjust the core until the tests pass. Do not weaken the
   assertions; the transcript is the contract.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check`. Fix the unused-import warning if it
   appears.

## Capstone: proof, not vibes

Run the golden transcript under seed 42:

```bash
cd gomoku
cargo test -p cli --test acceptance golden_transcript_seeded_study_session -- --nocapture
```

The test passes silently if the event stream matches. That silence is
the point: the core is now a deterministic machine whose behavior is
pinned by data, not by how the board looks in a terminal.

Then run the ignored measurement:

```bash
cargo test -p cli --release --test acceptance -- --ignored --nocapture
```

The 100% table is the final lesson. The study table is playable, it is
tested, and it is honest about what it has and has not proven.

## Done when

* `tests/acceptance.rs` contains the three tests named above.
* `golden_transcript_seeded_study_session` passes with the exact event
  stream and final snapshot from the reference implementation.
* `scripted_human_beats_tactics_ai` passes and ends with
  `Status::Won(Color::Black)`.
* `sims_vs_strength_measured` reproduces the measured table when run
  with `--ignored --nocapture`.
* `src/lib.rs` has `#![deny(missing_docs)]` and the crate compiles.
* `cargo test -p cli` is green (122 passed / 1 ignored on the
  reference machine).
* `cargo test --workspace` is green.
* `cargo clippy --all-targets -- -D warnings` is green.
* `cargo fmt --all -- --check` makes no changes.
* Commit message: `test(cli): acceptance — transcripts, scripted game, measured strength`.

## What was built

In seven chapters the `cli` crate grew from a human-vs-human board
into a study table. Chapter 01 gave it a persistent variation tree;
chapter 02 a command language and a headless `SessionCore`; chapter 03
seats for humans and the tactics AI; chapter 04 take/release, per-seat
sims, and the opening schedule; chapter 05 the rendered board and side
panel; chapter 06 the policy overlay and value strip; and chapter 07
the acceptance proof that pins it all. The result is playable today:
you can sit down against the search, watch it play itself, branch into
a variation, and see what the search believed at every node.

## What enrolls next

The seat abstraction is waiting. The baseline tutorial's evaluator
will become another `EvaluatorKind` behind a cargo feature, and the
first trained network will follow the same path. The table does not
change; only the occupants get stronger. Update `docs/WARM-UP.md` with
the milestone status when the owner signs off.

## References

* [`00-playable-cli-primer.md`](00-playable-cli-primer.md) — the study
  table, core/shell, commands/events, the four knobs, and the opening
  schedule.
* [`README.md`](README.md) — chapter map and tutorial conventions.
* [`docs/plans/2026-10-17-playable-cli.md`](../../plans/2026-10-17-playable-cli.md)
  — Task 7 (this chapter).
* [`docs/specs/2026-10-17-playable-cli-design.md`](../../specs/2026-10-17-playable-cli-design.md)
  — §12 (testing strategy) and §13 (relationship to the baseline
  tutorial).
* `gomoku/crates/cli/src/lib.rs` — crate entry point and
  `#![deny(missing_docs)]`.
* `gomoku/crates/cli/tests/acceptance.rs` — the three acceptance tests
  (reference commits `b6f5aab`, `e06a692`, and `fe0105f`).
* Christopher D. Rosin (2011). *Multi-armed bandits with episode
  context*. Annals of Mathematics and Artificial Intelligence 61(3),
  203–230. [`https://doi.org/10.1007/s10472-011-9258-6`](https://doi.org/10.1007/s10472-011-9258-6)
  — PUCT selection.
* David Silver, Julian Schrittwieser, Karen Simonyan, Ioannis
  Antonoglou, Aja Huang, Arthur Guez, Thomas Hubert, Lucas Baker,
  Matthew Lai, Adrian Bolton, Yutian Chen, Timothy Lillicrap, Fan Hui,
  Laurent Sifre, George van den Driessche, Thore Graepel, Demis
  Hassabis (2017). *Mastering the game of Go without human knowledge*.
  Nature 550, 354–359.
  [`https://doi.org/10.1038/nature24270`](https://doi.org/10.1038/nature24270)
  — AlphaGo Zero: root Dirichlet noise and temperature sampling.
