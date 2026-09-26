# Chapter 14 — Openings, External Data, and the Supervised Curriculum

## Abstract

This chapter settles two related design questions for the training
pipeline. First: **how the self-play agent performs the Swap2
opening** — the one part of a Gomoku game that the move-by-move
policy network cannot play directly. The answer developed here is
that every Swap2 decision is a *value* judgment, and the
me/you-relative value network supplies exactly that judgment; the
opening is therefore played by evaluation and search, not by a
dedicated policy, and it improves automatically as the value function
improves. Second: **which supervised, externally sourced components
may enter the curriculum** — this project's standing direction is that it
project follows AlphaZero in spirit but welcomes supervised head
starts ("alpha-epsilon"), so this chapter defines what is allowed,
from where, under which licenses, and in which order. The chapter
ends with five design decisions (D1–D5, locked 2026-09-20). All data
sources named here were verified by direct HTTP access on 2026-09-20
and are catalogued in `docs/references/`; negative findings are
recorded in `docs/references/findings-external-data.md`.

## Glossary

- **Swap2** — the balanced opening protocol: player A places three
  stones (two Black, one White); player B chooses Black, chooses
  White, or places two more stones (one Black, one White) and lets A
  choose a color. After the opening, White (the color with fewer
  stones) moves first (procedure text: renju.net, see References;
  engine encoding: ch. 13, Swap2 typestate).
- **me/you encoding** — the network's input representation: planes
  are always presented relative to the side to move ("me" = stones of
  the side to move, "you" = opponent). The engine stores absolute
  colors; the relative view is derived at encode time (ch. 13,
  decision 5).
- **v(s)** — the value function's estimate of the game outcome from
  position `s`, in `[-1, 1]`, from the perspective of the side to
  move. **|v|** — its absolute value; |v| ≈ 0 means the position is
  (estimated) balanced.
- **Opening offer** — the three stones placed by player A in Swap2.
  A **full opening** is the five-stone position after B's
  "place two more" branch.
- **Supervised signal** — any training target that does not originate
  from the agent's own self-play: human or engine game records,
  synthetic tactical generators, verified solver output.
- **Bootstrap signal** — training targets produced by the agent
  itself: self-play game outcomes (`z`) and search-improved policies
  (`π`). Noisy early, exact asymptotically.
- **Anchor set** — milestone 4's collection of machine-verified
  forced-win puzzles with exact ±1 labels, fed as a small fraction of
  each training batch (ch. 12, milestone 4; soundness: decision 8).
- **PSQ** — the plain-text game-record format of the Piskvork
  tournament manager: a header line, then `x,y,ms` move lines.
- **VCF / VCT** — Victory by Continuous Four / Victory by Continuous
  Threat: standard Gomoku terms for forcing tactical sequences (no
  single citable origin verified; see the findings report).
- **Ruleset mismatch** — data generated under rules other than ours
  (most often Renju's forbidden moves, or exact-five wins without
  overlines). Ours is freestyle 15×15: overlines win, no forbidden
  moves (ch. 13, decision 1).
- **Claim labels** — **[paper]** / **[derived]** / **[experiment]** /
  **[proposal]** marks design items not yet
  locked; decisions D1–D5 of this chapter were locked on 2026-09-20.

## 1. Why the opening needs a design

AlphaZero-style training assumes the agent plays complete games
against itself and learns from the outcomes. Two properties of
freestyle Gomoku break the naive form of that assumption:

1. **Freestyle Gomoku is a proven first-player win** (Allis 1994 —
  the empty-board value is a Black win). Without an opening protocol,
  self-play would converge toward "whoever holds Black wins," and the
  network would learn an opening-book truth rather than Gomoku
  (ch. 12 §3).
2. **The Swap2 opening is not a sequence of alternating moves.** One
  player places three stones of mixed color; the other makes a
  protocol decision (which color to hold, or whether to extend the
  opening). A policy network that maps positions to single moves
  cannot express either action, and the me/you encoding — which
  assumes a well-defined side to move — has no meaning during the
  placement phase (this is exactly why the engine stores absolute
  colors, ch. 13 decision 5).

