# Chapter 03 — Recording the search

## Abstract

This chapter teaches the `selfplay` crate what a search record *means*.
Chapter 01 gave every searched ply a `PlySearch` slot and filled it with
an inline conversion from the seat's `player::SearchStats`; chapter 02
persisted that slot as-is. This chapter promotes the inline conversion
to a named, test-pinned `From<player::SearchStats> for PlySearch`
implementation and fixes the three semantics the durable record depends
on: the root value keeps the side to move's sign, verbatim and
unnegated; the visit list keeps only positive counts, raw and
unnormalized; and the prior is truncated to its first
`RECORDED_PRIOR_TOP_K = 16` entries, in the search's reported order
(descending by value), without re-sorting and without renormalization. Plies where no search
ran — a move played from a certified threat-space search (TSS) proof —
keep no `PlySearch` at all in memory and persist as empty visits, an
empty prior, and `root_value: None` on disk. The chapter's public
surface is `RECORDED_PRIOR_TOP_K` and the `From` implementation; no new
types appear. The conclusions: a recorded search can later become
training targets — the visit counts normalize into the policy target π
at read time — and can support audits, without sign ambiguity and
without unbounded prior lists; and the record never confuses "no search
ran" with "the search estimated zero". What this chapter deliberately
does not do: it does not normalize anything, does not derive the value
target z, and does not touch corpus generation, scheduling, certified
suffixes, replay, export, or acceptance. The intended reader is working
the selfplay-corpora tutorial in order and has completed chapter 02.

## Glossary

| Term | Definition |
|------|------------|
| **Seat** | One of the two players of a generated game: a persistent `player::AiPlayer` running the baseline evaluator at a declared simulation budget (chapter 01). Every game pairs two seats; this chapter speaks of the *deciding* seat of a ply. |
| **SearchStats** | `player::SearchStats`, the statistics the seat extracts from one real searched tree: the side to move, every root move with its visit count (descending by count), every root prior (descending by value), and the root value. Absent when a certified proof bypassed search. |
| **PlySearch** | The `selfplay` crate's per-ply search slot: `side_to_move`, `visits`, `prior`, `root_value`. This chapter owns its conversion semantics. |
| **PlyRecord** | The runner's per-ply record (chapter 01): `mv`, the played move, and `search`, an `Option<PlySearch>` — `None` for a certified-proof move. |
| **Side to move** | The color whose turn the searched position was — `board.to_move()` at decision time. The root value's sign is defined relative to this side. |
| **Root value** | The search's value estimate at the root after the simulation budget is spent, from the side to move's perspective: the sum of the root edges' accumulated values divided by the sum of their visit counts, or 0.0 when no edge was visited. Recorded as metadata, not as a training target. |
| **Prior** | The evaluator's policy probabilities over root moves before search — the P in predictor UCT (PUCT; Rosin 2011) — after the optional root Dirichlet noise (Silver et al. 2017) when noise was applied. The post-noise qualification holds on ordinary play; a resignation's statistics are pre-noise and never recorded, because a resignation produces no ply. Recorded sparsely for audit. |
| **Top-k** | Keeping only the k leading entries of an ordered list. Here k is `RECORDED_PRIOR_TOP_K = 16`. |
| **Sparse** | Storing only the entries that carry information: visited root children for the visit counts, the top-k for the priors. Unvisited children and the prior's tail are omitted. |
| **π (visit-distribution target)** | The normalized root visit-count distribution of the search at a ply; the policy training target (Silver et al. 2017). The record stores raw counts; normalization happens at read time in chapter 06. |
| **z (outcome target)** | The game result in {+1, 0, −1} from the perspective of the side to move at a ply; derived from the stored outcome at read time in chapter 06, never stored per ply. |
| **Certified forced win** | A forced-win line produced by the engine's threat-space search (TSS) prover (Allis 1994) and checked by `engine::verify_line`. A seat that holds one plays its first move immediately; no search runs, and the ply carries no search statistics. |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games. It never runs by default; it runs on explicit selection, in release mode. |
| **Swap2** | The project's opening protocol: one side places three stones, the other side chooses a color or places two more stones, and the first side then chooses a color. Its ceremony fixes the starting position and the colors, which is what makes the side to move derivable on replay. |
| **Claim labels** | The project's provenance labels (chapter 12): **[paper]** (stated in a cited paper), **[derived]** (computed from cited or measured values), **[experiment]** (a free parameter, to be settled by measurement). |

