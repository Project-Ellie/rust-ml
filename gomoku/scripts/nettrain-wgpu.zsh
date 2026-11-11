#!/bin/zsh
# Run nettrain on the wgpu device against the synthetic v0 dataset.
#
# Usage:
#   GOMOKU_DATA=../data ./nettrain-wgpu.zsh [--steps <n>]
#
# GOMOKU_DATA points at the data root (the directory containing
# synthetic/ and check-points/). --steps defaults to 5000.

set -euo pipefail

steps=5000

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

exec target/release/nettrain \
  --data "$GOMOKU_DATA/synthetic/v0" \
  --out "$GOMOKU_DATA/check-points/v0" \
  --config default \
  --seed 42 \
  --steps "$steps" \
  --batch-size 1024 \
  --eval-interval 1000 \
  --device wgpu
