#!/bin/bash
# The arena, with lab conventions: candidate vs incumbent in
# color-swapped pairs at equal sims, Wilson 95% verdict (promoted iff
# the lower bound clears 0.5), games recorded as a browsable corpus.
#
# Usage:
#   scripts/arena.sh <candidate-run-dir> [--incumbent-net <RUN_DIR>]
#                    [--games N] [--sims N] [--seed-base N]
#
# Defaults: 40 games at 800 sims vs the baseline incumbent. With
# --incumbent-net the candidate faces the reigning champion instead —
# the round-2+ gate. The record lands in
#   ../data/corpora/arena-<candidate>-vs-<incumbent>-<timestamp>
# timestamped so reruns never touch an existing record.
#
# Examples:
#   scripts/arena.sh ../data/check-points/round2-candidate \
#     --incumbent-net ../data/check-points/mass-default-v1
#   scripts/arena.sh ../data/check-points/round2-candidate --games 10 --sims 400

set -u
cd "$(dirname "$0")/.." || exit 1

CAND=${1:?"usage: scripts/arena.sh <candidate-run-dir> [--incumbent-net <RUN_DIR>] [--games N] [--sims N]"}
shift
GAMES=40
SIMS=800
SEED_BASE=0
INC=""
while [ $# -gt 0 ]; do
    case "$1" in
        --incumbent-net) INC=$2; shift 2 ;;
        --games) GAMES=$2; shift 2 ;;
        --sims) SIMS=$2; shift 2 ;;
        --seed-base) SEED_BASE=$2; shift 2 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

cand_name=$(basename "$CAND")
inc_name=baseline
if [ -n "$INC" ]; then
    inc_name=$(basename "$INC")
fi
STAMP=$(date +%Y-%m-%d-%H%M)
REC=../data/corpora/arena-$cand_name-vs-$inc_name-$STAMP

cargo build --release -p selfplay || exit 1

args=(--net "$CAND"
      --openings ../data/openings/gomocup-2024-freestyle15.txt
      --games "$GAMES" --sims "$SIMS" --seed-base "$SEED_BASE"
      --record "$REC")
if [ -n "$INC" ]; then
    args+=(--incumbent-net "$INC")
fi

echo "arena: $cand_name vs $inc_name, $GAMES games at $SIMS sims"
echo "arena: recording to $REC"
./target/release/arenagen "${args[@]}"
