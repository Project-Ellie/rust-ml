# Chapter 10 — Papers and reading list

Annotated, in reading order. Links verified 2026-09-13.

## The AlphaGo line (core reading)

1. **AlphaGo** — Silver et al., *Mastering the game of Go with deep neural
   networks and tree search*, Nature 529, 2016.
   <https://doi.org/10.1038/nature16961>
   The original: supervised policy from human games, RL policy, value
   network, MCTS. There is no arXiv version of this paper; the DOI page is
   the primary source. You know this one from DeepGomoku — skim it for
   nostalgia, the next two matter more now.

2. **AlphaGo Zero** — Silver et al., *Mastering the game of Go without human
   knowledge*, Nature 550, 2017. <https://doi.org/10.1038/nature24270>
   The paper our project implements. No human data, one network, self-play
   bootstrap. Also no arXiv version. Read this one closely, twice.

3. **AlphaZero** — Silver et al., *A general reinforcement learning
   algorithm that masters chess, shogi, and Go through self-play*, Science
   362, 2018. Preprint: <https://arxiv.org/abs/1712.01815>
   The generalization: no game-specific knowledge beyond rules, no gating,
   no symmetries baked into training (they appear as augmentation only).
   Our Gomoku agent follows this variant.

4. **MuZero** — Schrittwieser et al., *Mastering Atari, Go, chess and shogi
   by planning with a learned model*, Nature 588, 2020.
   <https://arxiv.org/abs/1911.08265>
   Replaces the game simulator with a learned dynamics model. Read after we
   have a working AlphaZero baseline, not before. It is the natural "what
   if" for later.

## Efficiency and engineering reading

5. **KataGo** — David J. Wu, *Accelerating Self-Play Learning in Go*.
   <https://arxiv.org/abs/1902.10565>
   The most practical paper on this list: how to make AlphaZero-style
   training an order of magnitude cheaper (auxiliary targets, playout
   cap randomization, rules handling). Read this before we tune anything.

6. **ELF OpenGo** — Tian et al., *ELF OpenGo: An Analysis and Open
   Reimplementation of AlphaZero*. <https://arxiv.org/abs/1902.04522>
   An open reimplementation with honest engineering analysis: what
   hyperparameters matter, what distributed self-play costs, what goes
   wrong.

## Gomoku-specific

7. **AlphaGomoku** — Xie, Fu, Yu, *AlphaGomoku: An AlphaGo-based Gomoku
   Artificial Intelligence using Curriculum Learning*.
   <https://arxiv.org/abs/1809.10595>
   The closest published work to our project. AlphaGo-style MCTS for
   Gomoku with curriculum learning and Gomoku-specific adjustments. Direct
   competition for what we build.

8. **A Game Model for Gomoku Based on Deep Learning and Monte Carlo Tree
   Search** — ICIS 2019. <https://doi.org/10.1007/978-981-32-9050-1_10>
   A second DL+MCTS Gomoku reference for comparison.

9. **MCTS review** — Świechowski et al., *Monte Carlo Tree Search: A Review
   of Recent Modifications and Applications*.
   <https://arxiv.org/abs/2103.04931>
   The catalogue of MCTS variants (RAVE, progressive widening, transposition
   tables, parallelization). Consult when we consider deviating from the
   AlphaZero PUCT formula.

## About Burn itself

There is **no paper** that presents the Burn framework. Cite the versioned
repository and book instead:

- Repository: <https://github.com/tracel-ai/burn> (this course: tag
  [v0.21.0](https://github.com/tracel-ai/burn/tree/v0.21.0))
- Book: <https://burn.dev/books/burn/>
- CubeCL (the compute layer under the GPU backends):
  <https://github.com/tracel-ai/cubecl>

One caution: arXiv:2606.15991 (*Fearless Concurrency on the GPU*) is about
NVIDIA's cuTile Rust, **not** about Burn or CubeCL. Do not miscite it.

## Reference implementations worth reading

- **alpha-zero-general** (Surag Nair et al.) — the clean Python reference
  with a Gobang game implementation. You have a local copy at
  `../alpha-zero-general`. Its `Coach.py`/`MCTS.py` are ~200 lines and show
  the loop in its most readable form.
- **DeepGomoku** — Wolfie's own TensorFlow implementation:
  <https://github.com/Project-Ellie/DeepGomoku>. The design baseline we
  intend to beat, in clarity if not (yet) in strength.

Next: [Chapter 11 — Pitfalls](11-pitfalls.md)
