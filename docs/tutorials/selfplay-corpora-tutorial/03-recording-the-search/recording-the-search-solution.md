> **Opt-in solution — read it only after attempting the chapter with the
> [implementation plan](recording-the-search-implementation.md), or after being
> stuck for more than twenty minutes.**
>
> This paper is inspiration, not authority over the learner's own code. The
> learner's implementation on `main` is the reference; the artifacts below
> exist so a stuck or finished learner can compare. No learner source is
> changed by this paper.

# Chapter 03 — Recording the search: solution

## Abstract

This paper is the annotated reference companion to
[chapter 03 of the selfplay-corpora tutorial](recording-the-search.md). It
quotes everything the chapter creates, along the execution path of one ply's
search data: the format-width constant `RECORDED_PRIOR_TOP_K` in the record
vocabulary and its crate-root re-export, the two conversion tests that are
this chapter's specification, the `From<player::SearchStats> for PlySearch`
implementation, the replacement of chapter 01's inline conversion block in the
runner's move loop by one `Option::map` call, the two doc-comment updates that
retire the "all root priors" slice wording of chapters 01 and 02, and the
chapter's verification evidence — the focused `ply_search` run, the full
`selfplay` suite, chapter 01's MANUAL pair in release mode, and the standing
clippy and rustfmt gates. Both new test bodies are quoted in full. Every
snippet carries an explanation, source line numbers, and an exact provenance
note: most fragments are byte-identical to the completed reference
implementation, and the two that are not are the chapter-3 slice forms the
approved papers specify — each such difference is named explicitly. All
chapter gates ran green on the chapter-3 slice on 2026-11-13; the precise
evidence is in the Provenance and Verification sections. The intended reader
is the learner who has read the chapter paper and, ideally, worked the
[implementation plan](recording-the-search-implementation.md) first.

## Glossary

| Term | Definition |
|------|------------|
| **Seat** | One of the two players of a generated game: a persistent `player::AiPlayer` running the baseline evaluator at a declared simulation budget (chapter 01). This paper speaks of the *deciding* seat of a ply. |
| **Simulation budget** | The number of Monte Carlo Tree Search (MCTS) simulations a seat runs per move decision. |
| **SearchStats** | `player::SearchStats`, the statistics the seat extracts from one real searched tree: the side to move, every root move with its visit count (descending by count, ties by move index), every root prior (descending by value, ties by move index), and the root value. Absent when a certified proof bypassed search. |
| **PlySearch** | The `selfplay` crate's per-ply search slot: `side_to_move`, `visits`, `prior`, `root_value`. This chapter owns its conversion semantics. |
| **PlyRecord** | The runner's per-ply record (chapter 01): `mv`, the played move, and `search`, an `Option<PlySearch>` — `None` for a certified-proof move. |
| **RecordedPly** | The durable per-ply record (chapter 02): `mv`, sparse raw `visits`, sparse `prior`, and `root_value: Option<f32>`. The disk image of one `PlyRecord`; unchanged in this chapter. |
| **Side to move** | The color whose turn the searched position was — `board.to_move()` at decision time. The root value's sign is defined relative to this side. |
| **Root value** | The search's value estimate at the root after the simulation budget is spent, from the side to move's perspective. Recorded as metadata, not as a training target. |
| **Prior** | The evaluator's policy probabilities over root moves before search, after the optional root Dirichlet noise (Silver et al. 2017) when noise was applied. Recorded sparsely for audit. |
| **Top-k** | Keeping only the k leading entries of an ordered list. Here k is `RECORDED_PRIOR_TOP_K = 16`. |
| **Sparse** | Storing only the entries that carry information: visited root children for the visit counts, the top-k for the priors. |
| **π (visit-distribution target)** | The normalized root visit-count distribution of the search at a ply; the policy training target (Silver et al. 2017). The record stores raw counts; normalization happens at read time in chapter 06. |
| **z (outcome target)** | The game result in {+1, 0, −1} from the perspective of the side to move at a ply; derived from the stored outcome at read time in chapter 06, never stored per ply. |
| **Certified forced win** | A forced-win line produced by the engine's threat-space search (TSS) prover (Allis 1994) and checked by `engine::verify_line`. A seat that holds one plays its first move immediately; no search runs, and the ply carries no search statistics. |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games. It never runs by default; it runs on explicit selection, in release mode. This chapter adds none. |
| **Claim labels** | The project's provenance labels (chapter 12 of the wiki): **[paper]** (stated in a cited paper), **[derived]** (computed from cited or measured values), **[experiment]** (a free parameter, to be settled by measurement). The width k = 16 is **[experiment]**. |
| **Chapter-3 slice** | The chapter-3 subset of the completed reference: chapters 01–03 together, without the later chapters' additions. Defined and verified in the Provenance section. |
| **TDD** | Test-driven development: write the test, watch it fail (red), implement, watch it pass (green). |

## Provenance, the chapter-3 slice, and verification

### The reference and its completed-tree form

