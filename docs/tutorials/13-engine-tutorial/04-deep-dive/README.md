# Slice 4 — Deep dives

The slice ([../04-win-detection.md](../04-win-detection.md)) tells you
*what* to build. These papers answer what the slice leaves open: paper 01
*why it is shaped that way*, paper 02 *in what order to build it*. Every
claim is either traced to the design or measured on a real run.

Series conventions (derive then measure; the chapter stays authoritative
for the API; one `NN-deep-dive/` folder per chapter that needs one) live
in [03-deep-dive/README.md](../03-deep-dive/README.md).

| # | Paper | Answers |
|---|-------|---------|
| 01 | [The staged AND, and why win detection needs no edge masks](01-staged-and-and-no-edge-masks.md) | Why two hops of 2 and 4 instead of one five-term AND — and why the naive form answers incorrectly for two of the four directions; why the padding invariant removes every per-direction edge mask (144 phantom fives without it, 0 with it); where a mask *is* required; what "≥ 5" costs if you ever count with it; which of the two call shapes belongs to `Board::play` and which to MCTS; and how to benchmark bit twiddling without measuring the optimizer. |
| 02 | [Slice 4 implementation plan, with reference solution](02-implementation-plan.md) | The build order as red→green steps: the test first, the smallest code that passes it, the gate that closes the step. Includes the integration hunks for `board.rs`/`bitboard.rs`, the optional neighbourhood check, the proptest global-reject pitfall, and the complete verified `win.rs`. |
