#!/bin/bash
# Mass baseline self-play corpus generation — the unattended run.
#
# 12 workers × ~25-game chunks, cycling through sim pairings. Every
# completed chunk is a finished corpus (manifest written last by
# selfplaygen); killing the run loses at most the in-flight chunks.
# Rerunning the script resumes: completed chunks are skipped,
# interrupted chunks (directory without manifest.json) are wiped and
# regenerated.
#
# Ends with a weighted mix export of all completed chunks into
# $OUT-samples, ready for nettrain --data.
#
# Usage:
#   scripts/mass-corpus.sh                  # balanced profile (default)
#   NET=<RUN_DIR> PROFILE=net scripts/mass-corpus.sh   # champion self-play
#   nohup ... &                             # detached
#
# Profiles:
#   balanced  800/800 · 400/400 · 1200/800 · 800/1200 · 1600/1600
#             (~10h, volume + grading)
#   deep      3000/3000 · 3000/4000 · 4000/4000
#             (~3.5-5 min/game; CYCLES default 8 ≈ 40h ≈ 7200 games;
#             the distillation profile: strong-search labels for a
#             net that will play much cheaper)
#   net       champion self-play: NET seats both colors; 80% 1600/1600,
#             20% asymmetric 800/2400 + 2400/800 (color-balanced value
#             signal). Open-ended (CYCLES default 999 — kill and rerun
#             to resume; sealed chunks are never touched). ~20 min/game
#             under 8-way load; snapshot exports any time via
#             scripts/corpus-export.sh.
#
# Knobs: WORKERS (12; 8 for net), CYCLES, CHUNK_GAMES (25; 10 for net),
# OUT, OPENINGS.

set -u

# Do not let the Mac sleep mid-run.
if [ -z "${CAFFEINATED:-}" ]; then
    exec caffeinate -dims env CAFFEINATED=1 "$0" "$@"
fi

# Resolve paths from the script location: gomoku/scripts/ -> gomoku/.
cd "$(dirname "$0")/.." || exit 1

NET=${NET:-}
PROFILE=${PROFILE:-balanced}
case "$PROFILE" in
    deep)
        SPECS="3000 3000
3000 4000
4000 4000"
        DEFAULT_OUT=../data/corpora/deep-2026-11-21
        DEFAULT_CYCLES=8
        DEFAULT_WORKERS=12
        DEFAULT_CHUNK_GAMES=25
        # Deep seeds live in their own range, clear of the balanced runs.
        SEED_OFFSET=100000000
        ;;
    net)
        if [ -z "$NET" ]; then
            echo "PROFILE=net requires NET=<RUN_DIR> (a nettrain run directory)" >&2
            exit 2
        fi
        if [ ! -f "$NET/model.mpk" ]; then
            echo "NET '$NET' holds no model.mpk" >&2
            exit 2
        fi
        # 80% equal-sims anchor, 20% asymmetric (color-balanced).
        SPECS="1600 1600
1600 1600
1600 1600
1600 1600
1600 1600
1600 1600
1600 1600
1600 1600
800 2400
2400 800"
        DEFAULT_OUT=../data/corpora/selfplay-v1-$(basename "$NET")
        DEFAULT_CYCLES=999
        DEFAULT_WORKERS=8
        DEFAULT_CHUNK_GAMES=10
        # Net self-play seeds live in their own range.
        SEED_OFFSET=200000000
        ;;
    balanced)
        SPECS="800 800
400 400
1200 800
800 1200
1600 1600"
        DEFAULT_OUT=../data/corpora/mass-2026-11-20
        DEFAULT_CYCLES=4
        DEFAULT_WORKERS=12
        DEFAULT_CHUNK_GAMES=25
        SEED_OFFSET=0
        ;;
    *)
        echo "unknown PROFILE '$PROFILE' (balanced|deep|net)" >&2
        exit 2
        ;;
esac

