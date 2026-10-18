# Playable CLI primer — the design authority

This primer pins the semantics of the study table. The chapters derive
implementations from it; when a chapter and this primer disagree, the
primer is wrong *only* if the spec says so — the ultimate authority is
the [design spec](../../specs/2026-10-17-playable-cli-design.md).

## The study table: there are no modes

The old CLI had one flow: humans take turns. The new one has a
**table** with two **seats** — Black and White — and each seat holds an
**occupant**: a `Human` or an `Ai`. That's the whole model.

- Hotseat is (Human, Human). "Play against the AI" is (Human, Ai).
  "Self-play" is (Ai, Ai). None of these are modes; they are *initial
  seat assignments*.
- You can **take** or **release** either seat at any time — mid-game,
  mid-variation. The table never resets when occupants change.
- Whose turn it is plus who occupies that seat determines what the
  table waits for: a `Play` command (human) or a `Step` command (AI).

Hold onto this when chapter 5 wires up `play` and `selfplay`
subcommands: they do nothing but choose initial occupants.

## Core/shell: the headless brain

The crate splits into a library and a thin binary:

```text
cli/
├── src/lib.rs
│   ├── history.rs    the variation tree (nodes + cursor)
│   ├── command.rs    Command, Event, parser, CommandError
│   ├── session.rs    SessionCore — the state machine
│   ├── actor.rs      Occupant, AiConfig, SearchInfo
│   └── fmt.rs        shared ASCII formatting helpers
├── src/tui/          the crossterm front-end (the shell)
└── src/bin/          gomoku, historydemo, sessiondemo
```

The library's entire contract:

```rust
impl SessionCore {
    pub fn new(init: SessionInit) -> Self;
    pub fn execute(&mut self, cmd: Command) -> Result<Vec<Event>, CommandError>;
    pub fn snapshot(&self) -> Snapshot;
    pub fn board(&self) -> Board;
}
```

**Commands** are the only way to change state. **Events** are the only
way to learn what changed. The core never prints, never reads a
terminal, never knows crossterm exists. This buys three things:

1. **Headless tests.** Every behavior in this tutorial is tested by
   feeding command sequences to the core and asserting event streams.
   No terminal mocking, ever.
2. **A shared vocabulary.** `command::parse` turns text lines
   (`play h8`, `back 3`, `var 2`, `take black`) into `Command`s. The
   terminal front-end feeds it lines today; a REST server would build
   `Command`s from JSON tomorrow. The core doesn't care.
3. **A thin shell.** The front-end renders `Snapshot`s and `Event`s.
   Overlay state (which heatmap is on) lives in the shell because it's
   visual; the *data* for overlays lives in the core.

## The variation tree: history you can't lose

Undo in the old CLI meant replaying a linear log. The study table keeps
a **tree**: every position ever reached stays reachable.

- A tree of nodes (the `mcts::Tree` idiom), a **cursor** pointing
  at the displayed position, and a root holding the opening board.
- `play` at the cursor: if that exact move already exists as a child,
  the cursor just *follows* it. Otherwise a new child is appended —
  **silently**. There is no "do you really want to branch?" because
  branching destroys nothing.
- `back` walks toward the root; `forward` walks toward a leaf,
  stopping at branch points; `var N` picks which child to explore.
- The board at the cursor is rebuilt by replaying root→cursor: at most
  225 plays, microseconds on the fast engine.
- Each node can carry a **`SearchInfo`** — the record of the AI search
  that produced it (visits, prior, value, config fingerprint).
  Navigating back to a position shows you the search that *was* run
  there. This is what makes the tree a study instrument rather than
  merely an undo buffer.

## The AI seat: search on demand, synchronously

An AI seat does nothing until the shell issues `Step`. Then, on the
calling thread: rebuild the board, run `mcts::search` with the seat's
config, pick a move with `mcts::select_move`, append the child, store
the `SearchInfo`. The contract returns a *result*, never streams
progress — so a future worker-thread driver (crossbeam, per docs/12
§5) is a local change, not a redesign.

The evaluator is `mcts::TacticsEvaluator`: it shapes priors from
engine truth (immediate wins, forced blocks) and has **no positional
sense at all**. On a quiet board it is deliberately, instructively
blind — every move looks the same. The tutorial will show you this
blindness in the demo output; the baseline tutorial exists to cure it.

### The four knobs — and why there is no fifth

| Knob | Meaning | Default |
|------|---------|---------|
| `simulations` | Playouts per move — width *and* depth | 400 |
| `c_puct` | Exploration vs exploitation | 1.5 |
| `temperature` | 0.0 = argmax; 1.0 = sample by visits | 0.0 |
| `noise_epsilon` | Dirichlet noise at the root (α = 0.3) | 0.0 |

**There is no depth knob.** MCTS is not minimax: the tree grows as
deep as the simulations push it, and depth is an *emergent* property
of simulation count. Chapter 3 has a sidebar demonstrating this on a
fixed position. If you reach for a depth parameter, what you actually
want is more simulations.

Per-seat `simulations` can diverge (`--black-sims 50 --white-sims
2000`, or `sims white 2000` at runtime) — strength-vs-width is one of
the cheapest experiments the table offers.

### The opening schedule `[experiment]`

AI-vs-AI from an empty board with temperature 0 produces the *same
game every time* — useless for study. So when a line starts AI-vs-AI
from an empty root **and** you haven't explicitly set temperature or
noise, the first 10 plies use temperature 1.0 + Dirichlet noise
(ε = 0.25), then argmax. Explicit flags always win. This is the
AlphaZero opening trick, hand-rolled, and labeled `[experiment]`:
replaceable, not ordained.

## The overlays: visits, prior, value

Chapter 6's payoffs, all sourced from the cursor node's `SearchInfo`:

- **`p` cycles the policy overlay**: off → **visits** (heatmap of the
  visit distribution — what the search concluded) → **prior** (heatmap
  of the evaluator's raw belief before search) → off.
- **`v` toggles the value strip** in the side panel: root value from
  the searching side's perspective, total visits, top-3 moves. A
  scalar, honestly displayed — per-cell values would need an evaluator
  call per child and are not what AlphaZero shows you anyway.
- Cell overlays are **mutually exclusive** (policy and the TSS overlay
  clear each other); the value strip composes with everything.

## Conventions

- Coordinates use **algebraic notation**: `h8` or `play h8` (column
  letter a–o, row number 1–15; the center is `h8`). Input is
  case-insensitive. The internal engine representation is still
  `Move { row, col }`; algebraic notation is a display and input
  mapping only.
- The Swap2 ceremony is unchanged and default-on for `play`; its
  result becomes the tree's root. `selfplay` skips it (the schedule
  provides variety instead).
- Seeded sessions (`--seed N`) are bit-reproducible — the acceptance
  chapter's golden transcript depends on it.
- Gates per chapter, from `gomoku/`: `cargo test -p cli`,
  `cargo clippy --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`.

## What this tutorial deliberately does not build

No trained network (later milestone). No game save/load (gomoku-gui's
record format is the prior art). No raw-key input (line commands
only). No time-based search budgets (sim counts are deterministic).
No REST server, no GUI — the core is *ready* for them; building them
is out of scope.

Next: [Chapter 01 — The variation tree](01-the-variation-tree.md)
