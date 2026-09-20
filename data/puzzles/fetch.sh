#!/usr/bin/env bash
# Download the VCF puzzle sets (local-only, data rights unclear — see README).
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p files
base="https://raw.githubusercontent.com/gugujiao953-ship-it/banbu-gomoku/main/public/puzzles"
curl -sS --max-time 60 -o files/vcf-material-763.json "$base/vcf-material.json"
curl -sS --max-time 60 -o files/kaibao-vcf-1052.json \
    "$base/kaibao/%E5%AE%9E%E6%88%98VCF_1052%E9%A2%98.json"
curl -sS --max-time 60 -o files/renju-portal-vcf-4024.json \
    "$base/kaibao/RenjuPortalVCF.json"

# format verified 2026-09-20
printf 'vcf-material-v1\n' > files/vcf-material-763.parser

echo "done: $(ls files/ | tr '\n' ' ')"
