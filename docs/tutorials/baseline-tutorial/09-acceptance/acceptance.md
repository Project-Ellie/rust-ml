# Sub-chapter 09 — Acceptance: the measured ladder

## Abstract

The final sub-chapter of the baseline tutorial. It adds no new
intelligence; it pins what the seat can do with three acceptance
artifacts: tactical asserts that run on every `cargo test`, a self-play
health flood, and a release-only `#[ignore]`d measurement — the
**strength ladder** — that plays the finished baseline against a
uniform-random driver at three simulation levels, against the
`TacticsEvaluator` in a seeded match, and against a sample of the
engine-proven VCF puzzle packs. The measured numbers below are from
the old `baseline-reference` CLI-probe integration; they are kept as
historical provenance, not as measurements of the current player-level
proof path (the intermediate root-hook design was retired before
landing, and the merged sub-chapter 08 moved the proof into the
`player` crate — the path changed twice since those numbers). The chapter ends with the ceremonial flip: `baseline` becomes the
CLI's default brain.

## Glossary

| Term | Definition |
|------|------------|
| **Strength ladder** | The tutorial's acceptance narrative: one fixed protocol, rerun after every sub-chapter, so each layer of intelligence shows up as a better number against the same yardsticks. |
| **Measured hypothesis** | A falsifiable numeric claim about behavior, pre-registered with its protocol, then measured and recorded as a real number. |
| **Uniform-random driver** | An opponent playing seeded uniform-random legal moves. A sanity check, not a yardstick (the CLI chapter-09 lesson). |
| **`Brain`** | The test-local enum (`Random`, `Baseline(sims)`, `Tactics(sims)`) that seats one side of a driven game. |
| **VCF solve rate** | The fraction of sampled engine-proven puzzles the baseline converts from the puzzle root, playing the attacker at 4,000 sims through the `player` crate's `AiPlayer` with its own TSS probe active, within `line length + 4` plies. |
| **Pre-registered bar** | The pass/fail threshold written into the test *before* the first run. The measured number stands whatever it is. |
| **The flip** | The one-line change in `ai_config_from` that makes `EvaluatorKind::Baseline` the CLI's default brain. |

## Context

Sub-chapters 01–08 built the whole player: the seat, the vicinity
kernel, gravity priors, wins and blocks, the honest value, the three
ladder, the quiet whisper, and the AI player itself (the merged
sub-chapter 08: the player's own TSS probe plus the opponent-proof
guard). Every layer was unit-tested as it landed. What is still
missing is the answer to the only question a player ultimately answers
to: **does it win, against whom, by how much, and how do you know?**

One honesty note on the recorded numbers below: they were measured on
the `baseline-reference` branch, which wired the TSS proof as a
`baseline::probe` short-circuit in the CLI step path. The proof path
has changed twice since: an intermediate redesign moved the proof
behind an evaluator-owned root hook (retired before landing on
`main`), and the current merged sub-chapter 08 design runs it at
player level — the `player` crate's `AiPlayer::decide` probes
`engine::prove_forced_win` once per move, before the search, while
`mcts::search` keeps returning `SearchOutcome`. The functional shape
of the player-level design is verified in the uncommitted reference at
`../rust-ml-unified08-ref` (563 passed / 0 failed / 5 ignored), but no
new strength ladder has been run. The numbers below are therefore
**historical baseline-reference / CLI-probe measurements**, kept for
provenance and rerun protocol only. Do not quote them as verified
measurements of the player-level design.

A second honesty note: the ladder was measured for the six-layer
player, *before* sub-chapter 07's quiet whisper landed. That is the
right order — the whisper was an `[experiment]` and had to earn its
place with its own A/B first. It did: 20/20 against the
`TacticsEvaluator` in both arms, but the whisper arm finished its wins
in 23.5 plies on average against 34.5 without it. Since the whisper is
on by default, **your run of this sub-chapter includes it** — expect
equal or better numbers, and if a number moves, you are watching one
experiment's verdict arrive in another experiment's protocol. Record
what you get.

The CLI tutorial's chapter 09 already taught the trap: a 100% win rate
against uniform-random play is not a strength claim, it is the minimum
bar — and it says nothing about the gradient between 50 and 800
simulations. This sub-chapter applies that lesson to the baseline
itself: random play appears in the protocol (as the sanity check), but
the yardstick is the `TacticsEvaluator`, and the tactical claim is
measured against the engine-proven VCF packs.

