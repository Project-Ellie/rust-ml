# 04 — The evaluation seam

## Context

You have a tree and you have a way to walk it. Selection from chapter 03 ends at a leaf: a node with no children, no statistics, nothing but potential. Before the tree can learn from that leaf, the leaf needs a score. In classical MCTS that score comes from a random rollout; in our AlphaZero-style search it comes from a neural network that says, for this position, "here is what I think of every legal move, and here is how good I think the position is for the player to move."

But milestone 2 deliberately has no network, no Burn, no GPU. The search crate must still compile, still run, still be testable. That means the place where the tree asks for an evaluation has to be a seam: a small, well-defined interface that can be satisfied by a real network later, by a channel client in a worker thread later, or by a two-line stub right now. This chapter builds that seam.

The seam is the single coupling point between search and network. Everything else in the `mcts` crate is pure tree bookkeeping. Get this boundary right and the rest of the project can evolve independently; get it wrong and you will be untangling network-shaped assumptions from search logic for the rest of the milestone. Primer §4.3 and §6.2 are the design authority here; we are turning those sections into types and a trait.

## Intention

By the end of this chapter you will have defined, in `src/eval.rs`:

- `EvalRequest` — the data the tree sends to an evaluator: encoded planes plus the list of legal moves.
- `EvalResult` — the data that comes back: 225 policy logits and a scalar value in `[-1, 1]`.
- `Evaluator` — a trait with one method, `evaluate(&mut self, EvalRequest) -> EvalResult`.
- `masked_softmax` — a function that takes raw logits over all 225 cells, keeps only the legal moves, and normalizes them into a probability distribution.
- `UniformEvaluator` — a tiny test stub that returns uniform logits and a constant value.

The observable behavior is: any position with legal moves can be passed through an evaluator, masked, and turned into `(move, probability)` pairs that sum to 1. Illegal moves receive exactly zero probability even if their raw logits are enormous. A terminal position never touches the evaluator; that is chapter 05's job, but the types you build here must not make terminal handling awkward.

## Mental mapping

### Why this is a trait, and why `&mut dyn Evaluator`

The tree's only question is "what is `fθ(s)`?" One natural first attempt is a closure:

```rust
fn search(board: &Board, evaluate: impl Fn(EvalRequest) -> EvalResult) { ... }
```

Closures are attractive because a uniform evaluator is literally one line. But the production evaluator is not a pure function. In milestone 4 it becomes a channel client: it holds a sender, sends a request, blocks on a oneshot receiver, and returns the result. That object has state. A closure can capture state, but it gets awkward when the same evaluator needs to live across many searches, be cloned into worker threads, or carry connection metadata. The named `Evaluator` trait gives us a clear contract and three clean implementations (primer §6.2's table):

| Implementation | Used by | Shape |
|---|---|---|
| `UniformEvaluator` / `TacticsEvaluator` | unit tests, tactical suite | pure Rust, no Burn |
| Direct net wrapper | arena play, integration tests | synchronous forward pass in-process |
| Channel client | self-play workers | blocking send/recv |

The trait method takes `&mut self` rather than `&self` because some future evaluators may want to maintain internal bookkeeping: a request id, a connection handle, a cache, a counter. Even if the simple stubs do not need mutation today, `&mut self` leaves the door open without costing anything. It also mirrors the way the tree will consume the evaluator: one mutable reference held for the duration of the search.

We use `&mut dyn Evaluator` rather than a generic `E: Evaluator` because the search function receives one evaluator from its caller and uses it at exactly one place (expansion). Genericizing saves a virtual call but complicates the public API and the tests. The tree is not hot in evaluation; the network forward pass dominates every cost profile. Readability wins.

> **Excursion — object safety and the trait boundary**
> `Evaluator` is object-safe because `evaluate` takes `Self` by `&mut self` and returns `EvalResult`, neither of which depends on the concrete type's size. That is why `&mut dyn Evaluator` compiles. If you ever add an associated type or a method with `Self: Sized`, you would break the dynamic dispatch. Keep the trait simple.

### Why the request carries encoded planes, not a `Board`

This is the cleanest architectural decision in the whole crate, and it is worth understanding slowly.

The `engine` crate knows absolute colors (Black and White stones on the board), Swap2's non-alternating opening, and the exact rules of Gomoku. It also knows how to `encode()` a board into a relative `Planes` struct: two 17×17 `u8` planes representing "me" and "you" from the side to move's point of view, with a border ring of opponent stones. That encoding is Burn-free and engine-clean.

The `net` crate, when it arrives, will know Burn tensors and the model's input layout. It will convert `Planes` into a `Tensor<B, 4>` of shape `[batch, channels, 17, 17]`. The `mcts` crate sits in the middle. It must not know Burn (milestone 2 rule) and it must not reimplement encoding (engine's job). So the data that crosses the `Evaluator` seam is the intermediate representation: already-encoded planes plus the legal moves. Primer §6.5 shows this pipeline:

