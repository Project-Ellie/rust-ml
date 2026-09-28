# Chapter 04 — The four level

## Abstract

This chapter adds the **four-level** threat maps: open-four makers,
broken-four makers, and the derived `double_threats` map. You will
learn why an open four contributes **two** five-completing cells while
a broken four contributes **one**, how a 1×1 convolution turns those
per-pattern counts into per-direction sums, and why the final map
needs a small piece of Rust "glue" rather than being a pure conv
output. The chapter's acceptance is a differential gate: the network's
`double_threats` map must agree with `engine::double_threats` on
seeded random boards. By the end you will have implemented the
`OpenFourMaker` and `BrokenFourMaker` families in `src/kernels.rs`,
the layer-2 1×1 combiner, and the `ThreatMaps` glue in `src/net.rs`.

## Glossary

| Term | Definition |
|------|------------|
| **Open four** | Four consecutive stones including the candidate cell `c`, with both ends empty. It has two five-completing cells. |
| **Broken four** | Four stones in a five-cell window including `c`, with exactly one gap; the gap is the unique five-completing cell. |
| **Five-completing cell** | An empty cell where the side would complete five by playing. |
| **Double threat** | An empty cell where playing leaves the side with two or more five-completing cells, excluding immediate wins. |
| **1×1 combiner** | A `Conv2d` with kernel size `1×1` that computes per-cell cross-channel weighted sums. |
| **Weight scale** | The layer-2 coefficient applied to a family's channels so the output carries a count rather than a bit. |
| **Glue** | The Rust code outside the conv layers that post-processes the raw count maps into the final engine-matching semantics. |
| **Before scalar** | `|immediate_wins(b, s)|`, added to the newly-created completing-cell count because the engine's `double_threats` counts all completing cells after the move. |
| **Unique-cell union** | Deduplication of completing cells that are produced by more than one firing pattern in the same direction. |
| **Win masking** | Setting `double_threats` to zero wherever the win map already fires, because immediate wins are excluded from double threats. |

## Context

Chapter 3 built the exact-pattern kernel language and the
`FiveCompleter` family. The win map — layer 2 output channel 0 —
marks every cell where playing completes five immediately. That is
the top of the threat ladder.

This chapter descends one rung. A move that creates an **open four**
leaves two winning cells; a move that creates **two** broken fours
leaves two distinct winning cells; a move that creates an open four
in one direction and a broken four in another also leaves two
winning cells. The engine calls all of these `double_threats`: moves
after which the side has at least two immediate wins.

The challenge is not detecting the local shapes — the kernel
language from chapter 3 already does that — but counting the
*correct* number of *unique* winning cells. A naïve sum of pattern
channels double-counts when two patterns share a completing cell,
and it ignores the winning cells that already existed before the
move. The network therefore exposes both the layer-1 activations and
the layer-2 count maps, and a small glue routine finishes the job.

## Intention

1. Extend `src/kernels.rs`:
   * Implement `OpenFourMaker` and `BrokenFourMaker` base shapes and
     their `PatternFamily` counts.
   * Verify that the four-level portion of the pattern table is
     correct: `OpenFourMaker` = 16 channels, `BrokenFourMaker` = 80
     channels.
2. Implement the layer-2 1×1 combiner in `src/kernels.rs`:
   * Output channel 0: OR of all `FiveCompleter` channels (win map).
   * Output channels 1–4: five-completing cells created per
     direction, with `OpenFourMaker` weighted ×2 and
     `BrokenFourMaker` weighted ×1.
3. Implement the `ThreatMaps` glue in `src/net.rs`:
   * Occupied-cell zeroing.
   * `before` scalar addition.
   * Unique-cell union across firing four-maker patterns per
     direction.
   * Win masking (immediate wins excluded from double threats).
4. Verify with the engine differential gate:
   * `double_threat_map == engine::double_threats`.
   * Win map re-verified against `engine::immediate_wins`.

