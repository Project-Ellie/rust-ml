# External data sources for the rust-ml Gomoku project

## Abstract

This report records the result of a directed web-research sweep for external data and tools that can support two upcoming design documents: the self-play **Swap2 opening procedure** and a **supervised curriculum** using outside training data. The search covered (1) Swap2 opening catalogues, (2) Gomoku game-record datasets, (3) tactical puzzle sets, (4) HuggingFace datasets and models, and (5) open-source engines usable as local data generators. Every positive claim below is paired with a directly HTTP-checked URL (status 200). Negative results are listed explicitly. The strongest usable finds are the **Gomocup tournament archives** (hundreds of thousands of PSQ games plus annual freestyle opening files), the **Piskvork `openings.txt`** (a compact Swap2 opening book), the **banbu-gomoku VCF material** (763 labelled tactical puzzles), the **Karesis/Gomoku** HuggingFace dataset (MIT, 26k positions), and the **Rapfi** engine (GPL-3.0, built-in selfplay and balanced-opening generation). Most other discovered material is either ruleset-incompatible (Renju), unlicensed, or blocked by authentication.

## Glossary

- **Freestyle Gomoku.** The ruleset used by the rust-ml project: 15×15 board, Black starts, five or more stones in a row win (overlines win), no forbidden moves, Swap2 opening.
- **Swap2.** The opening protocol in which Black places two black stones and one white stone; the second player then chooses to stay White, swap to Black, or place two more stones (one white, one black) and let Black choose the colour.
- **PSQ.** The plain-text game-record format written by the Piskvork manager: a header line (`Piskvorky 15x15, ...`) followed by `x,y,ms` move lines.
- **VCF / VCT.** Victory by Continuous Four / Victory by Continuous Threat: standard Gomoku/Renju terminology for forcing tactical sequences.
- **Renju.** A related ruleset with forbidden moves for Black and exact-five wins; positions from Renju sources must be re-adjudicated under freestyle rules before use.
- **Ruleset mismatch.** Any case where a source was generated under a ruleset different from the project's freestyle Gomoku (most commonly Renju or exact-five Gomoku).

## 1. Swap2 opening catalogues

### Verified sources

**Piskvork `openings.txt`.** The reference Gomocup manager ships a 41-line plain-text opening file. Each line is a sequence of comma-separated `(x,y)` coordinate pairs relative to the board centre; three-pair lines are Swap2 offers and five-pair lines are full Swap2 positions after the "add two stones" option. Direct raw URL: `https://raw.githubusercontent.com/plastovicka/Piskvork/master/openings.txt` (HTTP 200, 898 bytes). License: none declared in the repository. Fit: direct input to the project's Swap2 opening logic.

**Gomocup freestyle opening files.** The annual Gomocup result archives (2024, 2025, 2026) each contain a `openings_freestyle15.txt` file with 12 lines of comma-separated coordinates. Archive URLs return HTTP 200:
- 2024: `https://gomocup.org/static/tournaments/2024/results/gomocup2024results.zip` (46.8 MB)
- 2025: `https://gomocup.org/static/tournaments/2025/results/gomocup2025results.zip` (61.5 MB)
- 2026: `https://gomocup.org/static/tournaments/2026/results/gomocup2026results.zip` (76.0 MB)

The 2026 file is the cleanest (mostly 3- and 5-pair lines). Caveat: these are automatic tournament opening moves, not the interactive Swap2 decision protocol, and line lengths vary (some contain 7–25 pairs representing deeper starting positions). License: not stated.

**Renju.net Gomoku rules.** `https://www.renju.net/gomokurules/` (HTTP 200) gives the authoritative Swap2 procedure wording, but no catalogue of concrete positions. The same site's Swap2-specific page is `https://www.renju.net/rule/11/` (HTTP 200). Caveat: RIF Gomoku rules treat overlines as non-winning, so the pages are useful for procedure text only.

### Negative results

