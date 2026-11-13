# Chapter 04 — Diversity and grading

## Abstract

This chapter turns the game runner of the earlier chapters into a
corpus generator. Two seats of the hand-written baseline player at
equal budgets, playing a deterministic evaluator with greedy move
selection, would produce the same game twice; a training corpus needs
deliberate variety and declared strength. The chapter adds the five
diversity mechanics of the design spec and the binary that drives
them. `TournamentOpenings` parses and loads the committed tournament
opening lists (center-relative `x,y` coordinates) and serves them
through the `OpeningSource` seam of chapter 01, sampled uniformly per
game with a seeded Swap2 answer. The `ExplorationSchedule` — the type
chapter 02 already carries in every record header — is defined
behaviorally here: temperature 1 with root Dirichlet noise for the
first ten post-opening plies, then deterministic argmax selection,
with every other seat setting preserved. `generate_games` derives one
seed per game from a contiguous base-seed range, detects duplicate
games by the Zobrist keys of their full move walk, and retries a
collision with a derived seed, up to 128 attempts. The `selfplaygen`
binary ties the pieces together: declared per-seat simulation budgets
(the pairing), repeated `--openings` flags, a declared seed base and
game count, shard writing, and a read-back check. The conclusions:
one process invocation produces a varied, reproducible,
strength-labelled corpus — varied because openings, answers, and ten
exploration plies differ per seed; reproducible because the seed is
the game's complete identity; strength-labelled because the pairing
is declared on the command line and recorded in every header and in
the manifest. The chapter also reports three honest observations
without resolving them: the committed piskvork list is rejected by
the parser (line 41 is off-board), debug builds play far too slowly
for verification, and a small report-only probe inverted the grading
expectation. What this chapter deliberately does not do: it does not
extract anchors, export samples, mix sources, verify corpora, or run
the acceptance gates — chapters 05 through 07. The intended reader is
working the selfplay-corpora tutorial in order and has completed
chapter 03.

## Glossary

| Term | Definition |
|------|------------|
| **Seat** | One of the two players of a generated game: a persistent `player::AiPlayer` running the baseline evaluator at a declared simulation budget (chapter 01). Every game pairs two seats. |
| **Simulation budget** | The number of search simulations a seat runs per move decision; written baseline-X. Declared per seat on the command line and recorded in every game header. |
| **Pairing** | The declared Black-vs-White seat configuration of a corpus — `SeatPairing` in the record header (chapter 02), `pairing` in the manifest. Matched pairs (X vs X) produce balanced games; graded pairs (X vs Y) produce favorites and upsets. |
| **Corpus** | A set of game records produced at one declared pairing, stored as shards plus a manifest (chapter 02). |
| **Shard** | A fixed-capacity file of length-delimited bincode records; the `train` crate's storage unit, reused unchanged. |
| **Manifest** | The JSON file next to the shards: the exact seed set, the pairing, game counts, shard names. The shards are the data; the manifest is provenance. |
| **Swap2** | The project's opening protocol: one side places three stones, the other side chooses a color or places two more stones, and the first side then chooses a color. Executed through the engine's typestate, so illegal openings are unrepresentable. |
| **Opening offer** | The three-stone opening one side places in the Swap2 ceremony. In this chapter, drawn uniformly from the committed tournament lists. |
| **OpeningSource** | The runner's pluggable opening seam (chapter 01): one method, `opening(&mut self, rng: &mut StdRng) -> Opening`, sharing the game's single random number generator (RNG) stream. `GeneratedOpening` draws near-center stones; `TournamentOpenings` draws from loaded lists. |
| **Opening provenance** | `OpeningProvenance`: `Generated`, or `Listed { list, index }` — the stable identity of the source list and the zero-based offer position within it. Recorded in every game header. |
| **Exploration schedule** | `ExplorationSchedule`: the per-ply move-selection policy shared by both seats. Plies `0..plies` after the Swap2 ceremony sample moves at temperature τ with root Dirichlet noise; every later ply plays the argmax. |
| **Temperature (τ)** | The move-selection temperature: τ = 1 samples moves proportionally to visit counts; τ = 0 plays the argmax (Silver et al. 2017). |
| **Dirichlet noise (ε, α)** | Root noise mixed into the search priors during exploration plies: ε is the mix fraction, α the concentration parameter (Silver et al. 2017). |
| **Base seed / derived seed** | The seed a corpus slot starts from, and the replacement `base_seed + attempt` used when a duplicate is rejected. The accepted seed — base or derived — is the one recorded. |
| **Zobrist key** | A 64-bit hash of a board position, maintained incrementally by the engine; used here for game identity. |
| **Zobrist walk** | The ordered sequence of Zobrist keys of every position a game reaches, opening placements included. Equality of walks is this chapter's duplicate test. |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games. It never runs by default; it runs on explicit selection, in release mode. |
| **Threat-space search (TSS)** | A proof search over forced-win threat sequences (Allis 1994). Each seat's own-side probe runs before its search; a certified proof short-circuits the search, and that ply carries no search record (chapter 03). |
| **Claim labels** | The project's provenance labels (chapter 12): **[paper]** (stated in a cited paper), **[derived]** (computed from cited or measured values), **[experiment]** (a free parameter, to be settled by measurement). This chapter also uses **[invention]**: a choice of this track, not a fact of the architecture chapters. |

## Context

Four sources supply what this chapter combines. Each summary states
what the source delivered and why this chapter needs it.