## Context

Three sources supply what this chapter refines. Each summary states
what the source delivered and why this chapter needs it.

- [Chapter 01 — the game runner](../01-the-game-runner/the-game-runner.md) —
  built `PlySearch` as a slot and filled it with an inline conversion
  inside the move loop: copy the side to move, keep only the
  positive-count visit entries, carry the prior through untruncated,
  copy the root value. That inline form is this chapter's starting
  point. Chapter 01's MANUAL test
  `tiny_budget_game_records_raw_visits_for_every_searched_ply` pins two
  properties this chapter must preserve through the refactor: every
  recorded count is positive, and the counts of one ply sum to the
  deciding seat's simulation budget.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md) —
  built the durable side of the seam: `RecordedPly` with `mv`,
  `visits`, `prior`, and `root_value: Option<f32>`, persisted by
  `GameRecord::from_game` copying the slot data as-is — empty visits,
  empty prior, and `root_value: None` when the ply's `search` is
  `None`. Its round-trip test
  `populated_search_fields_round_trip_byte_exactly` pins that mapping
  byte-exactly. This chapter changes nothing on the record side; it
  changes what the slot *guarantees* before the record copies it.
- [Baseline tutorial](../../baseline-tutorial/README.md) — built
  `player`, whose `AiPlayer::decide` returns a `Decision` carrying
  `search: Option<SearchStats>`. The `Option` is the honesty rule of
  that track: statistics come from a real searched tree, never from a
  proof — a certified own-side forced win bypasses search and the
  decision carries `None`. This chapter consumes that contract; it
  does not weaken it.

The [tutorial README](../README.md) fixes this track's conventions;
the [primer](../00-selfplay-primer.md) §3.2 pins the storage
decisions (π sparse and normalized at read time, the prior sparse
top-k) and §8 pins the invariants this chapter enforces (invariants
5, 6, 7). The
[design spec](../../../specs/2026-11-11-selfplay-corpora-design.md)
§3.2 is the design authority for the record format, including the
sparse top-k prior with k = 16 as an **[experiment]** — a free
parameter of this track, not a constant from the literature or the
architecture chapters.

## Intention

You can take what a seat's real search saw at one ply and keep it in
the game in a form you can trust later. Months from now, chapter 06's
reader — or an audit script you have not written yet — picks up a
recorded ply and knows exactly what it holds: visit counts that
normalize into the policy target π, with no fabricated zero-count
entries diluting them; a bounded list of the sixteen strongest priors,
in the order the search itself ranked them, small enough that a record
stays a few kilobytes; and a root value whose sign needs no guessing,
because it is always the estimate of the side that was to move. When a
ply carries no search data, that absence is exact too: empty lists and
no value mean *no search ran*, never *the search estimated zero* or
*nothing was visited*. The searches the runner already performs become
durable training material — without sign ambiguity, and without
unbounded prior lists.

## Specification as tests

The tests live in
[`gomoku/crates/selfplay/tests/game.rs`](../../../../gomoku/crates/selfplay/tests/game.rs),
the game target this chapter shares with chapter 01. The list below is
ordered along the execution path of the conversion — the sign
convention first, then the truncation rule, then the regression
evidence from the earlier chapters. The reference file's lexical order
already leads with this chapter's two tests.

This chapter specifies **two** tests. Both run by default; neither is
ignored; neither plays a game. Each builds its `player::SearchStats`
input by hand, so each is a test of the conversion, not of the search
— and both finish in under a second in a debug build. No new MANUAL
test is added: nothing here plays a game, and the game-playing
properties the refactor must preserve are already pinned by chapter
01's MANUAL pair.

One count note, stated now so the numbers stay honest. Chapter 01's
slice of this target held five tests: three default plus the two
MANUAL-ignored. This chapter adds the two conversion tests, so the
target grows to **five default, two ignored** — and that is also the
completed reference's exact state: its `tests/game.rs` holds these
same seven tests, no more. The reference's full `selfplay` suite is
larger (50 passed, 4 ignored) only because later chapters add their
own targets. The corpus target is untouched by this chapter and stays
at chapter 02's **ten default, zero ignored**.

### The sign convention

- `ply_search_preserves_the_side_to_move_root_value_sign` — two
  hand-built `SearchStats` values, both with empty visit and prior
  lists: one with `side_to_move` Black and `value` 0.875, one with
  `side_to_move` White and `value` −0.625. Converting both with
  `PlySearch::from` preserves each side to move and each value
  exactly: Black keeps 0.875, White keeps −0.625.

