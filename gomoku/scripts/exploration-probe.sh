#!/bin/bash
# Exploration-schedule probe: 6 configs x 2 workers, chunks of 5 games.
#
# Mirror self-play with mass-default-v1 (or NET=...) at 800/800 sims,
# sweeping exploration-plies / temperature / epsilon.  Every finished
# chunk is a sealed corpus (manifest.json written last by selfplaygen).
# The run is open-ended: kill and rerun the same command to resume.
#
# Seeds are assigned per config/worker/chunk and are never reused on
# resume; an interrupted chunk directory is abandoned rather than
# regenerated so that its seed is not repeated.
#
# Usage:
#   scripts/exploration-probe.sh                 # default NET
#   NET=<dir> scripts/exploration-probe.sh       # custom checkpoint
#   DRYRUN=1 scripts/exploration-probe.sh        # print first chunks
#   nohup scripts/exploration-probe.sh &         # detached

set -u

# Do not let the Mac sleep mid-run.
if [ -z "${CAFFEINATED:-}" ]; then
    exec caffeinate -dims env CAFFEINATED=1 "$0" "$@"
fi

# Resolve paths from the script location: gomoku/scripts/ -> gomoku/.
cd "$(dirname "$0")/.." || exit 1

NET=${NET:-../data/check-points/mass-default-v1}
OPENINGS=${OPENINGS:-../data/openings/gomocup-2024-freestyle15.txt}
OUT=${OUT:-../data/corpora/probe-exploration}
WORKERS_PER_CONFIG=${WORKERS_PER_CONFIG:-2}
CHUNK_GAMES=${CHUNK_GAMES:-5}
SEED_OFFSET=${SEED_OFFSET:-400000000}

# Parallel indexed variables (macOS /bin/bash is 3.2 — no associative arrays).
CONFIG_NAMES=(A0 B5-02 C10-02 D10-05 E20-02 F10-pure)
CONFIG_PLIES=(0  5     10     10     20     10)
CONFIG_TEMP=(0  0.2   0.2    0.5    0.2    0.2)
CONFIG_EPS=(0   0.10  0.10   0.10   0.10   0)

