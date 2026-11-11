# Shortest forced-win lines — handoff

## What changed

- `crates/engine/src/tss.rs`: added `prove_shortest_forced_win`. It runs the ordinary prover first, then iteratively re-searches with one fewer attacker move per round. A single `Search` shares the total node budget across the initial proof and every minimization round. Both the initial proof and every improvement pass `verify_line`; budget exhaustion returns the shortest verified proof found so far. `prove_forced_win` is unchanged.
- `crates/engine/src/lib.rs`: re-exported the new function. No new engine dependencies or `SearchBudget` changes.
- `crates/player/src/lib.rs`: the certified own-side TSS bypass in `AiPlayer::decide` now plays the first move of the shortest verified line. Guard and resignation probes still use `prove_forced_win`; absent own proof still falls through to ordinary MCTS.
- `crates/player/tests/decisions.rs`: added an integration regression proving that the own-proof path chooses the shorter line's different first move without running the evaluator.
- `docs/WARM-UP.md`: added the requested brief status note.

Engine tests cover a non-minimal ordinary proof reduced to a verified one-attacker-move proof, exact initial-budget exhaustion returning the original verified line, and no-proof positions returning `None`.

## New public API

```rust
pub fn prove_shortest_forced_win(b: &Board, side: Color, budget: SearchBudget) -> Option<Proof>
```

Available as `engine::prove_shortest_forced_win`. `budget.max_depth` retains its existing ply-based meaning for the initial search; `budget.max_nodes` caps the initial search plus all minimization rounds combined.

## Commit and push

- Commit: `7cd0b7d059bb9ae2ba5ef11d2f86020a7b50e9ff`
- Message: `feat(engine): shortest forced-win proofs via iterative deepening`
- Pushed: `origin/main`; local `HEAD` and `origin/main` both resolve to the hash above.
- The commit contains only the five intended files. Unrelated owner changes, including the pre-existing staged `crates/net/src/bin/nettrain.rs`, were not included or altered.

## Gate evidence

Run from `gomoku/` after the final edits:

- `cargo fmt --all` and `cargo fmt --all -- --check`: exit 0.
- `cargo test`: exit 0; **720 passed, 0 failed, 7 ignored, 0 measured** across the full workspace.
- `cargo clippy --all-targets -- -D warnings`: exit 0; no warnings or errors.
- Additional review checks: `cargo check -p engine`, engine/player tests, and scoped clippy passed; independent review found no blocking issues.
- The pre-commit hook also ran `cargo test` successfully. The push completed normally without bypassing hooks.

## GUI consumer usage

Replace `engine::prove_forced_win(&board, side, budget)` with `engine::prove_shortest_forced_win(&board, side, budget)`; the arguments and `Option<Proof>` return type are identical.
