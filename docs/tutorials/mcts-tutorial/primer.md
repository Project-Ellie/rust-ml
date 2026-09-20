# MCTS Primer — how search works in our Gomoku agent

## Abstract

This document explains Monte Carlo Tree Search (MCTS) the way this
project uses it: what the algorithm is, which variant of it we
implement (the AlphaGo Zero variant, without random rollouts), how the
search tree integrates the Gomoku rules engine and the evaluation
network, and what milestone 2 — the `mcts` crate — must implement.
It is a reading document, not a build tutorial; the step-by-step build
tutorial lives in the same folder ([README.md](README.md)).

The document's conclusions, stated up front: MCTS is the component
that converts the network's raw position evaluations into stronger
move decisions, and — during self-play — into the training targets
the network then learns from. Our variant replaces random rollouts
with a single network evaluation per leaf, biases selection with the
network's policy output via the PUCT formula, treats terminal
positions as exact rules-truth rather than network estimates, and
extracts moves from visit counts using temperature-controlled
sampling, with Dirichlet noise added at the root during self-play
only. The implementation is a single-threaded Rust crate, generic
over an `Evaluator` trait, storing the tree as a flat vector arena;
all concurrency lives in a separate self-play crate (milestone 5).

The document is self-contained: all project-specific terms and all
symbols are defined in the glossary below and again at first use.
Familiarity with basic deep-learning vocabulary (policy, value,
forward pass) is assumed. Where the document builds on other project
documents, it summarizes the needed content at the point of use and
lists the source in the references at the end.

## Glossary

**Symbols used throughout** (also defined at first use in the body):

- `s` — a game position (a board state).
- `a` — an action; in Gomoku, a move.
- `θ` — the network's trainable weights; `fθ(s)` is the network's
  forward pass on position `s`.
- `p` — the network's **raw policy** output: a probability
  distribution over moves, before any search.
- `v` — the network's **value** output: the expected game outcome in
  `[-1, 1]` from the perspective of the side to move (`+1` = certain
  win, `-1` = certain loss, `0` = draw or dead even).
- `π` — the **improved policy**: the probability distribution over
  the root's moves produced by the search (the normalized visit
  counts, §5). The search's output and the network's training target.
- `z` — the final outcome of a completed game (`+1`, `0`, or `-1`),
  used as the value training target.
- `P(s,a)` — the **prior**: the raw policy's probability for move
  `a` in position `s`, stored on the corresponding tree edge.
- `N(s,a)` — the **visit count**: how many simulations have passed
  through the edge from `s` via `a`.
- `W(s,a)` — the **total action value**: the sum of all values backed
  up through that edge.
- `Q(s,a)` — the **mean action value**, `W(s,a) / N(s,a)`.
- `U(s,a)` — the **exploration bonus** of PUCT (§3).
- `c_puct` — the PUCT exploration constant (§3).
- `τ` (tau) — the sampling **temperature** used when converting
  visit counts into a chosen move (§5).

**Project and domain terms:**

- **Simulation** — one iteration of the four-phase loop
  select → expand → evaluate → back up (§2). The unit of the search
  budget.
- **Rollout** (also: playout) — the classical MCTS evaluation
  technique: play random moves to the end of the game and use the
  result. Our variant performs no rollouts (§2).
- **Prior** — see `P(s,a)` above: an estimate of move quality known
  *before* searching the position, used to bias selection.
- **Self-play** — training games in which the agent plays both sides,
  producing positions and outcomes used as training data.
- **Arena** — competitive games between two agent versions, used to
  measure playing strength (as Elo ratings), not to produce training
  data. **Arena (memory)** — §6.3's unrelated second meaning: the
  tree's storage layout, one flat vector indexed by `u32` node ids.
  The two uses never interact; context disambiguates.
- **Elo** — the standard rating system for relative playing strength;
  a 400-point advantage corresponds to a ~91% expected win rate.
- **Evaluator** — the trait abstraction over "compute `fθ(s)`"
  (§6.2); implemented by the test mock, the in-process network
  wrapper, or the self-play batching client.
