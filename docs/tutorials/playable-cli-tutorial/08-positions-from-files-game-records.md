# Chapter 08 — Positions from files: game records

## Abstract

This chapter enrolls the second external position source into the study
table: **game records**. A played game is a navigable position tree —
loading a Piskvork `.psq` record replays it as the variation tree's
main line with the cursor at the end position, and the engine
adjudicates the final status. You will write the record parser
(`GameRecord::from_psq`) with its tolerant-stop/strict-truncation line
handling, the `SessionInit::game` constructor with its `GameStudy`
context, and the startup flag `gomoku play --game <file.psq>`. No new
commands: navigation, take/release, AI seats, and the chapter-06
overlays already work at any cursor position. The felt capstone:
**load a `.psq`, land on the end position, walk back through the moves,
branch off at the critical moment, and let the AI show what it
thinks.**

## Glossary

| Term | Definition |
|------|------------|
| **Game record** | A Piskvork `.psq` file: one played game as a header line plus one move per line, loaded for study. Not a game mode. |
| **Main line** | The variation-tree path the record replays onto: root to end position, one child per node, cursor at the end. Branching off it creates variations; the recorded line stays variation 0. |
| **Metadata tail** | The lines Piskvork appends after the last move: two player-name lines, a result marker (`-1`), a tournament line. Ignored by the parser; every real record carries one. |
| **Adjudication** | Deriving the game status (`Won` / `Draw` / `Ongoing`) by replaying the moves through `engine::Board`, not by trusting the file's result marker. |
| **Tolerant stop** | The first line that is not a full move line ends the game; the rest of the file is ignored metadata. |
| **Strict truncation** | A line that starts like a move (`<digits>,<digits>`) but is not a full `x,y,time` line is a truncated record and is rejected, not silently dropped. |
| **`GameStudy`** | The session context carrying the record's source path and the parsed record, for display. |
| **Corpus validation** | The format decisions in this chapter were verified against all 11,400 records of the 2026 Gomocup Freestyle15 corpus: all parse; exactly 6 are rejected (loss-by-illegal-move duplicates). |

## Context

Chapter 07 added the first file source, puzzle packs. This chapter adds
the second and last: recorded games. The corpus is
`data/gomocup-games/` — the Gomocup tournament archives, Freestyle15
subset (the only ruleset matching ours: 15×15, overlines win),
git-ignored and regenerated with `data/gomocup-games/fetch.sh`.

The design constraints are the amendment's, unchanged from chapter 07:
a record is not a game mode but a starting position (a whole main line
of them); parsing lives in the library, file I/O in `main.rs` and
`sessiondemo`; a missing file exits 2 pointing at the corpus's
fetch.sh; tests use a committed fixture, never `data/`.

What this chapter adds beyond chapter 07's pattern is the *parsing
problem*. A puzzle pack is structured JSON with a schema. A `.psq`
file is a line-oriented format found in the wild — CRLF endings, a
metadata tail, no schema anywhere — and the parser's contract is the
result of measuring the actual corpus, not of reading a specification.

## Intention

1. Create `gomoku/crates/cli/src/game.rs`:
   * `GameRecord` and a thiserror `GameError`;
   * `GameRecord::from_psq` with header validation, tolerant-stop /
     strict-truncation move-line handling, CRLF tolerance, and full
     replay validation through `engine::Board`.
2. Extend `gomoku/crates/cli/src/session.rs`:
   * the `GameStudy` context and the infallible `SessionInit::game`
     constructor;
   * `build_history` — the record replayed as the main line with the
     cursor at the end — used by both `new` and `reset`.
3. Extend `gomoku/crates/cli/src/main.rs` with `--game <FILE.psq>`
   (conflicts with `--puzzle`, rejected for `selfplay`, Swap2 skipped,
   exit 2 plus the fetch.sh hint on a missing or invalid file) and
   `gomoku/crates/cli/src/bin/sessiondemo.rs` with `--game`.
