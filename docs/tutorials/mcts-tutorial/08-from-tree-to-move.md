# Chapter 08 — From tree to move

## Context

Chapter 07 left you with a `SearchOutcome`: a tree whose root node has
225 outgoing edges, each carrying a visit count. The search has done
its work. Now you must turn those counts into two different things:

* a move to play on the board;
* the improved policy π that the network will later be trained to
  predict.

These come from the same distribution, but they are consumed
differently. Self-play games sample from it to keep opening diversity;
arena games and late-game positions take the argmax. This chapter
builds the policy layer: extract counts, apply temperature, and add
Dirichlet noise at the root (primer §5).

## Intention

By the end of this chapter you will have:

* `visit_distribution(tree, root) -> Vec<(Move, u32)>` — the raw
  visit-count vector;
* `select_move(dist, temperature, rng) -> Option<Move>` — temperature
  sampling or deterministic argmax;
* `add_dirichlet_noise(tree, root, epsilon, alpha, rng)` — root-only
  prior perturbation for self-play;
* unit tests covering argmax, tie-breaking, sampling under a fixed
  seed, and the noise invariant (sums to one, touches only the root).

You will *not* wire any of this into a game loop. Milestone 2 is still
building pieces; the self-play crate will turn the knobs in milestone
5.

## Mental mapping

### π is the visit-count distribution