## Intent

Add `gomoku/crates/baseline/tests/acceptance.rs` with three always-on
tests and one ignored measurement:

1. `win_in_one_is_solved_at_50_sims` — on a board with an immediate
   five, a 50-simulation search with the baseline evaluator selects an
   immediate win.
2. `forced_block_is_taken_at_50_sims` — on a board where White has
   exactly one winning cell, a 50-simulation search takes the block.
3. `fifty_seeded_self_play_games_are_legal` — a seeded health flood:
   50 full games, every move legal, no game exceeding 225 plies, no
   panics.
4. `strength_ladder_measured` (`#[ignore]`, release-only) — the
   measured ladder: the sims gradient against the random driver, the
   match against the mock, throughput, and the VCF solve rate.

Then flip the default in `ai_config_from` so the baseline becomes the
brain an `--ai`-less `release` builds, run the gate ritual, and commit.

## Specification as tests

The three asserts are the certainties, verified on every run:

| Test | Claim |
|------|-------|
| `win_in_one_is_solved_at_50_sims` | 50 sims suffice to take an immediate five |
| `forced_block_is_taken_at_50_sims` | 50 sims suffice to see the one forced block |
| `fifty_seeded_self_play_games_are_legal` | the search/evaluator plumbing survives 50 full games |

The measurement carries the pre-registered bars as assertions:

| Measurement | Bar (written before the first run) |
|-------------|------------------------------------|
| Match vs `TacticsEvaluator`, 200 sims, 20 games | win rate ≥ 50% |
| VCF solve rate, 100 sampled puzzles | solved ≥ 50% |

The sims gradient against the random driver has no bar — it is a
sanity check, and the chapter says why.

## The walkthrough, in execution order

### The drivers

A match needs opponents, and the study table is not scriptable at this
volume — so the test drives games directly through the `mcts` public
API: `search`, `visit_distribution`, `select_move`. The `Brain` enum
names the three occupants a game can seat: `Random` (seeded uniform
legal moves), `Baseline(sims)`, and `Tactics(sims)`. One `play_game`
function runs any pairing to a terminal status and returns the status,
the ply count, and a `BaselineMetrics` record — the raw material of the
throughput line.

`BaselineMetrics` is honest about what it measures:

* `requested_sims` is the sum of configured `simulations` arguments
  passed to the baseline's searches.
* `search_time_secs` is wall-clock time spent inside the baseline's
  move helper, including the root TSS probe and all root overhead.
* Therefore `requested_sims / search_time_secs` is a **configured-budget
  rate**, not a count of performed leaf evaluations and not a lower
  bound. A root TSS proof may credit the full configured budget while
  running zero simulations, and root TSS work is included in the wall
  time.

Because move decisions now live in the `player` crate (sub-chapter
08), both `baseline_best_move` and `tactics_best_move` borrow their
seat's persistent `player::AiPlayer` and call `decide`. A certified
own proof returns its move directly and stores no search record; a
searched decision plays the guard-filtered proposal. The intended
helper shape (the harness is built against `gomoku/crates/player`,
which sub-chapter 08 supplies):

```rust
fn best_move(player: &mut AiPlayer, board: &Board, options: &PlayerOptions, rng: &mut StdRng) -> Move
```

`play_game` constructs a fresh `StdRng` per game from the protocol
seed, so every game of the measurement is independently reproducible.

### The tactical asserts

`win_in_one_is_solved_at_50_sims` builds the win-in-1 board with
`engine::reference::board_from_ascii` (Black has four in a row on row
8), runs a 50-simulation search, and asserts the selected move is in
`engine::immediate_wins`. `forced_block_is_taken_at_50_sims` scripts
eight plies that leave White one winning cell, and asserts the search
takes it. Fifty simulations is deliberately small: the trust ladder's
top tiers are exactly the moves PUCT explores first, so the asserts
pin the *point* of the tier table — the right move is found almost
immediately, not eventually.

### The flood

