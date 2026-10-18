# Chapter 03 — The AI takes a seat

## Abstract

This chapter seats the AI at the study table. The payoff is two-fold:
`release white` followed by `step` makes the AI answer your moves, and
`info` exposes the search's reasoning as concrete numbers — visits,
prior probabilities, and a root value. To get there you will:

* define the `Occupant` enum (`Human` or `Ai(AiConfig)`), the
  `AiConfig` search-knob record, and the `SearchInfo` record that
  stores what the search concluded;
* wire `SessionCore::execute(Step)` to call `mcts::search`, extract
  the visit distribution, pick a move with temperature, store the
  `SearchInfo` on the new history node, and emit the right events;
* seed a `StdRng` from `SessionInit::seed` so identical sessions are
  bit-reproducible;
* add an `info` command to `sessiondemo` that prints the cursor's
  stored search record.

The chapter's mental-mapping centerpiece is the **value sign
convention**: why `SearchInfo::value` is from the side-to-move's
perspective and why `sum(W) / sum(N)` over root edges gives it. The
capstone is a real `sessiondemo` transcript: the AI plays, and then
shows its working.

## Glossary

| Term | Definition |
|------|------------|
| **Occupant** | Who sits in a seat: `Human` or `Ai(AiConfig)`. The study table has no modes, only seat assignments. |
| **OccupantKind** | The serializable, configuration-free view of an occupant (`Human` or `Ai`) used in events and snapshots. |
| **AiConfig** | The four search knobs plus an evaluator tag: `simulations`, `c_puct`, `temperature`, `noise_epsilon`, `evaluator`. Stored as a fingerprint with every `SearchInfo`. |
| **EvaluatorKind** | The enum of available evaluators (`Tactics` today; `Baseline` and `Net` are future enrollments). |
| **SearchInfo** | The record stored on a history node when an AI `Step` produced it: sorted visits, sorted prior, root value, and the `AiConfig` fingerprint. |
| **Side-to-move value** | The estimated value of a position from the perspective of the player whose turn it is. Positive favors the player about to move. |
| **Negate-once convention** | MCTS backup flips the sign of the leaf value at every edge on the path to the root. Each edge therefore accumulates `W` from the perspective of the player who chose at its parent. |
| **Seeded determinism** | Two `SessionCore`s built from the same `SessionInit` seed produce identical event streams and identical `SearchInfo`. |
| **`[experiment]`** | A documented, replaceable hyper-parameter (see the primer and `docs/12-gomoku-architecture.md`). |

## Context

Chapter 02 left `Command::Step` parsed and validated, but executing it
with a human seat to move returned `CommandError::StepRequiresAi`.
That was correct: the command language and the session state machine
were in place before the AI plumbing arrived.

The `mcts` crate is already complete. It exposes exactly the four
functions this chapter needs (covered in detail below):
`search`, `visit_distribution`, `select_move`, and `add_dirichlet_noise`.
The `Evaluator` seam is the same one built in the mcts tutorial: the
`tactics` evaluator is a drop-in occupant, and later evaluators enroll
without redesign.

The study-table model from the primer still rules: there are no
"play-the-AI" or "self-play" modes. There are only two seats, each
occupied by a human or an AI, and `Step` asks the AI occupant of the
side-to-move seat to search and play.

## Intention

1. Create `gomoku/crates/cli/src/actor.rs` with `Occupant`,
   `OccupantKind`, `AiConfig`, `EvaluatorKind`, and `SearchInfo`.
   `SearchInfo::from_search` must collect root-edge statistics,
   sort visits and prior descending, and compute the root value as
   `sum(W) / sum(N)`.
2. Extend `gomoku/crates/cli/src/session.rs`:
   * store a `HashMap<HistoryNodeId, SearchInfo>`;
   * implement `execute_step` by rebuilding `board()`, calling
     `mcts::search`, optionally adding Dirichlet noise, building
     `SearchInfo`, selecting a move, playing it, and storing the info;
   * seed `StdRng` from `SessionInit::seed`;
   * expose `search_info()` at the cursor.
3. Extend `gomoku/crates/cli/src/bin/sessiondemo.rs` to recognize
   `info` and print the cursor's `SearchInfo`.
4. Create `gomoku/crates/cli/tests/actor.rs` with the headless tests
   listed in the TDD checklist.
5. Understand and document the value-sign derivation from the MCTS
   backup convention.

Observable done-state: the scripted demo prints a real AI-vs-human
exchange and an `info` block; the TDD checklist passes; the gates are
clean.

## Mental mapping

### The AI is just another occupant

