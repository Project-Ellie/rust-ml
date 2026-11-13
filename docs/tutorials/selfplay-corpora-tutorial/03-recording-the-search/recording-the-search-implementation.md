# Chapter 03 — Recording the search: implementation plan

## Abstract

This document is the implementation plan for
[chapter 03 of the selfplay-corpora tutorial](recording-the-search.md):
the hiding layer between the chapter paper and the solution, for the
learner who builds the code without reading the reference. It turns the
chapter's walkthrough into a numbered construction order along the
execution path of one ply's search data: the format-width constant
`RECORDED_PRIOR_TOP_K = 16` and its crate-root re-export, the two
conversion tests that go red on the missing `From` implementation, the
`From<player::SearchStats> for PlySearch` implementation itself, the
replacement of chapter 01's inline conversion block in the runner's move
loop, the two doc-comment updates, and the regression runs — five
default game tests plus ten corpus tests, and the chapter-01 MANUAL pair
in release mode. Every artifact names its file, its behavioral contract
in prose, the helpers to reuse, and the idioms to apply by name; method
bodies never appear. The intended reader has completed chapter 02, has
read the chapter paper and, ideally, the
[design paper](recording-the-search-design.md), and wants low-level
guidance without solution leakage.

## Glossary

| Term | Definition |
|------|------------|
| **Seat** | One of the two players of a generated game: a persistent `player::AiPlayer` running the baseline evaluator at a declared simulation budget (chapter 01). This plan speaks of the *deciding* seat of a ply. |
| **Simulation budget** | The number of Monte Carlo Tree Search (MCTS) simulations a seat runs per move decision. |
| **SearchStats** | `player::SearchStats`, the statistics the seat extracts from one real searched tree: the side to move, every root move with its visit count (descending by count, ties by move index), every root prior (descending by value, ties by move index), and the root value. Absent when a certified proof bypassed search. |
| **PlySearch** | The `selfplay` crate's per-ply search slot: `side_to_move`, `visits`, `prior`, `root_value`. This chapter owns its conversion semantics. |
| **PlyRecord** | The runner's per-ply record (chapter 01): `mv`, the played move, and `search`, an `Option<PlySearch>` — `None` for a certified-proof move. |
| **RecordedPly** | The durable per-ply record (chapter 02): `mv`, sparse raw `visits`, sparse `prior`, and `root_value: Option<f32>`. The disk image of one `PlyRecord`; unchanged in this chapter. |
| **Side to move** | The color whose turn the searched position was — `board.to_move()` at decision time. The root value's sign is defined relative to this side. |
| **Root value** | The search's value estimate at the root after the simulation budget is spent, from the side to move's perspective. Recorded as metadata, not as a training target. |
| **Prior** | The evaluator's policy probabilities over root moves before search, after the optional root Dirichlet noise (Silver et al. 2017) when noise was applied. Recorded sparsely for audit. |
| **Top-k** | Keeping only the k leading entries of an ordered list. Here k is `RECORDED_PRIOR_TOP_K = 16`. |
| **π (visit-distribution target)** | The normalized root visit-count distribution of the search at a ply; the policy training target (Silver et al. 2017). The record stores raw counts; normalization happens at read time in chapter 06. |
| **Certified forced win** | A forced-win line produced by the engine's threat-space search (TSS) prover (Allis 1994) and checked by `engine::verify_line`. A seat that holds one plays its first move immediately; no search runs, and the ply carries no search statistics. |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games. It never runs by default; it runs on explicit selection, in release mode. This chapter adds none. |
| **Claim labels** | The project's provenance labels (chapter 12 of the wiki): **[paper]** (stated in a cited paper), **[derived]** (computed from cited or measured values), **[experiment]** (a free parameter, to be settled by measurement). The width k = 16 is **[experiment]**. |
| **TDD** | Test-driven development: write the test, watch it fail (red), implement, watch it pass (green). |

## How to use this plan

