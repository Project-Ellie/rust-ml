# Sub-chapter 08 — The TSS eye

## Abstract

This sub-chapter gives the baseline seat a root-level threat-space
search eye. The hand-written policy and value readouts see one move
deep. A real Gomoku win is often a ten-move forcing sequence whose
individual moves look modest. The engine already owns sequence truth
through `prove_forced_win`. This chapter wires that prover into the
seat's step path as a once-per-move short-circuit. When the prover
returns `Some(proof)`, the seat plays `proof.line[0]` instantly and the
MCTS search is skipped. When it returns `None`, the seat falls back to
the normal search. `None` means only "not proven within budget"; it
never means "no win exists".

## Glossary

| Term | Definition |
|------|------------|
| **TSS eye** | The root-level call to `engine::prove_forced_win` that lets the seat see proven forced wins. |
| **Root probe** | A once-per-move call to the prover at the root of the search, outside the evaluator. |
| **`baseline::probe::ROOT_BUDGET`** | The pinned `SearchBudget` for the root probe: `max_nodes: 10_000`, `max_depth: 30`. |
| **`baseline::probe::root_probe`** | The public function that runs `prove_forced_win` for the side to move. |
| **`engine::prove_forced_win`** | The engine's bounded threat-space search. `Some` is a certified win; `None` is only "not proven within budget". |
| **`engine::verify_line`** | The soundness net that replays a proof line and rejects doctored proofs. |
| **`engine::SearchBudget`** | The budget struct with `max_nodes` and `max_depth`. |
| **`engine::Proof`** | The certified win: `winner` and `line`, a vector of moves starting with the attacker's first threat. |
| **VCF** | Victory by continuous forcing: a sequence of threats the defender must answer until the attacker wins. |
| **Defender seat** | The human seat opposite the puzzle attacker; the AI plays the attacker. |
| **Per-node probe** | A probe inside the evaluator's `evaluate` call. This chapter deliberately forbids it. |

## Context

Sub-chapters 01 through 06 built the baseline evaluator: a policy
readout with a trust ladder from proven wins down to gravity, and a
saturating value readout. Those readouts answer one question per call:
"what does this position look like right now?" They are fast enough to
call thousands of times per move, and the MCTS search calls them at
every leaf.

But Gomoku tactics are sequences. A VCF win is a chain of forcing moves
that the static maps cannot see as a single cell. The engine already
has a soundness-first threat-space search in `engine::prove_forced_win`.
It returns either a certified `Proof` or `None`. The `None` value is
intentionally weak: it says the budget ran out before a proof was
found, not that the position is a draw or a loss.

This chapter adds the cheapest possible integration: call the prover
once per move, before the search starts. If it certifies a win, play
the first move of the certified line. If not, let the normal MCTS step
run. The probe never lives inside the evaluator, because the evaluator
answers roughly 4,000 questions per move and a prover call can explore
thousands of threat-space nodes.

## Intention

The learner enables this behavior:

1. The baseline seat detects proven forced wins at the root before it
   spends simulations.
2. A certified win short-circuits the search: the seat plays
   `proof.line[0]` through the normal move path, so events and
   rendering stay unchanged.
3. A quiet board or an unproven position falls through to the normal
   MCTS step.
4. `None` is never read as "no win exists".
5. There is no defensive probe and no per-node probe.

## Specification as tests

### Baseline probe tests (`baseline/src/probe.rs`)

* `probe_finds_vcf_line` — replay the engine's win-in-3 script, call
  `root_probe` with `ROOT_BUDGET`, and assert that a proof is found for
  Black. The proof's `winner` is Black, the line is non-empty, and
  `engine::verify_line` accepts it.
* `probe_quiet_position_is_none` — a fresh empty board returns `None`.
* `probe_respects_budget` — the same VCF position returns `None` when
  the budget is too small (`max_nodes: 1`) or too shallow
  (`max_depth: 2`).

### CLI integration test (`cli/tests/acceptance.rs`)

* `baseline_root_probe_plays_proof_first_move` — start a session from
  the same win-in-3 opening with the Black seat set to
  `EvaluatorKind::Baseline`, execute one `Step`, and assert that the
  emitted AI move equals `proof.line[0]`. The probe, not the search,
  chose the move.

## Walkthrough

The probe is a thin wrapper over engine truth. The real work is
placement: the probe lives in the CLI's AI-seat step path, not inside
the evaluator.

### 1. The evaluator answers too many questions per move

A move at 4,000 simulations calls the evaluator roughly 4,000 times.
Each call must be cheap: a few convolutions on the flex device. A
threat-space search can explore thousands of nodes. If the prover lived
inside the evaluator, a single move could cost orders of magnitude more
than the budget allows. The primer §2 makes this arithmetic explicit.

### 2. The probe module is added to the baseline crate

`baseline/src/probe.rs` is a new module. It exposes one constant and one
function. It depends only on `engine` — no Burn, no patterns, no MCTS.

