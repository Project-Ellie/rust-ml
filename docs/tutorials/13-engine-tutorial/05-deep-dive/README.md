# Slice 5 — Deep dives

The slice ([../05-zobrist.md](../05-zobrist.md)) tells you *what* to
build. These papers answer what it leaves open: what a Zobrist key even
*is*, what problem from the 1970s it solves, and exactly where in our
system the concept earns its keep — including the one famous use we
deliberately reject.

Series conventions (derive then measure; the chapter stays authoritative
for the API; one `NN-deep-dive/` folder per chapter that needs one) live
in [03-deep-dive/README.md](../03-deep-dive/README.md).

| # | Paper | Answers |
|---|-------|---------|
| 01 | [What Zobrist hashing is good for](01-what-zobrist-hashing-is-good-for.md) | The position-identity problem and its naive answers; the XOR trick and why undo is free; why different move orders to the same position produce the same key; honest collision math at our scale; the four places the design uses keys (replay dedup, test identity, debug assertions, from-scratch validation); why the table is a compile-time `const fn`; and transposition tables — the classic use AlphaZero-style MCTS deliberately shelves. |
