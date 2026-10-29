# Sub-chapter 06c — The prior view: design paper

## Abstract

This paper records the decisions behind the Prior flavor of the policy
overlay. Prior mode shows the evaluator's raw belief before the search
simulates any plys. It normalizes by the maximum prior, prints raw
percentages on hot cells, and guards against empty priors and zero
maximums. The design makes the difference between prior and visits
visible: prior is the evaluator's starting opinion, visits are the
search's conclusion.

## Glossary

| Term | Definition |
|------|------------|
| **Evaluator** | The neural network or tactical function that produces the prior policy vector. |
| **Prior** | The evaluator's probability distribution over legal moves at a position. |
| **Visits** | The MCTS visit counts after simulation. |
| **Normalized intensity** | A value in 0.0–1.0 that selects the background colour. |
| **Raw percentage** | The prior probability multiplied by 100, rounded to the nearest whole percent. |
| **Hot threshold** | The inclusive 0.10 boundary on normalized intensity. |

## Context

Sub-chapter 06b implemented the Visits flavor of the policy overlay. It
normalizes each move's visit count by the total number of visits. This
answers the question "how much of the search budget did this move get?".
The Prior flavor answers a different question: "what did the evaluator
think before the search started?". The two answers can disagree, and
that disagreement is the point of the feature.

## Decision 1 — Prior normalizes by maximum; visits normalizes by total

The two denominators answer different questions.

Visits normalize by total visits because a visit share is meaningful in
absolute terms. If a move received 40% of the simulations, it dominated
the search, regardless of how the remaining 60% was distributed.

Prior normalizes by the maximum prior because the evaluator's raw
probabilities can be very flat. A tactics evaluator may assign 0.07 to
ten different moves and 0.14 to one winning move. Normalizing by total
would make every flat move look dim; normalizing by maximum makes the
best move bright and the flat moves dim. The brightest cell is always
the evaluator's favourite, which is what a human wants to see.

## Decision 2 — The hot label is the raw percentage

The cell background intensity is normalized, but the printed text is the
raw prior percentage. This is a deliberate split.

Intensity carries the visual comparison: a cell at 50% intensity is half
as bright as the maximum. The label carries the evaluator's actual
belief: a cell with raw prior 0.07 reads "7%", not "50%". Showing the
normalized percentage would hide the flatness of the distribution. A
board full of "0%" labels is honest evidence that the evaluator has no
strong opinion.

## Decision 3 — Empty and zero-max priors yield no overlay cells

If `SearchInfo.prior` is empty, there is no evaluator data to show. If
the maximum prior is `0.0`, every prior is zero and normalization would
divide by zero. Both cases return `None` from `prior_cell`. The renderer
falls back to the bare board glyph. This is the same rule as visits with
zero total visits.

## Decision 4 — The cycle grows rather than adding a separate key

The policy overlay has mutually exclusive cell flavors. A single key
that cycles through them keeps the normal-mode key table small and keeps
the user's mental model simple: press `p` repeatedly to see the next
belief layer. Adding a separate key for prior would force the user to
remember which key maps to which flavor and would require a new
`InputOutcome` variant for no benefit.

## Decision 5 — The 10% threshold is shared and inclusive

Both flavors use `intensity >= 0.10` as the hot threshold. Sharing the
boundary keeps the overlay behavior predictable. The boundary is
inclusive so a move at exactly 10% of the maximum is not silently
hidden. The tests pin both sides of the boundary.

## Decision 6 — Stones, cursor, and last move keep precedence

The policy overlay paints only bare empty cells. A stone is a fact on
the board, the last-move marker is navigation context, and the cursor is
the user's point of attention. All three take precedence over the
heatmap. This rule is inherited from the visits flavor and is not
relaxed for prior mode.

## Decision 7 — `prior_cell` is pure and testable

`prior_cell` has no I/O, no randomness, and no dependency on a real
MCTS search. A test can build a `SearchInfo` value directly and assert
on `intensity`, `is_hot`, and `text`. This makes the math fast to verify
and keeps the render path thin.

## Data flow

```text
key 'p' (second press)
        |
   handle_key / line_outcome
        |
   InputOutcome::CyclePolicy
        |
   study_loop
        |
   shell.policy_overlay = Prior
        |
   build_frame
        |
   StudyView.policy_overlay(Prior)
        |
   render_study
        |
   study_or_policy_cell
        |
   policy_empty_cell(_, Prior)
        |
   overlay_cell(_, _, Prior)
        |
   prior_cell
        |
   OverlayCell { intensity, text, is_hot }
```

## References

* [Sub-chapter 06b — The visits view](../b-visits-overlay/visits-overlay.md) — the policy overlay cycle.
* [Sub-chapter 06a — The value strip](../a-value-strip/value-strip.md) — the panel overlay that composes with cell overlays.
* [Sub-chapter 05d — The alternate-screen shell](../05-the-real-table/d-shell/shell.md) — the shell loop.
* [Playable CLI design paper](../../../../specs/2026-10-17-playable-cli-design.md) — §9 overlays.
