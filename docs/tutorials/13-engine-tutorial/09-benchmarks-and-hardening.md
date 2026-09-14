# Slice 9 — Benchmarks and hardening

Correct first, fast second — now it is second's turn. Verify the
milestone-1 performance bar, then lock the crate's public surface.
Design: ch. 13, "Test plan"; acceptance: ch. 12, §13 milestone 1.

## Part A — the benchmark

`crates/engine/benches/win_detection.rs`:

```rust
use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn bench_has_five(c: &mut Criterion) {
    // build one realistic mid-game bitboard (~60 stones)
    c.bench_function("has_five_any", |b| {
        b.iter(|| has_five_any(black_box(&board)))
    });
}

criterion_group!(benches, bench_has_five);
criterion_main!(benches);
```

`Cargo.toml` needs:

```toml
[[bench]]
name = "win_detection"
harness = false
```

**The bar: ≥ 50M checks/s** (one call ≤ 20 ns). Also add a
`play`/`undo` throughput bench — no fixed bar, but record the number in
your commit message; it is the baseline every later "optimization" must
beat or justify.

## Rust toolbox

**`harness = false`.** Criterion replaces Rust's built-in test harness
with its own `main` (the `criterion_main!` macro) — that is why the
target needs the flag.

**`black_box`.** The optimizer is allowed to delete computations whose
results are unused — your benchmark would measure nothing.
`black_box(x)` is an optimization barrier: forces `x` to exist and the
call to happen. Every benchmark input and result passes through it.

**Reading the output.** Criterion reports a distribution, not a number.
Watch the *mean* against the bar, and re-run after touching anything in
`bitboard.rs` or `win.rs` — regressions hide in innocent refactorings.

## Part B — hardening

1. **Visibility sweep.** `cargo doc -p engine --no-deps`, open the
   output, read the public API like a stranger. Everything visible
   should be in ch. 13's re-export list. Anything else: `pub(crate)` it.
   `Bitboard` must not leak — if `stones()` returns it publicly, switch
   to an iterator of `Move` (breaking the leak is exactly what slice 3's
   "your call" was about).
2. **Rustdoc pass.** Every public item: one-line summary, `# Errors`
   where it returns `Result`, an example on `Board::play` and `encode`
   (doc-examples run as tests — `cargo test` executes them).
3. **Feature check.** `cargo check -p engine --features testutil` and
   a plain `cargo check -p engine --release` — both clean.
4. **Full gates.** `cargo test -p engine` (all suites, 10k-case
   proptests included), `cargo clippy --all-targets -- -D warnings`,
   `cargo fmt --all`.

## Part C — the acceptance walkthrough

Chapter 12, milestone 1, line by line:

| Criterion | Evidence |
|---|---|
| property tests green | `cargo test -p engine` output |
| perft-style: 10k random games match reference | differential test log |
| `has_five` ≥ 50M checks/s | criterion output |

Paste the three outputs into the commit body. That is the discipline:
evidence in the commit, not claims in the chat.

## Systems refresh: why measure now, not "when it's slow"

Every later subsystem (MCTS, self-play) budgets its design around engine
speed — simulations per move, games per hour, worker counts. Those
numbers are `[derived]` in ch. 12, §9 and are only as good as this
measurement. A benchmark that lives in the repo (and re-runs on demand)
turns "feels fast enough" into a regression alarm. This is the same
reason ML training runs log throughput every step: you cannot tune what
you do not measure, and you cannot notice what you stopped measuring.

## Done when

The acceptance table has real outputs in it. Commit:
`perf(engine): benchmarks + milestone-1 acceptance evidence`.

Then: milestone 1 is **done**. Celebrate briefly — milestone 2 is MCTS,
and its design conversation starts the same way this one did.
