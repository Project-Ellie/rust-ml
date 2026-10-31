# Warm-up — rust-ml project context for agents (and tired humans)

Read this first when opening a session in this repository. It compresses
~10k lines of documentation into what you need to be useful immediately.
Referenced from [AGENTS.md](../AGENTS.md).

## What this project is

**DeepGomoku reborn in Rust.** The old TensorFlow project
([DeepGomoku](https://github.com/Project-Ellie/DeepGomoku)) is being
resurrected as an AlphaZero-style Gomoku agent in Rust with the
[Burn](https://github.com/tracel-ai/burn) framework. The repository is
two things at once:

1. **A completed Burn curriculum** (MNIST, chapters 1–8 of `docs/`) —
   the learning path that taught the Burn idioms the Gomoku build uses.
2. **The Gomoku project** (chapters 9–13 + `gomoku/` workspace) — the
   actual goal: a self-improving Gomoku training laboratory, later a
   game server and applications around it.

Rules of the game: **freestyle Gomoku** on 15×15 — overlines (6+) count
as a win, draw at 225 moves, **Swap2 opening protocol** (freestyle is a
proven first-player win; Swap2 keeps self-play on open, unsolved
ground). The approach is **"alpha-epsilon", not tabula rasa**: a small
hand-woven tactics module (win-in-1, forced block, double threats, pure
bit ops) gives learning a head start, doubles as the MCTS mock
evaluator, and generates the milestone-3 synthetic training set.

## Version policy (do not violate)

- **Burn 0.22.0, pinned** (`=0.22.0` in `gomoku/Cargo.toml`). The gomoku
  workspace tracks the latest Burn release with an exact pin; upgrades are
  deliberate. Burn 0.22 removes the backend type parameter from `Tensor`
  and `Module` — backend selection is via `burn::tensor::Device`
  (`Device::flex()` for CPU, `Device::wgpu(Default::default())` for GPU).
  The root MNIST curriculum (`src/`, `examples/`, chapters 1–8) stays on
  **Burn 0.21.0** until its own migration. Never mix `main`-branch Burn
  code into either pin. Gomoku docs link the `v0.22.0` tag.
- Backends: **Flex** (pure-Rust CPU, verified, deterministic) is the
  parity-test twin; **Wgpu/Metal** is the GPU path but **training on it
  is gated on a backend parity test** (Burn issues #5162 NaN gradients,
  #5626 autotune 1×1-conv — both hit load-bearing parts of our network).
- Rust: edition 2024 in the `gomoku/` workspace, edition 2021 in the
  root package. No tokio anywhere — the system is CPU-bound actors +
  `crossbeam-channel` (reasoning: ch. 12 §5).

## Repository layout

```text
rust-ml/                    root package "rust-ml" (the MNIST curriculum)
├── src/                    shared MNIST library: data, model, training, inference
├── examples/               01_tensors … 06_mnist_infer — one per curriculum chapter
├── artifacts/mnist/        training output (git-ignored)
├── data/                   external corpora: opening lists, puzzles, game records
├── docs/                   the wiki: 15 chapters + tutorials/ + this file
├── paper/                  LaTeX monograph on the AlphaZero mathematics
└── gomoku/                 SEPARATE Cargo workspace (resolver 3, edition 2024)
    └── crates/
        ├── engine/         rules engine — dependency island: no Burn, no I/O
        ├── mcts/           arena tree — milestone 2 (search still to come)
        └── cli/            `gomoku` binary: terminal UI on either board
```

## The documented design (read these when the task touches them)

- **[12-gomoku-architecture.md](12-gomoku-architecture.md)** — THE system
  design. Four subsystems (engine, MCTS, network, training) + the
  self-play loop. Seven planned crates: `engine`, `net`, `mcts`,
  `selfplay`, `train`, `arena`, `cli`. Key boundaries: `mcts` depends on
  an `Evaluator` trait, never on `net`; `engine` is a dependency island;
  the network lives exclusively on one evaluator thread (ownership
  instead of locks). 14 self-play workers → one evaluator service
  (dynamic batching ≤128 / ≤2 ms) → trainer; **phased v1** (self-play →
  train → arena, one GPU), continuous mode is a designed-in upgrade.
  Replay buffer **stores games** (~60 B each), not planes — sampler
  replays + encodes + applies one random D4 transform on the fly.
  Network v1: 4 input planes at **17×17** (border ring = opponent
  stones), 128ch × 10 residual blocks (3.17 M params), policy 225
  logits + tanh value head. Loss `(z−v)² − πᵀlog p + c‖θ‖²`, AdamW
  1e-4, warmup+cosine LR. Milestones 1–8 with acceptance tests (ch. 12
  §13). Every number is labelled [paper] / [derived] / [experiment] —
  and the appendix holds an **honesty ledger** of folklore traps
  (c_puct is NOT in the DeepMind papers — 1.5 is ELF's value; AlphaZero
  publishes no replay-window size; AGZ used 1600 sims, AlphaZero 800).
- **[13-engine-design.md](13-engine-design.md)** — milestone 1 in
  detail. Locked decisions: overlines win; Swap2 from the start; 17×17
  encoding with border-as-opponent; alpha-epsilon tactics + bounded TSS
  oracle (soundness over completeness, machine-verified labels); absolute
  color storage (Swap2's non-alternating opening breaks relative
  stores); naive reference engine kept permanently behind
  `#[cfg(any(test, feature = "testutil"))]` as differential oracle;
  tightest possible public API.
- **[09-toward-alphazero.md](09-toward-alphazero.md)** — algorithm →
  Burn mapping; "Burn owns the network, you own the loop" (manual
  training loop, not `SupervisedTraining`).
- **[10-papers.md](10-papers.md)** — annotated reading list (AlphaGo
  Zero is the paper we implement; KataGo + ELF OpenGo for engineering;
  AlphaGomoku is the closest prior work).
- **[11-pitfalls.md](11-pitfalls.md)** — Burn traps: `CompactRecorder`
  is f16 (use full-precision recorders — arena comparisons must reflect
  training, not quantization); `argmax` keeps dims; `i32` vs `i64` Int
  elems per backend; stale pre-0.21 API names in blog posts.

## Engine internals — the vocabulary you must speak

Fixed glossary (tutorial README has the full version):

- **Stride-16 layout**: `idx(r,c) = r*16 + c` in a `[u64; 4]` (240 bits
  used, 225 real cells). Directions become uniform shifts 1, 16, 15, 17.
- **Padding invariant**: padding bits (column 15 of each row + bits
  240–255) are always zero → runs die at row ends, no edge masks needed.
  Masks (`VALID`) are required only after complements (`!occupied`).
- **Staged AND** win detection: `two = b & b.shr(s)`, `four = two &
  two.shr(2s)`, `five = four & four.shr(s)` — keeps shifts < 64 and
  detects ≥5 (overlines) for free.
- **Move vs MoveSet vs Bitboard**: `Move(u8)` is logical stride-15
  (r*15+c); `MoveSet([u64;4])` is public, stride-15; `Bitboard` is
  `pub(crate)`, stride-16. Two layouts behind one type = bug farm; the
  type split makes mixing them a compile error.
- **Zobrist**: `const fn` xorshift table `[[u64; 225]; 2]` generated at
  compile time (no `rand` dep, reproducible forever); incremental XOR in
  play/undo; used for replay-buffer dedup and test identity — NOT for
  MCTS transposition merging (the tree stays a tree, as in AlphaZero).
- **Transforms** (D4 group): 8 const permutation tables `[[u8; 225]; 8]`,
  applied in 15×15 space at sample time (augmentation, never baked into
  the net, never averaged at search time). Key property: **encode
  commutes with every transform** (the 17×17 border ring is D4-invariant).
  Terminology: the 8 group elements are *transforms*; a *symmetry* is a
  transform that maps a given position to itself.
- **Swap2 as typestate**: `Placing3 → FirstChoice → (Placing2) →
  FinalChoice → Board::from_position` — invalid transitions are compile
  errors.
- **Differential testing**: the fast engine must agree with the naive
  `reference.rs` oracle on every ply of 10k random games + undo walks.

## Current status (verified 2026-09-28 — update this section as work lands)

**Milestone 1 (engine) COMPLETE.** Done:

- Workspace + `engine` skeleton (slice 1).
- `Move`, naive `reference.rs` + corpus (slice 2).
- `Bitboard` + `Board` with play/undo/status/legality/`empty_moves`
  (slice 3).
- Staged shift-AND win detection in `win.rs`, integrated into
  `Board::play` (slice 4) — overlines count, padding invariant kills
  wrap fives, no edge masks.
- Incremental Zobrist keys (slice 5): compile-time `const fn` table in
  `zobrist.rs`, `Board::zobrist()` updated in O(1) by play/undo,
  `compute_key` from-scratch ground truth, 10k-walk roundtrip proptest.
- D4 symmetry + encoding (slice 6): `Transform` with const-eval
  permutation tables (rot90 is counterclockwise — convention pinned),
  `inverse`, `permute`; 17×17 relative planes with border ring; the
  commutation proptest (10k boards) is green. Tactics-adjacent lesson:
  `prop_assume` filtering at ~9% reject rate aborts proptest at high
  case counts — generate from the complement instead (prop_flat_map).
- Differential harness (`tests/differential.rs`, feature-gated
  `testutil`): 10k random games + 1k undo walks vs the oracle.
- Tactics (slice 7): `MoveSet`, `immediate_wins` / `forced_blocks` /
  `double_threats` (hypothetical-placement, no pattern matching), naive
  line-scan oracles + ASCII puzzle parser in `reference.rs`,
  differential proptest. v1 simplifications documented (four-three
  blind spot, opponent-wins-first accepted, immediate wins excluded
  from double threats).
- Threat-space search (slice 8): `prove_forced_win` / `verify_line` in
  `tss.rs` — bounded prover (attacker threats vs defender forced
  blocks, defender-wins-first check) + verifier that enumerates EVERY
  defender block (gapped fours). Soundness gate green: 100% of emitted
  proofs verify, incl. at PROPTEST_CASES=2000; differential vs a
  depth-bounded reference adjudicator green.
- Swap2 opening (slice 9): `Swap2<Placing3|FirstChoice|Placing2|
  FinalChoice>` typestate + `Board::from_position` (overlap/count
  validation, from-scratch Zobrist). Opening-built and play-built
  boards share keys (tested).
- Benchmarks + hardening (slice 10): criterion bench — `has_five_any`
  ≈8.5 ns/call (bar ≤20 ns), play/undo ≈8 ns/op; `#![deny(missing_docs)]`
  public-surface lock.
- **CLI side quest complete**: playable human-vs-human terminal UI on
  both boards (`--engine naive|fast`), alternate-screen no-scroll UI
  (crossterm), undo/restart, via a `dyn GameBoard` trait owned by the
  CLI crate (the engine itself deliberately has no shared trait).
- **CLI demo features**: hotseat Swap2 opening flow (default; party
  switches for accept-or-add-2 and color choice, undo by replaying the
  placement log — `--no-swap2` skips), a TSS overlay (`t` key):
  the verified forced-win line rendered as numbered cells (attacker
  plain, defender bracketed), verdict line included, and a puzzle
  examination mode (`--puzzle <path> [--index N]`) with sidecar parser
  trust gate, undo/redo floor at the puzzle root, next/previous
  wrapping, and TSS overlay. `--engine naive` is a default-on cargo
  feature (`naive-engine`); lean builds use `--no-default-features`.

Verified: `cargo test -p engine --features testutil` → 92 unit + 5
differential + 2 doc-tests green; `cargo test -p cli` → 45 green;
`cargo test -p mcts` → 33 unit + 4 acceptance green; `cargo bench -p
engine` meets the milestone-1 bar.

**Milestone 2 (mcts) COMPLETE** — all 10 chapters of the
[mcts-tutorial](tutorials/mcts-tutorial/README.md) (00-mcts-primer +
10 chapters) landed:

- Arena tree (chapter 02): `NodeId`, `Edge` with `(P, N, W)`, `Tree`
  with `add_child`.
- PUCT selection (03): `puct_score`, `Selection`, `select` — read-only
  descent, stops at unexpanded/terminal.
- Evaluation seam (04): `EvalRequest` (planes + legal moves, no
  `Board`), `EvalResult` (policy logits + side-to-move value),
  object-safe `Evaluator` trait, `masked_softmax` (mask-first, f64
  accumulator, degenerate-uniform fallback), `UniformEvaluator`.
- Expansion (05): terminal-first via `board.status()` (exact values,
  evaluator never called), eager child creation, idempotent
  re-expansion.
- Backup (06): side-to-move sign convention — negate once, flip per
  level, one rule for terminal and network values.
- Simulation loop (07): `search` + `SearchConfig{simulations: 400,
  c_puct: 1.5}` (ELF's value, not DeepMind's) — root pre-expansion
  gives the invariant `sum(root edge N) == simulations`.
- Policy (08): `visit_distribution`, `select_move` (argmax /
  temperature sampling), `add_dirichlet_noise` (k Gamma(α,1) draws
  normalized); RNG threaded as `&mut impl Rng`, tests seeded.
- Tactics-shaped mock (09): `TacticsEvaluator` shapes priors from
  engine truth (`immediate_wins` / `forced_blocks`); reconstructs a
  `Board` from planes via stone-count parity; sign sentinels green
  (win-in-1 and forced block get most visits at 50 sims).
- Acceptance (10): `tests/acceptance.rs` — win-in-1 @50 sims, forced
  block @50, win-in-3 first move @200, and a seeded 1000-game uniform
  sanity flood (every move legal, every game ≤225 plies); crate locked
  with `#![deny(missing_docs)]` and a curated re-export surface.

**Milestone 3 (synthetic data + net) IN PROGRESS** — two of its three
tutorial tracks are written and verified end-to-end (reference
implementations built TDD-first in disposable worktrees, reviewed,
acceptance-measured, then deleted per convention; the solution files
are the durable record). The `train` and `patterns` crates exist in the
workspace; the `net` crate is next. Living docs for the gomoku workspace
have been updated to **Burn 0.22.0** on branch `burn-0.22-upgrade`.

Current milestone-3 status:

- [datagen-tutorial](tutorials/datagen-tutorial/README.md) (primer + 8
  chapters + opt-in solutions): the `train` crate — `Sample` record,
  seeded playouts, tactics labels (win/block/quiet, 90/10 shaped
  policy, ±1/0 values), quota collector with Zobrist dedup, bincode
  shards + manifest, soundness gate, key-hash held-out split, and the
  `datagen` binary. Verified acceptance: 50k samples in 45 s (debug),
  13 shards / 41 MB, 9.84% holdout, byte-identical regeneration.
- [convolutions-tutorial](tutorials/convolutions-tutorial/README.md)
  (side quest; COMPLETE 2026-10-17, all 8 chapters on `main`): the
  `patterns` crate — a hand-written 152-channel conv net (Burn 0.22,
  `Device::flex()` default + opt-in `gpu` feature) producing threat
  maps (wins, double threats, double-three forks) plus threat
  potential (`policy()`), the `patterndemo` heatmap binary, and green
  CPU-vs-GPU exact-equality tests. Acceptance ritual measured on
  `main`: default suite 67/3 ignored, full proof 3×1000 boards green
  in release, GPU parity 77/3, crate locked with
  `#![deny(missing_docs)]`. The chapter-3 slice
  landed the FULL pattern table (all six families, both conv layers)
  plus `ThreatNet`/`win_map` at once — chapters 4–8 and all solution
  files were then reconciled with that reality and re-quoted from a
  fresh verified reference build (worktree `../rust-ml-reference`,
  branch `patterns-reference-scratch`, kept for inspection). Verified
  acceptance of that reference: three differential gates at 1000
  boards × 2 colors vs `engine::{immediate_wins, double_threats}` and
  an in-crate naive three-oracle — exact; full proof ~4.5 s release;
  default suite ~36 s debug (83 passed / 3 ignored); exact CPU-vs-Wgpu
  parity (93 passed with `--features gpu`).

The net tutorial (phase-0 supervised training on the synthetic data)
is next after the two queued side quests; milestones 4–8 (TSS anchor
set, selfplay service, the phased loop, hardening, upgrades) have not
started.

Two further tutorial tracks are written and verified, ready to work:

- [baseline-tutorial](tutorials/baseline-tutorial/README.md) (primer +
  5 chapters, solutions land as worked): the `baseline` crate — a
  hand-written evaluator (threat maps + gravity kernel → tiered policy
  logits + saturating value) behind the `mcts::Evaluator` seam, with
  measured-strength acceptance. Design spec:
  [2026-10-13-baseline-player-design.md](specs/2026-10-13-baseline-player-design.md).
- [playable-cli-tutorial](tutorials/playable-cli-tutorial/README.md)
  (primer + 9 chapters + opt-in solutions, written 2026-10-17/18): the
  `cli` crate grows into the study table — two seats (human or AI,
  take/release anytime), a persistent variation tree with silent
  branching, a headless `SessionCore` (commands/events, REST-ready),
  policy overlays + value strip, per-seat sims, and positions from
  files: puzzle packs (`--puzzle`, hint/reveal/next/prev) and Piskvork
  game records (`--game`, cursor at the end position). **Single-CLI
  owner decision**: the legacy hotseat/puzzle UI is deleted (no
  `--plain`, no naive-engine play, no `dyn GameBoard`) — the study
  table is the only UI. Notation is standard Gomoku algebraic
  (columns a–o, rows 1–15, center h8) at input and display. Reference
  built TDD-first in worktree `../rust-ml-ref-playable-cli` (branch
  `reference/playable-cli`, 12 commits, 136 tests + 1 ignored
  measurement green, per-commit gates verified; kept for inspection
  until chapter 09 is worked). Design spec (three dated amendments:
  algebraic notation, single CLI, positions from files):
  [2026-10-17-playable-cli-design.md](specs/2026-10-17-playable-cli-design.md);
  plan: [2026-10-17-playable-cli.md](plans/2026-10-17-playable-cli.md).
  Note: baseline ch.4's `--versus baseline` plan is superseded — the
  baseline evaluator will enroll as an `EvaluatorKind` in the study
  table (spec §13).
  **Progress on main (2026-10-20)**: chapters 01–04 are implemented
  (variation tree, command language, AI seats via mcts, study loop
  with the opening schedule). Chapter 05 was redesigned as a
  six-part sub-project — design paper, TUI excursion, and the six
  tutorial + solution document pairs live in `05-the-real-table/`;
  the code is the learner's work, sub-chapter by sub-chapter. A
  complete reference implementation of the redesign is preserved on
  the branch `reference/chapter-05-redesign` (151 cli tests green;
  tip 87b170b). The branch tip carries the testkit-crate approach
  (one workspace crate `crates/testkit`, pulled into cli as a
  dev-dependency, replaces the earlier two-kit split
  (`src/testkit.rs` + `tests/common/mod.rs`)) and the Swap2
  strict-alternation simplification (no color token on `play`, no
  `b`/`w` arming keys at placement phases; colors alternate black
  first across both rounds), and the chapter-05
  docs are amended to both. **Progress on main (2026-10-28)**: chapter
  05 is COMPLETE. Sub-chapters a-c are the owner's own work
  (d1a208b, 39dd819+47eddd0, d509327 — 05c redone with strict
  alternation and the engine typestate as validated projection).
  Sub-chapters d-f landed in 44a28b1, taken from
  `reference/chapter-05-redesign` and reconciled with the redone 05c
  (parse-error/event wordings; repl::HELP regained the Swap2 section).
  All gates green. Caveat: the d-shell solution doc (06f1eff) is
  partly stale — its input.rs/render.rs quotes match the code
  byte-exactly, but its study.rs/screen.rs quotes describe an
  abandoned restructure; the e/f solution docs are byte-exact against
  main. Chapter 06 (seeing the mind) was REDESIGNED as four
  sub-chapters (06a value strip, 06b visits heatmap, 06c prior view,
  06d TSS overlay + policy/TSS mutual exclusion), each with spec,
  design, implementation-plan, and byte-exact solution artifacts —
  docs on main, reference implementation on
  `reference/chapter-06-redesign` (four slice commits, gates green;
  worktree `../rust-ml-ref-chapter-06` kept for inspection). Note:
  the study table had NO TSS overlay before 06d — the new-structure
  chapter 05 never ported it; 06d restores it per spec §9. The code
  is the learner's work, sub-chapter by sub-chapter.
  **Progress on main (2026-10-30)**: sub-chapter 06a (value strip) is
  implemented (7a724a7 — started as the owner's own work, completed
  against the verified solution doc; `v` key / typed `v`/`value`
  toggle, ShellState in the loop, line_outcome/empty_line_outcome
  moved from study.rs into input.rs, staleness note vs the live seat
  config; 16 spec tests green, scripted acceptance matches the
  chapter's example frame). Follow-up d80c714: the space bar plays
  and steps like Enter (both playing_key and opening_key).
  Sub-chapter 06b (visits heatmap) is implemented in the same working
  session: `p` / typed `p`|`policy` cycle the board cell overlay
  Off→Visits→Off, `overlay.rs` gained `PolicyOverlay`, `OverlayCell`,
  `overlay_cell`, `heatmap_bg` and the styled/plain cell renderers
  (hot ≥10% prints the raw visit count; stones, last move, and the
  cursor are never over-painted), and all fourteen spec tests pass.
  06c (prior view) is implemented too: the cycle is
  Off→Visits→Prior→Off, `prior_cell` normalizes by the maximum prior
  (hot ≥10% prints the raw prior percentage, the brightest cell is always
  the evaluator's favourite), `policy_empty_cell` now takes the active
  mode instead of hard-coding visits, and all nine spec tests pass.
  Review fix on landing: the 06c generalization first routed every
  search-backed cell through the policy path, which dropped the study
  board's dim grey empty cells whenever an overlay was Off — Off now
  falls back to `study_cell`, with a frame-equality regression test
  (`overlay_off_draws_the_same_board_as_no_search_info`); the help lines
  read `off → visits → prior`. 06d (TSS overlay) remains. Known noise:
  with the opening schedule's Dirichlet noise, real early-game
  searches spread visits below the 10% hot threshold — the heatmap is
  honest, the evaluator is weak; the trained net (net tutorial) makes
  these views sharp.

## How the tutorials work (respect the pedagogy)

The engine tutorial (`docs/tutorials/13-engine-tutorial/`) is
**learn-by-doing for the reader**: slices give exact API contracts and
TDD checklists but withhold implementations — the learner writes the
code. When
assisting with a slice: coach toward the contract, don't dump the
implementation. (Opt-in reference solutions exist for slices 2–4 in the
per-chapter companion folders; the CLI tutorial, by contrast, ends each
chapter with a full verified solution.) Stuck >20 min → coaching
conversation, per the tutorial README. Gates before every commit:
`cargo test` + `cargo clippy --all-targets -- -D warnings` +
`cargo fmt --all`, all green.

## Commands

```bash
# Root package (MNIST curriculum)
cargo run --example 01_tensors          # … through 06_mnist_infer
cargo run --release --example 05_mnist_train -- 1   # 1-epoch smoke test

# Gomoku workspace (cd gomoku)
cargo test -p engine                    # unit tests
cargo test -p engine --features testutil   # + differential suite (10k games)
PROPTEST_CASES=10000 cargo test -p engine  # more property iterations
cargo run -p cli                        # play! Swap2 opening + 't' for TSS overlay
                                      # (--engine naive|fast, --plain, --no-swap2)
```

## Roadmap beyond milestone 1

MCTS with mock evaluator → net+train on tactics-generated synthetic data
→ TSS oracle + proven-label anchor set (≥10k verified forced-win
puzzles) → 14-worker self-play with batched evaluator (≥400 games/h
target) → phased `gomoku run` loop with arena Elo tracking → hardening
(7-day runs, crash recovery) → registered upgrades one at a time
(playout-cap randomization, global-pooling heads, continuous mode, net
growth, forced playouts, auxiliary targets, TSS inside MCTS). After
that: applications around the lab — a game server, and the dream of a
physical Gomoku board.
