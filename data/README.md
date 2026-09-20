# data/ — external data, local copies

External datasets and opening lists, downloaded for examination and
(later) training-pipeline use. Sources, formats, and license status
are documented per subfolder; the canonical citations live in
`docs/references/catalogue.md`, and the research sweep that found
them is `docs/references/findings-external-data.md`.

## Layout

| Subfolder | Content | In git? |
|---|---|---|
| `openings/` | Swap2 opening lists (Piskvork + Gomocup 2024–2026) | **committed** (tiny) |
| `gomocup-games/` | Gomocup Freestyle15 PSQ game records (2026: 12,984 games) | bulk **ignored**, `fetch.sh` regenerates |
| `karesis-gomoku/` | HuggingFace `Karesis/Gomoku` (26,378 labelled positions, MIT) | bulk **ignored**, `fetch.sh` regenerates |
| `puzzles/` | VCF puzzle sets (763 labelled + ~5k unlabelled) | bulk **ignored**, `fetch.sh` regenerates |

## Licensing rule (ch. 14, proposal D3)

Only the tiny opening lists are committed, and they are coordinate
facts, not creative work. Everything else stays **local-only**:
publicly downloadable but unlicensed or unclear-licensed material is
never redistributed through this repository. To rebuild the local
copies from scratch, run each subfolder's `fetch.sh`.

## Ruleset warning

Our rules are freestyle 15×15 (overlines win, no forbidden moves —
ch. 13, decision 1). Data from Renju-oriented sources must be
re-adjudicated before any training use (ch. 14, proposal D5).