Do not think "now I add an AI mode." Think "now I add a second kind of
occupant." The seat table already existed in chapter 02; this chapter
only fills the `Ai` variant. Hotseat, play-against-AI, and self-play
are initial seat assignments, not separate code paths.

### The mcts refresher — four functions, exact signatures

The `mcts` crate is the authority; the tutorial chapters explain why
these functions look the way they do.

```rust
pub fn search(
    root: &Board,
    evaluator: &mut dyn Evaluator,
    config: &SearchConfig,
) -> SearchOutcome;
```

`search` runs `config.simulations` PUCT descents from `root`, using the
evaluator for priors and leaf values, and returns the full tree. See
[mcts tutorial chapter 07 — the simulation loop](../mcts-tutorial/07-the-simulation-loop.md)
for the select/expand/backup loop and
[chapter 02 — the arena](../mcts-tutorial/02-the-arena.md) for the
arena tree.

```rust
pub fn visit_distribution(tree: &Tree, root: NodeId) -> Vec<(Move, u32)>;
```

After search, the improved policy is the visit-count distribution over
root edges. See
[mcts tutorial chapter 08 — from tree to move](../mcts-tutorial/08-from-tree-to-move.md).

```rust
pub fn select_move(
    dist: &[(Move, u32)],
    temperature: f32,
    rng: &mut impl Rng,
) -> Option<Move>;
```

`temperature == 0.0` plays argmax (ties broken by lowest `Move::index`);
`temperature == 1.0` samples proportional to visits. The same chapter
explains the temperature schedule used in self-play.

```rust
pub fn add_dirichlet_noise(
    tree: &mut Tree,
    root: NodeId,
    eps: f32,
    alpha: f32,
    rng: &mut impl Rng,
);
```

Dirichlet noise mixes into the root priors for opening variety. It is
applied only when `AiConfig::noise_epsilon > 0.0`. See
[mcts tutorial chapter 08](../mcts-tutorial/08-from-tree-to-move.md) and the opening
schedule discussion in the playable CLI primer.

`SessionCore::execute_step` is a straight-line composition of these
four functions plus the history tree. There is no hidden state beyond
the RNG and the stored `SearchInfo`.

### SearchInfo: the record of a search

`SearchInfo` lives in `src/actor.rs` and is stored per history node:

```rust
pub struct SearchInfo {
    pub visits: Vec<(Move, u32)>,   // sorted by visits descending
    pub prior: Vec<(Move, f32)>,    // sorted by prior descending
    pub value: f32,                 // side-to-move perspective
    pub config: AiConfig,           // fingerprint at search time
}
```

`visits` is the root visit-count distribution. `prior` is the root
edge's post-softmax probability before (or, if noise was enabled,
after) Dirichlet mixing. `config` is a copy of the `AiConfig` that ran
the search, so a later `SetSims` cannot silently make an old
`SearchInfo` look as if it used the new budget.

### Why the value sign is what it is

This is the chapter's central conceptual knot. `SearchInfo::value` is
"the estimated value of the root position from the side-to-move's
perspective." The `mcts` crate's `backup` function makes that true.

`mcts::backup` (see `gomoku/crates/mcts/src/backup.rs`) receives a
`leaf_value` that is from the **leaf player's perspective**. It walks
the selection path from leaf back to root and, at every edge, flips the
sign. The comment in `src/actor.rs` states the consequence:

> Each edge therefore accumulates a value `W` from the perspective of
the player who chose at its parent node. For a root edge, that parent
is the root, so `W` is already from the root player's (i.e.
side-to-move's) point of view. The root value estimate is the
visit-weighted average of those root-edge values: `sum(W) / sum(N)`.
A positive value favours the side to move.

That is why `SearchInfo::from_search` can simply sum `e.w` over root
edges, divide by total visits, and call it the side-to-move value. No
extra sign flip is needed at the root: the backup convention already
did the work.

This convention is easy to get wrong. If you ever find yourself
negating `value` again in the front-end, stop: the convention says the
value is already from the perspective of whoever was to move when the
search ran. Chapter 6's value strip will display it exactly as stored.

### Sidebar: there is no depth knob

The primer's four-knob table has no depth row, and `SearchInfo` has no
depth field. MCTS depth is emergent: the tree grows as deep as the
simulations push it. To make that concrete, run the same position with
two simulation budgets and look only at the `SearchInfo`.

The position is the win-in-one setup from
`tactics_ai_takes_win_in_one` in `tests/actor.rs`: Black has an open
four on row 7 and can win by playing `c8` or `h8`. Measured in the
reference worktree at commit `baa2c1a`:

