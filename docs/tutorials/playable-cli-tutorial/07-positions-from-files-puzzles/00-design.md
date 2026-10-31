# Chapter 07 redesign — positions from files: puzzles

Status: the reference build is on the `reference/chapter-07-redesign`
branch (four slice commits: a `f2eb9ae`, b `b2548ea`, c `418389a`, d
`0489ecc`, gates green per slice). The code is the learner's work,
sub-chapter by sub-chapter. This paper is the design authority for the
four sub-chapters.

Decided (2026-10, with the owner):

- Folder layout: one subfolder per sub-chapter, named `a-pack-parser`,
  `b-puzzle-session`, `c-study-commands`, and `d-puzzle-flag`. Each
  subfolder contains the tutorial, the design paper, the implementation
  notes, and the opt-in solution file. The sub-chapter name appears in
  every file name.
- The old chapter-level `01-solution.md` file is removed. Its content
  is split across the four sub-chapter solution files and re-verified
  against the new-structure reference build (the old file quoted the
  pre-redesign reference at `d6c2af3`).
- All documents follow ASD-STE100: short sentences, one meaning per
  word, active voice, no figurative language.

## Why the restructure

The monolithic chapter mixed four concerns that teach different ideas:
parsing and validation (a), enrolling a position into the session (b),
the study commands (c), and the startup flag on the real table (d). The
split follows the execution path of a puzzle from disk to table: file →
validated data → session root → command vocabulary → `play` flag. Each
sub-chapter has one lesson and one verifiable artifact.

## Decision 1 — A puzzle is a position, not a mode

The legacy puzzle viewer was a separate flow with its own board and its
own input handling. The study table deletes that idea. Loading a puzzle
produces a hotseat session whose variation-tree root is the puzzle
position, attacker to move. Take/release, AI seats, branching, the
overlays from chapter 06 — all of it works by construction, because
there is nothing puzzle-specific below the command layer. The puzzle
adds exactly three things: the claimed solution line (`hint` /
`reveal`), the pack (`next` / `prev`), and one context line in the side
panel.

Note what the table does *not* do: it does not grade you. Trying a move
that is not the claimed first move emits an ordinary `Moved` event, not
a buzzer. Your try becomes a branch in the tree; `back` takes it back;
the branch stays visible. Studying a puzzle means comparing your try
against the claim — and, when the claim surprises you, releasing a seat
and letting the search examine it. A grader would collapse all of that
into right/wrong.

## Decision 2 — The pack format, and the coordinate trap

The pack format (from `data/puzzles/README.md`):

```json
{ "v": 1, "count": 763, "items": [
  { "i": 0, "a": "black", "d": 6, "b": [[x,y],...], "w": [[x,y],...],
    "l": [[x,y,side],...] } ] }
```

Two conventions, both kept from the legacy viewer:

* **Coordinates are 0-based `[x, y]` with `x` the column and `y` the
  row**, mapping to `Move::new(y, x)`. Swapping `x` and `y` mirrors
  every position across the main diagonal: stones are still on the
  board, fives are still fives, and every puzzle is subtly wrong. The
  test `file_coordinates_are_x_column_y_row` pins the convention.
* **Line side markers are `1` = Black, `2` = White**, and the first
  move of a claimed line must be the attacker's.

Validation is strict about what the bitboard builder would forgive:
duplicate stones within a color and overlap between colors are checked
explicitly, because `Board::from_position` would silently collapse
them. The board itself is built with
`Board::from_position(&black, &white, attacker)`, which enforces the
Gomoku turn invariant (stone counts consistent with the attacker).

## Decision 3 — Unlabelled packs are not errors

The `l` field is optional: two of the three corpora are positions only
— their labels are milestone-4 generator output, where the TSS prover
produces the lines and `verify_line` filters them. An empty line is
therefore data, not a parse failure. `hint` and `reveal` on an
unlabelled puzzle return the typed `NoSolutionLine` error naming the
puzzle, so a front-end can grey out the commands instead of crashing.

## Decision 4 — Navigation clamps instead of wrapping

`next` at the last puzzle has two candidate designs: wrap around to the
first, or fail. Wrapping silently teleports you away from the puzzle
you were studying — and because every load is a fresh tree root, it
discards your variations without a word. Clamping fails loudly with the
typed `AtFirstPuzzle` / `AtLastPuzzle` errors and leaves the current
puzzle loaded; a shell can catch the error and grey out the button.

Each load replaces the variation tree wholesale
(`History::from_opening` of the new position, `search_info` cleared).
Seats and AI configuration are untouched: if White was an AI at 800
simulations on puzzle 4, it still is on puzzle 5. And `new` resets to
the puzzle the session *started* with, not the one you navigated to —
the `SessionInit` is the source of truth for resets.

## Decision 5 — The session keeps the whole pack

`SessionInit::puzzle` takes the *parsed* pack, not a path. The core
never touches the filesystem (core/shell doctrine): `main.rs` and
`sessiondemo` read the file once at startup, and `next` / `prev` need
no further I/O. The pack path is kept in `PuzzleStudy` only for display
and error messages.

## Decision 6 — Startup: one more initial seat assignment

`gomoku play --puzzle pack.json --index 4` is the `play` flow with a
different opening board. Swap2 is skipped — the puzzle position *is*
the opening — and the `--ai` / `--color` / sims flags compose exactly as
they do for a Swap2 opening. A missing or invalid pack exits with code
2 (usage/data error), names the path, and points at
`data/puzzles/fetch.sh`, because the corpora are git-ignored and "not
found" most likely means "not fetched yet". The `selfplay` subcommand
rejects `--puzzle`: a game generator has no business starting
mid-position.

## Decision 7 — Fixture discipline

Tests load only the committed `crates/cli/tests/fixtures/mini-pack.json`
with `include_str!`, never the git-ignored corpora under `data/`. The
fixture holds three hand-verified puzzles: (1) Black to move, a
vertical four on column h with the one-move line `h8`; (2) White to
move, a horizontal four on row 5 with the one-move line `i5`; (3) a
three-ply line. Every stone list satisfies the turn invariant for its
attacker, and the extra `u` / `f` fields prove the schema ignores
unknown keys.

## References

* [07-positions-from-files-puzzles.md](../07-positions-from-files-puzzles.md)
  — the chapter overview.
* [Playable CLI design paper](../../../specs/2026-10-17-playable-cli-design.md)
  — the "positions from files" amendment.
* [data/puzzles/README.md](../../../../data/puzzles/README.md) —
  corpus format, provenance, milestone-4 labelling plan.