Observable done-state: the chapter-4 tests listed below pass,
`cargo clippy --all-targets -- -D warnings` is green from `gomoku/`,
and `cargo fmt --all` makes no changes from `gomoku/`.

## Mental mapping

### Open four vs. broken four

An **open four** is four consecutive stones with both ends empty:

```text
_ X X X X _
```

Playing at either end creates five, so it contributes **two**
five-completing cells. A single open four is already a double
threat.

A **broken four** is four stones in a five-cell window with one gap:

```text
X X _ X X
```

Only the gap completes five, so it contributes **one** five-
completing cell. Two broken fours (in the same or different
directions) are needed for a double threat.

### Why does the engine's `double_threats` definition matter verbatim?

The tutorial's goal is to reproduce the engine's semantics exactly,
including its deliberate simplifications. The engine's module doc
states:

> Moves after which `side` has >= 2 immediate wins: open fours and
> double fours — unanswerable next move. Immediate wins themselves
> are EXCLUDED (simplification 3 above). See the module docs for the
> four-three blind spot.

The three documented v1 simplifications are:

1. **Four-three blind spot.** `double_threats` counts immediate wins
   after one move. A four-three has exactly ONE immediate win now
   (the three matures next ply), so it escapes — the full win-in-2
   search is out of scope for the engine milestone.
2. **Opponent-wins-first.** A "double threat" on a board where the
   OPPONENT has an immediate win is answerable — they win before it
   matures. v1 accepts this; self-play checks forced blocks first.
3. **Immediate wins are not double threats.** A move that completes
   five ENDS the game; calling it a "threat" would be a category
   error, so `double_threats` excludes it and the three sets
   partition cleanly: win now / must block / unblockable threat.

Your network must match this exact contract, not a theoretically
purer one.

### Why a 1×1 convolution for the combiner?

A 1×1 conv has a receptive field of exactly one cell. At each board
position it computes a weighted sum across the layer-1 channels for
that cell only. That is perfect for turning per-pattern bits into
per-direction counts: for each direction channel, sum the relevant
four-maker channels, scaling open fours by 2 and broken fours by 1.
The operation is identical at every spatial position, so a conv
layer expresses it without hand-written loops.

### Why channels carry counts, not just bits?

If layer 2 only ORed the four-maker channels, an open four (two
winning cells) and a broken four (one winning cell) would both
produce `1`. The threshold `>= 2` could not tell that a single open
four is already a double threat. By weighting the open-four channel
×2, the layer-2 output carries the **number of five-completing
cells** created in that direction. ReLU keeps the count non-negative;
because every count is a small non-negative integer, ReLU does not
change the value.

### Why does the glue exist?

The conv layers are spatially uniform: they apply the same weights
everywhere. But the final `double_threats` map needs three pieces of
board-dependent information that a conv cannot know:

1. **Occupied cells.** Threat maps are only defined for empty cells;
   occupied cells must be zeroed.
2. **The `before` scalar.** The engine counts all completing cells
   after the hypothetical move. Some of those cells were already
   completing before the move (`immediate_wins(b, s)`). Because a
   move can only add completing cells, never remove them, the total
   after the move is `before + newly_created`.
3. **Unique-cell union.** Two different four-maker patterns can
   produce the same completing cell. The engine counts it once
   (its `winning_cells` returns a `MoveSet`). A naïve sum of pattern
   channels would count it twice.
4. **Win masking.** Immediate wins are excluded from double threats.

The cautionary tale is the **degenerate shared-completing-cell**
case discovered during the Task-2 spike: a single empty cell can
complete five via two different windows in the same direction, for
example two broken fours sharing the same gap. A per-pattern sum
counts that gap twice and reports a double threat where the engine
reports only one completing cell. The glue resolves this by tracking
a `seen` bitset of completing-cell offsets per direction and counting
each offset once.

## Low-level design

### Files you will create or modify

```text
gomoku/crates/patterns/
└── src/
    ├── kernels.rs   (add OpenFourMaker, BrokenFourMaker, layer2_conv)
    └── net.rs       (ThreatMaps, analyze/analyze_both, glue)
```

