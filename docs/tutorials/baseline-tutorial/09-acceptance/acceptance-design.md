# Design paper — Acceptance: the measured ladder

## The story

A player that has never played a recorded game is a hypothesis, not a
player. Seven sub-chapters stacked layer on layer — gravity, tiers,
value, the AI player with its own TSS probe and opponent-proof guard
(the merged sub-chapter 08) — and every
layer arrived with unit tests that prove the *mechanism* works. This sub-chapter asks the only question
left: does the whole thing win games, and how would you know?

The design hangs on a separation the CLI tutorial's chapter 09 already
taught: **certainties are asserted, strength is measured.** The two
never mix. An assert runs on every `cargo test` and must hold
forever — a win-in-1 found at 50 simulations is a fact about the tier
table. A measurement runs on demand, in release, takes half an hour,
and produces a number that is *recorded*, not enforced — except for
its pre-registered bar, the one sentence you wrote down before you
knew the answer.

The file `gomoku/crates/baseline/tests/acceptance.rs` therefore has
two halves. The always-on half holds the tactical asserts and the
health flood. The `#[ignore]`d half holds the strength ladder: one
test, one protocol, four tables.

## The cast

The games are driven headless, straight through the `mcts` public API
— the study table is a human surface, and twenty-game matches do not
belong in a terminal. Three helpers carry the whole show.

`Brain` is the cast list: an enum with one variant per possible
occupant — `Random`, `Baseline(sims)`, `Tactics(sims)`. Every pairing
the protocol needs is two enum values.

`play_game` is the referee. It takes two `Brain`s and a seed, plays to
a terminal status, and returns the status, the ply count, and a
`BaselineMetrics` record. That record tracks `requested_sims` (the sum
of configured simulation budgets) and `search_time_secs` (wall time in
the baseline's move helper, including the root TSS probe). The derived
`requested_sims / search_time_secs` is a configured-budget rate, not a
leaf-evaluation throughput and not a lower bound. It owns the legality
assert — every move, every game, `board.is_legal(mv)` — so the flood
and the match share one notion of a legal game.

`solve_puzzle_with` is the generic examiner. It seats any attacker
`E: Evaluator` — wrapped in the seat's `player::AiPlayer` — and lets
the player's own TSS probe run at the start of every `decide`. The
defender follows the pack's claimed line for the current ply, using
`solution.get(ply)` guarded by side so successive defender replies are
not replayed. It returns both a solved boolean and the list of
defender replies, which lets a regression test assert that distinct
replies were used. `solve_puzzle` is the production wrapper that runs
it with a fresh `BaselineEvaluator` and `StdRng`.

Two older wirings are historical, not the active design: the original
CLI-probe harness called `baseline::probe::root_probe` directly, and
the intermediate hook-era design let `mcts::search` call an
evaluator-owned `evaluate_root` (retired before landing). The proof
now lives in the `player` crate.

And `pack`, a private module, is the deliberate duplication: a minimal
`PuzzlePack` parser, copied in spirit from `cli::puzzle`, because
depending on `cli` from a `baseline` test would close a dependency
cycle. The comment at its top names the precedent.

## One game, end to end

```mermaid
sequenceDiagram
    participant test as acceptance test
    participant game as play game
    participant brain as brain
    participant search as search
    participant board as board
    test->>game: two brains and a seed
    loop until terminal status
        game->>brain: whose side is to move
        brain->>search: run the budget
        search-->>brain: a move
        brain-->>game: the move
        game->>board: assert legal then play
        board-->>game: new status
    end
    game-->>test: status and plies and timing
```

## The invariants

1. **Asserts and measurements never mix.** The default suite stays
   fast and deterministic; the ladder is opt-in and release-only.
2. **The bar is written before the number.** Both pre-registered bars
   (≥50% against the mock, ≥50% of the VCF sample) live in the test as
   assertions; the measured numbers are recorded whatever they are.
   The numbers quoted in this chapter are from the old
   `baseline-reference` CLI-probe integration and are kept as
   historical provenance only.
3. **Every game is reproducible.** Seeds derive from one base seed by
   level and game index; the same protocol reproduces the same table.
4. **Colors alternate in the match.** First-move advantage never
   enters the score.
5. **The random driver is a sanity check, not a yardstick.** Its
   table carries no bar, and the chapter says why.
6. **The throughput line is honest about its semantics.** The printed
   "configured-budget rate" is `requested_sims / search_wall_time`; it
   is neither leaf-evaluation throughput nor a lower bound. Root TSS
   work is included in wall time, and a root proof may credit the full
   configured budget while running zero simulations.
7. **The flood is honest about its trade-off.** It runs the fast
   evaluator and says so in a comment — the plumbing it exercises is
   evaluator-agnostic; baseline behavior is covered elsewhere.
8. **The proof path is player-level.** The own TSS probe runs inside
   `AiPlayer::decide`, before the search; no wrapper around the
   `Evaluator` can strip it, because the evaluator has no root hook to
   forward — the seam stays one-method.

## The flip as ceremony

The last act is one proposed line: `ai_config_from`'s fallback becomes
`EvaluatorKind::Baseline`. The CLI's default brain — what an `--ai`-less
`release` mid-session builds — would become the player this tutorial
 grew once the flip lands in main. The mock stays one flag away
(`--ai tactics`), because a yardstick never leaves the toolbox: the
net tutorial's first trained network will meet both.

## Noun catalog

| Name | Kind | Role |
|------|------|------|
| `Brain` | enum | the three seatable opponents |
| `play_game` | function | the referee: one game to a terminal status |
| `BaselineMetrics` | struct | per-search configured-budget accounting |
| `best_move` | function | drives the seat's `AiPlayer::decide` and plays `decision.mv` |
| `baseline_best_move` / `tactics_best_move` | functions | thin wrappers around `best_move` with the seat's persistent player |
| `solve_puzzle_with<E: Evaluator>` | function | generic puzzle examiner; wraps the evaluator in an `AiPlayer`; returns `(solved, defender_replies)` |
| `solve_puzzle` | function | production wrapper that runs `solve_puzzle_with` with `BaselineEvaluator` |
| `player::AiPlayer` | struct | the per-seat decision pipeline: own TSS probe, one `mcts::search`, opponent-proof guard |
| `random_move` | function | one seeded uniform legal move |
| `pack::PuzzlePack` | struct | the deliberately duplicated pack parser |
| `strength_ladder_measured` | test | the ignored measurement: four tables, two bars |
| `quiet_whisper_ab_measured` | test | the preserved ignored A/B from sub-chapter 07 |
| `win_in_one_is_solved_at_50_sims`, `forced_block_is_taken_at_50_sims` | tests | the tactical asserts |
| `fifty_seeded_self_play_games_are_legal` | test | the health flood |
| `solve_puzzle_follows_successive_defender_replies` | test | regression: defender branch indexes by ply, not by `find` |

## References

* [`acceptance.md`](acceptance.md) — the sub-chapter.
* [`../00-baseline-primer.md`](../00-baseline-primer.md) — §9 the
  strength-ladder contract.
* [Playable CLI chapter 09](../../playable-cli-tutorial/09-acceptance.md)
  — the ritual this design extends.
