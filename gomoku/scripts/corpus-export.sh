#!/bin/bash
# Snapshot export: mix every SEALED chunk under a corpus root into a
# timestamped samples directory. Safe to run while generation is live —
# chunks without a manifest.json (in flight) are ignored. Never
# overwrites: the default output name carries the export timestamp.
#
# Usage:
#   scripts/corpus-export.sh <corpus-root> [samples-dir]
#
# Example:
#   scripts/corpus-export.sh ../data/corpora/selfplay-v1-mass-default-v1
#   # -> ../data/corpora/selfplay-v1-mass-default-v1-samples-2026-11-22-0815

set -u

cd "$(dirname "$0")/.." || exit 1

ROOT=${1:?usage: scripts/corpus-export.sh <corpus-root> [samples-dir]}
STAMP=$(date +%Y-%m-%d-%H%M)
OUT=${2:-$ROOT-samples-$STAMP}

cargo build --release -p selfplay || exit 1

set --
count=0
for dir in "$ROOT"/*/; do
    if [ -f "$dir/manifest.json" ]; then
        set -- "$@" --corpus "${dir%/}"
        count=$((count + 1))
    fi
done

if [ "$count" -eq 0 ]; then
    echo "corpus-export: no sealed chunks under $ROOT" >&2
    exit 1
fi

echo "corpus-export: mixing $count sealed chunks -> $OUT"
./target/release/selfplayexport "$@" --out "$OUT"
echo "corpus-export: done. Train with:"
echo "  cargo run --release -p net --features gpu --bin nettrain -- \\"
echo "    --data $OUT --out ../data/check-points/<run-name> \\"
echo "    --config default --device metal --steps <N> --batch-size 256"