- [Chapter 01 — the game runner](../01-the-game-runner/the-game-runner.md) —
  built `play_game_with_opening(seed, black_options, white_options,
  opening_source)`: two persistent seats, the Swap2 ceremony through
  the engine typestate, and one `StdRng` stream per game, shared by
  the opening source and both seats — the seed is the game's complete
  identity. It also built the `OpeningSource` seam with the
  stand-in `GeneratedOpening`. This chapter supplies the second
  implementation of that seam and the per-ply selection schedule the
  runner applies.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md) —
  built the durable side: `GameRecord` with its `GameHeader` (seed,
  `SeatPairing`, evaluator identity, opening provenance), and
  `write_corpus` / `read_corpus` / `read_manifest` on the shared
  `train::storage` mechanics, with overwrite protection. Two of its
  record fields gain content here, with a precise ownership split:
  chapter 02 already defined and tested both `SeatPairing.exploration`
  and the manifest's `seeds`; this chapter fills `seeds` with the
  accepted seeds of a generated run, derived replacements included,
  and activates the schedule's behavior. Only `exploration` was
  behavior-free before this chapter.
- [Chapter 03 — recording the search](../03-recording-the-search/recording-the-search.md) —
  fixed the per-ply record semantics. Its honesty rule matters here:
  a move played from a certified TSS proof carries no search record,
  and the exploration schedule changes only move selection, so it
  never fabricates or alters what the search recorded.
- [gomoku/crates/cli/README.md](../../../../gomoku/crates/cli/README.md) —
  the CLI's AI-vs-AI session: when no temperature or noise flags are
  given, sessions from an empty board explore for the first ten plies
  (temperature 1.0 with Dirichlet noise) and then play greedily. The
  corpus schedule mirrors that existing practice **[invention]**, with
  two stated differences: corpus games start from a non-empty Swap2
  board, and there is no explicit-selection opt-out.

The [primer](../00-selfplay-primer.md) §4.4 fixes this chapter's five
diversity mechanics — grading, openings, exploration, seed discipline,
deduplication — and §7 holds the three open observations this chapter
must report honestly. The
[design spec](../../../specs/2026-11-11-selfplay-corpora-design.md)
§3.1 and §4 are the design authority: decision S5 makes strength
grading a primary axis, and decision S6 fixes the runner's shape —
synchronous, single-threaded, one seed base per process invocation,
parallelism by process.
[Chapter 14](../../../14-openings-and-supervised-curriculum.md) §3.2
ranks the tournament opening lists as the tier-1 offer source, and
§3.3 prescribes perturbing the answering side's argmax in self-play;
this chapter's seeded protocol choice is the no-value-model
approximation of that prescription **[invention]**.

## Intention

You can produce a corpus that a training run can trust. One command
names the pairing — baseline-800 versus baseline-800 for balanced
games, baseline-3000 versus baseline-800 for a graded rung — the
opening lists, the seed range, and the output directory; minutes or
hours later, the directory holds shards and a manifest, and every
record inside carries its seed, its opening provenance, and the
declared pairing. The corpus is *varied*: games open from different
tournament offers, answer the Swap2 question differently, and explore
for ten plies before playing greedily, so no two seeds walk the same
line. It is *reproducible*: rerun the command tomorrow and the shards
come back byte-identical, because each game's seed is its complete
identity. It is *strength-labelled*: the pairing in the manifest is
the corpus's declared identity, the property decision S5 makes
load-bearing for the later promotion criterion. And it is *honest
about its own duplicates*: when two seeds collide on the same game —
possible, because low-budget games can be short and greedy — the
collision is detected by the positions themselves, rejected, and
re-rolled under a derived seed that the manifest records exactly.

## Specification as tests

