> **Opt-in solution — read it only after attempting the chapter with the
> [implementation plan](the-game-runner-implementation.md), or after being
> stuck for more than twenty minutes.**
>
> This paper is inspiration, not authority over the learner's own code. The
> learner's implementation on `main` is the reference; the artifacts below
> exist so a stuck or finished learner can compare. No learner source is
> changed by this paper.

# Chapter 01 — The game runner: solution

## Abstract

This paper is the annotated reference companion to
[chapter 01 of the selfplay-corpora tutorial](the-game-runner.md). It quotes
everything the chapter creates, along the execution path of one game: the
workspace registration and the crate manifest, the chapter's one deliberate
touch to the `player` crate (serde serialization for `ResignReason`), the
crate skeleton, the opening vocabulary (`OpeningProvenance`, `OpeningAnswer`,
`Opening`), the Swap2 ceremony through the engine's typestate
(`Opening::board()`), the `OpeningSource` seam and `GeneratedOpening`, the
outcome and ply vocabulary (`WinReason`, `Outcome`, the chapter-1 `PlySearch`
slot, `PlyRecord`, `Game`), `GameError`, the runner (`play_game_with_opening`
and `play_game`), and the five tests of the chapter's specification. Every
snippet carries an explanation, source line numbers, and an exact provenance
note: most fragments are byte-identical to the completed reference
implementation, and the few that are not are the chapter-1 incremental forms
the approved papers specify — each such difference is named explicitly. All
chapter gates ran green on the chapter-1 slice on 2026-11-12; the precise
evidence is in the Provenance and Verification sections. The intended reader
is the learner who has read the chapter paper and, ideally, worked the
[implementation plan](the-game-runner-implementation.md) first.

## Glossary

| Term | Definition |
|------|------------|
| **Seat** | One of the two players of a generated game: a persistent `player::AiPlayer` running the baseline evaluator at a declared simulation budget. Every game pairs two seats. |
| **Simulation budget** | The number of Monte Carlo Tree Search (MCTS) simulations a seat runs per move decision; written baseline-X for a budget of X. |
| **Swap2** | The project's opening protocol: one side places three stones, the other side chooses a color or places two more stones, and the first side then chooses a color. |
| **Typestate** | A Rust pattern in which the phase of a protocol is a type parameter. The engine's `Swap2<State>` machine makes illegal opening sequences unrepresentable. |
| **Absolute colors** | The engine's storage convention: `Board` holds Black and White bitboards as-is, never a relative me/you view. |
| **Ply** | One move played after the opening. Ply indices in this chapter are zero-based and count post-opening moves only. |
| **Provenance** | The recorded origin of an opening: generated from the game's random stream, or drawn from an external list. |
| **MANUAL test** | A test marked `#[ignore]` with a MANUAL comment because it plays slow games. It runs only on explicit selection, in release mode. |
| **Certified forced win** | A forced-win line produced by the engine's threat-space search (TSS) prover and checked by `engine::verify_line`. The prover may be incomplete, never unsound. |
| **Dependency island** | The project's crate rule for `engine`: no deep-learning framework, no randomness, and no input/output inside it; other crates depend on `engine`, never the reverse. |
| **π (visit-distribution target)** | The normalized root visit-count distribution of the search at a ply; the policy training target (Silver et al. 2017). Chapter 03 owns it; this chapter only retains the raw counts. |
| **Chapter-1 slice** | The chapter-1 subset of the completed reference: same vocabulary and runner, without the later chapters' additions. Defined and verified in the Provenance section. |

## Provenance, the chapter-1 slice, and verification

### The reference and its completed-tree form

The reference implementation lives in a separate, read-only worktree beside
this repository (`rust-ml-selfplay-ref`, branch `selfplay-ref-v2`, committed
base `9028aa8` on `main`, all reference changes uncommitted). It was built
test-first and gate-verified before this tutorial was written; the
[hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md)
records its measured evidence (workspace gates green; `cargo test -p
selfplay` reporting 50 passed, 4 ignored across all test targets). The
worktree serves quotation only: never point a manifest at it, never build the
learner's crate from it, never commit there.

One structural fact shapes everything below: the worktree holds the
**completed** crate — all seven chapters at once. Its
`gomoku/crates/selfplay/src/game.rs` already contains chapter 02's
certified-suffix field, chapter 03's `From<SearchStats>` conversion with the
recorded top-k prior, chapter 04's exploration schedule, and chapter 05's
suffix tracking. Quoting those regions verbatim would leak later chapters and
would contradict this chapter's approved papers, which fix the chapter-1
form: six `Game` fields with no suffix, no exploration schedule, and an
inline slot conversion with an untruncated prior (see
[the implementation plan](the-game-runner-implementation.md), steps 7 and
10). This paper therefore distinguishes two kinds of quotation:

- **Reference-verbatim fragments.** Most of the chapter-1 code is stable:
  the completed reference carries it unchanged. These fragments are quoted
  exactly, and each carries a note naming the reference file and line range
  it was compared against. The comparisons were scripted, not eyeballed;
  every range reported as byte-identical was checked programmatically
  (trailing newline structure included).
- **Chapter-1 slice forms.** Where the completed reference has already
  grown past chapter 01, this paper quotes the chapter-1 form specified by
  the approved papers, and names exactly what the completed reference adds
  later. No later-chapter implementation is quoted.

### The slice was built and run

Quoting a slice the learner could not compile would violate the track's
first rule — a tutorial that does not run error-free is a defect. So the
chapter-1 slice was constructed and gate-verified on 2026-11-12 in a
scratch copy of the reference worktree (the reference itself was not
touched): the `player` and `train` crates were restored to their `main`
state, the chapter's serde touch to `ResignReason` was re-applied, the
`selfplay` crate was reduced to the chapter-1 form below (manifest, three
source files, one test file), and the chapter's verification commands were
run from the copy's `gomoku/`:

```text
cargo test -p selfplay
cargo test -p player
cargo test -p selfplay --release --test game -- --ignored
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Observed results on the slice. The first command reported **3 passed, 2
ignored** for the game target (12.46 s in a debug build — the budget-1
default tests are fast). The second reported 43 passed across the player's
three test targets, unchanged by the serde derive. The third ran both MANUAL
tests in release mode: **2 passed** in 0.15 s, with the smoke test printing
`smoke game: 10 plies, Won { winner: Black, reason: FiveOrMore },
153.245ms`. Clippy and rustfmt reported no findings and no changes. The
slice fragments quoted below are exactly the files those runs exercised.

One count distinction the chapter paper also makes: the completed
reference's `tests/game.rs` holds seven tests — the same target reports **5
passed, 2 ignored** there — because two tests
(`ply_search_preserves_the_side_to_move_root_value_sign` and
`ply_search_truncates_prior_to_top_16_without_reordering`) belong to chapter
03's `PlySearch`-conversion specification. The chapter-1 slice writes
exactly five tests, and this paper quotes all five bodies in full.

### Reading rules

The section order is the execution order of one game, preceded by the
build preparation (workspace, manifests, the `player` touch) and followed
by the specification tests and the verification commands.

Every code fragment carries a right-aligned `NN |` line-number margin. For
fragments from files this chapter creates, the numbers are the chapter-1
file's own line numbers — the same numbers the learner's new files will
have. For the three pre-existing files the chapter edits (the workspace
manifest plus the two `player` files), the numbers are the edited file's
line numbers. The margin is the only annotation added; the code text itself is
quoted exactly, and each fragment's provenance note states whether it is
byte-identical to the completed reference (with the reference's line range)
or a chapter-1 slice form (with the exact delta). Blank separators between
fragments may be restored freely; no fragment skips a nonblank line inside
its quoted range.

File links in this paper point into the learner's tree on `main`. Links to
the `selfplay` crate's files — for example
[`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs) —
name files this chapter creates; they resolve once the chapter lands. The
reference worktree is not linked: it is temporary provenance, deleted once
the tutorial is complete on `main`, and everything needed from it is quoted
here.