Read [the chapter paper](recording-the-search.md) first. It carries the
intention, the specification as tests, and the execution-order
walkthrough; the [design paper](recording-the-search-design.md) carries
the reasoning. This plan assumes both and adds only construction detail:
which file, which item, which contract, which test goes red then green.
Work one step at a time, in order — the order is the execution order of
one ply's search data, from the format width the conversion needs, to
the tests that pin the conversion, to the conversion, to the refactor
that routes the runner through it. Open
[recording-the-search-solution.md](recording-the-search-solution.md)
only if you are stuck for more than twenty minutes, or after
the chapter is complete.

## Context

Chapter 01 left the conversion inline. The runner's move loop in
[`src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs) fills
the `PlySearch` slot with a closure over `decision.search` that copies
the side to move, keeps only the positive-count visit entries, carries
the prior through untruncated, and copies the root value. A comment
block above that closure announces this chapter: chapter 03 replaces
the block with `PlySearch::from` and the recorded top-k prior. The
`PlySearch.prior` field doc still reads "all root priors" — slice
wording that chapter 01's solution paper marked for this chapter.
Chapter 02 left the durable side complete:
[`src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs)
copies the slot into `RecordedPly` as-is, and its `prior` field doc
carries the same "all root priors" slice wording. The corpus writer,
the readers, the format version, the schedule vocabulary, and the
certified-suffix slot are all chapter-02 (and later-chapter) territory;
this chapter touches none of them.

Two scope guards keep the chapter slice honest. First, the `From`
implementation is total and sees only present statistics: a decision
with `search: None` never reaches it, because the `Option::map` idiom
in the move loop is the whole mechanism for unsearched plies, and the
empty-visits, empty-prior, no-root-value mapping of an absent slot
stays exactly where chapter 02 put it, in `GameRecord::from_game`. Do
not move that handling into the conversion. Second, the conversion is
a carrier, not a computer: no normalization of the visit counts (π,
the normalized root visit distribution, is derived at read time in
chapter 06), no re-sorting of anything (the
player's extraction order is the recorded order), no renormalization
of the truncated prior. No manifest changes either: `selfplay` already
depends on `player`, and the test target already imports from it.

---

## Step 1 — `src/record.rs` and `src/lib.rs`: the format width

The conversion of step 3 truncates against a named constant, and step
2's tests import that constant from the crate root — so the constant
and its re-export land first, before any test names them.

### 1.1 `RECORDED_PRIOR_TOP_K`

In
[`src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs),
add one public `usize` constant, `RECORDED_PRIOR_TOP_K`, equal to 16,
next to `GAME_RECORD_FORMAT_VERSION`. The constant lives with the
record vocabulary because it is a *format width*: it bounds what any
reader of the durable format must expect, so it belongs beside
`RecordedPly`, not beside the in-memory slot. The doc comment carries
two sentences of honesty: this sparse audit width is a
selfplay-corpora format invention ([design
spec](../../../specs/2026-11-11-selfplay-corpora-design.md) §3.2), not
an AlphaZero or architecture-document constant; and the complete prior
remains recomputable from the position and the deterministic baseline
evaluator, so the omission is safe.

### 1.2 The crate-root re-export, now

In
[`src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs), add
`RECORDED_PRIOR_TOP_K` to the existing `record` re-export group, in
alphabetical position. This chapter adds exactly one public item, so
this is the only export checkpoint: the re-export lands here because
step 2's tests import the constant from the crate root, and no later
step adds or audits exports. The module stays private; the test file
must never reach for a path deeper than the crate root.

Tests that go red then green: none; compilation under
`#![deny(missing_docs)]` is the gate, and `cargo test -p selfplay`
still reports the chapter-02 state — 3 passed, 2 ignored for the game
target, 10 passed for the corpus target.

---

## Step 2 — `tests/game.rs`: the two conversion tests (red)

Both tests live at the top of
[`tests/game.rs`](../../../../gomoku/crates/selfplay/tests/game.rs),
ahead of chapter 01's ceremony test — the reference file's lexical
order leads with this chapter's pair. Extend the existing import
blocks: `player::SearchStats` joins the `player` import, and
`PlySearch` and `RECORDED_PRIOR_TOP_K` join the `selfplay`
crate-root import — chapter 02's test file names neither. The `mv`
helper is already in scope from chapter 01.

The red state is a compile error: `PlySearch::from` does not exist
yet. That is expected and correct — the missing `From` conversion is
exactly what these tests specify. Neither test plays a game; each
builds its `player::SearchStats` input by hand, so each is a test of
the conversion, not of the search, and both will finish in well under
a second once they compile.

### 2.1 `ply_search_preserves_the_side_to_move_root_value_sign`

Fixture: two hand-built `SearchStats` values, both with empty visit
and prior lists. One carries side to move Black and value 0.875; the
other carries side to move White and value −0.625. Both values are
exact binary fractions (7/8 and −5/8), so the equality assertions
below carry no floating-point tolerance. The empty lists are
deliberate: the test isolates the copy rule, and it incidentally pins
that empty input lists convert to empty slot lists — no fabricated
entries.

Convert both through `PlySearch::from` and assert, for each, that the
slot's side to move equals the input's side to move and the slot's
root value equals the input's value exactly: Black keeps 0.875, White
keeps −0.625.

This test pins the sign convention of the whole record format in one
place: the conversion never negates, never re-anchors a value to a
fixed color, and never clamps. A future "helpful normalization" fails
loudly here instead of silently corrupting a corpus.

### 2.2 `ply_search_truncates_prior_to_top_16_without_reordering`

Fixture: one hand-built `SearchStats` with side to move Black, value
0.0, a single visit entry (one move through the `mv` helper, count 1),
and a prior of twenty entries on twenty distinct cells — the first
twenty cells in row-major order, built with the `mv` helper through
the range-map-collect idiom — with strictly decreasing values: 1.0,
0.99, 0.98, and so on. Keep a clone of the input prior for the
assertion, because the conversion consumes the statistics.

Convert through `PlySearch::from` and assert two things. First,
`RECORDED_PRIOR_TOP_K` equals 16 — so a silent change of the format
width fails loudly. Second, the slot's prior equals the leading slice
of the input prior of length `RECORDED_PRIOR_TOP_K` — a slice-prefix
equality that pins three properties at once: the cut is a prefix (a
truncation, not a selection by some other criterion), the order is the
input's order (the strictly decreasing values make any re-sorting
visible), and the values are unchanged (no renormalization). The
twenty-entry input makes the cut visible; sixteen in, sixteen out, the
tail gone.

