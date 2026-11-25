# Exploration-Schedule Probe — Plan (2026-11-25)

**Status: plan, not started. Docs first — discussion before any run.**

## Background and hypothesis

Gomoku is tactically brittle: one soft move can hand the opponent a
forcing threat sequence that ends the game. Our exploration schedule
(Silver et al.: temperature 1.0 + Dirichlet ε=0.25, α=0.3 for the first
10 plies) deliberately plays *weak* moves early — which in Gomoku may
not "explore" so much as *inflict damage*.

Circumstantial evidence from the v1 production corpus
(`selfplay-v1-mass-default-v1`, 3,440 games):

- Median game length is **10 plies** — exactly the exploration window.
  Games end right when the noise stops, as if the damage done during
  exploration is punished immediately.
- Black wins **80%** overall, 79% in mirror matches — openings arrive
  pre-decided at our agents' strength.

Wolfie's competing observation: GUI games at zero temperature, where
both sides have TSS foresight, become *long* and careful — breaking
through a clean defense takes real work.

### The three hypotheses the probe must discriminate

- **H1 — noise damage**: exploration causes the short games. Prediction:
  zero-exploration games run markedly longer.
- **H2 — opening decisiveness**: the 12 fixed Swap2 openings are simply
  Black-fatal at this strength; exploration is innocent. Prediction:
  zero-exploration games stay short and Black-skewed.
- **H3 — resignation eagerness**: the value head concedes early
  regardless. Prediction: games end by resignation at similar rates in
  all configs, at similar plies.

These are not exclusive; the probe measures each one's share.

## Variables and configs

Fixed across all configs: `mass-default-v1` on both seats, **800/800
sims mirror** (removes the strength gradient so exploration is the only
variable), the same 12-opening pool, the same resignation and TSS
settings, seed protocol unchanged.

| Config | Plies | Temp | ε | Role |
|--------|-------|------|---|------|
| A0 | 0 | — | — | control: no exploration |
| B5 | 5 | 1.0 | 0.25 | short window |
| C10 | 10 | 1.0 | 0.25 | **current default** (reproduction) |
| D10-soft | 10 | 0.5 | 0.10 | soft noise, same window |
| E20 | 20 | 1.0 | 0.25 | long window |
| F20-soft | 20 | 0.5 | 0.10 | long + soft |

## Implementation prerequisites (small, ~1h with gates)

1. **Two flags**: `selfplaygen --exploration-temperature <T>` and
   `--exploration-epsilon <E>`. The `ExplorationSchedule` struct already
   carries all four fields and records them in every corpus header —
   the binary just never exposes them. Defaults unchanged (1.0 / 0.25),
   so existing corpora and the running profile are untouched.
2. **`scripts/exploration-probe.sh`**: reuses the mass-corpus
   discipline (chunks of 5 games, manifest-last, disjoint seed ranges —
   offset 400M, config-scoped), partitioning 12 workers as 2 per
   config, output `../data/corpora/probe-exploration/<config>-wNN-cNN/`.
3. **A committed stats example** `crates/selfplay/examples/corpusstats.rs`
   (the throwaway plystats/colorsplit pattern, made permanent): per
   corpus — length distribution, color split, termination mix, and the
   two new signals below. Committing it makes the probe reproducible.

## Metrics (computed from records; no training, no extra games)

1. **Length distribution** per config: median / p95 / max. Headline.
2. **Color split** per config (H2 watch).
3. **Termination mix**: five-in-a-row vs resignation vs board-fill draw.
4. **Decision ply**: first ply where |root_value| ≥ 0.9 and stays there
   until the end. If decision plies cluster inside/right after the
   exploration window, H1 confirmed with the mechanism exposed.
5. **TSS-decided share**: plies with `root_value = None` (no search —
   proof, guard, or resignation) mark where tactics, not noise, ruled.
6. **Diversity guard**: distinct positions at ply 10 (Zobrist) per
   opening — the no-exploration configs must not collapse into 12
   deterministic lines, or the corpus becomes a pamphlet.

## Protocol

- 12 workers, 2 per config, chunks of 5 games, ≤ 8 hours.
- Expected yield: 60–100 games/h aggregate under mixed lengths →
  roughly 80–150 games per config. Thin but sufficient for median
  length and outcome-split comparisons; we are probing dynamics, not
  publishing p-values.
- Kill-safe as always; analysis runs on whatever is sealed.

## Decision criteria (pre-registered)

- **H1 confirmed** (A0/F20-soft clearly longer, decision plies leave
  the window): production v2 adopts a *mix* — majority low/no
  exploration (Wolfie's instinct: ~50% plies=0) plus a minority at the
  softest setting that preserves diversity (likely D10-soft).
- **H2 confirmed** (A0 stays short and Black-skewed): exploration
  schedule stays; opening-pool expansion and the agent-answered Swap2
  feature become the priority instead.
- **Diversity check overrides**: if zero-exploration games collapse to
  repeated lines per opening, the no-exploration share is capped
  regardless of length gains.

## After the probe

The v2 production profile encodes the chosen mix by extending the
script's spec lines with the exploration settings (e.g.
`800 800 plies=0,temp=1` per chunk), and production restarts into a
fresh `selfplay-v2-mass-default-v1` corpus. The frozen v1 corpus
(57k samples) remains as a mixing fraction.
