# Playable CLI tutorial — implementation plan

**Spec:** `docs/specs/2026-10-17-playable-cli-design.md` (approved by the
owner 2026-10-17, "I like it"). **Format precedent:** the convolutions
tutorial pipeline (`docs/plans/2026-09-27-convolutions-tutorial.md`).
**Owner amendment 2026-10-17:** chapters must be *small*, each ending in
a **felt capstone** — a visible, runnable reward ("now you can branch
off and navigate back", "now you can see what the AI thinks") — and every
chapter ships a complete opt-in solution.

## Context

The `cli` crate (binary `gomoku`) grows from human-vs-human into the
study table: two seats (human or AI, take/release at any time) on a
persistent variation tree, driven by a headless `SessionCore`
(commands in, events + snapshot out). Deliverables:

1. **Reference implementation** — `gomoku/crates/cli` evolved in a
   disposable worktree (`/Users/wgiersche/workspace/Project-Ellie/rust-ml-ref-playable-cli`,
   branch `reference/playable-cli`), deleted after final verification.
2. **Tutorial** — `docs/tutorials/playable-cli-tutorial/` on `main`:
   `README.md`, `00-playable-cli-primer.md`, chapters `01`–`07`,
   per-chapter opt-in `NN-name/01-solution.md` files quoting the
   verified reference verbatim.

The reference build is *evolutionary*: the existing crate (hotseat,
Swap2, TSS overlay, puzzle mode) keeps working at every task boundary.

## Hard rules for every slice

- Gomoku workspace pins Burn `=0.22.0`; the `cli` crate stays Burn-free
  (no Burn dependency at all — the AI is `mcts::TacticsEvaluator`).
- `engine` untouched (dependency island). `mcts` untouched. No new
  features on either.
- No edits to `docs/WARM-UP.md`, `AGENTS.md`, or other tutorials
  (status updates are the controller's conversation with the owner).
  Exception: nothing in this plan touches them at all.
- No pushes anywhere; commits are local, in the worktree.
- Never weaken or delete a test to make it pass.
- Solution files quote the reference byte-for-byte (extract script +
  `diff` verification per file).
- Gates per code slice, run from `gomoku/` in the worktree:
  `cargo test -p cli`, `cargo clippy --all-targets -- -D warnings`,
  `cargo fmt --all -- --check` — plus `cargo build --workspace` to
  prove the other crates are unaffected.
- Subagent pipeline per slice: implement (TDD, red-green-refactor) →
  controller verifies gates → spec-compliance review pass → bounded
  amendments. Controller triages all findings.
- Coordinate format stays numeric `7 7` (continuity with the existing
  `input.rs`); the command language also accepts `play 7 7`.

## Chapter map — small steps, felt capstones

| # | Chapter | The one thing you get (capstone) |
|---|---------|----------------------------------|
| 00 | primer | The study table, core/shell, commands/events, the four knobs, why there is no depth knob. |
| 01 | The history arena | `historydemo` prints a scripted branch walk as ASCII boards: **now you can branch off and navigate back.** |
| 02 | The command language | `sessiondemo` REPL: play a full hotseat game typing commands: **you play a whole game through the language a web server will one day speak.** |
| 03 | The AI takes a seat | `release white` + `step` in the REPL; `info` shows visits and value: **the AI sits down at your table — and shows its working.** |
| 04 | The study loop | Take/release mid-game, per-seat sims, AI-vs-AI opening schedule: **you can steal the AI's seat, explore a variation, and hand it back.** |
| 05 | The real table | `cargo run -p cli -- play --ai tactics` on the rendered board with side panel: **the study table gets its wood.** |
| 06 | Seeing the mind | `p` cycles visits/prior heatmaps, `v` the value strip: **now you can see what the AI thinks.** |
| 07 | Acceptance | Golden transcripts, a scripted study session, measured sims-vs-strength table: **proof, not vibes.** |

## Task 0: worktree, crate split, clap skeleton

**Files:** worktree setup; `gomoku/crates/cli/Cargo.toml` (add `[lib]`,
`clap` workspace dep with `derive`); `src/lib.rs` (new); move the
existing UI modules under `src/tui/` (`board.rs`, `input.rs`,
`opening.rs`, `puzzle.rs`, `render.rs`, `ui.rs`); `src/main.rs` becomes
clap parsing + dispatch to the existing flows; `src/history.rs`,
`src/command.rs`, `src/session.rs`, `src/actor.rs` created empty with
module docs.

- `clap = { workspace = true, features = ["derive"] }` — add to
  workspace `Cargo.toml` (clap 4) if not already present.
- Subcommands: `Play(PlayArgs)`, `Selfplay(SelfplayArgs)`; `play` is
  the default when no subcommand is given (legacy invocation
  `gomoku --engine fast` keeps working). Legacy flags map onto
  `PlayArgs`: `--engine`, `--plain`, `--no-swap2`, `--puzzle`, `--index`.
- New flags on both subcommands (parsed, validated, unused until T3):
  `--ai tactics`, `--color black|white`, `--sims N` (default 400),
  `--c-puct F` (default 1.5), `--temperature F` (default 0.0),
  `--noise F` (default 0.0), `--black-sims N`, `--white-sims N`,
  `--seed N` (u64). `selfplay` implies `--no-swap2`; `--engine naive`
  combined with `--ai` or `selfplay` is a startup error (exit 2).
- Gate: the crate builds and every existing test passes with the
  modules moved; `gomoku play --engine fast` behaves exactly as before.
- Commit: `refactor(cli): lib/bin split + clap subcommands`.

## Task 1 (chapter 01): the history arena

**Files:** `src/history.rs`; `src/bin/historydemo.rs`;
`tests/history.rs`.

Contracts:

```rust
pub struct History { /* arena, root, cursor */ }
pub type NodeId = u32;
impl History {
    pub fn new() -> Self;                       // empty-board root
    pub fn from_opening(b: &Board) -> Self;     // Swap2 result as root
    pub fn cursor(&self) -> NodeId;
    pub fn board(&self) -> Board;               // replay root→cursor
    pub fn play(&mut self, mv: Move) -> PlayOutcome;
    pub fn back(&mut self, n: u32) -> u32;      // plies actually moved
    pub fn forward(&mut self, n: u32) -> ForwardOutcome;
    pub fn var(&mut self, index: usize) -> Result<Move, HistoryError>;
    pub fn children(&self, at: NodeId) -> Vec<Move>;
    pub fn ply(&self) -> u32;                   // cursor depth
}
pub enum PlayOutcome { Moved(NodeId), FollowedExisting(NodeId) }
pub enum ForwardOutcome { Moved(u32), NeedsChoice(Vec<Move>), AtLeaf }
```

- `play` at the cursor: if an identical child move exists, move the
  cursor to it (`FollowedExisting`); else append a child (`Moved`).
  **Silent branching — never a confirmation.**
- `forward` stops and reports `NeedsChoice` at a branch point; `var`
  picks a child by index into `children(cursor)`.
- `board()` replays from the root each call (≤225 plays; the existing
  CLI already uses replay-from-log for undo).
- Tests: branch/identity/navigation units; a proptest doing random
  command walks asserting `board()` equals a naive reference replay of
  the cursor path.
- `historydemo`: scripted scenario (main line, `back 3`, branch,
  `forward`, `var`, second branch) printing a compact ASCII board per
  step with the variation path. This is the chapter-01 capstone; its
  output is quoted in the chapter.
- Commit: `feat(cli): variation-tree history arena + historydemo`.

## Task 2 (chapter 02): the command language and the session core

**Files:** `src/command.rs`, `src/session.rs`; `src/bin/sessiondemo.rs`;
`tests/session.rs`.

Contracts (no AI yet — seats are both `Human`):

```rust
pub enum Command { Play(Move), Step, Back{n:u32}, Forward{n:u32},
    Var{index:usize}, Take{seat:SeatSel}, Release{seat:SeatSel},
    SetSims{seat:SeatSel, n:u32}, New, Help, Quit }
pub enum SeatSel { Black, White, Both }
pub enum Event { Moved{mv:Move, by:OccupantKind}, Branched{options:Vec<Move>},
    CursorMoved{ply:u32}, BranchPoint{options:Vec<(usize,Move)>},
    SeatChanged{seat:Color, now:OccupantKind},
    ConfigChanged{seat:Color, sims:u32}, GameOver{result:Status}, Reset }
pub struct SessionCore { /* history, seats, configs, rng */ }
impl SessionCore {
    pub fn new(init: SessionInit) -> Self;
    pub fn execute(&mut self, cmd: Command) -> Result<Vec<Event>, CommandError>;
    pub fn snapshot(&self) -> Snapshot;
    pub fn board(&self) -> Board;
}
pub fn parse(input: &str) -> Result<Command, CommandError>;
```

- `parse` line forms: `7 7` / `play 7 7`, `step`, `back [n]`,
  `forward [n]`, `var N`, `take black|white|both`,
  `release black|white|both`, `sims black|white N`, `new`, `h`,
  `q`. Table-driven parser tests; every documented form round-trips.
- `Step`/`Release`/`SetSims` parse and validate now; executing `Step`
  with both seats human returns a typed `CommandError` (the AI arrives
  in T3).
- `Snapshot`: serde-serializable read model (cursor ply, branch
  options, seat table, status, last move).
- `sessiondemo`: stdin REPL — read line, `parse`, `execute`, print
  events + ASCII board. Hotseat game playable end-to-end: the
  chapter-02 capstone.
- Tests: scripted command sequences headless (branch mid-game, navigate,
  resume); error cases return typed errors, never panic.
- Commit: `feat(cli): command language + SessionCore + sessiondemo`.

## Task 3 (chapter 03): the AI takes a seat

**Files:** `src/actor.rs`; `src/session.rs` (extend);
`src/bin/sessiondemo.rs` (add `info`); `tests/actor.rs`.

Contracts:

```rust
pub enum Occupant { Human, Ai(AiConfig) }
pub enum OccupantKind { Human, Ai }
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AiConfig { pub evaluator: EvaluatorKind, pub simulations: u32,
    pub c_puct: f32, pub temperature: f32, pub noise_epsilon: f32 }
pub enum EvaluatorKind { Tactics }   // Baseline/Net later
pub struct SearchInfo { pub visits: Vec<(Move, u32)>,
    pub prior: Vec<(Move, f32)>, pub value: f32,
    pub config: AiConfig }           // fingerprint at search time
```

- `Release` sets a seat to `Ai(config)`; config comes from the
  session defaults (startup flags) with per-seat sims overrides.
- `execute(Step)` with the side-to-move seat AI: rebuild `board()`,
  run `mcts::search` with the seat's config, `select_move` at the
  seat's temperature, append the child, store `SearchInfo` on the node,
  emit `Moved{by: Ai, ..}` (+ `GameOver` when terminal).
- `SearchInfo.value` is the root position's estimated value **from the
  side-to-move's perspective** at search time; the chapter derives the
  sign from the backup convention (negate-once rule) instead of
  guessing. `prior` is the root edges' `(P, move)` after softmax.
- RNG: `StdRng` in the core, seeded from `SessionInit.seed`
  (`Option<u64>`; entropy when `None`). Seeded sessions are
  bit-reproducible — one test asserts two same-seed sessions produce
  identical event streams.
- `sessiondemo` gains `info`: prints visits, top moves, value from
  `snapshot()` at the cursor. Capstone: `release white`, play, `step`,
  `info` — the AI answers and shows its working.
- Tests: tactics evaluator takes the obvious win-in-1 via `step`;
  forced block; seeded determinism; `Step` on a human seat's turn is a
  typed error.
- Commit: `feat(cli): AI seats via mcts + SearchInfo + seeded RNG`.

## Task 4 (chapter 04): the study loop

**Files:** `src/session.rs` (extend), `src/actor.rs` (extend);
`tests/study.rs`.

- `Take` mid-game (either seat, also mid-variation); `Take Both` =
  hotseat; `Release Both` = AI-vs-AI from here.
- `SetSims` per seat at runtime; `ConfigChanged` events.
- **Opening schedule `[experiment]`**: when a line starts AI-vs-AI from
  an empty root *and* the session has no explicit `--temperature`/
  `--noise` flags, the first 10 plies of the line use temperature 1.0
  and Dirichlet noise (ε = 0.25, α = 0.3, the mcts crate's
  `add_dirichlet_noise`); then argmax. Explicit flags always win.
  Schedule state lives in the core; documented as replaceable.
- Tests: scripted study transcript — AI-vs-AI for 6 plies, `take
  black`, human branch, `back`, `var`, `release black`, AI resumes —
  asserting event sequences; schedule applies only without explicit
  flags; per-seat sims actually reach the search (different visit
  totals at 50 vs 400).
- Commit: `feat(cli): take/release seats, per-seat sims, opening schedule`.

## Task 5 (chapter 05): the real table

**Files:** `src/main.rs` (wire subcommands), `src/tui/study.rs` (new:
the study-table screen), `src/tui/render.rs` (extend: side panel, star
points, last-move marker), `src/tui/input.rs` (delegate to
`command::parse`); `tests/` unchanged (shell is thin).

- `study.rs` drives `SessionCore`: read line → `command::parse` →
  `execute` → paint events. Enter on an empty line = `Step` when the
  side-to-move seat is AI, ignored otherwise.
- Swap2 ceremony unchanged; its result becomes
  `SessionInit::from_opening`. `--no-swap2` → empty root. `selfplay`:
  both seats AI + implied `--no-swap2` + `--seed` honored.
- Board rendering: star points (hoshi: rows/cols 3, 7, 11), last-move
  marker, colored coordinate gutter, right side panel (seat table with
  configs, ply + variation path, branch options when present, last
  events tail, key hints). Rendering functions return `String`
  (render_heatmap precedent) and get light snapshot tests.
- Existing flows untouched: `--puzzle`, `--plain`, naive hotseat all
  keep working; their tests stay green.
- Capstone: `cargo run -p cli -- play --ai tactics` — play on the real
  board; `cargo run -p cli -- selfplay --sims 200 --seed 7` — step
  through AI-vs-AI with Enter.
- Commit: `feat(cli): study-table TUI with side panel + clap subcommands`.

## Task 6 (chapter 06): seeing the mind

**Files:** `src/tui/overlay.rs` (new), `src/tui/study.rs` (extend),
`src/tui/render.rs` (extend).

- `p` cycles the policy overlay: off → visit counts → prior → off,
  sourced from the cursor node's `SearchInfo` (absent → status hint
  "no search here yet"). ANSI-256 heatmap, intensity = share; raw
  count in cells ≥ 10% (patterndemo idiom).
- `v` toggles the value strip in the side panel: root value
  (side-to-move perspective), total visits, top-3 moves with shares;
  shows the config fingerprint's sims when configs changed since the
  search (staleness note from spec §16).
- Cell overlays are mutually exclusive: activating `p` clears the TSS
  overlay and vice versa; the value strip composes with everything.
- Capstone: mid-game, `p` — the board lights up with what the search
  believes; `v` — the strip quantifies it.
- Commit: `feat(cli): policy overlay + value strip`.

## Task 7 (chapter 07): acceptance

**Files:** `tests/acceptance.rs`; `docs/` updates (controller).

- Golden transcript: seeded (`--seed 42`) scripted study session
  through `SessionCore` (no TUI), asserting the exact `Vec<Event>`
  stream and final snapshot — the reproducibility contract.
- A scripted "played game": human script vs tactics AI at 200 sims,
  the script wins with a known forcing line; asserting the game
  reaches the expected terminal status.
- Measured sims-vs-strength table: 50 / 200 / 800 sims tactics AI vs
  the uniform-random baseline, 20 seeded games each, win rates quoted
  in the chapter (numbers measured in the reference worktree, marked
  `[experiment]`-adjacent: environment note included).
- Full gates + `cargo build --workspace`; the reference worktree is
  then frozen for solution extraction.
- Commit: `test(cli): acceptance — transcripts, scripted game, measured strength`.

## Task 8: tutorial authoring on `main`

**Files:** `docs/tutorials/playable-cli-tutorial/README.md`,
`00-playable-cli-primer.md`, `01`–`07` chapters + `NN/01-solution.md`
each.

- Chapter template follows convolutions: abstract, glossary, context,
  **intent** (the one felt thing, from the chapter map), contracts,
  mental mapping, low-level design, TDD checklist with exact test
  names, "done when", gates, references.
- Solution extraction is scripted: a small shell script copies each
  referenced file/span from the frozen worktree and `diff`s it against
  the quoted block; byte-for-byte or the chapter doesn't ship.
- The reference worktree is kept until the owner finishes chapter 07
  (inspection convention from convolutions), then deleted.
- Controller (not subagents) updates `docs/WARM-UP.md` and the
  tutorial-index references after owner review.

## Self-review notes

- Spec coverage: §3 study table → T2–T4; §4 core/shell → T0, T5; §5
  commands/events → T2; §6 actors/knobs/schedule → T3, T4; §6.1 no
  depth knob → primer + ch.03 sidebar; §7 variation tree → T1; §8
  clap → T0, T5; §9 overlays → T6; §10 opening → T5; §11 errors → T2,
  T5; §12 testing → every task + T7; §13 baseline amendment → noted in
  primer + ch.07 follow-ups; §15 tutorial shape → chapter map.
- Type consistency: `Command`/`Event`/`Occupant`/`AiConfig`/
  `SearchInfo`/`History` defined once (T1–T3), reused verbatim after.
- Deliberate exclusions (spec non-goals): no save/load, no raw keys,
  no time budgets, no REST server, no game export.
