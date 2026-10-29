# Chapter 06 redesign — overlays on the study table

Status: the chapter-level documents are on the `reference/chapter-06-redesign`
branch. The code is the learner's work, sub-chapter by sub-chapter. This
paper is the design authority for the four sub-chapters.

Decided (2026-10, with the owner):

- Folder layout: one subfolder per sub-chapter, named `a-value-strip`,
  `b-visits-overlay`, `c-prior-overlay`, and `d-tss-overlay`. Each
  subfolder contains the tutorial, the design paper, the
  implementation notes, and the opt-in solution file. The sub-chapter
  name appears in every file name.
- The old chapter-level `01-solution.md` file is removed. Its content
  is split across the four sub-chapter solution files.
- All documents follow ASD-STE100: short sentences, one meaning per
  word, active voice, no figurative language.

## Why the restructure

The reference build implemented the overlays in one chapter. That
chapter mixed four concerns: the value strip, the visits heatmap, the
prior heatmap, and the TSS overlay. Each concern teaches a different
idea, and the heatmaps in particular share one subtle conceptual core:
the difference between prior belief and visited belief. The redesign
separates the four concerns so each sub-chapter has one lesson and one
verifiable artifact.

## Data flow

```text
cursor node
    │
    ▼
SessionCore::search_info() ──► Option<&SearchInfo>
                                    │
        ┌───────────────────────────┼───────────────────────────┐
        ▼                           ▼                           ▼
  value_strip()              overlay_cell()                 TssOverlay
  (panel)                    (cell heatmap)                 (cell numbers)
        │                           │                           │
        └───────────────────────────┼───────────────────────────┘
                                    ▼
                            StudyView
                                    │
                                    ▼
                            render.rs
                                    │
                                    ▼
                            Screen::draw
```

`SessionCore` owns the data. `overlay.rs` turns the data into formatted
lines or cells. `ShellState` holds the active toggles. `StudyView`
carries both the data and the toggles into `render.rs`. `input.rs`
translates `p`, `v`, and `t` into `InputOutcome` values; these outcomes
are shell concerns, not commands.

## The four concerns, separated

| Concern | Location after the redesign | Sub-chapter |
|---|---|---|
| Value strip (panel overlay) | `src/tui/overlay.rs::value_strip`, `src/tui/render.rs` side panel | a |
| Visits heatmap (cell overlay) | `src/tui/overlay.rs::overlay_cell` visits path, `src/tui/render.rs` board cell | b |
| Prior heatmap (cell overlay) | `src/tui/overlay.rs::overlay_cell` prior path | c |
| TSS overlay + mutual exclusion | `src/tui/overlay.rs::TssOverlay`, `src/tui/study.rs` shell rules | d |

Dependency rule: `overlay.rs` depends only on `session.rs` types and
`engine.rs` types. `render.rs` depends on `overlay.rs`. `study.rs`
depends on `render.rs`. Crossterm and the command grammar do not know
that overlays exist.

## Two flavors of belief

`SearchInfo` carries two policies:

| Field | Meaning | Normalization | Hot label |
|-------|---------|---------------|-----------|
| `visits` | Visit counts after `simulations` PUCT descents. | Share of total visits. | Raw visit count. |
| `prior` | Evaluator probabilities after softmax and Dirichlet noise. | Ratio to the maximum prior. | Raw percentage. |

The visits overlay answers the question "where did the search spend its
time?". The prior overlay answers the question "what did the evaluator
believe before the search started?". They read the same search but show
different things.

The normalization difference matters. Visits are a distribution over
the root's legal moves. They sum to the simulation budget, so the
denominator is the total. A hot cell is a move that received at least
10% of the budget. Prior probabilities also sum to one, but the prior
overlay normalizes to the maximum so the brightest cell is always the
evaluator's favourite. The label is still the raw percentage, so a cell
at half the maximum intensity may read "25%" if the maximum prior was
0.5.

The flat-prior phenomenon is real. With the noisy opening schedule the
early-game evaluator has no strong opinion, so many cells share a small
prior mass. Normalize-by-max turns those cells hot with "0%" labels.
The prior wall of "0%" is honest: the evaluator has no idea. The
visits overlay in the same position is also often dim, because early
searches with a weak prior spread visits thin and no cell crosses the
10% threshold. Both readings are correct for a weak evaluator. The
trained network in later milestones is what will make these views
sharp.

## Why the searching side is the opposite of to_move

`SearchInfo` is stored on the child node that the AI's move created.
The parent node asked for a move; the child node remembers the search
that produced it. If the cursor is on that child, then:

* `core.board().to_move()` is the side whose turn it is now.
* The side that ran the search is the one who just moved.

The side that just moved is `core.board().to_move().other()`. The new
`SearchInfo::side_to_move` field carries this value explicitly so the
value strip can label the value from the correct perspective, without
recomputing the derivation at every render site.