## 1. Workspace and manifests

Before any game can run, the workspace must know the crate, the crate must
declare its dependencies, and the `player` crate must make `ResignReason`
serializable — because section 6's `WinReason::Resignation` embeds it and
`WinReason` derives serde serialization in this chapter.

### 1.1 Register the workspace member

The first edit is one word in the
[workspace manifest](../../../../gomoku/Cargo.toml): `"crates/selfplay"`
joins the member list after `"crates/net"`. Without it, no package can
depend on the new crate. This is the resulting state:

**Source:** [`gomoku/Cargo.toml`](../../../../gomoku/Cargo.toml), lines 4–7
after the edit.

```text
  4 | [workspace]
  5 | resolver = "3"
  6 | members = ["crates/engine", "crates/cli", "crates/mcts", "crates/train", "crates/patterns",
  7 |     "crates/testkit", "crates/baseline", "crates/player", "crates/net", "crates/selfplay"]
```

*Provenance: byte-identical to the reference worktree's
`gomoku/Cargo.toml` lines 4–7. On `main` before this chapter, line 7 ends
at `"crates/net"]`; the edit appends `, "crates/selfplay"` inside the
brackets. Cargo records the new local package in `gomoku/Cargo.lock` on
first resolution; the lockfile is generated, not quoted.*

### 1.2 The crate manifest

The new crate's manifest declares the chapter's exact dependency footprint:
the three path dependencies it assembles (`engine` for the rules and the
ceremony, `baseline` for the evaluator and the `Device` re-export, `player`
for the seats), `rand` for the single seeded stream (pinned to the same
`0.10.3` the `player` and `baseline` crates use, so the workspace agrees on
one random-number API), and the workspace `serde` and `thiserror` for the
vocabulary derives and the error type. Two absences are deliberate:
`serde_json` and the `train` path dependency wait for chapter 02, where the
corpus writer first uses them, and Burn never appears — `Device::flex()`
reaches the runner through `baseline`'s re-export, keeping `engine` a
dependency island.

**Source:** `gomoku/crates/selfplay/Cargo.toml`, lines 1–14 (the complete
file). *Not yet present on `main`; the chapter creates it.*

```text
  1 | [package]
  2 | name = "selfplay"
  3 | description = "Synchronous baseline self-play game runner."
  4 | version.workspace = true
  5 | edition.workspace = true
  6 | license.workspace = true
  7 |
  8 | [dependencies]
  9 | baseline = { path = "../baseline" }
 10 | engine = { path = "../engine" }
 11 | player = { path = "../player" }
 12 | rand = "0.10.3"
 13 | serde = { workspace = true }
 14 | thiserror = { workspace = true }
```

*Provenance: chapter-1 slice form. The completed reference's manifest adds
two lines — `serde_json = { workspace = true }` and
`train = { path = "../train" }` — which belong to chapter 02's corpus
writer. Everything else is identical.*

### 1.3 The `player` touch: serde for `ResignReason`

This is the chapter's only cross-crate change, and it exists for one
reason: `WinReason::Resignation` embeds `player::ResignReason`, and
`WinReason` derives serde serialization — so the embedded type must
serialize too. The manifest edit adds the workspace `serde` to
[`gomoku/crates/player/Cargo.toml`](../../../../gomoku/crates/player/Cargo.toml):

**Source:** `gomoku/crates/player/Cargo.toml`, lines 7–11 after the edit.

```text
  7 | [dependencies]
  8 | engine = { path = "../engine" }
  9 | mcts = { path = "../mcts" }
 10 | rand = "0.10.3"
 11 | serde = { workspace = true }
```

*Provenance: byte-identical to the reference worktree's
`gomoku/crates/player/Cargo.toml` lines 7–11; on `main` the table ends at
line 10.*

