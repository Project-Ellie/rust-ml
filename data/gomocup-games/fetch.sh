#!/usr/bin/env bash
# Download Gomocup tournament archives and extract Freestyle15 PSQ games.
# Usage: ./fetch.sh [YEAR...]   (default: 2026)
set -euo pipefail
cd "$(dirname "$0")"
YEARS="${@:-2026}"
for y in $YEARS; do
    zip="gomocup${y}results.zip"
    url="https://gomocup.org/static/tournaments/${y}/results/${zip}"
    echo "== $y: $url"
    curl -sSL --max-time 600 -o "/tmp/$zip" "$url"
    mkdir -p "games/$y"
    # 2024/2025 nest games under Freestyle15_*; 2026 uses the same layout.
    unzip -o -q "/tmp/$zip" "Freestyle15_1/*" "Freestyle15_2/*" -d "games/$y"
    rm "/tmp/$zip"
    echo "   $(find "games/$y" -name '*.psq' | wc -l | tr -d ' ') games extracted"
done