`fifty_seeded_self_play_games_are_legal` plays 50 seeded games at 1
simulation per side and asserts every game terminates within 225
plies with every move legal. The flood uses the `TacticsEvaluator`,
not the baseline — the comment in the test says why: the baseline
evaluator costs milliseconds per call (a real convolution forward),
which makes a 50-game debug-mode flood minutes slow, while the
plumbing the flood exercises — search, backup, legality — is
evaluator-agnostic. Baseline behavior is covered by the tactical
asserts and the release measurement. Record the trade-off in the
comment; never pay it silently.

### The measured ladder

`strength_ladder_measured` is `#[ignore]`d: it is a measurement, not a
gate. Run it explicitly:

```bash
cd gomoku
cargo test -p baseline --release --test acceptance -- --ignored --nocapture
```

The protocol, pre-registered:

1. **Sims gradient vs the random driver.** Baseline (Black) at 400 /
   4,000 / 10,000 simulations against seeded uniform-random White, 10
   games per level, seeds derived from a base seed plus level and game
   index.
2. **The match.** 20 games against the `TacticsEvaluator`, both sides
   at 200 simulations, colors alternating by game index — the mock is
   the yardstick, and alternating colors keeps first-move advantage
   out of the score.
3. **Throughput.** Seconds per game and the configured-budget rate
   (`requested_sims / search_wall_time`) during the match — context
   for the self-play planning in docs/12, no bar attached. This rate
   is not leaf-evaluation throughput and not a lower bound: a root TSS
   proof may credit the full configured budget while running zero
   simulations, and root TSS work is included in the wall time.
4. **The VCF solve rate.** A seeded sample of 100 puzzles across
   `data/puzzles/kaibao-vcf-676.json`, the four
   `renju-portal-vcf-783-*of4.json` shards, and
   `data/puzzles/vcf-material-763.json`. The baseline plays the
   attacker at 4,000 sims; the player's own TSS probe runs
   automatically at the start of every `decide` on the puzzle root.
   The defender follows the pack's
   claimed line for the current ply (a seeded random move when the
   line runs out or the color does not match). A puzzle counts as
   solved when the attacker wins within `line length + 4` plies. Bar:
   at least half.

The pack parsing is a deliberate minimal duplication inside the test
file: depending on `cli` for `PuzzlePack` would create a dependency
cycle (`cli` already depends on `baseline`). The comment at the top of
the module names the precedent.

### The measured numbers

**OLD — historical baseline-reference / CLI-probe integration
(provenance only).** These numbers were recorded from the reference
build (release profile, Apple M5 Max, ~29 minutes total) before both
proof-path redesigns (root hook, then player-level probe). They are
kept to show the rerun protocol and the
old anchor datum; rerun against the player-level proof path is future
work.

```text
=== baseline strength ladder ===
vs uniform-random driver (baseline Black, random White)
sims       wins losses  draws win_rate
400          10      0      0     100%
4000         10      0      0     100%
10000        10      0      0     100%

vs TacticsEvaluator at 200 sims
games      wins losses  draws win_rate
20           20      0      0     100%
  seconds/game: 5.879, configured_budget_rate: 599 requested_sims/search_wall_time

VCF solve rate
solved 100/100
```

## What the numbers mean

**The random-driver table is the chapter-09 lesson, confirmed.** 100%
at every level — and it still says nothing about the gradient. Its
only job is to prove the seat is not broken.

**20/20 against the mock is the anchor datum.** The hand-written
readouts, wrapped in the same PUCT search, decisively beat the
tutorial scaffolding they replace — at the *same* 200-simulation
budget, so the difference is the evaluator, not the compute. Both bars
were pre-registered at ≥50%; the measured numbers are what they are
and are recorded as measured. A 100% match also carries the chapter-09
warning in the other direction: the mock has now joined random play on
the list of opponents that no longer discriminate. The next yardstick
is the trained network — which is the point of the anchor.

**100/100 VCF puzzles is the TSS eye working as designed — in the
old CLI-probe integration.** The packs are engine-proven forced wins,
and the probe asked exactly that question once per move. The 35
seconds for 100 puzzles is the probe doing its job at the root;
positions the probe cannot certify fall back to the search. In the
current design the same question is asked by the player's own probe
inside `AiPlayer::decide`, and the wiring from puzzle root to proof to
first move is identical in intent. Do not read 100/100 as
"the baseline solves all VCF puzzles": the sample is 100 of 4,571,
the puzzles are the same distribution the prover was exercised on,
and harder corpora exist. Rerun against the player-level design is
future work.