The reference implementation lives in a separate, read-only worktree beside
this repository (`rust-ml-selfplay-ref`, branch `selfplay-ref-v2`, committed
base `9028aa8` on `main`, all reference changes uncommitted). It was built
test-first and gate-verified before this tutorial was written; the
[hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md)
records its measured evidence (workspace gates green; `cargo test -p
selfplay` reporting 50 passed, 4 ignored across all test targets). The
worktree serves quotation only: never point a manifest at it, never build the
learner's crate from it, never commit there. Chapter 01's
[solution paper](../01-the-game-runner/the-game-runner-solution.md)
established the provenance discipline this paper continues; chapter 02's
[solution paper](../02-recording-games/recording-games-solution.md) applied
it to the record format this chapter extends.

One structural fact shapes everything below: the worktree holds the
**completed** crate — all seven chapters at once. Its
`gomoku/crates/selfplay/src/game.rs` already carries chapter 04's exploration
schedule application (`ExplorationSchedule::options_for_ply` in the move
loop) and chapter 05's suffix tracker with `decision.own_proof` and the
`suffix.finish()` assembly; its `src/lib.rs` declares and re-exports the
later chapters' modules; and its `src/record.rs` documents `SeatConfig`'s
temperature field in chapter 04's vocabulary. Quoting those regions verbatim
would leak later chapters and would contradict this chapter's approved
papers, which fix the chapter-3 form: the schedule type exists (chapter 02)
but has no behavior, the certified-suffix slot is `None`-initialized
(chapter 02), and the conversion and format width of this chapter are present
in full (see
[the implementation plan](recording-the-search-implementation.md), steps 1
through 4). This paper therefore distinguishes two kinds of quotation:

- **Reference-verbatim fragments.** Most of the chapter-3 code is stable:
  the completed reference carries it unchanged. These fragments are quoted
  exactly, and each carries a note naming the reference file and line range
  it was compared against. The comparisons were scripted, not eyeballed;
  every range reported as byte-identical was checked programmatically
  (trailing newline structure included).
- **Chapter-3 slice forms.** Two fragments sit in regions where the
  completed reference has already grown past chapter 03: the `src/game.rs`
  import block and the move-loop call-site region. This paper quotes the
  chapter-3 form specified by the approved papers, and names exactly what
  the completed reference adds later. No later-chapter implementation is
  quoted.

### The slice was built and run

Quoting a slice the learner could not compile would violate the track's
first rule — a tutorial that does not run error-free is a defect. So the
chapter-3 slice was constructed and gate-verified on 2026-11-13 at
`/tmp/selfplay-ch3-verify` (a scratch path, not a repository artifact). The
starting point was not the reference worktree but an *untouched copy* of the
already-verified chapter-2 slice at `/tmp/selfplay-ch2-verify` — so the
chapter-3 slice differs from chapter 02's verified state by exactly this
chapter's edits, and neither the chapter-2 scratch nor the reference
worktree was modified:

1. The chapter-2 scratch was copied in full, excluding its `target/`
   build directory.
2. `src/record.rs` gained the `RECORDED_PRIOR_TOP_K` constant beside
   `GAME_RECORD_FORMAT_VERSION`, and the `RecordedPly.prior` doc comment
   was updated to name the bound.
3. `src/lib.rs` gained the constant's crate-root re-export.
4. `src/game.rs` gained `SearchStats` and `record::RECORDED_PRIOR_TOP_K`
   imports, the `From<SearchStats> for PlySearch` implementation after the
   `PlySearch` struct, the one-line `Option::map` call replacing chapter
   01's inline conversion block and its comment, and the updated
   `PlySearch.prior` doc comment.
5. `tests/game.rs` gained the two conversion tests at the top of the test
   list and the two import extensions.

Nothing else was touched: no schedule behavior, no suffix tracking, no
opening lists, no generation, no read path, no `player`-crate change — the
completed reference's `tests/game.rs` is byte-identical to the slice's
(scripted comparison), and the slice's `tests/corpus.rs` is likewise
byte-identical to the reference's, because this chapter leaves chapter 02's
specification untouched.

The chapter's verification commands were run from the copy's `gomoku/`, with
stdout and stderr saved to `/tmp/selfplay-ch3-verify-logs/` (a scratch path,
not a repository artifact):

