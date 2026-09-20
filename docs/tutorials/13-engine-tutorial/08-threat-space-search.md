# Slice 8 — Threat-space search (the proving oracle)

Tactics (slice 7) detects what is true *now*. This slice proves what is
*forced*: a bounded, recursive search over threat sequences that
certifies "this side wins by force from here" and returns the winning
line — win-in-3, win-in-5, VCF-style. It is the engine's half of
milestone 4: the oracle whose verified proofs become exact ±1 training
labels. Design: ch. 13, "Threat-space search"; the soundness rules are
locked decision 8.

## Contract

`tss.rs`:

```rust
/// A certified forced win: the winning side and one forcing sequence.
/// `line` alternates moves, starting with `winner`'s first threat, and
/// ends when the attacker completes five (overlines count) or creates
/// an unblockable double threat (>= 2 immediate wins).
pub struct Proof {
    pub winner: Color,
    pub line: Vec<Move>,
}

/// Hard caps. Exhausting the budget means "no proof" — never "loss".
pub struct SearchBudget {
    pub max_nodes: u32,
    pub max_depth: u8,
}

/// `Some` = certified forced win for `side` from `b`.
/// `None` = not proven within budget; says NOTHING about the true value.
pub fn prove_forced_win(b: &Board, side: Color, budget: SearchBudget) -> Option<Proof>;

/// The soundness net. Replays `proof.line` from `b`: every move legal,
/// alternating, forcing; at each defender turn, EVERY alternative that
/// addresses the threat is enumerated and the attacker's continuation
/// must still win (gapped fours give the defender two blocks — this is
/// where naive verifiers lie). `true` only if the proof is airtight.
pub fn verify_line(b: &Board, proof: &Proof) -> bool;
```

Generation rule (milestone 4 will rely on it): a `(position, Proof)`
pair may become training data only after `verify_line` passes. `None`
results are discarded, never labeled.

## How the search is shaped

Design-level — the implementation is yours:

- **Attacker nodes** (side `side` to move): first check
  `immediate_wins(b, side)` — a one-move win ends the proof. Otherwise
  the candidate moves are the *threat-generating cells*: empty cells
  where a hypothetical placement (slice 7's trick) leaves `side` with
  >= 1 immediate win. That set is small; the full 225 never enters the
  recursion.
- **Defender nodes**: the defender's replies are exactly
  `forced_blocks` — the attacker's immediate-win cells. Usually one,
  sometimes two (gapped four). Zero means the attacker's last move
  created no threat: branch dies.
- **Terminal**: attacker completes five, or the threat is unblockable
  (>= 2 immediate wins after the attacker moves).
- **Budget**: every recursion spends nodes; `max_depth` caps the line
  length. `None` on exhaustion.

## Rust toolbox

- **Recursion with explicit state**: a small `struct Search { budget
  left, ... }` with `&mut self` methods beats threading counters
  through free functions. The budget is a plain countdown you
  decrement per node and check on entry.
- **Building the line on the success path only**: recurse returning
  `Option<Vec<Move>>`; on `Some(mut rest)`, push your move and return.
  No `Rc`, no arena — the line exists fully only when the proof does.
- **Play/undo**: `Board` already has O(1) `play`/`undo` (slice 3) and
  the Zobrist key comes free. Recursion explores by play/undo on ONE
  board, not by cloning.

## TDD checklist

1. Immediate win: attacker has an open four → `Some`, `line` is the
   one winning move.
2. Quiet position (scattered stones, no threes) → `None`.
3. Open-four maker: attacker plays the cell that creates TWO immediate
   wins → `Some`, one-move line (unblockable, nothing to enumerate).
4. VCF win-in-3: closed four → forced block → double threat →
   alternating line of length 3, defender's reply in `forced_blocks`.
5. Budget honesty: `max_nodes` tiny on puzzle 4 → `None` (and the same
   position with a generous budget → `Some`). Depth cap: same pattern.
6. `verify_line` accepts the proofs from 1–4; rejects doctored ones:
   illegal move, wrong starter color, non-forcing attacker move, line
   ending before the win.
7. **The gapped-four trap** (the soundness test that matters): a
   position where the defender has TWO blocks and one of them refutes
   the line. A proof claiming the win must fail `verify_line`. Build
   this puzzle by hand from the 07-tactics's gapped-four discussion.
8. Differential soundness: on shallow random positions, brute-force
   adjudication via the reference engine (depth-bounded full search —
   shallow only!) agrees with every `Some` the prover emits.
9. Fuzz: random mid-game positions, capped budget — 100% of `Some`
   results pass `verify_line`. This property IS the milestone-1
   soundness gate (ch. 12, §12 item 3).
10. Overline: a proof whose last move completes SIX in a row is valid
    (freestyle — ch. 13, decision 1).

## ML refresh — why exact labels are worth a slice

Self-play value targets are *bootstrap* estimates: `z` is the outcome
of a game between two imperfect agents, noisy for a long time. A
verified proof is rules-truth: `z = ±1` exactly, `π` = the forcing
move, no noise ever. That signal is aimed at the network's weakest
spot — sharp tactical lines where a weak prior makes MCTS blunder. The
caution is distributional: forcing positions are a narrow, weird slice
of all positions, so milestone 4 feeds them as a SMALL anchor fraction
of each batch, rooted in positions the agent actually visits. The
oracle is a tutor, not the curriculum.

## Pitfalls

- **`None` is not a verdict.** Budget exhaustion, depth caps, and an
  incomplete attacker move generator all produce `None` for WON
  positions. Only `Some` carries information. (Decision 8: soundness
  over completeness — a missed win costs coverage, a wrong `Some`
  poisons training.)
- **The defender's own win comes first.** After the attacker's move,
  check whether the DEFENDER has an immediate win before recursing —
  if so, that attacker branch loses outright (the defender plays it
  and the threat never matures). Forgetting this check is the classic
  way to "prove" lines that lose on the spot.
- **Gapped fours break string-match verifiers.** A four with a gap can
  have two distinct winning cells; the defender may block either, and
  the continuation differs. `verify_line` must enumerate, not assume.
- **Threats are a global property** (07-tactics): a cell can be a
  winning cell because of stones far away. Generate threats with the
  hypothetical-placement primitive, not with local pattern matching.
- **Do not call this per PUCT step.** The prover is for puzzle
  generation and (much later, milestone-8 upgrade) a search shortcut —
  capped and rare. Tactics at expansion time, TSS offline.

## Done when

Checklist green — especially 7–9 at 100%; gates green.
Commit: `feat(engine): threat-space prover + line verifier`.

Next: [Slice 9 — The Swap2 opening](09-swap2-opening.md)