The tests live in five targets under
[`gomoku/crates/selfplay/`](../../../../gomoku/crates/selfplay/):
`tests/openings.rs`, `tests/schedule.rs`, `tests/dedup.rs`,
`tests/binary.rs`, `tests/grading.rs`, plus the unit tests inside
[`src/bin/selfplaygen.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplaygen.rs).
The list below is ordered along the execution path of the generator:
the opening lists are parsed and loaded before a game starts; the
schedule shapes every ply; the seeded generation loop plays and
deduplicates games; the binary drives all of it; and the grading
probe stands last, a manual measurement over everything the chapter
built.

This chapter specifies **fourteen** tests: **twelve run by default,
two are MANUAL-ignored**. The per-target counts:

| Target | Default | Ignored | Chapter-04 tests |
|---|---:|---:|---:|
| `tests/openings.rs` | 5 | 0 | all five |
| `tests/schedule.rs` | 1 | 1 | both |
| `tests/dedup.rs` | 1 | 0 | the one |
| `tests/binary.rs` | 1 | 0 | one of seven |
| `src/bin/selfplaygen.rs` (unit) | 4 | 0 | all four |
| `tests/grading.rs` | 0 | 1 | the one |

One target needs a boundary statement. `tests/binary.rs` will hold
**seven** tests when the track is complete; this chapter adds exactly
one of them, `selfplaygen_help_documents_corpus_and_grading_flags`.
The other six belong to later chapters and are named here only to
keep the counts honest: `selfplayanchors_help_documents_corpus_extraction`
and `selfplayanchors_reads_a_corpus_and_writes_json_anchor_seeds`
belong to chapter 05; the four `selfplayverify_*` tests belong to
chapter 07. None of them is specified, quoted, or built in this
chapter.

A second count note, continued from chapter 03. Chapter 03 ended with
the suite at **15 passed, 2 ignored** (game target 5 passed / 2
MANUAL-ignored, corpus target 10 passed). This chapter's twelve
default tests and two ignored tests bring the chapter-04 state to
**27 passed, 4 ignored**. The completed reference's full suite is
larger (50 passed, 4 ignored) only because chapters 05–07 add their
own tests; the four ignored tests of the completed reference are
exactly the four this track has after this chapter — chapter 01's
pair, this chapter's schedule game, and this chapter's grading probe.

### Opening lists: parse, load, and reject

- `three_stone_and_longer_lines_parse_as_swap2_offers` — both a
  three-stone line (`0,0, 1,0, -1,0`) and a longer seven-stone line
  parse into offers whose `initial` is the first three stones
  mapped center-relative (`0,0` → row 7, column 7; `1,0` → row 7,
  column 8; `-1,0` → row 7, column 6), with
  `OpeningProvenance::Listed { list, index: 0 }`, and the resulting
  opening executes through the Swap2 typestate. Longer lines are
  validated in full but only their first three stones drive the
  ceremony — the later stones belong to the source's pre-set
  position and are not replayed.
- `malformed_lines_are_rejected_with_physical_line_numbers` — five
  malformed inputs, each rejected with the list identity, the
  one-based physical line number, and the specific reason: a
  duplicate coordinate on line 2; fewer than three pairs; an
  off-board coordinate (`8,0` maps to column 15, outside the 15×15
  board); a non-numeric x coordinate; an odd field count.
- `sampled_offers_come_from_the_loaded_list_with_seeded_determinism` —
  two identically parsed three-offer lists, sampled 64 times from two
  identically seeded RNGs, produce identical openings; over the 64
  draws all three offers appear, all three Swap2 answer kinds appear
  (take Black, take White, add two), both final colors appear, and
  every sampled opening executes legally.
- `valid_committed_tournament_lists_load_into_one_uniform_offer_pool` —
  the three committed Gomocup lists
  ([2024](../../../../data/openings/gomocup-2024-freestyle15.txt),
  [2025](../../../../data/openings/gomocup-2025-freestyle15.txt),
  [2026](../../../../data/openings/gomocup-2026-freestyle15.txt))
  load into one pool of exactly 36 offers (each committed list holds
  12 entries); a sampled offer's provenance names one of the three
  list paths and an index the test asserts only as `< 41` — not the
  per-list bound — and the offer executes.
- `malformed_committed_piskvork_line_is_rejected_with_its_line_number` —
  loading the committed
  [piskvork-openings.txt](../../../../data/openings/piskvork-openings.txt)
  fails with a message containing `line 41` and `outside`: its line 41
  holds the off-board coordinate `-8,-6`. This test pins a real data
  defect as a regression guard — see the open observations below.

### The exploration schedule

- `first_ten_plies_explore_then_selection_is_deterministic_without_changing_budgets` —
  from a deliberately loud base configuration (777 simulations,
  c_puct 1.75, temperature 4.0, noise 0.8/0.9, baseline TSS probe and
  guard, resignation enabled), `options_for_ply` at plies 0, 1, and
  `EXPLORATION_PLIES - 1` yields temperature 1.0, ε 0.25, α 0.3 — and
  every other setting preserved exactly, including the 777
  simulations. At plies `EXPLORATION_PLIES`, `EXPLORATION_PLIES + 1`,
  and 100 the test asserts temperature 0.0 and ε 0.0 and that the
  same other fields — simulations, c_puct, TSS probe, guard,
  resignation — are again preserved. One deliberate gap: the code
  also leaves `noise_alpha` untouched outside the window (ε 0.0 makes
  α inert), but the test does not assert that — α outside the window
  is stated source behavior, not a pinned contract. The test also
  asserts `EXPLORATION_PLIES == 10`, so a silent change of the window
  fails loudly.
- `scheduled_game_is_reproducible_and_records_its_schedule_and_pairing`
  (MANUAL) — two games at seed 29, Black at 2 simulations and White
  at 1, produce equal `GameRecord`s, and the header records the
  default schedule and the per-seat budgets. The MANUAL comment says
  why it is ignored: two full games are unnecessary for the isolated
  schedule check and can run for minutes in debug; the binary's
  budget-1 fixture already checks recording.

### Seeded generation and deduplication

- `forced_collision_is_rejected_and_rerolled_with_seed_plus_attempt` —
  a fabricated-game seam (no real games are played): games built from
  a seed place their opening stones on row `seed % 15`, so the base
  seeds `[7, 7]` collide. `generate_games_with` accepts the first
  game at seed 7, rejects the second game's attempt zero, and retries
  with the derived seed 8. The result holds two games with seeds
  exactly `[7, 8]` and different openings.

This is the chapter's only dedup test, and it is deliberately
fabricated: the contract under test is the seed arithmetic and the
rejection, not the probability that two real games collide.

### The `selfplaygen` binary

Four unit tests live inside the binary itself, plus one integration
test in `tests/binary.rs`:

- `generation_defaults_to_the_baseline_own_tss_probe` — parsing the
  minimal invocation (`--out`, one `--openings`) yields the baseline
  own-TSS probe configuration: both seats always run the probe at
  10 000 nodes / 30 plies.
- `parses_all_flags_with_last_numeric_value_winning` — every flag
  parses; a repeated numeric flag takes its last value (`--games 2
  --games 1` yields 1); repeated `--openings` flags accumulate in
  order.
- `rejects_missing_required_flags_and_zero_counts` — a missing
  `--out` fails with a message naming `--out`; `--games 0` fails
  with "greater than zero".
- `one_budget_one_game_writes_a_corpus_that_the_reader_round_trips` —
  the binary's run path with an injected fixed opening source, budget
  1 versus 1, seed base 11, shard capacity 1, and the TSS probe
  disabled for speed: the manifest reports one game, accepted seeds
  exactly `[11]`, shards exactly `["shard-000.bin"]`; the read-back
  record carries header seed 11, per-seat budgets 1 and 1, the fixed
  opening verbatim, and at least one ply.
- `selfplaygen_help_documents_corpus_and_grading_flags`
  (`tests/binary.rs`, this chapter's one) — `selfplaygen --help`
  succeeds and its output documents all seven flags:
  `--black-sims`, `--white-sims`, `--games`, `--seed-base`,
  `--shard-capacity`, `--out`, `--openings`.

### The grading probe (MANUAL)

- `higher_budget_seat_wins_more_often_on_measured_seeds`
  (`tests/grading.rs`, MANUAL) — four games on the pinned seeds
  `[11, 1, 7, 18]`, Black at 16 simulations versus White at 1; the
  assertion is only that the stronger seat wins more often than the
  weaker seat. The MANUAL comment states the design: four full games
  run for minutes in debug, and the seeds are pinned measurements, so
  a cheap sanity check does not become a flaky Elo estimate. (Elo:
  the relative-strength rating named after Arpad Elo.)

This test is a sanity probe, not the grading gate. The design spec's
gate 3 — a pre-registered margin over a fixed number of games — is
chapter 07's acceptance work, and the report-only inversion observed
in the reference measurements is discussed in the open observations
below, unresolved.

## Walkthrough

The order is the execution order of one `selfplaygen` invocation,
from argument parsing to the manifest on disk. Each step names the
artifact at the moment execution first touches it. Two source files
are new —
[`src/generate.rs`](../../../../gomoku/crates/selfplay/src/generate.rs)
and
[`src/bin/selfplaygen.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplaygen.rs)
— and four existing files grow:
[`src/opening.rs`](../../../../gomoku/crates/selfplay/src/opening.rs)
gains the tournament source,
[`src/schedule.rs`](../../../../gomoku/crates/selfplay/src/schedule.rs)
gains `options_for_ply` on the `ExplorationSchedule` type chapter 02
already created there (fields, serde derives, `Default`, and the
re-export all exist), the runner in
[`src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs)
changes in exactly one line region — the move loop applies the
schedule to the seat's base options before each decision — and
[`src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs)
re-exports the new vocabulary.