4. Extend `gomoku/crates/cli/src/tui/render.rs` with the side-panel
   game line.
5. Create `gomoku/crates/cli/tests/fixtures/mini-game.psq`,
   `gomoku/crates/cli/tests/game.rs`, the game session tests in
   `gomoku/crates/cli/tests/session.rs`, and
   `gomoku/crates/cli/game_demo.txt`.
6. Understand the pre-existing `History::forward` AtLeaf quirk the
   demo sidesteps (sidebar below).

Observable done-state: the scripted `sessiondemo --game` run in the
capstone reproduces; the TDD checklist passes; the gates are clean.

## Mental mapping

### A played game is a navigable position tree

The naive design for game records is a replay viewer: step forward,
step back, done. The study table needs no such thing. A record is a
sequence of legal moves; a sequence of legal moves is a path; the
variation tree stores paths. So loading a record replays it from an
empty root onto the main line, puts the cursor at the end position, and
stops. Everything you might want to do with a recorded game — walk back
through the moves, branch off at the critical moment, release a seat
and let the AI examine the position, toggle the visits overlay — is
already a table feature, and works by construction.

Two consequences worth holding onto:

* **The recorded line is variation 0.** When your try (or the AI's)
  branches off the main line, the recorded continuation stays first in
  every branch list. The record is the baseline you compare against.
* **The engine adjudicates, not the file.** The `.psq` tail carries a
  result marker, and the parser ignores it. The status you see —
  `Won(Black)` in the capstone — is the engine's verdict on the
  replayed position. A file that claims a result its moves do not
  produce is simply a file whose moves produce something else.

### The `.psq` format as found in the wild

A record looks like this (the chapter's fixture, shortened tail names
aside — this is the real shape):

```text
Piskvorky 15x15, 11:11, 0
7,8,0
1,1,0
8,8,120
15,1,0
9,8,250
1,15,0
5,5,410
15,15,0
10,8,530
3,12,60
6,6,700
12,3,0
11,8,900
CARBON17.zip
PISQ7.zip
-1
1,Freestyle15_2
```

* **Header**: `Piskvorky 15x15, 11:11, 0` — the board size, the
  tournament time control, and a trailing field. Only 15×15 is
  accepted; the remaining header fields may vary.
* **Move lines**: `x,y,time-ms` — 1-based, `x` the column, `y` the row,
  Black first, alternating; the time field is validated as a number and
  otherwise ignored.
* **CRLF endings** throughout the corpus; the parser trims them.
* **Metadata tail** after the last move: two player-name lines, a `-1`
  result marker, a tournament line (`1,Freestyle15_2`).

A caution on coordinates: `data/gomocup-games/README.md`'s format note
describes the coordinates as 0-based. The corpus validation says
otherwise — under the 1-based reading, a `0` coordinate is off the
board and would be rejected, and none of the 11,400 records tripped
that check. The 1-based reading in `src/game.rs` (pinned by the test
`file_coordinates_are_1_based_x_column_y_row`) is authoritative; the
README note is stale.

### Tolerant stop, strict truncation

The parser's line handling is a measured compromise between two
failure postures:

* **Rejecting trailing lines is not an option.** Every real record
  carries a metadata tail; a strict parser would reject the entire
  corpus.
* **Ignoring everything unparsable is not an option either.** A file
  truncated mid-game — a download cut short — looks exactly like a
  finished game with an odd tail, and would be silently accepted as a
  shorter game.

The split: move lines are consumed greedily from line 2; the first line
that is not `int,int,int` ends the game and the rest is ignored
(tolerant stop). But a line that *starts like a move* — `<digits>,
<digits>` — yet is not a full move line is a truncated record and is
rejected with `MalformedMoveLine` (strict truncation). The rule is safe
because no real metadata line starts that way, verified across the
corpus: player names are filenames, the result marker is `-1`, and the
tournament line starts with a bare number, not a digit pair.

The final position may be `Won`, `Draw`, or `Ongoing` — a record that
simply ends early is legal input.

### Validation is replay

Structure is not enough; the moves must be a game. Every move replays
through `engine::Board`: on-board coordinates, no cell played twice,
and nothing after a terminal position. This is where the corpus
produced its one surprise: 6 of the 11,400 records end with a duplicate
of an earlier move — Piskvork's way of recording a loss by illegal
move. The recorded sequence does not replay, so those 6 are rejected as
`IllegalMove` and documented as such in the `src/game.rs` module docs.
The other 11,394 parse clean.

### `SessionInit::game` is infallible

Because the parser already validated the replay, the constructor cannot
fail: `build_history` plays the record into a fresh `History` with
`expect("game record replay must be legal")`, and the same helper backs
`reset()`, so `new` returns to the fully replayed game — not to an
empty board. The session opens at the *end* position: study starts from
the result and walks backwards.

`--game` composes like chapter 07's `--puzzle`: Swap2 is skipped, the
`--ai` / `--color` / sims flags apply as usual, a missing or invalid
file exits 2 pointing at `data/gomocup-games/fetch.sh`. Two guards are
new: clap declares `--game` conflicting with `--puzzle` (one position
source per session), and `selfplay` rejects `--game` with exit 2 —
self-play from a finished game is meaningless, and from an unfinished
one it is just `play` with AI seats.

### Sidebar: the `History::forward` AtLeaf quirk

A pre-existing behavior from chapter 01 surfaces in record sessions.
`History::forward(n)` walks a single-child line, but if it hits a leaf
before exhausting `n` it returns `AtLeaf` — discarding the partial
count — and `execute_forward` maps *any* `AtLeaf` to
`Err(ForwardAtLeaf)`, even though the cursor moved. In a record session
the main line is exactly such a chain ending at a leaf, and the session
opens at that leaf, so the natural "jump to the end" gesture
(`forward 99` from the root) moves the cursor all the way and still
reports `Error: Forward reached a leaf`, with no `CursorMoved` event.

The demo sidesteps the quirk with exact counts (`forward 2`,
`forward 4`). It is a known quirk, not fixed in this chapter: the
contract and its tests date from chapter 02, and changing them is its
own slice.

## Low-level design

### Files changed vs chapter 07

Your work in this chapter is confined to the `cli` crate; the
footprint:

```text
$ git diff d6c2af3..3bf0544 --stat -- gomoku/crates/cli
 gomoku/crates/cli/game_demo.txt                |  24 +++
 gomoku/crates/cli/src/bin/sessiondemo.rs       |  26 ++++
 gomoku/crates/cli/src/game.rs                  | 192 ++++++++++++++++++++++++
 gomoku/crates/cli/src/lib.rs                   |   3 +-
 gomoku/crates/cli/src/main.rs                  |  51 ++++++-
 gomoku/crates/cli/src/session.rs               |  74 +++++++++-
 gomoku/crates/cli/src/tui/render.rs            |  12 ++
 gomoku/crates/cli/tests/acceptance.rs          |   1 +
 gomoku/crates/cli/tests/actor.rs               |   3 +
 gomoku/crates/cli/tests/fixtures/mini-game.psq |  18 +++
 gomoku/crates/cli/tests/game.rs                | 194 +++++++++++++++++++++++++
 gomoku/crates/cli/tests/session.rs             | 176 ++++++++++++++++++++++
 gomoku/crates/cli/tests/study.rs               |   1 +
 13 files changed, 771 insertions(+), 4 deletions(-)
```

`src/command.rs` is untouched: a record needs no new commands.
`src/tui/study.rs` is untouched as well; the help text already covers
navigation.

### `src/game.rs`

New module. Public surface:

```rust
pub struct GameRecord {
    /// All moves of the game, Black first, alternating.
    pub moves: Vec<Move>,
}

impl GameRecord {
    pub fn from_psq(text: &str) -> Result<Self, GameError>;
}
```

`GameError` variants: `MissingHeader`, `InvalidHeader`,
`UnsupportedBoardSize`, `MalformedMoveLine`, `OffBoard`,
`IllegalMove`.

Three private helpers carry the format rules: `parse_header` (the
`Piskvorky <W>x<H>, …` shape, 15×15 only), `parse_move_line` (exactly
three numeric fields), and `starts_like_a_move` (the truncation
detector). The module docs record the corpus findings — CRLF
throughout, the uniform header, the universal metadata tail, the 6
loss-by-illegal-move records — so the decisions are justified where
they are implemented.

### `src/session.rs`

```rust
pub struct GameStudy {
    /// Path the record was loaded from (for display and error messages).
    pub source: PathBuf,
    /// The parsed record.
    pub record: GameRecord,
}

impl SessionInit {
    pub fn game(source: PathBuf, record: GameRecord) -> Self;
}

impl SessionCore {
    pub fn game(&self) -> Option<&GameStudy>;
}
```

`build_history(&init)` replaces the direct `History::from_opening`
call in both `SessionCore::new` and `reset`: it replays
`game.record.moves` onto the fresh history when a game context exists.
The constructor consumes the validated record; there is no `Result`
because there is nothing left to fail.

### `src/main.rs`

`PlayArgs` gains:

```rust
/// Study a Piskvork game record (skips Swap2).
#[arg(long, value_name = "FILE.psq", conflicts_with = "puzzle")]
game: Option<PathBuf>,
```

`build_game_init` reads and parses the record (exit 2 with the
`data/gomocup-games/fetch.sh` hint on failure), builds the normal play
init from an empty opening, and attaches the `GameStudy`. The
`selfplay` subcommand rejects `--game` with exit 2.

### `src/bin/sessiondemo.rs`

`build_init` handles `--game` before `--puzzle`, and `print_state`
shows `game <file stem>  <N> plies` when a record is loaded.

### `src/tui/render.rs`

The side panel gains one line, consistent with the puzzle line:
`Game: <file stem> · <N> plies`.

### The fixture

`tests/fixtures/mini-game.psq` is a 13-move hand-made record with the
real header and metadata tail, ending in a detectable freestyle Black
win on row 8. It uses LF endings; CRLF tolerance is covered by the
`crlf_endings_are_tolerated` test with inline text. Tests load it with
`include_str!`, never the corpus.

## Solution (opt-in)

The complete target files for this chapter are quoted verbatim in
[08-positions-from-files-game-records/01-solution.md](08-positions-from-files-game-records/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare your work.

## TDD checklist

Run from `gomoku/` with `cargo test -p cli`.

### `tests/game.rs`

Write exactly these `#[test]` functions:

1. `parses_fixture_into_the_main_line`
2. `file_coordinates_are_1_based_x_column_y_row`
3. `fixture_replays_to_a_black_win`
4. `trailing_metadata_is_ignored`
5. `crlf_endings_are_tolerated`
6. `header_time_fields_may_vary`
7. `empty_text_is_rejected`
8. `missing_header_is_rejected`
9. `non_15x15_board_is_rejected`
10. `off_board_coordinate_is_rejected`
11. `duplicate_move_is_rejected`
12. `malformed_move_line_is_rejected`
13. `truncated_game_parses_as_an_ongoing_position`
14. `move_after_the_game_ended_is_rejected`

### `tests/session.rs`

Add these six session tests:

1. `game_session_replays_the_record_as_the_main_line_with_cursor_at_end`
2. `game_session_navigation_works_across_the_loaded_line`
3. `game_session_branch_and_var_at_a_mid_game_cursor`
4. `game_session_take_release_and_step_from_a_mid_game_cursor`
5. `game_session_search_info_stays_on_its_node_when_navigating`
6. `new_resets_to_the_fully_replayed_game`

All earlier tests continue to pass and are part of the chapter-08 gate;
the earlier `SessionInit` struct literals gain a `game: None` field
(in `tests/actor.rs` and `tests/study.rs` now, and in chapter 09's
`tests/acceptance.rs` when you write it).

### Red-green-refactor rhythm

1. **Red:** Create `src/game.rs`, extend `src/session.rs`, add the
   fixture, `tests/game.rs`, and the new session tests. Run
   `cargo test -p cli`. Expect failures.
2. **Green:** Implement the contracts above until `cargo test -p cli`
   passes.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any issues.

## Capstone: walk back, branch off, ask the AI

The script (`crates/cli/game_demo.txt`; `#` lines are comments):

```text
# game_demo.txt — studying a recorded game at the table.
# Run from gomoku/:
#   cargo run -p cli --bin sessiondemo -- --game crates/cli/tests/fixtures/mini-game.psq --script crates/cli/game_demo.txt
# The record opens at the END: 13 plies, Black has won (freestyle five
# on row 8). Step back to the position where the win was still open.
back 5
# Ply 8, Black to move. Peek at the recorded continuation, then return.
forward 2
back 2
# Branch: d6 is a legal alternative to the recorded j8.
d6
back
forward
# Variation 0 is the recorded winning line.
var 0
forward 4
# Back at ply 8: release the Black seat and let the AI analyze the
# mid-game position.
back 5
release black
step
info
take black
quit
```

Run it:

```text
cd gomoku
cargo run -q -p cli --bin sessiondemo -- --game crates/cli/tests/fixtures/mini-game.psq --script crates/cli/game_demo.txt
```

Expect these frames. The AI step is
reproducible without a seed: temperature 0 selects by argmax and no
noise is applied.

The record opens at the **end**: ply 13, and the engine has adjudicated
`Won(Black)` — the freestyle five on row 8 (`g8`–`k8`, the last move in
lowercase):

```text
Session started. Type 'h' for help.
ply 13  status Won(Black)  black Human  white Human
game mini-game  13 plies
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  O  .  .  .  .  .  .  .  .  .  .  .  .  .  O  
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  .  .  .  .  .  .  .  .  .  .  .  O  .  .  .  
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 5  .  .  .  .  X  .  .  .  .  .  .  .  .  .  .  
 6  .  .  .  .  .  X  .  .  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 8  .  .  .  .  .  .  X  X  X  X  x  .  .  .  .  
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  O  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  O  .  .  .  .  .  .  .  .  .  .  .  .  .  O  
```

`back 5` walks to the position where the win was still open: ply 8,
Black to move with three on row 8:

```text
  Cursor moved to ply 8
ply 8  status Ongoing  black Human  white Human
game mini-game  13 plies
branches: 0:j8
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  O  .  .  .  .  .  .  .  .  .  .  .  .  .  O  
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 5  .  .  .  .  X  .  .  .  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 8  .  .  .  .  .  .  X  X  X  .  .  .  .  .  .  
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
```

Peek at the recorded continuation and return:

```text
  Cursor moved to ply 10
ply 10  status Ongoing  black Human  white Human
game mini-game  13 plies
branches: 0:f6
```

```text
  Cursor moved to ply 8
ply 8  status Ongoing  black Human  white Human
game mini-game  13 plies
branches: 0:j8
```

Now branch: `d6` is a legal alternative to the recorded `j8`, and the
tree keeps both:

```text
  Moved d6 by Human
  Branched: j8 d6
ply 9  status Ongoing  black Human  white Human
game mini-game  13 plies
```

`back` and `forward` report the branch point:

```text
  Cursor moved to ply 8
ply 8  status Ongoing  black Human  white Human
game mini-game  13 plies
branches: 0:j8  1:d6
```

```text
  Branch point: 0: j8 1: d6
ply 8  status Ongoing  black Human  white Human
game mini-game  13 plies
branches: 0:j8  1:d6
```

Variation 0 is the recorded winning line; `forward 4` rides it to the
end:

```text
  Cursor moved to ply 9
ply 9  status Ongoing  black Human  white Human
game mini-game  13 plies
branches: 0:c12
```

```text
  Cursor moved to ply 13
ply 13  status Won(Black)  black Human  white Human
game mini-game  13 plies
```

Back at the critical position, release the Black seat and let the AI
examine it:

```text
  Cursor moved to ply 8
ply 8  status Ongoing  black Human  white Human
game mini-game  13 plies
branches: 0:j8  1:d6
```

```text
  Black seat is now Ai
ply 8  status Ongoing  black Ai  white Human
game mini-game  13 plies
branches: 0:j8  1:d6
```

The AI plays `f8` — the *other* four on row 8, a move the recorded game
never saw — and the move branches off the recorded line:

```text
  Moved f8 by Ai
  Branched: j8 d6 f8
ply 9  status Ongoing  black Ai  white Human
game mini-game  13 plies
    a  b  c  d  e  f  g  h  i  j  k  l  m  n  o
 1  O  .  .  .  .  .  .  .  .  .  .  .  .  .  O  
 2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 3  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 4  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 5  .  .  .  .  X  .  .  .  .  .  .  .  .  .  .  
 6  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 7  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
 8  .  .  .  .  .  x  X  X  X  .  .  .  .  .  .  
 9  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
10  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
11  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
12  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
13  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
14  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
15  O  .  .  .  .  .  .  .  .  .  .  .  .  .  O  
```

`info` shows the search that produced it:

```text
value 0.677  total visits 400
top moves (visits, prior%):
  1. (281, 0.5%) f8
  2. (13, 0.5%) e8
  3. (1, 0.5%) b1
  4. (1, 0.5%) c1
  5. (1, 0.5%) d1
```

Read the columns: the prior is flat (every move 0.5%) because the
tactics evaluator has no positional sense and there is no immediate win
or block at ply 8; the visits column is where the search's opinion
lives, 281 of 400 simulations on `f8`. On this position the chapter-06
overlays would paint exactly this disagreement. Then `take black`
returns the seat:

```text
  Black seat is now Human
ply 9  status Ongoing  black Human  white Human
game mini-game  13 plies
```

That is the workflow: load a record, land on the adjudicated end, walk
back to the critical moment, branch off, and let the AI show what it
thinks — from a position no engine ever searched in the real game.

## Done when

* `cargo test -p cli` passes from `gomoku/` (including all earlier
  tests, the 14 parser tests, and the 6 new session tests).
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* The `sessiondemo --game` scripted demo prints the frames above.
* Commit message for the code slice:
  `feat(cli): game records (.psq) enroll into the study table`.

Next: [Chapter 09 — Acceptance](09-acceptance.md)

## References

* [`00-playable-cli-primer.md`](00-playable-cli-primer.md) — the study
  table, core/shell, and the positions-from-files amendment summary.
* [`README.md`](README.md) — chapter map and tutorial conventions.
* [`docs/plans/2026-10-17-playable-cli.md`](../../plans/2026-10-17-playable-cli.md)
  — amendment task T9 (this chapter).
* [`docs/specs/2026-10-17-playable-cli-design.md`](../../specs/2026-10-17-playable-cli-design.md)
  — the "positions from files" amendment (game records).
* [`data/gomocup-games/README.md`](../../../data/gomocup-games/README.md)
  — the corpus (note: its 0-based coordinate remark is stale; this
  chapter and `src/game.rs` are authoritative).
* [`07-positions-from-files-puzzles.md`](07-positions-from-files-puzzles.md)
  — the first file source and the shared amendment constraints.
* [`01-the-variation-tree.md`](01-the-variation-tree.md) — the
  variation tree, including the `forward` AtLeaf behavior discussed in
  the sidebar.
* [`06-seeing-the-mind.md`](06-seeing-the-mind.md) — the overlays that
  work on recorded positions by construction.