The engine already knows the protocol (the Swap2 typestate, milestone
1). What the design has been missing is the *agent-facing* half:
given a value network and MCTS, how does a self-play worker actually
produce the opening? Sections 2–3 answer this. Section 4 surveys the
external data that can help; section 5 defines the supervised
curriculum it enables.

## 2. The value arithmetic of Swap2

The whole opening theory reduces to one observation: **every Swap2
decision is a comparison of values that the me/you network can
estimate**, because after any complete opening the position is an
ordinary alternating one (stone counts differ by exactly one; White
to move).

Let `s` be a complete offer (2 Black + 1 White, White to move) and
`v = v(s)` the value from the side-to-move's (White's) perspective.
Player B's three options evaluate as:

| B's choice | Resulting holder of White | B's payoff |
|---|---|---|
| take White | B | `+v` (B moves first) |
| take Black | A | `−v` (B is second) |
| place two more → `s′` | A chooses | `−|v(s′)|` (A takes the better side) |

So B computes `max(v, −v, −|v(s′)|)` over candidate extensions `s′`.
Player A, composing the initial offer, faces the same arithmetic from
the other end: B will take the better side, so A's payoff is `−|v|`
— and therefore **A's optimal offer minimizes |v|: the most balanced
opening A can find**. This is precisely the purpose of Swap2 stated
as an optimization problem, and it yields the agent's opening
behavior without any opening-specific machinery:

- **Composing an offer** = search the placement space for candidates
  with estimated |v| ≈ 0, then sample among them for diversity.
- **Answering an offer** = three value lookups (two immediate, one
  over a small extension search), pick the maximum, with exploration
  noise in self-play.
- **Extending an offer** = the same |v|-minimizing search as
  composing, over two-stone additions.

Three consequences worth stating explicitly:

1. **The opening requires values, not priors.** The policy head is
   never consulted during the opening; the value head (sharpened by
   MCTS) does all the work. A weak value function produces poorly
   balanced openings; the opening quality improves automatically as
   the value function improves. Opening skill is a *consequence* of
   evaluation skill, not a separate capability to train.
2. **The training distribution stays near-balanced.** Because A
   minimizes |v| and B takes the better side, self-play games start
   from positions the current network believes are close to even —
   exactly the distribution that forces genuine learning (ch. 12 §3)
   and exactly where value accuracy matters most, which in turn makes
   the *next* round of openings better balanced.
3. **Me/you is never violated.** Every evaluation in the procedure
   happens on a complete, alternating position. The placement phase
   itself is never encoded — it is protocol, not position (§1).

A caveat for completeness: the |v|-minimization is with respect to
the *current* network's beliefs. Early in training, "balanced" means
"balanced according to an ignorant network," which is harmless — the
procedure degrades gracefully into near-arbitrary openings, which is
what random opening lists provide anyway (§3.2).

## 3. The self-play opening procedure

This section proposes the concrete procedure each self-play worker
runs before move 1 of every game. Status: **locked decision D1** (2026-09-20).

### 3.1 Placement in the architecture

The procedure lives in the `selfplay` crate (milestone 5), not in
`engine` and not in `mcts`. The engine's Swap2 typestate remains the
*validator* (every placement and branch is replayed through it, so an
illegal opening cannot occur by construction); the procedure below is
the *policy* that drives it. Evaluations use the worker's normal
MCTS + evaluator path with a modest simulation budget [experiment —
opening evaluations need not cost the full 400].

### 3.2 Candidate offers, three tiers

Player A's offer comes from a mixture of three sources [experiment —
mixing proportions to be tuned]:

