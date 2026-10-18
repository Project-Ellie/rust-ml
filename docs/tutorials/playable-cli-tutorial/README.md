# Playable CLI tutorial — the study table

The capstone tutorial of milestone 3's hand-written era. The `cli`
crate grows from a human-vs-human board into a **study table**: two
seats, each occupied by a human or an AI at any moment, on top of a
persistent variation tree. Play against the search, watch it play
itself, steal its seat mid-game, branch off into a variation, navigate
back, and toggle an overlay to **see what the AI thinks**.

No trained network anywhere: the AI is the mcts crate's
`TacticsEvaluator` behind the `Evaluator` seam. The baseline
tutorial's evaluator — and later, trained nets — enroll into the same
seats with zero redesign.

**Design authority:** the [playable CLI design
spec](../../specs/2026-10-17-playable-cli-design.md). **Plan:**
[2026-10-17-playable-cli.md](../../plans/2026-10-17-playable-cli.md).

## Prerequisite

The [mcts tutorial](../mcts-tutorial/README.md) is complete (the
`Evaluator` seam, `search`, `SearchConfig`, `visit_distribution`,
`select_move`, `add_dirichlet_noise`, `TacticsEvaluator`). The
convolutions and baseline tutorials are *not* required — this tutorial
is deliberately decoupled from them (spec Q1). It helps to know the
existing `cli` crate's hotseat flow, but every chapter quotes what you
need.

## How this tutorial works

Same conventions as the mcts, datagen, convolutions, and baseline
tutorials:

- **Read the primer first.** [`00-playable-cli-primer.md`](00-playable-cli-primer.md)
  pins the semantics: the study table, core/shell, commands and
  events, the four knobs, and why there is no depth knob. The chapters
  derive; the primer decides.
- **Chapters are small and each ends in a felt capstone** — a runnable
  reward. The *Intent* section of every chapter names the one thing
  you get, the thing worth fighting for.
- **Chapters give contracts, you write the implementation.** TDD
  checklists name the exact tests. Stuck for more than ~20 minutes?
  Each chapter has an opt-in `NN-name/01-solution.md` quoting the
  verified reference implementation verbatim. Using it is not failure;
  it's what it's for.
- **Gates per chapter:** `cargo test -p cli`,
  `cargo clippy --all-targets -- -D warnings`, and
  `cargo fmt --all -- --check`, all from `gomoku/`.
- **Commit after every chapter**, with the message the chapter names.

## Chapter map

| # | Chapter | The one thing you get |
|---|---------|------------------------|
| 00 | [Primer](00-playable-cli-primer.md) | The study table, core/shell, commands/events, the four knobs, no depth knob. |
| 01 | [The variation tree](01-the-variation-tree.md) | `historydemo` walks branches as ASCII boards: **now you can branch off and navigate back.** |
| 02 | [The command language](02-the-command-language.md) | `sessiondemo` REPL: **you play a whole game through the language a web server will one day speak.** |
| 03 | [The AI takes a seat](03-the-ai-takes-a-seat.md) | `release white` + `step` + `info`: **the AI sits down at your table — and shows its working.** |
| 04 | [The study loop](04-the-study-loop.md) | Take/release mid-game, per-seat sims, the opening schedule: **steal the AI's seat, explore, hand it back.** |
| 05 | [The real table](05-the-real-table.md) | `gomoku play --ai tactics` on the rendered board: **the study table gets its wood.** |
| 06 | [Seeing the mind](06-seeing-the-mind.md) | `p` cycles visits/prior heatmaps, `v` the value strip: **now you can see what the AI thinks.** |
| 07 | [Acceptance](07-acceptance.md) | Golden transcripts, a scripted game, a measured sims table: **proof, not vibes.** |

## Status

Written 2026-10-17 against the approved spec. The reference
implementation was built TDD-first in a disposable worktree
(`../rust-ml-ref-playable-cli`, branch `reference/playable-cli`), kept
for inspection until chapter 07 is worked; solution files quote it
byte-for-byte. Reference verification: 122 tests green in the crate
(one ignored measurement), workspace green, gates clean.
