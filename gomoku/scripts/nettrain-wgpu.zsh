#!/bin/zsh
# Run nettrain on the wgpu device against the synthetic v0 dataset.
#
# Usage:
#   GOMOKU_DATA=../data ./nettrain-wgpu.zsh [--steps <n>]
#
# GOMOKU_DATA points at the data root (the directory containing
# synthetic/ and check-points/). --steps defaults to 4000.
#
# Batch 128 is the measured throughput optimum; 4000 steps at batch
# 128 on the metal device takes roughly ten minutes (2026-11-11,
# M5 Max). The progress monitor TUI activates automatically when
# stderr is a terminal (--progress auto); pipe stderr to get the
# line reporter instead.

set -euo pipefail

steps=4000

while [[ $# -gt 0 ]]; do
  case "$1" in
    --steps)
      [[ $# -ge 2 ]] || { echo "error: --steps needs a value" >&2; exit 1; }
      steps="$2"
      shift 2
      ;;
    *)
      echo "error: unknown argument '$1' (usage: $0 [--steps <n>])" >&2
      exit 1
      ;;
  esac
done

: "${GOMOKU_DATA:?error: GOMOKU_DATA is not set (e.g. GOMOKU_DATA=../data $0 --steps $steps)}"

# Release build with the gpu feature (metal + autotune + fusion; the
# default features keep the progress-monitor TUI compiled in).
# Incremental no-op when the sources are unchanged.
(cd "$(dirname "$0")/.." && cargo build --release -p net --features gpu --bin nettrain)

exec "$(dirname "$0")/../target/release/nettrain" \
  --data "$GOMOKU_DATA/synthetic/v0" \
  --out "$GOMOKU_DATA/check-points/v0" \
  --config default \
  --seed 42 \
  --steps "$steps" \
  --batch-size 128 \
  --eval-interval 500 \
  --device metal
