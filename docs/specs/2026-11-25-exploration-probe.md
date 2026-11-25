# Exploration-Schedule Probe — Plan (2026-11-25, revised)

**Status: plan, not started. Docs first — discussion before any run.**

## Background and hypothesis

Gomoku is tactically brittle: one soft move can hand the opponent a
forcing threat sequence that ends the game. Exploration in a minefield
is not strategy discovery — it is stepping on mines. **Owner's
directive (2026-11-25): abandon T=1.0 entirely.** Temperature caps at
0.5; 0.2 is the lower setting. The goal is not to find a hidden
super-strategy; it is to reach the next champion efficiently, without
burning games on self-inflicted damage.

Revised framing of what exploration is *for* in this project:

1. **Corpus anti-collapse** — without any stochasticity, mirror
   self-play over a 12-opening pool degenerates into a handful of
   repeated lines (a pamphlet, not a corpus).
2. **Value gradients** — games must occasionally be *contested* so the
   outcome is not pre-decided by the opening.

Both needs are served by *gentle* jitter among search-endorsed moves —
not by sampling moves the search has already judged weak.

Evidence from the v1 production corpus (3,440 games, Silver defaults:
T=1.0, ε=0.25, 10 plies):

- Median game length is **10 plies** — exactly the exploration window.
  Games end when the noise stops.
- Black wins **80%** overall, 79% in mirror matches.

**The v1 corpus is the T=1.0 reference arm.** Its statistics are
already measured; the probe spends nothing on re-confirming them.

### The three hypotheses the probe must discriminate

- **H1 — noise damage**: exploration causes the short games. Prediction:
  zero/low-exploration games run markedly longer than the v1 baseline
  (median 10 plies).
- **H2 — opening decisiveness**: the 12 fixed Swap2 openings are simply
  Black-fatal at this strength. Prediction: even zero-exploration games
  stay short and Black-skewed.
- **H3 — resignation eagerness**: the value head concedes early
  regardless. Prediction: resignation share and decision plies are
  flat across configs.

These are not exclusive; the probe measures each one's share.

## Variables and configs

Fixed across all configs: `mass-default-v1` both seats, **800/800 sims
mirror** (removes the strength gradient), the same 12-opening pool,
same resignation/TSS settings, same seed protocol.

| Config | Plies | Temp | ε | Role |
|--------|-------|------|---|------|
| A0 | 0 | — | — | control: fully deterministic |
| B5-02 | 5 | 0.2 | 0.10 | shortest window, gentle |
| C10-02 | 10 | 0.2 | 0.10 | gentle, standard window |
| D10-05 | 10 | 0.5 | 0.10 | the allowed maximum heat |
| E20-02 | 20 | 0.2 | 0.10 | long window, gentle |
| F10-pure | 10 | 0.2 | **0** | temperature-only: jitter exclusively among search-endorsed moves |

F10-pure isolates the Dirichlet mechanism: root noise boosts priors of
*arbitrary* moves (the crudest mine-stepper); pure low-temperature
sampling only jitters between moves the search already likes. If
F10-pure preserves diversity without shortening games, it is the ideal
production explorer.

## Implementation prerequisites (small, ~1h with gates)

1. **Two flags**: `selfplaygen --exploration-temperature <T>` and
   `--exploration-epsilon <E>`. The `ExplorationSchedule` struct already
   carries all four fields and records them in every corpus header —
   the binary just never exposed them. Defaults unchanged (1.0 / 0.25),
   so existing corpora and profiles are untouched.
2. **`scripts/exploration-probe.sh`**: reuses the mass-corpus
   discipline (chunks of 5 games, manifest-last, disjoint seed ranges —
   offset 400M, config-scoped), partitioning 12 workers as 2 per
   config, output `../data/corpora/probe-exploration/<config>-wNN-cNN/`.
3. **A committed stats example** `crates/selfplay/examples/corpusstats.rs`
   (the throwaway plystats/colorsplit pattern, made permanent): per
   corpus — length distribution, color split, termination mix, decision
   ply, TSS share, diversity. Committing it makes the probe
   reproducible — and it doubles as the v1 reference-arm reader.

## Metrics (computed from records; no training, no extra games)

1. **Length distribution** per config: median / p95 / max. Headline.
   The v1 baseline (median 10, avg 16.5) is the bar to beat.
2. **Color split** per config (H2 watch; v1 baseline: 80% Black).
3. **Termination mix**: five-in-a-row vs resignation vs board-fill draw.
4. **Decision ply**: first ply where |root_value| ≥ 0.9 and stays there.
   Clustering inside/right after the window exposes the H1 mechanism.
5. **TSS-decided share**: plies with `root_value = None` mark where
   tactics, not noise, ruled.
6. **Diversity guard**: distinct positions at ply 10 (Zobrist) per
   opening. The floor that configs A0 and F10-pure must not fall
   through.

## Protocol

- 12 workers, 2 per config, chunks of 5 games, ≤ 8 hours.
- Expected yield: 60–100 games/h aggregate under mixed lengths →
  roughly 80–150 games per config. Thin but sufficient for median
  length and outcome-split comparisons; we are probing dynamics, not
  publishing p-values.
- Kill-safe as always; analysis runs on whatever is sealed.

## Decision criteria (pre-registered)

- **H1 confirmed** (gentle/zero-exploration configs clearly longer than
  the v1 baseline, decision plies leave the window): production v2
  adopts a mix — ~50% plies=0 (owner's directive) plus the gentlest
  config that preserves diversity (expected: F10-pure or C10-02).
- **H2 confirmed** (even A0 stays short and Black-skewed): exploration
  was innocent; opening-pool expansion and the agent-answered Swap2
  feature take priority instead.
- **Diversity override**: if A0/F10-pure collapse to repeated lines per
  opening (distinct-position count per opening near 1), their
  production share is capped regardless of length gains.

## After the probe

The v2 production profile encodes the chosen mix by extending the
script's spec lines with the exploration settings (e.g.
`800 800 plies=0` vs `800 800 plies=10,temp=0.2,eps=0` per chunk), and
production restarts into a fresh `selfplay-v2-mass-default-v1` corpus.
The frozen v1 corpus (57k samples) remains as a mixing fraction.
