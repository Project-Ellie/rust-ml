# Chapter 08 — Acceptance: crate polish + the proof ritual

## Abstract

This is the final chapter of the convolutions tutorial. It locks the
crate's public surface with `#![deny(missing_docs)]` and defines the
acceptance ritual that decides whether the hand-written threat-map
network is done. By the end you will have run the full proof — the
1000-board differential gates, on both backends — and you will have
seen the measured numbers quoted below (all from the verified
reference implementation, never invented).

## Glossary

| Term | Definition |
|------|------------|
| **Crate polish** | Adding `#![deny(missing_docs)]` so every public item must carry a doc comment. |
| **Default suite** | `cargo test -p patterns`: unit tests, ASCII tests, and the 100-board differential gates. About one minute in debug. |
| **Full proof** | The 1000-board differential gates, `#[ignore]`d by default and run explicitly with `-- --ignored`. |
| **Differential gate** | A test that requires the network's maps to equal an oracle's answers exactly, on every board: the engine for wins and double threats, the naive oracle for double threes. |
| **GPU port test** | The `tests/gpu_port.rs` suite asserting exact CPU-vs-GPU equality, behind `--features gpu`. |

## Context

Chapters 1–7 built the complete crate: planes, a hand-rolled
cross-correlation, Burn's `conv2d` with hand-assigned weights, the
exact-pattern kernel language, the four-level maps gated against the
engine, the three-level maps and the fork gated against the naive
oracle, and the heatmap demo with the policy-shaped potential output.

What remains is not new code. It is the ritual that turns "the tests
pass" into a statement you can defend: the network reproduces the
engine's tactics and the oracle's three-detection *exactly*, at scale,
on two backends.

## The acceptance ritual

Acceptance is a sequence of checks that either pass or fail:

1. **Default suite.** `cargo test -p patterns` passes from `gomoku/`.
   This is the per-chapter gate you have run all along: every unit and
   ASCII test plus the 100-board differential gates.
2. **Full proof.** `cargo test -p patterns --release -- --ignored`
   runs the three 1000-board gates (wins vs `immediate_wins`, double
   threats vs `double_threats`, double threes vs the oracle), both
   colors per board. Run it in release mode: the proof is exhaustive,
   not slow.
3. **GPU parity.** `cargo test -p patterns --features gpu` passes,
   including the exact CPU-vs-Wgpu equality tests in
   `tests/gpu_port.rs`. (Environment-dependent: it needs a working GPU
   adapter. It is part of the ritual on machines that have one, and
   documented as optional on machines that do not.)
4. **Workspace hygiene.** `cargo clippy --all-targets -- -D warnings`
   and `cargo fmt --all -- --check` are clean; `cargo test --workspace`
   stays green; `cargo check -p engine` confirms the engine is still a
   Burn-free dependency island.
5. **Demo sanity.** `patterndemo` renders the four showcases, and the
   `--gpu` output is identical to the CPU output modulo the backend
   line.

### Measured numbers (reference implementation)

These are the numbers from the verified reference implementation, not
targets you must reproduce exactly — your hardware and build profile
will shift them. The shapes must match.

| Check | Result |
|-------|--------|
| Default suite (debug) | **66 s**, all tests pass (35 lib + 3 bin + 7 differential + 20 oracle + 4 spike + 15 three-maps) |
| Full proof, 3 × 1000 boards × 2 colors (**release**) | **8.9 s** — all three gates pass |
| Full proof (debug, measured at delivery) | ~620 s — this is why the proof is a release-mode ritual |
| GPU default suite (`--features gpu`) | all pass, incl. 4 `gpu_port` exact-equality tests |
| Demo CPU vs GPU | byte-identical modulo the `Backend:` line |
| Engine isolation | `cargo check -p engine` green — no Burn in sight |

The 70× debug-to-release gap is itself a lesson: the differential
proof's cost lives in the 152-channel 11×11 forward pass, and
unoptimized convolution is genuinely slow. Release mode is not a
luxury for the full proof; it is the point of the `--ignored`
scheduling decision from chapter 4.

## Why the gates are scheduled this way

The per-chapter gate must stay under a minute or the tutorial's
red-green loop dies. The acceptance claim needs a thousand boards, not
a hundred. The resolution you implemented in chapter 4 is the
compromise this tutorial recommends generally: *fast gates by default,
exhaustive proofs one flag away, and the proof habitually run in
release mode.* The default 100-board suite shares its seed range with
the full proof (seeds `0..100` of `0..1000`), so the fast gate is a
strict prefix of the slow one.

## Crate polish

Add `#![deny(missing_docs)]` to `src/lib.rs`, directly after the
module documentation. On the reference implementation this compiles
without a single new doc comment — the crate was documented as it
grew, which is exactly the habit the attribute enforces retroactively
on less disciplined code. If your crate produces errors here, write
the missing docs now; they are part of the public contract the next
tutorial will read.

## What this was for

Looking back: you now hold a convolutional network you understand
*completely* — every weight written by hand, every channel a named
pattern, every map proven equal to an independent oracle at scale.
Convolution stopped being machinery that produces numbers and became
what it is: parallel local pattern detection, with exact Boolean
semantics when you want them.

Looking forward — motivation, not promises:

* The net tutorial's first conv layer will **learn** kernels where you
  hand-wrote them. You now know what such kernels can express, and you
  will recognize the shapes when they emerge.
* A frozen hand-written first layer under a learned network (the
  DeepGomoku pattern) is an option you can now evaluate concretely.
* `potential.rs` already speaks the `Vec<(Move, f32)>` shape of the
  datagen tutorial's `Sample::policy` and the mcts crate's evaluator
  seam. An adapter is a thin wrapper, if you ever want a hand-written
  prior inside search.

## TDD checklist

This chapter adds no new tests; it runs the complete inventory. As
before, all previous chapters' tests keep passing.

1. `cargo test -p patterns` — default suite green.
2. `cargo test -p patterns --release -- --ignored` — the three full
   proofs green (`wins_full`, `double_threats_full`,
   `double_threes_full`).
3. `cargo test -p patterns --features gpu` — GPU parity green
   (`classic_fork_cpu_gpu_equal`, `corner_fork_cpu_gpu_equal`,
   `double_three_fork_cpu_gpu_equal`, `quiet_position_cpu_gpu_equal`).
4. `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` clean from `gomoku/`.
5. `cargo test --workspace` green; `cargo check -p engine` green.

## Done when

* The five ritual checks above all pass, from `gomoku/`.
* `#![deny(missing_docs)]` is in `src/lib.rs` and the crate compiles.
* Commit message: `feat(patterns): crate polish (deny missing_docs)`.

## References

* [`docs/specs/2026-09-27-convolutions-tutorial-design.md`](../../specs/2026-09-27-convolutions-tutorial-design.md)
  — the design authority (map semantics, kernel language, oracle
  ladder, acceptance bar).
* [`docs/plans/2026-09-27-convolutions-tutorial.md`](../../plans/2026-09-27-convolutions-tutorial.md)
  — the implementation plan, including the gate-scheduling amendment.
* `gomoku/crates/engine/src/tactics.rs` — the four-level oracle and
  its documented simplifications.
