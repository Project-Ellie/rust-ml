# Implementation plan — Acceptance: the measured ladder

The layer between the chapter and the solution: construction
instructions for the learner who builds the code themselves. Signatures
are prose; method bodies never appear. Each artifact names the tests
that go red, then green.

## File layout

Everything lands in one new file,
`gomoku/crates/baseline/tests/acceptance.rs`. One edit lands in
`gomoku/crates/cli/src/main.rs` (the flip). No library code changes.

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
* `fn baseline_best_move(board, simulations, rng, &mut BaselineEvaluator) -> Move`
  — build a `SearchConfig` (the budget, `c_puct` 1.5), run
  `mcts::search`, take `visit_distribution` at the root, and
  `select_move` at temperature 0.0. `tactics_best_move` is the same
  shape for the mock. The evaluator is passed in, not created inside:
  constructing the `ThreatNet` per move would multiply the cost.
* `fn play_game(black: Brain, white: Brain, seed: u64) -> (Status, u32, u64, f64)`
  — fresh `Board`, fresh `StdRng` from the seed, one evaluator per
  brain kind created once; loop to a terminal status or 225 plies;
  assert every move is legal before playing it. Return the status, ply
  count, and the baseline's evaluation count and wall time (the
  throughput raw material).

Red → green: write `win_in_one_is_solved_at_50_sims` now (§4) and
watch it fail to compile until the drivers exist.

## 3. `solve_puzzle`

`fn solve_puzzle(puzzle: &Puzzle, simulations: u32, seed: u64) -> bool`.
Loop at most `solution.len() + 4` plies. Terminal status decides:
attacker won ⇒ true, anything else ⇒ false. On the attacker's move:
`baseline::probe::root_probe` first (the CLI seat's exact wiring), the
searched move on `None`. On the defender's move: the pack's claimed
move for that ply when its side matches, else a seeded random move.
Illegal move ⇒ false. After the loop, the final status decides.

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
print seconds/game and evals/sec), and the VCF solve rate (seeded
Fisher-Yates prefix sample across the five pack files, assert the
≥50% bar, print progress every ten puzzles).

Idiom watch: table rows are printed with fixed-width `println!`
formatting; the numbers you quote in the chapter are the ones this
test prints — never transcribe from a paraphrase.

## 6. The flip

In `crates/cli/src/main.rs`, `ai_config_from`'s fallback becomes
`EvaluatorKind::Baseline`. Run `cargo test -p cli`: the chapter-09
acceptance suite constructs `EvaluatorKind::Tactics` explicitly and
stays green; the `args` tests pin flag behavior, not the default.

## Gates

From `gomoku/`: `cargo test -p baseline`, `cargo test -p cli`,
`cargo clippy --all-targets -- -D warnings`,
`cargo fmt --all -- --check`, then the measurement:
`cargo test -p baseline --release --test acceptance -- --ignored --nocapture`.
Commit: `test(baseline): the measured ladder — baseline sub-chapter 09`.