This test pins the sign convention of the whole record format in one
place: the conversion never negates and never re-anchors a value to a
fixed color. A negative root value recorded on White's ply stays
negative — it is White's estimate of White's position — and the stored
side to move is what makes that reading unambiguous later.

### The truncation rule

- `ply_search_truncates_prior_to_top_16_without_reordering` — a
  hand-built prior of twenty entries on distinct moves with strictly
  decreasing values (1.0, 0.99, 0.98, …), plus one visit entry.
  Converting with `PlySearch::from` yields a recorded prior equal to
  the first `RECORDED_PRIOR_TOP_K` entries of the input, in the input's
  order — and the test also asserts `RECORDED_PRIOR_TOP_K` equals 16,
  so a silent change of the format width fails loudly.

This test pins two properties at once: the recorded prior is a
*prefix* of the search's ordering (a truncation, not a selection by
some other criterion), and the conversion applies no ordering of its
own — the sixteen entries come out in exactly the order they went in.
The strictly decreasing values make any re-sorting visible; the
twenty-entry input makes the cut visible.

### Regression evidence from chapters 01 and 02

No new test re-pins what the earlier chapters already pin; their tests
are part of this chapter's specification because the refactor must not
break them:

- `tiny_budget_game_records_raw_visits_for_every_searched_ply`
  (chapter 01, MANUAL) — on a real game at budget 2 versus budget 1,
  every searched ply's visit list is non-empty, every count is
  positive, and the counts sum to the deciding seat's budget. After
  this chapter, those properties hold *through the new `From`
  implementation* rather than through the inline block: the positive
  filter is specified here, at the runner level, and this chapter
  deliberately does not duplicate it in a conversion unit test.
- `tiny_budget_game_completes_and_records_search_metadata` (chapter
  01, MANUAL) — every recorded root value of a real game is finite.
- Chapter 01's three default tests — the ceremony, the legal replay,
  and whole-game determinism — exercise the refactored conversion on
  every budget-1 game they play.
- `populated_search_fields_round_trip_byte_exactly` (chapter 02) —
  populated search fields persist and round-trip byte-exactly. Its
  fixtures build `PlySearch` by hand, so it does not exercise the
  `From` conversion; it pins the other side of the seam — the record
  copies the slot as-is, and this chapter does not touch that mapping.
  The whole corpus target, ten tests, stays green unchanged.

## Walkthrough

The order is the execution order of one ply's search data, from the
moment the seat finishes thinking to the moment the record rests on
disk. Each step names the artifact at the moment execution first
touches it. Two files change: `RECORDED_PRIOR_TOP_K` joins the record
vocabulary in
[`src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs),
and the `From` implementation joins `PlySearch` in
[`src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs),
which imports the constant from the record module.

### 1. Where the statistics come from: `Decision.search`

Every ply's data begins inside the `player` crate, which this tutorial
does not modify. `AiPlayer::decide` runs the search and extracts a
`SearchStats` from the real tree; understanding its four fields is the
prerequisite for every conversion rule below:

- `side_to_move` is `board.to_move()` at decision time — the color
  whose turn the searched position was.
- `visits` lists *every* root move with its visit count, descending by
  count with ties broken by move index — including zero-count entries
  for children the search never visited.
- `prior` lists *every* root prior, descending by value with the same
  tie-break. On ordinary play the statistics are extracted *after* the
  optional Dirichlet noise was mixed into the root, so the prior the
  conversion receives is the prior the move selection actually saw.
- `value` is the root estimate from the side to move's perspective:
  the accumulated edge values summed over the root edges, divided by
  the summed visit counts — or exactly 0.0 when no edge was visited at
  all.

And the `Option` around it is the honesty rule: when the seat's
own-side TSS probe certified a forced win, the decision plays the
proof's first move and carries `search: None`. A resignation, when
enabled, carries statistics but ends the game without a played move —
so it produces no ply, and nothing from it is ever recorded.

### 2. The conversion seam: from inline block to `From`

Chapter 01's move loop converted the statistics inline: a closure over
`decision.search` that built the `PlySearch` field by field. This
chapter replaces that block with a named conversion,
`impl From<SearchStats> for PlySearch`, placed next to `PlySearch` in
`src/game.rs`, and the loop now reads as one call:
`decision.search.map(PlySearch::from)`. The `Option::map` idiom is
unchanged — it was already the right shape — but the conversion now
has a name, and the name is what the two specification tests pin.