The improved policy π is, by definition, the distribution of visits
over the root's legal moves. Not Q. Not prior. Not some blended score.
Visit counts (primer §5; primer §9 bug #5).

This is the economy of AlphaZero: the same search produces both the
move you play and the label you train on. There is no separate
"playing policy" and "training policy". The network's raw `p` is fed
into PUCT; PUCT spends simulations; the resulting `N` vector is π.
Training teaches the network to predict π, so next time the raw `p`
looks a little more like the search's opinion.

> **Excursion — why counts and not values?**
>
> `Q` tells you how good a move has been on average, but it is a mean
> over a small sample and it can be noisy. `N` tells you where the
> search actually spent its budget, which encodes both quality and the
> exploration pressure from the prior. Empirically, `N` is a better
> training target. If you train on `Q`, you train a value network with
> extra steps; if you train on `N`, you train a policy network that
> learns to imitate the search's allocation.

### Temperature dissected

Temperature τ controls how stochastic the move selection is:

* **τ → 0**: play the most-visited move. Deterministic, maximum
  strength. Used in competitive play and after move 12 of self-play
  (primer §5).
* **τ = 1**: sample with probability proportional to `N`. Used during
  the first 12 moves of self-play to keep opening games diverse.
* **General τ > 0**: sample with probability proportional to
  `N^(1/τ)`. Higher τ flattens the distribution; lower τ sharpens it.

The first-12 threshold is `[derived]` from AlphaZero's 30 moves on
~200-move Go games, scaled to Gomoku's ~60-move games (primer §5).
Like every number in this project, it is an experiment waiting to
happen.

Numerical care: counts are `u32`, but exponentiation needs floats.
Convert each count to `f32`, raise it to `1.0 / τ`, accumulate in
`f64` if you are summing many tiny weights, then sample by cumulative
threshold against a uniform draw. Watch for the degenerate case where
all counts are zero (for example, before any simulations); a uniform
fallback is reasonable there.

### Deterministic tie-breaking in argmax

When τ is near zero, `select_move` must return the most-visited move.
If several moves share the maximum visit count, the choice must be
deterministic. The reference breaks ties by lowest `Move::index`
(i.e., lowest `(row, col)` lexicographically).

Why does this matter? Two reasons:

* **Tests.** A stochastic tie-break makes tests flaky, and flaky
  tests in stochastic code are usually a seed you forgot to fix.
* **Arena reproducibility.** Two runs with the same seed and model
  should produce the same game. Deterministic tie-breaking removes
  one source of divergence.

### Seeded RNG everywhere

`rand` is allowed in the `mcts` crate, unlike the engine, but every
random consumer takes `&mut impl Rng`. Tests pass `StdRng::seed_from_u64`,
so sampling behavior is reproducible across runs and across machines.

There are two random consumers in this chapter:

* `select_move`, for temperature sampling;
* `add_dirichlet_noise`, for the η sample.

Both signatures expose `rng: &mut impl Rng`. Do not hide a global
random source. A flaky test in stochastic code is usually a seed you
did not fix — fix it by threading the RNG through every call.

### Dirichlet noise at the root (self-play only)

At the root of every self-play search, the priors are perturbed:

```text
P'(a) = (1 − ε)·P(a) + ε·η_a,     η ~ Dirichlet(α)
ε = 0.25
α = 0.1
```

The purpose is the exploration floor. A young network can be
overconfident and collapse almost all prior mass onto one or two
moves. Dirichlet noise guarantees every root move keeps a nonzero
prior, so PUCT's exploration term can never be zeroed out and the
search keeps looking at alternatives (primer §5).

Two discipline points from the primer:

1. **Root only.** Interior nodes keep the clean network prior. Noise
   inside the lookahead would be static inside every simulation and
   would distort the search without adding useful exploration.
2. **Self-play only.** Arena games measure strength; adding noise
   there would blur the Elo measurement (primer §9 bug #3).

The α = 0.1 value is `[derived]`: AlphaZero used α ≈ 10/(typical
legal moves). Gomoku midgame has roughly 100–150 legal moves, giving
10/125 ≈ 0.08, rounded to 0.1 for a knob (primer §5).

### Implementation excursion: sampling Dirichlet without a const-size array

`rand_distr` 0.5 provides a `Dirichlet` distribution, but its
constructor requires a const-size array. The root has a dynamic
number of edges, so the reference samples `k` independent
`Gamma(α, 1)` variables and normalizes them.

> **Excursion — why Gamma sampling is equivalent to Dirichlet**
>
> A Dirichlet(α₁, ..., αₖ) vector has the same distribution as
> `(X₁/ΣXᵢ, ..., Xₖ/ΣXᵢ)` where each `Xᵢ ~ Gamma(αᵢ, 1)` independently.
> With a symmetric Dirichlet all `αᵢ` are equal, so `k` independent
> `Gamma(α, 1)` draws followed by normalization is exactly the
> construction. This is standard, not an approximation.

After blending `(1 − ε)·P + ε·η`, renormalize the root priors to
protect against tiny floating-point drift. The sum should be `1.0`
within a tight tolerance.

### Self-play-only discipline

This chapter builds the knobs. It does not decide when to turn them.
The actual wiring — τ=1 for the first 12 moves, τ→0 afterwards,
Dirichlet noise on, seeded RNG per worker — belongs to the self-play
crate in milestone 5. Milestone 2's job is to make each knob correct,
testable, and separately understandable.

## Low-level design

Add `src/policy.rs` to the `mcts` crate and expose it in `src/lib.rs`.

### Exact signatures

```rust
/// Extract the visit-count distribution over the root's edges.
#[must_use]
pub fn visit_distribution(tree: &Tree, root: NodeId) -> Vec<(Move, u32)>;

/// Pick one move from a visit-count distribution using temperature.
#[must_use]
pub fn select_move(
    dist: &[(Move, u32)],
    temperature: f32,
    rng: &mut impl Rng,
) -> Option<Move>;

/// Add Dirichlet noise to the priors of the root edges only.
pub fn add_dirichlet_noise(
    tree: &mut Tree,
    root: NodeId,
    epsilon: f32,
    alpha: f32,
    rng: &mut impl Rng,
);
```

`visit_distribution` returns moves in edge-creation order, with zero
visits included. Zero-visit moves are still legal and still part of
the full policy target, so dropping them would make π the wrong shape.

### `select_move` behavior

* Empty distribution → `None`.
* `temperature < 1e-8` → deterministic argmax, ties by lowest
  `Move::index`.
* Otherwise → compute weights `N^(1/τ)`, fall back to uniform if all
  weights are zero, then sample by cumulative threshold.

### `add_dirichlet_noise` behavior

* Panic if the root has no edges (a search bug).
* Sample `k = root.edges().len()` independent `Gamma(alpha, 1)` draws.
* Normalize to obtain η.
* Blend each edge prior: `(1 − ε)·prior + ε·ηᵢ`.
* Renormalize the resulting priors.
* Leave every other node untouched.

### Seeded-test strategy

* Argmax correctness: hand-pick counts, assert the right move.
* Tie-breaking: equal counts, assert lowest-index move wins.
* Sampling: use `StdRng::seed_from_u64(42)`, draw 1000 samples from a
  9:1 distribution, assert the first move is drawn between 700 and 980
  times. The bound is loose because it is a statistical test under a
  fixed seed, not a proof.
* Dirichlet: assert root priors sum to 1, all positive, interior prior
  unchanged, and reproducibility under the same seed.

## Solution (opt-in)

The complete reference implementation for this chapter lives in
[08-deep-dive/01-solution.md](08-deep-dive/01-solution.md). Open it
only if you have been stuck for more than twenty minutes, or after you
have finished the chapter and want to compare.

The solution file quotes `src/policy.rs` verbatim, including all
implementation and tests.

## TDD checklist

Write these tests before or alongside the implementation.

1. **Visit distribution matches edges.** Build a tiny tree with two
   root edges with known visit counts. Call `visit_distribution` and
   assert the returned `(Move, visits)` pairs match exactly, in order.

2. **Argmax picks most visited.** Hand a distribution `(0,0)=1`,
   `(7,7)=50`, `(0,1)=3` to `select_move` with `temperature=0.0` and
   a seeded RNG. Assert the chosen move is `(7,7)`.

3. **Argmax tie-breaks by lowest index.** Hand three moves with equal
   visit counts. Assert `select_move(..., 0.0, ...)` returns the move
   with the lowest `Move::index`.

4. **Empty distribution returns `None`.** Call `select_move` on an
   empty slice with any temperature and assert `None`.

5. **τ=1 sampling approximately matches the distribution.** Use counts
   `90` and `10` with `temperature=1.0`, seeded `StdRng::seed_from_u64(42)`,
   and 1000 trials. Assert the first move is chosen between 700 and
   980 times. The bound is intentionally loose; the point is that
   sampling is not broken, not that it matches the mean exactly.

6. **Dirichlet noise keeps sum one and touches only the root.** Build
   a root with three edges and an interior child with one edge. Call
   `add_dirichlet_noise(..., 0.25, 0.1, seeded_rng)`. Assert:
   * root priors sum to 1.0 within `1e-4`;
   * every root prior is strictly positive;
   * the interior child's edge prior is unchanged;
   * a second tree with the same seed produces identical root priors.

Run the tests with `cargo test -p mcts` from `gomoku/`.

## Done when

From `gomoku/`:

```bash
cargo test -p mcts
cargo clippy -p mcts --all-targets -- -D warnings
cargo fmt --all
```

All green. Then commit:

```
feat(mcts): move selection (temperature, dirichlet root noise)
```

Next: [Chapter 09 — The tactics-shaped evaluator](09-the-tactics-evaluator.md)