### 1. Parsing the command line

`selfplaygen` starts in `parse_args`, a hand-rolled loop over the
argument iterator — no argument-parsing dependency. The vocabulary is
seven flags. `--out` and at least one `--openings` are required;
`--openings` repeats and accumulates in order; the numeric flags
`--black-sims`, `--white-sims`, `--games`, `--seed-base`, and
`--shard-capacity` default to 800, 800, 1, 0, and 1000, and a
repeated numeric flag takes its last value. Values are separate
arguments only — `--flag value`, never `--flag=value` — and an
unknown flag is an error, as are `--games 0` and `--shard-capacity
0`. A parse failure prints the message and the usage text and exits
with code 2; `--help` (or `-h`) prints the usage and exits
successfully. The usage text also states the two operational rules:
both seats use the baseline own-TSS probe, and parallel generation is
separate processes over disjoint seed ranges.

Why hand-rolled, in a project that could afford a dependency? The
flag set is seven strings, the binary is a teaching artifact, and the
parse rules — last value wins, repeat to accumulate — are exactly
what the unit tests pin. A parser crate would move the contract out
of sight.

### 2. Loading the opening lists

The run path begins with `TournamentOpenings::load(&args.openings)`.
An empty path set is an error before any file is touched ("at least
one tournament opening list is required"). Each path is read; an
unreadable file fails with the path and the underlying I/O error.
Each file's text goes to `TournamentOpenings::parse(list, text)`,
where `list` is the path string — the stable identity that later
appears in every record's opening provenance.

The parser works line by line, tracking the one-based physical line
number. A line must hold at least three comma-separated `x,y` pairs
and an even field count. Each coordinate parses as a signed integer
and maps center-relative to the board: `x` is the column offset, `y`
the row offset, so `0,0` is row 7, column 7 and `-8,-6` would be
column −1 — off the board. Four rejections are possible per line, all
with the list identity, line number, and reason: invalid x or y,
off-board coordinate, duplicate coordinate, or too few pairs. An
empty file is rejected at line 1. Longer lines are validated in full —
every pair must be numeric, on-board, and distinct — but only the
first three stones enter the offer's `initial`, because the Swap2
ceremony consumes exactly three; the rest of a long line is the
source tournament's pre-set continuation, not this generator's to
replay. The offers of all lists concatenate into one pool, each
keeping its own list identity and its own zero-based index.

Why fail fast on the first malformed line rather than skip it? An
opening list is a curated artifact; a corrupt line means the artifact
is not what the invocation claimed. The piskvork observation below
shows the discipline working: the bad coordinate surfaced as a
line-numbered rejection, not as silently skewed openings.

### 3. The opening ceremony through the seam

The loaded pool enters the run as `&mut impl OpeningSource` — chapter
01's seam, unchanged. When the runner's game loop asks for an
opening, `TournamentOpenings::opening` draws one offer uniformly from
the concatenated pool using the game's RNG, then draws the Swap2
answer: one third take Black, one third take White, one third add two
stones — two unique cells from the central 7×7 region (rows and
columns 4..11, the same region `GeneratedOpening` draws from),
excluding the offer's stones — with a random final color. The
returned `Opening` carries `OpeningProvenance::Listed { list, index }`.

The uniform draw and the three-way answer are the **[invention]**
this chapter makes under chapter 14's authority. Section 3.2 of
chapter 14 ranks the tournament lists as the tier-1 offer source —
they encode decades of engine-tournament balance selection and need
no value function — and §3.3 prescribes perturbing the answering
side's argmax in self-play so games explore both colors of the same
offer. This track has no value model, so the perturbed argmax becomes
a seeded uniform protocol choice, including both color assignments.
The source comment in `src/opening.rs` says exactly this, so the
approximation is documented where a future value-guided answer will
replace it.

One RNG discipline carries the whole ceremony: the runner creates a
single `StdRng` from the game's seed and lends it to the opening
source and then to both seats. The opening draw, the answer draw, and
every stochastic move selection all consume the same stream — which
is what makes the seed the complete identity of the game.

### 4. Deriving one seed per game

`generate_games(base_seeds, black_options, white_options,
opening_source)` receives the base seeds as an iterator — the binary
supplies the contiguous range `seed_base .. seed_base + games`,
overflow-checked before play begins. The function is a thin wrapper:
it closes over the real runner call and forwards to
`generate_games_with(base_seeds, play)`, the injectable seam that
takes any `FnMut(u64) -> Result<Game, GameError>`. The split mirrors
chapter 01's `play_game` / `play_game_with_opening` and the binary's
own `run` / `run_with_opening_source`: every stage of the pipeline
has a seam where a test can substitute fabricated behavior, and the
dedup test uses exactly this seam to force a collision without
playing a game.

Inside `generate_games_with`, each base seed gets up to
`MAX_ATTEMPTS_PER_GAME = 128` candidates. Attempt zero uses the base
seed itself; attempt `n` uses `base_seed.wrapping_add(n)`. The
accepted candidate's actual seed — base or derived — stays in the
`Game`, flows into the record header, and lands in the manifest's
`seeds` list. Two consequences are worth stating plainly. A base
seed that collides is not lost; it is replaced, and the manifest
shows the replacement, so the recorded seed set is always the exact
set that produced the corpus. And base ranges of separate invocations
may stay contiguous without coordination: a derived replacement seed
entering the recorded set is expected behavior, visible in the
manifest, not an anomaly.

### 5. The schedule inside the move loop

Every game runs through chapter 01's loop with one addition: before
each decision, the loop applies
`ExplorationSchedule::default().options_for_ply(base_options,
plies.len())` to the moving seat's base options. The ply index counts
post-opening moves — the Swap2 placements are the ceremony, not plies.

`ExplorationSchedule` is the type chapter 02 already serializes into
every record header; this chapter defines its behavior. The default
window is `EXPLORATION_PLIES = 10`: plies 0 through 9 after the
ceremony run at temperature 1.0 with root Dirichlet noise ε = 0.25,
α = 0.3 **[paper]** for the mechanism (Silver et al. 2017),
**[experiment]** for the window and the values, which mirror the
CLI's AI-vs-AI practice **[invention]**. From ply 10 on, temperature
is 0.0 — argmax selection — and ε is 0.0, which makes α inert, so the
schedule leaves α untouched outside the window. The schedule's
contract is narrow on purpose: only move-selection settings change.
The seat's simulation budget, c_puct, TSS probe, guard, and
resignation settings pass through exactly — the loud-base test pins
this, because a schedule that quietly edited the budget would corrupt
the pairing, the corpus's declared identity.

Why explore only early? The corpus needs diverse positions, and
diversity is cheapest at the start, when the game tree is widest;
late-game exploration would mostly add blunders to positions that
strong play would have ended. The visit-distribution records of
chapter 03 make the cost visible: an exploration ply's π spreads mass
over moves the search ranked low, which is useful breadth early and
noise late.

### 6. The seat's decision: proof, search, selection, guard

The effective options go to `AiPlayer::decide`, unchanged from the
baseline tutorial. The decision order matters for what the record
contains. First the own-side TSS probe runs — the binary always
configures the baseline probe, 10 000 nodes and 30 plies — and a
certified forced win short-circuits everything: the proof's first
move is played, no search runs, and the ply carries no search record
(chapter 03's honesty rule). Otherwise the PUCT (predictor UCT;
Rosin 2011) search runs at the seat's declared simulation budget. An enabled resignation
check would come next, before any random draws; in corpus runs it
stays disabled — the binary builds its seat options from
`PlayerOptions::default()`, which disables it — so corpus games end
on the board or on a full board, never on a concession heuristic.
Only then does the scheduled exploration apply, at the root of the
finished search: during the window the Dirichlet mix is added to the
root priors and the move is sampled from the visit counts at
temperature 1; after it, the move is the argmax. Last — never before
the search — the defensive guard checks the selected proposal and
ranked alternatives against certified opponent refutations. In corpus
runs the guard, like resignation, is disabled: the binary builds its
seat options from `PlayerOptions::default()`, whose guard field is
`GuardConfig::disabled()`, so the check never runs and no move
substitution occurs in these games. The order is still stated because
the pipeline position exists in the baseline player: an enabled guard
may substitute the returned move, but the ply's search record always
describes the real search that ran, never the guard's hypothetical
probes.

### 7. Duplicate detection: the Zobrist walk

Back in `generate_games_with`, the accepted-looking candidate faces
one more check before it joins the run. `zobrist_walk` replays the
game's full ordered move list through a fresh `engine::Board` — the
Swap2 placements first (three stones, or five when the answer added
two), then every recorded ply — and collects the board's Zobrist key
after every move. The walk, not the final position, is the identity:
two games that reach the same end position through different move
orders are different games, and two games that share only an opening
prefix diverge in their walks. A `HashSet<Vec<u64>>` holds every walk
seen in this run; `insert` returning `false` is the collision signal,
and the candidate is discarded for the next derived seed.

Why deduplicate at all, when seeds differ? Because determinism cuts
both ways: after the ten exploration plies, selection is greedy, and
two seeds whose opening draws and early samples coincide can converge
to the same line — more plausibly at low budgets, where searches are
shallow. A corpus that silently contained the same game twice would
overweight one line in training; the walk check makes the duplicate
impossible to miss and the retry makes it cheap to fix. If all 128
attempts for one base seed collide, generation fails with
`GenerateError::DuplicateExhausted { base_seed, attempts }` rather
than looping forever — a guard against a pathological runner whose
seeds do not diversify its games. Runner errors propagate as
`GenerateError::Game`. Deduplication is scoped to the run: the set is
built fresh per invocation, so two corpora from separate processes
are not deduplicated against each other.

### 8. From games to records

The accepted `Game`s convert with chapter 02's
`GameRecord::from_game`: the header takes the game's actual seed, the
`SeatPairing` from both seats' base options plus the default
exploration schedule, the evaluator identity
`baseline@<workspace version>`, and the opening with its provenance;
each ply copies its move and its chapter-03 search semantics. Nothing
in this step is new — which is the point of the earlier chapters'
discipline: generation produces `Game`s, and everything durable was
already specified.

### 9. Writing the corpus, then proving it readable

`write_corpus(&records, &args.out_dir, args.shard_capacity)` streams
the records into fixed-capacity shards and writes the JSON manifest —
chapter 02's mechanics, with its default overwrite protection: a
non-empty output directory is refused before any file changes. The
manifest records the accepted seeds in corpus order, the declared
pairing, the game count, and the shard names.

Then the binary does something a writer rarely does: it immediately
reads its own output back. `read_corpus` must return records equal to
what was generated, and the manifest's pairing must preserve the
declared per-seat budgets; either mismatch fails the run. The
read-back check is cheap at corpus scale and turns a whole class of
serialization defects into a loud exit code 1 at generation time
rather than a surprise at training time. The progress lines on
stderr close the loop: the declared base-seed range and pairing
before play, and after the write the game count, shard count, and
the exact accepted seed list — the same list the manifest persists.

## Open observations — reported, not resolved

Three observations from building and measuring the reference belong
to this chapter's mechanics. None is resolved here; none is hidden.

### The piskvork list is rejected — and the working pool is 36 offers

The committed
[piskvork-openings.txt](../../../../data/openings/piskvork-openings.txt)
holds 41 lines, and its line 41 contains the coordinate `-8,-6` —
column −1 after the center-relative mapping, off the 15×15 board. The
parser rejects the file with `line 41` and `outside` in the message,
and the regression test
`malformed_committed_piskvork_line_is_rejected_with_its_line_number`
pins the behavior. The rejection is correct — but its operational
consequence was measured in the reference's acceptance run: a
generation command that included the piskvork path among its
`--openings` flags failed fast in 0.16 s and produced no games,
because `load` rejects the whole call on the first malformed line.
The working pool is therefore the three committed Gomocup freestyle
lists, 36 offers in total, and the measured first corpus (48 games,
chapter 07's evidence) saw 28 of those 36 offers with a maximum
single-offer share of 8.33 %. Whether and how the piskvork list joins
the pool — corrected, filtered, or replaced — is deliberately left
open; the [data/openings README](../../../../data/openings/README.md)
documents the file's source.

### Debug builds are too slow for game-playing verification

An 8-simulation game takes roughly 25 s in a debug build. The
consequence is procedural: every verification that plays games runs
with `--release`, and the slow game-playing tests are `#[ignore]`d
with a MANUAL comment and run by explicit selection. This chapter
follows the discipline exactly: the schedule's isolated property test
runs by default and plays no game; the two tests that play games —
`scheduled_game_is_reproducible_and_records_its_schedule_and_pairing`
and the grading probe — are MANUAL. One borderline case is stated
honestly: the binary's
`one_budget_one_game_writes_a_corpus_that_the_reader_round_trips`
runs by default and does play a game, but at budget 1 versus 1 with
the TSS probe disabled — seconds, not minutes. Its fixture, not a
full-budget game, is what makes a default run affordable.

### The grading probe inverted the expectation

The reference's acceptance measurements include a report-only run of
20 games at baseline-32 (Black) versus baseline-1 (White), generated
with this chapter's binary at seeds 200000–200019. The budget-1 seat
won 15 of 20 — the opposite of the grading expectation that more
simulations win more often. The caveats are real and stated: the
sample is 20 games, far too small to separate budget effects from
color, openings, or tactical quirks; the exploration schedule
handicaps the stronger seat, because ten plies of temperature-1
sampling with root noise perturb exactly the seat whose search would
otherwise exploit the weak opponent; and the run used fixed colors —
the stronger seat always played Black, because the binary declares
budgets per color and no color-swapping exists yet. None of these
caveats resolves the observation, and this chapter offers no
resolution and no strength claim: the probe is evidence that the
ladder's load-bearing assumption needs the real gate, which chapter
07 pre-registers and runs. The MANUAL grading test in this chapter is
deliberately smaller than that gate — four pinned seeds, a direction
assertion, no margin — a smoke signal, not a measurement.

## What this chapter defers

- Detecting and recording certified suffixes, the `verify_line`
  re-verification gate, and anchor-seed extraction with its
  `selfplayanchors` binary — chapter 05. This chapter's games already
  produce proofs through the seat probe; nothing records them yet.
- Replay-to-sample derivation, the holdout split, D4 augmentation at
  sample time, weighted source mixing, and the `selfplayexport`
  binary — chapter 06.
- Corpus verification (`selfplayverify`), the determinism, legality,
  grading, corpus, and integration gates, and the first corpora's
  quality report — chapter 07. The grading observation above is that
  chapter's pre-registered work, not this chapter's.
- Candidate-versus-baseline promotion matches and any network in the
  generator — the AlphaZero loop, not this tutorial (design spec
  S7 and §8 non-goals).

## TDD checklist

Work one artifact at a time, red to green, in
[`gomoku/crates/selfplay/`](../../../../gomoku/crates/selfplay/). No
manifest changes: `selfplay` already depends on `baseline`, `engine`,
`player`, `rand`, `thiserror`, `serde`, `serde_json`, and `train`,
and a binary
in `src/bin/` needs no manifest entry.

1. The opening vocabulary extensions in
   [`src/opening.rs`](../../../../gomoku/crates/selfplay/src/opening.rs):
   `OpeningParseError` and `OpeningLoadError` with their exact
   messages, then `TournamentOpenings` with `parse`, `load`, `len`,
   `is_empty`, and the `OpeningSource` implementation. Red: the five
   `tests/openings.rs` tests, in the specification's order — the
   committed-list tests included, so the real data files guard the
   parser from the first green run. Green against all five.
2. `ExplorationSchedule::options_for_ply` in
   [`src/schedule.rs`](../../../../gomoku/crates/selfplay/src/schedule.rs).
   Do not recreate what chapter 02 built: `EXPLORATION_PLIES`, the
   `ExplorationSchedule` type with its fields, serde derives, and
   `Default`, and the
   [`src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs)
   re-export all exist already; this chapter adds only the behavior
   method. Red:
   `first_ten_plies_explore_then_selection_is_deterministic_without_changing_budgets`.
   Then the one-line-region runner change: the move loop applies the
   default schedule per ply. Red: the MANUAL
   `scheduled_game_is_reproducible_and_records_its_schedule_and_pairing`,
   watched green once, in release.
3. `GenerateError` and the `generate_games` / `generate_games_with`
   pair with the Zobrist-walk dedup in
   [`src/generate.rs`](../../../../gomoku/crates/selfplay/src/generate.rs).
   Red: `forced_collision_is_rejected_and_rerolled_with_seed_plus_attempt`
   through the injectable seam. Green without playing a real game.
4. The binary in
   [`src/bin/selfplaygen.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplaygen.rs):
   the usage text, `parse_args`, the run path split into `run` and
   `run_with_opening_source`, and the read-back check. Red: the four
   unit tests in the binary's own test module and
   `selfplaygen_help_documents_corpus_and_grading_flags` in
   `tests/binary.rs`. Green in both places.
5. The MANUAL grading probe in
   [`tests/grading.rs`](../../../../gomoku/crates/selfplay/tests/grading.rs),
   with its pinned seeds and its MANUAL comment. Run once, in
   release, on explicit selection.
6. Regression: the game and corpus targets stay green unchanged, and
   chapter 01's MANUAL pair still passes in release — the schedule
   touches their move loop.

## Capstone

Generate a tiny corpus twice, and prove the seed discipline
byte-exactly. From `gomoku/`:

```text
cargo build --release -p selfplay

dir_a=$(mktemp -d)
dir_b=$(mktemp -d)

target/release/selfplaygen \
  --out "$dir_a/corpus" \
  --openings ../data/openings/gomocup-2024-freestyle15.txt \
  --openings ../data/openings/gomocup-2025-freestyle15.txt \
  --openings ../data/openings/gomocup-2026-freestyle15.txt \
  --black-sims 8 --white-sims 8 --games 2 --seed-base 424242

target/release/selfplaygen \
  --out "$dir_b/corpus" \
  --openings ../data/openings/gomocup-2024-freestyle15.txt \
  --openings ../data/openings/gomocup-2025-freestyle15.txt \
  --openings ../data/openings/gomocup-2026-freestyle15.txt \
  --black-sims 8 --white-sims 8 --games 2 --seed-base 424242

cmp "$dir_a/corpus/shard-000.bin" "$dir_b/corpus/shard-000.bin"
cmp "$dir_a/corpus/manifest.json" "$dir_b/corpus/manifest.json"
cat "$dir_a/corpus/manifest.json"
```

Three properties land in one exercise. The output directories are
temporary — `mktemp -d` keeps the run out of the repository's `data/`
tree — and the default shard capacity 1000 writes a single
`shard-000.bin`. The stderr lines report the declared range
(`base seeds 424242..=424243, baseline-8 vs baseline-8`) and the
accepted seeds. And both `cmp` calls exit silently: two invocations,
two processes, one seed set, byte-identical shards and manifest —
determinism gate 1 rehearsed at two games instead of the measured
three. The reference measured 2.79 s for three games at this budget
in release; this pair costs less. In debug, the same two games would
cost roughly a minute — the last practical demonstration of why every
game-playing command in this track carries `--release`.

## Done when

- `cargo test -p selfplay` reports **27 passed, 4 ignored**: the game
  target keeps its chapter-03 state (5 passed, 2 MANUAL-ignored), the
  corpus target keeps its chapter-02 state (10 passed, 0 ignored), and
  this chapter's targets report openings 5/0, schedule 1/1, dedup
  1/0, binary 1/0, the `selfplaygen` binary's unit tests 4/0, and
  grading 0/1. (The completed reference's full suite is 50 passed, 4
  ignored; the difference is chapters 05–07, and the four ignored
  tests are exactly this chapter's end state.)
- Both new MANUAL tests pass under the release commands below:
  `scheduled_game_is_reproducible_and_records_its_schedule_and_pairing`
  and `higher_budget_seat_wins_more_often_on_measured_seeds`.
- `cargo test -p train` and `cargo test -p player` are not required:
  this chapter touches no crate but `selfplay`.
- `cargo clippy --all-targets -- -D warnings` is green.
- `cargo fmt --all -- --check` makes no changes.
- The commit message is
  `feat(selfplay): diversity and grading — selfplay-corpora chapter 04`.

## Verification commands

From `gomoku/`:

```text
cargo test -p selfplay
cargo test -p selfplay --test openings --test schedule --test dedup
cargo test -p selfplay --bin selfplaygen
cargo test -p selfplay --test binary selfplaygen_help
cargo test -p selfplay --release --test schedule -- --ignored
cargo test -p selfplay --release --test grading -- --ignored
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the fast gate: every default test of the track
runs in seconds, and this chapter adds nothing slow to it — the
opening tests parse text and load the committed lists, the schedule
property test plays no game, the dedup test fabricates its games, and
only the binary's budget-1 fixture plays, in seconds. The next three
commands are this chapter's focused default checks: the parser,
schedule, and dedup targets; the binary's own unit tests; and the
help-text contract selected by name, which leaves the six later-chapter
binary tests unrun — they do not exist yet. The two `--release`
commands are the MANUAL selections; the `--release` qualifier is not
optional for them, because both play full games. The grading probe's
own MANUAL comment shows the selection without `--release`; this
track's convention (README and primer §7.3) adds it, and the probe's
four budget-16 games need it.

## References

**This track:**

- [Tutorial README](../README.md) — conventions, gates, the chapter
  map, and the reference provenance.
- [Self-play primer](../00-selfplay-primer.md) — §4.4 fixes the five
  diversity mechanics; §6 holds the measured acceptance evidence this
  chapter quotes; §7 states the three open observations.
- [Chapter 01 — the game runner](../01-the-game-runner/the-game-runner.md) —
  the `OpeningSource` seam, `GeneratedOpening`, the single-RNG
  discipline, and the move loop this chapter schedules.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md) —
  `SeatPairing.exploration`, the manifest's `seeds`, `write_corpus`
  and its overwrite protection, and the read-back mechanics.
- [Chapter 03 — recording the search](../03-recording-the-search/recording-the-search.md) —
  the honesty rule for certified-proof moves that the schedule never
  disturbs.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  §3.1 the runner shape, §4 the diversity mechanics, §5 the
  acceptance gates; decisions S5 (grading as primary axis) and S6
  (synchronous, one seed per process).
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  this chapter's public surface, the measured headline numbers, and
  the open questions.
- [docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) —
  the claim-label convention (**[paper]** / **[derived]** /
  **[experiment]**).
- [docs/14-openings-and-supervised-curriculum.md](../../../14-openings-and-supervised-curriculum.md) —
  §3.2 the tier-1 tournament lists, §3.3 the perturbed answering
  argmax this chapter approximates.

**Prerequisite tracks:**

- [Baseline tutorial](../../baseline-tutorial/README.md) — `AiPlayer::decide`,
  the own-TSS probe (10 000 nodes / 30 plies), and the guard.
- [MCTS tutorial](../../mcts-tutorial/README.md) — the PUCT search
  whose root carries the Dirichlet noise.
- [Engine tutorial](../../13-engine-tutorial/README.md) — the Swap2
  typestate and the Zobrist keys this chapter walks.
- [Datagen tutorial](../../datagen-tutorial/README.md) — the `train`
  crate's shard and manifest storage discipline.
- [gomoku/crates/cli/README.md](../../../../gomoku/crates/cli/README.md) —
  the AI-vs-AI exploration practice the schedule mirrors.
- [data/openings/README.md](../../../../data/openings/README.md) —
  the committed opening lists and their sources.

**Source:**

- [`gomoku/crates/selfplay/src/opening.rs`](../../../../gomoku/crates/selfplay/src/opening.rs) —
  gains `TournamentOpenings`, `OpeningParseError`,
  `OpeningLoadError`. Touched in this chapter.
- [`gomoku/crates/selfplay/src/schedule.rs`](../../../../gomoku/crates/selfplay/src/schedule.rs) —
  gains `ExplorationSchedule::options_for_ply`. The type,
  `EXPLORATION_PLIES`, and the `Default` were created in chapter 02.
  Touched in this chapter.
- [`gomoku/crates/selfplay/src/generate.rs`](../../../../gomoku/crates/selfplay/src/generate.rs) —
  new: `GenerateError`, `generate_games`, `generate_games_with`.
  Created in this chapter.
- [`gomoku/crates/selfplay/src/bin/selfplaygen.rs`](../../../../gomoku/crates/selfplay/src/bin/selfplaygen.rs) —
  new: the corpus generator binary and its unit tests. Created in
  this chapter.
- [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs) —
  the move loop applies the schedule per ply. Touched in this
  chapter.
- [`gomoku/crates/selfplay/src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs) —
  `SeatPairing.exploration` becomes behavioral. Not touched in this
  chapter.
- [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs) —
  re-exports the new vocabulary. Touched in this chapter.
- [`gomoku/crates/selfplay/tests/openings.rs`](../../../../gomoku/crates/selfplay/tests/openings.rs),
  [`tests/schedule.rs`](../../../../gomoku/crates/selfplay/tests/schedule.rs),
  [`tests/dedup.rs`](../../../../gomoku/crates/selfplay/tests/dedup.rs),
  [`tests/binary.rs`](../../../../gomoku/crates/selfplay/tests/binary.rs),
  [`tests/grading.rs`](../../../../gomoku/crates/selfplay/tests/grading.rs) —
  this chapter's specification. Created in this chapter.
- [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs) —
  `PlayerOptions`, `OwnTssConfig::baseline`. Not touched in this
  chapter.

**Companion papers:**

- [diversity-and-grading-design.md](diversity-and-grading-design.md) —
  the conceptual story of the diversity mechanics.
- [diversity-and-grading-implementation.md](diversity-and-grading-implementation.md) —
  the hiding layer: low-level construction instructions.
- [diversity-and-grading-solution.md](diversity-and-grading-solution.md) —
  the annotated reference snippets.

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). https://doi.org/10.1038/nature24270 —
  temperature sampling over visit counts and root Dirichlet noise,
  the exploration mechanisms the schedule applies.
- Rosin, C. D. (2011). *Multi-armed bandits with episode context*
  (PUCT). https://doi.org/10.1007/s10472-011-9258-6 — the selection
  rule of the search the schedule leaves untouched.
- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://doi.org/10.26481/dis.19940923la — threat-space search, the
  seat probe that short-circuits the search on certified wins.