The refactor is deliberate about scope. Three of the four field rules
move verbatim from the inline block into the implementation; one rule
is new — the prior truncation of step 5. Everything else about the
loop, the slot, and the record stays byte-identical, which is what the
regression tests of the specification are for.

### 3. The side to move and the root value's sign

The conversion copies `side_to_move` and the root value across
verbatim — no negation, no re-anchoring, no clamping. The value is the
deciding side's estimate of its own position, and it stays that way in
the record: 0.875 recorded on Black's ply is Black's confidence;
−0.625 recorded on White's ply is White's worry. The sign-convention
test pins both directions so a future "helpful normalization" — say,
re-expressing every value from Black's perspective — fails loudly
instead of silently corrupting a corpus.

Why keep the sign relative to the side to move at all, rather than
normalize to one color? Because the value's consumer reads it together
with the position. The recorded root value is **metadata for audit** —
it answers "how good did the search think this position was, for the
player who faced it?" — and that question is only meaningful in the
facing player's frame. What the root value is **not**: it is not the
value target z. Phase 1 training derives z per ply from the stored
game outcome at read time (chapter 06), from the side to move's
perspective; the recorded root value is never a training target in
this track. It exists so corpora can be audited — comparing the
search's own estimates against the outcomes those games actually
reached.

### 4. The visits: positive counts, kept raw

The conversion keeps only the positive-count entries of the visit
list. This rule is inherited unchanged from chapter 01's inline block,
and the reason is unchanged too: `SearchStats.visits` lists every root
move, and a zero-count child carries no information — it contributes
nothing to the count sum and nothing to the future policy target. The
MANUAL visit-sum test pins the surviving invariant end to end on real
games: the recorded counts of one ply sum to the deciding seat's
simulation budget.

Two things the conversion deliberately does **not** do to the counts.
It does not **normalize**: the counts stay raw, because π is a
read-time derivation (chapter 06), and storing raw counts keeps the
record honest about how much search stood behind each entry. And it
does not **sort**: the entries keep the player's extraction order —
descending by count, ties by move index. Move selection itself
consumes the root edges in creation order; the record keeps the order
the player's extraction reported.

### 5. The prior: sixteen entries, in the search's order

The conversion truncates the prior to its first
`RECORDED_PRIOR_TOP_K` entries — a constant this chapter adds to
`src/record.rs`, equal to 16, re-exported from the crate root. The
constant lives with the record vocabulary because it is a *format
width*: it bounds what any reader of the durable format must expect,
so it belongs beside `RecordedPly`, not beside the in-memory slot.

Three properties are pinned together. The cut is a **prefix**: the
player already ordered the priors descending by value, so taking the
first sixteen *is* taking the top sixteen — the truncation test's
strictly decreasing input makes any deviation visible. The order is
**preserved**: the conversion applies no sort of its own, because
re-sorting would duplicate the player's work and risk diverging from
the order the player's extraction reported. And the values are **not
renormalized**: the stored entries are the probabilities as mixed,
which sum to at most one — strictly less only when the truncation
removed positive prior mass, while a ply with no more than sixteen
legal moves retains the full normalized prior. That is acceptable
because the recorded prior is an audit record, not a distribution to
sample from — the design spec's stated purpose is comparing the
baseline's priors against a trained network's priors on identical
positions.

Why sixteen, and why truncate at all? The width is **[experiment]**
(design spec §3.2): a free parameter of this track, settled by
measurement, not a constant from the literature. The rationale for
bounding it at all is cost versus value. A 15×15 board has up to 225
root priors per ply; storing all of them would multiply the record's
size for entries that are, after the first handful, near-uniform tail
mass under the baseline's concentrated policy. The audit value of the
tail is low — and it is recoverable: the baseline evaluator is
deterministic, so the full prior of any recorded position is
recomputable by replaying to that ply and re-evaluating. The stored
top-16 makes casual inspection cheap; recomputation makes the omission
safe.

### 6. Plies without search: `None`, not zero

The `Option::map` in the loop is the whole mechanism for unsearched
plies: a decision with `search: None` — the certified-proof bypass of
step 1 — produces a `PlyRecord` whose `search` is `None`. No
`PlySearch` is constructed, so there is no struct in memory that could
hold fabricated zeros.