```text
Board (engine, absolute colors)
  │  encode()
  ▼
Planes [C, 17, 17] u8  (relative me/you view, border ring = opponent)
  │  net crate converts
  ▼
Tensor<B, 4>
  │  forward
  ▼
(policy logits [batch, 225], value [batch, 1])
```

Carrying `Planes` in `EvalRequest` means the `mcts` crate stays clean. It asks the engine for encoding before building the request, and the evaluator implementation (later the net crate) takes over from there. If you tried to pass a `Board` to the evaluator, the evaluator would need engine access to encode it; that would couple the network crate to the engine's `encode` function in ways that make channel-based evaluation messy. If you tried to pass raw tensors, the `mcts` crate would need Burn. `Planes` is exactly the right currency.

### Policy logits over all 225 cells

`EvalResult.policy` is `[f32; 225]`, not `[f32; 225]` minus the border cells. Why 225?

The board is 15×15 = 225 playable cells. The encoding adds a one-cell border around it, making a 17×17 visual field, so the network sees walls and edge threats correctly. The policy head of the network also emits 225 logits — one for each playable cell — because the 17×17 border cells are never legal move targets. The border is there for spatial context; it is excluded from move generation by construction. This is the chapter 13 encoding decision; we simply inherit it.

So the policy array index is `Move::index()`, which maps the inner 15×15 coordinates to 0..225. The border never appears in `board.empty_moves()`, so it never appears in the legal-move list, so `masked_softmax` never assigns it probability. The 225-length array is slightly wasteful for storage but trivially correct: no special-case coordinate remapping between network output and move index.

### Masked softmax: the order matters

Here is the bug this chapter exists to prevent. You have raw logits for 225 cells. Some cells are legal moves; some are occupied or off-board. You want a probability distribution over legal moves. The tempting implementation is:

```rust
// WRONG
let probs = softmax(logits);
let legal_probs: Vec<_> = legal.iter().map(|mv| probs[mv.index()]).collect();
let sum: f32 = legal_probs.iter().sum();
let normalized: Vec<_> = legal_probs.iter().map(|p| p / sum).collect();
```

This is wrong because softmax uses the illegal cells in the denominator. A huge logit on an illegal cell sucks probability mass away from legal cells. Even after you renormalize over legal moves, the relative weights among legal moves have been corrupted by the illegal logits. Primer §9 lists this as bug #2: "softmax before masking leaks probability onto illegal moves." The correct order is mask first, then normalize:

```rust
// RIGHT
let legal_logits: Vec<_> = legal.iter().map(|mv| logits[mv.index()]).collect();
let probs = softmax(legal_logits);
```

`masked_softmax` does exactly this. It collects only the legal logits, subtracts their maximum for numerical stability, exponentiates, sums, and divides. The illegal cells are never seen by the softmax.

### Numerical stability: subtract the max

The softmax formula is `exp(x_i) / Σ_j exp(x_j)`. If any `x_i` is large and positive, `exp(x_i)` overflows to `f32::INFINITY`. If any `x_i` is large and negative, the corresponding `exp(x_i)` underflows to zero, which is harmless, but the largest positive values can still overflow the sum.

The fix is the log-sum-exp trick: subtract `max_j(x_j)` from every `x_i` before exponentiating. Algebraically,

```text
exp(x_i) / Σ_j exp(x_j)
= exp(x_i - m) · exp(m) / Σ_j exp(x_j - m) · exp(m)
= exp(x_i - m) / Σ_j exp(x_j - m)
```

where `m = max_j(x_j)`. The new largest exponent is `exp(0) = 1`, so nothing overflows. The subtraction does not change the result, but it rescales the computation into a safe range.

> **Excursion — a worked example of why subtraction saves you**
>
> Suppose two legal moves have logits `[1000.0, 1001.0]`. The true softmax is approximately `[0.2689, 0.7311]`. Computing naively: `exp(1000.0)` overflows to infinity in `f32`. The ratio becomes `inf / inf = NaN`, and the search dies.
>
> With the trick: `m = 1001.0`, shifted logits are `[-1.0, 0.0]`, exponentials are `[0.3679, 1.0]`, sum is `1.3679`, probabilities are `[0.2689, 0.7311]`. Exact result, no overflow.
>
> The reference implementation uses an `f64` accumulator for the sum, which also guards against the opposite problem: 225 tiny exponentials summing to a value that loses precision in `f32`. At our sizes this is conservative, but it is cheap and correct.

### The value is from the side to move's perspective