**599 requested_sims/search_wall_time is the old datagen proxy.** It
was historically reported as "evals/sec", but it is actually the
configured-budget rate: the sum of configured simulation budgets
divided by the wall time inside the baseline's move helper. A root TSS
proof may credit the full configured budget while running zero
simulations, and root TSS work is included in the wall time, so the
number is neither a count of leaf evaluations nor a lower bound. It is
preserved as an old anchor datum only; the self-play throughput
planning in docs/12 should treat it as a coarse, pre-player-era proxy.

### The flip

The proposed change is one line in `ai_config_from`
(`gomoku/crates/cli/src/main.rs`):

```rust
evaluator: search.ai.map(Into::into).unwrap_or(EvaluatorKind::Baseline),
```

This flip is not yet verified in the current main source or in the
reference scratch; it remains a proposed landing action. The default
brain becomes the baseline once it lands. The mock stays available as
`--ai tactics` — a yardstick never leaves the toolbox. The CLI
chapter-09 acceptance tests construct `EvaluatorKind::Tactics`
explicitly, so they stay green; the `args` tests pin the flag's
behavior regardless of the default.

## TDD checklist

1. `win_in_one_is_solved_at_50_sims`
2. `forced_block_is_taken_at_50_sims`
3. `fifty_seeded_self_play_games_are_legal`
4. `strength_ladder_measured` (`--ignored --nocapture`, release)

Red-green rhythm: write the test file first, watch it fail to compile
(no `Brain`, no drivers), then build the drivers top-down: `Brain`,
`random_move`, `baseline_best_move`, `play_game`, `solve_puzzle`,
then the tests in checklist order.

## Capstone: the seat answers for itself

Run the measurement, quote your own numbers in this section — they
will differ from the reference numbers above by hardware, and the
protocol is seeded so they should not differ by chance — then sit down
to the finished player:

```bash
cd gomoku
cargo run -q -p cli --bin gomoku -- play --ai baseline --sims 4000
```

It hugs the fight, takes its wins, blocks yours, builds forks, and
when a forced line exists it simply plays it. That is the player the
net tutorial has to beat.

## Done when

* `baseline/tests/acceptance.rs` contains the three asserts and the
  ignored measurement named above.
* `cargo test -p player` is green (the sub-chapter 08 suite).
* `cargo test -p baseline` is green (asserts + flood run by default).
* The measurement reproduces the protocol and produces real numbers;
  the bars hold.
* `ai_config_from` defaults to `EvaluatorKind::Baseline`;
  `cargo test -p cli` is green, including the chapter-09 acceptance
  suite.
* `cargo clippy --all-targets -- -D warnings` and
  `cargo fmt --all -- --check` are green.
* Commit message:
  `test(baseline): the measured ladder — baseline sub-chapter 09`.
* `docs/WARM-UP.md` and the tutorial README status lines are updated
  when the owner signs off.

## References

* [`00-baseline-primer.md`](../00-baseline-primer.md) — §9 the
  strength-ladder contract.
* [`../README.md`](../README.md) — the chapter map.
* [Playable CLI chapter 09](../../playable-cli-tutorial/09-acceptance.md)
  — the proof-not-vibes ritual this sub-chapter extends, and the
  uniform-random lesson.
* `gomoku/crates/baseline/tests/acceptance.rs` — the acceptance suite.
* The player-level design is verified in the uncommitted reference
  worktree `../rust-ml-unified08-ref` (detached at base HEAD):
  `gomoku/crates/player/` (`AiPlayer::decide`, `OwnTssConfig`,
  `GuardConfig`) and the CLI integration (`SearchInfo::from_stats`
  beside the unchanged `SearchInfo::from_search`). The retired
  hook-era scratch reference `../rust-ml-tss-root-doc-ref` is legacy
  history. The main source uses the one-method `Evaluator` /
  `SearchOutcome` shape and has no `player` crate yet.
* [`../../../12-gomoku-architecture.md`](../../../12-gomoku-architecture.md) —
  the self-play planning the throughput numbers feed.
