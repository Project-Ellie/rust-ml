# Sub-chapter 06d — The TSS overlay + mutual exclusion: design paper

## Abstract

This paper records the decisions behind the TSS overlay. The overlay
runs the engine's threat-space-search prover on demand, verifies the
result before painting, and displays the forced-win line as numbered
cells. It is a cell overlay, so it is mutually exclusive with the policy
heatmap. The mapping from proof to rendered cells is pure and lives in
`overlay.rs`, while the actual painting logic lives in `render.rs`. A
capped search budget keeps the printed move numbers single-digit.

## Glossary

| Term | Definition |
|------|------------|
| **TSS** | Threat-space search, the engine's forced-win prover. |
| **Proof** | The `engine::Proof` value: winner color plus the move line. |
| **Cell overlay** | An overlay that paints individual board cells, as opposed to a side-panel element. |
| **Verify-before-paint gate** | The `engine::verify_line` call that rejects a proof before it is rendered. |
| **Shell invariant** | A property maintained by `study_loop`, here that only one cell overlay is active at a time. |
| **Defensive default** | A renderer behavior that keeps the UI deterministic even if the shell invariant is violated. |

## Context

The playable CLI design paper (§9) divides overlays into two kinds:
panel elements that compose freely and cell overlays that exclude each
other. Sub-chapter 06a implemented the value strip, a panel element.
Sub-chapter 06c implemented the policy heatmap, a cell overlay. This
sub-chapter implements the second cell overlay, the TSS proof overlay,
and wires mutual exclusion between the two cell overlays.

The engine already exposes `prove_forced_win` and `verify_line`. The CLI
must decide when to call them, how to display the result, and how to
keep the result consistent with the policy overlay.

## Decision 1 — The prover runs on demand

The TSS overlay could store a proof per board position, refreshed
whenever the cursor moves. That would be expensive and surprising: the
prover can search tens of thousands of nodes, and the user did not ask
for it. Instead, the prover runs only when the user presses `t` or types
`t`/`tss`. The result is stored in `ShellState::tss_proof` and is shown
until the user toggles it off, activates a policy overlay, or restarts
the shell.

## Decision 2 — Every painted proof passes `verify_line`

`prove_forced_win` returns `Option<Proof>`. A bug or an unexpected board
state could in principle produce a proof that does not actually win. A
picture of a wrong proof is worse than no picture, because the user may
act on it. `toggle_tss` therefore calls `engine::verify_line` before
storing the proof. If verification fails, the message line shows
`internal error: unverifiable proof` and no overlay is painted.

## Decision 3 — The search budget caps depth at 9

The CLI budget is fixed:

```rust
SearchBudget {
    max_nodes: 100_000,
    max_depth: 9,
}
```

The `max_depth` cap is not only a performance guard. The cell renderer
prints move numbers in a three-character cell (` 1 ` or `[2]`). A depth
of 9 guarantees single-digit numbers, so the display never overflows its
slot. This is documented as an invariant in the renderer.

## Decision 4 — Mutual exclusion is a shell invariant with a defensive renderer default

Two cell overlays cannot paint the same cells at the same time. The
shell enforces this invariant: the `CyclePolicy` arm clears
`shell.tss_proof` when a policy overlay is activated, and `toggle_tss`
clears `shell.policy_overlay` when a proof is stored.

The renderer still gives TSS precedence if both overlays are somehow
present in the same `StudyView`. This is a defensive default: a
deterministic proof outranks a sampled heatmap, and the renderer should
never produce a broken picture because the shell made a mistake. The
precedence is implemented by checking the TSS overlay first in
`study_or_policy_cell`.

## Decision 5 — The proof-to-cell mapping lives pure in `overlay.rs`

`TssOverlay` is a pure structure built from a `Proof`. It maps each move
to a 1-based number and a side (attacker or defender). The cell render
functions `render_tss_cell_plain` and `render_tss_cell_styled` are also
pure. They use raw SGR escapes for styled output, keeping `overlay.rs`
crossterm-free and testable without a terminal.

This mirrors the policy overlay design in sub-chapter 06c. All rendering
metadata is computed in `overlay.rs`; `render.rs` only decides which cell
to paint and with which helper.

## Decision 6 — The value strip continues to compose freely

The value strip is a panel element, not a cell overlay. Its visibility
flag is independent of the TSS and policy overlays. The side panel
carries the strip regardless of which board overlay is active, as
required by the design paper.

## Data flow

```text
key 't'              typed 'tss'
   |                       |
handle_normal_key      line_outcome
   |                       |
   +-------> InputOutcome::ToggleTss <--------+
                       |
                  study_loop
                       |
                 toggle_tss
                       |
       prove_forced_win / verify_line
                       |
            ShellState.tss_proof
                       |
       policy_overlay = Off (mutual exclusion)
                       |
                 build_frame
                       |
            StudyView.tss_proof
                       |
            render_study / study_board_lines
                       |
              TssOverlay::new(proof)
                       |
              study_or_policy_cell
                       |
              tss_cell / render_tss_cell_*
                       |
              numbered board cells + verdict message
```

## References

* [Playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md) — §9 overlays.
* [Sub-chapter 06a — The value strip](../a-value-strip/value-strip.md) — panel composition.
* [Sub-chapter 06c — The prior view](../c-prior-overlay/prior-overlay.md) — the other cell overlay.
* [docs/12-gomoku-architecture.md](../../../../12-gomoku-architecture.md) — crate topology and the core/shell split.
* [Milestone 1 TSS slice](../../../../gomoku/crates/engine/src/tss.rs) — the engine prover and verifier.