- `https://gomocup.org/opening-rules/` → 404.
- `http://gomocup.com/` and `https://gomocup.com/` → DNS error (domain does not resolve).
- `https://www.playok.com/piskvorky/` and `/en/piskvorky/` → 404.
- `https://www.kurnik.org/` → SSL certificate mismatch, unverifiable.
- GitHub code search for Swap2 opening files → 401 (authentication required).
- `https://boardgamegeek.com/forum/114/gomoku` → 403 Cloudflare challenge.

## 2. Gomoku game-record datasets

### Verified sources

**Gomocup tournament archives.** The Gomocup results page (`https://gomocup.org/results/`, HTTP 200) links to yearly ZIP archives from 2000 through 2026. The archives contain per-game `.psq` files grouped by ruleset (Freestyle15, Freestyle20, Standard, Caro, Renju, etc.). Verified counts: 2024 = 39,048 PSQ files; 2025 = 44,400; 2026 = 55,224. Total compressed size 2010–2026 is approximately 419 MB. License: not stated. Fit: the largest verified supervised-pretraining and sparring-data source; restrict to `Freestyle15_*` subdirectories to match the project's ruleset.

**HuggingFace `Karesis/Gomoku`.** `https://huggingface.co/datasets/Karesis/Gomoku` (HTTP 200). MIT-licensed dataset of 875 self-play games / 26,378 15×15 board-state/next-move positions generated with the WinePy alpha-beta engine, distributed as NumPy `.npy`, JSON, CSV and PKL. DOI: `10.57967/hf/4816`. Fit: small, clean, permissively licensed supervised pretraining supplement.

**HuggingFace `PoolC/gomoku-dataset-1.8M`.** `https://huggingface.co/datasets/PoolC/gomoku-dataset-1.8M` (HTTP 200). Single Parquet file with 1.88M rows and one feature column `input_ids` (int32 sequence); 91 MB download / ~334 MB uncompressed. License and token encoding are undocumented, making the dataset unusable without reverse engineering.

### Negative results

- **renju.net game database** (`https://www.renju.net/game/search/`, HTTP 200) has no bulk-download link; probed `/media/games/games.zip` and `/media/games/renju.zip` → 404; `/api/games/` → 401.
- **Kaggle "greedy gomoku"** (`https://www.kaggle.com/datasets/hauuto/greedy-gomoku`) metadata reports ~26.6 GB and "Unknown" license; download endpoint redirects to authentication → not verifiable anonymously.
- GitHub code search for `.psq`/`.sgf` Gomoku archives → 401 (authentication required).
- Zenodo, Figshare, LittleGolem, BoardGameGeek → no usable anonymous bulk Gomoku record archive found.

## 3. Tactical puzzle sets

### Verified sources

**banbu-gomoku VCF material.** `https://raw.githubusercontent.com/gugujiao953-ship-it/banbu-gomoku/main/public/puzzles/vcf-material.json` (HTTP 200, 763 puzzles, ~300 KB). This is the only verified public set that ships labelled VCF solution lines; each record contains attacker colour, depth, stone lists and the forced-win line. License: MIT for the repository code; third-party puzzle rights noted as belonging to original owners. Caveat: no explicit ruleset tag; re-verify under freestyle overlines-win.

**banbu-gomoku kaibao collections.** Both files return HTTP 200 but contain no stored answers:
- `实战VCF_1052题.json`: ~1,050 positions (`https://raw.githubusercontent.com/gugujiao953-ship-it/banbu-gomoku/main/public/puzzles/kaibao/实战VCF_1052题.json`).
- `RenjuPortalVCF.json`: 4,024 positions (`https://raw.githubusercontent.com/gugujiao953-ship-it/banbu-gomoku/main/public/puzzles/kaibao/RenjuPortalVCF.json`).

These can seed a puzzle generator if an internal VCF/VCT solver is run to produce labels.

**lfz084/renju puzzle JSON archive.** `https://github.com/lfz084/renju/tree/master/puzzle/json` (HTTP 200). Large collection including 黑先VCF (504 puzzles), 白先VCF (548 puzzles), mate-in-3 sets and exercises. No explicit answers; Renju-oriented.

