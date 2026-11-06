# Implementation plan — Acceptance: the measured ladder

The layer between the chapter and the solution: construction
instructions for the learner who builds the code themselves. Signatures
are prose; method bodies never appear. Each artifact names the tests
that go red, then green.

## File layout

Everything lands in one new file,
`gomoku/crates/baseline/tests/acceptance.rs`. One edit lands in
`gomoku/crates/cli/src/main.rs` (the proposed flip). No library code
changes.

## 0. Dev-dependency migration

The `baseline` crate needs `serde` (with `derive`) and `serde_json` as
`dev-dependencies` so the test file can parse puzzle packs, plus the
sub-chapter 08 `player` crate so the harness can drive games through
`AiPlayer::decide`. Add them
in `gomoku/crates/baseline/Cargo.toml` under `[dev-dependencies]` using
the workspace versions:

```toml
[dev-dependencies]
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
player = { path = "../player" }
```

These are test-only dependencies; they do not create a production
dependency cycle with `cli`.

## 1. The `pack` module

A private module at the top of the test file, behind a comment that
names the deliberate-duplication precedent (`cli` depends on
`baseline`, so the test cannot depend on `cli`).

* `struct Puzzle` — fields: zero-based `index`, `attacker: Color`,
  `board: Board`, `solution: Vec<(Move, Color)>`.
* `struct PuzzlePack` — one field: `items: Vec<Puzzle>`.
* `PuzzlePack::from_json(text) -> Result<Self, String>` — deserialize
  with a private serde mirror of the pack format (`count`, `items`
  with `i`, `a`, `b`, `w`, `l`); reject a count mismatch, an unknown
  attacker string, duplicate or overlapping stones, off-board
  coordinates (remember: file coordinates are `[x, y]` with
  `x` the column — `Move::new(y, x)`), and a line whose sides do not
  alternate from the attacker. Build each board with
  `Board::from_position`.
* `PuzzlePack::from_path(path) -> Self` — anchor the path with
  `env!("CARGO_MANIFEST_DIR")` (the packs live at
  `../../../data/puzzles/` from the crate), read, parse, panic with a
  naming message on failure.

Red → green: none of this is separately unit-tested here — it is
exercised by the VCF section of the measurement; the pack parser's own
proof lives in the CLI tutorial's sub-chapter 07a.

## 2. The drivers

* `enum Brain` — variants `Random`, `Baseline(u32)`, `Tactics(u32)`.
  Doc comment: the brain seated at one side.
* `fn random_move(board, rng) -> Move` — collect `empty_moves`, pick
  uniformly with the seeded `StdRng`.
* `fn best_move(player: &mut player::AiPlayer, board, options, rng) -> Move`
  — call `player.decide(board, options, rng)` and return
  `decision.mv`. The own TSS probe and the opponent-proof guard run
  inside `decide`: a certified own proof returns its move directly
  (no search, no noise, no search record); a searched decision plays
  the guard-filtered proposal. Build the `SearchConfig`-level knobs
  (`simulations`, `c_puct` 1.5) into the `PlayerOptions` alongside
  `own: OwnTssConfig::baseline()` and `guard: GuardConfig::baseline()`
  for the baseline seat; the tactics seat uses the same shape with
  both disabled.
* `fn baseline_best_move(...)` / `fn tactics_best_move(...)` — thin
  wrappers that call `best_move` with the seat's persistent player.
  The player is passed in, not created inside: constructing the
  `ThreatNet` per move would multiply the cost, and the player's whole
  point is one persistent evaluator per seat.
* `BaselineMetrics` — a struct with `searches: u64`,
  `requested_sims: u64`, and `search_time_secs: f64`. The derived
  `requested_sims / search_time_secs` is a configured-budget rate, not
  a leaf-evaluation throughput and not a lower bound.
* `fn play_game(black: Brain, white: Brain, seed: u64) -> (Status, u32, BaselineMetrics)`
  — fresh `Board`, fresh `StdRng` from the seed, one evaluator and one
  `AiPlayer` per brain kind created once; loop to a terminal status or
  225 plies;
  assert every move is legal before playing it. Return the status, ply
  count, and the baseline's configured-budget metrics.

Red → green: write `win_in_one_is_solved_at_50_sims` now (§4) and
watch it fail to compile until the drivers exist.

## 3. `solve_puzzle_with` and `solve_puzzle`

`fn solve_puzzle_with<E: Evaluator>(
    puzzle: &Puzzle,
    simulations: u32,
    ev: &mut E,
    rng: &mut StdRng,
) -> (bool, Vec<Move>)` — the evaluator is wrapped in an `AiPlayer`
for the attacker's moves, so the own TSS probe exercises the real
proof path. Loop at most `solution.len() + 4` plies. Terminal status decides:
attacker won ⇒ `(true, defender_replies)`, anything else ⇒
`(false, defender_replies)`. On the attacker's move: call the player's
`decide` through `best_move`. On the defender's move: use
`puzzle.solution.get(ply)` and guard by side (`(_, c)| c == side`);
if the line is shorter than expected or the color does not match,
fall back to a seeded random move. This indexes successive defender
replies by ply; do not use `find`, which would replay the first
defender move forever. Collect each defender reply in `defender_replies`.
Illegal move ⇒ return false. After the loop, the final status decides.

