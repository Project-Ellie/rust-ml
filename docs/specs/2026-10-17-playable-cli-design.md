# Playable CLI design — the study table: human and AI seats on a variation tree

**Status**: approved in design dialogue 2026-10-17 (questions Q1–Q9, answers
recorded inline). **Audience**: the tutorial author (the owner) and the
implementation agents. **Scope**: the `cli` crate of the gomoku workspace;
the next solution-backed tutorial after the baseline tutorial.

## Abstract

The terminal client grows from a human-vs-human board into a **study
table**: two seats, each independently occupied by a human or an AI, on
top of a persistent variation tree. Hotseat, play-against-the-AI, and
self-play cease to be modes — they are initial seat assignments, and any
seat can be taken or released at any time. Every position ever reached
stays reachable: branching is silent, navigation is explicit, and an AI
search can be requested at any cursor position.

The design's load-bearing decision is a **core/shell split** inside the
`cli` crate: a headless `SessionCore` library (commands in, events and
snapshots out, no terminal anything) and a thin crossterm front-end. The
same core can later serve a REST wrapper or feed a graphical front-end —
the [gomoku-gui](https://github.com/Project-Ellie/gomoku-gui) repository
validates exactly this architecture (`gomoku-core` vs `gomoku-gui`).

The AI is an `mcts::Evaluator` behind the existing seam. This tutorial
ships with the tactics-shaped evaluator that already exists; the baseline
tutorial's evaluator and eventually trained networks enroll as additional
seat occupants with zero redesign. There is no trained network in scope —
the capstone is playable the day it lands.

## Glossary

| Term | Definition |
|------|------------|
| **Study table** | The unified interaction model: two seats + a variation tree + AI search on demand. Replaces the hotseat/AI/self-play mode distinction. |
| **Seat** | One of the two colors (Black, White). Occupied by a human or an AI at any moment; changeable at any time. |
| **Occupant** | `Human` or `Ai(AiConfig)` — who is responsible for producing a seat's moves. |
| **Variation tree** | The persistent game history: an arena of position nodes with a cursor; playing a move at an old position adds a child branch without deleting siblings. |
| **Cursor** | The currently displayed position in the variation tree. All commands act at the cursor. |
| **SessionCore** | The headless library: `execute(Command) -> Result<Vec<Event>>` plus `snapshot()`. No crossterm, no stdout, no clock. |
| **Command** | A single intent (`Play`, `Step`, `Back`, `Take`, …). The only way to change core state. The shared vocabulary of all front-ends. |
| **Event** | A fact about what changed (`Moved`, `Branched`, `GameOver`, …), returned from `execute`. Front-ends render events; they never re-derive state. |
| **Step** | The command that lets the side-to-move's AI occupant search and play one move at the cursor. Human occupants move via `Play`. |
| **SearchInfo** | The record of one AI search: visit distribution, root value, prior policy. Stored on the node it produced, reviewable when navigating history. |
| **Policy overlay** | A per-cell heatmap over the board: visit counts or prior probabilities from a `SearchInfo`. Cycles off → visits → prior. |
| **Value strip** | A status-line readout: root value, total visits, top-3 moves by visits. |
| **`[experiment]`** | A documented, replaceable hyper-parameter — not ordained, not learned. |

## 1. Context

The `cli` crate today is a human-vs-human client: a `dyn GameBoard` trait,
line-based input, an alternate-screen crossterm UI, a Swap2 hotseat flow,
a TSS overlay (`t` key), and a puzzle mode. Hand-rolled argument parsing;
dependencies are `crossterm`, `engine`, `serde`, `serde_json`, `thiserror`.

The `mcts` crate is complete and exposes exactly what an AI seat needs:

```rust
pub trait Evaluator { fn evaluate(&mut self, req: EvalRequest) -> EvalResult; }
pub fn search(root: &Board, ev: &mut dyn Evaluator, cfg: &SearchConfig) -> SearchOutcome;
pub struct SearchConfig { pub simulations: u32, pub c_puct: f32 } // 400 / 1.5
pub fn visit_distribution(tree: &Tree, root: NodeId) -> Vec<(Move, u32)>;
pub fn select_move(dist: &[(Move, u32)], temperature: f32, rng: &mut impl Rng) -> Option<Move>;
pub fn add_dirichlet_noise(tree: &mut Tree, root: NodeId, eps: f32, alpha: f32, rng: &mut impl Rng);
pub struct TacticsEvaluator; // exists today, Burn-free
```

Documented constraints this design respects without exception:

- `engine` is a dependency island (no Burn, no I/O, no `rand`). `mcts`
  must not depend on `net`/`patterns`/`baseline`. The `Evaluator` trait
  is the only seam (docs/12 §6).
- No tokio (docs/12 §5). The design is synchronous; OS threads +
  crossbeam remain available for a future non-blocking driver without
  contract changes.
- `clap`-based subcommand CLI is the planned direction (docs/12).
- The baseline tutorial's chapter 4 plans a one-off `--versus baseline`
  mode; this design **absorbs and generalizes** it (see §13).

The owner will work this as a solution-backed tutorial (same convention
as convolutions/baseline: contracts in chapters, verified reference
solutions opt-in).

## 2. Goals and non-goals

### Goals

1. **Playable capstone**: start the binary, play against the AI, or watch
   AI-vs-AI, with Swap2 default-on for human games.
2. **Study workflow**: full persistent history; silent branching; explicit
   variation choice at branch points; take/release either seat at any
   time; run an AI search at any cursor position.
3. **Interpretability**: toggleable policy overlay (visits / prior) and a
   value strip — see what the search believes, move by move.
4. **Tunable search**: `simulations`, `c_puct`, `temperature`, Dirichlet
   noise as startup flags; per-seat `simulations` overridable at runtime.
5. **Front-end isolation**: a headless `SessionCore` that a REST server or
   a GUI could drive unchanged.
6. **Headless testability**: every behavior above is tested through
   command sequences, no terminal involved.

### Non-goals

- **No trained network.** The capstone ships on `TacticsEvaluator`.
  Baseline and net evaluators are later enrollments into an existing seat.
- **No game-record save/load.** The versioned record format from
  gomoku-gui is the prior art when this becomes a goal.
- **No raw-key input.** Line commands only; one input system.
- **No time-based search budgets.** Simulation counts are deterministic
  and reproducible; milliseconds are not.
- **No game export / datagen integration.** Store-games self-play is the
  future selfplay service's concern (docs/12 §9).
- **No web/REST server, no GUI.** The core is *ready* for them; building
  them is out of scope.

## 3. The study table — one model, no modes

The central simplification: **there are no play modes.** There is a table
with two seats and a history tree.

- Each seat holds an **occupant**: `Human` or `Ai(AiConfig)`.
- Startup flags choose the *initial* occupants; `take`/`release` commands
  change them at any time, mid-game, mid-variation.
- Hotseat = (Human, Human). "AI mode" = (Human, Ai). "Self-play" =
  (Ai, Ai) with step-driven pacing. All identical machinery.
- Whose turn it is and who occupies that seat determines what the core
  waits for: a `Play` command (human) or a `Step` command (AI).

The teaching consequence the tutorial must make explicit: the user stops
thinking in "modes" and starts thinking in "seats" — which is also exactly
how the future selfplay service and arena will think (docs/12 §9's
phases are seat assignments at scale).

## 4. Crate topology — core/shell inside `cli`

The `cli` crate gains a library target; the binary becomes a front-end:

```text
cli/
├── Cargo.toml          [lib] + [[bin]]; new deps: mcts, clap (derive)
├── src/
│   ├── lib.rs          pub mod session, actor, history, command
│   ├── session.rs      SessionCore — the state machine (§5)
│   ├── actor.rs        Occupant, AiConfig, seat table (§6)
│   ├── history.rs      Variation tree + cursor (§7)
│   ├── command.rs      Command, Event, parser, CommandError (§5)
│   ├── main.rs         clap CLI, front-end assembly
│   ├── tui/            (modules move here mostly unchanged)
│   │   ├── board.rs    dyn GameBoard (unchanged)
│   │   ├── render.rs   + star points, last-move marker, side panel
│   │   ├── input.rs    line parsing → command::parse
│   │   ├── overlay.rs  policy overlay, value strip (§9)
│   │   ├── opening.rs  Swap2 flow (unchanged)
│   │   └── ui.rs       the loop: read line → Command → Events → paint
│   └── …
```

Dependency rules: the lib (`session`/`actor`/`history`/`command`)
depends on `engine` + `mcts` + `serde` only — **no crossterm, no std::io
beyond `Debug`/`Display` impls**. The bin adds `crossterm` and `clap`.
`baseline` will join later as an opt-in cargo feature providing one more
`AiConfig` evaluator variant; the lib never *requires* it.

docs/12 §6 check: `cli` already sits at the top of the dependency graph;
adding `mcts` changes nothing for `engine` (still an island) or `mcts`
(still Burn-free, still unaware of `cli`).

## 5. The session core

### 5.1 The contract

```rust
pub struct SessionCore { /* seats, tree, cursor, configs, rng */ }

impl SessionCore {
    pub fn new(init: SessionInit) -> Self;
    pub fn execute(&mut self, cmd: Command) -> Result<Vec<Event>, CommandError>;
    pub fn snapshot(&self) -> Snapshot;          // read-only view for renderers
    pub fn board(&self) -> Board;                // replayed to cursor
    pub fn search_info(&self) -> Option<&SearchInfo>; // at cursor, if any
}
```

`SessionInit` records the initial occupants, the four search knobs, the
per-seat sims overrides, whether Swap2 preceded the session, and the
opening placements (Swap2 output becomes the tree root — see §10).

`Snapshot` is a serde-serializable, `Clone`able read model: cursor path,
seat table, status at cursor, branch options at cursor, last events.
It exists so that *any* front-end (TUI today, REST tomorrow) renders
from data, never from privileged access.

### 5.2 Commands

```rust
pub enum Command {
    Play(Move),                  // human seat plays at cursor
    Step,                        // AI seat-to-move searches and plays
    Back { n: u32 },             // cursor toward root
    Forward { n: u32 },          // cursor toward leaf (default child)
    Var { index: usize },        // choose child at a branch point
    Take { seat: SeatSel },      // seat becomes Human
    Release { seat: SeatSel },   // seat becomes Ai(current defaults)
    SetSims { seat: SeatSel, n: u32 },
    New,                         // fresh game (front-end reruns Swap2)
    Help,
    Quit,
}
pub enum SeatSel { Black, White, Both }
```

Parse errors and illegal commands (e.g. `Play` on an AI seat's turn,
`Forward` at a leaf, `Var` at a non-branch) are `CommandError` values —
never panics, never silent no-ops. The core never prints; errors are
data.

### 5.3 Events

```rust
pub enum Event {
    Moved { mv: Move, by: OccupantKind, search: Option<SearchInfo> },
    Branched { at: NodeId, options: Vec<Move> },
    CursorMoved { to: NodeId, ply: u32 },
    BranchPoint { options: Vec<(usize, Move)> },   // arriving via Forward
    SeatChanged { seat: Color, now: OccupantKind },
    ConfigChanged { seat: Color, sims: u32 },
    GameOver { result: Status },
    Reset,
}
```

Front-ends translate events into pixels or JSON. Nothing else changes
state; nothing else is needed to render.

### 5.4 The parser is part of the shared vocabulary

`command::parse(&str) -> Result<Command, CommandError>` parses the
line-based textual form (`play h8`, `back 3`, `var 2`, `take black`,
`sims white 2000`, …). The TUI feeds raw lines to it; a REST front-end
would construct `Command` directly. Text parsing is a convenience layer
*inside the lib*, tested headless.

## 6. Actors and seats

```rust
pub enum Occupant { Human, Ai(AiConfig) }
pub struct AiConfig {
    pub evaluator: EvaluatorKind,   // Tactics today; Baseline/Net later
    pub simulations: u32,
    pub c_puct: f32,
    pub temperature: f32,
    pub noise_epsilon: f32,         // 0.0 disables Dirichlet noise
}
```

- `EvaluatorKind` is an enum (not a trait object in config) so
  `AiConfig` stays `Clone + Debug + serde`; the concrete
  `Box<dyn Evaluator>` is constructed lazily per search or cached per
  seat inside the core — an implementation detail, decided in the
  tutorial. (`TacticsEvaluator` is `Copy`; no cache needed today.)
- `Step` at a cursor: rebuild `Board`, run `mcts::search` with the
  seat's config, `select_move(visits, temperature, rng)`, append child
  node, store `SearchInfo` on it. Synchronous, on the caller's thread.
  The contract returns results, never streams progress, so a future
  crossbeam worker is a local change (Q3 decision).
- **Self-play variety**: with both seats AI and `temperature 0`, every
  game is identical. The session defaults (applied when a seat is
  `Release`d or the table starts AI-vs-AI without explicit flags) use
  the AlphaZero opening schedule `[experiment]`: temperature 1.0 +
  Dirichlet noise (ε = 0.25, α = 0.3) for the first 10 plies of each
  line, then argmax. Explicit flags always win over defaults.
- The RNG lives in the core, seeded from a startup flag (`--seed`) for
  reproducible sessions in tests and demos; entropy-seeded by default.

### 6.1 There is no depth knob — teach it

MCTS depth is emergent: the tree grows as deep as the simulations push
it. The tutorial dedicates a sidebar to this, demonstrating tree depth
as a function of `--sims` on a fixed position. The tunable surface is
exactly four knobs (§8 startup flags, `SetSims` runtime command in §5.2).

## 7. The variation tree

```rust
pub struct History { arena: Vec<Node>, root: NodeId, cursor: NodeId }
struct Node {
    parent: Option<NodeId>,
    mv: Option<Move>,            // None at root
    children: Vec<NodeId>,
    search: Option<SearchInfo>,  // set when an AI search produced this node
}
```

- Arena + indices, the in-repo idiom (`mcts::Tree` is the precedent).
- `Play`/`Step` at the cursor: if an identical child move exists, the
  cursor *moves to it* (no duplicate branches); otherwise a new child
  is appended — **silently, no confirmation** (owner requirement).
- `Forward` at a single child follows it; at a branch point it emits
  `BranchPoint { options }` and waits for `Var { index }` (or
  `Forward { n }` with an explicit choice). The status area always shows
  branch options when the cursor has multiple children.
- `board()` replays root→cursor; ≤ 225 plays, microseconds on the fast
  engine — the CLI already uses replay-from-log for undo today.
- The Swap2 opening placements are the root: history starts at the
  first normal-play position. Rolling back into the opening ceremony is
  impossible by construction (Q8).
- Ply numbers for display derive from path length, not stored.

## 8. Startup interface (clap)

Two subcommands, both merely initial seat assignments:

```text
gomoku play [--engine fast|naive] [--no-swap2] [--plain]
            [--ai tactics] [--color black|white]
            [--sims N] [--c-puct F] [--temperature F] [--noise F]
            [--black-sims N] [--white-sims N] [--seed N]

gomoku selfplay [--sims N] [--c-puct F] [--temperature F] [--noise F]
                [--black-sims N] [--white-sims N] [--seed N]
```

- `play` defaults: both seats human, Swap2 on. `--ai tactics` puts the
  AI in the seat opposite `--color` (default: human Black, AI White).
- `selfplay`: both seats AI, `--no-swap2` implied (the temperature
  schedule provides variety; Swap2 is a human ritual), step-driven.
- `--engine naive` is rejected when any seat is AI (the AI needs the
  fast board) — startup error, exit code 2 (usage), matching the
  patterndemo conventions.
- The legacy flags (`--puzzle`, `--index`) stay on `play` untouched;
  puzzle mode is a separate flow, not part of the study table.

## 9. Overlays and the value strip (front-end)

Overlay *state* is a shell concern (it's visual); overlay *data* comes
from the core (`search_info()` at the cursor, plus stored `SearchInfo`
on visited nodes).

- **`p` cycles the policy overlay**: off → visit counts → prior → off.
  Visits render as an ANSI-256 heatmap (intensity = share of total
  visits) with the raw count in the hot cells; prior renders the
  evaluator's post-softmax policy the same way. If the cursor has no
  `SearchInfo`, the overlay shows nothing and the status line says so.
- **`v` toggles the value strip**: root value (side-to-move
  perspective, e.g. `value +0.42 for Black`), total visits, top-3 moves
  with shares. Composes with everything; lives in the side panel.
- **Mutual exclusion of cell overlays**: TSS (`t`) and policy (`p`) both
  paint cells; activating one clears the other. Status-line/panel
  elements compose freely.
- In AI-vs-AI stepping, the overlay refreshes from each new node's
  stored `SearchInfo`; navigating history shows the search that *was*
  run there — the study value of storing it per node.

## 10. The opening

Swap2 remains exactly as implemented (party switches, replay-log undo,
`Board::from_position` handoff) and remains default-on for `play`.
AI seats spectate during the ceremony and take over when normal play
begins — the study table starts at the position the opening produced.
`selfplay` implies `--no-swap2`. The tutorial does not touch
`opening.rs` except to pass its result into `SessionInit`.

## 11. Error handling

- Lib: `thiserror`-derived `CommandError` (parse, illegal-state,
  engine-illegal-move variants), no `unwrap` outside tests. The lib
  returns `Result` for everything a front-end can cause.
- Bin: `anyhow` at `main`, clap parse failures exit 2, runtime failures
  exit 1 (patterndemo convention).
- The AI cannot produce an illegal move by construction (`select_move`
  draws from the legal visit distribution); a violation is a bug and
  may panic — documented as such.

## 12. Testing strategy

- **History**: unit tests for branch/merge-by-identity/navigation;
  a proptest that random command walks keep `board()` consistent with a
  reference replay.
- **Session**: headless integration tests as command scripts — take a
  seat mid-game, branch, search at an old node, per-seat sims, opening
  schedule applied only without explicit flags, error cases as data.
- **Parser**: table-driven unit tests; every documented line form
  round-trips.
- **Seeded determinism**: `--seed` makes full sessions reproducible;
  one golden-transcript test drives a scripted session and compares
  events (not ANSI output).
- **Shell**: rendering functions return strings (the `render_heatmap`
  precedent) and are snapshot-tested lightly; the event loop itself is
  thin and manually verified in the acceptance chapter.
- Gates per chapter, from `gomoku/`: `cargo test -p cli`,
  `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.

## 13. Relationship to the baseline tutorial

Baseline chapter 4's planned `--versus baseline` mode is **superseded**
by this design: the baseline evaluator becomes `EvaluatorKind::Baseline`
behind a cargo feature, enrollable into any seat with all four knobs and
full study-table semantics. When the baseline tutorial is worked
(ordering is free — the tracks are decoupled, Q1 decision), its chapter
4 gets amended to plug into the actor system instead of building a
one-off mode. This spec is the amendment's authority.

## 14. Forward compatibility (not built now)

- **REST server**: wrap `SessionCore` in a handler; `Command` as JSON
  in, `Vec<Event>` + `Snapshot` out. The core's serde-derive configs and
  read model exist for this. One session per table; concurrency is the
  server's problem, not the core's.
- **Trained net seats**: `EvaluatorKind::Net` + a record path in
  `AiConfig`; the seat abstraction already carries it.
- **Non-blocking search**: replace the synchronous body of `Step` with a
  crossbeam worker; commands/events unchanged.
- **gomoku-gui**: a separate repository with a vendored engine and its
  own core. Unification (one engine, one core, two shells) is a real
  future project; this design's core/shell split is deliberately
  congruent with it.

## 15. Tutorial structure (sketch — refined in the plan)

Solution-backed, primer + chapters, capstone = the playable CLI:

0. **Primer** — the study table, core/shell, commands/events, the four
   knobs, why there is no depth parameter.
1. **Crate split** — lib target, `history` arena, replay-to-cursor.
2. **Commands and session** — `Command`/`Event`/`SessionCore`, parser,
   headless tests.
3. **Seats and the AI** — `Occupant`, `AiConfig`, `Step` wiring MCTS,
   `SearchInfo`, seeded RNG.
4. **The study loop** — take/release, silent branching, `Var`, the
   opening schedule.
5. **The front-end** — clap subcommands, side panel, star points,
   last-move marker, event-driven repaint.
6. **Overlays** — policy cycle, value strip, TSS mutual exclusion.
7. **Acceptance** — scripted session transcripts, a played game against
   the tactics evaluator, measured sims-vs-strength demo.

## 16. Risks

- **Scope creep in the shell** (panels, colors, markers). Mitigation:
  the shell chapters come after a fully tested core; acceptance gates
  on behavior, not pixels.
- **`GameBoard` duplication**: the study table uses `engine::Board`
  directly (AI needs it), leaving `dyn GameBoard` for puzzle/naive
  flows. Two board paths in one binary is a wart, accepted deliberately:
  the study table is fast-engine-only, and the rejection error is a
  startup check (§8).
- **SearchInfo staleness**: a stored `SearchInfo` reflects the config at
  search time; the value strip must say so when configs changed since.
  Cheap: store the config fingerprint next to it.
- **Line-input study friction**: typing `play h8` per move is fine for
  study; raw-key board cursors were considered and rejected (non-goal).

## 17. Verify when reached

- [ ] `cargo test -p cli` green with the lib/bin split; old puzzle/TSS
      flows untouched.
- [ ] Headless transcript test: scripted study session (branch, take
      seat, search at old node) reproduces event-for-event under `--seed`.
- [ ] Baseline ch.4 amended to `EvaluatorKind::Baseline` when that
      tutorial is worked.
- [ ] WARM-UP and docs/12 status updated when the capstone lands.

## 18. Workflow and deliverables

1. This spec (done).
2. Implementation plan (`docs/plans/2026-10-17-playable-cli.md`) via the
   writing-plans skill.
3. Tutorial chapters under `docs/tutorials/playable-cli-tutorial/`,
   written per the plan, reference implementations verified in a
   disposable worktree per house convention, solutions quoted verbatim.
4. The owner works the chapters; gates per chapter as in §12.

## References

- `docs/12-gomoku-architecture.md` — crate topology (§6), no-tokio (§5),
  clap CLI direction, phased GPU scheduling (§9).
- `docs/tutorials/mcts-tutorial/` — the `Evaluator` seam, `SearchConfig`,
  `TacticsEvaluator` (ch.9).
- `docs/tutorials/baseline-tutorial/04-the-evaluator-adapter.md` — the
  `--versus baseline` plan this spec supersedes.
- `docs/specs/2026-10-13-baseline-player-design.md` — sibling spec,
  format precedent.
- `../gomoku-gui/docs/specs/2026-09-22-gomoku-gui-design.md` — core/shell
  architecture and record-format prior art.
- `gomoku/crates/patterns/src/bin/patterndemo.rs` — CLI conventions,
  exit codes, ANSI-heatmap idiom.