1. **Tournament opening lists** (available from day one): the
   verified freestyle opening files — Piskvork's `openings.txt` (41
   Swap2 offers/full openings) and Gomocup's annual
   `openings_freestyle15.txt` (12 per year, 2024–2026 archives) —
   sampled uniformly. These encode decades of engine-tournament
   balance selection and need no value function at all (§4).
2. **Value-guided balance search** (once the value function is
   non-trivial): sample candidate three-stone patterns near the
   center, estimate each with MCTS, keep those with |v| below a
   threshold, sample among survivors. Rapfi's `opengen` tool
   demonstrates this pattern externally (§4); ours is the in-process
   equivalent.
3. **Random near-center offers** (small fraction): cheap diversity
   insurance against the mixture collapsing onto a fashionable
   handful of openings.

### 3.3 Answering the offer

Player B computes the §2 table with MCTS-sharpened values and takes
the argmax. In self-play, the argmax is perturbed [experiment — e.g.,
choose the second-best option with small probability, or add
Dirichlet-style noise to the three payoffs] so that games explore
both colors of the same opening; in arena play the argmax is exact
(measurement must not be blurred, cf. the root-noise discipline in
the MCTS primer §5).

### 3.4 What this procedure never does

- No opening policy head, no opening-specific network input. (A
  learned offer-composer is a conceivable **future upgrade**; it is
  unnecessary while tournament lists and balance search supply
  candidates.)
- No encoding of incomplete openings (§2, consequence 3).
- No trust in the network's raw `v` alone where it matters: all
  opening evaluations go through MCTS, exactly as midgame move
  decisions do.

## 4. The verified external-data landscape

A directed research sweep (2026-09-20; full report with negative
results and per-URL verification status:
`docs/references/findings-external-data.md`; canonical entries:
`docs/references/catalogue.md`) verified the following sources.
Ruleset-mismatch warning applies throughout: everything below is
freestyle-15×15 unless noted, and Renju-derived material must be
re-adjudicated under our rules before any use (decision D5, §5.4).

| Source | Content | License | Fit |
|---|---|---|---|
| Piskvork `openings.txt` | 41 Swap2 offers/full openings | none stated | opening lists (§3.2 tier 1) |
| Gomocup annual archives 2000–2026 | ~419 MB PSQ game records (e.g. 2026: 55,224 games) + `openings_freestyle15.txt` per year | none stated | opening lists; supervised games (local use only) |
| HuggingFace `Karesis/Gomoku` | 26,378 position/next-move pairs (875 engine games) | MIT | supervised pretraining supplement |
| HuggingFace `PoolC/gomoku-dataset-1.8M` | 1.88M tokenized rows | undocumented | unusable without reverse engineering — excluded for now |
| banbu-gomoku VCF material | 763 labelled VCF puzzles with solution lines | data rights unclear (wrapper MIT) | anchor-set seed after re-verification |
| banbu kaibao + RenjuPortal + lfz084 sets | ~5,500 unlabelled VCF positions | unclear | puzzle-generator input for our TSS oracle |
| Rapfi engine (+ CC0 networks) | top open engine (Gomocup Elo ~2716); `selfplay` and `opengen` commands | GPL-3.0 code | external opening/data generator, sparring benchmark |
| AlphaGomoku, KataGomo | full AlphaZero-stack Gomoku engines | GPL-3.0 / MIT-style | sparring, design reference |
| c-gomoku-cli | headless tournament orchestrator, emits training samples | GPL-3.0 | batch data generation harness |
| HF `maojh15/GomokuZeroAI`, `Nagi-ovo/alphazero-gomoku` | AlphaZero-style 15×15 checkpoints | MIT | weight-init experiments, sparring |

Two explicit non-findings matter for planning: **no public Renju.net
bulk game archive** (robots/auth walls), and **no second labelled
VCF puzzle set** beyond banbu's 763 — our own TSS oracle (milestone
4) remains the only scalable source of *verified* labels.

## 5. The supervised curriculum

### 5.1 The standing direction, restated

