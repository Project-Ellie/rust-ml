# Epilogue to Chapter 03 — the picture so far

This is not a chapter. There are no tasks here. This is the complete
description of what now exists in `gomoku/crates/patterns`, why it
exists, and how its parts fit together — the mental picture to carry
into chapter 4. Read it once, slowly; then open chapter 4.

## Why this crate exists

The project philosophy is **alpha-epsilon**: hand-woven tactics give
the agent a head start that learning later refines. The `engine` crate
already computes tactical truth as bitboard operations —
`immediate_wins`, `double_threats`, and friends. It is fast, exact,
and opaque: a pile of bit-twiddling that answers questions without
explaining anything.

The `patterns` crate computes *the same truths* as a small
convolutional network whose every weight was written by a human being
— by you. No training, no autodiff, no loss function, no randomness.
The point is twofold:

1. **Pedagogical.** The net tutorial will soon ask you to juggle
   training loops, loss functions, data loading, and Burn's module
   system all at once. Here you met the same machinery — tensors,
   `Conv2d`, NCHW layout, devices, forward passes — in a setting where
   every number is deterministic, every weight has a name, and every
   wrong answer is debuggable. When training enters, the machinery
   will already be boring.
2. **Architectural.** A hand-written detector is a proof of
   expressiveness: it demonstrates that a convolutional layer *can*
   represent exact tactical conditions, before we ask gradient descent
   to *find* them. The trained network later will learn patterns of
   the same shape, in the same layout, on the same planes.

The crate is a side quest: it adds no game capability the engine
lacks. Its value is that every channel is *provably* correct — and
the proof mechanism is one of the load-bearing ideas (see
"Differential testing" below).

## The machine at a glance

```text
   ASCII board / engine Board
            │
            ▼
   planes(board, side)                    ch.1 — planes.rs
   two 25×25 planes:
     ch 0 = own stones, ch 1 = blocked    (15×15 board + 5-cell border
            │                              treated as blocked)
            ▼
   layer1: Conv2d 11×11, 2 → 152          ch.3 — kernel.rs
   every pattern kernel, evaluated at     Valid padding: 25 − 11 + 1 = 15,
   every board cell simultaneously        so output is exactly the board
            │
            ▼  ReLU      (each channel is now a clean 0/1 pattern-presence map)
            │
   layer2: Conv2d 1×1, 152 → 9            ch.3 — kernel.rs
   cross-channel combiner: no spatial     OR-combine, sum, and scale the
   extent, pure channel arithmetic        152 pattern maps into 9 meaning maps
            │
            ▼  ReLU
            │
   glue (net.rs)                          mask occupied cells; later:
            │                             count unique completing cells
            ▼
   threat maps — 15×15 grids with game meaning
   (today: the win map; ch.4 adds the four-level maps)
```

Two structural decisions to notice, because everything follows from
them:

* **The border is data, not logic.** The planes are 25×25: the 15×15
  board plus a five-cell frame of "blocked". With an 11×11 kernel and
  valid padding, every one of the 225 board cells — corners included —
  gets a fully-centred evaluation, and off-board cells are handled by
  the same weights as opponent stones. There is no boundary code
  anywhere in the network.
* **The layers have different jobs.** Layer 1 answers *spatial*
  questions ("where does pattern P occur?"). Layer 2 answers
  *cross-channel* questions ("does any five-completer occur here?",
  "how many four-makers fire in the vertical direction?"). A 1×1
  convolution is just per-cell channel arithmetic — but writing it as
  a conv keeps the whole network inside Burn's module system: one
  `forward`, no special cases.

## The kernel language, in one image

Every layer-1 kernel is a sparse picture of a Boolean condition. The
vocabulary has exactly four symbols:

| Weight | Where | Meaning |
|--------|-------|---------|
| `+1` | required own-stone cell, channel 0 | "this must be our stone" |
| `−1` | required-empty cell, **both** channels | "nothing may be here — either colour, or border" |
| `−1` | anti-margin cell, channel 0 only | "not *our* stone (opponent or empty is fine)" |
| `0` | everywhere else | "don't care" |

plus the bias, `1 − n_stones`, and ReLU. A perfect match scores
exactly `+1`; every violated condition subtracts exactly `1`; ReLU
keeps only the perfect matches. The margin of 1 is sufficient because
all weights are small integers and every f32 sum is exact — on every
backend, GPU included. That is why `cpu_vs_wgpu_exact` in chapter 2
was not a luxury: the whole design stands on exact integer arithmetic
in floating-point clothing.

A trained network learns dense, unreadable kernels. These kernels are
the opposite: mostly zeros, and every nonzero weight placed by hand
for a reason you can state out loud.

## The pattern table is the artifact

The 152 kernels are never drawn as 11×11 matrices. They are
*generated* from a declarative table — `pattern_table()` in
`kernel.rs` — where each entry says, in offsets relative to the
candidate cell `c`: which cells must hold own stones, which must be
empty, which are anti-margins, and which cells the pattern completes.

The table is the thing you review. The weight matrices are its
compiled output.