WORKERS=${WORKERS:-$DEFAULT_WORKERS}
CYCLES=${CYCLES:-$DEFAULT_CYCLES}
CHUNK_GAMES=${CHUNK_GAMES:-$DEFAULT_CHUNK_GAMES}
OUT=${OUT:-$DEFAULT_OUT}
OPENINGS=${OPENINGS:-../data/openings/gomocup-2024-freestyle15.txt}
EXPORT_OUT=${EXPORT_OUT:-$OUT-samples}

# Seat flags: net self-play seats the champion on both colors.
NET_ARGS=()
if [ -n "$NET" ]; then
    NET_ARGS=(--black-net "$NET" --white-net "$NET")
fi

echo "mass-corpus: building selfplaygen (release)..."
cargo build --release -p selfplay || exit 1

mkdir -p "$OUT/logs"

run_chunk() {
    # worker, black sims, white sims, cycle, spec index
    local w=$1 sb=$2 sw=$3 cycle=$4 idx=$5
    local c=$(( cycle * 10 + idx ))
    local seed=$(( SEED_OFFSET + w * 1000000 + c * 1000 ))
    local dir
    dir=$(printf '%s/%dv%d-w%02d-c%02d' "$OUT" "$sb" "$sw" "$w" "$c")

    if [ -f "$dir/manifest.json" ]; then
        return 0 # completed earlier: skip
    fi
    if [ -d "$dir" ]; then
        rm -rf "$dir" # interrupted earlier: wipe and redo
    fi

    local started
    started=$(date +%s)
    ./target/release/selfplaygen \
        ${NET_ARGS[@]+"${NET_ARGS[@]}"} \
        --games "$CHUNK_GAMES" \
        --seed-base "$seed" \
        --black-sims "$sb" \
        --white-sims "$sw" \
        --openings "$OPENINGS" \
        --out "$dir" >> "$OUT/logs/w$(printf %02d "$w").log" 2>&1
    local rc=$?
    local elapsed=$(( $(date +%s) - started ))
    if [ $rc -eq 0 ]; then
        echo "$(date '+%H:%M:%S') w$w c$c ${sb}v${sw} ok ${elapsed}s" >> "$OUT/progress.log"
    else
        echo "$(date '+%H:%M:%S') w$w c$c ${sb}v${sw} FAILED rc=$rc" >> "$OUT/progress.log"
    fi
}

worker() {
    local w=$1
    local cycle idx
    for ((cycle = 0; cycle < CYCLES; cycle++)); do
        idx=0
        echo "$SPECS" | while read -r sb sw; do
            run_chunk "$w" "$sb" "$sw" "$cycle" "$idx"
            idx=$(( idx + 1 ))
        done
    done
    echo "$(date '+%H:%M:%S') w$w done" >> "$OUT/progress.log"
}

echo "mass-corpus: profile $PROFILE, $WORKERS workers, $CYCLES cycles, output $OUT"
echo "mass-corpus: progress in $OUT/progress.log, per-worker logs in $OUT/logs/"
echo "mass-corpus: partial export anytime with:"
echo "  ./target/release/selfplayexport \\"
echo "    $(printf -- '--corpus %s/... ' "$OUT")... --out $EXPORT_OUT"

for w in $(seq 0 $((WORKERS - 1))); do
    worker "$w" &
done
wait

# Mixed export over every completed chunk (a manifest marks completion).
echo "mass-corpus: all workers done, exporting the mixed dataset..."
set --
for dir in "$OUT"/*/; do
    if [ -f "$dir/manifest.json" ]; then
        set -- "$@" --corpus "${dir%/}"
    fi
done
if [ $# -eq 0 ]; then
    echo "mass-corpus: no completed chunks found, nothing to export" >&2
    exit 1
fi
./target/release/selfplayexport "$@" --out "$EXPORT_OUT"
echo "mass-corpus: done. Train with:"
echo "  cargo run --release -p net --features gpu --bin nettrain -- \\"
echo "    --data $EXPORT_OUT --out ../data/check-points/<run-name> \\"
echo "    --config mid --device metal --steps <N> --batch-size 256"