The source edit extends one derive list in
[`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs).
Nothing else in the crate changes — no other type, no behavior, no test:

**Source:** `gomoku/crates/player/src/lib.rs`, line 101 after the edit
(line 102 names the enum for context).

```text
101 | #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
102 | pub enum ResignReason {
```

*Provenance: byte-identical to the reference worktree's
`gomoku/crates/player/src/lib.rs` line 101; on `main` line 101 is the same
derive list without the two serde entries. The reference worktree carries
further `player` changes — they belong to chapter 05 and are neither quoted
nor needed here; the slice verification restored `main`'s `player` plus
exactly this touch.* The gate proving nothing else changed is `cargo test
-p player`, green before and after (43 passed on the slice).

## 2. `src/lib.rs` — the skeleton and the public surface

The crate root opens with the crate-level contract (synchronous,
single-threaded, in-memory), `#![deny(missing_docs)]` so every public item
is documented from the moment it exists, and the two module declarations.
The `pub use` lines are the chapter's curated public surface — exactly the
thirteen names the implementation plan's step 14 curates, nothing deeper.
(The chapter paper's abstract lists a shorter surface that omits
`GameError`; step 14's list is the authoritative one.) The implementation
plan declares each module as its file appears (its step 3) and adds
the crate-root re-exports incrementally — at its step 10, before the
first integration test can go green — with step 14 auditing the final
surface; the finished file is quoted once, complete:

**Source:** [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs),
lines 1–15 (the complete chapter-1 file). *Not yet present on `main`; the
chapter creates it.*

```rust
  1 | //! Synchronous, single-threaded baseline self-play.
  2 | //!
  3 | //! One seeded game runs a complete Swap2 ceremony and then alternates two
  4 | //! persistent baseline players. The result is entirely in memory: opening
  5 | //! provenance, per-seat options, ordered plies, search metadata, and outcome.
  6 |
  7 | #![deny(missing_docs)]
  8 |
  9 | mod game;
 10 | mod opening;
 11 |
 12 | pub use game::{
 13 |     Game, GameError, Outcome, PlyRecord, PlySearch, WinReason, play_game, play_game_with_opening,
 14 | };
 15 | pub use opening::{GeneratedOpening, Opening, OpeningAnswer, OpeningProvenance, OpeningSource};
```

*Provenance: chapter-1 slice form. The crate doc (lines 1–5) is
byte-identical to the completed reference's `src/lib.rs` lines 1–5. The
reference then declares and re-exports the later chapters' modules as well;
chapter 01 declares exactly `game` and `opening` and re-exports exactly the
thirteen names above.*

## 3. `src/opening.rs` — the opening vocabulary

Execution starts from a description, so the first code is vocabulary.
`OpeningProvenance` records where an opening came from — drawn from the
game's random stream, or taken from an external ordered list — and the
distinction exists from the start so chapter 04's list-drawn openings never
masquerade as generated ones. `OpeningAnswer` is the answering side's
choice in the ceremony: take a color after three stones, or add one White
and one Black stone (in placement order) and then take a color. `Opening`
bundles the three offer stones (placement order Black, White, Black), the
answer, and the provenance. All three types derive serde serialization now
— chapter 02 serializes them, and declaring it here keeps chapter 02 out of
the vocabulary types. The colors are absolute, never relative sides: Swap2
places colors non-alternatingly, so a description could not name relative
sides at all (the engine design's locked decision 5, applied to the opening
vocabulary). The two imports are everything the file needs: the engine's
board, color, move, error, and typestate names, and the random-stream
machinery.

**Source:** [`gomoku/crates/selfplay/src/opening.rs`](../../../../gomoku/crates/selfplay/src/opening.rs),
lines 1–41. *Not yet present on `main`; the chapter creates it.*

```rust
  1 | use engine::{Board, Color, Move, Placing3, PlayError, Swap2};
  2 | use rand::{RngExt, rngs::StdRng};
  3 |
  4 | /// Where an opening came from.
  5 | #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
  6 | pub enum OpeningProvenance {
  7 |     /// Stones and answers drawn from the game's seeded RNG stream.
  8 |     Generated,
  9 |     /// An opening selected from an external, ordered list.
 10 |     Listed {
 11 |         /// Stable identity of the opening list.
 12 |         list: String,
 13 |         /// Zero-based position within that list.
 14 |         index: usize,
 15 |     },
 16 | }
 17 |
 18 | /// The answering side's choice during Swap2.
 19 | #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
 20 | pub enum OpeningAnswer {
 21 |     /// Finish after three stones by taking this color.
 22 |     Take(Color),
 23 |     /// Add one White and one Black stone, then take this color.
 24 |     AddTwo {
 25 |         /// The two new cells, in placement order (White, then Black).
 26 |         stones: [Move; 2],
 27 |         /// The color taken after the five-stone offer.
 28 |         final_choice: Color,
 29 |     },
 30 | }
 31 |
 32 | /// A complete Swap2 offer and its answers.
 33 | #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
 34 | pub struct Opening {
 35 |     /// Origin of this opening for later corpus metadata.
 36 |     pub provenance: OpeningProvenance,
 37 |     /// Initial cells in placement order (Black, White, Black).
 38 |     pub initial: [Move; 3],
 39 |     /// The answering side's choice.
 40 |     pub answer: OpeningAnswer,
 41 | }
```

*Provenance: byte-identical to the completed reference's `src/opening.rs`
lines 4–5 (the two imports) and 8–45 (the three types). The reference file
carries three further imports for chapter 04's disk-backed opening lists
(`std::fs`, `std::path::{Path, PathBuf}`, and `thiserror::Error`); chapter 01
needs none of them.*

`Opening::new` is a `const fn` that stores the description without
executing it — description and execution are separate artifacts, so a
description can be recorded, compared, and only then played. The parameter
order (stones, answer, provenance) is the order the chapter's ceremony test
calls it.

**Source:** `gomoku/crates/selfplay/src/opening.rs`, lines 43–56.

```rust
 43 | impl Opening {
 44 |     /// Describe an opening without executing it.
 45 |     #[must_use]
 46 |     pub const fn new(
 47 |         initial: [Move; 3],
 48 |         answer: OpeningAnswer,
 49 |         provenance: OpeningProvenance,
 50 |     ) -> Self {
 51 |         Self {
 52 |             provenance,
 53 |             initial,
 54 |             answer,
 55 |         }
 56 |     }
```

*Provenance: byte-identical to the completed reference's `src/opening.rs`
lines 47–60.*

## 4. `src/opening.rs` — `Opening::board()` executes the ceremony

The ceremony is the first thing a game *does*, and it runs through the
engine's `Swap2<State>` typestate
([`gomoku/crates/engine/src/opening.rs`](../../../../gomoku/crates/engine/src/opening.rs)),
so illegal opening sequences are unrepresentable: each phase-transition
method consumes the machine and returns the next phase's type, and there is
no method for the wrong order. The method places the three offer stones
with explicit colors — Black, White, Black — propagating rejections with
`?`, then finishes the offer phase and branches on the recorded answer.
`Take` ends the ceremony immediately; `AddTwo` places White then Black in
the placing-two phase and lets the stored `final_choice` pick the closing
call. The only reachable execution error is `PlayError::Occupied`, raised
when a description reuses a cell — that is the `# Errors` contract. Both
branches return a playing `Board` with White to move; the chapter's
ceremony test asserts exactly that for the five-stone branch.

**Source:** `gomoku/crates/selfplay/src/opening.rs`, lines 58–87
(continuing the `impl Opening` block that line 43 opened).

```rust
 58 |     /// Execute this opening through the engine's Swap2 typestate.
 59 |     ///
 60 |     /// # Errors
 61 |     /// Returns [`PlayError::Occupied`] if the description reuses a cell.
 62 |     pub fn board(&self) -> Result<Board, PlayError> {
 63 |         let mut placing = Swap2::<Placing3>::new();
 64 |         placing.place(self.initial[0], Color::Black)?;
 65 |         placing.place(self.initial[1], Color::White)?;
 66 |         placing.place(self.initial[2], Color::Black)?;
 67 |         let first_choice = placing.finish()?;
 68 |
 69 |         match self.answer {
 70 |             OpeningAnswer::Take(Color::Black) => Ok(first_choice.take_black()),
 71 |             OpeningAnswer::Take(Color::White) => Ok(first_choice.take_white()),
 72 |             OpeningAnswer::AddTwo {
 73 |                 stones,
 74 |                 final_choice,
 75 |             } => {
 76 |                 let mut placing = first_choice.place_two_more();
 77 |                 placing.place(stones[0], Color::White)?;
 78 |                 placing.place(stones[1], Color::Black)?;
 79 |                 let final_state = placing.finish()?;
 80 |                 Ok(match final_choice {
 81 |                     Color::Black => final_state.take_black(),
 82 |                     Color::White => final_state.take_white(),
 83 |                 })
 84 |             }
 85 |         }
 86 |     }
 87 | }
```

*Provenance: byte-identical to the completed reference's `src/opening.rs`
lines 62–91.*

## 5. `src/opening.rs` — `OpeningSource` and `GeneratedOpening`

The runner must not hard-code where openings come from — chapter 04 loads
tournament lists — so the stream's first consumer sits behind a one-method
trait. `OpeningSource::opening` takes the game's `StdRng` and returns an
`Opening`, not a `Result`: the draw cannot fail, so error behavior is exact
— a malformed description surfaces only when the ceremony executes it, and
a list-backed source validates at load time instead. The doc comment states
the stream contract: the runner supplies the same seeded stream it later
uses for move selection, so one game has exactly one random stream and one
identity — its seed.

**Source:** `gomoku/crates/selfplay/src/opening.rs`, lines 89–96.

```rust
 89 | /// Pluggable source of complete Swap2 opening descriptions.
 90 | ///
 91 | /// The runner supplies the same seeded RNG later used for move selection,
 92 | /// so a game still has exactly one RNG stream and one identity: its seed.
 93 | pub trait OpeningSource {
 94 |     /// Produce the next opening description.
 95 |     fn opening(&mut self, rng: &mut StdRng) -> Opening;
 96 | }
```

*Provenance: byte-identical to the completed reference's `src/opening.rs`
lines 93–100.*

`GeneratedOpening` is this chapter's source — the stand-in for curated
openings until chapter 04 loads the tournament lists. Its draw has two
steps: three unique cells from the central 7×7 region (rows and columns 4
through 10 inclusive — central stones produce representative tactical games
instead of scattered full-board play), then a uniform three-way answer:
take Black, take White, or draw two more unique central cells and take a
random color. Two private helpers keep the draw readable: `random_color`
maps one random bit to a color, and `draw_unique` builds the candidate list
excluding already-taken cells, then selects repeatedly by random index with
the swap-remove idiom, collecting into a fixed-size array with
`std::array::from_fn`. `Move::new` validates coordinates and is `expect`ed
here because the region is on the board by construction.

**Source:** `gomoku/crates/selfplay/src/opening.rs`, lines 98–143 (to the
end of the chapter-1 file).

```rust
 98 | /// An opening source that draws unique cells and answers from the game RNG.
 99 | #[derive(Debug, Default)]
100 | pub struct GeneratedOpening;
101 |
102 | impl OpeningSource for GeneratedOpening {
103 |     fn opening(&mut self, rng: &mut StdRng) -> Opening {
104 |         let initial = draw_unique(rng, &[]);
105 |         let answer = match rng.random_range(0..3) {
106 |             0 => OpeningAnswer::Take(Color::Black),
107 |             1 => OpeningAnswer::Take(Color::White),
108 |             _ => {
109 |                 let stones = draw_unique(rng, &initial);
110 |                 OpeningAnswer::AddTwo {
111 |                     stones,
112 |                     final_choice: random_color(rng),
113 |                 }
114 |             }
115 |         };
116 |         Opening::new(initial, answer, OpeningProvenance::Generated)
117 |     }
118 | }
119 |
120 | fn random_color(rng: &mut StdRng) -> Color {
121 |     if rng.random_range(0..2) == 0 {
122 |         Color::Black
123 |     } else {
124 |         Color::White
125 |     }
126 | }
127 |
128 | fn draw_unique<const N: usize>(rng: &mut StdRng, excluded: &[Move]) -> [Move; N] {
129 |     // Chapter 1's stand-in for curated openings: nearby stones produce
130 |     // representative tactical games instead of scattered, full-board play.
131 |     let mut available: Vec<_> = (4..11)
132 |         .flat_map(|row| {
133 |             (4..11).map(move |col| {
134 |                 Move::new(row, col).expect("both coordinates are inside the 15 by 15 board")
135 |             })
136 |         })
137 |         .filter(|mv| !excluded.contains(mv))
138 |         .collect();
139 |     std::array::from_fn(|_| {
140 |         let selected = rng.random_range(0..available.len());
141 |         available.swap_remove(selected)
142 |     })
143 | }
```

*Provenance: byte-identical to the completed reference's `src/opening.rs`
lines 277–322. Between the trait and `GeneratedOpening`, the reference file
carries chapter 04's tournament-list machinery (its lines 101–276);
chapter 01 does not, and this paper does not quote it.*

## 6. `src/game.rs` — the outcome and ply vocabulary

The runner's records are defined before the runner itself. `WinReason`
says how a win happened — a completed line of at least five stones, or a
voluntary concession embedding the `player` crate's `ResignReason` (the
type section 1.3 made serializable). `Outcome` has exactly two variants:
`Won { winner, reason }` and `Draw`. There is no *unknown* outcome and no
adjudicated loss — every returned game ended on the board or by concession,
and the runner only ever maps the engine's terminal `Status` or a seat's
resignation into this type. Both enums derive serde serialization for
chapter 02. The file's imports name exactly what the chapter uses:
`BaselineEvaluator` and `Device` from `baseline`, the board vocabulary from
`engine`, the seat vocabulary from `player`, the seeded-stream machinery
from `rand`, `thiserror` for the error type, and the crate's own opening
types.

**Source:** [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs),
lines 1–7 and 33–54. *Not yet present on `main`; the chapter creates it.*

```rust
  1 | use baseline::{BaselineEvaluator, Device};
  2 | use engine::{Color, Move, PlayError, Status};
  3 | use player::{AiPlayer, DecisionAction, DecisionError, PlayerOptions, ResignReason};
  4 | use rand::{SeedableRng, rngs::StdRng};
  5 | use thiserror::Error;
  6 |
  7 | use crate::{GeneratedOpening, Opening, OpeningSource};
```

```rust
 33 | /// How a game was won.
 34 | #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
 35 | pub enum WinReason {
 36 |     /// The winner completed a line of at least five stones.
 37 |     FiveOrMore,
 38 |     /// The loser voluntarily conceded after an enabled resignation check.
 39 |     Resignation(ResignReason),
 40 | }
 41 |
 42 | /// Terminal result of a game.
 43 | #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
 44 | pub enum Outcome {
 45 |     /// One color won, with the reason recorded separately.
 46 |     Won {
 47 |         /// The winning color.
 48 |         winner: Color,
 49 |         /// Whether the win came from the board or a concession.
 50 |         reason: WinReason,
 51 |     },
 52 |     /// The board filled without a winner.
 53 |     Draw,
 54 | }
```

*Provenance: `WinReason` and `Outcome` are byte-identical to the completed
reference's `src/game.rs` lines 51–72. The import block is a chapter-1
slice form: the completed reference's import block additionally names
`engine::Proof`, `player::SearchStats`, and three later-chapter crate items
(the certified-suffix record, the exploration schedule, and the recorded
prior constant), which chapters 02–05 introduce.*

`PlySearch` is this chapter's window into the search — and deliberately a
*slot*: the struct exists and the runner fills it, but the
`From<SearchStats>` conversion, the recorded top-k prior truncation, and
the sign-convention tests belong to chapter 03. The four fields are the
side to move (whose perspective determines the root value's sign), the raw
visit counts per visited root child (deliberately not normalized — the
visit-sum MANUAL test pins them raw, and the doc comment's mention of the
chapter-6 reader is the reference's own forward pointer), the root priors
after optional noise, and the root value. `PlyRecord` pairs the played move
with the optional slot, and the honesty rule governs the `Option`: the slot
is present when a search produced it and absent when a certified forced-win
proof bypassed search entirely — fabricating statistics for a move no
search produced would be dishonest telemetry.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 9–31.

```rust
  9 | /// Search information retained for one played move.
 10 | #[derive(Debug, Clone, PartialEq)]
 11 | pub struct PlySearch {
 12 |     /// Side whose perspective determines `root_value`'s sign.
 13 |     pub side_to_move: Color,
 14 |     /// Raw visit counts for every visited root child, in descending count order.
 15 |     ///
 16 |     /// Counts are deliberately not normalized; the chapter-6 reader derives π.
 17 |     pub visits: Vec<(Move, u32)>,
 18 |     /// All root priors after optional Dirichlet noise, in descending order.
 19 |     pub prior: Vec<(Move, f32)>,
 20 |     /// The searched root's value estimate from `side_to_move`'s perspective.
 21 |     pub root_value: f32,
 22 | }
 23 |
 24 | /// One played move and the search metadata available when it was chosen.
 25 | #[derive(Debug, Clone, PartialEq)]
 26 | pub struct PlyRecord {
 27 |     /// The legal move appended to the game.
 28 |     pub mv: Move,
 29 |     /// Search metadata, or `None` when a certified TSS move bypassed search.
 30 |     pub search: Option<PlySearch>,
 31 | }
```

*Provenance: `PlyRecord` is byte-identical to the completed reference's
`src/game.rs` lines 42–49. `PlySearch` is the chapter-1 slice form: every
line is identical to the reference's lines 12–25 except the `prior` field's
doc comment, which in the completed reference reads "Top root priors after
optional Dirichlet noise, in descending order." — "top" because chapter 03
truncates the recorded prior to its leading entries. Chapter 01 carries the
prior through untruncated, so the slice documents "all root priors". The
reference's `From<SearchStats> for PlySearch` implementation (its lines
27–40) is chapter 03's artifact and is not quoted here.*

`Game` is the runner's in-memory result: the seed, both supplied option
sets preserved verbatim (so the record always shows what was asked for),
the opening with its provenance, the ordered plies, and the outcome — six
fields, no more. The `PartialEq` derive makes the determinism test a
whole-record comparison. `Game::moves()` iterates the played moves in
order, excluding the opening stones (which live in `game.opening`), with an
exact size so callers can count without collecting.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 56–78.

```rust
 56 | /// Complete in-memory description of one self-play game.
 57 | #[derive(Debug, Clone, PartialEq)]
 58 | pub struct Game {
 59 |     /// Seed of the single RNG stream used for opening and move selection.
 60 |     pub seed: u64,
 61 |     /// Base options for Black, preserved verbatim in the record.
 62 |     pub black_options: PlayerOptions,
 63 |     /// Base options for White, preserved verbatim in the record.
 64 |     pub white_options: PlayerOptions,
 65 |     /// Full Swap2 description and its provenance.
 66 |     pub opening: Opening,
 67 |     /// Played moves in order, with their available search metadata.
 68 |     pub plies: Vec<PlyRecord>,
 69 |     /// Terminal winner or draw.
 70 |     pub outcome: Outcome,
 71 | }
 72 |
 73 | impl Game {
 74 |     /// Iterate over the ordered move list, excluding opening stones.
 75 |     pub fn moves(&self) -> impl ExactSizeIterator<Item = Move> + '_ {
 76 |         self.plies.iter().map(|ply| ply.mv)
 77 |     }
 78 | }
```

*Provenance: chapter-1 slice form. The `moves()` implementation is
byte-identical to the completed reference's lines 93–98. In the reference's
`Game` (its lines 74–91), the two option-field doc comments end with
"; exploration adjusts selection settings per ply." — chapter 04's schedule
— and the struct carries a seventh field, `certified_suffix`, which
chapters 02 and 05 own. Chapter 01 has neither, and its doc comments say
so: the base options are preserved verbatim because nothing adjusts them
yet.*

## 7. `src/game.rs` — `GameError`

The error type names the three ways a run can fail, in execution order: the
engine rejecting the ceremony (`Opening`), a seat asked to decide on a
terminal board (`Decision` — a defensive seam the status-first loop never
hits), and the engine rejecting a selected move (`Play`, carrying the
zero-based post-opening ply index, the move, and the engine's error). The
thiserror `#[from]` idiom on the first two variants lets the runner's `?`
operators convert without explicit mapping; the third is constructed
explicitly because it carries context the engine's error does not have.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 80–99.

```rust
 80 | /// A failure while constructing or playing a game.
 81 | #[derive(Debug, Error)]
 82 | pub enum GameError {
 83 |     /// The opening description could not be executed by the engine.
 84 |     #[error("Swap2 opening failed: {0}")]
 85 |     Opening(#[from] PlayError),
 86 |     /// A player was asked to decide on a terminal board.
 87 |     #[error("player decision failed: {0}")]
 88 |     Decision(#[from] DecisionError),
 89 |     /// A selected move was rejected by the engine.
 90 |     #[error("move {mv:?} at ply {ply} was rejected: {source}")]
 91 |     Play {
 92 |         /// Zero-based index among post-opening moves.
 93 |         ply: usize,
 94 |         /// Rejected move.
 95 |         mv: Move,
 96 |         /// Engine legality error.
 97 |         source: PlayError,
 98 |     },
 99 | }
```

*Provenance: byte-identical to the completed reference's `src/game.rs`
lines 145–164.*

## 8. `src/game.rs` — the runner

Everything so far was vocabulary; now execution. `play_game` is the
convenience entry: it delegates to `play_game_with_opening` with a mutable
borrow of a fresh `GeneratedOpening` temporary. Its doc comment carries the
default-options contract — `PlayerOptions::default()` disables resignation,
and default options are reserved for ordinary corpus runs, so this
chapter's games end on the board.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 101–117.

```rust
101 | /// Play one game using a freshly generated seeded Swap2 opening.
102 | ///
103 | /// Each color owns a persistent [`AiPlayer`] with its own
104 | /// [`BaselineEvaluator`] on [`Device::flex`]. There is no I/O, threading, or
105 | /// GPU work. The supplied base options are preserved in the returned game;
106 | /// `PlayerOptions::default()` disables resignation for ordinary corpus runs.
107 | ///
108 | /// # Errors
109 | /// Returns an error if the generated opening, a player decision, or a selected
110 | /// move is rejected by the engine.
111 | pub fn play_game(
112 |     seed: u64,
113 |     black_options: PlayerOptions,
114 |     white_options: PlayerOptions,
115 | ) -> Result<Game, GameError> {
116 |     play_game_with_opening(seed, black_options, white_options, &mut GeneratedOpening)
117 | }
118 |
```

*Provenance: chapter-1 slice form, identical to the completed reference's
lines 166–183 except one doc sentence: the reference adds "The chapter-4
exploration schedule adjusts only selection settings per ply." at its line
172, directly after the `PlayerOptions::default()` sentence — true there,
and omitted here because chapter 01 has no schedule.*

`play_game_with_opening` is the one implementation, and its body is the
chapter's walkthrough made literal. The setup (lines 128–139) performs the
first four steps: one `StdRng` built from the seed with the
`SeedableRng::seed_from_u64` idiom — the single stream that feeds the
opening draw and both seats, making the seed the complete identity of the
game; the opening drawn from the source (infallibly); the ceremony executed
with `?`, so a reused cell becomes `GameError::Opening` through the
`#[from]` conversion; and the two persistent seats built once, each an
`AiPlayer` around a boxed `BaselineEvaluator` on `Device::flex()` — the
only Burn value the runner touches. Seat identity *is* the absolute color,
and nothing alternates colors between games.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 119–139.

```rust
119 | /// Play one game using an injectable opening source.
120 | ///
121 | /// Tournament-list sources can implement [`OpeningSource`] later without
122 | /// changing the game loop. The source and both players share one seeded
123 | /// [`StdRng`], making the seed the complete identity of the game.
124 | ///
125 | /// # Errors
126 | /// Returns an error if the opening, a player decision, or a selected move is
127 | /// rejected by the engine.
128 | pub fn play_game_with_opening(
129 |     seed: u64,
130 |     black_options: PlayerOptions,
131 |     white_options: PlayerOptions,
132 |     opening_source: &mut impl OpeningSource,
133 | ) -> Result<Game, GameError> {
134 |     let mut rng = StdRng::seed_from_u64(seed);
135 |     let opening = opening_source.opening(&mut rng);
136 |     let mut board = opening.board()?;
137 |     let mut black = AiPlayer::new(Box::new(BaselineEvaluator::new(Device::flex())));
138 |     let mut white = AiPlayer::new(Box::new(BaselineEvaluator::new(Device::flex())));
139 |     let mut plies = Vec::new();
140 |
```

*Provenance: chapter-1 slice form. The doc comment, the signature, and the
setup are byte-identical to the completed reference's lines 185–205
(through `let mut plies = Vec::new();`). The reference's setup then
constructs two further objects — the chapter-05 suffix tracker and the
chapter-04 exploration schedule — which chapter 01 does not have.*

The loop (lines 141–199) checks `board.status()` **first**: a win maps to
`Outcome::Won` with `WinReason::FiveOrMore`, a full board to
`Outcome::Draw`, and only an ongoing board reaches a decision — which is
why `GameError::Decision` is a defensive seam this loop never produces.
`board.to_move()` — not a turn counter — selects the deciding seat and its
base option set, and the base options pass to `decide` unchanged at every
ply. On `Play(mv)`, the slot conversion runs first (the chapter-1 inline
form: the `Option::map` idiom keeps only the positive-count visit entries —
an unvisited root child carries no information — and carries the prior
through untruncated; chapter 03 replaces this block with the
`From<SearchStats>` conversion and the recorded top-k prior), then the
engine remains the legality gate: `board.play(mv)` proposes the move to the
same code that adjudicates the outcome, and a rejection becomes
`GameError::Play` with the current ply count as its zero-based index. On
`Resign`, the *other* color wins with `WinReason::Resignation` — under
default options the arm is unreachable. The loop's value is the outcome,
and the returned `Game` assembles everything without writing anything to
disk.

**Source:** `gomoku/crates/selfplay/src/game.rs`, lines 141–199 (to the end
of the chapter-1 file).

```rust
141 |     let outcome = loop {
142 |         match board.status() {
143 |             Status::Won(winner) => {
144 |                 break Outcome::Won {
145 |                     winner,
146 |                     reason: WinReason::FiveOrMore,
147 |                 };
148 |             }
149 |             Status::Draw => break Outcome::Draw,
150 |             Status::Ongoing => {}
151 |         }
152 |
153 |         let side = board.to_move();
154 |         let (seat, options) = match side {
155 |             Color::Black => (&mut black, &black_options),
156 |             Color::White => (&mut white, &white_options),
157 |         };
158 |         let decision = seat.decide(&board, options, &mut rng)?;
159 |         match decision.action {
160 |             DecisionAction::Play(mv) => {
161 |                 // Chapter-1 slot conversion, inline: keep only positive-count
162 |                 // visit entries and carry the prior through untruncated.
163 |                 // Chapter 3 replaces this with `PlySearch::from` and the
164 |                 // recorded top-k prior.
165 |                 let search = decision.search.map(|stats| PlySearch {
166 |                     side_to_move: stats.side_to_move,
167 |                     visits: stats
168 |                         .visits
169 |                         .into_iter()
170 |                         .filter(|(_, count)| *count > 0)
171 |                         .collect(),
172 |                     prior: stats.prior,
173 |                     root_value: stats.value,
174 |                 });
175 |                 board.play(mv).map_err(|source| GameError::Play {
176 |                     ply: plies.len(),
177 |                     mv,
178 |                     source,
179 |                 })?;
180 |                 plies.push(PlyRecord { mv, search });
181 |             }
182 |             DecisionAction::Resign(reason) => {
183 |                 break Outcome::Won {
184 |                     winner: side.other(),
185 |                     reason: WinReason::Resignation(reason),
186 |                 };
187 |             }
188 |         }
189 |     };
190 |
191 |     Ok(Game {
192 |         seed,
193 |         black_options,
194 |         white_options,
195 |         opening,
196 |         plies,
197 |         outcome,
198 |     })
199 | }
```

*Provenance: chapter-1 slice form, and the largest of the deltas. Against
the completed reference's runner body (its lines 200–258: the setup at
200–207, the loop at 209–247, and the returned `Game` through the function's
closing brace at 249–258; line 259 is the trailing blank before the test
module): the status-first match, the seat selection, the
resignation arm, and the `board.play` / `GameError::Play` construction are
line-for-line identical apart from surrounding context. Four chapter-1 differences remain. (1) Line 158 passes
`options` directly; the reference passes schedule-adjusted options.
(2) Lines 161–174 are the inline slot conversion; the reference calls
`PlySearch::from` (chapter 03). (3) The reference wraps the play in
suffix-tracker calls that thread the seat's own certified proof (chapter
05); chapter 01 has no tracker. (4) The assembled `Game` on lines 191–198
has six fields; the reference's has seven, adding `certified_suffix`. The
inline comment on lines 161–164 is part of the slice and marks the
chapter-03 seam in place. All of this ran green on the slice — see
Verification.*

## 9. `tests/game.rs` — the specification

The tests are the chapter's specification, and the file opens with its
three helpers. `options(simulations)` builds a `PlayerOptions` with a
chosen budget over the defaults — the defaults keep resignation disabled,
which this chapter's games rely on. `mv(row, col)` validates a test
coordinate. `FixedOpening` is the injection seam made concrete: a tuple
struct that implements `OpeningSource` by ignoring the supplied stream and
returning a clone of the stored opening. The import block pulls the
vocabulary from the crate root — the curated re-exports of section 2, never
deeper paths.

**Source:** [`gomoku/crates/selfplay/tests/game.rs`](../../../../gomoku/crates/selfplay/tests/game.rs),
lines 1–28. *Not yet present on `main`; the chapter creates it.*

```rust
  1 | use std::time::Instant;
  2 |
  3 | use engine::{Color, Move, Status};
  4 | use player::PlayerOptions;
  5 | use rand::rngs::StdRng;
  6 | use selfplay::{
  7 |     Opening, OpeningAnswer, OpeningProvenance, OpeningSource, Outcome, WinReason, play_game,
  8 |     play_game_with_opening,
  9 | };
 10 |
 11 | fn options(simulations: u32) -> PlayerOptions {
 12 |     PlayerOptions {
 13 |         simulations,
 14 |         ..PlayerOptions::default()
 15 |     }
 16 | }
 17 |
 18 | fn mv(row: u8, col: u8) -> Move {
 19 |     Move::new(row, col).expect("test coordinate is on the board")
 20 | }
 21 |
 22 | struct FixedOpening(Opening);
 23 |
 24 | impl OpeningSource for FixedOpening {
 25 |     fn opening(&mut self, _rng: &mut StdRng) -> Opening {
 26 |         self.0.clone()
 27 |     }
 28 | }
```

*Provenance: the helpers (lines 11–28) are byte-identical to the completed
reference's `tests/game.rs` lines 11–28. The import block is a chapter-1
slice form: the reference's import block additionally names
`player::SearchStats` and the crate's `PlySearch` and
`RECORDED_PRIOR_TOP_K`, which only the chapter-03 tests use.*

The first test specifies three properties at once: an opening source is
injectable, the `AddTwo` branch of the ceremony executes through the
engine's typestate (220 cells remain after five stones, White to move), and
the opening — provenance included — round-trips into the returned `Game`.
The game half plays seed 11 at budget 1 per seat from the fixed opening and
asserts exactly 10 plies. Note for the curious: this count is a property of
the chapter-1 runner as well as of the completed reference — both forms
were executed, and both record 10 plies for this seed and budget.

**Source:** `gomoku/crates/selfplay/tests/game.rs`, lines 30–56.

```rust
 30 | #[test]
 31 | fn injectable_opening_executes_the_add_two_swap2_branch() {
 32 |     let opening = Opening::new(
 33 |         [mv(5, 8), mv(5, 10), mv(7, 9)],
 34 |         OpeningAnswer::AddTwo {
 35 |             stones: [mv(9, 8), mv(9, 10)],
 36 |             final_choice: Color::White,
 37 |         },
 38 |         OpeningProvenance::Listed {
 39 |             list: "fixed-test".to_owned(),
 40 |             index: 11,
 41 |         },
 42 |     );
 43 |     let board = opening.board().expect("five-stone ceremony completes");
 44 |     assert_eq!(board.empty_moves().count(), 220);
 45 |     assert_eq!(board.to_move(), Color::White);
 46 |
 47 |     let game = play_game_with_opening(
 48 |         11,
 49 |         options(1),
 50 |         options(1),
 51 |         &mut FixedOpening(opening.clone()),
 52 |     )
 53 |     .expect("game from fixed opening completes");
 54 |     assert_eq!(game.opening, opening);
 55 |     assert_eq!(game.plies.len(), 10);
 56 | }
```

*Provenance: byte-identical to the completed reference's `tests/game.rs`
lines 67–93.*

The determinism test plays the same generated game twice — seed 11, budget
1 per seat — and asserts structural equality of the whole record, which the
`PartialEq` derive makes one assertion. Determinism is a property of the
whole record: opening, plies, search metadata, and outcome, not just the
move list (which is asserted separately through `game.moves()` anyway).

**Source:** `gomoku/crates/selfplay/tests/game.rs`, lines 58–69.

```rust
 58 | #[test]
 59 | fn same_seed_and_seat_options_reproduce_entire_game() {
 60 |     let first = play_game(11, options(1), options(1)).expect("first game completes");
 61 |     let second = play_game(11, options(1), options(1)).expect("second game completes");
 62 |
 63 |     assert_eq!(first, second);
 64 |     assert_eq!(
 65 |         first.moves().collect::<Vec<_>>(),
 66 |         second.moves().collect::<Vec<_>>()
 67 |     );
 68 |     assert_eq!(first.outcome, second.outcome);
 69 | }
```

*Provenance: byte-identical to the completed reference's `tests/game.rs`
lines 95–106.*

The replay test is the legality gate in miniature: play one generated game,
rebuild the board from the recorded opening, replay every recorded move
with `board.is_legal` checked before each `board.play`, then map the
engine's terminal `Status` back to an `Outcome` and require equality with
the recorded one. A still-`Ongoing` board after the last recorded ply
panics. The record the runner produces replays through the engine without
error because the engine was the only legality gate at every step.

**Source:** `gomoku/crates/selfplay/tests/game.rs`, lines 71–90.

```rust
 71 | #[test]
 72 | fn recorded_moves_replay_legally_to_the_recorded_outcome() {
 73 |     let game = play_game(11, options(1), options(1)).expect("game completes");
 74 |     let mut board = game.opening.board().expect("Swap2 opening is legal");
 75 |
 76 |     for (ply, record) in game.plies.iter().enumerate() {
 77 |         assert!(board.is_legal(record.mv), "illegal move at ply {ply}");
 78 |         board.play(record.mv).expect("recorded move replays");
 79 |     }
 80 |
 81 |     let replayed = match board.status() {
 82 |         Status::Won(winner) => Outcome::Won {
 83 |             winner,
 84 |             reason: WinReason::FiveOrMore,
 85 |         },
 86 |         Status::Draw => Outcome::Draw,
 87 |         Status::Ongoing => panic!("recorded game ended before the engine did"),
 88 |     };
 89 |     assert_eq!(replayed, game.outcome);
 90 | }
```

*Provenance: byte-identical to the completed reference's `tests/game.rs`
lines 108–127.*

The two MANUAL tests play slower games and stay `#[ignore]`d with MANUAL
comments so default runs stay fast. The first pins the runner-level
contract for the raw material of the policy target π: budget 2 for Black,
budget 1 for White, and for every ply that carries search metadata the
visit list is non-empty, every count is positive, and the counts sum to the
deciding seat's budget — selected by the recorded `side_to_move`. That
positivity assertion is why the runner's slot conversion drops zero-count
entries: an unvisited root child carries no information and contributes
nothing to the sum.

**Source:** `gomoku/crates/selfplay/tests/game.rs`, lines 92–118.

```rust
 92 | #[test]
 93 | // MANUAL: this uses a budget-2 Black seat and plays a full game. Run explicitly
 94 | // when validating per-seat visit sums; default tests stay at budget 1.
 95 | #[ignore]
 96 | fn tiny_budget_game_records_raw_visits_for_every_searched_ply() {
 97 |     let game = play_game(11, options(2), options(1)).expect("tiny-budget game completes");
 98 |     let mut searched_plies = 0;
 99 |
100 |     for ply in &game.plies {
101 |         let Some(search) = &ply.search else {
102 |             continue;
103 |         };
104 |         searched_plies += 1;
105 |         let simulations = match search.side_to_move {
106 |             Color::Black => 2,
107 |             Color::White => 1,
108 |         };
109 |         assert!(!search.visits.is_empty());
110 |         assert!(search.visits.iter().all(|(_, count)| *count > 0));
111 |         assert_eq!(
112 |             search.visits.iter().map(|(_, count)| count).sum::<u32>(),
113 |             simulations,
114 |             "root child visits must sum to the recording seat's simulations"
115 |         );
116 |     }
117 |     assert!(searched_plies > 0);
118 | }
```

*Provenance: byte-identical to the completed reference's `tests/game.rs`
lines 129–155.*

The second MANUAL test is the chapter's capstone: a budget-8 game per seat,
timed. It asserts the accounting invariants — non-empty plies bounded by
the board (at most 222 after a three-stone opening), `game.moves().count()`
equal to the ply count, at least one ply carrying search metadata, every
recorded root value finite, both recorded option sets still
resignation-disabled — and prints the one-line summary the chapter's
capstone asks for. The `Option::is_none_or` idiom keeps the finite-value
assertion one expression. Its MANUAL comment warns that the game can run
for minutes in a debug build, which is why release mode is not optional for
this run.

**Source:** `gomoku/crates/selfplay/tests/game.rs`, lines 120–146 (to the
end of the chapter-1 file).

```rust
120 | #[test]
121 | // MANUAL: this budget-8 full game can run for minutes in debug. The budget-1
122 | // tests above cover the same runner invariants without the slow search.
123 | #[ignore]
124 | fn tiny_budget_game_completes_and_records_search_metadata() {
125 |     let started = Instant::now();
126 |     let game = play_game(11, options(8), options(8)).expect("the smoke game completes");
127 |
128 |     assert!(!game.plies.is_empty());
129 |     assert!(game.plies.len() <= 222);
130 |     assert_eq!(game.moves().count(), game.plies.len());
131 |     assert!(game.plies.iter().any(|ply| ply.search.is_some()));
132 |     assert!(game.plies.iter().all(|ply| {
133 |         ply.search
134 |             .as_ref()
135 |             .is_none_or(|search| search.root_value.is_finite())
136 |     }));
137 |     assert!(!game.black_options.resign);
138 |     assert!(!game.white_options.resign);
139 |
140 |     eprintln!(
141 |         "smoke game: {} plies, {:?}, {:.3?}",
142 |         game.plies.len(),
143 |         game.outcome,
144 |         started.elapsed()
145 |     );
146 | }
```

*Provenance: byte-identical to the completed reference's `tests/game.rs`
lines 157–183.*

## 10. Verification

The chapter's commands, run from `gomoku/`:

```text
cargo test -p selfplay
cargo test -p selfplay --release --test game -- --ignored
cargo test -p player
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first command is the fast check: the default tests play budget-1 games
and finish in seconds even in a debug build. On the chapter-1 slice it
reports **3 passed, 2 ignored** for the game target — the chapter's exact
slice. In the completed reference the same target reports **5 passed, 2
ignored**, because the two `PlySearch`-conversion tests belong to chapter
03; the reference's full `selfplay` suite (all targets, all chapters) is
the hand-off's 50 passed, 4 ignored. The second command is the slow
game-playing verification, and both qualifiers matter: `--release` is not
optional (the MANUAL comment warns of minutes in a debug build), and
`--test game` focuses the run on this chapter's target — in the completed
reference a bare `-- --ignored` would also select later chapters' MANUAL
tests, which this chapter has no business running. Naming the tests works
too: `-- --ignored tiny_budget` selects exactly the two by their shared
prefix. The third command guards the chapter's one cross-crate touch. The
last two are the project's standing quality gates.

Evidence actually measured for this paper (2026-11-12, on the chapter-1
slice constructed in the scratch copy described under Provenance — not a
re-run of the completed reference, whose gate evidence the
[hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md)
records):

| Command | Result on the chapter-1 slice |
|---------|-------------------------------|
| `cargo test -p selfplay` | game target: 3 passed, 0 failed, 2 ignored (12.46 s, debug build) |
| `cargo test -p selfplay --release --test game -- --ignored` | 2 passed, 0 failed (0.15 s); smoke printed `smoke game: 10 plies, Won { winner: Black, reason: FiveOrMore }, 153.245ms` |
| `cargo test -p player` | 43 passed, 0 failed across the crate's three test targets (17 lib, 13 decisions, 13 guard) |
| `cargo clippy --all-targets -- -D warnings` | clean, workspace-wide |
| `cargo fmt --all -- --check` | no changes |

The 10-ply number deserves one explicit note so the evidence claims stay
precise. The 10-ply assertion belongs to the budget-1 injectable ceremony
test (`injectable_opening_executes_the_add_two_swap2_branch`); the budget-8
smoke test carries no ply-count assertion — it bounds the ply count by the
board and only prints its one-line summary. The smoke output in the table
above is the freshly measured chapter-1 slice run of 2026-11-12 and is slice
evidence only. A preserved stale binary of an earlier reference form once
printed 107 plies for this smoke game; that print is not acceptance evidence
for the completed reference, whose verified evidence is the hand-off's gate
record (50 passed, 4 ignored across all targets) — no fresh final-reference
run of this chapter's game tests is claimed here. The completed reference
executes the same test bodies with chapter 04's exploration schedule active;
the slice executes them without it, and no test body needed a chapter-1
adaptation. Do not run these game tests to compare against a corpus run:
this chapter generates no corpus and no debug-mode game run is part of the
gates.

## References

**This chapter:**

- [the-game-runner.md](the-game-runner.md) — the chapter paper: intention,
  specification as tests, execution-order walkthrough.
- [the-game-runner-design.md](the-game-runner-design.md) — the conceptual
  story: one seed as the game's identity, absolute colors, the engine as
  the only legality gate.
- [the-game-runner-implementation.md](the-game-runner-implementation.md) —
  the hiding layer this paper quotes the construction contracts of; its
  steps 7 and 10 fix the chapter-1 slice forms.

**This track:**

- [Tutorial README](../README.md) — conventions, gates, the chapter map,
  and the reference-provenance rule this paper's Provenance section
  applies.
- [Self-play primer](../00-selfplay-primer.md) — why strength-graded
  self-play data; the invariants this chapter starts from.
- [Design spec](../../../specs/2026-11-11-selfplay-corpora-design.md) —
  decision S6 (the runner's shape) and the record format this chapter's
  `Game` grows into.
- [Tutorial hand-off](../../../specs/2026-11-11-selfplay-corpora-tutorial-handoff.md) —
  the reference-implementation layout and the measured evidence of the
  completed tree.

**Prerequisite tracks:**

- [Engine tutorial](../../13-engine-tutorial/README.md) and
  [engine design](../../../13-engine-design.md) — the Swap2 typestate and
  the absolute-colors decision (decision 5).
- [MCTS tutorial](../../mcts-tutorial/README.md) — the PUCT search both
  seats think with, behind the `Evaluator` seam.
- [Baseline tutorial](../../baseline-tutorial/README.md) — the seats:
  `AiPlayer`, `PlayerOptions`, the own-side TSS probe, and the honesty rule
  for certified moves.

**Source (live on `main`):**

- [`gomoku/Cargo.toml`](../../../../gomoku/Cargo.toml) — the workspace
  manifest; this chapter appends one member.
- [`gomoku/crates/player/Cargo.toml`](../../../../gomoku/crates/player/Cargo.toml)
  and [`gomoku/crates/player/src/lib.rs`](../../../../gomoku/crates/player/src/lib.rs) —
  the chapter's one cross-crate touch: serde serialization for
  `ResignReason`.
- [`gomoku/crates/engine/src/opening.rs`](../../../../gomoku/crates/engine/src/opening.rs) —
  the `Swap2<State>` typestate the ceremony executes through.
- [`gomoku/crates/engine/src/board.rs`](../../../../gomoku/crates/engine/src/board.rs) —
  `Board`, `Color`, `Status`, `PlayError`.
- [`gomoku/crates/baseline/src/lib.rs`](../../../../gomoku/crates/baseline/src/lib.rs) —
  `BaselineEvaluator` and the `Device` re-export.

**Source (created by this chapter; the links resolve once the chapter
lands on `main`):**

- [`gomoku/crates/selfplay/Cargo.toml`](../../../../gomoku/crates/selfplay/Cargo.toml) —
  the crate manifest (section 1.2).
- [`gomoku/crates/selfplay/src/lib.rs`](../../../../gomoku/crates/selfplay/src/lib.rs) —
  the crate's public surface (section 2).
- [`gomoku/crates/selfplay/src/opening.rs`](../../../../gomoku/crates/selfplay/src/opening.rs) —
  the opening vocabulary, the ceremony, the source seam (sections 3–5).
- [`gomoku/crates/selfplay/src/game.rs`](../../../../gomoku/crates/selfplay/src/game.rs) —
  the records, the error type, the runner (sections 6–8).
- [`gomoku/crates/selfplay/tests/game.rs`](../../../../gomoku/crates/selfplay/tests/game.rs) —
  the chapter's specification (section 9).

**Reference provenance:** the completed reference implementation lives in
the read-only sibling worktree `rust-ml-selfplay-ref` (branch
`selfplay-ref-v2`, base `9028aa8`, changes uncommitted); it is quoted and
compared against, never linked, and it is deleted once the tutorial is
complete on `main`. The chapter-1 slice forms in this paper were
constructed per the implementation plan in a scratch copy of that worktree
and gate-verified there on 2026-11-12; the reference worktree itself was
not modified.

**External sources:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). <https://doi.org/10.1038/nature24270> — the
  visit-distribution policy target π whose raw material the runner retains.
- Rosin, C. D. (2011). *Multi-armed bandits with episode context* (PUCT).
  <https://doi.org/10.1007/s10472-011-9258-6> — the selection rule of the
  search both seats run.
- Allis, L. V. (1994). *Searching for Solutions in Games and Artificial
  Intelligence*. PhD thesis, Maastricht University.
  <https://doi.org/10.26481/dis.19940923la> — threat-space search, the
  seat's certified-forced-win probe.