The project is AlphaZero in spirit with deliberate head starts
("alpha-epsilon"): hand-woven tactics as MCTS priors (milestone 2's
mock evaluator), synthetic tactical training data (milestone 3), and
solver-verified anchor labels (milestone 4) are already locked design.
The project's direction — recorded here as the premise of this section —
is that **supervised components are welcome in the early curriculum**
wherever they are honest about their provenance. Nothing below
changes the self-play loop itself; it changes what the network knows
*before* and *while* the loop runs.

### 5.2 The signal ladder

Training signals ordered by noise, from cleanest to noisiest:

1. **Solver-verified labels** (our TSS oracle; externally sourced
   puzzles only after re-verification): exact, but covering a narrow,
   weird slice of position space (ch. 12, decision 8 — the anchor
   fraction stays small).
2. **Synthetic tactical data** (milestone 3's generator): rules-true
   by construction, distribution by design.
3. **Strong-engine records** (Gomocup archives; Rapfi selfplay):
   strong but not exact; labels are engine moves, which bake in the
   engine's style and occasional blunders.
4. **Self-play bootstrap** (`π`, `z`): noisiest early, asymptotically
   exact, and the only signal that tracks the agent's own
   distribution.

The curriculum descends the ladder: clean supervised signal first,
bootstrap taking over as it becomes reliable.

### 5.3 Phases (locked decision D2)

- **Phase 0 (pre-training, extends milestone 3):** synthetic tactics
  set as already designed, *plus* behavior-cloning on the verified
  permissively-licensed subsets (Karesis/Gomoku; Gomocup PSQ
  restricted to Freestyle15 for local use). Purpose: the network
  starts the loop knowing what a five, a four, and a threat look
  like, so early MCTS is not guided by noise. Deliverable check: the
  phase-0 network must pass milestone 3's synthetic benchmark before
  any self-play game is generated.
- **Phase 1 (loop ignition, milestones 4–6):** self-play begins with
  the §3 opening procedure (tier-1 lists first), the milestone-4
  anchor fraction in each batch, and the 763-puzzle VCF seed
  re-verified into the anchor set.
- **Phase 2 (maturity):** supervised fractions decay [experiment —
  halve the anchor fraction each time arena Elo confirms a gating
  promotion]; the opening mixture shifts from tournament lists toward
  value-guided balance search as the value function earns trust.

### 5.4 Risks and their mitigations

1. **Distribution anchoring.** Behavior-cloned styles cap what
   self-play can discover if the supervised fraction never decays.
   Mitigation: phase-2 decay schedule (§5.3) and the anchor-fraction
   discipline already locked in ch. 12.
2. **Ruleset contamination.** Renju/exact-five positions are wrong
   under our rules (overlines win). Mitigation **(decision D5)**: all
   external positions are re-adjudicated by our engine before use;
   tactical labels are admitted only through `verify_line`
   (soundness gate, unchanged).
3. **License contamination.** Gomocup archives and Piskvork's opening
   file carry no stated license; several puzzle sets have unclear
   data rights. Mitigation **(decision D3)**: permissively licensed
   (MIT/CC0) or self-generated data may enter redistributable
   training sets; unlicensed-but-public archives may be consumed
   locally only and are never redistributed; GPL engines are used as
   external black-box generators/benchmarks, never linked or
   embedded.
4. **Weak-label echo.** Engine game labels inherit engine blunders.
   Mitigation: engine data is a phase-0 supplement, not a persistent
   fraction; the TSS anchor remains the only *trusted* external-ish
   signal, and it is re-verified in-house.

### 5.5 What is deliberately NOT proposed

- Weight initialization from the HF checkpoints (`GomokuZeroAI`,
  `alphazero-gomoku`): architecture mismatch (our 17×17 encoding and
  head shapes differ); kept as sparring partners and
  weight-transplant *experiments* only.
- Any use of the undocumented `PoolC/gomoku-dataset-1.8M` until its
  license and encoding are clarified.
- A learned opening-composer network (§3.4).

## 6. Decisions (locked 2026-09-20)

| # | Proposal |
|---|---|
| D1 | Self-play opening procedure per §3: typestate-validated, MCTS-valued, three-tier offer mixture, perturbed argmax for B in self-play only. |
| D2 | Curriculum phases per §5.3: phase-0 supervised pre-training (synthetic + verified external), phase-1 ignition with anchor fraction, phase-2 decay. |
| D3 | Licensing gate per §5.4 item 3 (permissive/self-generated for redistribution; unlicensed public archives local-only; GPL engines black-box only). |
| D4 | Rapfi (or c-gomoku-cli + engine) as the sanctioned external generator for opening candidates and sparring benchmarks — an [experiment], never part of the core loop. |
| D5 | Re-adjudication rule per §5.4 item 2: no external position or label enters any training set without passing our engine and, for tactical claims, `verify_line`. |

## References

**Project documents:**

- [docs/12-gomoku-architecture.md](12-gomoku-architecture.md) —
  ch. 12: §3 (solved status, opening protocols), milestone 3
  (synthetic data), milestone 4 (anchor set, soundness gate),
  decision 8 (soundness over completeness).
- [docs/13-engine-design.md](13-engine-design.md) — ch. 13:
  decision 1 (freestyle rules), decision 5 (absolute colors), Swap2
  typestate contract.
- [docs/tutorials/mcts-tutorial/00-mcts-primer.md](tutorials/mcts-tutorial/00-mcts-primer.md)
  — MCTS design: value/prior roles, root-noise discipline (§5).
- [docs/references/catalogue.md](references/catalogue.md) and
  [docs/references/findings-external-data.md](references/findings-external-data.md)
  — verified catalogue entries and the research sweep (including
  negative results and link-verification status) behind §4.

**External sources** (verification status as of 2026-09-20; see the
findings report for the full list):

- Allis, L. V. (1994). *Searching for Solutions in Games and
  Artificial Intelligence*. PhD thesis, Maastricht University.
  https://project.dke.maastrichtuniversity.nl/games/files/phd/SearchingForSolutions.pdf
  — freestyle Gomoku is a first-player win; threat-space search.
- Renju.net, *The International Rules of Gomoku (Swap2)*.
  https://www.renju.net/gomokurules/ — protocol wording.
- Piskvork `openings.txt`.
  https://raw.githubusercontent.com/plastovicka/Piskvork/master/openings.txt
- Gomocup results archives (opening files + PSQ games).
  https://gomocup.org/results/
- HuggingFace `Karesis/Gomoku` (MIT).
  https://huggingface.co/datasets/Karesis/Gomoku
- HuggingFace `PoolC/gomoku-dataset-1.8M` (license/encoding
  undocumented). https://huggingface.co/datasets/PoolC/gomoku-dataset-1.8M
- banbu-gomoku VCF material (763 labelled puzzles).
  https://raw.githubusercontent.com/gugujiao953-ship-it/banbu-gomoku/main/public/puzzles/vcf-material.json
- Rapfi engine (GPL-3.0) and CC0 networks.
  https://github.com/dhbloo/rapfi
- AlphaGomoku (GPL-3.0).
  https://github.com/MaciejKozarzewski/AlphaGomoku
- KataGomo (MIT-style). https://github.com/hzyhhzy/KataGomo
- c-gomoku-cli (GPL-3.0). https://github.com/nkg114mc/c-gomoku-cli
- HuggingFace `maojh15/GomokuZeroAI` (MIT).
  https://huggingface.co/maojh15/GomokuZeroAI
- HuggingFace `Nagi-ovo/alphazero-gomoku` (MIT).
  https://huggingface.co/Nagi-ovo/alphazero-gomoku
- Silver, D. et al. (2017). *Mastering the Game of Go without Human
  Knowledge* (AlphaGo Zero). Nature 550, 354–359.
  https://doi.org/10.1038/nature24270 — the self-play loop this
  curriculum feeds.
