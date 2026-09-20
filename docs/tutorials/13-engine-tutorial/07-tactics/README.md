# Slice 7 — Deep dives

The slice ([../07-tactics.md](../07-tactics.md)) tells you *what* to
build. Like slice 6, this folder has a single paper that folds three
layers into one: the *intention* (why hand-woven tactics exist in an
AlphaZero project), the *background* (threat arithmetic — why counting
to two is the whole game at this depth, and where v1's truth ends),
and the *plan* (red→green steps with the verified reference solution).

Series conventions (derive then measure; the chapter stays authoritative
for the API; one companion folder per chapter that needs one) live
in [03-bitboard-and-board/README.md](../03-bitboard-and-board/README.md).

| # | Paper | Answers |
|---|-------|---------|
| 01 | [Slice 7 implementation plan: intention, threat logic, and the price of certainty](01-implementation-plan.md) | Why a perfect tactical prior matters on day one (PUCT's `P(s,a)` term is noise early); "the engine vouches for what is true, training decides what to do with it"; the side/`to_move` asymmetry; why immediate wins are excluded from `double_threats` (semantic partition + oracle comparability — an ambiguous contract sentence, resolved and documented); the counting argument behind unanswerability; the gapped-four lesson (`O . O . O . O` is a double threat waiting to happen); the threat ladder and the four-three boundary; then the red→green build order with measured traps (dirty complements, `prop_assume` global rejects at scale, wide-run costs) and the complete verified `tactics.rs` + `MoveSet` + naive oracle. |
