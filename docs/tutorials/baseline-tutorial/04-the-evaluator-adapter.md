# Chapter 04 — The evaluator adapter and the CLI play mode

## Abstract

This chapter assembles the player. Chapters 1–3 built the pieces —
the gravity map, the policy readout, the value readout — and this
chapter wires them into `BaselineEvaluator`, an implementation of the
`mcts::eval::Evaluator` seam, and then into the `gomoku` terminal
client as a new `--versus baseline` play mode. You will learn the
adapter shape the trained `net` crate will later reuse
(`engine::Planes` in → reconstructed `Board` → `analyze_both` → maps +
gravity → `EvalResult` out); how to recover a `Board` from the
side-to-move-relative planes by stone-count parity, and why that small
helper is a *documented, deliberate duplication* of the mock's private
one; why the evaluator must be fully deterministic and why terminals
never reach it; and how the CLI rides its existing `GameBoard` seam so
the versus mode adds one code path, not a second client.

## Glossary

| Term | Definition |
|------|------------|
| **Adapter** | The glue between two contracts that must not know each other: here, the mcts seam's `Planes`-speaking `Evaluator` trait and the `patterns` network's `Board`-speaking `analyze_both`. |
| **Reconstruction** | Recovering an `engine::Board` from an `EvalRequest`'s 17×17 planes and legal-move list. |
| **Parity** | The stone-count argument that decides the side to move: Black moves first, so equal black/white counts mean Black to move, a one-stone difference means White to move. |
| **Absolute colors** | The locked docs/13 decision: `Board` always speaks Black/White, never me/you. `analyze_both` returns `[black_maps, white_maps]`. |
| **Side-to-move-relative planes** | The engine's 17×17 `encode` output: `me` is always the side to move, `you` the opponent (plus the border ring). Relative, not absolute. |
| **Deliberate duplication** | Copying a small helper across a crate boundary on purpose, with a comment saying so, instead of adding a dependency edge the architecture forbids. |
| **Determinism** | Same position in ⇒ identical `EvalResult` out. No RNG anywhere in evaluation. |
| **Versus mode** | The CLI's new play mode: human vs baseline, on the existing hotseat machinery. |
| **Hotseat** | The CLI's human-vs-human flow; in versus mode the human answers for both parties during the Swap2 opening, and the baseline takes over its assigned color once a normal position exists. |

## Context

The pieces exist: `gravity` (chapter 1), `policy` (chapter 2), `value`
(chapter 3), and — completed as the README's prerequisite — the
`patterns` final form (`ThreatNet`, `ThreatMaps`, `analyze_both`). The
mcts crate has run PUCT behind the `Evaluator` seam since milestone 2,
with the mock as its only non-trivial implementation. The CLI has run
human-vs-human on either engine board, with a `GameBoard` trait that
already exposes the fast engine's board for engine-specific features
(the TSS overlay is the precedent).

Nothing yet implements `Evaluator` *outside* the mcts crate. That is
exactly what the seam is for — docs/12 §6 describes "a direct
`net::Model` wrapper implements it for tests and arena play" — and the
baseline is such a wrapper, one tutorial early.

## Intention

1. Implement `src/reconstruct.rs`: `reconstruct(&Planes, &[Move]) ->
   Board` by stone-count parity, asserting the supplied legal set
   equals the reconstructed board's.
2. Implement `src/eval.rs`: `BaselineEvaluator` holding the threat
   network and the gravity branch, implementing
   `mcts::eval::Evaluator` — deterministic, side-to-move-relative,
   never consulted on terminals.
3. Implement `src/play.rs`: `choose_move(&Board, sims) -> Move` —
   search plus argmax move selection, the only entry point the CLI
   needs.
4. Add the CLI's `--versus baseline [--color black|white] [--sims N]`
   play mode through the existing `GameBoard::engine_board` seam, in
   both front-ends, with the naive-engine combination rejected as a
   clean startup error.

Observable done-state: the chapter-4 tests listed below pass,
`cargo run --release -p cli -- --versus baseline` plays a full game
against you, `cargo clippy --all-targets -- -D warnings` is green from
`gomoku/`, and `cargo fmt --all` makes no changes from `gomoku/`.