| Simulations | `value` | total visits | top move(s) |
|-------------|---------|--------------|-------------|
| 50 | +1.000 | 50 | `c8` with 50 visits |
| 800 | +1.000 | 800 | `c8` and `h8` with 400 visits each |

There is no `depth` field anywhere. The only knob that changed was
`simulations`. If you find yourself wanting a depth parameter, what
you actually want is more simulations. See the primer's "four knobs"
section and spec §6.1.

### The tactics evaluator's blindness is a feature

`TacticsEvaluator` shapes priors from engine truth: immediate wins and
forced blocks get most of the prior mass; everything else is uniform.
On a quiet board it has **no positional sense at all**. The demo below
shows exactly that: after a human center stone, the AI plays the corner
`a1`, then `b1`, then `c1` — the argmax tie-break on a flat
visit distribution. It looks silly, and it is. The baseline tutorial
exists specifically to cure this blindness with a positional
(gravity-style) evaluator; this tutorial uses the silly moves as a
clear baseline.

## Low-level design

### Files changed vs chapter 02

The diff from chapter 02 (`161c49b`) to this chapter (`baa2c1a`) is
confined to the `cli` crate:

```text
$ git diff 161c49b..baa2c1a --stat -- gomoku/crates/cli
 gomoku/crates/cli/Cargo.toml             |   1 +
 gomoku/crates/cli/demo_script.txt        |   8 +
 gomoku/crates/cli/src/actor.rs           |  63 +++++++
 gomoku/crates/cli/src/bin/sessiondemo.rs |  39 ++++-
 gomoku/crates/cli/src/command.rs         |   9 +-
 gomoku/crates/cli/src/session.rs         | 235 +++++++++++++++++++++-----
 gomoku/crates/cli/tests/actor.rs         | 276 +++++++++++++++++++++++++++++++
 gomoku/crates/cli/tests/session.rs       |  72 ++++++--
 8 files changed, 643 insertions(+), 60 deletions(-)
```

`src/history.rs` and `src/fmt.rs` do not appear in the diff: they are
unchanged from chapter 02. The `Cargo.toml` change adds the `mcts`
dependency.

### `src/actor.rs`

New file. It owns `Occupant`, `OccupantKind`, `AiConfig`,
`EvaluatorKind`, and `SearchInfo`. The key computation is in
`SearchInfo::from_search`:

```rust
let total_visits: u32 = edges.iter().map(|e| e.n).sum();
let total_value: f32 = edges.iter().map(|e| e.w).sum();
let value = if total_visits > 0 {
    total_value / total_visits as f32
} else {
    0.0
};
```

Visits and prior are sorted descending, with `Move::index` as a
deterministic tie-breaker, so the front-end can display a stable top-N.

### `src/session.rs`

* Add `search_info: HashMap<HistoryNodeId, SearchInfo>`.
* `new` seeds `StdRng` from `init.seed`, falling back to
  `rand::random::<u64>()` when no seed is supplied.
* `execute_release` builds a fresh `Ai(default_config)` for the target
  seat, applying any per-seat simulation override (`black_sims` or
  `white_sims`).
* `execute_step` is the heart:
  1. Check the game is ongoing and the side-to-move seat is `Ai`.
  2. Build the evaluator from `EvaluatorKind`.
  3. Run `mcts::search`.
  4. If `noise_epsilon > 0.0`, call `mcts::add_dirichlet_noise`.
  5. Build `SearchInfo` from the outcome.
  6. Extract visits with `mcts::visit_distribution`.
  7. Select a move with `mcts::select_move`.
  8. Play the move in the history tree.
  9. Store the `SearchInfo` keyed by the new cursor node.
  10. Emit `Moved { by: Ai }`, plus `Branched` / `GameOver` as needed.
* Add `search_info()` to retrieve the stored record at the cursor.

### `src/command.rs`

No new command variants are required — `Step` was already parsed in
chapter 02. The only change is that `Step` now executes instead of
returning `StepRequiresAi` (except when the side-to-move seat is human,
in which case the error still applies).

### `src/bin/sessiondemo.rs`

Add a top-level `info` branch in `process_line` and a
`print_search_info` helper. The helper prints `value`, total visits,
and the top five moves with their visit counts and prior percentages.

### `tests/actor.rs`

New headless tests for AI seats and `SearchInfo`. See the TDD
checklist for exact names.

### `demo_script.txt`

A short script used by the capstone:

```text
release white
h8
step
h7
step
i8
step
info
```

## Solution (opt-in)

