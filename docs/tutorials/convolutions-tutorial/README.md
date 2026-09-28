# Convolutions tutorial — hand-written kernels for Gomoku threat maps

A side-quest tutorial in the rust-ml Gomoku project, sitting between
the [datagen tutorial](../datagen-tutorial/README.md) and the
upcoming net tutorial. You build `gomoku/crates/patterns`: a small
convolutional network in Burn 0.21 whose weights are **written by
hand, not trained**. It reads a board and produces threat maps —
wins, double threats, and the double-three fork — every one proven
equal to an independent oracle on a thousand random boards, on CPU
and GPU alike.

No training loop. No waiting. You write every weight, so you understand
every weight — and convolution stops being magic.

## How this tutorial works

Same conventions as the mcts and datagen tutorials:

- **Read the primer first.** [`00-convolutions-primer.md`](00-convolutions-primer.md)
  pins the semantics (the threat ladder, the kernel language, the
  oracle ladder). The chapters derive; the primer decides.
- **Chapters give contracts, you write the implementation.** Each
  chapter has learning goals, precise contracts, and a TDD checklist
  with exact test names. The implementation is deliberately withheld.
- **Stuck for more than ~20 minutes?** That is a coaching
  conversation, not a failure — but each chapter also has an opt-in
  `NN-name/01-solution.md` quoting the verified reference
  implementation verbatim.
- **Gates per chapter:** `cargo test -p patterns`,
  `cargo clippy --all-targets -- -D warnings`, and
  `cargo fmt --all -- --check`, all from `gomoku/`. Chapter 4 explains
  the gate scheduling (fast default suite, full proof one flag away).

## Chapter map

| # | Chapter | What you build |
|---|---------|----------------|
| 00 | [Convolutions primer](00-convolutions-primer.md) | The design authority: threat ladder, kernel language, oracle ladder, backend story. |
| 01 | [Planes and a conv by hand](01-planes-and-a-conv-by-hand.md) | Board → two channels (stones / blocked-with-border); a nested-loop cross-correlation in plain Rust. |
| 02 | [Trusting the framework](02-trusting-the-framework.md) | Burn tensors, `Conv2d`, manual weight assignment; your loop must match `conv2d` exactly. |
| 03 | [The kernel language](03-the-kernel-language.md) | Exact-pattern kernels (+1/−1/bias/ReLU); the pattern table; the win map. |
| 04 | [The four-level](04-the-four-level.md) | Open/broken four makers; the 1×1 counting combiner; the engine differential gate. |
| 05 | [Three-makers](05-three-makers.md) | The three pattern families as kernels; anti-four margins; why radius 5. |
| 06 | [The fork](06-the-fork.md) | The double-three map; the naive oracle; the three-level gate. The tutorial's point. |
| 07 | [Seeing it](07-seeing-it.md) | Threat potential as a sparse policy; the heatmap demo; same network on GPU. |
| 08 | [Acceptance](08-acceptance.md) | Crate polish and the full proof ritual, with measured numbers. |

## Status

Complete. The reference implementation (a disposable worktree during
development) passed the full acceptance ritual: 152 hand-written
channels; three differential gates at 1000 boards × 2 colors (8.9 s in
release); exact CPU-vs-GPU parity; engine still a Burn-free island.
Chapter 8 quotes the measured numbers.

## References

- [Design spec](../specs/2026-09-27-convolutions-tutorial-design.md) —
  the semantic contracts this tutorial implements.
- [Implementation plan](../plans/2026-09-27-convolutions-tutorial.md) —
  how the tutorial was built.
- [`docs/12-gomoku-architecture.md`](../12-gomoku-architecture.md) —
  alpha-epsilon philosophy (§3).
- The [mcts tutorial](../mcts-tutorial/README.md) and
  [datagen tutorial](../datagen-tutorial/README.md) — format
  precedents and neighbors in the milestone sequence.
