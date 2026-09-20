# openings/ — Swap2 opening lists

Committed opening lists for the Swap2 procedure (ch. 14 §3.2, tier 1).
All files use the same format: one opening per line, comma-separated
`x,y` coordinate pairs relative to the board center (so `0,0` is the
center cell (7,7) in 0-based row,col terms). Three pairs = a Swap2
offer (two Black + one White, in play order); five pairs = a full
opening after the "place two more" branch. Some Gomocup lines are
longer — deeper pre-set starting positions used in specific
tournaments; filter on pair count when consuming.

## Files and sources

| File | Lines | Source |
|---|---|---|
| `piskvork-openings.txt` | 41 | Piskvork tournament manager, `openings.txt` — https://raw.githubusercontent.com/plastovicka/Piskvork/master/openings.txt |
| `gomocup-2024-freestyle15.txt` | 12 | Gomocup 2024 results archive (`openings/openings_freestyle15.txt`) — https://gomocup.org/static/tournaments/2024/results/gomocup2024results.zip |
| `gomocup-2025-freestyle15.txt` | 12 | Gomocup 2025 results archive (same inner path) — https://gomocup.org/static/tournaments/2025/results/gomocup2025results.zip |
| `gomocup-2026-freestyle15.txt` | 12 | Gomocup 2026 results archive (`openings_freestyle15_piskvork.txt`) — https://gomocup.org/static/tournaments/2026/results/gomocup2026results.zip |

Downloaded 2026-09-20. License: none stated by the sources; these are
coordinate facts republished for examination. See `data/README.md`
for the licensing rule.

These openings encode engine-tournament balance selection — they are
the day-one offer pool for the self-play opening procedure.