The durable representation of that absence belongs to chapter 02's
conversion, and it is worth restating precisely, because the two
layers say it differently. In memory the statement is strong: there is
no `PlySearch` at all. On disk, `GameRecord::from_game` maps the
absent slot to a `RecordedPly` with **empty visits, an empty prior,
and `root_value: None`** — the unsearched-ply representation this
track's invariants promise. The indirection is exact: the `Option`
around `PlySearch` becomes the `Option` around `root_value`, and the
empty vectors are what a missing slot maps to, not what a search
produced.

The empty-versus-zero distinction is the load-bearing point, and the
format keeps the cases strictly apart. `root_value: None` means *no
search ran*. `root_value: Some(0.0)` is a real recorded value: a
search that ran and visited no edge reports exactly 0.0 by the
`SearchStats` convention — reachable only at a zero simulation budget,
which no corpus configuration uses. Empty visits on a *searched* ply
are likewise only reachable at zero budget, because one or more
simulations always visit at least one child. So on any real corpus
ply: searched plies carry non-empty visits and `Some(value)`;
unsearched plies carry empty visits, an empty prior, and `None`; and
no reading of the record can mistake one case for the other.

### 7. Persistence: `RecordedPly`, and why the side to move is derived, not stored

From here the data meets chapter 02's machinery unchanged. Each
`PlySearch` becomes one `RecordedPly`: the move, the visits, the
prior, and `root_value` lifted from `f32` to `Option<f32>`. The record
copies; it does not convert — no truncation, no filtering, and no sign
handling live on the record side, because this chapter has already
made the slot data exactly what the record should hold.

One absence in `RecordedPly` deserves explanation: there is no
`side_to_move` field, although `PlySearch` has one. The in-memory slot
keeps the side because it is needed at the point of use — the MANUAL
visit-sum test selects each ply's expected budget from
`search.side_to_move`, and the field makes the root value's sign
self-describing while the game is in flight. On disk the side to move
is **derivable**: the recorded opening fixes the starting position,
White moves first after the Swap2 ceremony, colors alternate ply by
ply, and chapter 06's read path replays every game through the engine
anyway — the replay yields the side to move at every ply for free.
Storing it in every ply would duplicate derivable data into a durable
format and open a consistency hazard: a stored value that disagrees
with the replay. The sign convention loses nothing by the omission —
the replayed side to move *is* the recorded side to move, because the
statistics were extracted from the position the replay reconstructs —
so the root value's sign stays unambiguous with no field spent.

## What this chapter defers

- Normalizing the visit counts into π and deriving z per ply from the
  stored outcome — chapter 06's read path. This chapter keeps both raw
  and stored once.
- The exploration schedule's behavior, tournament openings, seeded
  generation with deduplication, and the `selfplaygen` binary —
  chapter 04.
- Detecting and recording certified suffixes — chapter 05. This
  chapter's `None`-for-unsearched-plies semantics is the contract the
  suffix detection will rely on, but no suffix logic exists yet.
- Replay verification and the acceptance gates — chapter 07.

## TDD checklist

Work one artifact at a time, red to green, in
[`gomoku/crates/selfplay/`](../../../../gomoku/crates/selfplay/). No
manifest changes: `selfplay` already depends on `player`, and the
tests already import from it.

1. `RECORDED_PRIOR_TOP_K: usize = 16` in
   [`src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs),
   documented as the format's sparse audit width — a selfplay-corpora
   format invention, not a literature constant — and re-exported from
   [`src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs)
   beside the record vocabulary.
2. Red: write
   `ply_search_preserves_the_side_to_move_root_value_sign` and
   `ply_search_truncates_prior_to_top_16_without_reordering` in
   [`tests/game.rs`](../../../../gomoku/crates/selfplay/tests/game.rs).
   The test imports gain `player::SearchStats`, `PlySearch`, and the
   crate's `RECORDED_PRIOR_TOP_K`.