```text
cargo test -p selfplay --test game ply_search
cargo test -p selfplay
cargo test -p selfplay --test corpus
cargo test -p selfplay --release --test game -- --ignored
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

All six commands passed on the slice; the measured evidence — per-target
counts, timings, and the count notes that relate the slice's counts to the
completed reference's — is tabled in [§7](#7-verification). The slice
fragments quoted below are exactly the files those runs exercised.

### Reading rules

The section order is the execution order of one ply's search data through
this chapter's edits — the format width the conversion needs, the tests that
pin the conversion, the conversion, the refactor that routes the runner
through it, the documentation that records the new guarantee — followed by
the verification commands. This is the implementation plan's construction
order.

Every code fragment carries a right-aligned `NN |` line-number margin. For
fragments from files this chapter edits, the numbers are the edited
chapter-3 file's own line numbers — the same numbers the learner's files
will have after the chapter's edits land. The margin is the only annotation
added; the code text itself is quoted exactly, and each fragment's
provenance note states whether it is byte-identical to the completed
reference (with the reference's line range) or a chapter-3 slice form (with
the exact delta). Blank separators between lines appear without a margin; no
fragment skips a nonblank line inside its quoted range.

File links in this paper point into the learner's tree on `main`. Links to
files the track creates — for example
[`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs) —
resolve once chapters 01–03 land. The reference worktree is not linked: it
is temporary provenance, deleted once the tutorial is complete on `main`,
and everything needed from it is quoted here. The scratch copies and their
logs (`/tmp/selfplay-ch2-verify`, `/tmp/selfplay-ch3-verify`,
`/tmp/selfplay-ch3-verify-logs/`) are cited as evidence of this paper's own
verification; they are ephemeral and not part of the repository.

## 1. `src/record.rs`: the format width

The chapter's one new constant lands first, because everything downstream
names it: the conversion truncates against it, the tests import it, and the
durable field's documentation links it. `RECORDED_PRIOR_TOP_K` lives in the
record module beside `GAME_RECORD_FORMAT_VERSION`, not beside the in-memory
slot, because it is a *format width*: it bounds what any reader of the
durable format must expect. The doc comment carries two sentences of
honesty, exactly as the implementation plan prescribes: this sparse audit
width is a selfplay-corpora format invention ([design
spec](../../../specs/2026-11-11-selfplay-corpora-design.md) §3.2), not an
AlphaZero or architecture-document constant — an **[experiment]** under the
claim labels of
[docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) — and
the complete prior remains recomputable from the position and the
deterministic baseline evaluator, so the omission of the tail is safe.

