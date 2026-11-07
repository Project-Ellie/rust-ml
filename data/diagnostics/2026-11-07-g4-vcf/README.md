# Captured position: White g4 analysis (2026-11-07)

## Abstract

This offline diagnostic establishes that the captured 42-stone position is already lost for Black: every one of its 183 legal moves allows an independently verified White forcing-win certificate. Neither e7/f6 nor any alternative can save this position. A fresh real-player 10,000-simulation decision timed out at the required 10-second cutoff. Smaller real-player decisions demonstrate five certified guard refutations followed by an unchecked sixth candidate, but do not establish the exact live decision path. No production changes or live-process interactions were made. The saved artifact is a position fixture, not a native game record.

## Glossary

- **VCF:** victory by continuous fours, a forcing sequence of four-stone threats.
- **TSS:** the engine's bounded threat-space search, which attempts a forcing-win proof.
- **MCTS:** Monte Carlo tree search, the AI's ordinary move search.
- **Guard:** the player-level check for a verified opponent win after a proposed move; default limits are five candidate checks, each with 1,000 nodes and depth 10.
- **NoProof / UNKNOWN:** no proof obtained within a budget; not evidence of safety or a draw.
- **Proposal:** the move selected before the guard runs; distinct from the final returned move.

## Preserved evidence and limits

[terminal.txt](terminal.txt) is the unchanged parent capture (SHA-256 `ac3b990b243df7f8fb2bb68a14350d2b8198503a5b84a211632b3213f363cdad`). Its 15 board rows yield 21 Black and 21 White stones. Black is to move at ply 42; Black is AI(10000), White is Human. Visible variations are e7 and f6. The visible path suffix is `n8 l8 e8 h5 n9 o10 d9 c10`. Neither the full 42-move history nor the branch random-number-generator state is known. They have not been inferred or invented.

Parent-supplied process metadata (not independently inspected here): PID 68910, started `2026-11-07 12:08:55`, executable modification time `12:08:45`, command:

```text
./target/release/gomoku play --ai baseline --sims 10000 --no-swap2 --temperature 1.0 --seed 3477
```

The source is main `a64420f740558fb37bcacb060fa28f13f31001b8` plus the existing uncommitted fork bonuses. The separate diagnostic executable uses current source; no claim is made that its bytes match the live executable. No keys, signals, attach, or game advancement were sent to PID 68910. The subprocess timeout applies only to newly spawned diagnostic children.

[metadata.json](metadata.json) records provenance and unknown fields. [capture.mjs](capture.mjs) parses board rows, treating `x` as Black, `o` as White, and `.`/`+` as empty; brackets and cursor decorations are removed. [position.json](position.json) is a CLI puzzle pack with `v=1`, `count=1`, item `i=0`, `a="black"`, `d=0` (unclaimed depth), zero-based `[x-column,y-row]` stone arrays, and empty `l`. The actual `cli::puzzle::PuzzlePack` parser accepted it; the example independently asserted counts and side to move. This validates the board fixture, not historical move order or a complete saved game.

## Load the board

These commands start a **new** study table, not the existing live process.

From the repository root:

```bash
gomoku/target/release/gomoku play --puzzle data/diagnostics/2026-11-07-g4-vcf/position.json --index 0 --no-swap2
```

From `gomoku/`:

```bash
./target/release/gomoku play --puzzle ../data/diagnostics/2026-11-07-g4-vcf/position.json --index 0 --no-swap2
```

The empty claimed solution deliberately avoids labeling a Black-to-move fixture with a White-to-move continuation.

## Verified forcing line

The owner's g4 claim was initially unverified. After each of Black e7 and Black f6, the explicit `Proof { winner: White, line: [g4,f3,h4] }` passes the actual engine `verify_line`.

1. Before White g4, neither color has an immediate winning move.
2. White g4 creates one immediate winning cell, f3, along `f3-g4-h5-i6-j7`. Black has no immediate win, so f3 must be blocked.
3. After Black f3, neither color has an immediate winning move.
4. White h4 forms `f4-g4-h4-i4`, leaving winning cells e4 and j4. Black still has no immediate win. One Black placement cannot block both.

The verifier accepts termination at this double threat; a completed five need not be included. The engine's depth-10 prover chooses a different, longer verified White line: `f3 g4 k4 l3 l5 i5 j3`. Here the alternating colors begin with White. Depth 3 finds exactly `g4 f3 h4`. A deeper search returning a longer line does not invalidate the short proof.

## Actual budget grid

All runs completed, all returned proofs passed `verify_line`, and no subprocess timed out. Times below are prover-only milliseconds, excluding compilation and independent verification; [results.json](results.json) also includes process wall time and [results.txt](results.txt) contains literal output. The first release build took 22.88 seconds.