`fn solve_puzzle(puzzle: &Puzzle, simulations: u32, seed: u64) -> bool`
is the production wrapper that creates a fresh `BaselineEvaluator` and
`StdRng` and returns only the boolean.

There is no `baseline::probe` module and no evaluator root hook: the
proof runs inside `player::AiPlayer::decide`. Never call the prover
directly from the harness — the player is the certifier.

## 4. The tactical asserts

* `win_in_one_is_solved_at_50_sims` — the board from the chapter (four
  in a row, Black to move) via `engine::reference::board_from_ascii`;
  a 50-simulation `baseline_best_move`; assert membership in
  `engine::immediate_wins`.
* `forced_block_is_taken_at_50_sims` — script eight plies that leave
  White exactly one winning cell (`engine::forced_blocks` has length
  1 — assert that first); a 50-simulation search takes it.
* `fifty_seeded_self_play_games_are_legal` — 50 seeded
  `Brain::Tactics(1)` vs `Brain::Tactics(1)` games; assert
  termination within 225 plies. The comment explains why the flood
  runs the fast evaluator.

## 5. The measurement

`#[ignore] #[test] fn strength_ladder_measured()`. Constants first:
`RANDOM_GAMES: 10`, `MOCK_GAMES: 20`, `VCF_SAMPLE: 100`,
`BASE_SEED: 20_260_801`. Then the four sections in protocol order:
tactical re-check, the sims gradient (400 / 4,000 / 10,000 vs
`Brain::Random`, seeds `BASE_SEED + g + sims * 10_000`), the match
(200 sims both sides, colors alternate by game index, seeds
`BASE_SEED + g + 200 * 100_000`, assert the pre-registered ≥50% bar,
print seconds/game and configured_budget_rate), and the VCF solve
rate (seeded Fisher-Yates prefix sample across the six pack files
listed below, assert the ≥50% bar, print progress every ten puzzles).

Pack files: `kaibao-vcf-676.json`, the four
`renju-portal-vcf-783-*of4.json` shards, and
`vcf-material-763.json`.

Idiom watch: table rows are printed with fixed-width `println!`
formatting; the numbers you quote in the chapter are the ones this
test prints — never transcribe from a paraphrase. This test is
`#[ignore]`; it is a measurement, not a gate.

## 5b. Preserved ignored A/B: `quiet_whisper_ab_measured`

Keep the `#[ignore]`d quiet-whisper A/B test from sub-chapter 07 in
this file if it was already there. It plays a small ladder of games
with and without the whisper and prints the win rates; it is not a
chapter-09 gate, just a preserved measurement anchor.

## 5c. Decorator-forwarding regression test — retired

The hook-era suite carried
`decorated_baseline_forwards_evaluate_root_and_bypasses_leaf_eval`, a
regression shield for wrappers that forgot to forward the optional
`evaluate_root` method. The root hook was retired before landing
(2026-11-06): there is no `evaluate_root` for a wrapper to forget, and
certification lives in the `player` crate, outside the evaluator — no
wrapper around `Evaluator` can strip it. No replacement test is needed
here; the probe's own tests live in the `player` crate (sub-chapter
08).

## 5d. Successive-defender-replies regression test

`solve_puzzle_follows_successive_defender_replies` uses a puzzle whose
solution line contains distinct defender replies at successive plies.
It runs `solve_puzzle_with`, collects the defender replies, and
asserts that the sequence is non-trivial (more than one distinct move,
or at least longer than a `find`-first replay would produce). This
prevents the bug where `find` always returns the first defender move
and the puzzle is only "solved" because the defender repeats it.

## 6. The flip

In `crates/cli/src/main.rs`, `ai_config_from`'s fallback should become
`EvaluatorKind::Baseline` once the change lands in main. This is a
proposed one-line action, not a currently verified source line. Run
`cargo test -p cli`: the chapter-09 acceptance suite constructs
`EvaluatorKind::Tactics` explicitly and stays green; the `args` tests
pin flag behavior, not the default.

## Gates

From `gomoku/`: `cargo test -p baseline`, `cargo test -p cli`,
`cargo test -p player`,
`cargo clippy --all-targets -- -D warnings`,
`cargo fmt --all -- --check`, then the measurement:
`cargo test -p baseline --release --test acceptance -- --ignored --nocapture`.
Commit: `test(baseline): the measured ladder — baseline sub-chapter 09`.