**Source:** [`gomoku/crates/selfplay/src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs),
lines 7–15 after the edit. *Created by chapter 02; this chapter inserts
lines 10–15.*

```rust
   7 | /// Current durable game-record layout version.
   8 | pub const GAME_RECORD_FORMAT_VERSION: u16 = 1;

  10 | /// Maximum number of root priors retained per searched ply.
  11 | ///
  12 | /// This sparse audit width is a selfplay-corpora format invention (§3.2), not
  13 | /// an AlphaZero or architecture-document constant. The complete prior remains
  14 | /// recomputable from the position and evaluator.
  15 | pub const RECORDED_PRIOR_TOP_K: usize = 16;
```

*Provenance: byte-identical to the completed reference's `src/record.rs`
lines 7–15. The completed reference's file differs from the chapter-3 slice
in exactly one further region — lines 77–78, the `SeatConfig.temperature`
doc comment, which the reference words in chapter 04's vocabulary ("the
pairing's exploration schedule overrides this on every ply") while the slice
keeps chapter 02's wording ("chapter 4's exploration schedule adjusts it per
ply"). That region is outside every fragment this paper quotes; the identity
of the rest of the file was checked programmatically (slice lines 1–76 and
79–215 against the same reference lines).*

## 2. `src/lib.rs`: the crate-root re-export

The constant joins the existing `record` re-export group in alphabetical
position — after `GuardSettings`, before `RecordedPly` — and this is the
chapter's only new public item: no new types, no new functions. The
re-export lands before the tests because they import the constant from the
crate root; the `record` module itself stays private, and no test reaches
for a path deeper than the crate root.

**Source:** [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs),
lines 23–26 after the edit. *Created by chapter 01, extended by chapter 02;
this chapter inserts the constant's name in line 25.*

```rust
  23 | pub use record::{
  24 |     CertifiedSuffix, GAME_RECORD_FORMAT_VERSION, GameHeader, GameRecord, GuardSettings,
  25 |     RECORDED_PRIOR_TOP_K, RecordedPly, SeatConfig, SeatPairing, TssProbeSettings,
  26 | };
```

*Provenance: byte-identical to the completed reference's `src/lib.rs` lines
36–39 — the record re-export group is finished with this chapter; no later
chapter extends it. The completed reference's `lib.rs` additionally declares
and re-exports the later chapters' modules (`anchor`, `export`, `generate`,
`mix`, `sample`, `sampler`, and chapter 04's opening-list names), which the
chapter-3 slice does not have.*

## 3. `tests/game.rs`: the two conversion tests (red)

The specification lands ahead of the implementation — test-driven
development (TDD): written first, watched failing, then made green by
section 4. The red state here is a *compile* error: `PlySearch::from` does
not exist yet, and that missing conversion is exactly what the tests
specify. Both tests sit at the top of the file, ahead of chapter 01's
ceremony test — the reference file's lexical order leads with this
chapter's pair — and neither plays a game: each builds its `SearchStats`
input by hand, so each is a test of the conversion, not of the search, and
both finish in well under a second.

### 3.1 The imports

The `player` import gains `SearchStats`, the conversion's input type. The
crate-root `selfplay` import gains `PlySearch` — the slot type the tests
construct through the conversion — and `RECORDED_PRIOR_TOP_K`, the format
width of section 1. Everything else in the import block is chapter 01's,
unchanged.

**Source:** [`gomoku/crates/selfplay/tests/game.rs`](../../../../gomoku/crates/selfplay/tests/game.rs),
lines 1–9 after the edit. *Created by chapter 01; this chapter extends lines
4 and 7–8.*

```rust
   1 | use std::time::Instant;

   3 | use engine::{Color, Move, Status};
   4 | use player::{PlayerOptions, SearchStats};
   5 | use rand::rngs::StdRng;
   6 | use selfplay::{
   7 |     Opening, OpeningAnswer, OpeningProvenance, OpeningSource, Outcome, PlySearch,
   8 |     RECORDED_PRIOR_TOP_K, WinReason, play_game, play_game_with_opening,
   9 | };
```

*Provenance: byte-identical to the completed reference's `tests/game.rs`
lines 1–9 — the completed reference carries this chapter's test file
unchanged, in full (whole-file scripted comparison).*

### 3.2 The sign convention

The first test pins the copy rule in isolation. Two hand-built
`SearchStats` values, both with empty visit and prior lists: one with side
to move Black and value 0.875, one with side to move White and value
−0.625. Both values are exact binary fractions (7/8 and −5/8), so the
assertions carry no floating-point tolerance. The empty lists are
deliberate: they isolate the copy rule, and they incidentally pin that
empty input lists convert to empty slot lists — no fabricated entries. The
conversion never negates, never re-anchors a value to a fixed color, and
never clamps: Black keeps 0.875, White keeps −0.625, and each slot keeps
its input's side to move. A future "helpful normalization" fails loudly
here instead of silently corrupting a corpus.

**Source:** `gomoku/crates/selfplay/tests/game.rs`, lines 30–49.

```rust
  30 | #[test]
  31 | fn ply_search_preserves_the_side_to_move_root_value_sign() {
  32 |     let black = PlySearch::from(SearchStats {
  33 |         side_to_move: Color::Black,
  34 |         visits: Vec::new(),
  35 |         prior: Vec::new(),
  36 |         value: 0.875,
  37 |     });
  38 |     let white = PlySearch::from(SearchStats {
  39 |         side_to_move: Color::White,
  40 |         visits: Vec::new(),
  41 |         prior: Vec::new(),
  42 |         value: -0.625,
  43 |     });

  45 |     assert_eq!(black.side_to_move, Color::Black);
  46 |     assert_eq!(black.root_value, 0.875);
  47 |     assert_eq!(white.side_to_move, Color::White);
  48 |     assert_eq!(white.root_value, -0.625);
  49 | }
```

*Provenance: byte-identical to the completed reference's `tests/game.rs`
lines 30–49.*

### 3.3 The truncation rule

The second test pins the chapter's one new behavior. The fixture is a prior
of twenty entries on twenty distinct cells — the first twenty cells in
row-major order, built with `mv` — chapter 01's `mv(row, col)` test helper,
which wraps `Move::new` with an on-board `expect` — through the
range-map-collect idiom — with strictly decreasing values (1.0, 0.99, 0.98, …), plus a single
visit entry; the input prior is cloned before the conversion consumes the
statistics. Two assertions follow. First, `RECORDED_PRIOR_TOP_K` equals 16,
so a silent change of the format width fails loudly. Second, the slot's
prior equals the leading slice of the input prior of length
`RECORDED_PRIOR_TOP_K` — a slice-prefix equality that pins three properties
at once: the cut is a prefix (a truncation, not a selection by some other
criterion), the order is the input's order (the strictly decreasing values
make any re-sorting visible), and the values are unchanged (no
renormalization). The twenty-entry input makes the cut visible: twenty in,
sixteen out, the tail gone.

**Source:** `gomoku/crates/selfplay/tests/game.rs`, lines 51–65.

```rust
  51 | #[test]
  52 | fn ply_search_truncates_prior_to_top_16_without_reordering() {
  53 |     let prior: Vec<_> = (0..20)
  54 |         .map(|index| (mv(index / 15, index % 15), 1.0 - f32::from(index) * 0.01))
  55 |         .collect();
  56 |     let search = PlySearch::from(SearchStats {
  57 |         side_to_move: Color::Black,
  58 |         visits: vec![(mv(0, 0), 1)],
  59 |         prior: prior.clone(),
  60 |         value: 0.0,
  61 |     });

  63 |     assert_eq!(RECORDED_PRIOR_TOP_K, 16);
  64 |     assert_eq!(search.prior, prior[..RECORDED_PRIOR_TOP_K]);
  65 | }
```

*Provenance: byte-identical to the completed reference's `tests/game.rs`
lines 51–65.*

Watch both tests fail to compile: `PlySearch::from` does not exist. That is
the red state, and it is the correct red state — the tests specify an
implementation, not an accident of an existing one.

## 4. `src/game.rs`: the `From` implementation (green)

### 4.1 The imports

Two names make the conversion expressible. `SearchStats` joins the existing
`player` import at the top of the file — the import is extended, not
duplicated. And the constant comes from the record module,
`crate::record::RECORDED_PRIOR_TOP_K` — from the module, not through the
crate-root re-export of section 2, matching the completed reference's
discipline of importing from the defining module inside the crate.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 1–9 after the
edit. *Created by chapter 01, touched by chapter 02; this chapter extends
line 3 and lines 7–9.*

```rust
   1 | use baseline::{BaselineEvaluator, Device};
   2 | use engine::{Color, Move, PlayError, Status};
   3 | use player::{AiPlayer, DecisionAction, DecisionError, PlayerOptions, ResignReason, SearchStats};
   4 | use rand::{SeedableRng, rngs::StdRng};
   5 | use thiserror::Error;

   7 | use crate::{
   8 |     CertifiedSuffix, GeneratedOpening, Opening, OpeningSource, record::RECORDED_PRIOR_TOP_K,
   9 | };
```

*Provenance: chapter-3 slice form. Line 3 is byte-identical to the completed
reference's line 3. The completed reference's import block differs in
exactly two names, both later chapters': its line 2 names `engine::Proof`
(chapter 05's suffix tracking), and its crate block names
`ExplorationSchedule` (chapter 04's schedule application), which shifts its
crate block to four lines. The slice's lines 1–2 and 4–6 are chapter 01's
own, unchanged.*

### 4.2 The conversion

One item, placed directly after the `PlySearch` struct definition:
`impl From<SearchStats> for PlySearch`. The method consumes the statistics
by value and returns the slot; the conversion is total — every input
converts, nothing is validated, nothing fails. Field by field, in struct
declaration order:

- `side_to_move`: copied verbatim. No negation, no re-anchoring, no special
  cases — the first test pins this.
- `visits`: only the positive-count entries survive, in the input's order —
  the player's extraction order, descending by count with ties by move
  index. The consuming-iterator idiom with a filter on the count and a
  collect. The counts stay raw (π, the normalized root visit distribution,
  is a read-time derivation in chapter 06) and unsorted (the player already
  ordered the list). An empty input list converts to an empty slot list.
- `prior`: only the first `RECORDED_PRIOR_TOP_K` entries survive, in the
  input's order, values unchanged — the consuming-iterator idiom with
  `take` on the constant and a collect. The player already ordered the
  priors descending by value, so taking the first sixteen *is* taking the
  top sixteen; the stored values are the probabilities as mixed and sum to
  at most one — no renormalization, because the recorded prior is an audit
  record, not a distribution to sample from. The second test pins all of
  this.
- `root_value`: copied verbatim from the statistics' value — the deciding
  side's estimate of its own position, and it stays that way in the record.

Note what the implementation does not contain: any handling of absent
statistics. The `Option` around `Decision.search` is resolved by the
caller's `Option::map` in section 5, and the durable shape of an absent
slot — empty visits, an empty prior, `root_value: None` — belongs to
chapter 02's `GameRecord::from_game`, which this chapter does not touch.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 26–39 after the
edit (to the end of the new implementation). *The struct above it is chapter
01's, with this chapter's doc update quoted in section 6.1.*

```rust
  26 | impl From<SearchStats> for PlySearch {
  27 |     fn from(stats: SearchStats) -> Self {
  28 |         Self {
  29 |             side_to_move: stats.side_to_move,
  30 |             visits: stats
  31 |                 .visits
  32 |                 .into_iter()
  33 |                 .filter(|(_, count)| *count > 0)
  34 |                 .collect(),
  35 |             prior: stats.prior.into_iter().take(RECORDED_PRIOR_TOP_K).collect(),
  36 |             root_value: stats.value,
  37 |         }
  38 |     }
  39 | }
```

*Provenance: byte-identical to the completed reference's `src/game.rs`
lines 27–40.*

Why the conversion may land before its call site: until section 5, the new
implementation is exercised only by the two new tests — chapter 01's inline
block is still in the loop and still produces the same slots, so the whole
default suite stays green. That is the correct intermediate state, not dead
code.

## 5. The refactor: one `map` in the move loop

Now the runner routes through the named conversion. In the move loop's
`Play` arm, chapter 01's inline conversion — the closure over
`decision.search` that built the slot field by field, together with the
comment block above it that announced this chapter — is deleted, and what
replaces it is the one-call form: the `Option::map` idiom applied to
`decision.search` with `PlySearch::from`. The `map` shape was already
right; only the closure became a named conversion. Everything else in the
loop — the status-first structure, the seat selection by absolute color,
the legality gate, the resignation arm — stays byte-identical.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 178–188 after the
edit (the decision and the `Play` arm). *Created by chapter 01; this
chapter replaces what stood between the `Play(mv)` pattern and the
legality gate.*

```rust
 178 |         let decision = seat.decide(&board, options, &mut rng)?;
 179 |         match decision.action {
 180 |             DecisionAction::Play(mv) => {
 181 |                 let search = decision.search.map(PlySearch::from);
 182 |                 board.play(mv).map_err(|source| GameError::Play {
 183 |                     ply: plies.len(),
 184 |                     mv,
 185 |                     source,
 186 |                 })?;
 187 |                 plies.push(PlyRecord { mv, search });
 188 |             }
```

*Provenance: chapter-3 slice form. The map line itself — slice line 181 —
is byte-identical to the completed reference's line 231, and the `Play`
arm's remaining lines are likewise identical (slice lines 179–180 and
182–188 against the reference's lines 228–229 and 232–236, 238–239,
checked programmatically). The completed reference's surrounding region
carries exactly three later-chapter additions the slice does not have:
chapter 04's `let effective_options = schedule.options_for_ply(*options,
plies.len());` between the seat selection and the decision (the slice
passes `options` directly), and chapter 05's `suffix.begin(plies.len(),
decision.own_proof);` before the map line and `suffix.observe(mv);` after
the legality gate. Neither is quoted here.*

One behavior actually changes, and it is the chapter's one new rule: the
recorded prior is now truncated to its leading `RECORDED_PRIOR_TOP_K`
entries. No existing test sees the difference — no chapter-01 or chapter-02
assertion inspects the prior's length — while section 3.3's truncation test
pins the new rule directly. This step must not change a single other test
outcome; if it does, the refactor drifted — fix the refactor, never a test.

## 6. The two doc comments

Two slice wordings that chapters 01 and 02 documented — both reading "all
root priors" — become the reference wordings. These are documentation
changes only: no type, field, or mapping changes shape, and the format
width bounds a field that already exists.

### 6.1 `PlySearch.prior`: "top root priors"

Chapter 01's field doc read "All root priors after optional Dirichlet
noise, in descending order." — true of chapter 01's untruncated
passthrough, false since section 5. The update changes exactly one word to
"Top root priors", keeping the rest of the sentence unchanged; chapter 01's
solution paper recorded this exact wording as the reference's, so the
update also retires that slice delta. The struct is quoted in full so the
field sits in its context: the side to move beside the value, the raw-count
honesty note on the visits.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 11–24 after the
edit.

```rust
  11 | /// Search information retained for one played move.
  12 | #[derive(Debug, Clone, PartialEq)]
  13 | pub struct PlySearch {
  14 |     /// Side whose perspective determines `root_value`'s sign.
  15 |     pub side_to_move: Color,
  16 |     /// Raw visit counts for every visited root child, in descending count order.
  17 |     ///
  18 |     /// Counts are deliberately not normalized; the chapter-6 reader derives π.
  19 |     pub visits: Vec<(Move, u32)>,
  20 |     /// Top root priors after optional Dirichlet noise, in descending order.
  21 |     pub prior: Vec<(Move, f32)>,
  22 |     /// The searched root's value estimate from `side_to_move`'s perspective.
  23 |     pub root_value: f32,
  24 | }
```

*Provenance: byte-identical to the completed reference's `src/game.rs`
lines 12–25.*

### 6.2 `RecordedPly.prior`: naming the bound

Same slice wording, one layer down: the durable field doc read "All root
priors after optional Dirichlet noise, in descending order; empty when
search did not run." The update names the bound — "Up to
[`RECORDED_PRIOR_TOP_K`] root priors after optional Dirichlet noise, in
descending order; empty when search did not run." — written as an intra-doc
link, which resolves because the constant lives in the same module. The
neighboring fields' docs are chapter 02's, unchanged: empty means no search
ran, and a resignation produces no ply and no fabricated value.

**Source:** `gomoku/crates/selfplay/src/record.rs`, lines 135–145 after the
edit (the `RecordedPly` struct through its `prior` field and the following
doc line). *Created by chapter 02; this chapter updates lines 141–142.*

```rust
 135 | pub struct RecordedPly {
 136 |     /// Move played at this ply.
 137 |     pub mv: Move,
 138 |     /// Sparse raw root visit counts for visited children, in descending order.
 139 |     /// Empty when search did not run. Normalization into π happens at read time.
 140 |     pub visits: Vec<(Move, u32)>,
 141 |     /// Up to [`RECORDED_PRIOR_TOP_K`] root priors after optional Dirichlet
 142 |     /// noise, in descending order; empty when search did not run.
 143 |     pub prior: Vec<(Move, f32)>,
 144 |     /// Root estimate from the recording side-to-move's perspective, or `None`
 145 |     /// when a certified own-side proof bypassed search. A resignation ends the
```

*Provenance: byte-identical to the completed reference's `src/record.rs`
lines 135–145. The fragment stops mid-sentence at the start of the
`root_value` documentation by deliberate elision — the remaining
`root_value` doc is chapter 02's wording, unchanged. This retires the
slice delta chapter 02's solution paper recorded on the same field.*

`RecordedPly` itself, `GameRecord::from_game`, `GAME_RECORD_FORMAT_VERSION`,
the corpus writer, and the readers are untouched — the record side copies
the slot as-is, and the guarantee this chapter added is pinned by tests on
the conversion itself, so chapter 02's persistence tests pass without
knowing the conversion exists.

## 7. Verification

The chapter's commands, run from `gomoku/` — exactly the set the approved
papers name:

```text
cargo test -p selfplay --test game ply_search
cargo test -p selfplay
cargo test -p selfplay --test corpus
cargo test -p selfplay --release --test game -- --ignored
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the chapter's capstone: the shared `ply_search` prefix
selects exactly the two conversion tests — no other test in the game target
carries that substring — and the run finishes in well under a second
because both fixtures are hand-built and no game is played. The second
command is the fast gate; cargo prints one report per test target, never
one aggregate line, and the chapter's slice is the sum of the two
per-target reports. The third command re-runs chapter 02's target alone:
the record side of the seam must be provably untouched, and
`populated_search_fields_round_trip_byte_exactly` is the proof that the
slot-to-record mapping still persists populated search fields byte-exactly
— its fixtures build `PlySearch` by hand, so it does not exercise the new
conversion; it pins the other side of the seam. The fourth command is
chapter 01's MANUAL pair, and both qualifiers matter: `--release` is not
optional for this focused run, and the pair is `#[ignore]`d for two
distinct reasons — the budget-8 smoke test's MANUAL comment warns that its
game runs for minutes in a debug build, while the budget-2-versus-1
visit-sum test stays out of the default run so the default suite's games
stay at budget 1. The last two commands are the project's standing quality
gates. `cargo test -p train` and `cargo test -p player` are not required:
this chapter touches no crate but `selfplay`.

The release-mode MANUAL run is *relevant*, not ceremonial, because section
5 rewired the runner: the two conversion properties this chapter
deliberately does not unit-test — the positive-count filter and the visit
sum on real games — are pinned only by
`tiny_budget_game_records_raw_visits_for_every_searched_ply`, and after the
refactor every real game's statistics flow through the named conversion
rather than the inline block. That MANUAL test now asserts, through
`PlySearch::from`, that every searched ply of a budget-2-versus-1 game
carries a non-empty visit list of positive counts summing to the deciding
seat's budget; its companion
`tiny_budget_game_completes_and_records_search_metadata` pins that every
recorded root value is finite.

Evidence actually measured for this paper (2026-11-13, on the chapter-3
slice constructed in the scratch copy described under Provenance — not a
re-run of the completed reference, whose gate evidence the
[hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md)
records; logs at `/tmp/selfplay-ch3-verify-logs/`, scratch-local and
ephemeral):

| Command | Result on the chapter-3 slice |
|---------|-------------------------------|
| `cargo test -p selfplay --test game ply_search` | 2 passed, 0 failed, 0 ignored, 5 filtered out (0.00 s of test time, after a 14.76 s debug build of the dependency chain) — the capstone |
| `cargo test -p selfplay` | lib target: 0 tests; corpus target: 10 passed, 0 failed, 0 ignored (0.02 s); game target: 5 passed, 0 failed, 2 ignored (12.48 s, debug build) — **15 passed, 2 ignored** in total |
| `cargo test -p selfplay --test corpus` | 10 passed, 0 failed, 0 ignored (0.01 s) — chapter 02's target provably untouched |
| `cargo test -p selfplay --release --test game -- --ignored` | 2 passed, 0 failed, 5 filtered out (0.15 s) — both MANUAL tests green through the named conversion |
| `cargo clippy --all-targets -- -D warnings` | clean, workspace-wide |
| `cargo fmt --all -- --check` | no changes |

Two count notes keep the evidence precise, and they double as the
distinction between this paper's slice evidence and the completed
reference's hand-off evidence. First, the game target's 5 passed, 2 ignored
and the corpus target's 10 passed are also the completed reference's exact
counts for both targets — the reference's `tests/game.rs` is byte-identical
to the slice's (scripted comparison), and the corpus target this chapter
does not touch was already byte-identical in chapter 02. The reference's
larger full suite — 50 passed, 4 ignored — is entirely later chapters'
targets plus chapter 05's two lib unit tests. Second, the corpus-focused
log shows three `test result` lines for one target: the cross-process
determinism test re-executes the test binary twice as a child process, and
the children's one-test reports appear interleaved in the parent log
because the children inherit its stdio — chapter 02's mechanism, not this
chapter's, and unchanged.

## References

**This chapter:**

- [recording-the-search.md](recording-the-search.md) — the chapter paper:
  intention, specification as tests, execution-order walkthrough.
- [recording-the-search-design.md](recording-the-search-design.md) — the
  conceptual story: record-versus-derive, the sign convention, sparsity as
  honesty, the empty-versus-zero distinction.
- [recording-the-search-implementation.md](recording-the-search-implementation.md) —
  the hiding layer this paper quotes the construction contracts of; its
  steps 1–4 fix the chapter-3 slice forms.

**This track:**

- [Tutorial README](../README.md) — conventions, gates, the chapter map,
  and the reference-provenance rule this paper's Provenance section
  applies.
- [Self-play primer](../00-selfplay-primer.md) — §3.2 pins the sparse
  storage decisions; §8 pins invariants 5 (π normalized at read time), 6
  (z derived at read time), and 7 (honest search records).
- [Chapter 01 — the game runner](../01-the-game-runner/the-game-runner.md)
  and its [solution](../01-the-game-runner/the-game-runner-solution.md) —
  the `PlySearch` slot, the inline conversion this chapter replaces, the
  "all root priors" slice wording, and the MANUAL positivity and visit-sum
  contract the release-mode regression re-pins through the named
  conversion.
- [Chapter 02 — recording games](../02-recording-games/recording-games.md)
  and its [solution](../02-recording-games/recording-games-solution.md) —
  `RecordedPly`, the as-is persistence of the slot, and the second "all
  root priors" slice wording.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  §3.2: the record format, the sparse top-k prior with k = 16 as
  **[experiment]**, and the audit purpose of the prior.
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  the reference-implementation layout and the measured evidence of the
  completed tree.

**Prerequisite tracks:**

- [Baseline tutorial](../../baseline-tutorial/README.md) —
  `AiPlayer::decide`, `Decision.search`, `SearchStats`, and the honesty
  rule for certified-proof moves.
- [MCTS tutorial](../../mcts-tutorial/README.md) — the search whose root
  visit distribution and edge priors `SearchStats` extracts.
- [Engine tutorial](../../13-engine-tutorial/README.md) — the TSS prover
  and `verify_line` behind the certified-proof bypass.
- [docs/12-gomoku-architecture.md](../../../12-gomoku-architecture.md) —
  defines the **[paper]** / **[derived]** / **[experiment]** claim-label
  convention used for k = 16.

**Source (live on `main`):**

- [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs) —
  `AiPlayer`, `Decision`, `SearchStats` — the conversion's input and the
  honesty rule. Not touched in this chapter.
- [`gomoku/crates/mcts/src/policy.rs`](../../../../gomoku/crates/mcts/src/policy.rs) —
  `visit_distribution`, `add_dirichlet_noise`, `select_move_with_priors` —
  the origin of the orderings the record keeps. Not touched in this
  chapter.

**Source (created by chapters 01 and 02, touched by this chapter; the links
resolve once the chapters land on `main`):**

- [`gomoku/crates/selfplay/src/record.rs`](../../../../gomoku/crates/selfplay/src/record.rs) —
  gains `RECORDED_PRIOR_TOP_K` (section 1); the `RecordedPly.prior` doc
  comment names the bound (section 6.2).
- [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs) —
  re-exports the constant (section 2).
- [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs) —
  gains `impl From<SearchStats> for PlySearch` (section 4); the move loop's
  inline conversion becomes one `Option::map` call (section 5); the
  `PlySearch.prior` doc comment updates (section 6.1).
- [`gomoku/crates/selfplay/tests/game.rs`](../../../../gomoku/crates/selfplay/tests/game.rs) —
  the chapter's specification: the two conversion tests (section 3).

**Source (unchanged in this chapter):**

- [`gomoku/crates/selfplay/tests/corpus.rs`](../../../../gomoku/crates/selfplay/tests/corpus.rs) —
  chapter 02's specification; regression evidence for the record side of
  the seam.

**Reference provenance:** the completed reference implementation lives in
the read-only sibling worktree `rust-ml-selfplay-ref` (branch
`selfplay-ref-v2`, base `9028aa8`, changes uncommitted); it is quoted and
compared against, never linked, and it is deleted once the tutorial is
complete on `main`. The chapter-3 slice in this paper was constructed per
the implementation plan in an untouched copy of the verified chapter-2
scratch (`/tmp/selfplay-ch2-verify` → `/tmp/selfplay-ch3-verify`) and
gate-verified there on 2026-11-13; the reference worktree, the chapter-2
scratch, and chapter 01's scratch copy (`/tmp/selfplay-ch1-verify`) were
not modified.

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). <https://doi.org/10.1038/nature24270> — the
  visit-distribution policy target π whose raw counts the record keeps,
  and the root Dirichlet noise the recorded prior reflects.
- Rosin, C. D. (2011). *Multi-armed bandits with episode context* (PUCT).
  <https://doi.org/10.1007/s10472-011-9258-6> — the selection rule whose P
  is the recorded prior.
- Allis, L. V. (1994). *Searching for Solutions in Games and Artificial
  Intelligence*. PhD thesis, Maastricht University.
  <https://doi.org/10.26481/dis.19940923la> — threat-space search, the
  source of the certified moves that bypass the record.
