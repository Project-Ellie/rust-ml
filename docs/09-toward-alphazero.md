# Chapter 9 — From MNIST to AlphaZero

This chapter is the bridge. It maps the AlphaGo Zero algorithm onto Burn
concepts you now know, and it defines the shape of the Gomoku project. It
has no runnable example yet — it is the design brief for what we build next.

## The algorithm in one paragraph

AlphaGo Zero trains a single network fθ(s) = (p, v): a policy vector p over
moves and a value v estimating the game outcome from position s. The loop:

1. **Self-play**: run MCTS guided by the current network. MCTS improves on
   the raw policy; play games against itself using the improved policy.
2. **Data generation**: store every position with the MCTS visit-count
   distribution π (the policy target) and the final game result z (the value
   target).
3. **Training**: minimize `(z − v)² − πᵀ log p + c‖θ‖²` on a replay buffer
   of recent positions.
4. **Evaluation** (AlphaGo Zero): gate the new network against the previous
   best; only replace it if it wins by a margin. (AlphaZero later dropped
   the gating.)

Repeat. The network improves MCTS; MCTS produces better training data than
the raw network; the loop bootstraps from random play to superhuman play.

## Component mapping: AlphaZero → Burn

| AlphaZero component | Burn home | Notes |
|---------------------|-----------|-------|
| Residual tower + policy/value heads | `#[derive(Module)]` struct | Plain CNN work: Conv2d, BatchNorm, residual adds via `x + block(x)` |
| Board encoding (input planes) | `TensorData` in the batcher | See the encoding section below |
| Loss: value MSE + policy cross-entropy + L2 | `nn::loss` + manual combination | Custom `TrainStep` returning a combined loss — you already know how |
| Replay buffer | `InMemDataset` or your own ring buffer + `Batcher` | Application code, not framework code |
| Self-play + MCTS | **your crate** | Pure Rust game logic; calls `model.forward` for evaluations. Nothing in Burn drives this |
| Training loop | **manual loop** (chapter 4), not `SupervisedTraining` | Self-play and training interleave; the dataset paradigm does not fit |
| Model gating / evaluation | your crate + records | Save candidates as records; load best and candidate; pit them |

The key architectural insight: **Burn owns the network, the optimizer, and
the batch math. You own the loop.** This is why chapter 4 (the manual
training loop) matters more for our goal than chapter 6 (`SupervisedTraining`).

## Board encoding for Gomoku

The AlphaGo Zero input is a stack of binary feature planes: current player's
stones, opponent's stones, (in Go: history planes, liberties, ...), plus a
constant plane for the side to move. For Gomoku we need much less. A first
encoding, from Wolfie's earlier `azrust` sketch:

```text
planes: 2 x (n+2) x (n+2)   for an n x n board with a 1-cell border
  plane 0 ("the turn"):  1 where the player to move has a stone
  plane 1 ("the other"): 1 where the opponent has a stone
scalar: color/side-to-move indicator
```

The border cells ("the edge") are always occupied in the opponent's plane —
a cheap way to give convolutions wall information without padding artifacts.
As a tensor: `Tensor<B, 4>` of shape `[batch, 2, n+2, n+2]`, produced by the
batcher from a compact bitboard (`[bool; 2*(n+2)*(n+2)]` plus side to move).
Two design questions we will revisit: 15x15 vs 19x19, and whether to add
history planes (the last k moves) as AlphaGo Zero does.

A deliberate simplification: Gomoku has no capture, no ko, and fixed
termination at five in a row. Relative to Go we can start with two planes
and grow only if training stalls.

## Symmetry: free data

The square board has 8 symmetries (dihedral group D4). Every training
position yields 8 equivalent ones; every MCTS evaluation can average over 8
transformed inputs. This is exactly the augmentation idea from chapter 7,
applied to boards. Plan for it in the batcher, not in the network.

## What transfers from your DeepGomoku experience

You built this loop once in TensorFlow from the original papers. What is
different this time:

- **No Python data path.** Self-play workers, the replay buffer, and the
  network live in one Rust process (or several, with records as the
  interface). No serialization tax between game logic and network.
- **The batcher is the only data bridge.** The same `Batcher` discipline
  from chapter 5 applies: raw game records in, tensors out.
- **MCTS is embarrassingly parallel in Rust.** `rayon` or threads with a
  shared model (Burn models are `Clone`; per-thread forward on one device
  works). This is where Rust should beat your TensorFlow setup on wall time.

## Milestones for the Gomoku project (preview)

1. Game engine: rules, win detection, fast bitboard, symmetry transforms.
2. MCTS with a uniform/ heuristic prior, no network. Playable baseline.
3. Policy/value network in Burn (small residual tower, 2-plane input).
4. Self-play driver + replay buffer + manual training loop (chapter 4).
5. Iteration loop: self-play → train → evaluate → promote.
6. Strength evaluation against the MCTS-only baseline and against your old
   DeepGomoku agent, if we can interface it.

We will design this properly when you are ready. The curriculum ends here;
the project begins next.

## Reading

- Papers: [chapter 10](10-papers.md) — start with AlphaGo Zero and
  AlphaGomoku.
- Book: [Custom training
  loop](https://burn.dev/books/burn/custom-training-loop.html) — the
  template for the self-play/training milestone (chapter 12, §13; note
  that chapter 12's numbering supersedes the preview list above).

## Think about

1. MCTS needs fast network evaluation on single positions or small batches.
   What batch size should self-play use, and who controls it?
2. AlphaGo Zero used 19x19 with 17 planes. We propose 15x15 with 2 planes.
   What do we lose? What experiment would tell us we lost too much?
3. The replay buffer holds positions from many network generations. How
   does stale data affect the value target, and why does AlphaZero get away
   with it?

Next: [Chapter 10 — Papers](10-papers.md)
