# MCTS Primer — how search works in our Gomoku agent

A reading document, not a build tutorial. It explains Monte Carlo Tree
Search the way *this project* uses it: what the algorithm is, what the
`mcts` crate (milestone 2) must implement, how the tree and the game
engine fit together, and how the evaluation network enters the loop.
Deeper considerations appear where they change the design — and only
there.

Prerequisites: chapter 12 §1–2 (the four subsystems and the loop) and
chapter 9 (the AlphaGo Zero algorithm in one paragraph). Everything here
is design-level; the build-it-yourself tutorial lives next door in this
same folder — start at [README.md](README.md) — with contracts and TDD
slices, following the conventions of the engine tutorial.

Numbers carry the chapter-12 labels: **[paper]** (cited, see ch. 12
appendix), **[derived]** (computed from cited values), **[experiment]**
(our tuning parameter).

---

## 1. Why search at all

The network gives us, for any position `s`, a raw opinion:

```text
fθ(s) = (p, v)     p: probability over each legal move ("looks good")
                   v: expected game outcome in [-1, 1] from the side
                      to move's perspective
```

The raw opinion of an undertrained network is mediocre. The central
empirical fact of the AlphaGo line is that **lookahead search turns a
mediocre opinion into a strong one**: run a few hundred simulated
continuations from `s`, let the network's opinions correct each other
along the way, and the resulting *improved* policy π is measurably
better than the raw p. In the language of the papers, MCTS is a
**policy improvement operator**.

And then the loop closes: we train the network to *predict* π (and the
final outcome z). The network chases the search; the search rides the
network. That bootstrap — p → π → training target → better p — is the
entire AlphaZero algorithm. Everything else is engineering.

So: MCTS is not an opponent for the network or an alternative to it.
It is the machine that manufactures the training targets, and — at
play time — the machine that converts a network evaluation budget into
playing strength.

## 2. The classical skeleton

MCTS grows one search tree per move decision. The root is the current
position; edges are moves; child nodes are the positions those moves
lead to. The tree is built by repeating **simulations**, each with four
phases:

```text
┌────────────┐   ┌────────────┐   ┌────────────┐   ┌────────────┐
│ 1 SELECT   │──▶│ 2 EXPAND   │──▶│ 3 EVALUATE │──▶│ 4 BACK UP  │
│ walk down  │   │ add the    │   │ score the  │   │ propagate  │
│ the tree   │   │ new leaf   │   │ leaf       │   │ the result │
└────────────┘   └────────────┘   └────────────┘   └────────────┘
       ▲                                                │
       └────────────────────────────────────────────────┘
                 repeat for N simulations, then move
```