```text
parent node          child node (cursor)
Black to move   ──►  White to move
search ran here      SearchInfo stored here
                     side_to_move = Black
```

## Overlay state is a shell concern

The playable CLI design spec §9 says that overlay state is visual and
belongs to the shell. `PolicyOverlay`, `show_value_strip`, and
`tss_proof` live in `ShellState`. They do not live in `SessionCore`
and they are not part of the command grammar. The reason is that a
future REST or GUI front-end will want its own overlay state, and the
session core should not carry visual toggles for one front-end.

```text
ShellState
├── input_state: InputState
├── show_value_strip: bool
├── policy_overlay: PolicyOverlay
└── tss_proof: Option<Proof>
```

## Mutual exclusion versus composition

The shell has three visual toggles:

| Toggle | Cell effect | Panel effect |
|--------|-------------|--------------|
| `p` | cycles policy heatmap | none |
| `t` | toggles TSS numbered line | none |
| `v` | none | toggles value strip |

`p` and `t` clear each other because they both paint cells. The shell
enforces this in both directions: the `CyclePolicy` arm clears
`tss_proof` when the new policy mode is not `Off`, and `toggle_tss`
sets `policy_overlay = PolicyOverlay::Off` when a proof is stored.

`v` clears neither because it only changes the side panel. The value
strip composes with the board, the policy heatmap, and the TSS overlay.
The renderer also has a defensive rule: if both cell overlays are
somehow active in the same `StudyView`, TSS takes precedence. The shell
should never create that situation, but the renderer states the
precedence explicitly.

## The heatmap idiom, copied not imported

The ANSI-256 heatmap ramp is the same one used by
`patterns::render_heatmap` for the patterndemo binary. The `cli` crate
does not depend on `patterns`. The dependency would violate the crate
layering: the CLI's AI is the `mcts` tactics evaluator, and adding a
`patterns` dependency for a six-entry colour helper would pull an
unrelated crate into the front-end. So `overlay.rs` re-implements the
tiny `heatmap_bg` helper. The lineage is documented in the source.

```text
patterns crate ──► patterndemo binary uses heatmap_bg
       │
       │ (no dependency)
       ▼
cli crate ───────► overlay.rs re-implements heatmap_bg
```

## The verify-before-paint gate for TSS proofs

The TSS overlay paints a forced-win line. A painted line must be
valid, because the user will read it as a certified fact. The engine's
`prove_forced_win` can return a proof within its budget, but the shell
does not render the proof directly. It first calls
`engine::verify_line(board, &proof)`. If verification fails, the shell
shows "internal error: unverifiable proof" and stores no overlay. This
gate guarantees that every painted TSS line is a verified line.

```text
ToggleTss
    │
    ▼
board ongoing?  ──no──►  refusal message
    │yes
    ▼
prove_forced_win ──none──► budget message
    │
    ▼
verify_line ──fail──► internal error message
    │
    ▼
store proof, clear policy overlay, show verdict
```

## File and size budget

No source file in this chapter should grow past its single concern.
`overlay.rs` stays a pure formatting module. `render.rs` adds only the
routing between board cells and overlay cells. `study.rs` adds only the
shell rules for toggles and mutual exclusion. `input.rs` adds only the
three key translations. If a file grows past its concern, split it.

## Cross-cutting decisions

1. **Overlay state is shell-only.** `SessionCore` provides
   `search_info()` and nothing else about overlays. The command grammar
   gains no overlay variants.
2. **Input translations are shell outcomes.** `p`, `v`, and `t` become
   `InputOutcome` values, not `Command` values. Both normal-mode keys
   and line-mode words produce the same outcomes.
3. **Prior is max-normalized, visits are total-normalized.** The two
   normalizations answer different questions and must not be unified.
4. **Hot threshold is 10% for both.** The threshold is inclusive. A
   cell at exactly 10% is hot.
5. **Heatmap ramp is copied, not imported.** Crate layering is more
   important than code reuse for a six-line helper.
6. **TSS proofs are verified before paint.** No unverified line is
   rendered.

## Chapter layout

```text
06-seeing-the-mind.md             index: what the chapter builds, links to sub-chapters
06-seeing-the-mind/
  00-design.md                    this paper
  a-value-strip/
    value-strip.md                tutorial
    value-strip-design.md         design paper
    value-strip-implementation.md implementation notes
    value-strip-solution.md       opt-in solution
  b-visits-overlay/
    visits-overlay.md
    visits-overlay-design.md
    visits-overlay-implementation.md
    visits-overlay-solution.md
  c-prior-overlay/
    prior-overlay.md
    prior-overlay-design.md
    prior-overlay-implementation.md
    prior-overlay-solution.md
  d-tss-overlay/
    tss-overlay.md
    tss-overlay-design.md
    tss-overlay-implementation.md
    tss-overlay-solution.md
```

## Status

All questions are decided. This paper is the contract for the
implementation of the four sub-chapters.