**renju-benchmark synthetic tactical tests.** `https://raw.githubusercontent.com/Tk-visionary/renju-benchmark/main/data/puzzles.jsonl` (HTTP 200, 8 labelled positions, GPL-3.0). Explicitly Renju-rule positions; useful as a correctness/edge-case unit-test set, not as a training corpus.

### Negative results

- All `https://www.renju.net/{training,puzzles,problems,vcf,vct,study,exercises}` → 404.
- `https://www.renju.net/downloads/` → 200 but links only to engines/apps, not puzzle downloads.
- `https://gomocup.org/forum/` → 404.
- `gomokuworld.com` → SSL certificate error, unverifiable.

## 4. HuggingFace datasets and models

### Verified datasets

- **`Karesis/Gomoku`** — MIT, 26k positions, 15×15 freestyle. Strong fit for supervised pretraining.
- **`PoolC/gomoku-dataset-1.8M`** — 1.88M rows, license/encoding unknown. Fit is conditional on reverse-engineering.
- **`eganscha/gomoku_vlm_ds`** — synthetic vision-language instruction data; not move-level game records; not aligned with AlphaZero-style training.
- General board-game QA datasets (`Boardgame-QA`, `BoardGameBench`, etc.) contain natural-language reasoning, not Gomoku positions.

### Verified models

- **`maojh15/GomokuZeroAI`** (`https://huggingface.co/maojh15/GomokuZeroAI`, HTTP 200). MIT-licensed AlphaZero-style 15×15 policy-value checkpoint.
- **`Nagi-ovo/alphazero-gomoku`** (`https://huggingface.co/Nagi-ovo/alphazero-gomoku`, HTTP 200). MIT-licensed AlphaZero-style Gomoku model.
- Several other models are 9×9/13×13, Renju-oriented, or language-model adapters and are not directly applicable.

### Negative search results (HuggingFace API returned zero hits)

- Datasets for `renju`, `"five in a row"`, `connect6`.
- Models for `"five in a row"`, `connect6`.

## 5. Engines as data generators

### Verified engines

**Rapfi.** `https://github.com/dhbloo/rapfi` (HTTP 200), GPL-3.0, C++, 273 stars. One of the strongest public engines; Gomocup Freestyle15 Elo 2716 (2025). Supports a `selfplay` command that emits `txt/bin/bin_lz4/binpack/binpack_lz4` training samples and an `opengen` command for balanced openings. Network weights are in the separate `dhbloo/rapfi-networks` repository under CC0-1.0. Best all-around choice for both sparring and synthetic data.

**AlphaGomoku.** `https://github.com/MaciejKozarzewski/AlphaGomoku` (HTTP 200), GPL-3.0, C++. Full AlphaZero implementation with MCTS, networks, selfplay and training front-ends; supports Swap2 and other opening controllers. Gomocup Freestyle15 Elo 2613 (2026).

**KataGomo.** `https://github.com/hzyhhzy/KataGomo` (HTTP 200), custom MIT-style license. KataGo fork trained for Gomoku/Renju; strong open-source engine with released binaries and KataGo selfplay/training scripts. Gomocup Freestyle15 Elo 2696 (2026).

**c-gomoku-cli.** `https://github.com/nkg114mc/c-gomoku-cli` (HTTP 200), GPL-3.0. Headless tournament manager for any Gomocup-protocol engine; supports fixed opening files, concurrent games, and outputs SGF plus CSV/binary training samples (`-sample freq=... format=csv`). The key orchestration layer for turning any engine into a batch data generator.

**Other usable engines.** All verified on GitHub with HTTP 200:
- `dhbloo/Rapfi-gomocup` — legacy MIT Rapfi (weaker but permissive).
- `wind23/SlowRenju` — GPL-3.0, mid-tier alpha-beta.
- `schibir/PentaZen` — GPL-3.0, strong alpha-beta (2021 version).
- `ChisBread/Chis` — MPL-2.0, mid-tier.
- `Joker2770/Z2I` — MIT, AlphaZero-style MCTS+ONNX with training scripts.
- `winterdl/Nut-engine` — Apache-2.0, mid-tier Gomocup engine.

