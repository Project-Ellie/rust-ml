# Convolutions tutorial — implementation plan

**Spec:** `docs/specs/2026-09-27-convolutions-tutorial-design.md`
(approved by owner 2026-09-27, including the backend-generic +
opt-in-GPU amendment). **Format precedent:** the datagen tutorial
pipeline. **Status:** approved for execution by owner ("go for it,
sub-agent-driven").

## Context

Side-quest tutorial between datagen and net tutorials. Deliverables:

1. **Reference implementation** — `gomoku/crates/patterns`, built in a
   disposable worktree (`/Users/wgiersche/workspace/Project-Ellie/rust-ml-ref-convolutions`,
   branch `reference/convolutions`), deleted after final verification.
2. **Tutorial** — `docs/tutorials/convolutions-tutorial/` on `main`:
   `README.md`, `00-convolutions-primer.md`, chapters `01`–`08`,
   per-chapter opt-in `NN-name/01-solution.md` files quoting the
   verified reference verbatim.

## Hard rules for every slice

- Burn pinned `=0.21.0`; verify APIs against the pinned source
  (`~/.cargo/registry/src/.../burn-0.21.0/`), never blog posts or
  0.22/main-branch docs.
- Engine untouched (dependency island). No new engine features.
- No edits to `docs/WARM-UP.md` or `AGENTS.md` (status updates are the
  controller's conversation with the owner).
- No pushes anywhere; commits are local.
- Never weaken or delete a test to make it pass; never fabricate an
  owner approval — claim only a real mediated `answer.json`.
- Solution files quote the reference byte-for-byte (extract + diff to
  verify).
- Gates per code slice: `cargo test -p patterns`,
  `cargo clippy --all-targets -- -D warnings`,
  `cargo fmt --all -- --check` (run from `gomoku/` in the worktree).
- Subagent pipeline per slice: implement → spec-compliance review →
  code-quality review → bounded amendments → re-review. Controller
  triages findings (approve / reject with reasoning / amend scope).

## Task 0: worktree, crate skeleton, Burn idiom spike

**Files:** worktree setup; `gomoku/Cargo.toml` (add member);
`gomoku/crates/patterns/Cargo.toml`, `src/lib.rs`, `src/naive.rs`;
`gomoku/crates/patterns/SPIKE.md` (running spike notes).

- Crate skeleton: deps `engine` (workspace), `burn` (workspace,
  `features = ["ndarray"]`); dev-deps `engine` + `testutil`, `rand
  = "0.10.3"`; feature `gpu = ["burn/wgpu"]`. Backend-generic code
  (`B: Backend`) from the first line.
- `naive.rs`: the learner's hand-rolled cross-correlation on plain
  Rust arrays (chapter 1's code) — contract: `cross_correlate(input:
  &[Vec<Vec<f32>>], kernel: &[Vec<Vec<f32>>], bias: f32) ->
  Vec<Vec<f32>>` with valid (no) padding, plus a ReLU wrapper.
- Spike A (idioms, recorded in SPIKE.md): manual weight assignment to
  a `Conv2d` module on `NdArray`; naive-vs-Burn equivalence test with
  small-integer random inputs/weights (exact f32 equality required);
  materialized-padding approach confirmed; the same manual-assignment
  idiom re-verified on `Wgpu` under `--features gpu` with a CPU-vs-GPU
  equality test.
- **Acceptance:** naive == Burn (exact) on NdArray and Wgpu; gates
  green; SPIKE.md records the exact 0.21 idioms with source citations.
- Commit: `feat(patterns): crate skeleton + conv idiom spike`.

## Task 1: planes, board generator, naive oracle

**Files:** `src/planes.rs`, `src/oracle.rs`, `tests/common/mod.rs`
(generator + helpers), oracle ASCII tests.

- `planes(b: &Board, s: Color) -> Planes` — two 23×23 channels
  (pad 4): channel 0 = stones of `s` (zero-padded), channel 1 =
  blocked = opponent stones or border (one-padded). Contract pinned:
  border cells are blocked, never stones.
- Test-only generator: seeded random playouts via engine rules,
  lengths 8..80 stones, stopped before terminal. Comment: duplicates
  datagen's playout deliberately; no `train` dependency allowed.
- `oracle.rs`: naive open-three enumerator implementing spec §3.4
  exactly (window patterns `_XXX_`, `_X_XX_`, `_XX_X_`, anti-four
  margins, per-direction); `open_three_makers(b, s) -> [MoveSet; 4]`
  (per direction) and `double_threes(b, s) -> MoveSet` (≥2 directions).
  Plain array scans; no bitboards, no conv; documented as reference
  oracle.
- ASCII unit tests (engine `testutil::board_from_ascii`): each pattern
  family fires / does not fire, edge-hugging threes, overlines,
  anti-four cases.
- **Acceptance:** gates green; oracle tests pin the spec semantics.
- Commit: `feat(patterns): planes + naive three-oracle`.

## Task 2: pattern table, network, differential gates

**Files:** `src/kernels.rs`, `src/net.rs`, `tests/differential.rs`;
SPIKE.md finalized.

- `kernels.rs`: declarative pattern table (shape, direction,
  required-stone offsets, required-empty offsets, output weight scale)
  for: five-completers (win map), open-four makers, broken-four
  makers, three makers (3 families). Generates 9×9 kernels + biases
  per §4.2 (strict margin ≥ 1 unit). Direction variants generated from
  base shapes. **Pin the channel count in SPIKE.md.**
- `net.rs`: `ThreatNet<B: Backend>` — `Conv2d` 9×9 (2 → N, valid,
  ReLU) → `Conv2d` 1×1 (N → M, ReLU); weights from the pattern table;
  `forward(planes) -> ThreatMaps { wins, double_threats,
  double_threes, threes_per_dir, fours_per_dir }`. Both colors as a
  batch of 2.
- `tests/differential.rs`: `wins == immediate_wins` and
  `double_threats == engine::double_threats` on ≥1000 seeded boards;
  `double_threes == oracle::double_threes` on ≥1000 seeded boards.
  Iterate on kernel enumeration until green — this task owns spec risk
  #1. Any deliberate semantic divergence found here must be resolved
  in favor of the oracle or escalated via the clarification protocol;
  never by weakening the gate.
- **Acceptance:** all three gates green at scale; gates + clippy +
  fmt; SPIKE.md complete (idioms, channel count, semantic decisions).
- Commit: `feat(patterns): hand-written threat maps + differential gates`.

## Task 3–8: chapter docs 01–06 (docs-only slices)

Reference code for these chapters is already verified (T0–T2). Each
slice writes the chapter + solution in `docs/tutorials/convolutions-tutorial/`
(main repo) following the mcts/datagen conventions: abstract-ish
intro, learning goals, contracts, TDD checklist (test names must match
the reference), Done-when with `feat(patterns): …` message, Next link;
solution quotes byte-verified.

- **T3** `01-planes-and-a-conv-by-hand.md` — planes (T1), `naive.rs`
  (T0); hand-computed tiny examples.
- **T4** `02-trusting-the-framework.md` — Burn tensors, `Conv2d`,
  manual weight assignment, exact-equality differential vs `naive.rs`;
  backend-generic signature, `NdArray` default.
- **T5** `03-the-kernel-language.md` — exact-pattern kernels
  (+w/−w/bias/ReLU, margin), five-completer kernels; ASCII unit tests.
- **T6** `04-the-four-level.md` — open/broken four makers, counting
  combiner (1×1, channels carry counts), engine differential gate.
- **T7** `05-three-makers.md` — the three pattern families as kernels,
  per-direction maps; maturation-room simplification + exercise.
- **T8** `06-the-fork.md` — direction combiner, `double_three_map`,
  the naive oracle, three-level gate; edge/overline cases.
- Commits: `docs(tutorials): convolutions chapter N`.

## Task 9: potential, heatmap demo, GPU section (code + docs)

**Files:** `src/potential.rs`, `src/bin/patterndemo.rs`, GPU test;
`07-seeing-it.md` + solution.

- `potential.rs`: maps → `policy(b, s) -> Vec<(Move, f32)>` (sparse,
  `Sample`-shaped; combination weights `[experiment]`, documented).
- `patterndemo`: renders board + selected threat map as ANSI heatmap;
  built-in showcase positions (classic fork, corner fork from engine
  tests); exact-cell-set regression tests.
- GPU: `--features gpu` equality test (CPU vs Wgpu, exact) + demo
  flag; documented environment-dependence.
- Chapter 7 docs + solution. Full review cycle (this is a code slice).
- Commits: `feat(patterns): threat potential + heatmap demo + gpu`;
  `docs(tutorials): convolutions chapter 7`.

## Task 10: acceptance chapter + crate polish

**Files:** `#![deny(missing_docs)]` pass; scale-up of gates if needed;
`08-acceptance.md` + solution.

- Polish pass (accumulated minors), `#![deny(missing_docs)]`, final
  numbers for the acceptance chapter (gate board counts, timings,
  channel count) — measured, never invented.
- Full review cycle. Commits: `feat(patterns): crate polish`;
  `docs(tutorials): convolutions chapter 8`.

## Task 11: primer + README (docs-only)

**Files:** `docs/tutorials/convolutions-tutorial/00-convolutions-primer.md`,
`README.md`.

- Primer = design authority: threat ladder semantics, kernel language,
  oracle ladder, backend story, what the tutorial defers (training,
  mcts integration, maturation room, TSS). Modeled on the datagen
  primer. README = chapter index + how-the-tutorial-works.
- Commit: `docs(tutorials): convolutions primer + README`.

## Task 12: final QA + cleanup

- Independent QA: workspace gates + `--features gpu` gates; quote-drift
  audit (every solution block vs reference, byte-identical);
  consistency read of all 10 docs; hygiene (`git status` clean both
  repos, no strays).
- Controller fixes or dispatches bounded fixers for findings.
- **Cleanup per convention:** `git worktree remove`, delete
  `reference/convolutions`. The tutorial's solutions are the durable
  record.
- Report to owner; WARM-UP/AGENTS status update is a separate
  conversation.

## Sequencing

Strictly sequential (all tasks touch shared files). T0–T2 are the
risk-bearing spike; no chapter docs begin until the differential gates
are green at scale and the channel count is pinned.