N_CONFIGS=${#CONFIG_NAMES[@]}
TOTAL_WORKERS=$((N_CONFIGS * WORKERS_PER_CONFIG))

DRYRUN=${DRYRUN:-0}

if [ "$DRYRUN" != "1" ]; then
    if [ ! -f "$NET/model.mpk" ]; then
        echo "exploration-probe: NET '$NET' has no model.mpk" >&2
        exit 2
    fi
    if [ ! -f "$OPENINGS" ]; then
        echo "exploration-probe: openings file not found: $OPENINGS" >&2
        exit 2
    fi

    echo "exploration-probe: building selfplaygen (release)..."
    cargo build --release -p selfplay || exit 1
fi

# Find the next chunk index for a given config/worker.  We look at all
# existing chunk directories (regardless of whether they sealed) and
# start one past the highest, guaranteeing unique seeds across resume.
next_chunk_for() {
    local cfg=$1 cw=$2
    local prefix="$OUT/$cfg-w$(printf %02d "$cw")-c"
    local max=-1
    local d tail n

    shopt -s nullglob
    for d in "${prefix}"*/; do
        [ -d "$d" ] || continue
        tail=${d#"$prefix"}
        tail=${tail%/}
        n=0
        if [ -n "$tail" ]; then
            n=$((10#$tail))
        fi
        if [ "$n" -gt "$max" ]; then
            max=$n
        fi
    done
    shopt -u nullglob

    if [ "$max" -lt 0 ]; then
        echo 0
    else
        echo $((max + 1))
    fi
}

run_chunk() {
    local ci=$1 cw=$2 chunk=$3
    local cfg=${CONFIG_NAMES[$ci]}
    local plies=${CONFIG_PLIES[$ci]}
    local temp=${CONFIG_TEMP[$ci]}
    local eps=${CONFIG_EPS[$ci]}
    local seed=$(( SEED_OFFSET + ci * 10000000 + cw * 1000000 + chunk * 100 ))
    local dir
    dir=$(printf '%s/%s-w%02d-c%02d' "$OUT" "$cfg" "$cw" "$chunk")
    local gw=$(( ci * WORKERS_PER_CONFIG + cw ))

    if [ -f "$dir/manifest.json" ]; then
        return 0
    fi
    if [ -d "$dir" ]; then
        # Partial/interrupted directory: abandon it so the seed is not
        # reused.  At most CHUNK_GAMES games are lost per interruption.
        return 0
    fi

    local extra=()
    if [ "$cfg" != "A0" ]; then
        extra=(--exploration-temperature "$temp" --exploration-epsilon "$eps")
    fi

    if [ "$DRYRUN" = "1" ]; then
        echo "./target/release/selfplaygen --black-net \"$NET\" --white-net \"$NET\" --openings \"$OPENINGS\" --black-sims 800 --white-sims 800 --games $CHUNK_GAMES --seed-base $seed --exploration-plies $plies ${extra[@]+"${extra[@]}"} --max-plies 50 --out \"$dir\""
        return 0
    fi

    local started elapsed rc
    started=$(date +%s)
    ./target/release/selfplaygen \
        --black-net "$NET" --white-net "$NET" \
        --openings "$OPENINGS" \
        --black-sims 800 --white-sims 800 \
        --games "$CHUNK_GAMES" \
        --seed-base "$seed" \
        --exploration-plies "$plies" \
        ${extra[@]+"${extra[@]}"} \
        --max-plies 50 \
        --out "$dir" >> "$OUT/logs/w$(printf %02d "$gw").log" 2>&1
    rc=$?
    elapsed=$(( $(date +%s) - started ))
    if [ $rc -eq 0 ]; then
        echo "$(date '+%H:%M:%S') $cfg-w$(printf %02d "$cw") c$(printf %02d "$chunk") ok ${elapsed}s" >> "$OUT/progress.log"
    else
        echo "$(date '+%H:%M:%S') $cfg-w$(printf %02d "$cw") c$(printf %02d "$chunk") FAILED rc=$rc" >> "$OUT/progress.log"
    fi
}

worker() {
    local ci=$1 cw=$2
    local chunk
    chunk=$(next_chunk_for "${CONFIG_NAMES[$ci]}" "$cw")
    while true; do
        run_chunk "$ci" "$cw" "$chunk"
        chunk=$((chunk + 1))
    done
}

summary() {
    echo
    echo "exploration-probe: summary at $(date '+%H:%M:%S')"
    local ci cfg sealed interrupted total d
    total=0
    for ci in $(seq 0 $((N_CONFIGS - 1))); do
        cfg=${CONFIG_NAMES[$ci]}
        sealed=0
        interrupted=0
        shopt -s nullglob
        for d in "$OUT/$cfg-w"??"-c"*/; do
            [ -d "$d" ] || continue
            if [ -f "$d/manifest.json" ]; then
                sealed=$((sealed + 1))
            else
                interrupted=$((interrupted + 1))
            fi
        done
        shopt -u nullglob
        printf '  %-8s sealed=%3d interrupted=%3d\n' "$cfg" "$sealed" "$interrupted"
        total=$((total + sealed))
    done
    echo "  total sealed chunks: $total"
}

if [ "$DRYRUN" = "1" ]; then
    echo "exploration-probe: DRYRUN=1 — planned first chunks (no build, no I/O)"
    echo "exploration-probe: NET=$NET OUT=$OUT"
    for ci in $(seq 0 $((N_CONFIGS - 1))); do
        for cw in 0 1; do
            chunk=$(next_chunk_for "${CONFIG_NAMES[$ci]}" "$cw")
            run_chunk "$ci" "$cw" "$chunk"
        done
    done
    summary
    exit 0
fi

mkdir -p "$OUT/logs"

echo "exploration-probe: $N_CONFIGS configs x $WORKERS_PER_CONFIG workers = $TOTAL_WORKERS workers"
echo "exploration-probe: NET=$NET, OUT=$OUT, progress in $OUT/progress.log"
echo "exploration-probe: kill and rerun the same command to resume"

trap summary EXIT

for ci in $(seq 0 $((N_CONFIGS - 1))); do
    for cw in 0 1; do
        worker "$ci" "$cw" &
    done
done
wait
