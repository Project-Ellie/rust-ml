# Chapter 15 — The Silver solution: how AlphaZero works, and why

An essay for the learner. It answers six questions:

1. Which algorithm, exactly, did David Silver's team use?
2. Why does the algorithm work so well?
3. Which neural network architecture, and why?
4. Why one network with two heads (policy and value), and why a shared
   trunk?
5. Why the low-level choices: SGD with momentum instead of Adam, the exact
   learning-rate schedules, and the loss function?
6. Which mathematical papers established the RL algorithm behind AlphaGo?

How to read this essay. All numbers come from the primary sources. Each
citation is marked [S#] and resolved in section 10. Where a paper is silent
and the text gives an interpretation, the text says so. Where a number is
not published anywhere, the text says so. You built DeepGomoku from the
original papers, so this essay does not explain what a CNN, a cross-entropy
loss, or MCTS is. It explains why each choice is the way it is.

The one-sentence answer, for those in a hurry:

> AlphaGo Zero is self-play reinforcement learning by approximate policy
> iteration. A single two-headed residual CNN is the function approximator.
> Monte-Carlo tree search with a PUCT selection rule acts as both the
> policy improvement operator and the policy evaluation operator. The
> network is trained to imitate the search.

The rest of the essay unpacks that sentence.

## 1. The three papers, and what each one removed

The "Silver solution" is not one design. It is a sequence of three
removals. Each paper deletes components that everyone believed were
necessary. Each deletion makes the system simpler and stronger.

| | AlphaGo (2016) [S1] | AlphaGo Zero (2017) [S2] | AlphaZero (2018) [S3] |
|---|---|---|---|
| Human game data | yes (KGS, SL policy) | no | no |
| Networks | 3 (RL policy, value, rollout policy) + SL policy | 1 (two heads) | 1 (two heads) |
| Handcrafted features | some | none (raw board + history) | none (raw board + history) |
| MCTS rollouts | yes (fast rollout policy) | no | no |
| Symmetry exploitation | — | yes (augmentation + eval averaging) | no |
| Gating (best-network selection) | — | yes (55% over 400 games) | no |
| Games handled | Go only | Go only | chess, shogi, Go |
| Hardware for the final player | 48 TPUs, distributed (Lee) | 4 TPUs, one machine | 4 TPUs, one machine |

AlphaGo (2016) was a pipeline: a supervised policy network trained on human
games, then improved by policy-gradient RL, a separate value network, a
fast rollout policy, and an MCTS that combined all of them [S1]. It beat
Fan Hui 5:0 and Lee Sedol 4:1. It contained four learned components and
decades of accumulated Go engineering.

AlphaGo Zero (2017) removed the human data, the rollouts, the handcrafted
features, and the separate networks. What remained was one network, one
search, and a self-play loop. After 3 days of training it beat the
Lee-beating version 100 games to 0. After 40 days it reached an Elo of
5,185, against 4,858 for the strongest previous version (AlphaGo Master)
and 3,739 for AlphaGo Lee [S2]. The raw network alone, with no search at
all, already rated 3,055 — roughly the distributed AlphaGo Fan, which used
176 GPUs. This is the paper our Gomoku project implements.

AlphaZero (2018) removed the last Go-specific parts: the gating step, the
symmetry exploitation, and the assumption of binary outcomes (chess and
shogi have draws). One algorithm, one architecture, one set of
hyperparameters, three games. It beat Stockfish in chess after 4 hours of
training, Elmo in shogi after 2 hours, and AlphaGo Lee after 8 hours [S3].

The pattern is the message. The strength does not come from the components
that were added over the years. It comes from the loop that survived every
removal.

## 2. The algorithm, exactly

This section is the AGZ algorithm [S2], with the AlphaZero differences [S3]
marked as they occur.

### 2.1 The network

One network: `fθ(s) = (p, v)`.

- Input `s`: the raw board position as a stack of binary planes (section 4
  gives the layout).
- Output `p`: a probability distribution over all moves, including pass in
  Go. `p_a = Pr(a|s)`.
- Output `v`: a scalar in [−1, +1], the expected game outcome from the
  perspective of the player to move. `v ≈ E[z|s]`. Produced by a tanh.

One forward pass produces everything the search needs: a prior over moves
and an evaluation of the position.

### 2.2 The search: MCTS with PUCT selection

Each move is chosen by a tree search of many simulations (1,600 per move in
AGZ, 800 in AlphaZero — do not mix these numbers). Each simulation has
three phases.

**Select.** From the root, descend the tree. At each state `s`, choose the
action that maximizes

```
a = argmax_a [ Q(s,a) + U(s,a) ]
U(s,a) = c_puct · P(s,a) · √(Σ_b N(s,b)) / (1 + N(s,a))
```

- `Q(s,a)` is the mean value of all simulations that passed through `(s,a)`.
  Exploitation.
- `U(s,a)` is the exploration bonus. It grows with the prior `P(s,a)` (the
  network's opinion) and shrinks with the visit count `N(s,a)`. The
  `√ΣN` term makes the bonus grow slowly as the node is visited more.
- `c_puct` is the exploration constant. The AGZ and AlphaZero papers do not
  publish its value. ELF OpenGo used 1.5 [S5]; treat it as an experiment
  parameter. MuZero later published a schedule that grows with visit count:
  `c(s) = log((1 + N(s) + 19652)/19652) + 1.25` [S4]. The selection rule
  with priors is called PUCT; it descends from UCT (section 7.2).

**Expand and evaluate.** When the simulation reaches a leaf `s_L`, evaluate
it once with the network: `(p, v) = fθ(s_L)`. Store `p` as the prior over
the leaf's edges. Two special cases:

- If `s_L` is terminal, use the true game outcome, not the network value.
  The network never evaluates a finished game.
- There are no rollouts. AGZ replaced the random playouts of AlphaGo 2016
  entirely with the value head. This is a large quality gain: one trained
  evaluation instead of hundreds of random moves.

**Backup.** Propagate `v` up the path. Each edge on the path updates its
visit count and its mean value `Q`. Values are negated at each ply, because
the game is zero-sum and players alternate.

The tree is reused between moves: after a move is played, the child
subtree becomes the new root. Statistics survive.

### 2.3 Self-play and data generation

Self-play games are generated with the current network as the MCTS guide.
Two exploration mechanisms operate at the root.

**Dirichlet noise.** Before the search, the root priors are perturbed:

```
P(s_root, a) = (1 − ε)·p_a + ε·η_a,   η ~ Dir(α),   ε = 0.25
```

AGZ used α = 0.03 for Go. AlphaZero scaled α in inverse proportion to the
typical number of legal moves: 0.3 for chess, 0.15 for shogi, 0.03 for Go
[S3]. The paper's reason: "this noise ensures that all moves may be tried,
but the search may still overrule bad moves" [S2]. A small α makes the
noise spiky: a few random moves get a large prior boost per game, so
different games explore different lines. (Interpretation: the α ≈
10/legal-moves rule behaves like a fixed budget of phantom visits spread
over the root's children.)

**Temperature.** After the search, the played move is sampled from

```
π_a ∝ N(s_root, a)^(1/τ)
```

For the first 30 moves of each AGZ game, τ = 1: play proportional to visit
counts, which gives diverse openings. After that, τ → 0: play the
most-visited move, which gives a clean, strong remainder of the game.
(Interpretation: early diversity feeds exploration; late greediness keeps
the value target `z` informative. A game thrown away by random midgame
play teaches the value head little.)

Each position `s_t` of the finished game is stored as a triple `(s, π, z)`:

- `s`: the board, augmented with a random dihedral symmetry in AGZ
  (8 transforms of the square board; AlphaZero dropped this because chess
  and shogi are not symmetric).
- `π`: the full MCTS visit-count distribution at that position. Not the
  played move. The whole distribution.
- `z`: the final game outcome, from the perspective of the player to move
  at `s`. +1 win, −1 loss, 0 draw (AlphaZero; Go is effectively draw-free).

AGZ also resigns games when the value estimate stays below a threshold,
with the false-positive rate held at or below 5% (checked by forcing 10% of
games to play to the end) [S2].

### 2.4 The training step

Training samples mini-batches uniformly from the replay window: the
positions of the most recent 500,000 self-play games in AGZ. (AlphaZero's
window size is not published.) Mini-batch size 2,048 in AGZ — 32 positions
per worker on 64 GPU workers with 19 CPU parameter servers. AlphaZero used
4,096.

The loss for a sample `(s, π, z)` with network output `(p, v)` is:

```
l = (z − v)²  −  πᵀ log p  +  c‖θ‖²,    c = 10⁻⁴
```

Sum over the batch, descend with SGD and momentum 0.9 (section 5). The
value and policy terms are weighted equally. The paper's justification:
"this is reasonable because rewards are unit scaled, r ∈ {−1, +1}" [S2].
Section 6 takes this equation apart.

### 2.5 The outer loop

**AGZ** runs in iterations with a gate:

1. Generate self-play games with the current best network.
2. Train a candidate on the replay window.
3. Every 1,000 training steps, checkpoint. The candidate must win at least
   55% of 400 evaluation games against the current best to replace it.
4. The winner generates the next games.

The gate protects the data stream: a bad network never generates training
games. The cost is extra compute and a slower adoption of improvements.

**AlphaZero** removed the gate. One network, updated continually; self-play
always uses the latest parameters [S3]. Simpler, and it works. (Risk,
noted in the literature: without the gate, nothing prevents a transient
regression from contaminating the replay window. In practice the window's
size and the continuous updates absorb it. ELF OpenGo and KataGo both train
this way [S9, S10].)

The headline training numbers, verified against the papers:

| quantity | AGZ (3-day) | AGZ (40-day) | AlphaZero |
|---|---|---|---|
| residual blocks | 20 | 40 | conv + 19 (all games) |
| mini-batches | 700,000 × 2,048 | 3.1M × 2,048 | 700,000 × 4,096 |
| MCTS sims / move | 1,600 | 1,600 | 800 |
| training hardware | 64 GPU workers + 19 CPU param servers | same | 5,000 TPUv1 (self-play) + 64 TPUv2 (train) |
| wall time | 72 h | 40 days | 9 h chess, 12 h shogi, 34 h Go |
| self-play games | ~4.9M | — | 44M chess, 24M shogi, 21M Go |
| final match | 100:0 vs AlphaGo Lee | 89:11 vs AlphaGo Master | 28W-72D-0L vs Stockfish; 90W-2D-8L vs Elmo; 60:40 vs AGZ 3-day |

AGZ learning-rate schedule (SGD, momentum 0.9) [S2, Extended Data Table 3]:

| steps (thousands) | learning rate |
|---|---|
| 0–400 | 10⁻² |
| 400–600 | 10⁻³ |
| 600+ | 10⁻⁴ |

AlphaZero: learning rate 0.2, dropped three times to 0.02, 0.002, 0.0002
during the 700k steps [S3] (the drop points — 100k, 300k, 500k — are in the
Science supplementary materials).

## 3. Why the algorithm works

Six ideas carry the result. The first two are the engine; the other four
remove the things that usually break RL.

### 3.1 The skeleton: approximate policy iteration

The AGZ paper says it in one sentence: "The AlphaGo Zero self-play
algorithm can be understood as an approximate policy iteration scheme in
which MCTS is used for both policy improvement and policy evaluation" [S2].

Classical policy iteration alternates two steps:

- **Policy evaluation**: compute `v^π` for the current policy π.
- **Policy improvement**: set π′ to act greedily with respect to `v^π`.
  The policy improvement theorem guarantees π′ is at least as good as π
  [S15, ch. 4.2].

AlphaZero does both steps approximately, with different tools:

- Improvement: MCTS. The visit-count distribution `π` is what the network
  policy `p` becomes after 800–1,600 steps of lookahead, error correction,
  and comparison of alternatives. It is, almost always, a better policy
  than `p` alone.
- Evaluation: the value head, trained on actual game outcomes of the
  improved policy.

Then the network is refitted to the improved targets, and the loop
repeats. This framing is not a retrofit: the Expert Iteration paper
(section 7.3) states exactly this decomposition and proves convergence in
the exact case. The reason the algorithm cannot stall at the network's own
level is structural: the teacher (search over the network) is always
stronger than the student (the network alone).

### 3.2 Search is the teacher

The bootstrap problem of self-play: if the network learns only from its
own games, where does new knowledge come from? The answer is the asymmetry
between generation and training. The network does not learn from its own
output `p`. It learns from `π`, the output of a search procedure that uses
`p` but corrects it with lookahead. Each iteration distills search quality
into the weights, so the next search starts from a better prior and a
better evaluation, and produces a still better `π`. This is the closed
loop: intuition trains analysis, analysis trains intuition. (The Expert
Iteration paper's framing, explicitly borrowed from dual-process
psychology: System 1 is the network, System 2 is the search [S9].)

Evidence that the loop discovers rather than imitates: during AGZ training,
the system rediscovered human joseki — corner patterns developed over
centuries — and then, with more training, abandoned several of them for
variants it preferred [S2, Figure 5]. Human knowledge appeared as a
transient phase, not as a ceiling.

### 3.3 The value target is the raw game result — no bootstrapping

The value head is trained on `z`, the actual outcome of the game. There is
no TD target of the form `v(s) ← r + v(s′)`. This is a deliberate,
consequential choice.

- **Unbiased targets.** `z` is a ground-truth sample of the outcome under
  the current (MCTS-improved) policy. It carries no approximation error.
- **No deadly triad.** The classic instability of RL — function
  approximation + bootstrapping + off-policy learning — requires
  bootstrapping. Remove it and the triad cannot form.
- **Cost: variance.** A single game outcome is a noisy estimate of a
  position's value. The cure is volume: tens of millions of games, and a
  replay window that averages the noise away across the batch.

Contrast with TD-Gammon (section 8.2), which did bootstrap and also
worked — in a game with dice, where luck itself supplies exploration noise.
In deterministic perfect-information games, the DeepMind team judged
unbiased targets worth their variance. Later work (KataGo) kept the same
choice [S6].

### 3.4 The policy target is a distribution, not a move

The policy head is not trained on the move that was played. It is trained
on the full visit-count distribution `π`. Three reasons.

- **Information density.** `π` encodes the search's opinion about every
  legal move: which alternatives were considered, how strongly they were
  rejected. An argmax target throws all of that away and keeps one bit of
  relative preference. (This is the same argument as soft targets in
  knowledge distillation [S18]: the relative probabilities of wrong answers
  carry most of the teaching signal.)
- **Stability.** `π` averages hundreds of simulations. It is a
  low-variance, smoothed target compared with any single decision.
- **It is the improvement operator's output.** Imitating `π` is exactly
  the "apprentice imitates expert" step of policy iteration (section 3.1).
  Imitating the played move would imitate only a sample from `π` — the
  same expectation, far more noise per sample.

### 3.5 MCTS averages out neural-network errors

The AlphaZero paper gives this argument for why MCTS pairs so well with a
deep network, and it is one of the most quotable parts of the Methods
section [S3]:

> MCTS averages over these approximation errors, which therefore tend to
> cancel out when evaluating a large subtree. In contrast, alpha-beta
> search computes an explicit minimax, which propagates the biggest
> approximation errors to the root of the subtree.

A deep network's value estimates contain spurious errors — confident,
wrong, hard to predict. If you plug such an evaluator into minimax, the
search systematically seeks out the largest errors: the max operator is an
error amplifier. MCTS instead averages evaluations over many leaf visits,
so independent errors partially cancel. The search is not just compatible
with the neural network; it is the search algorithm whose error model
matches the neural network's failure mode.

### 3.6 The value network replaces rollouts: knowledge replaces computation

AlphaGo 2016 evaluated leaves two ways — a value network and fast random
rollouts — and mixed them. AGZ deleted the rollouts. Random playouts are
cheap but their estimate is biased (the rollout policy is much weaker than
the players being simulated) and high-variance. A trained value head costs
one forward pass and returns a calibrated expectation.

The aggregate effect is visible in the search statistics. In chess,
AlphaZero searches 80,000 positions per second; Stockfish searches 70
million. In shogi, 40,000 against 35 million [S3]. AlphaZero wins with
roughly three orders of magnitude less search, because each of its
evaluations carries learned knowledge instead of statistics over random
play. Search breadth is traded for evaluation depth. This is also why the
algorithm transfers across games: nothing in it depends on how fast
rollouts can be made, only on how well positions can be evaluated.

### 3.7 Self-play is an automatic curriculum

The opponent is always exactly as strong as the learner. Early games are
between near-random players: short, simple, full of basic tactical
lessons. As the network improves, so does the opposition, and the data
always sits at the edge of current ability — the regime where the gradient
signal is richest. There is no distribution shift between training data
and deployment, because both are self-play. And the exploration mechanisms
(Dirichlet noise, τ = 1 openings) guarantee the data never collapses onto
a single deterministic line of play, which would starve the value head of
diverse positions.

(Interpretation: self-play also explains a known failure mode. If the loop
converges to a degenerate equilibrium — e.g., every game ends the same
way — the data stream dies. The noise mechanisms and the replay window are
the countermeasures. In Gomoku, expect the first-player advantage to shape
the equilibrium; the engine's Swap2 opening rule exists partly for this
reason, see chapter 13.)

## 4. The neural network architecture

### 4.1 Input: a stack of planes

AGZ input: a 19×19×17 image stack [S2]:

- 8 planes for the current player's stones, one per time step of history
  (`X_t … X_{t−7}`),
- 8 planes for the opponent's stones,
- 1 constant plane for the side to move.

Why planes at all? The board is a grid, so the state is an image; the
architecture should match the data's structure. The AlphaZero paper lists
exactly what Go gives a CNN [S3]: the rules are translationally invariant
(matches weight sharing), defined through adjacencies between neighboring
points (matches local 3×3 receptive fields), and symmetric under rotation
and reflection (matches data augmentation). History planes give the
network the recent temporal context (repetitions, ko-like situations) that
a single snapshot cannot show.

AlphaZero generalized the encoding per game [S3, Table S1]: keep the
8-step history, but replace "stone / no stone" with one plane per piece
type. Chess: 6 piece types per side + 2 repetition-count planes = 14
planes per time step, ×8 history = 112, plus 7 constant planes (color,
move count, 4 castling rights, no-progress count) = 119 planes per cell.
Shogi adds prisoner-count planes. The design principle survived unchanged:
binary planes, player-relative ("the board is oriented to the perspective
of the current player" [S3]), history stacked along channels.

### 4.2 The trunk: a residual tower

AGZ trunk [S2]:

1. Convolutional block: 256 filters, 3×3, stride 1 → batch norm → ReLU.
2. Then 19 or 39 residual blocks (for the 20-block and 40-block networks).
   Each block: conv 256 3×3 → BN → ReLU → conv 256 3×3 → BN → add the
   block's input → ReLU.

Total depth: 39 or 79 parameterized layers.

Why residual? This is the ResNet design [S19] transplanted from image
classification, and the AGZ paper says the architecture "is based on the
current state of the art in image recognition, and hyperparameters for
training were chosen accordingly" [S2]. Two reasons matter for us:

- Very deep networks train badly without skip connections: gradients
  vanish, and accuracy degrades even on the training set. The skip
  connection gives every block a direct path to the input and the loss.
- It is measured, not assumed. AGZ ran a four-way ablation (next section):
  replacing the convolutional tower with a residual tower improved playing
  strength by over 600 Elo [S2].

Batch normalization does the other half of the job: it keeps the
activation scale stable across 79 layers and across the shifting data
distribution of self-play. (One caution from the replication literature:
with a non-stationary data stream, BN running statistics lag the current
network; high BN momentum is the standard fix [S9, S10].)

### 4.3 The two heads

Policy head (AGZ) [S2]: conv 2 filters 1×1 → BN → ReLU → fully connected
layer to 19²+1 = 362 logits → softmax. (AlphaZero replaced the flat layer
with action planes for chess and shogi; see 4.5.)

Value head [S2]: conv 1 filter 1×1 → BN → ReLU → fully connected to 256 →
ReLU → fully connected to 1 → tanh.

Notes on the details:

- The 1×1 convolutions collapse the 256-channel trunk to almost nothing
  before the expensive fully connected layers. The heads are deliberately
  tiny next to the trunk: the knowledge lives in the shared body.
- tanh bounds `v` to [−1, +1], matching the range of `z`. A bounded output
  with a bounded target keeps the MSE term well-scaled and prevents
  occasional large logits from producing huge gradients.
- The value head's hidden layer of 256 is where "how is this game going"
  becomes a number. The policy head has no hidden layer at all in AGZ:
  move quality is read directly off the trunk's spatial features.

### 4.4 Why two heads, and why one trunk

This was your question, and the paper answers it with an experiment, not
an argument. AGZ trained four architectures on the same fixed dataset of
self-play games [S2, Figure 4]:

- `dual-res`: one residual tower + both heads (the AGZ design),
- `sep-res`: two residual towers, one per task,
- `dual-conv`: one convolutional (non-residual) tower + both heads,
- `sep-conv`: two convolutional towers (the AlphaGo Lee design).

Result: `dual-res` wins. The residual tower accounts for over 600 Elo.
Combining policy and value into one network accounts for roughly another
600 Elo: it "slightly reduced the move prediction accuracy, but reduced
the value error and boosted playing performance" [S2]. The paper's
explanation, worth quoting:

> Combining policy and value together into a single network … improved
> computational efficiency, but more importantly the dual objective
> regularises the network to a common representation that supports
> multiple use cases. [S2]

Unpack the two reasons.

**Compute.** MCTS needs both outputs at every leaf: the policy becomes the
edge priors, the value becomes the backup signal. Two separate towers mean
two forward passes per leaf — double the dominant cost of self-play, for
no gain. One trunk, one pass, two cheap heads.

**Regularization through a shared representation.** The value task forces
the trunk to encode features that predict the game outcome: territory,
life and death, influence, race-to-five threats. The policy task forces it
to encode features that discriminate good moves from bad. Neither task
alone needs the full concept set of the other, but the shared trunk must
serve both, so it learns the union. The value gradient flows through the
same convolutions the policy head reads from. The result: each task
trains the other's features, the effective dataset is doubled, and the
trunk cannot overfit to quirks of one objective. The small drop in raw
move-prediction accuracy is the signature of this trade — the network
gives up a little policy fit for a lot of position understanding, and
playing strength follows position understanding.

(Interpretation, for our build: this is the strongest argument against a
Gomoku design with separate policy and value networks. It is also why the
value head should stay even if a future experiment only needs the policy.)

### 4.5 AlphaZero's generalization to chess and shogi

Same trunk shape for all three games: one convolutional block, then 19
residual blocks, 256 filters throughout [S3, Methods]. What changes is the
input planes (4.1) and the action encoding:

- Go: flat distribution over 19²+1 moves, as in AGZ.
- Chess: an 8×8×73 stack of planes = 4,672 moves. 56 "queen move" planes
  (8 directions × 7 distances), 8 knight-move planes, 9 underpromotion
  planes.
- Shogi: 9×9×139 = 11,259 moves, same idea plus drop moves.

The move-as-planes encoding keeps the output spatial: a move is "from this
square, in this direction, by this much", so the same translation symmetry
that helps the input helps the output. The price is a fixed maximal move
set; illegal moves are masked and renormalized at search time.

## 5. The low-level training choices

You asked specifically about this layer. Here is what the papers say, and
what they do not say.

### 5.1 SGD with momentum, not Adam

Both papers train with stochastic gradient descent, momentum 0.9. Neither
uses Adam, RMSProp, or any adaptive method. Neither paper explains the
choice. So the following separates the verified facts from the reasoning.

Verified facts:

- The optimizer is SGD + momentum 0.9, with the L2 term in the loss
  (c = 10⁻⁴) and a step-decay learning-rate schedule [S2]. AlphaZero keeps
  the same recipe and loss, with a higher initial rate [S3].
- The AGZ paper says architecture and training hyperparameters were chosen
  to match "the current state of the art in image recognition" [S2]. In
  2017, that meant ResNet on ImageNet, and ResNet's recipe was SGD +
  momentum 0.9 + step decay [S19]. The optimizer came with the
  architecture.
- Every major replication kept SGD + momentum: ELF OpenGo [S5], KataGo
  [S6], Leela Zero. The recipe is not incidental; it reproduced.

Reasoning (literature and practice, not the papers):

- **Adaptive methods generalize worse on exactly this class of problem.**
  Wilson et al. 2017, "The Marginal Value of Adaptive Gradient Methods in
  Machine Learning" [S20], showed that on image-classification-style
  tasks, Adam converges faster early but plateaus at worse held-out
  performance than tuned SGD. Self-play RL is a perpetual
  generalization problem: the network must keep evaluating positions it
  has never seen, for the whole run. Final generalization is the metric
  that matters; early convergence speed is not.
- **The data distribution never stops moving.** Adam's per-coordinate
  second-moment estimates are tuned to the current gradient statistics.
  In self-play, those statistics shift every iteration as the policy
  improves. Momentum is a simple exponential average of the gradient
  direction; it does not calibrate itself to a distribution that is about
  to change. (This is an argument, not a measurement — no published
  ablation isolates it for AlphaZero.)
- **Step decay is understood for momentum SGD.** The large drops (factor
  10) give the run distinct phases: fast progress, consolidation,
  polishing. The same schedule shape worked from ImageNet to AGZ to every
  replication, so there was no reason to gamble on an alternative.

One nuance for our build: with SGD + momentum, an L2 term in the loss and
decoupled weight decay are almost the same mechanism (they differ by a
learning-rate factor). With Adam they are not — that is the AdamW result
[S21]. Chapter 12 chose AdamW at our small scale, deliberately trading
paper fidelity for faster convergence and less learning-rate tuning; SGD +
momentum remains the documented fallback. That trade is reasonable at our
batch sizes; at DeepMind's batch sizes (2,048–4,096) the SGD recipe is
canonical.

### 5.2 The learning-rate schedules

AGZ [S2, Extended Data Table 3]: 10⁻² until 400k steps, 10⁻³ until 600k,
10⁻⁴ afterwards. Supervised comparison runs used a parallel schedule with
their own values.

AlphaZero [S3]: 0.2, dropped three times to 0.02, 0.002, 0.0002 across the
700k steps. The higher starting rate matches the doubled batch size
(4,096 vs 2,048) — the standard large-batch scaling rule of thumb.

### 5.3 Equal weighting of the two loss terms

The value MSE and the policy cross-entropy enter the loss with equal
weight. The paper justifies this in a parenthesis: "this is reasonable
because rewards are unit scaled, r ∈ {−1, +1}" [S2]. Read it carefully,
because it is a small piece of craft:

- The value term's gradient scale is governed by the range of `z`. With
  outcomes in {−1, +1} and `v` in [−1, +1], the squared error is at most
  4 and typically far below 1.
- The policy term's gradient scale is governed by the entropy gap between
  `p` and `π`, which for a 362-way distribution is also O(1) once
  training is underway.
- Because both terms live on comparable scales, no tuning constant is
  needed between them. The equal weight is not laziness; it follows from
  the bounded outcome. (Corollary for us: if we ever change the value
  target — e.g., KataGo-style score or ownership auxiliary targets — the
  equal-weight argument must be revisited, and KataGo indeed reweights
  [S6].)

### 5.4 Batch sizes, replay window, checkpoints

- Batch 2,048 (AGZ) / 4,096 (AlphaZero): large batches keep gradient noise
  low, which matters because the value term is high-variance by design
  (section 3.3).
- Replay window 500,000 games (AGZ). Uniform sampling from recent games
  decorrelates consecutive samples (the DQN lesson) while staying near the
  current policy's state distribution (the policy-iteration requirement).
  The window is the compromise between those two pulls. AlphaZero's window
  size is not published; KataGo grows its window over the run, and chapter
  12 adopts that model [S6].
- Checkpoint every 1,000 steps (AGZ), gated at 55% over 400 games. The
  gate costs evaluation games but guarantees monotone generation quality.
  AlphaZero showed the gate is optional.

### 5.5 Exploration lives in the search, not in the loss

Notice what the training objective does not contain: no entropy bonus, no
exploration term. Exploration is entirely the search's job: Dirichlet
noise at the root (every move may be tried), τ = 1 openings (diverse
early play), PUCT's visit-count term (unexplored good-prior moves get
visited). This separation is clean engineering: the loss measures fit to
targets; the data-collection procedure owns diversity. Mixing an entropy
bonus into the loss would fight the actual goal — matching `π` — because
`π` is already as entropic as the search wants it to be.

## 6. The loss function, term by term

```
l = (z − v)²  −  πᵀ log p  +  c‖θ‖²
```

Three terms, three jobs. This is the equation you asked about, and its
specialness is worth stating precisely: the loss is where the two halves
of policy iteration are glued together.

**Term 1, `(z − v)²`: policy evaluation.** Fit the value head to the true
outcome of games played by the improved policy. Squared error on a tanh
output, target in {−1, 0, +1}. Why MSE and not a two-class cross-entropy
on win/loss? The paper treats `v` as an expectation — `v ≈ E[z|s]` — and
MSE is the natural fit for an expectation of a bounded scalar. It keeps
the unit-scale argument of section 5.3 intact, and it handles the draw
value 0 in AlphaZero without any change (a cross-entropy formulation needs
a third class or a reparametrization). Later work showed cross-entropy on
discretized outcomes also works; the choice is not sacred, but the
bounded-regression choice is clean.

**Term 2, `−πᵀ log p`: policy improvement, written as distillation.**
Cross-entropy between the search distribution and the network policy. Its
minimum over `p` is at `p = π`: the network is asked to reproduce the
result of the search, for every move, not just the move played. This is
the term that makes the loop climb: `π` is the output of an improvement
operator, so fitting `π` pulls the network above its own raw-policy level,
every iteration, with supervision generated on the fly. It is
self-distillation through a search amplifier — the network teaches itself,
but only after the search has upgraded the lesson. (Section 3.4 covers why
a soft distribution rather than a one-hot target; the short version: the
rejected alternatives carry most of the information [S18].)

**Term 3, `c‖θ‖²`: smoothness.** L2 regularization, c = 10⁻⁴. Its job in
this system is larger than generic overfitting prevention. MCTS trusts the
network: the prior steers selection, the value steers backup. A network
that overfits the replay window becomes locally erratic — priors spike on
memorized positions, values swing between neighboring positions — and the
search built on top of it degrades, which degrades the next data. Weight
decay keeps the function smooth, which keeps the search meaningful, which
keeps the data stream healthy. The regularizer protects the loop, not just
the fit.

**What is deliberately absent.** The equation is also defined by four
things it does not contain:

- No TD/bootstrapped term. Targets are unbiased (section 3.3).
- No entropy bonus. Exploration is the search's job (section 5.5).
- No importance weights or off-policy corrections. The replay window is
  fresh enough, and the policy iteration framing tolerates the drift —
  the targets (`π`, `z`) come from the improved policy, which is what we
  want to imitate, whoever generated the states.
- No baseline or advantage subtraction. There is no policy-gradient term
  at all in the loss. The RL signal enters through the data (the search
  results), not through the gradient estimator. This is the deepest
  difference from REINFORCE-style training: AlphaZero does not estimate
  ∇J directly. It runs policy iteration and supervises the pieces.

That last point is the one-sentence answer to "what is so special about
the loss": it is not a policy gradient. It is a supervised objective whose
targets are produced by an improvement operator, which is why it trains
stably at scales where direct policy-gradient methods are fragile.

## 7. The mathematical papers behind the algorithm

You asked for the paper that established the RL algorithm behind AlphaGo.
There are three layers to the answer — the policy-gradient layer, the
tree-search layer, and the loop layer — and each has its own canonical
paper. All three predate AlphaGo by years. The DeepMind work was to make
them survive contact with deep networks at scale.

### 7.1 The RL part: REINFORCE and the policy gradient theorem

AlphaGo 2016 trained its RL policy network by policy gradient: play
self-play games, then move the policy's parameters along

```
∇J(θ) = z · ∇ log p(a|s)
```

with `z = ±1` the game outcome. Increase the log-probability of moves in
won games, decrease it in lost games. This is REINFORCE, from:

- **Williams (1992), "Simple statistical gradient-following algorithms
  for connectionist reinforcement learning", Machine Learning 8:229–256**
  [S7]. The likelihood-ratio trick: the gradient of expected reward can
  be written as an expectation of `reward × ∇log π`, which is estimable
  from samples alone, with no model of the environment.

The theorem that turned this trick into a foundation is:

- **Sutton, McAllester, Singh, Mansour (2000), "Policy Gradient Methods
  for Reinforcement Learning with Function Approximation", NIPS 1999**
  [S8]. Two results. First, the policy gradient theorem:

  ```
  ∇J(θ) = Σ_s d^π(s) Σ_a Q^π(s,a) · ∇π(a|s;θ)
  ```

  — the gradient depends on the state distribution `d^π` but, remarkably,
  not on its derivative, so sample-based estimation stays valid. Second,
  the first proof that policy iteration with arbitrary differentiable
  function approximation converges (to a locally optimal policy). Before
  this paper, function approximation in RL was theoretically adrift; after
  it, parameterized policies had a convergence guarantee.

This is the mathematical paper behind AlphaGo's RL stage. Note the
personal line: Richard Sutton was David Silver's PhD supervisor. The
policy-gradient theorem is, almost literally, the family business. (Its
echo inside AlphaZero is indirect: by 2017 the explicit policy-gradient
step was gone — replaced by distillation from search, section 6 — but the
object being optimized is still the expected outcome of a parameterized
policy, and the convergence intuition still comes from this paper.)

### 7.2 The search part: UCB1, UCT, and PUCT

- **Auer, Cesa-Bianchi, Fischer (2002), "Finite-time analysis of the
  multiarmed bandit problem", Machine Learning 47** [S12]. UCB1: choose
  the arm maximizing `mean + √(2 ln n / n_a)`. Logarithmic regret,
  proven. The exploration bonus that still lives, barely modified, inside
  PUCT's `√ΣN / (1 + N)` term.
- **Coulom (2006), "Efficient Selectivity and Backup Operators in
  Monte-Carlo Tree Search"** [S13]. The MCTS template itself — select,
  expand, simulate, backup — first demonstrated in the Go program Crazy
  Stone. This is the paper that made tree search work in Go, the domain
  where alpha-beta had failed for decades.
- **Kocsis & Szepesvári (2006), "Bandit Based Monte-Carlo Planning",
  ECML** [S14]. UCT: apply a UCB rule at every internal node of the tree.
  The paper proves UCT is consistent — given enough samples, it converges
  to the minimax value — and derives finite-sample error bounds. This is
  the mathematical paper behind the search half of AlphaGo: it turned
  Coulom's heuristic into an algorithm with guarantees.
- **Rosin (2011), "Multi-Armed Bandits with Episode Context", Annals of
  Mathematics and Artificial Intelligence 61(3)** [S15]. PUCT: UCT with
  prior probabilities inside the exploration bonus. This is the selection
  rule AGZ and AlphaZero actually use, with the network's `p` as the
  prior. Honest caveat: unlike UCT, PUCT with learned priors has no
  published convergence or regret proof. The theory covers the skeleton;
  the neural-network prior is extra-theoretical, and works anyway.
- Worth one more ancestor, because it is Silver's own: **Gelly & Silver
  (2007), "Combining Online and Offline Knowledge in UCT", ICML** [S16] —
  the MoGo line, first strong (dan-level) Go program, and the first
  demonstration that learned knowledge injected into UCT multiplies its
  strength. AlphaGo's MCTS is the direct descendant.

### 7.3 The loop part: Expert Iteration

- **Anthony, Tian, Barber (2017), "Thinking Fast and Slow with Deep
  Learning and Tree Search", NeurIPS 2017, arXiv:1705.08439** [S9].

This is the paper you are most likely looking for when you ask about "the
math behind the AlphaZero algorithm". It defines Expert Iteration (ExIt):
decompose RL into an *expert* (a planner that improves a policy by
lookahead) and an *apprentice* (a function approximator that imitates the
expert and generalizes its decisions). Iterate: the apprentice imitates
the expert; the expert plans on top of the apprentice; the improved plans
become new imitation targets. The paper proves that in the exact,
tabular case ExIt converges to the optimal policy, and demonstrates the
approximate version on Hex.

AlphaGo Zero is Expert Iteration with MCTS as the expert and a residual
CNN as the apprentice. The AGZ paper's own summary — "approximate policy
iteration in which MCTS is used for both policy improvement and policy
evaluation" [S2] — is the same statement in different vocabulary. If you
read one theory paper after the three Silver papers, read this one: it is
short, it names the moving parts, and it tells you exactly which
assumptions the proofs need and the AlphaZero loop drops.

### 7.4 The classical root

- **Howard (1960), "Dynamic Programming and Markov Processes"** [S11]:
  policy iteration itself.
- **Sutton & Barto, "Reinforcement Learning: An Introduction"** [S10],
  ch. 4.2: the policy improvement theorem — acting greedily w.r.t. `v^π`
  yields a policy at least as good as π. Every improvement step of the
  AlphaZero loop is an approximate, amortized application of this theorem.

### 7.5 What is proven, and what is not

Worth stating plainly, because the field's folklore overstates it:

| claim | status |
|---|---|
| UCB1 has logarithmic regret | proven [S12] |
| UCT converges to minimax | proven [S14] |
| Policy gradient with function approximation converges to a local optimum | proven [S8] |
| ExIt converges to the optimal policy | proven, exact tabular case only [S9] |
| PUCT with learned priors converges | **not proven** |
| The AlphaZero loop (deep network + MCTS + self-play) converges | **not proven** |

The engine that beat Lee Sedol, Stockfish, and Elmo has no convergence
theorem. It has a convergent skeleton (policy iteration, UCT, ExIt-exact)
plus components that break every assumption of the proofs (deep function
approximation, learned priors, non-stationary replay data, finite search
budgets). Its justification is empirical: it reproduces — DeepMind, ELF,
Leela, KataGo, alpha-zero-general, and your own DeepGomoku. This is the
normal state of deep RL, not a scandal; but when you make design choices
for our build, know which guarantees you are standing on.

## 8. Side explorations

### 8.1 Silver's decade: the removals were the research

The AlphaZero design looks inevitable in hindsight. It was the residue of
ten years of subtracting one crutch at a time:

- 2007: **RLGo** (Silver, Sutton, Müller, "Reinforcement Learning of
  Local Shape in the Game of Go") — TD learning over handcrafted
  3×3 pattern features. Weak amateur play. The lesson: local patterns
  alone do not scale [S2, related work].
- 2007–2008: **MoGo** (Gelly & Silver) — UCT plus learned pattern priors.
  First dan-level Go program. The lesson: learned knowledge inside tree
  search multiplies strength [S16].
- 2009: Silver's PhD thesis with Sutton at the University of Alberta,
  "Reinforcement Learning and Simulation-Based Search in Computer Go"
  [S17]. The title is the whole future program: learning plus search.
- 2016: **AlphaGo** — deep networks for policy and value, MCTS with
  rollouts, human data to start [S1].
- 2017: **AlphaGo Zero** — remove human data, remove rollouts, merge the
  networks [S2].
- 2018: **AlphaZero** — remove gating, symmetries, and Go-specific
  structure [S3].
- 2020: **MuZero** — remove even the rules: learn the dynamics model, and
  the same loop masters Atari as well [S4].

Read as a sequence, each step deleted the most human part that remained,
and each deletion either held or raised the playing strength. The
algorithm you implement in Rust is the fixed point of that process.

### 8.2 TD-Gammon, the ancestor

Tesauro's TD-Gammon (1992–1995) trained a value network purely by
self-play temporal-difference learning and reached strong master level in
backgammon — the first proof that self-play plus function approximation
can produce world-class play [S22]. AlphaZero differs in three ways:
perfect-information games (no dice to supply exploration noise — Dirichlet
noise and temperature replace the dice), search in the loop (TD-Gammon
used the raw network with shallow lookahead), and no bootstrapping in the
value target. The AGZ paper cites TD-Gammon explicitly as the closest
prior success of self-play RL.

### 8.3 What "without human knowledge" means, precisely

The claim is easy to over-read. AGZ/AlphaZero do receive: the game rules
(as the MCTS simulator and the legality masks), the board's grid structure
(baked into the CNN), the history-plane representation, and the
hyperparameters (chosen by Gaussian process optimization for the MCTS
parameters [S2]). What they do not receive: human game records, handcrafted
evaluation features, human-designed heuristics, or human strategy. The
honest summary is the paper's own: "tabula rasa" refers to game knowledge,
not to engineering.

### 8.4 Why AlphaZero dropped the gate

AGZ's 55%/400-game gate is an insurance policy: bad networks never poison
the data stream. AlphaZero deleted it and simply always trains on data
from the latest network [S3]. Why acceptable? The replay window mixes data
from many recent checkpoints, so a transient regression dilutes instead of
dominating; the continuous update fixes regressions quickly; and the
evaluation matches showed no instability. What they got back: a much
simpler system (no evaluator worker, no best-checkpoint bookkeeping) and
zero training steps spent "stuck" behind a gate waiting for a lucky
evaluation. ELF OpenGo's ablations and KataGo's runs both confirmed the
gate is unnecessary at scale [S9, S10]. Chapter 12 follows AlphaZero here.

### 8.5 What KataGo proved about the design

KataGo [S6] is the strongest evidence that the *ideas*, not the exact
constants, carry the result. It changed almost every detail — auxiliary
targets (ownership, score), playout-cap randomization (most moves
searched cheaply, a few deeply), a growing replay window, forced root
playouts to fix Dirichlet-noise blind spots — and reproduced superhuman
strength at roughly an order of magnitude lower cost than the DeepMind
runs. The loop itself never changed: search improves policy, network
imitates search. When we tune our Gomoku agent, chapter 12 defers to
KataGo for exactly this reason: it is the best map of which knobs matter
and which are ceremony.

## 9. What this means for our Gomoku build

The design in chapter 12 is this essay, scaled down. The correspondence:

- Same loss, same three terms, equal weighting — valid because our `z` is
  also unit-scaled {−1, 0, +1} (draws exist on a filled 15×15 board).
- Same dual-res architecture family, scaled: 128 channels × 10 blocks
  instead of 256 × 20, and 2 input planes instead of 17 to start
  (Gomoku has no captures or ko; chapter 9 explains the encoding).
- Same PUCT-MCTS; `c_puct` unpublished in the papers, so we anchor at
  ELF's 1.5 and tune.
- Same exploration: Dirichlet noise at the root with α scaled by legal
  moves — starting value ≈ 0.1 for 15×15 (~225 moves), between Go's 0.03
  and shogi's 0.15 — plus τ = 1 for the opening phase.
- No gating (AlphaZero style), a KataGo-style growing replay window.
- One deliberate deviation: AdamW instead of SGD + momentum at our scale
  (section 5.1; the SGD recipe stays as fallback). The mechanism the
  papers rely on — L2/decoupled decay at 10⁻⁴ — is preserved.
- The honesty table of section 7.5 applies to us unchanged: we are
  building an empirically justified system, and the replication record
  (alpha-zero-general, DeepGomoku, ELF, KataGo) is the ground we stand on.

## 10. References

Primary sources (the Silver solution):

- [S1] Silver et al., *Mastering the game of Go with deep neural networks
  and tree search*, Nature 529, 2016. https://doi.org/10.1038/nature16961
- [S2] Silver et al., *Mastering the game of Go without human knowledge*,
  Nature 550, 2017. https://doi.org/10.1038/nature24270 — open-access
  author copy: https://discovery.ucl.ac.uk/10045895/1/agz_unformatted_nature.pdf
- [S3] Silver et al., *A general reinforcement learning algorithm that
  masters chess, shogi, and Go through self-play*, Science 362, 2018.
  Preprint: https://arxiv.org/abs/1712.01815
- [S4] Schrittwieser et al., *Mastering Atari, Go, chess and shogi by
  planning with a learned model* (MuZero), Nature 588, 2020.
  https://arxiv.org/abs/1911.08265

Replications and engineering analyses:

- [S5] Tian et al., *ELF OpenGo: An Analysis and Open Reimplementation of
  AlphaZero*, 2019. https://arxiv.org/abs/1902.04522
- [S6] Wu, *Accelerating Self-Play Learning in Go* (KataGo), 2019.
  https://arxiv.org/abs/1902.10565

The mathematical foundations:

- [S7] Williams, *Simple statistical gradient-following algorithms for
  connectionist reinforcement learning* (REINFORCE), Machine Learning 8,
  1992.
- [S8] Sutton, McAllester, Singh, Mansour, *Policy Gradient Methods for
  Reinforcement Learning with Function Approximation*, NIPS 1999.
  https://proceedings.neurips.cc/paper_files/paper/1999/file/464d828b85b0bed98e80ade0a5c43b0f-Paper.pdf
- [S9] Anthony, Tian, Barber, *Thinking Fast and Slow with Deep Learning
  and Tree Search* (Expert Iteration), NeurIPS 2017.
  https://arxiv.org/abs/1705.08439
- [S10] Sutton & Barto, *Reinforcement Learning: An Introduction*, 2nd
  ed., MIT Press, 2018.
- [S11] Howard, *Dynamic Programming and Markov Processes*, MIT Press,
  1960.
- [S12] Auer, Cesa-Bianchi, Fischer, *Finite-time analysis of the
  multiarmed bandit problem*, Machine Learning 47, 2002.
- [S13] Coulom, *Efficient Selectivity and Backup Operators in
  Monte-Carlo Tree Search*, Computers and Games 2006.
- [S14] Kocsis & Szepesvári, *Bandit Based Monte-Carlo Planning*, ECML
  2006. https://link.springer.com/chapter/10.1007/11871842_29
- [S15] Rosin, *Multi-Armed Bandits with Episode Context*, Annals of
  Mathematics and Artificial Intelligence 61(3), 2011.
- [S16] Gelly & Silver, *Combining Online and Offline Knowledge in UCT*,
  ICML 2007.
- [S17] Silver, *Reinforcement Learning and Simulation-Based Search in
  Computer Go*, PhD thesis, University of Alberta, 2009.

Architecture and optimization background:

- [S18] Hinton, Vinyals, Dean, *Distilling the Knowledge in a Neural
  Network*, 2015. https://arxiv.org/abs/1503.02531
- [S19] He, Zhang, Ren, Sun, *Deep Residual Learning for Image
  Recognition*, CVPR 2016. https://arxiv.org/abs/1512.03385
- [S20] Wilson et al., *The Marginal Value of Adaptive Gradient Methods
  in Machine Learning*, NeurIPS 2017. https://arxiv.org/abs/1705.08292
- [S21] Loshchilov & Hutter, *Decoupled Weight Decay Regularization*
  (AdamW), ICLR 2019. https://arxiv.org/abs/1711.05101

History:

- [S22] Tesauro, *Temporal Difference Learning and TD-Gammon*,
  Communications of the ACM 38(3), 1995.

Related chapters: [9 — From MNIST to AlphaZero](09-toward-alphazero.md),
[10 — Papers](10-papers.md), [12 — The Gomoku
Architecture](12-gomoku-architecture.md).