- **Burn** — the Rust deep-learning framework this project uses
  (pinned to 0.21.0). Only the `net` crate depends on it; `engine`
  and `mcts` deliberately do not.
- **Swap2** — the balanced opening protocol: one player places the
  first stones, the other player then chooses which color to play
  (details: ch. 12 §3, ch. 13). Relevant here because it makes the
  opening non-alternating, which is why the engine stores absolute
  colors (§6.5).
- **Claim labels** — numbered claims carry a provenance label:
  **[paper]** (taken from cited literature), **[derived]** (computed
  from cited values), **[experiment]** (our tuning parameter, to be
  settled by measurement). Unverified claims are recorded in the
  project's honesty ledger (ch. 12) rather than stated as fact.

## 1. Why search at all

The network gives us, for any position `s`, a raw opinion:

```text
fθ(s) = (p, v)     p: probability over each legal move ("looks good")
                   v: expected game outcome in [-1, 1] from the side
                      to move's perspective
```

The raw opinion of an undertrained network is mediocre. The central
empirical fact of the AlphaGo line of research is that **lookahead
search turns a mediocre opinion into a strong one**: run a few hundred
simulated continuations from `s`, let the network's evaluations
correct one another along the way, and the resulting improved policy
`π` is measurably stronger than the raw policy `p`. In the language of
the papers, MCTS is a **policy improvement operator**: a function that
takes a policy and returns a better one.

The training loop then closes around this operator. The network is
trained to predict `π` (and the final outcome `z`) directly — that
is, training adjusts the network toward the search's output, and the
improved network in turn produces a stronger search. This feedback
loop — raw policy `p` → improved policy `π` → training target →
better raw policy `p` — is the entire AlphaZero algorithm. Everything
else is engineering.

Two consequences follow. MCTS is not an opponent for the network and
not an alternative to it: it is the component that produces the
training targets and, at play time, the component that converts a
network evaluation budget into playing strength. And because the
search defines what the network learns, the search must be correct
before any network is trained — which is why milestone 2 builds and
tests it against a deterministic stand-in evaluator (§6.2).

## 2. The classical skeleton

MCTS grows one search tree per move decision. The root is the current
position; edges are moves; child nodes are the positions those moves
lead to. The tree is built by repeating **simulations**, each with four
phases:

```text
┌────────────┐   ┌────────────┐   ┌────────────┐   ┌────────────┐
│ 1 SELECT   │──▶│ 2 EXPAND   │──▶│ 3 EVALUATE │──▶│ 4 BACK UP  │
│ walk down  │   │ add the    │   │ score the  │   │ propagate  │
│ the tree   │   │ new leaf   │   │ the leaf   │   │ the result │
└────────────┘   └────────────┘   └────────────┘   └────────────┘
       ▲                                                │
       └────────────────────────────────────────────────┘
                 repeat for N simulations, then move
```