### Negative results

- `https://api.github.com/repos/sun-yuliang/PentaZen` → 404 (old Gomocup link; current fork is `schibir/PentaZen`).
- `https://api.github.com/repos/jinjiebang/wine` → 404 (Wine engine is closed-source).
- `https://sourceforge.net/projects/piskvork` → 403 (direct curl blocked); the project is mirrored at `plastovicka/Piskvork` on GitHub.

## Fit summary

| Source | Opening procedure | Supervised pretraining | Puzzle anchor set | Sparring / data generator |
|---|---|---|---|---|
| Piskvork `openings.txt` | yes | no | maybe | no |
| Gomocup result archives | yes (opening files) | yes (PSQ games) | maybe (root positions) | yes |
| `Karesis/Gomoku` (HF) | no | yes | maybe | no |
| `PoolC/gomoku-dataset-1.8M` | no | maybe | no | no |
| banbu VCF material | no | maybe | yes | no |
| banbu kaibao / RenjuPortal / lfz084 | no | maybe | yes (after labelling) | no |
| `maojh15/GomokuZeroAI` | no | maybe (init) | no | yes |
| `Nagi-ovo/alphazero-gomoku` | no | maybe (init) | no | yes |
| Rapfi | maybe (opengen) | yes (selfplay) | yes (analysis) | yes |
| AlphaGomoku / KataGomo | yes | yes | yes | yes |
| c-gomoku-cli | yes (feeds openings) | yes (samples) | no | yes (orchestrator) |

## License and redistribution caveats

- **Gomocup archives** and **Piskvork `openings.txt`** have no stated license. They are publicly distributed for download, but redistribution of derived datasets should be documented carefully.
- **Renju-derived puzzle sets** are overwhelmingly third-party content; even when the wrapper repository is MIT, the puzzle data itself may carry separate rights.
- **GPL-3.0 / MPL-2.0 engines** are copyleft at the file or project level. Using them as external black-box sparring partners does not infect the rust-ml codebase, but linking or embedding their code would.
- **Apache-2.0 (Nut-engine)** and **MIT (legacy Rapfi, Z2I)** engines are the most permissive options for closer integration if needed.
- **HuggingFace datasets** with `license: unknown` should not be redistributed without clarifying the license.

## References

- [catalogue.md](catalogue.md) — canonical entries for all sources above, generated from [generate.py](generate.py).
- Piskvork `openings.txt`: `https://raw.githubusercontent.com/plastovicka/Piskvork/master/openings.txt`
- Gomocup results archives: `https://gomocup.org/results/`
- Renju.net international Gomoku rules (Swap2): `https://www.renju.net/gomokurules/`
- HuggingFace `Karesis/Gomoku`: `https://huggingface.co/datasets/Karesis/Gomoku`
- HuggingFace `PoolC/gomoku-dataset-1.8M`: `https://huggingface.co/datasets/PoolC/gomoku-dataset-1.8M`
- banbu-gomoku VCF material: `https://raw.githubusercontent.com/gugujiao953-ship-it/banbu-gomoku/main/public/puzzles/vcf-material.json`
- lfz084/renju puzzles: `https://github.com/lfz084/renju/tree/master/puzzle/json`
- renju-benchmark: `https://raw.githubusercontent.com/Tk-visionary/renju-benchmark/main/data/puzzles.jsonl`
- HuggingFace `maojh15/GomokuZeroAI`: `https://huggingface.co/maojh15/GomokuZeroAI`
- HuggingFace `Nagi-ovo/alphazero-gomoku`: `https://huggingface.co/Nagi-ovo/alphazero-gomoku`
- Rapfi engine: `https://github.com/dhbloo/rapfi`
- AlphaGomoku engine: `https://github.com/MaciejKozarzewski/AlphaGomoku`
- KataGomo engine: `https://github.com/hzyhhzy/KataGomo`
- c-gomoku-cli: `https://github.com/nkg114mc/c-gomoku-cli`
- Gomocup Elo ratings (strength claims): `https://gomocup.org/elo-ratings/`