`ROOT_BUDGET` is a `pub const SearchBudget` with `max_nodes: 10_000` and
`max_depth: 30`. The doc comment marks it `[experiment]`: the numbers
are pinned starting values, not eternal truth.

`root_probe(board, budget)` calls `engine::prove_forced_win(board,
board.to_move(), budget)` and returns its `Option<Proof>`. Its doc
comment carries the engine's warning verbatim: `None` only means "not
proven within budget" and says nothing about the true value.

### 3. The crate root exposes the module

`baseline/src/lib.rs` adds `pub mod probe;` so the CLI can reach the
function and the constant.

### 4. The CLI step path checks the probe first

`cli/src/session.rs::execute_step` already decides whether the side to
move is an AI seat. After it has the AI config, it checks whether the
seat's evaluator is `EvaluatorKind::Baseline`. If so, it calls
`baseline::probe::root_probe(&board, baseline::probe::ROOT_BUDGET)`.

This call is feature-gated with `#[cfg(feature = "baseline")]` so the
CLI compiles when the feature is off.

### 5. A proof pre-empts the search

If the probe returns `Some(proof)`, the step extracts `proof.line[0]`
and calls `execute_play(mv)`. This is the same move path a searched
move uses, so the emitted events, the history update, and the rendering
stay identical. The only difference is that no search ran.

### 6. `None` falls through to normal search

If the probe returns `None`, `execute_step` continues with the normal
MCTS path: build the evaluator, run `mcts::search`, add Dirichlet noise
if configured, select a move, and play it. `None` is not a signal to
panic or to change the search config; it is simply "the prover had
nothing to say, so estimate the move instead."

### 7. There is no defensive probe

If the opponent has a proven forced win, the position is already lost.
Spending budget to confirm that changes no decision. The value readout's
opponent-double-threat rung already paints such positions as lost. So
the seat never probes to prove its own loss.

### 8. There is no per-node probe

The `Evaluator` seam speaks `EvalRequest`, which carries no depth or
budget information. A per-node probe would be both unaffordable and
architecturally smuggled. The root probe lives in the CLI seat path,
where "once per move" is enforced by structure, not by discipline.

## TDD checklist

Work one test group at a time, red to green.

From `baseline/src/probe.rs`:

1. `probe_quiet_position_is_none`
2. `probe_respects_budget`
3. `probe_finds_vcf_line`

From `cli/tests/acceptance.rs`:

4. `baseline_root_probe_plays_proof_first_move`

## Verification commands

From `gomoku/`:

```text
cargo test -p baseline
cargo test -p cli --test acceptance
```

When the CLI is touched, also run the full CLI test suite.

## Capstone

Take the defender seat and watch the baseline seat execute a forcing
line instantly:

```text
cargo run -q -p cli --bin gomoku -- play --puzzle data/puzzles/kaibao-vcf-676.json --index 0 --ai baseline
```

The puzzle attacker is to move. You sit opposite the AI. Step once. The
baseline seat should play the first move of the engine-certified proof
immediately, with no visible search delay. Step a few more times and
watch the forcing line unfold.

Then try an index deeper in the same pack:

```text
cargo run -q -p cli --bin gomoku -- play --puzzle data/puzzles/kaibao-vcf-676.json --index 12 --ai baseline
```

Some positions solve in one move; some need a short sequence. If the
probe returns `None`, the seat falls back to MCTS and the move takes
longer. That difference is the TSS eye opening and closing.

## Done when

* `cargo test -p baseline` passes.
* `cargo test -p cli --test acceptance` passes.
* `cargo clippy --all-targets -- -D warnings` passes.
* `cargo fmt --all -- --check` makes no changes.
* `baseline/src/probe.rs` is fully documented and has no warnings under
  `#![deny(missing_docs)]`.
* The commit is `feat(baseline): the TSS eye — baseline sub-chapter 08`.

## References

* [Baseline primer](../00-baseline-primer.md) — §7 pins the root-only
  probe design and the meaning of `None`.
* [Sub-chapter 06 — The three ladder](../06-the-three-ladder/the-three-ladder.md)
  — the readout layer the probe sits on top of.
* [`gomoku/crates/engine/src/tss.rs`](../../../../gomoku/crates/engine/src/tss.rs)
  — `prove_forced_win`, `SearchBudget`, `Proof`, `verify_line`, and the
  documented `None` warning.
* [`gomoku/crates/baseline/src/probe.rs`](../../../../gomoku/crates/baseline/src/probe.rs)
  — the probe module.
* [`gomoku/crates/baseline/src/lib.rs`](../../../../gomoku/crates/baseline/src/lib.rs)
  — the `pub mod probe;` declaration.
* [`gomoku/crates/cli/src/session.rs`](../../../../gomoku/crates/cli/src/session.rs)
  — the AI-seat step path where the probe is called.
* [`gomoku/crates/cli/tests/acceptance.rs`](../../../../gomoku/crates/cli/tests/acceptance.rs)
  — the `baseline_root_probe_plays_proof_first_move` integration test.