In *classical* MCTS (the 2006–2015 vintage, the algorithm that cracked
Go's amateur ranks), phase 3 is a **rollout**: play random moves to
the end of the game and use the actual result. Phase 1 balances
exploitation and exploration with UCT (Upper Confidence bounds applied
to Trees), the tree form of the bandit formula UCB1.

The AlphaGo Zero variant — ours — changes three things:

1. **No rollouts, ever.** Phase 3 is one network forward pass:
   `fθ(leaf) = (p, v)`, and `v` is the result estimate. The network
   *replaces* the random rollout. (This is also why "leaf
   parallelization" from the classical MCTS taxonomy — running many
   rollouts concurrently — is meaningless for us: there is no rollout
   to parallelize. Ch. 12 §8.)
2. **The policy enters selection.** The network's `p` becomes a prior
   on each edge, biasing which paths phase 1 explores (§3).
3. **Terminal leaves are exact.** When the engine says the leaf is a
   won or drawn position, we use the true outcome (`-1` or `0`), not a
   network estimate — and we never evaluate the network there (§4).

One simulation is cheap: a walk down maybe 5–15 edges, a board update
per edge, one evaluation, a walk back up. We budget **400 simulations
per move** in v1 [experiment] — half of AlphaZero's 800 [paper] —
while the network is still weak and self-play throughput matters more
than search depth.

## 3. Selection: the PUCT formula, dissected

Phase 1 starts at the root and repeatedly picks the edge maximizing

```text
a* = argmax_a [ Q(s,a) + U(s,a) ]

Q(s,a) = W(s,a) / N(s,a)                       mean value of action a
U(s,a) = c_puct · P(s,a) · √Σ_b N(s,b) / (1 + N(s,a))
```

This selection rule is the PUCT variant ("predictor" UCT — polynomial
UCT with a prior, Rosin 2011; the AlphaGo Zero paper uses the same
functional form). The per-edge quantities:

- **`P(s,a)`** — the prior: the network's raw-policy probability for
  move `a` in position `s`, stored once when the node is expanded.
  Never changes.
- **`N(s,a)`** — the visit count: how many simulations have passed
  through this edge. The search's accumulated experience.
- **`W(s,a)`** — the total action value accumulated through this edge
  (from the perspective of the player who chose at `s` — see the
  sign convention, §4.4).
- **`Q(s,a)`** — the exploitation term: "what has this move been
  worth, on average?"
- **`U(s,a)`** — the exploration term. It is large when the prior
  `P(s,a)` is high or the visit count `N(s,a)` is low; it decays as
  the edge gets visited. `Σ_b N(s,b)` in the numerator means: as *any*
  sibling gets visited, all unexplored siblings become more attractive.
- **`c_puct`** — the exploration constant. Ours starts at **1.5**
  [paper — ELF OpenGo's value]. The honesty ledger applies (see
  Glossary): **DeepMind never published a numeric `c_puct`**; 1.5 is
  ELF's choice, and ours is a tuning knob ±0.5 [experiment].

A worked example — a root with three candidate moves after some
simulations, `c_puct = 1.5`, `ΣN = 100`:

| move | P | N | W | Q = W/N | U | Q + U |
|---|---|---|---|---|---|---|
| a | 0.50 | 60 | 30 | 0.50 | 1.5·0.5·10/61 ≈ 0.12 | **0.62** |
| b | 0.30 | 30 | 12 | 0.40 | 1.5·0.3·10/31 ≈ 0.15 | 0.55 |
| c | 0.20 | 10 | −1 | −0.10 | 1.5·0.2·10/11 ≈ 0.27 | **0.17** |

Move `a` leads on value, and its high prior keeps `U` meaningful even
at 60 visits. Move `c` has a poor track record, so even a
comparatively large exploration bonus cannot rescue it — yet. Note
the dynamics: `U` for `c` grows as the *other* moves get visited
(√ΣN grows), so if `a` keeps being chosen, `c` eventually gets
another look. The formula spends the simulation budget where the
network's prior is high *and* where results are promising, and it
never permanently writes a move off.

Versus classical UCT, the entire difference is `P(s,a)`: a good
network concentrates the search on the handful of moves that matter.
That is exactly what Gomoku's tactical positions require, where one
forcing move dominates and roughly 150 legal alternatives are noise.

## 4. One simulation, end to end, in our system

This section walks through a single simulation, naming the component
that performs each step. It is the integration of engine, tree, and
evaluator in miniature.

### 4.1 Select

Starting at the root, walk down: at each node, compute PUCT over its
edges and descend the winner, until reaching a **leaf** — a node that
is either *terminal* (game over, per the engine) or *unexpanded*
(never evaluated yet).

Board handling during the walk: the tree stores moves on edges, not
boards in nodes. One `Board` (the engine's value type) is cloned or
mutated-and-unwound along the path — the engine offers both cheap
`Clone` and `play`/`undo`, and which one the tree uses is a
measurement question, not a design question (ch. 12 §7). Either way,
**all rules knowledge comes from the engine crate**: legality, win
detection, draw at 225 moves. The tree never reimplements a rule.

### 4.2 Terminal check first

Before anything network-related: ask the engine for the leaf's status.

- **Won** — the player who just moved made five in a row (overlines
  count — freestyle rules, ch. 13 decision 1). The leaf's value is
  exact: *from the perspective of the side to move at the leaf*, this
  is a loss, `v = −1`. No network call.
- **Draw** (225 moves) — `v = 0`. No network call.
- **Ongoing** — expand and evaluate (next step).

This ordering matters for two reasons: it supplies exact values where
rules-truth (an outcome entailed by the rules, not an estimate) is
available (the search propagates *certainty* up from
terminal nodes — this is how the tree learns "this move forces a win
in 3"), and it saves network evaluations on positions that need none.

### 4.3 Expand and evaluate

For an ongoing leaf:

1. **Encode.** The engine's `encode` turns the board into 17×17
   planes of `u8` values (the border ring is encoded as opponent
   stones — ch. 13; the network reads walls correctly at edges).
   The encoding is Burn-free: plain arrays, no tensor types.
2. **Evaluate.** The planes plus the legal-move mask go into an
   `EvalRequest`; what comes back is `EvalResult { policy, value }`:
   policy logits over all 225 cells, and `v ∈ [−1, 1]` from the
   side-to-move's perspective. (Logits are the network's raw,
   unnormalized output scores; a softmax turns them into
   probabilities.) Which component answers the request depends on the
   run mode — that indirection is the `Evaluator` trait, §6.2.
3. **Mask and normalize.** Policy logits for illegal cells are
   discarded; the remainder are softmaxed into a proper probability
   distribution over legal moves. Two details are classic defect
   sites: normalize *after* masking, not before (softmax first would
   leak probability mass onto illegal moves), and remember that the
   17×17 border cells are never legal targets — the policy head
   emits 225 logits exactly so the border is excluded by construction.
4. **Create edges.** The leaf gets one edge per legal move, each
   carrying its prior `P = p_a`, `N = 0`, `W = 0`.

### 4.4 Back up — and the sign convention (read this section twice)

Walk back up the path to the root. For every edge traversed:

```text
N += 1
W += v_from_the_perspective_of_the_player_who_chose_at_that_node
```

The network's `v` is always stated from the perspective of the player
**to move at the evaluated leaf**. But edges belong to alternating
players, and a value that is good for one player is bad for the other.
So the backed-up value flips sign at every level of the ascent:

```text
leaf evaluation: v          (good for the player to move at the leaf)
parent's edge:   W += -v    (what is good for my opponent is bad for me)
grandparent:     W += +v
... alternating up to the root
```

An equivalent formulation: negate `v` once at the leaf ("value for the
player who *just moved*"), then flip the sign on every step up.
Terminal values follow the same rule — §4.2 defined them from the
side-to-move's perspective precisely so that one uniform backup rule
handles both terminal and evaluated leaves.

This alternating-perspective bookkeeping is the most common source of
defects in MCTS implementations. The milestone-2 tactical tests exist
largely to catch sign errors: if the search, given a sane prior, does
not play the winning move in a won position, the sign convention is
the first suspect.

### 4.5 Repeat, then move

After 400 simulations, the root's edges hold visit counts. How a move
is chosen from them — and the extra ingredients that apply only at
the root, only during self-play — is §5.

## 5. From tree to move: π, temperature, and root noise

### The training target π

The visit-count distribution over the root's edges *is* the improved
policy `π` that training will later teach the network to predict
directly. Note the economy of this arrangement: the same search that
plays the game also produces the training label.

### Temperature: how a move is picked from counts

- **Moves 1–12 of a self-play game** [derived: AlphaGo Zero used 30
  moves on ~200-move Go games [paper]; scaled to ~60-move Gomoku
  games]: **sample** the move with probability proportional to
  `N^(1/τ)`, with temperature `τ = 1` [experiment] — that is,
  proportional to `N` itself. Proportional sampling keeps opening
  games diverse; without it, every self-play game starts with the
  same currently-favored moves and the replay buffer (the store of
  past self-play positions used for training) converges to a
  monoculture.
- **After move 12, and always in arena play**: `τ → 0`, which means
  simply playing the most-visited move. Maximum strength, no
  randomness.

### Dirichlet noise at the root (self-play only)

At the root of every self-play search, the priors are perturbed:

```text
P_root(a) = (1 − ε)·P(a) + ε·η_a,     η ~ Dirichlet(α)
ε = 0.25 [paper — the AlphaGo Zero/KataGo form]
α = 0.1  [derived: AlphaZero's α ≈ 10/(typical legal moves):
         Go 0.03@361, shogi 0.15@~80, chess 0.3@~35 [paper];
         Gomoku midgame ≈ 100–150 legal → 10/125 ≈ 0.08 → 0.1]
         [experiment]
```

(A Dirichlet distribution draws a random probability vector — here, a
random distribution over the root's moves; small `α` concentrates the
draw on a few entries.) Purpose: guarantee that *every* root move
keeps a nonzero prior, so PUCT's exploration term can never be zeroed
out by an overconfident young network. It is the exploration floor.
Two discipline points: the noise is applied **only at the root**
(interior nodes keep the clean network prior — we want diversity in
which *game* we explore, not randomness inside each lookahead), and
**only in self-play** (arena games measure strength; noise would blur
the Elo measurement).

### Resignation

Self-play games resign when the root value stays below a threshold,
recalibrated each iteration to a 5% false-positive rate from arena
games (method from AlphaGo Zero [paper — via ELF OpenGo]). Resignation
is pure throughput: games whose outcome is effectively decided stop
consuming simulations. Milestone 2 ignores resignation; it lands with
the self-play crate (milestone 5).

## 6. The software architecture: who owns what

This section turns to the integration question properly: engine, tree,
and network as Rust crates. Crate boundaries follow ch. 12 §6.

### 6.1 Engine: the rules oracle, nothing more

The `mcts` crate depends on `engine` and uses exactly four kinds of
things from it:

- `Board` — clone / play / undo during selection walks (§4.1);
- `status()` — terminal checks (§4.2);
- `empty_moves()` / legality — the move set at expansion (§4.3);
- `encode()` — board → 17×17 planes (§4.3).

The engine stays a dependency island: no Burn, no channels, no I/O —
it runs identically on any number of worker threads. MCTS never
inspects bitboards, never reimplements a rule, never special-cases a
win. If the engine says the game is over, it is over.

### 6.2 The Evaluator trait: the only coupling between search and network

The tree needs to ask "what is `fθ(s)`?" — nothing else. That is one
trait in the `mcts` crate (ch. 12 §6):

```rust
pub trait Evaluator {
    /// Evaluate one position: policy priors over legal moves + value.
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult;
}
```

`EvalRequest` carries plain data — encoded planes, the legal-move
mask, and (in the self-play implementation) a oneshot reply channel.
`EvalResult` carries plain data back — 225 policy logits and a value.
Three implementations, three lifecycles:

| Implementation | Used by | Shape |
|---|---|---|
| **Mock** (rules the engine's tactics module into a deterministic prior and value) | MCTS unit tests, milestone-2 tactical suite | two lines of logic, no Burn |
| **Direct network wrapper** (`net::Model` behind the trait) | arena play, integration tests | synchronous forward pass in-process |
| **Channel client** (holds a sender to the evaluator service) | self-play workers | blocking send/receive — §6.4 |

Why a trait and not a closure of type `Fn(Board) -> (Policy, Value)`?
Because the production evaluator is a stateful client: it holds a
channel sender and blocks on a reply channel per request. Expressing
that as a captured-closure parameter obscures the ownership structure
and complicates testing; a named trait makes the state explicit and
keeps `mcts` compilable and testable with no GPU in sight (ch. 12 §6).
Milestone 2 builds and tests the entire search before the `net` crate
exists.

### 6.3 The tree in Rust: a vector arena, not a graph of boxes

A tree node owns children that own children — the classic shape where
naive Rust representations (`Box`, `Rc<RefCell<_>>`) become unwieldy.
The standard representation for game-tree engines, and ours:

```text
nodes: Vec<Node>                    // one growable arena per search
Node  { edges: <small collection of Edge> }
Edge  { mv: Move, prior: f32, n: u32, w: f32, child: Option<u32> }
                                                  ▲
                                     index INTO nodes, not a pointer
```

Children are `u32` indices into the arena. There is no reference
counting, no borrow-checker conflicts, and no per-node allocation; the
entire tree is a small number of contiguous vectors — cache-friendly
in the loop that runs 400 times per move. Per-move node
counts are small (a few hundred to ~1–2k nodes at 400 simulations), so
memory use is a non-issue.

After the move is played, the tree can be discarded — or the subtree
under the played edge can be **reused** as the next search's root
(AlphaGo Zero did this [paper — via ELF OpenGo]): the previous
search's experience carries over, at no additional cost in playing
strength terms. Whether we reuse is an `[experiment]` that the
milestone-2 design will settle; the arena representation supports both
options (reuse means re-rooting the index while keeping the vector).

### 6.4 Parallelism: many trees, one batched evaluator

Recap of ch. 12 §8–9, from the MCTS seat:

- **Game-level parallelism.** Each self-play worker (14 in the
  milestone-5 configuration) plays its own game with its own
  single-threaded tree. No shared tree, no locks, no virtual loss (the
  classical mechanism for coordinating threads inside one shared
  tree). Each worker's data is cache-private. We parallelize by having
  *more trees*, not more threads per tree. AlphaGo's problem was
  scarce evaluations feeding one deep search; ours is the opposite
  situation — evaluations come from one shared GPU in large batches,
  so what we lack is trees, not threads.
- **The evaluator service is the only synchronization point.** A
  worker's `evaluate()` blocks: send an `EvalRequest`, wait on the
  oneshot reply. The service collects requests from all workers into
  batches (at most 128 requests or a 2 ms collection window
  [experiment]), makes **one** GPU forward pass, and distributes the
  results. Workers never touch Burn types; the model lives on the
  evaluator thread alone — ownership instead of
  `Arc<Mutex<weights>>`.
- **Backpressure needs no additional mechanism.** Bounded channels
  throttle workers to GPU speed automatically.

Consequence for the `mcts` crate itself: it is written as purely
single-threaded code. All concurrency lives in `selfplay` (workers,
channels, service). The tree neither knows nor cares that its
`evaluate()` call crosses a process-wide batching point — that is the
trait boundary doing its job.

### 6.5 Where the network sees the board

One data-flow detail is central enough to fix explicitly, because
three design documents intersect at it:

```text
Board (engine, stride-16 bitboards, absolute colors)
  │  encode()                       — engine, Burn-free
  ▼
Planes [C,17,17] u8  (me/you relative view, border ring = opponent)
  │  net crate converts             — the ONLY Burn-aware seam
  ▼
Tensor<B,4> [batch,C,17,17]
  │  forward
  ▼
(policy logits [batch,225], value [batch,1])
```

The relative me/you view is derived at encode time — the network
never learns "black" versus "white", halving the state space — while
the engine stores absolute colors (the Swap2 opening — see Glossary —
does not alternate players in the usual way, ch. 13 decision 5). The 17×17
border ring is added at encoding; in 15×15 symmetry space the D4
transforms (the board's eight rotations and reflections) commute with
it; and the policy head emits 225 logits so border cells are never
move targets. Each of those properties is a documented decision from
ch. 12/13 arriving at one diagram.

## 7. What milestone 2 actually implements

The crate, in build order (each step testable in isolation):

1. **Vector-arena tree** — `Vec<Node>` + edge indices (§6.3), with
   the per-edge statistics `P, N, W` and derived `Q`.
2. **PUCT selection** — the formula of §3, descending from root to
   leaf, the board cloned or unwound along the path.
3. **Expansion** — terminal check via the engine; otherwise encode →
   `evaluate()` → mask illegal moves → normalize → create edges
   (§4.2–4.3).
4. **Backup** — visit counts and sign-flipped value accumulation
   (§4.4). Unit-tested on hand-built trees where the correct `Q`
   values are computable by hand.
5. **Move selection** — visit-count extraction, temperature sampling
   versus argmax, Dirichlet root noise (§5). Deterministic under a
   seeded random number generator for tests.
6. **The mock evaluator** — priors and values derived from the
   engine's tactics module: immediate wins receive a high prior and a
   near-certain value; forced blocks likewise; other moves receive
   mild shaping. Fully deterministic. This is what lets the whole
   crate be built and *proven* with no network and no GPU.
7. **The acceptance tests** (ch. 12 §12 items 1–2, milestone 2 row):
   - **Tactical suite**: win-in-1 solved at 50 simulations; forced
     blocks taken; win-in-3 sequences found. Published Gomoku
     tactical puzzles have verified solutions, which makes them
     ready-made test data (ch. 12 §3).
   - **Sanity**: 1,000 full games against a uniform-random evaluator
     — every move legal, every game terminates, no panic, no leak.

What milestone 2 explicitly does **not** build: the network interface
beyond the trait (milestone 3), workers/channels/batching (milestone
5), resignation, gating, Elo tracking. One crate, one responsibility:
given a position and a way to evaluate positions, produce a strong
move and a visit-count distribution.

## 8. Deliberately excluded (recorded for future reconsideration)

- **Transposition tables / DAG search.** The classic use of Zobrist
  keys (incremental position hashes) is *not* used in search: the
  tree stays a tree, as in AlphaZero — identical positions reached by
  different move orders are NOT merged into a directed acyclic graph
  (DAG). PUCT statistics interact subtly with merged nodes;
  Gomoku's transposition rate is low (stones never move once placed);
  the payoff is unproven at our scale. Full reasoning:
  [05-deep-dive/01](../13-engine-tutorial/05-deep-dive/01-what-zobrist-hashing-is-good-for.md)
  §6. Revisit only if profiling shows duplicate subtrees dominating.
- **Tree parallelization with virtual loss.** AlphaGo's
  asynchronously parallelized MCTS (APV-MCTS) solved a problem we do
  not have (scarce evaluations feeding one deep search). Ours is the
  opposite situation, and game-level parallelism wins (§6.4).
  Ch. 12 §8 has the literature map if single-game latency ever
  matters.
- **Playout-cap randomization** (KataGo: most moves searched cheaply,
  a quarter searched fully, ~1.37× efficiency [paper — KataGo]).
  **Registered upgrade #1** — but it changes the training-data
  distribution, so it lands only after a clean phased baseline exists
  (ch. 12 §11).
- **First-play urgency, RAVE, progressive widening.** Classical
  enhancements for prior-free search, catalogued in the MCTS survey
  (Browne et al. 2012; ch. 10 #9); unneeded while the prior is a
  network.

## 9. Expected defects (and the tests that guard against them)

Experience — this project's predecessor and the published literature —
identifies the following as the most common defect sites in MCTS
implementations. The milestone-2 test plan is shaped around them:

1. **Sign flips in backup** (§4.4) — the most common of all.
   Tactical puzzles catch it immediately.
2. **Mask/normalize order** — softmax before masking leaks
   probability onto illegal moves (§4.3).
3. **Noise in the wrong place** — Dirichlet applied at interior nodes
   (randomness inside every lookahead) or in arena games (blurred Elo
   measurement) (§5).
4. **Terminal values from the wrong perspective**, or evaluating the
   network on terminal leaves (§4.2).
5. **π extracted from Q instead of N** — the training target is visit
   *counts*, not values (§5).
6. **Forgetting that value is relative to the side to move** when
   positions are stored for training — the training sample stores `z`
   from the perspective of the player to move at `s`, consistently
   with the encoding's me/you planes (§6.5).

## 10. Summary

- MCTS is the **policy improvement operator**: 400 simulations of
  select → expand → evaluate → back up turn the network's raw `(p, v)`
  into a stronger `π`, which is both the move played and the training
  target.
- Selection is PUCT: exploitation `Q` plus prior-weighted exploration
  `U`; `c_puct = 1.5` is ELF OpenGo's published value, not DeepMind
  folklore.
- Terminal positions are exact engine truth and never touch the
  network; everything else is one forward pass per new leaf.
- The value perspective alternates up the tree; the sign convention
  is the most common defect site, so the tactical suite guards it.
- Moves come from visit counts: sampled early in self-play games
  (τ = 1, first 12 moves), argmax afterwards; Dirichlet noise at the
  root, self-play only.
- Crates: `engine` is the rules oracle; `mcts` is single-threaded and
  generic over an `Evaluator` trait (mock / direct network wrapper /
  channel client); `selfplay` owns all concurrency and the batched
  GPU evaluator. The tree is a vector arena with `u32` child indices.
- Milestone 2 = tree + PUCT + backup + move selection + mock
  evaluator, proven by tactical puzzles and 1,000 random games — no
  network, no GPU, no threads.

## References

**Project documents** (paths relative to the repository root):

- docs/09-toward-alphazero.md — chapter 9: the AlphaGo Zero algorithm
  in one paragraph; the bridge from the MNIST curriculum to this
  project.
- docs/10-papers.md — chapter 10: the annotated reading list; item 9
  is the MCTS survey (Browne et al. below).
- docs/12-gomoku-architecture.md — chapter 12: system architecture,
  milestone plan, claim-label conventions, and the honesty ledger.
  Cited sections: §3 (rules and puzzle corpora), §6 (crate
  boundaries), §7 (board representation choices), §8 (MCTS
  parallelization analysis), §9 (evaluator service), §11 (phased
  plan and registered upgrades), §12–13 (test plan and milestone
  acceptance criteria), appendix (paper citations).
- docs/13-engine-design.md — chapter 13: engine design and locked
  decisions (decision 1: freestyle overlines; decision 5: absolute
  colors; the encoding section: 17×17 planes with border ring).
- docs/tutorials/13-engine-tutorial/05-deep-dive/01-what-zobrist-hashing-is-good-for.md
  — why Zobrist keys exist and why the search does not use them (§6).
- docs/tutorials/mcts-tutorial/README.md — the build tutorial for
  milestone 2 (chapters 01–10).

**External literature:**

- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). Nature 550, 354–359.
  https://doi.org/10.1038/nature24270 — The algorithm we implement:
  800 simulations per move, `τ = 1` for the first 30 moves, Dirichlet
  noise at the root with `ε = 0.25`, resignation thresholding, tree
  reuse.
- Silver, D. et al. (2018). *A General Reinforcement Learning
  Algorithm that Masters Chess, Shogi, and Go through Self-Play*
  (AlphaZero). Science 362, 1140–1144.
  https://arxiv.org/abs/1712.01815 — The `α ≈ 10/(legal moves)`
  heuristic underlying our `α = 0.1`.
- Tian, Y. et al. (2019). *ELF OpenGo: An Analysis and Open
  Reimplementation of AlphaZero*. ICML 2019.
  https://arxiv.org/abs/1902.04522 — The published numeric
  `c_puct = 1.5`; batching and engineering practice.
- Wu, D. J. (2019). *Accelerating Self-Play Learning in Go*
  (KataGo). arXiv:1902.10565. https://arxiv.org/abs/1902.10565 —
  Playout-cap randomization (registered upgrade #1).
- Auer, P. et al. (2002). *Finite-time Analysis of the Multiarmed
  Bandit Problem*. Machine Learning 47(2–3), 235–256.
  https://doi.org/10.1023/a:1013689704352 — UCB1, the bandit formula
  that underlies UCT.
- Kocsis, L. & Szepesvári, C. (2006). *Bandit Based Monte-Carlo
  Planning*. ECML 2006, LNCS 4212.
  https://doi.org/10.1007/11871842_29 — UCT, the tree form of UCB1.
- Rosin, C. D. (2011). *Multi-armed Bandits with Episode Context*.
  Annals of Mathematics and Artificial Intelligence 61, 203–230.
  https://doi.org/10.1007/s10472-011-9258-6 — The PUCT selection
  rule (polynomial UCT with a predictor prior).
- Chaslot, G. et al. (2008). *Parallel Monte-Carlo Tree Search*.
  Computers and Games (CG 2008), LNCS 5131.
  https://doi.org/10.1007/978-3-540-87608-3_6 — Virtual loss and the
  parallel-MCTS taxonomy (root/leaf/tree parallelization) this design
  draws its exclusions from.
- Browne, C. et al. (2012). *A Survey of Monte Carlo Tree Search
  Methods*. IEEE Transactions on Computational Intelligence and AI in
  Games 4(1), 1–43. https://doi.org/10.1109/tciaig.2012.2186810 —
  The classical MCTS survey (selection enhancements, rollout
  parallelization) this design draws its exclusions from.
