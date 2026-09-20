# Slice 3 — Deep dives

The chapter (`../03-bitboard-and-board.md`) tells you *what* to build and
in what order. These papers answer *why it is shaped that way* — the
reasoning behind a signature or a primitive, derived from what it is for,
with every claim either traced to the design or measured on a real run.

Read them when a slice's "contract" raises a question like *"why this
type and not the obvious one?"* — that is the question they exist to
answer. They are not required to pass the slice; they are what turns
"it compiles and the tests are green" into "I know why none of the
alternatives would have been better".

| # | Paper | Answers |
|---|-------|---------|
| 01 | [The stride-16 layout and the `shr` primitive](01-stride16-and-shr.md) | Why a 256-bit right shift exists at all, why 15 rows live in a stride of 16, why one padding bit per row makes wrap-around fives impossible, and what the `0 < s < 64` contract protects. |
| 02 | [`empty_moves()`: the signature, the loop, and `+ '_`](02-empty-moves.md) | Why the return type is a lazy `impl Iterator` and not a `MoveSet`/`Vec`, why the loop is over *results* rather than cells, why it is not a performance problem (measured), and what `+ '_` means now that Rust 2024 captures lifetimes implicitly. |

## Conventions for this series

- **Terminology is fixed.** Concept names come from the
  [tutorial glossary](../README.md#glossary) — papers do not coin their
  own vocabulary.
- **Derive, then measure.** Each paper starts from the consumer's
  question, builds the design one decision at a time, and then checks the
  claims against a measurement or a compiler experiment. Numbers are
  labelled as illustrative when they come from a stand-in benchmark
  rather than from the engine.
- **The chapter stays authoritative for the API.** If a paper and a
  chapter ever disagree, the chapter is the code you write; the
  disagreement is a bug in one of them — fix it and say so (that is what
  paper 02 did to `13-engine-design.md`).
- **One folder per chapter that needs one.** Any chapter can grow a
  companion folder named after it; papers are numbered in reading order. The
  sibling for slice 4 is [04-win-detection](../04-win-detection/README.md) —
  win detection, where the same padding invariant pays for itself again.