In *classical* MCTS (the 2006–2015 vintage, the algorithm that cracked
Go's amateur ranks), phase 3 is a **rollout**: play random moves to the
end of the game and use the actual result. Phase 1 balances
exploitation and exploration with a formula called UCB1/UCT.

The AlphaGo Zero variant — ours — changes three things:

1. **No rollouts, ever.** Phase 3 is one network forward pass:
   `fθ(leaf) = (p, v)`, and `v` is the result estimate. The network
   *replaces* the random playout. (This is also why "leaf
   parallelization" from the MCTS taxonomy is meaningless for us —
   there is no playout to parallelize. Ch. 12 §8.)
2. **The policy enters selection.** The network's `p` becomes a prior
   on each edge, biasing which paths phase 1 explores (§3).
3. **Terminal leaves are exact.** When the engine says the leaf is a
   won or drawn position, we use the true outcome (`-1` or `0`), not a
   network guess — and we never evaluate the network there (§4).

One simulation is cheap: a walk down maybe 5–15 edges, a board update
per edge, one evaluation, a walk back up. We budget **400 simulations
per move** in v1 [experiment] — half of AlphaZero's 800 [paper], while
the network is still weak and self-play throughput matters more than
search depth.

## 3. Selection: the PUCT formula, dissected

Phase 1 starts at the root and repeatedly picks the edge maximizing

```text
a* = argmax_a [ Q(s,a) + U(s,a) ]

Q(s,a) = W(s,a) / N(s,a)                       mean value of action a
U(s,a) = c_puct · P(s,a) · √Σ_b N(s,b) / (1 + N(s,a))
```

per-edge quantities:

- **`P(s,a)`** — the network's prior probability for move `a` in
  position `s`, stored once when the node is expanded. Never changes.
- **`N(s,a)`** — visit count: how many simulations have passed through
  this edge. The search's experience.
- **`W(s,a)`** — total value accumulated through this edge (from the
  perspective of the player who chose at `s` — see the sign convention,
  §4.3).
- **`Q(s,a)`** — the exploitation term: "what has this move been
  worth, on average?"
- **`U(s,a)`** — the exploration term. It is large when the prior
  `P(s,a)` is high or the visit count `N(s,a)` is low; it decays as
  the edge gets visited. `Σ_b N(s,b)` in the numerator means: as *any*
  sibling gets visited, all unexplored siblings become more attractive.
- **`c_puct`** — the exploration constant. Ours starts at **1.5**
  [paper — ELF OpenGo's value]. The honesty ledger applies: **DeepMind
  never published a numeric `c_puct`**; 1.5 is ELF's choice, ours is a
  tuning knob ±0.5 [experiment].

A worked miniature — root with three candidate moves after some
simulations, `c_puct = 1.5`, `ΣN = 100`:

| move | P | N | W | Q = W/N | U | Q + U |
|---|---|---|---|---|---|---|
| a | 0.50 | 60 | 30 | 0.50 | 1.5·0.5·10/61 ≈ 0.12 | **0.62** |
| b | 0.30 | 30 | 12 | 0.40 | 1.5·0.3·10/31 ≈ 0.15 | 0.55 |
| c | 0.20 | 10 | −1 | −0.10 | 1.5·0.2·10/11 ≈ 0.27 | **0.17** |

Move `a` leads on value, and its high prior keeps `U` meaningful even
at 60 visits. Move `c` has a terrible track record, so even a
comparatively big exploration bonus can't rescue it — yet. Note the
shape of the dynamics: `U` for `c` grows as the *others* get visited
(√ΣN grows), so if `a` keeps being chosen, eventually `c` gets another
look. The formula spends the simulation budget where the network is
curious *and* where results are promising, and it never permanently
writes a move off.

Versus classical UCT, the whole difference is `P(s,a)`: a good network
concentrates the search on the handful of moves that matter — which is
exactly what Gomoku's tactical positions need, where one forcing move
dominates and 150 legal alternatives are noise.

## 4. One simulation, end to end, in our system

Here is the full anatomy of a single simulation, naming the component
that does each step. This section is the integration answer in
miniature.

### 4.1 Select

Starting at the root, walk down: at each node, compute PUCT over its
edges and descend the winner, until reaching a **leaf** — a node that
is either *terminal* (game over, per the engine) or *unexpanded* (never
evaluated yet).

Board handling during the walk: the tree stores moves on edges, not
boards in nodes. One `Board` (engine value type) is cloned or
mutated-and-unwound along the path — the engine offers both cheap
`Clone` and `play`/`undo`, and which one the tree uses is a
measurement question, not a design question (ch. 12 §7). Either way,
**all rules knowledge comes from the engine crate**: legality, win
detection, draw at 225. The tree never reimplements a rule.

### 4.2 Terminal check first

Before anything network-related: ask the engine for the leaf's status.

- **Won** — the player who just moved made five (overlines count —
  freestyle rules, ch. 13 decision 1). The leaf's value is exact:
  *from the perspective of the side to move at the leaf*, this is a
  loss, `v = −1`. No network call.
- **Draw** (225 moves) — `v = 0`. No network call.
- **Ongoing** — expand and evaluate (next step).

This ordering matters twice over: it gives exact values where truth is
available (the search propagates *certainty* up from terminal nodes —
that is how the tree learns "this move forces a win in 3"), and it
saves GPU evaluations on positions that need none.

### 4.3 Expand and evaluate

For an ongoing leaf:

1. **Encode.** The engine's `encode` turns the board into the 17×17
   `u8` planes (border ring as opponent stones — ch. 13; the network
   reads walls correctly at edges). Burn-free, pure arrays.
2. **Evaluate.** The planes plus the legal-move mask go into an
   `EvalRequest`; what comes back is `EvalResult { policy, value }`:
   logits over all 225 cells, and `v ∈ [−1, 1]` from the side-to-move's
   perspective. Who answers the request depends on the mode — that
   indirection is the `Evaluator` trait, §6.2.
3. **Mask and normalize.** Policy logits for illegal cells are
   discarded; the rest are softmaxed into a proper distribution over
   legal moves. (A classic bug farm: normalize *after* masking, not
   before, and remember the 17×17 border cells are never legal targets
   — the policy head emits 225 logits exactly so the border is excluded
   by construction.)
4. **Create edges.** The leaf gets one edge per legal move, each
   carrying its prior `P = p_a`, `N = 0`, `W = 0`.

### 4.4 Back up — and the sign convention (read this twice)

Walk back up the path to the root. For every edge traversed:

```text
N += 1
W += v_from_the_perspective_of_the_player_who_chose_at_that_node
```

The network's `v` is always stated from the perspective of the player
**to move at the evaluated leaf**. But edges belong to alternating
players. So the value flips sign at every level of the ascent:

```text
leaf evaluation: v          (good for the player to move at the leaf)
parent's edge:   W += -v    (what's good for my opponent is bad for me)
grandparent:     W += +v
... alternating up to the root
```

Equivalently: negate `v` once at the leaf ("value for the player who
*just moved*"), then flip on every step up. Terminal values follow the
same rule — we defined them from the side-to-move's perspective in §4.2
precisely so one uniform backup handles both.

This alternating-perspective bookkeeping is the single most fertile bug
farm in every MCTS implementation ever written (your DeepGomoku scars
remember). The milestone-2 tactical tests exist largely to catch sign
errors: if MCTS with a sane prior doesn't win a won position, the sign
convention is suspect number one.

### 4.5 Repeat, then move

After 400 simulations, the root's edges hold visit counts. How a move
is chosen from them — and the extra ingredients that apply only at the
root, only during self-play — is §5.

## 5. From tree to move: π, temperature, and root noise

### The training target π

The visit-count distribution over the root's edges *is* the improved
policy π that training will later teach the network to predict
directly. Note the economy of it: the same search that plays the game
produces the label.

### Temperature: how a move is picked from counts

- **Moves 1–12 of a self-play game** [derived: AGZ used 30 moves on
  ~200-move Go games [paper]; scaled to ~60-move Gomoku games]:
  **sample** the move with probability ∝ `N^(1/τ)`, `τ = 1`
  [experiment]. Proportional sampling keeps opening games diverse —
  otherwise every self-play game starts with the same currently-fashionable
  moves and the replay buffer converges to a monoculture.
- **After move 12, and always in arena/competitive play**: `τ → 0`,
  i.e. simply play the most-visited move. Maximum strength, no
  randomness.

### Dirichlet noise at the root (self-play only)

At the root of every self-play search, the priors are perturbed:

```text
P_root(a) = (1 − ε)·P(a) + ε·η_a,     η ~ Dirichlet(α)
ε = 0.25 [paper — AGZ/KataGo form]
α = 0.1  [derived: AlphaZero's α ≈ 10/(typical legal moves):
         Go 0.03@361, shogi 0.15@~80, chess 0.3@~35 [paper];
         Gomoku midgame ≈ 100–150 legal → 10/125 ≈ 0.08 → 0.1]
         [experiment]
```

Purpose: guarantee that *every* root move keeps a nonzero prior, so
PUCT's exploration term can never be zeroed out by an overconfident
young network. It's the exploration floor. Two discipline points: the
noise is applied **only at the root** (interior nodes keep the clean
network prior — we want diversity in which *game* we explore, not
static inside each lookahead), and **only in self-play** (arena games
measure strength; noise would just blur the Elo measurement).

### Resignation

Self-play games resign when the root value stays below a threshold,
recalibrated each iteration to a 5% false-positive rate from arena
games (method from AGZ [paper — via ELF]). It is pure throughput: games
that are decided stop burning simulations. Milestone 2 ignores this;
it lands with the self-play crate.

## 6. The software architecture: who owns what

Now the integration question properly: engine, tree, and network as
Rust crates. Boundaries follow ch. 12 §6.

### 6.1 Engine: the rules oracle, nothing more

The `mcts` crate depends on `engine` and uses exactly four kinds of
things from it:

- `Board` — clone / play / undo during selection walks (§4.1);
- `status()` — terminal checks (§4.2);
- `empty_moves()` / legality — the move set at expansion (§4.3);
- `encode()` — board → 17×17 planes (§4.3).

The engine stays a dependency island: no Burn, no channels, no I/O —
it runs identically on all 14 worker threads. MCTS never inspects
bitboards, never reimplements a rule, never special-cases a win. If
the engine says the game is over, it is over.

### 6.2 The Evaluator trait: the only coupling between search and network

The tree needs to ask "what is fθ(s)?" — nothing else. That is one
trait in the `mcts` crate (ch. 12 §6):

```rust
pub trait Evaluator {
    /// Evaluate one position: policy priors over legal moves + value.
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult;
}
```

`EvalRequest` carries plain data — encoded planes, legal-move mask,
and (in the self-play implementation) a oneshot reply channel.
`EvalResult` carries plain data back — 225 policy logits and a value.
Three implementations, three lifecycles:

| Implementation | Used by | Shape |
|---|---|---|
| **Mock** (rules the engine's tactics module into a deterministic prior/value) | MCTS unit tests, milestone-2 tactical suite | two lines of logic, no Burn |
| **Direct net wrapper** (`net::Model` behind the trait) | arena play, integration tests | synchronous forward pass in-process |
| **Channel client** (holds a sender to the evaluator service) | self-play workers | blocking send/recv — §6.4 |

Why a trait and not a generic closure `Fn(Board) -> (Policy, Value)`?
Because the production evaluator is a *stateful actor client* (channel
+ pending request), and closures capture awkwardly around that
[ch. 12 §6]. The named trait keeps `mcts` compilable and testable with
no GPU in sight — milestone 2 builds and tests the entire search before
the `net` crate exists.

### 6.3 The tree in Rust: an arena, not a graph of boxes

A tree node owns children that own children — the classic shape where
naive Rust (`Box`, `Rc<RefCell<_>>`) gets noisy. The standard engine
answer, and ours:

```text
nodes: Vec<Node>                    // one growable arena per search
Node  { edges: <small collection of Edge> }
Edge  { mv: Move, prior: f32, n: u32, w: f32, child: Option<u32> }
                                                  ▲
                                     index INTO nodes, not a pointer
```

Children are `u32` indices into the arena. No reference counting, no
borrow fights, no per-node allocation; the whole tree is a couple of
contiguous `Vec`s — cache-friendly in exactly the hot loop that runs
400 times per move. Per-move node counts are small (a few hundred to
~1–2k nodes at 400 simulations), so memory is a non-issue.

After the move is played, the tree can be discarded — or the subtree
under the played edge can be **reused** as the next search's root
(AGZ did this [paper — via ELF]): the prior search's experience carries
over, a free strength boost. Whether we reuse is an `[experiment]` the
milestone-2 design will settle; the arena representation supports both
(reuse = re-root the index, keep the Vec).

### 6.4 Parallelism: many trees, one batched evaluator

Recap of ch. 12 §8–9, from the MCTS seat:

- **Game-level parallelism.** Each of the 14 self-play workers plays
  its own game with its own single-threaded tree. No shared tree, no
  locks, no virtual loss. Each worker's data is cache-private. We
  parallelize by having *more trees*, not more threads per tree —
  because evaluations come from one shared GPU in large batches; what
  we lack is trees, not threads (the AlphaGo situation, inverted).
- **The evaluator service is the only synchronizer.** A worker's
  `evaluate()` blocks: send `EvalRequest`, wait on the oneshot. The
  service collects requests from all workers into batches (≤128,
  ≤2 ms window [experiment]), makes **one** GPU forward pass, and
  distributes results. Workers never touch Burn types; the model lives
  on the evaluator thread alone — ownership instead of
  `Arc<Mutex<weights>>`.
- **Backpressure is free.** Bounded channels throttle workers to GPU
  speed automatically.

Consequence for the `mcts` crate itself: it is written as purely
single-threaded code. All concurrency lives in `selfplay` (workers,
channels, service). The tree neither knows nor cares that its
`evaluate()` call crosses a process-wide batching point — that is the
trait boundary doing its job.

### 6.5 Where the network sees the board

One data-flow detail worth fixing in your head, because three chapters
intersect here:

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

The relative me/you view is derived at encode time — the network never
learns "black" vs "white", halving the state space — while the engine
stores absolute colors (Swap2's non-alternating opening, ch. 13
decision 5). The 17×17 border ring is added at encoding, in 15×15
symmetry space the D4 transforms commute with it, and the policy head
emits 225 logits so border cells are never move targets. Every one of
those sentences is a decision from ch. 12/13 arriving at one diagram.

## 7. What milestone 2 actually implements

The crate, in build order (each step testable in isolation):

1. **Arena tree** — `Vec<Node>` + edge indices (§6.3), with the
   per-edge statistics `P, N, W` and derived `Q`.
2. **PUCT selection** — the formula of §3, descending from root to
   leaf, board cloned/unwound along the path.
3. **Expansion** — terminal check via engine; otherwise encode →
   `evaluate()` → mask illegal → normalize → create edges (§4.2–4.3).
4. **Backup** — visit counts and sign-flipped value accumulation
   (§4.4). Unit-tested on hand-built trees where the correct `Q`
   values are computable by hand.
5. **Move selection** — visit-count extraction, temperature sampling
   vs argmax, Dirichlet root noise (§5). Deterministic under a seeded
   RNG for tests.
6. **The mock evaluator** — tactics-module-shaped priors (immediate
   wins get high prior and near-certain value; forced blocks likewise;
   else mild shaping). Deterministic. This is what lets the whole
   crate be built and *proven* with no network and no GPU.
7. **The acceptance tests** (ch. 12 §12 items 1–2, milestone 2 row):
   - **Tactical suite**: win-in-1 solved at 50 simulations; forced
     blocks taken; win-in-3 sequences found. Threat puzzles are free
     oracle data (Gomoku's solved positions — ch. 12 §3).
   - **Sanity**: 1,000 full games against a uniform-random evaluator
     — every move legal, every game terminates, no panic, no leak.

What milestone 2 explicitly does **not** build: the network interface
beyond the trait (milestone 3), workers/channels/batching (milestone
4), resignation, gating, Elo. One crate, one responsibility: given a
position and a way to evaluate positions, produce a strong move and a
visit-count distribution.

## 8. Deliberately shelved (with the shelf labeled)

- **Transposition tables / DAG search.** The classic use of Zobrist
  keys is *not* used in search: the tree stays a tree, as in AlphaZero.
  PUCT statistics interact subtly with merged nodes; Gomoku's
  transposition rate is low (stones never move once placed); payoff
  unproven at our scale. Full reasoning:
  [05-deep-dive/01](../13-engine-tutorial/05-deep-dive/01-what-zobrist-hashing-is-good-for.md)
  §6. Revisit only if profiling shows duplicate subtrees dominating.
- **Tree parallelization + virtual loss.** AlphaGo's APV-MCTS solved a
  problem we don't have (scarce evaluations, deep single search). Ours
  is inverted; game-level parallelism wins. Ch. 12 §8 has the
  literature map if single-game latency ever matters.
- **Playout-cap randomization** (KataGo: most moves searched cheap, a
  quarter searched fully, ~1.37× efficiency [paper — KataGo]).
  **Registered upgrade #1** — but it changes the data distribution, so
  it lands only after a clean phased baseline exists (ch. 12 §11).
- **First-play urgency, RAVE, progressive widening.** Catalogued in
  the MCTS review (ch. 10 #9); unneeded while the prior is a network.

## 9. The bugs we expect (so we test for them first)

Experience (DeepGomoku's and the literature's) says these are where
MCTS implementations go wrong — the milestone-2 test plan is shaped
around them:

1. **Sign flips in backup** (§4.4) — the champion. Tactical puzzles
   catch it instantly.
2. **Mask/normalize order** — softmax before masking leaks probability
   onto illegal moves (§4.3).
3. **Noise in the wrong place** — Dirichlet applied at interior nodes
   (static in every lookahead) or in arena games (blurred Elo) (§5).
4. **Terminal values from the wrong perspective**, or network
   evaluation of terminal leaves (§4.2).
5. **π extracted from Q instead of N** — the training target is visit
   *counts*, not values (§5).
6. **Forgetting that value is relative to side-to-move** when
   positions are stored for training — the replay sample stores `z`
   from the perspective of the player to move at `s`, consistently
   with the encoding's me/you planes (§6.5).

## 10. Summary

- MCTS is the **policy improvement operator**: 400 simulations of
  select → expand → evaluate → backup turn the network's raw `(p, v)`
  into a stronger π, which is both the move and the training target.
- Selection is PUCT: exploitation `Q` plus prior-weighted exploration
  `U`; `c_puct = 1.5` is ELF's value, not DeepMind folklore.
- Terminal positions are exact engine truth and never touch the
  network; everything else is one forward pass per new leaf.
- Value perspective alternates up the tree; the sign convention is
  where implementations die, so the tactical suite guards it.
- Moves come from visit counts: sampled early in self-play games
  (τ=1, first 12), argmax after; Dirichlet noise at the root, self-play
  only.
- Crates: `engine` is the rules oracle; `mcts` is single-threaded and
  generic over an `Evaluator` trait (mock / direct-net / channel-client);
  `selfplay` owns all concurrency and the batched GPU evaluator. The
  tree is a `Vec` arena with `u32` child indices.
- Milestone 2 = tree + PUCT + backup + move selection + mock evaluator,
  proven by tactical puzzles and 1,000 random games — no network, no
  GPU, no threads.

Reading, when you want the primary sources: AlphaGo Zero (the
algorithm we implement), the MCTS review (ch. 10 #9, for the taxonomy
and everything we shelved), and ch. 12 §8 for the parallelization
reasoning. The build tutorial is [README.md](README.md) in this folder
— same conventions as the engine tutorial.