The complete reference files for this chapter are quoted verbatim in
[03-the-ai-takes-a-seat/01-solution.md](03-the-ai-takes-a-seat/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare your work.

## TDD checklist

Run from `gomoku/` with `cargo test -p cli`.

### `tests/actor.rs`

Exact `#[test]` function names from commit `baa2c1a`:

1. `tactics_ai_takes_win_in_one`
2. `tactics_ai_blocks_opponent_open_four`
3. `seeded_determinism_same_seed_same_events_and_search_info`
4. `different_sims_per_seat_yield_different_total_visits`
5. `step_on_human_seat_turn_is_typed_error`
6. `step_after_game_over_is_error`
7. `search_info_prior_and_visits_are_sorted_descending`

### `tests/session.rs` and `tests/command.rs`

All chapter-02 tests continue to pass and are part of the chapter-03
gate. Notably:

* `step_on_human_seat_returns_typed_error` (in `tests/session.rs`) must
  still reject `Step` when both seats are human.
* Parser tests must still round-trip `step`, `release white`,
  `sims black 800`, etc.

### Red-green-refactor rhythm

1. **Red:** Add `src/actor.rs`, extend `src/session.rs` and
   `src/bin/sessiondemo.rs`, and add `tests/actor.rs`. Run
   `cargo test -p cli`. Expect failures.
2. **Green:** Implement the contracts above until `cargo test -p cli`
   passes.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any issues.

## Capstone: the AI answers and shows its working

Run the scripted demo in the worktree:

```text
cd gomoku
cargo run -q -p cli --bin sessiondemo -- --script ../crates/cli/demo_script.txt
```

Key frames from the real output (measured at commit `baa2c1a`):

Releasing White:

```text
  White seat is now Ai
ply 0  status Ongoing  black Human  white Ai
```

A human move:

```text
  Moved h8 by Human
ply 1  status Ongoing  black Human  white Ai
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 5  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 8  .  .  .  .  .  .  .  x  .  .  .  .  .  .  .  
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
```

The AI's answer (note the corner junk — the uniform prior at work):

```text
  Moved a1 by Ai
ply 2  status Ongoing  black Human  white Ai
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  o  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 5  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 8  .  .  .  .  .  .  .  X  .  .  .  .  .  .  .  
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
```

After the script's final `info`, the stored `SearchInfo` at the cursor:

```text
value 0.000  total visits 400
top moves (visits, prior%):
  1. (2, 0.5%) c1
  2. (2, 0.5%) d1
  3. (2, 0.5%) e1
  4. (2, 0.5%) f1
  5. (2, 0.5%) g1
```

On a quiet board the search has nothing to prefer, so the top moves
are scattered along the first row with two visits each and a flat
0.5% prior. The AI sat down, answered, and honestly reported that it
had no idea what to do. The baseline tutorial will teach it better.
## Done when

* `cargo test -p cli` passes from `gomoku/` (including all chapter-02
  tests and the new `tests/actor.rs` tests).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* The `sessiondemo` scripted demo prints the frames above.
* Commit message for the code slice:
  `feat(cli): AI seats via mcts + SearchInfo + seeded RNG`.

Next: [Chapter 04 — The study loop](04-the-study-loop.md)

## References

* [`00-playable-cli-primer.md`](00-playable-cli-primer.md) — the study
  table, core/shell, the four knobs, the no-depth-knob sidebar, and
  the opening schedule.
* [`README.md`](README.md) — chapter map and tutorial conventions.
* [`docs/plans/2026-10-17-playable-cli.md`](../../plans/2026-10-17-playable-cli.md)
  — Task 3 (this chapter).
* [`docs/specs/2026-10-17-playable-cli-design.md`](../../specs/2026-10-17-playable-cli-design.md)
  — §6 (actors, `SearchInfo`, seeded RNG) and §6.1 (no depth knob).
* [`docs/tutorials/mcts-tutorial/00-mcts-primer.md`](../mcts-tutorial/00-mcts-primer.md)
  — MCTS vocabulary and conventions used by the `mcts` crate.
* [`docs/tutorials/mcts-tutorial/07-the-simulation-loop.md`](../mcts-tutorial/07-the-simulation-loop.md)
  — the `search` function and the select/expand/backup loop.
* [`docs/tutorials/mcts-tutorial/08-from-tree-to-move.md`](../mcts-tutorial/08-from-tree-to-move.md)
  — `visit_distribution`, `select_move`, `add_dirichlet_noise`, and
  temperature.
* [`docs/tutorials/mcts-tutorial/09-the-tactics-evaluator.md`](../mcts-tutorial/09-the-tactics-evaluator.md)
  — the `TacticsEvaluator` used by the first AI seat.
* `gomoku/crates/mcts/src/backup.rs` — the negate-once backup
  convention.