| After Black | Nodes | Depth | White proof | Prover ms |
|---|---:|---:|---|---:|
| e7 | 1,000 | 10 | f3 g4 k4 l3 l5 i5 j3 | 13.646 |
| e7 | 10,000 | 10 | f3 g4 k4 l3 l5 i5 j3 | 11.776 |
| e7 | 1,000 | 3 | g4 f3 h4 | 1.659 |
| e7 | 10,000 | 3 | g4 f3 h4 | 1.563 |
| e7 | 10,000 | 30 | f3 g4 k4 l3 l5 i5 j3 | 79.941 |
| f6 | 1,000 | 10 | f3 g4 k4 l3 l5 i5 j3 | 9.019 |
| f6 | 10,000 | 10 | f3 g4 k4 l3 l5 i5 j3 | 8.983 |
| f6 | 1,000 | 3 | g4 f3 h4 | 1.294 |
| f6 | 10,000 | 3 | g4 f3 h4 | 1.324 |
| f6 | 10,000 | 30 | f3 g4 k4 l3 l5 i5 j3 | 88.390 |

### Proposed root defenses

Blocking this particular three-ply line is not equivalent to defending the position. At the default 1,000-node/depth-10 budget:

| Black root move | Specific g4 f3 h4 verifies? | Other verified White proof | Prover ms |
|---|---|---|---:|
| g4 | No (g4 occupied) | k4 l3 l5 i5 j3 | 24.423 |
| f3 | No (f3 occupied) | k4 l3 l5 i5 j3 | 24.414 |
| h4 | No (h4 occupied) | f3 g4 k4 l3 l5 i5 j3 | 8.934 |

All three proposed defenses are therefore refuted by actual proofs. The exhaustive follow-up below now establishes the stronger result for every legal Black move. NoProof remains UNKNOWN, never safe.

## Actual controlled guard outcomes

The standalone example uses public `AiPlayer`, a fixed evaluator strongly preferring e7 or f6, zero MCTS simulations, temperature zero, own-side TSS disabled, and no noise. It asserts the requested move really was the original proposal. It then runs the real default guard with the real engine prover and verifier, without injected proof results.

| Original proposal | Refuted sequence | Checked | Final choice | Outcome | Decision ms |
|---|---|---:|---|---|---:|
| e7 | e7, a1, b1, c1, d1 | 5 | e1 | BudgetExhausted | 45.301 |
| f6 | f6, a1, b1, c1, d1 | 5 | e1 | BudgetExhausted | 45.346 |

`e1` is an **unchecked** sixth candidate, not certified safe. Remaining fixed-evaluator priors tie, so the alternatives follow move index. These are not the live MCTS rankings. Seed 3477 initializes only the isolated example's generator; it does not reconstruct the live branch generator state.

## Exhaustive root certificate (authorized offline follow-up)

[Root certificates](root-certificates.json) enumerate all 183 distinct legal Black moves, in move-index order. Each successor is a fresh clone. For each successor the diagnostic records its terminal status and independently calls the actual engine verifier on **all three** known White lines; a line is admitted only when its winner matches the successor's side to move, its first move is legal, and verification succeeds. No proof is transferred between different boards.

| Candidate White line (alternating colors, White first) | Verified and admitted successors |
|---|---:|
| g4 f3 h4 | 178 / 183 |
| k4 l3 l5 i5 j3 | 176 / 183 |
| f3 g4 k4 l3 l5 i5 j3 | 172 / 183 |
| Union of these certificates | **183 / 183** |

There are 549 independent line checks: 526 verify and are admitted, 23 fail. All 183 successors have status `Ongoing`; none is a Black terminal win or draw. No move is missing or duplicated, and no successor remains unknown. The three known lines cover every successor, so the conditional default-TSS and optional deeper-TSS stages require **zero calls**. This conclusion is an exhaustive certificate, not an inference from the guard's five-candidate cap or from NoProof.

The separate actual Black own-side probe at the root (10,000 nodes / depth 30) returns `NoProof=UNKNOWN`, not a certified move. Neither completed real-player decision bypasses search with an own proof. This negative probe is not the evidence that Black loses; the exhaustive White certificates are.

**Root-cause finding at this captured position:** there is no saving Black alternative for the guard or ordinary search to select. Interpreting e7/f6 as a failure to choose g4, f3, h4, or another surviving defense is contradicted by the certificates. This does not identify which earlier move first lost the game: the full history is unavailable.

## Fresh real-player decisions, not live replay

The [follow-up runner](followup.mjs) invokes actual `AiPlayer` with `BaselineEvaluator(Device::flex())`, including the existing fork bonuses, baseline own-side probe and baseline guard. Every run starts with a fresh `StdRng` seeded 3477, temperature 1.0, noise epsilon 0, and c_puct 1.5. The current CLI's explicit temperature disables its opening schedule; ply 42 is also outside the schedule. Search ordering is visits descending, then prior descending, then move index ascending. The complete 183-edge visits/prior ordering is preserved for each completed run in [machine results](followup-results.json), alongside [literal stdout](followup-stdout.txt).

