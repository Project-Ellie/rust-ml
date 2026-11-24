# Released champions

Tracked, play-ready net checkpoints — the models that earned promotion
in the arena. Each subdirectory is a complete nettrain run directory
(`model.mpk` + `run.json`), so any of them can be seated directly:
point the GUI's *Checkpoint root* at this `models/` folder, or pass a
run directory to `selfplaygen --black-net/--white-net`,
`arenagen --net/--incumbent-net`, or `mass-corpus.sh NET=`.

Training telemetry (`metrics.jsonl`) stays in the local run
directories under `data/check-points/` (gitignored, bulk); only what
is needed to play and to know the provenance is committed here.

## mass-default-v1 (2026-11-21)

The first champion.

- **Architecture**: default config — 128 channels × 10 residual
  blocks, 3.17M parameters (Burn 0.22, full-precision record).
- **Trained on**: `data/corpora/mass-2026-11-20-samples` —
  432,303 samples from 17,500 baseline self-play games
  (400–1600 sims, strength-graded pairings, Gomocup 2024 freestyle-15
  openings), 20,000 steps at batch 256 on Metal. Final losses:
  policy 2.22, value 0.51.
- **Promotion**: arena vs the baseline incumbent, 4,000 sims,
  40 games — **39W 1L**, Wilson lower bound ≈ 0.87 ≫ 0.5.
- **Seeds**: self-play production with this champion lives in the
  200M seed range (`PROFILE=net`, `selfplay-v1-mass-default-v1`).