Watch both tests fail to compile: the conversion does not exist. That
is the red state.

---

## Step 3 — `src/game.rs`: the `From` implementation (green)

One item, placed directly after the `PlySearch` struct definition in
[`src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs):
`impl From<SearchStats> for PlySearch`. Two imports make it
expressible: `SearchStats` joins the existing `player` import at the
top of the file, and `RECORDED_PRIOR_TOP_K` is imported from the
record module (`crate::record`) — from the module, not through the
crate-root re-export, matching the completed reference.

Behavioral contract of the `from` method, field by field, in struct
declaration order. The method consumes the statistics by value and
returns the slot; the conversion is total — every input converts,
nothing is validated, nothing fails.

- `side_to_move`: copied verbatim. No negation, no re-anchoring, no
  special cases.
- `visits`: only the positive-count entries survive, in the input's
  order — the player's extraction order, descending by count with ties
  by move index. Apply the consuming-iterator idiom with a filter on
  the count and a collect. The counts stay raw: no normalization,
  because π is a read-time derivation (chapter 06), and no sorting,
  because the player already ordered the list. An empty input list
  converts to an empty slot list.
- `prior`: only the first `RECORDED_PRIOR_TOP_K` entries survive, in
  the input's order, values unchanged. Apply the consuming-iterator
  idiom with `take` on the constant and a collect. The player already
  ordered the priors descending by value, so taking the first sixteen
  *is* taking the top sixteen; the stored values are the probabilities
  as mixed and sum to at most one — no renormalization, because the
  recorded prior is an audit record, not a distribution to sample
  from.
- `root_value`: copied verbatim from the statistics' value. No
  negation, no clamping. The value is the deciding side's estimate of
  its own position, and it stays that way in the record.

What the implementation deliberately does not contain: any handling of
absent statistics. The `Option` around `Decision.search` is resolved
by the caller's `Option::map`, and the durable shape of an absent slot
— empty visits, an empty prior, `root_value: None` — belongs to
chapter 02's `GameRecord::from_game`, which this chapter does not
touch.

Watch the two step-2 tests compile and pass: the conversion now runs.
The whole default suite stays green — chapter 01's inline conversion
is still in the loop and still produces the same slots, so the three
chapter-01 default tests and the ten corpus tests see no change. Why
the conversion may land before its call site: the new implementation
is exercised only by the two new tests until step 4; that is the
correct intermediate state, not dead code.

---

## Step 4 — the refactor: one `map`, two doc comments

Now the runner routes through the named conversion, and the two slice
wordings that chapters 01 and 02 documented become the reference
wordings. This step must not change a single test outcome; if it does,
the refactor drifted — fix the refactor, never a test.

### 4.1 `src/game.rs`: replace the inline conversion

In the move loop's `Play` arm, delete chapter 01's inline conversion —
the closure over `decision.search` that builds the slot field by
field, together with the comment block above it that announced this
chapter — and replace it with the one-call form: the `Option::map`
idiom applied to `decision.search` with `PlySearch::from`. The `map`
shape was already right; only the closure becomes a named conversion.
Everything else in the loop — the status-first structure, the seat
selection by absolute color, the legality gate, the resignation arm —
stays byte-identical. So does the rest of the function around it:
chapter 02's `certified_suffix` slot initialization to `None` lives in
the returned `Game` assembly, not in the move loop, and remains
untouched there.

One behavior actually changes, and it is the chapter's one new rule:
the recorded prior is now truncated to its leading
`RECORDED_PRIOR_TOP_K` entries. No existing test sees the difference —
chapter 01's games run at budgets whose positions can exceed sixteen
legal replies, but no chapter-01 or chapter-02 assertion inspects the
prior's length — while step 2's truncation test pins the new rule
directly.

### 4.2 `src/game.rs`: the `PlySearch.prior` doc comment

The field doc reads "all root priors" — true of chapter 01's
untruncated passthrough, false now. Update it to read "top root
priors", keeping the rest of the sentence (after optional Dirichlet
noise, in descending order) unchanged. Chapter 01's solution paper
recorded this exact wording as the reference's, so the update also
retires the slice delta.

### 4.3 `src/record.rs`: the `RecordedPly.prior` doc comment

Same slice wording, one layer down: the durable field doc reads "all
root priors". Update it to name the bound — "up to
`RECORDED_PRIOR_TOP_K` root priors", keeping the rest (after optional
Dirichlet noise, in descending order; empty when search did not run)
unchanged. Write the constant's name as an intra-doc link; it resolves
because the constant lives in the same module. This is a documentation
change only: `RecordedPly` itself, `GameRecord::from_game`,
`GAME_RECORD_FORMAT_VERSION`, the writer, and the readers are
untouched — the format width bounds a field that already exists, and
chapter 02's mapping already copies the slot as-is.

Run the default suite now: `cargo test -p selfplay` reports 5 passed,
2 ignored for the game target and 10 passed for the corpus target —
the chapter's slice, with no outcome changed.

---

## Step 5 — regression and gates

The refactor touched the runner's move loop, so the chapter closes
with the game-playing evidence, then the standing gates. From
`gomoku/`:

```text
cargo test -p selfplay --test game ply_search
cargo test -p selfplay
cargo test -p selfplay --test corpus
cargo test -p selfplay --release --test game -- --ignored
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the chapter's capstone: the shared `ply_search`
prefix selects exactly the two conversion tests — no other test in the
game target carries that substring — and the run finishes in well
under a second because both fixtures are hand-built and no game is
played.

The second command is the fast gate. Cargo prints one report per test
target, never one aggregate line; the chapter's slice is the sum of
the two per-target reports — 5 passed, 2 ignored for the game target
and 10 passed, 0 ignored for the corpus target, 15 passed and 2
ignored in total.

The third command re-runs chapter 02's target alone: the record side
of the seam must be provably untouched, and
`populated_search_fields_round_trip_byte_exactly` is the proof that
the slot-to-record mapping still persists populated search fields
byte-exactly. Its fixtures build `PlySearch` by hand, so it does not
exercise the new conversion; it pins the other side of the seam.

The fourth command is the chapter-01 MANUAL pair, and both qualifiers
matter. `--release` is not optional for this focused run, and the pair
is `#[ignore]`d for two distinct reasons: the budget-8 smoke test's
MANUAL comment warns that its game runs for minutes in a debug build,
while the budget-2-versus-1 visit-sum test stays out of the default
run so the default suite's games stay at budget 1. And the run is
*relevant*, not ceremonial, because step 4 rewired the
runner: the two conversion properties this chapter deliberately does
not unit-test — the positive-count filter and the visit sum on real
games — are pinned only by
`tiny_budget_game_records_raw_visits_for_every_searched_ply`, and after
step 4 every real game's statistics flow through the named conversion
rather than the inline block. That MANUAL test now asserts, through
`PlySearch::from`, that every searched ply of a budget-2-versus-1 game
carries a non-empty visit list of positive counts summing to the
deciding seat's budget; its companion
`tiny_budget_game_completes_and_records_search_metadata` pins that
every recorded root value is finite. The `--test game` selection keeps
the run on this chapter's target — the only one with ignored tests in
this track so far. No `--release` qualifier and no `--ignored`
selection belong to the new conversion tests themselves.

`cargo test -p train` and `cargo test -p player` are not required:
this chapter touches no crate but `selfplay`. The last two commands
are the project's standing quality gates; both must be green with no
changes.

---

## Done when

- `cargo test -p selfplay` reports **15 passed, 2 ignored**: the game
  target reports 5 passed, 2 ignored (chapter 01's three defaults plus
  this chapter's two), and the corpus target reports 10 passed,
  unchanged from chapter 02. (These are also the completed reference's
  exact counts for both targets; the reference's full suite is larger
  only because later chapters add their own targets.)
- Both MANUAL tests pass under the release command in step 5.
- `cargo clippy --all-targets -- -D warnings` is green.
- `cargo fmt --all -- --check` makes no changes.
- The commit message is
  `feat(selfplay): recording the search — selfplay-corpora chapter 03`.

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `selfplay/src/record.rs` | `RECORDED_PRIOR_TOP_K: usize = 16` | format width beside `GAME_RECORD_FORMAT_VERSION`; doc comment states the §3.2 invention and the recomputable-tail rationale |
| `selfplay/src/lib.rs` | crate-root re-export of the constant | lands in step 1, before the tests import it; the chapter's only export |
| `selfplay/tests/game.rs` | imports gain `player::SearchStats`, `PlySearch`, and `RECORDED_PRIOR_TOP_K` | crate-root paths only; `mv` already present |
| `selfplay/tests/game.rs` | `ply_search_preserves_the_side_to_move_root_value_sign` | Black 0.875 and White −0.625, empty lists, exact copy assertions |
| `selfplay/tests/game.rs` | `ply_search_truncates_prior_to_top_16_without_reordering` | twenty-entry decreasing prior; the constant equals 16; the slot's prior equals the input's leading slice |
| `selfplay/src/game.rs` | `impl From<SearchStats> for PlySearch` after the struct | verbatim side and value; positive visits in input order, raw; `take(RECORDED_PRIOR_TOP_K)` prior prefix, unrenormalized; total |
| `selfplay/src/game.rs` | move loop | inline block and its comment gone; one `Option::map` call on `decision.search`; nothing else changed |
| `selfplay/src/game.rs` | `PlySearch.prior` doc | "top root priors" |
| `selfplay/src/record.rs` | `RecordedPly.prior` doc | names the `RECORDED_PRIOR_TOP_K` bound; intra-doc link resolves |
| untouched | corpus writer/readers, `GAME_RECORD_FORMAT_VERSION`, schedule, certified suffix, `train`, `player`, `mcts` | the record side copies the slot as-is; absence stays `None`-mapped by chapter 02's conversion |

## References

**This chapter:**

- [recording-the-search.md](recording-the-search.md) — the chapter
  paper: intention, specification as tests, execution-order
  walkthrough.
- [recording-the-search-design.md](recording-the-search-design.md) —
  the conceptual story and the design invariants this plan builds
  under.
- [recording-the-search-solution.md](recording-the-search-solution.md)
  — the annotated reference snippets. Open only after
  twenty stuck minutes, or afterwards to compare.

**This track:**

- [Tutorial README](../README.md) — conventions, gates, and the
  chapter map.
- [Self-play primer](../00-selfplay-primer.md) — §3.2 pins the sparse
  storage decisions; §8 pins invariants 5 (π normalized at read time),
  6 (z derived at read time), and 7 (honest search records).
- [Chapter 01 — the game runner](../01-the-game-runner/the-game-runner.md),
  its [implementation plan](../01-the-game-runner/the-game-runner-implementation.md),
  and its [solution](../01-the-game-runner/the-game-runner-solution.md)
  — the inline conversion this chapter replaces, the "all root priors"
  slice wording, and the MANUAL positivity and visit-sum contract.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md),
  its [implementation plan](../02-recording-games/recording-games-implementation.md),
  and its [solution](../02-recording-games/recording-games-solution.md)
  — `RecordedPly`, the as-is persistence of the slot, and the second
  "all root priors" slice wording.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  §3.2: the record format, the sparse top-k prior with k = 16 as
  **[experiment]**, and the audit purpose of the prior.
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  this chapter's public surface and the unsearched-ply representation.

**Prerequisite tracks:**

- [Baseline tutorial](../../baseline-tutorial/README.md) —
  `AiPlayer::decide`, `SearchStats`, and the honesty rule for
  certified-proof moves.
- [MCTS tutorial](../../mcts-tutorial/README.md) — the search whose
  root visit distribution and edge priors `SearchStats` extracts.
- [Engine tutorial](../../13-engine-tutorial/README.md) — the TSS
  prover and `verify_line` behind the certified-proof bypass.
- [docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) —
  defines the **[paper]** / **[derived]** / **[experiment]** claim-label
  convention used for k = 16.

**Source (created by chapters 01 and 02, touched by this chapter; not
yet present on `main`):**

- `gomoku/crates/selfplay/src/game.rs` — gains
  `impl From<SearchStats> for PlySearch`; the move loop's inline
  conversion becomes one `Option::map` call; the `PlySearch.prior` doc
  comment updates.
- `gomoku/crates/selfplay/src/record.rs` — gains
  `RECORDED_PRIOR_TOP_K`; the `RecordedPly.prior` doc comment names
  the bound.
- `gomoku/crates/selfplay/src/lib.rs` — re-exports
  `RECORDED_PRIOR_TOP_K`.
- `gomoku/crates/selfplay/tests/game.rs` — the chapter's
  specification: the two conversion tests.

**Source (unchanged in this chapter):**

- `gomoku/crates/selfplay/tests/corpus.rs` — chapter 02's
  specification; regression evidence for the record side of the seam.
- [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs) —
  `AiPlayer`, `Decision`, `SearchStats` — the conversion's input and
  the honesty rule.
- [`gomoku/crates/mcts/src/policy.rs`](../../../../gomoku/crates/mcts/src/policy.rs) —
  `visit_distribution`, `add_dirichlet_noise`,
  `select_move_with_priors` — the origin of the orderings the record
  keeps.

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). https://doi.org/10.1038/nature24270 —
  the visit-distribution policy target π whose raw counts the record
  keeps, and the root Dirichlet noise the recorded prior reflects.
- Rosin, C. D. (2011). *Multi-armed bandits with episode context*
  (PUCT). https://doi.org/10.1007/s10472-011-9258-6 — the selection
  rule whose P is the recorded prior.
- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://doi.org/10.26481/dis.19940923la — threat-space search, the
  source of the certified moves that bypass the record.