### The four families' shapes

#### `OpenFourMaker`

A horizontal base shape places `c` at offset `k` in a 4-cell segment
`[-k, -k+1, -k+2, -k+3]`:

* `stone_offsets`: the three cells in the segment other than `c`.
* `empty_offsets`: the two cells immediately outside the segment,
  `(-k-1)` and `(-k+4)`. These act as the anti-five margin: if either
  held a stone, the shape would not be an open four.
* `completing_offsets`: the two empty end cells.

There are 4 base shapes, giving `4 × 4 = 16` layer-1 channels.

#### `BrokenFourMaker`

A horizontal base shape places `c` at offset `k` in a 5-cell window
`[-k, -k+1, -k+2, -k+3, -k+4]`:

* One of the other four positions is the gap (empty).
* `stone_offsets`: the three non-gap, non-`c` cells.
* `empty_offsets`: the gap cell.
* `completing_offsets`: the gap cell.
* No margin checks: the five-window itself is the entire shape.

There are `5 × 4 = 20` base shapes (`c` position × gap position,
gap ≠ `c`), giving `20 × 4 = 80` layer-1 channels.

### Layer-2 structure

Layer 2 is a 1×1 conv from `LAYER1_OUT` (= 152) channels to
`LAYER2_OUT` (= 9) channels:

| Output channel | Meaning |
|---------------|---------|
| `0` | Win map: OR of all `FiveCompleter` channels. |
| `1` | Five-completing cells created in the horizontal direction. |
| `2` | Five-completing cells created in the vertical direction. |
| `3` | Five-completing cells created in the diagonal-down direction. |
| `4` | Five-completing cells created in the diagonal-up direction. |
| `5` | Open-three makers in the horizontal direction. |
| `6` | Open-three makers in the vertical direction. |
| `7` | Open-three makers in the diagonal-down direction. |
| `8` | Open-three makers in the diagonal-up direction. |

The four-created channels weight `OpenFourMaker` by 2.0 and
`BrokenFourMaker` by 1.0, so each channel carries a count of
five-completing cells. The three-maker channels are placeholders for
chapters 5–6.

### `ThreatMaps`

```rust
pub struct ThreatMaps {
    pub wins: [[f32; 15]; 15],
    pub double_threats: [[f32; 15]; 15],
    pub double_threes: [[f32; 15]; 15],
    pub threes_per_dir: [[[f32; 15]; 15]; 4],
    pub fours_created_per_dir: [[[f32; 15]; 15]; 4],
}
```

All maps are restricted to the 15×15 in-board region. Occupied cells
are zeroed by the glue.

### `analyze` and `analyze_both`

```rust
pub fn analyze<B: Backend>(
    net: &ThreatNet<B>,
    b: &Board,
    s: Color,
    device: &B::Device,
) -> ThreatMaps;

pub fn analyze_both<B: Backend>(
    net: &ThreatNet<B>,
    b: &Board,
    device: &B::Device,
) -> [ThreatMaps; 2];
```

`analyze` evaluates one color. `analyze_both` stacks black's and
white's planes as a batch of two and runs one forward pass, then
splits the outputs. Both use `forward_intermediate` so the glue has
access to the layer-1 activations.

### The glue contract (pseudo-code)

For each empty cell `(r, c)`:

```text
before = |immediate_wins(b, s)|
if win_map[r][c] > 0:
    double_threats[r][c] = 0          # simplification 3
else:
    created = 0
    for each direction d:
        seen = empty bitset along d
        for each layer-1 channel ch of family OpenFourMaker or BrokenFourMaker
              with direction d:
            if layer1_activation[ch][r][c] > 0:
                for each completing offset (dy, dx) of ch:
                    c2 = (r + dy, c + dx)
                    if c2 is in-board and win_map[c2] == 0:
                        mark offset of c2 along d as seen
        created += number of seen offsets
    if created + before >= 2:
        double_threats[r][c] = 1
```