3. Green: `impl From<SearchStats> for PlySearch` in
   [`src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs),
   next to `PlySearch`, importing the constant from the record module.
   Both new tests pass.
4. Replace chapter 01's inline conversion block in the move loop with
   the named conversion — the `Option::map` call on
   `decision.search` — and update the two doc comments that the chapter 01
   and chapter 02 solution papers marked as slice wording: `PlySearch.prior` now reads
   "top root priors" instead of "all root priors", and
   `RecordedPly.prior` names the `RECORDED_PRIOR_TOP_K` bound. The
   whole default suite stays green; this step must not change a single
   test outcome.
5. Regression: run chapter 01's MANUAL pair once in release mode. Both
   now exercise the named conversion on real games.

## Capstone

The chapter's specification is two fast, focused tests:

```text
cargo test -p selfplay --test game ply_search
```

The `ply_search` prefix selects exactly the two conversion tests. No
game is played — the fixtures are hand-built `SearchStats` values —
and the run finishes in well under a second. Then the proof that the
refactor changed nothing on real games:

```text
cargo test -p selfplay --release --test game -- --ignored tiny_budget_game_records_raw_visits_for_every_searched_ply
```

A real budget-2-versus-budget-1 game flows through the named
conversion, and every searched ply's raw counts still sum to the
deciding seat's budget — the same contract, now carried by a named,
test-pinned implementation instead of an inline block.

## Done when

- `cargo test -p selfplay` reports **15 passed, 2 ignored**: the game
  target reports 5 passed, 2 ignored (chapter 01's three defaults plus
  this chapter's two), and the corpus target reports 10 passed,
  unchanged from chapter 02. (These are also the completed reference's
  exact counts for both targets; the reference's full suite is larger
  only because later chapters add their own targets.)
- Both MANUAL tests pass under the release command below.
- `cargo test -p train` and `cargo test -p player` are not required:
  this chapter touches no crate but `selfplay`.
- `cargo clippy --all-targets -- -D warnings` is green.
- `cargo fmt --all -- --check` makes no changes.
- The commit message is
  `feat(selfplay): recording the search — selfplay-corpora chapter 03`.

## Verification commands

From `gomoku/`:

```text
cargo test -p selfplay
cargo test -p selfplay --test game ply_search
cargo test -p selfplay --test corpus
cargo test -p selfplay --release --test game -- --ignored
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the fast gate: every default test of the track so
far runs in seconds, and this chapter adds nothing slow — both new
tests build their inputs by hand and play no games. The second command
is the focused check of this chapter's specification, selected by the
shared `ply_search` prefix. The third command re-runs chapter 02's
target alone: the record side of the seam must be provably untouched.
The fourth is the release-mode game-playing verification from chapter
01, still the only `#[ignore]`d selection this track has; the
`--release` qualifier is not optional for it. No `--release` qualifier
and no `--ignored` selection belong to the new tests themselves.

## References

**This track:**

- [Tutorial README](../README.md) — conventions, gates, and the
  chapter map.
- [Self-play primer](../00-selfplay-primer.md) — §3.2 pins the sparse
  storage decisions; §8 pins invariants 5 (π normalized at read time),
  6 (z derived at read time), and 7 (honest search records).
- [Chapter 01 — the game runner](../01-the-game-runner/the-game-runner.md) —
  the `PlySearch` slot, the inline conversion this chapter replaces,
  and the MANUAL positivity and visit-sum contract.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md) —
  `RecordedPly`, the as-is persistence of the slot, and the unsearched
  ply's durable shape.
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

**Source:**

- [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs) —
  `PlySearch` gains `From<player::SearchStats>`; the move loop's inline
  conversion becomes one `Option::map` call. Touched in this chapter.
- [`gomoku/crates/selfplay/src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs) —
  gains `RECORDED_PRIOR_TOP_K`; the `RecordedPly.prior` documentation
  names the bound. Touched in this chapter.
- [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs) —
  re-exports `RECORDED_PRIOR_TOP_K`. Touched in this chapter.
- [`gomoku/crates/selfplay/tests/game.rs`](../../../../gomoku/crates/selfplay/tests/game.rs) —
  this chapter's specification: the two conversion tests. Touched in
  this chapter.
- [`gomoku/crates/selfplay/tests/corpus.rs`](../../../../gomoku/crates/selfplay/tests/corpus.rs) —
  chapter 02's specification, unchanged; regression evidence for the
  record side of the seam.
- [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs) —
  `AiPlayer`, `Decision`, `SearchStats` — the conversion's input.
  Not touched in this chapter.

**Companion papers:**

- [recording-the-search-design.md](recording-the-search-design.md) —
  the conceptual story of the search record.
- [recording-the-search-implementation.md](recording-the-search-implementation.md) —
  the hiding layer: low-level construction instructions.
- [recording-the-search-solution.md](recording-the-search-solution.md) —
  the annotated reference snippets.

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