```text
6 families × base shapes × 4 directions = 152 layer-1 channels

FiveCompleter      5 shapes × 4 = 20   cXXXX XcXXX XXcXX XXXcX XXXXc
OpenFourMaker      4 shapes × 4 = 16   _cXXX_ _XcXX_ _XXcX_ _XXXc_
BrokenFourMaker   20 shapes × 4 = 80   c + one gap in a 5-window
ThreeXxx           3 shapes × 4 = 12   _cXX_ _XcX_ _XXc_      (_XXX_)
ThreeXxX           3 shapes × 4 = 12   _c_XX_ _X_cX_ _X_Xc_   (_X_XX_)
ThreeXxXRev        3 shapes × 4 = 12   _cX_X_ _Xc_X_ _XX_c_   (_XX_X_)
```

Remember: windows are drawn **after** the placement being evaluated.
`c` is the empty cell being scored; the kernel puts no weight on `c`
itself — it verifies the *other* cells. Channel order is table order:
all FiveCompleter shapes per direction first (Horizontal, Vertical,
DiagDown, DiagUp), then OpenFourMaker, and so on. That order is what
makes layer 2's channel arithmetic possible: layer 2 knows that
channels 5..=9 are the vertical FiveCompleters, because the table put
them there.

## The win map, end to end

Chapter 3's worked example traced one vertical FiveCompleter kernel
from offsets to a lit cell. The full pipeline adds exactly two steps:

1. **Layer 2 ORs all 20 FiveCompleter channels** (weight 1.0 each)
   into output channel 0, followed by ReLU. Positive = at least one
   five-completing window touches this cell, regardless of direction
   or of where `c` sits in the window.
2. **The glue masks occupied cells.** The kernels deliberately ignore
   what sits at `c`, so the raw map lights up under existing stones
   too (a five-window of own stones fires all five of its FiveCompleter
   shapes). That is not a bug; it is the price of the clean kernel
   language, paid in one line of glue: zero every cell where
   `board.stone_at(mv).is_some()`. `engine::immediate_wins` only ever
   returns empty cells, and after masking the two agree exactly.

`net.rs` wraps this in `ThreatNet`
(`layer1 → relu1 → layer2 → relu2`) with `win_map(board, side, device)`
returning the masked 15×15 grid. The same `ThreatNet`, unchanged,
will serve chapters 4–6: its output channels 1..=8 already carry the
per-direction four-maker and three-maker sums.

## Differential testing is the load-bearing idea

Every claim this crate makes is verified the same way: run the
network and the engine over the same position and require **exact set
equality** on all 225 cells — not "the expected cells are present",
but "present exactly where the engine says, absent everywhere else".

The two chapter-3 tests are the template. `open_four_win_map_matches_engine`
puts `_XXXX_` on the board and demands the win map equal
`engine::immediate_wins` — two cells lit, 223 dark.
`broken_four_win_map_matches_engine` does the same for `XXX_X` (one
cell lit), and parks a white stone directly beside the window: an
occupied neighbour must leave the detector unmoved.

This is what "provably correct" means here. Not a proof in the
mathematical sense — a proof by exhaustive comparison against an
independent implementation the project already trusts. And it
compounds: chapter 4's double-threat maps will be tested against
`engine::double_threats` the same way, so when chapter 6 stacks a
double-three fork on top, every layer beneath it is already pinned to
the oracle. You are not hoping the tower is straight; each floor was
levelled before the next was poured.

## What chapter 4 will ask of you

The conv machinery is *done* — the table is complete, both layers are
built, and the per-direction sums already flow through channels 1..=8.
What remains is **interpretation**: turning channel values into game
meaning. Concretely, chapter 4 introduces:

* **The four level.** Channels 1..=4 count, per direction, the
  five-completing cells a move creates: an open four contributes **2**
  (both ends — hence `weight_scale() == 2.0`), a broken four
  contributes **1** (its gap).
* **The double threat.** A move that leaves you with two or more
  five-completing cells is unanswerable — the engine calls these
  `double_threats`. Summing per-direction counts *almost* computes
  this, but one completing cell can be produced by two different
  windows, and the engine counts it once. That is why
  `forward_intermediate` already exists: it exposes the layer-1
  activations, so the glue can take the **union** of completing cells
  across firing patterns instead of the sum. You will meet the exact
  case where the sum lies.
* **`ThreatMaps` and `analyze`.** A struct of named 15×15 maps
  (`wins`, `double_threats`, `double_threes`, per-direction details)
  and the function that produces them from a `Board` — the crate's
  public face, and the input to the visualiser in chapter 7.

The shape of the work changes here: less "how do I manufacture a
detector", more "what is the exact semantics of a threat, and how do
I honour it in glue code without lying". The detectors are already
honest; the glue must be too.

## The crate as it stands

```text
gomoku/crates/patterns/src/
├── planes.rs   ch.1 — the padded 2×25×25 planes from a Board
├── naive.rs    ch.1 — the hand-rolled cross-correlation reference
├── tools.rs    ch.2 — DetectorBuilder, ASCII↔tensor helpers, the SPIKE idiom
├── kernel.rs   ch.3 — the pattern table and both conv layers (final form)
└── net.rs      ch.3 — ThreatNet, win_map (grows ThreatMaps in ch.4)
```

Everything above the line `net.rs` is finished infrastructure. The
next three chapters add meaning (ch.4), the third level (ch.5), and
the fork (ch.6) — on top of a machine you can already run, on CPU or
GPU, with every weight accounted for.

Next: [Chapter 04 — The four level](04-the-four-level.md)