## Mental mapping

### Why reconstruct a `Board` at all?

The seam does not speak `Board`. Quoted verbatim from
`gomoku/crates/mcts/src/eval.rs` — the real contract, unchanged since
milestone 2 (chapter 5's acceptance ritual re-verifies it):

```rust
/// Evaluation request, contains the board as Planes and a vector of legal moves
#[derive(Debug, Clone, PartialEq)]
pub struct EvalRequest {
    /// the board in its 17x17 representation
    pub planes: Planes,

    /// Vector with all legal moves
    pub legal: Vec<Move>,
}

/// The result: Policy and Value
#[derive(Debug, Clone, PartialEq)]
pub struct EvalResult {
    /// the policy vector: 15x15
    pub policy: [f32; 225],

    /// the value of the position `[-1.0 .. 1.0]` from current player's point of view
    pub value: f32,
}

/// the contract with any evaluator implementation
pub trait Evaluator {
    /// return policy and value for current board position
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult;
}
```

The evaluator receives the position as `engine::Planes` — the 17×17
side-to-move-relative `me`/`you` encoding with the border ring in
`you` — because the seam is designed for *networks*, and networks eat
planes. But `patterns::net::analyze_both` speaks `engine::Board`, and
the readouts need occupancy (`board.stone_at`) and the side to move
(`board.to_move`). So the adapter's first job is planes → `Board`. The
mock solved the same problem with a private helper; the seam's
consumers each carry their own.

### Why does stone-count parity recover the position?

During normal search, every evaluated position is alternation-legal:
Black moves first, players alternate, so Black's count either equals
White's (Black to move) or exceeds it by one (White to move). The
planes are side-to-move-relative, so `me`-stones and `you`-stones
decode like this:

* counts equal ⇒ the side to move is **Black** ⇒ `me` = black stones,
  `you` = white stones;
* `me` has one fewer ⇒ the side to move is **White** ⇒ `me` = white
  stones, `you` = black stones.

`Board::from_position(&black, &white, to_move)` rebuilds the board
(Zobrist key recomputed from scratch), and then the **legal-set
assertion** is the tripwire: the supplied `req.legal` must equal the
reconstructed board's `empty_moves` set exactly. If the planes were
corrupt, non-alternating, or from a different position than the legal
list, the assert fires at the boundary instead of letting a wrong
board quietly produce confident logits. The assert is `debug`-grade
insurance for an invariant the search maintains by construction — and
the doctored-request test below proves the tripwire works.

### Why duplicate the mock's private helper instead of sharing it?

The mock's `reconstruct_board` is private to `mcts`, and the
architecture wants it that way: mcts is a Burn-free, engine-only crate
whose public surface is the search; the seam is `Planes`, not `Board`.
Exporting the helper would couple mcts's API to one consumer's
convenience. The established precedent is set in the convolutions
design spec: `patterns` duplicating datagen's playout generator
rather than adding a dependency edge. So
the baseline carries its own small copy — **documented, deliberate
duplication**, with a comment in `reconstruct.rs` naming the original
(`mcts::mock`'s private `reconstruct_board`) and the reason. If the
encoding ever changes, both copies fail their legal-set asserts
loudly, together.

### Why do absolute colors matter here?

`analyze_both` returns `[black_maps, white_maps]` — absolute, per the
locked docs/13 decision. The readouts want `(own, opponent)` —
relative. The adapter is the one place that converts: pick by
`board.to_move()`. Get this backwards and every test below still
compiles — but the forced-block tier starts highlighting *your own*
wins as emergencies and the value flips sign. The determinism and
sentinel tests are the net; the color swap is the classic bug they
catch.

### Why must the evaluator be deterministic — and who provides variety?

`evaluate` contains no RNG: same request in, identical `EvalResult`
out, bitwise. Match-play variety comes from the search side —
temperature sampling in `mcts::policy::select_move` — never from the
evaluator. The division of labor matters because the acceptance
chapter's numbers only mean something if every game is reproducible:
seeded match, seeded openings, deterministic evaluation. (The
evaluator is also why matches must not be judged move-by-move: an
evaluator that rolled dice would make "the same" position score
differently on two visits.)

### Why do terminals never reach `evaluate`?

`mcts::expand` checks `board.status()` first: won positions return
−1.0, draws 0.0, and only ongoing positions are encoded and sent to
the evaluator — its doc comment states it plainly: "The evaluator is
never called for terminals." Two consequences. First, the value
heuristic's ±0.99 saturation (chapter 3) never collides with a proven
outcome, and ±1 stays reserved for truth. Second, the adapter needs no
terminal branch: if `evaluate` ever *is* called on a terminal, the bug
is upstream, and a defensive special-case here would mask it. Do not
add one.

### Why does `choose_move` live in `baseline` — and the CLI never touch `mcts`?

The CLI's new dependency edge is `cli → baseline`, full stop. The
search-side APIs the play helper consumes are quoted from
`gomoku/crates/mcts/src/`:

```rust
pub fn search(
    root_board: &Board,
    evaluator: &mut dyn Evaluator,
    config: &SearchConfig,
) -> SearchOutcome;

pub fn visit_distribution(tree: &Tree, root: NodeId) -> Vec<(Move, u32)>;

pub fn select_move(dist: &[(Move, u32)], temperature: f32, rng: &mut impl Rng)
    -> Option<Move>;
```

`choose_move` composes them once — `search` with
`SearchConfig { simulations: sims, ..Default::default() }` (the
default is 400 simulations, c_puct 1.5 — ELF OpenGo's published value,
since DeepMind never published one), then `visit_distribution`, then
`select_move` at temperature 0: argmax, deterministic, ties broken by
lowest `Move::index`. The `rng` parameter the signature demands is
satisfied with a locally seeded `StdRng`; the argmax path consumes no
randomness. One entry point, one line of dependency, and the CLI
learns nothing about trees, priors, or PUCT.

### Why does versus mode ride the existing Swap2 flow?

The Swap2 opening already has a human answering for both parties — the
hotseat flow places stones and answers the opening choices from the
keyboard. Versus mode changes *who answers after the opening*: the
baseline takes over its assigned color once a normal position exists.
With `--no-swap2`, the game starts from an empty board as today and the
baseline simply answers its turns from move one. One opening code
path, no special cases — the versus turn is a dispatch decision at the
same seam the hotseat uses, in both front-ends (`run_ui`'s alternate
screen and `run_plain`'s scrolling output).

## Low-level design

### Files you will create or modify

```text
gomoku/crates/baseline/src/
├── lib.rs          (adds `pub mod reconstruct; pub mod eval; pub mod play;`)
├── gravity.rs      (chapter 1; unchanged)
├── policy.rs       (chapter 2; unchanged)
├── value.rs        (chapter 3; unchanged)
├── reconstruct.rs  (new)
├── eval.rs         (new)
└── play.rs         (new)
gomoku/crates/cli/
├── Cargo.toml      (adds `baseline = { path = "../baseline" }`)
└── src/
    ├── main.rs     (flags, usage, the versus arm, the naive check)
    ├── board.rs    (unchanged — the seam already exists)
    └── ui.rs       (the versus turn in both run loops)
```

### `src/reconstruct.rs`

```rust
pub fn reconstruct(planes: &engine::Planes, legal: &[engine::Move]) -> engine::Board;
```

Mirror of the mock's private helper, with the duplication comment:

* scan the in-board region of the 17×17 planes — board cell `(r, c)`
  sits at `(r + 1) * engine::EXT + (c + 1)` — collecting `me`-stones
  and `you`-stones as two `Vec<Move>`;
* parity: equal counts ⇒ `Color::Black` to move, `me` = black; one
  fewer `me`-stone ⇒ `Color::White` to move, `me` = white;
* `Board::from_position(&black, &white, to_move)`
  (`.expect("Encoded planes must form a valid position")`) — which is
  itself a second tripwire: it errors on overlapping colors and on
  counts that differ by more than one or do not match `to_move`;
* assert the supplied legal set equals the reconstructed board's
  `empty_moves` set (a `HashSet` comparison, as in the mock);
* panics on inconsistency — the function is a boundary, and boundary
  violations are bugs, not data.

### `src/eval.rs`

```rust
pub struct BaselineEvaluator {
    net: patterns::net::ThreatNet,
    gravity: burn::nn::conv::Conv2d,   // the chapter-1 branch
    device: burn::tensor::Device,
}

impl BaselineEvaluator {
    pub fn new() -> Self;                       // everything on Device::flex()
    pub fn with_device(device: burn::tensor::Device) -> Self;  // opt-in gpu
}

impl Default for BaselineEvaluator {
    fn default() -> Self { Self::new() }
}

impl mcts::eval::Evaluator for BaselineEvaluator {
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult;
}
```

`evaluate`, step by step:

1. `let board = reconstruct(&req.planes, &req.legal);`
2. `let [black, white] = analyze_both(&self.net, &board, &self.device);`
   — one batched forward pass, absolute colors in.
3. `let (own, opp) = match board.to_move() { Color::Black => (&black,
   &white), Color::White => (&white, &black) };` — absolute to
   relative, at the boundary.
4. `let occ = occupied_plane(&board); let g = apply_gravity(&self.gravity,
   &occ, &self.device);` — the held branch, never rebuilt per call.
5. `let policy = policy_logits(own, opp, &g, &board);`
6. `let value = value(own, opp);`
7. `EvalResult { policy, value }` — raw logits and a saturating
   heuristic, exactly as chapters 2–3 defined them. No normalization,
   no RNG, no terminal branch.

### `src/play.rs`

```rust
pub fn choose_move(board: &engine::Board, sims: u32) -> engine::Move;
```

Build a `BaselineEvaluator::new()`, run
`mcts::search(board, &mut ev, &SearchConfig { simulations: sims, ..Default::default() })`,
extract `mcts::visit_distribution(&outcome.tree, outcome.root)`, and
pick with `mcts::select_move(&dist, 0.0, &mut rng)` where `rng` is a
`StdRng` seeded from a fixed constant. `.expect(...)` the `Option`:
callers never call on terminal positions (the CLI checks
`status()` first, as the hotseat already does), so the root always has
edges.

### The CLI's `--versus` mode

Usage line (semantics are the contract; exact flag spellings and
defaults are pinned in the implementation plan):

```text
gomoku --versus baseline [--color black|white] [--sims N]
```

* `--versus baseline` — the only recognized opponent name today; the
  flag exists so later opponents (the trained net) join the same
  dispatch.
* `--color` — the **baseline's** assigned color. During the Swap2
  opening the human answers for both parties (hotseat behavior,
  unchanged); the baseline takes over its color once a normal position
  exists. `--no-swap2` starts from an empty board as today.
* `--sims N` — simulations per baseline move, default 400. An
  `[experiment]` knob, documented as such: it trades thinking time for
  strength on first contact, and it keeps the scripted smoke test
  fast.
* The mode requires the fast engine: the versus turn calls
  `board.engine_board()` (the `GameBoard` seam whose default returns
  `None`, overridden by the fast board — the TSS overlay at
  `ui.rs`'s dispatch is the usage precedent) and feeds the resulting
  `&engine::Board` to `baseline::play::choose_move`. Combining
  `--versus` with `--engine naive` is a **clean startup error**,
  mirroring the existing Swap2/naive check in `main.rs` ("error: the
  Swap2 opening requires the fast engine …"), and the usage strings
  (both `naive-engine` cfg variants) mention the new flags.
* Both front-ends support the mode through the same dispatch seam;
  `run_plain` reading moves as `<row> <col>` lines means a scripted
  stdin file doubles as the end-to-end smoke test.

## TDD checklist

Run these tests from `gomoku/` with `cargo test -p baseline` (and
`cargo test -p cli` for the CLI items).

The chapter's suite:

From `reconstruct::tests`:

1. `reconstruct_round_trips_position_and_zobrist` — scripted games
   (short legal sequences, both colors to move): `encode` →
   `reconstruct` yields a board with the same stone at every one of
   the 225 cells, the same `to_move`, and the same `zobrist()` key.
2. `legal_set_assertion_fires_on_doctored_request` —
   `#[should_panic]`: drop one move from `req.legal` and the
   assertion trips.

From `eval::tests`:

3. `evaluate_is_deterministic` — the same `EvalRequest` twice yields
   an identical `EvalResult` (`==` on the struct).
4. `baseline_matches_mock_on_win_in_one` — the mcts acceptance
   win-in-one board (open four, Black to move); build the request with
   `engine::encode` + `empty_moves`; the argmax of the returned policy
   over the legal cells is inside `engine::immediate_wins` — the same
   set the mock highlights. The baseline is *at least* as tactical as
   what it replaces.
5. `baseline_matches_mock_on_forced_block` — the mcts acceptance
   forced-block board; the policy argmax is inside
   `engine::forced_blocks`.

From `play::tests`:

6. `choose_move_takes_win_in_one_at_50_sims` — the win-in-one board,
   50 simulations: the returned move is an immediate win.

In the `cli` crate:

7. `versus_naive_engine_is_a_clean_startup_error` — parsing/dispatch
   rejects `--versus baseline --engine naive` with the startup error,
   mirroring the Swap2/naive check.
8. `scripted_plain_versus_game_completes` — an integration test
   (`cli/tests/`) spawning the `gomoku` binary with
   `--plain --versus baseline --no-swap2 --sims 10`, piping a scripted
   move sequence to stdin, and asserting a clean exit: the smoke test
   for the whole mode, kept fast by `--sims`.

Follow the red-green-refactor rhythm:

1. **Red:** Create the three modules with signatures and `todo!()`
   bodies, add tests 1–6; add the CLI flag parsing and its tests 7–8.
   Run `cargo test -p baseline` and `cargo test -p cli`. Expect
   failures.
2. **Green:** Implement `reconstruct` first (and watch the round-trip
   test go green), then `evaluate`, then `choose_move`, then the
   versus turn in both run loops. All eight tests should pass.
3. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

Then the real gate: `cargo run --release -p cli -- --versus baseline`
and play a game. Release mode is not vanity here — the convolutions
tutorial measured a ~70× debug-to-release gap on the same forward pass
(8.9 s vs ~620 s for its full proof) — it is the difference between a
move every few seconds and a move every few minutes.

## Done when

* `cargo test -p baseline` and `cargo test -p cli` pass from `gomoku/`
  (chapters 1–3 stay green).
* `cargo run --release -p cli -- --versus baseline` is playable
  end-to-end, in `--plain` and in the alternate-screen UI.
* `--versus baseline --engine naive` fails cleanly at startup.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* `cargo check -p engine` still passes; `cargo tree -p mcts` shows no
  Burn — the dependency rules held.
* Commit message for the code slice:
  `feat(baseline): evaluator adapter + CLI versus mode`.

Next: [Chapter 05 — Acceptance: measured games](05-acceptance.md)

## References

* [`../../specs/2026-10-13-baseline-player-design.md`](../../specs/2026-10-13-baseline-player-design.md)
  — §6 (crate topology), §7 (the adapter), §8 (the CLI play mode).
* [`00-baseline-primer.md`](00-baseline-primer.md) — §6 (the adapter,
  end to end), §9 (color-swap, terminal-branch, and RNG bugs).
* `gomoku/crates/mcts/src/eval.rs` — the `Evaluator` seam, quoted
  verbatim above; `mock.rs` — the private `reconstruct_board` this
  chapter deliberately duplicates; `search.rs` — `SearchConfig` and
  the 400/1.5 defaults; `policy.rs` — temperature-0 argmax with
  lowest-index tie-breaking; `expand.rs` — the terminal guard.
* `gomoku/crates/cli/src/board.rs` — the `GameBoard::engine_board`
  seam; `main.rs` — the Swap2/naive startup-error precedent;
  `ui.rs` — the TSS overlay's `engine_board` usage precedent.
* [`../convolutions-tutorial/04-the-four-level/01-solution.md`](../convolutions-tutorial/04-the-four-level/01-solution.md)
  — the `analyze_both` contract the adapter consumes.
