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
#   net       champion self-play: NET seats both colors. v2 mixture
#             (probe-informed, 2026-11-25): per 20 chunks — 10 without
#             exploration, 5 gentle 5-ply window (T=0.2, eps=0.10),
#             5 temperature-only 10-ply whisper (T=0.2, eps=0); sims
#             ~65% asymmetric 400/800 + 800/400. No ply cap: honest
#             labels (capped games record as false draws). Open-ended
#             (CYCLES default 999 — kill and rerun the SAME command to
#             resume; the corpus only grows). Snapshot exports via
#             scripts/corpus-export.sh.
#
# Knobs: WORKERS (12; 14 for net), CYCLES, CHUNK_GAMES (25; 10 for net),
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
# Chunk id = cycle * IDX_BASE + spec-index; 10 fits balanced/deep, the
# net profile's 20-spec cycle needs 100.
IDX_BASE=10
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
        # The v2 mixture (probe 2026-11-25, docs/specs/2026-11-25-exploration-probe.md):
        # exploration noise was the mine-stepper — T=1.0 games ended fatally
        # at ply ~10; gentle/no exploration tripled game length and balanced
        # colors. Per 20 chunks: 10 without exploration (the backbone),
        # 5 with a gentle 5-ply window (T=0.2, eps=0.10), 5 with a
        # temperature-only 10-ply whisper (T=0.2, eps=0). Sims stay ~65%
        # asymmetric for the value gradient. Fields: black-sims white-sims
        # exploration-plies temperature epsilon.
        SPECS="800 800 0 0 0
800 800 0 0 0
400 400 0 0 0
400 800 0 0 0
400 800 0 0 0
400 800 0 0 0
400 800 0 0 0
800 400 0 0 0
800 400 0 0 0
800 400 0 0 0
800 800 5 0.2 0.10
800 800 5 0.2 0.10
400 800 5 0.2 0.10
800 400 5 0.2 0.10
800 400 5 0.2 0.10
800 800 10 0.2 0
400 400 10 0.2 0
400 800 10 0.2 0
400 800 10 0.2 0
800 400 10 0.2 0"
        DEFAULT_OUT=../data/corpora/selfplay-v2-$(basename "$NET")
        DEFAULT_CYCLES=999
        DEFAULT_WORKERS=14
        DEFAULT_CHUNK_GAMES=10
        # v2 self-play seeds live in their own range (v1 used 200M).
        SEED_OFFSET=300000000
        # 20 specs per cycle need two digits of index in the chunk id.
        IDX_BASE=100
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
    local w=$1 sb=$2 sw=$3 cycle=$4 idx=$5 plies=$6 temp=$7 eps=$8
    local c=$(( cycle * IDX_BASE + idx ))
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
    # Exploration flags ride only when the spec line carries them;
    # two-field spec lines (balanced/deep) keep the binary defaults.
    local exp_args=()
    if [ -n "$plies" ]; then
        exp_args=(--exploration-plies "$plies" --exploration-temperature "$temp" --exploration-epsilon "$eps")
    fi
    ./target/release/selfplaygen \
        ${NET_ARGS[@]+"${NET_ARGS[@]}"} \
        ${exp_args[@]+"${exp_args[@]}"} \
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
        echo "$SPECS" | while read -r sb sw plies temp eps; do
            run_chunk "$w" "$sb" "$sw" "$cycle" "$idx" "$plies" "$temp" "$eps"
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