The win-map check on `c2` filters out completing cells that were
already winning before the move; those are counted in `before`. The
`seen` bitset deduplicates shared completing cells within a
direction.

## Solution (opt-in)

The complete reference `src/net.rs` for the finished crate lives in
[04-the-four-level/01-solution.md](04-the-four-level/01-solution.md).
Open it only if you have been stuck for more than twenty minutes, or
after you have finished the chapter and want to compare. The file is
quoted in its final form; the three-maker channels are still
placeholders here and land in chapters 5–6.

## TDD checklist

Run these tests from `gomoku/` with `cargo test -p patterns`.

The reference suite contains exactly these chapter-4 tests:

From `net::tests`:

1. `classic_fork_double_threat`
2. `open_four_maker_is_double_threat`
3. `two_broken_fours_make_double_threat`

From `tests/differential.rs` (default suite):

1. `wins_default`
2. `double_threats_default`
3. `double_threes_default`

From `tests/differential.rs` (full proof suite, run with
`cargo test -p patterns -- --ignored`):

1. `wins_full`
2. `double_threats_full`
3. `double_threes_full`

The win-map tests from chapter 3
(`open_four_win_map_matches_engine` and
`broken_four_win_map_matches_engine`) continue to pass and are part
of the default gate.

Follow the red-green-refactor rhythm:

1. **Red:** Add the `OpenFourMaker` and `BrokenFourMaker` families
   to `src/kernels.rs` with `todo!()` bodies or empty shape lists,
   and add `layer2_conv` as `todo!()`. Add the `ThreatMaps` struct,
   `analyze`, `analyze_both`, and the glue skeleton in `src/net.rs`.
   Add all the tests above. Run `cargo test -p patterns`. Expect
   failures.
2. **Green:** Implement the four-maker base shapes, the layer-2 1×1
   combiner, and the glue. Run `cargo test -p patterns`. The default
   tests should pass.
3. **Full proof:** Run `cargo test -p patterns --release -- --ignored`
   to exercise the 1000-board gates. This is the chapter's
   acceptance ritual; it should finish in a few minutes in release.
4. **Refactor:** Run `cargo clippy --all-targets -- -D warnings` and
   `cargo fmt --all -- --check` from `gomoku/`. Fix any warnings or
   formatting issues.

### Gate scheduling

The default `cargo test -p patterns` runs the differential gates at
100 boards per test. On a typical development machine this finishes
in roughly a minute and stays fast enough for tight iteration. The
full 1000-board proof gates are marked `#[ignore]` and run explicitly
with `cargo test -p patterns -- --ignored` (or `--release` for
speed). This split keeps the everyday gate responsive while
preserving a stronger proof that can be run before committing or in
CI.

## Done when

* `cargo test -p patterns` passes from `gomoku/` (including the
  default 100-board differential gates).
* `cargo test -p patterns -- --ignored` passes in release as the full
  proof.
* `cargo clippy --all-targets -- -D warnings` passes from `gomoku/`.
* `cargo fmt --all -- --check` makes no changes.
* Commit message for the code slice:
  `feat(patterns): four-level maps + engine differential gate`.

Next: [Chapter 05 — Three-makers](05-three-makers.md)

## References

* [`03-the-kernel-language.md`](03-the-kernel-language.md) — the
  previous chapter: exact-pattern kernels and the `FiveCompleter`
  win map.
* [`docs/specs/2026-09-27-convolutions-tutorial-design.md`](../../specs/2026-09-27-convolutions-tutorial-design.md)
  — §3.2 (four-level semantics) and §4.3 (1×1 combiner, channels
  carry counts).
* `gomoku/crates/engine/src/tactics.rs` — the engine's
  `double_threats` definition and its three documented v1
  simplifications.
* `gomoku/crates/patterns/SPIKE.md` — the Task-2 spike, including
  the degenerate shared-completing-cell case and the `created +
  before` decomposition.