Compilation was separate. The **10,000-simulation runtime timed out**: Node reported `ETIMEDOUT`, child signal `SIGKILL`, wall time 10,003 ms for the configured 10,000 ms cutoff. Only that newly spawned child was killed. It printed its start/options marker but no decision; no 10,000-simulation proposal, final choice or ranking is claimed. The cutoff was not increased.

The permitted smaller fixed-simulation runs are causal guard traces, **not live 10,000-simulation reproductions**:

| Simulations | Original proposal (rank) | Five refuted candidates, in order | Returned unchecked sixth (rank) | Outcome | Decision ms |
|---|---|---|---|---|---:|
| 100 | n7 (6) | n7, m7, n6, e10, g12 | c8 (5) | BudgetExhausted | 264.515 |
| 1,000 | m7 (2) | m7, l9, e7, n6, e11 | h4 (6) | BudgetExhausted | 1776.263 |

The sixth **checked-order candidate** need not have search rank six: the sampled proposal is checked first and removed from the ranked alternatives. Root edge visit sums equal 100 and 1,000 respectively.

| Simulations | e7 rank | f6 rank | g4 rank | h4 rank | f3 rank |
|---|---:|---:|---:|---:|---:|
| 100 | 23 | 19 | 11 | 10 | 54 |
| 1,000 | 3 | 24 | 14 | 6 | 54 |

These completed real-player traces directly demonstrate the cap mechanism with actual search rankings, rather than the earlier fixed-evaluator ranking. At 1,000 simulations e7 is explicitly refuted, not returned. Neither run returns e7/f6 as an unchecked sixth candidate; therefore neither establishes that mechanism for the live e7/f6 choices. The exact live proposal, branch RNG state, 10,000-simulation ranking, earlier losing move, and binary/source identity remain unknown. A fresh seed is not the original branch generator state. The captured root's lost status is established independently of those unknowns.

## Reproduction and focused checks

From the repository root:

```bash
(cd gomoku && cargo build --release --locked -p cli --example diagnose_g4)
node data/diagnostics/2026-11-07-g4-vcf/followup.mjs
# Optional: reproduce the earlier controlled probes (does not rewrite captures).
node data/diagnostics/2026-11-07-g4-vcf/probe.mjs
rustfmt --edition 2024 --check gomoku/crates/cli/examples/diagnose_g4.rs
(cd gomoku && cargo clippy --release --locked -p cli --example diagnose_g4 -- -D warnings)
```

Do not rerun the historical capture generator here: terminal, position and metadata are immutable evidence. The follow-up runner overwrites only its own generated results. It runs ten exhaustive chunks of at most 20 successors, the own-side probe, and each real-player decision with Node `spawnSync`, a 10,000 ms timeout and `SIGKILL` only for that child. Compilation is separate. Focused diagnostic formatting and clippy checks passed; [follow-up checks](followup-checks.txt) record immutable hashes and the unchanged pre-existing tracked production/documentation diff. Earlier checks remain in [checks.txt](checks.txt).

Independent review approved the diagnostic. It reproduced all 549 line checks with a separate coordinate-based five-in-a-row oracle, confirming the same 183 covered successors without relying on engine bitboards. It also reproduced the completed 100- and 1,000-simulation traces. Subsequent workspace QA passed: `cargo test --release --locked --all-targets` (608 passed, 0 failed, 7 ignored), `cargo test --release --locked --doc` (3 passed), `cargo clippy --locked --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --check`. No production fix, live-game interaction, commit, or push was performed.

## References

- [Immutable terminal capture](terminal.txt), [position pack](position.json), [process metadata](metadata.json), and [earlier literal probe output](results.txt).
- [Exhaustive per-move certificates](root-certificates.json), [follow-up machine results](followup-results.json), [literal follow-up stdout](followup-stdout.txt), [bounded runner](followup.mjs), and [focused checks](followup-checks.txt).
- [Standalone diagnostic example](../../../gomoku/crates/cli/examples/diagnose_g4.rs).
- [Puzzle pack parser](../../../gomoku/crates/cli/src/puzzle.rs): validates coordinates, counts, side to move and solution schema.
- [Engine prover and verifier](../../../gomoku/crates/engine/src/tss.rs): bounded proof search and exhaustive checks of required defender replies.
- [Player guard](../../../gomoku/crates/player/src/guard.rs): five-check cap, verified refutations and unchecked next-candidate fallback.
- [CLI player settings](../../../gomoku/crates/cli/src/session.rs): baseline seats enable the own-side proof probe and defensive guard; explicit selection and ply ≥10 disable the opening schedule.
- [Player decision and own-proof admission](../../../gomoku/crates/player/src/lib.rs): proof-first decision, actual search statistics, proposal selection, then guard.
- [Baseline evaluator](../../../gomoku/crates/baseline/src/lib.rs): current evaluator used in fresh real-player runs.
