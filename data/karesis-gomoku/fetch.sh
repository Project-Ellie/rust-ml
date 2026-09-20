#!/usr/bin/env bash
# Download the Karesis/Gomoku dataset (MIT) from HuggingFace.
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p files
base="https://huggingface.co/datasets/Karesis/Gomoku/resolve/main/gomoku_dataset_split/full"
for f in board_states.npy next_moves_coords.npy next_moves_players.npy; do
    curl -sSL --max-time 300 -o "files/$f" "$base/$f"
done
curl -sSL --max-time 30 -o files/DATASET-README.md \
    "https://huggingface.co/datasets/Karesis/Gomoku/raw/main/README.md"
echo "done: $(ls files/ | tr '\n' ' ')"