`EvalResult.value` is a single `f32` in `[-1, 1]`. The sign convention is: positive means good for the player whose turn it is at the position being evaluated, negative means bad. This is the only perspective that makes sense for a network: the network sees relative planes (me vs. you), so its output is naturally relative to the side to move.

This matters enormously for backup, which is chapter 06. For now, plant one seed in your head: the value is not absolute. It is not "good for Black" or "good for White." It is "good for the player to move at this leaf." When the search backs that value up the tree, every edge belongs to the opposite player from the edge below it, so the sign must flip at every level. We will spend a whole chapter on that flip; here we just make sure the evaluator contract says the right thing.

### `UniformEvaluator` as the test stub

A trait needs implementations. For tests we build `UniformEvaluator`: it returns `[0.0f32; 225]` for the policy logits and a configurable constant for the value. Because all logits are equal, `masked_softmax` produces a uniform distribution over legal moves. This is the dullest possible evaluator and exactly what we need:

- It proves the tree works without a network.
- It makes expansion deterministic and easy to assert on.
- It gives us a baseline: if MCTS cannot solve simple puzzles with a uniform evaluator, the bug is in the search, not the evaluator.

Later, chapter 09 replaces it with `TacticsEvaluator`, which uses the engine's tactics module to produce strong priors. But the seam is the same.

## Low-level design

Create `gomoku/crates/mcts/src/eval.rs` and add `pub mod eval;` to `lib.rs`.

Types:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct EvalRequest {
    pub planes: engine::Planes,
    pub legal: Vec<engine::Move>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvalResult {
    pub policy: [f32; 225],
    pub value: f32,
}

pub trait Evaluator {
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult;
}
```

Functions:

```rust
#[must_use]
pub fn masked_softmax(logits: &[f32; 225], legal: &[engine::Move]) -> Vec<(engine::Move, f32)>;
```

`UniformEvaluator`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UniformEvaluator { pub value: f32 }

impl UniformEvaluator {
    #[must_use]
    pub fn new(value: f32) -> Self;
}

impl Default for UniformEvaluator { ... }

impl Evaluator for UniformEvaluator { ... }
```

Tests live in `#[cfg(test)] mod tests` at the bottom of `eval.rs`:

1. `masked_softmax_sums_to_one` on an empty board with uniform logits.
2. `illegal_moves_get_zero_probability` — play two moves, give those cells enormous logits, assert they do not appear in the distribution.
3. `mask_before_normalize_guard` — one legal move with a modest logit, one illegal move with a gigantic logit, assert the legal move receives all the mass.
4. `uniform_evaluator_returns_constant_value` — create a `UniformEvaluator`, build a request from `Board::new()`, assert the value is what you configured and the policy is all zeros.

All tests use `engine::Board`, `engine::Move`, and `engine::encode`. No randomness; deterministic assertions.

## Solution (opt-in)

The complete reference implementation for this step lives in the deep-dive folder. Open it if you are stuck for more than twenty minutes, or after you have written your own version and want to compare. The file is a verbatim quote from the verified reference crate: it compiles and all chapter tests pass.

- [04-the-evaluation-seam/01-solution.md](04-the-evaluation-seam/01-solution.md)

Do not copy-paste without reading. The point is to understand why every line exists.

## TDD checklist

1. **Red:** write the type definitions and `masked_softmax` signature. `cargo check -p mcts` fails because nothing is implemented.
2. **Green:** implement `masked_softmax` with mask-first, subtract-the-max, `f64` accumulator, and the degenerate uniform fallback.
3. **Green:** implement `UniformEvaluator` returning `[0.0f32; 225]` and the constant value.
4. **Red:** write `masked_softmax_sums_to_one` and watch it fail or pass depending on your stub state.
5. **Green:** make it pass; assert the distribution length equals the number of legal moves.
6. **Red:** write `illegal_moves_get_zero_probability`.
7. **Green:** confirm illegal cells get zero probability even with logits of `1000.0`.
8. **Red:** write `mask_before_normalize_guard` with one legal move and one illegal move with logit `1e9`.
9. **Green:** confirm the legal move gets probability `1.0`.
10. **Red:** write `uniform_evaluator_returns_constant_value`.
11. **Green:** confirm value and policy match.
12. **Gates:** `cargo test -p mcts`, `cargo clippy -p mcts --all-targets -- -D warnings`, `cargo fmt --all`.

## Done when

- `cargo test -p mcts` passes.
- `cargo clippy -p mcts --all-targets -- -D warnings` passes.
- `cargo fmt --all` passes.
- The evaluator seam is present: `EvalRequest`, `EvalResult`, `Evaluator`, `masked_softmax`, `UniformEvaluator`.
- Illegal moves receive exactly zero probability; legal probabilities sum to 1.

Commit: `feat(mcts): evaluator seam + masked softmax`

Next: [Chapter 05 — Expansion](05-expansion.md)
