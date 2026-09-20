# Slice 6 — Deep dives

The slice ([../06-symmetry-and-encoding.md](../06-symmetry-and-encoding.md))
tells you *what* to build. Unlike slices 4–5 (theory in 01, plan in
02), this folder has a single paper that folds all three layers into
one: the *intention* (what this slice is for, top down from the
AlphaZero loop), the *theory* (D4 as a group — generators, relations,
orbits, fixed points, and the commuting diagram), and the *plan*
(red→green steps with the verified reference solution).

Series conventions (derive then measure; the chapter stays authoritative
for the API; one companion folder per chapter that needs one) live
in [03-bitboard-and-board/README.md](../03-bitboard-and-board/README.md).

| # | Paper | Answers |
|---|-------|---------|
| 01 | [Slice 6 implementation plan: intention, transforms, and the commuting diagram](01-implementation-plan.md) | Why encode+transforms live in the engine (store-games replay ⇒ 10k re-encodings per iteration); the intention "the network must never tell a position from its seven other forms"; the transform/symmetry terminology doctrine; D4's generators and relations, the orbit decomposition of the 225 cells (1 + 14·4 + 21·8 = 225, 36 orbits by Burnside), why reflections are self-inverse; why the commutation property is the actual deliverable; then the red→green build order and the complete verified `symmetry.rs` + `encode.rs`. |
